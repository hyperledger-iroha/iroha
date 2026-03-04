---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 35fe9abd10cb1454b72042b5b9dfbc35d45cc1cd91e2a4d0af4909032189df22
source_last_modified: "2025-11-10T17:38:16.543481+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-telemetry-remediation
title: Plano de remediacao de telemetria do Nexus (B2)
description: Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, documentando a matriz de lacunas de telemetria e o fluxo operacional.
---

# Visao geral

O item do roadmap **B2 - ownership de lacunas de telemetria** exige um plano publicado que vincule cada lacuna de telemetria pendente do Nexus a um sinal, um guardrail de alerta, um responsavel, um prazo e um artefato de verificacao antes do inicio das janelas de auditoria do Q1 2026. Esta pagina espelha `docs/source/nexus_telemetry_remediation_plan.md` para que release engineering, telemetry ops e os owners de SDK confirmem a cobertura antes dos ensaios routed-trace e `TRACE-TELEMETRY-BRIDGE`.

# Matriz de lacunas

| ID da lacuna | Sinal e guardrail de alerta | Responsavel / escalonamento | Prazo (UTC) | Evidencia e verificacao |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`** disparando quando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` por 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (sinal) + `@telemetry-ops` (alerta); escalonamento via on-call routed-trace do Nexus. | 2026-02-23 | Testes de alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais o registro do ensaio `TRACE-LANE-ROUTING` mostrando alerta disparado/recuperado e o scrape Torii `/metrics` arquivado em [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloqueando deploys (`docs/source/telemetry.md`). | `@nexus-core` (instrumentacao) -> `@telemetry-ops` (alerta); oficial de governanca e paginado quando o contador incrementa de forma inesperada. | 2026-02-26 | Saidas de dry-run de governanca armazenadas ao lado de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; a checklist de release inclui a captura da consulta Prometheus mais o trecho de log provando que `StateTelemetry::record_nexus_config_diff` emitiu o diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** quando falhas ou resultados ausentes persistem por >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) com escalonamento para `@sec-observability`. | 2026-02-27 | O gate de CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva payloads NDJSON e falha quando uma janela TRACE nao tem evento de sucesso; capturas de alerta anexadas ao relatorio routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` com guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentando a checklist on-call de SRE. | `@telemetry-ops` (gauge/export) com escalonamento para `@nexus-core` quando os nos reportam tamanhos de catalogo inconsistentes. | 2026-02-28 | O teste de telemetria do scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` comprova a emissao; operadores anexam diff de Prometheus + trecho de log `StateTelemetry::set_nexus_catalogs` ao pacote do ensaio TRACE. |

# Fluxo operacional

1. **Triagem semanal.** Owners reportam progresso na chamada de readiness do Nexus; blockers e artefatos de testes de alerta sao registrados em `status.md`.
2. **Ensaios de alertas.** Cada regra de alerta e entregue junto com uma entrada `dashboards/alerts/tests/*.test.yml` para que o CI execute `promtool test rules` quando o guardrail mudar.
3. **Evidencia de auditoria.** Durante os ensaios `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` o on-call captura resultados de consultas Prometheus, historico de alertas e saidas relevantes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para sinais correlacionados) e as armazena com os artefatos routed-trace.
4. **Escalonamento.** Se algum guardrail disparar fora de uma janela ensaiada, a equipe responsavel abre um ticket de incidente Nexus referenciando este plano, incluindo o snapshot da metrica e os passos de mitigacao antes de retomar as auditorias.

Com esta matriz publicada - e referenciada em `roadmap.md` e `status.md` - o item de roadmap **B2** agora atende aos criterios de aceitacao "responsabilidade, prazo, alerta, verificacao".
