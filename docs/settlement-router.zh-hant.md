---
lang: zh-hant
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 確定性結算路由器 (NX-3)

**狀態：** 已完成 (NX-3)  
**所有者：** 經濟工作組/核心賬本工作組/財務/SRE  
**範圍：** 所有通道/數據空間使用的規範 XOR 結算路徑。運送的路由器箱、車道級收據、緩衝護欄、遙測和操作員證據表面。

## 目標
- 跨單通道和 Nexus 版本統一 XOR 轉換和收據生成。
- 應用確定性折扣+帶有護欄緩衝區的波動性邊際，以便運營商可以安全地調整結算速度。
- 公開收據、遙測和儀表板，審核員無需定制工具即可重放。

## 架構
|組件|地點 |責任|
|------------|----------|----------------|
|路由器原語| `crates/settlement_router/` |影子價格計算器、理髮層級、緩衝政策助手、結算收據類型。 【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/src/policy.rs:1】 |
|運行時外觀 | `crates/iroha_core/src/settlement/mod.rs:1` |將路由器配置包裝到 `SettlementEngine` 中，公開塊執行期間使用的 `quote` + 累加器。 |
|區塊整合 | `crates/iroha_core/src/block.rs:120` |排出 `PendingSettlement` 記錄，聚合每個通道/數據空間的 `LaneSettlementCommitment`，解析通道緩衝區元數據，並發出遙測數據。 |
|遙測和儀表板 | `crates/iroha_telemetry/src/metrics.rs:4847`，`dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP 緩衝區、方差、削減、轉換計數指標；用於 SRE 的 Grafana 板。 |
|參考架構| `docs/source/nexus_fee_model.md:1` |單據結算收據字段保留在 `LaneBlockCommitment` 中。 |

## 配置
路由器旋鈕位於 `[settlement.router]` 下（由 `iroha_config` 驗證）：

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

每個數據空間緩衝區帳戶中的通道元數據連線：
- `settlement.buffer_account` — 持有儲備金的賬戶（例如，`buffer::cbdc_treasury`）。
- `settlement.buffer_asset` — 借記淨空的資產定義（通常為 `xor#sora`）。
- `settlement.buffer_capacity_micro` — 微 XOR（十進製字符串）中配置的容量。

缺少元數據會禁用該通道的緩衝區快照（遙測回退到零容量/狀態）。## 轉換管道
1. **報價：** `SettlementEngine::quote` 將配置的 epsilon + 波動率保證金和折扣層應用於 TWAP 報價，返回 `SettlementReceipt` 以及 `xor_due` 和 `xor_after_haircut` 加上時間戳和調用者提供的`source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **累積：** 在塊執行期間，執行器記錄 `PendingSettlement` 條目（本地金額、TWAP、epsilon、波動性桶、流動性概況、oracle 時間戳）。 `LaneSettlementBuilder` 在密封區塊之前按照 `(lane, dataspace)` 聚合總計和交換元數據。 【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **緩衝區快照：** 如果通道元數據聲明了緩衝區，則構建器使用配置中的 `BufferPolicy` 閾值捕獲 `SettlementBufferSnapshot`（剩餘空間、容量、狀態）。 【crates/iroha_core/src/block.rs:203】
4. **提交 + 遙測：** 收據和交換證據落在 `LaneBlockCommitment` 內，並鏡像到狀態快照中。遙測記錄緩衝區規格、方差（`iroha_settlement_pnl_xor`）、應用保證金（`iroha_settlement_haircut_bp`）、可選交換線利用率以及每個資產轉換/削減計數器，以便儀表板和警報與塊內容保持同步。 【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **證據表面：** `status::set_lane_settlement_commitments` 發布了對繼電器/DA 消費者的承諾，Grafana 儀表板讀取 Prometheus 指標，操作員使用 `ops/runbooks/settlement-buffers.md` 與 `dashboards/grafana/settlement_router_overview.json` 一起跟踪再填充/節流事件。

## 遙測和證據
- `iroha_settlement_buffer_xor`、`iroha_settlement_buffer_capacity_xor`、`iroha_settlement_buffer_status` — 每個通道/數據空間的緩衝區快照（微異或 + 編碼狀態）。 【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` — 實現了塊批次到期與理髮後異或之間的差異。 【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — 應用於批次的有效 epsilon/haircut 基點。 【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — 當交換證據存在時，可選利用率按流動性情況劃分。 【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — 用於結算轉換和累積折扣（XOR 單位）的每通道/數據空間計數器。 【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha_core/src/block.rs:304】
- Grafana 板：`dashboards/grafana/settlement_router_overview.json`（緩衝區淨空、方差、理髮）以及 Nexus 通道警報包中嵌入的 Alertmanager 規則。
- 操作員操作手冊：`ops/runbooks/settlement-buffers.md`（補充/警報工作流程）和 `docs/source/nexus_settlement_faq.md` 中的常見問題解答。## 開發人員和 SRE 清單
- 在 `config/config.json5`（或 TOML）中設置 `[settlement.router]` 值並通過 `irohad --version` 日誌進行驗證；確保閾值滿足 `alert > throttle > xor_only > halt`。
- 使用緩衝區賬戶/資產/容量填充車道元數據，以便緩衝區指標反映實時儲備；省略不應跟踪緩衝區的通道字段。
- 通過 `dashboards/grafana/settlement_router_overview.json` 監控 `settlement_router_*` 和 `iroha_settlement_*` 指標；對油門/僅異或/停止狀態發出警報。
- 運行 `cargo test -p settlement_router` 以進行定價/保單覆蓋範圍以及 `crates/iroha_core/src/block.rs` 中的現有塊級聚合測試。
- 在 `docs/source/nexus_fee_model.md` 中記錄配置更改的治理批准，並在閾值或遙測表面發生變化時保持 `status.md` 更新。

## 推出計劃快照
- 每個構建中的路由器+遙測船；沒有功能門。通道元數據控制緩衝區快照是否發布。
- 默認配置與路線圖值匹配（60s TWAP、25bp 基本 epsilon、72h 緩衝區水平）；通過配置調整併重新啟動 `irohad` 以應用。
- 證據包 = 通道結算承諾 + `settlement_router_*`/`iroha_settlement_*` 系列的 Prometheus 抓取 + 受影響窗口的 Grafana 屏幕截圖/JSON 導出。

## 證據和參考文獻
- NX-3結算路由器驗收備註：`status.md`（NX-3部分）。
- 操作員表面：`dashboards/grafana/settlement_router_overview.json`、`ops/runbooks/settlement-buffers.md`。
- 收據架構和 API 表面：`docs/source/nexus_fee_model.md`、`/v2/sumeragi/status` -> `lane_settlement_commitments`。