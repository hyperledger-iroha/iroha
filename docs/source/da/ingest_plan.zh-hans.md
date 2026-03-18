---
lang: zh-hans
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

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
- 在 SoraFS 热存储和排队复制作业中保留块元数据。
- 将 pin 意图 + 策略标签发布到 SoraFS 注册表和治理
  观察员。
- 公开入场收据，以便客户重新获得确定性的出版证据。

## API 表面 (Torii)

```
POST /v1/da/ingest
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

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

返回从当前通道目录派生的版本化 `DaProofPolicyBundle`。
该捆绑包通告 `version`（当前为 `1`），即 `policy_hash`（
有序策略列表），以及携带 `lane_id`、`dataspace_id` 的 `policies` 条目，
`alias`，以及强制执行的 `proof_scheme`（今天的 `merkle_sha256`；KZG 通道是
在 KZG 承诺可用之前，会被摄取拒绝）。现在的区块头
通过 `da_proof_policies_hash` 提交到捆绑包，以便客户端可以固定
验证 DA 承诺或证明时设置的主动策略。获取这个端点
在建立证据以确保它们符合车道的政策和当前的情况之前
捆绑哈希。承诺列表/证明端点具有相同的捆绑包，因此 SDK
不需要额外的往返来将证明绑定到活动策略集。

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

返回 `DaProofPolicyBundle`，其中包含有序策略列表以及
`policy_hash`，因此 SDK 可以固定生成块时使用的版本。的
哈希是通过 Norito 编码的策略数组计算的，并且每当
Lane 的 `proof_scheme` 已更新，允许客户检测之间的漂移
缓存的证明和链配置。

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
```> 实现说明：这些有效负载的规范 Rust 表示现在位于
> `iroha_data_model::da::types`，带有 `iroha_data_model::da::ingest` 中的请求/收据包装器
> 以及 `iroha_data_model::da::manifest` 中的清单结构。

`compression` 字段通告调用者如何准备有效负载。 Torii 接受
`identity`、`gzip`、`deflate`、`zstd`，透明解压之前的字节
散列、分块和验证可选清单。

### 验证清单

1. 验证请求 Norito 标头是否与 `DaIngestRequest` 匹配。
2. 如果 `total_size` 与规范（解压缩）有效负载长度不同或超过配置的最大负载长度，则失败。
3. 强制 `chunk_size` 对齐（二的幂，= 2。
5. `retention_policy.required_replica_count` 必须尊重治理基线。
6. 针对规范哈希的签名验证（不包括签名字段）。
7. 拒绝重复的 `client_blob_id`，除非有效负载哈希 + 元数据相同。
8. 当提供 `norito_manifest` 时，验证架构 + 重新计算的哈希匹配
   分块后显现；否则节点生成清单并存储它。
9. 强制执行配置的复制策略：Torii 重写提交的
   `RetentionPolicy` 和 `torii.da_ingest.replication_policy`（参见
   `replication_policy.md`）并拒绝保留其保留的预建清单
   元数据与强制配置文件不匹配。

### 分块和复制流程1. 将有效负载分块到 `chunk_size` 中，计算每个块的 BLAKE3 + Merkle 根。
2. 构建 Norito `DaManifestV1`（新结构）捕获块承诺（role/group_id），
   擦除布局（行和列奇偶校验计数加上 `ipa_commitment`）、保留策略、
   和元数据。
3. 将规范清单字节排列在 `config.da_ingest.manifest_store_dir` 下
   （Torii 写入由通道/纪元/序列/票据/指纹键入的 `manifest.encoded` 文件）因此 SoraFS
   编排可以摄取它们并将存储票证链接到持久数据。
4. 通过 `sorafs_car::PinIntent` 使用治理标签 + 策略发布 pin 意图。
5. 发出 Norito 事件 `DaIngestPublished` 通知观察者（轻客户端、
   治理、分析）。
6. 返回 `DaIngestReceipt`（由 Torii DA 服务密钥签名）并添加
   `Sora-PDP-Commitment` 响应标头包含 base64 Norito 编码
   派生的承诺，以便 SDK 可以立即隐藏采样种子。
   收据现在嵌入 `rent_quote`（`DaRentQuote`）和 `stripe_layout`
   因此提交者可以提出 XOR 义务、储备份额、PDP/PoTR 奖金预期、
   以及提交资金之前的 2D 擦除矩阵维度以及存储票据元数据。
