//! Which scene is active, and turning its actions into session setter calls.

use mdr_proto::{NcAsmState, NcMode, UltLevel};
use mdr_session::{Session, SessionError};

use super::eval::active_scene;
use super::model::{Actions, Inputs, Nc, Scene};

/// The scene list and the active scene, kept on the broker thread.
///
/// The active scene changes when the automatic evaluation changes or on a
/// manual apply. A hand-applied scene stays active until the evaluation
/// moves on, so a `Manual` scene is not dropped on the next tick.
#[derive(Debug, Clone, Default)]
pub struct Engine {
    scenes: Vec<Scene>,
    active: Option<String>,
    /// The scene the triggers chose on the last tick.
    evaluated: Option<String>,
    /// Actions became due while the headphones were not connected.
    held: bool,
}

impl Engine {
    pub fn new(scenes: Vec<Scene>) -> Engine {
        Engine {
            scenes,
            ..Engine::default()
        }
    }

    pub fn scenes(&self) -> &[Scene] {
        &self.scenes
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// Replaces the list. A deleted active scene is cleared without a revert;
    /// a deleted evaluated scene is forgotten so the next tick re-evaluates.
    pub fn set_scenes(&mut self, scenes: Vec<Scene>) {
        let known = |id: &Option<String>| {
            id.as_ref()
                .is_some_and(|id| scenes.iter().any(|s| &s.id == id))
        };
        if !known(&self.active) {
            self.active = None;
        }
        if !known(&self.evaluated) {
            self.evaluated = None;
        }
        self.scenes = scenes;
    }

    /// Re-evaluates the triggers. Returns the new scene's actions only when
    /// the active scene changes to it; a scene that stops matching is not
    /// reverted.
    pub fn tick(&mut self, inputs: &Inputs) -> Option<Actions> {
        let now = active_scene(&self.scenes, inputs).map(|s| s.id.clone());
        if now == self.evaluated {
            return None;
        }
        let previous = std::mem::replace(&mut self.evaluated, now.clone());
        match now {
            None => {
                if self.active == previous {
                    self.active = None;
                }
                None
            }
            Some(id) if self.active.as_deref() == Some(id.as_str()) => None,
            Some(id) => {
                let actions = self.actions_of(&id);
                self.active = Some(id);
                actions
            }
        }
    }

    /// Makes `id` the active scene and returns its actions; `None` for an
    /// unknown id (the active scene is then unchanged).
    pub fn apply_manual(&mut self, id: &str) -> Option<Actions> {
        let actions = self.actions_of(id)?;
        self.active = Some(id.to_owned());
        Some(actions)
    }

    /// Passes `actions` through when the headphones are connected; otherwise
    /// holds them for `on_connected` and returns `None`.
    pub fn deliver(&mut self, actions: Option<Actions>, connected: bool) -> Option<Actions> {
        let actions = actions?;
        if connected {
            Some(actions)
        } else {
            self.held = true;
            None
        }
    }

    /// After a connect: the active scene's actions if any were held for it
    /// while disconnected, once.
    pub fn on_connected(&mut self) -> Option<Actions> {
        if !std::mem::take(&mut self.held) {
            return None;
        }
        let id = self.active.clone()?;
        self.actions_of(&id)
    }

    fn actions_of(&self, id: &str) -> Option<Actions> {
        self.scenes
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.actions.clone())
    }
}

/// Sends a scene's actions through the session setters, so capability gating
/// and the deny-list apply as for the popover. Noise mode, ambient level and
/// voice focus form one NC request built on the current NC state; ULT and the
/// EQ preset form one EQ request when both are set. One result per setter
/// call, in order; a failure does not stop the rest.
pub fn apply_actions(session: &mut Session, a: &Actions) -> Vec<Result<(), SessionError>> {
    let mut results = Vec::new();
    if a.nc.is_some() || a.ambient_level.is_some() || a.voice_focus.is_some() {
        let mut nc = session.state().nc.value.unwrap_or(NcAsmState {
            mode: NcMode::Off,
            level: 1,
            voice_focus: false,
        });
        if let Some(mode) = a.nc {
            nc.mode = match mode {
                Nc::Off => NcMode::Off,
                Nc::NoiseCancelling => NcMode::NoiseCancelling,
                Nc::Ambient => NcMode::Ambient,
            };
        }
        if let Some(level) = a.ambient_level {
            nc.level = level;
        }
        if let Some(on) = a.voice_focus {
            nc.voice_focus = on;
        }
        results.push(session.set_nc(nc));
    }
    let ult = a.ult.map(ult_level);
    match (ult, a.eq_preset) {
        (Some(Some(ult)), Some(preset)) => results.push(session.set_eq_preset_ult(preset, ult)),
        (Some(ult), preset) => {
            results.push(ult.map_or(Err(SessionError::InvalidValue), |u| session.set_ult(u)));
            if let Some(preset) = preset {
                results.push(session.set_eq_preset(preset));
            }
        }
        (None, Some(preset)) => results.push(session.set_eq_preset(preset)),
        (None, None) => {}
    }
    if let Some(v) = a.volume {
        results.push(session.set_volume(v));
    }
    if let Some(auto) = a.dsee_auto_off {
        results.push(session.set_dsee_auto_off(auto));
    }
    results
}

fn ult_level(v: u8) -> Option<UltLevel> {
    match v {
        0 => Some(UltLevel::Off),
        1 => Some(UltLevel::Ult1),
        2 => Some(UltLevel::Ult2),
        _ => None,
    }
}
