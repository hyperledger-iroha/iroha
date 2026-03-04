---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2C صلاحیت جمع کرنے کی رپورٹ

تاریخ: 2026-03-21

## دائرہ کار

اس رپورٹ میں ڈٹرمینسٹک SoraFS صلاحیت جمع کرنے کی جانچ اور ادائیگیوں کو بھگوا دیا گیا ہے
روٹ شیٹ SF-2C کے تحت درخواست کی۔

-** 30 دن کے ملٹی فراہم کرنے والے بھگوا: ** چل رہا ہے
  `capacity_fee_ledger_30_day_soak_deterministic` آن
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`۔
  ہارنس نے پانچ فراہم کنندگان کو انسٹینیٹ کیا ، 30 تصفیہ ونڈوز پر پھیلا ہوا ہے ، اور
  توثیق کرتا ہے کہ لیجر کل ایک حوالہ پروجیکشن سے میل کھاتا ہے
  آزادانہ طور پر حساب کیا. ٹیسٹ ایک ڈائجسٹ بلیک 3 جاری کرتا ہے
  (`capacity_soak_digest=...`) تاکہ CI اسنیپ شاٹ کو پکڑ کر موازنہ کرسکے
  کینونیکل
- ** انڈر ڈیلیوری کے لئے جرمانے: ** نافذ کیا گیا
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (ایک ہی فائل) ٹیسٹ اس بات کی تصدیق کرتا ہے کہ ہڑتالوں ، کولڈاونس ، کے لئے دہلیز ،
  کولیٹرل سلیش اور لیجر کاؤنٹر عین مطابق رہتے ہیں۔

## عمل درآمد

مقامی طور پر بھگنے کی توثیق کو چلائیں:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

معیاری لیپ ٹاپ پر ایک سیکنڈ سے بھی کم وقت میں ٹیسٹ مکمل کرتے ہیں اور اس کی ضرورت نہیں ہوتی ہے
بیرونی فکسچر۔

## مشاہدہ

Torii اب فیس لیجرز کے ساتھ فراہم کنندہ کریڈٹ اسنیپ شاٹس کو بے نقاب کرتا ہے تاکہ ڈیش بورڈز
کم بیلنس اور جرمانے کے حملوں پر رینگ سکتے ہیں:

- باقی: `GET /v1/sorafs/capacity/state` اندراجات `credit_ledger[*]` واپس کرتا ہے
  SAAK ٹیسٹ میں تصدیق شدہ لیجر فیلڈز کی عکاسی کریں۔ دیکھو
  `crates/iroha_torii/src/sorafs/registry.rs`۔
- Grafana کی درآمد: `dashboards/grafana/sorafs_capacity_penalties.json` گراف
  برآمد شدہ ہڑتال کے کاؤنٹرز ، جرمانے کے مجموعی اور کولیٹرل گارنٹی میں تاکہ
  آن کال ٹیم براہ راست ماحول کے ساتھ بھگنے والی بیس لائنوں کا موازنہ کرسکتی ہے۔

## ٹریکنگ

- سی آئی اے (دھواں دار درجے) کے ٹیسٹ کو دوبارہ سے نکالنے کے لئے سی آئی میں ہفتہ وار گیٹ پھانسی کا شیڈول بنائیں۔
- جب برآمد ہوتا ہے تو Torii کے کھرچنے والے اہداف کے ساتھ Grafana کے ڈیش بورڈ کو بڑھاؤ
  پروڈکشن ٹیلی میٹری دستیاب ہے۔