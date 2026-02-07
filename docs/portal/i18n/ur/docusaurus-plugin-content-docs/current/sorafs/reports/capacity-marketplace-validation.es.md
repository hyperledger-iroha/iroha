---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: صلاحیت مارکیٹ کی توثیق SoraFS
ٹیگز: [SF-2C ، قبولیت ، چیک لسٹ]
خلاصہ: قبولیت چیک لسٹ فراہم کرنے والے کو بورڈنگ ، تنازعہ کے بہاؤ اور ٹریژری مفاہمت کا احاطہ کرنے والے جو SoraFS صلاحیت مارکیٹ کی عمومی دستیابی کو قابل بناتے ہیں۔
---

# صلاحیت مارکیٹ کی توثیق چیک لسٹ SoraFS

** نظرثانی ونڈو: ** 2026-03-18-> 2026-03-24  
** پروگرام مینیجرز: ** اسٹوریج ٹیم (`@storage-wg`) ، گورننس کونسل (`@council`) ، ٹریژری گلڈ (`@treasury`)  
** دائرہ کار: ** فراہم کنندہ پائپ لائنوں پر سوار ، تنازعہ کے فیصلے کے بہاؤ اور GA SF-2C کے لئے درکار ٹریژری مفاہمت کے عمل۔

تیسری پارٹی کے آپریٹرز کے لئے مارکیٹ کو چالو کرنے سے پہلے مندرجہ ذیل چیک لسٹ کا جائزہ لیا جانا چاہئے۔ ہر صف میں عین مطابق ثبوت (ٹیسٹ ، فکسچر یا دستاویزات) کو جوڑتا ہے جو آڈیٹر دوبارہ پیش کرسکتے ہیں۔

## قبولیت چیک لسٹ

### فراہم کنندہ آن بورڈنگ

| چیک اپ | توثیق | ثبوت |
| ------- | ------------ | ------------ |
| رجسٹری قابلیت کے اعلانیہ اعلامیے کو قبول کرتی ہے انضمام ٹیسٹ میں API ایپ کے ذریعہ `/v1/sorafs/capacity/declare` کی مشق کی جاتی ہے ، دستخطی انتظام ، میٹا ڈیٹا کیپچر اور نوڈ رجسٹری میں ہینڈ آف کی تصدیق ہوتی ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| سمارٹ معاہدہ غلط پے لوڈ کو مسترد کرتا ہے یونٹ ٹیسٹ اس بات کو یقینی بناتا ہے کہ پرعزم فراہم کرنے والے IDs اور GIB فیلڈز برقرار رہنے سے پہلے دستخط شدہ اعلامیہ سے میل کھاتے ہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| سی ایل آئی نے آن بورڈنگ نمونے کو جاری کیا | سی ایل آئی کنٹرول ڈٹرمینسٹک Norito/JSON/BASE64 آؤٹ پٹ لکھتا ہے اور راؤنڈ ٹرپس کی توثیق کرتا ہے تاکہ آپریٹرز آف لائن بیانات تیار کرسکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| آپریٹرز ہدایت نامہ داخلہ کے بہاؤ اور گورننس گارڈز کو اپنی گرفت میں لے لیتا ہے دستاویزات میں اعلامیہ اسکیم ، پالیسی ڈیفالٹس اور کونسل کے لئے جائزہ لینے کے اقدامات کی فہرست دی گئی ہے۔ | `../storage-capacity-marketplace.md` |

### تنازعہ کا حل

