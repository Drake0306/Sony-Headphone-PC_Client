//! The broker thread: owns the transport and the `Session`, applies UI
//! commands and publishes every state change.

use std::borrow::Borrow;
use std::io;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mdr_proto::{request, NcAsmState, UltLevel};
use mdr_session::{DeviceState, Event, PairedDevice, Session, SessionError};
use mdr_transport::{connect, discover, FakeDevice, Transport};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio_macos::current_output_device_name;

/// General setting id of multipoint.
const GS_MULTIPOINT: u8 = 0xD1;
const RECV_TIMEOUT: Duration = Duration::from_millis(10);
const RECONNECT_EVERY: Duration = Duration::from_secs(5);
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
}

/// Runs one connection until `Disconnect` (or the command channel closing)
/// returns `Ok`, or the link is lost, which returns `Err` after emitting the
/// disconnected state.
///
/// `cmds` is borrowed or owned so the app can keep one receiver across reconnects.
pub fn run_broker<E: Fn(&DeviceState)>(
    mut transport: Box<dyn Transport>,
    cmds: impl Borrow<Receiver<BrokerCmd>>,
    emit: E,
    mut clock: impl FnMut() -> u64,
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

        while let Some(frame) = session.poll_output() {
            if let Err(e) = transport.write(&frame) {
                return Err(link_lost(&mut session, &emit, e));
            }
        }

        let mut events = Vec::new();
        match transport.recv_timeout(RECV_TIMEOUT) {
            Ok(Some(bytes)) => events.extend(session.handle_input(&bytes, clock())),
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
                Event::Log(msg) => eprintln!("hairline: {msg}"),
            }
        }
        if changed {
            emit(session.state());
        }
        if gave_up {
            transport.close();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "device stopped acknowledging"));
        }
    }
}

fn apply(session: &mut Session, cmd: BrokerCmd) -> Result<(), SessionError> {
    match cmd {
        BrokerCmd::SetNc(s) => session.set_nc(s),
        BrokerCmd::SetUlt(u) => session.set_ult(u),
        BrokerCmd::SetMultipoint(on) => session.request(request::gs_set_bool(GS_MULTIPOINT, on)),
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
    vec![PairedDevice { name: "This Mac".into(), playing }]
}

/// Starts the broker thread: connects (or uses the fake device), runs the
/// session, and retries every five seconds after a lost link.
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
            loop {
                let result = match open_transport(fake) {
                    Ok(t) => run_broker(t, &rx, emit, clock),
                    Err(e) => Err(e),
                };
                let next = match result {
                    Ok(()) => wait_for_connect(&rx, None),
                    Err(e) => {
                        eprintln!("hairline: no link ({e}); retrying in {}s", RECONNECT_EVERY.as_secs());
                        wait_for_connect(&rx, Some(RECONNECT_EVERY))
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
    let candidates = discover()?;
    let pick = candidates
        .iter()
        .find(|c| c.name.contains("ULT WEAR") || c.name.contains("Sony ULT"))
        .or(candidates.first())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no paired device exposes the v2 service"))?;
    connect(&pick.address)
}
