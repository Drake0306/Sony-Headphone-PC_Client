//! The Advanced equalizer's ten-band curve: `eq_curve.json` in the settings
//! directory, and sending its best fit to the headphones' six EQ values.
//! The curve itself never goes to the device; only the fit does, in one EQ
//! write that also selects the target preset (Manual, Custom 1 or Custom 2)
//! and keeps the ULT level.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mdr_proto::eq::{best_fit, to_raw, DEVICE_BANDS, LEVEL_MAX, LEVEL_MIN};
use mdr_proto::EqUltCapability;
use mdr_session::DeviceState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::broker::BrokerCmd;
use crate::commands::Broker;
use crate::settings;

const FILE_NAME: &str = "eq_curve.json";
/// Where an unreadable `eq_curve.json` is kept (an older one is replaced).
const BAD_NAME: &str = "eq_curve.json.bad";
/// Written first, then renamed over `eq_curve.json`.
const TMP_NAME: &str = "eq_curve.json.tmp";

/// Number of bands in the Advanced curve (31 Hz .. 16 kHz).
pub const CURVE_BANDS: usize = 10;

/// The Advanced curve: levels −10..=10 at the frequencies of
/// `mdr_proto::eq::ADVANCED_BANDS_HZ`. Flat by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqCurve {
    pub bands: [i8; CURVE_BANDS],
}

fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// `bands` as a curve if there are exactly ten, each within −10..=10.
pub fn check_curve(bands: &[i8]) -> Result<[i8; CURVE_BANDS], String> {
    let bands: [i8; CURVE_BANDS] = bands
        .try_into()
        .map_err(|_| format!("expected {CURVE_BANDS} bands, got {}", bands.len()))?;
    if let Some(b) = bands.iter().find(|b| !(LEVEL_MIN..=LEVEL_MAX).contains(*b)) {
        return Err(format!(
            "band level {b} is outside {LEVEL_MIN}..={LEVEL_MAX}"
        ));
    }
    Ok(bands)
}

/// The curve in `dir`. A missing file gives the flat curve. A corrupt or
/// out-of-range one is renamed to `eq_curve.json.bad` and gives the flat
/// curve. Any other read error (permissions, a directory in its place) is
/// logged and gives the flat curve, leaving the file alone.
pub fn load(dir: &Path) -> EqCurve {
    let path = path_in(dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return EqCurve::default(),
        Err(e) => {
            eprintln!("hairline: could not read eq_curve.json ({e}); using a flat curve");
            return EqCurve::default();
        }
    };
    let parsed = serde_json::from_str::<EqCurve>(&text)
        .map_err(|e| e.to_string())
        .and_then(|curve| check_curve(&curve.bands).map(|_| curve));
    match parsed {
        Ok(curve) => curve,
        Err(e) => {
            eprintln!(
                "hairline: eq_curve.json is not a valid curve ({e}); kept as eq_curve.json.bad, using a flat curve"
            );
            if let Err(e) = std::fs::rename(&path, dir.join(BAD_NAME)) {
                eprintln!("hairline: could not rename eq_curve.json: {e}");
            }
            EqCurve::default()
        }
    }
}

/// Writes the curve to `dir` as pretty JSON, creating `dir` if needed;
/// written to a `.tmp` file first and renamed.
pub fn save(dir: &Path, curve: &EqCurve) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(curve).map_err(io::Error::other)?;
    let tmp = dir.join(TMP_NAME);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path_in(dir))
}

/// The presets best fit may be written into: Manual, Custom 1, Custom 2.
pub const FIT_TARGETS: [u8; 3] = [0xA0, 0xA1, 0xA2];

/// `preset` if best fit may be written into it (Manual, Custom 1 or Custom 2).
pub fn check_target_preset(preset: u8) -> Result<u8, String> {
    if FIT_TARGETS.contains(&preset) {
        Ok(preset)
    } else {
        Err("best fit can only be written into Manual, Custom 1 or Custom 2".to_string())
    }
}

