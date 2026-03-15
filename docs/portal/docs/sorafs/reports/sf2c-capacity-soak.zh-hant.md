---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-12-29T18:16:35.201180+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SF-2c 容量累積浸泡報告

日期：2026-03-21

## 範圍

該報告記錄了確定性 SoraFS 容量累積和支付浸泡
SF-2c 路線圖軌道下要求的測試。

- **30 天多提供商浸泡：** 執行者
  `capacity_fee_ledger_30_day_soak_deterministic` 中
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。
  該工具實例化了 5 個提供商，跨越 30 個結算窗口，並且
  驗證分類總數是否與獨立計算的參考相匹配
  投影。該測試發出 Blake3 摘要 (`capacity_soak_digest=...`)，因此
  CI 可以捕獲並區分規範快照。
- **交付不足的處罰：** 執行者
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  （同一文件）。該測試確認了打擊閾值、冷卻時間、附帶削減、
  賬本計數器仍然是確定性的。

## 執行

使用以下命令在本地運行浸泡驗證：

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

測試在標準筆記本電腦上在一秒內完成，並且不需要
外部固定裝置。

## 可觀察性

Torii 現在將提供商信用快照與費用分類賬一起公開，以便儀表板
可以控制低餘額和罰球：

- REST：`GET /v2/sorafs/capacity/state` 返回 `credit_ledger[*]` 條目
  鏡像浸泡測試中驗證的賬本字段。參見
  `crates/iroha_torii/src/sorafs/registry.rs`。
- Grafana 導入：`dashboards/grafana/sorafs_capacity_penalties.json` 繪製
  出口罷工計數器、罰款總額和保稅抵押品等
  工作人員可以將浸泡基線與現場環境進行比較。

## 後續行動

- 在 CI 中安排每週門運行以重播浸泡測試（煙霧層）。
- 一旦生產遙測，使用 Torii 刮擦目標擴展 Grafana 板
  出口上線。