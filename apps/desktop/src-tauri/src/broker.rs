//! The broker thread: owns the transport and the `Session`, applies UI
//! commands and publishes every state change.

use std::borrow::Borrow;
use std::io;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mdr_proto::{request, Group, NcAsmState, Request, Table, UltLevel};
use mdr_session::{DeviceState, Event, PairedDevice, Session, SessionError};
use mdr_transport::{connect, discover, is_ult_wear_name, FakeDevice, Transport};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio_macos::current_output_device_name;
use crate::frame_log::FrameLog;

/// General setting id of multipoint.
const GS_MULTIPOINT: u8 = 0xD1;
const RECV_TIMEOUT: Duration = Duration::from_millis(10);
/// First wait, in seconds, after a failed connect; it doubles up to the maximum.
const BACKOFF_START_S: u64 = 5;
const BACKOFF_MAX_S: u64 = 60;
/// How often the host's default audio output is re-read.
const DEVICES_EVERY_MS: u64 = 1000;

#[derive(Debug, Clone, PartialEq)]
pub enum BrokerCmd {
    Connect,
    Disconnect,
    SetNc(NcAsmState),
    SetUlt(UltLevel),
    SetMultipoint(bool),
    Refresh,
    /// A hand-written request (`HAIRLINE_DEBUG=1` only); still deny-listed.
    Raw {
        table: Table,
        bytes: Vec<u8>,
    },
}

/// Builds `BrokerCmd::Raw` from a table name (`t1`/`t2`) and hex bytes, which
/// may be separated by spaces, colons or dashes.
pub fn parse_raw(table: &str, hex: &str) -> Result<BrokerCmd, String> {
    let table = match table.to_ascii_lowercase().as_str() {
        "t1" => Table::T1,
        "t2" => Table::T2,
        other => return Err(format!("unknown table {other:?}; use t1 or t2")),
    };
    let digits: Vec<char> = hex
        .chars()
        .filter(|c| !matches!(c, ' ' | ':' | '-'))
        .collect();
    if digits.is_empty() || digits.len() % 2 != 0 {
        return Err(format!("{hex:?} is not a whole number of hex bytes"));
    }
    let bytes = digits
        .chunks(2)
        .map(|pair| {
            let pair: String = pair.iter().collect();
            u8::from_str_radix(&pair, 16).map_err(|_| format!("{pair:?} is not a hex byte"))
        })
        .collect::<Result<_, _>>()?;
    Ok(BrokerCmd::Raw { table, bytes })
}

/// Runs one connection until `Disconnect` (or the command channel closing)
/// returns `Ok`, or the link is lost, which returns `Err` after emitting the
/// disconnected state.
///
/// `cmds` is borrowed or owned so the app can keep one receiver across reconnects.
pub fn run_broker<E: Fn(&DeviceState)>(
    transport: Box<dyn Transport>,
    cmds: impl Borrow<Receiver<BrokerCmd>>,
    emit: E,
    clock: impl FnMut() -> u64,
) -> io::Result<()> {
    run_broker_logged(transport, cmds, emit, clock, None)
}

