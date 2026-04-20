use chrono::{DateTime, Utc};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub t: Duration,
    pub lat: f64,
    pub lon: f64,
    pub altitude_m: Option<f32>,
    pub speed_mps: Option<f32>,
    pub heart_rate_bpm: Option<u8>,
    pub cadence_rpm: Option<u8>,
    pub power_w: Option<u16>,
    pub distance_m: Option<f64>,
    pub elev_gain_cum_m: Option<f32>,
    pub gradient_pct: Option<f32>,  // percent slope (vertical/horizontal × 100)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub start_time: DateTime<Utc>,
    pub samples: Vec<Sample>,
}

impl Activity {
    pub fn from_samples(start_time: DateTime<Utc>, samples: Vec<Sample>) -> Self {
        Self { start_time, samples }
    }

    pub fn duration(&self) -> Duration {
        self.samples.last().map(|s| s.t).unwrap_or_default()
    }

    /// If any sample already has `distance_m`, assume all do and return unchanged.
    /// Otherwise, cumulate haversine distance between consecutive lat/lon pairs,
    /// starting from 0.0 on the first sample.
    pub fn fill_derived_distance(&mut self) {
        if self.samples.iter().all(|s| s.distance_m.is_some()) {
            return;
        }
        let mut acc = 0.0f64;
        for i in 0..self.samples.len() {
            if i == 0 {
                self.samples[0].distance_m = Some(0.0);
            } else {
                let prev = &self.samples[i - 1];
                let curr = &self.samples[i];
                let step = crate::geo::haversine_m(prev.lat, prev.lon, curr.lat, curr.lon);
                acc += step;
                self.samples[i].distance_m = Some(acc);
            }
        }
    }

    /// Fill `speed_mps` on samples where it is missing, by finite-differencing
    /// `distance_m` against `t`.
    ///
    /// - Requires `distance_m` to be populated — call `fill_derived_distance`
    ///   first when loading GPS-only data.
    /// - No-op for samples that already have a `speed_mps` value; per-sample
    ///   decision, not all-or-nothing.
    /// - Interior samples use a central difference: `(d[i+1] - d[i-1]) / (t[i+1] - t[i-1])`.
    /// - First and last samples use a one-sided (forward/backward) difference.
    /// - Single-sample activities leave speed unset.
    pub fn fill_derived_speed(&mut self) {
        let n = self.samples.len();
        if n < 2 {
            return;
        }
        for i in 0..n {
            if self.samples[i].speed_mps.is_some() {
                continue;
            }
            let (j_lo, j_hi) = if i == 0 {
                (0, 1)
            } else if i == n - 1 {
                (n - 2, n - 1)
            } else {
                (i - 1, i + 1)
            };
            let (Some(d_lo), Some(d_hi)) = (
                self.samples[j_lo].distance_m,
                self.samples[j_hi].distance_m,
            ) else {
                continue; // can't derive without both distances
            };
            let dt = self.samples[j_hi].t.as_secs_f64() - self.samples[j_lo].t.as_secs_f64();
            if dt <= 0.0 {
                continue;
            }
            let v = ((d_hi - d_lo) / dt) as f32;
            self.samples[i].speed_mps = Some(v);
        }
    }

    /// Apply a time-windowed moving average to `speed_mps` on every sample
    /// that currently has a value. Samples with `speed_mps == None` are left
    /// unchanged; for the averaging, they are excluded from both pointer
    /// advancement and sum. Implementation: build parallel `ts`/`vs` vectors of
    /// only the samples with speed, smooth, and write back.
    pub fn smooth_speed(&mut self, window: Duration) {
        let mut ts = Vec::with_capacity(self.samples.len());
        let mut vs = Vec::with_capacity(self.samples.len());
        let mut indices = Vec::with_capacity(self.samples.len());
        for (i, s) in self.samples.iter().enumerate() {
            if let Some(v) = s.speed_mps {
                ts.push(s.t);
                vs.push(v);
                indices.push(i);
            }
        }
        let out = crate::smooth::moving_avg_time(&ts, &vs, window);
        for (k, &i) in indices.iter().enumerate() {
            self.samples[i].speed_mps = Some(out[k]);
        }
    }

