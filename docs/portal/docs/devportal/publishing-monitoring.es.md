---
lang: es
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6efe6943d41c95ebaf768360ead55a18996db371587c20571ece906c5ede56f1
source_last_modified: "2025-11-20T04:38:45.090032+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: publishing-monitoring
title: Publicacion y monitoreo SoraFS
sidebar_label: Publicacion y monitoreo
description: Captura el flujo de monitoreo end-to-end para releases del portal SoraFS para que DOCS-3c tenga probes deterministas, telemetria y bundles de evidencia.
---

Roadmap item **DOCS-3c** requiere mas que un checklist de empaquetado: despues de cada
publicacion de SoraFS debemos demostrar continuamente que el portal de desarrolladores, el proxy Try it
y los bindings del gateway se mantienen saludables. Esta pagina documenta la superficie de monitoreo
que acompana la [guia de despliegue](./deploy-guide.md) para que CI y los ingenieros on call puedan
aplicar las mismas comprobaciones que Ops usa para hacer cumplir el SLO.

## Resumen del pipeline

1. **Build y firma** - sigue la [guia de despliegue](./deploy-guide.md) para ejecutar
   `npm run build`, `scripts/preview_wave_preflight.sh`, y los pasos de envio a Sigstore + manifest.
   El script de preflight emite `preflight-summary.json` para que cada preview lleve metadatos
   de build/link/probe.
2. **Pin y verificacion** - `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   y el plan de corte DNS proveen artefactos deterministas para gobernanza.
3. **Archivar evidencia** - guarda el resumen CAR, bundle Sigstore, proof de alias,
   salida de probe y snapshots del dashboard `docs_portal.json` bajo
   `artifacts/sorafs/<tag>/`.

## Canales de monitoreo

### 1. Monitores de publicacion (`scripts/monitor-publishing.mjs`)

El nuevo comando `npm run monitor:publishing` envuelve el probe del portal, el probe del proxy Try it
y el verificador de bindings en un unico chequeo apto para CI. Provee un config JSON
(guardado en secretos de CI o `configs/docs_monitor.json`) y ejecuta:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Agrega `--prom-out ../../artifacts/docs_monitor/monitor.prom` (y opcionalmente
`--prom-job docs-preview`) para emitir metricas en formato de texto Prometheus
aptas para Pushgateway o scrapes directos en staging/produccion. Las metricas
replican el resumen JSON para que los dashboards de SLO y las reglas de alerta
puedan seguir la salud del portal, Try it, bindings y DNS sin parsear el bundle de evidencia.

Ejemplo de config con knobs requeridos y multiples bindings:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/i105.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

El monitor escribe un resumen JSON (friendly para S3/SoraFS) y sale con codigo distinto de cero
cuando algun probe falla, lo que lo hace apto para Cron jobs, steps de Buildkite o webhooks de
Alertmanager. Pasar `--evidence-dir` persiste `summary.json`, `portal.json`, `tryit.json` y
`binding.json` junto con un manifest `checksums.sha256` para que los reviewers de gobernanza
puedan hacer diff de los resultados sin tener que re-ejecutar los probes.

> **Guardrail TLS:** `monitorPortal` rechaza URLs base `http://` salvo que configures
> `allowInsecureHttp: true` en el config. Mantener probes de produccion/staging en
> HTTPS; la opcion existe solo para previews locales.

Each binding entry runs `cargo xtask soradns-verify-binding` against the captured
`portal.gateway.binding.json` bundle (and optional `manifestJson`) so alias,
proof status, and content CID stay aligned with the published evidence. The
optional `hostname` guard confirms the alias-derived canonical host matches the
gateway host you intend to promote, preventing DNS cutovers that drift from the
recorded binding.


El bloque opcional `dns` conecta el rollout SoraDNS de DOCS-7 en el mismo monitor.
Cada entrada resuelve un par hostname/record-type (por ejemplo el CNAME
`docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name`) y confirma
que las respuestas coinciden con `expectedRecords` o `expectedIncludes`. La segunda
entrada del snippet de arriba fija el hostname canonico hasheado producido por
`cargo xtask soradns-hosts --name docs-preview.sora.link`; el monitor ahora prueba
que tanto el alias friendly como el hash canonico (`igjssx53...gw.sora.id`)
resuelven al pretty host fijado. Esto hace automatica la evidencia de promocion DNS:
el monitor fallara si cualquiera de los hosts deriva, incluso cuando los bindings HTTP
siguen adjuntando el manifest correcto.

