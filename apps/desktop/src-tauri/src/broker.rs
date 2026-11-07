//! The broker thread: owns the transport and the `Session`, applies UI
//! commands and publishes every state change.

use std::borrow::Borrow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use mdr_proto::function::{opcode_role, Role};
use mdr_proto::{request, Group, NcAsmState, Request, Table, UltLevel};
use mdr_session::{DeviceState, Event, PairedDevice, Session, SessionError};
use mdr_transport::{connect, discover, is_ult_wear_name, Candidate, FakeDevice, Transport};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio_macos::current_output_device_name;
use crate::device_record;
use crate::frame_log;
use crate::frame_ring::{FrameEntry, FrameTap, Recorder};
use crate::probe::{ProbeEvent, ProbeHost, ProbeRunner};
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
    /// Starts or stops the `"frames"` stream (starting emits a snapshot).
    WatchFrames(bool),
    /// Starts or stops recording frames to a file; emits `"recording"`.
    RecordFrames(bool),
    /// A hand-written request from the Advanced panel; still deny-listed.
    /// Anything but a GET is refused unless `confirmed_set` (the typed
    /// confirmation was given).
    Raw {
        table: Table,
        bytes: Vec<u8>,
        confirmed_set: bool,
    },
    /// Starts the opcode probe (connected only; GETs only).
    ProbeStart,
    /// Stops the probe, keeping the results so far.
    ProbeStop,
}

/// The word the Advanced panel asks for before a raw SET.
pub const RAW_SET_CONFIRMATION: &str = "SEND";

/// Whether a raw payload needs the typed confirmation: everything whose
/// opcode is not a GET in the libmdr role table (SETs, and unknown opcodes
/// to be safe).
pub fn needs_confirmation(table: Table, bytes: &[u8]) -> bool {
    bytes
        .first()
        .is_none_or(|&op| opcode_role(table, op) != Role::Get)
}

/// The Advanced panel's raw command: parses like `parse_raw`, then refuses
/// a deny-listed payload, and a SET unless `confirm` is exactly `SEND`. The
/// session applies the deny-list again when sending.
pub fn raw_command(table: &str, hex: &str, confirm: &str) -> Result<BrokerCmd, String> {
    let (table, bytes) = parse_table_hex(table, hex)?;
    // The session refuses these too; checking here lets the panel say why.
    if let Some(reason) = mdr_proto::denied(table, &bytes) {
        return Err(format!("never sent: {reason}"));
    }
    let confirmed_set = if needs_confirmation(table, &bytes) {
        if confirm != RAW_SET_CONFIRMATION {
            return Err("type SEND to send a SET".into());
        }
        true
    } else {
        false
    };
    Ok(BrokerCmd::Raw {
        table,
        bytes,
        confirmed_set,
    })
}

/// Builds `BrokerCmd::Raw` from a table name (`t1`/`t2`) and hex bytes, which
/// may be separated by spaces, colons or dashes.
pub fn parse_raw(table: &str, hex: &str) -> Result<BrokerCmd, String> {
    let (table, bytes) = parse_table_hex(table, hex)?;
    Ok(BrokerCmd::Raw {
        table,
        bytes,
        confirmed_set: false,
    })
}

