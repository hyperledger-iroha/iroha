---
lang: ur
direction: rtl
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Data Availability تھریٹ ماڈل آٹومیشن (DA-1)

<div dir="rtl">

روڈمیپ آئٹم DA-1 اور `status.md` ایک متعین آٹومیشن لوپ کا تقاضا کرتے ہیں جو
Norito PDP/PoTR تھریٹ ماڈل کے خلاصے تیار کرے، جو `docs/source/da/threat_model.md`
اور Docusaurus کے آئینے میں شائع ہوتے ہیں۔ یہ ڈائریکٹری اُن آؤٹ پٹس کو محفوظ
کرتی ہے جن کا حوالہ درج ذیل کمانڈز دیتی ہیں:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (جو `scripts/docs/render_da_threat_model_tables.py` چلاتا ہے)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## فلو

1. **رپورٹ بنائیں**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON خلاصہ replication failure rate کی سمولیشن، chunker تھریش ہولڈز، اور
   `integration_tests/src/da/pdp_potr.rs` میں PDP/PoTR ہارنس کے ذریعے معلوم ہونے
   والی پالیسی خلاف ورزیوں کو ریکارڈ کرتا ہے۔
2. **Markdown ٹیبلز رینڈر کریں**
   ```bash
   make docs-da-threat-model
   ```
   یہ `scripts/docs/render_da_threat_model_tables.py` چلا کر
   `docs/source/da/threat_model.md` اور `docs/portal/docs/da/threat-model.md` کو
   دوبارہ لکھتا ہے۔
3. **آرٹیفیکٹ محفوظ کریں**: JSON رپورٹ (اور اختیاری CLI لاگ) کو کاپی کر کے
   `docs/automation/da/reports/<timestamp>-threat_model_report.json` میں رکھیں۔ جب
   گورننس فیصلے کسی مخصوص رن پر منحصر ہوں تو کمٹ ہیش اور سمیولیٹر seed کو ساتھ
   والے `<timestamp>-metadata.md` میں درج کریں۔

## شواہد کی توقعات

- JSON فائلیں 100 KiB سے کم رکھیں تاکہ git میں رہ سکیں۔ بڑی ٹریسز کو بیرونی
  اسٹوریج میں رکھیں؛ ضرورت پر دستخط شدہ ہیش میٹا ڈیٹا نوٹ میں شامل کریں۔
- ہر محفوظ فائل میں seed، کنفیگ پاتھ اور سمیولیٹر ورژن درج ہونا چاہیے تاکہ
  دوبارہ چلانے پر وہی نتائج مل سکیں۔
- جب بھی DA-1 کے قبولیتی معیار آگے بڑھیں، `status.md` یا روڈمیپ اینٹری سے محفوظ
  فائل کا لنک دیں تاکہ ریویورز ہارنس چلائے بغیر بیس لائن کی تصدیق کر سکیں۔

## کمٹمنٹ ریکنسیلی ایشن (sequencer omission)

`cargo xtask da-commitment-reconcile` استعمال کریں تاکہ DA ingest رسیدوں کو DA
کمٹمنٹ ریکارڈز سے ملا کر sequencer کی ommission یا چھیڑ چھاڑ پکڑی جا سکے:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito یا JSON فارمیٹ کے رسیدیں اور `SignedBlockWire`, `.norito` یا JSON bundles
  سے کمٹمنٹس قبول کرتا ہے۔
- اگر بلاک لاگ میں کوئی ٹکٹ غائب ہو یا hashes مختلف ہوں تو ناکام ہوتا ہے؛
  `--allow-unexpected` اُن ٹکٹس کو نظرانداز کرتا ہے جو صرف بلاک لاگ میں ہوں جب
  آپ جان بوجھ کر رسیدوں کا سیٹ محدود کریں۔
- خارج کردہ JSON کو گورننس/Alertmanager پیکٹس کے ساتھ جوڑیں تاکہ omission alerts
  نکل سکیں؛ ڈیفالٹ `artifacts/da/commitment_reconciliation.json` ہے۔

## پرولیج آڈٹ (سہ ماہی رسائی جائزہ)

`cargo xtask da-privilege-audit` استعمال کریں تاکہ DA manifest/replay ڈائریکٹریز
(اور اختیاری اضافی راستے) میں missing, non-directory یا world-writable انٹریز
کو اسکین کیا جا سکے:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Torii کنفیگ سے DA ingest راستے پڑھتا ہے اور جہاں دستیاب ہو Unix permissions
  چیک کرتا ہے۔
- missing/non-directory/world-writable راستوں کو نشان زد کرتا ہے اور مسئلہ ہو تو
  نان زیرو exit کوڈ واپس کرتا ہے۔
- JSON بنڈل (`artifacts/da/privilege_audit.json` بطور ڈیفالٹ) کو دستخط کر کے
  سہ ماہی رسائی جائزہ کے پیکٹس اور ڈیش بورڈز کے ساتھ منسلک کریں۔

</div>
