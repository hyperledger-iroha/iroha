---
lang: zh-hans
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-05T09:28:12.003461+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 连接会话架构 Strawman (Swift / Android / JS)

这个稻草人提案概述了 Nexus Connect 工作流程的共享设计
跨 Swift、Android 和 JavaScript SDK。其目的是支持
2026 年 2 月跨 SDK 研讨会并在实施前捕获悬而未决的问题。

> 最后更新: 2026-01-29  
> 作者：Swift SDK 主管、Android 网络 TL、JS 主管  
> 状态：供理事会审查的草案（2026 年 3 月 12 日添加威胁模型 + 数据保留调整）

## 目标

1. 调整钱包 ↔ dApp 会话生命周期，包括连接引导，
   批准、签署请求和拆除。
2.定义所有人共享的Norito信封模式（打开/批准/签署/控制）
   SDK 并确保与 `connect_norito_bridge` 同等。
3. 传输（WebSocket/WebRTC）、加密之间的职责划分
   （Norito 连接框架+密钥交换）和应用程序层（SDK 外观）。
4. 确保跨桌面/移动平台的确定性行为，包括
   离线缓冲和重新连接。

## 会话生命周期（高级）

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## 信封/Norito 架构

所有 SDK 必须使用 `connect_norito_bridge` 中定义的规范 Norito 模式：

- `EnvelopeV1`（打开/批准/签名/控制）
- `ConnectFrameV1`（带有 AEAD 有效负载的密文帧）
- 控制代码：
  - `open_ext`（元数据、权限）
  - `approve_ext`（账户、权限、证明、签名）
  - `reject`、`close`、`ping/pong`、`error`

Swift 之前发布了占位符 JSON 编码器 (`ConnectCodec.swift`)。截至 2026 年 4 月 SDK
总是使用 Norito 桥，并且当 XCFramework 丢失时关闭失败，但是这个稻草人
仍然抓住了导致桥梁集成的任务：

|功能|描述 |状态 |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | dApp 开放框架 |在桥中实施 |
| `connect_norito_encode_control_approve_ext` |钱包审批 |已实施 |
| `connect_norito_encode_envelope_sign_request_tx/raw` |签署请求 |已实施 |
| `connect_norito_encode_envelope_sign_result_ok/err` |签署结果|已实施 |
| `connect_norito_decode_*` |解析钱包/dApp |已实施 |

### 所需工作

- Swift：用桥接调用和表面替换占位符 `ConnectCodec` JSON 助手
  使用共享 Norito 类型的类型化包装器（`ConnectFrame`、`ConnectEnvelope`）。 ✅（2026 年 4 月）
- Android/JS：确保存在相同的包装器；对齐错误代码和元数据键。
- 共享：具有一致密钥派生的文档加密（X25519 密钥交换，AEAD）
  根据 Norito 规范，并使用 Rust 桥提供示例集成测试。

## 运输合同- 主要传输：WebSocket (`/v1/connect/ws?sid=<session_id>`)。
- 可选的未来：WebRTC（TBD）——超出了最初稻草人的范围。
- 重新连接策略：带完全抖动的指数退避（基本 5 秒，最大 60 秒）； Swift、Android 和 JS 之间共享常量，因此重试仍然是可预测的。
- Ping/pong 节奏：30 秒心跳，在重新连接之前可以容忍 3 次错过的 Pong； JS 将最小间隔限制为 15 秒以满足浏览器限制规则。
- 推送挂钩：Android 钱包 SDK 公开了用于唤醒的可选 FCM 集成，而 JS 保持基于轮询（已记录的浏览器推送权限限制）。
- SDK职责：
  - 保持乒乓心跳（避免耗尽手机电池）。
  - 离线时缓冲传出帧（有界队列，为 dApp 保留）。
- 提供事件流API（Swift Couple `AsyncStream`、Android Flow、JS async iter）。
- 表面重新连接挂钩并允许手动重新订阅。
- 遥测编辑：仅发出会话级计数器（`sid` 哈希、方向、
  序列窗口、队列深度）以及 Connect 遥测中记录的盐
  指导；标头/键绝不能出现在日志或调试字符串中。

## 加密和密钥管理

### 会话标识符和盐

- `sid` 是从 `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)` 派生的 32 字节标识符。  
  DApp 在调用 `/v1/connect/session` 之前计算它；钱包在 `approve` 帧中回应它，因此双方可以一致地输入日志和遥测数据。
