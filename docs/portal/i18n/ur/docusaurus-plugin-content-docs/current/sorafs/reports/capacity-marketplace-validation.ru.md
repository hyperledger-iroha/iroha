---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: صلاحیت SoraFS کی مارکیٹ پلیس کی توثیق
ٹیگز: [SF-2C ، قبولیت ، چیک لسٹ]
خلاصہ: قبولیت چیک لسٹ فراہم کرنے والے کو بورڈنگ ، تنازعہ کے بہاؤ ، اور ٹریژری مفاہمت کا احاطہ کرنے والا جو GA کے لئے SoraFS صلاحیت مارکیٹ کی تیاری کو بند کرتا ہے۔
---

# صلاحیت مارکیٹ کی توثیق کے لئے چیک لسٹ SoraFS

** ونڈو چیک کریں: ** 2026-03-18-> 2026-03-24  
** پروگرام مالکان: ** اسٹوریج ٹیم (`@storage-wg`) ، گورننس کونسل (`@council`) ، ٹریژری گلڈ (`@treasury`)  
** دائرہ کار: ** فراہم کنندہ پائپ لائنز ، تنازعات کے حل کے بہاؤ ، اور GA SF-2C کے لئے درکار ٹریژری مفاہمت کے عمل پر فراہم کنندہ۔

بیرونی آپریٹرز کے لئے بازار کو چالو کرنے سے پہلے نیچے دی گئی چیک لسٹ کی جانچ پڑتال کرنی ہوگی۔ ہر لائن سے مراد عزم ثبوت (ٹیسٹ ، فکسچر ، یا دستاویزات) ہیں جو آڈیٹر دوبارہ پیدا کرسکتے ہیں۔

## قبولیت چیک لسٹ

### آن بورڈنگ فراہم کرنے والے

| چیک | توثیق | ثبوت |
| ------- | ----------- | ------------ |
| رجسٹری کیننیکل صلاحیت کے اعلانات کو قبول کرتی ہے انضمام ٹیسٹ `/v1/sorafs/capacity/declare` کو ایپ API کے ذریعے چلتا ہے ، دستخطی پروسیسنگ ، میٹا ڈیٹا کیپچر اور نوڈ کی رجسٹری میں ٹرانسمیشن کی جانچ پڑتال کرتا ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| سمارٹ معاہدہ مماثل پے لوڈ کو مسترد کرتا ہے یونٹ ٹیسٹ اس بات کو یقینی بناتا ہے کہ فراہم کنندہ کی شناخت اور جی آئی بی کے پرعزم فیلڈز بچت سے پہلے دستخط شدہ اعلامیہ سے مماثل ہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| سی ایل آئی نے کیننیکل آن بورڈنگ نوادرات کو جاری کیا | سی ایل آئی کنٹرول ڈٹرمینسٹک Norito/JSON/BASE64 آؤٹ پٹ لکھتا ہے اور راؤنڈ ٹرپس کی توثیق کرتا ہے تاکہ آپریٹرز آف لائن اعلامیہ تیار کرسکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| آپریٹر کے دستی میں استقبالیہ ورک فلو اور گارڈ ریلز مینجمنٹ کی وضاحت کی گئی ہے دستاویزات میں اعلامیہ اسکیم ، ڈیفالٹس پالیسی اور کونسل کے لئے جائزہ لینے کے اقدامات کی فہرست دی گئی ہے۔ | `../storage-capacity-marketplace.md` |

### تنازعہ کا حل

