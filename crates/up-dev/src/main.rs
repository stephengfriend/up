//! `up-dev <subcommand> [args...]`: dev-time checks for the engine repo.
//!
//! Exit codes: 0 pass, 1 check failure (one stderr line per message),
//! 2 usage error.

use std::process::ExitCode;

use up_dev::check::Ctx;
use up_dev::dispatch::SUBCOMMANDS;

fn usage() -> String {
    let mut s = String::from("usage: up-dev <subcommand> [args...]\n\nsubcommands:\n");
    for (name, _) in SUBCOMMANDS {
        s.push_str("  ");
        s.push_str(name);
        s.push('\n');
    }
    s
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(name) = args.first() else {
        eprint!("{}", usage());
        return ExitCode::from(2);
    };
    if name == "--help" || name == "-h" {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }
    let Some((_, run)) = SUBCOMMANDS.iter().find(|(n, _)| n == name) else {
        eprintln!("up-dev: unknown subcommand `{name}`");
        eprint!("{}", usage());
        return ExitCode::from(2);
    };
    let result = Ctx::from_env().and_then(|ctx| run(&ctx, &args[1..]));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            for message in &failure.messages {
                eprintln!("{message}");
            }
            ExitCode::from(1)
        }
    }
}
