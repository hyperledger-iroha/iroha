---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-telemetria-remediação
título: Plano de remediação de telemetria do Nexus (B2)
descrição: Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, documentando a matriz de lacunas de telemetria e o fluxo operacional.
---

# Visão geral

O item do roadmap **B2 - propriedade de lacunas de telemetria** exige um plano publicado que vincule cada lacuna de telemetria pendente do Nexus a um sinal, um guardrail de alerta, um responsavel, um prazo e um artefatos de verificação antes do início das janelas de auditoria do primeiro trimestre de 2026. Esta página espelha `docs/source/nexus_telemetry_remediation_plan.md` para que releaseengine, telemetry ops e os proprietários de SDK confirmem a cobertura antes dos ensaios routed-trace e `TRACE-TELEMETRY-BRIDGE`.

# Matriz de lacunas

| ID da lacuna | Sinal e guarda-corpo de alerta | Responsavel/escalonamento | Prazo (UTC) | Evidência e verificação |
|----|-------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`** disparando quando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` por 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (sinal) + `@telemetry-ops` (alerta); escalonamento via rastreamento roteado de plantão do Nexus. | 23/02/2026 | Testes de alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais o registro do ensaio `TRACE-LANE-ROUTING` mostrando alerta disparado/recuperado e o scrape Torii `/metrics` arquivado em [Nexus transaction note](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloqueando implantações (`docs/source/telemetry.md`). | `@nexus-core` (instrumentação) -> `@telemetry-ops` (alerta); oficial de governança e paginado quando o contador incrementa de forma inesperada. | 26/02/2026 | Saidas de simulação de governança armazenadas ao lado de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; a checklist de release inclui a captura da consulta Prometheus mais o trecho de log provando que `StateTelemetry::record_nexus_config_diff` emitiu o diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (métrica `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** quando falhas ou resultados ausentes persistem por >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) com escalação para `@sec-observability`. | 27/02/2026 | O portão de CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva payloads NDJSON e falha quando uma janela TRACE não tem evento de sucesso; capturas de alerta anexadas ao relatorio routed-trace. |
| `GAP-TELEM-004` | Medidor `nexus_lane_configured_total` com guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentando um checklist de plantão do SRE. | `@telemetry-ops` (gauge/export) com escalonamento para `@nexus-core` quando os tamanhos do catálogo são inconsistentes. | 28/02/2026 | O teste de telemetria do agendador `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` comprova a emissão; operadores anexam diff de Prometheus + trecho de log `StateTelemetry::set_nexus_catalogs` ao pacote do ensaio TRACE. |

# Fluxo operacional

1. **Triagem semanal.** Proprietários reportam progresso na chamada de prontidão do Nexus; bloqueadores e artistas de testes de alerta são registrados em `status.md`.
2. **Ensaios de alertas.** Cada regra de alerta e entregue junto com uma entrada `dashboards/alerts/tests/*.test.yml` para que o CI execute `promtool test rules` quando o guardrail mudar.
3. **Evidência de auditorias.** Durante os ensaios `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` ou captura on-call de resultados de consultas Prometheus, histórico de alertas e dados relevantes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para sinais correlacionados) e os materiais com os objetos rastreamento roteado.
4. **Escalonamento.** Se algum guardrail disparar fora de uma janela ensaiada, a equipe responsável abre um ticket de incidente Nexus referenciando este plano, incluindo o snapshot da métrica e os passos de mitigação antes de retomar as auditorias.

Com esta matriz publicada - e referenciada em `roadmap.md` e `status.md` - o item de roadmap **B2** agora atende aos critérios de aceitação "responsabilidade, prazo, alerta, verificação".