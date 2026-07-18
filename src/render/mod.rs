//! 渲染层：把 go 的事件流转成 cargo 风格的终端输出。

pub mod build;
pub mod clippy;
pub mod diagnostic;
pub mod fmt;
pub mod gomod;
pub mod style;
pub mod test;
pub mod vet;

use std::time::Duration;

/// 获取当前 Go 模块路径（`go list -m`），失败时退化为目录名。
/// cargo 会显示 `Compiling foo v0.1.0 (/path)`，go 没有版本概念，这里只取模块名。
pub fn module_path() -> String {
    let output = std::process::Command::new("go")
        .args(["list", "-m"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "unknown".into()),
    }
}

/// 格式化耗时为 cargo 风格："0.72s"，超过一分钟用 "1m 03s"。
pub fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs >= 60.0 {
        format!("{}m {:02}s", d.as_secs() / 60, d.as_secs() % 60)
    } else {
        format!("{secs:.2}s")
    }
}

/// 当前工作目录的显示串，用于 Compiling 行末尾的路径标注。
pub fn current_dir_display() -> String {
    std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{current_dir_display, format_elapsed, module_path};
    use std::time::Duration;

    #[test]
    fn formats_subminute_and_minute_durations() {
        assert_eq!(format_elapsed(Duration::from_millis(5)), "0.01s");
        assert_eq!(format_elapsed(Duration::from_secs(59)), "59.00s");
        assert_eq!(format_elapsed(Duration::from_secs(63)), "1m 03s");
    }

    #[test]
    fn exposes_current_directory_and_a_nonempty_module_fallback_or_path() {
        assert!(!current_dir_display().is_empty());
        assert!(!module_path().is_empty());
    }
}
