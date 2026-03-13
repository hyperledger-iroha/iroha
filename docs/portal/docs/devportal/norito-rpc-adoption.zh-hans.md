---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-12-29T18:16:35.105044+00:00"
translation_last_reviewed: 2026-02-07
id: norito-rpc-adoption
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
---

> 规范的规划说明位于 `docs/source/torii/norito_rpc_adoption_schedule.md` 中。  
> 此门户副本提炼了 SDK 作者、运营商和审阅者的推出期望。

## 目标

- 在 AND4 生产切换之前对齐二进制 Norito-RPC 传输上的每个 SDK（Rust CLI、Python、JavaScript、Swift、Android）。
- 保持阶段门、证据包和遥测挂钩的确定性，以便治理可以审核部署。
- 使用路线图 NRPC-4 调用的共享助手来轻松捕获固定装置和金丝雀证据。

## 阶段时间线

|相|窗口|范围 |退出标准 |
|--------|--------|--------|---------------|
| **P0 – 实验室平价** | 2025 年第二季度 | Rust CLI + Python Smoke 套件在 CI 中运行 `/v2/norito-rpc`，JS 帮助程序通过单元测试，Android 模拟线束练习双重传输。 | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` 和 `javascript/iroha_js/test/noritoRpcClient.test.js` 在 CI 中呈绿色； Android 线束连接至 `./gradlew test`。 |
| **P1 – SDK 预览** | Q32025 |签入共享夹具包，`scripts/run_norito_rpc_fixtures.sh --sdk <label>` 在 `artifacts/norito_rpc/` 中记录日志 + JSON，SDK 示例中公开可选的 Norito 传输标志。 |已签名的夹具清单、自述文件更新显示选择加入的使用情况、IOS2 标志后面可用的 Swift 预览 API。 |
| **P2 – 分期/AND4 预览** | Q12026 |暂存 Torii 池更喜欢 Norito，Android AND4 预览客户端和 Swift IOS2 奇偶校验套件默认为二进制传输，填充遥测仪表板 `dashboards/grafana/torii_norito_rpc_observability.json`。 | `docs/source/torii/norito_rpc_stage_reports.md` 捕获金丝雀，`scripts/telemetry/test_torii_norito_rpc_alerts.sh` 通过，Android 模拟线束重放捕获成功/错误情况。 |
| **P3 – 正式发布** | Q42026 | Norito 成为所有 SDK 的默认传输； JSON 仍然是一种停电后备方案。发布作业归档每个标签的奇偶工件。 |发布 Rust/JS/Python/Swift/Android 的清单包 Norito 烟雾输出；强制执行 Norito 与 JSON 错误率 SLO 的警报阈值； `status.md` 和发行说明引用了 GA 证据。 |

## SDK 可交付成果和 CI 挂钩

- **Rust CLI 和集成工具** – 扩展 `iroha_cli pipeline` 烟雾测试，以在 `cargo xtask norito-rpc-verify` 落地后强制 Norito 传输。使用 `cargo test -p integration_tests -- norito_streaming`（实验室）和 `cargo xtask norito-rpc-verify`（登台/GA）进行防护，将工件存储在 `artifacts/norito_rpc/` 下。
- **Python SDK** – 默认释放烟雾 (`python/iroha_python/scripts/release_smoke.sh`) 为 Norito RPC，保留 `run_norito_rpc_smoke.sh` 作为 CI 入口点，并在 `python/iroha_python/README.md` 中进行文档奇偶校验处理。 CI 目标：`PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`。
- **JavaScript SDK** – 稳定 `NoritoRpcClient`，让治理/查询助手在 `toriiClientConfig.transport.preferred === "norito_rpc"` 时默认为 Norito，并捕获 `javascript/iroha_js/recipes/` 中的端到端样本。 CI 必须在发布之前运行 `npm test` 以及 dockerized `npm run test:norito-rpc` 作业；来源在 `javascript/iroha_js/artifacts/` 下上传 Norito 烟雾日志。
- **Swift SDK** – 在 IOS2 标志后面连接 Norito 桥接传输，镜像夹具节奏，并确保 Connect/Norito 奇偶校验套件在 `docs/source/sdk/swift/index.md` 中引用的 Buildkite 通道内运行。
- **Android SDK** – AND4 预览客户端和模拟 Torii 工具采用 Norito，重试/退避遥测记录在 `docs/source/sdk/android/networking.md` 中。该线束通过 `scripts/run_norito_rpc_fixtures.sh --sdk android` 与其他 SDK 共享固定装置。

## 证据和自动化

- `scripts/run_norito_rpc_fixtures.sh` 包装 `cargo xtask norito-rpc-verify`，捕获 stdout/stderr，并发出 `fixtures.<sdk>.summary.json`，因此 SDK 所有者可以将确定性工件附加到 `status.md`。使用 `--sdk <label>` 和 `--out artifacts/norito_rpc/<stamp>/` 保持 CI 包整洁。
- `cargo xtask norito-rpc-verify` 强制架构哈希奇偶校验 (`fixtures/norito_rpc/schema_hashes.json`)，如果 Torii 返回 `X-Iroha-Error-Code: schema_mismatch`，则失败。将每个失败与 JSON 后备捕获配对以进行调试。
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` 和 `dashboards/grafana/torii_norito_rpc_observability.json` 定义 NRPC-2 的警报合约。每次仪表板编辑后运行脚本并将 `promtool` 输出存储在金丝雀包中。
- `docs/source/runbooks/torii_norito_rpc_canary.md` 描述了分阶段和生产演习；每当夹具哈希或警报门发生变化时更新它。

## 审稿人清单

在勾选 NRPC-4 里程碑之前，请确认：

1. 最新的装置包哈希值与 `fixtures/norito_rpc/schema_hashes.json` 和 `artifacts/norito_rpc/<stamp>/` 下记录的相应 CI 工件相匹配。
2. SDK 自述文件/门户文档描述了如何强制 JSON 回退并引用 Norito 传输默认值。
3. 遥测仪表板显示带有警报链接的双堆栈错误率面板，并且 Alertmanager 试运行 (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) 连接到跟踪器。
4. 这里的采用时间表与跟踪器条目 (`docs/source/torii/norito_rpc_tracker.md`) 匹配，路线图 (NRPC-4) 引用相同的证据包。

遵守时间表可以保持跨 SDK 行为的可预测性，并允许治理审核 Norito-RPC 的采用，而无需定制请求。