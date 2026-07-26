---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Руководство по экспозиции vista previa-хоста

Tarjeta de crédito DOCS-SORA, vista previa de todos los archivos disponibles, suma de comprobación disponible, revisión de códigos проверяют локально. Utilice este runbook para actualizar los controladores integrados (y los programas de actualización), para saber cuál es el host de vista previa de la versión beta en esta configuración.

## Предварительные требования

- Todas las actualizaciones y actualizaciones en el rastreador de vista previa.
- El portal de imágenes actualizado está conectado a `docs/portal/build/` y la suma de comprobación está disponible (`build/checksums.sha256`).
- Vista previa de la vista previa de SoraFS (URL Torii, autoridad, clave privada, época destacada) integrada en configuración permanente o JSON configuración, nombre [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Registrar el número de DNS con el nombre de host del servidor (`docs-preview.sora.link`, `docs.iroha.tech` y т.д.) y contactos de guardia.

## Шаг 1 - Paquete Собрать и проверить

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Los scripts que muestran la suma de comprobación o la configuración de la pantalla muestran la suma de comprobación o la configuración de la vista previa de los artefactos.

## Capítulo 2 - Упаковать артефакты SoraFS

Asegúrese de que la información estática esté determinada por CAR/manifest. `ARTIFACT_DIR` по умолчанию `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Escriba `portal.car`, `portal.manifest.*`, descriptor y suma de comprobación del manifiesto de onda de vista previa del ticket.

## Шаг 3 - Alias ​​de vista previa de ОпубликоватьIntroduzca el ayudante de pin **без** `--skip-submit`, que permite desbloquear el servidor. Mantenga la configuración JSON o las banderas CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Los comandos `portal.pin.report.json`, `portal.manifest.submit.summary.json` y `portal.submit.response.json`, que son dos cosas que se encuentran en el paquete de pruebas.

## Paso 4 - Сгенерировать план DNS cutover

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Compatible con JSON en Ops, esta configuración DNS se conecta al manifiesto de resumen actual. Una vez que se utiliza el descriptor anterior a la reversión histórica, se activa `--previous-dns-plan path/to/previous.json`.

## Paso 5 - Проверить развернутый хост

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonda puede contener etiquetas de liberación no deseadas, CSP заголовки y метаданные подписи. Este comando está en dos regiones (o utiliza curl), qué auditores monitorean y qué programa de caché de borde.

## Paquete de evidencia

Mire los siguientes artefactos en la onda de vista previa del ticket y úselo en las páginas de inicio:| Artefacto | Назначение |
|----------|------------|
| `build/checksums.sha256` | Por favor, qué paquete incluye compilación de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канонический SoraFS carga útil + manifiesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Показывает, что отправка manifest y привязка alias успешны. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS метаданные (тикет, окно, контакты), сводка продвижения маршрута (`Sora-Route-Binding`), указатель `route_plan` (plan JSON + encabezado de archivo), Instrucciones de purga de caché e instrucciones de reversión para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Подписанный descriptor, связывающий archivo + suma de comprobación. |
| Tipo `probe` | Подтверждает, что live host publica una etiqueta de lanzamiento. |