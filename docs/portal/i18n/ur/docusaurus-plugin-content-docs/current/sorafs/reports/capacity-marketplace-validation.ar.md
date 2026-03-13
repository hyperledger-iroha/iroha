---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: چیک صلاحیت مارکیٹ SoraFS
ٹیگز: [SF-2C ، قبولیت ، چیک لسٹ]
خلاصہ: ایک قبولیت چیک لسٹ فراہم کنندہ کو جہاز پر چلنے والے ، تنازعہ کے بہاؤ ، اور ٹریژری ریزولوشن جو SoraFS صلاحیت مارکیٹ کے عوامی آغاز کے لئے تیاری کو کنٹرول کرتی ہے۔
---

#SoraFS صلاحیت مارکیٹ کی توثیق چیک لسٹ

** جائزہ ونڈو: ** 2026-03-18-> 2026-03-24  
** پروگرام مالکان: ** اسٹوریج ٹیم (`@storage-wg`) ، گورننس کونسل (`@council`) ، ٹریژری گلڈ (`@treasury`)  
** دائرہ کار: ** فراہم کنندہ راستے ، تنازعہ ثالثی کے بہاؤ ، اور SF-2C GA کے لئے درکار ٹریژری مفاہمت کے عمل۔

بیرونی آپریٹرز کے لئے بازار کو چالو کرنے سے پہلے آپ کو نیچے دیئے گئے چیک لسٹ کا جائزہ لینا چاہئے۔ ہر صف ایک ڈٹرمینسٹک ڈائرکٹری (ٹیسٹ ، فکسچر ، یا دستاویزات) سے منسلک ہوتی ہے جسے آڈیٹر دوبارہ چلا سکتے ہیں۔

## داخلہ چیک لسٹ

### فراہم کنندگان میں شامل ہوں

| امتحان | توثیق | گائیڈ |
| ------- | ------------ | ------------ |
| رجسٹری معیاری صلاحیت کے اعلان کو قبول کرتی ہے | ایک انضمام ٹیسٹ `/v2/sorafs/capacity/declare` کو ایپ API کے ذریعے چلتا ہے ، پروسیسنگ کے دستخطوں کی تصدیق کرتا ہے ، میٹا ڈیٹا پر قبضہ کرتا ہے ، اور اسے رجسٹری نوڈ تک پہنچاتا ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| سمارٹ معاہدہ بے مثال پے لوڈز ماڈیولر ٹیسٹنگ کو مسترد کرتا ہے اس بات کو یقینی بناتا ہے کہ فراہم کنندہ آئی ڈی اور پرعزم جی آئی بی فیلڈز بچت سے پہلے دستخط شدہ اعلامیہ سے ملتے ہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| سی ایل آئی کے معاملات معیاری میں شامل ہونا نوادرات | سی ایل آئی کنٹرول ڈٹرمینسٹک Norito/JSON/BASE64 آؤٹ پٹ لکھتا ہے اور راؤنڈ ٹرپس کی جانچ پڑتال کرتا ہے تاکہ آپریٹرز آف لائن اشتہارات ترتیب دے سکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| آپریٹرز ہدایت نامہ داخلہ کے عمل اور گورننس کی رکاوٹوں کی دستاویزات کو کونسل کے اعلان کے منصوبے ، پالیسی کے پہلے سے طے شدہ ، اور جائزہ لینے کے اقدامات کی فہرست میں شامل ہیں۔ | `../storage-capacity-marketplace.md` |

### تنازعات کا تصفیہ

