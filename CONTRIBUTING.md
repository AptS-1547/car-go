# Contributing to car-go

Thanks for helping improve car-go. This repository began as a joke and remains an experimental side project, so contributions are welcome but ongoing maintenance and review availability are not guaranteed. Read the [Code of Conduct](CODE_OF_CONDUCT.md) before participating.

## Before You Start

Search existing issues and pull requests first. For a bug report, include:

- `car-go --version`
- `go version` and the operating system
- the exact car-go command and module shape
- sanitized output and the underlying tool output when useful

Never include passwords, tokens, private module URLs, or proprietary source code.

## Development Setup

```bash
git clone https://github.com/AptS-1547/car-go.git
cd car-go
cargo check
cargo test
```

The wrapped Go commands require Go in `PATH`. Staticcheck is needed only for `car-go clippy`:

```bash
go install honnef.co/go/tools/cmd/staticcheck@latest
```

## Workflow

Use focused branches such as `feat/<description>`, `fix/<description>`, `docs/<description>`, or `refactor/<description>`. Prefer small, reviewable commits. Conventional commit prefixes are recommended:

```text
feat(render): add cargo-style benchmark output
fix(vet): preserve diagnostics from stderr
docs: clarify staticcheck setup
test(diagnostic): cover Windows-like paths
```

## Code Conventions

- Keep Go semantics in the Go toolchain; car-go owns process orchestration and presentation.
- Reuse `runner`, `event`, and `render::diagnostic` instead of duplicating subprocess or location parsing code.
- Preserve the wrapped command's exit status unless documented car-go behavior requires an additional diagnostic status.
- Keep output deterministic when buffering or ordering events.
- Add or update unit tests for parsing, grouping, exit-code, and edge-case behavior.
- Keep comments short and explain non-obvious protocol or rendering decisions.

## Checks Before a Pull Request

```bash
cargo fmt --all -- --check
cargo check
cargo test
```

For changes to a command renderer, also run a small real-module smoke test with the relevant Go command. If Staticcheck behavior changes, run `cargo run -- clippy ./...` as well.

## Pull Requests

Describe the user-visible output change, wrapped command behavior, and test plan. Call out compatibility changes to command-line arguments or exit statuses explicitly. Documentation-only changes should still state which links and examples were checked.

By submitting a contribution, you agree that it will be licensed under the repository's [MIT License](LICENSE).
