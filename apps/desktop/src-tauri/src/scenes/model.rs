//! The scene data model, as stored in `scenes.json` and shown to the frontend.

use serde::{Deserialize, Serialize};

/// What makes a scene active. `HH:MM` strings are 24-hour local time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// Only applied by hand or hotkey.
    Manual,
    HeadphonesConnected,
    TimeRange {
        from: String,
        to: String,
    },
    /// The frontmost app's localized name (case-insensitive).
    FrontmostApp {
        name: String,
    },
    MicInUse,
}

/// Noise-control mode a scene can set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Nc {
    Off,
    NoiseCancelling,
    Ambient,
}

/// What a scene changes. `None` means keep the current setting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actions {
    pub nc: Option<Nc>,
    /// 1..=20
    pub ambient_level: Option<u8>,
    pub voice_focus: Option<bool>,
    /// 0 = off, 1, 2
    pub ult: Option<u8>,
    pub eq_preset: Option<u8>,
    /// 0..=30
    pub volume: Option<u8>,
    pub dsee_auto_off: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scene {
    /// UUID v4.
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub actions: Actions,
    /// Tauri accelerator string, e.g. `Ctrl+Alt+1`.
    pub hotkey: Option<String>,
}

/// What the evaluator looks at on each tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    pub connected: bool,
    /// Minutes since local midnight.
    pub now_minutes: u16,
    pub frontmost_app: Option<String>,
    pub mic_in_use: bool,
}
