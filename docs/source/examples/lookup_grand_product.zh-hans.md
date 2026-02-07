---
lang: zh-hans
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2025-12-29T18:16:35.953884+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 查找大产品示例

此示例扩展了中提到的 FASTPQ 权限查找参数
`fastpq_plan.md`。  在第 2 阶段管道中，证明者评估选择器
低度扩展 (LDE) 上的 (`s_perm`) 和见证 (`perm_hash`) 列
域，更新正在运行的盛大产品 `Z_i`，最后提交整个
与波塞冬的序列。  散列累加器被附加到转录本中
在 `fastpq:v1:lookup:product` 域下，而最终的 `Z_i` 仍然匹配
提交的权限表产品 `T`。

我们考虑一个具有以下选择器值的小批量：

|行| `s_perm` | `perm_hash` |
| ---| -------- | ---------------------------------------------------------- |
| 0 | 1 | `0x019a...`（授予角色=审核员，perm=transfer_asset）|
| 1 | 0 | `0xabcd...`（无权限更改）|
| 2 | 1 | `0x42ff...`（撤销角色=审计员，perm=burn_asset）|

令 `gamma = 0xdead...` 为来自以下项的 Fiat-Shamir 查找挑战：
成绩单。  证明者初始化 `Z_0 = 1` 并折叠每一行：

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

`s_perm = 0` 的行不会更改累加器。  处理后
跟踪，证明者 Poseidon 对转录本的序列 `[Z_1, Z_2, ...]` 进行哈希处理
但还发布了 `Z_final = Z_3`（最终运行产品）以匹配该表
边界条件。

在表端，提交的权限 Merkle 树对确定性进行编码
插槽的活动权限集。  验证者（或期间的证明者）
证人生成）计算

```
T = product over entries: (entry.hash + gamma)
```

该协议强制执行边界约束 `Z_final / T = 1`。  如果踪迹
引入了表中不存在的权限（或省略了
是），总乘积比率偏离 1，验证者拒绝。  因为
两边都在金发姑娘域内乘以`(value + gamma)`，比率
在 CPU/GPU 后端保持稳定。

要将示例序列化为灯具的 Norito JSON，请记录以下元组
每行后面有 `perm_hash`、选择器和累加器，例如：

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

十六进制占位符 (`0x...`) 可以替换为具体的金发姑娘
生成自动化测试时的字段元素。  第二阶段额外赛程
记录正在运行的累加器的 Poseidon 哈希值，但保持相同的 JSON 形状，
因此该示例可以兼作未来测试向量的模板。