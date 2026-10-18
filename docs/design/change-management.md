---
title: Change management
status: draft
prefix: CHG
milestones: [M0, M1, M2, M4, M7, M8]
depends_on: [architecture, trust, cli]
---

# Change management

Git is the only way a change reaches a machine, and each machine pulls and verifies it for itself. There are two version streams: the engine, released as signed semver releases, and the instance, whose signed commits on `main` are what hosts apply.

## Versioning

- **Engine:** semver, with signed tags, signed `SHA256SUMS`, and a CHANGELOG with migration notes for breaking changes. `main` and tags are protected by rulesets. `up.sh` and its environment variables are a stable interface.
- **Instance:** trunk-based on protected `main`. Required CI passes before merge, every commit is signed by the instance signer set, and hosts apply `main` fast-forward only (TRUST-07, TRUST-08). There are no instance release tags and no canary ring. That is an accepted risk in [threat-model.md](threat-model.md): a commit you sign reaches every host within one schedule interval.
- **Engine updates reach hosts only through the instance.** `up self update` writes `engine.version` and `engine.commit` in one signed commit; hosts verify and install that release on their next run.
- **Pins:** the engine's `versions.yml` holds versions and hashes; an instance `versions.yml` may override them, hashes included. Renovate opens PRs; it never merges.
- **Homebrew:** rolling versions for user CLI tools from allowed taps (SUPPLY-05). Anything security-relevant is pinned in the root toolchain.

## Signed merges

GitHub's web-flow signature is not in any signer set, so a merge made with GitHub's merge button is not applied by hosts. Merge locally instead: rebase or squash the PR branch, sign the result with your key, and fast-forward `main`. `up` documents a `git` alias for this. PRs from Renovate, CI or a coding agent are reviewed, then re-signed by a human in that local merge.

## Developing the engine

Engine work happens in an ordinary working clone (for example `~/src/up`) with guard hooks installed. To test a change as root on this host, `sudo up self dev --activate ~/src/up` copies the tree into `/opt/up/engine/dev-<sha>` after a TTY confirmation; `up doctor` flags it until `up self dev --off` (TRUST-13). Changes land through PRs on the public repo and reach hosts only in a signed release.

## How changes reach hosts

- **Pull, on demand (default):** `up` fetches and verifies the instance and the pinned engine, then converges.
- **Pull, scheduled:** `up schedule on` installs a root-owned launchd daemon (`/Library/LaunchDaemons`, `root:wheel 0644`) or systemd timer whose program is `/opt/up/bin/up --unattended`. It takes the host lock (OPS-01) and adds random delay.
- **Push (optional, M7):** only from a `control_node` host, over SSH to a dedicated `up-push` account on each target. That account has a scoped sudoers rule that allows only `/opt/up/bin/up converge --as-root --pushed`. Push keys are hardware-backed (`ed25519-sk`, verify-required) and restricted in `authorized_keys` with `from=` and `restrict`. A pushed run still verifies the instance commit it applies.
- **Drift report:** `up converge --check --unattended` exits `2` on drift.
- **Notifications and staleness:** see [operations.md](operations.md).

## Offline behavior

`up` decides it is offline before running any task. Either the instance remote is unreachable at the start-of-run probe, or `--offline` was passed. Then:

1. Neither tree is updated; `up` converges the trees it already has and warns once.
2. Each tool's own offline switch is set for the run, for example `HOMEBREW_NO_AUTO_UPDATE=1`, `MISE_OFFLINE=1`, no apt or dnf metadata refresh, and chezmoi with `--refresh-externals=never`.
3. Tasks tagged `network` are skipped and recorded. Every other task runs normally, and **any failure is a failure** (exit `1`).
4. With skips and no failures, `up` exits `3`.

When the probe succeeds, `up` is online for the whole run: a task that fails for a network reason is exit `1`. A blocked GitHub can hold a host on old config, but not silently: staleness alerts after `stale_after` days (OPS-05).

## Testing

