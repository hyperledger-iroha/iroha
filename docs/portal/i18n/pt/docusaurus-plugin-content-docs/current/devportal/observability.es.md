---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/observability.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Observabilidad y analitica del portal

El roadmap DOCS-SORA requiere analitica, probes sinteticos y automatizacion de enlaces rotos
para cada build de preview. Esta nota documenta la plomeria que ahora viene con el portal
para que los operadores puedan conectar monitoreo sin filtrar datos de visitantes.

## Etiquetado de release

- Definir `DOCS_RELEASE_TAG=<identifier>` (hace fallback a `GIT_COMMIT` o `dev`) al
  construir el portal. El valor se inyecta en `<meta name="sora-release">`
  para que probes y dashboards distingan despliegues.
- `npm run build` emite `build/release.json` (escrito por
  `scripts/write-checksums.mjs`) describiendo el tag, timestamp y el
  `DOCS_RELEASE_SOURCE` opcional. El mismo archivo se empaqueta en los artefactos de preview y
  se referencia en el reporte del link checker.

## Analitica con preservacion de privacidad

- Configurar `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para
  habilitar el tracker liviano. Los payloads contienen `{ event, path, locale,
  release, ts }` sin metadata de referrer o IP, y se usa `navigator.sendBeacon`
  siempre que sea posible para evitar bloquear navegaciones.
- Controlar el muestreo con `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). El tracker guarda
  el ultimo path enviado y nunca emite eventos duplicados para la misma navegacion.
- La implementacion vive en `src/components/AnalyticsTracker.jsx` y se monta
  globalmente a traves de `src/theme/Root.js`.

## Probes sinteticos

- `npm run probe:portal` emite requests GET contra rutas comunes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) y verifica que el
  meta tag `sora-release` coincide con `--expect-release` (o
  `DOCS_RELEASE_TAG`). Ejemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Los fallos se reportan por path, lo que facilita gatear CD con el resultado del probe.

## Automatizacion de enlaces rotos

- `npm run check:links` escanea `build/sitemap.xml`, asegura que cada entrada mapea a un
  archivo local (chequeando fallbacks `index.html`), y escribe
  `build/link-report.json` con metadata de release, totales, fallos y el fingerprint
  SHA-256 de `checksums.sha256` (expuesto como `manifest.id`) para que cada reporte
  se pueda vincular al manifiesto del artefacto.
- El script sale con codigo distinto de cero cuando falta una pagina, asi que CI puede
  bloquear releases en rutas obsoletas o rotas. Los reportes citan las rutas candidatas
  que se intentaron, lo que ayuda a rastrear regresiones de routing hasta el arbol de docs.

## Dashboard y alertas de Grafana

- `dashboards/grafana/docs_portal.json` publica el tablero Grafana **Docs Portal Publishing**.
  Incluye los siguientes paneles:
  - *Gateway Refusals (5m)* usa `torii_sorafs_gateway_refusals_total` con scope
    `profile`/`reason` para que SRE detecte pushes de politica malos o fallas de tokens.
  - *Alias Cache Refresh Outcomes* y *Alias Proof Age p90* siguen
    `torii_sorafs_alias_cache_*` para demostrar que existen proofs frescos antes de un cut
    over de DNS.
  - *Pin Registry Manifest Counts* y la estadistica *Active Alias Count* reflejan el
    backlog del pin-registry y los aliases totales para que gobernanza pueda auditar
    cada release.
  - *Gateway TLS Expiry (hours)* destaca cuando el TLS cert del gateway de publishing
    se acerca al vencimiento (umbral de alerta en 72 h).
  - *Replication SLA Outcomes* y *Replication Backlog* vigilan la telemetria
    `torii_sorafs_replication_*` para asegurar que todas las replicas cumplen el
    nivel GA despues de publicar.
- Usa las variables de plantilla integradas (`profile`, `reason`) para enfocarte en el
  perfil de publishing `docs.sora` o investigar picos en todos los gateways.
- El routing de PagerDuty usa los paneles del dashboard como evidencia: las alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry`
  disparan cuando la serie correspondiente cruza su umbral. Enlaza el runbook de la alerta
  a esta pagina para que on-call pueda reproducir las queries exactas de Prometheus.

## Poniendolo en conjunto

1. Durante `npm run build`, define las variables de entorno de release/analitica y
   deja que el post-build emita `checksums.sha256`, `release.json` y
   `link-report.json`.
2. Ejecuta `npm run probe:portal` contra el hostname de preview con
   `--expect-release` conectado al mismo tag. Guarda el stdout para el checklist de publishing.
3. Ejecuta `npm run check:links` para fallar rapido en entradas rotas del sitemap y archiva
   el reporte JSON generado junto con los artefactos de preview. CI deja el ultimo reporte en
   `artifacts/docs_portal/link-report.json` para que gobernanza pueda descargar el bundle de evidencia
   directo desde los logs de build.
4. Enruta el endpoint de analitica hacia tu colector con preservacion de privacidad (Plausible,
   OTEL ingest self-hosted, etc.) y asegura que las tasas de muestreo queden documentadas por
   release para que los dashboards interpreten los conteos correctamente.
5. CI ya conecta estos pasos en los workflows de preview/deploy
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), asi que los dry runs locales solo necesitan
   cubrir comportamiento especifico de secretos.
