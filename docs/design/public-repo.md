---
title: Public repository, onboarding and AI assets
status: draft
prefix: PUB
milestones: [M0, M4]
depends_on: [architecture, cli, guard, trust]
---

# Public repository, onboarding and AI assets

The public `up` repo is a complete product. Today it has one user, but anyone can fork it and end up with their own instance by following the guides, by running the `up init` wizard, or by letting a coding agent drive the wizard. Because the engine is public and runs as root, it also ships the threat model, the security policy, and the guard tooling that keeps every user's credentials and personal data out of both repos.

## What the public repo contains

| Area | Path | Purpose |
| --- | --- | --- |
| Entrypoint | `up.sh` (also attached to each release with its signature) | Fetch, verify and run the signed binary |
| Engine | `crates/up/`, `ansible/`, `chezmoi/`, `images/`, `containers/`, `schema/`, `toolchain/`, `defaults.yml`, `versions.yml` | Everything a host runs |
| Guard | `hooks/`, `guard/` | Commit and push checks |
| Template | `template/` | Skeleton instance for `up init` and the manual guide |
| Spec | `docs/README.md`, `docs/design/` (including `threat-model.md`), `docs/roadmap.md`, `docs/decisions.md`, `docs/glossary.md`, `docs/open-questions.md`, `docs/reviews/` | This design, with requirement IDs and acceptance criteria |
| Guides | `docs/guides/` | Getting started, manual setup, wizard, agent-assisted setup, enrollment, signed merges, add host, add role, secrets, images, offline, dev containers, upgrading and developing the engine, runbooks, troubleshooting |
| Reference | `docs/reference/` | CLI and schema reference, generated |
| Agent instructions | `AGENTS.md`, `prompts/` | Agent-agnostic instructions and task prompts |
| Agent adapters | `CLAUDE.md`, `.claude/commands/` | Thin, generated Claude Code wrappers |
| Project files | `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `LICENSE`, `.up-engine` | Standard files; `.up-engine` marks the repo for guard |

M0 enforces the skeleton subset of this table (PUB-01): every path except `up.sh`, `docs/guides/`, `docs/reference/` and `CHANGELOG.md`, which are enforced from M4.

`template/` is a minimal working instance: `up.yml` with `engine.version`, `engine.commit` and `visibility: private`; `.up/allowed_signers` and `.up/recipients`; an example inventory using aliases; starter dotfiles; an example catalog on the `fake` backend; `.sops.yaml` rules; `CODEOWNERS`; a Renovate config with automerge off; instance CI; and its own `AGENTS.md`.

## Three ways to create an instance

All three produce the same result, because the wizard and the agent both end in the same CLI command.

1. **Manual.** Follow `docs/guides/manual-setup.md`. Create a private repo and copy `template/`. Fill in `up.yml` and inventory, and add your signing keys (primary and backup) to `.up/allowed_signers`. Configure rulesets (signed commits, required CI, no force push, tag protection), enable push protection, add the discovery topic, then commit (signed), push and run the one-liner.
2. **Wizard.** `up.sh init`, or `up init` with the binary installed. It discovers your GitHub login and asks for the name, secrets backends and vaults, your signing keys from the agent, and the first host with its roles. It copies `template/`, renders `up.yml` with the running engine's version and commit, installs guard hooks and makes the first signed commit. Only after you confirm does it create the private repo, apply the rulesets and push protection, push, and bootstrap this machine.
3. **Agent-assisted.** A coding agent reads `AGENTS.md` and `prompts/init-instance.md`, asks the same questions, writes an answers file, validates it against `schema/answers.schema.json`, and runs `up init --answers answers.yml`. The agent never signs, never holds a secrets session, and never re-implements wizard logic; the signed first commit happens at the operator's terminal.

## AI assets

- **`AGENTS.md`** is the canonical instruction file for any coding agent: repo map, test commands, spec-first rule, ID conventions, and security rules. Agents never hold a signing key or a secrets-backend session, never bypass guard, and never change signers, recipients or exceptions.
- **`prompts/`** holds one Markdown file per task, with front matter naming its inputs and the `up` command or pull request it ends with.
- **Adapters are generated:** `CLAUDE.md` imports `AGENTS.md`; each `.claude/commands/<prompt>.md` wraps a prompt.
- Agent-authored changes reach hosts only after a human signs them (CHG-11). Protected operations need a TTY (TRUST-12).

## The spec as a buildable plan

Every requirement has an ID, MUST/SHOULD/MAY wording, and acceptance criteria written as tests. Requirements that run as root, fetch from the network or handle a credential carry at least one negative AC. A checker in CI keeps the spec honest; `prompts/plan-from-spec.md` turns it into milestones, issues and tasks, and `prompts/implement-requirement.md` builds one ID at a time.

## Requirements

- **PUB-01** The public repo MUST contain every item in the contents table before the first tagged release; M0 requires the skeleton paths only.
  - AC: A CI check asserts each path exists, with the M0 subset enforced from M0.
- **PUB-02** `template/` MUST be a working instance that converges in a container with the `fake` backend and a test signer set.
  - AC: Engine CI creates an instance from `template/` and converges it.
- **PUB-03** `docs/guides/manual-setup.md` MUST let a person create an instance without the wizard or an agent.
  - AC: Code blocks marked `sh test` run in CI against a scratch directory; (manual) a fresh GitHub account completes the guide before the first release.
- **PUB-04** `up init` MUST run interactively and from `--answers`, validate input against `schema/answers.schema.json`, and produce identical instances for identical answers.
  - AC: CI runs `up init --answers` twice and diffs the results (ignoring commit metadata); the result passes instance CI.
- **PUB-05** `up init` MUST NOT write secret values, MUST default to a private repo, MUST ask before creating the remote, pushing, or bootstrapping, and MUST configure rulesets (signed commits, required CI, no force push), push protection and the discovery topic when it creates the remote.
  - AC: (negative) Without confirmation no GitHub API write happens; with it, the repo's rulesets and push protection match the expected settings.
- **PUB-06** `AGENTS.md` and `prompts/` MUST reference only paths and commands that exist; the check is enforced from M4.
  - AC: A CI check resolves every referenced path and `up` command.
- **PUB-07** Prompts MUST finish with documented `up` commands or a pull request, and contain no logic the CLI lacks.
  - AC: Each prompt's `ends_with` front matter names an existing command or `pull-request`.
- **PUB-08** Adapters MUST be generated from `AGENTS.md` and `prompts/`.
  - AC: CI regenerates adapters and fails on any diff.
- **PUB-09** The spec checker MUST enforce unique IDs, front matter, roadmap coverage of every requirement, a negative AC on every requirement whose text involves root, the network or a credential, and MUST flag normative words (MUST, never, only) outside requirement bullets.
  - AC: (negative) Fixture specs violating each rule fail the checker.
- **PUB-10** The repo MUST carry an OSI-approved license before it is made public.
  - AC: `LICENSE` exists and is detected by GitHub's license check.
- **PUB-11** `template/AGENTS.md` MUST explain how an agent operates an instance using the engine's prompts, including that it cannot sign.
  - AC: Covered by the PUB-06 check run against `template/`.
- **PUB-12** `SECURITY.md` MUST state the private reporting channel, supported versions, and the embargo and disclosure process.
  - AC: A CI check asserts the three sections exist.
- **PUB-13** `AGENTS.md` MUST state that agents never hold signing keys or secrets-backend sessions, never bypass guard, and never change signers, recipients or guard exceptions.
  - AC: (negative) Removing any one statement from the rules section fails CI.
