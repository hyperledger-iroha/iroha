---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل تعريض مضيف المعاينة

Verifique se o documento DOCS-SORA está disponível para verificação de soma de verificação. يختبرها المراجعون محليا. استخدم هذا الدليل بعد اكتمال تأهيل المراجعين (وتذكرة اعتماد الدعوات) لوضع مضيف المعاينة التجريبي على الشبكة.

## المتطلبات المسبقة

- اعتماد موجة تأهيل المراجعين e تسجيلها في متعقب المعاينة.
- Você pode usar o `docs/portal/build/` e a soma de verificação (`build/checksums.sha256`).
- بيانات اعتماد معاينة SoraFS (عنوان Torii, الجهة المخولة, المفتاح الخاص, epoch المرسل) مخزنة O arquivo JSON é [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- فتح تذكرة تغيير DNS بالاسم المرغوب (`docs-preview.sora.link`, `docs.iroha.tech`, ... ) بالاضافة الى جهات الاتصال المناوبة.

## الخطوة 1 - بناء الحزمة والتحقق منها

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

يرفض سكربت التحقق المتابعة عند غياب بيان checksum او العبث به, ما يبقي كل اثر معاينة مدققا.

## Passo 2 - Passo 2 SoraFS

حوّل الموقع الثابت الى زوج CAR/manifest حتمي. `ARTIFACT_DIR` é compatível com `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car` e `portal.manifest.*` são usados ​​e a soma de verificação é definida como uma soma de verificação.

## الخطوة 3 - نشر alias المعاينة

Certifique-se de que o pino **بدون** `--skip-submit` seja removido do lugar. Usando JSON e CLI como:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Você pode usar `portal.pin.report.json` e `portal.manifest.submit.summary.json` e `portal.submit.response.json`, e você pode fazer isso sem problemas Então.

## Passo 4 - Como configurar o DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

O JSON do arquivo de Ops é definido como o DNS do digest do manifesto. Você pode usar o rollback e `--previous-dns-plan path/to/previous.json`.

## الخطوة 5 - فحص المضيف المنشور

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Você pode usar o CSP e o CSP para obter mais informações. اعد الامر من منطقتين (او ارفق خرج curl) حتى يرى المدققون ان cache الحافة ساخن.

## حزمة الادلة

ضمن العناصر التالية في تذكرة موجة المعاينة واشر اليها في بريد الدعوة:

| الاثر | الغرض |
|-------|-------|
| `build/checksums.sha256` | يثبت ان الحزمة تطابق بناء CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Nome do arquivo SoraFS + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | يثبت نجاح ارسال الـ manifest وربط الـ alias. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS (`Sora-Route-Binding`), `route_plan` (`Sora-Route-Binding`) (só JSON + cabeçalho de cabeçalho), como purge e rollback para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | واصف موقع يربط الارشيف + soma de verificação. |
| Cabo `probe` | Não há nada que você possa fazer e que não seja adequado. |