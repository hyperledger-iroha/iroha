---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری
عنوان: SoraFS چنکر پروفائل رجسٹریشن
سائڈبار_لیبل: چنکر رجسٹریشن
تفصیل: SoraFS چنکر ریکارڈ کے لئے پروفائل IDs ، پیرامیٹرز اور مذاکرات کا منصوبہ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/chunker_registry.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات کا سیٹ ریٹائر نہیں ہوتا ہے تب تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

## SoraFS چنکر پروفائلز (SF-2A) کو رجسٹر کرنا

SoraFS اسٹیک ایک چھوٹے ، نام کی جگہ والے رجسٹر کا استعمال کرتے ہوئے chunking سلوک پر بات چیت کرتا ہے۔
ہر پروفائل ڈٹرمینسٹک سی ڈی سی پیرامیٹرز ، سیمور میٹا ڈیٹا ، اور متوقع ڈائجسٹ/ملٹی کوڈیک کو ظاہر کرتا ہے جو منشور اور کار فائلوں میں استعمال ہوتا ہے۔

پروفائل مصنفین سے مشورہ کرنا چاہئے
[`docs/source/sorafs/chunker_profile_authoring.md`] (./chunker-profile-authoring.md)
مطلوبہ میٹا ڈیٹا کے لئے ، نئی اندراجات جمع کروانے سے پہلے توثیق کی فہرست اور تجویز ٹیمپلیٹ۔
ایک بار جب گورننس کسی تبدیلی کی منظوری دے تو ،
[رجسٹری رول آؤٹ چیک لسٹ] (./chunker-registry-rollout-checklist.md) اور دی
[اسٹیجنگ میں پلے بوک مینی فیسٹ]] (./staging-manifest-playbook) فروغ دینے کے لئے
اسٹیجنگ اور پروڈکشن کے فکسچر۔

### پروفائلز

