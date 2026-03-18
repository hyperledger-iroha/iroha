---
lang: ur
direction: rtl
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/da/replication_policy.md` کی عکاسی کرتا ہے۔ دونوں ورژن رکھیں
:::

# ڈیٹا کی دستیابی کی نقل پالیسی (DA-4)

_ اسٹیٹس: پیشرفت میں - مالکان: کور پروٹوکول WG / اسٹوریج ٹیم / SRE_

ڈا انسٹ پائپ لائن اب ہر ایک کے لئے برقرار رکھنے کے اہداف کا اطلاق کرتی ہے
`roadmap.md` (ورک اسٹریم DA-4) میں بیان کردہ بلاب کلاس۔ Torii انکار کرتا ہے
کالر کے ذریعہ فراہم کردہ برقراری کے لفافے برقرار رکھیں اگر وہ مماثل نہیں ہیں
تشکیل شدہ پالیسی ، اس بات کو یقینی بناتے ہوئے کہ ہر جائز/اسٹوریج نوڈ کا انعقاد ہوتا ہے
مرسل کے ارادوں پر بھروسہ کیے بغیر مطلوبہ عہد اور نقل کی مطلوبہ تعداد۔

## ڈیفالٹ پالیسی

| کلاس بلاب | گرم برقرار رکھنا | سرد برقرار رکھنا | مطلوبہ اشارے | اسٹوریج کلاس | گورننس ٹیگ |
| ------------ | ---------------- | ------------------ | ------------------------------------------ | -------------------- |
| `taikai_segment` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 گھنٹے | 7 دن | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 گھنٹے | 180 دن | 3 | `cold` | `da.governance` |
| _ ڈیفالٹ (دیگر تمام کلاسز) _ | 6 گھنٹے | 30 دن | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں بنی ہیں اور اس پر لاگو ہیں
تمام `/v1/da/ingest` گذارشات۔ Torii لاگو کے ساتھ ظاہر ہوتا ہے
برقرار رکھنے کا پروفائل اور اگر کال کرنے والے نامناسب بھیجتے ہیں تو انتباہ جاری کرتے ہیں
اقدار تو آپریٹرز فرسودہ ایس ڈی کے کی شناخت کرسکتے ہیں۔

### تائیکائی قابل رسائی کلاسز

تائیکائی روٹنگ مینیفنس (`taikai.trm`) `availability_class` کا اعلان کریں
(`hot` ، `warm` ، یا `cold`)۔ Torii چنکنگ سے پہلے مناسب پالیسی کا اطلاق کرتا ہے ،
تاکہ آپریٹرز بغیر کسی ترمیم کے فی اسٹریم کی نقل کی تعداد کی پیمائش کرسکیں
عالمی جدول پہلے سے طے شدہ:

| رسائ کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | مطلوبہ اشارے | اسٹوریج کلاس | گورننس ٹیگ |
| ------------------- | ---------------- | ------------------ | ---------------------------------------- | -------------------- |
| `hot` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 گھنٹے | 30 دن | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 گھنٹہ | 180 دن | 3 | `cold` | `da.taikai.archive` |

اگر کوئی اشارے نہیں ہیں تو ، `hot` استعمال کیا جاتا ہے تاکہ براہ راست نشریات سب سے زیادہ رکھے۔
سخت پروفائل ڈیفالٹس کو اوور رائڈ کریں
Torii اگر نیٹ ورک استعمال کرتا ہے
دوسرے اہداف

## کنفیگریشن

پالیسی `torii.da_ingest.replication_policy` کے تحت ہے اور فراہم کرتی ہے
* ڈیفالٹ* ٹیمپلیٹ کے علاوہ ہر کلاس کے لئے ایک اوور رائڈ سرنی۔ کلاس IDs
کیس غیر حساس اور قبول `taikai_segment` ، `nexus_lane_sidecar` ،
`governance_artifact` ، یا `custom:<u16>` مینجمنٹ سے منظور شدہ توسیع کے لئے۔
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
```

مذکورہ بالا ڈیفالٹس کو استعمال کرنے کے لئے بلاک کو کوئی تبدیلی نہیں چھوڑیں۔ سخت کرنے کے لئے
کلاس ، اسی اوور رائڈ کو اپ ڈیٹ کریں۔ نئی کلاسوں کے اڈے کو تبدیل کرنے کے لئے ،
`default_retention` میں ترمیم کریں۔تائیکائی دستیابی کی کلاسوں کو الگ الگ سے اوور پرائڈ کیا جاسکتا ہے
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

## سیمنٹکس انفورسمنٹ

