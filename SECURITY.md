# Security policy

`up` runs as root on every host it manages, so a flaw in the engine can become a flaw on every machine that applies it. Please report problems privately and give us time to ship a signed fix.

## Reporting a vulnerability

Report vulnerabilities through GitHub private vulnerability reporting: open this repository's Security tab and choose "Report a vulnerability". Only the maintainers can read the report.

Never report a vulnerability in a public issue, discussion or pull request.

Please include:

- the affected version or commit
- the platform (macOS arm64, or Linux arm64 or x86_64, and the distribution)
- steps to reproduce
- the impact you expect, for example what an attacker needs and what they gain

Do not include real secrets, credentials or personal data in a report. If a proof of concept needs one, describe it and use a placeholder.

## Supported versions

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older releases | No |

Until 1.0 only the latest release receives fixes. No release exists yet.

## Embargo and disclosure

- We acknowledge a report within 7 days and give an initial assessment within 14 days.
- The embargo lasts up to 90 days from the report, or until a fixed, signed release is published, whichever comes first. It can be extended by agreement with the reporter.
- Fixes ship as signed releases. Hosts only apply engine releases signed by the maintainers' keys, so a fix reaches a host only after a human has signed it.
- When the fixed release is published we publish a GitHub Security Advisory, with a CVE where applicable, and credit the reporter unless they decline.
