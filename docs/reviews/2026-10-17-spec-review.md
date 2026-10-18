# `up` engine spec: critical review

Reviewed: all 24 files in `up-engine-spec.zip` (README, AGENTS/CLAUDE, docs index, 11 design docs, decisions, glossary, open questions, roadmap, 2 prompts, 2 adapters). Date: 2026-10-17.

## Verdict

The spec is well above average in **process**. It has requirement IDs, testable ACs, a decision log that keeps superseded entries, a glossary, open questions with owners, and a clean engine/instance split. Its own consistency rule (PUB-09) holds: 93 IDs are defined, and every one is assigned to a milestone with no orphans.

It is not yet ready for integration because of three structural problems in the **system design**:

1. **The trust model is implicit, and several paths escalate to root.** Root runs code from user-writable checkouts. Hosts apply unsigned commits from instance `main`. Engine tags are mutable and unverified.
2. **Local state overrides the source of truth.** `host.yml` roles beat inventory (D-13), so a host's roles freeze at flash time.
3. **The on-host layout assumes a single user's `$HOME`.** That conflicts with the root-started unattended model (D-31), with multi-user hosts, and with immutable OSes.

Everything else is fixable with targeted spec edits. Findings are grouped by severity, and each one names the IDs it touches and a proposed change.

---

## What is strong (keep it)

- **Engine/instance separation with an explicit pin (D-25, ARCH-02).** This is the right call over submodules, and the rename test (`fleet`/`flotilla`) is a nice forcing function.
- **Pull by default, push optional (D-08).** No required control plane.
- **`up.sh` is minimal and wrapped in `main "$@"`, with a truncation test (BOOT-03).** Good one-liner hygiene.
- **The token is passed through `GIT_CONFIG_*` env, not argv or config (BOOT-08), and BOOT-08 has a real test.**
- **Phase tags enforced by lint (IMG-01)** make bake safety mechanically checkable.
- **Offline is designed in, not bolted on.** It has a distinct exit code and Molecule offline scenarios.
- **Guard fails closed, the allowlist holds fingerprints and never matched text (GRD-07), and bypasses leave a recorded trailer.**
- **The `fake` secrets backend** lets CI exercise the whole secret path.
- **Performance budgets are measured in CI (PERF-03/04)** rather than asserted.

---

## Critical: fix before any implementation work

### C1. Root executes user-writable code (local privilege escalation)

The engine lives at `~/.local/share/up` and the instance at `~/.local/share/<name>`, both owned by the user. `/usr/local/bin/up` symlinks into the user's home. Unattended runs start as root (D-31, CHG-05). Any process running as that user can edit `bin/up`, a role or a `libexec/` script, and the next scheduled run executes the edit as root. That includes a malicious npm postinstall, a compromised editor extension, or a dev container with a home bind-mount.

The same layout also breaks in practice. Since Git 2.35.2, git refuses to operate on a repo owned by another user ("dubious ownership"), so root's `up update` against the user's checkout fails. The usual workaround, `safe.directory=*`, disables the protection that would have caught this.

**Proposed change.** Split system and user state:
- Put root-owned, user-read-only checkouts at a system path, for example `/opt/up/engine` and `/opt/up/<name>`, or `/var/lib/up/...`. Root runs only from these.
- The user layer runs as the user, from the same root-owned tree. Personal edits happen in a separate dev clone (`up self dev` points there).
- `/usr/local/bin/up` points at the root-owned tree.
- New requirement: *"Code executed as root MUST reside in paths writable only by root."* AC: a test that `find` reports no user-writable file in the executed tree, and that a scheduled run refuses to start otherwise.

### C2. No integrity verification on what hosts apply

- **Engine.** `engine.ref` is a tag name, and tags are mutable. Nothing verifies a tag signature. D-21 ("GitHub trusted") is the whole trust model. Anyone with tag-push rights on the engine reaches every new bootstrap through `up.sh`'s "newest `v*` tag" rule, and reaches every host on its next `up self update`.
- **Instance.** Hosts follow instance `main` by default (`host.yml` `ref: main`). They apply whatever lands there within one schedule interval, as root, on every host. Anyone who can push to the instance gets root on the fleet.