| چیک اپ | توثیق | ثبوت |
| ------- | ------------ | ------------ |
| پے لوڈ کیننیکل ڈائجسٹ کے ساتھ تنازعہ کے نوشتہ برقرار ہیں یونٹ ٹیسٹ تنازعہ کو لاگ ان کرتا ہے ، ذخیرہ شدہ پے لوڈ کو ضابطہ کشائی کرتا ہے ، اور لیجر کے تعین کو یقینی بنانے کے لئے زیر التوا ریاست پر زور دیتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| سی ایل آئی تنازعہ جنریٹر کیننیکل اسکیم سے میل کھاتا ہے CLI ٹیسٹ میں `CapacityDisputeV1` کے لئے BASE64/Norito آؤٹ پٹ اور JSON ڈائجسٹ کا احاطہ کیا گیا ہے ، اس بات کو یقینی بناتے ہیں کہ ثبوت کے بنڈل کو عین مطابق کیا جاتا ہے۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| ری پلے ٹیسٹ تنازعہ/جرمانے کا تعین ثابت کرتا ہے | دو بار کھیلے جانے والے پروف-فیلچر ٹیلی میٹری سے لیجر ، کریڈٹ اور تنازعات کے ایک جیسے سنیپ شاٹس پیدا ہوتے ہیں تاکہ ہم عمروں کے مابین سلیشس کا تعی .ن ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| رن بک میں اضافے اور منسوخی کے بہاؤ کو دستاویز کیا گیا ہے آپریشنز گائیڈ کونسل کے بہاؤ ، شواہد کی ضروریات اور رول بیک کے طریقہ کار کو اپنی گرفت میں لے لیتا ہے۔ | `../dispute-revocation-runbook.md` |### خزانہ مفاہمت

| چیک اپ | توثیق | ثبوت |
| ------- | ------------ | ------------ |
| لیجر جمع 30 دن کے بھگوئے پروجیکشن کے ساتھ موافق ہے ایس اے اے سی ٹیسٹ 30 آبادکاری ونڈوز میں پانچ فراہم کنندگان پر پھیلا ہوا ہے ، جس میں لیجر اندراجات کا موازنہ متوقع ادائیگی کے حوالہ سے کیا گیا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| لیجر ایکسپورٹ مفاہمت ہر رات ریکارڈ کی جاتی ہے | `capacity_reconcile.py` فیس لیجر کی توقعات کو پھانسی والے XOR برآمدات ، Prometheus میٹرکس کے ساتھ موازنہ کرتا ہے اور الرٹ مینجر کے ذریعہ ٹریژری کی منظوری کو متحرک کرتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1` ، `docs/source/sorafs/runbooks/capacity_reconciliation.md:1` ، `dashboards/alerts/sorafs_capacity_rules.yml:100` |
| بلنگ ڈیش بورڈز جرمانے اور جمع ٹیلی میٹری کو بے نقاب کرتے ہیں Grafana درآمد گراف گیب گھنٹے جمع ، ہڑتال کاؤنٹرز اور بانڈڈ کولیٹرل آن کال کی نمائش کے لئے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| شائع شدہ رپورٹ میں ایس اے اے سی کے طریقہ کار اور ری پلے کمانڈز کو آرکائیو کیا گیا ہے اس رپورٹ میں آڈیٹرز کے لئے بھگنے ، عمل درآمد کے احکامات اور مشاہدہ کرنے کے ہکس کے دائرہ کار میں بتایا گیا ہے۔ | `./sf2c-capacity-soak.md` |

## نوٹ کریں

سائن آف سے پہلے توثیق سویٹ کو دوبارہ چلائیں:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

آپریٹرز کو `sorafs_manifest_stub capacity {declaration,dispute}` کے ساتھ جہاز پر سوار/تنازعہ کی درخواست پے لوڈ کو دوبارہ تخلیق کرنا چاہئے اور اس کے نتیجے میں JSON/Norito بائٹس کو گورننس کے ٹکٹ کے ساتھ محفوظ کرنا چاہئے۔

## منظوری نمونے

| نمونہ | روٹ | بلیک 2 بی -256 |
| ---------- | ------ | --------------- |
| فراہم کنندہ آن بورڈنگ منظوری پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| تنازعات کے حل کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| ٹریژری مفاہمت کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان نمونے کی دستخط شدہ کاپیاں ریلیز کے بنڈل کے ساتھ محفوظ کریں اور انہیں گورننس چینج لاگ میں جوڑیں۔

## منظوری

-اسٹوریج ٹیم لیڈر- @اسٹوریج-ٹی ایل (2026-03-24)  
-گورننس کونسل کے سکریٹری- @کونسل سیکنڈ (2026-03-24)  
-ٹریژری آپریشن لیڈر- @ٹریژری آپس (2026-03-24)