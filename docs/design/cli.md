---
title: The up CLI
status: draft
prefix: CLI
milestones: [M1, M2, M4]
depends_on: [architecture, trust, bootstrap]
---

# The `up` CLI

`up` is a single compiled Rust binary, released with signed checksums for macOS and Linux on `aarch64` and `x86_64`. It parses and validates config natively, verifies signatures natively, talks to the GitHub API, and drives Ansible and chezmoi. It installs itself root-owned at `/opt/up/bin/up` and is on every user's `PATH`. With no arguments it does the obvious thing: bootstrap if this machine has no `/etc/up/host.yml`, otherwise converge.

```sh
$ up                 # new machine: bootstrap; known machine: converge (one sudo prompt)
```

## Design

- **Built-ins in the binary.** Every engine command is compiled in. An instance may add commands as executables in its `libexec/` (`up-<name>`), but only under names the binary doesn't already use.
- **Root orchestrates.** A converge makes one `sudo /opt/up/bin/up converge --as-root` call (no keepalive loop). Root runs the system layer hermetically, then drops to the user for chezmoi (TRUST-02, TRUST-03).
- **Instance edits go through git.** Commands that change the instance (`host set`, `self update`, `secrets enroll`, `init`) edit the user's working clone and make a signed commit. Root never commits, and nothing changes a host except a signed commit it pulls.
- **Wraps, doesn't hide.** `up ansible -- <args>` and `up chezmoi -- <args>` run the pinned tools with the right inventory, roles path, source dir and environment. Their exit codes pass through unchanged.
- **Exit codes:** `0` success; `1` failure; `2` changes pending (with `--check`); `3` converged with declared skips. When several apply, the highest-priority one wins: `1`, then `2`, then `3`, then `0`.
- **Declared skips only.** Exit `3` happens only for reasons decided before a task runs: offline mode (CHG-09), no secrets session in an unattended run (SEC-12), or a host awaiting enrollment (SEC-10). Each skip is recorded with its reason. A task that fails is a failure.
- **Fast paths.** `up help`, `up host`, `up guard` and `up doctor` work offline and without sudo.

## Commands

| Command | What it does | Milestone |
| --- | --- | --- |
| `up` | Bootstrap if no `host.yml`; otherwise `up converge` | M1 |
| `up converge` | Fetch and verify, then system (root), then user. Flags: `--check`, `--only system\|user`, `--tags`, `--offline`, `--unattended`, `--allow-downgrade` (interactive only) | M1 |
| `up update` | Fetch and verify the instance and the pinned engine; activate new trees; nothing else | M1 |
| `up bootstrap` | Re-run bootstrap on purpose | M1 |
| `up host` | Show identity, applied roles, last run and state | M1 |
| `up doctor` | Check trust anchors, tree ownership, toolchain, schemas, guard hooks, schedule, staleness, shadowed roles, shell startup budget | M1 |
| `up guard install\|check\|scan\|allow` | Guard hooks and checks in working clones; `allow` is interactive only | M1 |
| `up help [command]` | List commands, including instance ones | M1 |
| `up system` / `up ansible -- …` | System layer only / `ansible-playbook` passthrough | M2 |
| `up dotfiles [diff\|edit\|add\|cd]` / `up chezmoi -- …` | chezmoi actions in the working clone / passthrough | M2 |
| `up schedule on\|off\|status\|logs` | Root-owned launchd daemon or systemd timer | M2 |
| `up host set roles <…>` | Edit the host's inventory groups in the working clone and commit | M2 |
| `up test lint\|molecule\|vm` | Run CI checks locally | M2 |
| `up secrets get\|edit\|enroll\|rotate` | Resolve, `sops` edit, enroll a host's keys, rotate | M3 |
| `up host retire <host>` | Decommission: revoke, rotate, remove (SEC-15) | M3 |
| `up init [--answers <file>]` | Create an instance from `template/` | M4 |
| `up self status\|update\|dev` | Engine version; bump `engine.version` and `engine.commit` in a signed commit; dev engine | M4 |
| `up release` | Engine maintainers: verify CI artifacts and sign a release locally | M4 |
| `up image render\|build\|flash\|list\|prune` | Boot configs, bake, sign, verify-and-flash, manage images | M5 |
| `up container build\|push\|init\|shell` | Dev container images | M6 |
| `up push <host\|group>` | `control_node` only: same playbook over SSH | M7 |
| `up image netboot sync` | Publish signed iPXE scripts and configs to a netboot server | M7 |
| `up versions check\|bump` | Manage pins and hashes in `versions.yml` | M8 |

