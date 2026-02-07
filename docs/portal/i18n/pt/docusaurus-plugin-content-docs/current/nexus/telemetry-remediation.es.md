---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-telemetria-remediação
título: Plano de remediação de telemetria de Nexus (B2)
description: Espejo de `docs/source/nexus_telemetry_remediation_plan.md`, documenta a matriz de brechas de telemetria e o fluxo operativo.
---

# Resumo geral

O item do roteiro **B2 - propriedade de brechas de telemetria** requer um plano publicado que vincule cada brecha de telemetria pendente de Nexus com um senal, um guardrail de alerta, um responsável, uma data limite e um artefato de verificação antes de iniciar as vendas de auditorias do primeiro trimestre de 2026. Esta página atualizada `docs/source/nexus_telemetry_remediation_plan.md` para que a engenharia de liberação, operações de telemetria e os responsáveis pelo SDK confirmem a cobertura antes dos ensaios routed-trace e `TRACE-TELEMETRY-BRIDGE`.

# Matriz de brechas

| ID da brecha | Sinal e guarda-corpo de alerta | Responsável / escalamento | Data (UTC) | Evidência e verificação |
|----|-------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`** que dispara quando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` durante 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (senal) + `@telemetry-ops` (alerta); escalar via on-call de routed-trace de Nexus. | 23/02/2026 | Testes de alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml`, mas a captura do ensaio `TRACE-LANE-ROUTING` mostrando alerta disparado/recuperado e o rascunho de Torii `/metrics` arquivado em [Nexus notas de transição](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` que bloqueia despliegues (`docs/source/telemetry.md`). | `@nexus-core` (instrumentação) -> `@telemetry-ops` (alerta); se pagina al oficial de guardia de gobernanza quando o contador incrementa de forma inesperada. | 26/02/2026 | Saídas de simulação de governo almacenadas junto com `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; a lista de verificação de liberação inclui a captura da consulta de Prometheus, mas o extrato de logs que verifica que `StateTelemetry::record_nexus_config_diff` emite o diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (métrica `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** quando falhas ou resultados ausentes persistem por >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) com escalação para `@sec-observability`. | 27/02/2026 | A computação de CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva payloads NDJSON e falha quando uma janela TRACE carece de um evento de saída; capturas de alertas adjuntas ao relatório routed-trace. |
| `GAP-TELEM-004` | Medidor `nexus_lane_configured_total` com guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` que alimenta o checklist de plantão do SRE. | `@telemetry-ops` (medidor/exportação) com escalação para `@nexus-core` quando os nós reportam tamanhos de catálogo inconsistentes. | 28/02/2026 | O teste de telemetria do agendador `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` mostra a emissão; os operadores complementam Prometheus diff + extrato de log de `StateTelemetry::set_nexus_catalogs` no pacote do ensaio TRACE. |

# Flujo operativo

1. **Triagem semanal.** Os proprietários relatam o progresso na chamada de prontidão de Nexus; bloqueadores e artefatos de teste de alerta são registrados em `status.md`.
2. **Ensaios de alertas.** Cada regulamento de alerta é entregue junto com uma entrada em `dashboards/alerts/tests/*.test.yml` para que o CI execute `promtool test rules` quando o guarda-corpo muda.
3. **Evidência de auditoria.** Durante os ensaios `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE`, a captura de plantão dos resultados de consultas de Prometheus, o histórico de alertas e as saídas relevantes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para senais correlacionados) e os almacenas com os artefatos routed-trace.
4. **Escalamento.** Se algum guarda-corpo for disparado fora de uma janela ensaiada, a equipe responsável abre um ticket de incidente Nexus que referencia este plano, incluindo o instantâneo da métrica e as etapas de mitigação antes de reanudar as auditorias.

Com esta matriz publicada - e referenciada desde `roadmap.md` e `status.md` - o item de roadmap **B2** agora cumpre os critérios de aceitação "responsabilidade, data limite, alerta, verificação".