- 相同的盐提供每个密钥派生步骤，因此 SDK 从不依赖于从主机平台获取的熵。

### 临时密钥处理

- 每个会话都使用新的 X25519 密钥材料。  
  Swift 通过 `ConnectCrypto` 将其存储在 Keychain/Secure Enclave 中，Android 钱包默认为 StrongBox（回退到 TEE 支持的密钥库），JS 需要安全上下文 WebCrypto 实例或本机 `iroha_js_host` 插件。
- 开放框架包括 dApp 临时公钥以及可选的证明捆绑包。钱包批准会返回钱包公钥以及合规流程所需的任何硬件证明。
- 证明有效负载遵循可接受的架构：  
  `attestation { platform, evidence_b64, statement_hash }`。  
  浏览器可能会忽略该块；每当使用硬件支持的密钥时，本机钱包都会包含它。

### 方向键和 AEAD

- 使用 HKDF-SHA256（通过 Rust 桥助手）和域分隔的信息字符串扩展共享密钥：
  - `iroha-connect|k_app` → 应用→钱包流量。
  - `iroha-connect|k_wallet`→钱包→应用程序流量。
- AEAD 是 v1 信封的 ChaCha20-Poly1305（`connect_norito_bridge` 在每个平台上公开助手）。  
  关联数据等于 `("connect:v1", sid, dir, seq_le, kind=ciphertext)`，因此检测到对标头的篡改。
- 随机数源自 64 位序列计数器（`nonce[0..4]=0`、`nonce[4..12]=seq_le`）。共享帮助程序测试可确保 BigInt/UInt 转换在 SDK 中的行为相同。

### 旋转和恢复握手- 轮换仍然是可选的，但协议已定义：当序列计数器接近包装防护时，dApp 会发出 `Control::RotateKeys` 帧，钱包使用新的公钥和签名的确认进行响应，双方立即派生新的方向密钥而不关闭会话。
- 钱包端密钥丢失会触发相同的握手，然后是 `resume` 控制，因此 dApp 知道要刷新针对已停用密钥的缓存密文。

有关历史 CryptoKit 回退，请参阅 `docs/connect_swift_ios.md`； Kotlin 和 JS 在 `docs/connect_kotlin_ws*.md` 下有匹配的引用。

## 权限和证明

- 权限清单必须通过桥导出的共享 Norito 结构进行往返。  
  领域：
  - `methods` — 动词（`sign_transaction`、`sign_raw`、`submit_proof`，...）。  
  - `events` — 允许 dApp 附加的订阅。  
  - `resources` — 可选帐户/资产过滤器，以便钱包可以限制访问范围。  
  - `constraints` — 链 ID、TTL 或钱包在签名前强制执行的自定义策略旋钮。
- 合规性元数据与权限并存：
  - 可选 `attachments[]` 包含 Norito 附件参考（KYC 捆绑包、监管机构收据）。  
  - `compliance_manifest_id` 将请求与先前批准的清单联系起来，以便操作员可以审核出处。
- 钱包响应使用约定的代码：
  - `user_declined`、`permissions_mismatch`、`compliance_failed`、`internal_error`。  
  每个可能携带一个用于 UI 提示的 `localized_message` 以及一个机器可读的 `reason_code`。
- 批准框架包括选定的帐户/控制器、权限回显、证明包（ZK 证明或证明）以及任何策略切换（例如 `offline_queue_enabled`）。  
  拒绝镜像具有空 `proof` 的相同架构，但仍记录 `sid` 以供审核。

## SDK 外观

| SDK |提议的 API |笔记|
|-----|--------------|--------|
|斯威夫特 | `ConnectClient`、`ConnectSession`、`ConnectRequest`、`ConnectApproval` |用类型化包装器 + 异步流替换占位符。 |
|安卓 | Kotlin 协程 + 框架的密封类 |与 Swift 结构保持一致以实现可移植性。 |
| JS |框架类型的异步迭代器 + TypeScript 枚举 |提供捆绑器友好的 SDK（浏览器/节点）。 |

### 常见行为- `ConnectSession` 协调生命周期：
  1. 建立WebSocket，进行握手。
  2. 交换开放/批准框架。
  3. 处理签名请求/响应。
  4. 向应用层发送事件。
