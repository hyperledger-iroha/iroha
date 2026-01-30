---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fc5d28b83f54da34dc967b213588ac0034eddcaf4f857cc66aec51be0906eb63
source_last_modified: "2025-11-14T04:43:19.996623+00:00"
translation_last_reviewed: 2026-01-30
---

# دليل تعريض مضيف المعاينة

تتطلب خارطة طريق DOCS-SORA ان يعتمد كل معاينة عامة على نفس الحزمة المحققة بالـ checksum التي يختبرها المراجعون محليا. استخدم هذا الدليل بعد اكتمال تأهيل المراجعين (وتذكرة اعتماد الدعوات) لوضع مضيف المعاينة التجريبي على الشبكة.

## المتطلبات المسبقة

- اعتماد موجة تأهيل المراجعين وتسجيلها في متعقب المعاينة.
- آخر بناء للبوابة موجود تحت `docs/portal/build/` وتم التحقق من checksum (`build/checksums.sha256`).
- بيانات اعتماد معاينة SoraFS (عنوان Torii، الجهة المخولة، المفتاح الخاص، epoch المرسل) مخزنة اما في متغيرات البيئة او في ملف JSON مثل [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- فتح تذكرة تغيير DNS بالاسم المرغوب (`docs-preview.sora.link`, `docs.iroha.tech`, ... ) بالاضافة الى جهات الاتصال المناوبة.

## الخطوة 1 - بناء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

يرفض سكربت التحقق المتابعة عند غياب بيان checksum او العبث به، ما يبقي كل اثر معاينة مدققا.

## الخطوة 2 - حزم آثار SoraFS

حوّل الموقع الثابت الى زوج CAR/manifest حتمي. `ARTIFACT_DIR` افتراضيا هو `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

ارفق `portal.car` و `portal.manifest.*` والواصف وبيان checksum بتذكرة موجة المعاينة.

## الخطوة 3 - نشر alias المعاينة

اعد تشغيل مساعد pin **بدون** `--skip-submit` عندما تكون مستعدا لتعريض المضيف. قدم ملف JSON او رايات CLI صريحة:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

يكتب الامر `portal.pin.report.json` و `portal.manifest.submit.summary.json` و `portal.submit.response.json`، والتي يجب ان ترافق حزمة الادلة الخاصة بالدعوات.

## الخطوة 4 - توليد خطة تحويل DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

شارك ملف JSON الناتج مع فريق Ops حتى يشير تغيير DNS الى digest الدقيق للـ manifest. عند اعادة استخدام واصف سابق كمصدر rollback، اضف `--previous-dns-plan path/to/previous.json`.

## الخطوة 5 - فحص المضيف المنشور

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

يؤكد الفحص وسم الاصدار المخدوم، ترويسات CSP، وبيانات التوقيع. اعد الامر من منطقتين (او ارفق خرج curl) حتى يرى المدققون ان cache الحافة ساخن.

## حزمة الادلة

ضمن العناصر التالية في تذكرة موجة المعاينة واشر اليها في بريد الدعوة:

| الاثر | الغرض |
|-------|-------|
| `build/checksums.sha256` | يثبت ان الحزمة تطابق بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة القياسية SoraFS + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | يثبت نجاح ارسال الـ manifest وربط الـ alias. |
| `artifacts/sorafs/portal.dns-cutover.json` | بيانات DNS (التذكرة، النافذة، جهات الاتصال)، ملخص ترقية المسار (`Sora-Route-Binding`)، مؤشر `route_plan` (خطة JSON + قوالب header)، معلومات purge للذاكرة المؤقتة وتعليمات rollback لفريق Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع يربط الارشيف + checksum. |
| خرج `probe` | يؤكد ان المضيف الحي يعلن وسم الاصدار المتوقع. |
