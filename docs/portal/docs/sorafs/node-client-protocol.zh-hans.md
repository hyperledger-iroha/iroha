---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e0cdd8242b45628e688d94ebec08e2d9900787ec93a81417e6683d399d43be2d
source_last_modified: "2026-01-22T14:35:36.781385+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 节点 ↔ 客户端协议

本指南总结了规范协议定义
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
使用字节级 Norito 布局和变更日志的上游规范；门户网站
副本使操作要点与 SoraFS 操作手册的其余部分保持接近。

## 提供商广告和验证

SoraFS 提供商八卦 `ProviderAdvertV1` 有效负载（请参阅
`crates/sorafs_manifest::provider_advert`) 由受监管运营商签署。
广告引脚发现元数据和多源护栏
Orchestrator 在运行时强制执行。

- **终身** — `issued_at < expires_at ≤ issued_at + 86 400 s`。供应商
  应每 12 小时刷新一次。
- **功能 TLV** — TLV 列表通告传输功能（Torii、
  QUIC+Noise、SoraNet 继电器、供应商扩展）。未知代码可能会被跳过
  当 `allow_unknown_capabilities = true` 时，遵循 GREASE 指南。
- **QoS 提示** — `availability` 层（热/暖/冷），最大检索
  延迟、并发限制和可选的流预算。 QoS 必须符合
  观察遥测并通过入场审核。
- **端点和集合主题** — 具有 TLS/ALPN 的具体服务 URL
  元数据以及客户端在构建时应订阅的发现主题
  守卫组。
- **路径多样性策略** — `min_guard_weight`、AS/池扇出上限，以及
  `provider_failure_threshold` 使确定性的多点获取成为可能。
- **配置文件标识符** - 提供者必须公开规范句柄（例如
  `sorafs.sf1@1.0.0`);可选的 `profile_aliases` 帮助旧客户端迁移。

验证规则拒绝零权益、空能力/端点/主题列表，
生命周期顺序错误或缺少 QoS 目标。录取信封比较
八卦更新之前的广告和提案机构（`compare_core_fields`）。

### 范围获取扩展

支持范围的提供程序包括以下元数据：

|领域|目的|
|--------|---------|
| `CapabilityType::ChunkRangeFetch` |声明 `max_chunk_span`、`min_granularity` 和对齐/验证标志。 |
| `StreamBudgetV1` |可选并发/吞吐量信封（`max_in_flight`、`max_bytes_per_sec`、可选 `burst`）。需要范围能力。 |
| `TransportHintV1` |有序的传输首选项（例如，`torii_http_range`、`quic_stream`、`soranet_relay`）。优先级为 `0–15`，重复项将被拒绝。 |

工具支持：

- 提供商广告管道必须验证范围能力、流预算和
  在发出确定性负载进行审计之前传输提示。
- `cargo xtask sorafs-admission-fixtures` 捆绑规范多源
  降级赛程旁边的广告
  `fixtures/sorafs_manifest/provider_admission/`。
- 省略 `stream_budget` 或 `transport_hints` 的范围功能广告是
  在调度之前被 CLI/SDK 加载器拒绝，保留多源
  安全带符合 Torii 录取期望。

## 网关范围端点

网关接受镜像广告元数据的确定性 HTTP 请求。

### `GET /v1/sorafs/storage/car/{manifest_id}`

|要求 |详情 |
|-------------|---------|
| **标题** | `Range`（与块偏移对齐的单个窗口）、`dag-scope: block`、`X-SoraFS-Chunker`、可选 `X-SoraFS-Nonce` 和强制 base64 `X-SoraFS-Stream-Token`。 |
| **回应** | `206` 与 `Content-Type: application/vnd.ipld.car`、描述服务窗口的 `Content-Range`、`X-Sora-Chunk-Range` 元数据以及回显的分块器/令牌标头。 |
| **故障模式** | `416` 表示范围未对齐，`401` 表示丢失/无效令牌，`429` 表示超出流/字节预算。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

使用相同标头加上确定性块摘要的单块获取。
当不需要 CAR 切片时，对于重试或取证下载很有用。

## 多源 Orchestrator 工作流程

