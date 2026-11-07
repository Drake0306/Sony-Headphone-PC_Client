//! The app's ten-band curve and its best fit onto the device's six EQ values.
//! Pure: no I/O. The headphones store six values (Clear Bass plus five bands),
//! 21 steps each, sent as raw 0..=20 with 10 as the centre.

/// Centre frequencies of the ten Advanced bands, in Hz.
pub const ADVANCED_BANDS_HZ: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/// One of the device's six EQ values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceBand {
    pub label: &'static str,
    /// Centre frequency in Hz; `None` for Clear Bass, which is not a band.
    pub hz: Option<u32>,
}

/// The device's six EQ values, in wire order.
pub const DEVICE_BANDS: [DeviceBand; 6] = [
    DeviceBand {
        label: "Clear Bass",
        hz: None,
    },
    DeviceBand {
        label: "400 Hz",
        hz: Some(400),
    },
    DeviceBand {
        label: "1 kHz",
        hz: Some(1000),
    },
    DeviceBand {
        label: "2.5 kHz",
        hz: Some(2500),
    },
    DeviceBand {
        label: "6.3 kHz",
        hz: Some(6300),
    },
    DeviceBand {
        label: "16 kHz",
        hz: Some(16000),
    },
];

/// The labels of [`DEVICE_BANDS`], in wire order.
pub const DEVICE_BAND_LABELS: [&str; 6] = [
    DEVICE_BANDS[0].label,
    DEVICE_BANDS[1].label,
    DEVICE_BANDS[2].label,
    DEVICE_BANDS[3].label,
    DEVICE_BANDS[4].label,
    DEVICE_BANDS[5].label,
];

/// Lowest and highest level of a band, in either curve.
pub const LEVEL_MIN: i8 = -10;
pub const LEVEL_MAX: i8 = 10;

/// Value at `hz` on the straight line between (`lo_hz`, `lo`) and
/// (`hi_hz`, `hi`) in log frequency.
fn interpolate(lo_hz: f64, lo: i8, hi_hz: f64, hi: i8, hz: f64) -> f64 {
    let t = (hz / lo_hz).ln() / (hi_hz / lo_hz).ln();
    f64::from(lo) + t * (f64::from(hi) - f64::from(lo))
}

/// Rounded half away from zero, then clamped to `LEVEL_MIN..=LEVEL_MAX`.
fn level(v: f64) -> i8 {
    v.round().clamp(f64::from(LEVEL_MIN), f64::from(LEVEL_MAX)) as i8
}

/// The closest six device values for a ten-band curve (levels −10..=10, in
/// the order of [`ADVANCED_BANDS_HZ`]): Clear Bass is the mean of 31..250 Hz,
/// 400 Hz interpolates 250↔500, 1 kHz is direct, 2.5 kHz interpolates 2k↔4k,
/// 6.3 kHz interpolates 4k↔8k, 16 kHz is direct; interpolation is in log
/// frequency; each result is rounded half away from zero and clamped to −10..=10.
pub fn best_fit(curve: &[i8; 10]) -> [i8; 6] {
    let c = curve;
    let bass: i32 = c[..4].iter().map(|&v| i32::from(v)).sum();
    [
        level(f64::from(bass) / 4.0),
        level(interpolate(250.0, c[3], 500.0, c[4], 400.0)),
        level(f64::from(c[5])),
        level(interpolate(2000.0, c[6], 4000.0, c[7], 2500.0)),
        level(interpolate(4000.0, c[7], 8000.0, c[8], 6300.0)),
        level(f64::from(c[9])),
    ]
}

/// A level (−10..=10, clamped) on the device's raw 0..=20 scale.
pub fn to_raw(v: i8) -> u8 {
    (v.clamp(LEVEL_MIN, LEVEL_MAX) + 10) as u8
}

