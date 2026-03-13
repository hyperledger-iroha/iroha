---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری
عنوان: چنکر پروفائل رجسٹری SoraFS
سائڈبار_لیبل: رجسٹر چنکر
تفصیل: چنکر رجسٹر SoraFS کے لئے پروفائل IDs ، پیرامیٹرز اور تجارتی منصوبہ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/chunker_registry.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفینکس سیٹ مکمل طور پر ریٹائر نہیں ہوتا ہے ، دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

## چنکر پروفائل رجسٹری SoraFS (SF-2A)

SoraFS اسٹیک ایک چھوٹے نام کی جگہ والے رجسٹر کے ذریعہ chunking سلوک پر بات چیت کرتا ہے۔
ہر پروفائل ڈٹرمینسٹک سی ڈی سی پیرامیٹرز ، سیمور میٹا ڈیٹا ، اور متوقع ڈائجسٹ/ملٹی کوڈیک کو ظاہر کرتا ہے جو منشور اور کار آرکائیوز میں استعمال ہوتا ہے۔

پروفائل مصنفین سے مشورہ کرنا چاہئے
[`docs/source/sorafs/chunker_profile_authoring.md`] (./chunker-profile-authoring.md)
مطلوبہ میٹا ڈیٹا کے لئے ، توثیق چیک لسٹ اور پروپوزل ٹیمپلیٹ سے پہلے
نئی اندراجات پیش کرنے کے لئے۔ ایک بار جب تبدیلی کی منظوری دی جاتی ہے
گورننس ، فالو کریں
[رجسٹری رول آؤٹ چیک لسٹ] (./chunker-registry-rollout-checklist.md) اور دی
[اسٹیجنگ مینی فیسٹ پلے بک] (./staging-manifest-playbook) فروغ دینے کے لئے
اسٹیجنگ اور پروڈکشن کے لئے فکسچر۔

### پروفائلز