**Proposed change.**
- Pin the engine by tag **and** commit SHA in `up.yml`, for example `engine: {ref: v0.3.0, commit: <sha>}`. `up self update` writes both, and `up update` refuses to run if the tag resolves to a different SHA. CHG-03's AC (`git describe --tags` equals ref) does not detect a moved tag, so it needs replacing.
- Sign engine release tags. At bootstrap, store an `allowed_signers` file root-owned on the host (trust on first use). Verify every later engine checkout against it with `git verify-tag`.
- For the instance, require signed commits on protected `main` and have hosts verify against the instance's `allowed_signers` before converging (`git verify-commit`). Make the default host `ref` a signed dated tag, not `main` (see C5).
- Add tag protection (rulesets) to the manual steps for both repos.

### C3. Bootstrap secrets outlive bootstrap, and the scrub misses the real copies

- **The SSH host key, which is also the host's age identity, is written through cloud-init `write_files`.** cloud-init's default `ssh_deletekeys: true` makes the `ssh` module delete and regenerate host keys on first boot, and it runs after `write_files`. The injected key is likely destroyed and replaced, so the pre-registered age recipient no longer matches, and the host cannot decrypt its own secrets. Use cloud-init's `ssh_keys:` (`ed25519_private` and `ed25519_public`) instead, and add an AC that boots a stock image and decrypts a host file.
- **The scrub targets "cloud-init's cached user-data" only.** On Raspberry Pi images the seed (`user-data`) lives on the FAT boot partition. That copy holds the bootstrap token **and the host's age private key** in plaintext. It survives the scrub and is readable by anyone holding the SD card. On cloud VMs, user-data stays retrievable from the metadata service for the instance's lifetime, whatever you delete locally.
- **SEC-06 deletes credentials only "after the first successful converge."** A host that fails its first converge keeps a live token indefinitely.
- **Key custody is undefined.** `up secrets add-host` generates each host's private key on the workstation, and the spec never says where those keys live afterward or when they are destroyed.

**Proposed change.**
- Scrub the boot-partition seed too.
- Scrub on exit regardless of outcome, or make the token short-lived by construction. The fine-grained PAT should expire in hours, and the Packer build credential likewise.
- Don't put long-lived private keys in user-data on platforms with a metadata service. Deliver them through the `push` backend or a one-time fetch.
- Specify that workstation-generated host keys are either stored only in the password manager or shredded after flashing.
- Amend SEC-06's AC to cover `/boot/firmware/user-data` and the failed-converge case.

### C4. Local facts override inventory, so roles freeze at flash time

D-13 and BOOT-12 resolve identity as flags > local facts > inventory, and `host.yml` contains `roles`. The cloud-init template writes `roles: [k8s_agent]` at flash. After that, changing a host's groups in `inventory/hosts.yml`, the git source of truth, has no effect on that host. `up host set roles +ai` creates further local-only state that git never sees. This quietly breaks the GitOps premise ("Git is the only way a change reaches a machine").

**Proposed change.** `host.yml` holds **identity only**: the inventory hostname, the user, and possibly the backend. Roles always come from inventory. Local role overrides are allowed only for hosts absent from inventory (containers, ad-hoc machines). `up host set roles` should edit inventory and commit, mirroring how `up self update` edits `up.yml`.

There is a related conflict with the public-instance model. Guard requires inventory to use aliases (`node-a`), with real hostnames encrypted, but identity step 3 matches **the real hostname** against `hosts.yml`. On a public instance, that match fails unless hostname equals alias. Define the match key explicitly: the alias stored in `host.yml`, or a machine fingerprint.

---

## High: resolve before the milestone that owns them

### H1. In-place checkout swaps produce mixed-version runs

`up update` and BOOT-09's re-exec run `git checkout` on the tree that the running `up` is executing from. Git replaces files, so the running dispatcher keeps its old inode while later-loaded subcommands, roles and Python modules come from the new ref. A partially failed checkout leaves a tree that matches no release. The `up.sh` sketch has the same hazard, and it also conflicts with dev mode: `up.sh` force-checks out the newest tag over a dev branch or a dirty tree, and dies under `set -e`.

**Proposed change.** Use immutable versioned trees with an atomic pointer: `…/engine/<sha>/` plus a `current` symlink swapped with `rename(2)`, as Nix, Capistrano and ostree do. This also gives instant engine rollback, a prune policy, and safe concurrent reads.

### H2. Concrete bugs in the `up.sh` sketch (BOOT-02, BOOT-04, BOOT-11)