7. 可选的注册表元数据：
   - `da.registry.alias` — 公共、未加密的 UTF-8 别名字符串，用于为 pin 注册表项提供种子。
   - `da.registry.owner` — 用于记录注册表所有权的公共、未加密的 `AccountId` 字符串。
   Torii 将这些复制到生成的 `DaPinIntent` 中，以便下游引脚处理可以绑定别名
   和所有者，无需重新解析原始元数据映射；格式错误或空值在期间被拒绝
   摄取验证。

## 存储/注册表更新

- 使用 `DaManifestV1` 扩展 `sorafs_manifest`，从而实现确定性解析。
- 添加新的注册表流 `da.pin_intent` 以及版本化有效负载引用
  清单哈希 + 票证 ID。
- 更新可观测性管道以跟踪摄取延迟、分块吞吐量、
  复制积压和失败计数。
- Torii `/status` 响应现在包括一个 `taikai_ingest` 数组，该数组显示最新的
  编码器到摄取延迟、实时边缘漂移和每个（集群、流）的错误计数器，支持 DA-9
  仪表板直接从节点获取运行状况快照，而无需抓取 Prometheus。

## 测试策略- 用于模式验证、签名检查、重复检测的单元测试。
- 验证 `DaIngestRequest`、清单和收据的 Norito 编码的黄金测试。
- 集成工具旋转模拟 SoraFS + 注册表，断言块 + 引脚流。
- 涵盖随机擦除配置文件和保留组合的性能测试。
- 对 Norito 有效负载进行模糊测试，以防止格式错误的元数据。
- 每个 Blob 类别的黄金赛程都在下面
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` 与伴随块
  列在 `fixtures/da/ingest/sample_chunk_records.txt` 中。被忽视的测试
  `regenerate_da_ingest_fixtures`刷新灯具，同时
  添加新的 `BlobClass` 变体后，`manifest_fixtures_cover_all_blob_classes` 就会失败
  无需更新 Norito/JSON 包。这使得 Torii、SDK 和文档在 DA-2 时保持诚实
  接受新的斑点表面。【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

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
  `--plan`、`--manifest-id`）**或**只需通过 `--storage-ticket` 传递 Torii 存储票证。当
  使用票证路径 CLI 从 `/v1/da/manifests/<ticket>` 中提取清单，保留捆绑包
  在 `artifacts/da/fetch_<timestamp>/` 下（用 `--manifest-cache-dir` 覆盖），派生出 **manifest
  `--manifest-id` 的 hash**，然后使用提供的 `--gateway-provider` 运行编排器
  列表。有效负载验证仍然依赖于嵌入式 CAR/`blob_hash` 摘要，而网关 ID 为
  现在清单哈希，以便客户端和验证器共享单个 blob 标识符。所有高级旋钮均来自
  SoraFS 获取器表面完好无损（清单信封、客户端标签、保护缓存、匿名传输
  覆盖、记分板导出和 `--output` 路径），并且可以通过以下方式覆盖清单端点
  `--manifest-endpoint` 用于自定义 Torii 主机，因此端到端可用性检查完全在
  `da` 命名空间，无需重复编排器逻辑。
- `iroha app da get-blob` 通过 `GET /v1/da/manifests/{storage_ticket}` 直接从 Torii 提取规范清单。
  该命令现在使用清单哈希（blob id）标记人工制品，写入
  `manifest_{manifest_hash}.norito`、`manifest_{manifest_hash}.json` 和 `chunk_plan_{manifest_hash}.json`
  在 `artifacts/da/fetch_<timestamp>/` （或用户提供的 `--output-dir`）下，同时回显确切的
  后续协调器获取所需的 `iroha app da get` 调用（包括 `--manifest-id`）。
  这使操作员远离清单假脱机目录，并保证获取器始终使用
  Torii 发出的签名工件。 JavaScript Torii 客户端通过以下方式镜像此流程
  `ToriiClient.getDaManifest(storageTicketHex)` 而 Swift SDK 现在公开
  `ToriiClient.getDaManifestBundle(...)`。两者都返回解码后的 Norito 字节、清单 JSON、清单哈希、和块计划，以便 SDK 调用者可以水合编排器会话，而无需使用 CLI 和 Swift
  客户端还可以调用 `fetchDaPayloadViaGateway(...)` 通过本机传输这些包
  SoraFS Orchestrator 包装器。【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v1/da/manifests` 响应现在显示 `manifest_hash` 以及 CLI + SDK 帮助程序（`iroha app da get`、
  `ToriiClient.fetchDaPayloadViaGateway` 和 Swift/JS 网关包装器）将此摘要视为
  规范清单标识符，同时继续根据嵌入的 CAR/blob 哈希验证有效负载。
