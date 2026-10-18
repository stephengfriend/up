//! Shared contract for every dev-time check (D1-01, D1-02).
//!
//! A check is a function `run(&Ctx, &[String]) -> CheckResult`. It either
//! passes (`Ok`) or returns a [`Failure`] holding one message per problem.
//! A check that has not landed yet returns [`Failure::pending`], so no check
//! ever passes vacuously.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Everything a check needs to know about where and how it runs.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// Repository root, from `git rev-parse --show-toplevel`.
    pub root: PathBuf,
    /// True only when the environment variable `GITHUB_ACTIONS` equals `true`.
    pub github_actions: bool,
}

impl Ctx {
    /// Build a context from the current working directory and environment.
    pub fn from_env() -> Result<Ctx, Failure> {
        let out = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map_err(|e| {
                Failure::one(format!(
                    "up-dev: cannot run `git rev-parse --show-toplevel`: {e}"
                ))
            })?;
        if !out.status.success() {
            return Err(Failure::one(format!(
                "up-dev: `git rev-parse --show-toplevel` failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if root.is_empty() {
            return Err(Failure::one(
                "up-dev: `git rev-parse --show-toplevel` printed no path",
            ));
        }
        let github_actions = std::env::var("GITHUB_ACTIONS")
            .map(|v| v == "true")
            .unwrap_or(false);
        Ok(Ctx {
            root: PathBuf::from(root),
            github_actions,
        })
    }

    /// Print a non-blocking warning to stderr: a GitHub annotation in CI,
    /// `warning: <file>:<line>: <msg>` elsewhere.
    pub fn warn(&self, file: &str, line: usize, msg: &str) {
        if self.github_actions {
            eprintln!("::warning file={file},line={line}::{msg}");
        } else {
            eprintln!("warning: {file}:{line}: {msg}");
        }
    }
}

/// A failed check: one message per problem, printed one per stderr line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub messages: Vec<String>,
}

impl Failure {
    pub fn one(msg: impl Into<String>) -> Failure {
        Failure {
            messages: vec![msg.into()],
        }
    }

    pub fn many(msgs: Vec<String>) -> Failure {
        Failure { messages: msgs }
    }

    /// The failure of a check whose implementation has not landed yet.
    pub fn pending(check: &str, plan: &str) -> Failure {
        Failure::one(format!(
            "up-dev {check}: not implemented yet; it lands in plan {plan}"
        ))
    }
}

pub type CheckResult = Result<(), Failure>;

/// Every path in the git index (tracked, including staged additions), from
/// `git ls-files -z`. Untracked files on disk are not included.
pub fn tracked_files(root: &Path) -> Result<BTreeSet<String>, Failure> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| Failure::one(format!("up-dev: cannot run `git ls-files -z`: {e}")))?;
    if !out.status.success() {
        return Err(Failure::one(format!(
            "up-dev: `git ls-files -z` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect())
}
