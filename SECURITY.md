# Security Policy

car-go is a local command-line wrapper. It launches tools from the current user's `PATH` and forwards command-line arguments to them. Security reports are most useful when they identify a concrete issue in car-go's argument handling, process handling, output parsing, or release artifacts.

## Supported Versions

The active development branch and the latest tagged release receive security attention. Older releases are best effort. Because this is an experimental joke project, response and fix timelines are not guaranteed.

| Version or branch | Security support |
| --- | --- |
| `main` | Supported for current development |
| Latest tagged release | Supported |
| Older releases | Best effort |

## Reporting a Vulnerability

Do not publish security vulnerabilities in a public issue, pull request, discussion, or review comment. Use GitHub Private Vulnerability Reporting when available, or email `report@esaps.net` with a subject beginning `[car-go Security]`.

Include, when possible:

- the affected version, commit, or binary checksum
- operating system and Rust/Go versions
- the exact command line and a minimal reproduction
- expected and observed behavior
- practical impact, such as unintended argument injection, arbitrary command execution through car-go itself, sensitive data exposure, or a denial of service
- relevant logs or traces with secrets removed

Do not send real tokens, private source code, credentials, or data copied from systems you do not own. Replace sensitive values with placeholders.

## Response and Disclosure

Maintainers will triage actionable reports when time permits. The usual process is private triage, a tested fix or mitigation, a release or advisory when appropriate, and coordinated public disclosure after users have had a reasonable opportunity to update.

## Research Guidance

Test only binaries and repositories you own or are explicitly permitted to test. Prefer local, disposable fixtures. Keep proof-of-concept input minimal and non-destructive, and stop testing if you encounter data or capabilities outside the intended fixture.
