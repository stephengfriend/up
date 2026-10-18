//! SUPPLY-02 / SUPPLY-06 (WR-03): when tool installation fails, the rule findings already
//! collected are still reported, together with the installation failure.

use std::path::Path;
use std::process::Command;

use up_dev::check::{Ctx, Failure};

fn git_repo_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    for (rel, text) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
        git(&["add", rel]);
    }
    dir
}

fn ctx(root: &Path) -> Ctx {
    Ctx {
        root: root.to_path_buf(),
        github_actions: false,
    }
}

const BAD_WORKFLOW: &str = "name: bad
on:
  push:
    branches: [main]
permissions:
  contents: read
jobs:
  build:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4
";

#[test]
fn test_supply_02_findings_survive_tool_setup_failure() {
    let repo = git_repo_with(&[(".github/workflows/bad.yml", BAD_WORKFLOW)]);
    let err = up_dev::workflow_lint::run_with(&ctx(repo.path()), |_| {
        Err(Failure::one("SUPPLY-01: simulated download failure"))
    })
    .expect_err("must fail");
    let all = err.messages.join("\n");
    assert!(
        all.contains("unpinned-action"),
        "unpinned finding lost:\n{all}"
    );
    assert!(
        all.contains("self-hosted-runner"),
        "runner finding lost:\n{all}"
    );
    assert!(
        all.contains("simulated download failure"),
        "setup failure lost:\n{all}"
    );
}

#[test]
fn test_supply_06_findings_survive_venv_setup_failure() {
    let repo = git_repo_with(&[(
        "renovate.json",
        "{\"automerge\": true, \"platformAutomerge\": false}\n",
    )]);
    let err = up_dev::renovate_check::run_with(&ctx(repo.path()), |_| {
        Err(Failure::one("python-tools: simulated venv failure"))
    })
    .expect_err("must fail");
    let all = err.messages.join("\n");
    assert!(all.contains("SUPPLY-06"), "automerge finding lost:\n{all}");
    assert!(
        all.contains("simulated venv failure"),
        "setup failure lost:\n{all}"
    );
}
