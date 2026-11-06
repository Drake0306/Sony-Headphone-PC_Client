//! `scenes.json` in the app config directory: pretty JSON, seeds on any trouble.

use std::io;
use std::path::{Path, PathBuf};

use super::model::{Actions, Nc, Scene, Trigger};

const FILE_NAME: &str = "scenes.json";

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

/// Four disabled examples so the screen is not empty on first run.
pub fn seed_scenes() -> Vec<Scene> {
    vec![
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

/// The scenes in `dir`. A missing file gives the seeds; an unreadable or
/// corrupt one gives the seeds and a warning. Never writes.
pub fn load(dir: &Path) -> Vec<Scene> {
    let path = path_in(dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return seed_scenes(),
        Err(e) => {
            log::warn!(
                "could not read {}: {e}; using the seed scenes",
                path.display()
            );
            return seed_scenes();
        }
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        log::warn!("{} is corrupt ({e}); using the seed scenes", path.display());
        seed_scenes()
    })
}

/// Writes the scenes to `dir` as pretty JSON, creating `dir` if needed.
pub fn save(dir: &Path, scenes: &[Scene]) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(scenes).map_err(io::Error::other)?;
    std::fs::write(path_in(dir), text)
}
