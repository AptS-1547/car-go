//! 通过临时工具链进程验证真实 CLI 边界。
//!
//! 测试不依赖本机是否安装 Go、gofmt 或 staticcheck；每个用例都提供只实现
//! 所需协议的 fake 可执行文件，并检查参数、退出码和渲染结果。

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    root: PathBuf,
    bin: PathBuf,
    source_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("car-go-tests-{}-{nonce}", std::process::id()));
        let bin = root.join("bin");
        let source_dir = root.join("src");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("main.go"), "package main\nfunc main() {}\n").unwrap();
        Self {
            root,
            bin,
            source_dir,
        }
    }

    fn write_executable(&self, name: &str, body: &str) {
        let path = self.bin.join(name);
        let script = format!("#!/bin/sh\nset -eu\n{body}\n");
        fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
    }

    fn go(&self, body: &str) {
        let common = format!(
            r#"if [ "$1" = "list" ] && [ "$2" = "-m" ]; then
    printf '%s\n' 'example.com/car-go-fixture'
    exit 0
fi
if [ "$1" = "list" ] && [ "$2" = "-f" ]; then
    printf '%s\n' "$CAR_GO_SOURCE_DIR"
    exit 0
fi
{body}"#
        );
        self.write_executable("go", &common);
    }

    fn run(&self, args: &[&str]) -> Output {
        let binary = env!("CARGO_BIN_EXE_car-go");
        let path = format!("{}:/usr/bin:/bin", self.bin.display());
        Command::new(binary)
            .args(args)
            .env("PATH", path)
            .env("CAR_GO_SOURCE_DIR", &self.source_dir)
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(1)
}

#[test]
fn build_failure_renders_diagnostic_and_package_summary() {
    let fixture = Fixture::new();
    fixture.go(
        r##"case "$1" in
    build)
        printf '%s\n' 'not json build output'
        printf '%s\n' '{"Action":"build-output"}'
        printf '%s\n' '{"Action":"build-output","Output":"# example.com/p\nmissing.go:2:3: undefined: value\n"}'
        printf '%s\n' '{"Action":"build-fail","ImportPath":"example.com/p"}'
        printf '%s\n' '{"Action":"build-fail"}'
        printf '%s\n' '{"Action":"unknown"}'
        exit 1
        ;;
    *) exit 0 ;;
esac"##,
    );

    let output = fixture.run(&["build"]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(text.contains("Compiling example.com/car-go-fixture"));
    assert!(text.contains("error: undefined: value"));
    assert!(text.contains("--> missing.go:2:3"));
    assert!(text.contains("could not compile `example.com/p` due to 1 previous error"));
    assert!(!text.contains("Finished"));
}

#[test]
fn build_failure_without_build_fail_event_falls_back_to_diagnostic_packages() {
    let fixture = Fixture::new();
    fixture.go(
        r##"case "$1" in
    build)
        printf '%s\n' '{"Action":"build-output","Output":"# example.com/fallback\nmissing.go:1:1: compile error\n"}'
        exit 1
        ;;
    *) exit 0 ;;
esac"##,
    );
    let output = fixture.run(&["build"]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(text.contains("could not compile `example.com/fallback` due to 1 previous error"));
}

#[test]
fn test_events_buffer_failures_and_ignore_framework_output() {
    let fixture = Fixture::new();
    fixture.go(
        r##"case "$1" in
    test)
        printf '%s\n' 'not json test output'
        printf '%s\n' '{"Action":"build-output"}'
        printf '%s\n' '{"Action":"unknown"}'
        printf '%s\n' '{"Action":"unknown","Package":"example.com/p"}'
        printf '%s\n' '{"Action":"start","Package":"example.com/p"}'
        printf '%s\n' '{"Action":"output","Package":"example.com/p","Test":"TestFail","Output":"=== RUN   TestFail\nfailed stdout\n--- FAIL: TestFail\n"}'
        printf '%s\n' '{"Action":"output","Package":"example.com/p","Test":"TestPass"}'
        printf '%s\n' '{"Action":"fail","Package":"example.com/p","Test":"TestFail"}'
        printf '%s\n' '{"Action":"output","Package":"example.com/p","Test":"TestPass/sub","Output":"child output\n"}'
        printf '%s\n' '{"Action":"pass","Package":"example.com/p","Test":"TestPass/sub"}'
        printf '%s\n' '{"Action":"pass","Package":"example.com/p","Test":"TestPass"}'
        printf '%s\n' '{"Action":"skip","Package":"example.com/p","Test":"TestSkip"}'
        printf '%s\n' '{"Action":"fail","Package":"example.com/p","Elapsed":0.25}'
        printf '%s\n' '{"Action":"build-output","ImportPath":"example.com/build [example.com/build.test]","Output":"# [example.com/build]\ncompile.go:1:1: compile failed\nplain compiler output\nPASS\n"}'
        printf '%s\n' '{"Action":"start","Package":"example.com/build"}'
        printf '%s\n' '{"Action":"fail","Package":"example.com/build","Elapsed":0}'
        printf '%s\n' '{"Action":"start","Package":"example.com/runtime"}'
        printf '%s\n' '{"Action":"output","Package":"example.com/runtime","Output":"runtime.go:1:1: runtime compile issue\npanic output\nPASS\n"}'
        printf '%s\n' '{"Action":"pass","Package":"example.com/runtime","Elapsed":0}'
        exit 1
        ;;
    *) exit 0 ;;
