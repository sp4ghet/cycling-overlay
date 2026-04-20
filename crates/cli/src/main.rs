mod args;
mod ffmpeg;
mod pipeline;
mod run;

use args::{Args, Command};
use clap::Parser;

fn main() {
    let args = Args::parse();
    let exit_code = match args.command {
        Command::Render(r) => {
            if r.dry_run {
                match run::dry_run(&r) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("error: {:?}", e);
                        classify_error(&e)
                    }
                }
            } else {
                eprintln!("gpx-overlay render: full render not yet implemented (Tasks 22+)");
                1
            }
        }
    };
    std::process::exit(exit_code);
}

/// Map an anyhow::Error to one of our exit codes.
///
/// Exit-code convention:
///   0 = success
///   1 = usage / arg / runtime error
///   2 = parse error (GPX, FIT, or layout JSON)
///   3 = render / ffmpeg error (not yet reachable — Tasks 22+)
fn classify_error(err: &anyhow::Error) -> i32 {
    // For v1 we use very simple heuristics. A proper version uses typed
    // errors at module boundaries, but for a dry_run, 2 ("parse") covers
    // parse-layer issues and is a fine default.
    let msg = format!("{:#}", err);
    if msg.contains("parse") || msg.contains("JSON") || msg.contains("GPX") || msg.contains("FIT") {
        2
    } else {
        1
    }
}