- **When `ref=main` and the clone already exists,** `git fetch` updates `origin/main`, but `git checkout main` checks out the stale local branch. Re-runs never advance it. Check out `origin/<branch>` detached, or `reset --hard` to it.
- **`git fetch --tags … || true` hides a moved tag.** Since Git 2.20, fetch refuses to clobber a changed tag, and `|| true` swallows that error. The host silently keeps the old tag, which is exactly the C2 situation and should be a hard error.
- **`--sort=-v:refname` ranks `v1.0.0-rc1` above `v1.0.0`** unless `versionsort.suffix` is set. A pre-release becomes the "newest" tag. Filter pre-releases or set `-c versionsort.suffix=-`.
- **On Linux, `install_git_pkg` needs root**, but the one-liner runs as the user, and `sudo` handling is not specified anywhere in `up.sh`.
- **The 80-line limit (BOOT-02) will pressure authors toward compressed, unreadable code** once CLT install, apt/dnf, sudo, pre-release filtering and dev-mode detection are real. Prefer behavioral constraints, such as BOOT-01's banned-tools check, over a line count.

### H3. Bootstrap ordering: `up.yml` is read before any YAML parser exists

Bootstrap step 3.2 reads `up.yml` to get `engine.ref` and re-exec. uv is installed at step 3.4. A bash dispatcher cannot reliably parse YAML. The same gap affects `up doctor`, which validates schemas and is allowed to run without bootstrapping (CLI-05). In the same vein, nothing in BOOT-10 installs **gitleaks**, but hooks fail closed (GRD-09). The first commit in a freshly bootstrapped checkout is blocked until gitleaks is installed by some unspecified step.

**Proposed change.**
- Install uv, or a pinned single static binary, before reading any config.
- Add gitleaks (pinned, with a checksum) to BOOT-10's post-conditions.
- This is the strongest argument in **Q-03**. A single compiled `up` binary, released with checksums and signatures, removes the bash-plus-uv bootstrapping knot, YAML parsing, schema validation and the GitHub API client in one stroke. Recommend deciding Q-03 that way before M1.

### H4. Exit code 3 can hide real failures, and nothing detects silent hosts

- CHG-10 records "any other task that fails for a network reason" instead of failing it. "Network reason" has no definition, and a heuristic classifier will misfile genuine failures as exit `3`. Scheduler and first boot treat `3` as success, and CHG-07 never notifies on it.
- **There is no heartbeat.** A failures-only webhook cannot detect a host that stopped running: timer disabled, disk full, clock wrong, powered off, or offline for six months while reporting `3` daily.
- Exit-code semantics collide. CLI-06 passes `up ansible --` through unchanged, and `ansible-playbook` uses `2` for host failures, while CLI-03 says `2` means "changes pending". Precedence is also undefined for drift plus offline under `--check`.

