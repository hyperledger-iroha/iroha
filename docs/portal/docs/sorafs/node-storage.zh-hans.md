---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffa884bf745ab5f79c20d4b20baaba842878301dc56a66463c4520275ce4fd0b
source_last_modified: "2026-01-05T09:28:11.899124+00:00"
translation_last_reviewed: 2026-02-07
id: node-storage
title: SoraFS Node Storage Design
sidebar_label: Node Storage Design
description: Storage architecture, quotas, and lifecycle hooks for Torii nodes hosting SoraFS data.
translator: machine-google-reviewed
---

:::注意规范来源
:::

## SoraFS节点存储设计（草案）

本说明细化了 Iroha (Torii) 节点如何选择加入 SoraFS 数据
可用性层并专用于本地磁盘片用于存储和服务
大块。它补充了 `sorafs_node_client_protocol.md` 发现规范，并且
SF-1b 夹具通过概述存储端架构、资源来工作
必须落在节点和网关中的控制和配置管道
代码路径。实际操作员演习现场
[节点操作操作手册](./node-operations)。

### 目标

- 允许任何验证器或辅助 Iroha 进程将备用磁盘公开为
  SoraFS 提供者不影响核心账本职责。
- 保持存储模块确定性和 Norito 驱动：清单、
  块计划、可检索性证明 (PoR) 根和提供商广告是
  真相的来源。
- 强制执行运营商定义的配额，以便节点无法耗尽自己的资源
  接受过多的 pin 或获取请求。
- 表面健康/遥测（PoR 采样、块获取延迟、磁盘压力）
  回到治理和客户。

### 高层架构

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

关键模块：

- **网关**：公开 Norito HTTP 端点以进行 pin 建议、块获取
  请求、PoR 采样和遥测。它验证 Norito 有效负载和
  将请求编组到块存储中。重用现有的 Torii HTTP 堆栈
  以避免新的守护进程。
- **Pin 注册表**：`iroha_data_model::sorafs` 中跟踪的清单 pin 状态
  和 `iroha_core`。当清单被接受时，注册表会记录
  清单摘要、块计划摘要、PoR 根和提供者能力标志。
- **块存储**：摄取的磁盘支持的 `ChunkStore` 实现
  签署清单，使用 `ChunkProfile::DEFAULT` 实现块计划，以及
  在确定性布局下保留块。每个块都与一个相关联
  内容指纹和 PoR 元数据，因此采样可以重新验证，无需
  重新读取整个文件。
- **配额/调度程序**：强制执行操作员配置的限制（最大磁盘字节，
  最大未完成引脚、最大并行读取、块 TTL）和坐标
  IO 因此节点的账本职责不会匮乏。调度器也是
  负责使用有限的 CPU 提供 PoR 证明和采样请求。

### 配置

向 `iroha_config` 添加新部分：

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`：参与切换。当 false 时，网关返回 503
  存储端点和节点不在发现中通告。
- `data_dir`：块数据、PoR 树和获取遥测数据的根目录。
  默认为 `<iroha.data_dir>/sorafs`。
- `max_capacity_bytes`：固定块数据的硬限制。后台任务
  当达到限制时拒绝新的引脚。
- `max_parallel_fetches`：调度程序强制执行并发上限以平衡
  带宽/磁盘 IO 与验证器工作负载的比较。
- `max_pins`：应用之前节点接受的清单引脚的最大数量
  驱逐/背压。
- `por_sample_interval_secs`：自动 PoR 采样作业的节奏。每份工作
  样本 `N` 离开（可根据清单进行配置）并发出遥测事件。
  治理可以通过设置容量元数据来确定性地扩展 `N`
  密钥 `profile.sample_multiplier`（整数 `1-4`）。该值可能是单个
  数字/字符串或具有每个配置文件覆盖的对象，例如
  `{"default":2,"sorafs.sf2@1.0.0":3}`。
- `adverts`：提供商广告生成器用来填充的结构
  `ProviderAdvertV1` 字段（权益指针、QoS 提示、主题）。如果省略
  节点使用治理注册表中的默认值。

配置管道：

- `[sorafs.storage]` 在 `iroha_config` 中定义为 `SorafsStorage`，并且是
  从节点配置文件加载。
- `iroha_core` 和 `iroha_torii` 将存储配置线程到网关中
  启动时的构建器和块存储。
- 存在开发/测试环境覆盖（`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`），但是
  生产部署应依赖配置文件。

### CLI 实用程序

虽然 Torii 的 HTTP 表面仍在接线，但 `sorafs_node` 板条箱运送了
精简 CLI，以便操作员可以针对持久性数据编写摄取/导出演练脚本
后端.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` 需要 Norito 编码的清单 `.to` 文件以及匹配的有效负载
  字节。它根据清单的分块配置文件重建块计划，
  强制摘要奇偶校验，保留块文件，并可选择发出
  `chunk_fetch_specs` JSON blob，以便下游工具可以对
  布局。
- `export` 接受清单 ID 并将存储的清单/有效负载写入磁盘
  （使用可选的计划 JSON），因此灯具在不同环境中保持可重复性。