esac"##,
    );

    let output = fixture.run(&["test", "./..."]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(text.contains("Finished"));
    assert!(text.contains("test TestFail ... FAILED"));
    assert!(text.contains("test TestPass ... ok"));
    assert!(text.contains("test TestSkip ... ignored"));
    assert!(text.contains("failed stdout"));
    assert!(!text.contains("=== RUN"));
    assert!(!text.contains("child output"));
    assert!(text.contains("1 passed; 1 failed; 1 ignored"));
    assert!(text.contains("---- TestFail stdout ----"));
    assert!(text.contains("could not compile `example.com/build` (test) due to 1 previous error"));
    assert!(
        text.contains("could not compile `example.com/runtime` (test) due to 1 previous error")
    );
    assert!(text.contains("not json test output"));
}

#[test]
fn vet_handles_json_diagnostics_and_malformed_position_fallback() {
    let fixture = Fixture::new();
    fixture.go(
        r##"case "$1" in
    vet)
        printf '%s\n' '{"example.com/p":{"printf":[{"posn":"missing.go:3:2","end":"missing.go:3:5","message":"bad format"}],"build":[{"posn":"not-a-position","message":"raw warning"}]}}'
        printf '%s\n' '# example.com/p' >&2
        printf '%s\n' 'vet: missing.go:4:2: vet compile error' >&2
        printf '%s\n' 'vet: malformed vet error' >&2
        printf '%s\n' '# [example.com/p]' >&2
        exit 0
        ;;
    *) exit 0 ;;
esac"##,
    );

    let output = fixture.run(&["vet"]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(text.contains("Checking example.com/p"));
    assert!(text.contains("warning[printf]: bad format"));
    assert!(text.contains("warning[build]: raw warning"));
    assert!(text.contains("error: vet compile error"));
    assert!(text.contains("error: malformed vet error"));
    assert!(text.contains("generated 2 warnings"));
}

#[test]
fn clippy_maps_warning_and_error_severity_without_finished_on_error() {
    let fixture = Fixture::new();
    fixture.go("case \"$1\" in *) exit 0 ;; esac");
    fixture.write_executable(
        "staticcheck",
        r#"printf '\n'
printf '%s\n' 'staticcheck raw output'
printf '%s\n' '{"code":"ST1000","severity":"warning","location":{"file":"missing.go","line":1,"column":1},"message":"package comment"}'
printf '%s\n' '{"code":"SA4006","severity":"error","location":{"file":"missing.go","line":2,"column":1},"end":{"file":"missing.go","line":2,"column":4},"message":"value unused"}'
exit 1"#,
    );

    let output = fixture.run(&["clippy", "./..."]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(text.contains("warning[ST1000]: package comment"));
    assert!(text.contains("staticcheck raw output"));
    assert!(text.contains("error: value unused"));
    assert!(text.contains("generated 1 warning"));
    assert!(
        text.contains("could not compile `example.com/car-go-fixture` due to 1 previous error")
    );
    assert!(!text.contains("Finished"));
}

#[test]
fn fmt_check_returns_one_for_diff_and_forwards_package_targets() {
    let fixture = Fixture::new();
    fixture.go("case \"$1\" in fmt) printf '%s\\n' 'main.go' ;; *) exit 0 ;; esac");
    fixture.write_executable(
        "gofmt",
        r#"printf '%s\n' 'diff main.go' '--- before' '+++ after'
exit 0"#,
    );

    let output = fixture.run(&["fmt", "--check", "./pkg"]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(
        text.contains("diff main.go"),
        "STDOUT=[{text}] STDERR=[{}]",
        String::from_utf8_lossy(&output.stderr)
    );

    let formatted = fixture.run(&["fmt", "./pkg"]);
    assert_eq!(code(&formatted), 0);
    assert!(Fixture::stdout(&formatted).contains("main.go"));
}

#[test]
fn package_commands_render_progress_and_remove_uses_none_suffix() {
    let fixture = Fixture::new();
    fixture.go(r#"case "$1" in
    get)
        printf '%s\n' 'go: downloading example.com/new v1.0.0' >&2
        printf '%s\n' 'go: removed example.com/old v1.0.0'
        printf '%s\n' "args: $*"
        exit 0
        ;;
    mod)
        printf '%s\n' 'go: upgraded example.com/mod v1.0.0 => v1.1.0' >&2
        printf '%s\n' "args: $*"
        exit 0
        ;;
    *) exit 0 ;;
esac"#);

    let remove = fixture.run(&["remove", "example.com/old"]);
    let remove_text = Fixture::stdout(&remove);
    assert_eq!(code(&remove), 0);
    assert!(remove_text.contains("Removing example.com/old v1.0.0"));
    assert!(remove_text.contains("args: get example.com/old@none"));
    assert!(remove_text.contains("Finished"));

    let add = fixture.run(&["add", "example.com/new@v1.0.0"]);
    let add_text = Fixture::stdout(&add);
    assert_eq!(code(&add), 0);
    assert!(add_text.contains("args: get example.com/new@v1.0.0"));

    let update = fixture.run(&["update"]);
    let update_text = Fixture::stdout(&update);
    assert_eq!(code(&update), 0);
    assert!(update_text.contains("Downloaded example.com/new"));
    assert!(update_text.contains("args: get -u ./..."));

    let update_one = fixture.run(&["update", "example.com/new"]);
    assert_eq!(code(&update_one), 0);
    assert!(Fixture::stdout(&update_one).contains("args: get -u example.com/new"));

    let tidy = fixture.run(&["mod", "tidy"]);
    let tidy_text = Fixture::stdout(&tidy);
    assert_eq!(code(&tidy), 0);
    assert!(tidy_text.contains("Upgrading example.com/mod"));
    assert!(tidy_text.contains("args: mod tidy -v"));

    let missing = fixture.run(&["mod"]);
    assert_eq!(code(&missing), 2);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("requires a subcommand"));

    let missing_add = fixture.run(&["add"]);
    assert_eq!(code(&missing_add), 2);
    assert!(String::from_utf8_lossy(&missing_add.stderr).contains("required"));
}

