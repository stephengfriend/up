---
title: Entrypoint and bootstrap
status: draft
prefix: BOOT
milestones: [M1]
depends_on: [architecture, trust, supply-chain]
---

# Entrypoint and bootstrap

`up.sh` puts a verified `up` binary on the machine and runs it. It needs no credentials, because engine releases are public, and it trusts nothing it hasn't verified against the release signer set it carries. When `up` finds no `/etc/up/host.yml`, it bootstraps: it discovers your GitHub login, finds your instance, anchors trust, installs itself under `/opt/up` and converges.

## One-liners

```sh
# New machine: find my instance from my GitHub credentials and converge
curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash

# Naming the instance, or skipping discovery
curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash -s -- --name flotilla
curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash -s -- --repo https://github.com/<login>/fleet.git

# No instance yet: create one with the wizard
curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash -s -- init
```

**Verified install.** For high-assurance hosts, skip `main`: download `up.sh` and its signature from a release, run `ssh-keygen -Y verify` against the published signer keys, then run it. Rendered boot configs always pin `up.sh` by SHA-256 from a release URL (TRUST-09). Baked images skip `up.sh` entirely.

## up.sh

1. **Prerequisites.** Check for `git`, `curl`, `ssh-keygen` and a SHA-256 tool, and enforce the minimum git and OpenSSH versions (SUPPLY-08). If git is missing it says so and installs it: Command Line Tools on macOS, `sudo apt-get install git` or `sudo dnf install git` on Linux. This is the only `sudo` in `up.sh`.
2. **Pick a release:** `UP_VERSION` if set, otherwise GitHub's `releases/latest`, which never points at a pre-release or draft.
3. **Verify:** download `SHA256SUMS` and `SHA256SUMS.sig`; verify the signature with `ssh-keygen -Y verify` against `UP_SIGNERS`, the engine signer set embedded in the script (primary and backup key); download the binary archive for this platform and check its hash against the signed sums.
4. `exec` the verified binary as `up bootstrap`, passing every argument through. The binary installs itself under `/opt/up` and removes the temp directory.

`UP_REPO` (engine `owner/repo`) and `UP_SIGNERS` are the two constants a fork changes together.

```bash
#!/usr/bin/env bash
# up.sh: fetch, verify and run the signed up binary. Safe to re-run.
set -euo pipefail

UP_REPO="${UP_REPO:-<you>/up}"                   # a fork changes this ...
UP_SIGNERS='up-release namespaces="up-release" sk-ssh-ed25519@openssh.com AAAA...primary
up-release namespaces="up-release" sk-ssh-ed25519@openssh.com AAAA...backup'   # ... and this

die() { printf 'up.sh: %s\n' "$*" >&2; exit 1; }

main() {
  ensure_prereqs                                 # git, curl, ssh-keygen, sha256; version floors
  local target base tmp
  target="$(detect_target)"                      # e.g. aarch64-apple-darwin
  if [ -n "${UP_VERSION:-}" ]; then base="https://github.com/$UP_REPO/releases/download/v$UP_VERSION"
  else base="https://github.com/$UP_REPO/releases/latest/download"; fi
  tmp="$(mktemp -d)"
  curl -fsSL "$base/SHA256SUMS"     -o "$tmp/SHA256SUMS"
  curl -fsSL "$base/SHA256SUMS.sig" -o "$tmp/SHA256SUMS.sig"
  printf '%s\n' "$UP_SIGNERS" > "$tmp/signers"
  ssh-keygen -Y verify -f "$tmp/signers" -I up-release -n up-release \
    -s "$tmp/SHA256SUMS.sig" < "$tmp/SHA256SUMS" >/dev/null || die "release signature invalid"
  curl -fsSL "$base/up-$target.tar.gz" -o "$tmp/up.tar.gz"
  check_sha256 "$tmp/SHA256SUMS" "up-$target.tar.gz" "$tmp/up.tar.gz" || die "checksum mismatch"
  tar -xzf "$tmp/up.tar.gz" -C "$tmp"
  exec "$tmp/up" bootstrap --staging-dir "$tmp" "$@"
}
main "$@"
```

