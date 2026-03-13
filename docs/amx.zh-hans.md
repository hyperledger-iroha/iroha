---
lang: zh-hans
direction: ltr
source: docs/amx.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f11f0a83efc46035aeeaf4c1ad626a2a773303e9dfab188704016cf483a78ce6
source_last_modified: "2026-01-23T08:31:38.611123+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AMX 执行和操作指南

**状态：** 草案（NX-17）  
**受众：** 核心协议、AMX/共识工程师、SRE/Telemetry、SDK & Torii 团队  
**上下文：** 完成路线图项目“文档（所有者：Docs）——使用时序图、错误目录、操作员期望以及生成/使用 PVO 的开发人员指南更新 `docs/amx.md`。”【roadmap.md:2497】

## 总结

原子跨数据空间事务 (AMX) 允许单次提交触及多个数据空间 (DS)，同时保留 1s 时隙最终性、确定性故障代码和私有 DS 片段的机密性。本指南捕获了时序模型、规范错误处理、操作员证据要求以及开发人员对证明验证对象 (PVO) 的期望，因此路线图交付成果在 Nexus 设计文件 (`docs/source/nexus.md`) 之外保持独立。

主要保证：

- 每个 AMX 提交都会收到确定性的准备/提交预算；使用记录的代码而不是悬挂通道溢出中止。
- 未达到预算的 DA 样本将被记录为缺少可用性证据，并且事务将继续排队等待下一个时隙，而不是默默地停止吞吐量。
- 证明验证对象 (PVO) 通过让客户端/批处理程序预先注册 Nexus 主机在时隙中快速验证的工件，将繁重的证明与 1s 时隙解耦。
- IVM 主机从空间目录派生每个数据空间 AXT 策略：句柄必须以目录中通告的通道为目标，提供最新的清单根，满足 expiry_slot、handle_era 和 sub_nonce 最小值，并在执行前使用 `PermissionDenied` 拒绝未知数据空间。
- 时隙到期使用 `nexus.axt.slot_length_ms`（默认 `1`ms，在 `1`ms 和 `600_000`ms 之间验证）加上有界的 `nexus.axt.max_clock_skew_ms`（默认 `0`ms，由时隙长度和`60_000`ms）。主机计算 `current_slot = block.creation_time_ms / slot_length_ms`，应用偏差限额来证明和处理过期检查，并拒绝通告偏差大于配置限制的句柄。
- 证明缓存 TTL 边界重用：`nexus.axt.proof_cache_ttl_slots`（默认 `1`，经过验证的 `1`–`64`）限制接受或拒绝的证明在主机缓存中保留的时间；一旦 TTL 窗口或证明的 `expiry_slot` 过去，条目就会丢失，因此重放保护保持有限。
- 重放账本保留：`nexus.axt.replay_retention_slots`（默认 `128`，经过验证的 `1`–`4_096`）设置保留的句柄使用历史记录的最小槽窗口，以便跨对等/重新启动拒绝重放；将其与您期望操作员发出的最长句柄有效性窗口对齐。账本持久保存在 WSV 中，在启动时进行水合，并在保留窗口和句柄到期（以较晚者为准）后确定性地进行修剪，因此对等交换机不会重新打开重播间隙。
- 调试缓存状态：Torii 公开 `/v2/debug/axt/cache`（遥测/开发人员门）以返回当前 AXT 策略快照版本、最近的拒绝（通道/原因/版本）、缓存的证明（数据空间/状态/清单根/插槽）和拒绝提示（`next_min_handle_era`/`next_min_sub_nonce`）。使用此端点来确认插槽/清单轮换是否反映在缓存状态中，并在故障排除期间确定性地刷新句柄。

## 时隙时序模型

