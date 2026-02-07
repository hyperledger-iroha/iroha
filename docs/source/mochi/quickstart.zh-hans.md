---
lang: zh-hans
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2025-12-29T18:16:35.985408+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI 快速入门

**MOCHI** 是本地 Hyperledger Iroha 网络的桌面管理程序。本指南将介绍
安装先决条件，构建应用程序，启动 egui shell，并使用
用于日常开发的运行时工具（设置、快照、擦除）。

## 先决条件

- Rust 工具链：`rustup default stable`（工作区目标版本 2024 / Rust 1.82+）。
- 平台工具链：
  - macOS：Xcode 命令行工具 (`xcode-select --install`)。
  - Linux：GCC、pkg-config、OpenSSL 标头 (`sudo apt install build-essential pkg-config libssl-dev`)。
- Iroha 工作区依赖项：
  - `cargo xtask mochi-bundle` 需要内置 `irohad`、`kagami` 和 `iroha_cli`。通过构建一次
    `cargo build -p irohad -p kagami -p iroha_cli`。
- 可选：`direnv` 或 `cargo binstall` 用于管理本地货物二进制文件。

MOCHI shell 到 CLI 二进制文件。确保可以通过环境变量发现它们
下面或在 PATH 上可用：

|二进制|环境覆盖 |笔记|
|----------|------------------------------------|--------------------------------------------------------|
| `irohad` | `MOCHI_IROHAD` |监督同行|
| `kagami` | `MOCHI_KAGAMI` |生成创世清单/快照 |
| `iroha_cli` | `MOCHI_IROHA_CLI` |即将推出的辅助功能可选 |

## 构建 MOCHI

从存储库根目录：

```bash
cargo build -p mochi-ui-egui
```

此命令构建 `mochi-core` 和 egui 前端。要生成可分发的包，请运行：

```bash
cargo xtask mochi-bundle
```

捆绑任务在 `target/mochi-bundle` 下组装二进制文件、清单和配置存根。

## 启动 egui shell

直接从 Cargo 运行 UI：

```bash
cargo run -p mochi-ui-egui
```

默认情况下，MOCHI 在临时数据目录中创建单点预设：

- 数据根：`$TMPDIR/mochi`。
- Torii 基本端口：`8080`。
- P2P基本端口：`1337`。

启动时使用 CLI 标志覆盖默认值：

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

当省略 CLI 标志时，环境变量会镜像相同的覆盖：设置 `MOCHI_DATA_ROOT`，
`MOCHI_PROFILE`、`MOCHI_CHAIN_ID`、`MOCHI_TORII_START`、`MOCHI_P2P_START`、`MOCHI_RESTART_MODE`、
`MOCHI_RESTART_MAX` 或 `MOCHI_RESTART_BACKOFF_MS` 来预置 Supervisor 构建器；二进制路径
继续尊重 `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`，并且 `MOCHI_CONFIG` 指向
显式 `config/local.toml`。

## 设置和持久化

从仪表板工具栏打开 **设置** 对话框以调整主管配置：

- **数据根** — 对等配置、存储、日志和快照的基本目录。
- **Torii / P2P 基本端口** — 用于确定性分配的起始端口。
- **日志可见性** — 在日志查看器中切换 stdout/stderr/system 通道。

高级旋钮，例如主管重启策略
`config/local.toml`。将 `[supervisor.restart] mode = "never"` 设置为禁用
事件调试时自动重启，或者调整
`max_restarts`/`backoff_ms`（通过配置文件或 CLI 标志
`--restart-mode`、`--restart-max`、`--restart-backoff-ms`) 控制重试
行为。应用更改会重建主管，重新启动任何正在运行的对等点，并将覆盖写入
`config/local.toml`。配置合并保留不相关的密钥，以便高级用户可以保留
手动调整以及 MOCHI 管理的值。

## 快照和擦除/重新生成

**维护**对话框公开了两个安全操作：

- **导出快照** — 将对等存储/配置/日志和当前创世清单复制到
  活动数据根下的 `snapshots/<label>`。标签会自动清理。
- **恢复快照** - 重新水化对等存储、快照根、配置、日志和起源
  来自现有包的清单。 `Supervisor::restore_snapshot` 接受绝对路径或
  清理后的 `snapshots/<label>` 文件夹名称； UI 反映了此流程，因此维护 → 恢复
  可以重播证据包而无需手动触摸文件。
- **擦除和重新创世** — 停止运行对等点，删除存储目录，通过重新生成创世
  Kagami，并在擦除完成后重新启动对等点。

回归测试涵盖了这两个流程（`export_snapshot_captures_storage_and_metadata`，
`wipe_and_regenerate_resets_storage_and_genesis`）以保证确定性输出。

## 日志和流

仪表板一目了然地显示数据/指标：

- **日志** — 遵循 `irohad` stdout/stderr/系统生命周期消息。在“设置”中切换频道。
- **块/事件** — 托管流通过指数退避和注释帧自动重新连接
  带有 Norito 解码的摘要。
- **状态** — 轮询 `/status` 并呈现队列深度、吞吐量和延迟的迷你图。
- **启动准备** - 按**开始**（单个对等点或所有对等点）后，MOCHI 探测
  `/status` 有界退避；横幅报告每个对等点何时准备好（以及观察到的
  队列深度）或在准备超时时显示 Torii 错误。

状态浏览器和编辑器的选项卡提供对帐户、资产、对等点和常见内容的快速访问
无需离开 UI 即可执行指令。 Peers 视图镜像 `FindPeers` 查询，以便您可以确认
在运行集成测试之前，当前在验证器集中注册了哪些公钥。

使用编写器工具栏的 **管理签名库** 按钮导入或编辑签名权限。的
对话框将条目写入活动网络根 (`<data_root>/<profile>/signers.json`)，并保存
保管库密钥可立即用于交易预览和提交。当金库是
清空作曲家后退到捆绑的开发密钥，以便本地工作流程继续工作。
表单现在涵盖铸币/销毁/转移（包括隐式接收）、域/帐户/资产定义
注册、帐户准入政策、多重签名提案、空间目录清单 (AXT/AMX)、
SoraFS pin 清单以及授予或撤销角色等治理操作非常常见
无需手写 Norito 有效负载即可演练路线图创作任务。

## 清理和故障排除- 停止应用程序以终止受监督的对等点。
- 删除数据根（`rm -rf <data_root>`）以重置所有状态。
- 如果 Kagami 或 irohad 位置发生变化，请更新环境变量或使用以下命令重新运行 MOCHI
  适当的 CLI 标志；设置对话框将在下次应用时保留新路径。

对于额外的自动化检查 `mochi/mochi-core/tests`（主管生命周期测试）和
`mochi/mochi-integration` 用于模拟 Torii 场景。运送捆绑或接线
桌面到 CI 管道中，请参阅 {doc}`mochi/packaging` 指南。

## 本地测试门

在发送补丁之前运行 `ci/check_mochi.sh`，以便共享 CI 门执行所有三个 MOCHI
板条箱：

```bash
./ci/check_mochi.sh
```

帮助程序针对 `mochi-core`、`mochi-ui-egui` 执行 `cargo check`/`cargo test`，以及
`mochi-integration`，捕获夹具漂移（规范块/事件捕获）和 egui 线束
一次性回归。如果脚本报告陈旧的装置，请重新运行忽略的再生测试，
例如：

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

重新生成后重新运行门可确保更新的字节在推送之前保持一致。