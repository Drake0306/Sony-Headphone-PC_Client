//! The scenes engine against the fake headphones, pumped by hand so every
//! frame the session writes can be counted.

use std::time::Duration;

use hairline_lib::scenes::engine::{apply_actions, Engine};
use hairline_lib::scenes::model::{Actions, Inputs, Nc, Scene, Trigger};
use mdr_frame::{Decoder, FrameType};
use mdr_proto::{NcMode, UltLevel};
use mdr_session::{Event, Session, SessionError, Status};
use mdr_transport::{FakeDevice, Transport};

/// A session wired to the fake device, with every written data frame recorded.
struct Rig {
    session: Session,
    fake: FakeDevice,
    now: u64,
    /// Payloads of the data frames the session wrote (ACKs left out).
    sent: Vec<Vec<u8>>,
    events: Vec<Event>,
}

impl Rig {
    fn new(fake: FakeDevice) -> Rig {
        let mut session = Session::new(0);
        session.start();
        Rig {
            session,
            fake,
            now: 0,
            sent: Vec::new(),
            events: Vec::new(),
        }
    }

    /// A rig whose handshake has completed.
    fn connected(fake: FakeDevice) -> Rig {
        let mut rig = Rig::new(fake);
        rig.pump();
        assert!(rig.session.state().connected, "handshake did not complete");
        rig
    }

    /// Exchanges frames until neither side has anything left to say.
    fn pump(&mut self) {
        let mut decoder = Decoder::new();
        for _ in 0..10_000 {
            self.now += 1;
            let mut wrote = false;
            while let Some(bytes) = self.session.poll_output(self.now) {
                wrote = true;
                for frame in decoder.push(&bytes).into_iter().flatten() {
                    if frame.frame_type != FrameType::Ack {
                        self.sent.push(frame.payload);
                    }
                }
                self.fake.write(&bytes).unwrap();
            }
            let got = self.fake.recv_timeout(Duration::ZERO).unwrap();
            if let Some(bytes) = &got {
                let evs = self.session.handle_input(bytes, self.now);
                self.events.extend(evs);
            }
            if !wrote && got.is_none() {
                return;
            }
        }
        panic!("the conversation never settled");
    }

    fn inputs(&self, mic_in_use: bool) -> Inputs {
        Inputs {
            connected: self.session.state().connected,
            now_minutes: 12 * 60,
            frontmost_app: None,
            mic_in_use,
        }
    }

    /// Data frames sent with this opcode (first payload byte).
    fn sent_with(&self, opcode: u8) -> usize {
        self.sent
            .iter()
            .filter(|p| p.first() == Some(&opcode))
            .count()
    }
}

const NCASM_SET: u8 = 0x68;
const EQ_SET: u8 = 0x58;

fn scene(id: &str, trigger: Trigger, actions: Actions) -> Scene {
    Scene {
        id: id.into(),
        name: id.into(),
        enabled: true,
        trigger,
        actions,
        hotkey: None,
    }
}

fn nc_ambient_scene(id: &str, trigger: Trigger) -> Scene {
    scene(
        id,
        trigger,
        Actions {
            nc: Some(Nc::Ambient),
            ambient_level: Some(14),
            voice_focus: Some(true),
            ..Actions::default()
        },
    )
}

fn assert_ambient_14_voice(rig: &Rig) {
    let nc = &rig.session.state().nc;
    assert_eq!(nc.status, Status::Confirmed);
    let v = nc.value.as_ref().unwrap();
    assert_eq!(
        (v.mode, v.level, v.voice_focus),
        (NcMode::Ambient, 14, true)
    );
}

#[test]
fn unchanged_active_scene_sends_nothing() {
    let mut rig = Rig::connected(FakeDevice::ult_wear());
    let mut engine = Engine::new(vec![nc_ambient_scene("call", Trigger::MicInUse)]);
    let inputs = rig.inputs(true);

    let actions = engine
        .tick(&inputs)
        .expect("first tick activates the scene");
    let results = apply_actions(&mut rig.session, &actions);
    assert_eq!(results, vec![Ok(())]);
    rig.pump();
    assert_eq!(rig.sent_with(NCASM_SET), 1);
    assert_ambient_14_voice(&rig);
    assert_eq!(engine.active(), Some("call"));

    let before = rig.sent.len();
    assert_eq!(engine.tick(&inputs), None);
    rig.pump();
    assert_eq!(
        rig.sent.len(),
        before,
        "no new frames: {:02X?}",
        &rig.sent[before..]
    );
    assert_eq!(engine.active(), Some("call"));
}

