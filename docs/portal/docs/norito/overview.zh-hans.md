---
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2025-12-29T18:16:35.153135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 概述

Norito 是跨 Iroha 使用的二进制序列化层：它定义数据如何
结构在线路上进行编码，保存在磁盘上，并在之间交换
合约和主机。工作区中的每个板条箱都依赖于 Norito 而不是
`serde` 因此不同硬件上的对等点产生相同的字节。

本概述总结了核心部分以及规范参考文献的链接。

## 架构概览

- **标头 + 有效负载** – 每个 Norito 消息均以功能协商开头
  标头（标志、校验和）后跟裸负载。打包布局和
  压缩是通过标头位协商的。
- **确定性编码** – `norito::codec::{Encode, Decode}` 实现
  裸编码。将有效负载包装在标头中时会重复使用相同的布局，因此
  散列和签名仍然是确定性的。
- **架构 + 派生** – `norito_derive` 生成 `Encode`、`Decode` 和
  `IntoSchema` 实现。默认情况下启用打包结构/序列
  并记录在 `norito.md` 中。
- **多编解码器注册表** – 哈希、密钥类型和有效负载的标识符
  描述符位于 `norito::multicodec` 中。权威的表是
  维护在 `multicodec.md` 中。

## 工具

|任务|命令/API |笔记|
| ---| ---| ---|
|检查标题/部分 | `ivm_tool inspect <file>.to` |显示 ABI 版本、标志和入口点。 |
|在 Rust 中编码/解码 | `norito::codec::{Encode, Decode}` |针对所有核心数据模型类型实施。 |
| JSON 互操作 | `norito::json::{to_json_pretty, from_json}` |由 Norito 值支持的确定性 JSON。 |
|生成文档/规格 | `norito.md`、`multicodec.md` |存储库根目录中的真实来源文档。 |

## 开发流程

1. **添加派生** – 新数据首选 `#[derive(Encode, Decode, IntoSchema)]`
   结构。除非绝对必要，否则避免手写序列化程序。
2. **验证打包布局** – 使用 `cargo test -p norito`（以及打包布局）
   `scripts/run_norito_feature_matrix.sh` 中的功能矩阵）以确保新
   布局保持稳定。
3. **重新生成文档** – 当编码更改时，更新 `norito.md` 和
   多编解码器表，然后刷新门户页面 (`/reference/norito-codec`
   以及本概述）。
4. **保留测试 Norito-first** – 集成测试应使用 Norito JSON
   helper 而不是 `serde_json`，因此它们使用与生产相同的路径。

## 快速链接

- 规格：[`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- 多编解码器分配：[`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- 特征矩阵脚本：`scripts/run_norito_feature_matrix.sh`
- 打包布局示例：`crates/norito/tests/`

将此概述与快速入门指南 (`/norito/getting-started`) 结合起来，了解
使用 Norito 编译和运行字节码的实践演练
有效负载。