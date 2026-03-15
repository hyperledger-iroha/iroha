---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d9b7606a4cd550ee4c4a4231d75e8a3a0d75d593341965e10fc7d9e0316110c
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/chunker_registry.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

## سجل ملفات chunker في SoraFS (SF-2a)

تتفاوض حزمة SoraFS على سلوك chunking عبر سجل صغير بفضاءات اسمية.
يُسند كل ملف معلمات CDC حتمية وبيانات semver ووظيفة digest/multicodec المتوقعة المستخدمة في manifests وأرشيفات CAR.

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
للاطلاع على البيانات المطلوبة وقائمة التحقق وقالب المقترح قبل إرسال أي إدخال جديد.
وبعد أن تعتمد الحوكمة أي تغيير، اتبع
[قائمة تحقق إطلاق السجل](./chunker-registry-rollout-checklist.md) و
[دليل manifest في staging](./staging-manifest-playbook) لترقية
الـ fixtures إلى staging والإنتاج.

### الملفات

| Namespace | الاسم | SemVer | معرف الملف | الحد الأدنى (بايت) | الهدف (بايت) | الحد الأقصى (بايت) | قناع القطع | Multihash | البدائل | ملاحظات |
|-----------|-------|--------|------------|--------------------|--------------|--------------------|-----------|-----------|--------|---------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | الملف المعتمد المستخدم في fixtures SF-1 |

يعيش السجل في الشيفرة ضمن `sorafs_manifest::chunker_registry` (ويحكمه [`chunker_registry_charter.md`](./chunker-registry-charter.md)). ويُعبَّر عن كل إدخال كـ `ChunkerProfileDescriptor` يضم:

* `namespace` – تجميع منطقي للملفات ذات الصلة (مثل `sorafs`).
* `name` – تسمية مقروءة للبشر (`sf1`, `sf1-fast`, …).
* `semver` – سلسلة نسخة دلالية لمجموعة المعلمات.
* `profile` – `ChunkProfile` الفعلي (min/target/max/mask).
* `multihash_code` – الـ multihash المستخدم لإنتاج digests للـ chunk (`0x1f`
  للإعداد الافتراضي في SoraFS).

يقوم manifest بتسلسل الملفات عبر `ChunkingProfileV1`. تسجل البنية بيانات السجل (namespace, name, semver) إلى جانب
معلمات CDC الخام وقائمة البدائل أعلاه. ينبغي على المستهلكين أولاً محاولة البحث في السجل عبر `profile_id`
والرجوع إلى المعلمات المضمّنة عند ظهور معرفات غير معروفة؛ تضمن قائمة البدائل استمرار العملاء HTTP في إرسال
المقابض القديمة ضمن `Accept-Chunker` دون تخمين. وتفرض قواعد ميثاق السجل أن يكون المقبض المعتمد
(`namespace.name@semver`) أول إدخال في `profile_aliases`، تليه أي بدائل قديمة.

لفحص السجل من الأدوات، شغّل CLI المساعد:

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
```

تقبل كل أعلام CLI التي تكتب JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) المسار `-`، ما يبث الحمولة إلى stdout بدل إنشاء ملف. يسهل ذلك تمرير
البيانات إلى الأدوات مع الحفاظ على السلوك الافتراضي لطباعة التقرير الرئيسي.

### مصفوفة التوافق وخطة الإطلاق


يلخص الجدول التالي حالة الدعم الحالية لـ `sorafs.sf1@1.0.0` عبر المكونات الأساسية. يشير "Bridge"
إلى مسار التوافق CARv1 + SHA-256 الذي يتطلب تفاوضًا صريحًا من العميل (`Accept-Chunker` + `Accept-Digest`).

| المكون | الحالة | ملاحظات |
|--------|--------|---------|
| `sorafs_manifest_chunk_store` | ✅ مدعوم | يتحقق من المقبض المعتمد + البدائل، ويبث التقارير عبر `--json-out=-`، ويفرض ميثاق السجل عبر `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ قديم | مُنشئ manifest قديم؛ استخدم `iroha app sorafs toolkit pack` لتغليف CAR/manifest وأبقِ `--plan=-` لإعادة التحقق الحتمية. |
| `sorafs_provider_advert_stub` | ⚠️ قديم | مساعد تحقق offline فقط؛ يجب إنتاج provider adverts عبر خط أنابيب النشر والتحقق منها عبر `/v2/sorafs/providers`. |
| `sorafs_fetch` (developer orchestrator) | ✅ مدعوم | يقرأ `chunk_fetch_specs` ويفهم حمولة قدرة `range` ويجمع إخراج CARv2. |
| Fixtures للـ SDK (Rust/Go/TS) | ✅ مدعوم | يُعاد توليدها عبر `export_vectors`؛ المقبض المعتمد يظهر أولاً في كل قائمة بدائل ويُوقَّع بواسطة أظرف المجلس. |
| تفاوض ملفات gateway Torii | ✅ مدعوم | يطبق كامل قواعد `Accept-Chunker`، ويتضمن ترويسات `Content-Chunker`، ويعرض Bridge CARv1 فقط لطلبات downgrade الصريحة. |

