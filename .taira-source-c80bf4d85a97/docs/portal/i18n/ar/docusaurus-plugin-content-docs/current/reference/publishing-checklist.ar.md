---
lang: ar
direction: rtl
source: docs/portal/docs/reference/publishing-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#قائمة التحقق للنشر

استخدم قائمة التحقق من كل ما قمت به. بوابة المطورين. يضمن ان بناء CI ونشر صفحات GitHub وتجربت الدخان اليدوية تغطي كل قسم قبل وصول اصدار او محطة طريق.

## 1. التحقق المحلي

- `npm run sync-openapi -- --version=current --latest` (اضف علما او اكثر `--mirror=<label>` عندما أضاف Torii OpenAPI لاخذ لقطة مجمدة).
- `npm run build` – تحقق من ان نص الـ البطل `Build on Iroha with confidence` ما يُظهر في `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – تحقق من بيان المجاميع الاختبارية (اضف `--descriptor`/`--archive` عند اختبار المصنوعات اليدوية اعتبارا من CI).
- `npm run serve` – مساعد معاينة المحمي بالـ المجموع الاختباري الذي يتحقق من الاتصال قبل الاتصال `docusaurus serve`، حتى لا يتصفح المراجعون لقطة غير موقعة (يبقى الاسم المستعار `serve:verified` متاحا للاستدعاءات الصريحة).
- راجع الـ markdown الذي لمسته عبر `npm run start` وخادم Live reload.

## 2. فحوصات طلب الانسحاب

- حقق نجاح بمهمة `docs-portal-build` في `.github/workflows/check-docs.yml`.
- تأكد من تشغيل `ci/check_docs_portal.sh` (سجلات CI لقياس دخان البطل).
- تأكد ان سير عمل معاينة رفع بيانا (`build/checksums.sha256`) وان سكربت التحقق من المعاينة نريد (تظهر خروج `scripts/preview_verify.sh`).
- اضف عنوان URL للمعاينة المنشورة من صفحات بيئة GitHub الى وصف PR.

## 3. اعتمد الاقسام| القسم | المالك | قائمة التحقق |
|---------|-------|-----------|
| الصفحة الرئيسية | ديفريل | ويظهر نص الـ البطل، وبطاقات البداية السريعة محددة بمسارات صالحة، وازرار CTA تعمل. |
| Norito | Norito WG | نظرة عامة على المثل و البدء شاهد الى اخر الأعلام للـ CLI و الوثائق مخطط Norito. |
| SoraFS | فريق التخزين | تعمل البداية السريعة حتى الاكتمال، وحقول تقرير البيان موثق، وتوقعات الجلب متحققة. |
| أدلة SDK | لـ SDK | بديل Rust/Python/JS يقوم بترجمة الامثلة الحالية وتربط بمستودعات حية. |
| مرجع | مستندات/ديفريل | يسرد الفهرس احدث المواصفات، ومرجع Norito codec يطابق `norito.md`. |
| معاينة القطعة الأثرية | مستندات/ديفريل | تم اصطياد قطعة أثرية `docs-portal-preview` بالـ PR، واجتازت السيولة الدخان، وتمت مشاركة الرابط مع المراجعين. |
| الأمان وجرب وضع الحماية | المستندات/DevRel · الأمان | تم تهيئة تسجيل الدخول برمز جهاز OAuth (`DOCS_OAUTH_*`)، مع مراعاة قائمة `security-hardening.md`، والتحقق من ترويسات CSP/Trusted Types عبر `npm run build` او `npm run probe:portal`. |

علّم كل جزء من مراجعة العلاقات العامة، أو دوّن أي مهمة متابعة حتى يبقى متابعة الحالة الدقيقة.

## 4. ملاحظات الاصدار

- ادرج `https://docs.iroha.tech/` (او عنوان بيئة النشر من مهمة النشر) في تعليقات الاصدار وتحديثات الحالة.
- اذكر اي اقسام جديدة او معدلة بوضوح حتى تعرف الفرق التابع لي لتسيير السيولة الخاصة بها.