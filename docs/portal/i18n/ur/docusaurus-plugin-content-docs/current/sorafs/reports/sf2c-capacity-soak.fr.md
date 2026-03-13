---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2C صلاحیت جمع کرنے کی رپورٹ

تاریخ: 2026-03-21

## دائرہ کار

اس رپورٹ میں مطلوبہ صلاحیت کی صلاحیت جمع اور ادائیگی کے لئے درخواست کی گئی ہے SoraFS
SF-2C روڈ میپ میں۔

- **Multi-provider soak over 30 days:** Executed by
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`۔
  ہارنس نے پانچ فراہم کنندگان کو انسٹینیٹ کیا ہے ، 30 آباد کاری کی کھڑکیوں کا احاطہ کرتا ہے اور
  توثیق کرتا ہے کہ لیجر کل ایک ریفرنس پروجیکشن کے مطابق ہے
  آزادانہ طور پر حساب کیا. ٹیسٹ ایک بلیک 3 ڈائجسٹ (`capacity_soak_digest=...`) خارج کرتا ہے
  تاکہ CI کیننیکل اسنیپ شاٹ کو پکڑ کر موازنہ کرسکے۔
- ** انڈر ڈیلیوری جرمانے: ** کے ذریعہ لاگو
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (ایک ہی فائل) ٹیسٹ اس بات کی تصدیق کرتا ہے کہ ہڑتالوں ، کولڈاونس ، کے لئے دہلیز ،
  کولیٹرل سلیش اور لیجر کاؤنٹر عین مطابق رہتے ہیں۔

## عمل درآمد

مقامی طور پر بھگنے کی توثیق کو دوبارہ جاری کریں:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

ٹیسٹ ایک معیاری لیپ ٹاپ پر ایک سیکنڈ سے بھی کم وقت میں مکمل ہوجاتے ہیں اور ایسا نہیں کرتے ہیں
بیرونی حقیقت کی ضرورت نہیں ہے۔

## مشاہدہ

Torii اب فیس لیجرز کے ساتھ ساتھ کریڈٹ فراہم کرنے والے سنیپ شاٹس کو بے نقاب کرتا ہے تاکہ ڈیش بورڈز
کم بیلنس اور جرمانے کے حملوں پر گیٹ کر سکتے ہیں:

- باقی: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` اندراجات واپس کرتا ہے
  سوک ٹیسٹ میں چیک کیے گئے لیجر فیلڈز کی عکاسی کریں۔ دیکھو
  `crates/iroha_torii/src/sorafs/registry.rs`۔
- Grafana درآمد کریں: `dashboards/grafana/sorafs_capacity_penalties.json` کا سراغ لگائیں
  برآمد شدہ ہڑتال کے کاؤنٹرز ، جرمانے کے مجموعی اور پرعزم کولیٹرل تاکہ
  آن کال ٹیم براہ راست ماحول کے ساتھ بھگنے والی بیس لائنوں کا موازنہ کرسکتی ہے۔

## ٹریکنگ

- سی اے اے سی ٹیسٹ (دھواں درجے) کو دوبارہ چلانے کے لئے سی آئی میں ہفتہ وار گیٹ پھانسی کا شیڈول بنائیں۔
- ایک بار پروڈکشن ٹیلی میٹری کی برآمدات کے بعد کھرچنے والے اہداف Torii کے ساتھ ٹیبل Grafana میں توسیع کریں
  آن لائن ہوں گے۔