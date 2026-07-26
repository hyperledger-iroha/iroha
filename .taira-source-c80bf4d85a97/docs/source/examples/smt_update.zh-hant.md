---
lang: zh-hant
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2025-12-29T18:16:35.954370+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 稀疏 Merkle 更新示例

此工作示例說明了 FASTPQ 第 2 階段跟踪如何對
使用 `neighbour_leaf` 列的非成員見證人。稀疏默克爾樹
是 Poseidon2 場元素的二進制。密鑰轉換為規範密鑰
32 字節小端字符串，散列到字段元素，並且最多
有效位選擇每個級別的分支。

## 場景

- 預狀態離開
  - `asset::alice::rose` -> 散列鍵 `0x12b7...`，值為 `0x0000_0000_0000_0005`。
  - `asset::bob::rose` -> 散列鍵 `0x1321...`，值為 `0x0000_0000_0000_0003`。
- 更新請求：插入值為 2 的 `asset::carol::rose`。
- Carol 的規範密鑰哈希擴展到 5 位前綴 `0b01011`。的
  現有鄰居的前綴為 `0b01010` (Alice) 和 `0b01101` (Bob)。

因為不存在前綴與 `0b01011` 匹配的葉子，所以證明者必須提供
額外的證據表明區間 `(alice, bob)` 為空。第 2 階段已填充
跨列的跟踪行 `path_bit_{level}`、`sibling_{level}`、
`node_in_{level}` 和 `node_out_{level}`（`level` 位於 `[0, 31]` 中）。所有值
是以小端形式編碼的 Poseidon2 字段元素：

|級別 | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` |筆記|
| -----| ---------------- | ------------------------ | | ------------------------------------------------ | ------------------------------------------------ | -----|
| 0 | 1 | `0x241f...`（愛麗絲葉哈希）| `0x0000...` | `0x4b12...` (`value_2 = 2`) |插入：從零開始，存儲新值。 |
| 1 | 1 | `0x7d45...`（右空）|波塞冬2(`node_out_0`, `sibling_0`) |波塞冬2(`sibling_1`, `node_out_1`) |遵循前綴位 1。
| 2 | 0 | `0x03ae...`（鮑勃分支）|波塞冬2(`node_out_1`, `sibling_1`) |波塞冬2(`node_in_2`, `sibling_2`) |分支翻轉，因為位 = 0。
| 3 | 1 | `0x9bc4...` |波塞冬2(`node_out_2`, `sibling_2`) |波塞冬2(`sibling_3`, `node_out_3`) |更高的水平繼續向上散列。 |
| 4 | 0 | `0xe112...` |波塞冬2(`node_out_3`, `sibling_3`) |波塞冬2(`node_in_4`, `sibling_4`) |根級；結果是後狀態根。 |

該行的 `neighbour_leaf` 列填充有 Bob 的葉子
（`key = 0x1321...`、`value = 3`、`hash = Poseidon2(key, value) = 0x03ae...`）。當
進行驗證時，AIR 檢查：

1. 提供的鄰居對應於第 2 層使用的同級。
2. 鄰居鍵按字典順序大於插入鍵並且
   左兄弟（Alice）按字典順序較小。
3. 用鄰居替換插入的葉子會重現狀態前的根。這些檢查一起證明了區間 `(0b01010,
0b01101)`更新前。生成 FASTPQ 跟踪的實現可以使用
逐字佈局；上面的數值常數是說明性的。對於一個完整的
JSON見證，發出與上表中顯示的列完全相同的列（帶有
每個級別的數字後綴），使用小端字節字符串序列化
Norito JSON 幫助程序。