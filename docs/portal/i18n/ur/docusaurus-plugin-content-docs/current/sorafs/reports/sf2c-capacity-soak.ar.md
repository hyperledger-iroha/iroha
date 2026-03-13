---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SF-2C صلاحیت جمع کرنے والے کے لئے #SOAK ٹیسٹ کی رپورٹ

تاریخ: 03-21-2026

## رینج

اس رپورٹ میں SoraFS صلاحیت کے حصول اور SF-2C پلان کے راستے میں مطلوبہ ادائیگیوں کے لئے عین مطابق SAAK ٹیسٹ ریکارڈ کیا گیا ہے۔

- ** ملٹی پروویڈر 30 دن کے لئے ٹیسٹنگ بھگائیں: ** کے ذریعے کیا گیا
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`۔
  کنٹرول پانچ فراہم کرنے والے پیدا کرتا ہے ، 30 تصفیے کی کھڑکیوں پر پھیلا ہوا ہے ، اور تصدیق کرتا ہے کہ مجموعی طور پر
  لیجر آزادانہ طور پر حساب شدہ حوالہ پروجیکشن سے میل کھاتا ہے۔ بلیک 3 سے ڈائجسٹ ٹیسٹ کے نتائج
  (`capacity_soak_digest=...`) تاکہ CI معیاری سنیپ شاٹ لے سکے اور اس کا موازنہ کرسکے۔
- ** انڈر ڈلیوری کے لئے جرمانے: ** کے ذریعہ عائد کیا گیا ہے
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (ایک ہی فائل) ٹیسٹنگ اس بات کی تصدیق کرتی ہے کہ وارنٹی اور کاؤنٹرز کے لئے ہڑتالیں ، کوولڈونز اور سلیشنگ دہلیز
  لیجر تعصب پسند رہتا ہے۔

## نفاذ

مقامی طور پر استعمال کرتے ہوئے بھگوا توثیق کریں:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

معیاری لیپ ٹاپ پر ٹیسٹ ایک سیکنڈ سے بھی کم وقت میں مکمل ہوجاتے ہیں اور انہیں بیرونی فکسچر کی ضرورت نہیں ہوتی ہے۔

## مانیٹرنگ

Torii اب فیس لیجرز کے ساتھ فراہم کنندہ بیلنس سنیپ شاٹس دکھاتا ہے تاکہ ڈیش بورڈز نگرانی کرسکیں
کم بیلنس اور جرمانے کی ہڑتالوں میں ایڈجسٹ کرنے سے:

- باقی: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` کے لئے اندراجات واپس کرتا ہے
  SAAK ٹیسٹ میں تصدیق شدہ لیجر فیلڈز کی عکاسی کرتا ہے۔ دیکھو
  `crates/iroha_torii/src/sorafs/registry.rs`۔
- Grafana درآمد: `dashboards/grafana/sorafs_capacity_penalties.json` ڈرا
  ہڑتالوں کے کاؤنٹرز جاری کیے گئے ، کل جرمانے ، اور خودکش حملہ بندھے تاکہ ایک ٹیم کر سکے
  براہ راست ماحول کے ساتھ بدلاؤ کا موازنہ کرنا۔

## فالو اپ

- سی آئی میں ہفتہ وار گٹ رن کا شیڈول بنائیں تاکہ سوک (تمباکو نوشی) کی جانچ کو دوبارہ چلائیں۔
- ایک بار پروڈکشن ٹیلی میٹری کی برآمدات کو فعال کرنے کے بعد Torii سے کھرچنے والے اہداف کے ساتھ Grafana بورڈ کو وسعت دیں۔