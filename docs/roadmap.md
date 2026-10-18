---
title: Roadmap
status: draft
---

# Roadmap

Nine milestones, each shippable on its own. Each lists the requirements it delivers and the exit criteria that close it. The trust design lands first: it changes paths, schemas and the bootstrap sequence, so it is expensive to retrofit. After M3, onboarding, images and dev containers can proceed in parallel.

| Milestone | Goal | Depends on |
| --- | --- | --- |
| M0 | Foundations: repo, hardened CI, threat model, spec tooling | none |
| M1 | Trusted bootstrap: signed binary to a converged, verified base host | M0 |
| M2 | System and user layers, security baseline, operations | M1 |
| M3 | Secrets, enrollment and lifecycle | M2 |
| M4 | Onboarding: template, wizard, agent assets, guides, first release | M3 |
| M5 | Images and first boot | M3 |
| M6 | Dev containers | M3 |
| M7 | Push, netboot, Fedora Atomic | M5 |
| M8 | Hardening, public instances and 1.0 | M4, M6, M7 |

Kubernetes, Talos and Flatcar are out of the engine's scope (D-38).

## M0: Foundations

- Delivers: PUB-01 (skeleton paths only), PUB-09, PUB-10, PUB-12, SUPPLY-02, ARCH-05, ARCH-11, CHG-07 (lint stage only).
- Scope: engine repo with `docs/` (threat model included), `AGENTS.md`, `CLAUDE.md`, `LICENSE`, `SECURITY.md`, `CONTRIBUTING.md`, `.up-engine`; Rust crate skeleton; JSON Schemas with value patterns; spec checker; CI workflow lint; engine rulesets and push protection; two hardware signing keys.
- Exit criteria:
  - Engine CI is green: cargo test, shellcheck, yamllint, ansible-lint, gitleaks, workflow lint, spec checker.
  - The engine's rulesets and push protection are on.

## M1: Trusted bootstrap

Goal: on macOS and Ubuntu, the one-liner verifies and runs the signed binary, anchors trust, installs root-owned verified trees, and converges `base`, with the host lock and audit record in place.

- Delivers: BOOT-01 to BOOT-14, TRUST-01 to TRUST-08, TRUST-11, TRUST-12, TRUST-13, SUPPLY-01, SUPPLY-03, SUPPLY-07, SUPPLY-08, CLI-01 to CLI-05, CLI-08, ARCH-01, ARCH-02, ARCH-03, ARCH-04, ARCH-06, ARCH-08, ARCH-09, ARCH-10, CHG-03, CHG-11, OPS-01, OPS-02, GRD-01, GRD-02, GRD-03, GRD-06, GRD-08, GRD-09, GRD-10.
- Exit criteria:
  - A fresh macOS VM and a fresh Ubuntu container each go from the one-liner to a converged `base` host.
  - Every negative trust test passes: tampered binary, moved tag, unsigned commit, web-flow merge, user-writable file in the executed set, planted `ansible.cfg`, downgrade.
  - The template converges as both `fleet` and `flotilla`.
  - A token in a commit is blocked by the hooks and by push protection.

## M2: System and user layers, baseline, operations

- Delivers: ARCH-07, CLI-06, CLI-07, CLI-09, CHG-04, CHG-05, CHG-09, CHG-10, PERF-01 to PERF-07, HARD-01 to HARD-08, OPS-03 to OPS-06, SUPPLY-05.
- Scope: roles `packages`, `homebrew`, `macos`, `security`, `hardening`, `ssh`, `users`, `docker`, `telemetry_off`, `os_ai_off`, `ai`, `scheduler`; chezmoi handshake via `/run/up`; role manifests and removal; offline mode; notifications, heartbeat, staleness; reboot policy.
- Exit criteria:
  - Your MacBook and one Linux PC are fully managed by `up`, with the baseline verified by `up doctor`.
  - With networking off, `up` exits `3` with zero failed tasks; a host left stale for 7 days alerts.
  - Removing any role removes what it installed; shell and converge budgets pass.

## M3: Secrets, enrollment and lifecycle

- Delivers: SEC-01 to SEC-15, TRUST-10, GRD-04, GRD-05, GRD-07, GRD-11.
- Exit criteria:
  - The same fixture passes on `fake` and `sops` in CI and on `op` and `bw` by hand.
  - A fresh VM goes from "awaiting enrollment" to converged via `up secrets enroll`, and decrypts only its own files.
  - `up host retire` revokes and rotates everything for a test host; a signer rotation signed by the backup key is accepted.
  - Every SEC-03 non-disclosure channel is clean in the `--check --diff` and failing-task tests.

## M4: Onboarding and first release

- Delivers: PUB-01 (complete), PUB-02 to PUB-08, PUB-11, PUB-13, CLI-10, CLI-11, CLI-12, CHG-01, CHG-02, SUPPLY-06; `up init`, `up self`, `up release`.
- Exit criteria:
  - (manual) A fresh GitHub account creates a working private instance by each of the three paths.
  - Engine `v0.1.0` is released and signed locally with `up release`, and your instance pins it with `up self update`.

## M5: Images and first boot

- Delivers: IMG-01 to IMG-10, TRUST-09, SUPPLY-04.
- Exit criteria:
  - A stock Pi image converges on first boot from a hash-pinned `up.sh`, and enrolls.
  - A signed baked Pi image converges on first boot with no network (exit `3`), and fully once the network returns.
  - The image filesystem scan is clean; a tampered image is refused by `up image flash`.

## M6: Dev containers

- Delivers: DEVC-01 to DEVC-07.
- Exit criteria:
  - Signed images build for both architectures; `up container init` plus VS Code opens a project with dotfiles, a confirm-on-use agent and signing.

## M7: Push, netboot, Fedora Atomic

- Delivers: CHG-06, IMG-11; the `push` backend; the `control_node` role; the Fedora Atomic image recipe.
- Exit criteria:
  - A hardware-key push converges a server through the `up-push` account, and nothing else can run as root through it.
  - A netbooted host installs from signed iPXE over HTTPS and reaches "awaiting enrollment" without receiving a secret.

## M8: Hardening, public instances and 1.0

- Delivers: CHG-07 (all stages), CHG-08, IMG-12, GRD-12; `up versions`; Renovate in the engine and template; scheduled VM tests; public-instance support.
- Exit criteria:
  - Every MUST requirement's ACs pass.
  - A public test instance passes guard with personal data only in sops files.
  - Engine `v1.0.0` is released with upgrade notes.
