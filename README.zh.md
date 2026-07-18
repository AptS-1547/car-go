# car-go

把 Go 工具链的终端体验改造成 Cargo 风格的命令行包装器。

[English README](README.md)

`car-go` 不替换 Go 编译器、测试框架或包管理器。它调用本机已经安装的 `go`、`gofmt` 和可选的 `staticcheck`，解析结构化事件或诊断，再渲染成更接近 Rust/Cargo 的状态行、源码片段、错误标注和测试汇总。

## 特点

- `go build` 输出 `Compiling`、`Finished` 和 rustc 风格编译诊断
- `go test` 输出按包分组的 `Running unittests`、测试结果和失败 stdout
- `go vet` 输出带分析器名称的 `warning[analyzer]` 诊断
- `go fmt --check` 提供类似 `cargo fmt --check` 的差异检查
- `staticcheck` 通过 `car-go clippy` 以 Cargo Clippy 风格展示
- `go mod` 的下载、增删、升级消息重排为 Cargo 风格
- `add`、`remove`、`update` 提供接近 `cargo add/remove/update` 的快捷入口
- 子命令参数尽量原样转发，通常保留底层工具退出码；发现诊断时按对应命令语义返回失败
- 纯 Rust 单文件二进制，默认启用 LTO、单代码生成单元、符号剥离和 `panic = abort`

## 命令对照

| car-go | 实际调用 | 说明 |
| --- | --- | --- |
| `car-go build ...` | `go build -json ...` | 编译事件和错误诊断 |
| `car-go test ...` | `go test -json ...` | Cargo 风格测试块和汇总 |
| `car-go vet ...` | `go vet -json ...` | 源码位置和分析器警告 |
| `car-go fmt ...` | `go fmt ...` | 默认格式化整个模块 |
| `car-go fmt --check ...` | `go list` + `gofmt -d` | 只检查格式，不修改文件 |
| `car-go clippy ...` | `staticcheck -f json ...` | 需要单独安装 Staticcheck |
| `car-go mod tidy` | `go mod tidy -v` | 自动补充 `-v` 以显示进度 |
| `car-go mod ...` | `go mod ...` | 保留 Go 的其他模块子命令 |
| `car-go add PKG...` | `go get PKG...` | 添加或升级依赖 |
| `car-go remove PKG...` | `go get PKG@none...` | 按 Go 官方语法移除依赖 |
| `car-go update [PKG...]` | `go get -u [PKG...]` | 无参数时使用 `./...` |

所有额外参数都会传给对应的底层工具。例如：

```bash
car-go build ./...
car-go test ./... -run TestUser -count=1
car-go vet ./...
car-go fmt --check ./...
car-go clippy ./...
car-go mod tidy
car-go add github.com/example/project@v1.2.3
```

## 安装

### 从源码安装

需要 Rust `1.88.0` 或更高版本，以及本机可执行的 Go 工具链。

```bash
git clone https://github.com/AptS-1547/car-go.git
cd car-go
cargo install --path .
```

安装完成后，`car-go` 会被放到 Cargo 的 bin 目录。确认该目录已经加入 `PATH`，然后运行：

```bash
car-go --version
car-go --help
```

也可以不安装，直接在仓库内运行：

```bash
cargo run -- test ./...
```

### Staticcheck

只有使用 `car-go clippy` 时才需要 Staticcheck：

```bash
go install honnef.co/go/tools/cmd/staticcheck@latest
```

确保 `$(go env GOPATH)/bin` 在 `PATH` 中。`car-go clippy` 会自行调用 `staticcheck -f json`，不需要额外配置文件。

## 使用要求与边界

- `go` 必须在 `PATH` 中；`build`、`test`、`vet`、`fmt` 和 `mod` 直接调用它
- `gofmt` 必须可用；`fmt --check` 会调用它
- `clippy` 子命令对应的是 Staticcheck，不是 Go 官方的 `go vet`
- car-go 不改变 Go 的构建、测试、格式化或依赖解析语义，只改变可读输出
- `go test` 的包是并行执行的，car-go 会先缓存包级输出，再按包打印测试块
- 复杂的链接器错误、工具链异常输出或无法识别的诊断会原样透传

## 开发

```bash
cargo fmt --all
cargo check
cargo test
cargo run -- --help
```

代码按职责分为：

```text
src/cli.rs                命令行参数和子命令定义
src/runner.rs             外部进程启动、输出泵和退出码处理
src/event.rs              Go JSON / Staticcheck 诊断协议类型
src/render/               各子命令的 Cargo 风格渲染器
src/render/diagnostic.rs  通用源码位置和诊断渲染
```

新增渲染逻辑时，优先复用 `runner`、`event` 和 `diagnostic` 中已有的边界，不要在每个子命令里重新实现进程管理、退出码处理或位置解析。

## 项目状态

这是一个起因于玩笑的实验性项目，不是对长期维护、稳定接口或持续发布的承诺。项目可能继续迭代，也可能长期停留在当前状态，甚至停止维护。命令行接口和输出格式可能继续调整，尤其是不同 Go 版本返回的 JSON 事件细节。提交问题时请附上 `car-go --version`、`go version`、完整命令和经过脱敏的输出。

## 参与贡献

请先阅读 [贡献指南](CONTRIBUTING.md)、[行为准则](CODE_OF_CONDUCT.md)、[安全策略](SECURITY.md) 和 [支持与提问](SUPPORT.md)。Bug、功能建议和文档问题可以通过 GitHub Issue 提交；安全问题请走私下报告渠道，不要公开发布复现细节。

## 许可证

本项目使用 [MIT License](LICENSE)。版权所有 `2026 AptS-1547`。
