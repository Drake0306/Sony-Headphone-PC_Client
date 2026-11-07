//! The settings store: `settings.json` in a directory, defaults on any trouble.

use std::path::PathBuf;

use hairline_lib::broker::BrokerCmd;
use hairline_lib::settings::{advanced_off_commands, load, save, AppSettings, Theme};

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
            theme: Theme::Paper,
            launch_at_login: false,
            hide_dock_icon: true,
            show_advanced: false,
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
        show_advanced: true,
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

#[test]
fn a_file_without_show_advanced_keeps_its_settings() {
    let dir = temp_dir("no-advanced");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        r#"{ "theme": "ink", "launch_at_login": true, "hide_dock_icon": false }"#,
    )
    .unwrap();
    let s = load(&dir);
    assert_eq!(s.theme, Theme::Ink);
    assert!(!s.show_advanced);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_leaves_no_temp_file_and_overwrites() {
    let dir = temp_dir("atomic");
    save(&dir, &AppSettings::default()).unwrap();
    let ink = AppSettings {
        theme: Theme::Ink,
        ..AppSettings::default()
    };
    save(&dir, &ink).unwrap();
    assert_eq!(load(&dir), ink);
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["settings.json"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn switching_advanced_off_stops_the_probe_and_the_recording() {
    let on = AppSettings {
        show_advanced: true,
        ..AppSettings::default()
    };
    let off = AppSettings::default();
    assert_eq!(
        advanced_off_commands(&on, &off),
        [BrokerCmd::ProbeStop, BrokerCmd::RecordFrames(false)]
    );
    assert!(advanced_off_commands(&off, &on).is_empty());
    assert!(advanced_off_commands(&on, &on).is_empty());
    // Another setting saved while Advanced stays off sends nothing.
    let ink = AppSettings {
        theme: Theme::Ink,
        ..AppSettings::default()
    };
    assert!(advanced_off_commands(&off, &ink).is_empty());
}