/// The six fitted levels (−10..=10) for `curve` and the broker command that
/// writes them, on the device's raw 0..=20 scale, into `preset` in one frame.
pub fn fit_command(curve: &EqCurve, preset: u8) -> ([i8; 6], BrokerCmd) {
    let fitted = best_fit(&curve.bands);
    let raw = fitted.iter().map(|&v| to_raw(v)).collect();
    (fitted, BrokerCmd::SetEqPresetBands(preset, raw))
}

/// Best fit writes six values of 21 steps; refuses a headphone that reports
/// another EQ shape. An unknown capability (not reported yet) is allowed.
pub fn check_shape(cap: Option<&EqUltCapability>) -> Result<(), String> {
    const STEPS: u8 = (LEVEL_MAX - LEVEL_MIN + 1) as u8;
    match cap {
        Some(c) if usize::from(c.bands) != DEVICE_BANDS.len() || c.steps != STEPS => Err(format!(
            "this headphone's equalizer has {} bands / {} steps; best fit needs {} and {STEPS}",
            c.bands,
            c.steps,
            DEVICE_BANDS.len()
        )),
        _ => Ok(()),
    }
}

/// Checks the device's EQ shape, the target preset and the ten bands, then
/// gives the curve to save, the six fitted levels and the command that writes
/// them into `preset`.
pub fn prepare_send(
    cap: Option<&EqUltCapability>,
    bands: &[i8],
    preset: u8,
) -> Result<(EqCurve, [i8; 6], BrokerCmd), String> {
    check_shape(cap)?;
    let preset = check_target_preset(preset)?;
    let curve = EqCurve {
        bands: check_curve(bands)?,
    };
    let (fitted, cmd) = fit_command(&curve, preset);
    Ok((curve, fitted, cmd))
}

/// Saves `curve` in the settings dir and keeps it, holding the lock across
/// both so concurrent saves cannot interleave.
fn store(app: &AppHandle, state: &State<'_, Mutex<EqCurve>>, curve: EqCurve) -> Result<(), String> {
    let dir = settings::dir(app).map_err(|e| e.to_string())?;
    let mut held = state
        .lock()
        .map_err(|_| "the curve is unavailable".to_string())?;
    save(&dir, &curve).map_err(|e| format!("could not save the curve: {e}"))?;
    *held = curve;
    Ok(())
}

fn current(state: &State<'_, Mutex<EqCurve>>) -> Result<EqCurve, String> {
    state
        .lock()
        .map(|c| *c)
        .map_err(|_| "the curve is unavailable".to_string())
}

#[tauri::command]
pub fn get_eq_curve(state: State<'_, Mutex<EqCurve>>) -> EqCurve {
    current(&state).unwrap_or_default()
}

/// Validates, saves and keeps the curve. Never writes to the headphones.
#[tauri::command]
pub fn set_eq_curve(
    app: AppHandle,
    state: State<'_, Mutex<EqCurve>>,
    bands: Vec<i8>,
) -> Result<(), String> {
    let curve = EqCurve {
        bands: check_curve(&bands)?,
    };
    store(&app, &state, curve)
}

/// The six levels `send_best_fit` would write, without sending them.
#[tauri::command]
pub fn best_fit_preview(state: State<'_, Mutex<EqCurve>>) -> [i8; 6] {
    best_fit(&current(&state).unwrap_or_default().bands)
}

/// Saves the ten `bands` given, then writes their best fit to the headphones'
/// six EQ values in one frame that also selects `preset` (Manual, Custom 1 or
/// Custom 2; the ULT level is kept) and returns the six fitted levels.
/// Refused when the headphones report an EQ that is not six bands of 21
/// steps, or when `preset` is not one of those three.
#[tauri::command]
pub fn send_best_fit(
    app: AppHandle,
    broker: State<'_, Broker>,
    state: State<'_, Mutex<EqCurve>>,
    device: State<'_, Mutex<DeviceState>>,
    bands: Vec<i8>,
    preset: u8,
) -> Result<[i8; 6], String> {
    let cap = device
        .lock()
        .map_err(|_| "the device state is unavailable".to_string())?
        .eq_capability
        .clone();
    let (curve, fitted, cmd) = prepare_send(cap.as_ref(), &bands, preset)?;
    store(&app, &state, curve)?;
    broker.send(cmd)?;
    Ok(fitted)
}
