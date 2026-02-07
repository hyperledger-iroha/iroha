---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری
عنوان: SoraFS چنکر پروفائل رجسٹری
سائڈبار_لیبل: چنکر رجسٹریشن
تفصیل: SoraFS چنکر رجسٹری کے لئے پروفائل IDs ، پیرامیٹرز اور تجارتی منصوبہ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/chunker_registry.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

## SoraFS (SF-2A) چنکر پروفائلز کی رجسٹریشن

SoraFS اسٹیک ایک چھوٹے نام کی جگہ والے رجسٹر کے ذریعہ chunking سلوک پر بات چیت کرتا ہے۔
ہر پروفائل ڈٹرمینسٹک سی ڈی سی پیرامیٹرز ، سیمور میٹا ڈیٹا ، اور متوقع ڈائجسٹ/ملٹی کوڈیک کو ظاہر کرتا ہے جو منشور اور کار فائلوں میں استعمال ہوتا ہے۔

پروفائل مصنفین سے مشورہ کرنا چاہئے
[`docs/source/sorafs/chunker_profile_authoring.md`] (./chunker-profile-authoring.md)
مطلوبہ میٹا ڈیٹا کے لئے ، نئی اندراجات جمع کروانے سے پہلے توثیق کی چیک لسٹ اور تجویز ٹیمپلیٹ۔
ایک بار جب گورننس کسی تبدیلی کی منظوری دے تو اس کی پیروی کریں
[رجسٹری رول آؤٹ چیک لسٹ] (./chunker-registry-rollout-checklist.md) اور دی
[اسٹیجنگ مینی فیسٹ پلے بک] (./staging-manifest-playbook) فروغ دینے کے لئے
اسٹیجنگ اور پروڈکشن کے لئے فکسچر۔

### پروفائلز

