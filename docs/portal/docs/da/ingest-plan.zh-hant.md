---
lang: zh-hant
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 710286691d09a5707829a36ca98ed24a6af5c5629e708dd7b1bd0f01db4e31c1
source_last_modified: "2026-01-22T14:35:36.737834+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

標題：數據可用性攝取計劃
sidebar_label：攝取計劃
描述：Torii blob 攝取的架構、API 表面和驗證計劃。
---

:::注意規範來源
:::

# Sora Nexus 數據可用性攝取計劃

_起草時間：2026-02-20 - 所有者：核心協議工作組/存儲團隊/DA 工作組_

DA-2 工作流使用發出 Norito 的 Blob 攝取 API 擴展了 Torii
元數據和種子 SoraFS 複製。本文件涵蓋了擬議的
模式、API 表面和驗證流程，因此實施無需
阻止未完成的模擬（DA-1 後續行動）。所有有效負載格式必須
使用 Norito 編解碼器；不允許使用 serde/JSON 後備。

## 目標

- 接受大斑點（Taikai 段、車道邊車、治理工件）
  確定性超過 Torii。
- 生成描述 blob、編解碼器參數的規範 Norito 清單，
  擦除配置文件和保留策略。
- 將塊元數據保留在 SoraFS 熱存儲和排隊複製作業中。
- 將 pin 意圖 + 策略標籤發佈到 SoraFS 註冊表和治理
  觀察員。
- 公開入場收據，以便客戶重新獲得確定性的出版證據。

## API 表面 (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

有效負載是 Norito 編碼的 `DaIngestRequest`。響應使用
`application/norito+v1` 並返回 `DaIngestReceipt`。

|回應 |意義|
| ---| ---|
| 202 已接受 | Blob 排隊等待分塊/複製；收據退回。 |
| 400 錯誤請求 |架構/大小違規（請參閱驗證檢查）。 |
| 401 未經授權 | API 令牌缺失/無效。 |
| 409 衝突 |重複的 `client_blob_id` 元數據不匹配。 |
| 413 有效負載太大 |超出配置的 blob 長度限制。 |
| 429 太多請求 |達到速率限制。 |
| 500 內部錯誤 |意外失敗（記錄+警報）。 |

## 提議的 Norito 架構

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

> 實現說明：這些有效負載的規範 Rust 表示現在位於
> `iroha_data_model::da::types`，帶有 `iroha_data_model::da::ingest` 中的請求/收據包裝器
> 以及 `iroha_data_model::da::manifest` 中的清單結構。

`compression` 字段通告調用者如何準備有效負載。 Torii 接受
`identity`、`gzip`、`deflate`、`zstd`，透明解壓之前的字節
散列、分塊和驗證可選清單。

### 驗證清單

1. 驗證請求 Norito 標頭是否與 `DaIngestRequest` 匹配。
2. 如果 `total_size` 與規範（解壓）有效負載長度不同或超過配置的最大負載長度，則失敗。
3. 強制 `chunk_size` 對齊（二的冪，<= 2 MiB）。
4. 確保 `data_shards + parity_shards` <= 全局最大值且奇偶校驗 >= 2。
5. `retention_policy.required_replica_count` 必須尊重治理基線。
6. 針對規范哈希的簽名驗證（不包括簽名字段）。
7. 拒絕重複的 `client_blob_id`，除非有效負載哈希 + 元數據相同。
8. 當提供 `norito_manifest` 時，驗證模式 + 重新計算的哈希匹配
   分塊後顯現；否則節點生成清單並存儲它。
9. 強制執行配置的複制策略：Torii 重寫提交的
   `RetentionPolicy` 和 `torii.da_ingest.replication_policy`（參見
   `replication-policy.md`）並拒絕保留其保留的預建清單
   元數據與強製配置文件不匹配。

### 分塊和復制流程

1. 將有效負載分塊到 `chunk_size` 中，計算每個塊的 BLAKE3 + Merkle 根。
2. 構建 Norito `DaManifestV1` （新結構）捕獲塊承諾（role/group_id），
   擦除佈局（行和列奇偶校驗計數加上 `ipa_commitment`）、保留策略、
   和元數據。
3. 將規范清單字節排列在 `config.da_ingest.manifest_store_dir` 下
   （Torii 寫入由通道/紀元/序列/票據/指紋鍵入的 `manifest.encoded` 文件）因此 SoraFS
   編排可以攝取它們並將存儲票證鏈接到持久數據。
