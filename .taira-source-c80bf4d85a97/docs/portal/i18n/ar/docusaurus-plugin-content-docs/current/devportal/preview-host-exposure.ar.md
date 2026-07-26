---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل ترحيل المعاينة

تتطلب بخارة طريق DOCS-SORA ان يعتمد كل معاينة عامة على نفس التقرير العدلي بالـ المجموع الاختباري الذي يختبرها المراجعون محليا. استخدم هذا الدليل بعد القانوني لتأهيل المراجعين (وتذكرة تعتمد الدعوات) لتضيف المعاينة التجريبية على الشبكة.

##المتطلبات المسبقة

- اعتماد شهادة تأهيل المراجعين وتسجيلها في متعقب المعاينة.
- آخر بناء للبوابة موجود تحت `docs/portal/build/` وتم التحقق من المجموع الاختباري (`build/checksums.sha256`).
- بيانات تعتمد معاينة SoraFS (عنوان Torii، وصول المخولة، المفتاح الخاص، عصر المرسل) مخزنة اما في تنوعات البيئة او في ملف JSON مثل [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- فتح تذكرة تغيير DNS بالاسم (`docs-preview.sora.link`, `docs.iroha.tech`, ... ) بالاضافة الى جهات الاتصال المناوبة.

## العنصر 1 - بناء المادة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

رفض سكربت التحقق من المتابعة عند غياب بيان المجموع الاختباري او العبث به، ما يبقي كل معاينة معاينة مترا.

## الخطوة 2 - معالجات السرطان SoraFS

تحويل الموقع الثابت الى زوج CAR/manifest حتمي. `ARTIFACT_DIR` افتراضي هو `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

ارفق `portal.car` و `portal.manifest.*` والواصف والبيان الاختباري بتذكر موجة المعاينة.

## الخطوة 3 - نشر الاسم المستعار للمعاينة

اعد تشغيل مساعد pin **بدون** `--skip-submit` عندما تكون مستعدا للائحة الوطنية. تقديم ملف JSON او رايات CLI صريح:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

يكتب الأمر `portal.pin.report.json` و `portal.manifest.submit.summary.json` و `portal.submit.response.json`، والتي يجب ان ترافق حزمة المعادلة الخاصة بالدعوات.

## الخطوة الرابعة - إنشاء خطة تحويل DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

شارك في ملف JSON الناتج مع فريق Ops حتى يشير إلى تغيير DNS حتى يهضم الدقيق للـ Manifest. عند إعادة استخدام واصف سابق كمصدر التراجع، اضف ​​`--previous-dns-plan path/to/previous.json`.

## الخطوة 5 - استقبال فحص المنشور

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

قام بتوقيع القلم الاصدار المخدوم، ترويسات CSP، وبيانات التوقيع. اعد الأمر من المنطقتين (او ارفق تفريغ الضفيرة) حتى يرى المرقمون ان مخبأ الصفر.

## حزمة المعادلة

ضمن العناصر التالية في تذكرة موجة المعاينة واشر إليها في بريد التحرير:

| الاثر | اللحوم |
|-------|-------|
| `build/checksums.sha256` | يثبت ان الحزمة تطابق بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | خفيفة الوزن SoraFS + مانيفست. |
| `portal.pin.report.json`، `portal.manifest.submit.summary.json`، `portal.submit.response.json` | أثبت نجاح الـ Manifest وربط الـ alias. |
| `artifacts/sorafs/portal.dns-cutover.json` | بيانات DNS (التذكرة، النافذة، جهات الاتصال)، ملخص المسار (`Sora-Route-Binding`)، مؤشر `route_plan` (خطة JSON + رأس القالب)، معلومات التطهير للذاكرة المؤقتة ولمات التراجع عن Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع ربط الارشيف + المجموع الاختباري. |
| تحلل `probe` | هي الدورة الشهرية التي أعلنت عن الاصدار المتوقع. |