//! Go 工具链 `-json` 输出的协议类型定义。
//!
//! `go build -json` 和 `go test -json` 都是 NDJSON（每行一个 JSON 事件），
//! 两种事件结构可以统一：build 事件没有 Time/Test/Elapsed，
//! test 事件没有 ImportPath，全部用 Option 兜住差异。

use serde::Deserialize;

/// `go build -json` / `go test -json` 共用的单行事件。
#[derive(Debug, Deserialize)]
pub struct GoEvent {
    /// 事件时间（test 专有，目前渲染用不到，留着方便排查）
    #[serde(rename = "Time")]
    #[allow(dead_code)]
    pub time: Option<String>,

    /// 事件动作：start/run/pause/cont/pass/fail/skip/output（test），
    /// 或 build-output/build-fail（build）
    #[serde(rename = "Action")]
    pub action: String,

    /// 所属包（test 事件）
    #[serde(rename = "Package")]
    pub package: Option<String>,

    /// 所属测试名；子测试形如 "TestFoo/sub"
    #[serde(rename = "Test")]
    pub test: Option<String>,

    /// 耗时（秒），出现在 pass/fail 事件上
    #[serde(rename = "Elapsed")]
    pub elapsed: Option<f64>,

    /// 一行原始输出（output / build-output 事件）
    #[serde(rename = "Output")]
    pub output: Option<String>,

    /// 所属包（build 事件，等价于 test 的 Package）
    #[serde(rename = "ImportPath")]
    pub import_path: Option<String>,

    /// 编译失败的包（test 的包级 fail 事件，形如 "pkg [pkg.test]"）
    #[serde(rename = "FailedBuild")]
    #[allow(dead_code)]
    pub failed_build: Option<String>,
}

/// `go vet -json` 输出的单个诊断条目。
/// 注意 vet 输出**不是** NDJSON，而是一个完整的 JSON 文档。
#[derive(Debug, Deserialize)]
pub struct VetDiagnostic {
    /// 起始位置，形如 "file.go:6:14"（可能是绝对路径）
    pub posn: String,
    /// 结束位置，形如 "file.go:6:16"；可用来精确计算下划线长度
    pub end: Option<String>,
    /// 诊断消息
    pub message: String,
}

/// vet 输出的顶层结构：包路径 -> 分析器名 -> 诊断列表。
/// 用 BTreeMap 保证输出顺序稳定（按字典序），结果可复现。
pub type VetReport =
    std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<VetDiagnostic>>>;

/// `staticcheck -f json` 的单行诊断（NDJSON）。
#[derive(Debug, Deserialize)]
pub struct StaticcheckDiag {
    /// 检查代码，如 "ST1000"、"SA4006"，渲染成 warning[code]
    pub code: String,
    /// "error" / "warning" / "ignored"，映射到诊断级别
    pub severity: Option<String>,
    /// 起始位置
    pub location: ScLocation,
    /// 结束位置（精确下划线用）
    pub end: Option<ScLocation>,
    pub message: String,
}

/// staticcheck 的位置结构。
#[derive(Debug, Deserialize)]
pub struct ScLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}
