//! The `up` binary, M0 skeleton. Std-only: the shipped binary carries no
//! dependencies and none of the dev-check code (D1-01).

use std::process::ExitCode;

const USAGE: &str = "usage: up [--version | --help]\n\n\
This is the M0 skeleton of the engine binary. It has no commands yet;\n\
see docs/roadmap.md for the milestones that add them.\n";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            println!("up {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