这两个命令都将 Norito JSON 摘要打印到标准输出，从而可以轻松通过管道输入
脚本。 CLI 包含集成测试，以确保清单和
在 Torii API 落地之前有效负载干净利落地往返。【crates/sorafs_node/tests/cli.rs:1】

> HTTP 奇偶校验
>
> Torii 网关现在公开由相同支持的只读帮助程序
> `NodeHandle`：
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — 返回存储的
> Norito 清单（base64）和摘要/元数据。【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — 返回确定性
> 用于下游工具的块计划 JSON (`chunk_fetch_specs`)。【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> 这些端点镜像 CLI 输出，以便管道可以从本地切换
> 无需更改解析器即可编写HTTP探针脚本。【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### 节点生命周期

1. **启动**：
   - 如果启用存储，节点将使用以下命令初始化块存储：
     配置目录和容量。这包括验证或创建
     PoR 清单数据库并将固定清单重播到热缓存。
   - 注册 SoraFS 网关路由（Norito JSON POST/GET 引脚端点，
     获取、PoR 样本、遥测）。
   - 生成 PoR 采样工作人员和配额监视器。
2. **发现/广告**：
   - 使用当前容量/运行状况生成 `ProviderAdvertV1` 文件，签名
     使用理事会批准的密钥，并通过发现频道发布。
     可用的。
3. **固定工作流程**：
   - 网关收到已签名的清单（包括块计划、PoR 根、理事会
     签名）。验证别名列表（需要 `sorafs.sf1@1.0.0`）并
     确保块计划与清单元数据匹配。
   - 检查配额。如果超出容量/引脚限制，请响应
     策略错误（Norito 结构）。
   - 将块数据流式传输到 `ChunkStore`，在我们摄取时验证摘要。
     更新 PoR 树并将清单元数据存储在注册表中。
4. **获取工作流程**：
   - 服务来自磁盘的块范围请求。调度程序强制执行
     `max_parallel_fetches` 并在饱和时返回 `429`。
   - 发出结构化遥测数据 (Norito JSON)，包含延迟、服务字节数和
     下游监控的错误计数。
5. **PoR 采样**：
   - 工作人员选择与重量成比例的清单（例如，存储的字节数）和
     使用块存储的 PoR 树运行确定性采样。
   - 保留治理审计的结果并将摘要包含在提供商中
     广告/遥测端点。
6. **驱逐/配额执行**：
   - 当达到容量时，节点默认拒绝新的引脚。可选地，
     操作员可以配置驱逐策略（例如，基于 TTL、LRU）
     治理模式已达成一致；目前，该设计假设有严格的配额，并且
     操作员发起的取消固定操作。

### 容量声明和调度集成- Torii 现在从 `/v2/sorafs/capacity/declare` 中继 `CapacityDeclarationRecord` 更新
  到嵌入式 `CapacityManager`，因此每个节点都会构建其自身的内存视图
  已提交的分块器和车道分配。管理器公开只读快照
  用于遥测 (`GET /v2/sorafs/capacity/state`) 并强制按配置文件或按通道
  接受新订单前的预约。【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v2/sorafs/capacity/schedule` 端点接受治理颁发的 `ReplicationOrderV1`
  有效负载。当订单针对本地提供商时，经理会检查
  重复调度，验证分块器/通道容量，保留切片，以及
  返回 `ReplicationPlan` 描述剩余容量，以便编排工具
  可以继续摄入。其他供应商的订单均通过
  `ignored` 响应以简化多操作员工作流程。【crates/iroha_torii/src/routing.rs:4845】
- 完成挂钩（例如，在摄取成功后触发）命中
  `POST /v2/sorafs/capacity/complete` 通过以下方式释放预订
  `CapacityManager::complete_order`。响应包括 `ReplicationRelease`
  快照（剩余总数、分块器/通道残差），因此编排工具可以
  无需轮询即可对下一个订单进行排队。后续工作会将其连接到块中
  一旦摄取逻辑落地，就存储管道。【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- 嵌入式 `TelemetryAccumulator` 可以通过以下方式进行变异
  `NodeHandle::update_telemetry`，让后台工作人员记录 PoR/正常运行时间样本
  并最终导出规范的 `CapacityTelemetryV1` 有效负载，而无需触及
  调度器内部结构。【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### 集成和未来的工作

- **治理**：通过存储遥测扩展 `sorafs_pin_registry_tracker.md`
  （PoR 成功率、磁盘利用率）。入学政策可能要求最低
  接受广告之前的容量或最低 PoR 成功率。
- **客户端 SDK**：公开新的存储配置（磁盘限制、别名），以便
  管理工具可以通过编程方式引导节点。
- **遥测**：与现有指标堆栈集成（Prometheus /
  OpenTelemetry），因此存储指标出现在可观察性仪表板中。
- **安全性**：在专用异步任务池中运行存储模块
  背压并考虑通过 io_uring 或 tokio 的沙箱块读取
  有界池，以防止恶意客户端耗尽资源。

这种设计使存储模块保持可选性和确定性，同时给出
操作员参与 SoraFS 数据可用性所需的旋钮
层。实施它将涉及 `iroha_config`、`iroha_core`、
`iroha_torii` 和 Norito 网关，以及提供商广告工具。