- `iroha app da rent-quote` 计算所提供存储大小的确定性租金和激励细目
  和保留窗口。帮助程序消耗活动的 `DaRentPolicyV1`（JSON 或 Norito 字节）或
  内置默认值、验证策略并打印 JSON 摘要（`gib`、`months`、策略元数据、
  和 `DaRentQuote` 字段），因此审计人员可以在治理分钟内引用确切的 XOR 费用，而无需
  编写临时脚本。该命令现在还在 JSON 之前发出一行 `rent_quote ...` 摘要
  有效负载，使控制台日志和运行手册在事件期间生成报价时更易于扫描。
  通过 `--quote-out artifacts/da/rent_quotes/<stamp>.json`（或任何其他路径）
  保留打印精美的摘要并在以下情况下使用 `--policy-label "governance ticket #..."`
  artefact 需要引用特定的投票/配置包； CLI 修剪自定义标签并拒绝空白
  使 `policy_source` 值在证据包中保持有意义的字符串。参见
  子命令为 `crates/iroha_cli/src/commands/da.rs` 和 `docs/source/da/rent_policy.md`
  用于策略模式。【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- Pin 注册表奇偶校验现在扩展到 SDK：`ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK 构建 `iroha app sorafs pin register` 使用的确切负载，强制执行规范
  分块器元数据、pin 策略、别名证明和 POST 之前的后继摘要
  `/v1/sorafs/pin/register`。这可以防止 CI 机器人和自动化在以下情况下向 CLI 发起攻击：
  记录清单注册，并且帮助程序附带了 TypeScript/README 覆盖范围，因此 DA-8 的
  JS 和 Rust/Swift 完全满足“提交/获取/证明”工具对等性。【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` 链接以上所有内容：它需要存储票，下载
  规范清单包，针对
  提供的 `--gateway-provider` 列表，将下载的有效负载 + 记分板保留在
  `artifacts/da/prove_availability_<timestamp>/`，并立即调用现有的 PoR 助手
  (`iroha app da prove`) 使用获取的字节。操作员可以调整协调器旋钮
  （`--max-peers`、`--scoreboard-out`、清单端点覆盖）和证明采样器
  （`--sample-count`、`--leaf-index`、`--sample-seed`），而单个命令会生成工件
  DA-5/DA-9 审计所期望的：有效负载副本、记分板证据和 JSON 证明摘要。- `da_reconstruct`（DA-6 中的新增功能）读取规范清单以及块发出的块目录
  存储（`chunk_{index:05}.bin`布局）并在验证时确定性地重新组装有效负载
  每一个 Blake3 的承诺。 CLI 位于 `crates/sorafs_car/src/bin/da_reconstruct.rs` 下并作为
  SoraFS 工具包的一部分。典型流程：
  1. `iroha app da get-blob --storage-ticket <ticket>` 下载 `manifest_<manifest_hash>.norito` 和 chunk plan。
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     （或 `iroha app da prove-availability`，它将获取工件写入
     `artifacts/da/prove_availability_<ts>/` 并将每个块的文件保留在 `chunks/` 目录中）。
  3.`cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`。

  回归装置位于 `fixtures/da/reconstruct/rs_parity_v1/` 下并捕获完整清单
  和 `tests::reconstructs_fixture_with_parity_chunks` 使用的块矩阵（数据 + 奇偶校验）。重新生成它

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  灯具发出：

  - `manifest.{norito.hex,json}` — 规范的 `DaManifestV1` 编码。
  - `chunk_matrix.json` — 用于文档/测试参考的有序索引/偏移/长度/摘要/奇偶校验行。
  - `chunks/` — `chunk_{index:05}.bin` 数据和奇偶校验分片的有效负载切片。
  - `payload.bin` — 奇偶校验感知线束测试使用的确定性有效负载。
  - `commitment_bundle.{json,norito.hex}` — 示例 `DaCommitmentBundle`，具有文档/测试的确定性 KZG 承诺。

  该工具拒绝丢失或截断的块，根据 `blob_hash` 检查最终有效负载 Blake3 哈希，
  并发出一个摘要 JSON blob（有效负载字节、块计数、存储票证），以便 CI 可以断言重建
  证据。这结束了 DA-6 对操作员和 QA 确定性重建工具的要求
  无需连接定制脚本即可调用作业。