طرح التليمترية:

- **تليمترية جلب الـ chunks** — يصدر CLI الخاص بـ Iroha `sorafs toolkit pack` digests للـ chunk، وبيانات CAR، وجذور PoR لإدخالها في لوحات المتابعة.
- **Provider adverts** — تتضمن حمولة الإعلانات بيانات القدرات والبدائل؛ تحقّق من التغطية عبر `/v2/sorafs/providers` (مثل وجود قدرة `range`).
- **مراقبة الـ gateway** — على المشغلين الإبلاغ عن أزواج `Content-Chunker`/`Content-Digest` لاكتشاف أي خفض غير متوقع؛ ومن المتوقع أن ينخفض استخدام الـ bridge إلى الصفر قبل الإيقاف.

سياسة الإيقاف: بعد اعتماد ملف خلف، حدّد نافذة نشر مزدوجة (موثقة في المقترح) قبل وسم
`sorafs.sf1@1.0.0` بأنه متوقف ضمن السجل وإزالة Bridge CARv1 من بوابات الإنتاج.

لفحص شاهد PoR محدد، قدم مؤشرات chunk/segment/leaf واحفظ البرهان على القرص إذا رغبت:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك اختيار ملف عبر معرف رقمي (`--profile-id=1`) أو عبر المقبض المسجل
(`--profile=sorafs.sf1@1.0.0`)؛ صيغة المقبض مناسبة للسكريبتات التي تمرر namespace/name/semver
مباشرة من بيانات الحوكمة.

استخدم `--promote-profile=<handle>` لإخراج كتلة JSON من البيانات الوصفية (بما في ذلك كل البدائل
المسجلة) يمكن لصقها في `chunker_registry_data.rs` عند ترقية ملف افتراضي جديد:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف البرهان الاختياري) الـ digest الجذري، وbytes الأوراق المُعاينة
(مشفرة بالهيكس)، وdigests الأشقاء للـ segment/chunk بحيث يمكن للمدققين إعادة هاش طبقات
64 KiB/4 KiB مقابل قيمة `por_root_hex`.

للتحقق من برهان موجود مقابل حمولة، مرر المسار عبر
`--por-proof-verify` (تضيف CLI الحقل `"por_proof_verified": true` عندما يتطابق الشاهد مع الجذر المحسوب):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

لأخذ عينات على دفعات، استخدم `--por-sample=<count>` ويمكنك تمرير seed/مسار إخراج اختياري.
تضمن CLI ترتيبًا حتميًا (`splitmix64` seeded) وتقوم بالاقتطاع تلقائيًا عندما تتجاوز
الطلبات الأوراق المتاحة:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (ضمنيًا عبر السجل)
    capabilities: [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

تختار البوابات ملفًا مدعومًا من الطرفين (الافتراضي `sorafs.sf1@1.0.0`) وتعكس القرار عبر ترويسة
الاستجابة `Content-Chunker`. تضمن manifests الملف المختار كي تتمكن العقد اللاحقة من التحقق
من تخطيط الـ chunks دون الاعتماد على تفاوض HTTP.

### توافق CAR

تستخدم حزمة manifest المعتمدة جذور CIDv1 مع `dag-cbor` (`0x71`). وللتوافق القديم نحتفظ
بمسار تصدير CARv1+SHA-2:

* **المسار الأساسي** – CARv2، digest حمولة BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`، والملف مسجل كما سبق.
  هذا الخيار عندما يهمل العميل `Accept-Chunker` أو يطلب `Accept-Digest: sha2-256`.

لكن يجب ألا تستبدل الـ digest المعتمد.

### المطابقة

* يطابق ملف `sorafs.sf1@1.0.0` الـ fixtures العامة في
  `fixtures/sorafs_chunker` والمجموعات المسجلة تحت
  `fuzz/sorafs_chunker`. يتم اختبار التكافؤ الطرفي في Rust وGo وNode عبر الاختبارات المتاحة.
* تؤكد `chunker_registry::lookup_by_profile` أن معلمات الوصف تطابق `ChunkProfile::DEFAULT` للحماية من الانحرافات العرضية.
* تتضمن manifests الناتجة عن `iroha app sorafs toolkit pack` و `sorafs_manifest_stub` بيانات السجل.
