---
lang: ur
direction: rtl
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ معیاری ماخذ
`docs/source/da/replication_policy.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ کام نہ کریں
پرانی دستاویزات کا خاتمہ۔
:::

# ڈیٹا کی دستیابی بے کار پالیسی (DA-4)

_ اسٹیٹس: پیشرفت میں - مالکان: کور پروٹوکول WG / اسٹوریج ٹیم / SRE_

ڈی اے کی انجسٹ پائپ لائن ہر بلاب کلاس میں برقرار رکھنے کے اہداف کا اطلاق کرتی ہے جس میں ذکر کیا گیا ہے
`roadmap.md` (DA-4 راستہ)۔ Torii اپنے فراہم کردہ برقرار رکھنے کے گولوں کو برقرار رکھنے سے انکار کرتا ہے
کالر اگر تشکیل شدہ پالیسی مماثل نہیں ہے تو ، اس بات کو یقینی بناتے ہوئے کہ ہر جائز/اسٹوریج نوڈ ایک نمبر کو برقرار رکھے
مرسل کے ارادے پر بھروسہ کیے بغیر مطلوبہ دور اور کاپیاں۔

## ڈیفالٹ پالیسی

| کلاس بلاب | گرم رکھیں | سردی رکھیں | مطلوبہ کاپیاں | اسٹوریج کلاس | گورننس ٹیگ |
| ---------- | -------------- | ------------- | ------------- | ------------- | ------------- |
| `taikai_segment` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 گھنٹے | 7 دن | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 گھنٹے | 180 دن | 3 | `cold` | `da.governance` |
| _ ڈیفالٹ (دیگر تمام زمرے) _ | 6 گھنٹے | 30 دن | 3 | `warm` | `da.default` |

ان اقدار کو `torii.da_ingest.replication_policy` میں ملایا گیا ہے اور سب پر لاگو کیا گیا ہے
درخواستیں `/v2/da/ingest`۔ Torii نافذ شدہ فائل اور ریلیز کے ساتھ ظاہر ہوتا ہے
انتباہ جب کال کرنے والے مماثل اقدار فراہم کرتے ہیں تاکہ آپریٹرز ایس ڈی کے کا پتہ لگاسکیں
متروک

### زمرے تائیکائی فراہم کرتا ہے

`availability_class` کے لئے تائیکائی ہدایت نامہ ظاہر (`taikai.trm`) کا اعلان کریں
(`hot` ، `warm` ، یا `cold`)۔ Torii تقسیم سے پہلے مماثل پالیسی کو نافذ کرتا ہے تاکہ یہ ہوسکے
آپریٹرز عالمی نظام الاوقات میں ترمیم کیے بغیر فی اسٹریم کی کاپیاں کی تعداد کو بڑھا سکتے ہیں۔ پہلے سے طے شدہ:

| دستیابی کا زمرہ | گرم رکھیں | سردی رکھیں | مطلوبہ کاپیاں | اسٹوریج کلاس | گورننس ٹیگ |
| ------------ | -------------- | ------------- | ------------- | ------------- | ------------- |
| `hot` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 گھنٹے | 30 دن | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 گھنٹہ | 180 دن | 3 | `cold` | `da.taikai.archive` |

گمشدہ اشارے `hot` پر واپس جاتے ہیں تاکہ رواں سلسلہ مضبوط پالیسی کو برقرار رکھیں۔ اٹھو
ڈیفالٹس کے ذریعے اوور رائڈنگ کرکے
`torii.da_ingest.replication_policy.taikai_availability` اگر آپ کا نیٹ ورک استعمال کررہا ہے
مختلف اہداف

## ترتیبات

پالیسی `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور اس کے ساتھ * ڈیفالٹ * ٹیمپلیٹ دکھاتی ہے
ہر کلاس کے لئے اوور رائڈس کا میٹرکس۔ طبقاتی شناخت کنندہ کیس غیر حساس ہیں اور انہیں قبول کرلیا جاتا ہے
`taikai_segment` ، `nexus_lane_sidecar` ، `governance_artifact` ، یا `custom:<u16>`
منظور شدہ توسیع کے ل them ، ان کو چیک کریں۔ اسٹوریج کلاسز `hot` ، `warm` ، یا `cold` کو قبول کرتے ہیں۔

