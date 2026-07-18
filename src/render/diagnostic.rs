//! rustc 风格的诊断渲染器。
//!
//! 把 go 的 `file:line:col: message` 错误文本渲染成带源码片段和
//! `^^^` 标注的形式，还原 cargo/rustc 的观感。

use std::io;

use colored::Colorize;

use super::style;

/// 诊断级别：go 编译错误是 error，vet 发现是 warning。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

/// 一条编译/静态检查诊断。
#[derive(Debug)]
pub struct Diagnostic {
    pub level: Level,
    /// 诊断代码（vet 的分析器名，如 `printf`）；编译错误没有
    pub code: Option<String>,
    pub file: String,
    pub line: usize,
    /// 1-based 列号；部分 go 错误只有行号
    pub col: Option<usize>,
    /// 1-based 结束列号（vet 提供），用于精确计算下划线长度
    pub end_col: Option<usize>,
    pub message: String,
}

impl Diagnostic {
    /// 解析 go 的错误行："path/file.go:6:14: message" 或 "path/file.go:6: message"。
    /// 不是错误行时返回 None。
    pub fn parse_go_line(text: &str, level: Level) -> Option<Diagnostic> {
        // 从头扫描每个 ':'，第一个满足 ":行号[:列号]: " 模式的位置即为文件路径的结尾。
        // 正向扫描比从右往左拆更稳：消息里可能含数字和冒号（如 "expected 2, got 3"）。
        // 手写解析避免引入 regex 依赖。
        for (i, &b) in text.as_bytes().iter().enumerate() {
            if b != b':' || i == 0 {
                continue;
            }
            let rest = &text[i + 1..];
            let line_digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
            if line_digits == 0 {
                continue;
            }
            let line: usize = rest[..line_digits].parse().ok()?;
            let after_line = &rest[line_digits..];

            // 可选的 ":列号"
            let (col, after_col) = match after_line.strip_prefix(':') {
                Some(s) => {
                    let col_digits = s.bytes().take_while(|b| b.is_ascii_digit()).count();
                    if col_digits == 0 {
                        (None, after_line)
                    } else {
                        (s[..col_digits].parse().ok(), &s[col_digits..])
                    }
                }
                None => (None, after_line),
            };

            // 位置之后必须是 ": "，否则说明这个冒号不是路径分隔符，继续找
            let Some(message) = after_col.strip_prefix(": ") else {
                continue;
            };

            return Some(Diagnostic {
                level,
                code: None,
                file: text[..i].to_string(),
                line,
                col,
                end_col: None,
                message: message.trim_end().to_string(),
            });
        }
        None
    }

    /// 渲染到 out。结尾不带空行，由调用方控制诊断之间的间距。
    pub fn render(&self, out: &mut dyn io::Write) -> io::Result<()> {
        // 标题行：error: msg / warning[code]: msg（消息本身加粗，与 rustc 一致）
        let tag = match self.level {
            Level::Error => style::error_tag(),
            Level::Warning => style::warning_tag(self.code.as_deref()),
        };
        writeln!(out, "{tag}: {}", self.message.bold())?;

        // 位置行： --> file:line[:col]；文件名为空说明是工具链级错误（无位置），跳过
        if !self.file.is_empty() {
            let pos = match self.col {
                Some(col) => format!("{}:{}:{}", self.file, self.line, col),
                None => format!("{}:{}", self.file, self.line),
            };
            writeln!(out, " {} {pos}", style::gutter("-->"))?;
        }

        // 源码片段：文件读不到（比如路径是临时的）就跳过，不影响诊断本身
        if !self.file.is_empty()
            && let Some(src) = read_source_line(&self.file, self.line)
        {
            let num = self.line.to_string();
            let pad = " ".repeat(num.len());
            let bar = style::gutter("|");
            writeln!(out, "{pad} {bar}")?;
            writeln!(out, "{} {bar} {src}", style::gutter(&num))?;

            if let Some(col) = self.col {
                let caret = match self.level {
                    Level::Error => "^".repeat(self.underline_span(&src, col)).red().bold(),
                    Level::Warning => "^".repeat(self.underline_span(&src, col)).yellow().bold(),
                };
                // go 的列号是 1-based 字节偏移；转换成字符数对齐（CJK 宽字符会有偏差，可接受）
                let col0 = col.saturating_sub(1).min(src.len());
                let spaces = " ".repeat(src[..col0].chars().count());
                writeln!(out, "{pad} {bar} {spaces}{caret}")?;
            }
        }
        Ok(())
    }

    /// 计算下划线长度（字符数）。
    fn underline_span(&self, src: &str, col: usize) -> usize {
        // vet 给了结束位置就直接用（最精确）
        if let (Some(end), Some(start)) = (self.end_col, self.col)
            && end > start
        {
            return end - start;
        }
        let col0 = col.saturating_sub(1).min(src.len());
        let rest = &src[col0..];
        let line_chars = rest.chars().count();
        let span = match rest.chars().next() {
            // 字符串字面量：划到配对的结束引号（处理转义）
            Some('"') => scan_string_literal(rest),
            // 标识符/数字：划整个单词
            Some(c) if c.is_alphanumeric() || c == '_' => rest
                .chars()
                .take_while(|&c| c.is_alphanumeric() || c == '_')
                .count(),
            // 其他情况只划一个字符
            _ => 1,
        };
        span.clamp(1, line_chars.max(1))
    }
}

