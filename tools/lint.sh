#!/usr/bin/env bash
# Engine lint stage. CI jobs call these same entry points (D1-02); `up test lint` wraps this in M2 (cli.md).
#
# Usage: tools/lint.sh <check>
#   checks: rust cargo-deny repo-files schema-check spec-check workflow-lint
#           renovate-check shellcheck yamllint ansible-lint gitleaks all
#
# Exit codes: 0 pass, 1 a check failed, 2 usage error.
# Bash 3.2 compatible (macOS system bash): no mapfile, no associative arrays.
set -euo pipefail

CHECKS="rust cargo-deny repo-files schema-check spec-check workflow-lint renovate-check shellcheck yamllint ansible-lint gitleaks"

usage() {
  echo "usage: tools/lint.sh <check>" >&2
  echo "  checks: ${CHECKS} all" >&2
}

# rust-toolchain.toml pins the toolchain; install it when rustup has not yet.
ensure_toolchain() {
  if command -v rustup >/dev/null 2>&1; then
    rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install
  fi
}

run_rust() {
  ensure_toolchain &&
    cargo test --locked --workspace &&
    cargo clippy --locked --workspace --all-targets -- -D warnings
}

run_updev() {
  ensure_toolchain &&
    cargo run --locked --quiet -p up-dev -- "$1"
}

run_check() {
  case "$1" in
    rust) run_rust ;;
    cargo-deny | repo-files | schema-check | spec-check | workflow-lint | renovate-check | shellcheck | yamllint | ansible-lint | gitleaks)
      run_updev "$1"
      ;;
    *)
      echo "lint: unknown check: $1" >&2
      return 2
      ;;
  esac
}

run_all() {
  local failed="" name
  for name in ${CHECKS}; do
    echo "lint: running ${name}" >&2
    if run_check "${name}"; then
      :
    else
      failed="${failed} ${name}"
    fi
  done
  if [ -n "${failed}" ]; then
    echo "lint: failed:${failed}" >&2
    return 1
  fi
  echo "lint: all checks passed"
}

is_known_check() {
  local name
  for name in ${CHECKS}; do
    if [ "$1" = "${name}" ]; then
      return 0
    fi
  done
  return 1
}

main() {
  if [ "$#" -ne 1 ]; then
    usage
    return 2
  fi
  if [ "$1" != "all" ] && ! is_known_check "$1"; then
    echo "lint: unknown check: $1" >&2
    usage
    return 2
  fi
  cd "$(git rev-parse --show-toplevel)"
  if [ "$1" = "all" ]; then
    run_all
  else
    run_check "$1"
  fi
}

main "$@"
