---
title: Secrets
status: draft
prefix: SEC
milestones: [M3, M5]
depends_on: [architecture, trust, cli]
---

# Secrets

Secrets are referenced by logical name and resolved at run time by the backend a host is configured for. Neither repo holds a plaintext secret, no image holds a long-lived one, and each host can read only what its class and its own identity allow. Host keys are generated on the device and never travel.

## Backends

| Backend | Resolves in | Credential | Best for |
| --- | --- | --- | --- |
| `sops` (age) | Root | The host's own age identity, `/etc/up/age.key` | Servers, Pis, offline hosts (default for headless) |
| `op` service account / `bws` machine token | Root | A token scoped to the host's **class** vault or project (`vault.classes` in `up.yml`) | Headless hosts that need vault-managed secrets |
| `op` / `bw` desktop session | The user layer, interactive runs only | The user's unlocked app (biometric) | Workstations |
| `push` | The control node | None on the target | Hosts that should hold no secrets credential (M7) |
| `fake` | Root | None | CI only (SEC-13) |

Machine credentials stay with root. The user layer never sees a machine token: root resolves what chezmoi needs and writes it to `/run/up/<uid>/data.json` (directory `0700`, owned by the user, on tmpfs, removed when the run ends). Human-session backends resolve in the user layer only when a person is present; unattended runs skip secret-bearing templates and record the skip (exit `3`).

## Catalog

```yaml
# secrets/catalog.yml (instance)
smtp_password:
  op:   "op://{{ vault }}/smtp/password"
  bw:   "<secret-uuid>"
  sops: "secrets/groups/servers.sops.yaml#smtp_password"
notify_webhook:
  sops: "secrets/groups/all.sops.yaml#notify_webhook"
```

`{{ vault }}` is the host's class vault. The engine's `up_secret` lookup reads the catalog and calls the backend; the engine's chezmoi `secret` partial reads from the root-rendered data file.

## Host keys and enrollment

Each host has two separate keys, both generated **on the device**:

- an age identity, `/etc/up/age.key` (root, `0400`), used only for sops;
- a read-only deploy key, `/etc/up/deploy_key` (root, `0400`), used only to fetch the instance.

SSH host keys are left to the OS. They are recorded in the control node's `known_hosts` when needed, never pre-generated.

**Enrollment** makes a new host able to read its secrets and fetch updates:

1. On first boot or bootstrap, the host generates both keys and prints their public parts and its SSH host key fingerprint to the console and journal. `up host enroll-info` shows them again.
2. The operator runs `up secrets enroll <host>` in the instance working clone. It fetches the public parts over SSH (checking the host key against the printed fingerprint), and asks for TTY confirmation. Then it:
    - adds the age recipient to `.up/recipients` and `.sops.yaml`;
    - registers the deploy key read-only on the instance repo through the GitHub API;
    - re-encrypts the files that host may read;
    - commits, signed.
3. Until its recipient appears in a verified commit, the host converges everything that needs no secrets and reports "awaiting enrollment" (exit `3`).

Interactive bootstrap enrolls the machine it runs on in one step.

**Pre-generation**, only for baked images that must decrypt secrets offline on first boot: `up image flash --pregen <host>` generates the keys and streams the private parts straight onto the target device, never to the workstation's disk. It registers the public parts as in enrollment. First boot scrubs every copy outside `/etc/up` (BOOT-12).

**Never in user-data on cloud VMs.** On platforms with a metadata service, no private key or long-lived token goes into user-data. Those hosts use enrollment, with metadata access restricted (IMDSv2, hop limit 1, or equivalent).

## Bootstrap and fetch credentials

- **Before enrollment:** a GitHub App installation token with `contents:read` on the instance repo only, minted by `up image render` and valid for one hour. A fine-grained PAT with the shortest expiry GitHub allows is the fallback. The Packer build credential follows the same rule.
- **After enrollment:** the host's own deploy key. One stolen host means revoking one key.

## Non-disclosure

Secret values never appear in:

