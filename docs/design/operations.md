---
title: Operations
status: draft
prefix: OPS
milestones: [M1, M2]
depends_on: [architecture, trust, change-management]
---

# Operations

A fleet you can't see is a fleet you can't trust. Every run takes one host-wide lock, leaves an audit record, and updates a state file. Failures, security events and silence all raise an alert. Reboots happen only when a class allows them, inside its window.

## Lock

Every mutating command (`converge`, `update`, `bootstrap`, `system`, `dotfiles` apply, `schedule`, the first-boot unit, an incoming `up push`) takes one host-wide `flock` on `/run/up/lock` (root, directory `0700`). A second run waits a short time, then exits `1` with "locked by pid N".

## State and audit

- `/var/lib/up/state.json` holds: last start, last full success, last exit code, applied engine version and commit, applied instance commit, skipped-task count and reasons, and the role ledger used for role removal (PERF-02).
- `/var/lib/up/audit.jsonl` gets one append-only JSON line per run: start, end, trigger (interactive user, scheduler, first boot, push), verified signer fingerprint, engine and instance SHAs, exit code, changed and skipped tasks.
- `/var/log/up/up.log` is JSON lines, rotated by logrotate on Linux and newsyslog on macOS.

## Notifications

`notify.webhook` and `notify.heartbeat` in `up.yml` are catalog references: the URLs are secrets. Payloads carry the host alias, exit code, event type and failing task names, never task output.

| Event | Notified |
| --- | --- |
| Run failed (exit `1`) | Yes |
| Signature verification failed, downgrade refused | Yes |
| Shadowing of a protected role or command attempted | Yes |
| A commit carrying a `UP_GUARD_SKIP` trailer was applied | Yes |
| Signers, recipients, engine pin or mirror changed (TRUST-11) | Yes |
| No full converge in `stale_after` days (default 7) | Yes |
| Full success | Heartbeat only, if `notify.heartbeat` is set |
| Exit `2` or `3` | No (recorded in state) |

The heartbeat is a dead-man's switch: the receiver alerts when a host stops checking in, which catches hosts that are off, frozen, or have lost their timer.

## Reboots and upgrades

- Converge detects when a reboot is required (`/var/run/reboot-required`, `needs-restarting -r`, `rpm-ostree status`, macOS pending updates).
- Each class sets `maintenance_window` and `auto_reboot` in inventory. Defaults: workstations never reboot automatically and only report; servers and Pis reboot within their window.
- OS and feature upgrades follow the same window. Security updates are not gated (HARD-04).
- Hosts in a group marked `serial: true` get staggered windows, computed from their order in inventory, so no two reboot at once.

## Requirements

- **OPS-01** Every mutating command MUST hold the host-wide lock in `/run/up/lock` for its duration.
  - AC: An interactive `up` started during a scheduled run exits `1` naming the holder; (negative) a symlink planted at the lock path is refused.
- **OPS-02** Every run MUST update `state.json` and append one record to `audit.jsonl`, both root-owned and not group- or world-writable; logs MUST be JSON lines with rotation configured.
  - AC: After a run, the audit record contains SHAs, trigger, exit code and changed-task count; rotation config exists on macOS and Linux.
  - AC: (negative) Appending to `audit.jsonl` or editing `state.json` as the user fails.
- **OPS-03** The notification events in the table MUST be sent, with redacted payloads; webhook URLs MUST come from the catalog.
  - AC: One fixture per event produces one webhook call; (negative) a literal URL in `notify.webhook` fails schema validation.
- **OPS-04** When `notify.heartbeat` is set, every full success MUST ping it.
  - AC: A successful run produces one heartbeat call; a failed run produces none.
- **OPS-05** A host with no full converge in `stale_after` days MUST notify and make `up doctor` exit `1`.
  - AC: With `state.json` aged eight days, `doctor` exits `1` and a notification is sent.
- **OPS-06** Reboots and OS upgrades MUST happen only for classes with `auto_reboot: true`, and only inside `maintenance_window`; reboot-required state MUST be reported otherwise.
  - AC: (negative) A reboot-required VM outside its window is not rebooted, and `up host` shows "reboot required".
