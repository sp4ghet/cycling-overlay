use crate::{Activity, Sample};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum GpxError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("GPX parse error: {0}")]
    Parse(#[from] gpx::errors::GpxError),
    #[error("GPX extension parse error: {0}")]
    ExtensionXml(#[from] quick_xml::Error),
    #[error("GPX time parse error: {0}")]
    TimeParse(#[from] chrono::ParseError),
    #[error("GPX file contains no track points")]
    Empty,
    #[error("GPX track point missing time information - required by this tool")]
    MissingTime,
    #[error("GPX extension pass parsed {exts} trkpt extensions but coordinate pass produced {samples} samples — file may be malformed or mix trkpt with route/waypoint elements")]
    ExtensionMismatch { exts: usize, samples: usize },
}

pub fn load_gpx(path: &Path) -> Result<Activity, GpxError> {
    // First pass: use the gpx crate for coordinates, elevation, and time.
    let reader = BufReader::new(File::open(path)?);
    let g = gpx::read(reader)?;

    let mut start_time = None;
    let mut samples: Vec<Sample> = Vec::new();

    for track in &g.tracks {
        for segment in &track.segments {
            for pt in &segment.points {
                let time = pt.time.ok_or(GpxError::MissingTime)?;
                let time = gpx_time_to_chrono(time)?;
                let start = *start_time.get_or_insert(time);
                let t_sd = time.signed_duration_since(start);
                let t = Duration::from_millis(t_sd.num_milliseconds().max(0) as u64);

                samples.push(Sample {
                    t,
                    lat: pt.point().y(), // geo_types: y = latitude
                    lon: pt.point().x(), // geo_types: x = longitude
                    altitude_m: pt.elevation.map(|v| v as f32),
                    speed_mps: None,
                    heart_rate_bpm: None,
                    cadence_rpm: None,
                    power_w: None,
                    distance_m: None,
                    elev_gain_cum_m: None,
                });
            }
        }
    }

    let start_time = start_time.ok_or(GpxError::Empty)?;

    // Second pass: extract per-trkpt extensions (hr, power) by streaming the
    // file with quick-xml. The gpx 0.10 crate does not expose extensions on
    // Waypoint (see the TODO in gpx/src/parser/extensions.rs), so we do it
    // ourselves. Track points are matched positionally by document order,
    // which is exactly the order the gpx crate returns them in as well.
    let extensions = read_trkpt_extensions(path)?;
    if extensions.len() != samples.len() {
        return Err(GpxError::ExtensionMismatch {
            exts: extensions.len(),
            samples: samples.len(),
        });
    }
    for (s, ext) in samples.iter_mut().zip(extensions.iter()) {
        s.heart_rate_bpm = ext.hr;
        s.power_w = ext.power;
        if s.cadence_rpm.is_none() {
            s.cadence_rpm = ext.cadence;
        }
    }

    Ok(Activity::from_samples(start_time, samples))
}

/// Convert the gpx crate's `Time` into a `chrono::DateTime<Utc>`.
///
/// `gpx::Time` wraps `time::OffsetDateTime`, but the `time` crate is not a
/// direct dependency of this crate. Rather than pull it in, we use
/// `Time::format()` to get an ISO 8601 string and parse it with chrono.
fn gpx_time_to_chrono(t: gpx::Time) -> Result<chrono::DateTime<chrono::Utc>, GpxError> {
    let s = t.format().map_err(GpxError::from)?;
    let dt = chrono::DateTime::parse_from_rfc3339(&s)?;
    Ok(dt.with_timezone(&chrono::Utc))
}

#[derive(Default, Debug)]
struct TrkptExt {
    hr: Option<u8>,
    power: Option<u16>,
    cadence: Option<u8>,
}

/// Streams the GPX file and returns one `TrkptExt` per `<trkpt>` element, in
/// document order. Values that are missing or out of range are left as `None`.
fn read_trkpt_extensions(path: &Path) -> Result<Vec<TrkptExt>, GpxError> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut results: Vec<TrkptExt> = Vec::new();

    // State stack of tag local-names so we know which element we are inside.
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<TrkptExt> = None;
    let mut text_buf: Vec<u8> = Vec::new();

    fn local_name(qname: &[u8]) -> &[u8] {
        match qname.iter().position(|&b| b == b':') {
            Some(i) => &qname[i + 1..],
            None => qname,
        }
    }

    fn inside_trkpt_extensions(stack: &[Vec<u8>]) -> bool {
        // The trkpt extension block is <trkpt>...<extensions>...
        let has_trkpt = stack.iter().any(|t| t == b"trkpt");
        let has_ext = stack.iter().any(|t| t == b"extensions");
        has_trkpt && has_ext
    }

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let name = local_name(e.name().as_ref()).to_vec();
                if name == b"trkpt" {
                    current = Some(TrkptExt::default());
                }
                stack.push(name);
                text_buf.clear();
            }
            Event::Empty(e) => {
                // Self-closing element inside <extensions> has no text.
                let _ = local_name(e.name().as_ref()); // ignore for now
            }
            Event::Text(t) => {
                text_buf.extend_from_slice(t.as_ref());
            }
            Event::End(e) => {
                let name = local_name(e.name().as_ref()).to_vec();
                let leaf_tag_name = stack.last().cloned();

                // Process leaf values that live inside a trkpt's extensions block.
                if inside_trkpt_extensions(&stack) {
                    if let Some(ext) = current.as_mut() {
                        let text = std::str::from_utf8(&text_buf).unwrap_or("").trim();
                        match name.as_slice() {
                            b"hr" => {
                                if let Ok(v) = text.parse::<u16>() {
                                    ext.hr = u8::try_from(v).ok();
                                }
                            }
                            b"power" => {
                                if let Ok(v) = text.parse::<u32>() {
                                    ext.power = u16::try_from(v).ok();
                                }
                            }
                            b"cad" => {
                                if let Ok(v) = text.parse::<u16>() {
                                    ext.cadence = u8::try_from(v).ok();
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // Pop the stack (expecting matched tags).
                if leaf_tag_name.as_deref() == Some(name.as_slice()) {
                    stack.pop();
                } else {
                    // Malformed input: still try to pop something to keep
                    // moving. quick-xml would normally have errored already.
                    stack.pop();
                }
                text_buf.clear();

                if name == b"trkpt" {
                    if let Some(ext) = current.take() {
                        results.push(ext);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpx_fixture_loads() {
        let a = load_gpx(std::path::Path::new("../../examples/short.gpx")).unwrap();
        assert_eq!(a.samples.len(), 20);
        assert!(a.samples.iter().any(|s| s.altitude_m.is_some()));
        assert_eq!(a.samples[0].t, std::time::Duration::ZERO);
        // At least a few samples should have HR and power from the extensions
        assert!(a.samples.iter().filter(|s| s.heart_rate_bpm.is_some()).count() >= 6);
        assert!(a.samples.iter().filter(|s| s.power_w.is_some()).count() >= 6);
    }
}
