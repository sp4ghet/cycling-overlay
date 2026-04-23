use crate::{Activity, Sample};
use fitparser::profile::MesgNum;
use fitparser::{FitDataField, Value};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

/// Factor to convert FIT semicircles to degrees.
const SEMICIRCLE_TO_DEG: f64 = 180.0 / 2_147_483_648.0;

#[derive(Debug, thiserror::Error)]
pub enum FitError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("FIT parse error: {0}")]
    Parse(#[from] fitparser::Error),
    #[error("FIT file contains no record messages")]
    Empty,
}

/// Find a field by name and return a reference to its Value.
fn field<'a>(fields: &'a [FitDataField], name: &str) -> Option<&'a Value> {
    fields.iter().find(|f| f.name() == name).map(|f| f.value())
}

/// Extract a float value from a Value, handling the numeric variants we care about.
fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Float64(x) => Some(*x),
        Value::Float32(x) => Some(*x as f64),
        Value::UInt8(x) => Some(*x as f64),
        Value::UInt16(x) => Some(*x as f64),
        Value::UInt32(x) => Some(*x as f64),
        Value::SInt8(x) => Some(*x as f64),
        Value::SInt16(x) => Some(*x as f64),
        Value::SInt32(x) => Some(*x as f64),
        Value::UInt8z(x) => Some(*x as f64),
        Value::UInt16z(x) => Some(*x as f64),
        Value::UInt32z(x) => Some(*x as f64),
        _ => None,
    }
}

fn as_u8(v: &Value) -> Option<u8> {
    match v {
        Value::UInt8(x) | Value::UInt8z(x) | Value::Byte(x) => Some(*x),
        Value::UInt16(x) => Some(*x as u8),
        _ => None,
    }
}

fn as_u16(v: &Value) -> Option<u16> {
    match v {
        Value::UInt16(x) | Value::UInt16z(x) => Some(*x),
        Value::UInt8(x) | Value::UInt8z(x) => Some(*x as u16),
        Value::UInt32(x) | Value::UInt32z(x) => Some(*x as u16),
        _ => None,
    }
}

/// Convert a semicircle `SInt32` position field to degrees. Returns None if the
/// value is not present or not an `SInt32`.
fn semicircle_to_deg(v: &Value) -> Option<f64> {
    match v {
        Value::SInt32(x) => Some(*x as f64 * SEMICIRCLE_TO_DEG),
        // Defensive: if some future fitparser release pre-converts to float degrees,
        // hand back the value unchanged. Guard with a range check so we can tell.
        Value::Float64(x) if x.abs() <= 180.0 => Some(*x),
        Value::Float32(x) if (*x as f64).abs() <= 180.0 => Some(*x as f64),
        _ => None,
    }
}

pub fn load_fit(path: &Path) -> Result<Activity, FitError> {
    let mut reader = BufReader::new(File::open(path)?);
    let records = fitparser::from_reader(&mut reader)?;

    let mut start_time: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut samples: Vec<Sample> = Vec::new();

    for rec in records {
        if rec.kind() != MesgNum::Record {
            continue;
        }
        let fields = rec.fields();

        // timestamp is required to anchor the sample on the activity timeline.
        let ts_value = match field(fields, "timestamp") {
            Some(v) => v,
            None => continue,
        };
        let ts_utc = match ts_value {
            Value::Timestamp(dt) => dt.with_timezone(&chrono::Utc),
            _ => continue,
        };

        if start_time.is_none() {
            start_time = Some(ts_utc);
        }
        let start = start_time.expect("start_time set above");
        // Saturate to zero to guard against records arriving out of order.
        let elapsed = ts_utc.signed_duration_since(start);
        let t = elapsed.to_std().unwrap_or(Duration::ZERO);

        let lat = field(fields, "position_lat")
            .and_then(semicircle_to_deg)
            .unwrap_or(0.0);
        let lon = field(fields, "position_long")
            .and_then(semicircle_to_deg)
            .unwrap_or(0.0);

        // Prefer enhanced_* over base fields when both are present.
        let altitude_m = field(fields, "enhanced_altitude")
            .or_else(|| field(fields, "altitude"))
            .and_then(as_f64)
            .map(|v| v as f32);

        let speed_mps = field(fields, "enhanced_speed")
            .or_else(|| field(fields, "speed"))
            .and_then(as_f64)
            .map(|v| v as f32);

        let heart_rate_bpm = field(fields, "heart_rate").and_then(as_u8);
        let cadence_rpm = field(fields, "cadence").and_then(as_u8);
        let power_w = field(fields, "power").and_then(as_u16);
        let distance_m = field(fields, "distance").and_then(as_f64);

        samples.push(Sample {
            t,
            lat,
            lon,
            altitude_m,
            speed_mps,
            heart_rate_bpm,
            cadence_rpm,
            power_w,
            distance_m,
            elev_gain_cum_m: None,
            gradient_pct: None,
        });
    }

    let start_time = start_time.ok_or(FitError::Empty)?;
    Ok(Activity::from_samples(start_time, samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_fixture_loads() {
        // Relies on a developer-local fixture that isn't in git. Skip
        // silently when absent so CI and fresh clones stay green.
        let path = std::path::Path::new("../../examples/ride.fit");
        if !path.exists() {
            return;
        }
        let a = load_fit(path).unwrap();
        assert!(a.samples.len() >= 2);
        assert!(
            a.samples.iter().any(|s| s.power_w.is_some())
                || a.samples.iter().any(|s| s.heart_rate_bpm.is_some())
        );
    }
}
