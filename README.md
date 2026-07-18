# car-go

Cargo-style terminal output for the Go toolchain.

[中文文档](README.zh.md)

`car-go` is a small Rust wrapper around the tools already installed on your machine. It invokes `go`, `gofmt`, and optionally `staticcheck`, then turns structured events and diagnostics into Cargo-like status lines, source snippets, test summaries, and dependency progress messages.

It does not replace Go's compiler, test runner, formatter, or module resolver. The underlying command remains authoritative; car-go focuses on presentation and keeps the wrapped command's exit status.

## Features

- Cargo-style `Compiling`, `Checking`, `Running`, and `Finished` status lines
- rustc-like source locations and caret diagnostics for build errors, vet findings, and Staticcheck output
- grouped `go test` output with package-level summaries and failed-test stdout
- `car-go fmt --check`, implemented with `go list` and `gofmt -d`
- `car-go clippy`, backed by `staticcheck -f json`
- Cargo-like rendering for `go mod` download, add, remove, upgrade, and downgrade messages
- `add`, `remove`, and `update` shortcuts modeled after Cargo commands
- forwarded arguments and preserved subprocess exit codes in the normal case; diagnostic findings follow each command's documented failure semantics
- a single optimized Rust binary with no runtime service or configuration file

## Command mapping

| car-go | Wrapped command |
| --- | --- |
| `car-go build ...` | `go build -json ...` |
| `car-go test ...` | `go test -json ...` |
| `car-go vet ...` | `go vet -json ...` |
| `car-go fmt ...` | `go fmt ...` |
| `car-go fmt --check ...` | `go list` followed by `gofmt -d` |
| `car-go clippy ...` | `staticcheck -f json ...` |
| `car-go mod ...` | `go mod ...` |
| `car-go add PKG...` | `go get PKG...` |
| `car-go remove PKG...` | `go get PKG@none...` |
| `car-go update [PKG...]` | `go get -u [PKG...]` |

```bash
car-go build ./...
car-go test ./... -run TestUser -count=1
car-go vet ./...
car-go fmt --check ./...
car-go clippy ./...
car-go mod tidy
car-go add github.com/example/project@v1.2.3
```

Arguments after a subcommand are forwarded to the wrapped tool. `car-go mod tidy` adds `-v` unless verbosity was requested explicitly.

## Installation

Build requirements are Rust `1.88.0+` and a Go toolchain available in `PATH`.

```bash
git clone https://github.com/AptS-1547/car-go.git
cd car-go
cargo install --path .
```

For a checkout-local run:

```bash
cargo run -- test ./...
```

Install Staticcheck only for `car-go clippy`:

```bash
go install honnef.co/go/tools/cmd/staticcheck@latest
```

Make sure `$(go env GOPATH)/bin` is on `PATH`.

## Runtime boundaries

- `build`, `test`, `vet`, `fmt`, and `mod` call the local `go` executable.
- `fmt --check` calls `gofmt` after resolving package directories with `go list`.
- `clippy` means Staticcheck here; it is not another spelling of `go vet`.
- Go remains responsible for build, test, formatting, and module semantics.
- Unrecognized tool output and diagnostics that cannot be parsed are passed through unchanged.
- Go tests run packages concurrently; car-go buffers package output before printing Cargo-like blocks.

## Development

```bash
cargo fmt --all
cargo check
cargo test
cargo run -- --help
```

The main boundaries are `src/cli.rs` for the command surface, `src/runner.rs` for subprocess handling, `src/event.rs` for structured protocols, and `src/render/` for presentation. New commands should reuse those boundaries instead of duplicating process, exit-code, or diagnostic parsing logic.

## Project status

This project started as a joke and remains an experimental side project. It is not a promise of long-term maintenance, stable CLI compatibility, or regular releases. It may continue to evolve, stay as-is, or be discontinued. The CLI and rendering format may also change as Go versions expose different JSON event details. When reporting a problem, include `car-go --version`, `go version`, the complete command, and sanitized output.

## Community

- [Contributing](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
- [Support](SUPPORT.md)

## License

[MIT License](LICENSE), Copyright `2026 AptS-1547`.
