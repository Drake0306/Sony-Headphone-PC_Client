//! The settings store: `settings.json` in a directory, defaults on any trouble.

use std::path::PathBuf;

use hairline_lib::settings::{load, save, AppSettings, Theme};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hairline-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn load_defaults_when_missing() {
    let dir = temp_dir("missing");
    let s = load(&dir);
    assert_eq!(
        s,
        AppSettings {
            theme: Theme::System,
            launch_at_login: false,
            hide_dock_icon: true,
        }
    );
    assert!(s == AppSettings::default());
}

#[test]
fn round_trip_save_load() {
    let dir = temp_dir("round-trip");
    let s = AppSettings {
        theme: Theme::Ink,
        launch_at_login: true,
        hide_dock_icon: false,
    };
    save(&dir, &s).unwrap();
    let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(text.contains("\"theme\": \"ink\""), "{text}");
    assert!(load(&dir) == s);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn corrupt_file_falls_back_to_defaults() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), "{ not json").unwrap();
    assert!(load(&dir) == AppSettings::default());
    std::fs::remove_dir_all(&dir).unwrap();
}
