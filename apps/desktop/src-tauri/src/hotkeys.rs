//! Global shortcuts: one per enabled scene with a hotkey, plus the fixed
//! window shortcut. A press applies the scene through the broker, the same
//! path the `apply_scene` command takes.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::broker::BrokerCmd;
use crate::commands::Broker;
use crate::popover;
use crate::scenes::model::Scene;

/// Opens or focuses the main window (⌥⌘N on a Mac).
pub const WINDOW_SHORTCUT: &str = "Alt+Super+N";

/// The key under which `HotkeyErrors` holds a failed window shortcut; no
/// scene may use it as its id.
pub const WINDOW_ERROR_KEY: &str = "__window";

/// The latest registration errors by scene id (and `WINDOW_ERROR_KEY`), sent
/// with the `"scenes"` event.
#[derive(Default)]
pub struct HotkeyErrors(pub Mutex<HashMap<String, String>>);

/// Parses a Tauri accelerator string (modifiers in any order and case).
pub fn parse(accelerator: &str) -> Result<Shortcut, String> {
    accelerator.parse::<Shortcut>().map_err(|e| e.to_string())
}

/// True when an accelerator that parses holds none of Ctrl, Alt, Super and is
/// not an F1–F24 key, so it would take over ordinary typing. One that does not
/// parse gives false; registration reports it.
pub fn lacks_modifier(accelerator: &str) -> bool {
    let Ok(shortcut) = parse(accelerator.trim()) else {
        return false;
    };
    let function_key = shortcut
        .key
        .to_string()
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n));
    !function_key
        && !shortcut
            .mods
            .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
}

/// Identity of an accelerator for duplicate checks: the parsed shortcut when
/// it parses, otherwise the trimmed text without regard to case.
fn identity(accelerator: &str) -> String {
    match parse(accelerator) {
        Ok(s) => format!("#{}", s.id()),
        Err(_) => accelerator.trim().to_uppercase(),
    }
}

/// `(scene_id, text)` pairs: an accelerator or an error.
type ById = Vec<(String, String)>;

/// The plan: which scene gets which shortcut, and which are refused as
/// duplicates (the window shortcut and earlier scenes win).
fn plan(scenes: &[Scene]) -> (ById, ById) {
    let mut taken: HashMap<String, String> = HashMap::new();
    taken.insert(identity(WINDOW_SHORTCUT), "Open window".into());
    let mut register = Vec::new();
    let mut refused = Vec::new();
    for s in scenes.iter().filter(|s| s.enabled) {
        let Some(accel) = s.hotkey.as_deref().map(str::trim).filter(|a| !a.is_empty()) else {
            continue;
        };
        match taken.get(&identity(accel)) {
            Some(owner) => refused.push((s.id.clone(), format!("already used by {owner}"))),
            None => {
                taken.insert(identity(accel), format!("scene {:?}", s.name));
                register.push((s.id.clone(), accel.to_string()));
            }
        }
    }
    (register, refused)
}

/// `(scene_id, accelerator)` for each enabled scene with a hotkey; for a
/// shortcut used twice, the first in list order.
pub fn plan_registrations(scenes: &[Scene]) -> Vec<(String, String)> {
    plan(scenes).0
}

/// `(scene_id, error)` for each enabled scene whose hotkey is already taken
/// by the window shortcut or an earlier scene.
pub fn conflicts(scenes: &[Scene]) -> Vec<(String, String)> {
    plan(scenes).1
}

/// Unregisters our shortcuts, registers the window shortcut and the plan, and
/// stores and returns the errors by scene id (parse and OS failures, conflicts),
/// with a failed window shortcut under `WINDOW_ERROR_KEY`.
pub fn apply(app: &AppHandle, scenes: &[Scene]) -> HashMap<String, String> {
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        eprintln!("shdc: could not clear hotkeys: {e}");
    }
    let window = gs.on_shortcut(WINDOW_SHORTCUT, |app, _, event| {
        if event.state == ShortcutState::Pressed {
            let _ = popover::open_main_window(app);
        }
    });
    let (register, refused) = plan(scenes);
    let mut errors: HashMap<String, String> = refused.into_iter().collect();
    if let Err(e) = window {
        eprintln!("shdc: window hotkey {WINDOW_SHORTCUT} not registered: {e}");
        errors.insert(WINDOW_ERROR_KEY.into(), e.to_string());
    }
    let mut registered = 0;
    for (id, accel) in register {
        let result = parse(&accel).and_then(|shortcut| {
            let scene = id.clone();
            gs.on_shortcut(shortcut, move |app, _, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                if let Some(broker) = app.try_state::<Broker>() {
                    let _ = broker.0.send(BrokerCmd::ApplyScene(scene.clone()));
                }
            })
            .map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => registered += 1,
            Err(e) => {
                errors.insert(id, e);
            }
        }
    }
    eprintln!(
        "shdc: hotkeys registered ({registered} scene, {} refused or invalid)",
        errors.len()
    );
    if let Some(state) = app.try_state::<HotkeyErrors>() {
        if let Ok(mut latest) = state.0.lock() {
            latest.clone_from(&errors);
        }
    }
    errors
}

/// The stored errors, for the `"scenes"` event.
pub fn latest_errors(app: &AppHandle) -> HashMap<String, String> {
    app.try_state::<HotkeyErrors>()
        .and_then(|s| s.0.lock().ok().map(|e| e.clone()))
        .unwrap_or_default()
}
