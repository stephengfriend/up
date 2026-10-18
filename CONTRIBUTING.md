# Contributing

`up` runs as root on every host it manages, so changes are reviewed against the spec and checked by CI before a human signs them. Coding agents: read `AGENTS.md` first; this file adds the contributor workflow.

## Spec first

The spec in `docs/design/` is the source of truth. Follow "How to work" in `AGENTS.md`:

1. Find the requirement IDs for the change (for example `TRUST-06`). If there are none, the change needs a spec change first.
2. If behavior changes, edit the design doc in the same change and keep roadmap coverage complete.
3. Write tests named after the IDs they prove (`test_trust_06_*`), one or more per `AC:`, including the negative ACs that prove malicious input is refused.
4. Implement the smallest change that makes them pass.

Do not decide an open question from `docs/open-questions.md` silently.

## Running the checks

```sh
tools/lint.sh all
```

runs the whole lint stage. Each CI job name is also an argument, so you can run one check at a time: `rust`, `cargo-deny`, `repo-files`, `schema-check`, `spec-check`, `workflow-lint`, `renovate-check`, `shellcheck`, `yamllint`, `ansible-lint` and `gitleaks`.

`tools/lint.sh` stands in for `up test lint` until that command lands in M2 (see `docs/design/cli.md`). A check whose implementation has not landed yet exits non-zero and names the plan that adds it; it never passes vacuously.

## Continuous integration

There is one workflow, `.github/workflows/ci.yml`, with one named job per check (the lint stage of CHG-07). The ten jobs are `rust`, `shellcheck`, `yamllint`, `ansible-lint`, `gitleaks`, `workflow-lint`, `spec-check`, `schema-check`, `repo-files` and `renovate-check`. Each job runs the matching `tools/lint.sh` entry. The `rust` job runs on ubuntu-24.04 and macos-15, and also runs `cargo-deny` on Linux.

The `gitleaks` job is the credential half of the "guard" stage in the Testing table of `docs/design/change-management.md`. It stands in until `up guard check` replaces it in Phase 10 (GRD-06).

Third-party actions are pinned by commit SHA and the default token is read-only (SUPPLY-02).

## Commits and merges

- Use Conventional Commits and name the spec IDs, for example `feat(dev): enforce SECURITY.md sections (PUB-12)`.
- Maintainers merge locally and sign the result with their hardware key (CHG-11). Hosts refuse GitHub's web-flow signature, so the merge button must not be used.
- Commits from Renovate, CI or a coding agent are reviewed and re-signed by a human in that local merge. Agents never hold a signing key and never sign.

## Guard and public-repo hygiene

- Never bypass guard or its hooks (`--no-verify`, `UP_GUARD_SKIP`, unsetting hooks). If a finding looks false, say so in the pull request; a human records any exception.
- This repo is public. Never commit secrets, real hostnames, addresses, emails or personal names. Use the placeholders `<you>`, `<login>`, `<name>` and `<host>`, and documentation address ranges.

## Pins

CI binaries are pinned with a SHA-256 per target in `versions.yml`. Python CI tools are hash-locked in `tools/ci-requirements.txt`; regenerate it with:

```sh
"$(cargo run --locked --quiet -p up-dev -- tool-path uv)" pip compile --no-config --universal --generate-hashes --python-version 3.13 -o tools/ci-requirements.txt tools/ci-requirements.in
```

Both are updated by hand until `up versions bump` lands in M8. Renovate opens pull requests for GitHub Actions and Cargo only, and never merges (SUPPLY-06).

## Reporting security issues

See `SECURITY.md`. Never report a vulnerability in a public issue.