4. 通過 `sorafs_car::PinIntent` 使用治理標籤 + 策略發布 pin 意圖。
5. 發出 Norito 事件 `DaIngestPublished` 通知觀察者（輕客戶端、
   治理、分析）。
6. 將 `DaIngestReceipt` 返回給調用者（由 Torii DA 服務密鑰簽名）並發出
   `Sora-PDP-Commitment` 標頭，以便 SDK 可以立即捕獲編碼的承諾。收據
   現在包括 `rent_quote`（Norito `DaRentQuote`）和 `stripe_layout`，讓提交者顯示
   基本租金、儲備份額、PDP/PoTR 獎金預期以及 2D 擦除佈局
   提交資金之前的存儲票。

## 存儲/註冊表更新

- 使用 `DaManifestV1` 擴展 `sorafs_manifest`，從而實現確定性解析。
- 添加新的註冊表流 `da.pin_intent` 以及版本化有效負載引用
  清單哈希 + 票證 ID。
- 更新可觀測性管道以跟踪攝取延遲、分塊吞吐量、
  複製積壓和失敗計數。

## 測試策略

- 用於模式驗證、簽名檢查、重複檢測的單元測試。
- 驗證 `DaIngestRequest`、清單和收據的 Norito 編碼的黃金測試。
- 集成工具旋轉模擬 SoraFS + 註冊表，斷言塊 + 引腳流。
- 涵蓋隨機擦除配置文件和保留組合的性能測試。
- 對 Norito 有效負載進行模糊測試，以防止格式錯誤的元數據。

## CLI 和 SDK 工具 (DA-8)- `iroha app da submit`（新的 CLI 入口點）現在包裝共享攝取構建器/發佈器，以便操作員
  可以攝取 Taikai 束流之外的任意斑點。該命令位於
  `crates/iroha_cli/src/commands/da.rs:1` 並消耗有效負載、擦除/保留配置文件，以及
  使用 CLI 簽署規範 `DaIngestRequest` 之前的可選元數據/清單文件
  配置鍵。成功運行後仍保留 `da_request.{norito,json}` 和 `da_receipt.{norito,json}`
  `artifacts/da/submission_<timestamp>/`（通過 `--artifact-dir` 覆蓋），因此釋放工件可以
  記錄攝取期間使用的確切 Norito 字節。
- 該命令默認為 `client_blob_id = blake3(payload)` 但接受覆蓋
  `--client-blob-id`，支持元數據 JSON 映射 (`--metadata-json`) 和預生成的清單
  (`--manifest`)，並支持`--no-submit`用於離線準備以及`--endpoint`用於定制
  Torii 主機。收據 JSON 除了寫入磁盤之外，還會打印到 stdout，從而關閉
  DA-8“submit_blob”工具要求和解鎖 SDK 奇偶校驗工作。
- `iroha app da get` 為已經支持的多源協調器添加了一個以 DA 為中心的別名
  `iroha app sorafs fetch`。操作員可以將其指向清單+塊計劃工件（`--manifest`，
  `--plan`、`--manifest-id`)**或**通過 `--storage-ticket` 傳遞 Torii 存儲票證。買票的時候
  使用 CLI 從 `/v1/da/manifests/<ticket>` 中提取清單的路徑，將捆綁包保留在
  `artifacts/da/fetch_<timestamp>/`（用 `--manifest-cache-dir` 覆蓋），派生 blob 哈希
  `--manifest-id`，然後使用提供的 `--gateway-provider` 列表運行協調器。全部
  SoraFS 提取器表面的高級旋鈕完好無損（清單信封、客戶標籤、保護緩存、
  匿名傳輸覆蓋、記分板導出和 `--output` 路徑），並且清單端點可以
  對於自定義 Torii 主機，可通過 `--manifest-endpoint` 覆蓋，因此實時進行端到端可用性檢查
  完全位於 `da` 命名空間下，無需重複編排器邏輯。