fn parse_table_hex(table: &str, hex: &str) -> Result<(Table, Vec<u8>), String> {
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
    Ok((table, bytes))
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
    /// Handles `ApplyScene`, `SetScenes` (an edited active scene is
    /// re-applied) and `EmitScenes` (other commands are ignored), emits
    /// `"scenes"`, and returns the actions to send now (none while
    /// disconnected).
    fn command(&mut self, cmd: &BrokerCmd, connected: bool) -> Option<Actions> {
        let actions = match cmd {
            BrokerCmd::ApplyScene(id) => {
                let actions = self.engine.apply_manual(id);
                if actions.is_none() {
                    eprintln!("hairline: no scene {id:?} to apply");
                }
                actions
            }
            BrokerCmd::SetScenes(scenes) => self.engine.set_scenes(scenes.clone()),
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
    run_broker_logged(transport, cmds, emit, clock, BrokerContext::default())
}

/// What a connection runs with besides the transport; all optional.
#[derive(Default)]
pub struct BrokerContext<'a, 's, 'f, 'p> {
    /// The connected headphones as discovered: the Bluetooth name is matched
    /// against the default audio output (without it the model name is used),
    /// the address is shown in the UI and keys the device record.
    pub device: Option<&'a Candidate>,
    /// Ticked every two seconds; handles the scene commands.
    pub scenes: Option<&'a mut SceneHost<'s>>,
    /// The frame ring, recording and stream; handles `WatchFrames`/`RecordFrames`.
    pub frames: Option<&'a mut FrameTap<'f>>,
    /// The settings directory; device records live in its `devices/`.
    pub record_dir: Option<&'a Path>,
    /// The opcode probe; handles `ProbeStart`/`ProbeStop`.
    pub probe: Option<&'a mut ProbeHost<'p>>,
}

/// As `run_broker`, with the device, scenes, frame tap and device records
/// in `ctx`. The device record for the connected address is loaded into the
/// session (`mark_proven`) before the handshake.
pub fn run_broker_logged<E: Fn(&DeviceState)>(
    mut transport: Box<dyn Transport>,
    cmds: impl Borrow<Receiver<BrokerCmd>>,
    emit: E,
    mut clock: impl FnMut() -> u64,
    ctx: BrokerContext<'_, '_, '_, '_>,
) -> io::Result<()> {
    let BrokerContext {
        device,
        mut scenes,
        mut frames,
        record_dir,
        mut probe,
    } = ctx;
    let address = device.map(|d| d.address.as_str());
    // Ends a running probe with its partial results (Stop or a lost link).
    // Returns true when the probe marked functions proven.
    let end_probe = |probe: &mut Option<&mut ProbeHost<'_>>, session: &mut Session| {
        probe
            .as_deref_mut()
            .is_some_and(|host| host.abort(Some(session), address, record_dir))
    };
    let cmds = cmds.borrow();
    let device_name = device.map(|d| d.name.as_str());
    let mut session = Session::new(clock());
    session.set_link(device.map(|d| d.address.clone()), transport.channel());
    if let (Some(dir), Some(device)) = (record_dir, device) {
        for (table, function) in device_record::load(dir, &device.address).proven {
            session.mark_proven(table, function);
        }
    }
    if let Some(tap) = frames.as_deref_mut() {
        tap.new_link();
    }
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
            if let Some(tap) = frames.as_deref_mut() {
                tap.sent(now, &frame);
            }
            if let Err(e) = transport.write(&frame) {
                end_probe(&mut probe, &mut session);
                return Err(link_lost(&mut session, &emit, e));
            }
        }

        match transport.recv_timeout(RECV_TIMEOUT) {
            Ok(Some(bytes)) => {
                let now = clock();
                if let Some(tap) = frames.as_deref_mut() {
                    tap.received(now, &bytes);
                }
                events.extend(session.handle_input(&bytes, now));
            }
            Ok(None) => {}
            Err(e) => {
                end_probe(&mut probe, &mut session);
                return Err(link_lost(&mut session, &emit, e));
            }
        }
        let now = clock();
        if session.next_timeout_ms().is_some_and(|t| now >= t) {
            events.extend(session.handle_timeout(now));
        }

        loop {
            match cmds.try_recv() {
                Ok(BrokerCmd::Disconnect) | Err(TryRecvError::Disconnected) => {
                    end_probe(&mut probe, &mut session);
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
                Ok(BrokerCmd::WatchFrames(on)) => {
                    if let Some(tap) = frames.as_deref_mut() {
                        tap.watch(on);
                    }
                    if let (true, Some(host)) = (on, probe.as_deref()) {
                        host.emit_current();
                    }
                }
                Ok(BrokerCmd::ProbeStart) => {
                    if let Some(host) = probe.as_deref_mut() {
                        if let Some(why) = host.start(&session, clock()) {
                            events.push(Event::Log(why));
                        }
                    }
                }
                Ok(BrokerCmd::ProbeStop) => changed |= end_probe(&mut probe, &mut session),
                Ok(BrokerCmd::RecordFrames(on)) => {
                    if let Some(tap) = frames.as_deref_mut() {
                        tap.record(on);
                    }
                }
                Ok(BrokerCmd::Raw {
                    table,
                    ref bytes,
                    confirmed_set: false,
                }) if needs_confirmation(table, bytes) => {
                    events.push(Event::Log(format!(
                        "raw {table:?} {bytes:02X?} not sent: a SET needs confirmation"
                    )));
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

        if let Some(host) = probe.as_deref_mut() {
            changed |= host.step(&mut session, &events, clock(), address, record_dir);
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
                    if let Some(tap) = frames.as_deref_mut() {
                        tap.note(clock(), msg);
                    }
                }
                Event::Payload { .. } => {}
            }
        }
        if changed {
            emit(session.state());
        }
        if let Some(tap) = frames.as_deref_mut() {
            tap.flush();
        }
        if gave_up {
            end_probe(&mut probe, &mut session);
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
        BrokerCmd::Raw { table, bytes, .. } => session.request(Request {
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
        // Already connected, or handled by the frame tap or the probe host.
        BrokerCmd::Connect
        | BrokerCmd::Disconnect
        | BrokerCmd::WatchFrames(_)
        | BrokerCmd::RecordFrames(_)
        | BrokerCmd::ProbeStart
        | BrokerCmd::ProbeStop => Ok(()),
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
/// a lost link. A `Connect` command cuts the wait short. `record_dir` is the
/// settings directory (device records); `recorder` is shared with the
/// `record_frames` command.
pub fn spawn_broker(
    app: AppHandle,
    fake: bool,
    record_dir: Option<PathBuf>,
    recorder: Recorder,
) -> Sender<BrokerCmd> {
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
            let mut tap = FrameTap::new(
                frame_log::default_dir(),
                recorder,
                Box::new(|frames: &[FrameEntry]| {
                    let _ = app.emit("frames", frames);
                }),
                Box::new(|path: Option<&Path>| {
                    let _ = app.emit("recording", RecordingEvent::new(path));
                }),
            );
            if std::env::var("HAIRLINE_FRAME_LOG").is_ok_and(|v| v == "1") {
                tap.record(true);
            }
            let mut probe = ProbeHost::new(
                ProbeRunner::new(),
                frame_log::default_dir(),
                Box::new(|event: &ProbeEvent| {
                    let _ = app.emit("probe", event);
                }),
            );
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
                            BrokerContext {
                                device: device.as_ref(),
                                scenes: Some(&mut host),
                                frames: Some(&mut tap),
                                record_dir: record_dir.as_deref(),
                                probe: Some(&mut probe),
                            },
                        )
                    }
                    Err(e) => Err(e),
                };
                let next = match result {
                    Ok(()) => wait_for_connect(&rx, None, &mut host, &mut tap, &probe),
                    Err(e) => {
                        backoff = next_backoff(backoff);
                        eprintln!(
                            "hairline: no link ({:?}: {e}); retrying in {backoff}s",
                            e.kind()
                        );
                        wait_for_connect(
                            &rx,
                            Some(Duration::from_secs(backoff)),
                            &mut host,
                            &mut tap,
                            &probe,
                        )
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

/// The `"recording"` event payload: the file being written, or null.
#[derive(Clone, serde::Serialize)]
pub struct RecordingEvent {
    pub path: Option<String>,
}

impl RecordingEvent {
    pub fn new(path: Option<&Path>) -> RecordingEvent {
        RecordingEvent {
            path: path.map(|p| p.display().to_string()),
        }
    }
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
/// next connect) and scene and frame-tap commands are handled meanwhile.
fn wait_for_connect(
    rx: &Receiver<BrokerCmd>,
    timeout: Option<Duration>,
    host: &mut SceneHost<'_>,
    tap: &mut FrameTap<'_>,
    probe: &ProbeHost<'_>,
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
        match cmd {
            BrokerCmd::Connect => return Ok(()),
            BrokerCmd::WatchFrames(on) => {
                tap.watch(on);
                if on {
                    probe.emit_current();
                }
            }
            BrokerCmd::RecordFrames(on) => tap.record(on),
            BrokerCmd::ProbeStart => eprintln!("hairline: probe not started: not connected"),
            cmd => {
                host.command(&cmd, false);
            }
        }
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
