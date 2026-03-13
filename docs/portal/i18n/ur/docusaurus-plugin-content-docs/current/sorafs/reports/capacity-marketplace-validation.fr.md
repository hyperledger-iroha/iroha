---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: صلاحیت مارکیٹ SoraFS کی توثیق
ٹیگز: [SF-2C ، قبولیت ، چیک لسٹ]
خلاصہ: قبولیت چیک لسٹ فراہم کرنے والے کو بورڈنگ ، تنازعہ کے بہاؤ اور ٹریژری مفاہمت کی کنڈیشنگ کنڈیشنگ کی صلاحیت مارکیٹ SoraFS کی عمومی دستیابی۔
---

# صلاحیت مارکیٹ کی توثیق چیک لسٹ SoraFS

** جائزہ ونڈو: ** 2026-03-18-> 2026-03-24  
** پروگرام لیڈز: ** اسٹوریج ٹیم (`@storage-wg`) ، گورننس کونسل (`@council`) ، ٹریژری گلڈ (`@treasury`)  
** دائرہ کار: ** فراہم کنندہ پائپ لائنوں پر سوار ، تنازعہ فیصلہ کن ورک فلوز اور ٹریژری مفاہمت کے عمل GA SF-2C کے لئے درکار ہے۔

بیرونی آپریٹرز کے لئے مارکیٹ کو چالو کرنے سے پہلے نیچے دی گئی چیک لسٹ کا جائزہ لیا جانا چاہئے۔ ہر لائن سے مراد عین مطابق ثبوت (ٹیسٹ ، فکسچر یا دستاویزات) ہیں جو سامعین دوبارہ چل سکتے ہیں۔

## قبولیت چیک لسٹ

### فراہم کنندگان کی بورڈنگ

| چیک | توثیق | ثبوت |
| ------- | ------------ | --------- |
| رجسٹری قابلیت کے اعلانیہ اعلامیے کو قبول کرتی ہے انضمام ٹیسٹ میں API ایپ کے ذریعہ `/v2/sorafs/capacity/declare` کی مشق کی جاتی ہے ، نوڈ رجسٹری میں دستخطی انتظام ، میٹا ڈیٹا کیپچر اور ہینڈ آف کی جانچ پڑتال کی جاتی ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| سمارٹ معاہدہ متضاد پے لوڈ کو مسترد کرتا ہے یونٹ ٹیسٹ اس بات کو یقینی بناتا ہے کہ فراہم کنندہ آئی ڈی اور پرعزم جی آئی بی فیلڈز استقامت سے قبل دستخط شدہ اعلامیہ سے ملتے ہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| سی ایل آئی نے کیننیکل آن بورڈنگ نمونے کا اخراج کیا | سی ایل آئی کنٹرول ڈٹرمینسٹک Norito/JSON/BASE64 آؤٹ پٹ لکھتا ہے اور راؤنڈ ٹرپس کی توثیق کرتا ہے تاکہ آپریٹرز آف لائن اعلامیہ تیار کرسکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| آپریٹر گائیڈ میں داخلے کے ورک فلو اور گورننس گارڈریلز کا احاطہ کیا گیا ہے دستاویزات میں رپورٹنگ اسکیم ، پالیسی ڈیفالٹس اور کونسل کے لئے جائزہ لینے والے اقدامات کی فہرست دی گئی ہے۔ | `../storage-capacity-marketplace.md` |

### تنازعہ کا حل

