//! `go fmt` 的 cargo fmt 风格渲染。
//!
//! 注意包装的是 `go fmt`（认包模式，如 ./...），不是裸 `gofmt`（只认文件/目录）。
//! - 默认：直接格式化（go fmt 会列出改动的文件，保留这个输出，比 cargo fmt 的静默更有用）
//! - --check：go fmt 没有检查模式，先用 `go list` 把包模式解析成目录，
//!   再用 `gofmt -d` 打印 diff，有差异时返回 1（对齐 cargo fmt --check）

use std::io::{self, Write};

use crate::runner;

pub fn run(check: bool, args: &[String]) -> io::Result<u8> {
    // cargo fmt 格式化整个 crate，对应到 go 就是整个模块，默认 ./...
    let targets: Vec<String> = if args.is_empty() {
        vec!["./...".to_string()]
    } else {
        args.to_vec()
    };

    if check {
        return run_check(targets);
    }

    let mut full = vec!["fmt".to_string()];
    full.extend(targets);
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let status = runner::run("go", &full, |line| {
        // go fmt 输出被修改的文件名，原样透传
        let _ = writeln!(out, "{line}");
    })?;
    out.flush()?;
    Ok(runner::exit_code(status))
}

/// --check 模式：包模式 -> 目录 -> gofmt -d。
fn run_check(targets: Vec<String>) -> io::Result<u8> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    // 1. 用 go list 把 ./... 之类的包模式解析成具体目录
    let mut list_args = vec!["list".to_string(), "-f".to_string(), "{{.Dir}}".to_string()];
    list_args.extend(targets);
    let mut dirs = Vec::new();
    let status = runner::run("go", &list_args, |line| {
        dirs.push(line.to_string());
    })?;
    if !status.success() {
        return Ok(runner::exit_code(status));
    }

    // 2. gofmt -d 打印所有差异；有差异则透传并返回 1
    let mut fmt_args = vec!["-d".to_string()];
    fmt_args.extend(dirs);
    let mut diff = String::new();
    let status = runner::run("gofmt", &fmt_args, |line| {
        diff.push_str(line);
        diff.push('\n');
    })?;
    if !diff.trim().is_empty() {
        write!(out, "{diff}")?;
        out.flush()?;
        return Ok(1);
    }
    Ok(runner::exit_code(status))
}
