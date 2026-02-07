---
lang: zh-hans
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

[![许可证](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha 是一个用于许可和联盟部署的确定性区块链平台。它通过Iroha虚拟机（IVM）提供账户/资产管理、链上权限和智能合约。

> 在 [`status.md`](./status.md) 中跟踪工作区状态和最近的更改。

## 发行曲目

该存储库从同一代码库提供了两个部署轨道：

- **Iroha 2**：自托管许可/联盟网络。
- **Iroha 3 (SORA Nexus)**：使用相同核心板条箱的面向 Nexus 的部署轨道。

两个轨道共享相同的核心组件，包括 Norito 序列化、Sumeragi 共识和 Kotodama -> IVM 工具链。

## 存储库布局

- [`crates/`](./crates)：核心 Rust 箱子 (`iroha`、`irohad`、`iroha_cli`、`iroha_core`、`ivm`、 `norito` 等）。
- [`integration_tests/`](./integration_tests)：跨组件网络/集成测试。
- [`IrohaSwift/`](./IrohaSwift)：Swift SDK 包。
- [`java/iroha_android/`](./java/iroha_android)：Android SDK 包。
- [`docs/`](./docs)：用户/操作员/开发人员文档。

## 快速入门

### 先决条件

- [防锈稳定](https://www.rust-lang.org/tools/install)
- 可选：Docker + Docker Compose 用于本地多点运行

### 构建和测试（工作区）

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

注意事项：

- 完整的工作区构建可能需要大约 20 分钟。
- 完整的工作空间测试可能需要几个小时。
- 工作区目标为 `std`（不支持 WASM/no-std 版本）。

### 有针对性的测试命令

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK测试命令

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

## 运行本地网络

启动提供的 Docker Compose 网络：

```bash
docker compose -f defaults/docker-compose.yml up
```

针对默认客户端配置使用 CLI：

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

有关特定于守护程序的本机部署步骤，请参阅 [`crates/irohad/README.md`](./crates/irohad/README.md)。

## API 和可观察性

Torii 公开 Norito 和 JSON API。常见的操作符端点：

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

请参阅以下位置的完整端点参考：

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## 核心箱

- [`crates/iroha`](./crates/iroha)：客户端库。
- [`crates/irohad`](./crates/irohad)：对等守护程序二进制文件。
- [`crates/iroha_cli`](./crates/iroha_cli)：参考 CLI。
- [`crates/iroha_core`](./crates/iroha_core)：账本/核心执行引擎。
- [`crates/iroha_config`](./crates/iroha_config)：键入的配置模型。
- [`crates/iroha_data_model`](./crates/iroha_data_model)：规范数据模型。
- [`crates/iroha_crypto`](./crates/iroha_crypto)：加密原语。
- [`crates/norito`](./crates/norito)：确定性序列化编解码器。
- [`crates/ivm`](./crates/ivm)：Iroha 虚拟机。
- [`crates/iroha_kagami`](./crates/iroha_kagami)：密钥/创世/配置工具。

## 文档地图

- 主要文档索引：[`docs/README.md`](./docs/README.md)
- 创世纪：[`docs/genesis.md`](./docs/genesis.md)
- 共识（Sumeragi）：[`docs/source/sumeragi.md`]（./docs/source/sumeragi.md）
- 交易管道：[`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P 内部结构：[`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM 系统调用：[`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama 语法：[`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito 接线格式：[`norito.md`](./norito.md)
- 当前工作跟踪：[`status.md`](./status.md)、[`roadmap.md`](./roadmap.md)

## 翻译

日语概述：[`README.ja.md`](./README.ja.md)

其他概述：
[`README.he.md`](./README.he.md)、[`README.es.md`](./README.es.md)、[`README.pt.md`](./README.pt.md)、 [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

翻译工作流程：[`docs/i18n/README.md`](./docs/i18n/README.md)

## 贡献和帮助

- 贡献指南：[`CONTRIBUTING.md`](./CONTRIBUTING.md)
- 社区/支持渠道：[`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## 许可证

Iroha 在 Apache-2.0 下获得许可。请参阅 [`LICENSE`](./LICENSE)。

文档已获得 CC-BY-4.0 许可：http://creativecommons.org/licenses/by/4.0/