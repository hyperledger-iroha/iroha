---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guía de exposición del host de vista previa

La hoja de ruta DOCS-SORA exige que toda la vista previa pública use el mismo paquete verificado por la suma de verificación que los revisores ejercen localmente. Use este runbook apos o onboarding de revisores (y o ticket de aprovacao de convites) para estarem completos para colocar el host beta en línea.

##Requisitos previos

- Onda de incorporación de revisores aprobada y registrada no tracker de vista previa.
- Última compilación del portal presentada en `docs/portal/build/` y suma de verificación verificada (`build/checksums.sha256`).
- Credencia de vista previa SoraFS (URL Torii, autoridad, chave privada, época enviada) armadas en variaciones de ambiente o en una configuración JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de mudanca DNS abierto con el nombre de host desejado (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) más contactos de guardia.

## Paso 1 - Construir y verificar el paquete

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

El script de verificación se recusa a continuar cuando el manifiesto de checksum está ausente o adulterado, manteniendo cada artefato de vista previa auditado.

## Paso 2 - Empacotar los artefatos SoraFS

Converta o site estatico em um par CAR/manifest deterministico. `ARTIFACT_DIR` padrao e `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Anexo `portal.car`, `portal.manifest.*`, el descriptor y el manifiesto de suma de comprobación del ticket de la onda de vista previa.

## Paso 3 - Publicar o alias de vista previaVuelva a ejecutar el ayudante de pin **sem** `--skip-submit` cuando estiver pronto para exportar el host. Forneca o config JSON o flags CLI explícitos:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

El comando grava `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json`, que debe acompañar el paquete de evidencia de invitaciones.

## Paso 4 - Generar el plano de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Comparta el JSON resultante con operaciones para cambiar la referencia DNS al resumen exacto del manifiesto. Para reutilizar un descriptor anterior como origen de reversión, agregue `--previous-dns-plan path/to/previous.json`.

## Paso 5 - Testar o host implantado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonda confirma la etiqueta de liberación servida, los encabezados CSP y los metadados de assinatura. Repita el comando a partir de dos regiones (o anexo a dicha de curl) para que los auditores vean que el borde cache esta quente.

## Paquete de evidencia

Incluye los siguientes artefatos sin ticket da onda de vista previa y referencias sin correo electrónico de invitación:| Artefacto | propuesta |
|----------|-----------|
| `build/checksums.sha256` | Prueba que el paquete corresponde a la compilación de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga útil canonico SoraFS + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Mostra que o envio do manifesto + o alias vinculante foram concluidos. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (ticket, janela, contactos), resumen de promoción de rotación (`Sora-Route-Binding`), puente `route_plan` (plano JSON + plantillas de encabezado), información de purga de caché e instrucciones de reversión para operaciones. |
| `artifacts/sorafs/preview-descriptor.json` | Descriptor assinado que liga o archive + checksum. |
| Dicho esto `probe` | Confirma que el anfitrión ao vivo anuncia el tag de lanzamiento esperado. |