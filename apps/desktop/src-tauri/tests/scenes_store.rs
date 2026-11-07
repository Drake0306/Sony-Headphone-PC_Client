//! The scenes store: `scenes.json` in a directory, seeds on any trouble.

use std::path::PathBuf;

use hairline_lib::scenes::eval::active_scene;
use hairline_lib::scenes::model::{Inputs, Nc, Trigger};
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
    assert_eq!(names, ["On a call", "Late", "At the desk", "Walking"]);
    assert!(scenes.iter().all(|s| !s.enabled));
    assert!(!dir.exists(), "load must not write");
    let ids: std::collections::HashSet<_> = scenes.iter().map(|s| s.id.clone()).collect();
    assert_eq!(ids.len(), 4);
    assert_eq!(scenes[0].trigger, Trigger::MicInUse);
    assert_eq!(scenes[0].actions.nc, Some(Nc::NoiseCancelling));
    assert_eq!(scenes[1].actions.volume, Some(12));
    assert_eq!(scenes[2].trigger, Trigger::HeadphonesConnected);
    assert_eq!(scenes[2].actions.nc, Some(Nc::Ambient));
    assert_eq!(scenes[2].actions.ambient_level, Some(14));
    assert_eq!(scenes[2].actions.voice_focus, Some(true));
    assert_eq!(scenes[3].actions.ult, Some(1));
}

#[test]
fn seeds_prefer_the_specific_trigger() {
    let mut scenes = seed_scenes();
    for s in &mut scenes {
        s.enabled = true;
    }
    let at = |hour: u16, mic_in_use: bool| Inputs {
        connected: true,
        now_minutes: hour * 60 + if hour == 23 { 30 } else { 0 },
        frontmost_app: None,
        mic_in_use,
    };
    let active = |inputs: Inputs| active_scene(&scenes, &inputs).map(|s| s.name.clone());
    assert_eq!(active(at(12, true)).as_deref(), Some("On a call"));
    assert_eq!(active(at(23, false)).as_deref(), Some("Late"));
    assert_eq!(active(at(12, false)).as_deref(), Some("At the desk"));
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
fn corrupt_file_is_kept_as_bad() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("scenes.json.bad"), "an older one").unwrap();
    std::fs::write(dir.join("scenes.json"), "{ not json").unwrap();
    let scenes = load(&dir);
    assert_eq!(scenes.len(), 4);
    assert!(scenes.iter().all(|s| !s.enabled));
    assert!(!dir.join("scenes.json").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("scenes.json.bad")).unwrap(),
        "{ not json"
    );
}

#[test]
fn save_leaves_no_tmp_file() {
    let dir = temp_dir("tmp");
    save(&dir, &seed_scenes()).unwrap();
    // A second save replaces the file in place.
    save(&dir, &seed_scenes()).unwrap();
    assert!(dir.join("scenes.json").exists());
    assert!(!dir.join("scenes.json.tmp").exists());
}

#[test]
fn invalid_loaded_scene_is_disabled() {
    let dir = temp_dir("invalid");
    let mut scenes = seed_scenes();
    for s in &mut scenes {
        s.enabled = true;
    }
    scenes[1].actions.volume = Some(31);
    scenes[3].hotkey = Some("Space".into());
    save(&dir, &scenes).unwrap();
    let loaded = load(&dir);
    assert_eq!(loaded.len(), 4, "never dropped");
    let enabled: Vec<bool> = loaded.iter().map(|s| s.enabled).collect();
    assert_eq!(enabled, [true, false, true, false]);
    assert_eq!(loaded[1].actions.volume, Some(31), "kept as written");
    assert_eq!(loaded[1].id, scenes[1].id);
}

#[test]
fn repeated_loaded_id_is_renewed_and_disabled() {
    let dir = temp_dir("repeated");
    let mut scenes = seed_scenes();
    scenes[1].enabled = true;
    scenes[1].id = scenes[0].id.clone();
    save(&dir, &scenes).unwrap();
    let loaded = load(&dir);
    assert_eq!(loaded[0].id, scenes[0].id);
    assert_ne!(loaded[1].id, scenes[0].id);
    assert!(!loaded[1].enabled);
}

#[test]
fn frontmost_app_trigger_round_trips() {
    let dir = temp_dir("frontmost");
    let mut scenes = seed_scenes();
    scenes[3].trigger = Trigger::FrontmostApp {
        name: "zoom.us".into(),
    };
    save(&dir, &scenes).unwrap();
    assert_eq!(load(&dir), scenes);
    let text = std::fs::read_to_string(dir.join("scenes.json")).unwrap();
    assert!(text.contains("\"kind\": \"frontmost_app\""), "{text}");
    assert!(text.contains("\"name\": \"zoom.us\""), "{text}");
}
