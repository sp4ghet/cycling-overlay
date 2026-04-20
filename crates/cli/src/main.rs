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
            let result = if r.dry_run {
                run::dry_run(&r)
            } else {
                run::render(&r)
            };
            match result {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("error: {:?}", e);
                    classify_error(&e)
                }
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
///   3 = render / ffmpeg error
fn classify_error(err: &anyhow::Error) -> i32 {
    // Simple substring heuristics. A proper version uses typed errors at
    // module boundaries, but this is enough for v1.
    let msg = format!("{:#}", err);
    if msg.contains("ffmpeg") || msg.contains("pixmap") || msg.contains("Pixmap") || msg.contains("render_frame") {
        3
    } else if msg.contains("parse") || msg.contains("JSON") || msg.contains("GPX") || msg.contains("FIT") {
        2
    } else {
        1
    }
}
