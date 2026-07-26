---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-integrity-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plano de previsualizacao com المجموع الاختباري

توضح هذه الخطة ما تبقى من مهمة ضرورية لنسخ كل قطعة مصطنعة من التصور المسبق للبوابة التي تم التحقق منها قبل النشر. الهدف والتأكد من أن المراجعات ستتم بشكل مباشر أو تم إنشاء لقطة في CI، وأن بيان المجموع الاختباري سيتم تغييره وسيتم اكتشاف التصور المسبق عبر SoraFS مع التعريفات Norito.

##الأهداف

- **يبني الحتميات:** يضمن أن `npm run build` قد تم إنتاجه واستمر في العمل `build/checksums.sha256`.
- **التصورات المسبقة التي تم التحقق منها:** تأكد من أن كل قطعة من التصورات المسبقة تتضمن بيان المجموع الاختباري وتمنع النشر عند حدوث التحقق.
- **تم نشر التعريفات عبر Norito:** استمر في استخدام الواصفات المرئية المسبقة (انتقالات الالتزام، ملخص المجموع الاختباري، CID SoraFS) مثل JSON Norito حتى تتمكن أدوات الإدارة من تدقيق الإصدارات.
- **أدوات للمشغلين:** إنشاء برنامج نصي للتحقق من المرور الذي يمكن للمستهلكين تنفيذه محليًا (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`)؛ يشتمل البرنامج النصي الآن على تدفق التحقق من المجموع الاختباري + واصف النقطة إلى النقطة. يتم الآن استدعاء أمر المعاينة المسبقة (`npm run serve`) تلقائيًا قبل `docusaurus serve` حتى تكون اللقطات محمية بشكل دائم من خلال المجموع الاختباري (com `npm run serve:verified` يتم الاحتفاظ بها باسم مستعار صريح).

## المرحلة 1 - تطبيق CI

1. تحديث `.github/workflows/docs-portal-preview.yml` لـ:
   - قم بتنفيذ `node docs/portal/scripts/write-checksums.mjs` عند إنشاء Docusaurus (تم استدعاؤه محليًا).
   - تنفيذ `cd build && sha256sum -c checksums.sha256` وبدء المهمة في حالة الاختلاف.
   - إنشاء مجلد أو إنشاء مجلد مثل `artifacts/preview-site.tar.gz` ونسخ بيان المجموع الاختباري وتنفيذ `scripts/generate-preview-descriptor.mjs` وتنفيذ `scripts/sorafs-package-preview.sh` بتكوين JSON (الإصدار `docs/examples/sorafs_preview_publish.json`) حتى يصدر سير العمل الكثير من الفوقية أم حزمة SoraFS الحتمية.
   - قم بإرسال الموقع الإلكتروني الثابت والبيانات التعريفية (`docs-portal-preview` و`docs-portal-preview-metadata`) والحزمة SoraFS (`docs-portal-preview-sorafs`) للبيان أو استئناف السيارة أو التخطيط ليتم فحصها دون إعادة تصميمها.
2. أضف تعليقًا على شارة CI لاستئناف أو نتيجة التحقق من المجموع الاختباري لطلبات السحب (يتم تنفيذه عبر ممر تعليق GitHub Script من `docs-portal-preview.yml`).
3. قم بتوثيق سير العمل على `docs/portal/README.md` (فقط CI) وربطه بخطوات التحقق في قائمة المراجعة العامة.

## نص التحقق

`docs/portal/scripts/preview_verify.sh` يقوم بالتحقق من صحة الصور المصطنعة مسبقًا دون الحاجة إلى طلب استدعاءات يدوية من `sha256sum`. استخدم `npm run serve` (أو الاسم المستعار الصريح `npm run serve:verified`) لتنفيذ البرنامج النصي وبدء `docusaurus serve` في خطوة واحدة لمشاركة اللقطات المحلية. منطق التحقق:

1. قم بتنفيذ أداة SHA المناسبة (`sha256sum` أو `shasum -a 256`) مقابل `build/checksums.sha256`.
2. قم بشكل اختياري بمقارنة الملخص/اسم ملف واصف التصور المسبق `checksums_manifest`، وعند الطلب، اسم الملخص/الملف الموجود في ملف التصور المسبق.
3. قم بالتواصل مع الكود دون الصفر عندما يتم اكتشاف بعض الاختلافات والاختلافات حتى تتمكن المراجع من حظر التصورات الزائفة مسبقًا.

مثال على الاستخدام (من خلال أدوات CI الإضافية):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

يجب على مهندسي CI والإصدار أن ينفذوا البرنامج النصي باستمرار من خلال إضافة حزمة من التصور المسبق أو إضافة المصنوعات اليدوية إلى تذكرة الإصدار.

## المرحلة 2 - الجمهور em SoraFS

1. قم بإعداد سير العمل للتصور المسبق كمهمة:
   - أرسل الموقع الذي تم إنشاؤه لبوابة التدريج SoraFS باستخدام `sorafs_cli car pack` و`manifest submit`.
   - التقط ملخصًا للبيان المعاد وCID لـ SoraFS.
   - التسلسل `{ commit, branch, checksum_manifest, cid }` em JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. قم بتخزين الواصف جنبًا إلى جنب مع قطعة البناء والتصدير أو CID بدون تعليق لطلب السحب.
3. أضف اختبارات التكامل التي تمارس `sorafs_cli` في وضع التشغيل الجاف لضمان أن النماذج المستقبلية تحافظ على اتساق مخطط التعريفات.

## المرحلة 3 - الإدارة والاستماع

1. قم بنشر المخطط Norito (`PreviewDescriptorV1`) وقم بتحليل إعداد الواصف في `docs/portal/schemas/`.
2. قم بتحديث قائمة مرجعية لنشر DOCS-SORA للتنزيل:
   -Rodar `sorafs_cli manifest verify` ضد إرسال CID.
   - سجل ملخص بيان المجموع الاختباري وCID الموضح في بيان الإصدار.
3. يتم ربط الإدارة تلقائيًا بالتنقل أو الواصف باستخدام بيان المجموع الاختباري خلال أصوات الإصدار.

## المشاركة والمسؤولية

| ماركو | الملكية (الملكيات) | ألفو | نوتاس |
|-------|-----------------|------|-------|
| تطبيق المجموع الاختباري في CI Confluida | البنية التحتية للمستندات | سيمانا 1 | إضافة بوابة النشر وتحميل الأعمال الفنية. |
| Publicacao de previsualizacao no SoraFS | البنية التحتية للمستندات / معدات التخزين | سيمانا 2 | اطلب الوصول إلى بيانات الاعتماد المرحلية وتحديث المخطط Norito. |
| تكامل الحوكمة | Lider de Docs/DevRel / WG de Governanca | سيمانا 3 | نشر المخطط وتحديث قوائم المراجعة وإدخالات خريطة الطريق. |

## Perguntas em aberto

- ما هي البيئة المحيطة بـ SoraFS التي تحتاج إلى استضافة Artefatos de previsualizacao (التدريج مقابل مسار التصور المسبق المخصص)؟
- هل تحتاج إلى تفاصيل الإضافات المزدوجة (Ed25519 + ML-DSA) دون واصف للتصور المسبق قبل النشر؟
- هل يجب على سير عمل CI إصلاح تكوين orquestrador (`orchestrator_tuning.json`) أو تنفيذ `sorafs_cli` لمواصلة إصدار البيانات؟

قم بالتسجيل كقرارات في `docs/portal/docs/reference/publishing-checklist.md` وتحقق من هذه الخطة عند الانتهاء من الحلول.