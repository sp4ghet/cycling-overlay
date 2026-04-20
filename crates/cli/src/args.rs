use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "gpx-overlay", version, about = "Render a transparent video overlay from GPX or FIT data")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render an overlay video for the given time range.
    Render(RenderArgs),
}

#[derive(clap::Args, Debug)]
pub struct RenderArgs {
    /// Input activity file (GPX or FIT).
    #[arg(short, long)]
    pub input: PathBuf,

    /// Layout JSON file.
    #[arg(short, long)]
    pub layout: PathBuf,

    /// Output video path (should end in .mov for ProRes 4444).
    #[arg(short, long)]
    pub output: PathBuf,

    /// Start time offset from activity start: HH:MM:SS, MM:SS, seconds, or ISO timestamp.
    #[arg(long, value_parser = parse_time_spec_cli)]
    pub from: Option<Duration>,

    /// End time offset from activity start. Defaults to activity end.
    #[arg(long, value_parser = parse_time_spec_cli)]
    pub to: Option<Duration>,

    /// Frames per second. Overrides the layout's canvas.fps.
    #[arg(long)]
    pub fps: Option<u32>,

    /// Canvas size, e.g. 1920x1080. Overrides the layout's canvas.width/height.
    #[arg(long, value_parser = parse_size)]
    pub size: Option<(u32, u32)>,

    /// Number of render threads. Defaults to num_cpus::get().
    #[arg(long)]
    pub threads: Option<usize>,

    /// ProRes qscale (0 = lossless, 13 = aggressive). Default 11.
    #[arg(long, default_value_t = 11)]
    pub qscale: u32,

    /// Parse + validate only; don't render.
    #[arg(long)]
    pub dry_run: bool,
}

/// Parse time specs like:
///   - "01:23:45" -> 1h23m45s
///   - "02:30" -> 2m30s
///   - "90" -> 90s
///
/// ISO timestamps like "2024-06-01T06:00:10Z" are NOT handled here -- they
/// need the activity start_time to be resolved, which is a higher-level concern.
/// For v1, only the three HMS forms are supported via the CLI; a future task
/// can add ISO via a different arg.
pub fn parse_time_spec(s: &str) -> Result<Duration, String> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            let secs: u64 = parts[0].parse().map_err(|e| format!("invalid seconds: {}", e))?;
            Ok(Duration::from_secs(secs))
        }
        2 => {
            let m: u64 = parts[0].parse().map_err(|e| format!("invalid minutes: {}", e))?;
            let s: u64 = parts[1].parse().map_err(|e| format!("invalid seconds: {}", e))?;
            if s >= 60 { return Err(format!("seconds out of range: {}", s)); }
            Ok(Duration::from_secs(m * 60 + s))
        }
        3 => {
            let h: u64 = parts[0].parse().map_err(|e| format!("invalid hours: {}", e))?;
            let m: u64 = parts[1].parse().map_err(|e| format!("invalid minutes: {}", e))?;
            let sec: u64 = parts[2].parse().map_err(|e| format!("invalid seconds: {}", e))?;
            if m >= 60 { return Err(format!("minutes out of range: {}", m)); }
            if sec >= 60 { return Err(format!("seconds out of range: {}", sec)); }
            Ok(Duration::from_secs(h * 3600 + m * 60 + sec))
        }
        _ => Err(format!("expected HH:MM:SS, MM:SS, or seconds; got '{}'", s)),
    }
}

fn parse_time_spec_cli(s: &str) -> Result<Duration, String> {
    parse_time_spec(s)
}

pub fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("expected WxH, got '{}'", s));
    }
    let w: u32 = parts[0].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let h: u32 = parts[1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    if w == 0 || h == 0 {
        return Err("width and height must be positive".into());
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_spec_hms() {
        assert_eq!(parse_time_spec("01:23:45").unwrap(), Duration::from_secs(5025));
    }

    #[test]
    fn time_spec_ms() {
        assert_eq!(parse_time_spec("02:30").unwrap(), Duration::from_secs(150));
    }

    #[test]
    fn time_spec_secs() {
        assert_eq!(parse_time_spec("90").unwrap(), Duration::from_secs(90));
    }

    #[test]
    fn time_spec_invalid_format() {
        assert!(parse_time_spec("abc").is_err());
        assert!(parse_time_spec("1:2:3:4").is_err());
        assert!(parse_time_spec("1:90").is_err()); // seconds out of range
    }

    #[test]
    fn size_parses_1920x1080() {
        assert_eq!(parse_size("1920x1080").unwrap(), (1920, 1080));
    }

    #[test]
    fn size_rejects_bad_input() {
        assert!(parse_size("1920").is_err());
        assert!(parse_size("1920x0").is_err());
        assert!(parse_size("x1080").is_err());
    }
}
