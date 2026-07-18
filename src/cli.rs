//! 命令行参数定义。

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Parser, Subcommand};

/// cargo 风格的 help 配色：标题绿色加粗、字面量青色（对齐 cargo help 的观感）
const CARGO_STYLING: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

/// cargo-style output for the Go toolchain.
#[derive(Parser)]
#[command(name = "car-go", version, about, styles = CARGO_STYLING)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Wrap `go build`, rendering Compiling/Finished + rustc-style errors
    Build {
        /// Arguments forwarded verbatim to `go build` (e.g. ./..., -o out)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Wrap `go test`, rendering cargo test style output
    Test {
        /// Arguments forwarded verbatim to `go test` (e.g. ./..., -run Xxx)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Wrap `go vet`, rendering cargo warning style
    Vet {
        /// Arguments forwarded verbatim to `go vet`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Wrap `go fmt`, behaving like cargo fmt
    Fmt {
        /// Check only; print diffs and exit 1 on differences (like cargo fmt --check)
        #[arg(long)]
        check: bool,
        /// Package patterns to format (default ./..., the whole module)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Wrap `staticcheck`, like cargo clippy (staticcheck must be installed)
    Clippy {
        /// Arguments forwarded verbatim to `staticcheck` (default ./...)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Wrap `go mod`, restyling download/add/remove messages in cargo style
    Mod {
        /// Arguments forwarded verbatim to `go mod` (e.g. tidy, download, verify)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Add dependencies (go get), like cargo add
    Add {
        /// Modules to add, optionally versioned (e.g. github.com/foo/bar@v1.2.3)
        #[arg(required = true)]
        pkgs: Vec<String>,
    },
    /// Remove dependencies (go get pkg@none), like cargo remove
    Remove {
        /// Modules to remove
        #[arg(required = true)]
        pkgs: Vec<String>,
    },
    /// Update dependencies (go get -u), like cargo update
    Update {
        /// Modules to update (defaults to the whole module)
        pkgs: Vec<String>,
    },
}
