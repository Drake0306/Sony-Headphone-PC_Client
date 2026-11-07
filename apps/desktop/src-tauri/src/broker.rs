//! The broker thread: owns the transport and the `Session`, applies UI
//! commands and publishes every state change.

use std::borrow::Borrow;
use std::io;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mdr_proto::{request, Group, NcAsmState, Request, Table, UltLevel};
use mdr_session::{DeviceState, Event, PairedDevice, Session, SessionError};
use mdr_transport::{connect, discover, is_ult_wear_name, Candidate, FakeDevice, Transport};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio_macos::current_output_device_name;
use crate::frame_log::FrameLog;
use crate::scenes::engine::{apply_actions, Engine};
use crate::scenes::model::{Actions, Inputs, Scene};
use crate::sensors_macos::{frontmost_app_name, mic_in_use};

const RECV_TIMEOUT: Duration = Duration::from_millis(10);
/// First wait, in seconds, after a failed connect; it doubles up to the maximum.
const BACKOFF_START_S: u64 = 5;
const BACKOFF_MAX_S: u64 = 60;
/// How often the host's default audio output is re-read.
const DEVICES_EVERY_MS: u64 = 1000;
/// How often scene triggers are re-evaluated.
const SCENES_EVERY_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq)]
pub enum BrokerCmd {
    Connect,
    Disconnect,
    SetNc(NcAsmState),
    SetUlt(UltLevel),
    SetMultipoint(bool),
    SetEqPreset(u8),
    SetEqBands(Vec<u8>),
    SetVolume(u8),
    SetDseeAutoOff(bool),
    SetConnectionMode(u8),
    SetAutoPowerOff(u8),
    SetWearPause(bool),
    SetVoiceAssistant(u8),
    SetQuickAccess(Vec<u8>),
    SetVoiceGuideVolume(i8),
    SetTouchPanel(bool),
    SetSidetone(bool),
    /// Applies a scene by hand.
    ApplyScene(String),
    /// The saved scene list.
    SetScenes(Vec<Scene>),
    /// Re-emits the `"scenes"` event (list and active scene); nothing else.
    EmitScenes,
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

/// The scenes engine and how it reaches the outside world. It outlives each
/// connection, so the active scene survives reconnects.
pub struct SceneHost<'a> {
    pub engine: Engine,
    /// Builds the trigger inputs; the argument is whether the headphones are connected.
    pub sense: Box<dyn FnMut(bool) -> Inputs + 'a>,
    /// Publishes the list and the active scene (the `"scenes"` event).
    pub emit: Box<dyn Fn(&Engine) + 'a>,
}

impl SceneHost<'_> {
    /// Handles `ApplyScene`, `SetScenes` and `EmitScenes` (other commands are
    /// ignored), emits `"scenes"`, and
    /// returns the actions to send now (none while disconnected).
    fn command(&mut self, cmd: &BrokerCmd, connected: bool) -> Option<Actions> {
        let actions = match cmd {
            BrokerCmd::ApplyScene(id) => {
                let actions = self.engine.apply_manual(id);
                if actions.is_none() {
                    eprintln!("hairline: no scene {id:?} to apply");
                }
                actions
            }
            BrokerCmd::SetScenes(scenes) => {
                self.engine.set_scenes(scenes.clone());
                None
            }
            BrokerCmd::EmitScenes => None,
            _ => return None,
        };
        (self.emit)(&self.engine);
        self.engine.deliver(actions, connected)
    }

    /// After a connect: re-evaluates as connected and returns what to send,
    /// either a newly outranking scene or the held actions (never both).
    fn on_connected(&mut self) -> Option<Actions> {
        let inputs = (self.sense)(true);
        let before = self.engine.active().map(str::to_owned);
        let actions = self.engine.on_connected(&inputs);
        if self.engine.active() != before.as_deref() {
            (self.emit)(&self.engine);
        }
        actions
    }

    /// Re-evaluates the triggers, emitting when the active scene changes.
    fn tick(&mut self, connected: bool) -> Option<Actions> {
        let inputs = (self.sense)(connected);
        let before = self.engine.active().map(str::to_owned);
        let actions = self.engine.tick(&inputs);
        if self.engine.active() != before.as_deref() {
            (self.emit)(&self.engine);
        }
        self.engine.deliver(actions, connected)
    }
}

