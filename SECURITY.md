# Security Policy

## Supported Versions

Awo Console is still pre-1.0. Security fixes should be assumed to target the latest state of the `main` branch unless a maintainer states otherwise.

## Reporting A Vulnerability

Please do not post detailed exploit information in a public issue.

Preferred reporting path:

1. Use GitHub Security Advisories / private vulnerability reporting if it is enabled for the repository.
2. If private reporting is not available, open a minimal public issue without exploit details and ask maintainers for a secure follow-up path.

When reporting a vulnerability, include:

- affected version or commit
- impacted platform
- reproduction steps
- expected impact
- any proof-of-concept details that are needed for maintainers to validate the issue safely

## Scope

The highest-priority reports are issues that could:

- expose secrets or sensitive local data
- execute unintended commands
- break workspace isolation guarantees
- corrupt or destroy user work unexpectedly
- bypass safety constraints around slots, sessions, or cleanup

## Response Expectations

Because the project is pre-1.0 and maintainer capacity may vary, response times are best-effort rather than guaranteed. Valid reports will still be treated seriously and triaged as quickly as practical.
