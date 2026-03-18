---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist and expected digests for validating the canonical `sorafs.sf1@1.0.0` chunker profile.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS SF1 確定性試運行

該報告捕獲了規範的基線試運行
`sorafs.sf1@1.0.0` 分塊器配置文件。工具工作組應重新運行檢查表
當驗證裝置刷新或新的消費者管道時，如下所示。記錄
表中每個命令的結果都保持可審計的跟踪。

## 清單

|步驟|命令|預期結果 |筆記|
|------|---------|------------------|--------|
| 1 | `cargo test -p sorafs_chunker` |所有測試均通過； `vectors` 奇偶校驗測試成功。 |確認規範裝置編譯並匹配 Rust 實現。 |
| 2 | `ci/check_sorafs_fixtures.sh` |腳本退出0；下面報告清單摘要。 |驗證裝置是否乾淨地重新生成並且簽名是否保持連接。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` 條目與註冊表描述符 (`profile_id=1`) 匹配。 |確保註冊表元數據保持同步。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` |再生成功，無需 `--allow-unsigned`；清單和簽名文件不變。 |為塊邊界和清單提供確定性證明。 |
| 5 | `node scripts/check_sf1_vectors.mjs` |報告 TypeScript 裝置和 Rust JSON 之間沒有差異。 |可選幫手；確保運行時之間的奇偶性（由工具工作組維護的腳本）。 |

## 預期摘要

- 塊摘要 (SHA3-256)：`13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`
- `sf1_profile_v1.json`: `d89a4fdc030b0c7c4911719ea133c780d9f4610b08eef1d6d0e0ca443391718e`
- `sf1_profile_v1.ts`: `9a3bb8e4d96518b3a0a1301046b2d86a793991959ebdd8adda1fb2988e4292dc`
- `sf1_profile_v1.go`: `0f0348b8751b0f85fe874afda3371af75b78fac5dad65182204dcb3cf3e4c0a1`
- `sf1_profile_v1.rs`：`66b5956826c86589a24b71ca6b400cc1335323c6371f1cec9475f09af8743f61`

## 簽核日誌

|日期 |工程師|檢查清單結果 |筆記|
|------|----------|------------------|--------|
| 2026-02-12 |模具（法學碩士）| ❌ 失敗 |步驟 1：`cargo test -p sorafs_chunker` 未能通過 `vectors` 套件，因為裝置已過時。步驟 2：`ci/check_sorafs_fixtures.sh` 中止 - `manifest_signatures.json` 在存儲庫狀態中丟失（在工作樹中刪除）。步驟 4：當清單文件不存在時，`export_vectors` 無法驗證簽名。建議恢復簽名的裝置（或提供理事會密鑰）並重新生成綁定，以便根據測試的要求嵌入規範句柄。 |
| 2026-02-12 |模具（法學碩士）| ✅ 通過 |通過 `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` 重新生成固定裝置，生成規範的僅句柄別名列表和新的清單摘要 `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`。使用 `cargo test -p sorafs_chunker` 和乾淨的 `ci/check_sorafs_fixtures.sh` 運行進行驗證（用於檢查的分階段固定裝置）。步驟 5 等待節點奇偶校驗助手落地。 |
| 2026-02-20 |存儲工具 CI | ✅ 通過 |通過 `ci/check_sorafs_fixtures.sh` 獲取議會信封 (`fixtures/sorafs_chunker/manifest_signatures.json`)；腳本重新生成固定裝置，確認清單摘要 `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`，並重新運行 Rust 工具（Go/Node 步驟可用時執行），沒有差異。 |

工具工作組應在運行清單後附加一個註明日期的行。如果有任何一步
失敗，請提交此處鏈接的問題並在之前包含修復詳細信息
批准新的固定裝置或型材。