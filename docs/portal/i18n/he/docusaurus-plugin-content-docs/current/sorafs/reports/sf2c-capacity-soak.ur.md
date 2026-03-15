---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c Capacity Accrual Soak رپورٹ

תאריך: 2026-03-21

## اسکوپ

یہ رپورٹ SF-2c روڈمیپ ٹریک کے تحت مانگے گئے SoraFS capacity accrual اور payout کے deterministic soak tests ریکارڈ کرتی ہے۔

- **30 دن کا multi-provider soak:**
  `capacity_fee_ledger_30_day_soak_deterministic` کے ذریعے
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں چلایا جاتا ہے۔
  harness پانچ providers بناتا ہے، 30 settlement ونڈوز کا احاطہ کرتا ہے، اور
  تصدیق کرتا ہے کہ ledger totals ایک آزادانہ طور پر حساب شدہ reference projection سے ملتے ہیں۔
  یہ ٹیسٹ Blake3 digest (`capacity_soak_digest=...`) خارج کرتا ہے تاکہ CI canonical snapshot کو
  capture اور diff کر سکے۔
- **קנסות בתת-מסירה:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (اسی فائل میں) نافذ کیا جاتا ہے۔ ٹیسٹ تصدیق کرتا ہے کہ strikes thresholds، cooldowns، collateral slashes
  اور ledger counters deterministic رہتے ہیں۔

## ביצוע

לספוג אימותים.

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

2000 מכשירי תקן

## יכולת צפייה

Torii اب provider credit snapshots کو fee ledgers کے ساتھ ظاہر کرتا ہے تاکہ dashboards کم balances اور penalty strikes پر gate کر سکیں:

- REST: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` entries واپس کرتا ہے جو soak test میں verify ہونے والے
  ledger fields کی عکاسی کرتے ہیں۔ دیکھیں
  `crates/iroha_torii/src/sorafs/registry.rs`.
- ייבוא Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` מיוצאים נגדי שביתה, סכומי עונשין,
  اور bonded collateral کو plot کرتا ہے تاکہ on-call ٹیم soak baselines کو live environments سے موازنہ کر سکے۔

## מעקב

- CI میں ہفتہ وار gate runs شیڈول کریں تاکہ soak test دوبارہ چل سکے (smoke-tier).
- جب production telemetry exports live ہوں تو Grafana board میں Torii scrape targets شامل کریں۔