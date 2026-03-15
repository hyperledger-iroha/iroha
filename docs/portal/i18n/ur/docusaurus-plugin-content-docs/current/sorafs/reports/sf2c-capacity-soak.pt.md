---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2C صلاحیت جمع کرنے کی رپورٹ

تاریخ: 2026-03-21

## دائرہ کار

اس رپورٹ میں تعی .ن جمع جمع کرنا اور صلاحیت کی ادائیگی کے ٹیسٹ SoraFS کو ریکارڈ کیا گیا ہے
روڈ میپ کے SF-2C ٹریک میں درخواست کی۔

-** 30 دن کے ملٹی فراہم کرنے والے بھگوا: ** چل رہا ہے
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`۔
  ہارنس نے پانچ فراہم کنندگان کو انسٹینیٹ کیا ہے ، 30 آباد کاری کی کھڑکیوں کا احاطہ کرتا ہے اور
  توثیق کرتا ہے کہ لیجر کل ایک ریفرنس پروجیکشن کے مطابق ہے
  آزادانہ طور پر حساب کیا. ٹیسٹ ایک بلیک 3 ڈائجسٹ جاری کرتا ہے
  (`capacity_soak_digest=...`) لہذا CI اسنیپ شاٹ کو پکڑ کر موازنہ کرسکتا ہے
  کینونیکل
- ** انڈر ڈیلیوری کے لئے جرمانے: ** کے ذریعہ لاگو
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (ایک ہی فائل) ٹیسٹ اس بات کی تصدیق کرتا ہے کہ ہڑتالوں ، کوولڈونز ، سلیشس کے لئے دہلیز ہے
  خودکش حملہ اور لیجر کاؤنٹر عین مطابق رہتے ہیں۔

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
کم بیلنس اور جرمانے کے حملوں پر گیٹ کر سکتے ہیں:

- باقی: `GET /v1/sorafs/capacity/state` `credit_ledger[*]` اندراجات واپس کرتا ہے
  SAAK ٹیسٹ میں تصدیق شدہ لیجر فیلڈز کی عکاسی کریں۔ دیکھو
  `crates/iroha_torii/src/sorafs/registry.rs`۔
- Grafana درآمد کریں: `dashboards/grafana/sorafs_capacity_penalties.json` پلاٹ
  برآمد شدہ ہڑتالوں ، کل جرمانے اور پھنسے ہوئے کولیٹرل کے کاؤنٹر
  آن کال ٹیم پیداواری ماحول کے ساتھ لینا بیس لائنوں کا موازنہ کرسکتی ہے۔

## فالو اپ

- سی آئی اے کے ٹیسٹ (دھواں دار درجے) کو دوبارہ شروع کرنے کے لئے سی آئی میں ہفتہ وار گیٹ پھانسی کا شیڈول بنائیں۔
- ایک بار پروڈکشن ٹیلی میٹری برآمدات کے براہ راست ہونے کے بعد Grafana پینل کو Torii سکریپ اہداف کے ساتھ بڑھائیں۔