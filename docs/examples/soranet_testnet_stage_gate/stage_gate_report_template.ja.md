---
lang: ja
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

# SNNet-10 ステージゲート レポート (T?_ -> T?_)

> 送信前に placeholder (山括弧内の項目) をすべて置き換えてください。ガバナンス自動化が読み取れるようにセクション見出しは維持してください。

## 1. メタデータ

| 項目 | 値 |
|------|------|
| 昇格 | `<T0->T1 または T1->T2>` |
| 報告期間 | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| 対象 relays | `<count + IDs または "appendix A を参照">` |
| 主要連絡先 | `<name/email/Matrix handle>` |
| 提出アーカイブ | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| アーカイブ SHA-256 | `<sha256:...>` |

## 2. メトリクス概要

| メトリクス | 観測値 | しきい値 | 合格? | ソース |
|-----------|--------|---------|------|--------|
| 回路成功率 | `<0.000>` | >=0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| fetch brownout 比率 | `<0.000>` | <=0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR mix 分散 | `<+0.0%>` | <=+/-10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 秒 | `<0.0 s>` | <=3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| レイテンシ p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ ratio (平均) | `<0.00>` | >= target | ☐ / ☑ | `telemetry/pq_summary.json` |

**Narrative:** `<summaries of anomalies, mitigations, overrides>`

## 3. drill とインシデントのログ

| Timestamp (UTC) | Region | Type | Alert ID | Mitigation summary |
|-----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. 添付ファイルとハッシュ

| Artefact | Path | SHA-256 |
|----------|------|---------|
| Metrics snapshot | `reports/metrics-window.json` | `<sha256>` |
| Metrics report | `reports/metrics-report.json` | `<sha256>` |
| Guard rotation transcripts | `evidence/guard_rotation/*.log` | `<sha256>` |
| Exit bonding manifests | `evidence/exit_bonds/*.to` | `<sha256>` |
| Drill logs | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Rollback plan (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. 承認

| 役割 | 名前 | 署名 (Y/N) | Notes |
|------|------|-----------|-------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Governance rep | `<name>` | ☐ / ☑ | `<comments>` |
| SRE delegate | `<name>` | ☐ / ☑ | `<comments>` |

## Appendix A - Relay roster

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Appendix B - Incident summaries

```
<Detailed context for any incidents or overrides referenced above.>
```
