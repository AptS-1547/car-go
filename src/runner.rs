//! 外部进程的启动与输出读取。

use std::io::{BufRead, BufReader};
use std::process::{Command, ExitStatus, Stdio};

/// 构造 go 的 JSON 子命令参数：["<subcmd>", "-json", extra...]
pub fn json_args(subcmd: &str, extra: &[String]) -> Vec<String> {
    let mut v = Vec::with_capacity(extra.len() + 2);
    v.push(subcmd.to_string());
    v.push("-json".to_string());
    v.extend(extra.iter().cloned());
    v
}

/// 运行外部命令，stdout 逐行交给 `on_line` 处理。
///
/// stderr 继承父进程：命令自身的参数错误等会直接原样显示，
/// 这和我们"只替换结构化输出"的定位一致。
pub fn run(
    program: &str,
    args: &[String],
    mut on_line: impl FnMut(&str),
) -> std::io::Result<ExitStatus> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    // 上面已设置 piped，take 必然成功
    let stdout = child.stdout.take().expect("stdout 已设置为 piped");
    for line in BufReader::new(stdout).lines() {
        on_line(&line?);
    }
    child.wait()
}

/// 运行外部命令，stdout 和 stderr 的行都交给 `on_line`。
/// 用于 `go mod` 这类把进度信息写到 stderr 的命令；
/// stderr 由一个独立线程泵入和 stdout 相同的通道。
pub fn run_merged(
    program: &str,
    args: &[String],
    mut on_line: impl FnMut(&str),
) -> std::io::Result<ExitStatus> {
    use std::sync::mpsc;

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (tx, rx) = mpsc::channel::<std::io::Result<String>>();
    // stdout 和 stderr 各一个泵线程；tx 全部 drop 后 rx 自然结束
    let stdout = child.stdout.take().expect("stdout 已设置为 piped");
    let stderr = child.stderr.take().expect("stderr 已设置为 piped");
    pump(stdout, tx.clone());
    pump(stderr, tx.clone());
    drop(tx);

    while let Ok(line) = rx.recv() {
        on_line(&line?);
    }
    child.wait()
}

/// 启动一个线程，把 pipe 的每一行转发到通道。
fn pump(
    pipe: impl std::io::Read + Send + 'static,
    tx: std::sync::mpsc::Sender<std::io::Result<String>>,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(pipe).lines() {
            if tx.send(line).is_err() {
                break; // 接收端已关闭
            }
        }
    });
}

/// 提取进程退出码；被信号杀死（无退出码）时退化为 1。
pub fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{exit_code, json_args};

    #[test]
    fn json_args_puts_json_before_forwarded_arguments() {
        let args = vec!["./...".to_string(), "-run=Test/edge".to_string()];
        assert_eq!(
            json_args("test", &args),
            ["test", "-json", "./...", "-run=Test/edge"]
        );
    }

    #[test]
    fn json_args_handles_empty_arguments() {
        assert_eq!(json_args("build", &[]), ["build", "-json"]);
    }

    #[test]
    fn exit_code_preserves_normal_codes_and_maps_large_codes() {
        let status = std::process::Command::new("sh")
            .args(["-c", "exit 7"])
            .status()
            .unwrap();
        assert_eq!(exit_code(status), 7);

        let status = std::process::Command::new("sh")
            .args(["-c", "exit 255"])
            .status()
            .unwrap();
        assert_eq!(exit_code(status), 255);
    }
}
