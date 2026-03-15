---
lang: zh-hant
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

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
- 在 SoraFS 熱存儲和排隊複製作業中保留塊元數據。
- 將 pin 意圖 + 策略標籤發佈到 SoraFS 註冊表和治理
  觀察員。
- 公開入場收據，以便客戶重新獲得確定性的出版證據。

## API 表面 (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

有效負載是 Norito 編碼的 `DaIngestRequest`。響應使用
`application/norito+v1` 並返回 `DaIngestReceipt`。

|回應 |意義|
| --- | --- |
| 202 已接受 | Blob 排隊等待分塊/複製；收據退回。 |
| 400 錯誤請求 |架構/大小違規（請參閱驗證檢查）。 |
| 401 未經授權 | API 令牌缺失/無效。 |
| 409 衝突 |重複的 `client_blob_id` 元數據不匹配。 |
| 413 有效負載太大 |超出配置的 blob 長度限制。 |
| 429 太多請求 |達到速率限制。 |
| 500 內部錯誤 |意外失敗（記錄+警報）。 |

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

返回從當前通道目錄派生的版本化 `DaProofPolicyBundle`。
該捆綁包通告 `version`（當前為 `1`），即 `policy_hash`（
有序策略列表），以及攜帶 `lane_id`、`dataspace_id` 的 `policies` 條目，
`alias`，以及強制執行的 `proof_scheme`（今天的 `merkle_sha256`；KZG 通道是
在 KZG 承諾可用之前，會被攝取拒絕）。現在的區塊頭
通過 `da_proof_policies_hash` 提交到捆綁包，以便客戶端可以固定
驗證 DA 承諾或證明時設置的主動策略。獲取這個端點
在建立證據以確保它們符合車道的政策和當前的情況之前
捆綁哈希。承諾列表/證明端點具有相同的捆綁包，因此 SDK
不需要額外的往返來將證明綁定到活動策略集。

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