#[test]
fn partial_refusal_keeps_rest() {
    let mut fake = FakeDevice::ult_wear();
    fake.without_capability(0x53);
    let mut rig = Rig::connected(fake);
    let mut engine = Engine::new(vec![scene(
        "loud",
        Trigger::Manual,
        Actions {
            nc: Some(Nc::NoiseCancelling),
            ult: Some(2),
            ..Actions::default()
        },
    )]);

    let actions = engine.apply_manual("loud").expect("known scene");
    let results = apply_actions(&mut rig.session, &actions);
    assert_eq!(results.len(), 2);
    assert!(results.contains(&Ok(())));
    assert!(results.contains(&Err(SessionError::Unavailable(0x53))));
    rig.pump();

    let state = rig.session.state();
    assert_eq!(state.nc.status, Status::Confirmed);
    assert_eq!(
        state.nc.value.as_ref().map(|v| v.mode),
        Some(NcMode::NoiseCancelling)
    );
    assert_eq!(state.eq_ult.status, Status::Unavailable);
    assert_eq!(rig.sent_with(EQ_SET), 0);
}

#[test]
fn manual_apply_sends_actions() {
    let mut rig = Rig::connected(FakeDevice::ult_wear());
    let mut engine = Engine::new(vec![scene(
        "walk",
        Trigger::Manual,
        Actions {
            nc: Some(Nc::Ambient),
            ambient_level: Some(20),
            ult: Some(1),
            eq_preset: Some(0x16),
            volume: Some(12),
            dsee_auto_off: Some(false),
            ..Actions::default()
        },
    )]);
    // A manual scene never activates on its own.
    assert_eq!(engine.tick(&rig.inputs(false)), None);
    assert_eq!(engine.active(), None);
    assert_eq!(engine.apply_manual("nope"), None);

    let actions = engine.apply_manual("walk").expect("known scene");
    assert_eq!(engine.active(), Some("walk"));
    // nc, ULT with the EQ preset (one request, so neither undoes the
    // other), volume and dsee: four setter calls.
    let results = apply_actions(&mut rig.session, &actions);
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    rig.pump();

    let state = rig.session.state();
    let nc = state.nc.value.as_ref().unwrap();
    assert_eq!((nc.mode, nc.level), (NcMode::Ambient, 20));
    assert_eq!(state.nc.status, Status::Confirmed);
    assert_eq!(state.volume.value, Some(12));
    assert_eq!(state.dsee_auto_off.value, Some(false));
    assert_eq!(state.eq_ult.status, Status::Confirmed);
    let eq = state.eq_ult.value.as_ref().unwrap();
    assert_eq!(eq.ult, UltLevel::Ult1);
    assert_eq!(eq.preset, 0x16);

    // The next tick keeps the hand-applied scene and sends nothing.
    let before = rig.sent.len();
    assert_eq!(engine.tick(&rig.inputs(false)), None);
    assert_eq!(engine.active(), Some("walk"));
    rig.pump();
    assert_eq!(rig.sent.len(), before);
}

#[test]
fn connect_then_scene_applies_once() {
    let mut rig = Rig::new(FakeDevice::ult_wear());
    let mut engine = Engine::new(vec![nc_ambient_scene("call", Trigger::MicInUse)]);

    // Not connected yet: the scene becomes active but nothing is sent.
    let inputs = rig.inputs(true);
    assert!(!inputs.connected);
    let actions = engine.tick(&inputs);
    assert_eq!(engine.deliver(actions, inputs.connected), None);
    assert_eq!(engine.active(), Some("call"));

    rig.pump();
    assert!(rig.events.contains(&Event::Connected));
    assert_eq!(rig.sent_with(NCASM_SET), 0);

    let actions = engine
        .on_connected(&rig.inputs(true))
        .expect("held actions on connect");
    assert!(apply_actions(&mut rig.session, &actions)
        .iter()
        .all(Result::is_ok));
    rig.pump();
    assert_eq!(rig.sent_with(NCASM_SET), 1);
    assert_ambient_14_voice(&rig);

    // Neither a second connect nor the next tick sends it again.
    assert_eq!(engine.on_connected(&rig.inputs(true)), None);
    let inputs = rig.inputs(true);
    let actions = engine.tick(&inputs);
    assert_eq!(engine.deliver(actions, inputs.connected), None);
    rig.pump();
    assert_eq!(rig.sent_with(NCASM_SET), 1);
}