## The control_node role

Optional, on any host. It installs push tooling (a hardware-backed push key, SSH config for the `up-push` account on targets), the image toolchain (Packer, QEMU user emulation, pi-gen dependencies, BlueBuild CLI), a multi-arch buildx builder, and VM test tooling. A dedicated control node (a small VM or a Pi) is recommended once you manage more than a handful of servers.

## Requirements

- **CLI-01** Engine commands MUST be built into the binary; an instance `libexec/up-<name>` MUST be dispatched only when `<name>` is not a built-in.
  - AC: (negative) An instance `libexec/up-guard` is ignored and reported by `up doctor`; an instance `libexec/up-hello` runs.
- **CLI-02** `up` with no arguments MUST bootstrap when `/etc/up/host.yml` is absent (BOOT-05) and otherwise behave exactly like `up converge`.
  - AC: With `host.yml` present, `up` and `up converge` produce identical invocations in a dry-run test.
- **CLI-03** Built-in commands MUST use exit codes `0`, `1`, `2` and `3` as defined, with precedence `1 > 2 > 3 > 0`; exit `3` MUST only come from declared skips; passthrough commands are exempt.
  - AC: Drift plus offline under `--check` exits `2`; (negative) a task failing online with a network error exits `1`, not `3`.
- **CLI-04** Every command MUST support `--help`, and `up help` MUST list built-in and instance commands.
  - AC: A test runs `--help` for every command and checks exit `0`.
- **CLI-05** `up help`, `up host`, `up guard` and `up doctor` MUST work offline and without sudo.
  - AC: (negative) They pass in a container with networking disabled and no sudo.
- **CLI-06** `up ansible --` and `up chezmoi --` MUST pass arguments and exit codes through unchanged, with the instance's inventory, roles path and source dir set.
  - AC: `up ansible -- --list-hosts all` lists the test inventory; a failing playbook's exit code is returned as is.
- **CLI-07** Commands that need a role's tooling MUST exit `1` naming the role when it is missing.
  - AC: (negative) `up push` on a host without `control_node` prints the role name and exits `1`.
- **CLI-08** A converge MUST make at most one sudo call, MUST run the system layer before the user layer, and MUST stop before the user layer if the system layer fails.
  - AC: A failing system fixture leaves chezmoi un-run and exits `1`; (negative) no `sudo -v` loop runs during a converge.
- **CLI-09** Commands that change the instance MUST do so only by editing the user's working clone and creating a signed commit; they MUST NOT run as root.
  - AC: `up host set roles +ai` produces one signed commit changing inventory; (negative) running it via `sudo` exits `1`.
- **CLI-10** `up self update` MUST set `engine.version` and `engine.commit` to a newer verified release (or `--to <version>`), show the CHANGELOG entries in between, and create one signed commit.
  - AC: The commit changes only the `engine` block; (negative) a target release whose tag does not verify is refused.
- **CLI-11** `up self status` MUST show the engine version, whether it matches the pin, and whether a dev engine is active (TRUST-13).
  - AC: Status output differs for pinned, dev-active, and mismatched states.
- **CLI-12** zsh completions MUST be generated from the built-in and instance commands.
  - AC: Completion output contains an instance-only test command.
