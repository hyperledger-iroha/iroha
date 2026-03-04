---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ہوسٹ ایکسپوژر گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی پریویو وہی paquete verificado con suma de comprobación استعمال کرے جو ریویورز لوکل طور پر چلاتے ہیں۔ ریویور آن بورڈنگ (اور invite منظوری ٹکٹ) مکمل ہونے کے بعد اس runbook کو استعمال کریں تاکہ بیٹا پریویو ہوسٹ آن لائن لایا جا سکے۔

## پیشگی شرائط

- ریویور آن بورڈنگ ویو منظور اور پریویو ٹریکر میں درج ہو چکی ہو۔
- تازہ ترین پورٹل build `docs/portal/build/` میں موجود ہو اور checksum تصدیق شدہ ہو (`build/checksums.sha256`).
- SoraFS پریویو اسناد (Torii URL, autoridad, clave privada, جمع شدہ época) ماحولاتی ویری ایبلز میں یا Configuración JSON میں محفوظ ہوں جیسے [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Nombre de host principal (`docs-preview.sora.link`, `docs.iroha.tech` وغیرہ) کے ساتھ DNS تبدیلی ٹکٹ کھلا ہو اور de guardia رابطے شامل ہوں۔

## مرحلہ 1 - paquete بنائیں اور verificar کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

verificar el manifiesto de suma de comprobación کے غائب یا چھیڑ چھاڑ ہونے پر آگے بڑھنے سے انکار کرتا ہے، جس سے ہر پریویو آرٹیفیکٹ آڈٹ میں رہتا ہے۔

## مرحلہ 2 - SoraFS artefactos پیک کریں

اسٹیٹک سائٹ کو determinista CAR/manifiesto جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` کی ڈیفالٹ ویلیو `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

تیار شدہ `portal.car`, `portal.manifest.*`, descriptor, اور checksum manifest کو پریویو ویو ٹکٹ کے ساتھ منسلک کریں۔

## مرحلہ 3 - پریویو alias شائع کریںجب ہوسٹ ایکسپوز کرنے کے لئے تیار ہوں تو pin helper کو **بغیر** `--skip-submit` کے دوبارہ چلائیں۔ Configuración JSON y banderas CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

یہ کمانڈ `portal.pin.report.json`, `portal.manifest.submit.summary.json` اور `portal.submit.response.json` لکھتی ہے، جو paquete de pruebas de invitación کے ساتھ ہونی چاہئیں۔

## مرحلہ 4 - Transferencia de DNS پلان بنائیں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Configuración de JSON, Operaciones, configuración de DNS, resumen de manifiesto y resumen de manifiesto اگر rollback کے لئے پچھلا descriptor استعمال کیا جا رہا ہو تو `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - ڈیپلائےڈ ہوسٹ کو sonda کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

sonda, etiqueta de lanzamiento, encabezados CSP, metadatos de firma, etc. دو ریجنز سے کمانڈ دوبارہ چلائیں (یا curl salida منسلک کریں) تاکہ آڈیٹرز دیکھ سکیں کہ caché de borde گرم ہے۔

## Paquete de evidencia

پریویو ویو ٹکٹ میں درج ذیل artefactos شامل کریں اور دعوتی ای میل میں ان کا حوالہ دیں:| Artefacto | مقصد |
|----------|------|
| `build/checksums.sha256` | ثابت کرتا ہے کہ paquete CI build سے میل کھاتا ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | carga útil canónica SoraFS + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | envío de manifiesto اور enlace de alias کی کامیابی دکھاتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadatos DNS (ٹکٹ، ونڈو، رابطے), promoción de ruta (`Sora-Route-Binding`) کا خلاصہ، `route_plan` puntero (JSON پلان + plantillas de encabezado), purga de caché معلومات، اور Ops کے لئے reversión ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | دستخط شدہ descriptor جو archivo + suma de comprobación کو جوڑتا ہے۔ |
| Salida `probe` | تصدیق کرتا ہے کہ لائیو ہوسٹ متوقع etiqueta de lanzamiento دکھا رہا ہے۔ |