| چیک | توثیق | ثبوت |
| ------- | ------------ | --------- |
| قانونی چارہ جوئی کے ریکارڈ پے لوڈ کے کیننیکل ڈائجسٹ کے ساتھ برقرار ہیں یونٹ ٹیسٹ تنازعہ کا اندراج کرتا ہے ، ذخیرہ شدہ پے لوڈ کو ڈی کوڈ کرتا ہے اور لیجر کے عزم کی ضمانت کے لئے زیر التواء حیثیت پر زور دیتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| سی ایل آئی تنازعہ جنریٹر کیننیکل اسکیما سے مساوی ہے سی ایل آئی ٹیسٹ میں `CapacityDisputeV1` کے لئے BASE64/Norito آؤٹ پٹ اور JSON ڈائجسٹ کا احاطہ کیا گیا ہے ، اس بات کو یقینی بناتا ہے کہ شواہد ہیش کو طے شدہ طور پر بنڈل بناتے ہیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| ری پلے ٹیسٹ میں قانونی چارہ جوئی/جرمانے کا تعین ثابت ہوتا ہے دو بار دوبارہ چلائے جانے والے پروف-فیلچر ٹیلی میٹری سے لیجر ، کریڈٹ اور قانونی چارہ جوئی کے ایک جیسے سنیپ شاٹس پیدا ہوتے ہیں تاکہ ہم عمروں کے مابین سلیشس کا تعی .ن ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| رن بوک دستاویزات میں اضافہ اور منسوخی کا بہاؤ | آپریشنز گائیڈ کونسل کے ورک فلو ، ثبوت کی ضروریات اور رول بیک کے طریقہ کار کو اپنی گرفت میں لے لیتا ہے۔ | `../dispute-revocation-runbook.md` |

### خزانے کا مفاہمت| چیک | توثیق | ثبوت |
| ------- | ------------ | --------- |
| لیجر کا جمع 30 دن سے زیادہ بھگنے کی پیش گوئی کے مساوی ہے ایس اے او سی ٹیسٹ میں 30 آبادکاری ونڈوز میں پانچ فراہم کنندگان کا احاطہ کیا گیا ہے ، جس میں لیجر اندراجات کو متوقع ادائیگی کے حوالہ سے موازنہ کیا گیا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| لیجر برآمدات کا مفاہمت ہر رات ریکارڈ کیا جاتا ہے | `capacity_reconcile.py` لیجر فیس کی توقعات کو پھانسی شدہ XOR برآمدات سے موازنہ کرتا ہے ، میٹرکس Prometheus کا اخراج کرتا ہے اور الرٹ مینجر کے ذریعہ ٹریژری کی منظوری کو دروازہ دیتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1` ، `docs/source/sorafs/runbooks/capacity_reconciliation.md:1` ، `dashboards/alerts/sorafs_capacity_rules.yml:100` |
| بلنگ ڈیش بورڈز جرمانے اور جمع ٹیلی میٹری کو بے نقاب کرتے ہیں Grafana درآمد گیب گھنٹے جمع ہونے ، ہڑتال کے کاؤنٹرز اور کال کی نمائش کے لئے خودکش حملہ کا سراغ لگاتا ہے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| رپورٹ جاری آرکائیوز کو ریلیز کرتا ہے طریقہ کار اور ری پلے کمانڈز | رپورٹ میں تفصیلات سننے والوں کے لئے دائرہ کار ، عملدرآمد کے احکامات ، اور مشاہدہ کرنے کے ہکس کو بھگوائیں۔ | `./sf2c-capacity-soak.md` |

## نوٹ چل رہا ہے

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

آپریٹرز کو `sorafs_manifest_stub capacity {declaration,dispute}` کے ساتھ جہاز پر سوار/تنازعہ کی درخواست پے لوڈ کو دوبارہ تخلیق کرنا ہوگا اور اس کے نتیجے میں JSON/Norito بائٹس کو گورننس کے ٹکٹ کے ساتھ محفوظ کرنا ہوگا۔

## سائن آف آرٹیکٹ

| نمونہ | راستہ | بلیک 2 بی -256 |
| --------- | ------ | --------------- |
| فراہم کنندہ آن بورڈنگ منظوری پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| تنازعات کے حل کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| ٹریژری مفاہمت کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان نمونے کی دستخط شدہ کاپیاں ریلیز کے بنڈل کے ساتھ رکھیں اور انہیں گورننس چینج رجسٹری میں جوڑیں۔

## منظوری

-اسٹوریج ٹیم لیڈ- @اسٹوریج-ٹی ایل (2026-03-24)  
-گورننس کونسل کے سکریٹری- @کونسل سیکنڈ (2026-03-24)  
-ٹریژری آپریشنز لیڈ- @ٹریژری آپس (2026-03-24)