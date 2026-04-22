//! Pure math helpers shared by Meter and Gauge. No tiny-skia dependencies.
//!
//! These helpers are consumed by the Meter (Task 4+) and Gauge (Task 7+)
//! widget implementations. The `allow(dead_code)` below applies until those
//! call sites land.
#![allow(dead_code)]

/// Fraction of `v` between `min` and `max`, clamped to [0, 1].
/// If `max <= min` the range is degenerate — returns 0.
pub(crate) fn frac(v: f32, min: f32, max: f32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((v - min) / (max - min)).clamp(0.0, 1.0)
}

/// Pick a "nice" major-tick interval that divides `max - min` into roughly
/// 6 segments and lands on a round number in the 1-2-5-10 family.
///
/// Intermediate arithmetic runs in `f64` so the returned `f32` matches the
/// closest representation of the semantic target (e.g. `0.05`, not
/// `0.049999997`) when the f32 path would introduce a 1-ULP drift.
pub(crate) fn nice_major_interval(min: f32, max: f32) -> f32 {
    let range = ((max - min) as f64).abs();
    if range <= 0.0 {
        return 1.0;
    }
    let raw = range / 6.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;
    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.0 {
        2.0
    } else if normalized < 7.0 {
        5.0
    } else {
        10.0
    };
    (nice * magnitude) as f32
}

/// Walk values from `min` to `max` inclusive at `step`. Uses integer indexing
/// to avoid accumulated float drift over many steps.
pub(crate) fn tick_values(min: f32, max: f32, step: f32) -> impl Iterator<Item = f32> {
    let n = ((max - min) / step).round() as i64;
    (0..=n).map(move |k| min + (k as f32) * step)
}

/// Convert from our user-facing angle (0° up, clockwise positive) to
/// tiny-skia's (0° right, counterclockwise positive).
///
/// Identity: `to_skia(0) = 90` (up), `to_skia(90) = 0` (right),
/// `to_skia(-90) = 180` (left), `to_skia(180) = -90` (down).
pub(crate) fn to_skia_angle(deg_up_cw: f32) -> f32 {
    90.0 - deg_up_cw
}

/// Linearly interpolate between angles from `start_deg` to `end_deg` at
/// `frac` in [0, 1]. If `end_deg < start_deg`, adds 360° internally so the
/// sweep wraps clockwise through the top.
pub(crate) fn angle_lerp(start_deg: f32, end_deg: f32, frac: f32) -> f32 {
    let end_eff = if end_deg >= start_deg {
        end_deg
    } else {
        end_deg + 360.0
    };
    start_deg + (end_eff - start_deg) * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_clamps() {
        assert_eq!(frac(-5.0, 0.0, 100.0), 0.0);
        assert_eq!(frac(50.0, 0.0, 100.0), 0.5);
        assert_eq!(frac(150.0, 0.0, 100.0), 1.0);
    }

    #[test]
    fn nice_interval_picks_round_numbers() {
        assert_eq!(nice_major_interval(0.0, 100.0), 20.0);
        assert_eq!(nice_major_interval(0.0, 60.0), 10.0);
        assert_eq!(nice_major_interval(0.0, 5.0), 1.0);
        assert_eq!(nice_major_interval(0.0, 0.3), 0.05);
        assert_eq!(nice_major_interval(0.0, 200.0), 50.0);
    }

    #[test]
    fn tick_values_inclusive() {
        let vs: Vec<f32> = tick_values(0.0, 10.0, 2.0).collect();
        assert_eq!(vs, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    }

    #[test]
    fn to_skia_angle_conversions() {
        // User convention: 0° = up (12 o'clock), clockwise positive.
        // tiny-skia convention: 0° = right (3 o'clock), counterclockwise positive.
        assert!((to_skia_angle(0.0) - 90.0).abs() < 1e-4);
        assert!((to_skia_angle(90.0) - 0.0).abs() < 1e-4);
        assert!((to_skia_angle(-90.0) - 180.0).abs() < 1e-4);
        assert!((to_skia_angle(180.0) - (-90.0)).abs() < 1e-4);
    }

    #[test]
    fn angle_lerp_no_wrap() {
        assert!((angle_lerp(-135.0, 135.0, 0.5) - 0.0).abs() < 1e-4);
        assert!((angle_lerp(-135.0, 135.0, 0.0) - (-135.0)).abs() < 1e-4);
        assert!((angle_lerp(-135.0, 135.0, 1.0) - 135.0).abs() < 1e-4);
    }

    #[test]
    fn angle_lerp_wraps_through_top() {
        // Start 315, end 45 should sweep through top (315 -> 360/0 -> 45).
        // At frac=0.5 we expect angle at the halfway point of a 90° sweep,
        // which is 360° (or 0°, since angles mod 360 are equal).
        let mid = angle_lerp(315.0, 45.0, 0.5);
        let normalized = mid.rem_euclid(360.0);
        assert!(
            normalized < 1e-4 || (normalized - 360.0).abs() < 1e-4,
            "got {} (normalized {})", mid, normalized
        );
    }
}