返回 `DaProofPolicyBundle`，其中包含有序策略列表以及
`policy_hash`，因此 SDK 可以固定生成塊時使用的版本。的
哈希是通過 Norito 編碼的策略數組計算的，並且每當
Lane 的 `proof_scheme` 已更新，允許客戶檢測之間的漂移
緩存的證明和鏈配置。

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
```> 實現說明：這些有效負載的規範 Rust 表示現在位於
> `iroha_data_model::da::types`，帶有 `iroha_data_model::da::ingest` 中的請求/收據包裝器
> 以及 `iroha_data_model::da::manifest` 中的清單結構。

`compression` 字段通告調用者如何準備有效負載。 Torii 接受
`identity`、`gzip`、`deflate`、`zstd`，透明解壓之前的字節
散列、分塊和驗證可選清單。

### 驗證清單

1. 驗證請求 Norito 標頭是否與 `DaIngestRequest` 匹配。
2. 如果 `total_size` 與規範（解壓縮）有效負載長度不同或超過配置的最大負載長度，則失敗。
3. 強制 `chunk_size` 對齊（二的冪，= 2。
5. `retention_policy.required_replica_count` 必須尊重治理基線。
6. 針對規范哈希的簽名驗證（不包括簽名字段）。
7. 拒絕重複的 `client_blob_id`，除非有效負載哈希 + 元數據相同。
8. 當提供 `norito_manifest` 時，驗證架構 + 重新計算的哈希匹配
   分塊後顯現；否則節點生成清單並存儲它。
9. 強制執行配置的複制策略：Torii 重寫提交的
   `RetentionPolicy` 和 `torii.da_ingest.replication_policy`（參見
   `replication_policy.md`）並拒絕保留其保留的預建清單
   元數據與強製配置文件不匹配。

### 分塊和復制流程1. 將有效負載分塊到 `chunk_size` 中，計算每個塊的 BLAKE3 + Merkle 根。
2. 構建 Norito `DaManifestV1`（新結構）捕獲塊承諾（role/group_id），
   擦除佈局（行和列奇偶校驗計數加上 `ipa_commitment`）、保留策略、
   和元數據。
3. 將規范清單字節排列在 `config.da_ingest.manifest_store_dir` 下
   （Torii 寫入由通道/紀元/序列/票據/指紋鍵入的 `manifest.encoded` 文件）因此 SoraFS
   編排可以攝取它們並將存儲票證鏈接到持久數據。
4. 通過 `sorafs_car::PinIntent` 使用治理標籤 + 策略發布 pin 意圖。
5. 發出 Norito 事件 `DaIngestPublished` 通知觀察者（輕客戶端、
   治理、分析）。
6. 返回 `DaIngestReceipt`（由 Torii DA 服務密鑰簽名）並添加
   `Sora-PDP-Commitment` 響應標頭包含 base64 Norito 編碼
   派生的承諾，以便 SDK 可以立即隱藏採樣種子。
   收據現在嵌入 `rent_quote`（`DaRentQuote`）和 `stripe_layout`
   因此提交者可以提出 XOR 義務、儲備份額、PDP/PoTR 獎金預期、
   以及提交資金之前的 2D 擦除矩陣維度以及存儲票據元數據。
7. 可選的註冊表元數據：
   - `da.registry.alias` — 公共、未加密的 UTF-8 別名字符串，用於為 pin 註冊表項提供種子。
   - `da.registry.owner` — 用於記錄註冊表所有權的公共、未加密的 `AccountId` 字符串。
   Torii 將這些複製到生成的 `DaPinIntent` 中，以便下游引腳處理可以綁定別名
   和所有者，無需重新解析原始元數據映射；格式錯誤或空值在期間被拒絕
   攝取驗證。

## 存儲/註冊表更新

- 使用 `DaManifestV1` 擴展 `sorafs_manifest`，從而實現確定性解析。
- 添加新的註冊表流 `da.pin_intent` 以及版本化有效負載引用
  清單哈希 + 票證 ID。
- 更新可觀測性管道以跟踪攝取延遲、分塊吞吐量、
  複製積壓和失敗計數。
- Torii `/status` 響應現在包括一個 `taikai_ingest` 數組，該數組顯示最新的
  編碼器到攝取延遲、實時邊緣漂移和每個（集群、流）的錯誤計數器，支持 DA-9
  儀表板直接從節點獲取運行狀況快照，而無需抓取 Prometheus。

## 測試策略- 用於模式驗證、簽名檢查、重複檢測的單元測試。
- 驗證 `DaIngestRequest`、清單和收據的 Norito 編碼的黃金測試。
- 集成工具旋轉模擬 SoraFS + 註冊表，斷言塊 + 引腳流。
- 涵蓋隨機擦除配置文件和保留組合的性能測試。
- 對 Norito 有效負載進行模糊測試，以防止格式錯誤的元數據。
- 每個 Blob 類別的黃金賽程都在下面
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` 與伴隨塊
  列在 `fixtures/da/ingest/sample_chunk_records.txt` 中。被忽視的測試
  `regenerate_da_ingest_fixtures`刷新燈具，同時
  添加新的 `BlobClass` 變體後，`manifest_fixtures_cover_all_blob_classes` 就會失敗
  無需更新 Norito/JSON 包。這使得 Torii、SDK 和文檔在 DA-2 時保持誠實
  接受新的斑點表面。 【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

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
  `--plan`、`--manifest-id`）**或**只需通過 `--storage-ticket` 傳遞 Torii 存儲票證。當
  使用票證路徑 CLI 從 `/v2/da/manifests/<ticket>` 中提取清單，保留捆綁包
  在 `artifacts/da/fetch_<timestamp>/` 下（用 `--manifest-cache-dir` 覆蓋），派生出 **manifest
  `--manifest-id` 的 hash**，然後使用提供的 `--gateway-provider` 運行編排器
  列表。有效負載驗證仍然依賴於嵌入式 CAR/`blob_hash` 摘要，而網關 ID 為
  現在清單哈希，以便客戶端和驗證器共享單個 blob 標識符。所有高級旋鈕均來自
  SoraFS 獲取器表面完好無損（清單信封、客戶端標籤、保護緩存、匿名傳輸
  覆蓋、記分板導出和 `--output` 路徑），並且可以通過以下方式覆蓋清單端點
  `--manifest-endpoint` 用於自定義 Torii 主機，因此端到端可用性檢查完全在
  `da` 命名空間，無需重複編排器邏輯。