## TODO 解决方案摘要

所有先前阻止的摄取 TODO 均已实施并验证：- **压缩提示** — Torii 接受调用者提供的标签（`identity`、`gzip`、`deflate`、
  `zstd`）并在验证之前标准化有效负载，以便规范清单哈希与
  解压后的字节数。【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **仅治理元数据加密** — Torii 现在使用以下方法加密治理元数据
  配置 ChaCha20-Poly1305 密钥，拒绝不匹配的标签，并显示两个显式
  配置旋钮（`torii.da_ingest.governance_metadata_key_hex`，
  `torii.da_ingest.governance_metadata_key_label`) 保持旋转确定性。【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **大负载流** - 多部分摄取是实时的。客户端流确定性
  `DaIngestChunk` 包络由 `client_blob_id` 键入，Torii 验证每个切片，对它们进行暂存
  在 `manifest_store_dir` 下，并在 `is_last` 标志落地后自动重建清单，
  消除单次调用上传时出现的 RAM 峰值。【crates/iroha_torii/src/da/ingest.rs:392】
- **清单版本控制** — `DaManifestV1` 带有显式 `version` 字段，而 Torii 拒绝
  未知版本，保证新清单布局发布时确定性升级。【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR 挂钩** — PDP 承诺直接源自块存储并被持久化
  除了清单之外，DA-5 调度程序可以从规范数据发起采样挑战；的
  `Sora-PDP-Commitment` 接头现在随 `/v1/da/ingest` 和 `/v1/da/manifests/{ticket}` 一起提供
  响应，以便 SDK 立即了解未来探测将参考的已签署承诺。【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】
- **分片游标日志** — 通道元数据可能指定 `da_shard_id`（默认为 `lane_id`），以及
  Sumeragi 现在将每个 `(shard_id, lane_id)` 的最高 `(epoch, sequence)` 保留到
  `da-shard-cursors.norito` 与 DA 线轴并存，因此重新启动会丢弃重新分片/未知的通道并保留
  重播确定性。内存中分片游标索引现在在承诺上快速失败
  未映射的车道而不是默认的车道 ID，导致光标前进和重放错误
  显式的，块验证使用专用的拒绝分片游标回归
  `DaShardCursorViolation` 原因 + 操作员遥测标签。启动/追赶现在停止 DA
  如果 Kura 包含未知车道或回归光标并记录有问题的情况，则索引水合
  块高度，以便操作员可以在服务 DA 之前进行修复state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **分片光标滞后遥测** — `da_shard_cursor_lag_blocks{lane,shard}` 仪表报告如何分片远远落后于正在验证的高度。丢失/陈旧/未知的车道将滞后设置为
  所需的高度（或增量），并且成功的前进将其重置为零，以便稳态保持平坦。
  操作员应对非零滞后发出警报，检查 DA 线轴/日志是否存在违规通道，
  并在重放块之前验证车道目录是否存在意外重新分片以清除
  差距。
- **机密计算通道** — 标记为的通道
  `metadata.confidential_compute=true` 和 `confidential_key_version` 被视为
  SMPC/加密 DA 路径：Sumeragi 强制执行非零负载/清单摘要和存储票据，
  拒绝完整副本存储配置文件，并索引 SoraFS 票证 + 策略版本，无需
  暴露有效负载字节。在重播期间从 Kura 获取收据，以便验证者恢复相同的内容
  重启后的机密性元数据。【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## 实施注意事项- Torii 的 `/v1/da/ingest` 端点现在标准化有效负载压缩，强制重播缓存，
  确定性地对规范字节进行分块，重建 `DaManifestV1`，并删除编码的有效负载
  在发出收据之前进入 `config.da_ingest.manifest_store_dir` 进行 SoraFS 编排；的
  处理程序还附加一个 `Sora-PDP-Commitment` 标头，以便客户端可以捕获编码的承诺
  立即。【crates/iroha_torii/src/da/ingest.rs:220】