/// 解析 vet 的 "file:line:col" 位置串（位置格式规整，从右往左拆即可）。
pub fn parse_posn(text: &str) -> Option<(String, usize, usize)> {
    let (head, col) = text.rsplit_once(':')?;
    let (file, line) = head.rsplit_once(':')?;
    Some((file.to_string(), line.parse().ok()?, col.parse().ok()?))
}

/// 读取源文件的指定行（1-based）。读不到返回 None。
fn read_source_line(file: &str, line: usize) -> Option<String> {
    if line == 0 {
        return None;
    }
    let content = std::fs::read_to_string(file).ok()?;
    content
        .lines()
        .nth(line - 1)
        .map(|s| s.trim_end().to_string())
}

/// 计算字符串字面量的长度（字符数，含两端引号）。
fn scan_string_literal(rest: &str) -> usize {
    let mut len = 1; // 开引号
    let mut escaped = false;
    for c in rest[1..].chars() {
        len += 1;
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            break;
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Level, parse_posn, read_source_line, scan_string_literal};

    fn source_file(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "car-go-diagnostic-{}-{}.go",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn parses_paths_with_colons_and_optional_columns() {
        let d =
            Diagnostic::parse_go_line("C:/work/main.go:12:4: bad: value", Level::Error).unwrap();
        assert_eq!(d.file, "C:/work/main.go");
        assert_eq!(d.line, 12);
        assert_eq!(d.col, Some(4));
        assert_eq!(d.message, "bad: value");

        let d = Diagnostic::parse_go_line("main.go:9: syntax error", Level::Error).unwrap();
        assert_eq!(d.col, None);
        assert_eq!(d.line, 9);
    }

    #[test]
    fn rejects_malformed_or_zero_width_locations() {
        assert!(Diagnostic::parse_go_line("not a diagnostic", Level::Error).is_none());
        assert!(Diagnostic::parse_go_line("main.go:x bad", Level::Error).is_none());
        assert!(Diagnostic::parse_go_line("main.go:1 message", Level::Error).is_none());
        assert!(parse_posn("main.go:line:col").is_none());
        assert!(parse_posn("main.go:0:0").is_some());
    }

    #[test]
    fn parses_absolute_and_windows_like_positions_from_the_right() {
        assert_eq!(
            parse_posn("/tmp/a:b/main.go:10:22"),
            Some(("/tmp/a:b/main.go".to_string(), 10, 22))
        );
    }

    #[test]
    fn string_scanner_stops_at_unescaped_quote() {
        assert_eq!(scan_string_literal(r#""a\"b" tail"#), 6);
        assert_eq!(scan_string_literal("\"unterminated"), 13);
    }

    #[test]
    fn render_skips_missing_source_without_losing_diagnostic() {
        let d = Diagnostic {
            level: Level::Warning,
            code: Some("SA0001".into()),
            file: "/path/that/does/not/exist.go".into(),
            line: 4,
            col: Some(2),
            end_col: Some(5),
            message: "problem".into(),
        };
        let mut output = Vec::new();
        d.render(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("warning[SA0001]: problem"));
        assert!(output.contains("--> /path/that/does/not/exist.go:4:2"));
        assert!(!output.contains("source"));
    }

    #[test]
    fn render_covers_source_snippets_word_strings_punctuation_and_missing_columns() {
        let path = source_file("value = \"abc\\\"def\"\n!x\n");

        let cases = [
            Diagnostic {
                level: Level::Error,
                code: None,
                file: path.display().to_string(),
                line: 1,
                col: Some(1),
                end_col: None,
                message: "word".into(),
            },
            Diagnostic {
                level: Level::Warning,
                code: Some("printf".into()),
                file: path.display().to_string(),
                line: 1,
                col: Some(9),
                end_col: None,
                message: "string".into(),
            },
            Diagnostic {
                level: Level::Error,
                code: None,
                file: path.display().to_string(),
                line: 2,
                col: Some(1),
                end_col: None,
                message: "punctuation".into(),
            },
            Diagnostic {
                level: Level::Warning,
                code: None,
                file: path.display().to_string(),
                line: 2,
                col: Some(1),
                end_col: Some(3),
                message: "explicit span".into(),
            },
            Diagnostic {
                level: Level::Warning,
                code: None,
                file: path.display().to_string(),
                line: 1,
                col: None,
                end_col: None,
                message: "without column".into(),
            },
        ];

        let mut output = Vec::new();
        for diagnostic in cases {
            diagnostic.render(&mut output).unwrap();
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("value = \"abc\\\"def\""));
        assert!(output.contains("word"));
        assert!(output.contains("string"));
        assert!(output.contains("punctuation"));
        assert!(output.contains("explicit span"));
        assert!(output.contains("without column"));
        assert!(read_source_line(&path.display().to_string(), 0).is_none());
        assert!(read_source_line(&path.display().to_string(), 99).is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_without_file_skips_location_and_source() {
        let diagnostic = Diagnostic {
            level: Level::Error,
            code: None,
            file: String::new(),
            line: 0,
            col: None,
            end_col: None,
            message: "toolchain failure".into(),
        };
        let mut output = Vec::new();
        diagnostic.render(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("error: toolchain failure"));
        assert!(!output.contains("-->"));
    }
}