- `iroha app da get-blob` 通過 `GET /v2/da/manifests/{storage_ticket}` 直接從 Torii 提取規范清單。
  該命令現在使用清單哈希（blob id）標記人工製品，寫入
  `manifest_{manifest_hash}.norito`、`manifest_{manifest_hash}.json` 和 `chunk_plan_{manifest_hash}.json`
  在 `artifacts/da/fetch_<timestamp>/` （或用戶提供的 `--output-dir`）下，同時回顯確切的
  後續協調器獲取所需的 `iroha app da get` 調用（包括 `--manifest-id`）。
  這使操作員遠離清單假脫機目錄，並保證獲取器始終使用
  Torii 發出的簽名工件。 JavaScript Torii 客戶端通過以下方式鏡像此流程
  `ToriiClient.getDaManifest(storageTicketHex)` 而 Swift SDK 現在公開
  `ToriiClient.getDaManifestBundle(...)`。兩者都返回解碼後的 Norito 字節、清單 JSON、清單哈希、和塊計劃，以便 SDK 調用者可以水合編排器會話，而無需使用 CLI 和 Swift
  客戶端還可以調用 `fetchDaPayloadViaGateway(...)` 通過本機傳輸這些包
  SoraFS Orchestrator 包裝器。 【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v2/da/manifests` 響應現在顯示 `manifest_hash` 以及 CLI + SDK 幫助程序（`iroha app da get`、
  `ToriiClient.fetchDaPayloadViaGateway` 和 Swift/JS 網關包裝器）將此摘要視為
  規范清單標識符，同時繼續根據嵌入的 CAR/blob 哈希驗證有效負載。
- `iroha app da rent-quote` 計算所提供存儲大小的確定性租金和激勵細目
  和保留窗口。幫助程序消耗活動的 `DaRentPolicyV1`（JSON 或 Norito 字節）或
  內置默認值、驗證策略並打印 JSON 摘要（`gib`、`months`、策略元數據、
  和 `DaRentQuote` 字段），因此審計人員可以在治理分鐘內引用確切的 XOR 費用，而無需
  編寫臨時腳本。該命令現在還在 JSON 之前發出一行 `rent_quote ...` 摘要
  有效負載，使控制台日誌和運行手冊在事件期間生成報價時更易於掃描。
  通過 `--quote-out artifacts/da/rent_quotes/<stamp>.json`（或任何其他路徑）
  保留打印精美的摘要並在以下情況下使用 `--policy-label "governance ticket #..."`
  artefact 需要引用特定的投票/配置包； CLI 修剪自定義標籤並拒絕空白
  使 `policy_source` 值在證據包中保持有意義的字符串。參見
  子命令為 `crates/iroha_cli/src/commands/da.rs` 和 `docs/source/da/rent_policy.md`
  用於策略模式。 【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- Pin 註冊表奇偶校驗現在擴展到 SDK：`ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK 構建 `iroha app sorafs pin register` 使用的確切負載，強制執行規範
  分塊器元數據、pin 策略、別名證明和 POST 之前的後繼摘要
  `/v2/sorafs/pin/register`。這可以防止 CI 機器人和自動化在以下情況下向 CLI 發起攻擊：
  記錄清單註冊，並且幫助程序附帶了 TypeScript/README 覆蓋範圍，因此 DA-8 的
  JS 和 Rust/Swift 完全滿足“提交/獲取/證明”工具對等性。 【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` 鏈接以上所有內容：它需要存儲票，下載
  規范清單包，針對
  提供的 `--gateway-provider` 列表，將下載的有效負載 + 記分板保留在
  `artifacts/da/prove_availability_<timestamp>/`，並立即調用現有的 PoR 助手
  (`iroha app da prove`) 使用獲取的字節。操作員可以調整協調器旋鈕
  （`--max-peers`、`--scoreboard-out`、清單端點覆蓋）和證明採樣器
  （`--sample-count`、`--leaf-index`、`--sample-seed`），而單個命令會生成工件
  DA-5/DA-9 審計所期望的：有效負載副本、記分板證據和 JSON 證明摘要。- `da_reconstruct`（DA-6 中的新增功能）讀取規范清單以及塊發出的塊目錄
  存儲（`chunk_{index:05}.bin`佈局）並在驗證時確定性地重新組裝有效負載
  每一個 Blake3 的承諾。 CLI 位於 `crates/sorafs_car/src/bin/da_reconstruct.rs` 下並作為
  SoraFS 工具包的一部分。典型流程：
  1. `iroha app da get-blob --storage-ticket <ticket>` 下載 `manifest_<manifest_hash>.norito` 和 chunk plan。
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     （或 `iroha app da prove-availability`，它將獲取工件寫入
     `artifacts/da/prove_availability_<ts>/` 並將每個塊的文件保留在 `chunks/` 目錄中）。
  3.`cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`。

  回歸裝置位於 `fixtures/da/reconstruct/rs_parity_v1/` 下並捕獲完整清單
  和 `tests::reconstructs_fixture_with_parity_chunks` 使用的塊矩陣（數據 + 奇偶校驗）。重新生成它

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  燈具發出：

  - `manifest.{norito.hex,json}` — 規範的 `DaManifestV1` 編碼。
  - `chunk_matrix.json` — 用於文檔/測試參考的有序索引/偏移/長度/摘要/奇偶校驗行。
  - `chunks/` — `chunk_{index:05}.bin` 數據和奇偶校驗分片的有效負載切片。
  - `payload.bin` — 奇偶校驗感知線束測試使用的確定性有效負載。
  - `commitment_bundle.{json,norito.hex}` — 示例 `DaCommitmentBundle`，具有文檔/測試的確定性 KZG 承諾。

  該工具拒絕丟失或截斷的塊，根據 `blob_hash` 檢查最終有效負載 Blake3 哈希，
  並發出一個摘要 JSON blob（有效負載字節、塊計數、存儲票證），以便 CI 可以斷言重建
  證據。這結束了 DA-6 對操作員和 QA 確定性重建工具的要求
  無需連接定制腳本即可調用作業。

## TODO 解決方案摘要

所有先前阻止的攝取 TODO 均已實施並驗證：- **壓縮提示** — Torii 接受調用者提供的標籤（`identity`、`gzip`、`deflate`、
  `zstd`）並在驗證之前標準化有效負載，以便規范清單哈希與
  解壓後的字節數。 【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **僅治理元數據加密** — Torii 現在使用以下方法加密治理元數據
  配置 ChaCha20-Poly1305 密鑰，拒絕不匹配的標籤，並顯示兩個顯式
  配置旋鈕（`torii.da_ingest.governance_metadata_key_hex`，
  `torii.da_ingest.governance_metadata_key_label`) 保持旋轉確定性。 【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **大負載流** - 多部分攝取是實時的。客戶端流確定性
  `DaIngestChunk` 包絡由 `client_blob_id` 鍵入，Torii 驗證每個切片，對它們進行暫存
  在 `manifest_store_dir` 下，並在 `is_last` 標誌落地後自動重建清單，
  消除單次調用上傳時出現的 RAM 峰值。 【crates/iroha_torii/src/da/ingest.rs:392】
- **清單版本控制** — `DaManifestV1` 帶有顯式 `version` 字段，而 Torii 拒絕
  未知版本，保證新清單佈局發佈時確定性升級。 【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR 掛鉤** — PDP 承諾直接源自塊存儲並被持久化
  除了清單之外，DA-5 調度程序可以從規範數據發起採樣挑戰；的
  `Sora-PDP-Commitment` 接頭現在隨 `/v2/da/ingest` 和 `/v2/da/manifests/{ticket}` 一起提供
  響應，以便 SDK 立即了解未來探測將參考的已簽署承諾。 【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】
- **分片游標日誌** — 通道元數據可能指定 `da_shard_id`（默認為 `lane_id`），以及
  Sumeragi 現在將每個 `(shard_id, lane_id)` 的最高 `(epoch, sequence)` 保留到
  `da-shard-cursors.norito` 與 DA 線軸並存，因此重新啟動會丟棄重新分片/未知的通道並保留
  重播確定性。內存中分片游標索引現在在承諾上快速失敗
  未映射的車道而不是默認的車道 ID，導致光標前進和重放錯誤
  顯式的，塊驗證使用專用的拒絕分片游標回歸
  `DaShardCursorViolation` 原因 + 操作員遙測標籤。啟動/追趕現在停止 DA
  如果 Kura 包含未知車道或回歸光標並記錄有問題的情況，則索引水合
  塊高度，以便操作員可以在服務 DA 之前進行修復state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **分片光標滯後遙測** — `da_shard_cursor_lag_blocks{lane,shard}` 儀表報告如何分片遠遠落後於正在驗證的高度。丟失/陳舊/未知的車道將滯後設置為
  所需的高度（或增量），並且成功的前進將其重置為零，以便穩態保持平坦。
  操作員應對非零滯後發出警報，檢查 DA 線軸/日誌是否存在違規通道，
  並在重放塊之前驗證車道目錄是否存在意外重新分片以清除
  差距。
- **機密計算通道** — 標記為的通道
  `metadata.confidential_compute=true` 和 `confidential_key_version` 被視為
  SMPC/加密 DA 路徑：Sumeragi 強制執行非零負載/清單摘要和存儲票據，
  拒絕完整副本存儲配置文件，並索引 SoraFS 票證 + 策略版本，無需
  暴露有效負載字節。在重播期間從 Kura 獲取收據，以便驗證者恢復相同的內容
  重啟後的機密性元數據。 【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## 實施注意事項- Torii 的 `/v2/da/ingest` 端點現在標準化有效負載壓縮，強制重播緩存，
  確定性地對規範字節進行分塊，重建 `DaManifestV1`，並刪除編碼的有效負載
  在發出收據之前進入 `config.da_ingest.manifest_store_dir` 進行 SoraFS 編排；的
  處理程序還附加一個 `Sora-PDP-Commitment` 標頭，以便客戶端可以捕獲編碼的承諾
  立即。 【crates/iroha_torii/src/da/ingest.rs:220】
- 保留規範的 `DaCommitmentRecord` 後，Torii 現在發出
  清單假脫機旁邊的 `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` 文件。
  每個條目將記錄與原始 Norito `PdpCommitment` 字節捆綁在一起，因此 DA-3 捆綁構建器和
  DA-5 調度程序攝取相同的輸入，無需重新讀取清單或塊存儲。 【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK 幫助程序公開 PDP 標頭字節，而不強制每個客戶端重新實現 Norito 解析：
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` 覆蓋 Rust，Python `ToriiClient`
  現在導出 `decode_pdp_commitment_header`，並且 `IrohaSwift` 提供匹配的助手，因此可以移動
  客戶端可以立即存儲編碼的採樣計劃。 【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii 還公開 `GET /v2/da/manifests/{storage_ticket}`，以便 SDK 和操作員可以獲取清單
  和塊計劃而不觸及節點的假脫機目錄。響應返回 Norito 字節
  (base64)，渲染清單 JSON，為 `sorafs fetch` 準備的 `chunk_plan` JSON blob，以及相關的
  十六進制摘要（`storage_ticket`、`client_blob_id`、`blob_hash`、`chunk_root`），因此下游工具可以
  向編排器提供數據而無需重新計算摘要，並發出相同的 `Sora-PDP-Commitment` 標頭
  鏡像攝取響應。傳遞 `block_hash=<hex>` 作為查詢參數返回確定性
  `sampling_plan` 植根於 `block_hash || client_blob_id`（在驗證器之間共享），其中包含
  `assignment_hash`、請求的 `sample_window` 和採樣的 `(index, role, group)` 元組
  整個 2D 條帶佈局，以便 PoR 採樣器和驗證器可以重放相同的索引。採樣器
  將 `client_blob_id`、`chunk_root` 和 `ipa_commitment` 混合到賦值哈希中； `iroha 應用程序 da get
  --block-hash ` now writes `sampling_plan_.json` 位於清單 + 塊計劃旁邊
  保留的哈希值，並且 JS/Swift Torii 客戶端公開相同的 `assignment_hash_hex`，因此驗證器
  和證明者共享一個確定性探針集。當 Torii 返回抽樣計劃時，`iroha app da
  證明可用性` now reuses that deterministic probe set (seed derived from `sample_seed`)
  即席抽樣，因此 PoR 見證人與驗證者分配保持一致，即使操作員忽略了
  `--block-hash` 覆蓋。 【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### 大負載流需要攝取大於配置的單次請求限制的資產的客戶端發起
