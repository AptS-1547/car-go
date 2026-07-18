# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `cargo binstall` support — prebuilt binaries from GitHub Releases are discoverable via `[package.metadata.binstall]`

## [0.1.0] - 2026-07-19

### Added

- `car-go build` — wraps `go build -json`, renders `Compiling`/`Finished` and rustc-style diagnostics with source snippets
- `car-go test` — wraps `go test -json`, renders per-package `Running unittests` blocks with `test X ... ok/FAILED/ignored`, `failures:` sections and `test result:` summaries; subtests are merged into their parent test
- `car-go vet` — wraps `go vet -json`, renders per-package `Checking` lines and `warning[analyzer]` diagnostics; exit code matches plain `go vet` semantics (1 on findings)
- `car-go fmt` — wraps `go fmt`; `--check` prints diffs and exits 1, like `cargo fmt --check`
- `car-go clippy` — wraps `staticcheck -f json`, error/warning levels with `could not compile` summary
- `car-go mod` — wraps `go mod`, restyles download/add/remove progress messages in cargo style; `tidy` automatically runs verbose
- `car-go add` / `remove` / `update` — cargo-style package management on top of `go get`
- Cargo-style colored help output

[0.1.0]: https://github.com/AptS-1547/car-go/releases/tag/v0.1.0
