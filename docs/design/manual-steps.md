---
title: Manual steps
status: draft
prefix: none
milestones: [M4]
depends_on: [public-repo, trust, secrets]
---

# Manual steps

These are the steps that can't be automated without MDM or physical access, and the one-time trust setup that must be done by a human with hardware keys. They are the input for the guides in `docs/guides/`.

## One-time: keys and repos

- [ ] Get two hardware security keys. Create an `ed25519-sk` signing key on each (verify-required). The primary is for daily use; the backup is stored offline.
- [ ] If you forked the engine: set `UP_REPO` and `UP_SIGNERS` in `up.sh` to your fork and your two keys; build and sign a release with `up release`.
- [ ] Engine repo: rulesets for `main` (signed commits, required CI, no force push) and for `v*` tags (only you can create them, no updates or deletions); secret scanning with push protection.
- [ ] Create your instance: `up.sh init` (it applies the instance rulesets and push protection for you), an agent with `prompts/init-instance.md`, or the manual guide.
- [ ] Create a GitHub App with `contents:read` on the instance repo only; `up image render` uses it to mint one-hour bootstrap tokens.
- [ ] Create class vaults in 1Password or projects in Bitwarden Secrets Manager (`vault.classes`), with one machine token per class.
- [ ] Generate the admin age key on a hardware key (or store it in the vault) and list it in `.up/recipients`.
- [ ] Store your FileVault recovery keys in the vault.
- [ ] Set up a heartbeat receiver (dead-man's switch) and a webhook, and store both URLs in the catalog.

Optional:

- [ ] Add the `control_node` role to a host, ideally a dedicated small VM or Pi rather than your daily driver, and create a hardware-backed push key.
- [ ] Set up an isolated provisioning VLAN and netboot server, only if you use PXE.
- [ ] Run a LAN mirror and set `mirror` in `up.yml`.

## Per machine

| Platform | Before | After |
| --- | --- | --- |
| macOS | Finish Setup Assistant (admin user, FileVault on); set the computer name; sign in to the App Store; sign in to `gh` or your PM's SSH agent | Confirm the instance signer fingerprints when asked; approve the signed `.mobileconfig` |
| Linux desktop | Install the OS with your user and hostname; sign in to `gh` or a PM SSH agent, or have a token | Confirm the signer fingerprints when asked |
| Linux server / Pi (stock) | `up image render <host>` and `up image flash <host> <device>` on your workstation; power on | `up secrets enroll <host>` once its fingerprint appears |
| Baked image | `up image flash <host> <device>` (add `--pregen` only if it must decrypt secrets offline at first boot) | `up secrets enroll <host>`, unless pre-generated |
| Netboot | Add the MAC and alias to inventory; `up image netboot sync`; power on | `up secrets enroll <host>` |
| Fedora Atomic / Bazzite | Install, or netboot with the rebase kickstart | Reboot into your image once; `up secrets enroll <host>` |
| Dev container | Nothing | Nothing |

## Lifecycle

- **Retiring or losing a host:** `up host retire <host>` and follow its output.
- **Compromised workstation or signing key:** follow the runbooks in `docs/guides/runbooks.md`; the backup key is what lets you recover without touching every host.
