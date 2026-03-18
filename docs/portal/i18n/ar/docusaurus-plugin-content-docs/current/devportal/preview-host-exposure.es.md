---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل عرض مضيف المعاينة

تتطلب خريطة الطريق DOCS-SORA أن تقوم كل معاينة عامة باستخدام نفس الحزمة التي تم التحقق منها من خلال المجموع الاختباري الذي تم اختباره محليًا. يتم استخدام هذا الدليل بعد إكمال إعداد المراجعين (وتذكرة الموافقة على الدعوات) لبدء تشغيل مضيف المعاينة التجريبي عبر الإنترنت.

## المتطلبات السابقة

- قم بإعداد التحديثات المعتمدة والتسجيل في متتبع المعاينة.
- تم إنشاء البوابة الأخيرة في `docs/portal/build/` وتم التحقق من المجموع الاختباري (`build/checksums.sha256`).
- اعتمادات معاينة SoraFS (URL لـ Torii، تلقائي، مفتاح خاص، عصر مرسل) مخزنة في متغيرات Entorno أو في تكوين JSON مثل [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- تذكرة تغيير DNS مفتوحة مع اسم المضيف المطلوب (`docs-preview.sora.link`، `docs.iroha.tech`، وما إلى ذلك) مع جهات الاتصال عند الطلب.

## الخطوة 1 - إنشاء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

لن يستمر نص التحقق في الاستمرار عندما يكون بيان المجموع الاختباري خاطئًا أو تم التلاعب به، مع الاستمرار في تدقيقه كل معاينة مصطنعة.

## باسو 2 - تعبئة القطع الأثرية SoraFS

تحويل الموقع الثابت إلى مستوى سيارة/بيان محدد. `ARTIFACT_DIR` بسبب الخلل `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

ملحق `portal.car`، `portal.manifest.*`، الواصف وبيان المجموع الاختباري لبطاقة المعاينة.

## الخطوة 3 - نشر الاسم المستعار للمعاينة

كرر مساعد الدبوس **الخطيئة** `--skip-submit` عندما تكون هذه القائمة لتوضيح المضيف. دعم التكوين JSON أو علامات CLI الواضحة:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

يكتب الأمر `portal.pin.report.json` و`portal.manifest.submit.summary.json` و`portal.submit.response.json`، والذي يجب أن يسافر مع حزمة أدلة الدعوات.

## الخطوة 4 - إنشاء خطة قص DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

قم بمقارنة ملف JSON الناتج مع العمليات حتى يشير تغيير DNS إلى ملخص البيان الدقيق. عند إعادة استخدام واصف سابق مثل التراجع، قم بإضافة `--previous-dns-plan path/to/previous.json`.

## الخطوة 5 - هزيمة المضيف

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

يؤكد المسبار علامة إصدار الخادم ورؤوس CSP وبيانات الشركة. كرر الأمر من المنطقتين (أو قم بإضافة لفة اللف) حتى يرى المدققون أن ذاكرة التخزين المؤقت الحافة دافئة.

## حزمة الأدلة

قم بتضمين العناصر التالية في تذكرة المعاينة والمراجع في رسالة البريد الإلكتروني للدعوة:

| قطعة أثرية | اقتراح |
|----------|----------|
| `build/checksums.sha256` | Demuestra أن الحزمة تتزامن مع بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة النافعة canonico SoraFS + توضح ذلك. |
| `portal.pin.report.json`، `portal.manifest.submit.summary.json`، `portal.submit.response.json` | يُشار إلى أن إرسال البيان + الرابط الاسمي المستعار قد اكتمل. |
| `artifacts/sorafs/portal.dns-cutover.json` | بيانات تعريف DNS (التذكرة، النافذة، جهات الاتصال)، استئناف الترويج للمسار (`Sora-Route-Binding`)، المنفذ `route_plan` (خطة JSON + مجموعات الرأس)، معلومات تنظيف ذاكرة التخزين المؤقت وتعليمات التراجع لـ Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف ثابت يقوم بتعبئة الأرشيف + المجموع الاختباري. |
| ساليدا دي `probe` | تأكد من أن المضيف في الحياة سيعلن عن علامة الإصدار المتوقعة. |