### 时间轴

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- 预算与全球账本计划一致：mempool 70ms，DA commit ≤300ms，共识 300ms，IVM/AMX 250ms，结算 40ms，guard 40ms。【roadmap.md:2529】
- 违反 DA 窗口的交易将被记录为缺少可用性证据，并在下一个时隙中重试；所有其他违规表面代码，例如 `AMX_TIMEOUT` 或 `SETTLEMENT_ROUTER_UNAVAILABLE`。
- 保护切片吸收遥测导出和最终审核，因此即使导出器短暂滞后，插槽仍会在 1 秒处关闭。
- 配置提示：默认保持严格过期（`slot_length_ms = 1`、`max_clock_skew_ms = 0`）。对于 1s 节奏集 `slot_length_ms = 1_000` 和 `max_clock_skew_ms = 250`；对于 2 秒的节奏，请使用 `slot_length_ms = 2_000` 和 `max_clock_skew_ms = 500`。验证窗口之外的值（`1`–`600_000`ms 或 `max_clock_skew_ms` 大于槽长度/`60_000`ms）将在配置解析时被拒绝，并且通告的句柄偏差必须保持在配置的范围内。

###跨DS泳道

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

每个 DS 片段必须在通道组装插槽之前完成其 30ms 准备窗口。丢失的证明会保留在内存池中的下一个槽位，而不是阻塞对等点。

### 仪器清单

|公制/迹线 |来源 | SLO / 警报 |笔记|
|----------------|--------|-------------|--------|
| `iroha_slot_duration_ms`（直方图）/`iroha_slot_duration_ms_latest`（仪表）| `iroha_telemetry` | p95 ≤ 1000 毫秒 | Ci 门在 `ans3.md` 中描述。 |
| `iroha_da_quorum_ratio` | `iroha_telemetry`（提交挂钩）|每 30 分钟窗口 ≥0.95 |源自缺失可用性遥测，因此每个块都会更新仪表（`crates/iroha_core/src/telemetry.rs:3524`，`crates/iroha_core/src/telemetry.rs:4558`）。 |
| `iroha_amx_prepare_ms` | IVM 主机 |每个 DS 范围 p95 ≤ 30ms |驱动器 `AMX_TIMEOUT` 中止。 |
| `iroha_amx_commit_ms` | IVM 主机 |每个 DS 范围 p95 ≤ 40ms |涵盖增量合并+触发器执行。 |
| `iroha_ivm_exec_ms` | IVM 主机 |如果每通道 >250 毫秒，则发出警报 |镜像 IVM 覆盖块执行窗口。 |
| `iroha_amx_abort_total{stage}` |执行人|如果 >0.05 中止/槽或持续单级尖峰，则发出警报 |阶段标签：`prepare`、`exec`、`commit`。 |
| `iroha_amx_lock_conflicts_total` | AMX 调度程序 |如果每个槽冲突 >0.1，则发出警报 |表示 R/W 设置不准确。 |
| `iroha_axt_policy_reject_total{lane,reason}` | IVM 主机 |留意尖峰 |区分manifest/lane/era/sub_nonce/expiry 拒绝。 |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | IVM 主机 |预计cache_miss仅在启动/清单更改时出现持续的失误表明政策的水分已经过时。 |
| `iroha_axt_proof_cache_events_total{event}` | IVM 主机 |主要期待 `hit`/`miss` | `reject`/`expired` 峰值通常表示明显的漂移或过时的证明。 |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | IVM 主机 |检查缓存的证明 |缓存证明的计量值是 expiry_slot （应用了倾斜）。 |
|缺少可用性证据 (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) |车道遥测|如果每个 DS 的交易量 >5%，则发出警报 |意味着证明者或证明是滞后的。 |

`/v2/debug/axt/cache` 镜像 `iroha_axt_proof_cache_state` 仪表，并为操作员提供每个数据空间快照（状态、清单根、已验证/到期槽）。

`iroha_amx_commit_ms` 和 `iroha_ivm_exec_ms` 共享相同的延迟桶
`iroha_amx_prepare_ms`。中止计数器用通道 ID 标记每次拒绝
和阶段（`prepare` = 覆盖构建/验证，`exec` = IVM 块执行，
`commit` = 增量合并 + 触发重播），因此遥测可以突出显示是否
争用来自读/写不匹配或状态后合并。

运营商必须将这些指标与插槽接受证据一起存档以供审核，并在 `status.md` 中记录回归。

### AXT 黄金赛程

