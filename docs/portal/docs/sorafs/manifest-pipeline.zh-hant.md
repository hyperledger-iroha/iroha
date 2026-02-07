---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-05T09:28:11.879581+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 分塊 → 清單管道

快速入門的這個配套文件跟踪了將原始數據轉化為數據的端到端管道
Norito 清單中的字節適合 SoraFS Pin 註冊表。內容是
改編自[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)；
請查閱該文檔以獲取規範規範和變更日誌。

## 1. 確定性分塊

SoraFS 使用 SF-1 (`sorafs.sf1@1.0.0`) 配置文件：受 FastCDC 啟發的滾動
具有 64KiB 最小塊大小、256KiB 目標、512KiB 最大大小和
`0x0000ffff` 破壞掩碼。該個人資料註冊於
`sorafs_manifest::chunker_registry`。

### Rust 助手

- `sorafs_car::CarBuildPlan::single_file` – 發出塊偏移量、長度和
  BLAKE3 在準備 CAR 元數據時進行摘要。
- `sorafs_car::ChunkStore` – 流式傳輸有效負載，保留塊元數據，以及
  派生 64KiB / 4KiB 可檢索性證明 (PoR) 採樣樹。
- `sorafs_chunker::chunk_bytes_with_digests` – 兩個 CLI 背後的庫助手。

### CLI 工具

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON 包含有序偏移量、長度和塊摘要。堅持
在構建清單或協調器獲取規範時進行規劃。

### PoR 見證人

`ChunkStore` 暴露 `--por-proof=<chunk>:<segment>:<leaf>` 和
`--por-sample=<count>`，以便審核員可以請求確定性見證集。配對
這些帶有 `--por-proof-out` 或 `--por-sample-out` 的標誌來記錄 JSON。

## 2. 包裝清單

`ManifestBuilder` 將塊元數據與治理附件相結合：

- 根 CID (dag-cbor) 和 CAR 承諾。
- 別名證明和提供商能力聲明。
- 委員會簽名和可選元數據（例如構建 ID）。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

重要輸出：

- `payload.manifest` – Norito 編碼的清單字節。
- `payload.report.json` – 人類/自動化可讀摘要，包括
  `chunk_fetch_specs`、`payload_digest_hex`、CAR 摘要和別名元數據。
- `payload.manifest_signatures.json` – 包含清單 BLAKE3 的信封
  摘要、塊計劃 SHA3 摘要和排序的 Ed25519 簽名。

使用 `--manifest-signatures-in` 驗證外部提供的信封
簽署人之前將其寫回，以及 `--chunker-profile-id` 或
`--chunker-profile=<handle>` 鎖定註冊表選擇。

## 3. 發布並固定

1. **治理提交** – 提供清單摘要和簽名
   將信封寄給理事會，以便允許使用密碼。外部審計師應
   將塊計劃 SHA3 摘要與清單摘要一起存儲。
2. **Pin 有效負載** – 上傳引用的 CAR 存檔（和可選的 CAR 索引）
   在 Pin 註冊表的清單中。確保清單和 CAR 共享
   相同的根 CID。
3. **記錄遙測** – 保留 JSON 報告、PoR 見證和任何獲取
   發布工件中的指標。這些記錄提供給操作員儀表板和
   幫助重現問題，而無需下載大量負載。

## 4. 多提供商獲取模擬

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` 增加了每個提供者的並行性（上面的 `#4`）。
- `@<weight>` 調整調度偏差；默認為 1。
- `--max-peers=<n>` 限制了計劃運行的提供者數量
  發現產生的候選者數量超出了預期。
- `--expect-payload-digest` 和 `--expect-payload-len` 防范靜音
  腐敗。
- `--provider-advert=name=advert.to` 之前驗證提供商的能力
  在模擬中使用它們。
- `--retry-budget=<n>` 覆蓋每個塊的重試計數（默認值：3），因此 CI
  在測試故障場景時可以更快地發現回歸。

`fetch_report.json` 顯示聚合指標 (`chunk_retry_total`,
`provider_failure_rate`等）適合CI斷言和可觀察性。

## 5. 註冊表更新和治理

當提出新的分塊配置文件時：

1. 在 `sorafs_manifest::chunker_registry_data` 中編寫描述符。
2.更新`docs/source/sorafs/chunker_registry.md`及相關章程。
3. 重新生成裝置 (`export_vectors`) 並捕獲簽名清單。
4. 提交帶有治理簽名的章程合規報告。

自動化應該更喜歡規範句柄（`namespace.name@semver`）並下降