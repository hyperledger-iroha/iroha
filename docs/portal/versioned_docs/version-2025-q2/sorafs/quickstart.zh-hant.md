---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.997191+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 快速入門

本實踐指南介紹了確定性 SF-1 分塊器配置文件，
支持 SoraFS 的清單簽名和多提供商獲取流程
存儲管道。將其與[清單管道深入研究](manifest-pipeline.md)配對
獲取設計說明和 CLI 標誌參考材料。

## 先決條件

- Rust 工具鏈 (`rustup update`)，本地克隆的工作區。
- 可選：[OpenSSL 生成的 Ed25519 密鑰對](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  用於簽署清單。
- 可選：如果您計劃預覽 Docusaurus 門戶，則 Node.js ≥ 18。

設置 `export RUST_LOG=info`，同時嘗試顯示有用的 CLI 消息。

## 1. 刷新確定性裝置

重新生成規範的 SF-1 分塊向量。該命令還發出有符號的
提供 `--signing-key` 時的艙單信封；使用 `--allow-unsigned`
僅在本地開發期間。

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

輸出：

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json`（如果簽名）
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. 對有效負載進行分塊並檢查計劃

使用 `sorafs_chunker` 對任意文件或存檔進行分塊：

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

關鍵領域：

- `profile` / `break_mask` – 確認 `sorafs.sf1@1.0.0` 參數。
- `chunks[]` – 有序偏移、長度和塊 BLAKE3 摘要。

對於較大的裝置，運行 proptest 支持的回歸以確保流式傳輸和
批量分塊保持同步：

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. 構建並簽署清單

使用以下命令將塊計劃、別名和治理簽名包裝到清單中
`sorafs-manifest-stub`。下面的命令展示了一個單文件有效負載；通過
打包樹的目錄路徑（CLI 按字典順序遍歷）。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

查看 `/tmp/docs.report.json`：

- `chunking.chunk_digest_sha3_256` – 偏移量/長度的 SHA3 摘要，與
  分塊裝置。
- `manifest.manifest_blake3` – 在清單信封中籤名的 BLAKE3 摘要。
- `chunk_fetch_specs[]` – 協調器的有序獲取指令。

準備好提供真實簽名時，添加 `--signing-key` 和 `--signer`
論據。該命令在寫入之前驗證每個 Ed25519 簽名
信封。

## 4. 模擬多提供商檢索

使用開發人員獲取 CLI 針對一個或多個重放塊計劃
提供商。這非常適合 CI 冒煙測試和協調器原型設計。

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

斷言：

- `payload_digest_hex` 必須與艙單報告匹配。
- `provider_reports[]` 顯示每個提供商的成功/失敗計數。
- 非零 `chunk_retry_total` 突出顯示背壓調整。
- 通過 `--max-peers=<n>` 來限制計劃運行的提供者數量
  並使 CI 模擬集中於主要候選人。
- `--retry-budget=<n>` 覆蓋默認的每塊重試計數 (3)，因此您
  在註入故障時可以更快地顯示協調器回歸。

添加 `--expect-payload-digest=<hex>` 和 `--expect-payload-len=<bytes>` 失敗
當重建的有效負載偏離清單時速度很快。

## 5. 後續步驟

- **治理集成** – 通過管道傳輸清單摘要和
  `manifest_signatures.json` 進入理事會工作流程，以便 Pin 註冊表可以
  廣告可用性。
- **註冊協商** – 諮詢 [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  在註冊新的配置文件之前。自動化應該更喜歡規範句柄
  (`namespace.name@semver`) 通過數字 ID。
- **CI 自動化** – 添加上面的命令來發布管道，所以文檔，
  固定裝置和工件在簽名的同時發布確定性清單
  元數據。