Norito 描述符/句柄/策略快照的固定装置位于 `crates/iroha_data_model/tests/fixtures/axt_golden.rs`，并在 `crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`) 中提供再生帮助程序。 CoreHost 使用 `core_host_enforces_fixture_snapshot_fields` (`crates/ivm/tests/core_host_policy.rs`) 中的相同装置来执行通道绑定、清单根匹配、expiry_slot 新鲜度、handle_era/sub_nonce 最小值和缺失数据空间拒绝。
- 多数据空间 JSON 固定装置 (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) 固定描述符/触摸模式、规范 Norito 字节和 Poseidon 绑定 (`compute_descriptor_binding`)。 `axt_descriptor_fixture` 测试保护编码字节，SDK 可以使用 `AxtDescriptorBuilder::builder` 和 `TouchManifest::from_read_write` 为文档/SDK 组装确定性示例。

### Lane 目录映射和清单

- AXT 策略快照是根据空间目录清单集和通道目录构建的。每个数据空间都映射到其配置的通道；活动清单提供清单哈希、激活纪元 (`min_handle_era`) 和子随机数下限。没有活动清单的 UAID 绑定仍会发出具有归零清单根的策略条目，因此通道选通保持活动状态，直到真正的清单落地。
- 快照中的 `current_slot` 源自最新提交的块时间戳 (`creation_time_ms / slot_length_ms`)，仅在提交的标头可用之前回落到块高度。
- 遥测将水合快照显示为 `iroha_axt_policy_snapshot_version`（Norito 编码快照哈希的低 64 位），并通过 `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}` 缓存事件。拒绝计数器使用标签 `lane`、`manifest`、`era`、`sub_nonce` 和 `expiry`，因此操作员可以立即查看哪个字段阻止了句柄。

### 跨数据空间可组合性清单- 确认空间目录中列出的每个数据空间都有通道条目和活动清单；旋转应该在发出新句柄之前刷新绑定和清单根。归零根意味着句柄将保持被拒绝状态，直到清单出现为止，并且主机/块验证现在拒绝呈现归零清单根的句柄。
- 启动时和空间目录更改后，策略快照指标上预计会出现一个 `cache_miss`，随后是稳定的 `cache_hit` 事件；持续的错过率表明清单提要已过时或丢失。
- 当句柄被拒绝时，查看 `iroha_axt_policy_reject_total{lane,reason}` 和快照版本，以决定是否请求刷新句柄 (`expiry`/`era`/`sub_nonce`) 或修复通道/清单绑定（`lane`/`manifest`）。 Torii 调试端点 `/v2/debug/axt/cache` 还返回 `reject_hints` 以及 `dataspace`、`target_lane`、`next_min_handle_era` 和 `next_min_sub_nonce`，以便操作员可以在策略碰撞后确定性地刷新句柄。

### SDK示例：无令牌出口的远程支出

1. 构建一个 AXT 描述符，列出拥有资产的数据空间以及本地所需的任何读/写操作；保持描述符的确定性，以便绑定哈希保持稳定。
2. 调用 `AXT_TOUCH` 获取具有您期望的清单视图的远程数据空间；如果主机需要，可以选择通过 `AXT_VERIFY_DS_PROOF` 附加证明。
3. 请求或刷新资产句柄，并使用在远程数据空间内使用的 `RemoteSpendIntent` 调用 `AXT_USE_ASSET_HANDLE`（无桥接腿）。预算执行针对上述快照使用句柄的 `remaining`、`per_use`、`sub_nonce`、`handle_era` 和 `expiry_slot`。
4.通过`AXT_COMMIT`提交；如果主机返回 `PermissionDenied`，则使用拒绝标签来决定是否获取新句柄（expiry/sub_nonce/era）或修复清单/通道绑定。

## 操作员期望

1. **时段前准备**
   - 确保每个配置文件的 DA 证明者池（A=12、B=9、C=7）健康；证明者流失记录在该槽的空间目录快照中。
   - 在启用新的工作负载组合之前，验证 `iroha_amx_prepare_ms` 是否低于代表性运行者的预算。

