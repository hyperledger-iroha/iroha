---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-integrity-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#خطة المعاينة للحكومة بالتحقق من المجموع الاختباري

وبناءً على ذلك، يجب أن يتطلب الأمر كل ما تحتاجه للنظر في البوابة المطلوبة قبل النشر. الهدف هو استخدام المراجعين النشطة لتحميل اللقطة بنفسها في CI، وأن بيان المجموع الاختباري غير قابل للتغيير، وأن المعاينة للاكتشاف عبر SoraFS مع بيانات Norito الوصفية.

##عجز

- **عمليات بناء حتمية:** تأكد من أن `npm run build` ينتج مخرجات غير قابلة للإنتاج ويصدر دائماً `build/checksums.sha256`.
- **معاينات متحقق منها:** اشترط أن يصاحب كل جهود فحص بيان المجموع الاختباري ورفض النشر عندما يفشل المصادقة.
- **بيانات وصفية منشورة عبر Norito:** احفظ واصفات المعاينة (بيانات الالتزام الوصفية، الملخص الاختباري، ومعرف CID لـ SoraFS) كـ Norito JSON لكي تتمكن أدوات الـ ما يمكن تدقيقها.
- **أدوات للمشغلين:** وفر سكربت للتحكم في وحدة واحدة يمكن للمستهلكين تشغيله محليًا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); يقوم السكربت الآن بتغليف التحقق من المجموع الاختباري + الواصف من البداية إلى النهاية. أمر المعاينة القياسية (`npm run serve`) يستدعى هذا المساعد مباشرة قبل `docusaurus serve` كي يبقى شي محلي محكومة بالتحقق من المجموع الاختباري (مع تسوية بـ `npm run serve:verified` كاسم مستعار صريح).

## المرحلة 1 — فرض CI

1. تحديث `.github/workflows/docs-portal-preview.yml` لـ:
   - `node docs/portal/scripts/write-checksums.mjs` بعد بناء Docusaurus (مستدعىنا الأصلي بالفعل).
   - ينفذ `cd build && sha256sum -c checksums.sha256` ويفشل المهمة عند عدم التطابق.
   - يغلّف مجلد البناء على الشكل `artifacts/preview-site.tar.gz`، وينسخ بيان المجموع الاختباري، ويستدعي `scripts/generate-preview-descriptor.mjs`، وينفذ `scripts/sorafs-package-preview.sh` مع إعداد JSON (انظر `docs/examples/sorafs_preview_publish.json`) حتى يخرج سير العمل كلًا من البيانات الوصفية وزمة SoraFS حتمية.
   - يرفع الموقع الثابت وآثار البيانات الوصفية (`docs-portal-preview`, `docs-portal-preview-metadata`)، وحزمة SoraFS (`docs-portal-preview-sorafs`) حتى يمكن فحص البيان وملخص CAR والخطة بدون إعادة البناء.
2. إضافة تعليق شارة CI يلخص نتيجة التحقق من المجموع الاختباري في طلبات السحب (مُنفذ عبر خطوة تعليق GitHub Script في `docs-portal-preview.yml`).
3. توثيق سير العمل في `docs/portal/README.md` (قسم CI) وربط خطوات التحقق في قائمة النشر.

## سكربت التحقق

`docs/portal/scripts/preview_verify.sh` يتحقق من صدم المعاينة التي تم تنزيلها دون حاجة لاستدعاءات يدوية لـ `sha256sum`. استخدم `npm run serve` (أو الاسم المستعار الصريح `npm run serve:verified`) لتشغيل السكربت مؤقتًا `docusaurus serve` لاستخدام واحد عند مشاركة قطعة المحلية. التحقق من المنطقة:

1. أفضل أداة SHA (`sha256sum` أو `shasum -a 256`) مقابل `build/checksums.sha256`.
2. يقارن اختياريا الملخص/اسم ملف واصف المعاينة `checksums_manifest`، ويتوفره الملخص/اسم ملف أرشيف المعاينة.
3. ينهي بخروج غير صفري عند اكتشاف أي عدم تطابق حتى يتوصلوا بالمراجعين من حظر معاينات تم العبث بها.

مثال الاستخدام (بعد الضرر الناجم عن CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

يجب على مهندسي CI وإصدارات السكربت كلما تولى نتنياهو حزمة معاينة أو أرفقوا الإصابات بتذكر الإصدار.

## المرحلة 2 — نشر SoraFS

1. وسّع سير عمل المعاينة بوظيفة تقوم بـ:
   - رفع الموقع المبني إلى بوابة التدريج في SoraFS باستخدام `sorafs_cli car pack` و `manifest submit`.
   - خلاصه الضربة المعاد ومعرف CID لـ SoraFS.
   - السلسلة من `{ commit, branch, checksum_manifest, cid }` إلى Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. خزن الواصف بجوار البناء واظهر CID في تعليق طلب الانسحاب.
3. إضافة السيولة تكامل تشغل `sorafs_cli` في وضع التشغيل الجاف وضمان بقاء مخطط البيانات الوصفية مع المستقبل المستقبل.

## المرحلة 3 — التورم والدقيق

1. انشر مخطط Norito (`PreviewDescriptorV1`) الذي طبق نظام الواصف تحت `docs/portal/schemas/`.
2. تعديل قائمة النشر DOCS-SORA لتتطلب:
   - تشغيل `sorafs_cli manifest verify` مقابل CID المرفوعة.
   - تسجيل بيان المجموع الاختباري و CID في نسخة PR الموصوفة.
3. الاشتراك في المساهمة في جميع الأجزاء المتقاطعة من الواصف مع بيان المجموع الاختباري أثناء تصويتات الإصدار.

## النهايات والمسؤوليات

| المعلم | المالك | الهدف | تعليقات |
|--------|--------|--------|--------|
| تطبيق تحقق من المجموع الاختباري في CI | نظام تحتي للوثائق | الأسبوع 1 | بوابة مضاعفة التردد. |
| نشر معاينات SoraFS | تأسيس تحتية للوثائق / فريق التخزين | الأسبوع 2 | يلزم الوصول إلى اعتمادات التدريج وتحديثات مخطط Norito. |
| تكامل ال تور | قائد Docs/DevRel / فريق عمل النتوستر | الأسبوع 3 | ينشر تحديد ويحدد القواعد المتعلقة بالتحكم وبنود خطوط الطريق. |

## أسئلة

- أي بيئة في SoraFS هل يجب أن تساعد في التصوير الشعاعي (staging أم مسار معاينة مخصص)؟
- هل نحتاج توقيعات مزدوجة (Ed25519 + ML-DSA) على واصف المعاينة قبل النشر؟
- هل يجب أن ثبّت سير عمل CI تهيئة الأوركسترا (`orchestrator_tuning.json`) عند تشغيل `sorafs_cli` الإضاءة على قابلة لإعادة إنتاج البيانات؟

وثّق القرار في `docs/portal/docs/reference/publishing-checklist.md` وحدث هذا البناء عند التحديد المجهولات.