---
lang: zh-hans
direction: ltr
source: docs/source/mochi_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffca282b5a2bb2506f46ac2c7a8985ff2f7d10a46bc999a002277956c9f452b0
source_last_modified: "2026-01-05T09:28:12.023255+00:00"
translation_last_reviewed: 2026-02-07
title: MOCHI Architecture Plan
description: High-level design for the MOCHI local-network GUI supervisor.
translator: machine-google-reviewed
---

# MOCHI 架构计划


## 目标

- 快速引导单点或多点（四节点 BFT）本地网络。
- 将 `kagami`、`irohad` 和支持二进制文件封装在友好的 GUI 工作流程中。
- 通过 Torii HTTP/WebSocket 端点显示实时块、事件和状态数据。
- 为交易和 Iroha 特别指令 (ISI) 提供结构化构建器，并进行本地签名和提交。
- 管理快照、重新生成流程和配置调整，无需手动编辑文件。
- 作为单个跨平台 Rust 二进制文件发布，没有 webview 或 Docker 依赖性。

## 架构概述

MOCHI 分为两个主要 crate，位于新的 `/mochi` 目录中（请参阅
[MOCHI 快速入门](mochi/quickstart.md) 用于构建和使用说明）：

1. `mochi-core`：一个无头库，负责配置模板、密钥和创世材料生成、监督子进程、驱动 Torii 客户端以及管理文件系统状态。
2. `mochi-ui-egui`：基于 `egui`/`eframe` 构建的桌面应用程序，用于呈现用户界面并通过 `mochi-core` API 委托所有编排。

其他前端（例如 Tauri shell）可以稍后连接到 `mochi-core`，而无需重新设计管理程序逻辑。

## 流程模型

- 对等节点作为单独的 `irohad` 子进程运行。 MOCHI 永远不会将对等点作为库进行链接，从而避免不稳定的内部 API 并匹配生产部署拓扑。
- 创世和密钥材料是通过 `kagami` 调用以及用户提供的输入（链 ID、初始账户、资产）创建的。
- 配置文件从 TOML 模板生成，填写 Torii 和 P2P 端口、存储路径、快照设置和可信对等列表。生成的配置存储在每个网络工作空间目录下。
- 主管跟踪进程生命周期，流式传输日志表面的 stdout/stderr，并轮询 `/status`、`/metrics` 和 `/configuration` 端点的运行状况。
- 瘦 Torii 客户端层包装 HTTP 和 WebSocket 调用，尽可能依赖 Iroha Rust 客户端包，以避免重新实现 SCALE 编码/解码。

## `mochi-core` 支持的用户流程- **网络创建向导**：选择单点或四点配置文件，选择目录，然后调用 `kagami` 来生成身份和创世。
- **生命周期控制**：启动、停止、重新启动对等点；表面实时指标；暴露日志尾部；切换运行时配置端点（例如日志级别）。
- **块和事件流**：订阅 `/block/stream` 和 `/events`，为 UI 面板存储内存滚动缓冲区。
- **状态资源管理器**：运行 Norito 支持的 `/query` 调用以列出域、帐户、资产和资产定义以及分页帮助程序和元数据摘要。
- **交易编辑器**：阶段铸造/转移指令草稿，将其批处理为签名交易，预览 Norito 有效负载，通过 `/transaction` 提交，并监控生成的事件；保险库签名挂钩仍然是未来的迭代。
- **快照和重新创世**：编排 Kura 快照导出/导入、擦除存储并重新生成创世材料以进行快速重置。

## UI 层 (`mochi-ui-egui`)

- 使用 `egui`/`eframe` 传送单个本机可执行文件，无需外部运行时。
- 布局包括：
  - **网络仪表板**，带有对等卡、运行状况指示器和快速操作。
  - **块**面板流式传输最近的提交并允许高度搜索。
  - **事件**面板按哈希或帐户过滤交易状态。
  - **状态资源管理器** 域、帐户、资产和资产定义选项卡，其中包含分页 Norito 结果以及用于检查的原始转储。
- **Composer** 表单，具有可批量铸造/传输调色板、队列管理（添加/删除/清除）、原始 Norito 预览以及由签名者库支持的提交反馈，以便操作员可以在开发者和真实权限之间进行交换。
- **创世与快照**管理视图。
- **运行时切换和数据目录快捷方式的设置**。
- UI通过通道订阅来自`mochi-core`的异步更新；核心公开了一个 `SupervisorHandle`，用于传输结构化事件（对等状态、块头、交易更新）。

## 本地开发笔记

- 工作区配置设置 `ZSTD_SYS_USE_PKG_CONFIG=1`，以便 `zstd-sys` 链接到主机 `libzstd`，而不是获取供应商提供的存档。这使得 pqcrypto 相关的构建（和 MOCHI 测试）在离线或沙盒环境中运行。

## 包装和分发

- MOCHI 捆绑（或在 `PATH` 上发现）`irohad`、`iroha_cli` 和 `kagami` 二进制文件。
- 使用 `rustls` 进行出站 HTTPS 以避免 OpenSSL 依赖性。
- 将所有生成的工件存储在专用应用程序数据根（例如，`~/.local/share/mochi` 或等效平台）中，并具有每个网络子目录。 GUI 提供“在 Finder/Explorer 中显示”帮助程序。
- 在启动对等点之前自动检测并保留 Torii (8080+) 和 P2P (1337+) 端口以防止冲突。

## 未来的扩展（超出 MVP 的范围）- 备用前端（Tauri、CLI 无头模式）共享 `mochi-core`。
- 用于分布式测试集群的多主机编排。
- 共识内部结构的可视化工具（Sumeragi 轮状态、八卦计时）。
- 与 CI 管道集成以实现自动化的临时网络快照。
- 用于自定义仪表板或特定领域检查器的插件系统。

## 参考文献

- [Torii 端点](https://docs.iroha.tech/reference/torii-endpoints.html)
- [对等配置参数](https://docs.iroha.tech/reference/peer-config/params.html)
- [`kagami` 存储库文档](https://github.com/hyperledger-iroha/iroha)
- [Iroha 特别说明](https://iroha-test.readthedocs.io/en/iroha2-dev/references/isi/)