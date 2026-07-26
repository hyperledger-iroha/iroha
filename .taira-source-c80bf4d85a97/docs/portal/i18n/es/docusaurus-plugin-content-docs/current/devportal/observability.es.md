---
lang: es
direction: ltr
source: docs/portal/docs/devportal/observability.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilidad y analítica del portal.

El roadmap DOCS-SORA requiere analítica, sondas sintéticas y automatización de enlaces rotos
para cada construcción de vista previa. Esta nota documenta la plomeria que ahora viene con el portal
para que los operadores puedan conectar monitoreo sin filtrar datos de visitantes.

## Etiquetado de lanzamiento

- Definir `DOCS_RELEASE_TAG=<identifier>` (hace fallback a `GIT_COMMIT` o `dev`) al
  construir el portal. El valor se inyecta en `<meta name="sora-release">`
  para que sondas y paneles de control distingan desplegables.
- `npm run build` emite `build/release.json` (escrito por
  `scripts/write-checksums.mjs`) describiendo la etiqueta, la marca de tiempo y el
  `DOCS_RELEASE_SOURCE` opcional. El mismo archivo se empaqueta en los artefactos de vista previa y
  consulte la referencia en el informe del verificador de enlaces.

## Analítica con preservación de privacidad

- Configurar `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para
  habilitar el rastreador ligero. Las cargas útiles contienen `{ evento, ruta, configuración regional,
  lanzamiento, ts }` sin metadata de referrer o IP, y se usa `navigator.sendBeacon`
  siempre que sea posible para evitar bloquear las navegaciones.
- Controlar el muestreo con `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). El rastreador guarda
  el ultimo path enviado y nunca emite eventos duplicados para la misma navegación.
- La implementacion vive en `src/components/AnalyticsTracker.jsx` y se monta
  globalmente a través de `src/theme/Root.js`.

##Sondas sinteticas- `npm run probe:portal` emite solicitudes GET contra rutas comunes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) y verifique que el
  La metaetiqueta `sora-release` coincide con `--expect-release` (o
  `DOCS_RELEASE_TAG`). Ejemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Los fallos se reportan por camino, lo que facilita gatear CD con el resultado del probe.

## Automatización de enlaces rotos

- `npm run check:links` escanea `build/sitemap.xml`, asegura que cada entrada mapea a un
  archivo local (chequeando fallbacks `index.html`), y escribe
  `build/link-report.json` con metadatos de liberación, totales, fallos y la huella digital
  SHA-256 de `checksums.sha256` (expuesto como `manifest.id`) para que cada informe
  se pueda vincular al manifiesto del artefacto.
- El script sale con código distinto de cero cuando falta una página, así que CI puede
  bloquear liberaciones en rutas obsoletas o rotas. Los reportes citan las rutas candidatas.
  que se intentaron, lo que ayuda a rastrear regresiones de enrutamiento hasta el árbol de documentos.

## Dashboard y alertas de Grafana- `dashboards/grafana/docs_portal.json` publica el tablero Grafana **Docs Portal Publishing**.
  Incluye los siguientes paneles:
  - *Rechazos de puerta de enlace (5m)* usa `torii_sorafs_gateway_refusals_total` con alcance
    `profile`/`reason` para que SRE detecte empujones de política malos o fallas de tokens.
  - *Alias Cache Refresh Outcomes* y *Alias Proof Age p90* siguen
    `torii_sorafs_alias_cache_*` para demostrar que existen pruebas frescos antes de un corte
    sobre el DNS.
  - *Recuentos de manifiestos de registro de pines* y la estadística *Recuento de alias activos* refleja el
    backlog del pin-registry y los alias totales para que gobernanza pueda auditar
    cada lanzamiento.
  - *Gateway TLS Expiry (horas)* destaca cuando el TLS cert del gateway de publicación
    se acerca al vencimiento (umbral de alerta en 72 h).
  - *Replication SLA Outcomes* y *Replication Backlog* vigilan la telemetria
    `torii_sorafs_replication_*` para asegurar que todas las réplicas cumplen el
    nivel GA después de publicar.
- Usa las variables de plantilla integradas (`profile`, `reason`) para enfocarte en el
  perfil de publicación `docs.sora` o investigar picos en todos los gateways.
- El enrutamiento de PagerDuty usa los paneles del tablero como evidencia: las alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry`
  disparan cuando la serie correspondiente cruza su umbral. Enlaza el runbook de la alertaa esta pagina para que on-call pueda reproducir las consultas exactas de Prometheus.

## Poniendolo en conjunto

1. Durante `npm run build`, defina las variables de entorno de liberación/analítica y
   deja que el post-build emite `checksums.sha256`, `release.json` y
   `link-report.json`.
2. Ejecuta `npm run probe:portal` contra el nombre de host de vista previa con
   `--expect-release` conectado al mismo tag. Guarde la salida estándar para la lista de verificación de publicación.
3. Ejecuta `npm run check:links` para fallar rapido en entradas rotas del sitemap y archiva
   el informe JSON generado junto con los artefactos de vista previa. CI deja el último reporte en
   `artifacts/docs_portal/link-report.json` para que gobernanza pueda descargar el paquete de evidencia
   directo desde los logs de build.
4. Enruta el endpoint de analitica hacia tu colector con preservacion de privacidad (Plausible,
   OTEL ingiere autohospedado, etc.) y asegura que las tasas de muestreo queden documentadas por
   lanzamiento para que los paneles interpreten los contenidos correctamente.
5. CI ya conecta estos pasos en los flujos de trabajo de vista previa/despliegue
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), asi que los ensayos locales solo necesitan
   cubrir comportamiento específico de secretos.