---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS التقطيع → خط أنابيب البيان

يتتبع هذا المصاحب للبدء السريع خط الأنابيب الشامل الذي يتحول إلى الخام
البايتات في Norito تظهر بشكل مناسب لـ SoraFS Pin Registry. المحتوى هو
مقتبس من [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)؛
راجع هذا المستند لمعرفة المواصفات الأساسية وسجل التغيير.

## 1. قطعة قطعية

يستخدم SoraFS ملف التعريف SF-1 (`sorafs.sf1@1.0.0`): ملف تعريف مستوحى من FastCDC
تجزئة مع الحد الأدنى لحجم القطعة 64 كيلو بايت، والهدف 256 كيلو بايت، والحد الأقصى 512 كيلو بايت، و
قناع الكسر `0x0000ffff`. تم تسجيل الملف الشخصي في
`sorafs_manifest::chunker_registry`.

### مساعدين الصدأ

- `sorafs_car::CarBuildPlan::single_file` – يصدر إزاحات وأطوال و
  يتم هضم BLAKE3 أثناء إعداد البيانات التعريفية لـ CAR.
- `sorafs_car::ChunkStore` - تدفق الحمولات، وبيانات تعريف القطعة المستمرة، و
  يشتق شجرة عينات إثبات الاسترجاع (PoR) بقدرة 64 كيلو بايت / 4 كيلو بايت.
- `sorafs_chunker::chunk_bytes_with_digests` - مساعد المكتبة خلف كلا سطر الأوامر.

### أدوات سطر الأوامر

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يحتوي JSON على الإزاحات والأطوال وملخصات القطع المطلوبة. الإصرار على
التخطيط عند إنشاء البيانات أو جلب المنسق للمواصفات.

### شهود PoR

يعرض `ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` و
`--por-sample=<count>` حتى يتمكن المدققون من طلب مجموعات الشهود الحتمية. زوج
تلك العلامات ذات `--por-proof-out` أو `--por-sample-out` لتسجيل JSON.

## 2. قم بلف البيان

يجمع `ManifestBuilder` بيانات تعريف المجموعة مع مرفقات الإدارة:

- التزامات الجذر CID (dag-cbor) وCAR.
- البراهين الاسم المستعار ومطالبات قدرة المزود.
- توقيعات المجلس والبيانات التعريفية الاختيارية (على سبيل المثال، معرفات البناء).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

مخرجات مهمة:

- `payload.manifest` - بايت البيان المشفر Norito.
- `payload.report.json` – ملخص قابل للقراءة من قبل الإنسان/الأتمتة، بما في ذلك
  `chunk_fetch_specs`، `payload_digest_hex`، ملخصات CAR، وبيانات تعريف الاسم المستعار.
- `payload.manifest_signatures.json` - مظروف يحتوي على البيان BLAKE3
  ملخص، ملخص SHA3 للخطة المقطوعة، وتوقيعات Ed25519 المصنفة.

استخدم `--manifest-signatures-in` للتحقق من المغلفات المقدمة من الخارج
الموقعون قبل كتابتهم مرة أخرى، و`--chunker-profile-id` أو
`--chunker-profile=<handle>` لقفل اختيار التسجيل.

## 3. النشر والتثبيت

1. **تقديم الحوكمة** – تقديم ملخص البيان والتوقيع
   مظروف إلى المجلس حتى يمكن قبول الدبوس. ينبغي للمدققين الخارجيين
   قم بتخزين ملخص SHA3 الخاص بخطة القطع جنبًا إلى جنب مع ملخص البيان.
2. **تثبيت الحمولات** - تحميل أرشيف CAR (وفهرس CAR الاختياري) المشار إليه
   في البيان إلى Pin Registry. تأكد من مشاركة البيان وCAR
   نفس الجذر CID.
3. **تسجيل القياس عن بعد** – استمر في تقرير JSON وشهود إثبات صحة البيانات وأي عملية جلب
   المقاييس في القطع الأثرية الإصدار. تغذي هذه السجلات لوحات معلومات المشغل و
   المساعدة في إعادة إنتاج المشكلات دون تنزيل حمولات كبيرة.

## 4. محاكاة جلب متعددة الموفرين

`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- يزيد `#<concurrency>` من التوازي لكل موفر (`#4` أعلاه).
- `@<weight>` يضبط انحياز الجدولة؛ الإعدادات الافتراضية إلى 1.
- يحدد `--max-peers=<n>` عدد الموفرين المقرر تشغيلهم
  الاكتشاف ينتج مرشحين أكثر من المطلوب.
- `--expect-payload-digest` و`--expect-payload-len` يحميان من الصمت
  الفساد.
- يتحقق `--provider-advert=name=advert.to` من إمكانيات الموفر من قبل
  استخدامها في المحاكاة.
- يتجاوز `--retry-budget=<n>` عدد مرات إعادة المحاولة لكل مجموعة (الافتراضي: 3) لذلك CI
  يمكن أن تظهر الانحدارات بشكل أسرع عند اختبار سيناريوهات الفشل.

`fetch_report.json` أسطح المقاييس المجمعة (`chunk_retry_total`،
`provider_failure_rate`، وما إلى ذلك) مناسب لتأكيدات CI وإمكانية الملاحظة.

## 5. تحديثات التسجيل والحوكمة

عند اقتراح ملفات تعريف مقسمة جديدة:

1. قم بتأليف الواصف في `sorafs_manifest::chunker_registry_data`.
2. تحديث `docs/source/sorafs/chunker_registry.md` والمواثيق ذات الصلة.
3. قم بإعادة إنشاء التركيبات (`export_vectors`) والتقاط البيانات الموقعة.
4. تقديم تقرير الالتزام بالميثاق مع توقيعات الحوكمة.

يجب أن تفضل الأتمتة المقابض الأساسية (`namespace.name@semver`) والسقوط