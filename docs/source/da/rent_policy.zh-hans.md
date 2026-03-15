---
lang: zh-hans
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T14:35:37.691079+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 数据可用性租金和激励政策 (DA-7)

_状态：起草 — 所有者：经济工作组/财务/存储团队_

路线图项目 **DA-7** 为每个 blob 引入了明确的以 XOR 计价的租金
提交至 `/v1/da/ingest`，加上奖励 PDP/PoTR 执行的奖金和
出口用于获取客户端。该文件定义了初始参数，
它们的数据模型表示，以及 Torii 使用的计算工作流程，
SDK 和财务仪表板。

## 政策结构

该策略编码为 [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
在数据模型内。 Torii 和治理工具坚持该策略
Norito 有效负载，以便可以重新计算租金报价和激励分类账
确定性地。该架构公开了五个旋钮：

|领域 |描述 |默认 |
|--------|-------------|---------|
| `base_rate_per_gib_month` | XOR 按保留每月 GiB 收费。 | `250_000` 微异或 (0.25 异或) |
| `protocol_reserve_bps` |转入协议储备金的租金份额（基点）。 | `2_000` (20%) |
| `pdp_bonus_bps` |每次成功的 PDP 评估的奖金百分比。 | `500` (5%) |
| `potr_bonus_bps` |每次成功 PoTR 评估的奖励百分比。 | `250` (2.5%) |
| `egress_credit_per_gib` |当提供商提供 1GiB DA 数据时支付信用。 | `1_500` 微异或 |

所有基点值均根据 `BASIS_POINTS_PER_UNIT` (10000) 进行验证。
策略更新必须经过治理，每个 Torii 节点都会公开
通过 `torii.da_ingest.rent_policy` 配置部分的主动策略
（`iroha_config`）。操作员可以覆盖 `config.toml` 中的默认值：

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI 工具 (`iroha app da rent-quote`) 接受相同的 Norito/JSON 策略输入
并发出镜像活动 `DaRentPolicyV1` 的伪像，但未达到
返回到 Torii 状态。提供用于摄取运行的策略快照，以便
报价仍然是可复制的。

### 持续存在的租金报价文物

运行 `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` 以
发出屏幕上的摘要和打印精美的 JSON 工件。该文件
记录 `policy_source`，内联 `DaRentPolicyV1` 快照，计算的
`DaRentQuote`，以及派生的 `ledger_projection`（通过序列化
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) 使其适用于财务仪表板和分类账 ISI
管道。当 `--quote-out` 指向嵌套目录时，CLI 将创建任何
缺少父母，因此运营商可以标准化位置，例如
`artifacts/da/rent_quotes/<timestamp>.json` 以及其他 DA 证据包。
将工件附加到租金批准或调节运行中，以便异或
细目分类（基本租金、储备金、PDP/PoTR 奖金和出口积分）为
可重现。通过 `--policy-label "<text>"` 自动覆盖
派生 `policy_source` 描述（文件路径、嵌入式默认值等）
人类可读的标签，例如治理票证或清单哈希； CLI 修剪
该值并拒绝空/仅空白字符串，因此记录的证据
仍可审计。

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```账本投影部分直接输入 DA 租金账本 ISI：
定义用于协议储备、提供商支出和的 XOR 增量
每个证明的奖金池，无需定制编排代码。

### 生成租金分类帐计划

运行 `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
将持久的租金报价转换为可执行的分类帐转账。命令
解析嵌入的 `ledger_projection`，发出 Norito `Transfer` 指令
将基本租金收入国库，路由储备金/提供商
部分，并直接从付款人处为 PDP/PoTR 奖金池预先提供资金。的
输出 JSON 镜像报价元数据，以便 CI 和财务工具可以进行推理
关于同一个文物：

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

最后的 `egress_credit_per_gib_micro_xor` 字段允许仪表板和支付
调度程序将出口报销与产生的租金政策相一致
无需在脚本胶水中重新计算策略数学即可引用。

## 引用示例

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

该报价可在 Torii 节点、SDK 和财务报告中重现，因为
它使用确定性 Norito 结构而不是临时数学。运营商可以
将 JSON/CBOR 编码的 `DaRentPolicyV1` 附加到治理提案或租金中
审核以证明哪些参数对于任何给定的 blob 有效。

## 奖金和储备金

- **协议储备：** `protocol_reserve_bps` 为支持的 XOR 储备提供资金
  紧急重新复制和大幅退款。财政部跟踪这个桶
  单独进行，以确保分类帐余额与配置的费率相匹配。
- **PDP/PoTR 奖励：** 每个成功的证明评估都会获得额外的奖励
  支出源自 `base_rent × bonus_bps`。当 DA 调度器发出证明时
  它包含基点标签，以便可以重播激励措施。
- **出口信用：** 提供商记录每个清单提供的 GiB，乘以
  `egress_credit_per_gib`，并通过`iroha app da prove-availability`提交收据。
  租金政策使每 GiB 的金额与治理保持同步。

## 操作流程

1. **摄取：** `/v1/da/ingest`加载活跃的`DaRentPolicyV1`，报价租金
   基于 blob 大小和保留，并将报价嵌入到 Norito 中
   明显。提交者签署一份引用租金哈希的声明，并
   存储票证 ID。
2. **记账：** 金库摄取脚本解码清单，调用
   `DaRentPolicyV1::quote`，并填充租金分类账（基本租金、储备金、
   奖金和预期出口学分）。记录的租金之间存在任何差异
   并且重新计算的报价失败了 CI。
3. **证明奖励：** 当 PDP/PoTR 调度程序标记成功时，他们会发出收据
   包含清单摘要、证明类型以及源自的 XOR 奖励
   政策。治理可以通过重新计算相同的报价来审计支出。
4. **出口报销：** 获取协调器提交签名的出口摘要。
   Torii 将 GiB 计数乘以 `egress_credit_per_gib` 并发出付款
   针对租金托管的指示。

## 遥测Torii 节点通过以下 Prometheus 指标公开租金使用情况（标签：
`cluster`、`storage_class`）：

- `torii_da_rent_gib_months_total` — `/v1/da/ingest` 引用的 GiB 月。
- `torii_da_rent_base_micro_total` — 摄取时应计的基本租金（微异或）。
- `torii_da_protocol_reserve_micro_total` — 协议储备金贡献。
- `torii_da_provider_reward_micro_total` — 提供商方租金支付。
- `torii_da_pdp_bonus_micro_total` 和 `torii_da_potr_bonus_micro_total` —
  PDP/PoTR 奖金池源自摄取报价。

经济仪表板依靠这些计数器来确保账本 ISI、储备水龙头、
和 PDP/PoTR 奖金计划均与每个有效的政策参数相匹配
集群和存储类别。 SoraFS 容量健康 Grafana 板
(`dashboards/grafana/sorafs_capacity_health.json`) 现在渲染专用面板
用于租金分配、PDP/PoTR 奖金累积和 GiB 月捕获，允许
审查摄取时按 Torii 集群或存储类别进行筛选的库
数量和支出。

## 后续步骤

- ✅ `/v1/da/ingest` 收据现在嵌入 `rent_quote` 并且 CLI/SDK 界面显示报价
  基本租金、储备份额和 PDP/PoTR 奖金，以便提交者可以在之前审查 XOR 义务
  提交有效负载。
- 将租金分类账与即将推出的 DA 声誉/订单簿源集成
  证明高可用性提供商正在收到正确的付款。