| نام کی جگہ | نام | سیمور | پروفائل ID | منٹ (بائٹس) | ہدف (بائٹس) | زیادہ سے زیادہ (بائٹس) | بریک اپ ماسک | ملٹی ہش | عرفیوس | نوٹ |
| ---------- | --------- | -------- | -------- | --------- | ------------------ | --------- | --------- | --------- | ------- | ------- |
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (Blake3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 فکسچر میں استعمال شدہ کیننیکل پروفائل |

رجسٹر `sorafs_manifest::chunker_registry` کے تحت کوڈ میں رہتا ہے ([`chunker_registry_charter.md`] (./chunker-registry-charter.md)) کے تحت۔ ہر اندراج
`ChunkerProfileDescriptor` کے طور پر اظہار کیا جاتا ہے:

* `namespace` - منسلک پروفائلز کی منطقی گروپ بندی (جیسے ، `sorafs`)۔
* `name` - پڑھنے کے قابل لیبل (`sf1` ، `sf1-fast` ،…)۔
* `semver` - پیرامیٹر سیٹ کے لئے سیمنٹک ورژن سٹرنگ۔
* `profile` - اصل `ChunkProfile` (کم سے کم/ہدف/زیادہ سے زیادہ/ماسک)۔
* `multihash_code` - ملٹی ہاش استعمال شدہ جب ڈائی ڈائجسٹس تیار کرتے وقت استعمال ہوتا ہے (`0x1f`
  غلطی کے لئے SoraFS)۔

ظاہر ہوتا ہے `ChunkingProfileV1` کے ذریعے پروفائلز کو سیریلائز کرتا ہے۔ ڈھانچہ میٹا ڈیٹا اسٹور کرتا ہے
مذکورہ بالا خام سی ڈی سی کی ترتیبات اور عرف کی فہرست کے ساتھ رجسٹری (نام کی جگہ ، نام ، سیمور) سے۔
صارفین کو پہلے `profile_id` تک رجسٹری کی تلاش کی کوشش کرنی چاہئے اور واپس جانا چاہئے
جب نامعلوم IDs ظاہر ہوتے ہیں تو ان لائن پیرامیٹرز ؛ عرف کی فہرست HTTP کلائنٹ کو یقینی بناتی ہے
رجسٹری کا تقاضا ہے کہ کیننیکل ہینڈل (`namespace.name@semver`) میں پہلی اندراج ہو
SoraFS ، اس کے بعد میراثی عرفی نام۔

ٹولز سے رجسٹری کا معائنہ کرنے کے لئے ، سی ایل آئی مددگار چلائیں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```تمام سی ایل آئی جھنڈے جو JSON لکھتے ہیں (`--json-out` ، `--por-json-out` ، `--por-proof-out` ،
`--por-sample-out`) `-` کو بطور راستہ قبول کریں ، جو پے لوڈ کو اس کے بجائے stdout پر بھیج دیتا ہے
ایک فائل بنائیں۔ اس سے ٹولز میں ڈیٹا پائپ کرنا آسان ہوجاتا ہے جبکہ برقرار رکھتے ہوئے
مرکزی رپورٹ پرنٹ کرنے کے لئے پہلے سے طے شدہ سلوک۔

### رول آؤٹ میٹرکس اور تعیناتی کا منصوبہ


نیچے دیئے گئے جدول میں `sorafs.sf1@1.0.0` کے لئے موجودہ سپورٹ کی حیثیت حاصل کی گئی ہے
اہم اجزاء۔ "پل" سے مراد CARV1 + SHA-256 راستہ ہے جو
واضح کلائنٹ سائیڈ مذاکرات (`Accept-Chunker` + `Accept-Digest`) کی ضرورت ہے۔

| اجزاء | حیثیت | نوٹ |
| --------------- | -------- | ------- |
| `sorafs_manifest_chunk_store` | ✅ سپورٹ | کیننیکل ہینڈل + عرف کی توثیق کرتا ہے ، `--json-out=-` کے ذریعے رپورٹس کو اسٹریم کرتا ہے اور `ensure_charter_compliance()` کے ذریعے رجسٹری چارٹر کا اطلاق کرتا ہے۔ |
| `sorafs_fetch` (ڈویلپر آرکسٹریٹر) | ✅ سپورٹ | `chunk_fetch_specs` کو پڑھتا ہے ، جس میں صلاحیت پے لوڈ `range` شامل ہے ، اور CARV2 آؤٹ پٹ کو جمع کرتا ہے۔ |
| ایس ڈی کے فکسچر (زنگ/گو/ٹی ایس) | ✅ سپورٹ | `export_vectors` کے ذریعے دوبارہ پیدا ہوا ؛ کیننیکل ہینڈل ہر عرف کی فہرست میں پہلے ظاہر ہوتا ہے اور اشارے ریپرز کے ذریعہ اس پر دستخط ہوتے ہیں۔ |
| گیٹ وے پروفائلز Torii کی بات چیت | ✅ سپورٹ | تمام `Accept-Chunker` گرائمر کو نافذ کرتا ہے ، `Content-Chunker` ہیڈر شامل ہوتا ہے اور صرف ڈاون گریڈ کی واضح درخواستوں پر CARV1 پل کو بے نقاب کرتا ہے۔ |

ٹیلی میٹری کی تعیناتی:

۔
- ** فراہم کنندہ اشتہارات ** - اشتہاری پے لوڈ میں صلاحیت اور عرف میٹا ڈیٹا شامل ہیں۔ `/v2/sorafs/providers` (جیسے ، صلاحیت کی موجودگی `range`) کے ذریعے کوریج کی توثیق کریں۔
۔ توقع ہے کہ فرسودگی سے قبل پل کے استعمال سے صفر کی طرف جائے گا۔

فرسودگی کی پالیسی: ایک بار جب کسی جانشین کی پروفائل کی توثیق ہوجائے تو ، ڈبل ریلیز ونڈو کا شیڈول بنائیں
(تجویز میں دستاویزی کردہ) `sorafs.sf1@1.0.0` کو نشان زد کرنے سے پہلے رجسٹری میں فرسودہ اور ہٹانے سے پہلے
پیداوار میں گیٹ ویز کا برج کار وی ون۔

کسی مخصوص پور گواہ کا معائنہ کرنے کے لئے ، حصہ/طبقہ/پتی کے اشاریہ فراہم کریں اور ، اختیاری طور پر ،
ڈسک پر ثبوت جاری رکھیں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

آپ عددی ID (`--profile-id=1`) کے ذریعہ یا رجسٹری ہینڈل کے ذریعہ ایک پروفائل منتخب کرسکتے ہیں
(`--profile=sorafs.sf1@1.0.0`) ؛ ہینڈل فارم اسکرپٹ کے ل useful مفید ہے
گورننس میٹا ڈیٹا سے براہ راست نام کی جگہ/نام/سیمور پاس کریں۔

میٹا ڈیٹا کا JSON بلاک (تمام عرفی ناموں سمیت JSON بلاک کو خارج کرنے کے لئے `--promote-profile=<handle>` استعمال کریں
رجسٹرڈ) جسے کسی نئے پروفائل کو فروغ دیتے وقت `chunker_registry_data.rs` میں چسپاں کیا جاسکتا ہے
پہلے سے طے شدہ:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```مین رپورٹ (اور اختیاری پروف فائل) میں روٹ ڈائجسٹ ، نمونے والے پتے بائٹس شامل ہیں
۔
64 KIB/4 KIB پرتوں کی قیمت `por_root_hex` کا سامنا ہے۔

کسی پے لوڈ کے خلاف موجودہ ثبوت کی توثیق کرنے کے لئے ، راستے کو راستے سے گزرنا
`--por-proof-verify` (CLI `"por_proof_verified": true` کو شامل کرتا ہے جب اشارے
حساب کتاب کی جڑ سے مطابقت رکھتا ہے):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

بیچ کے نمونے لینے کے ل i ، `--por-sample=<count>` استعمال کریں اور اختیاری طور پر بیج/آؤٹ پٹ راستہ فراہم کریں۔
سی ایل آئی ایک عین مطابق ترتیب کی ضمانت دیتا ہے (`splitmix64` کے ساتھ مل کر)
استفسار دستیاب شیٹس سے زیادہ ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

```
$ کارگو رن -پی sorafs_manifest-bin sorafs_manifest_stub--list-chunker-profiles
کے بعد کے کے لئے کے آیا کے آیا کے آیا کے آیا کے آیا کے آیا کے آیا کے آیا کے آیا کے آیا کے آیا ، کے آیا کے ایل کے کے لئے کے یا.
  {
    "پروفائل_ آئی ڈی": 1 ،
    "نام کی جگہ": "sorafs" ،
    "نام": "SF1" ،
    "سیمور": "1.0.0" ،
    "ہینڈل": "sorafs.sf1@1.0.0" ،
    "من_سائز": 65536 ،
    "ٹارگٹ_سائز": 262144 ،
    "میکس_سائز": 524288 ،
    "بریک_ماسک": "0x0000fffff" ،
    "ملٹی ہش_کوڈ": 31
  دہ
ن
```

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
پرووائڈر ایڈورٹ بائیڈ وی 1 {
    8 رہنے کے بارے میں دن کے بولتے ہیں
    chunk_profile: profile_id (رجسٹری کے ذریعے مضمر)
    صلاحیتیں: [...]
دہ
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

گیٹ ویز باہمی تعاون یافتہ پروفائل (ڈیفالٹ `sorafs.sf1@1.0.0`) منتخب کریں
اور رسپانس ہیڈر `Content-Chunker` کے ذریعے فیصلے کی عکاسی کریں۔ منشور
منتخب کردہ پروفائل کو مربوط کریں تاکہ بہاو نوڈس حصوں کی ترتیب کو درست کرسکیں
HTTP مذاکرات پر بھروسہ کیے بغیر۔

### کار سپورٹ

ہم ایک CARV1+SHA-2 برآمدی راستہ رکھتے ہیں:

*** مین راہ ** - CARV2 ، BLAKE3 پے لوڈ ڈائجسٹ (`0x1f` ملٹی ہش) ،
  `MultihashIndexSorted` ، اوپر کی طرح محفوظ پروفائل محفوظ کیا گیا ہے۔
  جب مؤکل `Accept-Chunker` یا درخواستوں کو چھوڑ دیتا ہے تو اس مختلف حالت کو بے نقاب کرسکتا ہے
  `Accept-Digest: sha2-256`۔

منتقلی کے لئے اضافی لیکن کیننیکل ڈائجسٹ کو تبدیل نہیں کرنا چاہئے۔

### تعمیل

* پروفائل `sorafs.sf1@1.0.0` عوامی فکسچر کے مطابق ہے
  `fixtures/sorafs_chunker` اور کارپوریشنوں کے تحت رجسٹرڈ
  `fuzz/sorafs_chunker`۔ آخری سے آخر میں برابری کا استعمال زنگ ، گو اور نوڈ میں کیا جاتا ہے
  فراہم کردہ ٹیسٹوں کے ذریعے۔
* `chunker_registry::lookup_by_profile` نے زور دیا ہے کہ ڈسکرپٹر پیرامیٹرز
  حادثاتی تضادات سے بچنے کے لئے `ChunkProfile::DEFAULT` سے میچ کریں۔
* `iroha app sorafs toolkit pack` اور `sorafs_manifest_stub` کے ذریعہ تیار کردہ منشور میں رجسٹری میٹا ڈیٹا شامل ہے۔