---
lang: zh-hans
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

此工作示例说明了 FASTPQ 第 2 阶段跟踪如何对
使用 `neighbour_leaf` 列的非成员见证人。稀疏默克尔树
是 Poseidon2 场元素的二进制。密钥转换为规范密钥
32 字节小端字符串，散列到字段元素，并且最多
有效位选择每个级别的分支。

## 场景

- 预状态离开
  - `asset::alice::rose` -> 散列键 `0x12b7...`，值为 `0x0000_0000_0000_0005`。
  - `asset::bob::rose` -> 散列键 `0x1321...`，值为 `0x0000_0000_0000_0003`。
- 更新请求：插入值为 2 的 `asset::carol::rose`。
- Carol 的规范密钥哈希扩展到 5 位前缀 `0b01011`。的
  现有邻居的前缀为 `0b01010` (Alice) 和 `0b01101` (Bob)。

因为不存在前缀与 `0b01011` 匹配的叶子，所以证明者必须提供
额外的证据表明区间 `(alice, bob)` 为空。第 2 阶段已填充
跨列的跟踪行 `path_bit_{level}`、`sibling_{level}`、
`node_in_{level}` 和 `node_out_{level}`（`level` 位于 `[0, 31]` 中）。所有值
是以小端形式编码的 Poseidon2 字段元素：

|级别 | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` |笔记|
| -----| ---------------- | ------------------------ | | ------------------------------------------------ | ------------------------------------------------ | -----|
| 0 | 1 | `0x241f...`（爱丽丝叶哈希）| `0x0000...` | `0x4b12...` (`value_2 = 2`) |插入：从零开始，存储新值。 |
| 1 | 1 | `0x7d45...`（右空）|波塞冬2(`node_out_0`, `sibling_0`) |波塞冬2(`sibling_1`, `node_out_1`) |遵循前缀位 1。
| 2 | 0 | `0x03ae...`（鲍勃分支）|波塞冬2(`node_out_1`, `sibling_1`) |波塞冬2(`node_in_2`, `sibling_2`) |分支翻转，因为位 = 0。
| 3 | 1 | `0x9bc4...` |波塞冬2(`node_out_2`, `sibling_2`) |波塞冬2(`sibling_3`, `node_out_3`) |更高的水平继续向上散列。 |
| 4 | 0 | `0xe112...` |波塞冬2(`node_out_3`, `sibling_3`) |波塞冬2(`node_in_4`, `sibling_4`) |根级；结果是后状态根。 |

该行的 `neighbour_leaf` 列填充有 Bob 的叶子
（`key = 0x1321...`、`value = 3`、`hash = Poseidon2(key, value) = 0x03ae...`）。当
进行验证时，AIR 检查：

1. 提供的邻居对应于第 2 层使用的同级。
2. 邻居键按字典顺序大于插入键并且
   左兄弟（Alice）按字典顺序较小。
3. 用邻居替换插入的叶子会重现状态前的根。这些检查一起证明了区间 `(0b01010,
0b01101)`更新前。生成 FASTPQ 跟踪的实现可以使用
逐字布局；上面的数值常数是说明性的。对于一个完整的
JSON见证，发出与上表中显示的列完全相同的列（带有
每个级别的数字后缀），使用小端字节字符串序列化
Norito JSON 帮助程序。