**Proposed change.**
- Online, a network failure is a failure (exit `1`). Only an explicit offline determination (the probe, or `--offline`) produces `3`, and in offline mode only `network`-tagged tasks are skipped.
- Write a state file on every run: last start, last full success, engine SHA, instance SHA, exit code and skipped-task count.
- Add a staleness alert ("no full converge in N days") and an optional success heartbeat (dead-man's switch).
- Exempt passthrough commands from CLI-03, and document the precedence `1 > 2 > 3 > 0`.

### H5. A single global lock, rollout safety and reboot policy are missing

- Locking is specified only for the scheduler (CHG-05). An interactive `up` can race a scheduled run, a first-boot run or `up push`. Require one host-wide lock for every mutating command.
- "Canary host per class follows `main`; others follow the latest tag" appears only as a row in the Testing table. No requirement implements it, and the default `host.yml` `ref: main` contradicts it. Make rollout rings a requirement, with a default of tags for everything except canaries.
- **Reboot and upgrade policy is undefined** (Q-11 is open). For `k8s_server` and `k8s_agent`, jittered daily runs can drain or reboot several nodes at once. Specify a reboot-required detection step, a maintenance window per class, and serialization (cordon and drain, one node at a time) for cluster roles.

### H6. Instance shadowing can disable controls and silently fork engine code

- CLI-01 lets an instance `libexec/up-guard` replace guard. The thing being guarded can switch off its own guard. Make `guard`, `self`, `bootstrap`, `update` and `doctor` non-overridable.
- Whole-role shadowing (ARCH-03) means an instance `base` forks the engine's `base`. It stops receiving fixes, and any engine role depending on `base` gets the instance copy. Prefer extension points (`<role>_pre`/`_post` hooks, documented role variables). Where shadowing stays, `up doctor` should list shadowed roles and the engine version they were forked from.

### H7. Secrets blast radius, and no decommission or offboarding workflow

- One `op` or `bws` machine token "scoped to the instance vault" means every headless host can read every secret, including other hosts' keys and the deploy key. Use per-class vaults or projects, or keep host-specific material in sops only.
- One shared `instance_deploy_key` on every headless host means one stolen Pi leaks fleet-wide read access, and revoking it breaks every host. GitHub allows many deploy keys per repo, so use one per host, or GitHub App installation tokens.
- **There is no host decommission or lost-device procedure.** It needs to: remove the age recipient and **rotate the secret values** (`sops updatekeys` does not protect ciphertext already in git history), revoke the deploy key and machine token, remove the host from inventory and the netboot server, and wipe it. Add a `SEC` requirement and `up host retire <host>`.
- **sops-decrypted files for chezmoi** ("a file Ansible already decrypted to a `0600` path") have no location or lifetime. Specify a tmpfs or runtime directory removed at the end of the run.
- **On macOS workstations, scheduled user-layer runs with the `op` backend** depend on the desktop app's biometric unlock and will fail or hang headless. Define the behavior: skip secret-bearing templates when no session is available, and exit `3` with a reason.

### H8. Bake vs. user identity conflict

The `bake` phase allows "chezmoi apply" and Homebrew, but the user account is created at **first boot** from cloud-init, and Q-10 (one username or per host) is still open. A class image cannot apply a user's dotfiles to a user who does not exist yet. Templates that call the `secret` partial will also fail at bake time, because bake forbids secrets.

**Proposed change.** Bake with a fixed, class-wide user (or system-wide installs only). Mark secret-bearing chezmoi templates so the bake phase skips them, and add an AC that bakes the template with secret partials present.

### H9. Unpinned network fetches during bootstrap

`versions.yml` pins versions but not hashes. The bootstrap fetches uv, ansible-core, a "static `gh` binary", Homebrew's installer, mise and gitleaks. For each one, pin SHA-256 checksums (or verify signatures or attestations) next to the version, and make verification a requirement. In the same spirit, `ssh -T git@github.com` login discovery prompts on an unknown host key, which hangs unattended runs, and invites MITM. Ship GitHub's published SSH host keys as a pinned `known_hosts`.

---

## Medium

- **The guard is client-side, and `git push --no-verify` skips it.** For a public repo, CI runs after the secret is already public. Add GitHub secret scanning **push protection** (server-side) to the manual steps for both repos. Require redacted output everywhere (`gitleaks --redact`). Today, a public instance's CI log would print the very line that leaked. When `visibility` flips from private to public, require a full `--history` scan under public rules.
- **The guard's IP rule** ("IPs outside documentation ranges") will flag RFC 1918 addresses and version strings like `1.2.3.4`. Expect allowlist churn, and consider a warn tier.
- **PERF-02 (removing a role removes what it installed) has no mechanism.** Ansible does not run tasks for roles a host no longer has. This needs a ledger of previously applied roles plus per-role `absent` handlers, and it needs reference counting, since packages are the union of roles. `brew bundle cleanup` covers Homebrew only. Specify the mechanism or downgrade to SHOULD with a defined scope.
- **First-boot unit.** `/var/lib/up/` must exist, or `ExecStartPost` fails after a successful converge. There is no retry policy for exit `1` (`Restart=on-failure` with backoff works for oneshot units on recent systemd). An offline first boot writes `firstboot.done` and relies on the scheduler to finish provisioning, so require the scheduler to be enabled during provision. Sanitization should empty `/etc/machine-id` rather than delete it, per systemd's first-boot semantics.
- **Platform reach.** Flatcar has no Python and no package manager, and its `/usr` is read-only. Local Ansible, Homebrew and `/usr/local/bin/up` all need special handling there. Treat Flatcar like Talos (no `up` on the host) or drop it from the roadmap. Fedora Atomic package changes need `rpm-ostree` and a reboot, which strains "one code path." Homebrew refuses to run as root, so the `homebrew` role in the root-run system layer must become the user explicitly. On macOS, user-domain `defaults` written from a root launchd daemon need `launchctl asuser`, and some domains need Full Disk Access granted to the scheduled binary.
- **The macOS fleet without MDM:** user-approved `.mobileconfig` profiles cannot enforce many restrictions, and FileVault key escrow and OS update enforcement are absent. State this explicitly as a non-goal, or plan an MDM integration point.
- **Engine and instance compatibility contract.** The two repos version independently, but `up.yml` and `host.yml` have no `schema_version`, and the engine declares no supported instance range. Note also that bootstrap always runs the *newest* engine (via `up.sh`), then re-execs into the *pinned* one. Newest-engine bootstrap code must therefore handle every supported older instance, and an older engine must read state files written by a newer one. Add `schema_version` fields, an engine `supports: instance_schema >= N` declaration, and on-host migration steps in CHG-01's notes.
- **Discovery.** Restrict topic matches to repos **owned by** the login, not collaborator or org repos (this relates to Q-15). Unset `GH_TOKEN` and `GITHUB_TOKEN` after the clone so they are not inherited by Ansible, brew and every other child process.
- **Dev containers.** A fixed UID 1000 breaks bind-mount permissions on hosts where the user's UID differs; specify `updateRemoteUserUID` behavior. Homebrew in `<name>-base` contradicts "lean"; default to mise's aqua backend. Drop the "read-only `~/.ssh` mount", which exposes private keys to every process in the container, and use agent forwarding only.
- **Logging.** There is no rotation for `/var/log/up` on macOS (newsyslog) and no structured format. JSON-lines output would make the state file and webhook trivial.

---

## Spec-process issues

- **ARCH-04 vs. BOOT-07.** ARCH-04 forbids hardcoding the project name, and its AC greps for `fleet`. But bootstrap discovery step 4 tries `<login>/fleet`. Move the default name into `versions.yml` or the template, or accept the exception explicitly.
- **Missing requirements for described behavior.** These appear in prose or tables with no ID: the canary/rollout ring, global locking, the state file, reboot policy, decommissioning, compatibility and schema versioning, checksum verification, and GitHub host-key pinning. PUB-09 checks ID hygiene but cannot catch behavior with no ID. Consider adding to `plan-from-spec`'s "Gaps" step a check for normative words (*must*, *never*, *only*) outside requirement bullets.
- **There is no threat model.** D-21's "GitHub trusted" is the only statement of trust. Add a one-page `docs/design/threat-model.md` covering: actors (engine maintainer account, instance pusher, local unprivileged malware, stolen SD card, stolen laptop, compromised Pi, GitHub itself); assets (root on hosts, secrets, personal data); and which requirement mitigates each actor. Most of the Critical findings above fall out of writing it.
- **Scope.** One maintainer, a dozen platforms (macOS, Debian, Ubuntu, Pi OS, Fedora, Atomic, Bazzite, Flatcar, Talos, NAS, PXE), three image toolchains, and a public product with a wizard. Make M7 (Talos, netboot, Atomic, Flatcar) explicitly post-1.0, and cut Flatcar outright unless there is a concrete host for it.
- **`AGENTS.md` and `prompts/` reference files that don't exist yet** (`CONTRIBUTING.md`, `docs/guides/*`, `prompts/init-instance.md`). That's fine at design stage, but PUB-06's path check will fail from M0. Gate it to M4.

---

## Suggested resolutions for open questions

| Q | Recommendation |
| --- | --- |
| Q-03 | Compiled single binary (for example Go), released with checksums and signatures. It removes H3 and simplifies C1, C2 and H1 |
| Q-10 | One username across hosts by default, with an override in `host.yml`. This unblocks H8 |
| Q-11 | Converge config and apply security updates. Full OS upgrades are an explicit command with per-class maintenance windows (H5) |
| Q-12 | Start private. Treat public as an M8 feature once guard and push protection are proven |
| Q-15 | Use a specific topic (`up-instance-v1` or `<you>-up-instance`) **and** require owner equals login |

---

## Recommended order of work

1. Write the threat model, then amend the design for C1 (system-owned trees), C2 (signed and SHA-pinned refs) and C4 (identity-only local facts). These change paths and schemas, so they must land before M1 code.
2. Decide Q-03, then fix H2 and H3 in the bootstrap spec.
3. Add requirements for the global lock, state file and heartbeat, rollout rings, and decommissioning. Assign them to M1, M2, M2 and M3 respectively.
4. Fold C3 and H7 into M3 (secrets) and M5 (images) ACs.
5. Re-scope M7, and mark the Medium items as SHOULDs in their owning docs.
