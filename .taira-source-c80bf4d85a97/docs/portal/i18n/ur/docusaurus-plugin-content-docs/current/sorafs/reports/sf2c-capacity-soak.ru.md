---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SAAK صلاحیت کے حصول SF-2C پر رپورٹ کریں

تاریخ: 2026-03-21

## ایریا

اس رپورٹ میں صلاحیت کے حصول SoraFS اور ادائیگیوں کے تعصب کے ٹیسٹوں کو حاصل کیا گیا ہے ،
SF-2C روڈ میپ میں درخواست کی۔

-** 30 دن کے ملٹی فراہم کرنے والے بھگوا: ** لانچ
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`۔
  کنٹرول پانچ فراہم کنندگان پیدا کرتا ہے ، 30 تصفیے کی کھڑکیوں کا احاطہ کرتا ہے اور
  چیک جو لیجر کل ایک آزادانہ طور پر حساب کتاب کے حوالہ سے ملتے ہیں
  پروجیکشن ٹیسٹ بلیک 3 ڈائجسٹ (`capacity_soak_digest=...`) سے CI سے نکلتا ہے
  کیننیکل اسنیپ شاٹ کو پکڑ کر موازنہ کرسکتے ہیں۔
- ** انڈر ڈلیوری کے لئے جرمانے: ** فراہم کردہ
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (ایک ہی فائل) ٹیسٹ اس بات کی تصدیق کرتا ہے کہ دہلیز ہڑتالیں ، کوولڈونز ، سلیشز
  خودکش حملہ اور لیجر کاؤنٹر عین مطابق رہتے ہیں۔

## عمل درآمد

مقامی طور پر بھگنا چیک کریں:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

معیاری لیپ ٹاپ پر ایک سیکنڈ سے بھی کم وقت میں ٹیسٹ مکمل کرتے ہیں اور اس کی ضرورت نہیں ہوتی ہے
بیرونی فکسچر۔

## مشاہدہ

Torii اب فیس لیجرز کے ساتھ کریڈٹ فراہم کرنے والوں کے سنیپ شاٹس دکھاتا ہے
ڈیش بورڈز کم بیلنس اور جرمانے کے حملوں پر گیٹ ہوسکتے ہیں:

- باقی: `GET /v1/sorafs/capacity/state` ریکارڈز `credit_ledger[*]` ،
  جو SAAK ٹیسٹ میں آزمائے گئے لیجر فیلڈز کی عکاسی کرتے ہیں۔ دیکھو
  `crates/iroha_torii/src/sorafs/registry.rs`۔
- Grafana درآمد کریں: `dashboards/grafana/sorafs_capacity_penalties.json` بلڈز
  برآمد شدہ ہڑتالوں کا کاؤنٹرز ، ٹھیک مقدار اور خودکش ذخائر ، تاکہ
  ڈیوٹی ٹیم براہ راست ماحول کے ساتھ بیس لائن بھگنے کا موازنہ کرسکتی ہے۔

## اگلے اقدامات

- سوک ٹیسٹ (دھواں درجے) کو دوبارہ پیش کرنے کے لئے سی آئی میں ہفتہ وار گیٹ کا شیڈول چلتا ہے۔
- Telemetry برآمدات کو پروڈ میں لانچ کرنے کے بعد Grafana پینل کو کھرچنا اہداف Torii کے ساتھ بڑھاؤ۔