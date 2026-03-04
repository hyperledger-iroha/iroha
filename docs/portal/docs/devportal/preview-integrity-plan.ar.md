---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 02c46b727c66293532b0fe689b0af78a1c22cc71533c3d4f0e3f231788c51160
source_last_modified: "2025-11-10T17:57:34.936513+00:00"
translation_last_reviewed: 2026-01-01
---

# خطة المعاينة المحكومة بالتحقق من checksum

توضح هذه الخطة العمل المتبقي المطلوب لجعل كل أثر معاينة في البوابة قابلا للتحقق قبل النشر. الهدف هو ضمان أن المراجعين يقومون بتنزيل اللقطة نفسها المبنية في CI، وأن بيان checksum غير قابل للتغيير، وأن المعاينة قابلة للاكتشاف عبر SoraFS مع بيانات Norito الوصفية.

## الأهداف

- **عمليات بناء حتمية:** تأكد من أن `npm run build` ينتج مخرجات قابلة لإعادة الإنتاج ويصدر دائما `build/checksums.sha256`.
- **معاينات متحقق منها:** اشترط أن يصاحب كل أثر معاينة بيان checksum وارفض النشر عندما يفشل التحقق.
- **بيانات وصفية منشورة عبر Norito:** احفظ واصفات المعاينة (بيانات commit الوصفية، digest checksum، ومعرف CID لـ SoraFS) كـ Norito JSON لكي تستطيع أدوات الحوكمة تدقيق الإصدارات.
- **أدوات للمشغلين:** وفر سكربت تحقق بخطوة واحدة يمكن للمستهلكين تشغيله محليا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); يقوم السكربت الآن بتغليف تدفق التحقق من checksum + الواصف من البداية إلى النهاية. أمر المعاينة القياسي (`npm run serve`) يستدعي هذا المساعد تلقائيا قبل `docusaurus serve` كي تبقى اللقطات المحلية محكومة بالتحقق من checksum (مع الاحتفاظ بـ `npm run serve:verified` كاسم مستعار صريح).

## المرحلة 1 — فرض CI

1. حدّث `.github/workflows/docs-portal-preview.yml` لكي:
   - يشغل `node docs/portal/scripts/write-checksums.mjs` بعد بناء Docusaurus (مستدعى محليا بالفعل).
   - ينفذ `cd build && sha256sum -c checksums.sha256` ويفشل المهمة عند عدم التطابق.
   - يغلّف مجلد build على شكل `artifacts/preview-site.tar.gz`، وينسخ بيان checksum، ويستدعي `scripts/generate-preview-descriptor.mjs`، وينفذ `scripts/sorafs-package-preview.sh` مع إعداد JSON (انظر `docs/examples/sorafs_preview_publish.json`) حتى ينتج سير العمل كلًا من البيانات الوصفية وحزمة SoraFS حتمية.
   - يرفع الموقع الثابت، وآثار البيانات الوصفية (`docs-portal-preview`, `docs-portal-preview-metadata`)، وحزمة SoraFS (`docs-portal-preview-sorafs`) حتى يمكن فحص البيان وملخص CAR والخطة بدون إعادة البناء.
2. أضف تعليق شارة CI يلخص نتيجة التحقق من checksum في طلبات السحب (مُنفذ عبر خطوة تعليق GitHub Script في `docs-portal-preview.yml`).
3. وثق سير العمل في `docs/portal/README.md` (قسم CI) واربط خطوات التحقق في قائمة النشر.

## سكربت التحقق

`docs/portal/scripts/preview_verify.sh` يتحقق من آثار المعاينة التي تم تنزيلها دون الحاجة لاستدعاءات يدوية لـ `sha256sum`. استخدم `npm run serve` (أو الاسم المستعار الصريح `npm run serve:verified`) لتشغيل السكربت وإطلاق `docusaurus serve` بخطوة واحدة عند مشاركة اللقطات المحلية. منطق التحقق:

1. يشغل أداة SHA المناسبة (`sha256sum` أو `shasum -a 256`) مقابل `build/checksums.sha256`.
2. يقارن اختياريا digest/اسم ملف واصف المعاينة `checksums_manifest`، وعند توفره digest/اسم ملف أرشيف المعاينة.
3. ينهي بخروج غير صفري عند اكتشاف أي عدم تطابق حتى يتمكن المراجعون من حظر معاينات تم العبث بها.

مثال الاستخدام (بعد استخراج آثار CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

يجب على مهندسي CI والإصدارات تشغيل السكربت كلما حمّلوا حزمة معاينة أو أرفقوا آثارا بتذكرة إصدار.

## المرحلة 2 — نشر SoraFS

1. وسّع سير عمل المعاينة بوظيفة تقوم بـ:
   - رفع الموقع المبني إلى بوابة staging في SoraFS باستخدام `sorafs_cli car pack` و `manifest submit`.
   - التقاط digest البيان المعاد ومعرف CID لـ SoraFS.
   - تسلسل `{ commit, branch, checksum_manifest, cid }` إلى Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. خزن الواصف بجوار أثر البناء واظهر CID في تعليق طلب السحب.
3. أضف اختبارات تكامل تشغل `sorafs_cli` في وضع dry-run لضمان بقاء توافق مخطط البيانات الوصفية مع التغييرات المستقبلية.

## المرحلة 3 — الحوكمة والتدقيق

1. انشر مخطط Norito (`PreviewDescriptorV1`) الذي يصف بنية الواصف تحت `docs/portal/schemas/`.
2. حدّث قائمة النشر DOCS-SORA لتتطلب:
   - تشغيل `sorafs_cli manifest verify` مقابل CID المرفوع.
   - تسجيل digest بيان checksum و CID في وصف PR الإصدار.
3. اربط أتمتة الحوكمة للتحقق المتقاطع من الواصف مع بيان checksum أثناء تصويتات الإصدار.

## المخرجات والمسؤوليات

| المعلم | المالك | الهدف | ملاحظات |
|--------|--------|-------|---------|
| تطبيق تحقق checksum في CI | بنية تحتية للوثائق | الأسبوع 1 | يضيف بوابة فشل ورفع آثار. |
| نشر معاينات SoraFS | بنية تحتية للوثائق / فريق التخزين | الأسبوع 2 | يتطلب الوصول لاعتمادات staging وتحديثات مخطط Norito. |
| تكامل الحوكمة | قائد Docs/DevRel / فريق عمل الحوكمة | الأسبوع 3 | ينشر المخطط ويحدّث قوائم التدقيق وبنود خارطة الطريق. |

## أسئلة مفتوحة

- أي بيئة في SoraFS يجب أن تستضيف آثار المعاينة (staging أم مسار معاينة مخصص)؟
- هل نحتاج توقيعات مزدوجة (Ed25519 + ML-DSA) على واصف المعاينة قبل النشر؟
- هل يجب أن يثبّت سير عمل CI تهيئة orchestrator (`orchestrator_tuning.json`) عند تشغيل `sorafs_cli` للحفاظ على قابلية إعادة إنتاج البيانات؟

وثّق القرارات في `docs/portal/docs/reference/publishing-checklist.md` وحدث هذه الخطة عند حسم المجهولات.
