---
lang: zh-hans
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 710286691d09a5707829a36ca98ed24a6af5c5629e708dd7b1bd0f01db4e31c1
source_last_modified: "2026-01-22T14:35:36.737834+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

标题：数据可用性摄取计划
sidebar_label：摄取计划
描述：Torii blob 摄取的架构、API 表面和验证计划。
---

:::注意规范来源
:::

# Sora Nexus 数据可用性摄取计划

_起草时间：2026-02-20 - 所有者：核心协议工作组/存储团队/DA 工作组_

DA-2 工作流使用发出 Norito 的 Blob 摄取 API 扩展了 Torii
元数据和种子 SoraFS 复制。本文件涵盖了拟议的
模式、API 表面和验证流程，因此实施无需
阻止未完成的模拟（DA-1 后续行动）。所有有效负载格式必须
使用 Norito 编解码器；不允许使用 serde/JSON 后备。

## 目标

- 接受大斑点（Taikai 段、车道边车、治理工件）
  确定性超过 Torii。
- 生成描述 blob、编解码器参数的规范 Norito 清单，
  擦除配置文件和保留策略。
- 将块元数据保留在 SoraFS 热存储和排队复制作业中。
- 将 pin 意图 + 策略标签发布到 SoraFS 注册表和治理
  观察员。
- 公开入场收据，以便客户重新获得确定性的出版证据。

## API 表面 (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

有效负载是 Norito 编码的 `DaIngestRequest`。响应使用
`application/norito+v1` 并返回 `DaIngestReceipt`。

|回应 |意义|
| ---| ---|
| 202 已接受 | Blob 排队等待分块/复制；收据退回。 |
| 400 错误请求 |架构/大小违规（请参阅验证检查）。 |
| 401 未经授权 | API 令牌缺失/无效。 |
| 409 冲突 |重复的 `client_blob_id` 元数据不匹配。 |
| 413 有效负载太大 |超出配置的 blob 长度限制。 |
| 429 太多请求 |达到速率限制。 |
| 500 内部错误 |意外失败（记录+警报）。 |

## 提议的 Norito 架构

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> 实现说明：这些有效负载的规范 Rust 表示现在位于
> `iroha_data_model::da::types`，带有 `iroha_data_model::da::ingest` 中的请求/收据包装器
> 以及 `iroha_data_model::da::manifest` 中的清单结构。

`compression` 字段通告调用者如何准备有效负载。 Torii 接受
`identity`、`gzip`、`deflate`、`zstd`，透明解压之前的字节
散列、分块和验证可选清单。

### 验证清单

1. 验证请求 Norito 标头是否与 `DaIngestRequest` 匹配。
2. 如果 `total_size` 与规范（解压）有效负载长度不同或超过配置的最大负载长度，则失败。
3. 强制 `chunk_size` 对齐（二的幂，<= 2 MiB）。
4. 确保 `data_shards + parity_shards` <= 全局最大值且奇偶校验 >= 2。
5. `retention_policy.required_replica_count` 必须尊重治理基线。
6. 针对规范哈希的签名验证（不包括签名字段）。
7. 拒绝重复的 `client_blob_id`，除非有效负载哈希 + 元数据相同。
8. 当提供 `norito_manifest` 时，验证模式 + 重新计算的哈希匹配
   分块后显现；否则节点生成清单并存储它。
9. 强制执行配置的复制策略：Torii 重写提交的
   `RetentionPolicy` 和 `torii.da_ingest.replication_policy`（参见
   `replication-policy.md`）并拒绝保留其保留的预建清单
   元数据与强制配置文件不匹配。

### 分块和复制流程

1. 将有效负载分块到 `chunk_size` 中，计算每个块的 BLAKE3 + Merkle 根。
2. 构建 Norito `DaManifestV1` （新结构）捕获块承诺（role/group_id），
   擦除布局（行和列奇偶校验计数加上 `ipa_commitment`）、保留策略、
   和元数据。
3. 将规范清单字节排列在 `config.da_ingest.manifest_store_dir` 下
   （Torii 写入由通道/纪元/序列/票据/指纹键入的 `manifest.encoded` 文件）因此 SoraFS
   编排可以摄取它们并将存储票证链接到持久数据。
4. 通过 `sorafs_car::PinIntent` 使用治理标签 + 策略发布 pin 意图。
5. 发出 Norito 事件 `DaIngestPublished` 通知观察者（轻客户端、
   治理、分析）。