    /// Compute cumulative elevation gain in meters with a hysteresis threshold.
    ///
    /// Anchor-based filter: we track a "confirmed" altitude anchor. When the
    /// current altitude exceeds the anchor by > `threshold_m`, the excess is
    /// added to cumulative gain and the anchor rises to the current altitude.
    /// When the current altitude falls below the anchor by > `threshold_m`,
    /// the anchor drops to the current altitude (but cumulative gain is NOT
    /// decremented — elevation gain only counts upward motion).
    ///
    /// Samples without altitude carry forward the last computed gain (so the
    /// output is dense on `elev_gain_cum_m` as long as at least one prior sample
    /// had altitude). If no sample has altitude, the field stays None everywhere.
    pub fn fill_elev_gain(&mut self, threshold_m: f32) {
        let mut anchor: Option<f32> = None;
        let mut cum: f32 = 0.0;
        let mut any_alt = false;

        for s in self.samples.iter_mut() {
            match s.altitude_m {
                Some(alt) => {
                    any_alt = true;
                    match anchor {
                        None => anchor = Some(alt),
                        Some(a) => {
                            if alt > a + threshold_m {
                                cum += alt - a;
                                anchor = Some(alt);
                            } else if alt < a - threshold_m {
                                anchor = Some(alt);
                            }
                        }
                    }
                    s.elev_gain_cum_m = Some(cum);
                }
                None => {
                    s.elev_gain_cum_m = if any_alt { Some(cum) } else { None };
                }
            }
        }
    }

    /// Compute gradient (percent slope) at each sample from altitude over distance,
    /// using a rolling window of approximately `window_m` meters of distance.
    ///
    /// For each sample `i`, find indices `j < i < k` such that
    /// `distance[k] - distance[j]` is as close to `window_m` as possible. Then
    /// `gradient_pct = (altitude[k] - altitude[j]) / (distance[k] - distance[j]) * 100`.
    ///
    /// Requires both `altitude_m` and `distance_m` to be populated on neighboring
    /// samples. Endpoints use whatever window is available. Samples missing
    /// altitude or distance get `None`.
    pub fn fill_gradient(&mut self, window_m: f32) {
        let n = self.samples.len();
        if n < 2 {
            return;
        }
        let half = (window_m / 2.0) as f64;

        // Precompute distances and altitudes for fast access.
        let d: Vec<Option<f64>> = self.samples.iter().map(|s| s.distance_m).collect();
        let a: Vec<Option<f32>> = self.samples.iter().map(|s| s.altitude_m).collect();

        for i in 0..n {
            if a[i].is_none() || d[i].is_none() {
                self.samples[i].gradient_pct = None;
                continue;
            }
            let d_i = d[i].unwrap();

            // Walk outward from i to find j (earliest) and k (latest) within half window.
            let mut j = i;
            while j > 0 {
                if let Some(dj) = d[j - 1] {
                    if d_i - dj > half { break; }
                    j -= 1;
                } else {
                    break;
                }
            }
            let mut k = i;
            while k + 1 < n {
                if let Some(dk) = d[k + 1] {
                    if dk - d_i > half { break; }
                    k += 1;
                } else {
                    break;
                }
            }

            if j == k {
                // No span available (single point); gradient is undefined.
                self.samples[i].gradient_pct = None;
                continue;
            }

            let (dj, dk) = match (d[j], d[k]) {
                (Some(dj), Some(dk)) => (dj, dk),
                _ => {
                    self.samples[i].gradient_pct = None;
                    continue;
                }
            };
            let (aj, ak) = match (a[j], a[k]) {
                (Some(aj), Some(ak)) => (aj, ak),
                _ => {
                    self.samples[i].gradient_pct = None;
                    continue;
                }
            };

            let dist = dk - dj;
            if dist <= 0.0 {
                self.samples[i].gradient_pct = None;
                continue;
            }
            let slope = (ak - aj) as f64 / dist; // dimensionless
            self.samples[i].gradient_pct = Some((slope * 100.0) as f32);
        }
    }

