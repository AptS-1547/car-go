//! car-go：把 go 工具链（build/test/vet）的输出渲染成 cargo 风格。
//!
//! 原理：go 的 `-json` 标志会输出结构化事件，本工具包装对应子命令，
//! 解析事件流后按 cargo 的样式重新渲染，退出码原样透传。

mod cli;
mod event;
mod render;
mod runner;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Build { args } => render::build::run(args),
        Command::Test { args } => render::test::run(args),
        Command::Vet { args } => render::vet::run(args),
        Command::Fmt { check, args } => render::fmt::run(*check, args),
        Command::Clippy { args } => render::clippy::run(args),
        Command::Mod { args } => render::gomod::run(args),
        Command::Add { pkgs } => render::gomod::run_add(pkgs),
        Command::Remove { pkgs } => render::gomod::run_remove(pkgs),
        Command::Update { pkgs } => render::gomod::run_update(pkgs),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            // 典型原因：go 不在 PATH 里
            eprintln!("error: failed to execute go command: {err}");
            ExitCode::FAILURE
        }
    }
}
