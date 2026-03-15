---
lang: ur
direction: rtl
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
`docs/source/da/replication_policy.md` کی عکاسی کرتا ہے۔ دونوں ورژن کو اندر رکھیں
:::

# ڈیٹا کی دستیابی کی نقل پالیسی (DA-4)

_ اسٹیٹس: پیشرفت میں - ذمہ دار: کور پروٹوکول WG / اسٹوریج ٹیم / SRE_

ڈی اے انجسٹ پائپ لائن اب برقرار رکھنے کے اہداف کو نافذ کرتی ہے
`roadmap.md` (ورک اسٹریم میں بیان کردہ ہر بلاب کلاس کے لئے تعصب پسند
DA-4)۔ Torii کے ذریعہ فراہم کردہ برقراری کے لفافوں کو برقرار رکھنے سے انکار کرتا ہے
کالر جو تشکیل شدہ پالیسی سے مماثل نہیں ہے ، اس بات کو یقینی بناتے ہیں
ہر جائز/اسٹوریج نوڈ میں مطلوبہ تعداد میں اور
جاری کرنے والے کے لئے انحصار کے بغیر نقلیں۔

## ڈیفالٹ پالیسی

| بلاب کلاس | برقرار رکھنا گرم | برقرار رکھنے کی سردی | نقل کی ضرورت ہے | اسٹوریج کلاس | گورننس ٹیگ |
| --------------- | --------------- | ------------------ | ------------------- | ---------------------- | ----------------------- |
| `taikai_segment` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 گھنٹے | 7 دن | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 گھنٹے | 180 دن | 3 | `cold` | `da.governance` |
| _ ڈیفالٹ (دیگر تمام کلاسز) _ | 6 گھنٹے | 30 دن | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں مربوط ہیں اور اس کا اطلاق ہوتا ہے
تمام گذارشات `/v1/da/ingest` پر۔ Torii کے ساتھ ظاہر ہوتا ہے
جب کال کرنے والے فراہم کرتے ہیں تو برقرار رکھنے کا پروفائل ایک انتباہ نافذ کرتا ہے اور اس کا اخراج کرتا ہے
متضاد اقدار تاکہ آپریٹرز متروک SDKs کا پتہ لگائیں۔

### تائکائی دستیابی کی کلاسیں

تائیکائی روٹنگ مینیفنس (`taikai.trm`) `availability_class` کا اعلان کریں
(`hot` ، `warm` ، یا `cold`)۔ Torii اس سے پہلے متعلقہ پالیسی کا اطلاق کرتا ہے
چنکنگ تاکہ آپریٹرز نقل کی گنتی کو ایڈجسٹ کرسکیں
عالمی جدول میں ترمیم کیے بغیر اسٹریم کریں۔ پہلے سے طے شدہ:

| دستیابی کی کلاس | برقرار رکھنا گرم | برقرار رکھنے کی سردی | نقل کی ضرورت ہے | اسٹوریج کلاس | گورننس ٹیگ |
| ------------------------- | --------------- | ------------------ | ---------------------------------------------------------------------- |
| `hot` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 گھنٹے | 30 دن | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 گھنٹہ | 180 دن | 3 | `cold` | `da.taikai.archive` |

لاپتہ اشارے `hot` میں پلٹ جاتے ہیں تاکہ براہ راست نشریات کو برقرار رکھیں
سب سے مضبوط پالیسی۔ ڈیفالٹس کے ذریعے تبدیل کریں
Torii اگر آپ کا نیٹ ورک استعمال کرتا ہے
مختلف اہداف۔

## کنفیگریشن

پالیسی `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک ٹیمپلیٹ کو بے نقاب کرتی ہے
* ڈیفالٹ* پلس فی کلاس اوور رائڈز کی ایک صف۔ کلاس شناخت کرنے والے ہیں
کیس غیر حساس اور قبول `taikai_segment` ، `nexus_lane_sidecar` ،
`governance_artifact` ، یا `custom:<u16>` کے ذریعہ منظور شدہ توسیع کے لئے
گورننس اسٹوریج کلاسز `hot` ، `warm` ، یا `cold` کو قبول کرتے ہیں۔```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

مذکورہ بالا ڈیفالٹس کو استعمال کرنے کے لئے بلاک کو برقرار رکھیں۔ سخت کرنے کے لئے a
کلاس ، اسی اوور رائڈ کو اپ ڈیٹ کریں۔ کے لئے اڈے کو تبدیل کرنے کے لئے
نئی کلاسیں ، `default_retention` میں ترمیم کریں۔

تائکائی کی دستیابی کی کلاسوں کو آزادانہ طور پر اس کے ذریعے ختم کیا جاسکتا ہے
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## نفاذ کے الفاظ