6. 将 `DaIngestReceipt` 返回给调用者（由 Torii DA 服务密钥签名）并发出
   `Sora-PDP-Commitment` 标头，以便 SDK 可以立即捕获编码的承诺。收据
   现在包括 `rent_quote`（Norito `DaRentQuote`）和 `stripe_layout`，让提交者显示
   基本租金、储备份额、PDP/PoTR 奖金预期以及 2D 擦除布局
   提交资金之前的存储票。

## 存储/注册表更新

- 使用 `DaManifestV1` 扩展 `sorafs_manifest`，从而实现确定性解析。
- 添加带有版本有效负载引用的新注册表流 `da.pin_intent`
  清单哈希 + 票证 ID。
- 更新可观测性管道以跟踪摄取延迟、分块吞吐量、
  复制积压和失败计数。

## 测试策略

- 用于模式验证、签名检查、重复检测的单元测试。
- 验证 `DaIngestRequest`、清单和收据的 Norito 编码的黄金测试。
- 集成工具旋转模拟 SoraFS + 注册表，断言块 + 引脚流。
- 涵盖随机擦除配置文件和保留组合的性能测试。
- 对 Norito 有效负载进行模糊测试，以防止格式错误的元数据。

## CLI 和 SDK 工具 (DA-8)- `iroha app da submit`（新的 CLI 入口点）现在包装共享摄取构建器/发布器，以便操作员
  可以摄取 Taikai 束流之外的任意斑点。该命令位于
  `crates/iroha_cli/src/commands/da.rs:1` 并消耗有效负载、擦除/保留配置文件，以及
  使用 CLI 签署规范 `DaIngestRequest` 之前的可选元数据/清单文件
  配置键。成功运行后仍保留 `da_request.{norito,json}` 和 `da_receipt.{norito,json}`
  `artifacts/da/submission_<timestamp>/`（通过 `--artifact-dir` 覆盖），因此释放工件可以
  记录摄取期间使用的确切 Norito 字节。
- 该命令默认为 `client_blob_id = blake3(payload)` 但接受覆盖
  `--client-blob-id`，支持元数据 JSON 映射 (`--metadata-json`) 和预生成的清单
  (`--manifest`)，并支持`--no-submit`用于离线准备以及`--endpoint`用于定制
  Torii 主机。收据 JSON 除了写入磁盘之外，还会打印到 stdout，从而关闭
  DA-8“submit_blob”工具要求和解锁 SDK 奇偶校验工作。
- `iroha app da get` 为已经支持的多源协调器添加了一个以 DA 为中心的别名
  `iroha app sorafs fetch`。操作员可以将其指向清单+块计划工件（`--manifest`，
  `--plan`、`--manifest-id`)**或**通过 `--storage-ticket` 传递 Torii 存储票证。买票的时候
  使用 CLI 从 `/v2/da/manifests/<ticket>` 中提取清单的路径，将捆绑包保留在
  `artifacts/da/fetch_<timestamp>/`（用 `--manifest-cache-dir` 覆盖），派生 blob 哈希
  `--manifest-id`，然后使用提供的 `--gateway-provider` 列表运行协调器。全部
  SoraFS 提取器表面的高级旋钮完好无损（清单信封、客户标签、保护缓存、
  匿名传输覆盖、记分板导出和 `--output` 路径），并且清单端点可以
  对于自定义 Torii 主机，可通过 `--manifest-endpoint` 覆盖，因此实时进行端到端可用性检查
  完全位于 `da` 命名空间下，无需重复编排器逻辑。
