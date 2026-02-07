---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تعريض مضيف المعاينة

Utilice el código DOCS-SORA para obtener una suma de comprobación de la suma de comprobación correspondiente. المراجعون محليا. استخدم هذا الدليل بعد اكتمال تأهيل المراجعين (وتذكرة اعتماد الدعوات) لوضع مضيف المعاينة التجريبي على الشبكة.

## المتطلبات المسبقة

- اعتماد موجة تأهيل المراجعين وتسجيلها في متعقب المعاينة.
- Utilice la suma de comprobación `docs/portal/build/` y la suma de comprobación (`build/checksums.sha256`).
- مخزنة اما في Utilice el archivo JSON [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Utilice el servidor DNS (`docs-preview.sora.link`, `docs.iroha.tech`, ...) para configurar el servidor DNS.

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

حوّل الموقع الثابت الى زوج CAR/manifiesto حتمي. `ARTIFACT_DIR` افتراضيا هو `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

ارفق `portal.car` e `portal.manifest.*` y la suma de comprobación del dispositivo.

## الخطوة 3 - نشر alias المعاينة

اعد تشغيل مساعد pin **بدون** `--skip-submit` عندما تكون مستعدا لتعريض المضيف. Utilice el código JSON y la CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

يكتب الامر `portal.pin.report.json` و `portal.manifest.submit.summary.json` و `portal.submit.response.json`, والتي يجب ان ترافق حزمة الادلة الخاصة بالدعوات.## الخطوة 4 - توليد خطة تحويل DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Utilice un código JSON para configurar Ops y crear un resumen de DNS para el manifiesto. Utilice la opción `--previous-dns-plan path/to/previous.json` para realizar la reversión.

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
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة القياسية SoraFS + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | يثبت نجاح ارسال الـ manifiesto وربط الـ alias. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS (es decir, DNS) (`Sora-Route-Binding`), `route_plan` (JSON + قوالب header), معلومات purge للذاكرة المؤقتة وتعليمات rollback لفريق Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع يربط الارشيف + suma de comprobación. |
| Texto `probe` | يؤكد ان المضيف الحي يعلن وسم الاصدار المتوقع. |