2. **槽内监控**
   - 对缺失可用性峰值（两个连续时段> 5%）和 `AMX_TIMEOUT` 发出警报，因为两者都表明错过了预算。
   - 跟踪 PVO 缓存利用率（`iroha_pvo_cache_hit_ratio`，由证明服务导出）以证明偏离路径验证与提交保持同步。

3. **证据捕获**
   - 将 DA 收据集、AMX 准备直方图和 PVO 缓存报告附加到 `status.md` 引用的夜间工件包。
   - 每当 DA 抖动、oracle 停止或缓冲区耗尽测试运行时，在 `ops/drill-log.md` 中记录混沌钻取输出。

4. **运行手册维护**
   - 每当 AMX 错误代码或覆盖发生变化时，更新 Android/Swift SDK Runbook，以便客户端团队继承确定性故障语义。
   - 使配置片段（例如，`iroha_config.amx.*`）与 `docs/source/nexus.md` 中的规范参数保持同步。

## 遥测和故障排除

### 遥测快速参考

|来源 |捕捉什么 |命令/路径|证据预期|
|--------|-----------------|----------------|------------------------|
| Prometheus (`iroha_telemetry`) |插槽和 AMX SLO：`iroha_slot_duration_ms`、`iroha_amx_prepare_ms`、`iroha_amx_commit_ms`、`iroha_da_quorum_ratio`、`iroha_amx_abort_total{stage}` |抓取 `https://$TORII/telemetry/metrics` 或从 `docs/source/telemetry.md` 中描述的仪表板导出。 |将直方图快照（以及触发的警报历史记录）附加到夜间 `status.md` 注释中，以便审核员可以查看 p95/p99 值和警报状态。 |
| Torii RBC 快照 | DA/RBC 积压：每个会话块积压、视图/高度元数据和 DA 可用性计数器（`sumeragi_da_gate_block_total{reason="missing_local_data"}`；`sumeragi_rbc_da_reschedule_total` 是旧版）。 | `GET /v2/sumeragi/rbc` 和 `GET /v2/sumeragi/rbc/sessions`（有关示例，请参阅 `docs/source/samples/sumeragi_rbc_status.md`）。 |每当 AMX DA 警报触发时，存储 JSON 响应（带有时间戳）；将它们包含在事件包中，以便审核人员可以确认背压与遥测相匹配。 |
|证明服务指标| PVO 缓存运行状况：`iroha_pvo_cache_hit_ratio`、缓存填充/逐出计数器、证明队列深度 | `GET /metrics` 在证明服务 (`IROHA_PVO_METRICS_URL`) 上或通过共享 OTLP 收集器。 |导出缓存命中率和队列深度以及 AMX 插槽指标，以便路线图 OA/PVO 门具有确定性的人工制品。 |
|验收线束|受控抖动下的端到端混合负载（时隙/DA/RBC/PVO）|重新运行 `ci/acceptance/slot_1s.yml`（或 CI 中的相同作业）并将日志包 + 生成的工件存档在 `artifacts/acceptance/slot_1s/<timestamp>/` 中。 |在 GA 之前以及起搏器/DA 设置更改时需要；在操作员移交数据包中包含 YAML 运行摘要以及 Prometheus 快照。 |

### 故障排除手册

|症状|先检查|建议补救措施|
|--------|-------------|--------------------------|
| `iroha_slot_duration_ms` p95 爬行超过 1000ms |从 `/telemetry/metrics` 导出 Prometheus 加上最新的 `/v2/sumeragi/rbc` 快照以确认 DA 延期；与最后一个 `ci/acceptance/slot_1s.yml` 工件进行比较。 |降低 AMX 批量大小或启用额外的 RBC 收集器 (`sumeragi.collectors.k`)，然后重新运行验收工具并捕获新的遥测证据。 |
|缺少可用性峰值 | `/v2/sumeragi/rbc/sessions` 积压字段（`lane_backlog`、`dataspace_backlog`）以及证明者运行状况仪表板。 |移除不健康的证明者，暂时增加`redundant_send_r`以加快交付速度，并在`status.md`中发布修复说明。积压工作清除后，附上更新的 RBC 快照。 |
|收据中频繁出现 `PVO_MISSING_OR_EXPIRED` |证明服务缓存指标 + 发行者的 PVO 调度程序日志。 |重新生成过时的 PVO 工件，缩短旋转节奏，并确保每个 SDK 在 `expiry_slot` 之前刷新手柄。在证据包中包含证明服务指标以证明缓存已恢复。 |
|重复 `AMX_LOCK_CONFLICT` 或 `AMX_TIMEOUT` | `iroha_amx_lock_conflicts_total`、`iroha_amx_prepare_ms` 以及受影响的交易清单。 |重新运行 Norito 静态分析器，更正读/写选择器（或拆分批次），并发布更新的清单固定装置，以便冲突计数器返回到基线。 |
| `SETTLEMENT_ROUTER_UNAVAILABLE` 警报 |结算路由器日志 (`docs/settlement-router.md`)、金库缓冲区仪表板和受影响的收据。 |充值 XOR 缓冲区或将通道翻转为仅 XOR 模式，记录财务操作，并重新运行时隙验收测试以证明结算已恢复。 |