- `iroha app da get-blob` 通过 `GET /v2/da/manifests/{storage_ticket}` 直接从 Torii 提取规范清单。
  该命令写入 `manifest_{ticket}.norito`、`manifest_{ticket}.json` 和 `chunk_plan_{ticket}.json`
  在 `artifacts/da/fetch_<timestamp>/` （或用户提供的 `--output-dir`）下，同时回显确切的
  后续协调器获取所需的 `iroha app da get` 调用（包括 `--manifest-id`）。
  这使操作员远离清单假脱机目录，并保证获取器始终使用
  Torii 发出的签名工件。 JavaScript Torii 客户端通过以下方式镜像此流程
  `ToriiClient.getDaManifest(storageTicketHex)`，返回解码后的 Norito 字节，清单 JSON，
  和块计划，以便 SDK 调用者可以补充编排器会话，而无需支付 CLI 费用。
  Swift SDK 现在公开相同的表面（`ToriiClient.getDaManifestBundle(...)` 加上
  `fetchDaPayloadViaGateway(...)`)，管道束到本机 SoraFS 协调器包装器中，以便
  iOS 客户端可以下载清单、执行多源获取并捕获证据，而无需
  调用CLI。【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` 计算所提供存储大小的确定性租金和激励细目
  和保留窗口。帮助程序消耗活动的 `DaRentPolicyV1`（JSON 或 Norito 字节）或
  内置默认值、验证策略并打印 JSON 摘要（`gib`、`months`、策略元数据、
  和 `DaRentQuote` 字段），因此审计人员可以在治理分钟内引用确切的 XOR 费用，而无需
  编写临时脚本。该命令还在 JSON 之前发出一行 `rent_quote ...` 摘要
  在事件演习期间保持控制台日志可读的有效负载。将 `--quote-out artifacts/da/rent_quotes/<stamp>.json` 与
  `--policy-label "governance ticket #..."` 保留引用确切政策投票的美化文物
  或配置包； CLI 修剪自定义标签并拒绝空白字符串，因此 `policy_source` 值
  在财务仪表板中保持可操作性。有关子命令，请参阅 `crates/iroha_cli/src/commands/da.rs`
  策略模式为 `docs/source/da/rent_policy.md`。【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` 链接以上所有内容：它需要一张存储票，下载
  规范清单包，针对
  提供的 `--gateway-provider` 列表，将下载的有效负载 + 记分板保留在
  `artifacts/da/prove_availability_<timestamp>/`，并立即调用现有的 PoR 助手
  (`iroha app da prove`) 使用获取的字节。操作员可以调整协调器旋钮
  （`--max-peers`、`--scoreboard-out`、清单端点覆盖）和证明采样器
  （`--sample-count`、`--leaf-index`、`--sample-seed`），而单个命令会生成工件
  DA-5/DA-9 审计所期望的：有效负载副本、记分板证据和 JSON 证明摘要。

## TODO 解决方案摘要

所有先前阻止的摄取 TODO 均已实施并验证：

- **压缩提示** — Torii 接受调用者提供的标签（`identity`、`gzip`、`deflate`、
  `zstd`）并在验证之前标准化有效负载，以便规范清单哈希与
  解压后的字节数。【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **仅治理元数据加密** — Torii 现在使用以下方法加密治理元数据
  配置 ChaCha20-Poly1305 密钥，拒绝不匹配的标签，并显示两个显式
  配置旋钮（`torii.da_ingest.governance_metadata_key_hex`，
  `torii.da_ingest.governance_metadata_key_label`) 保持旋转确定性。【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **大负载流** - 多部分摄取是实时的。客户端流确定性
  `DaIngestChunk` 包络由 `client_blob_id` 键入，Torii 验证每个切片并对其进行暂存
  在 `manifest_store_dir` 下，并在 `is_last` 标志落地后自动重建清单，
  消除单次调用上传时出现的 RAM 峰值。【crates/iroha_torii/src/da/ingest.rs:392】
- **清单版本控制** — `DaManifestV1` 带有显式 `version` 字段，而 Torii 拒绝
  未知版本，保证新清单布局发布时确定性升级。【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR 挂钩** — PDP 承诺直接源自块存储并被持久化
  除了清单之外，DA-5 调度程序可以从规范数据发起采样挑战，以及
  `/v2/da/ingest` 和 `/v2/da/manifests/{ticket}` 现在包含 `Sora-PDP-Commitment` 标头
  携带base64 Norito有效负载，以便SDK缓存确切的承诺DA-5探针目标。【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## 实施注意事项

- Torii 的 `/v2/da/ingest` 端点现在标准化有效负载压缩，强制重播缓存，
  确定性地对规范字节进行分块，重建 `DaManifestV1`，删除编码的有效负载
  到 `config.da_ingest.manifest_store_dir` 中进行 SoraFS 编排，并添加 `Sora-PDP-Commitment`
  标头，以便操作员捕获 PDP 调度程序将引用的承诺。【crates/iroha_torii/src/da/ingest.rs:220】
