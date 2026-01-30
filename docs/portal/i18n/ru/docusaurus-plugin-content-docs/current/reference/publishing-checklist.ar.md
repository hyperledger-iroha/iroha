---
lang: ru
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# قائمة التحقق للنشر

استخدم قائمة التحقق هذه كلما قمت بتحديث بوابة المطورين. تضمن ان بناء CI ونشر GitHub Pages واختبارات smoke اليدوية تغطي كل قسم قبل وصول اصدار او محطة طريق.

## 1. التحقق المحلي

- `npm run sync-openapi -- --version=current --latest` (اضف علما او اكثر `--mirror=<label>` عندما يتغير Torii OpenAPI لاخذ لقطة مجمدة).
- `npm run build` – تحقق من ان نص الـ hero `Build on Iroha with confidence` ما زال يظهر في `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – تحقق من بيان checksums (اضف `--descriptor`/`--archive` عند اختبار artefacts محملة من CI).
- `npm run serve` – يشغل مساعد preview المحمي بالـ checksum الذي يتحقق من البيان قبل استدعاء `docusaurus serve`، حتى لا يتصفح المراجعون لقطة غير موقعة (يبقى alias `serve:verified` متاحا للاستدعاءات الصريحة).
- راجع الـ markdown الذي لمسته عبر `npm run start` وخادم live reload.

## 2. فحوصات طلب السحب

- تحقق من نجاح مهمة `docs-portal-build` في `.github/workflows/check-docs.yml`.
- تاكد من تشغيل `ci/check_docs_portal.sh` (سجلات CI تظهر hero smoke check).
- تاكد ان سير عمل preview رفع بيانا (`build/checksums.sha256`) وان سكربت التحقق من preview نجح (تظهر السجلات خرج `scripts/preview_verify.sh`).
- اضف عنوان URL للمعاينة المنشورة من بيئة GitHub Pages الى وصف PR.

## 3. اعتماد الاقسام

| القسم | المالك | قائمة التحقق |
|---------|-------|-----------|
| Homepage | DevRel | يظهر نص الـ hero، وبطاقات quickstart تربط بمسارات صالحة، وازرار CTA تعمل. |
| Norito | Norito WG | ادلة overview و getting-started تشير الى اخر flags للـ CLI ووثائق مخطط Norito. |
| SoraFS | Storage Team | يعمل quickstart حتى الاكتمال، وحقول تقرير manifest موثقة، وتعليمات محاكاة fetch متحققة. |
| SDK guides | قادة SDK | ادلة Rust/Python/JS تقوم بترجمة الامثلة الحالية وتربط بمستودعات حية. |
| Reference | Docs/DevRel | يسرد الفهرس احدث specs، ومرجع Norito codec يطابق `norito.md`. |
| Preview artifact | Docs/DevRel | تم ارفاق artefact `docs-portal-preview` بالـ PR، واجتازت اختبارات smoke، وتمت مشاركة الرابط مع المراجعين. |
| Security & Try it sandbox | Docs/DevRel · Security | تم تهيئة OAuth device-code login (`DOCS_OAUTH_*`)، وتنفيذ قائمة `security-hardening.md`، والتحقق من ترويسات CSP/Trusted Types عبر `npm run build` او `npm run probe:portal`. |

علّم كل صف كجزء من مراجعة PR، او دوّن اي مهام متابعة حتى يبقى تتبع الحالة دقيقا.

## 4. ملاحظات الاصدار

- ادرج `https://docs.iroha.tech/` (او عنوان بيئة النشر من مهمة النشر) في ملاحظات الاصدار وتحديثات الحالة.
- اذكر اي اقسام جديدة او معدلة بوضوح حتى تعرف الفرق التابعة اين تعيد تشغيل اختبارات smoke الخاصة بها.
