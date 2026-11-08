//! `scenes.json` in the app config directory: pretty JSON, seeds on any trouble.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use super::model::{Actions, Nc, Scene, Trigger};
use super::validate::check_scene;
use crate::hotkeys::WINDOW_ERROR_KEY;

const FILE_NAME: &str = "scenes.json";
/// Where an unreadable `scenes.json` is kept (an older one is replaced).
const BAD_NAME: &str = "scenes.json.bad";
/// Written first, then renamed over `scenes.json`.
const TMP_NAME: &str = "scenes.json.tmp";

fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

fn seed(name: &str, trigger: Trigger, actions: Actions) -> Scene {
    Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        enabled: false,
        trigger,
        actions,
        hotkey: None,
    }
}

/// Four disabled examples so the screen is not empty on first run, the more
/// specific triggers first (the order is the priority).
pub fn seed_scenes() -> Vec<Scene> {
    vec![
        seed(
            "On a call",
            Trigger::MicInUse,
            Actions {
                nc: Some(Nc::NoiseCancelling),
                ..Actions::default()
            },
        ),
        seed(
            "Late",
            Trigger::TimeRange {
                from: "22:00".into(),
                to: "07:00".into(),
            },
            Actions {
                nc: Some(Nc::NoiseCancelling),
                volume: Some(12),
                ..Actions::default()
            },
        ),
        seed(
            "At the desk",
            Trigger::HeadphonesConnected,
            Actions {
                nc: Some(Nc::Ambient),
                ambient_level: Some(14),
                voice_focus: Some(true),
                ..Actions::default()
            },
        ),
        seed(
            "Walking",
            Trigger::Manual,
            Actions {
                nc: Some(Nc::Ambient),
                ambient_level: Some(20),
                ult: Some(1),
                ..Actions::default()
            },
        ),
    ]
}

/// The scenes in `dir`. A missing file gives the seeds. An unreadable or
/// corrupt one is renamed to `scenes.json.bad` and gives the seeds. Scenes
/// that fail `check_scene`, or repeat or reserve an id, are disabled (with a
/// fresh id for a repeated or reserved one), never dropped.
pub fn load(dir: &Path) -> Vec<Scene> {
    let path = path_in(dir);
    let parsed = std::fs::read_to_string(&path)
        .and_then(|text| serde_json::from_str::<Vec<Scene>>(&text).map_err(io::Error::other));
    match parsed {
        Ok(scenes) => disable_invalid(scenes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => seed_scenes(),
        Err(e) => {
            eprintln!(
                "shdc: scenes.json unreadable ({e}); kept as scenes.json.bad, using the examples"
            );
            if let Err(e) = std::fs::rename(&path, dir.join(BAD_NAME)) {
                eprintln!("shdc: could not rename scenes.json: {e}");
            }
            seed_scenes()
        }
    }
}

/// Disables each scene that would fail `check_scenes`, logging why.
fn disable_invalid(mut scenes: Vec<Scene>) -> Vec<Scene> {
    let mut ids = HashSet::new();
    for s in &mut scenes {
        let reserved = s.id == WINDOW_ERROR_KEY;
        let problem = if reserved || !ids.insert(s.id.clone()) {
            let why = if reserved {
                "is reserved"
            } else {
                "is used twice"
            };
            let why = format!("scene {:?}: id {:?} {why}; given a new id", s.name, s.id);
            s.id = uuid::Uuid::new_v4().to_string();
            ids.insert(s.id.clone());
            Some(why)
        } else {
            check_scene(s).err()
        };
        if let Some(why) = problem {
            if s.enabled {
                s.enabled = false;
                eprintln!("shdc: {why}; scene disabled");
            } else {
                eprintln!("shdc: {why}");
            }
        }
    }
    scenes
}

/// Writes the scenes to `dir` as pretty JSON, creating `dir` if needed, by
/// way of `scenes.json.tmp` so a crash cannot leave half a file.
pub fn save(dir: &Path, scenes: &[Scene]) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(scenes).map_err(io::Error::other)?;
    let tmp = dir.join(TMP_NAME);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path_in(dir))
}
