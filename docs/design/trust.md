---
title: Trust and privilege
status: draft
prefix: TRUST
milestones: [M1, M3, M5]
depends_on: [threat-model, architecture]
---

# Trust and privilege

This doc turns the invariant in [threat-model.md](threat-model.md) into mechanisms: where root's code lives, how it gets there, how it is verified, and how root hands off to the user.

## Root-owned, verified, immutable trees

Root never executes anything from a user's home directory. Everything root runs lives under `/opt/up`, is owned by root, and is written only by `up` after verification.

```
/opt/up/
├── bin/up -> ../releases/<version>/up      # verified release binary
├── releases/<version>/up
├── engine/<sha>/                           # verified engine tree (git archive export, no .git)
├── engine/current -> <sha>                 # swapped atomically with rename(2)
├── instance/<sha>/                         # verified instance tree (shallow, sparse)
├── instance/current -> <sha>
└── toolchain/<hash>/                       # Python + ansible-core, sops, age, gitleaks, chezmoi
/var/lib/up/git/{engine,instance}.git       # root-owned bare mirrors used for fetch and verify
```

Trees are exported with `git archive` from the verified commit, so they contain no `.git`, no hooks and no config that git could execute. A run reads `current` once at start, so a concurrent update never mixes versions. The last three trees of each kind are kept for instant local rollback.

People edit in ordinary **working clones** anywhere in their home directory. Root never reads them. Changes reach hosts only as signed commits fetched into the root-owned mirror.

## Hermetic root runs and privilege drop

Every converge is orchestrated by root:

- **Interactive:** the user runs `up`; it makes one `sudo /opt/up/bin/up converge --as-root` call (no `sudo -v` keepalive loop), which does all the work and exits.
- **Unattended:** the scheduler and first-boot unit start `/opt/up/bin/up` as root directly.

Root starts from a clean environment: `env -i`, `PATH=/opt/up/bin:/opt/up/toolchain/current/bin:/usr/sbin:/usr/bin:/sbin:/bin`, `HOME=/root` (or `/var/root`), `ANSIBLE_CONFIG` set to the engine's file, `cwd=/`. The instance location comes only from `/etc/up/host.yml`.

For the user layer, root drops to the configured user with a clean environment (`runuser -u <user>` on Linux; `launchctl asuser <uid> sudo -u <user>` on macOS). User-layer secrets are resolved by root and handed over as a data file in `/run/up/<uid>/` (SEC-12). Root reads nothing the user layer produces. Homebrew is never run as root; tasks that drive it become the user.

## Signed engine releases

- The maintainer signs engine **release tags** and each release's `SHA256SUMS` with a hardware-backed SSH key (`sk-ssh-ed25519`, verify-required). A second hardware key, kept offline, is pre-registered as a backup signer.
- `up.sh` carries the engine signer set as a constant. It verifies `SHA256SUMS` with `ssh-keygen -Y verify` and the binary's hash before running it (BOOT-04).
- The binary embeds the same signer set and writes it to `/etc/up/allowed_signers.engine` at bootstrap.
- `up.yml` pins the engine by **version and commit**: `engine: {version: 0.3.0, commit: <sha>}`. Before installing a tree, `up` verifies the tag signature against the anchored signer set and checks that the tag resolves to the pinned commit.
- CI builds release artifacts but never holds a signing key (SUPPLY-03).

## Signed instance commits

- Hosts follow the instance's protected `main` and apply a commit only if it is **signed by the instance signer set** anchored at `/etc/up/allowed_signers.instance`.
- The anchor is written at bootstrap: from the rendered boot config (render knows it), or interactively after the operator confirms the signer fingerprints shown on screen.
- The instance keeps its signer set in `.up/allowed_signers`. A commit that changes it is accepted only if it verifies under the **current** anchored set; the host then updates its anchor. The anchor is never read from the tree being verified.
- GitHub's web-flow signature is not in the signer set. Merges are made locally and signed (CHG-11).

## Anti-rollback

`/var/lib/up/state.json` records the last applied engine version and commit and the last applied instance commit. A new instance commit must be a descendant of the last applied one (fast-forward only), and a new engine version must not be older. Going backwards needs a local, interactive `--allow-downgrade`. The normal rollback is a forward `git revert`.

## Protected operations

Some operations can weaken the system, so they need a human at a terminal: adding a guard exception, changing instance signers or sops recipients, allowing a downgrade, and activating a dev engine. Each one requires an interactive TTY confirmation that shows exactly what will change. An agent or script cannot supply it.

The engine's built-in commands and its protected roles (`base`, `security`, `ssh`, `users`, `hardening`, `scheduler`) cannot be shadowed by an instance (ARCH-03).

## Requirements

