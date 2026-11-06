//! First-match evaluation of scenes against the current inputs.

use super::model::{Inputs, Scene, Trigger};

/// Parses `H:MM` or `HH:MM` into minutes since midnight (`00:00`..=`23:59`).
pub fn parse_hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Whether `now` is in `[from, to)`, wrapping midnight when `to < from`.
/// `from == to` means the whole day.
pub fn time_in_range(now: u16, from: u16, to: u16) -> bool {
    match from.cmp(&to) {
        std::cmp::Ordering::Equal => true,
        std::cmp::Ordering::Less => now >= from && now < to,
        std::cmp::Ordering::Greater => now >= from || now < to,
    }
}

fn matches(trigger: &Trigger, inputs: &Inputs) -> bool {
    match trigger {
        Trigger::Manual => false,
        Trigger::HeadphonesConnected => inputs.connected,
        Trigger::TimeRange { from, to } => match (parse_hhmm(from), parse_hhmm(to)) {
            (Some(from), Some(to)) => time_in_range(inputs.now_minutes, from, to),
            _ => false,
        },
        Trigger::FrontmostApp { name } => inputs
            .frontmost_app
            .as_deref()
            .is_some_and(|app| app.to_lowercase() == name.trim().to_lowercase()),
        Trigger::MicInUse => inputs.mic_in_use,
    }
}

/// The first enabled scene, in list order, whose trigger matches.
pub fn active_scene<'a>(scenes: &'a [Scene], inputs: &Inputs) -> Option<&'a Scene> {
    scenes
        .iter()
        .find(|s| s.enabled && matches(&s.trigger, inputs))
}

/// The active id after deleting `deleted`: cleared when it was the active one.
pub fn active_id_after_delete(active: Option<&str>, deleted: &str) -> Option<String> {
    active.filter(|id| *id != deleted).map(str::to_owned)
}
