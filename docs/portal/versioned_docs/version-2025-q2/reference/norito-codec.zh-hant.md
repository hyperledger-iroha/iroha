---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 編解碼器參考

Norito 是 Iroha 的規範序列化層。磁盤上的每一條線上消息
有效負載，跨組件 API 使用 Norito，因此節點同意相同的字節
即使它們在不同的硬件上運行。本頁總結了移動部件
並指向 `norito.md` 中的完整規範。

## 核心佈局

|組件|目的|來源 |
| ---| ---| ---|
| **標題** |協商功能（打包結構/序列、緊湊長度、壓縮標誌）並嵌入 CRC64 校驗和，以便在解碼之前檢查有效負載的完整性。 | `norito::header` — 請參閱 `norito.md`（“標頭和標誌”，存儲庫根）|
| **裸負載** |用於散列/比較的確定性值編碼。相同的佈局由標頭包裝以進行傳輸。 | `norito::codec::{Encode, Decode}` |
| **壓縮** |通過 `compression` 標誌字節激活可選的 Zstd（和實驗性 GPU 加速）。 | `norito.md`，“壓縮協商”|

完整的標誌註冊表（打包結構、打包序列、緊湊長度、壓縮）
住在 `norito::header::flags` 中。 `norito::header::Flags` 暴露便利性
檢查運行時檢查；保留的佈局位被解碼器拒絕。

## 獲得支持

`norito_derive` 附帶 `Encode`、`Decode`、`IntoSchema` 和 JSON 幫助程序派生。
主要約定：

- 當 `packed-struct` 功能為時，結構/枚舉派生打包佈局
  啟用（默認）。實現位於 `crates/norito_derive/src/derive_struct.rs` 中
  該行為記錄在 `norito.md`（“打包佈局”）中。
- 打包集合在 v1 中使用固定寬度的序列頭和偏移量；僅
  每個值的長度前綴受 `COMPACT_LEN` 影響。
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