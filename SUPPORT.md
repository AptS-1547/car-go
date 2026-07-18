# Support

Use the channel that matches the problem:

- **Bug:** open a GitHub issue with reproduction steps, versions, the exact command, and sanitized output.
- **Feature idea:** open a feature request explaining the user problem and proposed behavior.
- **Usage question:** open a question issue after checking the README and existing issues.
- **Documentation:** report the file or section that is missing, stale, or confusing.
- **Security issue:** follow [SECURITY.md](SECURITY.md); do not post exploit details publicly.

## Useful Diagnostic Bundle

```text
car-go --version
go version
rustc --version
OS:
Command:
Working directory/module shape:
Output:
```

Remove credentials, tokens, private module paths, and proprietary source before posting. For output differences, include both the car-go output and the corresponding direct `go`, `gofmt`, or `staticcheck` output when possible.

Issues are the primary support channel for this experimental project. Response times may vary; a focused, reproducible report is easier to triage than a screenshot without command context.
