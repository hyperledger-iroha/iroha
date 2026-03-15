---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: سجل المقطع
العنوان: سجل ملفات Chunker في SoraFS
Sidebar_label: سجل مقسم
الوصف: معرفات الملفات والمعلمات وشبكة الانترنت لسجل Chunker في SoraFS.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/chunker_registry.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

## سجل الملفات المقسمة في SoraFS (SF-2a)

التفاوض حزمة SoraFS على التحكم بالتقطيع عبر سجل صغير بفضاءات اسمية.
يُسند كل ملف معلمات CDC حتمية وبيانات سيمفر ووظيفة الملخص/الترميز المتعدد في البيانات المستخدمة ومحاولة شيفات CAR.

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
بالإضافة إلى البيانات المطلوبة مسبقًا وقائمة التحقق والموافقة على اقتراح إرسال أي رقم جديد.
وبعد أن تعتمد الـ أي تغيير، تابع
[قائمة اختر تخصيص السجل](./chunker-registry-rollout-checklist.md) و
[دليل البيان في التدريج](./staging-manifest-playbook) اختلاف
الـ التركيبات إلى الإنتاج والإنتاج.

### الملفات

| مساحة الاسم | الاسم | سيمفير | معرف الملف | الحد (بايت) | الهدف (بايت) | الحد الأقصى (بايت) | قناع القطع | مولتيهاش | البدائل | تعليقات |
|-----------|-------|--------|------------|-------------|-------------------|-----------|----------|---------|--|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | الملف المعتمد للمستخدم في التركيبات SF-1 |يعيش سجل في الشيفرة ضمن `sorafs_manifest::chunker_registry` (ويحكمه [`chunker_registry_charter.md`](./chunker-registry-charter.md)). ويعبَّر عن كل الحروف كـ `ChunkerProfileDescriptor` ويضم:

* `namespace` – تجميع منطقي للملفات ذات الصلة (مثل `sorafs`).
* `name` – تسمية مخفية للبشر (`sf1`, `sf1-fast`, …).
* `semver` – سلسلة نسخة دلالية المعلمات.
* `profile` – `ChunkProfile` الفعلي (الحد الأدنى/الهدف/الحد الأقصى/القناع).
* `multihash_code` – الـ multihash المستخدم لإنتاج Digests للـ Chunk (`0x1f`
  للإعداد الافتراضي في SoraFS).

يقوم بتسلسل الملفات عبر `ChunkingProfileV1`. فقدت بنية بيانات السجل (مساحة الاسم، الاسم، الفصل) إلى جزء
معلمات مراكز مكافحة الأمراض والوقاية منها (CDC) وقائمة المفضلة أعلاه. ينبغي للمستهلكين محاولة البحث في السجل عبر `profile_id`
والرجوع إلى المعلمات المتينة عند ظهور معرفات غير معروفة؛ تتضمن قائمة البدائل للعملاء HTTP في الإرسال
المقابض القديمة ضمن `Accept-Chunker` دون الإختبار. وتفرض شروط سجل السجل أن يكون المقبض مؤهلاً
(`namespace.name@semver`) الخط في `profile_aliases`، تليه أي بديل بديل.

لفحص السجل من القائمة، شغّل CLI المساعد:

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
`--por-sample-out`) المسار `-`، ما يبث الحمولة إلى stdout بدل إنشاء ملف. اتبع ذلك
إلى الأدوات مع وجود سلوك افتراضي للبيانات لطباعة التقرير الرئيسي.

### مصفوفة التوافق واسعة النطاقيلخص الجدول التالي حالة الدعم الحالية لـ `sorafs.sf1@1.0.0` عبر المكونات الأساسية. يشير "الجسر"
إلى مسار التوافق CARv1 + SHA-256 الذي يتطلب التفاوض صراحةً من العميل (`Accept-Chunker` + `Accept-Digest`).

