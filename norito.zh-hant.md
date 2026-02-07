---
lang: zh-hant
direction: ltr
source: norito.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f91db1ad9e91a6fca60c5ecbd70e7700bc1a4cbebdcbe61233dd83a03bc89f
source_last_modified: "2026-01-30T12:29:10.234437+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 格式 (v1)

本文檔是 Norito 的在線編碼的真實來源
Iroha 工作區。它定義了標頭、標誌和規范長度
跨組件使用的字符串佈局。

## 標題

Norito 標頭始終存在於線路和磁盤上。它構建有效負載
並提供確定性解碼所需的模式哈希和校驗和。

|領域|大小（字節）|筆記|
| ---| ---| ---|
|魔法| 4 | ASCII `NRT0` |
|專業| 1 | `VERSION_MAJOR = 0` |
|次要| 1 | `VERSION_MINOR = 0x00` |
|架構哈希 | 16 | 16 FNV-1a 完全限定類型名稱的哈希值 (v1) |
|壓縮| 1 | `0 = None`、`1 = Zstd` |
|有效負載長度| 8 |未壓縮的有效負載長度（u64，小端）|
| CRC64 | 8 |有效負載上的 CRC64-XZ（ECMA 多項式、反射、初始化/異或所有）|
|旗幟| 1 |佈局標誌（見下文）|

標頭總大小：40 字節。

對齊填充：
- 對於未壓縮的有效負載，編碼器必須在有效負載之間插入零填充
  標題和有效負載，否則存檔類型的對齊方式是
  違反了。
- 填充長度必須是類型和所需的精確對齊填充
  填充字節必須為零。沒有具體類型對齊的解碼器必須
  接受最多 64 字節的任何零填充，並將剩餘字節視為
  有效負載。額外的非零字節將被拒絕。

架構執行：
- 類型化解碼器必須拒絕標頭架構哈希不匹配的有效負載
  預期的類型。 `ArchiveView::decode` 執行此檢查；使用
  `ArchiveView::decode_unchecked` 僅適用於原始檢查工具。

## 標頭標誌

這些標誌被或運算到最終的頭字節中。未知位被拒絕。

|旗幟|十六進制 |意義|
| ---| ---| ---|
| `PACKED_SEQ` | `0x01` |可變大小集合的打包序列佈局。 |
| `COMPACT_LEN` | `0x02` |每個值的長度前綴是緊湊的變體。 |
| `PACKED_STRUCT` | `0x04` |派生生成類型的打包結構佈局。 |
| `VARINT_OFFSETS` | `0x08` |在 v1 中保留；打包序列始終使用 `(len + 1)` u64 偏移量。 |
| `COMPACT_SEQ_LEN` | `0x10` |在 v1 中保留；序列長度標頭是固定的 u64。 |
| `FIELD_BITSET` | `0x20` |打包結構混合使用一個位集來指示哪些字段攜帶顯式大小（需要 `PACKED_STRUCT` + `COMPACT_LEN`）。 |

標誌範圍規則：
- `COMPACT_LEN` 僅影響每個值的長度前綴。
- 解碼標頭時，拒絕保留的佈局位（`VARINT_OFFSETS`、`COMPACT_SEQ_LEN`）。

這些標誌是獨立的；不允許出現啟發式交叉效應。

## 長度前綴

Norito 在多個位置使用長度前綴，並使用顯式標誌決定長度
編碼：

- 每個值的前綴（字段、元素、字符串、blob）使用 `COMPACT_LEN`。
  - 如果設置：unsigned varint（7 位連續）。
  - 如果未設置：固定 8 字節小端 u64。
- v1 中序列長度標頭固定為 8 字節小端 u64。
- 打包序列偏移量始終為 `(len + 1)` u64 偏移量，與
  第一個偏移量 0。Varint 編碼必須適合 `u64` 並使用最短（規範）編碼；
溢出或過長的編碼被拒絕。

## 字符串編碼

`String` 和 `&str` 值編碼為：

```
[len][utf8-bytes]
```

`len` 使用上面的每個值前綴規則 (`COMPACT_LEN`)。解碼器不得
應用嵌套長度啟發式或根據其重新解釋字符串有效負載
內容。

## 數字和 BigInt

`BigInt` 編碼為：

```
[len_u32][twos_complement_le_bytes]
```

`len_u32` 是以下有效負載的 4 字節小端長度。字節數
是小尾數二進制補碼，並且必須符合 512 位上限。