```toml
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

مذکورہ بالا پہلے سے طے شدہ اقدار کے ساتھ کام کرنے کے لئے بلاک کو چھوڑ دیں۔ کسی زمرے کو سخت کرنے کے لئے ، اوور رائڈ اپ ڈیٹ کریں
ملاپ ؛ نئی قسموں کی بنیاد کو تبدیل کرنے کے لئے ، `default_retention` میں ترمیم کریں۔

تائکائی کی دستیابی کے زمرے آزادانہ طور پر اس کے ذریعے اوورڈ ہوسکتے ہیں
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

- Torii صارف کے فراہم کردہ `RetentionPolicy` کو تقسیم سے پہلے نافذ فائل کے ساتھ تبدیل کرتا ہے
  یا ظاہر
- پری بلٹ کے ظاہر ہونے کو مسترد کرتا ہے جو برقرار رکھنے کی فائل کا اعلان کرتا ہے جو مماثل نہیں ہے
  `400 schema mismatch` تاکہ متروک کلائنٹ معاہدے کو جوڑ نہ سکیں۔
- ہر اوور رائڈ ایونٹ لاگ ان ہوتا ہے (`blob_class` ، پالیسی بھیجا گیا بمقابلہ متوقع)
  رول آؤٹ کے دوران غیرمعمولی کال کرنے والوں کو ظاہر کرنے کے لئے۔تازہ ترین گیٹ وے کے لئے [ingest-plan.md) (ingest-plan.md) دیکھیں
جو برقرار رکھنے کے نفاذ کا احاطہ کرتا ہے۔

## بے کار ورک فلو (DA-4 کے بعد)

برقرار رکھنے کا نفاذ صرف پہلا قدم ہے۔ آپریٹرز کو یہ بھی ثابت کرنا ہوگا کہ ظاہر ہوتا ہے
براہ راست اور دہرانے والے احکامات تشکیل شدہ پالیسی کے مطابق رہتے ہیں تاکہ SoraFS دوبارہ شروع ہوسکے
خود بخود متضاد بلبس کی نقل تیار کریں۔

1. ** انحراف کی نگرانی کریں۔ ** Torii جاری کیا گیا ہے
   `overriding DA retention policy to match configured network baseline` جب
   کال کرنے والا پرانی اقدار کو برقرار رکھنے کے لئے بھیجتا ہے۔ پیمائش کے ساتھ اس ریکارڈ سے وابستہ ہوں
   `torii_sorafs_replication_*` گمشدہ کاپیاں یا دیر سے دوبارہ شائع ہونے کا پتہ لگانے کے لئے۔
2. ** ارادے کا فرق بمقابلہ براہ راست نقلیں۔ ** نئے آڈٹ اسسٹنٹ کا استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   فراہم کردہ ترتیبات سے کمانڈ `torii.da_ingest.replication_policy` لوڈ کریں ،
   یہ ہر مینی فیسٹ (JSON یا Norito) کو ڈکرپٹ کرتا ہے ، اور اختیاری طور پر پے لوڈ سے میل کھاتا ہے۔
   `ReplicationOrderV1` منشور کے لئے ڈائجسٹ کے ذریعے۔ مندرجہ ذیل دو شرائط کا خلاصہ:

   - `policy_mismatch` - منشور میں برقرار رکھنے کی فائل مسلط پالیسی سے مختلف ہے
     (یہ تب ہی ہونا چاہئے جب Torii غلط طریقے سے تشکیل دیا گیا ہو۔)
   - `replica_shortfall` - براہ راست ڈپلیکیٹ کمانڈ سے اس سے کم کاپیاں درخواست کرتی ہیں
     `RetentionPolicy.required_replicas` یا اسائنمنٹس فراہم کرتا ہے جو ہدف سے کم ہیں۔

   غیر صفر سے باہر نکلنے کی حیثیت کا مطلب ایک فعال ناکامی ہے تاکہ CI/آن کال آٹومیشن الرٹ کرسکے
   فورا. `docs/examples/da_manifest_review_template.md` پیکیج سے JSON رپورٹ منسلک کریں
   پارلیمانی ووٹ کے لئے۔
3. ** ٹرگر فالتو پن۔ ** جب آڈٹ کی کمی کی اطلاع ہے تو ، `ReplicationOrderV1` جاری کریں
   بیان کردہ گورننس ٹولز کے ذریعے نیا
   [SoraFS اسٹوریج صلاحیت کی مارکیٹ پلیس] (../sorafs/storage-capacity-marketplace.md)
   جب تک کاپی سیٹ نہ ہوجائے تب تک آڈٹ دوبارہ جاری رکھیں۔ ایمرجنسی اوور رائڈس کے ل link ، آؤٹ پٹ لنک کریں
   `iroha app da prove-availability` کے ساتھ CLI تاکہ SREs ایک ہی ڈائجسٹ کا حوالہ دے سکیں
   اور PDP دستی۔

ریگریشن کوریج `integration_tests/tests/da/replication_policy.rs` پر واقع ہے۔ تم اٹھو
پیکیج `/v2/da/ingest` کو ایک مماثل برقراری کی پالیسی بھیجتا ہے اور اس کی تصدیق کرتا ہے
مینی فیسٹ کالر کے ارادے کے بجائے جبری فائل دکھاتا ہے۔