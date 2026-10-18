---
title: Lean and fast
status: draft
prefix: PERF
milestones: [M2]
depends_on: [architecture]
---

# Lean and fast

Nothing is installed unless a role asks for it, removing a role removes what it installed, and every speed target is measured in CI.

## Package policy

- `base` role: the short list every host gets (git, curl, zsh, chezmoi, mise, ripgrep, fd, jq, a few more). Under about 20 items.
- Everything else belongs to a role. A host's footprint is the union of its roles. `control_node` and `ai` are roles like any other.
- System packages from apt, dnf or rpm-ostree, only what can't run in user space. User CLI tools from Homebrew (official taps). Runtimes from mise.
- macOS GUI apps from Homebrew casks; Mac App Store apps through `mas`, listed per role.
- Fedora Atomic: avoid layering. Flatpak for GUI apps, Homebrew for CLI, toolbox/distrobox for the rest.

## Role removal

Every engine and instance role declares what it installs in `meta/up.yml`: packages per manager, files and directories, services, users and groups. After each converge, the set of applied roles is recorded in the role ledger in `/var/lib/up/state.json`. When a role disappears from a host's groups, `up` computes:

> removals = what the removed role declares − what every remaining role declares

and applies them with the role's `absent` tasks. Shared packages stay. `brew bundle cleanup` handles Homebrew state not covered by manifests.

## Speed

- `up.sh` verifies and runs one binary; the binary parses config natively.
- The root toolchain is built once per pin set and reused.
- Package tasks take whole lists; `gather_subset: [min, hardware]`; JSON fact caching (root-only).
- `brew bundle` once per run, with a role-merged Brewfile.
- Baked images already contain everything; first boot only does identity, keys, enrollment and secrets.
- No download caches are managed by `up`; Homebrew, apt, dnf, mise, uv and chezmoi keep their own.
- Shell: zsh everywhere; chezmoi decides prompt and plugins. Cache `brew shellenv` and `mise activate` output at apply time instead of running `eval` on every start; `zcompile`; daily `compinit` cache; lazy-load heavy completions.

## Telemetry and AI

`telemetry_off` (on for every host) writes `/etc/up/env.d/telemetry.sh` (root-owned) for system shells and `~/.config/up/telemetry.sh` for the user's shell. Root's scheduled runs do not source either file; root's environment is fixed (TRUST-02), and the variables are set explicitly in it.

```sh
DO_NOT_TRACK=1
HOMEBREW_NO_ANALYTICS=1
DOTNET_CLI_TELEMETRY_OPTOUT=1
NEXT_TELEMETRY_DISABLED=1
GATSBY_TELEMETRY_DISABLED=1
AZURE_CORE_COLLECT_TELEMETRY=0
SAM_CLI_TELEMETRY=0
```

`os_ai_off` (on by default) disables built-in OS AI features (Apple Intelligence, Siri suggestions; Linux desktop assistants). The setting keys are version-gated per OS release and checked in VM tests. The `ai` role is opt-in. AI assets in the engine repo (prompts, `AGENTS.md`, adapters) install nothing on hosts.

## Requirements

- **PERF-01** The `base` role MUST install no more than 20 packages.
  - AC: A CI check counts the merged `base` package list.
- **PERF-02** Every role MUST declare its installed state in `meta/up.yml`; when a role is removed from a host, converge MUST remove what that role declares and no remaining role declares.
  - AC: Molecule per role: apply, remove, converge, assert absent; (negative) a package shared with a remaining role is not removed.
- **PERF-03** A converge on an already-baselined host with no changes MUST finish in under 60 seconds.
  - AC: CI times a second converge in a container and fails above 60 s.
- **PERF-04** `zsh -i -c exit` MUST average under 50 ms with the default dotfiles.
  - AC: hyperfine in CI (Linux container and macOS runner); `up doctor` reports the number.
- **PERF-05** Telemetry opt-outs MUST be applied for user shells and set in root's fixed environment; root MUST NOT source user-writable files.
  - AC: An interactive shell has the variables set; (negative) editing `~/.config/up/telemetry.sh` has no effect on a root run.
- **PERF-06** `os_ai_off` MUST be on by default and version-gated per OS release.
  - AC: (manual) A VM test per supported macOS release verifies each listed setting.
- **PERF-07** AI tooling MUST be installed only through the `ai` role.
  - AC: (negative) A converge of every non-`ai` role fixture installs no package on a maintained AI-tool denylist.