| نام کی جگہ | نام | سیمور | پروفائل ID | منٹ (بائٹس) | ہدف (بائٹس) | زیادہ سے زیادہ (بائٹس) | بریکنگ کاجل | ملٹی ہش | عرفیوس | نوٹ |
| ----------- | ------ | -------- | --------------- | ------------- | ------------------ | ------------------ | ---------------------- | ---------------------------------------------------- |
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (Blake3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 فکسچر میں استعمال شدہ کیننیکل پروفائل |

ریکارڈ `sorafs_manifest::chunker_registry` ([`chunker_registry_charter.md`] (./chunker-registry-charter.md) کے طور پر کوڈ میں رہتا ہے۔ ہر اندراج
اور `ChunkerProfileDescriptor` کے ساتھ اظہار کیا:

* `namespace` - متعلقہ پروفائلز کی منطقی گروپ بندی (جیسے ، `sorafs`)۔
* `name` - انسانی پڑھنے کے قابل لیبل (`sf1` ، `sf1-fast` ، ...)۔
* `semver` - پیرامیٹر سیٹ کے لئے سیمنٹک ورژن سٹرنگ۔
* `profile` - اصل `ChunkProfile` (کم سے کم/ہدف/زیادہ سے زیادہ/ماسک)۔
* `multihash_code` - ملٹی ہاش استعمال کیا جاتا ہے جب حصہ ڈائجسٹس تیار کرتے وقت (`0x1f`
  SoraFS کے پہلے سے طے شدہ)۔

`ChunkingProfileV1` کے ذریعے مینی فیسٹ پروفائلز کو سیریلائز کرتا ہے۔ فریم ورک نے میٹا ڈیٹا ریکارڈ کیا
 خام سی ڈی سی پیرامیٹرز کے ساتھ رجسٹری (نام کی جگہ ، نام ، سیمور) کی
اور مذکورہ بالا عرفی خطوط کی فہرست۔ صارفین کو پہلے کوشش کرنی چاہئے a
`profile_id` کے لئے رجسٹری تلاش کریں اور جب ان لائن پیرامیٹرز کا استعمال کریں
نامعلوم IDs ظاہر ؛ عرفی کی فہرست اس بات کو یقینی بناتی ہے کہ HTTP کلائنٹ کر سکتے ہیں
`Accept-Chunker` میں بغیر اندازہ کیے متبادل ہینڈلز بھیجنا جاری رکھیں۔ کے قواعد
رجسٹری چارٹر کا تقاضا ہے کہ کیننیکل ہینڈل (`namespace.name@semver`) ہے
`profile_aliases` میں پہلی اندراج ، اس کے بعد کوئی متبادل عرفی نام کی گئی۔

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
`--por-sample-out`) `-` کو بطور راستہ قبول کریں ، جو پے لوڈ کو اس کے بجائے stdout پر منتقل کرتا ہے
ایک فائل بنائیں۔ اس سے برقرار رکھنے کے دوران ٹولنگ کے ل data ڈیٹا کو زنجیر بنانا آسان ہوجاتا ہے
مرکزی رپورٹ پرنٹنگ کا پہلے سے طے شدہ سلوک۔

### رول آؤٹ میٹرکس اور تعیناتی کا منصوبہ


نیچے دیئے گئے جدول میں `sorafs.sf1@1.0.0` in کے لئے موجودہ سپورٹ کی حیثیت حاصل کی گئی ہے
اہم اجزاء۔ "برج" سے مراد CARV1 + SHA-256 رینج ہے
جس کے لئے گاہک (`Accept-Chunker` + `Accept-Digest`) کے ذریعہ واضح مذاکرات کی ضرورت ہے۔

| اجزاء | حیثیت | نوٹ |
| ----------- | -------- | ------- |
| `sorafs_manifest_chunk_store` | ✅ سپورٹ | کیننیکل ہینڈل + عرفی ناموں کی توثیق کرتا ہے ، Iroha کے ذریعے رپورٹس کی رپورٹس کرتا ہے اور `ensure_charter_compliance()` کے ذریعے رجسٹری چارٹر کا اطلاق کرتا ہے۔ |
| `sorafs_manifest_stub` | ⚠ ہٹا دیا گیا | سپورٹ سے باہر منشور بلڈر ؛ کار/مینی فیسٹ پیکیجنگ کے لئے `iroha app sorafs toolkit pack` استعمال کریں اور A18NI00000059x کو عین مطابق تعی .ن کے ل. رکھیں۔ |
| `sorafs_provider_advert_stub` | ⚠ ہٹا دیا گیا | صرف آف لائن توثیق مددگار ؛ فراہم کنندہ اشتہارات کو پبلشنگ پائپ لائن کے ذریعہ تیار کیا جانا چاہئے اور `/v1/sorafs/providers` کے ذریعے توثیق کیا جانا چاہئے۔ |
| `sorafs_fetch` (ڈویلپر آرکسٹریٹر) | ✅ سپورٹ | LE `chunk_fetch_specs` ، صلاحیت کے پے لوڈ کو سمجھتا ہے `range` اور MUNTS CARV2 آؤٹ پٹ۔ |
| ایس ڈی کے فکسچر (زنگ/گو/ٹی ایس) | ✅ سپورٹ | `export_vectors` کے ذریعے دوبارہ پیدا ہوا ؛ کیننیکل ہینڈل عرفی ناموں کی ہر فہرست میں پہلے ظاہر ہوتا ہے اور بورڈ کے لفافوں کے ذریعہ اس پر دستخط ہوتے ہیں۔ |
| گیٹ وے پر پروفائل مذاکرات Torii | ✅ سپورٹ | `Accept-Chunker` کے مکمل گرائمر کو نافذ کرتا ہے ، `Content-Chunker` ہیڈر شامل ہے ، اور صرف واضح ڈاون گریڈ درخواستوں پر CARV1 پل کو بے نقاب کرتا ہے۔ |

ٹیلی میٹری رول آؤٹ:

۔
- ** فراہم کنندہ اشتہارات ** - اشتہاری پے لوڈ میں قابلیت کا میٹا ڈیٹا اور عرفیت شامل ہیں۔ `/v1/sorafs/providers` (جیسے ، صلاحیت کی موجودگی `range`) کے ذریعے کوریج کی توثیق کریں۔
۔ توقع کی جاتی ہے کہ فرسودگی سے قبل پل کے استعمال سے صفر ہوجائے گا۔

فرسودگی کی پالیسی: ایک بار جب جانشین کی پروفائل کی توثیق ہوجائے تو ، ڈبل پبلشنگ ونڈو کا شیڈول بنائیں
پیداوار میں گیٹ ویز کا کاروی 1 پل۔

کسی مخصوص پور ٹوکن کا معائنہ کرنے کے لئے ، حصہ/طبقہ/پتی کے اشاریے اور اختیاری طور پر فراہم کریں
ڈسک پر ثبوت جاری رکھیں:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

آپ عددی ID (`--profile-id=1`) کے ذریعہ یا رجسٹری ہینڈل کے ذریعہ ایک پروفائل منتخب کرسکتے ہیں
(`--profile=sorafs.sf1@1.0.0`) ؛ ہینڈل فارم اسکرپٹ کے لئے آسان ہے
سلسلہ نام کی جگہ/نام/سیمور براہ راست گورننس میٹا ڈیٹا سے۔

میٹا ڈیٹا کے JSON بلاک کو آؤٹ پٹ کرنے کے لئے `--promote-profile=<handle>` استعمال کریں (تمام عرفی ناموں سمیت
رجسٹرڈ) جس کو نئے ڈیفالٹ پروفائل کو فروغ دیتے وقت `chunker_registry_data.rs` میں چسپاں کیا جاسکتا ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```مرکزی رپورٹ (اور اختیاری پروف فائل) میں روٹ ڈائجسٹ ، نمونے والے پتے بائٹس شامل ہیں
(ہیکس انکوڈڈ) اور بہن طبقہ/حصہ ڈائجسٹس تاکہ تصدیق کرنے والے
`por_root_hex` کی قیمت کے خلاف 64 KIB/4 KIB پرتوں کی ہیش کو دوبارہ گنتی کریں۔

کسی پے لوڈ کے خلاف موجودہ ثبوت کی توثیق کرنے کے لئے ، راستے کو راستے سے گزرنا
`--por-proof-verify` (CLI `"por_proof_verified": true` کو شامل کرتا ہے جب کور
حساب کتاب کی جڑ سے مطابقت رکھتا ہے):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

بیچ کے نمونے لینے کے ل i ، `--por-sample=<count>` استعمال کریں اور اختیاری طور پر بیج/آؤٹ پٹ راستہ فراہم کریں۔
سی ایل آئی ڈٹرمینسٹک آرڈرنگ کی ضمانت دیتا ہے (`splitmix64` کے ساتھ بیج) اور جب خود بخود تراش جائے گا
درخواست دستیاب چادروں سے زیادہ ہے:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

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

O campo `handle` (`namespace.name@semver`) corresponde ao que os CLIs aceitam via
`--profile=...`, tornando seguro copiar direto para automacao.

### Negociar chunkers

Gateways e clientes anunciam perfis suportados via provider adverts:

```
پرووائڈر ایڈورٹ بائیڈ وی 1 {
    8 رہنے کے بارے میں دن کے بولتے ہیں
    chunk_profile: profile_id (رجسٹری کے ذریعے مضمر)
    صلاحیتیں: [...]
دہ
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```

گیٹ وے باہمی تعاون یافتہ پروفائل منتخب کریں (ڈیفالٹ `sorafs.sf1@1.0.0`)
اور رسپانس ہیڈر `Content-Chunker` کے ذریعے فیصلے کی عکاسی کریں۔ ظاہر
منتخب کردہ پروفائل کو سرایت کریں تاکہ بہاو والے صارفین حصہ لے آؤٹ کی توثیق کرسکیں
HTTP مذاکرات پر بھروسہ کیے بغیر۔

### کار سپورٹ

ہم ایک CARV1+SHA-2 برآمدی راستہ برقرار رکھتے ہیں:

*** بنیادی راستہ ** - CARV2 ، BLAKE3 پے لوڈ ڈائجسٹ (`0x1f` ملٹی ہش) ،
  `MultihashIndexSorted` ، اوپر کے طور پر رجسٹرڈ ہے۔
  جب مؤکل `Accept-Chunker` یا درخواستوں کو چھوڑ دیتا ہے تو اس مختلف حالت کو بے نقاب کرسکتا ہے
  `Accept-Digest: sha2-256`۔

منتقلی کے لئے اضافی ، لیکن کیننیکل ڈائجسٹ کو تبدیل نہیں کرنا چاہئے۔

### تعمیل

* `sorafs.sf1@1.0.0` پروفائل میں عوامی فکسچر میں نقشہ تیار کرتا ہے
  `fixtures/sorafs_chunker` اور کارپورا میں رجسٹرڈ
  `fuzz/sorafs_chunker`۔ آخری سے آخر میں برابری کا استعمال زنگ ، گو اور نوڈ میں کیا جاتا ہے
  فراہم کردہ ٹیسٹوں کے ذریعے۔
* `chunker_registry::lookup_by_profile` بیان کرتا ہے کہ ڈسکرپٹر پیرامیٹرز
  حادثاتی تغیر سے بچنے کے لئے `ChunkProfile::DEFAULT` سے مطابقت رکھتا ہے۔
* `iroha app sorafs toolkit pack` اور `sorafs_manifest_stub` کے ذریعہ تیار کردہ منشور میں ریکارڈ میٹا ڈیٹا شامل ہے۔