| نام کی جگہ | نام | semsee | پروفائل ID | منٹ (بائٹس) | ہدف (بائٹس) | زیادہ سے زیادہ (بائٹس) | کاٹنے کا ماسک | ملٹی ہش | عرف | نوٹ |
| -------- | -------- | -------- | --------------- | ------------- | ------------------ | ---------------- | --------- | ----------- | ------- | ------- |
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (Blake3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 فکسچر میں استعمال شدہ کیننیکل پروفائل |

ریکارڈ کوڈ میں `sorafs_manifest::chunker_registry` ([`chunker_registry_charter.md`] (./chunker-registry-charter.md)) کے طور پر رہتا ہے۔ ہر اندراج
اس کا اظہار `ChunkerProfileDescriptor` کے ساتھ کیا جاتا ہے:

* `namespace` - متعلقہ پروفائلز کی منطقی گروپ بندی (جیسے `sorafs`)۔
* `name` - انسانی پڑھنے کے قابل لیبل (`sf1` ، `sf1-fast` ،…)۔
* `semver` - پیرامیٹر سیٹ کے لئے سیمنٹک ورژن سٹرنگ۔
* `profile` - اصل `ChunkProfile` (کم سے کم/ہدف/زیادہ سے زیادہ/ماسک)۔
* `multihash_code` - ملٹی ہاش استعمال شدہ جب ڈائی ڈائجسٹس تیار کرتے وقت استعمال ہوتا ہے (`0x1f`
  SoraFS کے پہلے سے طے شدہ کے لئے)۔

`ChunkingProfileV1` کا استعمال کرتے ہوئے مینی فیسٹ پروفائلز کو سیریلائز کرتا ہے۔ ڈھانچے کے ریکارڈ
رجسٹری میٹا ڈیٹا (نام کی جگہ ، نام ، سیمور) کے ساتھ ساتھ سی ڈی سی پیرامیٹرز
خام اور مذکورہ بالا عرفی خطوط کی فہرست۔ صارفین کو کوشش کرنی چاہئے a
`profile_id` کے لئے رجسٹری تلاش کریں اور جب ان لائن پیرامیٹرز میں واپس گریں جب
نامعلوم IDs ظاہر ؛ الیاس کی فہرست اس بات کو یقینی بناتی ہے کہ HTTP کلائنٹ کر سکتے ہیں
`Accept-Chunker` میں میرا اندازہ لگائے بغیر جاری رکھیں۔ کے قواعد
رجسٹری لیٹر کا تقاضا ہے کہ کیننیکل ہینڈل (`namespace.name@semver`) ہو
`profile_aliases` میں پہلی اندراج ، اس کے بعد کسی بھی وراثت میں ملنے والے عرفی نام ہیں۔

ٹولنگ سے رجسٹری کا معائنہ کرنے کے لئے ، سی ایل آئی مددگار چلائیں:

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
```تمام CLI جھنڈے جو JSON لکھتے ہیں (`--json-out` ، `--por-json-out` ، `--por-proof-out` ،
`--por-sample-out`) `-` کو راستے کے طور پر قبول کریں ، جو پے لوڈ کو اس کے بجائے stdout پر منتقل کرتا ہے
ایک فائل بنائیں۔ اس سے ڈیٹا کو برقرار رکھنے کے دوران ڈیٹا کو ٹولنگ میں ڈالنا آسان ہوجاتا ہے
مرکزی رپورٹ پرنٹنگ کا پہلے سے طے شدہ سلوک۔

### رول آؤٹ میٹرکس اور تعیناتی کا منصوبہ


مندرجہ ذیل ٹیبل `sorafs.sf1@1.0.0` IN کے لئے حمایت کی موجودہ حیثیت حاصل کرتا ہے
اہم اجزاء۔ "برج" سے مراد CARV1 + SHA-256 لین ہے
جس کے لئے مؤکل (`Accept-Chunker` + `Accept-Digest`) سے واضح مذاکرات کی ضرورت ہے۔

| اجزاء | حیثیت | نوٹ |
| ----------- | -------- | ------- |
| `sorafs_manifest_chunk_store` | ✅ سپورٹ | کیننیکل ہینڈل + عرف کی توثیق کرتا ہے ، Iroha کے ذریعے رپورٹوں کو منتقل کرتا ہے اور `ensure_charter_compliance()` کے ساتھ رجسٹری لیٹر کا اطلاق کرتا ہے۔ |
| `sorafs_manifest_stub` | ⚠ ریٹائرڈ | سپورٹ سے باہر منشور تعمیر کنندہ ؛ کار/مینی فیسٹ پیکیجنگ کے لئے `iroha app sorafs toolkit pack` استعمال کریں اور A18NI00000059x کو عین مطابق تعی .ن کے ل. رکھیں۔ |
| `sorafs_provider_advert_stub` | ⚠ ریٹائرڈ | صرف آف لائن توثیق مددگار ؛ فراہم کنندہ اشتہارات کو اشاعت پائپ لائن کے ذریعہ تیار کیا جانا چاہئے اور `/v2/sorafs/providers` کے ذریعے توثیق کی جانی چاہئے۔ |
| `sorafs_fetch` (ڈویلپر آرکسٹریٹر) | ✅ سپورٹ | `chunk_fetch_specs` کو پڑھتا ہے ، `range` صلاحیت پے لوڈ کو سمجھتا ہے ، اور CARV2 آؤٹ پٹ کو جمع کرتا ہے۔ |
| ایس ڈی کے فکسچر (زنگ/گو/ٹی ایس) | ✅ سپورٹ | `export_vectors` کے ذریعے دوبارہ پیدا ہوا ؛ کیننیکل ہینڈل ہر عرف کی فہرست میں پہلے ظاہر ہوتا ہے اور کونسل کے لفافوں کے ذریعہ اس پر دستخط ہوتے ہیں۔ |
| گیٹ وے میں پروفائل مذاکرات Torii | ✅ سپورٹ | مکمل `Accept-Chunker` گرائمر کو نافذ کرتا ہے ، `Content-Chunker` ہیڈر شامل ہے ، اور صرف واضح ڈاون گریڈ درخواستوں میں CARV1 پل کو بے نقاب کرتا ہے۔ |

ٹیلی میٹری کی تعیناتی:

۔
- ** فراہم کنندہ اشتہارات ** - اشتہارات پے لوڈ میں صلاحیتیں اور عرفی میٹا ڈیٹا شامل ہیں۔ `/v2/sorafs/providers` (جیسے ، صلاحیت کی موجودگی `range`) کے ذریعے کوریج کی توثیق کرتا ہے۔
۔ توقع کی جاتی ہے کہ فرسودگی سے قبل پل کے استعمال سے صفر ہوجائے گا۔

فرسودگی کی پالیسی: ایک بار جب کسی جانشین کی تصدیق ہوجاتی ہے تو ، دوہری پوسٹنگ ونڈو کا شیڈول بنائیں
(تجویز میں دستاویزی کردہ) `sorafs.sf1@1.0.0` کو نشان زد کرنے سے پہلے رجسٹری میں فرسودہ اور ر کو ہٹانے سے پہلے
پیداوار میں گیٹ ویز کا کاروی 1 پل۔

کسی مخصوص پور ٹوکن کا معائنہ کرنے کے لئے ، حصہ/طبقہ/پتی اشاریہ اور اختیاری طور پر فراہم کریں
ڈسک ٹیسٹ برقرار ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

آپ عددی ID (`--profile-id=1`) کے ذریعہ یا رجسٹریشن ہینڈل کے ذریعہ ایک پروفائل منتخب کرسکتے ہیں
(`--profile=sorafs.sf1@1.0.0`) ؛ ہینڈل فارم اسکرپٹ کے لئے آسان ہے
گورننس میٹا ڈیٹا سے براہ راست نام کی جگہ/نام/سیمور پاس کریں۔میٹا ڈیٹا کا JSON بلاک (تمام عرفی ناموں سمیت JSON بلاک کو خارج کرنے کے لئے `--promote-profile=<handle>` کا استعمال کریں
رجسٹرڈ) جو نئے ڈیفالٹ پروفائل کو فروغ دیتے وقت `chunker_registry_data.rs` میں پھنس سکتا ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

مرکزی رپورٹ (اور اختیاری ٹیسٹ فائل) میں روٹ ڈائجسٹ ، نمونے والے پتے بائٹس شامل ہیں
.
`por_root_hex` کی قیمت کے خلاف 64 KIB/4 KIB پرتیں۔

پے لوڈ کے خلاف موجودہ ٹیسٹ کی توثیق کرنے کے لئے ، راستے کو راستے سے گزریں
`--por-proof-verify` (CLI `"por_proof_verified": true` کو شامل کرتا ہے جب انتباہی روشنی
حساب کتاب کی جڑ سے میچ کرتا ہے):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

بیچ کے نمونے لینے کے ل i ، `--por-sample=<count>` استعمال کریں اور اختیاری طور پر بیج/آؤٹ پٹ راستہ فراہم کریں۔
سی ایل آئی ایک عین مطابق ترتیب کی ضمانت دیتا ہے (`splitmix64` کے ساتھ مل کر) اور جب شفافیت سے چھوٹا جائے گا جب
درخواست دستیاب چادروں سے زیادہ ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

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

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
پرووائڈر ایڈورٹ بائیڈ وی 1 {
    8 رہنے کے بارے میں دن کے بولتے ہیں
    chunk_profile: profile_id (رجسٹری کے ذریعے مضمر)
    صلاحیتیں: [...]
دہ
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```

گیٹ وے باہمی تعاون یافتہ پروفائل منتخب کریں (ڈیفالٹ `sorafs.sf1@1.0.0`)
اور رسپانس ہیڈر `Content-Chunker` کے ذریعے فیصلے کی عکاسی کریں۔ منشور
منتخب کردہ پروفائل کو سرایت کریں تاکہ بہاو والے نوڈس حصے کی ترتیب کو توثیق کرسکیں
HTTP مذاکرات پر انحصار کیے بغیر۔

### کار سپورٹ

ہم ایک CARV1+SHA-2 برآمدی راستہ برقرار رکھیں گے:

*** پرائمری روٹ ** - CARV2 ، BLAKE3 پے لوڈ ڈائجسٹ (`0x1f` ملٹی ہش) ،
  `MultihashIndexSorted` ، اوپر کے طور پر رجسٹرڈ ہے۔
  جب وہ مؤکل `Accept-Chunker` یا درخواستوں کو چھوڑ دیتا ہے تو وہ اس مختلف حالت کو بے نقاب کرسکتے ہیں
  `Accept-Digest: sha2-256`۔

منتقلی کے لئے اضافی لیکن کیننیکل ڈائجسٹ کو تبدیل نہیں کرنا چاہئے۔

### تعمیل

* پروفائل `sorafs.sf1@1.0.0` میں عوامی فکسچر کو تفویض کیا گیا ہے
  `fixtures/sorafs_chunker` اور کارپوریشنز میں رجسٹرڈ
  `fuzz/sorafs_chunker`۔ آخری سے آخر میں برابری کا استعمال زنگ ، گو اور نوڈ میں کیا جاتا ہے
  فراہم کردہ شواہد کے ذریعے۔
* `chunker_registry::lookup_by_profile` نے زور دیا ہے کہ ڈسکرپٹر پیرامیٹرز
  حادثاتی تفریق سے بچنے کے لئے `ChunkProfile::DEFAULT` سے میچ کریں۔
* `iroha app sorafs toolkit pack` اور `sorafs_manifest_stub` کے ذریعہ تیار کردہ منشور میں ریکارڈ میٹا ڈیٹا شامل ہے۔