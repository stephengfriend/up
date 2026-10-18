//! PUB-01: the shipped `up` binary skeleton builds and answers `--version`
//! and `--help` (M0). It carries no dependencies and no dev-check code (D1-01).

use std::process::Command;

fn up(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_up"))
        .args(args)
        .output()
        .expect("run the up binary")
}

#[test]
fn test_pub_01_up_binary_skeleton_runs() {
    let version = up(&["--version"]);
    assert!(version.status.success(), "--version must exit 0");
    assert_eq!(String::from_utf8_lossy(&version.stdout), "up 0.0.0\n");

    let help = up(&["--help"]);
    assert!(help.status.success(), "--help must exit 0");
    assert!(!help.stdout.is_empty(), "--help must print usage");

    let unknown = up(&["no-such-command"]);
    assert_eq!(unknown.status.code(), Some(2), "unknown input exits 2");
    assert!(!unknown.stderr.is_empty(), "usage goes to stderr");
}
