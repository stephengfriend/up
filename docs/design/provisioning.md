---
title: Imaging and provisioning
status: draft
prefix: IMG
milestones: [M5, M7, M8]
depends_on: [architecture, trust, supply-chain, secrets, bootstrap]
---

# Imaging and provisioning

All provisioning paths (stock OS with cloud-init or an installer, baked image, netboot) are rendered from the instance inventory and end in `up`. Baking is `up` run inside the image build, so an image leaves the factory with everything installed and each tool's own cache and state in place; `up` manages no caches. Templates live in `engine:images/`; an instance MAY override them per class.

Kubernetes, Talos and Flatcar are out of scope for the engine. Clusters are built with instance roles like any other software.

## Task phases

| Phase tag | Runs when | Allowed | Forbidden |
| --- | --- | --- | --- |
| `bake` | Image build and every run | Packages, Homebrew, mise tools, non-secret chezmoi files, static config | Hostname, secrets, host keys, machine IDs, enrollment |
| `provision` | First boot and every run | Host identity, key generation, enrollment, secrets, scheduler | Nothing extra |
| `converge` | Every run | Drift correction, updates | Nothing extra |

## Baking

`up image build <class>` runs Packer (or pi-gen, or BlueBuild for Fedora Atomic) to:

1. Start from a stock OS image verified against the vendor's checksum or signature (SUPPLY-04).
2. Create the class user from the inventory group variable `up_user`, so the user layer has an account to apply to.
3. Run the hash-pinned `up.sh` with `--repo <instance> --host <class> --unattended` and `UP_PHASE=bake`. Only `bake` tasks run, and chezmoi skips every file that uses the `secret` partial. A private instance is read with a one-hour App token (SEC-11).
4. Install a **shallow, sparse** instance checkout at the build's signed commit: no history, and no other hosts' `secrets/hosts/*` files.
5. Sanitize (below), then scan the final filesystem with gitleaks in filesystem mode, failing the build on any finding.
6. Sign the image digest with the maintainer's key, and record the engine, instance and base-image digests in the image's metadata. `up image flash` verifies the signature before writing.

**Sanitization:** empty (not delete) `/etc/machine-id`; remove `/var/lib/dbus/machine-id`, the systemd random seed, SSH host keys, cloud-init state, the build credential, shell histories, journald and `/var/log/*`, `/root/.ansible`, Ansible temp and fact caches, Homebrew, uv and mise config that could hold tokens, git reflogs and config in the bare mirrors, Packer temp files, and local facts.

## First boot

Every path writes the same few things at flash or install time: hostname, the class user's public SSH keys, `/etc/up/host.yml`, the trust anchors (`allowed_signers.*`, `recipients.allow`, `known_hosts`), and for a private stock image a one-hour App token. With `--pregen`, the host's private keys are streamed onto the device as well (SEC-10).

- **Stock image:** cloud-init or the installer runs the hash-pinned `up.sh`. Bootstrap generates keys, waits for enrollment and converges.
- **Baked image:** the first-boot unit runs `/opt/up/bin/up --unattended`. It generates keys if none were pre-generated, enables the scheduler in the `provision` phase so provisioning finishes on a later run if it can't now, and converges what it can.

```yaml
#cloud-config (rendered for pi-03, stock image)
hostname: pi-03
users:
  - name: <you>
    groups: [sudo]
    shell: /bin/bash   # Ansible switches to zsh once installed
    ssh_authorized_keys: ["ssh-ed25519 AAAA… <you>@<name>"]
write_files:
  - path: /etc/up/host.yml
    content: |
      schema_version: 1
      name: <name>
      instance: https://github.com/<login>/<name>.git
      host: pi-03
      user: <you>
  - path: /etc/up/allowed_signers.instance
    content: "<signer lines from .up/allowed_signers, rendered>"
  - path: /etc/up/bootstrap-token
    permissions: "0400"
    content: "<one-hour GitHub App token, minted at render>"
runcmd:
  - [sh, -c, "curl -fsSLo /run/up.sh https://github.com/<you>/up/releases/download/v0.3.0/up.sh && echo '<sha256>  /run/up.sh' | sha256sum -c - && UP_VERSION=0.3.0 bash /run/up.sh --repo https://github.com/<login>/<name>.git --unattended"]
```

```ini
# /etc/systemd/system/up-firstboot.service
[Unit]
Description=First converge of a baked image
After=cloud-final.service network-online.target
Wants=network-online.target
ConditionPathExists=!/var/lib/up/firstboot.done

[Service]
Type=oneshot
StateDirectory=up
ExecStart=/opt/up/bin/up --unattended
SuccessExitStatus=3
Restart=on-failure
RestartSec=60
ExecStartPost=/usr/bin/touch /var/lib/up/firstboot.done

[Install]
WantedBy=multi-user.target
```

