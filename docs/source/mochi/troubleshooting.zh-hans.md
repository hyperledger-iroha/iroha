---
lang: zh-hans
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2025-12-29T18:16:35.985892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI 故障排除指南

当本地 MOCHI 集群拒绝启动、陷入困境时，请使用此 Runbook
重新启动循环，或停止流式传输块/事件/状态更新。它扩展了
通过转变主管行为来制定路线图项目“文档和部署”
`mochi-core` 进入具体恢复步骤。

## 1. 急救人员清单

1. 捕获 MOCHI 正在使用的数据根。默认如下
   `$TMPDIR/mochi/<profile-slug>`；自定义路径出现在 UI 标题栏中，并且
   通过 `cargo run -p mochi-ui-egui -- --data-root ...`。
2. 从工作区根目录运行 `./ci/check_mochi.sh`。这验证了核心，
   在开始修改配置之前，先了解 UI 和集成箱。
3. 记下预设（`single-peer` 或 `four-peer-bft`）。生成的拓扑
   确定数据根下应有多少对等文件夹/日志。

## 2.收集日志和遥测证据

`NetworkPaths::ensure`（参见 `mochi/mochi-core/src/config.rs`）创建了一个稳定的
布局：

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

进行更改之前请按照以下步骤操作：

- 使用 **Logs** 选项卡或直接打开 `logs/<alias>.log` 捕获最后一个
  每个对等点 200 行。主管尾部标准输出/标准错误/系统通道
  通过 `PeerLogStream`，因此这些文件与 UI 输出匹配。
- 通过**维护→导出快照**导出快照（或调用
  `Supervisor::export_snapshot`）。快照捆绑了存储、配置和
  登录 `snapshots/<timestamp>-<label>/`。
- 如果问题涉及流小部件，请复制 `ManagedBlockStream`，
  `ManagedEventStream` 和 `ManagedStatusStream` 运行状况指标
  仪表板。 UI 显示上次重新连接尝试和错误原因；抢
  事件记录的屏幕截图。

## 3. 解决对等启动问题

大多数对等启动失败分为三类：

### 缺少二进制文件或错误覆盖

`SupervisorBuilder` 外壳为 `irohad`、`kagami` 和（未来）`iroha_cli`。
如果 UI 报告“无法生成进程”或“权限被拒绝”，请点 MOCHI
在已知良好的二进制文件中：

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

您可以设置 `MOCHI_IROHAD`、`MOCHI_KAGAMI` 和 `MOCHI_IROHA_CLI` 以避免
重复输入标志。调试捆绑包构建时，比较
`mochi/mochi-ui-egui/src/config/` 中的 `BundleConfig` 与
`target/mochi-bundle`。

### 端口冲突

`PortAllocator` 在写入配置之前探测环回接口。如果你看到
`failed to allocate Torii port` 或 `failed to allocate P2P port`，另一个
进程已在默认范围 (8080/1337) 上侦听。重新启动MOCHI
具有明确的基础：

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

构建器将从这些基地扇出连续端口，因此保留一个范围
根据您的预设调整大小（`peer_count` 对等体→ `peer_count` 每个传输端口）。

### 创世和存储损坏如果 Kagami 在发出清单之前退出，对等点将立即崩溃。检查
数据根内的 `genesis/*.json`/`.toml`。重新运行
`--kagami /path/to/kagami` 或将 **设置** 对话框指向右侧的二进制文件。
对于存储损坏，请使用维护部分的**擦除和重新生成**
按钮（如下所示），而不是手动删除文件夹；它重新创建了
重新启动进程之前的对等目录和快照根。

### 调整自动重启

`config/local.toml` 中的 `[supervisor.restart]` （或 CLI 标志
`--restart-mode`、`--restart-max`、`--restart-backoff-ms`) 控制频率
主管重试失败的对等点。当需要UI时设置`mode = "never"`
立即出现第一个故障，或缩短 `max_restarts`/`backoff_ms`
收紧必须快速失败的 CI 作业的重试窗口。

## 4. 安全重置对等点

1. 从仪表板停止受影响的对等点或退出 UI。主管
   拒绝在对等方运行时擦除存储（`PeerHandle::wipe_storage`
   返回 `PeerStillRunning`)。
2. 导航至**维护 → 擦除和重新生成**。莫奇将：
   -删除`peers/<alias>/storage`；
   - 重新运行 Kagami 以在 `genesis/` 下重建配置/创世；和
   - 使用保留的 CLI/环境覆盖重新启动对等点。
3. 如果您必须手动执行此操作：
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   然后，重新启动 MOCHI，以便 `NetworkPaths::ensure` 重新创建树。

擦除之前务必存档 `snapshots/<timestamp>` 文件夹，即使在本地也是如此
开发 - 这些捆绑包捕获所需的精确 `irohad` 日志和配置
重现错误。

### 4.1 从快照恢复

当实验损坏存储或者您需要重播已知良好的状态时，请使用维护
对话框的 **恢复快照** 按钮（或调用 `Supervisor::restore_snapshot`）而不是复制
手动目录。提供捆绑包的绝对路径或经过清理的文件夹名称
在 `snapshots/` 下。主管将：

1. 停止任何正在运行的节点；
2. 验证快照的 `metadata.json` 是否与当前 `chain_id` 和对等计数匹配；
3. 将 `peers/<alias>/{storage,snapshot,config.toml,latest.log}` 复制回活动配置文件中；和
4. 如果对等点之前正在运行，请在重新启动对等点之前恢复 `genesis/genesis.json`。

如果快照是为不同的预设或链标识符创建的，则恢复调用将返回
`SupervisorError::Config`，这样您就可以获取匹配的捆绑包，而不是默默地混合人工制品。
每个预设至少保留一个新快照，以加速恢复演练。

## 5. 修复块/事件/状态流- **流停滞但对等体健康。**检查**事件**/**块**面板
  用于红色状态栏。单击“停止”，然后单击“启动”以强制托管流
  重新订阅；主管记录每次重新连接尝试（使用对等别名和
  错误），以便您可以确认退避阶段。
- **状态覆盖已过时。** `ManagedStatusStream` 每隔一次轮询 `/status`
  两秒并在“STATUS_POLL_INTERVAL *”之后标记数据过时
  STATUS_STALE_MULTIPLIER`（默认六秒）。如果徽章保持红色，请验证
  对等配置中的 `torii_status_url` 并确保网关或 VPN 不是
  阻止环回连接。
- **事件解码失败。** UI 打印解码阶段（原始字节、
  `BlockSummary` 或 Norito 解码）和违规交易哈希。出口
  通过剪贴板按钮事件，以便您可以在测试中重现解码
  （`mochi-core` 公开了辅助构造函数
  `mochi/mochi-core/src/torii.rs`）。

当流反复崩溃时，使用确切的对等别名更新问题并
错误字符串 (`ToriiErrorKind`)，因此路线图遥测里程碑保持联系
到具体的证据。