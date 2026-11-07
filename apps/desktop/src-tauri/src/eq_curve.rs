//! The Advanced equalizer's ten-band curve: `eq_curve.json` in the settings
//! directory, and sending its best fit to the headphones' six EQ values.
//! The curve itself never goes to the device; only the fit does, through the
//! same `SetEqBands` path as the Sound screen's faders.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mdr_proto::eq::{best_fit, to_raw, LEVEL_MAX, LEVEL_MIN};
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

/// The curve in `dir`. A missing file gives the flat curve. An unreadable,
/// corrupt or out-of-range one is renamed to `eq_curve.json.bad` and gives
/// the flat curve.
pub fn load(dir: &Path) -> EqCurve {
    let path = path_in(dir);
    let parsed = std::fs::read_to_string(&path).and_then(|text| {
        let curve: EqCurve = serde_json::from_str(&text).map_err(io::Error::other)?;
        check_curve(&curve.bands).map_err(io::Error::other)?;
        Ok(curve)
    });
    match parsed {
        Ok(curve) => curve,
        Err(e) if e.kind() == io::ErrorKind::NotFound => EqCurve::default(),
        Err(e) => {
            eprintln!(
                "hairline: eq_curve.json unreadable ({e}); kept as eq_curve.json.bad, using a flat curve"
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

/// The six fitted levels (−10..=10) for `curve` and the broker command that
/// writes them, on the device's raw 0..=20 scale, through `SetEqBands`.
pub fn fit_command(curve: &EqCurve) -> ([i8; 6], BrokerCmd) {
    let fitted = best_fit(&curve.bands);
    let raw = fitted.iter().map(|&v| to_raw(v)).collect();
    (fitted, BrokerCmd::SetEqBands(raw))
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
    let dir = settings::dir(&app).map_err(|e| e.to_string())?;
    // Held across save and update so concurrent saves cannot interleave.
    let mut held = state
        .lock()
        .map_err(|_| "the curve is unavailable".to_string())?;
    save(&dir, &curve).map_err(|e| format!("could not save the curve: {e}"))?;
    *held = curve;
    Ok(())
}

/// The six levels `send_best_fit` would write, without sending them.
#[tauri::command]
pub fn best_fit_preview(state: State<'_, Mutex<EqCurve>>) -> [i8; 6] {
    best_fit(&current(&state).unwrap_or_default().bands)
}

/// Writes the best fit of the stored curve to the headphones' six EQ values
/// (preset and ULT are kept) and returns the six fitted levels.
#[tauri::command]
pub fn send_best_fit(
    broker: State<'_, Broker>,
    state: State<'_, Mutex<EqCurve>>,
) -> Result<[i8; 6], String> {
    let (fitted, cmd) = fit_command(&current(&state)?);
    broker.send(cmd)?;
    Ok(fitted)
}
