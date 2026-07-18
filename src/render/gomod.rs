//! Go 包管理的 cargo 风格渲染。
//!
//! 命令映射（cargo -> go）：
//! - `car-go add`     -> `go get`
//! - `car-go remove`  -> `go get <pkg>@none`（go 官方的移除方式）
//! - `car-go update`  -> `go get -u`
//! - `car-go mod ...` -> `go mod ...`（原始通道，go 特有的 tidy/verify 等）
//!
//! go 把下载/增删依赖的进度消息写到 stderr（且形如 "go: added ..."），
//! 统一用 run_merged 合并两路输出后按 cargo 样式重渲染。

use std::io::{self, Write};
use std::time::Instant;

use super::{format_elapsed, style};
use crate::runner;

/// `go mod` 原始通道：tidy / download / verify 等。
pub fn run(args: &[String]) -> io::Result<u8> {
    if args.is_empty() {
        eprintln!("error: car-go mod requires a subcommand, e.g. tidy / download / verify");
        return Ok(2);
    }

    let mut full = Vec::with_capacity(args.len() + 2);
    full.push("mod".to_string());
    full.extend(args.iter().cloned());
    // cargo 默认会展示 Downloaded/Adding 进度，而新版 go mod tidy 是静默的、
    // 进度消息只在 -v 下输出——自动补上，否则毫无 cargo 感
    if args[0] == "tidy" && !args.iter().any(|a| a == "-v" || a == "--v") {
        full.insert(2, "-v".to_string());
    }
    run_and_render(full)
}

/// `cargo add` -> `go get <pkgs...>`
pub fn run_add(pkgs: &[String]) -> io::Result<u8> {
    let mut full = vec!["get".to_string()];
    full.extend(pkgs.iter().cloned());
    run_and_render(full)
}

/// `cargo remove` -> `go get <pkg>@none...`（@none 是 go 官方的移除语法）
pub fn run_remove(pkgs: &[String]) -> io::Result<u8> {
    let mut full = vec!["get".to_string()];
    full.extend(pkgs.iter().map(|p| format!("{p}@none")));
    run_and_render(full)
}

/// `cargo update` -> `go get -u`（不带参数时更新整个模块，对齐 cargo update）
pub fn run_update(pkgs: &[String]) -> io::Result<u8> {
    let mut full = vec!["get".to_string(), "-u".to_string()];
    if pkgs.is_empty() {
        full.push("./...".to_string());
    } else {
        full.extend(pkgs.iter().cloned());
    }
    run_and_render(full)
}

/// 运行命令并流式渲染，成功时补一行 Finished（cargo 的包管理命令也有完成输出）。
fn run_and_render(full: Vec<String>) -> io::Result<u8> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let start = Instant::now();

    let status = runner::run_merged("go", &full, |line| {
        let _ = render_line(&mut out, line);
    })?;

    if status.success() {
        writeln!(
            out,
            "    {} in {}",
            style::status("Finished"),
            format_elapsed(start.elapsed())
        )?;
    }
    out.flush()?;
    Ok(runner::exit_code(status))
}

/// 单行渲染：认识的 `go:` 前缀消息换成 cargo 样式，其余原样透传。
fn render_line(out: &mut dyn Write, line: &str) -> io::Result<()> {
    const MAPPINGS: &[(&str, &str)] = &[
        ("go: downloading ", "Downloaded"),
        ("go: added ", "Adding"),
        ("go: removed ", "Removing"),
        ("go: upgraded ", "Upgrading"),
        ("go: downgraded ", "Downgrading"),
    ];
    for (prefix, word) in MAPPINGS {
        if let Some(rest) = line.strip_prefix(prefix) {
            // cargo 的状态词右对齐到 12 列，手动补齐空格（ANSI 转义码会干扰宽度计算）
            let pad = 12usize.saturating_sub(word.len());
            return writeln!(out, "{:pad$}{} {rest}", "", style::status(word));
        }
    }
    writeln!(out, "{line}")
}

#[cfg(test)]
mod tests {
    use super::render_line;

    #[test]
    fn maps_known_go_progress_and_preserves_unknown_lines() {
        let mut out = Vec::new();
        render_line(&mut out, "go: downloading example.com/mod v1.2.3").unwrap();
        render_line(&mut out, "go: removed example.com/old v0.1.0").unwrap();
        render_line(&mut out, "plain output: 1:2").unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Downloaded example.com/mod v1.2.3"));
        assert!(output.contains("Removing example.com/old v0.1.0"));
        assert!(output.contains("plain output: 1:2"));
    }

    #[test]
    fn maps_prefix_only_and_handles_empty_remainder() {
        let mut out = Vec::new();
        render_line(&mut out, "go: added ").unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Adding "));
        assert!(!output.contains("go: added"));
    }
}
