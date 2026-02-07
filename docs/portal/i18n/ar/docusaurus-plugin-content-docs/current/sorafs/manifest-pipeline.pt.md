---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقطيع SoraFS → خط البيانات

يرافق هذا المكمل للتشغيل السريع خط الأنابيب الذي يقوم بتحويل البايتات
البيانات الكاملة في Norito مناسبة لـ Pin Registry في SoraFS. لقد تم تكييف هذه القصة
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)؛
راجع هذا المستند للحصول على المواصفات الأساسية وسجل التغيير.

## 1. طريقة قطع الشكل الحتمي

SoraFS يستخدم ملف SF-1 (`sorafs.sf1@1.0.0`): تجزئة ملهمة على FastCDC com
الحد الأدنى للقطعة هو 64 كيلو بايت، والأعلى 256 كيلو بايت، والحد الأقصى 512 كيلو بايت وماسكارا البحث
`0x0000ffff`. تم تسجيل الملف الشخصي في `sorafs_manifest::chunker_registry`.

### مساعدين في الصدأ

- `sorafs_car::CarBuildPlan::single_file` – إنشاء إزاحات القطع والإضافات والهضم
  BLAKE3 أثناء إعداد Metadados de CAR.
- `sorafs_car::ChunkStore` - تدفق سريع للحمولات واستمرار عمليات القطع والاشتقاق
  مجموعة كبيرة من إثباتات الاسترجاع (PoR) تبلغ 64 كيلو بايت / 4 كيلو بايت.
- `sorafs_chunker::chunk_bytes_with_digests` – مساعد المكتبة من خلال اثنين من CLIs.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يقوم JSON بإزاحة الأوامر والملحقات وهضم القطعتين. الحفاظ على بلانو آو
قم ببناء بيانات أو تفاصيل جلب للأوركسترادور.

### Testemunhas PoRيعرض `ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` لذلك
يمكن للمدققين طلب مجموعات من الشهادات الحتمية. الجمع بين أعلام ess com
`--por-proof-out` أو `--por-sample-out` للمسجل أو JSON.

## 2. قم بتعبئة البيان

يجمع `ManifestBuilder` بين أجزاء التعريف مع ملحقات الإدارة:

- CID raiz (dag-cbor) وتسويات CAR.
- إثبات الاسم المستعار وصلاحية المثبت.
- نصائح للاستشارات والتوصيفات الاختيارية (على سبيل المثال، معرفات البناء).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

المهمات السعيدة:

- `payload.manifest` – تقوم وحدات البايت بترميز البيان في Norito.
- `payload.report.json` – ملخص قانوني للإنسان/التشغيل الآلي، بما في ذلك `chunk_fetch_specs`،
  `payload_digest_hex`، ملخصات CAR وامتدادات الأسماء المستعارة.
- `payload.manifest_signatures.json` – المغلف المتنافس أو ملخص BLAKE3 للبيان، o
  ملخص SHA3 هو مخطط القطع والقتل Ed25519.

استخدم `--manifest-signatures-in` للتحقق من المغلفات المقدمة من قبل التوقيعات الخارجية
ما قبل الثقل الجديد e `--chunker-profile-id` أو `--chunker-profile=<handle>` لـ
قم بإصلاح خيار التسجيل.

## 3. النشر والنشر بالقرب1. **الطموح إلى الحكم** – ملخص البيان ومغلف الاغتيالات
   Conselho para que o pin possa ser الاعتراف. يجب على المدققين الخارجيين حماية SHA3
   قم بعمل خطة القطع جنبًا إلى جنب مع ملخص البيان.
2. **الحمولات الصنوبرية** – مرجع تحميل ملف CAR (ومؤشر CAR اختياري)
   البيان الخاص بـ Pin Registry. يضمن أن البيان وCAR يشاركان في نفس الوقت الذي يتم فيه رفع CID.
3. **مسجل القياس عن بعد** – احتفظ بالعلاقة بتنسيق JSON، كشهادة وخبرة
   métricas de fetch nos artefatos de Release. هذه السجلات الغذائية من لوحات المعلومات
   المشغلون ويساعدون على حل المشكلات دون زيادة الحمولات الكبيرة.

## 4. محاكاة الجلب متعدد المصادر

`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` زيادة أو توازي من خلال إثبات (`#4` أعلاه).
- `@<weight>` تعديل مسار جدول الأعمال؛ البداية ه 1.
- `--max-peers=<n>` يحدد عدد مقدمي جدول الأعمال للتنفيذ عند
  Descoberta Retorna Mais Candidatos do Que o Desejado.
- `--expect-payload-digest` و`--expect-payload-len` يحميان مكافحة الفساد الصامت.
- `--provider-advert=name=advert.to` تم التحقق من القدرات التي تم إثباتها مسبقًا قبل الاستخدام
  محاكاة.
- `--retry-budget=<n>` بديل لرسالة الاختبار للقطعة (الإطار: 3) لـ CI
  قم بإظهار التراجعات بشكل أسرع عند اختبار سيناريوهات الخطأ.

`fetch_report.json` عرض المقاييس المتوافقة (`chunk_retry_total`، `provider_failure_rate`،
إلخ.) كافية لتأكيدات CI وقابلية الملاحظة.

## 5. تحديث السجل والحوكمة

بخصوص قطع الغيار الجديدة المثالية:

1. قم بكتابة الواصف على `sorafs_manifest::chunker_registry_data`.
2. قم بتفعيل `docs/source/sorafs/chunker_registry.md` e كمواثيق ذات صلة.
3. قم بإعادة إنشاء التركيبات (`export_vectors`) وقم بالتقاط البيانات المقتولة.
4. إرسال علاقة التوافق مع الميثاق إلى مناصب الحكم.

يجب أن يفضل الأتمتة التعامل مع الكنسي (`namespace.name@semver`) وإعادة تصحيح المعرفات
الأرقام فقط عند الحاجة إلى التسجيل.