## Bootstrap

`up` bootstraps when `/etc/up/host.yml` does not exist. `help`, `init`, `guard` and `doctor` never bootstrap.

### 1. Discover the GitHub login

First method that works:

1. `--owner <login>`.
2. `gh api user`, if `gh` is installed and signed in.
3. A token from `GH_TOKEN`, `GITHUB_TOKEN`, `--token-file`, or `/etc/up/bootstrap-token`, checked against the GitHub API.
4. The greeting from `ssh -T git@github.com`, using the engine's pinned GitHub host keys with `StrictHostKeyChecking=yes`. With several keys in the agent, `up` asks which login is meant.
5. Interactive only: the `gh` device flow, with a verified, temporary `gh` binary.

Unattended with none of these: exit `1` with nothing changed.

### 2. Find the instance

1. `--repo <url>`: `https://` or `ssh://` (or scp-style) only. Unattended runs MUST pass this; render always does.
2. `--name <name>`: `<login>/<name>`.
3. With an API credential: repos **owned by** the login (not collaborator or organization repos) that carry the discovery topic. The topic is a hint, never authorization. One match: confirm interactively. Several: ask.
4. Without an API credential: try `<login>/<default name>` and confirm interactively.
5. Nothing found: offer `up init`.

Discovery picks a repo; it cannot make the host trust one. A wrongly chosen instance still fails signature verification against the anchored signers.

### 3. Anchor trust

Before anything from the instance is applied, root writes:

- `/etc/up/allowed_signers.engine`, from the binary.
- `/etc/up/known_hosts`, GitHub's published SSH host keys, from the binary.
- `/etc/up/allowed_signers.instance`: from the rendered boot config if present; interactively, after showing the fingerprints in the instance's `.up/allowed_signers` and asking the operator to confirm them. Unattended with no provided anchor: exit `1`.
- `/etc/up/recipients.allow`, from the same source and confirmed the same way.

### 4. Fetch, verify, install

1. Ask for `sudo` once. Everything below runs in that one root process.
2. Install the verified binary to `/opt/up/releases/<version>/up` and link `/opt/up/bin/up`.
3. Fetch the instance into `/var/lib/up/git/instance.git`. Interactive runs fetch as the user (agent or `gh`) into a temporary bundle that root then imports; unattended runs fetch as root with the bootstrap token. The token is sent as a URL-scoped header through `GIT_CONFIG_*` environment variables: never argv, disk or git config.
4. Verify the tip is signed by the anchored instance signers (TRUST-07) and export it to `/opt/up/instance/<sha>`.
5. Read `up.yml`. If its `engine.version` differs from the running binary, fetch and verify that release (TRUST-05) and `exec` it. Install the engine tree at `engine.commit` after verifying its tag (TRUST-06).
6. Build the root toolchain from pinned artifacts (SUPPLY-07), including gitleaks.
7. Resolve host identity and write `/etc/up/host.yml`.
8. Interactive bootstrap enrolls the host itself (SEC-10): generate its age identity and deploy key, register them through the operator's credentials, and commit the recipient change, signed, in the operator's working clone.
9. Converge.
10. On exit, whatever the outcome: delete the bootstrap token, scrub provisioning seeds (`/boot/firmware/user-data` and other seed locations, cloud-init caches and logs), unset `GH_TOKEN` and `GITHUB_TOKEN`, and remove the staging directory.

Discovery credentials are dropped as soon as discovery finishes and never reach the converge.

With `--dev`, bootstrap also creates working clones of the engine and instance under `~/src` and installs guard hooks in them. Root never reads them.

## Host identity

1. Flags, at bootstrap only: `--host`, `--user`.
2. `/etc/up/host.yml`: the inventory alias in `host`.
3. Inventory: the system hostname matched against inventory aliases (only when `host` is unset).
4. Interactive picker: choose the alias from inventory.
5. Headless and still unknown: exit `1`.

