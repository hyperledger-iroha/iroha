---
lang: zh-hans
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito SwiftUI 演示贡献者指南

本文档记录了针对 SwiftUI 演示运行所需的手动设置步骤
本地 Torii 节点和模拟账本。它通过以下方式补充 `docs/norito_bridge_release.md`
专注于日常开发任务。有关集成的更深入演练
Norito 将堆栈桥接/连接到 Xcode 项目，请参阅 `docs/connect_swift_integration.md`。

## 环境设置

1. 安装 `rust-toolchain.toml` 中定义的 Rust 工具链。
2. 在 macOS 上安装 Swift 5.7+ 和 Xcode 命令行工具。
3. （可选）安装 [SwiftLint](https://github.com/realm/SwiftLint) 以进行 linting。
4. 运行 `cargo build -p irohad` 以确保节点在您的主机上编译。
5. 将 `examples/ios/NoritoDemoXcode/Configs/demo.env.example` 复制到 `.env` 并调整
   与您的环境相匹配的价值观。应用程序在启动时读取这些变量：
   - `TORII_NODE_URL` — 基本 REST URL（WebSocket URL 源自它）。
   - `CONNECT_SESSION_ID` — 32 字节会话标识符 (base64/base64url)。
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — `/v1/connect/session` 返回的令牌。
   - `CONNECT_CHAIN_ID` — 控制握手期间宣布的链标识符。
   - `CONNECT_ROLE` — 在 UI 中预先选择的默认角色（`app` 或 `wallet`）。
   - 用于手动测试的可选助手：`CONNECT_PEER_PUB_B64`、`CONNECT_SHARED_KEY_B64`、
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`。

## 引导 Torii + 模拟账本

该存储库提供了帮助程序脚本，这些脚本启动带有内存中分类帐预置的 Torii 节点。
加载模拟账户：

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

该脚本发出：

- Torii 节点记录到 `artifacts/torii.log`。
- 分类帐指标（Prometheus 格式）至 `artifacts/metrics.prom`。
- `artifacts/torii.jwt` 的客户端访问令牌。

`start.sh` 使演示对等点保持运行，直到您按 `Ctrl+C`。它写了一个就绪状态
`artifacts/ios_demo_state.json` 的快照（其他工件的真实来源），
复制活动的 Torii 标准输出日志，轮询 `/metrics` 直到 Prometheus 刮取
可用，并将配置的帐户渲染为 `torii.jwt`（包括私钥
当配置提供它们时）。该脚本接受 `--artifacts` 来覆盖输出
目录，`--telemetry-profile` 匹配自定义 Torii 配置，以及
`--exit-after-ready` 用于非交互式 CI 作业。

`SampleAccounts.json` 中的每个条目都支持以下字段：

- `name`（字符串，可选）— 存储为帐户元数据 `alias`。
- `public_key`（多重哈希字符串，必需）— 用作帐户签名人。
- `private_key`（可选）— 包含在 `torii.jwt` 中，用于生成客户端凭证。
- `domain`（可选）— 如果省略，则默认为资产域。
- `asset_id`（字符串，必需）— 为账户铸造的资产定义。
- `initial_balance`（字符串，必需）— 铸造到帐户中的数字金额。

## 运行 SwiftUI 演示

1. 按照 `docs/norito_bridge_release.md` 中所述构建 XCFramework 并将其捆绑
   进入演示项目（项目中的引用预计为 `NoritoBridge.xcframework`
   根）。
2. 在 Xcode 中打开 `NoritoDemoXcode` 项目。
3. 选择 `NoritoDemo` 方案并定位 iOS 模拟器或设备。
4. 确保通过方案的环境变量引用 `.env` 文件。
   填充 `/v1/connect/session` 导出的 `CONNECT_*` 值，以便 UI 为
   应用程序启动时预先填充。
5. 验证硬件加速默认值：`App.swift` 调用
   `DemoAccelerationConfig.load().apply()` 因此演示会选择
   `NORITO_ACCEL_CONFIG_PATH` 环境覆盖或捆绑
   `acceleration.{json,toml}`/`client.{json,toml}` 文件。删除/调整这些输入，如果您
   想要在运行之前强制 CPU 回退。
6. 构建并启动应用程序。如果没有，主屏幕会提示输入 Torii URL/令牌
   已通过 `.env` 设置。
7. 启动“连接”会话以订阅帐户更新或批准请求。
8. 提交 IRH 传输并检查屏幕上的日志输出以及 Torii 日志。

### 硬件加速切换（金属/霓虹灯）

`DemoAccelerationConfig` 镜像 Rust 节点配置，以便开发人员可以练习
没有硬编码阈值的金属/NEON 路径。加载器搜索以下内容
发射地点：

1. `NORITO_ACCEL_CONFIG_PATH`（在 `.env`/scheme 参数中定义）— 绝对路径或
   `tilde` 扩展的指向 `iroha_config` JSON/TOML 文件的指针。
2. 名为 `acceleration.{json,toml}` 或 `client.{json,toml}` 的捆绑配置文件。
3. 如果两个源都不可用，则保留默认设置 (`AccelerationSettings()`)。

示例 `acceleration.toml` 片段：

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

保留字段 `nil` 会继承工作区默认值。负数被忽略，
缺少 `[accel]` 部分则回退到确定性 CPU 行为。当运行于
没有金属支撑的模拟器，桥会默默地保持标量路径，即使
配置请求金属。

## 集成测试

- 集成测试驻留在 `Tests/NoritoDemoTests` 中（待 macOS CI 启动后添加）
  可用）。
- 使用上面的脚本测试启动 Torii 并执行 WebSocket 订阅、令牌
  余额，并通过 Swift 包转移流量。
- 测试运行的日志与指标一起存储在 `artifacts/tests/<timestamp>/` 中
  样本分类账转储。

## CI 奇偶校验

- 在发送涉及演示或共享装置的 PR 之前运行 `make swift-ci`。的
  目标执行夹具奇偶校验，验证仪表板提要，并呈现
  本地总结。在 CI 中，相同的工作流程取决于 Buildkite 元数据
  (`ci/xcframework-smoke:<lane>:device_tag`) 因此仪表板可以将结果归因于
  正确的模拟器或 StrongBox 通道 - 如果您调整，请验证元数据是否存在
  管道或代理标签。
- 当 `make swift-ci` 失败时，请按照 `docs/source/swift_parity_triage.md` 中的步骤操作
  并查看渲染的 `mobile_ci` 输出以确定需要哪个通道
  再生或事件后续。

## 故障排除

- 如果演示无法连接到 Torii，请验证节点 URL 和 TLS 设置。
- 确保 JWT 令牌（如果需要）有效且未过期。
- 检查 `artifacts/torii.log` 是否有服务器端错误。
- 对于 WebSocket 问题，请检查客户端日志窗口或 Xcode 控制台输出。