通過調用 `POST /v2/da/ingest/chunk/start` 進行流會話。 Torii 響應
`ChunkSessionId`（BLAKE3 - 從請求的 blob 元數據派生）和協商的塊大小。
每個後續 `DaIngestChunk` 請求均攜帶：

- `client_blob_id` — 與最終的 `DaIngestRequest` 相同。
- `chunk_session_id` — 將切片與正在運行的會話聯繫起來。
- `chunk_index` 和 `offset` — 強制執行確定性排序。
- `payload` — 達到協商的塊大小。
- `payload_hash` — 切片的 BLAKE3 哈希，因此 Torii 可以在不緩衝整個 blob 的情況下進行驗證。
- `is_last` — 表示終端片。

Torii 在 `config.da_ingest.manifest_store_dir/chunks/<session>/` 下保留經過驗證的切片，並且
記錄重播緩存內的進度以實現冪等性。當最後一片落地時，Torii
重新組裝磁盤上的有效負載（通過塊目錄流式傳輸以避免內存峰值），
與單次上傳完全相同地計算規范清單/收據，並最終響應
`POST /v2/da/ingest` 通過消耗分階段工件。失敗的會話可以顯式中止或
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
- Sumeragi 在密封或驗證 DA 捆綁包時強制執行清單可用性：
  如果假脫機缺少清單或哈希值不同，則塊驗證失敗
  來自承諾。 【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

