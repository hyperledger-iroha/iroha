---
lang: zh-hant
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO 線束

Iroha 3 版本系列為關鍵 Nexus 路徑提供顯式 SLO：

- 最終時隙持續時間（NX-18 節奏）
- 證明驗證（提交證書、JDG 證明、橋接證明）
- 證明端點處理（通過驗證延遲的 Axum 路徑代理）
- 費用和質押路徑（付款人/贊助商和債券/斜線流量）

## 預算

預算位於 `benchmarks/i3/slo_budgets.json` 中並直接映射到替補席
I3套件中的場景。目標是每次調用的 p99 目標：

- 費用/質押：每次調用 50 毫秒（`fee_payer`、`fee_sponsor`、`staking_bond`、`staking_slash`）
- 提交證書/JDG/橋驗證：80ms（`commit_cert_verify`、`jdg_attestation_verify`、
  `bridge_proof_verify`)
- 提交證書組裝：80ms (`commit_cert_assembly`)
- 訪問調度程序：50ms (`access_scheduler`)
- 證明端點代理：120ms (`torii_proof_endpoint`)

刻錄率提示 (`burn_rate_fast`/`burn_rate_slow`) 編碼 14.4/6.0
尋呼與工單警報的多窗口比率。

## 安全帶

通過 `cargo xtask i3-slo-harness` 運行線束：

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

輸出：

- `bench_report.json|csv|md` — 原始 I3 基準套件結果（git 哈希 + 場景）
- `slo_report.json|md` — 每個目標的通過/失敗/預算比的 SLO 評估

該線束使用預算文件並強制執行 `benchmarks/i3/slo_thresholds.json`
在替補跑期間，當目標退步時，會快速失敗。

## 遙測和儀表板

- 最終確定：`histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- 證明驗證：`histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana 入門面板位於 `dashboards/grafana/i3_slo.json` 中。 Prometheus
`dashboards/alerts/i3_slo_burn.yml` 中提供了燃燒率警報，其中
上述預算已納入（最終確定 2 秒，證明驗證 80 毫秒，證明端點代理
120 毫秒）。

## 操作注意事項

- 夜間使用安全帶；發布 `artifacts/i3_slo/<stamp>/slo_report.md`
  與治理證據的替補文物一起。
- 如果預算失敗，請使用基準降價來確定場景，然後進行鑽取
  進入匹配的 Grafana 面板/警報以與實時指標相關聯。
- 證明端點 SLO 使用驗證延遲作為代理以避免每條路由
  基數爆炸；基準目標 (120ms) 與保留/DoS 匹配
  證明 API 上的護欄。