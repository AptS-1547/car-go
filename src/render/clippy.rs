//! `staticcheck` 的 cargo clippy 风格渲染：诊断渲染成 `warning[代码]` +
//! rustc 风格源码片段，结尾给 generated N warnings 汇总。
//!
//! cargo clippy 依赖 clippy-driver，car-go clippy 依赖 staticcheck：
//! `go install honnef.co/go/tools/cmd/staticcheck@latest`

use std::io::{self, Write};
use std::time::Instant;

use super::diagnostic::{Diagnostic, Level};
use super::{current_dir_display, format_elapsed, module_path, style};
use crate::event::StaticcheckDiag;
use crate::runner;

pub fn run(args: &[String]) -> io::Result<u8> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let module = module_path();
    // clippy 对应 cargo 的 check 语义，状态词用 Checking
    writeln!(
        out,
        "    {} {} ({})",
        style::status("Checking"),
        module,
        current_dir_display()
    )?;
    out.flush()?;

    let start = Instant::now();
    // staticcheck -f json 是 NDJSON，逐行解析；默认检查 ./...
    let mut full = vec!["-f".to_string(), "json".to_string()];
    full.extend(if args.is_empty() {
        vec!["./...".to_string()]
    } else {
        args.to_vec()
    });

    let mut warnings = 0usize;
    let mut errors = 0usize;
    let status = match runner::run("staticcheck", &full, |line| {
        if line.trim().is_empty() {
            return;
        }
        match serde_json::from_str::<StaticcheckDiag>(line) {
            Ok(diag) => {
                let d = convert(&diag);
                match d.level {
                    Level::Error => errors += 1,
                    Level::Warning => warnings += 1,
                }
                let _ = d.render(&mut out);
                let _ = writeln!(out);
            }
            // 非 JSON 行（异常情况）原样透传
            Err(_) => {
                let _ = writeln!(out, "{line}");
            }
        }
    }) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            writeln!(out, "{}: staticcheck not found", style::error_tag())?;
            writeln!(
                out,
                "       car-go clippy requires staticcheck (like cargo clippy requires clippy-driver)"
            )?;
            writeln!(
                out,
                "       install: go install honnef.co/go/tools/cmd/staticcheck@latest"
            )?;
            out.flush()?;
            return Ok(1);
        }
        Err(e) => return Err(e),
    };

    if warnings > 0 {
        writeln!(
            out,
            "{}: `{module}` generated {warnings} warning{}",
            style::warning_tag(None),
            if warnings == 1 { "" } else { "s" }
        )?;
    }
    if errors > 0 {
        // 对齐 cargo clippy：lint 达到 error 级别时视为编译失败
        writeln!(
            out,
            "{}: could not compile `{module}` due to {errors} previous error{}",
            style::error_tag(),
            if errors == 1 { "" } else { "s" }
        )?;
    }
    if status.success() && errors == 0 {
        writeln!(
            out,
            "    {} `dev` profile [unoptimized + debuginfo] target(s) in {}",
            style::status("Finished"),
            format_elapsed(start.elapsed())
        )?;
    }
    out.flush()?;
    Ok(runner::exit_code(status))
}

/// 把 staticcheck 诊断转成通用 Diagnostic。
fn convert(diag: &StaticcheckDiag) -> Diagnostic {
    // severity 只有 error 和 warning 两档需要区分，其余（含 ignored）按 warning 渲染
    let level = match diag.severity.as_deref() {
        Some("error") => Level::Error,
        _ => Level::Warning,
    };
    // end 与 location 同行才用于下划线（跨行诊断下划线没有意义）
    let end_col = diag.end.as_ref().and_then(|e| {
        (e.file == diag.location.file && e.line == diag.location.line).then_some(e.column)
    });
    Diagnostic {
        level,
        code: Some(diag.code.clone()),
        file: diag.location.file.clone(),
        line: diag.location.line,
        col: Some(diag.location.column),
        end_col,
        message: diag.message.clone(),
    }
}
