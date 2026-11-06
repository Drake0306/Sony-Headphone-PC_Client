//! The scenes store: `scenes.json` in a directory, seeds on any trouble.

use std::path::PathBuf;

use hairline_lib::scenes::model::{Nc, Trigger};
use hairline_lib::scenes::store::{load, save, seed_scenes};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hairline-scenes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn load_seeds_when_missing() {
    let dir = temp_dir("missing");
    let scenes = load(&dir);
    let names: Vec<&str> = scenes.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["At the desk", "On a call", "Late", "Walking"]);
    assert!(scenes.iter().all(|s| !s.enabled));
    assert!(!dir.exists(), "load must not write");
    let ids: std::collections::HashSet<_> = scenes.iter().map(|s| s.id.clone()).collect();
    assert_eq!(ids.len(), 4);
    assert_eq!(scenes[0].trigger, Trigger::HeadphonesConnected);
    assert_eq!(scenes[0].actions.nc, Some(Nc::Ambient));
    assert_eq!(scenes[0].actions.ambient_level, Some(14));
    assert_eq!(scenes[0].actions.voice_focus, Some(true));
    assert_eq!(scenes[2].actions.volume, Some(12));
    assert_eq!(scenes[3].actions.ult, Some(1));
}

#[test]
fn round_trip() {
    let dir = temp_dir("round-trip");
    let mut scenes = seed_scenes();
    scenes[1].enabled = true;
    scenes[1].hotkey = Some("Ctrl+Alt+1".into());
    save(&dir, &scenes).unwrap();
    assert_eq!(load(&dir), scenes);
    let text = std::fs::read_to_string(dir.join("scenes.json")).unwrap();
    assert!(text.contains("\"kind\": \"mic_in_use\""), "{text}");
    assert!(text.contains("\"eq_preset\": null"), "{text}");
}

#[test]
fn corrupt_falls_back_to_seeds() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("scenes.json"), "{ not json").unwrap();
    let scenes = load(&dir);
    assert_eq!(scenes.len(), 4);
    assert!(scenes.iter().all(|s| !s.enabled));
}
