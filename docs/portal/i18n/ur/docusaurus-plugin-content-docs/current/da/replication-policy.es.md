---
lang: ur
direction: rtl
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
`docs/source/da/replication_policy.md` کی عکاسی کرتا ہے۔ دونوں ورژن جاری رکھیں
:::

# ڈیٹا کی دستیابی (DA-4) نقل کی پالیسی

_ اسٹیٹس: پیشرفت میں - ذمہ دار: کور پروٹوکول WG / اسٹوریج ٹیم / SRE_

دا انجشن پائپ لائن اب اس کے لئے برقرار رکھنے کے اہداف کے اہداف کا اطلاق کرتی ہے
`roadmap.md` (ورک اسٹریم DA-4) میں بیان کردہ ہر بلاب کلاس۔ Torii مسترد کرتا ہے
کالر کے ذریعہ فراہم کردہ برقراری کے لفافوں کو برقرار رکھیں جو مماثل نہیں ہیں
تشکیل شدہ پالیسی ، اس بات کو یقینی بناتے ہوئے کہ ہر جائز/اسٹوریج نوڈ برقرار رہے
جاری کرنے والے کے ارادے پر منحصر بغیر کسی عہدوں اور نقلوں کی مطلوبہ تعداد۔

## ڈیفالٹ پالیسی

| بلاب کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | نقل کی ضرورت ہے | اسٹوریج کلاس | گورننس ٹیگ |
| ---- | ----- | ------ | ------ | ----- |
| `taikai_segment` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 گھنٹے | 7 دن | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 گھنٹے | 180 دن | 3 | `cold` | `da.governance` |
| _ ڈیفالٹ (دیگر تمام کلاسز) _ | 6 گھنٹے | 30 دن | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں سرایت کر چکے ہیں اور اس پر لاگو ہیں
تمام درخواستیں `/v1/da/ingest`۔ Torii پروفائل کے ساتھ ظاہر ہوتا ہے
جب کال کرنے والے اقدار کی فراہمی کرتے ہیں تو ٹیکس کو روکتا ہے اور انتباہ جاری کرتا ہے
مماثلت ہے تاکہ آپریٹرز فرسودہ ایس ڈی کے کا پتہ لگائیں۔

### تائکائی دستیابی کی کلاسیں

تائیکائی روٹنگ ظاہر ہوتا ہے (`taikai.trm`) A کا اعلان کرتا ہے
`availability_class` (`hot` ، `warm` ، یا `cold`)۔ Torii پالیسی کا اطلاق کرتا ہے
چنکنگ سے پہلے اسی طرح کے مطابق تاکہ آپریٹرز اسکیل کرسکیں
عالمی جدول میں ترمیم کیے بغیر فی اسٹریم کی نقل کی گنتی۔ پہلے سے طے شدہ:

| دستیابی کی کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | نقل کی ضرورت ہے | اسٹوریج کلاس | گورننس ٹیگ |
| ------------------------------------------------------------------ | -------------------------------------------------------------- | ----- |
| `hot` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 گھنٹے | 30 دن | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 گھنٹہ | 180 دن | 3 | `cold` | `da.taikai.archive` |

گمشدہ پٹریوں کو پہلے سے طے شدہ طور پر `hot` استعمال کیا جاتا ہے تاکہ براہ راست سلسلہ
سب سے مضبوط پالیسی برقرار رکھیں۔ ڈیفالٹس کو اوور رائڈ کریں
Torii اگر آپ کا نیٹ ورک اہداف کا استعمال کرتا ہے
مختلف

## کنفیگریشن

پالیسی `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک ٹیمپلیٹ کو بے نقاب کرتی ہے
* ڈیفالٹ* پلس فی کلاس اوور رائڈز کی ایک صف۔ کلاس شناخت کرنے والے نہیں ہیں
شفٹ حساس ہیں اور `taikai_segment` ، `nexus_lane_sidecar` کو قبول کرتے ہیں ،
گورننس سے منظور شدہ توسیع کے لئے `governance_artifact` ، یا `custom:<u16>`۔
اسٹوریج کلاسز `hot` ، `warm` ، یا `cold` کو قبول کرتے ہیں۔

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
```مذکورہ بالا ڈیفالٹس کو استعمال کرنے کے لئے بلاک کو برقرار رکھیں۔ سخت کرنے کے لئے
ایک کلاس ، اسی اوور رائڈ کو اپ ڈیٹ کریں۔ نئے کی بنیاد کو تبدیل کرنے کے لئے
کلاس ، `default_retention` میں ترمیم کریں۔

