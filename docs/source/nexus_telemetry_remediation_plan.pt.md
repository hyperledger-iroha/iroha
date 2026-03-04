---
lang: pt
direction: ltr
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

% Plano de remediacao de telemetria Nexus (Fase B2)

# Visao geral

O item de roadmap **B2 - telemetry gap ownership** exige um plano publicado que vincule cada gap
pendente de telemetria Nexus a um sinal, guardrail de alerta, owner, prazo e artefato de
verificacao antes do inicio das janelas de auditoria do Q1 2026. Este documento centraliza essa
matriz para que release engineering, telemetry ops e owners de SDK confirmem cobertura antes dos
ensaios de routed-trace e `TRACE-TELEMETRY-BRIDGE`.

# Matriz de gaps

| Gap ID | Sinal e guardrail de alerta | Owner / Escalacao | Prazo (UTC) | Evidencia e verificacao |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`** que dispara quando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` por 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (sinal) + `@telemetry-ops` (alerta) - escalar via on-call Nexus routed-trace. | 2026-02-23 | Testes de alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml` e a captura do ensaio `TRACE-LANE-ROUTING` mostrando o alerta disparado/recuperado, mais o scrape Torii `/metrics` arquivado em `docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` que bloqueia deploys (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (alerta) - duty officer de governanca e avisado quando o contador incrementa de forma inesperada. | 2026-02-26 | Saidas de dry-run de governanca armazenadas junto de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; a checklist de release inclui a captura da query Prometheus mais o trecho de logs provando que `StateTelemetry::record_nexus_config_diff` emitiu o diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** quando falhas ou resultados ausentes persistem por >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) com escalacao para `@sec-observability`. | 2026-02-27 | O gate de CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva payloads NDJSON e falha quando uma janela TRACE nao tem evento de sucesso; capturas de alerta anexadas ao relatorio de routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` monitorado com guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (documentado em `docs/source/telemetry.md`) alimentando a checklist de plantao SRE. | `@telemetry-ops` (gauge/export) com escalacao para `@nexus-core` quando nodes reportam tamanhos de catalogo inconsistentes. | 2026-02-28 | O teste de telemetria do scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prova a emissao; operadores anexaram diff Prometheus mais o trecho de log `StateTelemetry::set_nexus_catalogs` ao pacote TRACE. |

# Orcamento de exportacao e limites OTLP

- **Decisao (2026-02-11):** limitar exportadores OTLP a **5 MiB/min por node** ou
  **25,000 spans/min**, o que for menor, com tamanho de lote de 256 spans e timeout de exportacao
  de 10 segundos. Exportes acima de 80% do limite disparam o alerta `NexusOtelExporterSaturated`
  em `dashboards/alerts/nexus_telemetry_rules.yml` e emitem o evento
  `telemetry_export_budget_saturation` para logs de auditoria.
- **Enforcement:** regras Prometheus leem os contadores `iroha.telemetry.export.bytes_total`
  e `iroha.telemetry.export.spans_total`; o perfil do collector OTLP entrega os mesmos limites
  como defaults, e configs em node nao devem aumentar esses limites sem isencao de governanca.
- **Evidencia:** vetores de teste de alerta e limites aprovados sao arquivados em
  `docs/source/nexus_transition_notes.md` junto dos artefatos de auditoria routed-trace. A
  aceitacao B2 agora trata o orcamento de exportacao como fechado.

# Fluxo operacional

1. **Triage semanal.** Owners reportam progresso na chamada de readiness de Nexus; bloqueios e
   artefatos de teste de alerta sao registrados em `status.md`.
2. **Dry-runs de alertas.** Cada regra de alerta segue com uma entrada em
   `dashboards/alerts/tests/*.test.yml` para que CI execute `promtool test rules` sempre que o
   guardrail mudar.
3. **Evidencia de auditoria.** Durante os ensaios `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE`,
   a on-call captura resultados de query Prometheus, historico de alertas e saidas de scripts
   relevantes (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` para sinais correlatos) e os guarda com os
   artefatos de routed-trace.
4. **Escalacao.** Se qualquer guardrail disparar fora de uma janela ensaiada, o time responsavel
   abre um ticket de incidente Nexus referenciando este plano, incluindo o snapshot de metricas e
   os passos de mitigacao antes de retomar auditorias.

Com esta matriz publicada e referenciada em `roadmap.md` e `status.md`, o item de roadmap
**B2** agora cumpre os criterios de aceitacao de "responsabilidade, prazo, alerta, verificacao".