- 提供高级帮手：
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
  - `cancelRequest(hash)` – 发出钱包确认的控制帧。
- 错误处理：将 Norito 错误代码映射到 SDK 特定错误；包括
  使用共享分类法的 UI 的域特定代码（`Transport`、`Codec`、`Authorization`、`Timeout`、`QueueOverflow`、`Internal`）。 Swift 的基线实现 + 遥测指南位于 [`connect_error_taxonomy.md`](connect_error_taxonomy.md) 中，是 Android/JS 奇偶校验的参考。
- 发出队列深度、重新连接计数和请求延迟的遥测挂钩（`connect.queue_depth`、`connect.reconnects_total`、`connect.latency_ms`）。
 
## 序列号和流量控制

- 每个方向都保留一个专用的 64 位 `sequence` 计数器，该计数器在会话打开时从零开始。共享帮助程序类型会限制增量并在计数器回绕之前触发 `ConnectError.sequenceOverflow` + 密钥旋转握手。
- 随机数和关联数据引用序列号，因此可以在不解析有效负载的情况下拒绝重复项。 SDK 必须将 `{sid, dir, seq, payload_hash}` 存储在其日志中，以使重复数据删除在重新连接时具有确定性。
- 钱包通过逻辑窗口（`FlowControl` 控制帧）通告背压。仅当窗口令牌可用时，DApp 才会出队；钱包在处理密文后发出新的代币以保持管道的边界。
- 恢复协商是明确的：双方在重新连接后都会发出 `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }`，以便观察者可以验证重新发送了多少数据以及日志是否包含间隙。
- 冲突（例如，两个有效负载具有相同的 `(sid, dir, seq)` 但不同的哈希值）升级为 `ConnectError.Internal` 并强制使用新的 `sid` 以避免静默分歧。

## 威胁模型和数据保留一致性- **考虑的表面：** WebSocket 传输、Norito 桥编码/解码、
  日志持久性、遥测导出器和面向应用程序的回调。
- **主要目标：** 保护会话机密（X25519 密钥、派生 AEAD 密钥、
  随机数/序列计数器）免受日志/遥测泄漏的影响，防止重放和
  降级攻击，并限制日志和异常报告的保留。
- **已编纂的缓解措施：**
  - 期刊仅携带密文；存储的元数据仅限于哈希值、长度
    字段、时间戳和序列号。
  - 遥测有效负载会编辑任何标头/有效负载内容，并且仅包含
    `sid` 加盐哈希加上聚合计数器；共享修订清单
    SDK 之间的审核奇偶校验。
  - 默认情况下，会话日志会轮换并在 7 天后过期。钱包暴露
    `connectLogRetentionDays` 旋钮（SDK 默认 7）并记录行为
    因此，受监管的部署可以固定更严格的窗口。
  - Bridge API 滥用（缺少绑定、损坏的密文、无效序列）
    返回键入的错误，而不回显原始有效负载或密钥。

审核中的待决问题在 `docs/source/sdk/swift/connect_workshop.md` 中进行跟踪
并将在理事会会议记录中予以解决；一旦关闭，稻草人将
从草稿晋升为接受。

## 离线缓冲和重新连接

### 日记合同

每个 SDK 都会为每个会话维护一个仅附加日志，因此 dApp 和钱包
可以在离线时对帧进行排队、在不丢失数据的情况下恢复并提供证据
用于遥测。该合约镜像 Norito 桥接类型，因此具有相同的字节
表示在移动/JS 堆栈中仍然存在。- 日志存在于散列会话标识符 (`sha256(sid)`) 下，产生两个
  每个会话的文件：`app_to_wallet.queue` 和 `wallet_to_app.queue`。斯威夫特使用
  沙盒文件包装器，Android 通过 `Room`/`FileChannel` 存储文件，
  JS写入IndexedDB；所有格式都是二进制且字节序稳定。
- 每条记录序列化为 `ConnectJournalRecordV1`：
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]`（Blake3密文+标头）
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]`（确切的 Norito 框架已经 AEAD 包装）
- 日志逐字存储密文。我们从不重新加密有效负载； AEAD
  标头已经验证了方向键，因此持久性减少为
  fsyncing 附加记录。
- 内存中的 `ConnectQueueState` 结构镜像文件元数据（深度、
  使用的字节数，最旧/最新的序列）。它为遥测出口商和
  `FlowControl` 帮助者。
