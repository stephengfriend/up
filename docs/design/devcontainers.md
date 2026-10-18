---
title: Dev containers
status: draft
prefix: DEVC
milestones: [M6]
depends_on: [architecture, secrets, supply-chain]
---

# Dev containers

Prebuilt, multi-arch images carry the same dotfiles and CLI tools as your hosts, so opening a project takes seconds. The engine holds the Dockerfiles and the Feature; each instance builds personalized images and publishes them under its owner's registry. Containers get the minimum credential surface: a forwarded agent that asks before each use, and nothing on disk.

## Image family

| Image | Built from | Adds |
| --- | --- | --- |
| `<name>-base` | Debian or Ubuntu slim, pinned by digest | zsh, git, curl, chezmoi-applied dotfiles (container profile), Homebrew (official taps), mise (aqua backend), core CLI set, non-root `dev` user |
| `<name>-go`, `-node`, `-python`, `-rust` | `<name>-base` | The toolchain via mise, linters |
| `<name>-polyglot` | `<name>-base` | Several of the above |

Homebrew stays in the base image by decision D-16; if image size hurts pull times, single-binary tools move to mise's aqua backend first.

`up container build` builds the family for `linux/arm64` and `linux/amd64` with buildx. `up container push` publishes to `ghcr.io/<login>/` as private packages and signs each image digest. Engine CI builds the same images from `template/` only, as a test. A Dev Container Feature (`ghcr.io/<login>/features/<name>-dotfiles`) applies the container profile to any upstream image.

## Stack detection

`up container init` writes `.devcontainer/devcontainer.json` with the image **pinned by digest** and `updateRemoteUserUID: true`, so the `dev` user takes your host UID for bind mounts:

| Signal | Image |
| --- | --- |
| `go.mod` | `<name>-go` |
| `package.json` | `<name>-node` |
| `pyproject.toml` or `requirements.txt` | `<name>-python` |
| `Cargo.toml` | `<name>-rust` |
| More than one | `<name>-polyglot` |
| None | `<name>-base` |

A repo's `.mise.toml` is **not** auto-trusted: mise's trust prompt stays on, because a cloned repo's tool definitions are code.

## Credentials

- **SSH agent:** forwarded, with confirm-on-use. 1Password and Bitwarden agents prompt per use; with OpenSSH, keys are added with `ssh-add -c` or are `verify-required` hardware keys. A separate, lower-privilege key for containers is recommended.
- **No `~/.ssh` mounts**, read-only or otherwise.
- **Git signing:** through the same agent.
- **Secrets:** env files or mounts from the host at run time; never `ARG` or `ENV` in a Dockerfile.
- chezmoi applies a no-sudo container profile: no launchd/systemd units, no GUI configs, no PM CLIs.

## Requirements

- **DEVC-01** `up container build` MUST build the image family for `linux/arm64` and `linux/amd64` from engine Dockerfiles, instance dotfiles, and digest-pinned bases.
  - AC: Engine CI builds all images from `template/` for both architectures; (negative) an unpinned `FROM` fails lint.
- **DEVC-02** Images MUST NOT contain secrets.
  - AC: (negative) A secret scanner over every layer fails the build on a planted credential.
- **DEVC-03** Images MUST run as a non-root `dev` user, and generated `devcontainer.json` files MUST set `updateRemoteUserUID: true`.
  - AC: A container started with a host UID of 1001 can write a bind-mounted directory.
  - AC: (negative) CI fails if any image's default user is root.
- **DEVC-04** The Feature MUST apply the container profile to an upstream Debian-based image.
  - AC: A test devcontainer using `mcr.microsoft.com/devcontainers/base:debian` plus the Feature has the dotfiles.
- **DEVC-05** `up container init` MUST choose an image per the detection table, pin it by digest, and never overwrite an existing file without `--force`.
  - AC: Fixture repos for each row produce the expected digest-pinned reference.
- **DEVC-06** Containers MUST receive SSH access only through a forwarded agent configured for confirm-on-use, and MUST NOT mount `~/.ssh`.
  - AC: (negative) The generated `devcontainer.json` contains no `.ssh` mount; (manual) a signing request inside the container triggers a confirmation prompt.
- **DEVC-07** Published images MUST be signed, and mise MUST keep its trust prompt for repo `.mise.toml` files.
  - AC: `up container push` produces a verifiable signature for each digest; (negative) a fresh repo's `.mise.toml` is not executed before the user trusts it.