- 保留规范的 `DaCommitmentRecord` 后，Torii 现在发出
  清单假脱机旁边的 `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` 文件。
  每个条目将记录与原始 Norito `PdpCommitment` 字节捆绑在一起，因此 DA-3 捆绑构建器和
  DA-5 调度程序摄取相同的输入，无需重新读取清单或块存储。【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK 帮助程序公开 PDP 标头字节，而不强制每个客户端重新实现 Norito 解析：
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` 覆盖 Rust，Python `ToriiClient`
  现在导出 `decode_pdp_commitment_header`，并且 `IrohaSwift` 提供匹配的助手，因此可以移动
  客户端可以立即存储编码的采样计划。【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii 还公开 `GET /v1/da/manifests/{storage_ticket}`，以便 SDK 和操作员可以获取清单
  和块计划而不触及节点的假脱机目录。响应返回 Norito 字节
  (base64)，渲染清单 JSON，为 `sorafs fetch` 准备的 `chunk_plan` JSON blob，以及相关的
  十六进制摘要（`storage_ticket`、`client_blob_id`、`blob_hash`、`chunk_root`），因此下游工具可以
  向编排器提供数据而无需重新计算摘要，并发出相同的 `Sora-PDP-Commitment` 标头
  镜像摄取响应。传递 `block_hash=<hex>` 作为查询参数返回确定性
  `sampling_plan` 植根于 `block_hash || client_blob_id`（在验证器之间共享），其中包含
  `assignment_hash`、请求的 `sample_window` 和采样的 `(index, role, group)` 元组
  整个 2D 条带布局，以便 PoR 采样器和验证器可以重放相同的索引。采样器
  将 `client_blob_id`、`chunk_root` 和 `ipa_commitment` 混合到赋值哈希中； `iroha 应用程序 da get
  --block-hash ` now writes `sampling_plan_.json` 位于清单 + 块计划旁边
  保留的哈希值，并且 JS/Swift Torii 客户端公开相同的 `assignment_hash_hex`，因此验证器
  和证明者共享一个确定性探针集。当 Torii 返回抽样计划时，`iroha app da
  证明可用性` now reuses that deterministic probe set (seed derived from `sample_seed`)
  即席抽样，因此 PoR 见证人与验证者分配保持一致，即使操作员忽略了
  `--block-hash` 覆盖。【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### 大负载流需要摄取大于配置的单次请求限制的资产的客户端发起
通过调用 `POST /v1/da/ingest/chunk/start` 进行流会话。 Torii 响应
`ChunkSessionId`（BLAKE3 - 从请求的 blob 元数据派生）和协商的块大小。
每个后续 `DaIngestChunk` 请求均携带：

- `client_blob_id` — 与最终的 `DaIngestRequest` 相同。
- `chunk_session_id` — 将切片与正在运行的会话联系起来。
- `chunk_index` 和 `offset` — 强制执行确定性排序。
- `payload` — 达到协商的块大小。
- `payload_hash` — 切片的 BLAKE3 哈希，因此 Torii 可以在不缓冲整个 blob 的情况下进行验证。
- `is_last` — 表示终端片。

Torii 在 `config.da_ingest.manifest_store_dir/chunks/<session>/` 下保留经过验证的切片，并且
记录重播缓存内的进度以实现幂等性。当最后一片落地时，Torii
重新组装磁盘上的有效负载（通过块目录流式传输以避免内存峰值），
与单次上传完全相同地计算规范清单/收据，并最终响应
`POST /v1/da/ingest` 通过消耗分阶段工件。失败的会话可以显式中止或
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
- Sumeragi 在密封或验证 DA 捆绑包时强制执行清单可用性：
  如果假脱机缺少清单或哈希值不同，则块验证失败
  来自承诺。【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

请求、清单和接收有效负载的往返覆盖范围在
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`，确保Norito编解码器
在更新中保持稳定。【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**保留默认值。** 治理期间批准了初始保留政策
SF-6； `RetentionPolicy::default()` 强制执行的默认值是：- 热门层：7天（`604_800`秒）
- 冷层：90 天（`7_776_000` 秒）
- 所需副本：`3`
- 存储类别：`StorageClass::Hot`
- 治理标签：`"da.default"`

