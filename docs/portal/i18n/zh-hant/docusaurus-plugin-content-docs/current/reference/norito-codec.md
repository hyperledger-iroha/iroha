---
lang: zh-hant
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito 編解碼器參考

Norito 是 Iroha 的規範序列化層。磁盤上的每一條線上消息
有效負載，跨組件 API 使用 Norito，因此節點同意相同的字節
即使它們在不同的硬件上運行。本頁總結了移動部件
並指向 `norito.md` 中的完整規範。

## 核心佈局

|組件|目的|來源 |
| ---| ---| ---|
| **標題** |使用 magic/version/schema 哈希、CRC64、長度和壓縮標籤構建有效負載； v1 需要 `VERSION_MINOR = 0x00` 並根據支持的掩碼（默認 `0x00`）驗證標頭標誌。 | `norito::header` — 請參閱 `norito.md`（“標頭和標誌”，存儲庫根）|
| **裸負載** |用於散列/比較的確定性值編碼。在線傳輸始終使用標頭；裸字節僅供內部使用。 | `norito::codec::{Encode, Decode}` |
| **壓縮** |通過標頭壓縮字節選擇可選的 Zstd（和實驗性 GPU 加速）。 | `norito.md`，“壓縮協商” |

佈局標誌註冊表（packed-struct、packed-seq、field bitset、compact
長度）位於 `norito::header::flags` 中。 V1 默認為標誌 `0x00` 但
接受受支持掩碼內的顯式標頭標誌；未知位是
被拒絕了。 `norito::header::Flags` 保留用於內部檢查和
未來的版本。

## 獲得支持

`norito_derive` 附帶 `Encode`、`Decode`、`IntoSchema` 和 JSON 幫助程序派生。
主要約定：

- 派生生成 AoS 和打包代碼路徑； v1 默認為 AoS
  佈局（標誌 `0x00`），除非標頭標誌選擇打包變體。
  實現位於 `crates/norito_derive/src/derive_struct.rs` 中。
- 影響佈局的功能（`packed-struct`、`packed-seq`、`compact-len`）是
  通過標頭標誌選擇加入，並且必須在對等點之間一致地編碼/解碼。
- JSON 助手 (`norito::json`) 提供確定性 Norito 支持的 JSON
  開放 API。使用 `norito::json::{to_json_pretty, from_json}` — 切勿使用 `serde_json`。

## 多編解碼器和標識符表

Norito 將其多編解碼器分配保留在 `norito::multicodec` 中。參考資料
表（哈希值、密鑰類型、有效負載描述符）維護在 `multicodec.md` 中
在存儲庫根目錄下。添加新標識符時：

1.更新`norito::multicodec::registry`。
2. 擴展`multicodec.md` 中的表。
3. 如果下游綁定 (Python/Java) 消耗了映射，則重新生成下游綁定。

## 重新生成文檔和裝置

由於門戶當前託管散文摘要，請使用上游 Markdown
來源作為真理的來源：

- **規格**：`norito.md`
- **多編解碼器表**：`multicodec.md`
- **基準**：`crates/norito/benches/`
- **黃金測試**：`crates/norito/tests/`

當 Docusaurus 自動化上線時，門戶將通過
從這些數據中提取數據的同步腳本（在 `docs/portal/scripts/` 中跟踪）
文件。在此之前，只要規範發生變化，請手動保持此頁面對齊。