- Torii صارف کے فراہم کردہ Torii کو پروفائل کے ساتھ تبدیل کرتا ہے
  chunking یا ظاہر اخراج سے پہلے مسلط کیا گیا ہے.
- پری بلٹ ظاہر ہوتا ہے جو ایک مختلف برقراری پروفائل کا اعلان کرتے ہیں
  `400 schema mismatch` کے ساتھ مسترد کردیئے گئے ہیں تاکہ متروک کلائنٹ ایسا نہ کریں
  معاہدہ کو کمزور نہیں کرسکتا۔
- ہر اوور رائڈ ایونٹ لاگ ان ہوتا ہے (`blob_class` ، پالیسی پیش کی گئی بمقابلہ متوقع)
  رول آؤٹ کے دوران غیر تعمیل کال کرنے والوں کو اجاگر کرنا۔

[ڈیٹا کی دستیابی انجسٹ پلان] (ingest-plan.md) (توثیق چیک لسٹ) کے لئے دیکھیں
برقرار رکھنے کے نفاذ کا احاطہ کرنے والا تازہ ترین گیٹ۔

## دوبارہ دوبارہ استعمال کرنے والا ورک فلو (DA-4 مانیٹرنگ)

برقرار رکھنا صرف پہلا قدم ہے۔ آپریٹرز کو لازمی ہے
یہ بھی ثابت کریں کہ براہ راست ظاہر اور نقل کے احکامات باقی ہیں
تشکیل شدہ پالیسی کے ساتھ منسلک
خود بخود تعمیل سے باہر۔

1. ** بہاؤ کے لئے دیکھیں۔ ** Torii اخراج
   `overriding DA retention policy to match configured network baseline` جب
   ایک کال کرنے والا متروک برقراری کی اقدار پیش کرتا ہے۔ اس لاگ کو اس کے ساتھ منسلک کریں
   ٹیلی میٹری `torii_sorafs_replication_*` نقل کی کمی کی نشاندہی کرنے کے لئے
   یا تاخیر سے تعیناتی۔
2. ** مختلف ارادے بمقابلہ نقلیں براہ راست۔

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   کمانڈ تشکیل سے `torii.da_ingest.replication_policy` لوڈ کرتا ہے
   بشرطیکہ ہر منشور (JSON یا Norito) ، اور اختیاری طور پر ساتھی
   پے لوڈ `ReplicationOrderV1` بذریعہ منشور ڈائجسٹ۔ خلاصہ اشارہ کرتا ہے
   دو شرائط:

   - `policy_mismatch` - مینی فیسٹ برقرار رکھنے کا پروفائل پروفائل سے ہٹ جاتا ہے
     مسلط (ایسا کبھی نہیں ہونا چاہئے جب تک کہ Torii غلط طریقے سے تشکیل نہ دیا جائے)۔
   - `replica_shortfall` - براہ راست نقل کے آرڈر کے لئے کم نقل کی ضرورت ہوتی ہے
     `RetentionPolicy.required_replicas` کے مقابلے میں یا اس سے کم اسائنمنٹ فراہم کرتا ہے
     اس کا ہدف

   غیر صفر سے باہر نکلنے کی حیثیت ایک فعال ناکامی کی نشاندہی کرتی ہے تاکہ
   CI/آن کال آٹومیشن فوری طور پر صفحہ رکھ سکتا ہے۔ JSON رپورٹ منسلک کریں
   کے ووٹوں کے لئے `docs/examples/da_manifest_review_template.md` کو پیکیج کرنا
   پارلیمنٹ۔
3. ** ٹرگر دوبارہ دوبارہ نقل۔
   بیان کردہ گورننس ٹولز کے ذریعہ ایک نیا `ReplicationOrderV1` جاری کریں
   [SoraFS اسٹوریج صلاحیت مارکیٹ] میں (../sorafs/storage-capacity-marketplace.md)
   اور آڈٹ کو دوبارہ شروع کریں جب تک کہ نقلوں کا سیٹ ایک دوسرے کے ساتھ نہ آجائے۔ اوور رائڈس کے لئے
   ایمرجنسی ، CLI آؤٹ پٹ کو `iroha app da prove-availability` کے ساتھ منسلک کریں تاکہ ایسا کریں
   ایس آر ای ایک ہی ڈائجسٹ اور پی ڈی پی پروف کا حوالہ دے سکتا ہے۔رجعت کا احاطہ `integration_tests/tests/da/replication_policy.rs` میں رہتا ہے۔
سویٹ برقرار رکھنے کی پالیسی پیش کرتا ہے جو `/v1/da/ingest` کے ساتھ تعمیل نہیں کرتا ہے اور تصدیق کرتا ہے
کہ بازیافت شدہ ظاہر ہوتا ہے کہ کال کرنے والے کے ارادے کے بجائے عائد کردہ پروفائل کو بے نقاب کرتا ہے۔