- `iroha app da get-blob` 通過 `GET /v1/da/manifests/{storage_ticket}` 直接從 Torii 提取規范清單。
  該命令寫入 `manifest_{ticket}.norito`、`manifest_{ticket}.json` 和 `chunk_plan_{ticket}.json`
  在 `artifacts/da/fetch_<timestamp>/` （或用戶提供的 `--output-dir`）下，同時回顯確切的
  後續協調器獲取所需的 `iroha app da get` 調用（包括 `--manifest-id`）。
  這使操作員遠離清單假脫機目錄，並保證獲取器始終使用
  Torii 發出的簽名工件。 JavaScript Torii 客戶端通過以下方式鏡像此流程
  `ToriiClient.getDaManifest(storageTicketHex)`，返回解碼後的 Norito 字節，清單 JSON，
  和塊計劃，以便 SDK 調用者可以補充編排器會話，而無需支付 CLI 費用。
  Swift SDK 現在公開相同的表面（`ToriiClient.getDaManifestBundle(...)` 加上
  `fetchDaPayloadViaGateway(...)`)，管道束到本機 SoraFS 協調器包裝器中，以便
  iOS 客戶端可以下載清單、執行多源獲取並捕獲證據，而無需
  調用CLI。 【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` 計算所提供存儲大小的確定性租金和激勵細目
  和保留窗口。幫助程序消耗活動的 `DaRentPolicyV1`（JSON 或 Norito 字節）或
  內置默認值、驗證策略並打印 JSON 摘要（`gib`、`months`、策略元數據、
  和 `DaRentQuote` 字段），因此審計人員可以在治理分鐘內引用確切的 XOR 費用，而無需
  編寫臨時腳本。該命令還在 JSON 之前發出一行 `rent_quote ...` 摘要
  在事件演習期間保持控制台日誌可讀的有效負載。將 `--quote-out artifacts/da/rent_quotes/<stamp>.json` 與
  `--policy-label "governance ticket #..."` 保留引用確切政策投票的美化文物
  或配置包； CLI 修剪自定義標籤並拒絕空白字符串，因此 `policy_source` 值
  在財務儀表板中保持可操作性。有關子命令，請參閱 `crates/iroha_cli/src/commands/da.rs`
  策略模式為 `docs/source/da/rent_policy.md`。 【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` 鏈接以上所有內容：它需要一張存儲票，下載
  規范清單包，針對
  提供的 `--gateway-provider` 列表，將下載的有效負載 + 記分板保留在
  `artifacts/da/prove_availability_<timestamp>/`，並立即調用現有的 PoR 助手
  (`iroha app da prove`) 使用獲取的字節。操作員可以調整協調器旋鈕
  （`--max-peers`、`--scoreboard-out`、清單端點覆蓋）和證明採樣器
  （`--sample-count`、`--leaf-index`、`--sample-seed`），而單個命令會生成工件
  DA-5/DA-9 審計所期望的：有效負載副本、記分板證據和 JSON 證明摘要。

## TODO 解決方案摘要

所有先前阻止的攝取 TODO 均已實施並驗證：

- **壓縮提示** — Torii 接受調用者提供的標籤（`identity`、`gzip`、`deflate`、
  `zstd`）並在驗證之前標準化有效負載，以便規范清單哈希與
  解壓後的字節數。 【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **僅治理元數據加密** — Torii 現在使用以下方法加密治理元數據
  配置 ChaCha20-Poly1305 密鑰，拒絕不匹配的標籤，並顯示兩個顯式
  配置旋鈕（`torii.da_ingest.governance_metadata_key_hex`，
  `torii.da_ingest.governance_metadata_key_label`) 保持旋轉確定性。 【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **大負載流** - 多部分攝取是實時的。客戶端流確定性
  `DaIngestChunk` 包絡由 `client_blob_id` 鍵入，Torii 驗證每個切片並對其進行暫存
  在 `manifest_store_dir` 下，並在 `is_last` 標誌落地後自動重建清單，
  消除單次調用上傳時出現的 RAM 峰值。 【crates/iroha_torii/src/da/ingest.rs:392】
- **清單版本控制** — `DaManifestV1` 帶有顯式 `version` 字段，而 Torii 拒絕
  未知版本，保證新清單佈局發佈時確定性升級。 【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR 掛鉤** — PDP 承諾直接源自塊存儲並被持久化
  除了清單之外，DA-5 調度程序可以從規範數據發起採樣挑戰，以及
  `/v1/da/ingest` 和 `/v1/da/manifests/{ticket}` 現在包含 `Sora-PDP-Commitment` 標頭
  攜帶base64 Norito有效負載，以便SDK緩存確切的承諾DA-5探針目標。 【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## 實施注意事項

- Torii 的 `/v1/da/ingest` 端點現在標準化有效負載壓縮，強制重播緩存，
  確定性地對規範字節進行分塊，重建 `DaManifestV1`，刪除編碼的有效負載
  到 `config.da_ingest.manifest_store_dir` 中進行 SoraFS 編排，並添加 `Sora-PDP-Commitment`
  標頭，以便操作員捕獲 PDP 調度程序將引用的承諾。 【crates/iroha_torii/src/da/ingest.rs:220】
