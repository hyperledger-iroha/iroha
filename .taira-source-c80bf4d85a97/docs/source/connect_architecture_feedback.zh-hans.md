---
lang: zh-hans
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2025-12-29T18:16:35.934098+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#！连接架构反馈清单

此清单捕获了 Connect 会话架构中的未决问题
需要来自 Android 和 JavaScript 的输入的稻草人
2026 年 2 月跨 SDK 研讨会。用它来异步收集评论，跟踪
所有权，并畅通研讨会议程。

> 状态/备注栏捕获了 Android 和 JS 线索的最终回复
> 2026 年 2 月研讨会前同步；如果决策内联新的后续问题
> 进化。

## 会话生命周期和传输

|主题 |安卓机主 | JS 所有者 |状态/注释|
|--------|-------------|----------|----------------|
| WebSocket 重新连接退避策略（指数与上限线性）| Android 网络 TL | JS 主管 | ✅ 同意带抖动的指数退避，上限为 60 秒； JS 镜像了浏览器/节点奇偶校验的相同常量。 |
|离线缓冲区容量默认值（当前稻草人：32 帧）| Android 网络 TL | JS 主管 | ✅ 确认 32 帧默认值并覆盖配置； Android 通过 `ConnectQueueConfig` 持久化，JS 尊重 `window.connectQueueMax`。 |
|推送式重新连接通知（FCM/APNS 与轮询）| Android 网络 TL | JS 主管 | ✅ Android 将为钱包应用程序公开可选的 FCM 挂钩； JS 仍然基于轮询，具有指数退避，并注意浏览器推送限制。 |
|适用于移动客户端的乒乓球节奏护栏 | Android 网络 TL | JS 主管 | ✅ 标准化 30 秒 ping，具有 3 倍失误容忍度； Android平衡Doze影响，JS钳位到≥15s以避免浏览器限流。 |

## 加密和密钥管理

|主题 |安卓机主 | JS 所有者 |状态/注释|
|--------|-------------|----------|----------------|
| X25519 关键存储期望（StrongBox、WebCrypto 安全上下文）| Android 加密 TL | JS 主管 | ✅ Android 在可用时将 X25519 存储在 StrongBox 中（回退到 TEE）； JS 要求 dApp 使用安全上下文 WebCrypto，回退到 Node.js 中的本机 `iroha_js_host` 桥。 |
| ChaCha20-Poly1305 跨 SDK 共享随机数管理 | Android 加密 TL | JS 主管 | ✅ 采用共享 `sequence` 计数器 API，具有 64 位包装防护和共享测试； JS 使用 BigInt 计数器来匹配 Rust 行为。 |
|硬件支持的证明负载架构 | Android 加密 TL | JS 主管 | ✅ 架构最终确定：`attestation { platform, evidence_b64, statement_hash }`； JS可选（浏览器），Node使用HSM插件钩子。 |
|丢失钱包的恢复流程（密钥轮换握手）| Android 加密 TL | JS 主管 | ✅ 接受钱包轮换握手：dApp 发出 `rotate` 控制权，钱包回复新的公钥 + 签名确认； JS 立即重新加密 WebCrypto 材料。 |

## 权限和证明包|主题 |安卓机主 | JS 所有者 |状态/注释|
|--------|-------------|----------|----------------|
| GA 的最低权限架构（方法/事件/资源）| Android 数据模型 TL | JS 主管 | ✅ GA基线：`methods`、`events`、`resources`、`constraints`； JS 将 TypeScript 类型与 Rust 清单保持一致。 |
|钱包拒绝负载（`reason_code`，本地化消息）| Android 网络 TL | JS 主管 | ✅ 代码最终确定（`user_declined`、`permissions_mismatch`、`compliance_failed`、`internal_error`）以及可选的 `localized_message`。 |
|证明包可选字段（合规性/KYC 附件）| Android 数据模型 TL | JS 主管 | ✅ 所有 SDK 均接受可选的 `attachments[]` (Norito `AttachmentRef`) 和 `compliance_manifest_id`；无需改变行为。 |
| Norito JSON 模式与桥生成的结构的对齐 | Android 数据模型 TL | JS 主管 | ✅ 决策：更喜欢桥生成的结构； JSON 路径保留仅用于调试，JS 保留 `Value` 适配器。 |

