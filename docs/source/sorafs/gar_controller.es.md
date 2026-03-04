---
lang: es
direction: ltr
source: docs/source/sorafs/gar_controller.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 67e8e60ef4944c5198ea0c2de9009cc39e78f9c1306e0e94c8daf13ad90fb36a
source_last_modified: "2025-11-21T15:16:43.702084+00:00"
translation_last_reviewed: "2026-01-30"
---

# Bundle del controlador GAR (SNNet-15G)

El controlador GAR agrupa eventos de dispatch de politica, receipts de
enforcement por PoP, un snapshot de reconciliacion/metricas y resuenes en
Markdown/JSON para que los paquetes de compliance se envien sin checklists
escritos a mano. Usa el config de ejemplo bajo
`configs/soranet/gateway_m0/gar_controller.sample.json` como punto de partida.

## Ejecutar el generador

- `cargo xtask soranet-gar-controller --config <path> [--output-dir <dir>] [--markdown-out <path>] [--now <unix>]`
- Defaults:
  - Config: `configs/soranet/gateway_m0/gar_controller.sample.json`
  - Output root: `artifacts/soranet/gateway/gar_controller`
  - Incluye un reporte Markdown por defecto (`gar_controller_report.md`); override con `--markdown-out`.
- Schema de config:
  - `base_subject`: prefijo de subject NATS usado cuando un pop omite `nats_subject`.
  - `operator`: Account ID registrado en cada `GarEnforcementReceiptV1`.
  - `default_expires_at_unix`: expiracion fallback opcional usada cuando una politica omite `expires_at_unix`.
  - `pops`: lista de entradas `{label, nats_subject?, audit_uri?}`. Cada pop recibe cada evento de politica.
  - `policies`: lista de `{gar_name, canonical_host, policy (GarPolicyPayloadV1), policy_version?, cache_version?, evidence_uris?, labels?, expires_at_unix?}`.

## Outputs

- `gar_events.jsonl` — spool NDJSON de eventos NATS (`{subject, payload}`) listo para `nats pub --stdin`.
- `gar_receipts/*.json` + `.to` — `GarEnforcementReceiptV1` por PoP en JSON y Norito.
- `gar_controller_summary.json` — resumen estructurado de subjects, politicas, digests y rutas de output.
- `gar_controller_report.md` — reporte orientado a operadores para paquetes de transparencia y evidencia SLA (incluye reconciliacion + tabla de warnings).
- `gar_reconciliation_report.json` — cobertura por GAR/pop con lista de warnings (politicas expiradas, pops faltantes).
- `gar_metrics.prom` — snapshot estilo Prometheus (dispatch, expiracion, contadores de warnings) para dashboards/runbooks.
- `gar_audit_log.jsonl` — feed NDJSON de auditoria por pop (accion/razon/rutas de receipt + evidencia).

## Ingesta y transparencia

- Archivar un intake de moderacion/legal GAR usando la plantilla en
  `docs/examples/soranet_gar_intake_form.md`; mantener `labels`/`evidence_uris`
  en sync para que receipts y logs de auditoria citen el request entrante.
- El reporte de reconciliacion alerta sobre GARs expirados y cobertura PoP
  faltante; adjuntar el reporte + snapshot de metricas a paquetes de governance
  para que los dashboards lean la misma evidencia sin re-ejecutar el generador.
- El audit log espeja el spool NATS y el set de receipts; alimentarlo al ops
  console o NATS fan-out para correlacionar ACKs por pop con receipts.

## Semantica de dispatch

- La accion de enforcement primaria se deriva de la politica CDN:
  - `legal_hold` → `legal_hold`
  - `allow_regions`/`deny_regions` → `geo_fence`
  - `rate_ceiling_rps` → `rate_limit_override`
  - `ttl_override_secs` → `ttl_override`
  - `purge_tags` → `purge_static_zone`
  - `moderation_slugs` → `moderation`
  - de lo contrario `audit_notice`
- Los IDs de receipts y digests de politica son deterministas (BLAKE3 sobre bytes de politica + pop + timestamp) para mantener exports de governance reproducibles.
- Las URIs de evidencia combinan entradas a nivel de politica/policy-level con `audit_uri` opcionales de cada pop para que logs de drills y dashboards coincidan con receipts.