当启用 SF-6 多源获取时（通过 `sorafs_fetch` 的 Rust CLI，
SDK 通过 `sorafs_orchestrator`）：

1. **收集输入** — 解码清单块计划，拉取最新广告，
   并可选择传递遥测快照（`--telemetry-json` 或
   `TelemetrySnapshot`）。
2. **构建记分板** — `Orchestrator::build_scoreboard` 评估
   资格和记录拒绝原因； `sorafs_fetch --scoreboard-out`
   保留 JSON。
3. **调度块** — `fetch_with_scoreboard`（或 `--plan`）强制范围
   限制、流预算、重试/对等上限 (`--retry-budget`、
   `--max-peers`），并为每个请求发出清单范围的流令牌。
4. **验证收据** — 输出包括 `chunk_receipts` 和
   `provider_reports`； CLI 摘要持续存在 `provider_reports`，
   `chunk_receipts` 和 `ineligible_providers` 用于证据包。

向运营商/SDK 提出的常见错误：

|错误 |描述 |
|--------|-------------|
| `no providers were supplied` |筛选后没有符合条件的条目。 |
| `no compatible providers available for chunk {index}` |特定块的范围或预算不匹配。 |
| `retry budget exhausted after {attempts}` |增加 `--retry-budget` 或驱逐失败的对等点。 |
| `no healthy providers remaining` |所有提供商在多次失败后都被禁用。 |
| `streaming observer failed` |下游 CAR 编写器中止。 |
| `orchestrator invariant violated` |捕获清单、记分板、遥测快照和 CLI JSON 以进行分类。 |

## 遥测与证据

- 协调器发出的指标：  
  `sorafs_orchestrator_active_fetches`，`sorafs_orchestrator_fetch_duration_ms`，
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  （按清单/区域/提供商标记）。在配置中或通过设置 `telemetry_region`
  CLI 标记，以便仪表板按队列分区。
- CLI/SDK 获取摘要包括持久记分板 JSON、块收据、
  和提供商报告，必须以 SF-6/SF-7 门的部署包形式提供。
- 网关处理程序暴露 `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  因此，SRE 仪表板可以将编排器决策与服务器行为关联起来。

## CLI 和 REST 助手

- `iroha app sorafs pin list|show`、`alias list` 和 `replication list` 包裹
  pin-registry REST 端点并使用证明块打印原始 Norito JSON
  为审计证据。
- `iroha app sorafs storage pin` 和 `torii /v1/sorafs/pin/register` 接受 Norito
  或 JSON 清单加上可选的别名证明和后继者；格式错误的证明
  提高 `400`，用 `Warning: 110` 证明过时的样张表面 `503`，以及
  硬过期的证明返回 `412`。
- `iroha app sorafs repair list` 镜像修复队列过滤器，同时
  `repair claim|complete|fail|escalate` 提交签名的工作人员操作或斜线
  向 Torii 提出建议。 Slash 提案可能包含治理批准摘要
  （批准/拒绝/弃权票数加上approved_at/finalized_at
  时间戳）；当存在时，它必须满足法定人数和争议/上诉窗口，
  否则，该提案将一直存在争议，直到截止日期投票结果出来为止。
- 修复列表和工作队列选择按照 SLA 截止日期、故障严重性和提供商待办事项以及确定性决定因素（排队时间、清单摘要、票证 ID）进行排序。
- 修复状态响应包括包含 base64 Norito 的 `events` 数组
  `RepairTaskEventV1` 按出现次数排序的条目以进行审计跟踪；清单
  仅限于最近的转换。
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` 发出只读信号
  来自本地舱单存储的保留报告作为审计证据。
- REST 端点（`/v1/sorafs/pin`、`/v1/sorafs/aliases`、
  `/v1/sorafs/replication`）包括证明结构，以便客户可以
  在采取行动之前根据最新的块头验证数据。

## 参考文献

- 规范规范：
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito 类型：`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI 帮助程序：`crates/iroha_cli/src/commands/sorafs.rs`，
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orchestrator 箱：`crates/sorafs_orchestrator`
- 仪表板包：`dashboards/grafana/sorafs_fetch_observability.json`