### 2. Verificacion del manifiesto de versiones OpenAPI

El requisito de DOCS-2b de "manifest OpenAPI firmado" ahora tiene una guardia automatizada:
`ci/check_openapi_spec.sh` llama a `npm run check:openapi-versions`, que invoca
`scripts/verify-openapi-versions.mjs` para cruzar
`docs/portal/static/openapi/versions.json` con las specs y manifests reales de Torii.
La guardia verifica que:

- Cada version listada en `versions.json` tenga un directorio matching bajo
  `static/openapi/versions/`.
- Los campos `bytes` y `sha256` coincidan con el archivo on-disk de la spec.
- El alias `latest` refleje el entry `current` (metadata de digest/size/signature)
  para que el download por defecto no derive.
- Entradas firmadas referencien un manifest cuyo `artifact.path` apunte de vuelta
  a la misma spec y cuyos valores de firma/clave publica en hex coincidan con el manifest.

Ejecuta la guardia localmente cada vez que espejes una nueva spec:

```bash
cd docs/portal
npm run check:openapi-versions
```

Los mensajes de fallo incluyen la pista de archivo viejo (`npm run sync-openapi -- --latest`)
para que los contribuidores del portal sepan como refrescar los snapshots.
Mantener la guardia en CI evita releases del portal donde el manifest firmado y el digest
publicado se desincronicen.

### 2. Dashboards y alertas

- **`dashboards/grafana/docs_portal.json`** - tablero principal para DOCS-3c. Los paneles
  siguen `torii_sorafs_gateway_refusals_total`, fallas de SLA de replicacion, errores del
  proxy Try it y latencia de probes (overlay `docs.preview.integrity`). Exporta el tablero
  despues de cada release y adjuntalo al ticket de operaciones.
- **Alertas del proxy Try it** - la regla de Alertmanager `TryItProxyErrors` se dispara ante
  caidas sostenidas de `probe_success{job="tryit-proxy"}` o picos de
  `tryit_proxy_requests_total{status="error"}`.
- **Gateway SLO** - `DocsPortal/GatewayRefusals` asegura que los bindings de alias sigan
  anunciando el digest del manifest fijado; las escalaciones enlazan a la transcripcion
  del CLI `cargo xtask soradns-verify-binding` capturada durante la publicacion.

### 3. Rastro de evidencia

Cada corrida de monitoreo deberia anexar:

- Bundle de evidencia de `monitor-publishing` (`summary.json`, archivos por seccion y
  `checksums.sha256`).
- Screenshots de Grafana para el board `docs_portal` durante la ventana de release.
- Transcripts de cambio/rollback del proxy Try it (logs de `npm run manage:tryit-proxy`).
- Salida de verificacion de alias de `cargo xtask soradns-verify-binding`.

Guarda estos bajo `artifacts/sorafs/<tag>/monitoring/` y enlazalos en el issue de release
para que el rastro de auditoria sobreviva cuando expiren los logs de CI.

## Checklist operativo

1. Ejecuta la guia de despliegue hasta el Paso 7.
2. Ejecuta `npm run monitor:publishing` con configuracion de produccion; archiva
   la salida JSON.
3. Captura paneles de Grafana (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) y adjuntalos al ticket de release.
4. Programa monitores recurrentes (recomendado: cada 15 minutos) apuntando a
   URLs de produccion con el mismo config para cumplir el gate de SLO de DOCS-3c.
5. Durante incidentes, re-ejecuta el comando de monitor con `--json-out` para
   registrar evidencia antes/despues y adjuntala al postmortem.

Seguir este loop cierra DOCS-3c: el flujo de build del portal, el pipeline de publicacion
y el stack de monitoreo ahora viven en un solo playbook con comandos reproducibles,
configs de ejemplo y hooks de telemetria.