#[test]
fn held_actions_are_dropped_when_no_scene_is_active_on_connect() {
    let mut engine = Engine::new(vec![nc_ambient_scene("call", Trigger::MicInUse)]);
    let offline = |mic| Inputs {
        connected: false,
        now_minutes: 0,
        frontmost_app: None,
        mic_in_use: mic,
    };
    let a = engine.tick(&offline(true));
    assert_eq!(engine.deliver(a, false), None);
    assert_eq!(engine.tick(&offline(false)), None);
    assert_eq!(engine.active(), None);
    let online = Inputs {
        connected: true,
        ..offline(false)
    };
    assert_eq!(engine.on_connected(&online), None);
}

#[test]
fn connect_prefers_an_outranking_scene_over_held_actions() {
    let desk = scene(
        "desk",
        Trigger::HeadphonesConnected,
        Actions {
            volume: Some(7),
            ..Actions::default()
        },
    );
    let mut engine = Engine::new(vec![
        desk.clone(),
        nc_ambient_scene("call", Trigger::MicInUse),
    ]);
    let mut inputs = Inputs {
        connected: false,
        now_minutes: 0,
        frontmost_app: None,
        mic_in_use: true,
    };
    let held = engine.tick(&inputs);
    assert_eq!(engine.deliver(held, false), None);
    assert_eq!(engine.active(), Some("call"));
    inputs.connected = true;
    assert_eq!(engine.on_connected(&inputs), Some(desk.actions));
    assert_eq!(engine.active(), Some("desk"));
    // The held actions were dropped, not saved for later.
    assert_eq!(engine.on_connected(&inputs), None);
}

#[test]
fn unknown_nc_state_is_busy_not_guessed() {
    let mut session = Session::new(0);
    let a = Actions {
        ambient_level: Some(5),
        ..Actions::default()
    };
    assert_eq!(
        apply_actions(&mut session, &a),
        vec![Err(SessionError::Busy)]
    );
}

#[test]
fn set_scenes_clears_a_deleted_active_scene() {
    let mut engine = Engine::new(vec![nc_ambient_scene("call", Trigger::MicInUse)]);
    let on = Inputs {
        connected: true,
        now_minutes: 0,
        frontmost_app: None,
        mic_in_use: true,
    };
    assert!(engine.tick(&on).is_some());
    engine.set_scenes(vec![nc_ambient_scene("other", Trigger::Manual)]);
    assert_eq!(engine.active(), None);
    assert_eq!(engine.scenes().len(), 1);
    // The rule that matched is gone, so nothing is applied.
    assert_eq!(engine.tick(&on), None);
}

#[test]
fn editing_the_active_scene_reapplies() {
    let mut engine = Engine::new(vec![
        nc_ambient_scene("walk", Trigger::Manual),
        nc_ambient_scene("other", Trigger::Manual),
    ]);
    assert!(engine.apply_manual("walk").is_some());
    let mut edited = engine.scenes().to_vec();
    edited[0].actions.volume = Some(9);
    assert_eq!(
        engine.set_scenes(edited.clone()),
        Some(edited[0].actions.clone())
    );
    assert_eq!(engine.active(), Some("walk"));
    // Editing a scene that is not active sends nothing.
    edited[1].actions.volume = Some(3);
    assert_eq!(engine.set_scenes(edited), None);
}

#[test]
fn saving_unchanged_active_scene_sends_nothing() {
    let mut engine = Engine::new(vec![nc_ambient_scene("walk", Trigger::Manual)]);
    assert!(engine.apply_manual("walk").is_some());
    let mut same = engine.scenes().to_vec();
    same[0].name = "Walking".into();
    same[0].hotkey = Some("Alt+Super+1".into());
    assert_eq!(engine.set_scenes(same), None);
    assert_eq!(engine.active(), Some("walk"));
}

