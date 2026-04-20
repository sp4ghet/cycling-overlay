/// Linear interpolate between `a` and `b` at parameter `u` ∈ [0, 1].
/// Returns None if either input is None (can't interpolate through missing data).
pub(crate) fn lerp_opt_f32(a: Option<f32>, b: Option<f32>, u: f32) -> Option<f32> {
    match (a, b) {
        (Some(av), Some(bv)) => Some(av + (bv - av) * u),
        _ => None,
    }
}

pub(crate) fn lerp_opt_f64(a: Option<f64>, b: Option<f64>, u: f64) -> Option<f64> {
    match (a, b) {
        (Some(av), Some(bv)) => Some(av + (bv - av) * u),
        _ => None,
    }
}

pub(crate) fn lerp_f64(a: f64, b: f64, u: f64) -> f64 {
    a + (b - a) * u
}

/// Nearest-neighbor over `Option<u8>` given parameter `u` ∈ [0, 1].
pub(crate) fn nearest_opt_u8(a: Option<u8>, b: Option<u8>, u: f32) -> Option<u8> {
    if u < 0.5 { a } else { b }
}

/// Linear interpolate `Option<u8>` (heart rate) and `Option<u16>` (power) —
/// values that are physically continuous but stored as integers for size.
pub(crate) fn lerp_opt_u8(a: Option<u8>, b: Option<u8>, u: f32) -> Option<u8> {
    match (a, b) {
        (Some(av), Some(bv)) => Some((av as f32 + (bv as f32 - av as f32) * u).round() as u8),
        _ => None,
    }
}

pub(crate) fn lerp_opt_u16(a: Option<u16>, b: Option<u16>, u: f32) -> Option<u16> {
    match (a, b) {
        (Some(av), Some(bv)) => Some((av as f32 + (bv as f32 - av as f32) * u).round() as u16),
        _ => None,
    }
}
