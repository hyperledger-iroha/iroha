---
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8de31f9e066b729fda8324b8847badba23de926888574d02a44fb0e6d4472f77
source_last_modified: "2026-01-16T17:12:51.444585+00:00"
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
| **标题** |使用 magic/version/schema 哈希、CRC64、长度和压缩标签构建有效负载； v1 需要 `VERSION_MINOR = 0x00` 并根据支持的掩码（默认 `0x00`）验证标头标志。 | `norito::header` — 请参阅 `norito.md`（“标头和标志”，存储库根）|
| **裸负载** |用于散列/比较的确定性值编码。在线传输始终使用标头；裸字节仅供内部使用。 | `norito::codec::{Encode, Decode}` |
| **压缩** |通过标头压缩字节选择可选的 Zstd（和实验性 GPU 加速）。 | `norito.md`，“压缩协商”|

布局标志注册表（packed-struct、packed-seq、field bitset、compact
长度）位于 `norito::header::flags` 中。 V1 默认为标志 `0x00` 但
接受受支持掩码内的显式标头标志；未知位是
被拒绝了。 `norito::header::Flags` 保留用于内部检查和
未来的版本。

## 获得支持

`norito_derive` 附带 `Encode`、`Decode`、`IntoSchema` 和 JSON 帮助程序派生。
主要约定：

- 派生生成 AoS 和打包代码路径； v1 默认为 AoS
  布局（标志 `0x00`），除非标头标志选择打包变体。
  实现位于 `crates/norito_derive/src/derive_struct.rs` 中。
- 影响布局的功能（`packed-struct`、`packed-seq`、`compact-len`）是
  通过标头标志选择加入，并且必须在对等点之间一致地编码/解码。
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