## Requirements

- **BOOT-01** `up.sh` MUST only check prerequisites, fetch and verify the release binary, and `exec` it; it MUST NOT handle credentials.
  - AC: (negative) CI fails if `up.sh` references `uv`, `ansible`, `brew`, `gh`, `op`, `bw`, a token variable, or an `Authorization` header.
- **BOOT-02** `up.sh` MUST run under bash 3.2 and pass shellcheck; its behavior, not its length, is the constraint.
  - AC: The script's test suite passes in a bash 3.2 container and on macOS.
- **BOOT-03** `up.sh` MUST wrap all logic in `main "$@"` called on the last line.
  - AC: (negative) Piping a truncated copy into bash executes nothing.
- **BOOT-04** `up.sh` MUST select `UP_VERSION` or the latest non-prerelease release and MUST verify the signed sums and the binary hash before executing anything.
  - AC: (negative) A tampered binary, a tampered `SHA256SUMS`, and sums signed by a non-listed key each exit non-zero before `exec`.
- **BOOT-05** `up` MUST bootstrap when `/etc/up/host.yml` is absent and MUST NOT when it exists; `help`, `init`, `guard` and `doctor` MUST never bootstrap.
  - AC: With no `host.yml`, `up` and `up converge` bootstrap; with one, neither does.
- **BOOT-06** Login discovery MUST follow the order above and stop at the first method that works; SSH discovery MUST use the pinned GitHub host keys.
  - AC: One test per method with a mock API and mock SSH greeting; (negative) an SSH greeting from a host key not in the pinned set is refused.
- **BOOT-07** Instance discovery MUST accept only repos owned by the login, MUST treat the topic as a hint, MUST NOT auto-select in unattended mode, and MUST accept only `https` and `ssh` remotes.
  - AC: (negative) A topic-tagged repo owned by another account is ignored; an unattended run without `--repo` exits `1`; a `file://` or `http://` remote is refused.
- **BOOT-08** A token MUST be passed only as a URL-scoped header via environment, MUST NOT appear in argv, on disk or in git config, and MUST be removed from the environment before converge.
  - AC: (negative) After a token clone, `git config --list --show-origin`, a captured `ps` listing and the environment of an Ansible task contain no token.
- **BOOT-09** All SSH connections to GitHub made by `up` MUST use `/etc/up/known_hosts` with `StrictHostKeyChecking=yes`.
  - AC: (negative) A MITM host key makes the connection fail without prompting.
- **BOOT-10** Bootstrap MUST write the engine and instance signer anchors, the recipient allowlist and `known_hosts` before applying any instance content; unattended bootstrap without a provided instance anchor MUST exit `1`.
  - AC: (negative) Unattended bootstrap with no `/etc/up/allowed_signers.instance` exits `1` with nothing converged.
- **BOOT-11** A completed bootstrap MUST leave: the binary and trees under `/opt/up` per TRUST-01, the toolchain including gitleaks, `/etc/up/host.yml`, and anchors in place.
  - AC: A post-bootstrap test checks each item, and that `gitleaks version` runs from the toolchain.
- **BOOT-12** Bootstrap MUST remove the bootstrap token, provisioning seeds on the boot partition, cloud-init caches and logs, and its staging directory on exit, whether it succeeds or fails.
  - AC: (negative) After both a successful and a deliberately failed first boot, no token or private key remains in `/boot*/user-data`, `/var/lib/cloud/**`, `/var/log/cloud-init*.log`, the journal or the staging directory.
- **BOOT-13** Re-running the one-liner on a bootstrapped host MUST be safe: it verifies, refuses downgrades (TRUST-08) and converges.
  - AC: A second run exits `0` with zero changes; (negative) `UP_VERSION` set to an older release is refused.
- **BOOT-14** Host identity MUST resolve by the precedence above, matching inventory by alias; an unattended run with unknown identity MUST exit `1` without changing the system.
  - AC: One test per precedence level; (negative) the headless-unknown test asserts exit `1` and no Ansible run.
