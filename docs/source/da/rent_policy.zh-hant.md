---
lang: zh-hant
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T14:35:37.691079+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 數據可用性租金和激勵政策 (DA-7)

_狀態：起草 — 所有者：經濟工作組/財務/存儲團隊_

路線圖項目 **DA-7** 為每個 blob 引入了明確的以 XOR 計價的租金
提交至 `/v2/da/ingest`，加上獎勵 PDP/PoTR 執行的獎金和
出口用於獲取客戶端。該文件定義了初始參數，
它們的數據模型表示，以及 Torii 使用的計算工作流程，
SDK 和財務儀表板。

## 政策結構

該策略編碼為 [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
在數據模型內。 Torii 和治理工具堅持該策略
Norito 有效負載，以便可以重新計算租金報價和激勵分類賬
確定性地。該架構公開了五個旋鈕：

|領域|描述 |默認|
|--------|-------------|---------|
| `base_rate_per_gib_month` | XOR 按保留每月 GiB 收費。 | `250_000` 微異或 (0.25 異或) |
| `protocol_reserve_bps` |轉入協議儲備金的租金份額（基點）。 | `2_000` (20%) |
| `pdp_bonus_bps` |每次成功的 PDP 評估的獎金百分比。 | `500` (5%) |
| `potr_bonus_bps` |每次成功 PoTR 評估的獎勵百分比。 | `250` (2.5%) |
| `egress_credit_per_gib` |當提供商提供 1GiB DA 數據時支付信用。 | `1_500` 微異或 |

所有基點值均根據 `BASIS_POINTS_PER_UNIT` (10000) 進行驗證。
策略更新必須經過治理，每個 Torii 節點都會公開
通過 `torii.da_ingest.rent_policy` 配置部分的主動策略
（`iroha_config`）。操作員可以覆蓋 `config.toml` 中的默認值：

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI 工具 (`iroha app da rent-quote`) 接受相同的 Norito/JSON 策略輸入
並發出鏡像活動 `DaRentPolicyV1` 的偽像，但未達到
返回到 Torii 狀態。提供用於攝取運行的策略快照，以便
報價仍然是可複制的。

### 持續存在的租金報價文物

運行 `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` 以
發出屏幕上的摘要和打印精美的 JSON 工件。該文件
記錄 `policy_source`，內聯 `DaRentPolicyV1` 快照，計算的
`DaRentQuote`，以及派生的 `ledger_projection`（通過序列化
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) 使其適用於財務儀表板和分類賬 ISI
管道。當 `--quote-out` 指向嵌套目錄時，CLI 將創建任何
缺少父母，因此運營商可以標準化位置，例如
`artifacts/da/rent_quotes/<timestamp>.json` 以及其他 DA 證據包。
將工件附加到租金批准或調節運行中，以便異或
細目分類（基本租金、儲備金、PDP/PoTR 獎金和出口積分）為
可重現。通過 `--policy-label "<text>"` 自動覆蓋
派生 `policy_source` 描述（文件路徑、嵌入式默認值等）
人類可讀的標籤，例如治理票證或清單哈希； CLI 修剪
該值並拒絕空/僅空白字符串，因此記錄的證據
仍可審計。

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
```賬本投影部分直接輸入 DA 租金賬本 ISI：
定義用於協議儲備、提供商支出和的 XOR 增量
每個證明的獎金池，無需定制編排代碼。

### 生成租金分類帳計劃

運行 `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
將持久的租金報價轉換為可執行的分類帳轉賬。命令
解析嵌入的 `ledger_projection`，發出 Norito `Transfer` 指令
將基本租金收入國庫，路由儲備金/提供商
部分，並直接從付款人處為 PDP/PoTR 獎金池預先提供資金。的
輸出 JSON 鏡像報價元數據，以便 CI 和財務工具可以進行推理
關於同一個文物：

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

最後的 `egress_credit_per_gib_micro_xor` 字段允許儀表板和支付
調度程序將出口報銷與產生的租金政策相一致
無需在腳本膠水中重新計算策略數學即可引用。

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

該報價可在 Torii 節點、SDK 和財務報告中重現，因為
它使用確定性 Norito 結構而不是臨時數學。運營商可以
將 JSON/CBOR 編碼的 `DaRentPolicyV1` 附加到治理提案或租金中
審核以證明哪些參數對於任何給定的 blob 有效。

## 獎金和儲備金

- **協議儲備：** `protocol_reserve_bps` 為支持的 XOR 儲備提供資金
  緊急重新復制和大幅退款。財政部跟踪這個桶
  單獨進行，以確保分類帳餘額與配置的費率相匹配。
- **PDP/PoTR 獎勵：** 每個成功的證明評估都會獲得額外的獎勵
  支出源自 `base_rent × bonus_bps`。當 DA 調度器發出證明時
  它包含基點標籤，以便可以重播激勵措施。
- **出口信用：** 提供商記錄每個清單提供的 GiB，乘以
  `egress_credit_per_gib`，並通過`iroha app da prove-availability`提交收據。
  租金政策使每 GiB 的金額與治理保持同步。

## 操作流程

1. **攝取：** `/v2/da/ingest`加載活躍的`DaRentPolicyV1`，報價租金
   基於 blob 大小和保留，並將報價嵌入到 Norito 中
   明顯。提交者簽署一份引用租金哈希的聲明，並
   存儲票證 ID。
2. **記賬：** 金庫攝取腳本解碼清單，調用
   `DaRentPolicyV1::quote`，並填充租金分類賬（基本租金、儲備金、
   獎金和預期出口學分）。記錄的租金之間存在任何差異
   並且重新計算的報價失敗了 CI。
3. **證明獎勵：** 當 PDP/PoTR 調度程序標記成功時，他們會發出收據
   包含清單摘要、證明類型以及源自的 XOR 獎勵
   政策。治理可以通過重新計算相同的報價來審計支出。
4. **出口報銷：** 獲取協調器提交簽名的出口摘要。
   Torii 將 GiB 計數乘以 `egress_credit_per_gib` 並發出付款
   針對租金託管的指示。

## 遙測Torii 節點通過以下 Prometheus 指標公開租金使用情況（標籤：
`cluster`、`storage_class`）：

- `torii_da_rent_gib_months_total` — `/v2/da/ingest` 引用的 GiB 月。
- `torii_da_rent_base_micro_total` — 攝取時應計的基本租金（微異或）。
- `torii_da_protocol_reserve_micro_total` — 協議儲備金貢獻。
- `torii_da_provider_reward_micro_total` — 提供商方租金支付。
- `torii_da_pdp_bonus_micro_total` 和 `torii_da_potr_bonus_micro_total` —
  PDP/PoTR 獎金池源自攝取報價。

經濟儀表板依靠這些計數器來確保賬本 ISI、儲備水龍頭、
和 PDP/PoTR 獎金計劃均與每個有效的政策參數相匹配
集群和存儲類別。 SoraFS 容量健康 Grafana 板
(`dashboards/grafana/sorafs_capacity_health.json`) 現在渲染專用面板
用於租金分配、PDP/PoTR 獎金累積和 GiB 月捕獲，允許
審查攝取時按 Torii 集群或存儲類別進行篩選的庫
數量和支出。

## 後續步驟

- ✅ `/v2/da/ingest` 收據現在嵌入 `rent_quote` 並且 CLI/SDK 界面顯示報價
  基本租金、儲備份額和 PDP/PoTR 獎金，以便提交者可以在之前審查 XOR 義務
  提交有效負載。
- 將租金分類賬與即將推出的 DA 聲譽/訂單簿源集成
  證明高可用性提供商正在收到正確的付款。