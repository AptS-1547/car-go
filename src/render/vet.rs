//! `go vet` 的 cargo 风格渲染：逐包 `Checking` 行 +
//! 诊断渲染成 `warning[分析器]` + rustc 风格源码片段。
//!
//! `go vet -json` 的输出成分复杂，且分散在两个流里：
//! - stdout：每个包一个独立的 JSON 文档（首尾相接，不是 NDJSON）
//! - stderr（包编译失败时）：`# pkg` 错误头 + `vet: file:line:col: msg` 诊断
//!
//! 处理策略：合并两路输出（run_merged），`# pkg` 头渲染成 cargo 的 Checking 行，
//! `vet: ` 行渲染成 rustc 风格诊断；其余行累积成缓冲，每到新头就先把缓冲里的
//! JSON 诊断冲刷渲染，保证输出顺序。

use std::io::{self, Write};
use std::time::Instant;

use super::diagnostic::{Diagnostic, Level, parse_posn};
use super::{current_dir_display, format_elapsed, module_path, style};
use crate::event::{VetDiagnostic, VetReport};
use crate::runner;

pub fn run(args: &[String]) -> io::Result<u8> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let module = module_path();
    // vet 对应 cargo 的 check 语义，状态词用 Checking
    writeln!(
        out,
        "    {} {} ({})",
        style::status("Checking"),
        module,
        current_dir_display()
    )?;
    out.flush()?;

    let start = Instant::now();
    // go vet -json 有发现时退出码仍是 0（纯文本模式才是 1），
    // 为了和 go vet 原生语义一致，有诊断时我们自己返回 1
    let mut count = 0usize;
    let mut buf = String::new();
    // 连续的 "# pkg" / "# [pkg]" 头指同一个包，去重避免重复打印
    let mut last_checking: Option<String> = None;

    let status = runner::run_merged("go", &runner::json_args("vet", args), |line| {
        if let Some(pkg) = line.strip_prefix("# ") {
            // 新进度/错误头：先冲刷之前累积的 JSON 诊断，再打印 Checking 行
            let _ = render_docs(&mut out, &mut buf, &mut count);
            let pkg = pkg.trim_start_matches('[').trim_end_matches(']');
            if last_checking.as_deref() != Some(pkg) {
                last_checking = Some(pkg.to_string());
                let _ = writeln!(out, "    {} {pkg}", style::status("Checking"));
                let _ = out.flush();
            }
        } else if let Some(rest) = line.strip_prefix("vet: ") {
            // 包编译失败的诊断（stderr）：渲染成 rustc 风格
            let _ = render_docs(&mut out, &mut buf, &mut count);
            match Diagnostic::parse_go_line(rest, Level::Error) {
                Some(d) => {
                    let _ = d.render(&mut out);
                    let _ = writeln!(out);
                }
                // 无法解析为位置诊断的错误原样透传
                None => {
                    let _ = writeln!(out, "{}: {rest}", style::error_tag());
                }
            }
        } else {
            buf.push_str(line);
            buf.push('\n');
        }
    })?;
    // 冲刷剩余的 JSON 文档
    render_docs(&mut out, &mut buf, &mut count)?;

    if count > 0 {
        writeln!(
            out,
            "{}: `{module}` generated {count} warning{}",
            style::warning_tag(None),
            if count == 1 { "" } else { "s" }
        )?;
    }

    if status.success() {
        writeln!(
            out,
            "    {} `dev` profile [unoptimized + debuginfo] target(s) in {}",
            style::status("Finished"),
            format_elapsed(start.elapsed())
        )?;
    }
    out.flush()?;
    if count > 0 && status.success() {
        return Ok(1);
    }
    Ok(runner::exit_code(status))
}

/// 解析并渲染 buf 中累积的所有 JSON 文档，然后清空 buf。
///
/// vet 构建失败时输出根本不是 JSON：解析失败且还没有渲染过任何诊断时，
/// 把原文透传，保证错误信息不丢。
fn render_docs(out: &mut dyn Write, buf: &mut String, count: &mut usize) -> io::Result<()> {
    if buf.trim().is_empty() {
        return Ok(());
    }
    let mut parse_failed = false;
    for doc in serde_json::Deserializer::from_str(buf.as_str()).into_iter::<VetReport>() {
        let Ok(report) = doc else {
            parse_failed = true;
            break;
        };
        for analyzers in report.values() {
            for (analyzer, diags) in analyzers {
                for vd in diags {
                    *count += 1;
                    match convert(vd, analyzer) {
                        Some(d) => d.render(out)?,
                        // 位置解析失败也不丢诊断，退化为单行警告
                        None => writeln!(
                            out,
                            "{}: {}",
                            style::warning_tag(Some(analyzer)),
                            vd.message
                        )?,
                    }
                    writeln!(out)?;
                }
            }
        }
    }
    if parse_failed && *count == 0 {
        write!(out, "{buf}")?;
    }
    buf.clear();
    Ok(())
}

/// 把 vet 诊断转成通用 Diagnostic；位置串解析失败返回 None。
fn convert(vd: &VetDiagnostic, analyzer: &str) -> Option<Diagnostic> {
    let (file, line, col) = parse_posn(&vd.posn)?;
    // end 与 posn 同行同列时才有效（跨行诊断下划线没有意义）
    let end_col = vd
        .end
        .as_deref()
        .and_then(parse_posn)
        .and_then(|(efile, eline, ecol)| (efile == file && eline == line).then_some(ecol));
    Some(Diagnostic {
        level: Level::Warning,
        code: Some(analyzer.to_string()),
        file,
        line,
        col: Some(col),
        end_col,
        message: vd.message.clone(),
    })
}
