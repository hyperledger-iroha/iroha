---
lang: ur
direction: rtl
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
آئینہ `docs/source/da/replication_policy.md`۔ دونوں ورژن کو اندر رکھیں
:::

# ڈیٹا کی دستیابی کی نقل پالیسی (DA-4)

_ اسٹیٹس: پیشرفت میں - ذمہ دار: کور پروٹوکول WG / اسٹوریج ٹیم / SRE_

دا انجشن پائپ لائن اب ہر ایک کے لئے برقرار رکھنے کے اہداف کا اطلاق کرتی ہے
`roadmap.md` (ورک اسٹریم DA-4) میں بیان کردہ بلاب کلاس۔ Torii برقرار رکھنے سے انکار کرتا ہے
کالر فراہم کردہ برقراری لفافے جو پالیسی سے مماثل نہیں ہیں
تشکیل شدہ ، اس بات کو یقینی بناتے ہوئے کہ ہر جائز/اسٹوریج نوڈ نمبر برقرار رکھتا ہے
بھیجنے والے کے ارادے پر منحصر بغیر کسی عہدوں اور نقلوں کی ضرورت ہے۔

## ڈیفالٹ پالیسی

| بلاب کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | نقل کی ضرورت ہے | اسٹوریج کلاس | گورننس ٹیگ |
| --------------- | ---------------- | --------------- | ---------------------------------------------- | -------------------------------------------------------- |
| `taikai_segment` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 گھنٹے | 7 دن | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 گھنٹے | 180 دن | 3 | `cold` | `da.governance` |
| _ ڈیفالٹ (دیگر تمام کلاسز) _ | 6 گھنٹے | 30 دن | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں سرایت کر چکے ہیں اور اس کا اطلاق ہوتا ہے
تمام `/v1/da/ingest` گذارشات کو۔ Torii پروفائل کے ساتھ ظاہر ہوتا ہے
ٹیکس روکنے اور جب کال کرنے والے مختلف اقدار فراہم کرتے ہیں تو انتباہ جاری کرتے ہیں
آپریٹرز کے لئے فرسودہ SDKs کا پتہ لگانے کے لئے۔

### تائکائی دستیابی کی کلاسیں

تائیکائی روٹنگ مینیفنس (`taikai.trm`) `availability_class` کا اعلان کریں
(`hot` ، `warm` ، یا `cold`)۔ Torii اس سے پہلے متعلقہ پالیسی کا اطلاق کرتا ہے
chunking تاکہ آپریٹرز بغیر کسی ندی کے نقل کی پیمائش کرسکیں
عالمی جدول میں ترمیم کریں۔ پہلے سے طے شدہ:

| دستیابی کی کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | نقل کی ضرورت ہے | اسٹوریج کلاس | گورننس ٹیگ |
| ---------------------------------------------------- | ----------------- | -------------------------------------------------- | -----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
| `hot` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 گھنٹے | 30 دن | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 گھنٹہ | 180 دن | 3 | `cold` | `da.taikai.archive` |

گمشدہ اشارے `hot` کو بطور ڈیفالٹ استعمال کرتے ہیں تاکہ براہ راست اسٹریمز برقرار رکھیں
مضبوط پالیسی۔ ڈیفالٹس کو اوور رائڈ کریں
Torii اگر آپ کا نیٹ ورک استعمال کرتا ہے
مختلف اہداف۔

## کنفیگریشن

پالیسی `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک ٹیمپلیٹ کو بے نقاب کرتی ہے
* ڈیفالٹ* پلس فی کلاس اوور رائڈس کی ایک صف۔ کلاس شناخت کرنے والے ایسا نہیں کرتے ہیں
کیس حساس ہیں اور `taikai_segment` ، `nexus_lane_sidecar` کو قبول کریں ،
حکومت سے منظور شدہ توسیع کے ل I `governance_artifact` ، یا `custom:<u16>`۔
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
```مذکورہ بالا ڈیفالٹس کے ساتھ چلانے کے لئے بلاک کو برقرار رکھیں۔ سخت کرنے کے لئے a
کلاس ، اسی اوور رائڈ کو اپ ڈیٹ کریں۔ نئی کلاسوں کی بنیاد کو تبدیل کرنے کے لئے ،
`default_retention` میں ترمیم کریں۔

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

- Torii USER کے ذریعہ مسلط پروفائل کے لئے فراہم کردہ `RetentionPolicy` کی جگہ لے لیتا ہے
  منشور کو چنک کرنے یا جاری کرنے سے پہلے۔
