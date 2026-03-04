---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2025-12-29T18:16:35.906407+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 概述

Norito 是跨 Iroha 使用的二進制序列化層：它定義數據如何
結構在線路上進行編碼，保存在磁盤上，並在之間交換
合約和主機。工作區中的每個板條箱都依賴於 Norito 而不是
`serde` 因此不同硬件上的對等點產生相同的字節。

本概述總結了核心部分以及規範參考文獻的鏈接。

## 架構概覽

- **標頭 + 有效負載** – 每個 Norito 消息均以功能協商開頭
  標頭（標誌、校驗和）後跟裸負載。打包佈局和
  壓縮是通過標頭位協商的。
- **確定性編碼** – `norito::codec::{Encode, Decode}` 實現
  裸編碼。將有效負載包裝在標頭中時會重複使用相同的佈局，因此
  散列和簽名仍然是確定性的。
- **架構 + 派生** – `norito_derive` 生成 `Encode`、`Decode` 和
  `IntoSchema` 實現。默認情況下啟用打包結構/序列
  並記錄在 `norito.md` 中。
- **多編解碼器註冊表** – 哈希、密鑰類型和有效負載的標識符
  描述符位於 `norito::multicodec` 中。權威的表是
  維護在 `multicodec.md` 中。

## 工具

|任務|命令/API |筆記|
| ---| ---| ---|
|檢查標題/部分 | `ivm_tool inspect <file>.to` |顯示 ABI 版本、標誌和入口點。 |
|在 Rust 中編碼/解碼 | `norito::codec::{Encode, Decode}` |針對所有核心數據模型類型實施。 |
| JSON 互操作 | `norito::json::{to_json_pretty, from_json}` |由 Norito 值支持的確定性 JSON。 |
|生成文檔/規格 | `norito.md`、`multicodec.md` |存儲庫根目錄中的真實來源文檔。 |

## 開發流程

1. **添加派生** – 新數據首選 `#[derive(Encode, Decode, IntoSchema)]`
   結構。除非絕對必要，否則避免手寫序列化程序。
2. **驗證打包佈局** – 使用 `cargo test -p norito`（以及打包佈局）
   `scripts/run_norito_feature_matrix.sh` 中的功能矩陣）以確保新
   佈局保持穩定。
3. **重新生成文檔** – 當編碼更改時，更新 `norito.md` 和
   多編解碼器表，然後刷新門戶頁面 (`/reference/norito-codec`
   以及本概述）。
4. **保留測試 Norito-first** – 集成測試應使用 Norito JSON
   helper 而不是 `serde_json`，因此它們使用與生產相同的路徑。

## 快速鏈接

- 規格：[`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- 多編解碼器分配：[`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- 特徵矩陣腳本：`scripts/run_norito_feature_matrix.sh`
- 打包佈局示例：`crates/norito/tests/`

將此概述與快速入門指南 (`/norito/getting-started`) 結合起來，了解
使用 Norito 編譯和運行字節碼的實踐演練
有效負載。