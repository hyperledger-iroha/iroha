---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Guia de exposicion del host de preview

El roadmap DOCS-SORA exige que cada preview publico use el mismo bundle verificado por checksum que los revisores prueban localmente. Usa este runbook despues de completar el onboarding de revisores (y el ticket de aprobacion de invitaciones) para poner en linea el host de preview beta.

## Requisitos previos

- Ola de onboarding de revisores aprobada y registrada en el tracker de preview.
- Ultimo build del portal presente en `docs/portal/build/` y checksum verificado (`build/checksums.sha256`).
- Credenciales de preview SoraFS (URL de Torii, autoridad, llave privada, epoch enviado) almacenadas en variables de entorno o en un config JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de cambio DNS abierto con el hostname deseado (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) mas contactos on-call.

## Paso 1 - Construir y verificar el bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

El script de verificacion se niega a continuar cuando el manifiesto de checksum falta o fue manipulado, manteniendo auditado cada artefacto de preview.

## Paso 2 - Empaquetar los artefactos SoraFS

Convierte el sitio estatico en un par CAR/manifest determinista. `ARTIFACT_DIR` por defecto es `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Adjunta `portal.car`, `portal.manifest.*`, el descriptor y el manifiesto de checksum al ticket de la ola de preview.

## Paso 3 - Publicar el alias de preview

Repite el helper de pin **sin** `--skip-submit` cuando estes listo para exponer el host. Proporciona el config JSON o flags CLI explicitos:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

El comando escribe `portal.pin.report.json`, `portal.manifest.submit.summary.json` y `portal.submit.response.json`, que deben viajar con el bundle de evidencia de invitaciones.

## Paso 4 - Generar el plan de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Comparte el JSON resultante con Ops para que el cambio DNS referencie el digest del manifiesto exacto. Cuando reutilices un descriptor anterior como fuente de rollback, agrega `--previous-dns-plan path/to/previous.json`.

## Paso 5 - Probar el host desplegado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

El probe confirma el tag de release servido, headers CSP y metadatos de firma. Repite el comando desde dos regiones (o adjunta la salida de curl) para que los auditores vean que el edge cache esta caliente.

## Bundle de evidencia

Incluye los siguientes artefactos en el ticket de la ola de preview y referencialos en el email de invitacion:

| Artefacto | Proposito |
|----------|-----------|
| `build/checksums.sha256` | Demuestra que el bundle coincide con el build de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Payload canonico SoraFS + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Muestra que el envio del manifiesto + el alias binding se completaron. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadatos DNS (ticket, ventana, contactos), resumen de promocion de ruta (`Sora-Route-Binding`), el puntero `route_plan` (plan JSON + plantillas de header), info de purge de cache e instrucciones de rollback para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descriptor firmado que enlaza el archive + checksum. |
| Salida de `probe` | Confirma que el host en vivo anuncia el tag de release esperado. |