- پہلے سے تعمیر شدہ ظاہر ہوتا ہے جو اعلان کرتے ہیں کہ مختلف برقرار رکھنے کا پروفائل ہے
  `400 schema mismatch` کے ساتھ مسترد کردیا گیا تاکہ متروک کلائنٹ نہیں کرسکتے ہیں
  معاہدہ کو کمزور کریں۔
- ہر اوور رائڈ ایونٹ لاگ ان ہوتا ہے (`blob_class` ، پالیسی بھیجا گیا بمقابلہ متوقع)
  رول آؤٹ کے دوران غیر تعمیل کال کرنے والوں کو بے نقاب کرنا۔

[ڈیٹا کی دستیابی انجسٹ پلان] (ingest-plan.md) (توثیق چیک لسٹ) کے لئے دیکھیں
برقرار رکھنے کے نفاذ کا احاطہ کرنے والا تازہ ترین گیٹ۔

## دوبارہ دوبارہ استعمال کرنے والا ورک فلو (DA-4 ٹریکنگ)

برقرار رکھنے کا نفاذ صرف پہلا قدم ہے۔ آپریٹرز کو بھی لازمی ہے
ثابت کریں کہ براہ راست ظاہر اور نقل کے احکامات پالیسی کے ساتھ منسلک ہیں
تشکیل شدہ تاکہ SoraFS غیر تعمیل بلابز کو دوبارہ دوبارہ ترتیب دے سکے
خود بخود.

1. ** بڑھے ہوئے نوٹ کریں۔ ** Torii مسائل
   `overriding DA retention policy to match configured network baseline` جب
   ایک کالر پرانی برقرار رکھنے کی رقم جمع کراتا ہے۔ اس لاگ کو اس کے ساتھ جوڑیں
   `torii_sorafs_replication_*` ٹیلی میٹری لاپتہ نقلوں کا پتہ لگانے کے لئے یا
   دوبارہ تعی .ن میں تاخیر ہوئی۔
2. ** مختلف ارادے بمقابلہ براہ راست نقلیں۔ ** نیا آڈٹ مددگار استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   کمانڈ ترتیب سے `torii.da_ingest.replication_policy` لوڈ کرتا ہے
   بشرطیکہ ہر منشور (JSON یا Norito) کو ضابطہ کشائی کرتا ہے ، اور اختیاری طور پر میچ کرتا ہے
   `ReplicationOrderV1` پے لوڈ فی منشور ڈائجسٹ۔ خلاصہ دو پر روشنی ڈالتا ہے
   شرائط:

   - `policy_mismatch` - مینی فیسٹ برقراری پروفائل پالیسی سے ہٹ جاتا ہے
     مسلط (یہ اس وقت تک نہیں ہونا چاہئے جب تک کہ Torii غلط کنفیگر نہیں ہوتا ہے)۔
   - `replica_shortfall` - براہ راست نقل کے آرڈر کے مقابلے میں کم نقل کی درخواست کرتا ہے
     `RetentionPolicy.required_replicas` کے مقابلے میں یا اس سے کم اسائنمنٹ فراہم کرتا ہے
     ہدف

   غیر صفر سے باہر نکلنے کی حیثیت ایک فعال کمی کی نشاندہی کرتی ہے تاکہ آؤٹ پٹ آٹومیشن
   CI/آن کال فوری طور پر صفحہ رکھ سکتا ہے۔ JSON رپورٹ کو پیکیج سے منسلک کریں
   `docs/examples/da_manifest_review_template.md` پارلیمنٹ کے ووٹوں کے لئے۔
3. ** ٹرگر دوبارہ دوبارہ نقل۔
   گورننس ٹولز کے ذریعے نیا `ReplicationOrderV1`
   [SoraFS اسٹوریج صلاحیت کی مارکیٹ پلیس] (../sorafs/storage-capacity-marketplace.md)
   اور دوبارہ آڈٹ چلائیں جب تک کہ نقل کا سیٹ ایک دوسرے کے ساتھ نہ آجائے۔ اوور رائڈس کے لئے
   ایمرجنسی ، CLI آؤٹ پٹ کو `iroha app da prove-availability` کے ساتھ جوڑیں
   کہ SREs ایک ہی ڈائجسٹ اور ثبوت PDP کا حوالہ دے سکتے ہیں۔

ریگریشن کوریج `integration_tests/tests/da/replication_policy.rs` پر رہتا ہے۔
سویٹ `/v1/da/ingest` اور چیک پر ایک مختلف برقرار رکھنے کی پالیسی بھیجتا ہے
جو ظاہر ہوتا ہے وہ کال کرنے والے کے ارادے کے بجائے مسلط کردہ پروفائل کو بے نقاب کرتا ہے۔