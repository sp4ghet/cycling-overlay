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
                     power_w: None, distance_m: None },
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
}
