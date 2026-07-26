---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تعريض مضيف المعاينة

Utilisez le document DOCS-SORA pour obtenir une somme de contrôle يختبرها المراجعون محليا. استخدم هذا الدليل بعد اكتمال تأهيل المراجعين (وتذكرة اعتماد الدعوات) لوضع مضيف المعاينة التجريبي على الشبكة.

## المتطلبات المسبقة

- اعتماد موجة تأهيل المراجعين وتسجيلها في متعقب المعاينة.
- Vous pouvez utiliser la somme de contrôle `docs/portal/build/` et la somme de contrôle (`build/checksums.sha256`).
- بيانات اعتماد معاينة SoraFS (عنوان Torii, الجهة المخولة، المفتاح الخاص، epoch المرسل) مخزنة اما Les fichiers PDF sont également JSON comme [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- فتح تذكرة تغيير DNS by المرغوب (`docs-preview.sora.link`, `docs.iroha.tech`, ... ) byاضافة الى جهات الاتصال المناوبة.

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

حوّل الموقع الثابت الى زوج CAR/manifeste حتمي. `ARTIFACT_DIR` est compatible avec `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

La somme de contrôle `portal.car` et `portal.manifest.*` est également utilisée pour la somme de contrôle.

## الخطوة 3 - نشر alias المعاينة

اعد تشغيل مساعد pin **بدون** `--skip-submit` عندما مستعدا لتعريض المضيف. JSON et CLI sont utilisés :

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

يكتب الامر `portal.pin.report.json` و `portal.manifest.submit.summary.json` و `portal.submit.response.json` ، والتي يجب ان ترافق حزمة الادلة الخاصة بالدعوات.## الخطوة 4 - توليد خطة تحويل DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Vous pouvez utiliser JSON pour Ops et utiliser DNS pour digérer le manifeste. La procédure de restauration est basée sur `--previous-dns-plan path/to/previous.json`.

## الخطوة 5 - فحص المضيف المنشور

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Il existe également des liens vers le CSP et des liens vers les réseaux sociaux. اعد الامر من منطقتين (او ارفق خرج curl) حتى يرى المدققون ان cache الحافة ساخن.

## حزمة الادلة

ضمن العناصر التالية في تذكرة موجة المعاينة واشر اليها في بريد الدعوة:

| الاثر | الغرض |
|-------|-------|
| `build/checksums.sha256` | يثبت ان الحزمة تطابق بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | الحمولة القياسية SoraFS + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | يثبت نجاح ارسال الـ manifest وربط الـ alias. |
| `artifacts/sorafs/portal.dns-cutover.json` | Les DNS (pour les noms de domaine) et les noms de domaine (`Sora-Route-Binding`) sont pour `route_plan` (pour JSON + قوالب header) ، معلومات purge للذاكرة المؤقتة وتعليمات rollback لفريق Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع يربط الارشيف + somme de contrôle. |
| Voir `probe` | يؤكد ان المضيف الحي يعلن وسم الاصدار المتوقع. |