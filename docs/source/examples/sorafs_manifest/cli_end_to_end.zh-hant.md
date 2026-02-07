---
lang: zh-hant
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS 清單 CLI 端到端示例

此示例演示如何使用以下命令將文檔版本發佈到 SoraFS
`sorafs_manifest_stub` CLI 與確定性分塊裝置一起
SoraFS 架構 RFC 中進行了描述。該流程涵蓋了明顯的生成，
期望檢查、獲取計劃驗證和檢索證明演練
團隊可以在 CI 中嵌入相同的步驟。

## 先決條件

- 克隆工作區並準備好工具鏈（`cargo`、`rustc`）。
- `fixtures/sorafs_chunker` 中的夾具可用，因此期望值可以
  派生（對於生產運行，從遷移分類帳條目中提取值
  與工件相關）。
- 要發布的示例有效負載目錄（本示例使用 `docs/book`）。

## 步驟 1 — 生成清單、CAR、簽名和獲取計劃

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

命令：

- 通過 `ChunkProfile::DEFAULT` 傳輸有效負載。
- 發出 CARv2 存檔以及塊獲取計劃。
- 構建 `ManifestV1` 記錄，驗證清單簽名（如果提供），以及
  信封上寫道。
- 強制執行期望標誌，因此如果字節漂移則運行失敗。

## 步驟 2 — 使用塊存儲 + PoR 演練驗證輸出

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

這通過確定性塊存儲重放 CAR，得出
可檢索性證明採樣樹，並發出適合的清單報告
治理審查。

## 步驟 3 — 模擬多提供商檢索

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

對於 CI 環境，為每個提供程序提供單獨的有效負載路徑（例如，安裝
固定裝置）來練習範圍調度和故障處理。

## 步驟 4 — 記錄賬本條目

在 `docs/source/sorafs/migration_ledger.md` 中記錄發布，捕獲：

- 清單 CID、CAR 摘要和理事會簽名哈希。
- 狀態（`Draft`、`Staging`、`Pinned`）。
- CI 運行或治理票證的鏈接。

## 步驟 5 — 通過治理工具固定（當註冊中心上線時）

部署 Pin 註冊表後（遷移路線圖中的 Milestone M2），
通過 CLI 提交清單：

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

提案標識符和後續批准交易哈希值應該是
在遷移分類帳條目中捕獲以進行審計。

## 清理

`target/sorafs/` 下的工件可以存檔或上傳到暫存節點。
將清單、簽名、CAR 和獲取計劃放在一起，以便下游
運營商和 SDK 團隊可以確定性地驗證部署。