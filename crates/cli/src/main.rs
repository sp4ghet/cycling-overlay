mod args;

use args::{Args, Command};
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Render(_r) => {
            // Task 21 will fill this in (dry-run) and Task 24 the full pipeline.
            eprintln!("gpx-overlay render: not yet implemented (Tasks 21+)");
            std::process::exit(1);
        }
    }
}