#[test]
fn disabling_the_active_scene_sends_nothing_and_clears_active() {
    let mut engine = Engine::new(vec![nc_ambient_scene("call", Trigger::MicInUse)]);
    let on = Inputs {
        connected: true,
        now_minutes: 0,
        frontmost_app: None,
        mic_in_use: true,
    };
    assert!(engine.tick(&on).is_some());
    let mut off = engine.scenes().to_vec();
    off[0].enabled = false;
    off[0].actions.volume = Some(9);
    assert_eq!(engine.set_scenes(off), None);
    assert_eq!(engine.active(), None);
    // It cannot match any more, so the next tick applies nothing.
    assert_eq!(engine.tick(&on), None);
    assert_eq!(engine.active(), None);

    // A scene that was already off and applied by hand stays active.
    assert!(engine.apply_manual("call").is_some());
    let same = engine.scenes().to_vec();
    assert_eq!(engine.set_scenes(same), None);
    assert_eq!(engine.active(), Some("call"));
}

// ---- The broker loop with a scene host and scripted sensors ----

mod broker_level {
    use std::io;
    use std::sync::mpsc::{channel, Receiver};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use hairline_lib::broker::{run_broker_logged, BrokerCmd, SceneHost};
    use hairline_lib::scenes::engine::Engine;
    use hairline_lib::scenes::model::{Actions, Inputs, Nc, Scene, Trigger};
    use mdr_frame::{Decoder, FrameType};
    use mdr_session::{DeviceState, Status};
    use mdr_transport::{FakeDevice, Transport};

    const NCASM_SET: u8 = 0x68;
    const VOLUME_SET: u8 = 0xA8;
    const DSEE_SET: u8 = 0xE8;

    fn monotonic_ms() -> u64 {
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_millis() as u64
    }

