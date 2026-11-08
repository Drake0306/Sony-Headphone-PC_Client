//! The scene evaluator: first enabled match in list order wins.

use shdc_lib::scenes::eval::{active_id_after_delete, active_scene, parse_hhmm, time_in_range};
use shdc_lib::scenes::model::{Actions, Inputs, Scene, Trigger};

fn scene(id: &str, enabled: bool, trigger: Trigger) -> Scene {
    Scene {
        id: id.into(),
        name: id.into(),
        enabled,
        trigger,
        actions: Actions::default(),
        hotkey: None,
    }
}

fn inputs() -> Inputs {
    Inputs {
        connected: true,
        now_minutes: 12 * 60,
        frontmost_app: None,
        mic_in_use: false,
    }
}

fn active_id(scenes: &[Scene], i: &Inputs) -> Option<String> {
    active_scene(scenes, i).map(|s| s.id.clone())
}

#[test]
fn first_match_wins() {
    let scenes = vec![
        scene("a", true, Trigger::HeadphonesConnected),
        scene("b", true, Trigger::HeadphonesConnected),
    ];
    assert_eq!(active_id(&scenes, &inputs()).as_deref(), Some("a"));
}

#[test]
fn reorder_changes_active() {
    let mut scenes = vec![
        scene("a", true, Trigger::HeadphonesConnected),
        scene("b", true, Trigger::HeadphonesConnected),
    ];
    scenes.reverse();
    assert_eq!(active_id(&scenes, &inputs()).as_deref(), Some("b"));
}

#[test]
fn disabled_scene_never_matches() {
    let scenes = vec![
        scene("a", false, Trigger::HeadphonesConnected),
        scene("b", true, Trigger::MicInUse),
    ];
    assert_eq!(active_id(&scenes, &inputs()), None);
}

#[test]
fn manual_never_matches_automatically() {
    let scenes = vec![scene("a", true, Trigger::Manual)];
    assert_eq!(active_id(&scenes, &inputs()), None);
}

#[test]
fn headphones_trigger_needs_connection() {
    let scenes = vec![scene("a", true, Trigger::HeadphonesConnected)];
    let i = Inputs {
        connected: false,
        ..inputs()
    };
    assert_eq!(active_id(&scenes, &i), None);
}

#[test]
fn time_range_wraps_midnight() {
    assert!(time_in_range(23 * 60 + 30, 22 * 60, 7 * 60));
    assert!(time_in_range(6 * 60, 22 * 60, 7 * 60));
    assert!(!time_in_range(12 * 60, 22 * 60, 7 * 60));
    // Same-day range: from inclusive, to exclusive.
    assert!(time_in_range(9 * 60, 9 * 60, 17 * 60));
    assert!(!time_in_range(17 * 60, 9 * 60, 17 * 60));
    // from == to is the whole day.
    assert!(time_in_range(300, 600, 600));

    let scenes = vec![scene(
        "late",
        true,
        Trigger::TimeRange {
            from: "22:00".into(),
            to: "07:00".into(),
        },
    )];
    let at = |m| Inputs {
        now_minutes: m,
        ..inputs()
    };
    assert!(active_scene(&scenes, &at(23 * 60 + 30)).is_some());
    assert!(active_scene(&scenes, &at(12 * 60)).is_none());
}

#[test]
fn bad_time_text_never_matches() {
    let scenes = vec![scene(
        "x",
        true,
        Trigger::TimeRange {
            from: "25:00".into(),
            to: "07:00".into(),
        },
    )];
    assert!(active_scene(&scenes, &inputs()).is_none());
    assert_eq!(parse_hhmm("07:05"), Some(425));
    assert_eq!(parse_hhmm("7:05"), Some(425));
    assert_eq!(parse_hhmm("24:00"), None);
    assert_eq!(parse_hhmm("12:60"), None);
    assert_eq!(parse_hhmm("noon"), None);
    assert_eq!(parse_hhmm("+7:05"), None);
    assert_eq!(parse_hhmm("07:+5"), None);
}

#[test]
fn frontmost_app_case_insensitive() {
    let scenes = vec![scene(
        "z",
        true,
        Trigger::FrontmostApp {
            name: "zoom.us".into(),
        },
    )];
    let i = Inputs {
        frontmost_app: Some("Zoom.US".into()),
        ..inputs()
    };
    assert!(active_scene(&scenes, &i).is_some());
    assert!(active_scene(&scenes, &inputs()).is_none());
}

#[test]
fn mic_in_use_matches() {
    let scenes = vec![scene("m", true, Trigger::MicInUse)];
    let i = Inputs {
        mic_in_use: true,
        ..inputs()
    };
    assert!(active_scene(&scenes, &i).is_some());
    assert!(active_scene(&scenes, &inputs()).is_none());
}

#[test]
fn delete_active_clears() {
    assert_eq!(active_id_after_delete(Some("a"), "a"), None);
    assert_eq!(active_id_after_delete(Some("a"), "b").as_deref(), Some("a"));
    assert_eq!(active_id_after_delete(None, "b"), None);
}