- **TRUST-01** Every file root executes, sources or loads as configuration, and every parent directory of it, MUST be owned by root and not writable by group or others.
  - AC: A test walks the resolved execution set (binary, engine tree, instance tree, toolchain, unit files, every binary root invokes) and fails on any non-root-owned or group/other-writable path.
  - AC: (negative) Making one file in `/opt/up/engine/current` user-writable makes the next scheduled run refuse to start, exit `1`, and name the path.
- **TRUST-02** Root runs MUST be hermetic: clean environment, fixed `PATH`, `HOME` and `ANSIBLE_CONFIG`, `cwd=/`, and the instance location read only from `/etc/up/host.yml`.
  - AC: (negative) A planted `ansible.cfg` in the user's home and in the current directory, and a hostile `PATH` entry, have no effect on a root run.
  - AC: (negative) A user pointer file naming another repo has no effect on a root run.
- **TRUST-03** Root MUST drop to the configured user with a clean environment for the user layer, MUST NOT read back anything the user layer writes, and MUST NOT run Homebrew as root.
  - AC: Files written by chezmoi are owned by the user; the root process opens no path under the user's home after the drop.
  - AC: (negative) A Homebrew task run without `become_user` is refused; a file the user layer writes for root to read is never opened by root.
- **TRUST-04** Engine and instance trees MUST be exported from verified commits into new directories under `/opt/up` and activated by an atomic `current` symlink swap; a run MUST read `current` once at start; the last three trees MUST be kept.
  - AC: Killing an update mid-export leaves `current` on the previous tree and the next run succeeds.
  - AC: A run started before a swap completes entirely on the old tree.
- **TRUST-05** Engine release artifacts MUST carry a `SHA256SUMS` file signed by the engine signer set, and hosts MUST verify it before installing any binary.
  - AC: (negative) A binary whose hash is missing from the signed sums, or sums signed by another key, is refused with exit `1`.
- **TRUST-06** Hosts MUST install an engine tree only if its tag is signed by the anchored engine signer set and resolves to the commit pinned in `up.yml`.
  - AC: (negative) A moved tag, an unsigned tag, and a tag signed by another key each make `up update` exit `1` without changing `current`.
- **TRUST-07** Hosts MUST apply only instance commits signed by the anchored instance signer set; a change to `.up/allowed_signers` MUST verify under the current anchored set before the anchor is updated.
  - AC: (negative) An unsigned commit, and a commit signed by a GitHub web-flow key, on `main` are not applied; the host logs, notifies, and exits `1`.
  - AC: (negative) A commit that adds a signer and is signed only by that new signer is refused.
- **TRUST-08** Hosts MUST refuse to move to an older engine version or to an instance commit that does not descend from the last applied commit, unless `--allow-downgrade` is given interactively.
  - AC: (negative) A force-pushed `main` that rewinds history makes the host exit `1` and notify.
  - AC: (negative) An instance commit that sets an older `engine.version` is refused.
- **TRUST-09** Rendered boot configs MUST pin `up.sh` by SHA-256 and fetch it from a release tag URL, never `main`; no executable content may be served over unauthenticated transport.
  - AC: Golden tests show the hash check in every rendered output; (negative) a modified `up.sh` fails the check and nothing runs.
- **TRUST-10** The instance's allowed sops recipients MUST be listed in `.up/recipients`, mirrored on hosts at `/etc/up/recipients.allow` and updated there only from verified commits; every recipient in `.sops.yaml` MUST be in that list; `up secrets` MUST refuse `updatekeys` otherwise; instance CI MUST report recipient changes on every PR.
  - AC: (negative) A commit adding an age recipient to `.sops.yaml` but not to `.up/recipients` fails instance CI, and `up secrets rotate` refuses to run against it.
- **TRUST-11** Changes to signers, recipients, the engine pin, `mirror`, and shadowed roles or commands MUST be reported in the converge log and through notification (OPS-03) before they are applied.
  - AC: Each change type in a fixture commit produces one notification naming it.
  - AC: (negative) If the audit record for a change cannot be written, the change is not applied.
- **TRUST-12** Adding a guard exception, changing signers or recipients, allowing a downgrade, and activating a dev engine MUST require an interactive TTY confirmation.
  - AC: (negative) Each operation run with stdin not a TTY exits `1` without changing anything.
- **TRUST-13** A dev engine MUST run as root only after `sudo up self dev --activate <path>`, which copies the tree into `/opt/up/engine/dev-<sha>`; `up doctor` and every converge log MUST show when a dev engine is active.
  - AC: With a dev engine active, `up doctor` exits `1` with a warning naming the tree; `up self dev --off` restores the pinned engine.
  - AC: (negative) Editing `~/src/up` after activation has no effect on root runs; `--activate` without a TTY exits `1`.
