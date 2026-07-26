---
lang: pt
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

# Template de intake GAR do SoraNet

Use este formulario de intake ao solicitar uma acao GAR (purge, ttl override, rate
ceiling, moderation directive, geofence, ou legal hold). O formulario enviado deve ser
fixado junto aos outputs do `gar_controller` para que logs de auditoria e recibos
citem as mesmas URIs de evidencia.

| Campo | Valor | Notas |
|-------|-------|-------|
| ID da solicitacao |  | ID do ticket guardian/ops. |
| Solicitado por |  | Conta + contato. |
| Data/hora (UTC) |  | Quando a acao deve iniciar. |
| Nome GAR |  | por ex., `docs.sora`. |
| Host canonico |  | por ex., `docs.gw.sora.net`. |
| Acao |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| Override de TTL (segundos) |  | Obrigatorio apenas para `ttl_override`. |
| Teto de taxa (RPS) |  | Obrigatorio apenas para `rate_limit_override`. |
| Regioes permitidas |  | Lista de regioes ISO ao solicitar `geo_fence`. |
| Regioes negadas |  | Lista de regioes ISO ao solicitar `geo_fence`. |
| Slugs de moderacao |  | Deve corresponder as diretrizes de moderacao GAR. |
| Tags de purge |  | Tags que devem ser purgadas antes de servir. |
| Labels |  | Labels de maquina (incident id, drill name, pop scope). |
| URIs de evidencia |  | Logs/dashboards/especificacoes que sustentam a solicitacao. |
| URI de auditoria |  | URI de auditoria por pop se diferente dos defaults. |
| Expiracao solicitada |  | Unix timestamp ou RFC3339; deixe em branco para o default. |
| Motivo |  | Explicacao para o usuario; aparece em recibos e dashboards. |
| Aprovador |  | Aprovador guardian/comite da solicitacao. |

### Etapas de envio

1. Preencha a tabela e anexe ao ticket de governance.
2. Atualize a config do GAR controller (`policies`/`pops`) com `labels`/`evidence_uris`/`expires_at_unix` correspondentes.
3. Execute `cargo xtask soranet-gar-controller ...` para emitir eventos/recibos.
4. Coloque `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom` e `gar_audit_log.jsonl` no mesmo ticket. O aprovador confirma que a contagem de recibos corresponde a lista PoP antes do envio.
