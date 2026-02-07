---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guía de exposición del host de vista previa

El roadmap DOCS-SORA exige que cada vista previa pública utilice el mismo paquete verificado por checksum que los revisores prueban localmente. Usa este runbook después de completar el onboarding de revisores (y el ticket de aprobación de invitaciones) para poner en línea el host devista previa beta.

##Requisitos previos

- Ola de onboarding de revisores aprobados y registrados en el rastreador de vista previa.
- Última compilación del portal presente en `docs/portal/build/` y checksum verificado (`build/checksums.sha256`).
- Credenciales de vista previa SoraFS (URL de Torii, autoridad, llave privada, época enviada) almacenadas en variables de entorno o en una configuración JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de cambio DNS abierto con el nombre de host deseado (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) más contactos de guardia.

## Paso 1 - Construir y verificar el paquete

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

El script de verificación se niega a continuar cuando el manifiesto de checksum falta o fue manipulado, manteniendo auditado cada artefacto de vista previa.

## Paso 2 - Empaquetar los artefactos SoraFS

Convierte el sitio estático en un par CAR/manifest determinista. `ARTIFACT_DIR` por defecto es `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Adjunta `portal.car`, `portal.manifest.*`, el descriptor y el manifiesto de suma de comprobación al ticket de la ola de vista previa.## Paso 3 - Publicar el alias de vista previa

Repite el helper de pin **sin** `--skip-submit` cuando estes listo para exponer el host. Proporciona la configuración JSON o flags CLI explícita:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

El comando escribe `portal.pin.report.json`, `portal.manifest.submit.summary.json` y `portal.submit.response.json`, que deben viajar con el paquete de evidencia de invitaciones.

## Paso 4 - Generar el plan de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Comparte el JSON resultante con Ops para que el cambio DNS haga referencia al resumen del manifiesto exacto. Cuando reutilice un descriptor anterior como fuente de rollback, agregue `--previous-dns-plan path/to/previous.json`.

## Paso 5 - Probar el host desplegado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonda confirma la etiqueta de liberación servida, encabezados CSP y metadatos de firma. Repite el comando desde dos regiones (o adjunta la salida de curl) para que los auditores vean que el edge cache esté caliente.

## Paquete de evidencia

Incluye los siguientes artefactos en el ticket de la ola de vista previa y referencias en el correo electrónico de invitación:| Artefacto | propuesta |
|----------|-----------|
| `build/checksums.sha256` | Demuestra que el paquete coincide con el build de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga útil canónico SoraFS + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Muestra que el envío del manifiesto + el alias vinculante se completaron. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadatos DNS (ticket, ventana, contactos), resumen de promoción de ruta (`Sora-Route-Binding`), el puntero `route_plan` (plan JSON + plantillas de encabezado), información de purga de caché e instrucciones de rollback para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descriptor firmado que enlaza el archivo + suma de comprobación. |
| Salida de `probe` | Confirma que el host en vivo anuncia el tag de lanzamiento esperado. |