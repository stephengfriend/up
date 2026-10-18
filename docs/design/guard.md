---
title: Guard
status: draft
prefix: GRD
milestones: [M1, M3, M8]
depends_on: [architecture, cli, trust]
---

# Guard

Guard is a seatbelt, not a boundary. Client-side hooks catch mistakes before they leave a working clone, but anyone can skip a hook. The boundary is server-side: GitHub secret scanning with **push protection** on both repos, plus signed-commit verification on hosts (TRUST-07). Guard exists to make the safe path the default and to keep leaks out of history in the first place.

## Policy

| Check | Engine | Private instance | Public instance (M8) |
| --- | --- | --- | --- |
| Credentials: tokens, API keys, passwords, private keys, `.env` files, kubeconfigs, cloud credentials | Block | Block | Block |
| Encryption: every file under `secrets/` and every `*.sops.*` file is sops-encrypted | n/a | Block | Block |
| Personal data, block tier: denylist entries, inventory hostnames and addresses, MACs, emails, publicly routable IPs | Block | Allow | Block, unless inside a sops-encrypted file |
| Personal data, warn tier: RFC 1918 addresses, ambiguous dotted quads (version strings) | Warn | Allow | Warn |
| Large binaries (over 1 MB) | Block | Warn | Warn |

Documentation addresses (`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`, `2001:db8::/32`), `example.com` names and the `<you>`, `<login>`, `<name>`, `<host>` placeholders are always allowed.

Instances start private (Q-12 resolved). The public-instance column is implemented and tested in M8, once guard and push protection have proven themselves.

## Where it runs

| Point | What is checked |
| --- | --- |
| `pre-commit` hook in working clones | Staged changes |
| `pre-push` hook in working clones | Every commit being pushed |
| CI in the engine and the instance | The pushed range; weekly full-history scan |
| GitHub push protection | Every push, server-side (manual step) |
| `up guard check` / `up guard scan --history` | On demand |
| `up doctor` | Hooks installed in each working clone |

Hooks live only in **working clones**, via `core.hooksPath` pointing at the hooks shipped with the engine's verified tree. Root never commits, so root never runs a hook.

Output is always redacted: findings name the file, line and rule, never the matched text (`gitleaks --redact` for credentials; the engine's own rules for the rest).

**Repo kind and visibility.** `.up-engine` marks the engine; `up.yml` marks an instance and states its visibility. If GitHub reports a repo public that `up.yml` calls private, guard applies public rules and warns. Changing `visibility` from private to public runs a full-history scan under public rules and blocks the change on any finding.

**Personal-data sources.** The denylist in `secrets/privacy.sops.yaml` (encrypted, decrypted in memory only); inventory hostnames, `ansible_host` values and MACs (sops vars decrypted in memory); the git author email; and `~/.config/up/guard.yml` for engine contributors with no instance.

**Exceptions.** `up guard allow <fingerprint> --reason "…"` needs an interactive TTY (TRUST-12). It adds a fingerprint (never the matched text) to `.upguard-allow`, which is covered by CODEOWNERS. CI fails when the number of exceptions grows in a PR unless that PR's commits are signed by the instance or engine signer set.

**Bypass.** Hooks fail closed. `UP_GUARD_SKIP="<reason>"` bypasses once and records the reason in a commit trailer; CI reports every trailer, and a host that applies a commit carrying one sends a notification (OPS-03).

### guard.yml fields

`~/.config/up/guard.yml` lets an engine working clone with no instance block personal strings (GRD-08).

| Field | Type | Required | Constraint | Source | Status |
| --- | --- | --- | --- | --- | --- |
| `schema_version` | integer | yes | const `1` | ARCH-05, ARCH-10 | spec |
| `denylist` | list | yes | unique single-line strings, 3 to 256 characters each, that guard blocks | GRD-08; Personal-data sources | proposed |

## Requirements

- **GRD-01** `up guard install` MUST install hooks only in user-owned working clones; bootstrap with `--dev`, `up init` and `up self dev` MUST run it.
  - AC: After `up init`, `core.hooksPath` in the new working clone points at the engine's hooks; (negative) no file under `/opt/up` or `/var/lib/up/git` has hooks configured.
- **GRD-02** Guard MUST identify the repo kind and apply the stricter of `up.yml` visibility and GitHub-reported visibility.
  - AC: Fixtures for engine, private instance, public instance and mismatched visibility select the expected policy.
- **GRD-03** Credential checks MUST block in every repo kind, and all guard output MUST be redacted.
  - AC: (negative) Fixture commits with a GitHub token, an AWS key, an OpenSSH private key and a `.env` file are each blocked, and no output contains the secret value.
- **GRD-04** Files under `secrets/` and any `*.sops.*` file MUST be sops-encrypted in an instance.
  - AC: (negative) A plaintext file under `secrets/` is blocked; the same file after `sops -e` passes.
- **GRD-05** Personal-data checks MUST block the block tier and warn on the warn tier in the engine; in a public instance, block-tier matches are allowed only inside sops-encrypted files.
  - AC: (negative) An inventory hostname in an engine commit is blocked; `10.0.0.5` in engine docs warns; `192.0.2.10` passes.
- **GRD-06** The checks MUST run in both hooks, in `up guard check`, in `up guard scan --history`, and in CI for the engine and the template; the manual steps MUST require GitHub push protection on both repos.
  - AC: One fixture finding is caught at each client-side point and in CI.
- **GRD-07** Exceptions MUST be recorded by fingerprint with a reason, MUST need an interactive TTY, and MUST NOT contain the matched text; CI MUST fail when the exception count rises without signer-set signatures.
  - AC: (negative) `up guard allow` with stdin not a TTY exits `1`; an unsigned PR adding an exception fails CI.
- **GRD-08** Guard MUST work in an engine working clone with no instance, using `~/.config/up/guard.yml`.
  - AC: In a container with only the engine cloned, a string from `guard.yml` is blocked.
- **GRD-09** Hooks MUST fail closed; `UP_GUARD_SKIP` MUST require a reason recorded in a commit trailer.
  - AC: (negative) With gitleaks missing, a commit is blocked; with `UP_GUARD_SKIP="test"`, it passes and carries the trailer.
- **GRD-10** `up doctor` MUST report missing or misconfigured hooks in each working clone listed in `~/.config/up/config.yml`.
  - AC: Unsetting `core.hooksPath` in the instance working clone makes `up doctor` exit `1` naming it.
- **GRD-11** The privacy denylist MUST be stored sops-encrypted and decrypted only in memory.
  - AC: (negative) A plaintext denylist in `up.yml` fails schema validation; no temp file holds denylist plaintext during a scan.
- **GRD-12** Changing an instance's visibility to public MUST run a full-history scan under public rules and block on findings.
  - AC: (negative) Flipping a fixture instance with a hostname in its history fails with the commit and file named.