| المكون | الحالة | تعليقات |
|--------|--------|---------|
| `sorafs_manifest_chunk_store` | ✅ مدعوم | إذا كان المفحوص مؤهلاً + بديلاً، ويستحق التقدير عبر `--json-out=-`، ويفرض ميثاق السجل عبر `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ قديم | مُنشئ البيان القديم؛ استخدم `iroha app sorafs toolkit pack` لتغليف CAR/manifest وأبقِ `--plan=-` إعادة التصديق الحتمية. |
| `sorafs_provider_advert_stub` | ⚠️ قديم | مساعد يتحقق غير متصل فقط؛ يجب إنتاج إعلانات مزود الخدمة عبر خط النشر والتحقق منها عبر `/v2/sorafs/providers`. |
| `sorafs_fetch` (منسق المطور) | ✅ مدعوم | تمت طباعة `chunk_fetch_specs` ويفهم قدرة تكتيكية `range` ويجمع من إخراج CARv2. |
| تركيبات للـ SDK (Rust/Go/TS) | ✅ مدعوم | يُعاد توليدها عبر `export_vectors`؛ المقبض سيظهر في كل قائمة استبدال ويعوقّ بواسطة أظرف المجلس. |
| التفاوض على ملفات البوابة Torii | ✅ مدعوم | يطبق كامل متطلبات `Accept-Chunker`، يفهم ترويسات `Content-Chunker`، ويعرض Bridge CARv1 فقط لطلبات الرجوع إلى إصدار أقدم من الصريحة. |

المخطط التليمي:- **تليمترية جلب الـ Chunks** — يصدر CLI الخاص بـ Iroha `sorafs toolkit pack` هضم للـ Chunk، وبيانات CAR، وجذور PoR لإدخالها في لوحات متابعة.
- **إعلانات الموفر** — تتضمن إعلانات تجارية بيانات القدرات والبدائل؛ تحقّق من التغطية عبر `/v2/sorafs/providers` (مثل وجود قدرة `range`).
- **مراقبة الـبوابة** — على إطلاق العنان للأرقام القياسية `Content-Chunker`/`Content-Digest` لمعرفة أي تخفيض غير متوقع؛ ومن تسبب أن ينخفض ​​استخدام الـ الجسر إلى الصفر الإيقاف.

يجب التوقف: بعد الاعتماد على العقدة الخلفية، نشر مزدوج (موثقة في المقترح) قبل التوقيع
`sorafs.sf1@1.0.0` فهو متوقف ضمن سجل السجل القديم Bridge CARv1 من بوابات الإنتاج.

لفحص شاهد PoR محدد، مؤشرات رئيسية القطعة/القطعة/الورقة واحفظ البرهان على القرص إذا لزم الأمر:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك اختيار الملف عبر المعرف الرقمي (`--profile-id=1`) أو عبر المقبض المسجل
(`--profile=sorafs.sf1@1.0.0`)؛ صيغة المقبض للسكربتات التي تمرر namespace/name/semver
مباشرة من بيانات الـ و.

استخدم `--promote-profile=<handle>` لإدخال كتلة JSON من البيانات الوصفية (بما في ذلك كل البدائل
لذلك) يمكن لصقها في `chunker_registry_data.rs` عند ترقية الملف الافتراضي الجديد:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف البرهان الاختياري) الـ ملخص تريجراي، وبايت الأصول المُعاينة
(مشفرة بالهيكس)، وملخص الأشقاء للـ مقطع/قطعة بحيث يمكن للمقرر إعادة هاش الطبقات
64 كيلو بايت/4 كيلو بايت مقابل القيمة `por_root_hex`.كونها برهان موجودة في مواجهة الحمولة، فهي تسير عبر المسار
`--por-proof-verify` (تضيف حقل CLI `"por_proof_verified": true` عندما يتطابق الشاهد مع RGB المحدد):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

لأخذ عينات على الدفعات، استخدم `--por-sample=<count>` وأكد أن البذور/مسار أخرج اختياري.
لضمان ترتيب CLI حتميًا (`splitmix64` المصنف) نريد بالاقتطاع بعد الآن
طلبات الاوراق المتاحة:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
تشغيل البضائع $ -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1،
    "مساحة الاسم": "سوراف"،
    "الاسم": "sf1"،
    "نصف": "1.0.0"،
    "المقبض": "sorafs.sf1@1.0.0"،
    "الحد الأدنى للحجم": 65536،
    "حجم_الهدف": 262144,
    "الحجم الأقصى": 524288،
    "break_mask": "0x0000ffff"،
    "كود_متعدد": 31
  }
]
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
موفرأدفرتبوديV1 {
    ...
    Chunk_profile: Profile_id (ضمنيًا عبر السجل)
    القدرات: [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

اختيار البوابات ملفًا مدعومًا من الطرفين (الافتراضي `sorafs.sf1@1.0.0`) ورفض تشجيع عبر ترويسة
الشكل `Content-Chunker`. بما في ذلك الملف المختار لكي يبدأ بعد ذلك من التحقق
من تخطيط الـ قطع دون الاعتماد على التفاوض HTTP.

### توافق CAR

تستخدم حزمة البيان المعتمدة أصل CIDv1 مع `dag-cbor` (`0x71`). ولتوافق نحتفظ القديم
بمسار تصدير CARv1+SHA-2 :* **المسار الأساسي** – CARv2، ملخص حمولة BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`، والملف مسجل كما سبق.
  هذا عندما يهمل العميل `Accept-Chunker` أو يطلب `Accept-Digest: sha2-256`.

لكن يجب ألا تستبدل الـ الملخص المختص.

### المطابقة

* يتوافق مع ملف `sorafs.sf1@1.0.0` الـ Installations العامة في
  `fixtures/sorafs_chunker` والمجموعات الأصلية تحت
  `fuzz/sorafs_chunker`. يتم اختبار التكافؤ الطرفي في Rust وGo وNode عبر المنطقة المتاحة.
* تؤكد `chunker_registry::lookup_by_profile` أن معلمات الوصف تطابق `ChunkProfile::DEFAULT` للحماية من الانحرافات العرضية.
*تتضمن البيانات الصادرة عن سجل بيانات `iroha app sorafs toolkit pack` و `sorafs_manifest_stub`.