| Stage | What runs | Where |
| --- | --- | --- |
| Engine, every push | cargo test and clippy, shellcheck, ansible-lint (custom rules), yamllint, guard, schema checks, spec checks, workflow lint (SUPPLY-02) | Engine CI (GitHub-hosted) |
| Engine, every PR | Molecule per role on Debian, Ubuntu, Fedora (arm64 + amd64), online and offline; template converge as `fleet` and `flotilla`; negative trust tests (moved tag, unsigned commit, user-writable file, downgrade) | Engine CI |
| Engine, every PR | Full one-liner on a macOS arm64 runner with a mock GitHub API, a test signer set and the `fake` backend | Engine CI |
| Engine, before release | First-boot runs in VMs: Tart (macOS), Lima/QEMU (Linux, Pi images), baked-image boot with networking disabled | `up test vm` on a maintainer machine; never a self-hosted runner |
| Instance, every push | Guard, schemas, recipient diff (TRUST-10), signature check of the pushed commits, chezmoi render for every host, container converge | Instance CI |

## Rollback

- **Instance:** `git revert`, signed, then `up`. This is a forward commit, so anti-rollback is satisfied.
- **Engine:** `up self update --to <previous version>` refuses unless run with `--allow-downgrade` at a TTY; prefer releasing a fixed version.
- **Local, emergency:** `sudo up converge --allow-downgrade` can activate one of the last three kept trees (TRUST-04).
- **Fedora Atomic:** `rpm-ostree rollback`. **Baked images:** reflash a previous signed image.

## Requirements

- **CHG-01** The engine MUST publish signed semver releases with CHANGELOG entries; breaking changes MUST include migration notes.
  - AC: (negative) Release tooling refuses a version without a CHANGELOG section.
- **CHG-02** `up.sh`'s environment variables and the documented one-liners MUST keep working across minor releases; removal needs a one-minor-release deprecation warning and a major version.
  - AC: A compatibility test runs the previous release's documented one-liners against the current `up.sh`.
- **CHG-03** Hosts MUST receive engine changes only through a signed instance commit that changes `engine.version` and `engine.commit`, except when a dev engine is active.
  - AC: (negative) A new engine release with no instance change does not reach a host.
- **CHG-04** Pins and hashes MUST be read from the engine's `versions.yml` merged with the instance's overrides; an override without a hash MUST be refused.
  - AC: An instance override changes the installed ansible-core version; (negative) an override with no hash exits `1`.
- **CHG-05** `up schedule` MUST install a root-owned unit that runs `/opt/up/bin/up --unattended`, holds the host lock, adds jitter, and drops privileges for the user layer.
  - AC: Unit files are root-owned and not writable by others; two overlapping runs result in one execution.
  - AC: (negative) A unit whose program path is outside `/opt/up` makes `up doctor` exit `1`.
- **CHG-06** Push MUST be available only from `control_node` hosts, MUST log in as `up-push` with a hardware-backed, `authorized_keys`-restricted key, MUST be limited by sudoers to the converge command, and MUST still verify the applied commit.
  - AC: (negative) `up-push` cannot run any other command with sudo; a push of an unsigned instance commit is refused on the target.
- **CHG-07** Engine and instance CI MUST implement the stages in the Testing table.
  - AC: Each stage exists as a named workflow job.
- **CHG-08** Every rollback path above MUST be documented in a guide and tested where automatable.
  - AC: A signed revert followed by `up` restores the previous state in a container test.
- **CHG-09** Offline mode MUST be determined before any task runs (failed probe or `--offline`); in offline mode only `network`-tagged tasks are skipped; online, any task failure MUST exit `1`.
  - AC: A bootstrapped container with networking then disabled exits `3` with zero failed tasks; (negative) online, a task that fails with a DNS error exits `1`.
- **CHG-10** Every role MUST pass its Molecule scenario offline, after one online converge, with no failed tasks.
  - AC: The offline Molecule job runs for every role.
- **CHG-11** Commits reaching `main` MUST be signed by the instance signer set; merges MUST be made locally and signed, and the guides MUST document the workflow.
  - AC: Instance CI fails a PR whose commits are not signed by a listed signer; (negative) a web-flow-signed merge commit on `main` is refused by hosts (TRUST-07).