/// A raw device value (0..=20, clamped) as a level −10..=10.
pub fn from_raw(r: u8) -> i8 {
    r.min(20) as i8 - 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_fit_fixtures() {
        assert_eq!(best_fit(&[0; 10]), [0; 6]);
        // +6 on 31..250 Hz: Clear Bass 6. 400 Hz sits log2(1.6) = 0.678 of the
        // way from 250 (+6) to 500 (0): 6 × (1 − 0.678) = 1.93, rounds to 2.
        assert_eq!(
            best_fit(&[6, 6, 6, 6, 0, 0, 0, 0, 0, 0]),
            [6, 2, 0, 0, 0, 0]
        );
        // +10 shelf on 4, 8 and 16 kHz (above 2 kHz): 2.5 kHz sits
        // log2(1.25) = 0.322 of the way from 2k (0) to 4k (+10): 3.22 → 3;
        // 6.3 kHz is between 4k and 8k, both +10.
        assert_eq!(
            best_fit(&[0, 0, 0, 0, 0, 0, 0, 10, 10, 10]),
            [0, 0, 0, 3, 10, 10]
        );
        // Shelf from 2 kHz up: every band at or above 2 kHz is +10.
        assert_eq!(
            best_fit(&[0, 0, 0, 0, 0, 0, 10, 10, 10, 10]),
            [0, 0, 0, 10, 10, 10]
        );
        // 2.5 kHz: 0.322 of the way from 2k (0) to 4k (−10): −3.22 → −3.
        // 6.3 kHz sits log2(1.575) = 0.655 of the way from 4k (−10) to 8k (0):
        // −10 × 0.345 = −3.45 → −3. 1 kHz and 16 kHz are direct.
        assert_eq!(
            best_fit(&[0, 0, 0, 0, 0, 4, 0, -10, 0, -7]),
            [0, 0, 4, -3, -3, -7]
        );
        // A V shape.
        assert_eq!(
            best_fit(&[8, 6, 4, 2, 0, -2, 0, 2, 4, 6]),
            [5, 1, -2, 1, 3, 6]
        );
    }

    #[test]
    fn best_fit_rounds_half_away_from_zero() {
        // Clear Bass means of exactly ±0.5, ±1.5, ±0.25 and ±0.75.
        assert_eq!(best_fit(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0])[0], 1);
        assert_eq!(best_fit(&[-1, -1, 0, 0, 0, 0, 0, 0, 0, 0])[0], -1);
        assert_eq!(best_fit(&[3, 3, 0, 0, 0, 0, 0, 0, 0, 0])[0], 2);
        assert_eq!(best_fit(&[-3, -3, 0, 0, 0, 0, 0, 0, 0, 0])[0], -2);
        assert_eq!(best_fit(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0])[0], 0);
        assert_eq!(best_fit(&[-1, 0, 0, 0, 0, 0, 0, 0, 0, 0])[0], 0);
        assert_eq!(best_fit(&[3, 0, 0, 0, 0, 0, 0, 0, 0, 0])[0], 1);
        assert_eq!(best_fit(&[-3, 0, 0, 0, 0, 0, 0, 0, 0, 0])[0], -1);
        // Interpolated: 400 Hz from 250 (−6) to 500 (0) is −1.93 → −2.
        assert_eq!(best_fit(&[0, 0, 0, -6, 0, 0, 0, 0, 0, 0])[1], -2);
    }

    #[test]
    fn best_fit_clamps() {
        assert_eq!(best_fit(&[10; 10]), [10; 6]);
        assert_eq!(best_fit(&[-10; 10]), [-10; 6]);
        assert_eq!(best_fit(&[100; 10]), [10; 6]);
        assert_eq!(best_fit(&[i8::MIN; 10]), [-10; 6]);
        assert_eq!(best_fit(&[i8::MAX; 10]), [10; 6]);
    }

    #[test]
    fn raw_scale_is_centred_on_ten() {
        assert_eq!(to_raw(-10), 0);
        assert_eq!(to_raw(0), 10);
        assert_eq!(to_raw(10), 20);
        assert_eq!(to_raw(42), 20);
        assert_eq!(to_raw(-42), 0);
        assert_eq!(from_raw(0), -10);
        assert_eq!(from_raw(10), 0);
        assert_eq!(from_raw(20), 10);
        assert_eq!(from_raw(255), 10);
        for v in LEVEL_MIN..=LEVEL_MAX {
            assert_eq!(from_raw(to_raw(v)), v);
        }
    }

    #[test]
    fn device_band_labels() {
        assert_eq!(
            DEVICE_BAND_LABELS,
            [
                "Clear Bass",
                "400 Hz",
                "1 kHz",
                "2.5 kHz",
                "6.3 kHz",
                "16 kHz"
            ]
        );
    }
}
