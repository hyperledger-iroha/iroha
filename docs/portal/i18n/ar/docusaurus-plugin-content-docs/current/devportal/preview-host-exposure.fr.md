---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل عرض المعاينة الساخنة

تتطلب خطوة المسار DOCS-SORA أن يتم الضغط على كل معاينة عامة على حزمة البيانات للتحقق من المجموع الاختباري الذي يختبره المراجعون محليًا. استخدم دليل التشغيل هذا بعد تأهيل المحاضرين (وتذكرة الموافقة على الدعوات) من أجل التواصل عبر الإنترنت في الإصدار التجريبي الساخن.

## المتطلبات الأساسية

- غامضة بشأن الموافقة على قبول المراجعين وتسجيلهم في معاينة المتتبع.
- أحدث إنشاء للبوابة يقدم Sous `docs/portal/build/` والتحقق من المجموع الاختباري (`build/checksums.sha256`).
- معاينة SoraFS للمعرفات (URL Torii، autorite، cle privee، epoch soumis) في متغيرات البيئة أو تكوين JSON tel que [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- تذكرة تغيير DNS تفتح مع الشخص المرغوب (`docs-preview.sora.link`، `docs.iroha.tech`، وما إلى ذلك) بالإضافة إلى جهات الاتصال عند الطلب.

## الشريط 1 - إنشاء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

يرفض نص التحقق الاستمرار عند تغيير بيان المجموع الاختباري أو تغييره، مما يؤدي إلى مراجعة كل قطعة أثرية للمعاينة.

## الشريط 2 - أداة تجميع القطع الأثرية SoraFS

قم بتحويل إحصائيات الموقع إلى زوج من CAR/البيان المحدد. `ARTIFACT_DIR` هو الاسم الافتراضي `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, الواصف وبيان المجموع الاختباري في تذكرة المعاينة الغامضة.

## الشريط 3 - معاينة الاسم المستعار للناشر

Relancez le helper de pin **sans** `--skip-submit` عندما تقوم بكشف الحرارة. Fournissez soit le config JSON soit des flags CLI صريحة:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

الأمر الذي سيكتب `portal.pin.report.json` و`portal.manifest.submit.summary.json` و`portal.submit.response.json`، هو ما يجب عليه إرفاق حزمة أدلة الدعوات.

## الشريط 4 - إنشاء خطة DNS الأساسية

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

قم بمشاركة JSON الناتج مع العمليات حتى تتمكن من الرجوع إلى DNS الأساسي للحصول على ملخص البيان الدقيق. عند إعادة استخدام واصف السابقة كمصدر للتراجع، قم بإضافة `--previous-dns-plan path/to/previous.json`.

## الشريط 5 - Sondage de l'hoteplodee

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

يؤكد المسبار علامة إصدار الخدمة ورؤوس CSP وبيانات التوقيع. أعد ضبط الأمر من منطقتين (أو قم بتحريك لفة) حتى يرى المراجعون أن حافة ذاكرة التخزين المؤقت أصبحت ساخنة.

## حزمة الأدلة

قم بتضمين المصنوعات اليدوية التالية في تذكرة المعاينة الغامضة والرجوع إليها في رسالة الدعوة الإلكترونية:

| قطعة أثرية | موضوعي |
|----------|---------|
| `build/checksums.sha256` | أثبت أن الحزمة تتوافق مع بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة SoraFS كانونيك + مانيفست. |
| `portal.pin.report.json`، `portal.manifest.submit.summary.json`، `portal.submit.response.json` | Montre que la soumission du Manifest + le Binding d'alias ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadonnees DNS (التذكرة، النافذة، جهات الاتصال)، استئناف ترقية المسار (`Sora-Route-Binding`)، المؤشر `route_plan` (خطة JSON + قوالب الرأس)، معلومات تنظيف ذاكرة التخزين المؤقت وتعليمات التراجع للعمليات. |
| `artifacts/sorafs/preview-descriptor.json` | واصف العلامة التي تكمن في الأرشيف + المجموع الاختباري. |
| طلعة جوية `probe` | تأكد من أن برنامج hote en ligne يعلن عن علامة الإصدار الحالية. |