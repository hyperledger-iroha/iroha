---
lang: zh-hans
direction: ltr
source: docs/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26e6f90205e98b5db87d442eb7e4e7691cce47e1c33ef3d11c9bfba25269294e
source_last_modified: "2026-01-14T17:53:24.552406+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 文档

日本语版の概要は [`README.ja.md`](./README.ja.md) を回转してください。

该工作区从同一代码库提供了两个发行版：**Iroha 2**（自托管部署）和
**Iroha 3 / SORA Nexus**（单一全局 Nexus 分类账）。两者都重复使用相同的 Iroha 虚拟机 (IVM) 和
Kotodama 工具链，因此合约和字节码在部署目标之间保持可移植性。文件适用
除非另有说明，否则均适用。

在[主要 Iroha 文档](https://docs.iroha.tech/) 中，您将找到：

- [入门指南](https://docs.iroha.tech/get-started/)
- [SDK 教程](https://docs.iroha.tech/guide/tutorials/) 适用于 Rust、Python、Javascript 和 Java/Kotlin
- [API参考](https://docs.iroha.tech/reference/torii-endpoints.html)

特定版本的白皮书和规格：

- [Iroha 2 白皮书](./source/iroha_2_whitepaper.md) — 自托管网络规范。
- [Iroha 3 (SORA Nexus) 白皮书](./source/iroha_3_whitepaper.md) — Nexus 多通道和数据空间设计。
- [数据模型和 ISI 规范（实现派生）](./source/data_model_and_isi_spec.md) — 逆向工程行为参考。
- [ZK 信封 (Norito)](./source/zk_envelopes.md) — 原生 IPA/STARK Norito 信封和验证者期望。

## 本地化

日语 (`*.ja.*`)、希伯来语 (`*.he.*`)、西班牙语 (`*.es.*`)、葡萄牙语
(`*.pt.*`)、法语 (`*.fr.*`)、俄语 (`*.ru.*`)、阿拉伯语 (`*.ar.*`) 和乌尔都语
(`*.ur.*`) 文档存根位于每个英文源文件旁边。参见
[`docs/i18n/README.md`](./i18n/README.md) 了解有关生成和
维护翻译，以及在中添加新语言的指南
未来。

## 工具

在此存储库中，您可以找到 Iroha 2 工具的文档：

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) 配置结构宏（请参阅 `config_base` 功能）
- [分析构建步骤](./profile_build.md) 用于识别缓慢的 `iroha_data_model` 编译任务

## Swift / iOS SDK 参考

- [Swift SDK 概述](./source/sdk/swift/index.md) — 管道助手、加速切换和 Connect/WebSocket API。
- [连接快速入门](./connect_swift_ios.md) — SDK 优先演练以及 CryptoKit 参考。
- [Xcode 集成指南](./connect_swift_integration.md) — 使用 ChaChaPoly 和框架助手将 NoritoBridgeKit/Connect 连接到应用程序中。
- [SwiftUI 演示贡献者指南](./norito_demo_contributor.md) — 针对本地 Torii 节点运行 iOS 演示，以及加速说明。
- 在发布 Swift 工件或 Connect 更改之前运行 `make swift-ci`；它验证夹具奇偶性、仪表板源和 Buildkite `ci/xcframework-smoke:<lane>:device_tag` 元数据。

## Norito（序列化编解码器）

Norito 是工作区序列化编解码器。我们不使用 `parity-scale-codec`
（规模）。文档或基准与 SCALE 相比，仅适用于
背景；所有生产路径均使用 Norito。 `norito::codec::{Encode, Decode}`
API 提供无标头（“裸”）Norito 有效负载，用于散列和连线
效率 — 它是 Norito，而不是 SCALE。

最新状态：

- 具有固定标头的确定性编码/解码（魔法、版本、16 字节模式、压缩、长度、CRC64、标志）。
- CRC64-XZ 校验和，具有运行时选择的加速：
  - x86_64 PCLMULQDQ（无进位乘法）+ Barrett 缩减，折叠在 32 字节块上。
  - aarch64 PMULL 具有匹配的折叠功能。
  - 8 切片和按位回退以实现可移植性。
- 由派生和核心类型实现的编码长度提示以减少分配。
- 解码期间更大的流缓冲区 (64 KiB) 和增量 CRC 更新。
- 可选的 zstd 压缩； GPU 加速具有功能门控性和确定性。
- 自适应路径选择：`norito::to_bytes_auto(&T)` 选择 no
  压缩、CPU zstd 或 GPU 卸载 zstd（已编译且可用时）
  基于有效负载大小和缓存的硬件功能。选择只影响
  性能和标头的 `compression` 字节；有效负载语义不变。

有关奇偶校验测试、基准测试和使用示例，请参阅 `crates/norito/README.md`。

注意：一些子系统文档（例如 IVM 加速和 ZK 电路）正在不断发展。当功能不完整时，文件会指出剩余的工作和行进方向。

状态端点编码注释
- Torii `/status` 主体默认使用 Norito 和无标头（“裸”）有效负载以实现紧凑性。客户端应首先尝试 Norito 解码。
- 服务器可能会在请求时返回 JSON；如果 `content-type` 是 `application/json`，客户端将回退到 JSON。
- 有线格式为 Norito，而不是 SCALE。 `norito::codec::{Encode,Decode}` API 用于裸变体。