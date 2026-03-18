---
lang: ar
direction: rtl
source: docs/portal/docs/reference/publishing-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# قائمة المراجعة العامة

استخدم قائمة التحقق هذه دائمًا لتحديث بوابة المطورين. إنه يضمن إنشاء CI، أو عدم نشر صفحات GitHub، كما يتم إجراء اختبارات الدخان على مدار كل الثواني قبل الإصدار أو وضع علامة على خريطة الطريق.

## 1. فاليداكاو محلي

- `npm run sync-openapi -- --version=current --latest` (أضف علامة أو أكثر إلى `--mirror=<label>` عندما أو Torii OpenAPI من أجل لقطة مجمعة).
- `npm run build` - أكد أن البطل نسخة `Build on Iroha with confidence` سيظهر في `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - التحقق من بيان المجموع الاختباري (أضف `--descriptor`/`--archive` إلى اختبار CI baixados).
- `npm run serve` - بدء مساعدة المعاينة من خلال المجموع الاختباري للتحقق من البيان قبل الاتصال بـ `docusaurus serve`، حتى لا يتمكن المراجعون من تصفح لقطة بدون حذفها (أو الاسم المستعار `serve:verified` للاستمرار في المكالمات الصريحة).
- قم بإجراء فحص فوري لإجراء تخفيض السعر عبر `npm run start` وخادم إعادة التحميل المباشر.

## 2. الشيكات طلب السحب

- تحقق من أن المهمة `docs-portal-build` تمر عبر `.github/workflows/check-docs.yml`.
- قم بتأكيد `ci/check_docs_portal.sh` Rodou (سجلات CI Mostram أو فحص الدخان البطل).
- ضمان أن سير العمل في المعاينة يرسل بيانًا (`build/checksums.sha256`) وأن نص التحقق من المعاينة قد نجح (يتم تسجيل الدخول إلى `scripts/preview_verify.sh`).
- إضافة عنوان URL للمعاينة المنشورة لصفحات GitHub المحيطة لوصف العلاقات العامة.## 3. Aprovacao por secao

| سيكاو | المالك | قائمة المراجعة |
|---------|-------|-----------|
| الصفحة الرئيسية | ديفريل | يتم عرض نسخة البطل، وربط بطاقات التشغيل السريع لتدوير الصالحيات، وحل أزرار CTA. |
| Norito | Norito WG | نظرة عامة على الدليل ومرجع البدء للأعلام الأحدث لـ CLI والمستندات للمخطط Norito. |
| SoraFS | فريق التخزين | قام برنامج Quickstart بقراءة الفيلم، حيث يقوم بالإبلاغ عن البيانات الموثقة، وتعليمات المحاكاة لجلب التحقق. |
| أدلة SDK | يؤدي SDK | يقوم Guias Rust/Python/JS بتجميع الأمثلة حاليًا وربطها للتخزين المباشر. |
| مرجع | مستندات/ديفريل | قائمة الفهرس حسب أحدث المواصفات، مرجعها إلى برنامج الترميز Norito يتزامن مع `norito.md`. |
| معاينة القطعة الأثرية | مستندات/ديفريل | O artefato `docs-portal-preview` ista anexado ao PR, smoke checks passam, o link e compartilhado com reviewers. |
| الأمان وجرب وضع الحماية | مستندات/DevRel / الأمن | تكوين تسجيل الدخول إلى رمز جهاز OAuth (`DOCS_OAUTH_*`)، وقائمة التحقق `security-hardening.md` المنفذة، ورؤوس CSP/الأنواع الموثوقة التي تم التحقق منها عبر `npm run build` أو `npm run probe:portal`. |

قم بتمييز كل جزء من الجزء الخاص بك بمراجعة العلاقات العامة، أو قم بتدوين مهام المتابعة للمساعدة في تتبع الحالة بدقة.

## 4. ملاحظات الإصدار- قم بتضمين `https://docs.iroha.tech/` (أو عنوان URL المحيط بمهمة النشر) في ملاحظات الإصدار وتحديث الحالة.
- قم بإزالة كل ما هو جديد أو متغير حتى تتمكن معدات سايبام من إعادة تنفيذ اختبارات الدخان الخاصة بها.