    /// The fake device, recording the payload of every data frame written to it.
    struct Recording {
        fake: FakeDevice,
        decoder: Decoder,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl Transport for Recording {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            for frame in self.decoder.push(bytes).into_iter().flatten() {
                if frame.frame_type != FrameType::Ack {
                    self.sent.lock().unwrap().push(frame.payload);
                }
            }
            self.fake.write(bytes)
        }
        fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            self.fake.recv_timeout(timeout)
        }
        fn close(&mut self) {
            self.fake.close()
        }
        fn channel(&self) -> Option<u8> {
            self.fake.channel()
        }
    }

    struct Running {
        cmds: std::sync::mpsc::Sender<BrokerCmd>,
        states: Receiver<DeviceState>,
        /// The `active` of every `"scenes"` emission.
        scenes: Receiver<Option<String>>,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        handle: std::thread::JoinHandle<io::Result<()>>,
    }

    impl Running {
        fn sent_with(&self, opcode: u8) -> usize {
            let sent = self.sent.lock().unwrap();
            sent.iter().filter(|p| p.first() == Some(&opcode)).count()
        }
        fn sent_len(&self) -> usize {
            self.sent.lock().unwrap().len()
        }
        fn wait_state(&self, pred: impl Fn(&DeviceState) -> bool) -> DeviceState {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.states.recv_timeout(left) {
                    Ok(s) if pred(&s) => return s,
                    Ok(_) => continue,
                    Err(e) => panic!("no matching state emitted: {e}"),
                }
            }
        }
        fn next_scenes(&self) -> Option<String> {
            self.scenes
                .recv_timeout(Duration::from_secs(5))
                .expect("a scenes emission")
        }
        fn stop(self) {
            self.cmds.send(BrokerCmd::Disconnect).unwrap();
            assert!(self.handle.join().unwrap().is_ok());
        }
    }

    /// Runs the broker on the fake with `scenes`; the microphone reads as in use.
    fn run(scenes: Vec<Scene>) -> Running {
        let (cmds, rx) = channel();
        let (stx, states) = channel();
        let (scx, scenes_rx) = channel();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let transport = Recording {
            fake: FakeDevice::ult_wear(),
            decoder: Decoder::new(),
            sent: Arc::clone(&sent),
        };
        let handle = std::thread::spawn(move || {
            let mut host = SceneHost {
                engine: Engine::new(scenes),
                sense: Box::new(|connected| Inputs {
                    connected,
                    now_minutes: 12 * 60,
                    frontmost_app: None,
                    mic_in_use: true,
                }),
                emit: Box::new(move |engine: &Engine| {
                    let _ = scx.send(engine.active().map(str::to_owned));
                }),
            };
            run_broker_logged(
                Box::new(transport),
                rx,
                |s| {
                    let _ = stx.send(s.clone());
                },
                monotonic_ms,
                None,
                None,
                Some(&mut host),
            )
        });
        Running {
            cmds,
            states,
            scenes: scenes_rx,
            sent,
            handle,
        }
    }

    fn scene(id: &str, trigger: Trigger, actions: Actions) -> Scene {
        Scene {
            id: id.into(),
            name: id.into(),
            enabled: true,
            trigger,
            actions,
            hotkey: None,
        }
    }

    #[test]
    fn connect_applies_only_the_higher_ranked_scene() {
        let h = scene(
            "desk",
            Trigger::HeadphonesConnected,
            Actions {
                volume: Some(7),
                ..Actions::default()
            },
        );
        let a = scene(
            "call",
            Trigger::MicInUse,
            Actions {
                nc: Some(Nc::Ambient),
                ambient_level: Some(5),
                dsee_auto_off: Some(false),
                ..Actions::default()
            },
        );
        let b = run(vec![h, a]);
        // Before the handshake the lower scene matches and is held...
        assert_eq!(b.next_scenes().as_deref(), Some("call"));
        // ...but on connect the headphones scene outranks it.
        assert_eq!(b.next_scenes().as_deref(), Some("desk"));
        b.wait_state(|s| s.volume.value == Some(7) && s.volume.status == Status::Confirmed);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(b.sent_with(VOLUME_SET), 1);
        assert_eq!(b.sent_with(NCASM_SET), 0, "held scene leaked through");
        assert_eq!(b.sent_with(DSEE_SET), 0, "held scene leaked through");
        b.stop();
    }

    #[test]
    fn emit_scenes_re_emits_without_sending() {
        let walk = scene(
            "walk",
            Trigger::Manual,
            Actions {
                volume: Some(9),
                ..Actions::default()
            },
        );
        let b = run(vec![walk]);
        b.wait_state(|s| s.connected);
        b.cmds.send(BrokerCmd::ApplyScene("walk".into())).unwrap();
        assert_eq!(b.next_scenes().as_deref(), Some("walk"));
        b.wait_state(|s| s.volume.value == Some(9) && s.volume.status == Status::Confirmed);
        std::thread::sleep(Duration::from_millis(200));
        let before = b.sent_len();

        b.cmds.send(BrokerCmd::EmitScenes).unwrap();
        assert_eq!(b.next_scenes().as_deref(), Some("walk"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(b.scenes.try_recv().is_err(), "exactly one more emission");
        assert_eq!(b.sent_len(), before, "EmitScenes sent frames");
        b.stop();
    }

    #[test]
    fn editing_the_active_scene_sends_the_new_actions() {
        let walk = scene(
            "walk",
            Trigger::Manual,
            Actions {
                volume: Some(9),
                ..Actions::default()
            },
        );
        let b = run(vec![walk.clone()]);
        b.wait_state(|s| s.connected);
        b.cmds.send(BrokerCmd::ApplyScene("walk".into())).unwrap();
        assert_eq!(b.next_scenes().as_deref(), Some("walk"));
        b.wait_state(|s| s.volume.value == Some(9) && s.volume.status == Status::Confirmed);

        let mut edited = walk;
        edited.actions.volume = Some(4);
        b.cmds
            .send(BrokerCmd::SetScenes(vec![edited.clone()]))
            .unwrap();
        assert_eq!(b.next_scenes().as_deref(), Some("walk"));
        b.wait_state(|s| s.volume.value == Some(4) && s.volume.status == Status::Confirmed);
        assert_eq!(b.sent_with(VOLUME_SET), 2);

        b.cmds.send(BrokerCmd::SetScenes(vec![edited])).unwrap();
        assert_eq!(b.next_scenes().as_deref(), Some("walk"));
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(b.sent_with(VOLUME_SET), 2, "unchanged save sent frames");
        b.stop();
    }
}
