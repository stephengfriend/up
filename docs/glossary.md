---
title: Glossary
status: draft
---

# Glossary

| Term | Meaning |
| --- | --- |
| Engine | The public `up` repo: source of the `up` binary, generic roles, guard, build definitions, docs, template and prompts |
| Instance | A user's private repo: `up.yml`, inventory, secrets, dotfiles, instance-only roles and commands, signer and recipient lists |
| `up` binary | The compiled Rust CLI, released with signed checksums; installed root-owned at `/opt/up/bin/up` |
| Verified tree | An engine or instance tree exported from a verified commit into `/opt/up/engine/<sha>` or `/opt/up/instance/<sha>` |
| `current` | The symlink naming the active verified tree, swapped atomically |
| Working clone | A user-owned clone used for editing; root never reads it |
| Name | The instance's project name in `up.yml` (default `fleet`); drives repo, vault, image and container names |
| `up.yml` | Instance config: name, remote, visibility, engine version and commit, vaults, mirror, taps, notification references, privacy denylist reference |
| `host.yml` | `/etc/up/host.yml`: host identity only (name, instance remote, inventory alias, user) |
| Trust anchor | A root-owned file in `/etc/up` that verification depends on: signer sets, recipient allowlist, GitHub host keys |
| Signer set | The SSH public keys allowed to sign engine releases (`allowed_signers.engine`) or instance commits (`allowed_signers.instance`); always includes an offline backup key |
| Recipient allowlist | `.up/recipients`, mirrored to `/etc/up/recipients.allow`: the age recipients `.sops.yaml` may use |
| Anti-rollback | Hosts refuse older engine versions and non-descendant instance commits without a local `--allow-downgrade` |
| Hermetic root run | A root run with a clean environment, fixed `PATH`, `HOME` and `ANSIBLE_CONFIG`, and `cwd=/` |
| Enrollment | Registering a host's on-device age recipient and deploy key (`up secrets enroll`) so it can read its secrets and fetch updates |
| Pre-generation | Generating a host's keys at flash time, streamed to the device, only for baked images that must decrypt offline at first boot |
| Discovery | Finding the GitHub login and the instance repo on a new machine |
| Bootstrap | What `up` does when `/etc/up/host.yml` doesn't exist: discover, anchor, install, converge |
| Converge | Fetch and verify, then the system layer as root, then the user layer as the user |
| Offline mode | A converge with remotes unreachable at the start-of-run probe (or `--offline`): only `network`-tagged tasks skip; exit `3` |
| Declared skip | A skip decided before a task runs: offline mode, no secrets session, awaiting enrollment |
| Phase | The tag every Ansible task carries: `bake`, `provision` or `converge` |
| Bake | Building a system image by running `up` for a class in the image build, then sanitizing and signing it |
| Protected roles | `base`, `security`, `ssh`, `users`, `hardening`, `scheduler`: cannot be shadowed by an instance |
| Guard | Client-side hooks and scans that catch credentials and personal data before they leave a working clone |
| Heartbeat | A ping on each full success to a dead-man's-switch receiver |
| Control node | Any host with the optional `control_node` role; pushes over SSH to the `up-push` account and builds images |
| Pull / Push | A host converging itself / a control node triggering the same converge over SSH |
| Wizard | `up init` |
| Prompt / Adapter | An agent-agnostic task file in `prompts/` / a generated tool-specific wrapper of one |
