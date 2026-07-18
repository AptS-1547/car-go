//! cargo/rustc 风格的终端配色，集中管理方便统一调整。

use colored::{ColoredString, Colorize};

/// cargo 的状态词（Compiling / Finished / Running / Checking）：绿色加粗
pub fn status(word: &str) -> ColoredString {
    word.green().bold()
}

/// error 标签：红色加粗
pub fn error_tag() -> ColoredString {
    "error".red().bold()
}

/// warning 标签：黄色加粗；可带诊断代码（如 vet 的分析器名）
pub fn warning_tag(code: Option<&str>) -> ColoredString {
    match code {
        Some(c) => format!("warning[{c}]").yellow().bold(),
        None => "warning".yellow().bold(),
    }
}

/// 诊断中的位置标记（`-->`、`|`、行号）：青色加粗
pub fn gutter(s: &str) -> ColoredString {
    s.cyan().bold()
}

/// 测试通过标记：绿色 ok
pub fn ok_tag() -> ColoredString {
    "ok".green()
}

/// 测试失败标记：红色加粗 FAILED
pub fn failed_tag() -> ColoredString {
    "FAILED".red().bold()
}

/// 测试忽略标记：黄色 ignored
pub fn ignored_tag() -> ColoredString {
    "ignored".yellow()
}