## SDK 外观和 API 形状

|主题 |安卓机主 | JS 所有者 |状态/注释|
|--------|-------------|----------|----------------|
|高级异步接口（`Flow`，异步迭代器）奇偶校验 | Android 网络 TL | JS 主管 | ✅ Android 暴露 `Flow<ConnectEvent>`； JS使用`AsyncIterable<ConnectEvent>`；两者都映射到共享 `ConnectEventKind`。 |
|错误分类映射（`ConnectError`，类型化子类）| Android 网络 TL | JS 主管 | ✅ 采用共享枚举 {`Transport`、`Codec`、`Authorization`、`Timeout`、`QueueOverflow`、`Internal`} 以及特定于平台的负载详细信息。 |
|飞行中标志请求的取消语义 | Android 网络 TL | JS 主管 | ✅ 引入`cancelRequest(hash)`控件；这两个 SDK 都提供了尊重钱包确认的可取消协程/承诺。 |
|共享遥测挂钩（事件、指标命名）| Android 网络 TL | JS 主管 | ✅ 对齐的指标名称：`connect.queue_depth`、`connect.latency_ms`、`connect.reconnects_total`；记录了样本出口商。 |

## 离线持久化和日志记录

|主题 |安卓机主 | JS 所有者 |状态/注释|
|--------|-------------|----------|----------------|
|排队帧的存储格式（二进制 Norito 与 JSON）| Android 数据模型 TL | JS 主管 | ✅ 到处存储二进制 Norito (`.to`)； JS 使用 IndexedDB `ArrayBuffer`。 |
|期刊保留政策和大小上限 | Android 网络 TL | JS 主管 | ✅ 默认保留 24 小时，每次会话 1MiB；可通过 `ConnectQueueConfig` 进行配置。 |
|双方重播帧时的冲突解决 | Android 网络 TL | JS 主管 | ✅ 使用 `sequence` + `payload_hash`；忽略重复项，与遥测事件冲突触发 `ConnectError.Internal`。 |
|队列深度和重放成功的遥测 | Android 网络 TL | JS 主管 | ✅ 发出 `connect.queue_depth` 仪表和 `connect.replay_success_total` 计数器；两个 SDK 都连接到共享的 Norito 遥测模式。 |

## 实施峰值和参考- **Rust 桥接装置：** `crates/connect_norito_bridge/src/lib.rs` 和相关测试涵盖每个 SDK 使用的规范编码/解码路径。
- **Swift 演示工具：** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` 练习将会话流与模拟传输连接起来。
- **Swift CI 门控：** 在更新 Connect 工件时运行 `make swift-ci`，以在与其他 SDK 共享之前验证夹具奇偶校验、仪表板源和 Buildkite `ci/xcframework-smoke:<lane>:device_tag` 元数据。
- **JavaScript SDK 集成测试：** `javascript/iroha_js/test/integrationTorii.test.js` 根据 Torii 验证连接状态/会话帮助程序。
- **Android 客户端弹性注释：** `java/iroha_android/README.md:150` 记录了激发队列/回退默认值的当前连接实验。

## 研讨会准备物品

- [x] Android：为上面的每个表格行分配重点人员。
- [x] JS：为上面的每个表格行指定负责人。
- [x] 收集现有实施峰值或实验的链接。
- [x] 在 2026 年 2 月理事会之前安排工作前审查（与 Android TL、JS Lead、Swift Lead 于 2026 年 1 月 29 日 15:00 UTC 预订）。
- [x] 使用已接受的答案更新 `docs/source/connect_architecture_strawman.md`。

## 预读包

- ✅ 记录在 `artifacts/connect/pre-read/20260129/` 下的捆绑包（刷新稻草人、SDK 指南和此清单后通过 `make docs-html` 生成）。
- 📄 总结 + 分发步骤位于 `docs/source/project_tracker/connect_architecture_pre_read.md` 中；将该链接包含在 2026 年 2 月研讨会邀请和 `#sdk-council` 提醒中。
- 🔁 刷新捆绑包时，更新预读注释中的路径和哈希，并将公告存档在 IOS7/AND7 就绪日志下的 `status.md` 中。