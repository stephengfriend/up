---
title: Decisions
status: accepted
---

# Decisions

Each decision has a stable ID. Superseded decisions stay listed with a pointer to their replacement.

| ID | Decision | Why |
| --- | --- | --- |
| D-01 | Public engine repo `up` + a separate instance repo per user, private by default | Reusable code is shareable; personal data stays with its owner |
| D-02 | ~~Instance includes the engine as a submodule~~ Superseded by D-25 | |
| D-03 | ~~Instance overrides engine wholesale~~ Superseded by D-35 | |
| D-04 | ~~`up.sh` clones the instance with credentials~~ Superseded by D-27 | |
| D-05 | ~~Bash dispatcher with git-style subcommands~~ Superseded by D-32 | |
| D-06 | Instance settings live in `up.yml`; nothing in engine code hardcodes the project name | Rename (`fleet` → `flotilla`) is one line |
| D-07 | Ansible owns the system; chezmoi owns `$HOME`; root hands chezmoi its data | One source of truth; chezmoi still works alone in containers |
| D-08 | Ansible runs locally on each host by default; push only from an optional `control_node` role | No central machine required |
| D-09 | ~~uv-installed ansible-core in the user's home~~ Superseded by D-33 | |
| D-10 | Pluggable secrets backends: 1Password, Bitwarden, SOPS/age, push, fake | Different hosts need different trust models; CI needs none |
| D-11 | PM SSH agent on workstations; no personal keys on servers; confirm-on-use agents in containers; FIDO2 supported | Keys off disk where a human is present |
| D-12 | ~~Repo access chain in `up.sh`~~ Superseded by D-28 | |
| D-13 | ~~Local facts (including roles) override inventory~~ Superseded by D-36 | |
| D-14 | Provisioning paths (stock, baked, netboot) all end in `up` | Flexibility without a second bootstrap |
| D-15 | ~~Baked local caches managed by `up`~~ Superseded by D-30 | |
| D-16 | Native packages for system, Homebrew for user CLI (including dev containers), mise for runtimes | Same CLI tools everywhere; per-project runtime pins |
| D-17 | Prebuilt personalized dev container images per stack, plus a Feature | Seconds to a working container |
| D-18 | zsh everywhere; prompt and plugins chosen in chezmoi under a 50 ms budget | Speed is measured, not assumed |
| D-19 | OS AI off and telemetry off by default; AI tooling only via the opt-in `ai` role | Lean hosts |
| D-20 | ~~Kubernetes install in engine roles~~ Superseded by D-38 | |
| D-21 | ~~"GitHub trusted" as the trust model~~ Superseded by D-34 | |
| D-22 | Spec with requirement IDs, ACs (including negative ACs) and milestones; IDs frozen once a doc is accepted | Buildable by people and coding agents |
| D-23 | `up init` wizard plus agent prompts that drive the same CLI | Deterministic logic in one place |
| D-24 | Agent-agnostic `AGENTS.md` and `prompts/`, with generated Claude Code adapters | Works with any agent; no drift |
| D-25 | Engine and instance are separate repos; the instance pins the engine release | No submodule friction |
| D-26 | Guard: client-side hooks and CI in both repos; GitHub push protection is the boundary | Leaks are caught early; the server enforces |
| D-27 | `up.sh` only fetches, verifies and runs the signed binary; no credentials | The public script stays small and trustless |
| D-28 | Discovery from existing credentials; owner must equal login; topic is a hint; unattended never auto-selects | Works on new machines without trusting strangers' repos |
| D-29 | Bootstrap is implicit when `/etc/up/host.yml` is absent | Nothing to remember on a new machine |
| D-30 | No `up`-managed caches; baking is `up` in the image build; offline mode skips only `network` tasks | Tools already cache; images are complete by construction |
| D-31 | ~~Unattended runs start as root from the user's checkout~~ Superseded by D-33 | |
| D-32 | The `up` CLI is a single compiled Rust binary with signed releases; instance commands add new names only | Native config parsing and signature checks, root-ownable, no runtime PyPI, safe for macOS TCC |
| D-33 | Root runs only root-owned, verified, immutable trees under `/opt/up`, hermetically, and drops privileges for the user layer | Closes user-to-root escalation |
| D-34 | Trust model: engine releases and tags signed with hardware keys and pinned by commit; instance commits on `main` signed and verified on hosts; anti-rollback; backup signer | GitHub provides availability, not integrity |
| D-35 | Instances may add roles and commands; protected roles and all built-ins cannot be shadowed; other shadowing is reported | Extension without disabling controls |
| D-36 | `host.yml` holds identity only; roles always come from inventory | Git stays the source of truth |
| D-37 | Hosts follow signed commits on `main`, with no release tags and no canary ring; risk accepted and documented | Simplicity for a single maintainer; signing still gates every change |
| D-38 | Kubernetes, Talos and Flatcar are out of the engine's scope | Clusters are ordinary instance roles |
| D-39 | Host keys generated on the device and enrolled; age identity separate from SSH host keys; per-host deploy keys | Smallest custody and blast radius |
| D-40 | Automatic security updates on every class; feature and OS upgrades only in maintenance windows (Q-11) | Patch fast, change carefully |
| D-41 | Instances start private; public instances are an M8 feature (Q-12) | Prove guard and push protection first |