    /// Return an interpolated `Sample` at time `t` relative to the activity's start.
    ///
    /// - Clamps to first/last sample when `t` is outside the activity range.
    /// - Panics if the activity has zero samples.
    /// - Linear interpolation for: lat, lon, altitude_m, speed_mps, heart_rate_bpm,
    ///   power_w, distance_m, elev_gain_cum_m, gradient_pct.
    /// - Nearest-neighbor for: cadence_rpm (steps discontinuously in real data).
    /// - Any continuous metric where either endpoint is None returns None.
    pub fn sample_at(&self, t: Duration) -> Sample {
        assert!(!self.samples.is_empty(), "sample_at on empty activity");

        // Clamp.
        let first = &self.samples[0];
        if t <= first.t { return first.clone(); }
        let last = self.samples.last().unwrap();
        if t >= last.t { return last.clone(); }

        // Binary search for the pair (i-1, i) bracketing t.
        let idx = match self.samples.binary_search_by_key(&t, |s| s.t) {
            Ok(i) => return self.samples[i].clone(),
            Err(i) => i, // insertion point — prev is i-1, next is i
        };
        let lo = &self.samples[idx - 1];
        let hi = &self.samples[idx];

        let span = (hi.t - lo.t).as_secs_f64();
        let u = if span > 0.0 {
            (t - lo.t).as_secs_f64() / span
        } else {
            0.0
        };
        let u_f32 = u as f32;

        use crate::interp::*;
        Sample {
            t,
            lat: lerp_f64(lo.lat, hi.lat, u),
            lon: lerp_f64(lo.lon, hi.lon, u),
            altitude_m: lerp_opt_f32(lo.altitude_m, hi.altitude_m, u_f32),
            speed_mps: lerp_opt_f32(lo.speed_mps, hi.speed_mps, u_f32),
            heart_rate_bpm: lerp_opt_u8(lo.heart_rate_bpm, hi.heart_rate_bpm, u_f32),
            cadence_rpm: nearest_opt_u8(lo.cadence_rpm, hi.cadence_rpm, u_f32),
            power_w: lerp_opt_u16(lo.power_w, hi.power_w, u_f32),
            distance_m: lerp_opt_f64(lo.distance_m, hi.distance_m, u),
            elev_gain_cum_m: lerp_opt_f32(lo.elev_gain_cum_m, hi.elev_gain_cum_m, u_f32),
            gradient_pct: lerp_opt_f32(lo.gradient_pct, hi.gradient_pct, u_f32),
        }
    }

    /// Like smooth_speed but for `altitude_m`.
    pub fn smooth_altitude(&mut self, window: Duration) {
        let mut ts = Vec::with_capacity(self.samples.len());
        let mut vs = Vec::with_capacity(self.samples.len());
        let mut indices = Vec::with_capacity(self.samples.len());
        for (i, s) in self.samples.iter().enumerate() {
            if let Some(v) = s.altitude_m {
                ts.push(s.t);
                vs.push(v);
                indices.push(i);
            }
        }
        let out = crate::smooth::moving_avg_time(&ts, &vs, window);
        for (k, &i) in indices.iter().enumerate() {
            self.samples[i].altitude_m = Some(out[k]);
        }
    }
}

#[cfg(test)]
impl Sample {
    /// Test helper: a blank Sample at t=0, (0.0, 0.0), all metrics None.
    pub(crate) fn blank() -> Self {
        Sample {
            t: std::time::Duration::ZERO,
            lat: 0.0, lon: 0.0,
            altitude_m: None, speed_mps: None,
            heart_rate_bpm: None, cadence_rpm: None,
            power_w: None, distance_m: None,
            elev_gain_cum_m: None,
            gradient_pct: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::time::Duration;

    #[test]
    fn from_samples_builds_activity() {
        let samples = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     altitude_m: Some(100.0), speed_mps: None,
                     heart_rate_bpm: None, cadence_rpm: None,
                     power_w: None, distance_m: None,
                     elev_gain_cum_m: None, gradient_pct: None },
        ];
        let a = Activity::from_samples(Utc.timestamp_opt(0, 0).unwrap(), samples);
        assert_eq!(a.samples.len(), 1);
        assert_eq!(a.duration(), Duration::from_secs(0));
    }

