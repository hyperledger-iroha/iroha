---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 编解码器参考

Norito 是 Iroha 的规范序列化层。磁盘上的每一条线上消息
有效负载，跨组件 API 使用 Norito，因此节点同意相同的字节
即使它们在不同的硬件上运行。本页总结了移动部件
并指向 `norito.md` 中的完整规范。

## 核心布局

|组件|目的|来源 |
| ---| ---| ---|
| **标题** |协商功能（打包结构/序列、紧凑长度、压缩标志）并嵌入 CRC64 校验和，以便在解码之前检查有效负载的完整性。 | `norito::header` — 请参阅 `norito.md`（“标头和标志”，存储库根）|
| **裸负载** |用于散列/比较的确定性值编码。相同的布局由标头包装以进行传输。 | `norito::codec::{Encode, Decode}` |
| **压缩** |通过 `compression` 标志字节激活可选的 Zstd（和实验性 GPU 加速）。 | `norito.md`，“压缩协商”|

完整的标志注册表（打包结构、打包序列、紧凑长度、压缩）
住在 `norito::header::flags` 中。 `norito::header::Flags` 暴露便利性
检查运行时检查；保留的布局位被解码器拒绝。

## 获得支持

`norito_derive` 附带 `Encode`、`Decode`、`IntoSchema` 和 JSON 帮助程序派生。
主要约定：

- 当 `packed-struct` 功能为时，结构/枚举派生打包布局
  启用（默认）。实现位于 `crates/norito_derive/src/derive_struct.rs` 中
  该行为记录在 `norito.md`（“打包布局”）中。
- 打包集合在 v1 中使用固定宽度的序列头和偏移量；仅
  每个值的长度前缀受 `COMPACT_LEN` 影响。
- JSON 助手 (`norito::json`) 提供确定性 Norito 支持的 JSON
  开放 API。使用 `norito::json::{to_json_pretty, from_json}` — 切勿使用 `serde_json`。

## 多编解码器和标识符表

Norito 将其多编解码器分配保留在 `norito::multicodec` 中。参考资料
表（哈希值、密钥类型、有效负载描述符）维护在 `multicodec.md` 中
在存储库根目录下。添加新标识符时：

1.更新`norito::multicodec::registry`。
2. 扩展`multicodec.md` 中的表。
3. 如果下游绑定 (Python/Java) 消耗了映射，则重新生成下游绑定。

## 重新生成文档和装置

由于门户当前托管散文摘要，请使用上游 Markdown
来源作为真理的来源：

- **规格**：`norito.md`
- **多编解码器表**：`multicodec.md`
- **基准**：`crates/norito/benches/`
- **黄金测试**：`crates/norito/tests/`

当 Docusaurus 自动化上线时，门户将通过
从这些数据中提取数据的同步脚本（在 `docs/portal/scripts/` 中跟踪）
文件。在此之前，只要规范发生变化，请手动保持此页面对齐。