//! `go test` 的 cargo 风格渲染：
//! `Compiling` -> `Finished` -> 每个包一个完整的 `Running unittests` 块
//! （`test X ... ok/FAILED/ignored` + `failures:` 区块 + `test result:` 汇总）。
//!
//! 注意 `go test ./...` 是并行跑包的，事件流会交错；而 cargo 顺序跑测试二进制。
//! 所以每个包的输出行先缓冲，等包级 pass/fail/skip 事件到达时整块输出，
//! 保证块与块之间不混杂。

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use colored::Colorize;

use super::diagnostic::{Diagnostic, Level};
use super::{current_dir_display, format_elapsed, module_path, style};
use crate::event::GoEvent;
use crate::runner;

/// 单个包的测试运行状态。
#[derive(Default)]
struct PackageRun {
    passed: u32,
    failed: u32,
    skipped: u32,
    /// 失败的顶层测试名，保持发现顺序（cargo 的 failures 列表也是这样）
    failed_tests: Vec<String>,
    /// 各测试的输出缓冲，key 为顶层测试名（子测试输出归并到父测试）。
    /// 测试通过/跳过时清空，只保留失败测试的输出（对齐 cargo 只展示失败 stdout 的行为）
    outputs: BTreeMap<String, Vec<String>>,
    /// 渲染好的输出行（`test X ... ok`、panic 等包级透传），
    /// 包结束时一次性整块输出，避免并行包的输出交错
    lines: Vec<String>,
    /// 编译错误数；>0 表示该包根本没跑起来，汇总时输出 could not compile
    build_errors: usize,
}

pub fn run(args: &[String]) -> io::Result<u8> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let module = module_path();
    writeln!(
        out,
        "   {} {} ({})",
        style::status("Compiling"),
        module,
        current_dir_display()
    )?;
    out.flush()?;

    let start = Instant::now();
    // go test 先编译后运行：第一个 start 事件 ≈ 编译完成的信号，
    // 此时补打 Finished 行，还原 cargo 的输出顺序
    let mut finished_printed = false;
    let mut pkgs: BTreeMap<String, PackageRun> = BTreeMap::new();
    // 编译错误归属的包：由包级输出里的 "# pkg" 行维护
    let mut error_pkg: Option<String> = None;

    let status = runner::run("go", &runner::json_args("test", args), |line| {
        let Ok(ev) = serde_json::from_str::<GoEvent>(line) else {
            let _ = writeln!(out, "{line}");
            return;
        };
        // go test 跑测试前先编译（默认还带一轮 vet 检查），编译失败以
        // build-output/build-fail 事件出现，这类事件没有 Package 字段，
        // 诊断归属靠输出里的 "# pkg" 行维护；编译阶段的诊断立即输出
        // （对齐 cargo 在 Compiling 阶段直接报错的行为）
        if ev.package.is_none() {
            if ev.action == "build-output" {
                let Some(text) = ev.output.as_deref() else {
                    return;
                };
                let fallback = ev
                    .import_path
                    .as_deref()
                    .map(base_import_path)
                    .unwrap_or_default();
                handle_package_output(&mut out, &mut pkgs, fallback, text, &mut error_pkg);
            }
            return;
        }
        let pkg = ev.package.unwrap();
        match ev.action.as_str() {
            "start" => {
                // 编译失败的包不打印 Running，汇总时统一交代
                let build_failed = pkgs.get(&pkg).map(|s| s.build_errors > 0).unwrap_or(false);
                if build_failed {
                    return;
                }
                if !finished_printed {
                    finished_printed = true;
                    // 只要有包编译失败就不打印 Finished：
                    // cargo 遇到编译错误会直接中止，不会 Finished
                    if !pkgs.values().any(|s| s.build_errors > 0) {
                        let _ = writeln!(
                            out,
                            "    {} `test` profile [unoptimized + debuginfo] target(s) in {}",
                            style::status("Finished"),
                            format_elapsed(start.elapsed())
                        );
                    }
                }
            }
            "output" => {
                let Some(text) = ev.output.as_deref() else {
                    return;
                };
                match &ev.test {
                    Some(test) => {
                        let top = top_test_name(test);
                        let entry = pkgs.entry(pkg).or_default().outputs.entry(top).or_default();
                        for l in text.lines() {
                            // 过滤 go 自己的 === RUN / --- PASS 等框架行，
                            // 还原 cargo failures 区块里纯 stdout 的观感
                            if !is_framework_line(l) {
                                entry.push(l.to_string());
                            }
                        }
                    }
                    None => {
                        // 包级输出（panic 等）：缓冲进该包的块，避免交错
                        let st = pkgs.entry(pkg.clone()).or_default();
                        for l in text.lines() {
                            if let Some(d) = Diagnostic::parse_go_line(l, Level::Error) {
                                // 罕见情况：测试期的编译诊断，立即输出
                                st.build_errors += 1;
                                let _ = d.render(&mut out);
                                let _ = writeln!(out);
                            } else if !is_summary_line(l) {
                                st.lines.push(l.to_string());
                            }
                        }
                    }
                }
            }
            "pass" | "fail" | "skip" => match &ev.test {
                Some(test) => {
                    // 子测试不单独计数/打印：它的失败会传导到父测试，
                    // 输出也已经归并进父测试的缓冲（cargo 没有子测试概念）
                    if test.contains('/') {
                        return;
                    }
                    let st = pkgs.entry(pkg).or_default();
                    match ev.action.as_str() {
                        "pass" => {
                            st.passed += 1;
                            st.outputs.remove(test.as_str());
                            st.lines
                                .push(format!("test {test} ... {}", style::ok_tag()));
                        }
                        "fail" => {
                            st.failed += 1;
                            st.failed_tests.push(test.clone());
                            st.lines
                                .push(format!("test {test} ... {}", style::failed_tag()));
                        }
                        "skip" => {
                            st.skipped += 1;
                            st.outputs.remove(test.as_str());
                            st.lines
                                .push(format!("test {test} ... {}", style::ignored_tag()));
                        }
                        _ => unreachable!(),
                    }
                }
                None => {
                    // 包级 pass/fail/skip：该包跑完了，整块输出。
                    // 注意没有测试文件的包（[no test files]）发的是 skip 事件，
                    // 它和 pass 一样都算成功
                    let st = pkgs.remove(&pkg).unwrap_or_default();
                    let ok = ev.action != "fail";
                    let elapsed = ev.elapsed.unwrap_or_default();
                    let _ = print_package_block(&mut out, &pkg, &st, ok, elapsed);
                    let _ = out.flush();
                }
            },
            // run/pause/cont/bench 不需要渲染
            _ => {}
        }
    })?;

    out.flush()?;
    Ok(runner::exit_code(status))
}

