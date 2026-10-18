# up

One command to baseline a macOS or Linux machine, and one command to keep it that way.

```sh
curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash
up
```

`up` is an engine: a signed Rust binary, Ansible roles, chezmoi helpers, guard tooling, and image and dev container builds. Your hosts, secrets and dotfiles live in your own private **instance** repo. On a new machine, the one-liner verifies and runs the binary, which finds your instance from your GitHub credentials, anchors trust in your signing keys, installs itself root-owned, and converges. Hosts apply only releases and commits signed by keys you hold.

## Get started

- **No instance yet?** `curl -fsSL https://raw.githubusercontent.com/<you>/up/main/up.sh | bash -s -- init`, or see `docs/guides/getting-started.md` for the manual and agent-assisted paths.
- **Have an instance?** Run the one-liner on a new machine, then `up` whenever you want to converge.
- **Forking?** Change `UP_REPO` and `UP_SIGNERS` at the top of `up.sh`, and sign your own releases with `up release`.

## Documentation

- Design spec: [docs/README.md](docs/README.md); threat model: [docs/design/threat-model.md](docs/design/threat-model.md)
- Roadmap: [docs/roadmap.md](docs/roadmap.md)
- Security reports: [SECURITY.md](SECURITY.md)
- Agents: [AGENTS.md](AGENTS.md) and [prompts/](prompts/)

## Status

Design stage. See the roadmap for milestones.
