---
lang: zh-hant
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2025-12-29T18:16:35.953884+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 查找大產品示例

此示例擴展了中提到的 FASTPQ 權限查找參數
`fastpq_plan.md`。  在第 2 階段管道中，證明者評估選擇器
低度擴展 (LDE) 上的 (`s_perm`) 和見證 (`perm_hash`) 列
域，更新正在運行的盛大產品 `Z_i`，最後提交整個
與波塞冬的序列。  散列累加器被附加到轉錄本中
在 `fastpq:v1:lookup:product` 域下，而最終的 `Z_i` 仍然匹配
提交的權限表產品 `T`。

我們考慮一個具有以下選擇器值的小批量：

|行| `s_perm` | `perm_hash` |
| ---| -------- | ---------------------------------------------------------- |
| 0 | 1 | `0x019a...`（授予角色=審核員，perm=transfer_asset）|
| 1 | 0 | `0xabcd...`（無權限更改）|
| 2 | 1 | `0x42ff...`（撤銷角色=審計員，perm=burn_asset）|

令 `gamma = 0xdead...` 為來自以下項的 Fiat-Shamir 查找挑戰：
成績單。  證明者初始化 `Z_0 = 1` 並折疊每一行：

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

`s_perm = 0` 的行不會更改累加器。  處理後
跟踪，證明者 Poseidon 對轉錄本的序列 `[Z_1, Z_2, ...]` 進行哈希處理
但還發布了 `Z_final = Z_3`（最終運行產品）以匹配該表
邊界條件。

在表端，提交的權限 Merkle 樹對確定性進行編碼
插槽的活動權限集。  驗證者（或期間的證明者）
證人生成）計算

```
T = product over entries: (entry.hash + gamma)
```

該協議強制執行邊界約束 `Z_final / T = 1`。  如果踪跡
引入了表中不存在的權限（或省略了
是），總乘積比率偏離 1，驗證者拒絕。  因為
兩邊都在金發姑娘域內乘以`(value + gamma)`，比率
在 CPU/GPU 後端保持穩定。

要將示例序列化為燈具的 Norito JSON，請記錄以下元組
每行後面有 `perm_hash`、選擇器和累加器，例如：

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

十六進制佔位符 (`0x...`) 可以替換為具體的金發姑娘
生成自動化測試時的字段元素。  第二階段額外賽程
記錄正在運行的累加器的 Poseidon 哈希值，但保持相同的 JSON 形狀，
因此該示例可以兼作未來測試向量的模板。