---
lang: zh-hant
direction: ltr
source: docs/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26e6f90205e98b5db87d442eb7e4e7691cce47e1c33ef3d11c9bfba25269294e
source_last_modified: "2026-01-14T17:53:24.552406+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 文檔

日本語版の概要は [`README.ja.md`](./README.ja.md) を迴轉してください。

該工作區從同一代碼庫提供了兩個發行版：**Iroha 2**（自託管部署）和
**Iroha 3 / SORA Nexus**（單一全局 Nexus 分類賬）。兩者都重複使用相同的 Iroha 虛擬機 (IVM) 和
Kotodama 工具鏈，因此合約和字節碼在部署目標之間保持可移植性。文件適用
除非另有說明，否則均適用。

在[主要 Iroha 文檔](https://docs.iroha.tech/) 中，您將找到：

- [入門指南](https://docs.iroha.tech/get-started/)
- [SDK 教程](https://docs.iroha.tech/guide/tutorials/) 適用於 Rust、Python、Javascript 和 Java/Kotlin
- [API參考](https://docs.iroha.tech/reference/torii-endpoints.html)

特定版本的白皮書和規格：

- [Iroha 2 白皮書](./source/iroha_2_whitepaper.md) — 自託管網絡規範。
- [Iroha 3 (SORA Nexus) 白皮書](./source/iroha_3_whitepaper.md) — Nexus 多通道和數據空間設計。
- [數據模型和 ISI 規範（實現派生）](./source/data_model_and_isi_spec.md) — 逆向工程行為參考。
- [ZK 信封 (Norito)](./source/zk_envelopes.md) — 原生 IPA/STARK Norito 信封和驗證者期望。

## 本地化

日語 (`*.ja.*`)、希伯來語 (`*.he.*`)、西班牙語 (`*.es.*`)、葡萄牙語
(`*.pt.*`)、法語 (`*.fr.*`)、俄語 (`*.ru.*`)、阿拉伯語 (`*.ar.*`) 和烏爾都語
(`*.ur.*`) 文檔存根位於每個英文源文件旁邊。參見
[`docs/i18n/README.md`](./i18n/README.md) 了解有關生成和
維護翻譯，以及在中添加新語言的指南
未來。

## 工具

在此存儲庫中，您可以找到 Iroha 2 工具的文檔：

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) 配置結構宏（請參閱 `config_base` 功能）
- [分析構建步驟](./profile_build.md) 用於識別緩慢的 `iroha_data_model` 編譯任務

## Swift / iOS SDK 參考

- [Swift SDK 概述](./source/sdk/swift/index.md) — 管道助手、加速切換和 Connect/WebSocket API。
- [連接快速入門](./connect_swift_ios.md) — SDK 優先演練以及 CryptoKit 參考。
- [Xcode 集成指南](./connect_swift_integration.md) — 使用 ChaChaPoly 和框架助手將 NoritoBridgeKit/Connect 連接到應用程序中。
- [SwiftUI 演示貢獻者指南](./norito_demo_contributor.md) — 針對本地 Torii 節點運行 iOS 演示，以及加速說明。
- 在發布 Swift 工件或 Connect 更改之前運行 `make swift-ci`；它驗證夾具奇偶性、儀表板源和 Buildkite `ci/xcframework-smoke:<lane>:device_tag` 元數據。

## Norito（序列化編解碼器）

Norito 是工作區序列化編解碼器。我們不使用 `parity-scale-codec`
（規模）。文檔或基準與 SCALE 相比，僅適用於
背景；所有生產路徑均使用 Norito。 `norito::codec::{Encode, Decode}`
API 提供無標頭（“裸”）Norito 有效負載，用於散列和連線
效率 — 它是 Norito，而不是 SCALE。

最新狀態：

- 具有固定標頭的確定性編碼/解碼（魔法、版本、16 字節模式、壓縮、長度、CRC64、標誌）。
- CRC64-XZ 校驗和，具有運行時選擇的加速：
  - x86_64 PCLMULQDQ（無進位乘法）+ Barrett 縮減，折疊在 32 字節塊上。
  - aarch64 PMULL 具有匹配的折疊功能。
  - 8 切片和按位回退以實現可移植性。
- 由派生和核心類型實現的編碼長度提示以減少分配。
- 解碼期間更大的流緩衝區 (64 KiB) 和增量 CRC 更新。
- 可選的 zstd 壓縮； GPU 加速具有功能門控性和確定性。
- 自適應路徑選擇：`norito::to_bytes_auto(&T)` 選擇 no
  壓縮、CPU zstd 或 GPU 卸載 zstd（已編譯且可用時）
  基於有效負載大小和緩存的硬件功能。選擇只影響
  性能和標頭的 `compression` 字節；有效負載語義不變。

有關奇偶校驗測試、基準測試和使用示例，請參閱 `crates/norito/README.md`。

注意：一些子系統文檔（例如 IVM 加速和 ZK 電路）正在不斷發展。當功能不完整時，文件會指出剩餘的工作和行進方向。

狀態端點編碼註釋
- Torii `/status` 主體默認使用 Norito 和無標頭（“裸”）有效負載以實現緊湊性。客戶端應首先嘗試 Norito 解碼。
- 服務器可能會在請求時返回 JSON；如果 `content-type` 是 `application/json`，客戶端將回退到 JSON。
- 有線格式為 Norito，而不是 SCALE。 `norito::codec::{Encode,Decode}` API 用於裸變體。