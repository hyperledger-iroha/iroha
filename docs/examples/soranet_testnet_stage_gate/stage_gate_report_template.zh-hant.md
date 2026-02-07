---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 階段門報告 (T?_→T?_)

> 提交前替換每個佔位符（尖括號中的項目）。保留
> 節標題，以便治理自動化可以解析文件。

## 1. 元數據

|領域|價值|
|--------|--------|
|促銷| `<T0→T1 or T1→T2>` |
|舉報窗口| `<YYYY-MM-DD → YYYY-MM-DD>` |
|範圍內的繼電器 | `<count + IDs or “see appendix A”>` |
|主要聯繫人 | `<name/email/Matrix handle>` |
|提交存檔 | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
|存檔 SHA-256 | `<sha256:...>` |

## 2. 指標總結

|公制|觀察|門檻|經過？ |來源 |
|--------|----------|------------|--------|--------|
|電路成功率| `<0.000>` | ≥0.95 | ○ / ☑ | `reports/metrics-report.json` |
|獲取掉電率| `<0.000>` | ≤0.01 | ○ / ☑ | `reports/metrics-report.json` |
| GAR 混合方差 | `<+0.0%>` | ≤±10% | ○ / ☑ | `reports/metrics-report.json` |
| PoW p95 秒 | `<0.0 s>` | ≤3秒| ○ / ☑ | `telemetry/pow_window.json` |
|延遲 p95 | `<0 ms>` | <200 毫秒 | ○ / ☑ | `telemetry/latency_window.json` |
| PQ 比率（平均）| `<0.00>` | ≥目標| ○ / ☑ | `telemetry/pq_summary.json` |

**敘述：** `<summaries of anomalies, mitigations, overrides>`

## 3. 演習和事件日誌

|時間戳 (UTC) |地區 |類型 |警報 ID |緩解總結|
|----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. 附件和哈希值

|文物|路徑| SHA-256 |
|----------|------|---------|
|指標快照 | `reports/metrics-window.json` | `<sha256>` |
|指標報告| `reports/metrics-report.json` | `<sha256>` |
|後衛輪換成績單| `evidence/guard_rotation/*.log` | `<sha256>` |
|退出綁定清單 | `evidence/exit_bonds/*.to` | `<sha256>` |
|鑽井日誌| `evidence/drills/*.md` | `<sha256>` |
|面具準備（T1→T2）| `reports/masque-readiness.md` | `<sha256 or n/a>` |
|回滾計劃（T1→T2）| `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. 批准

|角色 |名稱 |簽名（是/否）|筆記|
|------|------|--------------|--------|
|網絡 TL | `<name>` | ○ / ☑ | `<comments>` |
|治理代表| `<name>` | ○ / ☑ | `<comments>` |
| SRE 代表 | `<name>` | ○ / ☑ | `<comments>` |

## 附錄 A — 接力名單

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## 附錄 B — 事件摘要

```
<Detailed context for any incidents or overrides referenced above.>
```