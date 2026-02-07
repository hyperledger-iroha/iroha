---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 多源提供商广告和日程安排

此页面提炼了规范规范
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
使用该文档逐字记录 Norito 架构和变更日志；门户副本
使操作员指南、SDK 说明和遥测参考与其他内容保持一致
SoraFS 运行手册。

## Norito 架构添加

### 范围能力 (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – 每个请求的最大连续跨度（字节），`≥ 1`。
- `min_granularity` – 寻求分辨率，`1 ≤ value ≤ max_chunk_span`。
- `supports_sparse_offsets` – 允许一个请求中的非连续偏移。
- `requires_alignment` – 当为 true 时，偏移量必须与 `min_granularity` 对齐。
- `supports_merkle_proof` – 表示 PoR 见证支持。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` 强制规范编码
因此八卦有效负载仍然是确定性的。

### `StreamBudgetV1`
- 字段：`max_in_flight`、`max_bytes_per_sec`、可选 `burst_bytes`。
- 验证规则（`StreamBudgetV1::validate`）：
  - `max_in_flight ≥ 1`，`max_bytes_per_sec > 0`。
  - `burst_bytes`（如果存在）必须是 `> 0` 和 `≤ max_bytes_per_sec`。

### `TransportHintV1`
- 字段：`protocol: TransportProtocol`、`priority: u8`（0-15 窗口由
  `TransportHintV1::validate`）。
- 已知协议：`torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- 每个提供商的重复协议条目将被拒绝。

### `ProviderAdvertBodyV1` 添加
- 可选 `stream_budget: Option<StreamBudgetV1>`。
- 可选 `transport_hints: Option<Vec<TransportHintV1>>`。
- 这两个领域现在都流经 `ProviderAdmissionProposalV1`，治理
  信封、CLI 装置和遥测 JSON。

## 验证和治理绑定

`ProviderAdvertBodyV1::validate` 和 `ProviderAdmissionProposalV1::validate`
拒绝格式错误的元数据：

- 范围能力必须解码并满足跨度/粒度限制。
- 流预算/传输提示需要匹配
  `CapabilityType::ChunkRangeFetch` TLV 和非空提示列表。
- 重复的传输协议和无效的优先级提高了验证性
  广告八卦之前的错误。
- 录取信封通过以下方式比较范围元数据的提案/广告
  `compare_core_fields` 因此不匹配的八卦有效负载会被提前拒绝。

回归覆盖率位于
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## 工装和固定装置

- 提供商广告负载必须包含 `range_capability`、`stream_budget` 和
  `transport_hints` 元数据。通过 `/v1/sorafs/providers` 响应进行验证并
  入场装置； JSON 摘要应包括解析的功能，
  流预算和用于遥测摄取的提示数组。
- `cargo xtask sorafs-admission-fixtures` 表面流预算和传输
  其 JSON 工件内部有提示，以便仪表板跟踪功能采用情况。
- `fixtures/sorafs_manifest/provider_admission/` 下的赛程现在包括：
  - 规范的多源广告，
  - `multi_fetch_plan.json`，因此 SDK 套件可以重播确定性的多点
    获取计划。

## Orchestrator 和 Torii 集成

- Torii `/v1/sorafs/providers` 返回解析的范围能力元数据
  与 `stream_budget` 和 `transport_hints`。降级警告在以下情况下触发：
  提供者省略新的元数据，网关范围端点强制执行相同的元数据
  直接客户的限制。
- 多源编排器 (`sorafs_car::multi_fetch`) 现在强制范围
  分配工作时的限制、能力调整和流预算。单位
  测试涵盖了块太大、稀疏搜索和限制场景。
- `sorafs_car::multi_fetch` 流降级信号（对齐失败、
  限制请求），以便操作员可以追踪特定提供商的原因
  计划时跳过。

## 遥测参考

Torii 范围获取仪器提供 **SoraFS 获取可观测性**
Grafana 仪表板 (`dashboards/grafana/sorafs_fetch_observability.json`) 和
配对警报规则 (`dashboards/alerts/sorafs_fetch_rules.yml`)。

|公制|类型 |标签|描述 |
|--------|------|--------|------------|
| `torii_sorafs_provider_range_capability_total` |仪表| `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`) |提供商广告范围能力特征。 |
| `torii_sorafs_range_fetch_throttle_events_total` |专柜| `reason`（`quota`、`concurrency`、`byte_rate`）|按策略分组的限制范围提取尝试。 |
| `torii_sorafs_range_fetch_concurrency_current` |仪表| — |主动保护的流消耗共享并发预算。 |

PromQL 片段示例：

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

在启用之前使用限制计数器确认配额强制执行
多源协调器默认值，并在并发接近时发出警报
为您的车队提供最大预算。