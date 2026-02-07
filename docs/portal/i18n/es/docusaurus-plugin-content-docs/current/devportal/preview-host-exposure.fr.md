---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guía de exposición del hotel de vista previa

La hoja de ruta DOCS-SORA exige que cada vista previa pública se aplique en el paquete de memes para verificar la suma de verificación de los lectores que prueban la ubicación. Utilice este runbook después de la incorporación de los lectores (y el ticket de aprobación de invitaciones) para conectarse a la línea beta.

## Requisitos previos

- Incorporación vaga de los lectores aprobados y registrados en la vista previa del rastreador.
- La última construcción del portal está presente en `docs/portal/build/` y verifica la suma de verificación (`build/checksums.sha256`).
- Vista previa de los identificadores SoraFS (URL Torii, autorite, cle privee, epoch soumis) almacenados en las variables de entorno o una configuración JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de cambio DNS abierto con el hotel souhaite (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) más contactos de guardia.

## Etapa 1 - Construir y verificar el paquete

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

El script de verificación rechaza continuar cuando el manifiesto de verificación manque o esté alterado, ce qui garde cada artefacto de vista previa auditada.

## Etapa 2 - Empaquetador de artefactos SoraFS

Convertissez le site statique en paire CAR/manifest deterministe. `ARTIFACT_DIR` es el `docs/portal/artifacts/` predeterminado.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, le descripteur et le manifeste de checksum au ticket de la vague previa.

## Etapa 3 - Vista previa del editor aliasRelancez le helper de pin **sans** `--skip-submit` lorsque vous etes pret a exponenr l'hote. Al configurar la configuración JSON en la CLI de flags, se especifica lo siguiente:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

La orden escrita `portal.pin.report.json`, `portal.manifest.submit.summary.json` y `portal.submit.response.json`, que debe acompañar el paquete de pruebas de invitaciones.

## Etapa 4 - Generar el plan de DNS basculante

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Partagez the JSON resultant avec Ops afin que la bascule DNS reference le digest exactitud du manifeste. Si un descriptor precedente se reutiliza como fuente de reversión, agregue `--previous-dns-plan path/to/previous.json`.

## Etapa 5 - Sonda del hoyo desplegable

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonda confirma la etiqueta de servicio de liberación, los encabezados CSP y los metadones de firma. Relancez la commande depuis dos regiones (ou joignez une sortie curl) afin que les auditeurs voient que le cache edge est chaud.

## Paquete de pruebas

Incluya los artefactos siguientes en el ticket de la vaga vista previa y referencia en el correo electrónico de invitación:| Artefacto | Objetivo |
|----------|----------|
| `build/checksums.sha256` | Compruebe que el paquete corresponde a la compilación de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga útil SoraFS canonique + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montre que la soumission du manifeste + le vinculante d'alias ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadones DNS (boleto, ventana, contactos), currículum de promoción de ruta (`Sora-Route-Binding`), puntero `route_plan` (plan JSON + plantillas de encabezado), información de purga de caché e instrucciones de reversión para operaciones. |
| `artifacts/sorafs/preview-descriptor.json` | Descriptor firmado por el archivo + suma de comprobación. |
| Salida `probe` | Confirme que el hotel en línea anunciará la etiqueta de lanzamiento asistente. |