### AXT 拒绝信号

- 原因代码捕获为 `AxtRejectReason`（`lane`、`manifest`、`era`、`sub_nonce`、`expiry`、`missing_policy`、 `policy_denied`、`proof`、`budget`、`replay_cache`、`descriptor`、`duplicate`）。块验证现在显示 `AxtEnvelopeValidationFailed { message, reason, snapshot_version }`，因此事件可以将拒绝固定到特定的策略快照。
- `/v2/debug/axt/cache` 返回 `{ policy_snapshot_version, last_reject, cache, hints }`，其中 `last_reject` 携带最近主机拒绝的通道/原因/版本，`hints` 提供 `next_min_handle_era`/`next_min_sub_nonce` 刷新指导以及缓存的证明状态。
- 警报模板：当 `iroha_axt_policy_reject_total{reason="manifest"}` 或 `{reason="expiry"}` 在 5 分钟窗口内出现峰值时出现页面，将 `last_reject` 快照 + `policy_snapshot_version` 从 Torii 调试端点附加到事件，并使用提示有效负载在重试之前请求刷新句柄。

## 证明验证对象 (PVO)

### 结构

PVO 是 Norito 编码的信封，可让客户提前证明繁重的工作。规范字段是：

|领域|描述 |
|--------|-------------|
| `circuit_id` |证明系统/声明的静态标识符（例如，`amx.transfer.v1`）。 |
| `vk_hash` | DS 清单引用的验证密钥的 Blake2b-256 哈希值。 |
| `proof_digest` |存储在时隙外 PVO 注册表中的序列化证明有效负载的波塞冬摘要。 |
| `max_k` | AIR 域的上限；主机拒绝超过广告大小的证明。 |
| `expiry_slot` |槽位高度，超过该高度后工件无效；将过时的证明排除在车道之外。 |
| `profile` |可选提示（例如 DS 配置文件 A/B/C）可帮助调度程序批量共享配置文件的校样。 |

Norito 架构位于 `crates/iroha_data_model/src/nexus` 中的数据模型定义旁边，因此 SDK 无需 serde 即可派生它。

### 生成管道1. **编译电路元数据** — 从证明者构建中导出 `circuit_id`、验证密钥和最大迹线大小（通常通过 `fastpq_prover` 报告）。
2. **生成证明工件** — 运行时隙外证明器并存储完整的成绩单和承诺。
3. **通过证明服务注册** — 将 Norito PVO 有效负载提交到时隙外验证器（请参阅路线图 NX-17 证明管道）。该服务验证一次，固定摘要，并通过 Torii 公开句柄。
4. **交易中的参考** — 将 PVO 句柄附加到 AMX 构建器（`amx_touch` 或更高级别的 SDK 帮助程序）。主机查找摘要，验证缓存的结果，并且仅在缓存变冷时才在槽内重新计算。
5. **到期时轮换** — SDK 必须在 `expiry_slot` 之前刷新任何缓存的句柄。过期对象会触发 `PVO_MISSING_OR_EXPIRED`。

### 开发人员清单

