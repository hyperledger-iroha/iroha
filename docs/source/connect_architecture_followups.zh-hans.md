---
lang: zh-hans
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2025-12-29T18:16:35.934525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 连接架构后续行动

本说明记录了跨 SDK 出现的工程后续工作
连接架构审查。每行应映射到一个问题（Jira 票证或 PR）
一旦工作安排好。当所有者创建跟踪票时更新表。|项目 |描述 |所有者 |追踪 |状态 |
|------|-------------|----------|---------|--------|
|共享退避常数|实现指数退避 + 抖动助手 (`connect_retry::policy`) 并将其公开给 Swift/Android/JS SDK。 | Swift SDK、Android 网络 TL、JS 主管 | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) |已完成 — `connect_retry::policy` 具有确定性 splitmix64 采样； Swift (`ConnectRetryPolicy`)、Android 和 JS SDK 提供镜像助手和黄金测试。 |
|乒乓球执法 |使用商定的 30 秒节奏和浏览器最小限制添加可配置的心跳强制执行；表面指标 (`connect.ping_miss_total`)。 | Swift SDK、Android 网络 TL、JS 主管 | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) |已完成 — Torii 现在强制执行可配置的心跳间隔（`ping_interval_ms`、`ping_miss_tolerance`、`ping_min_interval_ms`），公开 `connect.ping_miss_total` 指标，并提供涵盖心跳断开处理的回归测试。 SDK 功能快照为客户展示了新的旋钮。 |
|离线队列持久化 |使用共享模式实现 Connect 队列的 Norito `.to` 日志写入器/读取器（Swift `FileManager`、Android 加密存储、JS IndexedDB）。 | Swift SDK、Android 数据模型 TL、JS 主管 | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) |已完成 - Swift、Android 和 JS 现在提供共享的 `ConnectQueueJournal` + 诊断助手以及保留/溢出测试，因此证据包在整个过程中保持确定性SDKs.【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| StrongBox 证明负载 |通过钱包批准线程 `{platform,evidence_b64,statement_hash}` 并向 dApp SDK 添加验证。 | Android 加密 TL、JS 主管 | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) |待定 |
|旋转控制架|实现 `Control::RotateKeys` + `RotateKeysAck` 并在所有 SDK 中公开 `cancelRequest(hash)` / 旋转 API。 | Swift SDK、Android 网络 TL、JS 主管 | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) |待定 |
|遥测出口商|将 `connect.queue_depth`、`connect.reconnects_total`、`connect.latency_ms` 和重播计数器发送到现有遥测管道 (OpenTelemetry)。 |遥测工作组、SDK 所有者 | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) |待定 |
| Swift CI 门控 |确保与 Connect 相关的管道调用 `make swift-ci`，以便夹具奇偶校验、仪表板源和 Buildkite `ci/xcframework-smoke:<lane>:device_tag` 元数据在 SDK 之间保持一致。 | Swift SDK 主管，构建基础设施 | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) |待定 |
|后备事件报告|将 XCFramework 烟雾安全事件（`xcframework_smoke_fallback`、`xcframework_smoke_strongbox_unavailable`）连接到 Connect 仪表板以实现共享可见性。 | Swift QA 主管，构建基础设施 | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) |待定 ||合规附件传递|确保 SDK 接受并转发批准负载中的可选 `attachments[]` + `compliance_manifest_id` 字段而不会丢失。 | Swift SDK、Android 数据模型 TL、JS 主管 | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) |待定 |
|错误分类对齐 |使用文档/示例将共享枚举（`Transport`、`Codec`、`Authorization`、`Timeout`、`QueueOverflow`、`Internal`）映射到特定于平台的错误。 | Swift SDK、Android 网络 TL、JS 主管 | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) |已完成 - Swift、Android 和 JS SDK 提供共享的 `ConnectError` 包装器 + 遥测助手，以及 README/TypeScript/Java 文档和涵盖 TLS/timeout/HTTP/codec/queue 的回归测试案例。【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
|车间决策日志 |将总结已接受决定的带注释的甲板/注释发布到理事会档案中。 | SDK 项目负责人 | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) |待定 |

> 业主开票时将填写跟踪标识符；更新 `Status` 列以及问题进度。