- stdout or `--diff` output (tasks that render a secret set `no_log: true` and `diff: false`, enforced by lint);
- facts or the fact cache (no secret in `set_fact`; the cache is root-only `0700`);
- retry files (disabled in the engine's `ansible.cfg`) or Ansible temp files (kept under `/run/up`);
- logs, the audit record or the webhook payload (OPS-03).

Decrypted material exists only under `/run/up`, a tmpfs, and is removed by an exit trap.

## Lifecycle

`up host retire <host>` decommissions a host. It removes the host's recipient and re-encrypts, then **rotates every secret the host could read**: `updatekeys` does not protect ciphertext already in git history. It revokes the host's deploy key and any class token the host held (or tells you to, where no API exists), removes the host from inventory and the netboot server, commits signed, and prints wipe instructions.

The guides include tested runbooks:

| Event | Response |
| --- | --- |
| Host lost or stolen | `up host retire`; rotate class tokens the host held; notify |
| Workstation compromised | Retire and rotate for every host whose keys it ever handled; rotate the admin age key; rotate engine and instance signing keys with the backup signer |
| Signing key compromised | Sign the signer-set change with the offline backup key, which every host already trusts; re-sign forward |

Machine tokens and the admin age key rotate on a cadence set in `up.yml` (default 90 days); `up doctor` warns when one is due.

## SSH and signing

| Context | SSH auth | Git commit signing |
| --- | --- | --- |
| Workstation | PM SSH agent (1Password or Bitwarden); `IdentityAgent` set by chezmoi | SSH signing via the agent with a hardware-backed or PM-held key |
| Server / Pi | No personal keys on disk; push reaches it through the `up-push` account (CHG-06) | None |
| Dev container | Forwarded agent with confirm-on-use (DEVC-06); never a mounted `~/.ssh` | Same agent |
| Hardware key | `ed25519-sk` resident keys; `ssh-keygen -K` on new hosts | Signing for engine releases and instance commits |

## Requirements

- **SEC-01** Roles and templates MUST reference secrets only by logical catalog name.
  - AC: (negative) An ansible-lint rule rejects `op read`, `bws`, or `sops -d` outside the lookup plugin.
- **SEC-02** The backends MUST sit behind one interface, resolving machine credentials in root and human sessions in the user layer.
  - AC: The same fixture passes against `fake` and `sops` (CI) and `op` and `bw` (manual).
  - AC: (negative) Configuring a human-session backend for root resolution is refused at schema validation.
- **SEC-03** Secret values MUST NOT appear in stdout, `--diff`, the journal, `/var/log/up`, the fact cache, retry or temp files, the audit record, or the webhook payload.
  - AC: (negative) A `--check --diff` run and a deliberately failing secret task with the `fake` backend are grepped across all of these, and no fake value is found.
- **SEC-04** Machine tokens MUST be scoped to a class vault or project; host-unique material MUST live only in sops files encrypted to that host.
  - AC: (negative) A class token cannot read another class's vault in a manual test; a CI check rejects host-unique catalog entries that use `op` or `bw`.
- **SEC-05** Each host MUST be able to decrypt only the files it is listed for.
  - AC: (negative) A test host decrypts its own file and fails on another host's.
- **SEC-06** Bootstrap credentials MUST be short-lived and read-only on one repo, and MUST be removed on exit as specified in BOOT-12.
  - AC: (negative) A rendered token older than its expiry is rejected by GitHub in the integration test.
- **SEC-07** chezmoi MUST configure SSH auth and commit signing per the SSH and signing table, based on host roles.
  - AC: Rendered `~/.ssh/config` and `~/.gitconfig` match golden files for a workstation and a server fixture.
  - AC: (negative) The server fixture's rendered config references no personal private key.
- **SEC-08** Secret values MUST live only in a backend or in sops-encrypted files; the template's `.sops.yaml` MUST cover `secrets/`, `*.sops.*` and `inventory/**/*.sops.yml`.
  - AC: `sops -e` on a new file in each covered path succeeds without extra flags.
  - AC: (negative) A plaintext file in a covered path is blocked by guard (GRD-04).
- **SEC-09** Each host's age identity MUST be separate from its SSH host keys and stored at `/etc/up/age.key` with mode `0400`.
  - AC: (negative) Regenerating SSH host keys leaves secret decryption working.
- **SEC-10** Host keys MUST be generated on the device and enrolled with `up secrets enroll`; pre-generation MUST stream private keys to the target without writing them to the workstation's disk; no private key or long-lived token may be placed in user-data on platforms with a metadata service.
  - AC: An enrollment test brings a fresh VM from "awaiting enrollment" (exit `3`) to fully converged; (negative) a filesystem trace of `--pregen` shows no private key written on the workstation.
- **SEC-11** Each host MUST have its own read-only deploy key; the bootstrap credential MUST be a one-hour GitHub App installation token, or the shortest-lived fine-grained PAT, with `contents:read` on the instance only.
  - AC: (negative) Revoking one host's deploy key stops only that host's fetches.
- **SEC-12** The user layer MUST receive resolved machine-backend secrets only through `/run/up/<uid>/` and MUST NOT hold a machine token; unattended runs MUST skip templates that need a human session and record the skip.
  - AC: (negative) No file readable by the user contains a machine token after a converge; an unattended workstation run with an `op` desktop template exits `3` with the reason.
- **SEC-13** The `fake` backend MUST be refused unless `UP_ENV=ci` is set and `/etc/up/host.yml` does not exist.
  - AC: (negative) A host with `fake` configured exits `1`; a lint rule rejects `fake` in inventory.
- **SEC-14** Decrypted material MUST exist only under `/run/up` and be removed by an exit trap.
  - AC: (negative) After a killed run, the next run finds and removes leftovers and logs it; no decrypted file exists outside `/run/up` at any time in a traced run.
- **SEC-15** `up host retire` MUST remove the recipient, rotate every secret the host could read, revoke its deploy key and class tokens, and remove it from inventory and netboot; the engine and instance signer sets MUST include an offline backup key; runbooks for a lost host, a compromised workstation and a compromised signing key MUST be documented and tested.
  - AC: A retire test leaves no recipient, deploy key or inventory entry for the host, and every affected secret has a new value.
  - AC: A signer rotation signed only by the backup key is accepted by a host.
  - AC: (negative) After retire, the retired host's deploy key can no longer fetch the instance.
