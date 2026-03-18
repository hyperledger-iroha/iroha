---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-integrity-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# خطة التحكم المسبق في المجموع الاختباري

تتطلب هذه الخطة أن يكون العمل ضروريًا لتقديم كل قطعة أثرية للرؤية المسبقة للبوابة يمكن التحقق منها قبل النشر. الهدف هو ضمان أن يتم إنشاء اللقطة بدقة من قبل المستشعرين في CI، وأن بيان المجموع الاختباري غير قابل للتغيير وأن المعاينة يمكن اكتشافها عبر SoraFS باستخدام metadonnees Norito.

## الأهداف

- **يبني المحددات:** أضمن أن `npm run build` ينتج صنفًا قابلاً لإعادة الإنتاج ويصدر دائمًا `build/checksums.sha256`.
- **التحقق من المعاينة المسبقة:** Exiger que chaque artefact de previsualisation fournisse un بيان المجموع الاختباري ورفض النشر عند صدى التحقق.
- **نشر البيانات الوصفية عبر Norito:** استمر في واصفات المراقبة (بيانات الالتزام، خلاصة المجموع الاختباري، CID SoraFS) وJSON Norito حتى تتمكن أدوات الإدارة من مراجعة الإصدارات.
- **مشغل النفاذ:** يوفر برنامج نصي للتحقق في خطوة يمكن للمستهلكين من خلالها تنفيذ الأمر المحلي (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`)؛ يقوم البرنامج النصي المغلف بتفكيك المجموع الاختباري للتحقق من الصحة + واصف النوبة في نوبة. يستدعي أمر المعاينة القياسية (`npm run serve`) هذا المساعد التلقائي المتقدم لـ `docusaurus serve` حتى تتمكن اللقطات المحلية من التحكم في المجموع الاختباري (مع `npm run serve:verified` مع الحفاظ على الاسم المستعار الصريح).

## المرحلة الأولى - تطبيق CI

1. ميتر كل يوم `.github/workflows/docs-portal-preview.yml` من أجل:
   - المُنفِّذ `node docs/portal/scripts/write-checksums.mjs` بعد إنشاء Docusaurus (قم باستدعاء المنطقة المحلية).
   - قم بتنفيذ `cd build && sha256sum -c checksums.sha256` وقم بتكرار المهمة في حالة التباعد.
   - قم بتجميع المرجع المبني على `artifacts/preview-site.tar.gz`، ونسخ بيان المجموع الاختباري، والمنفذ `scripts/generate-preview-descriptor.mjs`، والمنفذ `scripts/sorafs-package-preview.sh` مع تكوين JSON (النظر إلى `docs/examples/sorafs_preview_publish.json`) حتى يقوم سير العمل بتحرير البيانات التعريفية والحزمة مرة أخرى تحديد SoraFS.
   - قم ببث إحصائيات الموقع والعناصر الوصفية (`docs-portal-preview` و`docs-portal-preview-metadata`) والحزمة SoraFS (`docs-portal-preview-sorafs`) للوصول إلى البيان واستئناف CAR والخطة الفعالة والفحص دون إعادة البناء.
2. أضف تعليقًا على شارة CI لاستئناف نتيجة المجموع الاختباري للتحقق في طلبات السحب (يتم تنفيذه عبر شريط التعليق GitHub Script de `docs-portal-preview.yml`).
3. قم بتوثيق سير العمل في `docs/portal/README.md` (القسم CI) وقم بكتابة خطوات التحقق في قائمة التحقق من النشر.

## التحقق من البرنامج النصي

`docs/portal/scripts/preview_verify.sh` صالح لعناصر التصور المسبق التي تم تنزيلها دون الحاجة إلى إجراء استدعاءات يدوية لـ `sha256sum`. استخدم `npm run serve` (أو الاسم المستعار الصريح `npm run serve:verified`) لتنفيذ البرنامج النصي وإطلاق `docusaurus serve` في شريط واحد فقط عند مشاركة اللقطات المحلية. منطق التحقق:

1. قم بتنفيذ أداة SHA المناسبة (`sha256sum` أو `shasum -a 256`) مقابل `build/checksums.sha256`.
2. قارن خيارات الملخص/اسم ملف واصف المعاينة `checksums_manifest` وعندما يتم تقديم الملخص/اسم ملف أرشيف المعاينة.
3. قم بالفرز باستخدام رمز غير فارغ عندما يتم اكتشاف التباعد حتى تتمكن المراجع من منع تغيير التصورات.

مثال على الاستخدام (استخراج القطع الأثرية CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

يجب على Ingenieurs CI et Release استدعاء البرنامج النصي كل مرة حيث يتم تنزيل حزمة من التصور المسبق أو إرفاق العناصر الأثرية بتذكرة إصدار.

## المرحلة الثانية - النشر SoraFS

1. قم بمتابعة سير العمل المسبق بوظيفة:
   - قم ببث الموقع المبني على ممر التدريج SoraFS باستخدام `sorafs_cli car pack` و`manifest submit`.
   - التقط ملخص البيان renvoye و CID SoraFS.
   - إجراء تسلسل `{ commit, branch, checksum_manifest, cid }` وJSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. قم بتخزين الواصف باستخدام قطعة البناء وكشف CID في تعليق طلب السحب.
3. إضافة اختبارات التكامل التي تمارس `sorafs_cli` في وضع التشغيل الجاف للتأكد من أن التطورات المستقبلية تحافظ على تماسك مخطط التعريفات.

## المرحلة الثالثة - الحوكمة والتدقيق

1. قم بنشر المخطط Norito (`PreviewDescriptorV1`) الذي يشتق بنية واصف `docs/portal/schemas/`.
2. قم بقراءة القائمة المرجعية للنشر DOCS-SORA يوميًا للتنزيل:
   - لانسر `sorafs_cli manifest verify` على تهمة CID.
   - قم بتسجيل ملخص بيان المجموع الاختباري وCID في وصف بيان الإصدار.
3. قم بتوصيل أتمتة الإدارة لتنقل الواصف مع بيان المجموع الاختباري عند صدور الأصوات.

## الحياة والمسؤوليات

| جالون | المالك (المالكون) | سيبل | ملاحظات |
|-------|-----------------|-------|-------|
| تطبيق المجاميع الاختبارية في CI مجانًا | وثائق البنية التحتية | سيماين 1 | قم بإضافة بوابة التحقق وتحميل العناصر. |
| منشور المعاينات SoraFS | وثائق البنية التحتية / تجهيز التخزين | سيماين 2 | يلزم الوصول إلى معرفات التدريج والإعدادات في يوم المخطط Norito. |
| الحوكمة التكاملية | المستندات المسؤولة/DevRel/WG Governance | سيماين 3 | انشر المخطط واحصل على قوائم المراجعة ومدخل خريطة الطريق يوميًا. |

## الأسئلة المفتوحة

- Quel environnement SoraFS doit heberger les artefacts de previsualisation (التدريج مقابل حارة التصور المسبق ديدي)؟
- هل نحتاج إلى التوقيعات المزدوجة (Ed25519 + ML-DSA) على واصف الرؤية المسبقة للنشر؟
- هل يقوم CI الخاص بسير العمل بإرسال رسالة تكوين المخطط (`orchestrator_tuning.json`) أثناء تنفيذ `sorafs_cli` لحماية البيانات القابلة لإعادة الإنتاج؟

أرسل قراراتك إلى `docs/portal/docs/reference/publishing-checklist.md` وابدأ في تنفيذ هذه الخطة مرة أخرى بعد اتخاذ القرارات غير المهمة.