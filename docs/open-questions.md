---
title: Open questions
status: draft
---

# Open questions

Each question blocks or shapes the milestone shown. Answer it by adding a decision to `decisions.md` and updating the affected design doc. Resolved: Q-03 → D-32, Q-04 withdrawn (Kubernetes out of scope, D-38), Q-11 → D-40, Q-12 → D-41.

| ID | Question | Affects | Blocks |
| --- | --- | --- | --- |
| Q-01 | Engine owner, default project name (`fleet`?), and a vanity domain for the one-liner? | `up.sh`, `defaults.yml` | M0 |
| Q-02 | License for the engine (MIT, Apache-2.0, MPL-2.0, …)? | PUB-10 | M0 |
| Q-05 | NAS platform: TrueNAS, Unraid, Synology or plain Linux? Does it host netboot? | IMG-11 | M7 |
| Q-06 | Home Assistant OS appliance, or a container on a managed host? | Instance roles | M7 |
| Q-07 | Gaming box distro (Bazzite?) and GPU vendor? | Atomic image | M7 |
| Q-08 | Intel Macs, x86 Linux PCs, or both? | Build targets, test matrix | M1 |
| Q-09 | Are 1Password service accounts available on your plan, and how many class vaults? | SEC-04 | M3 |
| Q-10 | One admin username across hosts, or per host? (The `up-push` account is separate either way.) | `users` role, bake user | M2 |
| Q-13 | Which services receive the webhook and the heartbeat? | OPS-03, OPS-04 | M2 |
| Q-14 | Homebrew bottle coverage on ARM64 Linux for your tool list (fallback: mise aqua backend)? | Pi roles | M5 |
| Q-15 | Discovery topic name: `up-instance`, `up-instance-v1`, or owner-specific? | BOOT-07 | M1 |
| Q-16 | macOS code signing for the `up` binary: is a Developer ID worth it? Ad-hoc signatures change every build, so a TCC grant (Full Disk Access) would reset on each update. Until decided, nothing in the scheduled path may need a TCC grant | HARD-06, TRUST-05 | M2 |
| Q-17 | Maintenance windows and `stale_after` per class: what defaults suit your servers and Pis? | OPS-05, OPS-06 | M2 |