- Torii کسٹم Torii کو جبری پروفائل کے ساتھ تبدیل کرتا ہے
  منشور کو چھڑانے یا جاری کرنے سے پہلے۔
-پری بلٹ ظاہر ہوتا ہے جو غیر ملکی برقرار رکھنے کے پروفائل کا اعلان کرتے ہیں ،
  میراثی مؤکلوں کو کمزور ہونے سے روکنے کے لئے `400 schema mismatch` کے ساتھ مسترد کردیئے گئے ہیں
  معاہدہ
- ہر اوور رائڈ ایونٹ لاگ ان ہوتا ہے (`blob_class` ، بھیجے گئے پالیسی بمقابلہ
  توقع) رول آؤٹ کے دوران غیر تعمیل کال کرنے والوں کا پتہ لگانا۔

[ڈیٹا کی دستیابی انجسٹ پلان] (ingest-plan.md) (توثیق چیک لسٹ) کے لئے دیکھیں
نفاذ برقرار رکھنے کے لئے تازہ ترین گیٹ کا احاطہ کرنا۔

## دوبارہ دوبارہ استعمال کرنے والا ورک فلو (فالو اپ ڈی اے -4)

نفاذ برقرار رکھنا صرف پہلا قدم ہے۔ آپریٹرز کو یہ بھی ثابت کرنا ہوگا کہ وہ زندہ ہیں
ظاہر اور نقل کے احکامات تشکیل شدہ پالیسی کے مطابق رہتے ہیں ،
تاکہ SoraFS خود بخود غیر ملاپ والے بلابز کو دوبارہ تبدیل کرسکے۔

1. ** بہاؤ کے لئے دیکھیں۔ ** Torii لکھتا ہے
   `overriding DA retention policy to match configured network baseline` جب
   کالر فرسودہ برقرار رکھنے کی اقدار بھیجتا ہے۔ اس لاگ سے موازنہ کریں
   ٹیلی میٹری `torii_sorafs_replication_*` نقل کی کمی کا پتہ لگانے کے لئے
   یا تاخیر سے تعیناتی۔
2. ** ارادے اور براہ راست نقلوں کا موازنہ کریں۔ ** نیا آڈٹ مددگار استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   کمانڈ ترتیب سے `torii.da_ingest.replication_policy` لوڈ کرتا ہے ،
   ہر مینی فیسٹ (JSON یا Norito) کو ضابطہ کشائی کرتا ہے ، اور اختیاری طور پر میچ کرتا ہے
   پے لوڈ `ReplicationOrderV1` بذریعہ ڈائجسٹ مینی فیسٹ۔ نتیجہ دو حالات کی نشاندہی کرتا ہے:

   - `policy_mismatch` - برقرار رکھنے کا پروفائل جبری سے مبتلا ہوتا ہے
     پروفائل (ایسا نہیں ہونا چاہئے اگر Torii صحیح طریقے سے تشکیل دیا گیا ہو)۔
   - `replica_shortfall` - براہ راست نقل کے آرڈر کی درخواستوں سے کم نقلیں
     `RetentionPolicy.required_replicas` ، یا ہدف سے کم اسائنمنٹس تیار کرتا ہے۔

   غیر صفر سے باہر نکلنے والے کوڈ کا مطلب CI/آن کال آٹومیشن کے لئے فعال کمی ہے
   فوری طور پر صفحہ دے سکتا ہے. پیکیج میں JSON رپورٹ منسلک کریں
   `docs/examples/da_manifest_review_template.md` پارلیمنٹ کے ووٹوں کے لئے۔
3. ** دوبارہ نقل چلائیں۔ ** اگر آڈٹ کی کمی کی اطلاع ہے تو ، رہائی
   میں بیان کردہ مینجمنٹ ٹولز کے ذریعہ نیا `ReplicationOrderV1`
   [SoraFS اسٹوریج کی گنجائش کا بازار] (../sorafs/storage-capacity-marketplace.md) ،
   اور آڈٹ کو دہرائیں جب تک کہ نقل تیار نہ ہوجائے۔ ہنگامی حدود کے لئے
   `iroha app da prove-availability` پر CLI آؤٹ پٹ کا نقشہ بنائیں تاکہ SREs کا حوالہ دے سکے
   اسی ڈائجسٹ اور PDP ثبوت پر۔

ریگریشن کوریج `integration_tests/tests/da/replication_policy.rs` میں ہے۔
سویٹ `/v1/da/ingest` کو غیر ملکی برقرار رکھنے کی پالیسی بھیجتا ہے اور چیک کرتا ہے کہ آیا
کہ نتیجے میں ظاہر ہونے والا ایک جبری پروفائل دکھاتا ہے ، نہ کہ ارادے سے کال کرنے والا۔