/// As `run_broker`, also writing every frame and log line to `log`.
pub fn run_broker_logged<E: Fn(&DeviceState)>(
    mut transport: Box<dyn Transport>,
    cmds: impl Borrow<Receiver<BrokerCmd>>,
    emit: E,
    mut clock: impl FnMut() -> u64,
    mut log: Option<&mut FrameLog>,
) -> io::Result<()> {
    let cmds = cmds.borrow();
    let mut session = Session::new(clock());
    session.start();
    let mut devices_checked_at: Option<u64> = None;
    loop {
        let mut changed = false;

        let now = clock();
        if devices_checked_at.is_none_or(|t| now >= t + DEVICES_EVERY_MS) {
            devices_checked_at = Some(now);
            let devices = paired_devices();
            if devices != session.state().devices {
                session.set_devices(devices);
                changed = true;
            }
        }

        let now = clock();
        while let Some(frame) = session.poll_output(now) {
            if let Some(log) = log.as_deref_mut() {
                log.sent(now, &frame);
            }
            if let Err(e) = transport.write(&frame) {
                return Err(link_lost(&mut session, &emit, e));
            }
        }

        let mut events = Vec::new();
        match transport.recv_timeout(RECV_TIMEOUT) {
            Ok(Some(bytes)) => {
                let now = clock();
                if let Some(log) = log.as_deref_mut() {
                    log.received(now, &bytes);
                }
                events.extend(session.handle_input(&bytes, now));
            }
            Ok(None) => {}
            Err(e) => return Err(link_lost(&mut session, &emit, e)),
        }
        let now = clock();
        if session.next_timeout_ms().is_some_and(|t| now >= t) {
            events.extend(session.handle_timeout(now));
        }

        loop {
            match cmds.try_recv() {
                Ok(BrokerCmd::Disconnect) | Err(TryRecvError::Disconnected) => {
                    transport.close();
                    session.mark_disconnected();
                    emit(session.state());
                    return Ok(());
                }
                Ok(cmd) => {
                    if let Err(e) = apply(&mut session, cmd.clone()) {
                        events.push(Event::Log(format!("{cmd:?} not sent: {e:?}")));
                    }
                    // Emitted even on failure so the UI drops any optimistic selection.
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        let mut gave_up = false;
        for event in &events {
            match event {
                Event::StateChanged | Event::Connected => changed = true,
                Event::Disconnected => {
                    changed = true;
                    gave_up = true;
                }
                Event::Log(msg) => {
                    eprintln!("hairline: {msg}");
                    if let Some(log) = log.as_deref_mut() {
                        log.note(clock(), msg);
                    }
                }
            }
        }
        if changed {
            emit(session.state());
        }
        if gave_up {
            transport.close();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "device stopped acknowledging",
            ));
        }
    }
}

fn apply(session: &mut Session, cmd: BrokerCmd) -> Result<(), SessionError> {
    match cmd {
        BrokerCmd::SetNc(s) => session.set_nc(s),
        BrokerCmd::SetUlt(u) => session.set_ult(u),
        BrokerCmd::SetMultipoint(on) => session.request(request::gs_set_bool(GS_MULTIPOINT, on)),
        BrokerCmd::Raw { table, bytes } => session.request(Request {
            table,
            bytes,
            capability: None,
            group: Group::Raw,
        }),
        BrokerCmd::Refresh => {
            session.request(request::battery_get())?;
            session.request(request::ncasm_get())?;
            session.request(request::eq_ult_get())
        }
        // Already connected; nothing to do.
        BrokerCmd::Connect | BrokerCmd::Disconnect => Ok(()),
    }
}

fn link_lost<E: Fn(&DeviceState)>(session: &mut Session, emit: &E, e: io::Error) -> io::Error {
    session.mark_disconnected();
    emit(session.state());
    e
}

/// The host's own entry in the Link list.
fn paired_devices() -> Vec<PairedDevice> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let playing = current_output_device_name().is_some_and(|n| n.contains("ULT WEAR"));
    vec![PairedDevice {
        name: "This Mac".into(),
        playing,
    }]
}

/// Seconds to wait before the next connect attempt, given the previous wait
/// (0 after a successful connect): 5, 10, 20, 40, then 60 for good.
pub fn next_backoff(prev: u64) -> u64 {
    prev.saturating_mul(2).clamp(BACKOFF_START_S, BACKOFF_MAX_S)
}

/// Starts the broker thread: connects (or uses the fake device), runs the
/// session, and retries with an exponential back-off after a failed connect or
/// a lost link. A `Connect` command cuts the wait short.
pub fn spawn_broker(app: AppHandle, fake: bool) -> Sender<BrokerCmd> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("hairline-broker".into())
        .spawn(move || {
            let start = Instant::now();
            let clock = move || start.elapsed().as_millis() as u64;
            let emit = |state: &DeviceState| {
                if let Ok(mut latest) = app.state::<Mutex<DeviceState>>().lock() {
                    *latest = state.clone();
                }
                let _ = app.emit("state", state);
            };
            let mut log = FrameLog::from_env();
            let mut backoff = 0;
            loop {
                let result = match open_transport(fake) {
                    Ok(t) => {
                        backoff = 0;
                        run_broker_logged(t, &rx, emit, clock, log.as_mut())
                    }
                    Err(e) => Err(e),
                };
                let next = match result {
                    Ok(()) => wait_for_connect(&rx, None),
                    Err(e) => {
                        backoff = next_backoff(backoff);
                        eprintln!("hairline: no link ({e}); retrying in {backoff}s");
                        wait_for_connect(&rx, Some(Duration::from_secs(backoff)))
                    }
                };
                if next.is_err() {
                    return;
                }
            }
        })
        .expect("spawn broker thread");
    tx
}

/// Blocks until `Connect` arrives or `timeout` passes; `Err` once the app is gone.
fn wait_for_connect(rx: &Receiver<BrokerCmd>, timeout: Option<Duration>) -> Result<(), ()> {
    let deadline = timeout.map(|t| Instant::now() + t);
    loop {
        let cmd = match deadline {
            Some(d) => match rx.recv_timeout(d.saturating_duration_since(Instant::now())) {
                Ok(cmd) => cmd,
                Err(RecvTimeoutError::Timeout) => return Ok(()),
                Err(RecvTimeoutError::Disconnected) => return Err(()),
            },
            None => rx.recv().map_err(|_| ())?,
        };
        if cmd == BrokerCmd::Connect {
            return Ok(());
        }
    }
}

fn open_transport(fake: bool) -> io::Result<Box<dyn Transport>> {
    if fake {
        return Ok(Box::new(FakeDevice::ult_wear()));
    }
    // Only a device already connected to this computer is tried, so a
    // reconnect attempt never pages headphones that are elsewhere.
    let candidates: Vec<_> = discover()?
        .into_iter()
        .filter(|c| c.connected_audio)
        .collect();
    let pick = candidates
        .iter()
        .find(|c| is_ult_wear_name(&c.name))
        .or(candidates.first())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no connected device exposes the v2 service",
            )
        })?;
    connect(&pick.address)
}
