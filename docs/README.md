---
title: Documentation index
status: draft
---

# up documentation

`up` is a reusable bootstrap and change-management engine for macOS and Linux machines, dev containers and system images. This public repo is the **engine**. Each user keeps a separate, private **instance** repo with their own hosts, secrets and dotfiles. On every host, root runs verified, root-owned copies of both.

This folder is the specification. It is written so a person or a coding agent can turn it into milestones, tasks and tests without further interviews.

## Reading order

1. [design/overview.md](design/overview.md): what the system is, goals, principles
2. [design/threat-model.md](design/threat-model.md): assets, actors, trust boundaries, accepted risks
3. [glossary.md](glossary.md)
4. [design/architecture.md](design/architecture.md) and [design/trust.md](design/trust.md)
5. The component specs, in any order (table below)
6. [roadmap.md](roadmap.md), [decisions.md](decisions.md), [open-questions.md](open-questions.md)
7. [reviews/](reviews/): external reviews and how each finding was resolved

## Spec map

| Doc | Prefix | Covers | Milestones |
| --- | --- | --- | --- |
| [design/threat-model.md](design/threat-model.md) | none | Assets, actors, invariant, accepted risks | M0 |
| [design/architecture.md](design/architecture.md) | `ARCH` | Engine and instance, host layout, identity, naming, schemas | M0–M2 |
| [design/trust.md](design/trust.md) | `TRUST` | Root-owned trees, hermetic root, signatures, anti-rollback, protected operations | M1, M3, M5 |
| [design/supply-chain.md](design/supply-chain.md) | `SUPPLY` | Pinned hashes, releases, CI hardening, packages, base images | M0–M5 |
| [design/bootstrap.md](design/bootstrap.md) | `BOOT` | `up.sh`, discovery, anchoring, install, scrub, identity | M1 |
| [design/cli.md](design/cli.md) | `CLI` | The `up` binary and its commands | M1, M2, M4 |
| [design/guard.md](design/guard.md) | `GRD` | Client-side leak checks and their limits | M1, M3, M8 |
| [design/secrets.md](design/secrets.md) | `SEC` | Backends, key split, enrollment, scopes, non-disclosure, lifecycle | M3, M5 |
| [design/hardening.md](design/hardening.md) | `HARD` | sudo, sshd, firewall, updates, encryption, macOS integrity, audit, docker | M2 |
| [design/operations.md](design/operations.md) | `OPS` | Lock, state and audit, notifications, heartbeat, staleness, reboots | M1, M2 |
| [design/provisioning.md](design/provisioning.md) | `IMG` | Phases, baking, sanitization, signing, first boot, netboot, mirror | M5, M7, M8 |
| [design/change-management.md](design/change-management.md) | `CHG` | Versioning, signed merges, push, offline, testing, rollback | M0–M8 |
| [design/devcontainers.md](design/devcontainers.md) | `DEVC` | Dev container images and credentials | M6 |
| [design/lean-and-fast.md](design/lean-and-fast.md) | `PERF` | Package policy, role removal, speed budgets, telemetry, AI | M2 |
| [design/public-repo.md](design/public-repo.md) | `PUB` | Repo contents, template, wizard, AI assets, spec checker | M0, M4 |
| [design/manual-steps.md](design/manual-steps.md) | none | Human steps; input to the guides | M4 |

## Conventions

**Requirement IDs.** `<PREFIX>-<NN>`, for example `BOOT-03`. While a doc's status is `draft`, IDs may be renumbered. Once a doc is `accepted`, IDs are frozen: never reused or renumbered, and a removed requirement keeps its ID with the text `Withdrawn`.

**Keywords.** MUST, MUST NOT, SHOULD and MAY as in RFC 2119. MUST is required for the milestone that owns it.

**Acceptance criteria.** Each requirement lists one or more `AC:` lines, each a single checkable statement, ideally an automated test. `(manual)` marks one verified by hand and recorded in the PR. `(negative)` marks one that proves a malicious or invalid input is refused. Every requirement whose text involves root, the network or a credential has at least one negative AC.

**Front matter.** Every design doc starts with YAML front matter: `title`, `status`, `prefix`, `milestones`, `depends_on`.

**Placeholders.** `<you>` is the engine owner on GitHub; `<login>` is the GitHub login discovered on a host; `<name>` is the instance's project name (default `fleet`); `<host>`, `<class>`, `<device>` as they read. Example addresses use documentation ranges.

**Paths.** Relative to the repo the sentence is about, prefixed `engine:` or `instance:` where ambiguous. Host paths are absolute.

## Turning the spec into work

- One **milestone** becomes one milestone or epic.
- One **requirement** becomes one issue (or a few), titled with its ID: `TRUST-06: verify engine tag and pinned commit`.
- Each **AC** becomes a checklist item and, wherever possible, a test whose name contains the ID.
- Commits and PRs reference IDs. **Spec first:** a behavior change updates the design doc in the same PR.