/// Sends a scene's actions, logging each refusal.
fn send_scene(session: &mut Session, actions: &Actions, events: &mut Vec<Event>) {
    for result in apply_actions(session, actions) {
        if let Err(e) = result {
            events.push(Event::Log(format!("scene action not sent: {e:?}")));
        }
    }
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
    run_broker_logged(transport, cmds, emit, clock, None, None, None)
}

/// As `run_broker`, also writing every frame and log line to `log`.
/// `device` is the connected headphones as discovered: its Bluetooth name is
/// matched against the default audio output (without it the model name is
/// used) and its address is shown in the UI. `scenes`, when given, is ticked
/// every two seconds and handles the scene commands.
pub fn run_broker_logged<E: Fn(&DeviceState)>(
    mut transport: Box<dyn Transport>,
    cmds: impl Borrow<Receiver<BrokerCmd>>,
    emit: E,
    mut clock: impl FnMut() -> u64,
    mut log: Option<&mut FrameLog>,
    device: Option<&Candidate>,
    mut scenes: Option<&mut SceneHost<'_>>,
) -> io::Result<()> {
    let cmds = cmds.borrow();
    let device_name = device.map(|d| d.name.as_str());
    let mut session = Session::new(clock());
    session.set_link(device.map(|d| d.address.clone()), transport.channel());
    session.start();
    let mut devices_checked_at: Option<u64> = None;
    let mut scenes_checked_at: Option<u64> = None;
    loop {
        let mut changed = false;
        let mut events = Vec::new();

        let now = clock();
        if let Some(host) = scenes.as_deref_mut() {
            if scenes_checked_at.is_none_or(|t| now >= t + SCENES_EVERY_MS) {
                scenes_checked_at = Some(now);
                if let Some(actions) = host.tick(session.state().connected) {
                    send_scene(&mut session, &actions, &mut events);
                    changed = true;
                }
            }
        }

        let now = clock();
        if devices_checked_at.is_none_or(|t| now >= t + DEVICES_EVERY_MS) {
            devices_checked_at = Some(now);
            let name = device_name.or(session.state().model.as_deref());
            let devices = paired_devices(name);
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
                Ok(
                    cmd @ (BrokerCmd::ApplyScene(_)
                    | BrokerCmd::SetScenes(_)
                    | BrokerCmd::EmitScenes),
                ) => {
                    let connected = session.state().connected;
                    let due = scenes
                        .as_deref_mut()
                        .and_then(|h| h.command(&cmd, connected));
                    if let Some(actions) = due {
                        send_scene(&mut session, &actions, &mut events);
                        changed = true;
                    }
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

        if events.contains(&Event::Connected) {
            if let Some(host) = scenes.as_deref_mut() {
                if let Some(actions) = host.on_connected() {
                    send_scene(&mut session, &actions, &mut events);
                }
                scenes_checked_at = Some(clock());
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
        BrokerCmd::SetMultipoint(on) => session.set_multipoint(on),
        BrokerCmd::SetEqPreset(preset) => session.set_eq_preset(preset),
        BrokerCmd::SetEqBands(bands) => session.set_eq_bands(bands),
        BrokerCmd::SetVolume(v) => session.set_volume(v),
        BrokerCmd::SetDseeAutoOff(auto) => session.set_dsee_auto_off(auto),
        BrokerCmd::SetConnectionMode(v) => session.set_connection_mode(v),
        BrokerCmd::SetAutoPowerOff(v) => session.set_auto_power_off(v),
        BrokerCmd::SetWearPause(on) => session.set_wear_pause(on),
        BrokerCmd::SetVoiceAssistant(v) => session.set_voice_assistant(v),
        BrokerCmd::SetQuickAccess(functions) => session.set_quick_access(functions),
        BrokerCmd::SetVoiceGuideVolume(v) => session.set_voice_guide_volume(v),
        BrokerCmd::SetTouchPanel(on) => session.set_touch_panel(on),
        BrokerCmd::SetSidetone(on) => session.set_sidetone(on),
        // Handled by the scene host; without one there is nothing to do.
        BrokerCmd::ApplyScene(_) | BrokerCmd::SetScenes(_) | BrokerCmd::EmitScenes => Ok(()),
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

/// Whether the host's default audio output is the headphones, by name.
pub fn is_playing(output_name: Option<&str>, device_name: Option<&str>) -> bool {
    matches!((output_name, device_name), (Some(out), Some(dev)) if out.eq_ignore_ascii_case(dev))
}

/// The host's own entry in the Link list.
fn paired_devices(device_name: Option<&str>) -> Vec<PairedDevice> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let playing = is_playing(current_output_device_name().as_deref(), device_name);
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
            let scenes = app
                .state::<Mutex<Vec<Scene>>>()
                .lock()
                .map(|s| s.clone())
                .unwrap_or_default();
            let mut host = SceneHost {
                engine: Engine::new(scenes),
                sense: Box::new(desktop_inputs),
                emit: Box::new(|engine: &Engine| {
                    let _ = app.emit(
                        "scenes",
                        ScenesEvent {
                            scenes: engine.scenes(),
                            active: engine.active(),
                            hotkey_errors: crate::hotkeys::latest_errors(&app),
                        },
                    );
                }),
            };
            let mut log = FrameLog::from_env();
            let mut backoff = 0;
            loop {
                let result = match open_transport(fake) {
                    Ok((t, device)) => {
                        backoff = 0;
                        run_broker_logged(
                            t,
                            &rx,
                            emit,
                            clock,
                            log.as_mut(),
                            device.as_ref(),
                            Some(&mut host),
                        )
                    }
                    Err(e) => Err(e),
                };
                let next = match result {
                    Ok(()) => wait_for_connect(&rx, None, &mut host),
                    Err(e) => {
                        backoff = next_backoff(backoff);
                        eprintln!(
                            "hairline: no link ({:?}: {e}); retrying in {backoff}s",
                            e.kind()
                        );
                        wait_for_connect(&rx, Some(Duration::from_secs(backoff)), &mut host)
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

/// The `"scenes"` event payload.
#[derive(Clone, serde::Serialize)]
struct ScenesEvent<'a> {
    scenes: &'a [Scene],
    active: Option<&'a str>,
    /// Refused or failed hotkeys by scene id; empty when none.
    hotkey_errors: std::collections::HashMap<String, String>,
}

/// Trigger inputs from the desktop: local time and the macOS sensors.
fn desktop_inputs(connected: bool) -> Inputs {
    use chrono::Timelike;
    let now = chrono::Local::now();
    Inputs {
        connected,
        now_minutes: (now.hour() * 60 + now.minute()) as u16,
        frontmost_app: frontmost_app_name(),
        mic_in_use: mic_in_use(),
    }
}

/// Blocks until `Connect` arrives or `timeout` passes; `Err` once the app is
/// gone. Scenes keep ticking (nothing is sent; due actions are held for the
/// next connect) and scene commands are handled meanwhile.
fn wait_for_connect(
    rx: &Receiver<BrokerCmd>,
    timeout: Option<Duration>,
    host: &mut SceneHost<'_>,
) -> Result<(), ()> {
    let deadline = timeout.map(|t| Instant::now() + t);
    let tick_every = Duration::from_millis(SCENES_EVERY_MS);
    let mut next_tick = Instant::now();
    loop {
        let now = Instant::now();
        if now >= next_tick {
            host.tick(false);
            next_tick = now + tick_every;
        }
        if deadline.is_some_and(|d| now >= d) {
            return Ok(());
        }
        let wake = deadline.map_or(next_tick, |d| d.min(next_tick));
        let cmd = match rx.recv_timeout(wake.saturating_duration_since(Instant::now())) {
            Ok(cmd) => cmd,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Err(()),
        };
        if cmd == BrokerCmd::Connect {
            return Ok(());
        }
        host.command(&cmd, false);
    }
}

/// The transport and, for a real device, the candidate it was opened to.
fn open_transport(fake: bool) -> io::Result<(Box<dyn Transport>, Option<Candidate>)> {
    if fake {
        return Ok((Box::new(FakeDevice::ult_wear()), None));
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
    Ok((connect(&pick.address)?, Some(pick.clone())))
}