- 每个接受的 blob 现在都会生成 `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito`
  `manifest_store_dir` 下的条目将规范 `DaCommitmentRecord` 与原始数据捆绑在一起
  `PdpCommitmentV1` 字节，因此 DA-3 捆绑构建器和 DA-5 调度程序可以混合相同的输入，而无需
  重新读取清单或块存储。【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK 帮助程序 API 公开 PDP 标头有效负载，而不强制调用者重新实现 Norito 解码：
  Rust 箱导出 `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`，Python
  `ToriiClient` 现在包括 `decode_pdp_commitment_header`，并且 `IrohaSwift` 已发货
  原始标头映射或 `HTTPURLResponse` 实例的 `decodePdpCommitmentHeader` 重载。【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii 还公开 `GET /v2/da/manifests/{storage_ticket}`，以便 SDK 和操作员可以获取清单
  和块计划而不触及节点的假脱机目录。响应返回 Norito 字节
  (base64)，渲染清单 JSON，为 `sorafs fetch` 准备的 `chunk_plan` JSON blob，相关
  十六进制摘要（`storage_ticket`、`client_blob_id`、`blob_hash`、`chunk_root`），并镜像
  来自摄取响应的 `Sora-PDP-Commitment` 标头以进行奇偶校验。提供 `block_hash=<hex>`
  查询字符串返回确定性 `sampling_plan`（分配哈希、`sample_window` 和采样
  `(index, role, group)` 元组跨越完整的 2D 布局），因此验证器和 PoR 工具绘制相同
  指数。

### 大负载流

需要摄取大于配置的单次请求限制的资产的客户端发起
通过调用 `POST /v2/da/ingest/chunk/start` 进行流会话。 Torii 响应
`ChunkSessionId`（BLAKE3 - 从请求的 blob 元数据派生）和协商的块大小。
每个后续 `DaIngestChunk` 请求都携带：- `client_blob_id` — 与最终的 `DaIngestRequest` 相同。
- `chunk_session_id` — 将切片与正在运行的会话联系起来。
- `chunk_index` 和 `offset` — 强制执行确定性排序。
- `payload` — 达到协商的块大小。
- `payload_hash` — 切片的 BLAKE3 哈希，因此 Torii 可以在不缓冲整个 blob 的情况下进行验证。
- `is_last` — 表示终端片。

Torii 在 `config.da_ingest.manifest_store_dir/chunks/<session>/` 下保留经过验证的切片，并且
记录重播缓存内的进度以实现幂等性。当最后一片落地时，Torii
重新组装磁盘上的有效负载（通过块目录流式传输以避免内存峰值），
与单次上传完全相同地计算规范清单/收据，并最终响应
`POST /v2/da/ingest` 通过消耗分阶段工件。失败的会话可以显式中止或
在 `config.da_ingest.replay_cache_ttl` 之后被垃圾收集。这种设计保留了网络格式
Norito 友好，避免客户端特定的可恢复协议，并重用现有的清单管道
不变。

**实施状态。** 规范的 Norito 类型现在位于
`crates/iroha_data_model/src/da/`：

- `ingest.rs` 定义了 `DaIngestRequest`/`DaIngestReceipt`，以及
  Torii使用的`ExtraMetadata`容器。【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` 托管 `DaManifestV1` 和 `ChunkCommitment`，Torii 之后发出
  分块完成。【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` 提供共享别名（`BlobDigest`、`RetentionPolicy`、
  `ErasureProfile`等）并对下面记录的默认策略值进行编码。【crates/iroha_data_model/src/da/types.rs:240】
- 清单假脱机文件位于 `config.da_ingest.manifest_store_dir` 中，为 SoraFS 编排做好准备
  观察者拉入存储入场。【crates/iroha_torii/src/da/ingest.rs:220】

请求、清单和接收有效负载的往返覆盖范围在
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`，确保Norito编解码器
在更新中保持稳定。【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**保留默认值。** 治理期间批准了初始保留政策
SF-6； `RetentionPolicy::default()` 强制执行的默认值是：

- 热门层：7天（`604_800`秒）
- 冷层：90 天（`7_776_000` 秒）
- 所需副本：`3`
- 存储类别：`StorageClass::Hot`
- 治理标签：`"da.default"`

当车道采用时，下游操作员必须显式覆盖这些值
更严格的要求。