請求、清單和接收有效負載的往返覆蓋範圍在
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`，確保Norito編解碼器
在更新中保持穩定。 【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**保留默認值。 ** 治理期間批准了初始保留政策
SF-6； `RetentionPolicy::default()` 強制執行的默認值是：- 熱門層：7天（`604_800`秒）
- 冷層：90 天（`7_776_000` 秒）
- 所需副本：`3`
- 存儲類別：`StorageClass::Hot`
- 治理標籤：`"da.default"`

當車道採用時，下游操作員必須顯式覆蓋這些值
更嚴格的要求。

## Rust 客戶端證明文物

嵌入 Rust 客戶端的 SDK 不再需要通過 CLI 來實現
生成規範的 PoR JSON 包。 `Client` 公開了兩個助手：

- `build_da_proof_artifact` 返回由生成的確切結構
  `iroha app da prove --json-out`，包括提供的清單/有效負載註釋
  通過 [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` 包裝構建器並將工件保存到磁盤
  （默認情況下漂亮的 JSON + 尾隨換行符）因此自動化可以附加文件
  發布或治理證據包。 【crates/iroha/src/client.rs:3653】

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

留下幫助程序的 JSON 有效負載與 CLI 匹配到字段名稱
（`manifest_path`、`payload_path`、`proofs[*].chunk_digest`等），所以現有
自動化可以在沒有特定於格式的分支的情況下比較/鑲木地板/上傳文件。

