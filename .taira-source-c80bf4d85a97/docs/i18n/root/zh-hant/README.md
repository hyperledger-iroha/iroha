---
lang: zh-hant
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hyperledger Iroha

[![許可證](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha 是一個用於許可和聯盟部署的確定性區塊鏈平台。它通過Iroha虛擬機（IVM）提供賬戶/資產管理、鏈上權限和智能合約。

> 在 [`status.md`](./status.md) 中跟踪工作區狀態和最近的更改。

## 發行曲目

該存儲庫從同一代碼庫提供了兩個部署軌道：

- **Iroha 2**：自託管許可/聯盟網絡。
- **Iroha 3 (SORA Nexus)**：使用相同核心板條箱的面向 Nexus 的部署軌道。

兩個軌道共享相同的核心組件，包括 Norito 序列化、Sumeragi 共識和 Kotodama -> IVM 工具鏈。

## 存儲庫佈局

- [`crates/`](./crates)：核心 Rust 箱子 (`iroha`、`irohad`、`iroha_cli`、`iroha_core`、`ivm`、 `norito` 等）。
- [`integration_tests/`](./integration_tests)：跨組件網絡/集成測試。
- [`IrohaSwift/`](./IrohaSwift)：Swift SDK 包。
- [`java/iroha_android/`](./java/iroha_android)：Android SDK 包。
- [`docs/`](./docs)：用戶/操作員/開發人員文檔。

## 快速入門

### 先決條件

- [防銹穩定](https://www.rust-lang.org/tools/install)
- 可選：Docker + Docker Compose 用於本地多點運行

### 構建和測試（工作區）

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

注意事項：

- 完整的工作區構建可能需要大約 20 分鐘。
- 完整的工作空間測試可能需要幾個小時。
- 工作區目標為 `std`（不支持 WASM/no-std 版本）。

### 有針對性的測試命令

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK測試命令

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## 運行本地網絡

啟動提供的 Docker Compose 網絡：

```bash
docker compose -f defaults/docker-compose.yml up
```

針對默認客戶端配置使用 CLI：

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

有關特定於守護程序的本機部署步驟，請參閱 [`crates/irohad/README.md`](./crates/irohad/README.md)。

## API 和可觀察性

Torii 公開 Norito 和 JSON API。常見的操作符端點：

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

請參閱以下位置的完整端點參考：

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## 核心箱

- [`crates/iroha`](./crates/iroha)：客戶端庫。
- [`crates/irohad`](./crates/irohad)：對等守護程序二進製文件。
- [`crates/iroha_cli`](./crates/iroha_cli)：參考 CLI。
- [`crates/iroha_core`](./crates/iroha_core)：賬本/核心執行引擎。
- [`crates/iroha_config`](./crates/iroha_config)：鍵入的配置模型。
- [`crates/iroha_data_model`](./crates/iroha_data_model)：規範數據模型。
- [`crates/iroha_crypto`](./crates/iroha_crypto)：加密原語。
- [`crates/norito`](./crates/norito)：確定性序列化編解碼器。
- [`crates/ivm`](./crates/ivm)：Iroha 虛擬機。
- [`crates/iroha_kagami`](./crates/iroha_kagami)：密鑰/創世/配置工具。

## 文檔地圖

- 主要文檔索引：[`docs/README.md`](./docs/README.md)
- 創世紀：[`docs/genesis.md`](./docs/genesis.md)
- 共識（Sumeragi）：[`docs/source/sumeragi.md`]（./docs/source/sumeragi.md）
- 交易管道：[`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P 內部結構：[`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM 系統調用：[`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama 語法：[`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito 接線格式：[`norito.md`](./norito.md)
- 當前工作跟踪：[`status.md`](./status.md)、[`roadmap.md`](./roadmap.md)

## 翻譯

日語概述：[`README.ja.md`](./README.ja.md)

其他概述：
[`README.he.md`](./README.he.md)、[`README.es.md`](./README.es.md)、[`README.pt.md`](./README.pt.md)、 [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

翻譯工作流程：[`docs/i18n/README.md`](./docs/i18n/README.md)

## 貢獻和幫助

- 貢獻指南：[`CONTRIBUTING.md`](./CONTRIBUTING.md)
- 社區/支持渠道：[`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## 許可證

Iroha 在 Apache-2.0 下獲得許可。請參閱 [`LICENSE`](./LICENSE)。

文檔已獲得 CC-BY-4.0 許可：http://creativecommons.org/licenses/by/4.0/