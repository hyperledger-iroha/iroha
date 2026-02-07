---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# التحويل SoraFS → بيان خط الأنابيب

تعمل هذه المواد على دعم التشغيل السريع وتشير إلى خط عادي كامل والذي يمكن تحويله إلى سلك
البيانات الموجودة في البيانات Norito، متصلة بـ Pin Registry SoraFS. محول النص من
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)؛
قم بالاطلاع على هذه الوثيقة الخاصة بالمواصفات القانونية والمجلة المتغيرة.

## 1. تحديد التغيير

SoraFS يستخدم ملف التعريف SF-1 (`sorafs.sf1@1.0.0`): المتداول، سريع الحركة، FastCDC، مع
الحد الأدنى لمساحة 64 كيلو بايت، والتليفزيون 256 كيلو بايت، والحد الأقصى 512 كيلو بايت، ومساحة مخفية
`0x0000ffff`. الملف الشخصي заregistrirovan в `sorafs_manifest::chunker_registry`.

### مساعدات الصدأ

- `sorafs_car::CarBuildPlan::single_file` – عرض الإعدادات والخطوط ومنافذ BLAKE3
  مع السيارة المجهزة.
- `sorafs_car::ChunkStore` – تقليل الحمولات وتأمين الصناديق التحويلية وإخراج الأموال
  выборки إثبات الاسترجاع (PoR) 64 كيلو بايت / 4 كيلو بايت.
- `sorafs_chunker::chunk_bytes_with_digests` – المساعد المكتبي، المتوافق مع CLI.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يقوم JSON بتوفير التنسيقات الجيدة والخطوط والأدوات المساعدة. ضع خطة الرعاية في الرياضة
البيانات أو الخيارات المحددة للمنسق.

### Свидетели PoR

`ChunkStore` يمثل `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>`،
لكي يتمكن المدققون من تحديد أنواع المدققين. تعلم هذه الأعلام مع
`--por-proof-out` أو `--por-sample-out` لتسجل JSON.

## 2. بيان شامل`ManifestBuilder` يتبع القنوات التحويلية ذات الحوكمة المرغوبة:

- Конневой CID (dag-cbor) والتزامات السيارة.
- Доказательства الاسم المستعار والمطالبات بإمكانية إثباتها.
- تقديم الأفكار والطرق الاختيارية (على سبيل المثال، معرفات البناء).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

البيانات المهمة:

- `payload.manifest` – Norito بيان بيانات التشفير.
- `payload.report.json` – مياه للأشخاص/الأتمتة، بما في ذلك `chunk_fetch_specs`،
  `payload_digest_hex`، дадесты CAR والاسم المستعار المتغير.
- `payload.manifest_signatures.json` - تحويل، بيان توصيل BLAKE3،
  SHA3-Diggest Planan Chankovs and Outsortiovannыe Ed25519.

استخدم `--manifest-signatures-in` للتحقق من التحويلات من البريد الإلكتروني الحالي
للتخصيص و `--chunker-profile-id` أو `--chunker-profile=<handle>` لاختيار التحديد
ريسترا.

## 3. النشر والتثبيت1. **التنفيذ في الحوكمة** – قم بإدراج بيان الأداء وتحويل الفكرة المكملة إلى ما تريده
   دبوس يمكن أن يكون صديقا. مدقق الحسابات الداخلي يتابع خطة SHA3 للتحكم في الوقت المناسب مع
   بيان الصفحة.
2. **تحميل الحمولات** – تحميل أرشيف CAR (ومؤشر اختياري CAR)، محمي في
   بيان في Pin Registry. لاحظ أن البيان والسيارة يستخدمان واحدًا وهو أيضًا قسم البحث الجنائي.
3. **تسجيل القياسات عن بعد** – يتم جلب محتوي JSON وPoR والمقاييس المفضلة
   المصنوعات اليدوية. تقوم بتزويد مشغلي لوحات المفاتيح وتساعد على الاتصال
   مشاكل بدون زيادة الحمولات.

## 4. خيارات المحاكاة لعدد قليل من المطورين

`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` يزيد من توازي المعالج (`#4` آخر).
- `@<weight>` يقوم بتخطيط التخطيط; في التشجيع 1.
- `--max-peers=<n>` يقوم بإلغاء تأمين مقدمي الطلبات الذين يخططون للمغادرة عندما
  الإخطار يثير المزيد من المرشحين الذين يحتاجون إليهم.
- `--expect-payload-digest` و `--expect-payload-len` يحميان من هذه البيانات.
- `--provider-advert=name=advert.to` التحقق من القدرة على الاستخدام قبل الاستخدام
  في المحاكاة.
- `--retry-budget=<n>` يعيد توجيه رأس شانك (في العمق: 3)، لCI
  لقد حدث الانحدار الشديد أثناء الاختبار.

`fetch_report.json` مقاييس تجميعية (`chunk_retry_total`, `provider_failure_rate`,
و ر. د.)، دعم تأكيدات CI والملاحظة.

## 5.السجل والحوكمة

قبل المقترحات الجديدة للملف الشخصيchunker:

1. أدخل الواصف في `sorafs_manifest::chunker_registry_data`.
2. احصل على `docs/source/sorafs/chunker_registry.md` والرحلات الجوية.
3. قم بإعادة إنشاء الصور (`export_vectors`) وقم بطباعة بيانات التسليم.
4. تنفيذ الميثاق الجيد مع الحوكمة المكملة.

الأتمتة التالية تقترح المقابض الكنسي (`namespace.name@semver`) و
لقد تم التعبير عن معرف أنيق فقط من خلال المعرفة المتقنة.