/// 提取顶层测试名："TestFoo/sub" -> "TestFoo"
fn top_test_name(test: &str) -> String {
    test.split('/').next().unwrap_or(test).to_string()
}

/// 去掉 ImportPath 的构建变体后缀："pkg [pkg.test]" -> "pkg"
fn base_import_path(import_path: &str) -> &str {
    import_path.split(' ').next().unwrap_or(import_path)
}

/// 处理编译阶段（无 Package 字段事件）的包级输出："# pkg" 头和编译错误。
fn handle_package_output(
    out: &mut dyn Write,
    pkgs: &mut BTreeMap<String, PackageRun>,
    pkg: &str,
    text: &str,
    error_pkg: &mut Option<String>,
) {
    for l in text.lines() {
        if let Some(p) = l.strip_prefix("# ") {
            // go 有两种头："# pkg"（编译）和 "# [pkg]"（test 内置 vet），
            // 后者去掉方括号，否则错误会归到不存在的包名下
            *error_pkg = Some(p.trim_start_matches('[').trim_end_matches(']').to_string());
        } else if let Some(d) = Diagnostic::parse_go_line(l, Level::Error) {
            // 测试二进制的编译错误，渲染成 rustc 风格诊断并计数
            let owner = error_pkg.clone().unwrap_or_else(|| pkg.to_string());
            pkgs.entry(owner).or_default().build_errors += 1;
            let _ = d.render(out);
            let _ = writeln!(out);
        } else if !is_summary_line(l) {
            // 编译阶段的其他输出原样透传
            let _ = writeln!(out, "{l}");
        }
    }
}

/// go 测试框架的标记行（出现在测试输出缓冲里）。
fn is_framework_line(l: &str) -> bool {
    [
        "=== RUN",
        "=== PAUSE",
        "=== CONT",
        "=== NAME",
        "--- PASS:",
        "--- FAIL:",
        "--- SKIP:",
    ]
    .iter()
    .any(|p| l.starts_with(p))
}

/// go 的包级汇总行："ok  \tpkg\t0.5s"、"FAIL\tpkg\t0.5s"、"PASS"/"FAIL" 等。
fn is_summary_line(l: &str) -> bool {
    l == "PASS"
        || l == "FAIL"
        || l.starts_with("ok\t")
        || l.starts_with("ok  \t")
        || l.starts_with("FAIL\t")
        || l.starts_with("?   \t")
}