- 默认情况下，日志上限为 32 帧/1MiB；击中上限将驱逐
  最旧的条目 (`reason=overflow`)。 `ConnectFeatureConfig.max_queue_len`
  每个部署都会覆盖这些默认值。
- 日志保留数据 24 小时 (`expires_at_ms`)。后台 GC 删除陈旧内容
  急切地进行分段，以便磁盘上的足迹保持有限。
- 崩溃安全：在通知之前追加、fsync 和更新内存镜像
  来电者。启动时，SDK 会扫描目录，验证记录校验和，
  并重建 `ConnectQueueState`。腐败导致违规记录
  跳过，通过遥测标记，并可选择隔离支持转储。
- 因为密文已经满足 Norito 隐私信封，唯一
  记录的附加元数据是散列会话 ID。需要额外的应用程序
  隐私可以选择 `telemetry_opt_in = false`，它存储日志，但是
  编辑队列深度导出并禁用在日志中共享散列 `sid`。
- SDK 公开 `ConnectQueueObserver`，以便钱包/dApp 可以检查队列深度，
  排水管和 GC 结果；该钩子提供状态 UI，而不解析日志。

### 重放和恢复语义

1. 重新连接时，SDK 会发出 `Control::Resume` 和 `{seq_app_max,
   seq_wallet_max、queued_app、queued_wallet、journal_hash}`。哈希值是
   Blake3 仅附加日志的摘要，因此不匹配的同行可以检测漂移。
2. 接收方将恢复负载与其状态进行比较，请求
   存在间隙时重传，并通过以下方式确认重播帧
   `Control::ResumeAck`。
3. 重放的帧始终遵循插入顺序（`sequence`，然后是写入时间）。
   钱包 SDK 必须通过发行 `FlowControl` 代币（也
   日志），因此 dApp 无法在离线状态下淹没队列。
4. 日志逐字存储密文，因此重放只是泵送记录的字节
   通过传输和解码器返回。不允许按 SDK 重新编码。

### 重连流程1. Transport 重新建立 WebSocket 并协商新的 ping 间隔。
2. dApp 按顺序重放排队的帧，尊重钱包的背压
   （`ConnectSession.nextControlFrame()` 生成 `FlowControl` 代币）。
3. 钱包解密缓冲结果，验证序列单调性，并
   重播待批准/结果。
4.双方发出一个`resume`控制总结`seq_app_max`，`seq_wallet_max`，
   和遥测的队列深度。
5. 重复帧（匹配 `sequence` + `payload_hash`）被确认并丢弃；冲突引发 `ConnectError.Internal` 并触发强制会话重新启动。

### 故障模式

- 如果会话被视为过时（`offline_timeout_ms`，默认 5 分钟），
  缓冲帧被清除，SDK 引发 `ConnectError.sessionExpired`。
- 如果日志损坏，SDK 会尝试单个 Norito 解码修复；上
  如果失败，他们会丢弃日志并发出 `connect.queue_repair_failed` 遥测数据。
- 序列不匹配触发 `ConnectError.replayDetected` 并强制重新启动
  握手（使用新的 `sid` 重新启动会话）。

### 离线缓冲计划和操作员控制

研讨会交付成果需要一份书面计划，以便每个 SDK 都提供相同的
线下行为、补救流程和证据表面。下面的计划是
Swift (`ConnectSessionDiagnostics`)、Android 中常见
（`ConnectDiagnosticsSnapshot`）和JS（`ConnectQueueInspector`）。

|状态|触发|自动回复 |手动操作|遥测标志|
|--------|---------|--------------------|-----------------|----------------|
| `Healthy` |队列使用率  5/分钟 |暂停新的签名请求，以一半的速率发出流量控制令牌 |应用程序可能会调用 `clearOfflineQueue(.app|.wallet)`；一旦上线，SDK 就会重新获取对等方的状态 | `connect.queue_state=\"throttled\"`、`connect.queue_watermark` 仪表 |
| `Quarantined` |使用率 ≥ `disk_watermark_drop`（默认 85%），检测到损坏两次，或超出 `offline_timeout_ms` |停止缓冲，引发 `ConnectError.QueueQuarantined`，需要操作员确认 | `ConnectSessionDiagnostics.forceReset()` 导出捆绑包后删除日志 | `connect.queue_state=\"quarantined\"`、`connect.queue_quarantine_total` 计数器 |- 阈值位于 `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`、`max_disk_bytes`、`offline_timeout_ms`）。当主持人
  省略一个值，SDK 会回退到默认值并记录警告，以便配置
  可以通过遥测进行审核。
