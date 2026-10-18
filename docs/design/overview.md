---
title: Overview
status: draft
prefix: none
milestones: [M0]
depends_on: []
---

# Overview

A new machine reaches its baseline with one command:

```sh
curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash
```

`up.sh` downloads the signed `up` binary, verifies it, and runs it. Finding no instance on the machine, `up` works out who you are from your GitHub credentials, finds your instance repo, anchors trust in its signers, installs itself root-owned under `/opt/up`, and converges. After that, `up` does everything: update, converge, dotfiles, secrets, images, containers.

The public `up` repo is the **engine**: the source of the `up` binary (Rust), generic Ansible roles, guard tooling, image and container build definitions, the threat model and spec, a template for new instances, and AI prompts. Each user keeps a separate, private **instance** repo with what is theirs: `up.yml`, inventory, secrets, dotfiles, any instance-only roles, and the list of keys allowed to sign changes. Hosts apply only engine releases and instance commits signed by those keys, and only from root-owned, verified copies. Anyone can fork the engine and create an instance by following the docs or running `up init`, with or without an AI agent.

## Goals

- One command to baseline any macOS or Linux host, ARM64 or x86_64; one command (`up`) for everything after.
- Every host manages itself (Ansible local, pull). A central control node is optional.
- Root runs only root-owned code that was verified against keys you hold (see [threat-model.md](threat-model.md)).
- Lean by default: a host gets only what its roles ask for, and loses what a removed role installed. No AI features on hosts unless the `ai` role is added. Telemetry off.
- A security baseline on every host: sudo policy, sshd, firewall, automatic security updates, encryption checks.
- Developer speed first: fast bootstrap, fast shell, prebuilt dev containers.
- Baked images are complete; offline runs converge what they can, and stale hosts raise an alert.
- Reusable by others, and buildable by a coding agent from the spec.

## Non-goals

- Windows and WSL.
- Kubernetes, Talos and Flatcar in the engine. Clusters are built with instance roles like any other software.
- In-cluster applications, backup and restore of user data.
- MDM: no remote wipe, enforced OS updates or FileVault escrow on macOS.
- A required control node or required LAN infrastructure.
- Managing download caches.
- AI as a runtime dependency.

## Principles

1. **One code path.** Every provisioning path ends in `up`; baking is `up` too.
2. **Root runs only what it verified.** Root-owned trees, signed releases and commits, no downgrades.
3. **Engine generic, instance personal.** Nothing personal in the engine; nothing reusable only in an instance.
4. **Instance write access is root on every host,** so instance changes must be signed by a human.
5. **Deterministic logic lives in the CLI.** Wizards, agents and docs all drive the same `up` commands.
6. **Spec first, with negative tests.** Behavior changes update the spec in the same PR; every trust boundary has a test that a malicious input is refused.
7. **Online-preferred, offline-tolerant, never silently stale.**
8. **Measured, not hoped.** Speed and leanness targets are tested in CI.
