//! Which global shortcuts get registered, and which are refused as conflicts.
//! OS-level registration needs a running app and is not covered here.

use shdc_lib::hotkeys::{conflicts, parse, plan_registrations, WINDOW_SHORTCUT};
use shdc_lib::scenes::model::{Actions, Scene, Trigger};

fn scene(id: &str, enabled: bool, hotkey: Option<&str>) -> Scene {
    Scene {
        id: id.into(),
        name: id.to_uppercase(),
        enabled,
        trigger: Trigger::Manual,
        actions: Actions::default(),
        hotkey: hotkey.map(String::from),
    }
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn plan_registrations_skips_disabled_and_empty() {
    let list = [
        scene("a", true, Some("Alt+Super+1")),
        scene("b", false, Some("Alt+Super+2")),
        scene("c", true, None),
        scene("d", true, Some("  ")),
        scene("e", true, Some("Ctrl+ArrowUp")),
    ];
    assert_eq!(
        plan_registrations(&list),
        pairs(&[("a", "Alt+Super+1"), ("e", "Ctrl+ArrowUp")])
    );
    assert!(conflicts(&list).is_empty());
}

#[test]
fn duplicate_accelerators_are_conflicts() {
    // hotkey_conflict_is_refused: the first in list order wins.
    let list = [
        scene("a", true, Some("Alt+Super+1")),
        scene("b", true, Some("Alt+Super+1")),
        // Same keys in another order and case.
        scene("c", true, Some("super+alt+1")),
        // A disabled scene never takes a shortcut.
        scene("d", false, Some("Shift+F5")),
        scene("e", true, Some("Shift+F5")),
    ];
    assert_eq!(
        plan_registrations(&list),
        pairs(&[("a", "Alt+Super+1"), ("e", "Shift+F5")])
    );
    assert_eq!(
        conflicts(&list),
        pairs(&[
            ("b", "already used by scene \"A\""),
            ("c", "already used by scene \"A\""),
        ])
    );
}

#[test]
fn window_shortcut_is_always_planned() {
    assert_eq!(WINDOW_SHORTCUT, "Alt+Super+N");
    assert!(parse(WINDOW_SHORTCUT).is_ok());
    let list = [scene("a", true, Some("Super+Alt+N"))];
    assert!(plan_registrations(&list).is_empty());
    assert_eq!(
        conflicts(&list),
        pairs(&[("a", "already used by Open window")])
    );
}

#[test]
fn recorder_key_names_parse() {
    // What SceneEditor's recorder stores: letters, digits, then KeyboardEvent.code.
    for accel in [
        "Alt+Super+1",
        "Ctrl+Shift+A",
        "Ctrl+ArrowUp",
        "Alt+Minus",
        "F5",
        "Super+Space",
        "Ctrl+Alt+BracketLeft",
        "Shift+Numpad7",
    ] {
        assert!(parse(accel).is_ok(), "{accel} should parse");
    }
    assert!(parse("Alt+Nonsense").is_err());
}