    #[test]
    fn fill_distance_cumulates() {
        use chrono::Utc;
        use std::time::Duration;
        let samples = vec![
            Sample { t: Duration::ZERO, lat: 0.0, lon: 0.0, ..Sample::blank() },
            Sample {
                t: Duration::from_secs(1),
                lat: 0.0,
                lon: 0.001, // ~111 m east at equator (cos(0)=1)
                ..Sample::blank()
            },
        ];
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_distance();
        assert_eq!(a.samples[0].distance_m, Some(0.0));
        let d1 = a.samples[1].distance_m.unwrap();
        assert!(d1 > 100.0 && d1 < 120.0, "got {}", d1);
    }

    #[test]
    fn fill_distance_noop_if_all_present() {
        use chrono::Utc;
        use std::time::Duration;
        let samples = vec![
            Sample { t: Duration::ZERO, lat: 0.0, lon: 0.0, distance_m: Some(5.0), ..Sample::blank() },
            Sample { t: Duration::from_secs(1), lat: 0.0, lon: 0.001, distance_m: Some(20.0), ..Sample::blank() },
        ];
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_distance();
        assert_eq!(a.samples[0].distance_m, Some(5.0));
        assert_eq!(a.samples[1].distance_m, Some(20.0));
    }

    #[test]
    fn fill_speed_from_constant_distance_rate() {
        // 11 samples at 1 Hz, distance grows 10 m/s (0, 10, 20, ..., 100).
        let samples: Vec<Sample> = (0..11)
            .map(|i| Sample {
                t: Duration::from_secs(i as u64),
                lat: 0.0, lon: 0.0,
                distance_m: Some(i as f64 * 10.0),
                ..Sample::blank()
            })
            .collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_speed();
        // Middle samples should have speed very close to 10 m/s.
        for s in &a.samples[1..a.samples.len() - 1] {
            let v = s.speed_mps.unwrap();
            assert!((v - 10.0).abs() < 0.01, "got {}", v);
        }
    }

    #[test]
    fn fill_speed_noop_when_present() {
        let samples = vec![
            Sample {
                t: Duration::ZERO, lat: 0.0, lon: 0.0,
                distance_m: Some(0.0), speed_mps: Some(5.0),
                ..Sample::blank()
            },
            Sample {
                t: Duration::from_secs(1), lat: 0.0, lon: 0.0,
                distance_m: Some(10.0), speed_mps: Some(5.0),
                ..Sample::blank()
            },
        ];
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_speed();
        assert_eq!(a.samples[0].speed_mps, Some(5.0));
        assert_eq!(a.samples[1].speed_mps, Some(5.0));
    }