- 每個接受的 blob 現在都會生成 `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito`
  `manifest_store_dir` 下的條目將規範 `DaCommitmentRecord` 與原始數據捆綁在一起
  `PdpCommitmentV1` 字節，因此 DA-3 捆綁構建器和 DA-5 調度程序可以混合相同的輸入，而無需
  重新讀取清單或塊存儲。 【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK 幫助程序 API 公開 PDP 標頭有效負載，而不強制調用者重新實現 Norito 解碼：
  Rust 箱導出 `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`，Python
  `ToriiClient` 現在包括 `decode_pdp_commitment_header`，並且 `IrohaSwift` 已發貨
  原始標頭映射或 `HTTPURLResponse` 實例的 `decodePdpCommitmentHeader` 重載。 【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii 還公開 `GET /v1/da/manifests/{storage_ticket}`，以便 SDK 和操作員可以獲取清單
  和塊計劃而不觸及節點的假脫機目錄。響應返回 Norito 字節
  (base64)，渲染清單 JSON，為 `sorafs fetch` 準備的 `chunk_plan` JSON blob，相關
  十六進制摘要（`storage_ticket`、`client_blob_id`、`blob_hash`、`chunk_root`），並鏡像
  來自攝取響應的 `Sora-PDP-Commitment` 標頭以進行奇偶校驗。提供 `block_hash=<hex>`
  查詢字符串返回確定性 `sampling_plan`（分配哈希、`sample_window` 和採樣
  `(index, role, group)` 元組跨越完整的 2D 佈局），因此驗證器和 PoR 工具繪製相同
  指數。

### 大負載流

需要攝取大於配置的單次請求限制的資產的客戶端發起
通過調用 `POST /v1/da/ingest/chunk/start` 進行流會話。 Torii 響應
`ChunkSessionId`（BLAKE3 - 從請求的 blob 元數據派生）和協商的塊大小。
每個後續 `DaIngestChunk` 請求都攜帶：- `client_blob_id` — 與最終的 `DaIngestRequest` 相同。
- `chunk_session_id` — 將切片與正在運行的會話聯繫起來。
- `chunk_index` 和 `offset` — 強制執行確定性排序。
- `payload` — 達到協商的塊大小。
- `payload_hash` — 切片的 BLAKE3 哈希，因此 Torii 可以在不緩衝整個 blob 的情況下進行驗證。
- `is_last` — 表示終端片。

Torii 在 `config.da_ingest.manifest_store_dir/chunks/<session>/` 下保留經過驗證的切片，並且
記錄重播緩存內的進度以實現冪等性。當最後一片落地時，Torii
重新組裝磁盤上的有效負載（通過塊目錄流式傳輸以避免內存峰值），
與單次上傳完全相同地計算規范清單/收據，並最終響應
`POST /v1/da/ingest` 通過消耗分階段工件。失敗的會話可以顯式中止或
在 `config.da_ingest.replay_cache_ttl` 之後被垃圾收集。這種設計保留了網絡格式
Norito 友好，避免客戶端特定的可恢復協議，並重用現有的清單管道
不變。

**實施狀態。 ** 規範的 Norito 類型現在位於
`crates/iroha_data_model/src/da/`：

- `ingest.rs` 定義了 `DaIngestRequest`/`DaIngestReceipt`，以及
  Torii使用的`ExtraMetadata`容器。 【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` 託管 `DaManifestV1` 和 `ChunkCommitment`，Torii 之後發出
  分塊完成。 【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` 提供共享別名（`BlobDigest`、`RetentionPolicy`、
  `ErasureProfile`等）並對下面記錄的默認策略值進行編碼。 【crates/iroha_data_model/src/da/types.rs:240】
- 清單假脫機文件位於 `config.da_ingest.manifest_store_dir` 中，為 SoraFS 編排做好準備
  觀察者拉入存儲入場。 【crates/iroha_torii/src/da/ingest.rs:220】

請求、清單和接收有效負載的往返覆蓋範圍在
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`，確保Norito編解碼器
在更新中保持穩定。 【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**保留默認值。 ** 治理期間批准了初始保留政策
SF-6； `RetentionPolicy::default()` 強制執行的默認值是：

- 熱門層：7天（`604_800`秒）
- 冷層：90 天（`7_776_000` 秒）
- 所需副本：`3`
- 存儲類別：`StorageClass::Hot`
- 治理標籤：`"da.default"`

當車道採用時，下游操作員必須顯式覆蓋這些值
更嚴格的要求。