- SDK 公开 `ConnectQueueObserver` 以及诊断助手：
  - Swift：`ConnectSessionDiagnostics.snapshot()` 产生“{状态，深度，字节，
    Reason}` and `exportJournalBundle(url:)` 保留两个队列以获取支持。
  - 安卓：`ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`。
  - JS: `ConnectQueueInspector.read()` 返回相同的结构和 blob 句柄
    该UI代码可以上传到Torii支持工具。
- 当应用程序切换 `offline_queue_enabled=false` 时，SDK 会立即耗尽并
  清除两个日志，将状态标记为 `Disabled`，并发出一个终端
  遥测事件。面向用户的偏好反映在 Norito 中
  批准帧，以便同行知道他们是否可以恢复缓冲的帧。
- 操作员运行 `connect queue inspect --sid <sid>`（SDK 周围的 CLI 包装器）
  混沌测试期间的诊断）；该命令打印状态转换，
  水印历史记录，并恢复证据，以便治理审查不依赖于
  特定于平台的工具。

### 证据包工作流程

支持和合规团队在审计时依赖确定性证据
离线行为。因此，每个 SDK 都实现相同的三步导出：

1. `exportJournalBundle(..)` 写入 `{app_to_wallet,wallet_to_app}.queue` 加一个
   描述构建哈希、功能标志和磁盘水印的清单。
2. `exportQueueMetrics(..)` 发出最后 1000 个遥测样本，以便仪表板
   可以离线重建。示例包括哈希会话 ID，当
   用户选择加入。
3. CLI 帮助程序压缩导出并附加签名的 Norito 元数据文件
   (`ConnectQueueEvidenceV1`) 因此 Torii 摄取可以将捆绑包存档在 SoraFS 中。

验证失败的捆绑包将被拒绝，并显示 `connect.evidence_invalid`
遥测，以便 SDK 团队可以重现并修补导出器。

## 遥测和诊断- 通过共享 OpenTelemetry 导出器发出 Norito JSON 事件。强制性指标：
  - `connect.queue_depth{direction}`（表压）由 `ConnectQueueState` 供给。
  - `connect.queue_bytes{direction}`（规格）用于磁盘支持的占用空间。
  - `connect.queue_dropped_total{reason}`（计数器）用于 `overflow|ttl|repair`。
  - `connect.offline_flush_total{direction}`（计数器）在排队时递增
    无需运输即可排出；故障增量为 `connect.offline_flush_failed`。
  - `connect.replay_success_total`/`connect.replay_error_total`。
  - `connect.resume_latency_ms` 直方图（重新连接和稳定之间的时间
    状态）加上 `connect.resume_attempts_total`。
  - `connect.session_duration_ms` 直方图（每个已完成的会话）。
  - `connect.error` 与 `code`、`fatal`、`telemetry_profile` 结构化事件。
- 出口商必须贴上 `{platform, sdk_version, feature_hash}` 标签，以便
  仪表板可以按 SDK 版本进行拆分。散列 `sid` 是可选的，并且是唯一的
  当遥测选择加入为真时发出。
- SDK 级挂钩呈现相同的事件，以便应用程序可以导出更多详细信息：
  - 斯威夫特：`ConnectSession.addObserver(_:) -> ConnectEvent`。
  - 安卓：`Flow<ConnectEvent>`。
  - JS：异步迭代器或回调。
- CI 门控：Swift 作业运行 `make swift-ci`，Android 使用 `./gradlew sdkConnectCi`，
  并且 JS 运行 `npm run test:connect`，因此遥测/仪表板之前保持绿色
  合并连接更改。
- 结构化日志包括散列 `sid`、`seq`、`queue_depth` 和 `sid_epoch`
  值，以便操作员可以关联客户问题。修复失败的日志会发出
  `connect.queue_repair_failed{reason}` 事件以及可选的故障转储路径。

### 遥测挂钩和治理证据

- `connect.queue_state` 兼作路线图风险指示器。仪表板组
  通过 `{platform, sdk_version}` 并呈现状态时间，以便治理可以采样
  在批准分阶段部署之前每月进行演习证据。
- `connect.queue_watermark` 和 `connect.queue_bytes` 提供 Connect 风险评分
  (`risk.connect.offline_buffer`)，当超过
  5% 的会话在 `Throttled` 中花费超过 10 分钟。
- 出口商将 `feature_hash` 附加到每个事件，以便审核工具可以确认
  Norito 编解码器 + 离线计划与审核的版本匹配。 SDK CI 失败
  当遥测报告未知哈希值时速度很快。
- 稻草人仍然需要威胁模型附录；当指标超过
  策略阈值，SDK 会发出 `connect.policy_violation` 事件，总结
  有问题的 sid（散列）、状态和已解决的操作 (`drain|purge|quarantine`)。
- 通过 `exportQueueMetrics` 捕获的证据位于相同的 SoraFS 命名空间中
  作为 Connect 操作手册的工件，以便理事会审查者可以跟踪每一次演习
  返回特定的遥测样本，无需请求内部日志。

## 框架所有权和责任|框架/控制|业主|序列域|日记坚持了吗？ |遥测标签|笔记|
|----------------|--------------------|-----------------|--------------------|--------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` |携带元数据+权限位图；钱包重放最近一次打开之前的提示。 |
| `Control::Approve` |钱包| `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` |包括账户、证明、签名。此处记录元数据版本增量。 |
| `Control::Reject` |钱包| `seq_wallet` | ✅ | `event=reject`，`reason` |可选的本地化消息； dApp 会删除待处理的签名请求。 |
| `Control::Close`（初始化）| dApp | `seq_app` | ✅ | `event=close`，`initiator=app` |钱包用自己的 `Close` 进行确认。 |
| `Control::Close`（确认）|钱包| `seq_wallet` | ✅ | `event=close`，`initiator=wallet` |确认拆解；一旦双方都坚持框架，GC 就会移除轴颈。 |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`，`payload_hash` |记录有效负载哈希以进行重播冲突检测。 |
| `SignResult` |钱包| `seq_wallet` | ✅ | `event=sign_result`，`status=ok|err` |包括签名字节的 BLAKE3 哈希值；故障引发 `ConnectError.Signing`。 |
| `Control::Error` |钱包（大多数）/dApp（交通）|匹配所有者域 | ✅ | `event=error`、`code` |致命错误迫使会话重新启动；遥测标记 `fatal=true`。 |
| `Control::RotateKeys` |钱包| `seq_wallet` | ✅ | `event=rotate_keys`，`reason` |宣布新的方向键； dApp 回复 `RotateKeysAck`（在应用程序端记录）。 |
| `Control::Resume` / `ResumeAck` |两者 |仅本地域 | ✅ | `event=resume`、`direction=app|wallet` |总结队列深度+seq状态；散列期刊摘要有助于诊断。 |

- 每个角色的定向密码密钥保持对称（`app→wallet`、`wallet→app`）。
  钱包轮换提案通过 `Control::RotateKeys` 和 dApp 发布
  通过发出 `Control::RotateKeysAck` 进行确认；两个帧都必须击中磁盘
  在密钥交换之前避免重播间隙。
- 元数据附件（图标、本地化名称、合规性证明）由以下人员签名
  钱包并由 dApp 缓存；更新需要新的批准框架
  递增 `metadata_version`。
- 上面的所有权矩阵是从 SDK 文档中引用的，因此 CLI/web/automation
  客户遵循相同的合同和仪器默认值。

## 开放问题

1. **会话发现**：我们是否需要像 WalletConnect 那样的二维码/带外握手？ （未来的工作。）
2. **多重签名**：多重签名批准如何表示？ （扩展签名结果以支持多个签名。）
3. **合规性**：哪些字段对于受监管的流量是强制性的（根据路线图）？ （等待合规团队指导。）
4. **SDK 打包**：我们是否应该将共享代码（例如 Norito Connect 编解码器）纳入跨平台 crate 中？ （待定。）

## 后续步骤- 将此稻草人传给 SDK 委员会（2026 年 2 月的会议）。
- 收集有关未解决问题的反馈并相应更新文档。
- 按 SDK 安排实施细目（Swift IOS7、Android AND7、JS Connect 里程碑）。
- 通过路线图热门列表跟踪进度；一旦稻草人获得批准，请更新 `status.md`。