#[test]
fn clippy_reports_missing_staticcheck_tool() {
    let fixture = Fixture::new();
    fixture.go("exit 0");
    let binary = env!("CARGO_BIN_EXE_car-go");
    let output = Command::new(binary)
        .args(["clippy"])
        .env("PATH", &fixture.bin)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 1);
    assert!(text.contains("staticcheck not found"));
    assert!(text.contains("go install honnef.co/go/tools/cmd/staticcheck@latest"));
}

#[test]
fn clippy_reports_permission_errors_from_staticcheck() {
    let fixture = Fixture::new();
    fixture.go("exit 0");
    fs::write(fixture.bin.join("staticcheck"), "#!/bin/sh\nexit 0\n").unwrap();
    let output = fixture.run(&["clippy"]);
    assert_eq!(code(&output), 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to execute go command"));
}

#[test]
fn fmt_check_returns_go_list_failure_code() {
    let fixture = Fixture::new();
    fixture.write_executable(
        "go",
        r#"if [ "$1" = "list" ] && [ "$2" = "-f" ]; then
    printf '%s\n' 'go list failed' >&2
    exit 7
fi
exit 0"#,
    );
    let output = fixture.run(&["fmt", "--check"]);
    assert_eq!(code(&output), 7);
}

#[test]
fn vet_transmits_unparseable_json_when_no_diagnostics_exist() {
    let fixture = Fixture::new();
    fixture.go(r#"case "$1" in
    vet)
        printf '%s\n' 'not a vet json document'
        exit 0
        ;;
    *) exit 0 ;;
esac"#);
    let output = fixture.run(&["vet"]);
    let text = Fixture::stdout(&output);
    assert_eq!(code(&output), 0);
    assert!(text.contains("not a vet json document"));
}

#[test]
fn main_reports_external_command_errors() {
    let fixture = Fixture::new();
    let binary = env!("CARGO_BIN_EXE_car-go");
    let output = Command::new(binary)
        .args(["build"])
        .env("PATH", &fixture.bin)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(code(&output), 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to execute go command"));
}

#[test]
fn successful_commands_cover_clean_build_test_vet_fmt_and_clippy_paths() {
    let fixture = Fixture::new();
    fixture.go(r#"case "$1" in
    build)
        printf '%s\n' '{"Action":"build-output","Output":"build ok\n"}'
        exit 0
        ;;
    test)
        printf '%s\n' '{"Action":"start","Package":"example.com/p"}'
        printf '%s\n' '{"Action":"pass","Package":"example.com/p","Test":"TestOK"}'
        printf '%s\n' '{"Action":"pass","Package":"example.com/p","Elapsed":0.01}'
        exit 0
        ;;
    vet|fmt)
        exit 0
        ;;
    *) exit 0 ;;
esac"#);
    fixture.write_executable("staticcheck", "exit 0");
    fixture.write_executable("gofmt", "exit 0");

    let build = fixture.run(&["build"]);
    assert_eq!(code(&build), 0);
    assert!(Fixture::stdout(&build).contains("Finished"));

    let test = fixture.run(&["test"]);
    let test_text = Fixture::stdout(&test);
    assert_eq!(code(&test), 0);
    assert!(test_text.contains("test TestOK ... ok"));
    assert!(test_text.contains("test result: ok"));

    let vet = fixture.run(&["vet"]);
    assert_eq!(code(&vet), 0);
    assert!(Fixture::stdout(&vet).contains("Finished"));

    let fmt = fixture.run(&["fmt", "--check"]);
    assert_eq!(code(&fmt), 0);
    assert!(Fixture::stdout(&fmt).is_empty());

    let clippy = fixture.run(&["clippy"]);
    assert_eq!(code(&clippy), 0);
    assert!(Fixture::stdout(&clippy).contains("Finished"));
}