当车道采用时，下游操作员必须显式覆盖这些值
更严格的要求。

## Rust 客户端证明文物

嵌入 Rust 客户端的 SDK 不再需要通过 CLI 来实现
生成规范的 PoR JSON 包。 `Client` 公开了两个助手：

- `build_da_proof_artifact` 返回由生成的确切结构
  `iroha app da prove --json-out`，包括提供的清单/有效负载注释
  通过 [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` 包装构建器并将工件保存到磁盘
  （默认情况下漂亮的 JSON + 尾随换行符）因此自动化可以附加文件
  发布或治理证据包。【crates/iroha/src/client.rs:3653】

### 示例

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

留下帮助程序的 JSON 有效负载与 CLI 匹配到字段名称
（`manifest_path`、`payload_path`、`proofs[*].chunk_digest`等），所以现有
自动化可以在没有特定于格式的分支的情况下比较/镶木地板/上传文件。

## 证明验证基准

使用 DA 证明基准工具来验证验证者对代表性有效负载的预算
收紧区块级别上限：

- `cargo xtask da-proof-bench` 从清单/有效负载对重建块存储，对 PoR 进行采样
  离开，并根据配置的预算进行时间验证。 Taikai 元数据是自动填充的，并且
  如果夹具对不一致，则线束会退回到合成清单。当 `--payload-bytes`
  在没有显式 `--payload` 的情况下设置，生成的 blob 被写入
  `artifacts/da/proof_bench/payload.bin` 所以灯具保持不变。【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- 报告默认为 `artifacts/da/proof_bench/benchmark.{json,md}`，包括校样/运行、总计和
  每个证明的时间、预算通过率和建议的预算（最慢迭代的 110%）
  与`zk.halo2.verifier_budget_ms`对齐。【artifacts/da/proof_bench/benchmark.md:1】
- 最新运行（合成 1 MiB 有效负载、64 KiB 块、32 次验证/运行、10 次迭代、250 毫秒预算）
  建议验证器预算为 3 ms，迭代次数在上限内 100%。【artifacts/da/proof_bench/benchmark.md:1】
- 示例（生成确定性有效负载并写入两个报告）：

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

块装配强制执行相同的预算：`sumeragi.da_max_commitments_per_block` 和
`sumeragi.da_max_proof_openings_per_block` 在将 DA 束嵌入到块中之前对其进行门控，并且
每个承诺必须带有非零的 `proof_digest`。守卫将束长度视为
证明开放计数，直到明确的证明摘要通过共识，保持
≤128-在块边界可执行的开放目标。【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR失败处理和削减存储工作人员现在会提出 PoR 失败条纹，并在每个失败条纹旁边提供绑定斜杠建议
判决。高于配置的罢工阈值的连续失败会发出建议：
包括提供者/清单对、触发斜杠的条纹长度以及建议的
根据提供商保证金和 `penalty_bond_bps` 计算的罚款；冷却时间（秒）保持
在同一事件中触发重复的斜杠。【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- 通过存储工作构建器配置阈值/冷却时间（默认值反映治理
  处罚政策）。
- Slash 建议记录在判决摘要 JSON 中，以便治理/审计人员可以附加
  将它们打包成证据。
- 条带布局 + 每块角色现在通过 Torii 的存储引脚端点进行线程化
  （`stripe_layout` + `chunk_roles` 字段）并持久保存到存储工作线程中，以便
  审核员/修复工具可以计划行/列修复，而无需从上游重新导出布局

### 放置+修复线束

现在 `cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>`
通过 `(index, role, stripe/column, offsets)` 计算放置哈希并执行行优先，然后
在重建有效负载之前修复列 RS(16)：

- 当存在时，放置默认为 `total_stripes`/`shards_per_stripe` 并回退到块
- 首先使用行奇偶校验重建丢失/损坏的块；剩余的间隙被修复
  条带（列）奇偶校验。修复后的 chunk 被写回 chunk 目录，并且 JSON
  摘要捕获放置哈希加上行/列修复计数器。
- 如果行+列奇偶校验不能满足缺失集，则线束会快速失败并无法恢复
  索引，以便审计员可以标记无法修复的清单。