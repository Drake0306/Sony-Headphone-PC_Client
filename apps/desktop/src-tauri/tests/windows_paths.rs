//! Windows code paths that are plain logic, tested on every OS: the popover's
//! tray-click decision, the audio stub and the frontmost-app name mapping.

use std::time::{Duration, Instant};

use hairline_lib::audio_windows;
use hairline_lib::popover::{tray_click_action, TrayAction};
use hairline_lib::sensors_windows::app_name_from_path;

#[test]
fn popover_hides_on_focus_loss() {
    let now = Instant::now();
    // A visible popover: the tray click hides it.
    assert_eq!(tray_click_action(None, now, true), TrayAction::Hide);
    // Clicking the tray icon took focus away and hid the popover a moment ago:
    // the same click must not reopen it.
    let hidden_at = now - Duration::from_millis(50);
    assert_eq!(
        tray_click_action(Some(hidden_at), now, false),
        TrayAction::Ignore
    );
    // A later click opens it again.
    let hidden_at = now - Duration::from_millis(2000);
    assert_eq!(
        tray_click_action(Some(hidden_at), now, false),
        TrayAction::Show
    );
    // Never hidden before: the first click opens it.
    assert_eq!(tray_click_action(None, now, false), TrayAction::Show);
}

#[test]
fn tray_click_guard() {
    let hidden_at = Instant::now();
    let at = |ms| hidden_at + Duration::from_millis(ms);
    assert_eq!(
        tray_click_action(Some(hidden_at), at(0), false),
        TrayAction::Ignore
    );
    assert_eq!(
        tray_click_action(Some(hidden_at), at(299), false),
        TrayAction::Ignore
    );
    assert_eq!(
        tray_click_action(Some(hidden_at), at(300), false),
        TrayAction::Show
    );
    // Visible wins over the guard.
    assert_eq!(
        tray_click_action(Some(hidden_at), at(10), true),
        TrayAction::Hide
    );
    // A clock that reads earlier than the hide is treated as "just hidden".
    assert_eq!(
        tray_click_action(Some(at(100)), hidden_at, false),
        TrayAction::Ignore
    );
}

#[test]
fn windows_paired_devices_are_empty() {
    assert!(audio_windows::paired_devices(Some("ULT WEAR")).is_empty());
    assert!(audio_windows::paired_devices(None).is_empty());
    assert_eq!(audio_windows::output_device_name(), None);
}

#[test]
fn frontmost_app_name_from_image_path() {
    assert_eq!(
        app_name_from_path(r"C:\Program Files\Zoom\bin\Zoom.exe").as_deref(),
        Some("Zoom")
    );
    assert_eq!(
        app_name_from_path(r"C:\Windows\explorer.EXE").as_deref(),
        Some("explorer")
    );
    assert_eq!(
        app_name_from_path(r"C:\Tools\Spotify").as_deref(),
        Some("Spotify")
    );
    assert_eq!(app_name_from_path("Teams.exe").as_deref(), Some("Teams"));
}

#[test]
fn frontmost_app_none_on_failure() {
    assert_eq!(app_name_from_path(""), None);
    assert_eq!(app_name_from_path("   "), None);
    assert_eq!(app_name_from_path(r"C:\Program Files\"), None);
    assert_eq!(app_name_from_path(r"C:\Apps\.exe"), None);
}

/// The real sensor never panics; whatever the desktop shows, it is a name or `None`.
#[cfg(windows)]
#[test]
fn frontmost_app_sensor_does_not_panic() {
    if let Some(name) = hairline_lib::sensors_windows::frontmost_app_name() {
        assert!(!name.is_empty());
        assert!(!name.to_ascii_lowercase().ends_with(".exe"));
    }
    assert!(!hairline_lib::sensors_windows::mic_in_use());
}