/// 整块打印一个包的输出：Running 头 + 测试行 + failures 区块 + test result 行。
fn print_package_block(
    out: &mut dyn Write,
    pkg: &str,
    st: &PackageRun,
    ok: bool,
    elapsed: f64,
) -> io::Result<()> {
    if st.build_errors > 0 {
        writeln!(
            out,
            "\n{}: could not compile `{pkg}` (test) due to {} previous error{}\n",
            style::error_tag(),
            st.build_errors,
            if st.build_errors == 1 { "" } else { "s" }
        )?;
        return Ok(());
    }

    // cargo 会打印测试二进制路径，go 的 JSON 事件不提供，用包路径代替
    writeln!(
        out,
        "\n     {} unittests ({pkg})\n",
        style::status("Running")
    )?;
    for l in &st.lines {
        writeln!(out, "{l}")?;
    }

    if !st.failed_tests.is_empty() {
        writeln!(out, "\nfailures:\n")?;
        for name in &st.failed_tests {
            writeln!(out, "---- {name} stdout ----")?;
            if let Some(lines) = st.outputs.get(name) {
                for l in lines {
                    writeln!(out, "{l}")?;
                }
            }
            writeln!(out)?;
        }
        writeln!(out, "failures:")?;
        for name in &st.failed_tests {
            writeln!(out, "    {name}")?;
        }
    }

    // cargo 的 "0 measured; 0 filtered out" 对 go 没有意义，但保留以维持视觉一致
    let tag = if ok && st.failed == 0 {
        "ok".green().bold()
    } else {
        "FAILED".red().bold()
    };
    writeln!(
        out,
        "\ntest result: {tag}. {} passed; {} failed; {} ignored; 0 measured; 0 filtered out; finished in {}\n",
        st.passed,
        st.failed,
        st.skipped,
        format_elapsed(Duration::from_secs_f64(elapsed))
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PackageRun, base_import_path, handle_package_output, is_framework_line, is_summary_line,
        print_package_block, top_test_name,
    };
    use std::collections::BTreeMap;

    #[test]
    fn groups_nested_tests_and_strips_build_variant() {
        assert_eq!(top_test_name("TestHTTP/empty path"), "TestHTTP");
        assert_eq!(top_test_name(""), "");
        assert_eq!(
            base_import_path("example.com/p [example.com/p.test]"),
            "example.com/p"
        );
        assert_eq!(base_import_path("example.com/p"), "example.com/p");
    }

    #[test]
    fn recognizes_framework_and_summary_lines_at_boundaries() {
        assert!(is_framework_line("=== RUN   TestA"));
        assert!(is_framework_line("--- FAIL: TestA"));
        assert!(!is_framework_line("== RUN"));
        assert!(is_summary_line("ok  \texample.com/p\t0.01s"));
        assert!(is_summary_line("?   \texample.com/p\t[no test files]"));
        assert!(!is_summary_line("okay\texample.com/p"));
    }

    #[test]
    fn handles_compile_output_and_prints_build_error_block() {
        let mut packages = BTreeMap::new();
        let mut owner = None;
        let mut output = Vec::new();
        handle_package_output(
            &mut output,
            &mut packages,
            "fallback/pkg",
            "# [example.com/p]\nfile.go:4:2: bad\nok  \tignored\nplain compiler output",
            &mut owner,
        );
        assert_eq!(owner.as_deref(), Some("example.com/p"));
        assert_eq!(packages["example.com/p"].build_errors, 1);
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("plain compiler output")
        );

        let mut block = Vec::new();
        print_package_block(
            &mut block,
            "example.com/p",
            &PackageRun {
                build_errors: 2,
                ..Default::default()
            },
            false,
            0.0,
        )
        .unwrap();
        assert!(
            String::from_utf8(block)
                .unwrap()
                .contains("could not compile `example.com/p` (test) due to 2 previous errors")
        );
    }

    #[test]
    fn prints_failure_details_and_failed_result_summary() {
        let mut outputs = BTreeMap::new();
        outputs.insert("TestFail".into(), vec!["failure output".into()]);
        let state = PackageRun {
            passed: 1,
            failed: 1,
            skipped: 1,
            failed_tests: vec!["TestFail".into()],
            outputs,
            lines: vec!["test TestFail ... FAILED".into()],
            build_errors: 0,
        };
        let mut output = Vec::new();
        print_package_block(&mut output, "example.com/p", &state, false, 61.0).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("failures:"));
        assert!(output.contains("failure output"));
        assert!(output.contains("---- TestFail stdout ----"));
        assert!(output.contains("1 passed; 1 failed; 1 ignored"));
        assert!(output.contains("1m 01s"));
    }
}