- 准确声明读/写集，以便 AMX 可以预取锁并避免 `AMX_LOCK_CONFLICT`。
- 当跨 DS 传输触及受监管的 DS 时，将确定性津贴证明捆绑在同一 UAID 清单更新中。
- 重试策略：缺少可用性证据→不采取任何行动（交易保留在内存池中）； `AMX_TIMEOUT` 或 `PVO_MISSING_OR_EXPIRED` → 重建工件并以指数方式回退。
- 测试应包括缓存命中和冷启动（强制主机使用相同的 `max_k` 验证证明）以防止确定性回归。
- 证明 blob (`ProofBlob`) 必须编码 `AxtProofEnvelope { dsid, manifest_root, da_commitment?, proof }`；主机将证明绑定到空间目录清单根，并使用 `iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}` 缓存每个数据空间/槽的通过/失败结果。过期或清单不匹配的工件在提交之前会被拒绝，并在缓存的 `reject` 上的同一插槽短路中进行后续重试。
- 证明缓存重用是槽范围的：经过验证的证明在同一槽内的信封上保持热状态，并在槽前进时自动逐出，因此重试保持确定性。

### 静态读/写分析器

编译时选择器必须匹配合约的实际行为，然后 AMX 才能
预取锁或应用 UAID 清单。新的`ivm::analysis`模块
(`crates/ivm/src/analysis.rs`) 公开 `analyze_program(&[u8])`，它解码
`.to` 人工制品，记录寄存器读/写、内存操作和系统调用使用情况，
并生成 SDK 清单可以嵌入的 JSON 友好的报告。运行它
发布 UAID 时与 `koto_lint` 一起，因此生成的读/写摘要为
在 NX-17 准备情况审查期间引用的证据包中捕获。

## 空间目录策略执行

当主机有权访问空间目录快照时，AXT 句柄验证现在默认为空间目录快照（测试中的 CoreHost，集成流程中的 WsvHost）。每个数据空间策略条目包含 `manifest_root`、`target_lane`、`min_handle_era`、`min_sub_nonce` 和 `current_slot`。主办方强制执行：

- 通道绑定：句柄 `target_lane` 必须与空间目录条目匹配；
- 清单绑定：非零 `manifest_root` 值必须与句柄的 `manifest_view_root` 匹配；
- 过期：`current_slot`大于句柄的`expiry_slot`被拒绝；
- 计数器：`handle_era` 和 `sub_nonce` 必须至少为广告的最小值；
- 成员身份：拒绝快照中不存在的数据空间的句柄。

故障映射到 `PermissionDenied`，`crates/ivm/tests/core_host_policy.rs` 中的 CoreHost 策略快照测试涵盖每个字段的允许/拒绝情况。
块验证还需要每个数据空间的非空证明，其中 `expiry_slot` 覆盖策略槽（具有配置的偏差限额）并且在句柄之前不会过期，强制描述符绑定以及声明规范的触摸清单（并拒绝前缀外的条目），检查句柄意图不变量（非零金额、范围/主题对齐和非零era/sub_nonce/expiry），聚合每个数据空间的句柄预算，以及当信封提交时，`min_handle_era`/`min_sub_nonce` 会提前，因此即使在空间目录重建之后，重播的子随机数也会被拒绝。

## 错误目录

规范代码位于 `crates/iroha_data_model/src/errors.rs` 中。操作员必须在指标/日志中逐字显示它们，并且 SDK 应该将它们映射到可操作的重试。

