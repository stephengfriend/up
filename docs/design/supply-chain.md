---
title: Supply chain
status: draft
prefix: SUPPLY
milestones: [M0, M1, M2, M4, M5]
depends_on: [threat-model, trust]
---

# Supply chain

Everything a host runs comes from somewhere: the `up` binary, the engine tree, the Python toolchain, Homebrew and mise packages, base images, CI. This doc pins each source so that a compromised mirror, registry or CI runner cannot turn into root on a host.

## Pinned and verified artifacts

`versions.yml` records, for every artifact that bootstrap, converge or a build fetches, the version **and** a SHA-256 per target (or the signing identity to verify against):

```yaml
# versions.yml (excerpt)
schema_version: 1
tools:
  gitleaks:
    version: 8.30.1
    use: [ci, toolchain]
    assets:
      aarch64-apple-darwin:
        url: https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_darwin_arm64.tar.gz
        sha256: "<64 lowercase hex>"
        bin: gitleaks                           # path inside the archive
      x86_64-unknown-linux-musl:
        url: https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz
        sha256: "<64 lowercase hex>"
        bin: gitleaks
      # ... one entry per target; aarch64-unknown-linux-musl is the third
  ansible-core:
    version: 2.x.y
    use: [toolchain]
    requirements: toolchain/requirements.txt    # pip --require-hashes
  git:
    use: [toolchain]
    minimum: "<version>"
  openssh:
    use: [toolchain]
    minimum: "8.1"                              # ssh-keygen -Y verify
```

Asset keys are the `up` release targets (`aarch64-apple-darwin`, `aarch64-unknown-linux-musl`, `x86_64-unknown-linux-musl`) and name the host platform; the asset itself is whatever build runs there. Each asset has an `https://github.com/` `url`, a lowercase 64-hex `sha256` and the `bin` path inside the archive, and `use` lists the consumers (`ci`, `toolchain`, `build`). CI tooling, the root toolchain and instance overrides (CHG-04) share this one format, and a tool without a pin for the host target is refused rather than fetched unpinned.

The root toolchain (`/opt/up/toolchain/<hash>`) is built only from these artifacts. Security-relevant tools (sops, age, gitleaks, chezmoi, `op`, `bws`) come from checksummed downloads into the toolchain, not from rolling Homebrew.

## Releases

1. CI builds the `up` binary for each target (`aarch64` and `x86_64`, macOS and Linux musl) in a protected environment and publishes build attestations.
2. The maintainer runs `up release` locally: it downloads the artifacts, verifies their attestations, writes `SHA256SUMS`, signs it and the tag with the hardware-backed release key, and uploads.
3. CI never holds a signing key, so a CI compromise cannot produce a release hosts accept.

## CI hardening

The public engine's GitHub Actions are part of the attack surface.

- Third-party actions pinned by commit SHA.
- `permissions:` read-only by default; write scopes granted per job.
- No `pull_request_target` workflows that check out fork code; no secrets exposed to fork PRs.
- Release jobs run in a protected environment with required reviewers.
- No self-hosted runners for the engine, ever. VM tests run on GitHub-hosted runners or on demand with `up test vm`.

## Packages and images

- **Homebrew:** official taps only by default; any other tap must be on an allowlist in `up.yml`.
- **mise:** the aqua backend by default; asdf-style plugins (arbitrary shell from git) are disallowed unless allowlisted.
- **Python:** ansible-core and its dependencies are installed with `--require-hashes`; vendored Ansible collection tarballs have recorded hashes.
- **Base images:** Packer and pi-gen inputs are verified against vendor checksums and signatures (for example Ubuntu `SHA256SUMS.gpg`, the Raspberry Pi image SHA-256); container base images are pinned by digest.
- **Renovate and CI** may open PRs that bump pins. They never merge and never sign; a host-reaching change needs a human-signed commit (TRUST-07, CHG-11).

## Requirements

- **SUPPLY-01** Every artifact fetched by `up.sh`, bootstrap, converge or a build MUST be verified against a pinned SHA-256 or a signature before use.
  - AC: (negative) Corrupting one byte of any fetched artifact makes the run exit `1` and name the artifact.
- **SUPPLY-02** Engine CI MUST pin third-party actions by SHA, default `GITHUB_TOKEN` to read-only, avoid `pull_request_target` with fork checkouts, withhold secrets from fork PRs, run releases in a protected environment, and use no self-hosted runners.
  - AC: (negative) A workflow linter in CI fails on an unpinned action, a write-default token, a `pull_request_target` checkout of the PR head, or a `self-hosted` runner label.
- **SUPPLY-03** Release signing MUST happen outside CI with the hardware-backed release key, after verifying the CI build attestations.
  - AC: (negative) `up release` refuses artifacts whose attestation does not match the tagged commit.
- **SUPPLY-04** Base OS images and container base images MUST be verified against vendor checksums or signatures and pinned by digest.
  - AC: (negative) A base image with a wrong checksum fails `up image build`; a Dockerfile `FROM` without a digest fails lint.
- **SUPPLY-05** Homebrew taps and mise backends MUST be restricted to the defaults plus an explicit allowlist in `up.yml`.
  - AC: (negative) A Brewfile line using an unlisted tap fails converge with exit `1`.
- **SUPPLY-06** Automated dependency PRs MUST NOT be merged or signed automatically.
  - AC: The Renovate config in the engine and template has automerge disabled.
  - AC: (negative) A PR that enables automerge fails the CI check.
- **SUPPLY-07** The root toolchain MUST be built only from pinned artifacts: Python packages with `--require-hashes`, collections with recorded hashes, tools from checksummed downloads.
  - AC: (negative) A requirements file entry without a hash fails the toolchain build.
- **SUPPLY-08** `up.sh` and `up doctor` MUST enforce the minimum git and OpenSSH versions in `versions.yml`; root clones MUST use `--no-recurse-submodules` and `GIT_ALLOW_PROTOCOL=https:ssh`.
  - AC: (negative) A git older than the minimum makes `up.sh` exit with a message naming the minimum; a `file://` remote is refused.