تائکائی کی دستیابی کی کلاسوں کو آزادانہ طور پر اوور رائٹ کیا جاسکتا ہے
`torii.da_ingest.replication_policy.taikai_availability` کے ذریعے:

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
  چنکنگ یا منشور کے اجراء سے پہلے نافذ کیا گیا ہے۔
- پہلے سے تعمیر شدہ ظاہر جو ایک مختلف برقراری پروفائل کا اعلان کرتے ہیں
  `400 schema mismatch` کے ساتھ مسترد کردیا گیا تاکہ باسی کلائنٹ نہیں کرسکتے ہیں
  معاہدہ کو کمزور کریں۔
- ہر اوور رائڈ ایونٹ لاگ ان ہوتا ہے (`blob_class` ، پالیسی بھیجا گیا بمقابلہ متوقع)
  رول آؤٹ کے دوران غیر تعمیل کال کرنے والوں کو بے نقاب کرنا۔

دیکھیں [ڈیٹا کی دستیابی انجیریشن پلان] (ingest-plan.md) (توثیق کی فہرست)
تازہ ترین گیٹ کے لئے جو برقرار رکھنے کے نفاذ کا احاطہ کرتا ہے۔

## دوبارہ نقل کی روانی (DA-4 ٹریس)

برقرار رکھنے کا نفاذ صرف پہلا قدم ہے۔ آپریٹرز کو بھی لازمی ہے
ٹیسٹ جو براہ راست ظاہر اور نقل کے احکامات برقرار رکھے جاتے ہیں
پالیسی کے ساتھ منسلک کیا گیا ہے تاکہ SoraFS بلبس کو دوبارہ تبدیل کرسکے
خود بخود تعمیل سے باہر۔

1. ** بہاؤ کے لئے دیکھیں۔ ** Torii اخراج
   `overriding DA retention policy to match configured network baseline` جب
   ایک کال کرنے والا فرسودہ برقرار رکھنے کی اقدار بھیجتا ہے۔ اس لاگ سے ملیں
   `torii_sorafs_replication_*` ٹیلی میٹری لاپتہ نقلوں کا پتہ لگانے کے لئے
   یا تاخیر سے متعلق
2.

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   کمانڈ ترتیب سے `torii.da_ingest.replication_policy` لوڈ کرتا ہے
   بشرطیکہ ہر منشور (JSON یا Norito) کو ضابطہ کشائی کرتا ہے ، اور اختیاری طور پر میچ کرتا ہے
   پے لوڈ `ReplicationOrderV1` بذریعہ منشور ڈائجسٹ۔ خلاصہ دو کی نشاندہی کرتا ہے
   شرائط:

   - `policy_mismatch` - منشور کا برقرار رکھنے والا پروفائل رب سے ہٹ جاتا ہے
     مسلط پالیسی (یہ اس وقت تک نہیں ہونا چاہئے جب تک کہ Torii غلط نہ ہو
     تشکیل شدہ)۔
   - `replica_shortfall` - براہ راست نقل کے آرڈر کی درخواستیں کم نقلیں
     `RetentionPolicy.required_replicas` کے مقابلے میں یا اپنے سے کم اسائنمنٹ فراہم کریں
     مقصد

   غیر صفر آؤٹ پٹ کی حیثیت آٹومیشن کے لئے ایک فعال قلت کی نشاندہی کرتی ہے
   CI/آن کال فوری طور پر صفحہ رکھ سکتا ہے۔ JSON رپورٹ کو پیکیج سے منسلک کریں
   `docs/examples/da_manifest_review_template.md` پارلیمنٹ کے ووٹوں کے لئے۔
3. ** ٹرگر دوبارہ دوبارہ نقل۔
   گورننس ٹولز کے ذریعے نیا `ReplicationOrderV1`
   [SoraFS اسٹوریج صلاحیت کی مارکیٹ پلیس] (../sorafs/storage-capacity-marketplace.md)
   اور آڈٹ کو دوبارہ جاری کریں جب تک کہ نقلیں جمع نہ ہوں۔ کے لئے
   ایمرجنسی اوور رائڈز ، CLI آؤٹ پٹ کو `iroha app da prove-availability` کے ساتھ جوڑیں
   تاکہ ایس آر ای ایک ہی ڈائجسٹ اور پی ڈی پی شواہد کا حوالہ دے سکے۔رجعت کی کوریج جاری ہے
`integration_tests/tests/da/replication_policy.rs` ؛ سویٹ ایک پالیسی بھیجتا ہے
`/v1/da/ingest` پر مماثل برقرار رکھنا اور تصدیق کریں کہ مینی فیسٹ نے حاصل کیا
کال کرنے والے کے ارادے کے بجائے مسلط پروفائل کو بے نقاب کرتا ہے۔