---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# خطة المعاينة المحكومة بالتحقق من suma de comprobación

توضح هذه الخطة العمل المتبقي المطلوب لجعل كل أثر معاينة في البوابة قابلا للتحقق قبل النشر. هدف هو ضمان أن المراجعين يقومون بتنزيل اللقطة نفسها المبنية في CI، وأن بيان checksum غير قابل للتغيير، وأن La configuración de la unidad es SoraFS o Norito.

## الأهداف

- **عمليات بناء حتمية:** تأكد من أن `npm run build` ينتج مخرجات قابلة لإعادة الإنتاج ويصدر دائما `build/checksums.sha256`.
- **معاينات متحقق منها:** اشترط أن يصاحب كل أثر معاينة بيان checksum وارفض النشر عندما يفشل التحقق.
- **بيانات وصفية منشورة عبر Norito:** احفظ واصفات المعاينة (بيانات commit, resumen checksum, y CID لـ SoraFS) Utilice Norito JSON para configurar los archivos.
- **أدوات للمشغلين:** وفر سكربت تحقق بخطوة واحدة يمكن للمستهلكين تشغيله محليا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); يقوم السكربت الآن بتغليف تدفق التحقق من checksum + الواصف من البداية إلى النهاية. أمر المعاينة القياسي (`npm run serve`) يستدعي هذا المساعد تلقائيا قبل `docusaurus serve` كي تبقى اللقطات المحلية Haga una suma de comprobación (hay una suma de comprobación `npm run serve:verified`).

## المرحلة 1 — فرض CI1. حدّث `.github/workflows/docs-portal-preview.yml` aquí:
   - يشغل `node docs/portal/scripts/write-checksums.mjs` بعد بناء Docusaurus (مستدعى محليا بالفعل).
   - ينفذ `cd build && sha256sum -c checksums.sha256` ويفشل المهمة عند عدم التطابق.
   - Para compilar, use `artifacts/preview-site.tar.gz`, y agregue suma de verificación, y `scripts/generate-preview-descriptor.mjs`, y `scripts/sorafs-package-preview.sh` con JSON. (انظر `docs/examples/sorafs_preview_publish.json`) حتى ينتج سير العمل كلًا من البيانات الوصفية وحزمة SoraFS حتمية.
   - يرفع الموقع الثابت، وآثار البيانات الوصفية (`docs-portal-preview`, `docs-portal-preview-metadata`), y SoraFS (`docs-portal-preview-sorafs`) حتى يمكن فحص البيان وملخص CAR والخطة بدون إعادة البناء.
2. Haga clic en CI para realizar la suma de comprobación de la suma de comprobación (en el caso de GitHub Script) `docs-portal-preview.yml`).
3. Presione el botón `docs/portal/README.md` (CI) y presione el botón.

## سكربت التحقق

`docs/portal/scripts/preview_verify.sh` يتحقق من آثار المعاينة التي تم تنزيلها دون الحاجة لاستدعاءات يدوية لـ `sha256sum`. `npm run serve` (أو الاسم المستعار الصريح `npm run serve:verified`) لتشغيل السكربت AND إطلاق `docusaurus serve` بخطوة واحدة عند مشاركة اللقطات المحلية. منطق التحقق:

1. شغل أداة SHA المناسبة (`sha256sum` أو `shasum -a 256`) مقابل `build/checksums.sha256`.
2. يقارن اختياريا digest/اسم ملف واصف المعاينة `checksums_manifest`, وعند توفره digest/اسم ملف أرشيف المعاينة.
3. ينهي بخروج غير صفري عند اكتشاف أي عدم تطابق حتى يتمكن المراجعون من حظر معاينات تم العبث بها.

مثال الاستخدام (بعد استخراج آثار CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```يجب على مهندسي CI والإصدارات تشغيل السكربت كلما حمّلوا حزمة معاينة أو أرفقوا آثارا بتذكرة إصدار.

## المرحلة 2 — نشر SoraFS

1. وسّع سير عمل المعاينة بوظيفة تقوم بـ:
   - رفع الموقع المبني إلى بوابة puesta en escena في SoraFS باستخدام `sorafs_cli car pack` y `manifest submit`.
   - Resumen de resumen de contenido y CID según SoraFS.
   - Inserte `{ commit, branch, checksum_manifest, cid }` y Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. خزن الواصف بجوار أثر البناء واظهر CID في تعليق طلب السحب.
3. Haga clic en el botón `sorafs_cli` y realice un funcionamiento en seco en el equipo. المستقبلية.

## المرحلة 3 — الحوكمة والتدقيق

1. انشر مخطط Norito (`PreviewDescriptorV1`) الذي يصف بنية الواصف تحت `docs/portal/schemas/`.
2. Inserte el documento DOCS-SORA en:
   - Utilice el código `sorafs_cli manifest verify` del CID.
   - تسجيل digest بيان checksum y CID في وصف PR الإصدار.
3. اربط أتمتة الحوكمة للتحقق المتقاطع من الواصف مع بيان checksum أثناء تصويتات الإصدار.

## المخرجات والمسؤوليات| المعلم | المالك | الهدف | ملاحظات |
|--------|--------|-------|---------|
| تطبيق تحقق suma de comprobación en CI | بنية تحتية للوثائق | الأسبوع 1 | يضيف بوابة فشل ورفع آثار. |
| Fuentes de alimentación SoraFS | بنية تحتية للوثائق / فريق التخزين | الأسبوع 2 | يتطلب الوصول لاعتمادات puesta en escena y تحديثات مخطط Norito. |
| تكامل الحوكمة | قائد Docs/DevRel / فريق عمل الحوكمة | الأسبوع 3 | ينشر المخطط ويحدّث قوائم التدقيق وبنود خارطة الطريق. |

## أسئلة مفتوحة

- أي بيئة في SoraFS يجب أن تستضيف آثار المعاينة (puesta en escena أم مسار معاينة مخصص)؟
- هل نحتاج توقيعات مزدوجة (Ed25519 + ML-DSA) على واصف المعاينة قبل النشر؟
- هل يجب أن يثبّت سير عمل CI تهيئة Orchestrator (`orchestrator_tuning.json`) عند تشغيل `sorafs_cli` للحفاظ على قابلية إعادة إنتاج البيانات؟

وثّق القرارات في `docs/portal/docs/reference/publishing-checklist.md` وحدث هذه الخطة عند حسم المجهولات.