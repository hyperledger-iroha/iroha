---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → خط أنابيب البيانات

هذا تكملة للبدء السريع في إعادة توجيه خط الأنابيب من أقصى إلى أقصى ما يحول
بايت كدليل أولي على Norito مناسب لـ Pin Registry لـ SoraFS. هذا المحتوى
محول [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
قم بمراجعة هذا المستند للمواصفات الأساسية وسجل التغيير.

## 1. جزء من شكل التحديد

SoraFS يستخدم الملف SF-1 (`sorafs.sf1@1.0.0`): تجزئة ملهمة في FastCDC مع واحد
حجم القطعة الأدنى 64 كيلو بايت، وهدف 256 كيلو بايت، والحد الأقصى 512 كيلو بايت وماسكارا
دي كورتي `0x0000ffff`. تم تسجيل الملف الشخصي في `sorafs_manifest::chunker_registry`.

### يساعد على التخلص من الصدأ

- `sorafs_car::CarBuildPlan::single_file` - إصدار إزاحات القطع وخطوط الطول والهضم
  BLAKE3 يعمل على إعداد بيانات تعريف السيارة.
- `sorafs_car::ChunkStore` – تدفق الحمولات واستمرار البيانات الوصفية للقطع واشتقاق الشجرة
  إثبات قابلية الاسترجاع (PoR) يصل إلى 64 كيلو بايت / 4 كيلو بايت.
- `sorafs_chunker::chunk_bytes_with_digests` – مساعد المكتبة خارج مهمة CLIs.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يحتوي JSON على الإزاحات وخطوط الطول وخلاصات القطع. جواردا
توضح خطة البناء مواصفات جلب الأوركيستادور.

### تيستيجوس بور`ChunkStore` يعرض `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` لذلك
يمكن للمراجعين طلب مجموعات من الاختبارات المحددة. أعلام Combina esos يخدع
`--por-proof-out` أو `--por-sample-out` لتسجيل JSON.

## 2. تضمين بيان

يجمع `ManifestBuilder` بيانات تعريف القطع مع ملحقات الإدارة:

- CID raíz (dag-cbor) وتسويات CAR.
- Pruebas de alias y مطالبات القدرة على الموردين.
- شركات الاستشارة والبيانات التعريفية الاختيارية (ص. على سبيل المثال، معرفات البناء).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

أهمية ساليداس:

- `payload.manifest` – بايتات البيانات المشفرة في Norito.
- `payload.report.json` - استئناف مقروء للإنسان/الأتمتة، بما في ذلك
  `chunk_fetch_specs`، `payload_digest_hex`، يلخص CAR والبيانات التعريفية ذات الأسماء المستعارة.
- `payload.manifest_signatures.json` – فيما يتعلق بمحتوى بيان BLAKE3، فإن
  ملخص SHA3 لخطة القطع والشركة Ed25519 ordenadas.

الولايات المتحدة الأمريكية `--manifest-signatures-in` للتحقق من الحصص المقدمة من الشركات الخارجية
قبل استكمال الكتابة، و`--chunker-profile-id` أو `--chunker-profile=<handle>` لـ
افتح اختيار السجل.

## 3. النشر والصنوبر1. **إرسال الإدارة** – توزيع ملخص البيان ومعلومات الشركة على
   نصيحتي لك أن الدبوس يمكن أن يعترف به. يحتاج المدققون الخارجيون إلى المسير
   ملخص SHA3 لخطة القطع جنبًا إلى جنب مع ملخص البيان.
2. **Pinear payload** – قم بإدراج ملف CAR (ومؤشر CAR اختياري) المشار إليه في القائمة
   يصرح إلى آل Pin Registry. تأكد من أن البيان والسيارة يشتركان في نفس اسم CID.
3. ** مسجل القياس عن بعد ** – حفظ تقرير JSON، وبيانات الاختبارات وأي مقياس
   جلب وتحرير القطع الأثرية. هذه السجلات مخصصة للوحات المعلومات
   المشغلون ويساعدون في إعادة إنتاج الحوادث دون تنزيل حمولات كبيرة.

## 4. محاكاة الجلب المتعدد

`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` يعمل على زيادة التوازي بواسطة الموفر (`#4`).
- `@<weight>` لضبط عملية التخطيط؛ بسبب العيب 1.
- `--max-peers=<n>` يحد من عدد الموردين المبرمجين للتنفيذ عند تنفيذ الأمر
  يؤدي الاكتشاف إلى إنتاج المزيد من المرشحين للرغبة.
- `--expect-payload-digest` و `--expect-payload-len` محمي ضد الفساد الصامت.
- `--provider-advert=name=advert.to` التحقق من قدرات المورد قبل الاستخدام
  في المحاكاة.
- `--retry-budget=<n>` يقوم بإعادة تشغيل سجل إعادة التشغيل بواسطة القطعة (للعيب: 3) لذلك
  يمكن لـ CI تفسير التراجعات بشكل أسرع من خلال اختبار سيناريوهات الخريف.

`fetch_report.json` قياسات مضافة (`chunk_retry_total`، `provider_failure_rate`،
وما إلى ذلك) إجراء عمليات CI وإمكانية الملاحظة.

## 5. تحديثات السجل والإدارة

جميع ملفات تعريف القطع الجديدة المقترحة:

1. قم بتحرير الواصف في `sorafs_manifest::chunker_registry_data`.
2. تحديث `docs/source/sorafs/chunker_registry.md` والميثاق المتعلق به.
3. تركيبات التجديد (`export_vectors`) والالتقاط يوضح الشركات الثابتة.
4. إرسال معلومات الوفاء بالميثاق إلى شركات الإدارة.

يجب أن تتعامل الأتمتة مع المعايير التقليدية (`namespace.name@semver`) وتكرار المعرفات
الأرقام فقط عندما تكون هناك حاجة للتسجيل.