## 證明驗證基準

使用 DA 證明基準工具來驗證驗證者對代表性有效負載的預算
收緊區塊級別上限：

- `cargo xtask da-proof-bench` 從清單/有效負載對重建塊存儲，對 PoR 進行採樣
  離開，並根據配置的預算進行時間驗證。 Taikai 元數據是自動填充的，並且
  如果夾具對不一致，則線束會退回到合成清單。當 `--payload-bytes`
  在沒有顯式 `--payload` 的情況下設置，生成的 blob 被寫入
  `artifacts/da/proof_bench/payload.bin` 所以燈具保持不變。 【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- 報告默認為 `artifacts/da/proof_bench/benchmark.{json,md}`，包括校樣/運行、總計和
  每個證明的時間、預算通過率和建議的預算（最慢迭代的 110%）
  與`zk.halo2.verifier_budget_ms`對齊。 【artifacts/da/proof_bench/benchmark.md:1】
- 最新運行（合成 1 MiB 有效負載、64 KiB 塊、32 次驗證/運行、10 次迭代、250 毫秒預算）
  建議驗證器預算為 3 ms，迭代次數在上限內 100%。 【artifacts/da/proof_bench/benchmark.md:1】
- 示例（生成確定性有效負載並寫入兩個報告）：

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

塊裝配強制執行相同的預算：`sumeragi.da_max_commitments_per_block` 和
`sumeragi.da_max_proof_openings_per_block` 在將 DA 束嵌入到塊中之前對其進行門控，並且
每個承諾必須帶有非零的 `proof_digest`。守衛將束長度視為
證明開放計數，直到明確的證明摘要通過共識，保持
≤128-在塊邊界可執行的開放目標。 【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR失敗處理和削減存儲工作人員現在會提出 PoR 失敗條紋，並在每個失敗條紋旁邊提供綁定斜杠建議
判決。高於配置的罷工閾值的連續失敗會發出建議：
包括提供者/清單對、觸發斜杠的條紋長度以及建議的
根據提供商保證金和 `penalty_bond_bps` 計算的罰款；冷卻時間（秒）保持
在同一事件中觸發重複的斜杠。 【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- 通過存儲工作構建器配置閾值/冷卻時間（默認值反映治理
  處罰政策）。
- Slash 建議記錄在判決摘要 JSON 中，以便治理/審計人員可以附加
  將它們打包成證據。
- 條帶佈局 + 每塊角色現在通過 Torii 的存儲引腳端點進行線程化
  （`stripe_layout` + `chunk_roles` 字段）並持久保存到存儲工作線程中，以便
  審核員/修復工具可以計劃行/列修復，而無需從上游重新導出佈局

### 放置+修復線束

現在 `cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>`
通過 `(index, role, stripe/column, offsets)` 計算放置哈希並執行行優先，然後
在重建有效負載之前修復列 RS(16)：

- 當存在時，放置默認為 `total_stripes`/`shards_per_stripe` 並回退到塊
- 首先使用行奇偶校驗重建丟失/損壞的塊；剩餘的間隙被修復
  條帶（列）奇偶校驗。修復後的 chunk 被寫回 chunk 目錄，並且 JSON
  摘要捕獲放置哈希加上行/列修復計數器。
- 如果行+列奇偶校驗不能滿足缺失集，則線束會快速失敗並無法恢復
  索引，以便審計員可以標記無法修復的清單。