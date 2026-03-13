---
lang: zh-hans
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 确定性结算路由器 (NX-3)

**状态：** 已完成 (NX-3)  
**所有者：** 经济工作组/核心账本工作组/财务/SRE  
**范围：** 所有通道/数据空间使用的规范 XOR 结算路径。运送的路由器箱、车道级收据、缓冲护栏、遥测和操作员证据表面。

## 目标
- 跨单通道和 Nexus 版本统一 XOR 转换和收据生成。
- 应用确定性折扣+带有护栏缓冲区的波动性边际，以便运营商可以安全地调整结算速度。
- 公开收据、遥测和仪表板，审核员无需定制工具即可重放。

## 架构
|组件|地点 |责任|
|------------|----------|----------------|
|路由器原语| `crates/settlement_router/` |影子价格计算器、理发层级、缓冲政策助手、结算收据类型。【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/src/policy.rs:1】 |
|运行时外观 | `crates/iroha_core/src/settlement/mod.rs:1` |将路由器配置包装到 `SettlementEngine` 中，公开块执行期间使用的 `quote` + 累加器。 |
|区块整合 | `crates/iroha_core/src/block.rs:120` |排出 `PendingSettlement` 记录，聚合每个通道/数据空间的 `LaneSettlementCommitment`，解析通道缓冲区元数据，并发出遥测数据。 |
|遥测和仪表板 | `crates/iroha_telemetry/src/metrics.rs:4847`，`dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP 缓冲区、方差、削减、转换计数指标；用于 SRE 的 Grafana 板。 |
|参考架构| `docs/source/nexus_fee_model.md:1` |单据结算收据字段保留在 `LaneBlockCommitment` 中。 |

## 配置
路由器旋钮位于 `[settlement.router]` 下（由 `iroha_config` 验证）：

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

每个数据空间缓冲区帐户中的通道元数据连线：
- `settlement.buffer_account` — 持有储备金的账户（例如，`buffer::cbdc_treasury`）。
- `settlement.buffer_asset` — 借记净空的资产定义（通常为 `xor#sora`）。
- `settlement.buffer_capacity_micro` — 微 XOR（十进制字符串）中配置的容量。

缺少元数据会禁用该通道的缓冲区快照（遥测回退到零容量/状态）。## 转换管道
1. **报价：** `SettlementEngine::quote` 将配置的 epsilon + 波动率保证金和折扣层应用于 TWAP 报价，返回 `SettlementReceipt` 以及 `xor_due` 和 `xor_after_haircut` 加上时间戳和调用者提供的`source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **累积：** 在块执行期间，执行器记录 `PendingSettlement` 条目（本地金额、TWAP、epsilon、波动性桶、流动性概况、oracle 时间戳）。 `LaneSettlementBuilder` 在密封区块之前按照 `(lane, dataspace)` 聚合总计和交换元数据。【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **缓冲区快照：** 如果通道元数据声明了缓冲区，则构建器使用配置中的 `BufferPolicy` 阈值捕获 `SettlementBufferSnapshot`（剩余空间、容量、状态）。【crates/iroha_core/src/block.rs:203】
4. **提交 + 遥测：** 收据和交换证据落在 `LaneBlockCommitment` 内，并镜像到状态快照中。遥测记录缓冲区规格、方差（`iroha_settlement_pnl_xor`）、应用保证金（`iroha_settlement_haircut_bp`）、可选交换线利用率以及每个资产转换/削减计数器，以便仪表板和警报与块内容保持同步。【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **证据表面：** `status::set_lane_settlement_commitments` 发布了对继电器/DA 消费者的承诺，Grafana 仪表板读取 Prometheus 指标，操作员使用 `ops/runbooks/settlement-buffers.md` 与 `dashboards/grafana/settlement_router_overview.json` 一起跟踪再填充/节流事件。

## 遥测和证据
- `iroha_settlement_buffer_xor`、`iroha_settlement_buffer_capacity_xor`、`iroha_settlement_buffer_status` — 每个通道/数据空间的缓冲区快照（微异或 + 编码状态）。【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` — 实现了块批次到期与理发后异或之间的差异。【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — 应用于批次的有效 epsilon/haircut 基点。【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — 当交换证据存在时，可选利用率按流动性情况划分。【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — 用于结算转换和累积折扣（XOR 单位）的每通道/数据空间计数器。【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha_core/src/block.rs:304】
- Grafana 板：`dashboards/grafana/settlement_router_overview.json`（缓冲区净空、方差、理发）以及 Nexus 通道警报包中嵌入的 Alertmanager 规则。
- 操作员操作手册：`ops/runbooks/settlement-buffers.md`（补充/警报工作流程）和 `docs/source/nexus_settlement_faq.md` 中的常见问题解答。## 开发人员和 SRE 清单
- 在 `config/config.json5`（或 TOML）中设置 `[settlement.router]` 值并通过 `irohad --version` 日志进行验证；确保阈值满足 `alert > throttle > xor_only > halt`。
- 使用缓冲区账户/资产/容量填充车道元数据，以便缓冲区指标反映实时储备；省略不应跟踪缓冲区的通道字段。
- 通过 `dashboards/grafana/settlement_router_overview.json` 监控 `settlement_router_*` 和 `iroha_settlement_*` 指标；对油门/仅异或/停止状态发出警报。
- 运行 `cargo test -p settlement_router` 以进行定价/保单覆盖范围以及 `crates/iroha_core/src/block.rs` 中的现有块级聚合测试。
- 在 `docs/source/nexus_fee_model.md` 中记录配置更改的治理批准，并在阈值或遥测表面发生变化时保持 `status.md` 更新。

## 推出计划快照
- 每个构建中的路由器+遥测船；没有功能门。通道元数据控制缓冲区快照是否发布。
- 默认配置与路线图值匹配（60s TWAP、25bp 基本 epsilon、72h 缓冲区水平）；通过配置调整并重新启动 `irohad` 以应用。
- 证据包 = 通道结算承诺 + `settlement_router_*`/`iroha_settlement_*` 系列的 Prometheus 抓取 + 受影响窗口的 Grafana 屏幕截图/JSON 导出。

## 证据和参考文献
- NX-3结算路由器验收备注：`status.md`（NX-3部分）。
- 操作员表面：`dashboards/grafana/settlement_router_overview.json`、`ops/runbooks/settlement-buffers.md`。
- 收据架构和 API 表面：`docs/source/nexus_fee_model.md`、`/v2/sumeragi/status` -> `lane_settlement_commitments`。