`Numeric` 編碼為結構體 `(mantissa, scale)`：
- `mantissa` 是一個 `BigInt` 包含原始整數值（沒有小數位）
  嵌入到整數中）。
- `scale` 是 `u32` 小數位數（例如 `1.88` 是尾數 `188`，
  規模`2`)。

## 地圖編碼

地圖使用相同的活動佈局標誌進行確定性編碼：

- 條目計數使用固定的 8 字節小端 u64 標頭。
- 兼容佈局（`PACKED_SEQ` 未設置）：對於每個條目，
  `[key_len][key_payload][value_len][value_payload]` 具有鍵/值長度
  通過 `COMPACT_LEN` 編碼。
- 打包佈局（`PACKED_SEQ` set）：鍵大小和值大小位於數據之前，
  接下來是串聯的鍵有效負載和串聯的值有效負載。用途
  `(len + 1)` u64 鍵偏移量，然後 `(len + 1)` u64 值偏移量；
  偏移量與第一個偏移量 0 是單調的。
- `HashMap` 按排序鍵順序對條目進行編碼以獲得確定性輸出；
  `BTreeMap` 使用其自然順序。

## NCB 柱狀（內部）

NCB 有效負載是精確且規範的：
- NCB 列之間的對齊填充必須用零填充。
- 位集填充位（標誌和存在）必須為零。
- NCB 有效負載之後的尾隨字節被拒絕。

## AoS Ad-hoc（自適應柱狀）

自適應柱狀編碼器使用的 `norito::aos` 助手遵循相同的規則
長度前綴規則並遵守活動的 `COMPACT_LEN` 標誌，因此嵌入了 AoS
有效負載與其父 Norito 標頭保持一致。

## 壓縮結構佈局

當設置 `PACKED_STRUCT` 標誌時，派生生成的結構體/元組是
編碼為具有兩種佈局之一的單個打包有效負載：

- 兼容的打包結構（無 `FIELD_BITSET`）：`(field_count + 1)` 小端
  `u64` 偏移量後跟連接的字段有效負載。偏移量從 0 開始，
  是聲明順序中每個字段有效負載的累積字節長度，以及
  最終偏移量等於總數據長度。偏移量甚至是固定寬度的
  當啟用 `COMPACT_LEN` 時。
- 混合打包結構 (`FIELD_BITSET` + `COMPACT_LEN`)：長度的位集
  `ceil(field_count / 8)` 字節，後跟字段的大小前綴
  位已設置（根據 `COMPACT_LEN` 進行變體編碼），後跟連接字段
  按聲明順序排列的有效負載。字節 0 的位 0 引用字段 0，位 1 引用字段 0。
  字段 1，依此類推。固定大小或自定界的字段省略
  顯式大小標頭並按順序解碼。字段有效負載本身使用活動佈局標誌（例如，`PACKED_SEQ`，
`COMPACT_LEN`) 編碼嵌套集合或字符串/blob 值時。

## 壓縮選擇和驗證

標頭 `Compression` 字節標識有效負載編碼：

- `0 = None`：有效負載字節位於標頭後面（帶有可選的對齊填充）。
- `1 = Zstd`：有效負載字節使用 Zstandard 進行壓縮。

`Payload length` 和 `CRC64` 始終描述未壓縮的有效負載。對於
壓縮的有效負載，編碼的字節流在
標題沒有對齊填充。解碼器必須拒絕未知壓縮
值或不受支持的算法；沒有 `compression` 功能的構建
僅接受 `None`。

編碼器顯式選擇壓縮 (`to_compressed_bytes`) 或通過
應用確定性啟發式的自適應助手 (`to_bytes_auto`)。的
選擇的算法記錄在標頭中；沒有在線協商。

## 架構哈希詳細信息

16 字節模式哈希計算如下：

- 默認值：FNV-1a 完全限定類型名稱的 64 位哈希（Rust
  `core::any::type_name::<T>()`)，複製以填充 16 個字節。
- 使用 `schema-structural`: 生成的規範 JSON 模式
  `iroha_schema::IntoSchema`，使用 Norito 的 JSON writer 進行序列化並進行哈希處理
  具有相同的 FNV-1a 例程。

類型化解碼器必須拒絕其標頭架構哈希與
預期類型。 `ArchiveView::decode` 強制執行此檢查； `decode_unchecked`
保留用於顯式選擇退出模式驗證的工具。