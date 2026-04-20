use std::time::Duration;

/// Time-windowed moving average.
///
/// For each index `i`, averages all `vs[j]` whose timestamp `ts[j]` is within
/// `window` of `ts[i]` (inclusive) — i.e. the reach on each side is `window`,
/// so the total covered span is roughly `2 * window`. This is symmetric and
/// walks both pointers forward in O(n).
///
/// Empty input returns empty output. NaN inputs propagate via the average.
pub(crate) fn moving_avg_time(ts: &[Duration], vs: &[f32], window: Duration) -> Vec<f32> {
    assert_eq!(ts.len(), vs.len(), "moving_avg_time: ts and vs must match");
    let n = ts.len();
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    let reach = window;

    // Two-pointer sliding window
    let mut lo = 0usize;
    let mut hi = 0usize;
    let mut sum: f64 = 0.0;

    for i in 0..n {
        let t_i = ts[i];
        // Expand hi while ts[hi] <= t_i + reach
        while hi < n && ts[hi] <= t_i.saturating_add(reach) {
            sum += vs[hi] as f64;
            hi += 1;
        }
        // Shrink lo while ts[lo] < t_i - reach
        while lo < hi && ts[lo] < t_i.saturating_sub(reach) {
            sum -= vs[lo] as f64;
            lo += 1;
        }
        let count = (hi - lo) as f64;
        out.push((sum / count) as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn moving_avg_flattens_alternating_noise() {
        // 10 samples, 1 Hz, alternating 1.0/3.0 → smoothed ≈ 2.0 in middle.
        let ts: Vec<Duration> = (0..10).map(|i| Duration::from_secs(i)).collect();
        let vs: Vec<f32> = vec![1.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 3.0];
        let out = moving_avg_time(&ts, &vs, Duration::from_secs(3));
        for (i, v) in out.iter().enumerate() {
            if i >= 2 && i < 8 {
                assert!((v - 2.0).abs() < 0.2, "index {} got {}", i, v);
            }
        }
    }

    #[test]
    fn moving_avg_identity_for_constant() {
        let ts: Vec<Duration> = (0..5).map(|i| Duration::from_secs(i)).collect();
        let vs: Vec<f32> = vec![7.0, 7.0, 7.0, 7.0, 7.0];
        let out = moving_avg_time(&ts, &vs, Duration::from_secs(3));
        for v in out {
            assert!((v - 7.0).abs() < 0.001);
        }
    }

    #[test]
    fn moving_avg_empty_input_returns_empty() {
        let ts: Vec<Duration> = vec![];
        let vs: Vec<f32> = vec![];
        let out = moving_avg_time(&ts, &vs, Duration::from_secs(3));
        assert!(out.is_empty());
    }
}