| امتحان | توثیق | گائیڈ |
| ------- | ------------ | ------------ |
| تنازعات کے نوشتہ پے لوڈ کے معیاری ڈائجسٹ کے ساتھ باقی ہیں ماڈیول ٹیسٹ تنازعہ کو لاگ ان کرتا ہے ، اسٹور پے لوڈ کو کھول دیتا ہے ، اور لیجر کے تعین کو یقینی بنانے کے لئے زیر التوا ریاست پر زور دیتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| سی ایل آئی تنازعہ جنریٹر معیاری اسکیما سے مماثل ہے سی ایل آئی ٹیسٹ میں بیس 64/Norito آؤٹ پٹ اور `CapacityDisputeV1` کے JSON ڈائجسٹس کا احاطہ کیا گیا ہے ، اس بات کو یقینی بناتا ہے کہ ثبوت کے بنڈل لامحالہ بکھری ہوئے ہیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| ری پلے ٹیسٹنگ تنازعات/سزا کی ناگزیریت کو ثابت کرتی ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| رن بک میں اضافے اور منسوخی کے راستے کو دستاویز کیا گیا ہے جو آپریشنز دستی کونسل کی پیشرفت ، شواہد کی ضروریات اور رول بیک کے طریقہ کار کو اپنی گرفت میں لیتے ہیں۔ | `../dispute-revocation-runbook.md` |

### ٹریژری تصفیہ| امتحان | توثیق | گائیڈ |
| ------- | ------------ | ------------ |
| لیجر جمع 30 دن کے بھگنے کی پیشن گوئی سے مماثل ہے ادائیگیوں کے متوقع حوالہ کے مقابلے میں 30 آبادکاری ونڈوز سے زیادہ پانچ فراہم کنندگان میں بھرا ہوا ٹیسٹ پھیلا ہوا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| راتوں رات ریکارڈ شدہ لیجر برآمدات کا تصفیہ | `capacity_reconcile.py` فیس لیجر کی پیشن گوئی کا موازنہ XOR تبادلوں کی برآمدات ، برآمدات Prometheus میٹرکس کے ساتھ کرتا ہے ، اور الرٹ مینجر کے ذریعہ ٹریژری کی منظوری کا تعین کرتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1` ، `docs/source/sorafs/runbooks/capacity_reconciliation.md:1` ، `dashboards/alerts/sorafs_capacity_rules.yml:100` |
| بلنگ پینل جرمانے اور ٹیلی میٹری جمع کرنے کی درآمد Grafana GIB-HOR Accomulator ، ہڑتالوں کا مقابلہ کرتا ہے ، اور شفٹ ٹیم کی مرئیت کو قابل بنانے کے لئے سیکیورٹی کا پابند ہے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| شائع شدہ رپورٹ میں ایس اے اے سی کے طریقہ کار اور ری پلے کو آرکائیو کیا گیا ہے حکم دیتا ہے کہ رپورٹ میں آڈیٹرز کی نگرانی کے لئے بھگوا ، عملدرآمد کے احکامات ، اور ہکس کے دائرہ کار کی وضاحت کی گئی ہے۔ | `./sf2c-capacity-soak.md` |

## نفاذ کے نوٹ

سائن آف سے پہلے توثیق کے پیکیج کو دوبارہ چلائیں:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

آپریٹرز کو `sorafs_manifest_stub capacity {declaration,dispute}` کے ذریعے شامل/تنازعہ کی درخواست پے لوڈ کو دوبارہ تخلیق کرنا ہوگا اور اس کے نتیجے میں JSON/Norito بائٹس کو گورننس کے ٹکٹ کے ساتھ محفوظ کرنا ہوگا۔

## نوادرات کی منظوری

| نوادرات | راستہ | بلیک 2 بی -256 |
| ---------- | ------ | --------------- |
| سپلائر الحاق کی منظوری پیکیج `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| تنازعات کے حل کی رضامندی کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| ٹریژری تصفیہ کی منظوری کا پیکیج | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان نوادرات کی دستخط شدہ کاپیاں ریلیز پیکیج کے ساتھ رکھیں اور انہیں گورننس چینج لاگ سے جوڑیں۔

## منظوری

-اسٹوریج ٹیم لیڈ- @اسٹوریج-ٹی ایل (2026-03-24)  
-گورننس کونسل کے سکریٹری- @کونسل سیکنڈ (2026-03-24)  
-ٹریژری آپریشنز لیڈ- @ٹریژری آپس (2026-03-24)