|代码|触发|操作员回应 | SDK指导|
|------|---------|--------------------|--------------|
|缺少可用性证据（遥测）| 300 毫秒前验证的证明者收据少于 `q`。 |检查证明者的健康状况，扩大下一个槽的采样参数，保持事务排队，并捕获缺少可用性计数器以获取运行手册证据。 |没有行动；重试会自动发生，因为 tx 保持排队状态。 |
| `DA_DEADLINE_EXCEEDED` | Δ 窗口期已过，但未达到 DA 法定人数。 |辞职违规证明者，发布事件记录，强迫客户重新提交。 |证明者返回后重建交易；考虑拆分批次。 |
| `AMX_TIMEOUT` |每个 DS 切片的准备/提交组合时间超过 250 毫秒。 |捕获火焰图、验证 R/W 集并与 `iroha_amx_prepare_ms` 进行比较。 |使用较小的批次或减少争用后重试。 |
| `AMX_LOCK_CONFLICT` |主机检测到重叠的写入集或无信号的触摸。 |检查UAID清单和静态分析报告；如果缺少选择器则更新清单。 |使用更正的读/写声明重新编译事务。 |
| `PVO_MISSING_OR_EXPIRED` |引用的 PVO 句柄不在缓存中或位于 `expiry_slot` 之前。 |检查证明服务积压、重新生成工件并验证 Torii 索引。 |刷新证明工件并使用新句柄重新提交。 |
| `RWSET_UNBOUNDED` |静态分析无法绑定读/写选择器。 |拒绝部署，记录选择器堆栈跟踪，要求开发人员在重试之前修复。 |更新合约以发出显式选择器。 |
| `HEAVY_INSTRUCTION_DISALLOWED` |合约调用了 AMX 通道禁止的指令（例如，没有 PVO 的大型 FFT）。 |确保 Norito 构建器在重新启用之前使用批准的操作码集。 |分割工作负载或添加预先计算的证明。 |
| `SETTLEMENT_ROUTER_UNAVAILABLE` |路由器无法计算确定性转换（路径丢失、缓冲区耗尽）。 |让 Treasury 重新填充缓冲区或翻转仅 XOR 模式；记录在结算操作手册中。 |缓冲区警报清除后重试；显示面向用户的警告。 |

SDK 团队应在集成测试中镜像这些代码，以便 `iroha_cli`、Android、Swift、JS 和 Python 表面就错误文本和建议的操作达成一致。

### AXT 拒绝可观察性

- Torii 将策略失败显示为 `ValidationFail::AxtReject`（并将块验证显示为 `AxtEnvelopeValidationFailed`），具有稳定原因标签、活动 `snapshot_version`、可选 `lane`/`dataspace` 标识符以及提示字段`next_min_handle_era`/`next_min_sub_nonce`。 SDK 应该向用户冒泡这些字段，以便可以确定性地刷新过时的句柄。
- Torii 现在还使用 `X-Iroha-Axt-*` 标头标记 HTTP 响应，以便快速分类：`Code`/`Reason`、`Snapshot-Version`、`Dataspace`、`Lane` 和可选`Next-Handle-Era`/`Next-Sub-Nonce`。 ISO 桥接拒绝带有匹配的 `PRTRY:AXT_*` 原因代码和相同的详细字符串，因此仪表板和操作员可以在 AXT 故障类别中键入警报，而无需解码完整的有效负载。
- 主机使用相同字段记录 `AXT policy rejection recorded` 并通过遥测导出它们：`iroha_axt_policy_reject_total{lane,reason}` 计数拒绝，`iroha_axt_policy_snapshot_version` 跟踪活动快照的哈希值。证明缓存状态仍然可以通过 `/v2/debug/axt/cache`（数据空间/状态/​​清单根/插槽）获得。
- 警报：监视由 `reason` 分组的 `iroha_axt_policy_reject_total` 中的峰值，并从日志/ValidationFail 中使用 `snapshot_version` 进行分页，以确认操作员是否需要轮换清单（通道/清单拒绝）或刷新句柄（era/sub_nonce/expiry）。将警报与证明缓存端点配对，以确认拒绝是与缓存相关还是与策略相关。

## 测试与证据

- CI 必须运行 `ci/acceptance/slot_1s.yml` 套件（30 分钟混合工作负载），并且当不满足插槽/DA/遥测阈值时合并失败，如 `ans3.md` 中所述。
- 混沌演习（证明者抖动、预言机停顿、缓冲区耗尽）必须至少每季度执行一次，且工件存档在 `ops/drill-log.md` 下。
- 状态更新应包括：最新的插槽 SLO 报告、突出的错误峰值以及最新 PVO 缓存快照的链接，以便路线图利益相关者可以审核准备情况。

通过遵循本指南，贡献者可以满足 AMX 文档的路线图要求，并为操作员和开发人员提供计时、遥测和 PVO 工作流程的单一参考。