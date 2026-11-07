//! The checks a scene list must pass, shared by `set_scenes` (which refuses
//! the save) and the store (which disables a bad scene it loads).

use std::collections::HashSet;

use super::eval::parse_hhmm;
use super::model::{Nc, Scene, Trigger};
use crate::commands::{EQ_PRESETS, VOLUME_MAX};
use crate::hotkeys::{lacks_modifier, WINDOW_ERROR_KEY};

/// The first problem in a scene list, naming the scene and the field, in
/// list order (a repeated id, then `check_scene`).
pub fn check_scenes(scenes: &[Scene]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for s in scenes {
        if !ids.insert(s.id.as_str()) {
            return Err(problem(s, "id", format!("{:?} is used twice", s.id)));
        }
        check_scene(s)?;
    }
    Ok(())
}

fn problem(s: &Scene, field: &str, why: String) -> String {
    format!("scene {:?}: {field} {why}", s.name)
}

/// The first problem in one scene (everything but a repeated id).
pub fn check_scene(s: &Scene) -> Result<(), String> {
    let bad = |field: &str, why: String| Err(problem(s, field, why));
    if s.id == WINDOW_ERROR_KEY {
        return bad("id", format!("{:?} is reserved", s.id));
    }
    if s.name.trim().is_empty() {
        return bad("name", "is empty".into());
    }
    match &s.trigger {
        Trigger::TimeRange { from, to } => {
            for (field, value) in [("from", from), ("to", to)] {
                if parse_hhmm(value).is_none() {
                    return bad(field, format!("{value:?} is not HH:MM"));
                }
            }
        }
        Trigger::FrontmostApp { name } if name.trim().is_empty() => {
            return bad("app name", "is empty".into());
        }
        _ => {}
    }
    let a = &s.actions;
    if let Some(v) = a.ambient_level.filter(|v| !(1..=20).contains(v)) {
        return bad("ambient_level", format!("{v} is outside 1..=20"));
    }
    if a.voice_focus.is_some() && a.nc != Some(Nc::Ambient) {
        return bad("voice_focus", "needs Noise set to Ambient".into());
    }
    if let Some(v) = a.ult.filter(|&v| v > 2) {
        return bad("ult", format!("{v} is outside 0..=2"));
    }
    if let Some(v) = a.eq_preset.filter(|v| !EQ_PRESETS.contains(v)) {
        return bad("eq_preset", format!("{v:#04X} is not a known preset"));
    }
    if let Some(v) = a.volume.filter(|&v| v > VOLUME_MAX) {
        return bad("volume", format!("{v} is outside 0..={VOLUME_MAX}"));
    }
    if let Some(accel) = s.hotkey.as_deref().filter(|a| lacks_modifier(a)) {
        return bad("hotkey", format!("{accel:?} needs Ctrl, Alt or Super"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_scene, check_scenes};
    use crate::scenes::model::{Actions, Nc, Scene, Trigger};
    use crate::scenes::store::seed_scenes;

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

    fn err_of(scenes: &[Scene]) -> String {
        check_scenes(scenes).unwrap_err()
    }

    #[test]
    fn seed_scenes_are_valid() {
        assert_eq!(check_scenes(&seed_scenes()), Ok(()));
    }

    #[test]
    fn scene_checks_name_the_bad_field() {
        let with = |actions: Actions| [scene("a", Trigger::Manual, actions)];
        let e = err_of(&with(Actions {
            ambient_level: Some(0),
            ..Actions::default()
        }));
        assert!(e.contains("ambient_level"), "{e}");
        let e = err_of(&with(Actions {
            ambient_level: Some(21),
            ..Actions::default()
        }));
        assert!(e.contains("ambient_level"), "{e}");
        let e = err_of(&with(Actions {
            ult: Some(3),
            ..Actions::default()
        }));
        assert!(e.contains("ult"), "{e}");
        let e = err_of(&with(Actions {
            volume: Some(31),
            ..Actions::default()
        }));
        assert!(e.contains("volume"), "{e}");
        let e = err_of(&with(Actions {
            eq_preset: Some(0x42),
            ..Actions::default()
        }));
        assert!(e.contains("eq_preset"), "{e}");
        assert_eq!(
            check_scenes(&with(Actions {
                ambient_level: Some(20),
                ult: Some(2),
                volume: Some(30),
                eq_preset: Some(0xA2),
                ..Actions::default()
            })),
            Ok(())
        );
    }

    #[test]
    fn scene_checks_triggers_names_and_ids() {
        let range = |from: &str, to: &str| Trigger::TimeRange {
            from: from.into(),
            to: to.into(),
        };
        let e = err_of(&[scene("a", range("25:00", "07:00"), Actions::default())]);
        assert!(e.contains("from"), "{e}");
        let e = err_of(&[scene("a", range("22:00", "7"), Actions::default())]);
        assert!(e.contains("to"), "{e}");
        let app = Trigger::FrontmostApp { name: "  ".into() };
        let e = err_of(&[scene("a", app, Actions::default())]);
        assert!(e.contains("app name"), "{e}");
        let mut unnamed = scene("a", Trigger::Manual, Actions::default());
        unnamed.name = " ".into();
        assert!(err_of(&[unnamed]).contains("name"));
        let twice = [
            scene("a", Trigger::Manual, Actions::default()),
            scene("a", Trigger::MicInUse, Actions::default()),
        ];
        assert!(err_of(&twice).contains("used twice"));
        let reserved = scene("__window", Trigger::Manual, Actions::default());
        assert_eq!(
            err_of(&[reserved]),
            "scene \"__window\": id \"__window\" is reserved"
        );
    }

    #[test]
    fn voice_focus_needs_ambient() {
        for nc in [None, Some(Nc::NoiseCancelling), Some(Nc::Off)] {
            let e = check_scene(&scene(
                "v",
                Trigger::Manual,
                Actions {
                    nc,
                    voice_focus: Some(true),
                    ..Actions::default()
                },
            ))
            .unwrap_err();
            assert!(e.contains("voice_focus needs Noise set to Ambient"), "{e}");
        }
        check_scene(&scene(
            "v",
            Trigger::Manual,
            Actions {
                nc: Some(Nc::Ambient),
                voice_focus: Some(false),
                ..Actions::default()
            },
        ))
        .unwrap();
    }

    #[test]
    fn scene_hotkeys_need_a_modifier_or_a_function_key() {
        let with = |hotkey: &str| {
            let mut s = scene("a", Trigger::Manual, Actions::default());
            s.name = "X".into();
            s.hotkey = Some(hotkey.into());
            [s]
        };
        for hotkey in ["1", "Shift+A", "Space"] {
            assert_eq!(
                err_of(&with(hotkey)),
                format!("scene \"X\": hotkey {hotkey:?} needs Ctrl, Alt or Super")
            );
        }
        for hotkey in ["Alt+1", "F5", "Ctrl+Shift+A", "Super+Space", "F24"] {
            assert_eq!(check_scenes(&with(hotkey)), Ok(()), "{hotkey}");
        }
    }
}