    #[test]
    fn fill_speed_endpoints_use_one_sided_difference() {
        // Constant 10 m/s: endpoints should use forward/backward difference and
        // still land on ~10.0.
        let samples: Vec<Sample> = (0..11)
            .map(|i| Sample {
                t: Duration::from_secs(i as u64),
                lat: 0.0, lon: 0.0,
                distance_m: Some(i as f64 * 10.0),
                ..Sample::blank()
            })
            .collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_speed();
        assert!((a.samples[0].speed_mps.unwrap() - 10.0).abs() < 0.01);
        assert!((a.samples[10].speed_mps.unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn fill_speed_irregular_dt() {
        // t = [0, 1, 3], d = [0, 5, 25] → at i=1 central diff = (25-0)/(3-0) = 8.333
        let samples = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0, distance_m: Some(0.0),  ..Sample::blank() },
            Sample { t: Duration::from_secs(1), lat: 0.0, lon: 0.0, distance_m: Some(5.0),  ..Sample::blank() },
            Sample { t: Duration::from_secs(3), lat: 0.0, lon: 0.0, distance_m: Some(25.0), ..Sample::blank() },
        ];
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_speed();
        let v = a.samples[1].speed_mps.unwrap();
        assert!((v - 25.0 / 3.0).abs() < 0.01, "got {}", v);
    }

    #[test]
    fn fill_speed_skips_when_neighbor_distance_missing() {
        // Middle sample has no neighbors with distance → speed stays None.
        let samples = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0, distance_m: None,       ..Sample::blank() },
            Sample { t: Duration::from_secs(1), lat: 0.0, lon: 0.0, distance_m: None,       ..Sample::blank() },
            Sample { t: Duration::from_secs(2), lat: 0.0, lon: 0.0, distance_m: None,       ..Sample::blank() },
        ];
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_derived_speed();
        assert!(a.samples.iter().all(|s| s.speed_mps.is_none()));
    }

    #[test]
    fn smooth_speed_flattens_alternation() {
        let samples: Vec<Sample> = (0..10).map(|i| Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0, lon: 0.0,
            speed_mps: Some(if i % 2 == 0 { 1.0 } else { 3.0 }),
            ..Sample::blank()
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.smooth_speed(Duration::from_secs(3));
        for i in 2..8 {
            let v = a.samples[i].speed_mps.unwrap();
            assert!((v - 2.0).abs() < 0.2);
        }
    }

    #[test]
    fn smooth_altitude_flattens_jitter() {
        let samples: Vec<Sample> = (0..10).map(|i| Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0, lon: 0.0,
            altitude_m: Some(if i % 2 == 0 { 100.0 } else { 110.0 }),
            ..Sample::blank()
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.smooth_altitude(Duration::from_secs(5));
        for i in 2..8 {
            let v = a.samples[i].altitude_m.unwrap();
            assert!((v - 105.0).abs() < 1.5);
        }
    }

    #[test]
    fn elev_gain_counts_net_climb() {
        // 21 samples, altitude climbs linearly 100 → 200 (100m total gain).
        let samples: Vec<Sample> = (0..21).map(|i| Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0, lon: 0.0,
            altitude_m: Some(100.0 + (i as f32) * 5.0),  // 100, 105, 110, ..., 200
            ..Sample::blank()
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_elev_gain(3.0);
        let total = a.samples.last().unwrap().elev_gain_cum_m.unwrap();
        assert!((total - 100.0).abs() < 1.0, "got {}", total);
    }

    #[test]
    fn elev_gain_ignores_noise_below_threshold() {
        // 51 samples, altitude noisy ±1m around a linear climb 100 → 150 (50m net).
        let samples: Vec<Sample> = (0..51).map(|i| {
            let base = 100.0 + (i as f32);  // +1 m per sample over 50 samples -> +50 m
            let noise = if i % 2 == 0 { -1.0 } else { 1.0 }; // ±1 m
            Sample {
                t: Duration::from_secs(i as u64),
                lat: 0.0, lon: 0.0,
                altitude_m: Some(base + noise),
                ..Sample::blank()
            }
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_elev_gain(3.0);
        let total = a.samples.last().unwrap().elev_gain_cum_m.unwrap();
        // Net climb is ~50m; the hysteresis filter should keep it near 50, not
        // inflate due to noise. Allow generous tolerance because hysteresis on
        // linear + ±1m noise produces ~50m ± hysteresis.
        assert!(total < 60.0 && total > 40.0, "got {}", total);
    }

    #[test]
    fn gradient_constant_10_percent_climb() {
        // altitude rises 1.0 m per sample, distance increases 10.0 m per sample,
        // so gradient should be 10.0%. 30 samples to get plenty of interior.
        let samples: Vec<Sample> = (0..30).map(|i| Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0, lon: 0.0,
            altitude_m: Some((i as f32) * 1.0),
            distance_m: Some((i as f64) * 10.0),
            ..Sample::blank()
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_gradient(50.0);
        // Middle should be within a fraction of 10%.
        let mid = a.samples[15].gradient_pct.unwrap();
        assert!((mid - 10.0).abs() < 0.5, "got {}", mid);
    }

    #[test]
    fn gradient_zero_on_flat() {
        let samples: Vec<Sample> = (0..30).map(|i| Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0, lon: 0.0,
            altitude_m: Some(100.0),
            distance_m: Some((i as f64) * 10.0),
            ..Sample::blank()
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_gradient(50.0);
        for s in &a.samples[5..25] {
            assert!(s.gradient_pct.unwrap().abs() < 0.01);
        }
    }

    #[test]
    fn gradient_none_when_altitude_missing() {
        let samples: Vec<Sample> = (0..10).map(|i| Sample {
            t: Duration::from_secs(i as u64),
            lat: 0.0, lon: 0.0,
            altitude_m: None,
            distance_m: Some((i as f64) * 10.0),
            ..Sample::blank()
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_gradient(50.0);
        assert!(a.samples.iter().all(|s| s.gradient_pct.is_none()));
    }

    #[test]
    fn sample_at_interpolates_speed_linearly() {
        let s = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     speed_mps: Some(10.0), ..Sample::blank() },
            Sample { t: Duration::from_secs(10), lat: 0.0, lon: 0.0,
                     speed_mps: Some(20.0), ..Sample::blank() },
        ];
        let a = Activity::from_samples(Utc::now(), s);
        let mid = a.sample_at(Duration::from_secs(5));
        assert!((mid.speed_mps.unwrap() - 15.0).abs() < 0.01);
    }

    #[test]
    fn sample_at_clamps_before_start() {
        let s = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     speed_mps: Some(10.0), ..Sample::blank() },
            Sample { t: Duration::from_secs(10), lat: 0.0, lon: 0.0,
                     speed_mps: Some(20.0), ..Sample::blank() },
        ];
        let a = Activity::from_samples(Utc::now(), s);
        // Durations can't be negative, so "before start" means exactly 0.
        let out = a.sample_at(Duration::ZERO);
        assert_eq!(out.speed_mps, Some(10.0));
    }

    #[test]
    fn sample_at_clamps_after_end() {
        let s = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     speed_mps: Some(10.0), ..Sample::blank() },
            Sample { t: Duration::from_secs(10), lat: 0.0, lon: 0.0,
                     speed_mps: Some(20.0), ..Sample::blank() },
        ];
        let a = Activity::from_samples(Utc::now(), s);
        let out = a.sample_at(Duration::from_secs(100));
        assert_eq!(out.speed_mps, Some(20.0));
    }

    #[test]
    fn sample_at_interpolates_lat_lon() {
        let s = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0, ..Sample::blank() },
            Sample { t: Duration::from_secs(10), lat: 1.0, lon: 2.0, ..Sample::blank() },
        ];
        let a = Activity::from_samples(Utc::now(), s);
        let mid = a.sample_at(Duration::from_secs(5));
        assert!((mid.lat - 0.5).abs() < 1e-9);
        assert!((mid.lon - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sample_at_uses_nearest_for_cadence() {
        let s = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     cadence_rpm: Some(80), ..Sample::blank() },
            Sample { t: Duration::from_secs(10), lat: 0.0, lon: 0.0,
                     cadence_rpm: Some(90), ..Sample::blank() },
        ];
        let a = Activity::from_samples(Utc::now(), s);
        let near_first = a.sample_at(Duration::from_secs(3));
        assert_eq!(near_first.cadence_rpm, Some(80));
        let near_second = a.sample_at(Duration::from_secs(7));
        assert_eq!(near_second.cadence_rpm, Some(90));
    }

    #[test]
    fn sample_at_none_when_either_endpoint_none() {
        let s = vec![
            Sample { t: Duration::from_secs(0), lat: 0.0, lon: 0.0,
                     heart_rate_bpm: None, ..Sample::blank() },
            Sample { t: Duration::from_secs(10), lat: 0.0, lon: 0.0,
                     heart_rate_bpm: Some(150), ..Sample::blank() },
        ];
        let a = Activity::from_samples(Utc::now(), s);
        let mid = a.sample_at(Duration::from_secs(5));
        assert!(mid.heart_rate_bpm.is_none());
    }

    #[test]
    fn elev_gain_does_not_subtract_on_descent() {
        // 21 samples, altitude: 100 → 200 (first 11 samples), then 200 → 100 (last 10).
        let samples: Vec<Sample> = (0..21).map(|i| {
            let alt = if i <= 10 { 100.0 + (i as f32) * 10.0 }
                      else { 200.0 - ((i - 10) as f32) * 10.0 };
            Sample {
                t: Duration::from_secs(i as u64),
                lat: 0.0, lon: 0.0,
                altitude_m: Some(alt),
                ..Sample::blank()
            }
        }).collect();
        let mut a = Activity::from_samples(Utc::now(), samples);
        a.fill_elev_gain(3.0);
        let total = a.samples.last().unwrap().elev_gain_cum_m.unwrap();
        assert!((total - 100.0).abs() < 1.0, "got {}", total);
    }
}