## Paths by platform

| Platform | Stock + first boot | Baked | Netboot |
| --- | --- | --- | --- |
| Raspberry Pi | Pi OS or Ubuntu Server + cloud-init | Packer (arm-image plugin) or pi-gen, per class | Pi 4/5 EEPROM network boot |
| x86 Linux PC | Ubuntu autoinstall or Fedora kickstart running the pinned `up.sh` | Packer qcow2/raw for VMs | Signed iPXE → installer with per-MAC config |
| Fedora Atomic / Bazzite | Install stock, then `rpm-ostree rebase` to your image; layered changes apply after a reboot inside the maintenance window (OPS-06) | Custom OCI image from a uBlue/BlueBuild recipe, signed | Signed iPXE → installer, kickstart rebases |
| macOS | Not imaged; Setup Assistant then the one-liner | Not applicable | Not applicable |

## Netboot (M7)

Netboot runs on an isolated provisioning VLAN with DHCP snooping, or the threat model's accepted risk applies. iPXE is built with an **embedded script and trusted certificate**, fetches everything over HTTPS, and verifies kernels, initrds and scripts with `imgverify` against the maintainer's key. No secrets travel over netboot: hosts generate keys on the device and enroll (SEC-10). `up image netboot sync` publishes signed scripts and hash-pinned configs.

## Optional mirror

If you run a LAN mirror later, `mirror` in `up.yml` points each tool at it through the tool's own settings (apt and dnf proxies, Homebrew's bottle domain, uv's index, registry mirrors). Artifacts are still verified against pinned hashes (SUPPLY-01), so a mirror can deny service but cannot substitute content. Mirror changes are notified (TRUST-11).

## Requirements

- **IMG-01** Every Ansible task MUST carry exactly one phase tag (task or block level).
  - AC: (negative) An ansible-lint rule fails on a task with zero or several phase tags.
- **IMG-02** `up image build` MUST bake with the class user from `up_user`, run only `bake` tasks, and skip chezmoi files that use the `secret` partial.
  - AC: Baking the template with secret-bearing dotfiles present succeeds.
  - AC: (negative) No secret-bearing file, and no secret value, is present in the baked image.
- **IMG-03** Images MUST be sanitized per the list above and pass a filesystem secret scan before they are published.
  - AC: Two VMs from one image report different machine IDs and host keys; (negative) a credential planted during the build fails the scan.
- **IMG-04** A baked image MUST contain a shallow, sparse instance checkout at a signed commit, without history or other hosts' secret files.
  - AC: (negative) `git log` in the image's mirror shows one commit; no `secrets/hosts/*` file for another host exists.
- **IMG-05** Built images MUST be signed with their engine, instance and base digests recorded, and `up image flash` MUST verify the signature before writing.
  - AC: (negative) A modified image fails `up image flash` before any write.
- **IMG-06** `up image render` MUST produce cloud-init, kickstart, autoinstall and iPXE outputs from inventory, and every value MUST pass the ARCH-05 schema patterns.
  - AC: Golden tests per output type; (negative) a hostile host alias is rejected before rendering.
- **IMG-07** Rendered configs MUST pin `up.sh` by SHA-256 from a release URL (TRUST-09), pass `--repo` explicitly, include the trust anchors, and carry no private key or long-lived token on platforms with a metadata service.
  - AC: Golden tests check the hash command, `--repo`, and the anchors in each output.
  - AC: (negative) Rendering a private key or a long-lived token for a metadata-service platform is refused.
- **IMG-08** `up image flash` MUST refuse a device mounted as a system disk, show the target, and ask for confirmation unless `--yes`.
  - AC: (negative) A loop device mounted as the system disk is refused.
- **IMG-09** The first-boot unit MUST run once, use `StateDirectory`, retry on failure with backoff, treat exit `3` as success, and enable the scheduler during `provision`.
  - AC: A baked image booted with networking disabled exits `3`, has the scheduler enabled, and does not re-run the unit after reboot.
- **IMG-10** `up image prune` MUST keep the last three images per class, named `<name>-<class>-<os>-<arch>-<yyyymmdd>`.
  - AC: After four builds, prune leaves three.
- **IMG-11** Netboot MUST use iPXE with an embedded script and certificate, HTTPS only, signature-verified images and scripts, and MUST serve no secrets.
  - AC: (negative) The client refuses a plain-HTTP fetch and an unsigned script; a netbooted host reaches "awaiting enrollment" without having received a private key.
- **IMG-12** When `mirror` is set, `up` MUST configure each supported tool's own mirror setting, still verify pinned hashes, and configure nothing when it is unset.
  - AC: With a mirror set and upstream blocked, a stock bootstrap succeeds; (negative) a mirror serving a modified artifact fails SUPPLY-01.
