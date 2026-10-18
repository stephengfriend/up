---
title: Security baseline
status: draft
prefix: HARD
milestones: [M2]
depends_on: [threat-model, architecture]
---

# Security baseline

A configuration-management tool with a `security` role and no security requirements ships whatever its first implementer writes. These are the controls every managed host gets, implemented in the protected engine roles `security`, `hardening`, `ssh` and `users`, which instances can extend through `_pre`/`_post` hooks and variables but cannot replace. Each control is set per class in inventory; `up doctor` reports the state of each.

| Control | Default |
| --- | --- |
| sudo | Workstations: password or Touch ID (`pam_tid`), no `NOPASSWD`. Servers and Pis: `NOPASSWD` only for the admin user where unattended work needs it, reported by `doctor`. The `up-push` account gets a scoped rule (CHG-06) |
| sshd | `PasswordAuthentication no`, `KbdInteractiveAuthentication no`, `PermitRootLogin no`, `AllowGroups` set, Ed25519 and FIDO keys only |
| Firewall | Default-deny inbound: nftables or firewalld on Linux, the application firewall on macOS. Roles declare the ports they open |
| Security updates | Automatic on every class: `unattended-upgrades`, `dnf-automatic` (security), macOS critical updates. Feature and OS upgrades are explicit, within maintenance windows (OPS-06) |
| Disk encryption | Verified, not just recommended: FileVault on macOS, LUKS with TPM2 enrollment on x86 Linux. Pis are documented as unencrypted (accepted risk) |
| macOS integrity | SIP and Gatekeeper enabled, screen lock set; signed `.mobileconfig` profiles whose identifier and payload `doctor` verifies; LaunchDaemon plists `root:wheel 0644` |
| Audit | Privileged events kept by auditd or the unified log; every `up` run writes an audit record (OPS-02) |
| Docker | Membership in the `docker` group is root-equivalent. Servers use rootless Docker or Podman; group membership needs an explicit role variable, and `doctor` warns |

## Requirements

- **HARD-01** The `users` role MUST apply the sudo policy per class, MUST NOT grant blanket `NOPASSWD: ALL` on workstations, and `up doctor` MUST report every `NOPASSWD` rule.
  - AC: (negative) A workstation fixture with `NOPASSWD: ALL` in sudoers is corrected by converge; `doctor` lists a server's `NOPASSWD` rules.
- **HARD-02** The `ssh` role MUST enforce the sshd baseline.
  - AC: `sshd -T` output matches a golden file per platform; (negative) a password login attempt in a VM test fails.
- **HARD-03** The `hardening` role MUST enforce default-deny inbound firewalling with role-declared openings only.
  - AC: (negative) A VM scan finds no open port except those declared by the host's roles.
- **HARD-04** Automatic security updates MUST be enabled on every class; feature and OS upgrades MUST NOT run outside the class's maintenance window.
  - AC: The update service is enabled and configured for security updates only, per platform; (negative) a scheduled converge outside the window does not run an OS upgrade.
- **HARD-05** Converge and `up doctor` MUST verify disk encryption state (FileVault, LUKS) and report it; the FileVault personal recovery key MUST be stored in the vault.
  - AC: `doctor` reports encryption state correctly on an encrypted and an unencrypted VM.
  - AC: (negative) On a class that requires encryption, an unencrypted VM makes `doctor` exit `1`.
- **HARD-06** The `macos` role MUST verify SIP, Gatekeeper and screen lock; configuration profiles MUST be signed and verified by identifier and contents; LaunchDaemon plists MUST be `root:wheel 0644` and point at `/opt/up/bin/up`.
  - AC: (negative) A modified installed profile is reported by `doctor`.
- **HARD-07** Privileged events MUST be retained by auditd (Linux) or the unified log (macOS) for at least 30 days.
  - AC: A `sudo` event from a VM test is found in the retained audit log.
- **HARD-08** The `docker` role MUST default to rootless Docker or Podman on servers and MUST add users to the `docker` group only when an explicit variable is set; `doctor` MUST warn on group membership.
  - AC: (negative) A server converge without the variable leaves the `docker` group empty.
