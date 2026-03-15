---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-07T08:57:10.640650+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: capacity-reconciliation
title: SoraFS کپیسٹی ریکنسیلی ایشن
description: کپیسٹی فیس لیجرز کو XOR ٹرانسفر ایکسپورٹس کے ساتھ میچ کرنے کا راتانہ ورک فلو۔
---

روڈ میپ آئٹم **SF-2c** تقاضا کرتا ہے کہ خزانہ ثابت کرے کہ کپیسٹی فیس لیجر ہر رات انجام دی گئی XOR ٹرانسفرز سے میل کھاتا ہے۔ `scripts/telemetry/capacity_reconcile.py` helper استعمال کریں تاکہ `/v2/sorafs/capacity/state` snapshot کو انجام دی گئی ٹرانسفر بیچ کے ساتھ compare کیا جا سکے اور Alertmanager کے لیے Prometheus textfile metrics جاری ہوں۔

## پیشگی تقاضے
- Torii سے ایکسپورٹ کیا گیا کپیسٹی اسٹیٹ snapshot (`fee_ledger` entries)۔
- اسی ونڈو کے لیے لیجر ایکسپورٹ (JSON یا NDJSON، جس میں `provider_id_hex`,
  `kind` = settlement/penalty، اور `amount_nano` شامل ہو)۔
- اگر آپ الرٹس چاہتے ہیں تو node_exporter textfile collector کا راستہ۔

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Exit codes: `0` صاف میچ پر، `1` جب settlements/penalties غائب ہوں یا زائد ادائیگی ہو، `2` غلط inputs پر۔
- JSON summary + hashes خزانے کے پیکٹ کے ساتھ منسلک کریں:
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- جب `.prom` فائل textfile collector میں آ جائے تو الرٹ
  `SoraFSCapacityReconciliationMismatch` (دیکھیے
  `dashboards/alerts/sorafs_capacity_rules.yml`) تب فائر ہوتا ہے جب missing، overpaid، یا unexpected provider transfers سامنے آئیں۔

## Outputs
- ہر پرووائیڈر کے لیے statuses اور settlements/penalties کے diffs۔
- Totals جو gauges کے طور پر ایکسپورٹ ہوتے ہیں:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## متوقع حدود اور ٹولرنس
- ریکنسیلی ایشن بالکل درست ہے: settlement/penalty کے expected بمقابلہ actual nanos کو صفر ٹولرنس کے ساتھ میچ ہونا چاہیے۔ کوئی بھی non-zero diff آپریٹرز کو پیج کرے گا۔
- CI کپیسٹی فیس لیجر کے لیے 30 دن کا soak digest (ٹیسٹ `capacity_fee_ledger_30_day_soak_deterministic`) کو `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` پر پن کرتا ہے۔ digest صرف تب ریفریش کریں جب pricing یا cooldown semantics بدلیں۔
- soak پروفائل (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) میں penalties صفر رہتی ہیں؛ پروڈکشن میں penalties صرف تب جاری ہونی چاہئیں جب utilisation/uptime/PoR thresholds ٹوٹیں اور successive slashes سے پہلے configured cooldown کو فالو کیا جائے۔