| چیک | توثیق | ثبوت |
| ------- | ----------- | ------------ |
| تنازعہ کے ریکارڈ کو کیننیکل ڈائجسٹ پے لوڈ کے ساتھ محفوظ کیا جاتا ہے یونٹ ٹیسٹ تنازعہ کو رجسٹر کرتا ہے ، محفوظ شدہ پے لوڈ کو ضابطہ کشائی کرتا ہے ، اور لیجر کے تعین کو یقینی بنانے کے لئے زیر التواء کی حیثیت کی تصدیق کرتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| سی ایل آئی تنازعہ جنریٹر کیننیکل پیٹرن کی پیروی کرتا ہے CLI ٹیسٹ میں `CapacityDisputeV1` کے لئے BASE64/Norito آؤٹ پٹ اور JSON خلاصے کا احاطہ کیا گیا ہے ، جو ہیش ثبوت کے بنڈل کو یقینی بناتے ہیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| ری پلے ٹیسٹ تنازعات/جرمانے کے عزم کو ثابت کرتا ہے | ٹیلی میٹری پروف فیلچر ، جو دو بار کھیلا جاتا ہے ، ایک جیسی سنیپ شاٹس لیجر ، کریڈٹ اور تنازعہ دیتا ہے ، تاکہ ہم عمروں کے مابین سلیشٹسٹک ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| رن بک میں اضافے اور منسوخی کے بہاؤ کی دستاویزات | آپریشنل دستی ورک فلو کونسل ، شواہد کی ضروریات اور رول بیک کے طریقہ کار کا تعین کرتا ہے۔ | `../dispute-revocation-runbook.md` |

### خزانہ مفاہمت| چیک | توثیق | ثبوت |
| ------- | ----------- | ------------ |
| جمع کرنے والا لیجر 30 دن کے بھگنے والے پروجیکشن کے ساتھ موافق ہے ایس اے او سی ٹیسٹ میں 30 سے ​​زیادہ آبادکاری ونڈوز کا احاطہ کیا گیا ہے ، جس میں متوقع حوالہ ادائیگی کے ساتھ لیجر اندراجات کا موازنہ کیا گیا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| لیجر ایکسپورٹ مفاہمت ہر رات ریکارڈ کی گئی | `capacity_reconcile.py` فیس لیجر کی توقعات کا موازنہ مکمل XOR برآمدات کے ساتھ کرتا ہے ، Prometheus میٹرکس اور گیٹس ٹریژری منظوری کو الرٹ مینجر کے ذریعہ شائع کرتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1` ، `docs/source/sorafs/runbooks/capacity_reconciliation.md:1` ، `dashboards/alerts/sorafs_capacity_rules.yml:100` |
| بلنگ ڈیش بورڈز جرمانے اور ٹیلی میٹری کے حصول کو ظاہر کرتے ہیں Grafana کو امپورٹ کریں گب گھنٹہ ، ہڑتالیں اور بانڈڈ کولیٹرل کاؤنٹرز آن کال کی نمائش کے لئے بناتا ہے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| شائع شدہ رپورٹ میں ایس اے اے سی کے طریقہ کار اور ری پلے کمانڈز کو آرکائیو کیا گیا ہے اس رپورٹ میں آڈیٹرز کے لئے بھگنے ، عمل درآمد کے احکامات اور مشاہدہ ہکس کے دائرہ کار کی وضاحت کی گئی ہے۔ | `./sf2c-capacity-soak.md` |

## نوٹ چل رہا ہے

سائن آف سے پہلے چیک سیٹ دوبارہ کریں:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

آپریٹرز کو لازمی طور پر `sorafs_manifest_stub capacity {declaration,dispute}` کے ذریعے جہاز پر چلنے/تنازعات کی درخواستوں کے پے لوڈ کو دوبارہ بنانا ہوگا اور اس کے نتیجے میں JSON/Norito بائٹس کو گورننس کے ٹکٹ کے ساتھ محفوظ کریں۔

## نمونے سائن آف

| نمونہ | راستہ | بلیک 2 بی -256 |
| ---------- | ------ | -------------- |
| فراہم کرنے والے پر بورڈنگ منظوری پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| تنازعات کے حل کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| ٹریژری مفاہمت کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان نمونے کی دستخط شدہ کاپیاں ریلیز کے بنڈل کے ساتھ رکھیں اور کنٹرول چینج رجسٹری میں ان کا حوالہ دیں۔

## دستخط

-اسٹوریج ٹیم لیڈ- @اسٹوریج-ٹی ایل (2026-03-24)  
-گورننس کونسل کے سکریٹری- @کونسل سیکنڈ (2026-03-24)  
-ٹریژری آپریشنز لیڈ- @ٹریژری آپس (2026-03-24)