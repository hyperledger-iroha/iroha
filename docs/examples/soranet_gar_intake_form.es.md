---
lang: es
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de intake GAR de SoraNet

Usa este formulario de intake cuando solicites una accion GAR (purge, ttl override, rate
ceiling, moderation directive, geofence o legal hold). El formulario enviado debe quedar
fijado junto con los outputs de `gar_controller` para que los logs de auditoria y los
recibos citen los mismos URIs de evidencia.

| Campo | Valor | Notas |
|-------|-------|-------|
| ID de solicitud |  | ID de ticket guardian/ops. |
| Solicitado por |  | Cuenta + contacto. |
| Fecha/hora (UTC) |  | Cuando debe iniciar la accion. |
| Nombre GAR |  | p. ej., `docs.sora`. |
| Host canonico |  | p. ej., `docs.gw.sora.net`. |
| Accion |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| Override de TTL (segundos) |  | Requerido solo para `ttl_override`. |
| Techo de tasa (RPS) |  | Requerido solo para `rate_limit_override`. |
| Regiones permitidas |  | Lista de regiones ISO cuando se solicita `geo_fence`. |
| Regiones denegadas |  | Lista de regiones ISO cuando se solicita `geo_fence`. |
| Slugs de moderacion |  | Coincidir con las directivas de moderacion GAR. |
| Tags de purge |  | Tags que deben purgarse antes de servir. |
| Labels |  | Etiquetas de maquina (incident id, drill name, pop scope). |
| URIs de evidencia |  | Logs/dashboards/especificaciones que respaldan la solicitud. |
| URI de auditoria |  | URI de auditoria por pop si es diferente de los defaults. |
| Expiracion solicitada |  | Unix timestamp o RFC3339; dejar en blanco para el default. |
| Razon |  | Explicacion de cara al usuario; aparece en recibos y dashboards. |
| Aprobador |  | Aprobador guardian/comite para la solicitud. |

### Pasos de envio

1. Completa la tabla y adjuntala al ticket de governance.
2. Actualiza la config del GAR controller (`policies`/`pops`) con `labels`/`evidence_uris`/`expires_at_unix` que coincidan.
3. Ejecuta `cargo xtask soranet-gar-controller ...` para emitir eventos/recibos.
4. Coloca `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom` y `gar_audit_log.jsonl` en el mismo ticket. El aprobador confirma que el conteo de recibos coincide con la lista PoP antes del envio.
