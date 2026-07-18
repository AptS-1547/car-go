//! `go build` 的 cargo 风格渲染：
//! `Compiling` ->（逐条 rustc 风格诊断）-> `Finished` 或 `could not compile`。

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::Instant;

use super::diagnostic::{Diagnostic, Level};
use super::{current_dir_display, format_elapsed, module_path, style};
use crate::event::GoEvent;
use crate::runner;

pub fn run(args: &[String]) -> io::Result<u8> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let module = module_path();
    // cargo 的状态词右对齐到 12 列；手写空格避免 ANSI 转义码干扰宽度计算
    writeln!(
        out,
        "   {} {} ({})",
        style::status("Compiling"),
        module,
        current_dir_display()
    )?;
    out.flush()?;

    let start = Instant::now();
    // 每个包的编译错误数，用于最后的 could not compile 汇总
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    // 收到 build-fail 事件的包
    let mut failed: Vec<String> = Vec::new();
    // 当前诊断归属的包：由 go 输出里的 "# pkg" 行维护
    let mut current_pkg = String::new();

    let status = runner::run("go", &runner::json_args("build", args), |line| {
        let Ok(ev) = serde_json::from_str::<GoEvent>(line) else {
            // 不是 JSON 的行（异常情况），原样透传
            let _ = writeln!(out, "{line}");
            return;
        };
        match ev.action.as_str() {
            "build-output" => {
                let Some(text) = ev.output.as_deref() else {
                    return;
                };
                for l in text.lines() {
                    if let Some(pkg) = l.strip_prefix("# ") {
                        current_pkg = pkg.to_string();
                    } else if let Some(d) = Diagnostic::parse_go_line(l, Level::Error) {
                        *errors.entry(current_pkg.clone()).or_default() += 1;
                        let _ = d.render(&mut out);
                        let _ = writeln!(out);
                    } else {
                        // 链接器错误等无法解析为诊断的行，原样打印
                        let _ = writeln!(out, "{l}");
                    }
                }
            }
            "build-fail" => {
                if let Some(pkg) = &ev.import_path {
                    failed.push(pkg.clone());
                }
            }
            _ => {}
        }
    })?;

    if status.success() {
        writeln!(
            out,
            "    {} `dev` profile [unoptimized + debuginfo] target(s) in {}",
            style::status("Finished"),
            format_elapsed(start.elapsed())
        )?;
    } else {
        // cargo 风格的编译失败汇总；build-fail 缺失时退化为按报错包汇总
        let failed_pkgs = if failed.is_empty() {
            errors.keys().cloned().collect::<Vec<_>>()
        } else {
            failed
        };
        for pkg in failed_pkgs {
            let n = errors.get(&pkg).copied().unwrap_or(0);
            writeln!(
                out,
                "{}: could not compile `{pkg}` due to {n} previous error{}",
                style::error_tag(),
                if n == 1 { "" } else { "s" }
            )?;
        }
    }
    out.flush()?;
    Ok(runner::exit_code(status))
}
