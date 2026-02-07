---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-telemetria-remediação
título: Plano de remediação de telemetria Nexus (B2)
descrição: Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, documenta a matriz dos cartões de telemetria e o fluxo operacional.
---

# Vista do conjunto

O elemento de roteiro **B2 - propriedade de cartões de telemetria** exige um plano público dependente de cada cartão de telemetria remanescente de Nexus como um sinal, um guarda-corpo de alerta, um proprietário, um limite de data e um artefato de verificação antes da estreia das janelas de auditoria do primeiro trimestre de 2026. Esta página reflete `docs/source/nexus_telemetry_remediation_plan.md` para engenharia de liberação, operações de telemetria e proprietários SDK podem confirmar a cobertura antes das repetições roteadas-trace e `TRACE-TELEMETRY-BRIDGE`.

# Matriz de ecarts

| ID do carrinho | Sinal e guarda-corpo de alerta | Proprietário / escalada | Echeance (UTC) | Testes e verificação |
|----|-------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`** desativado quando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` permanece por 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (sinal) + `@telemetry-ops` (alerta); escalada via l'on-call routed-trace Nexus. | 23/02/2026 | Testes de alerta sob `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais a captura da repetição `TRACE-LANE-ROUTING` montam o alerta desativado/retablie e raspa Torii `/metrics` arquivo em [Nexus transição notas](./nexus-transition-notes). |
| `GAP-TELEM-002` | O computador `nexus_config_diff_total{knob,profile}` com guarda-corpo `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloqueia as implantações (`docs/source/telemetry.md`). | `@nexus-core` (instrumentação) -> `@telemetry-ops` (alerta); O oficial de governança é uma página quando o contador aumenta a desatenção. | 26/02/2026 | Sorties de dry-run de estoques de governo na costa de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; a lista de verificação de liberação inclui a captura da receita Prometheus, além do extrato de logs provando que `StateTelemetry::record_nexus_config_diff` emite a diferença. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (métrica `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** quando as verificações ou resultados são persistentes >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) com escalação versão `@sec-observability`. | 27/02/2026 | O portão CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva as cargas úteis NDJSON e ecoa quando uma janela TRACE não contém nenhum evento de sucesso; captura alertas conjuntos no rapport routed-trace. |
| `GAP-TELEM-004` | Medidor `nexus_lane_configured_total` com guarda-corpo `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimenta a lista de verificação SRE de plantão. | `@telemetry-ops` (manômetro/exportação) com escalade vers `@nexus-core` quando os noeuds sinalizam as caudas do catálogo incoerentes. | 28/02/2026 | O teste de telemetria do agendador `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` comprovou a emissão; Os operadores conectam um diff Prometheus + extração de log `StateTelemetry::set_nexus_catalogs` no pacote de repetição TRACE. |

# Operação de fluxo

1. **Triage hebdomadaire.** Os proprietários informam o avanço da chamada de prontidão Nexus; os bloqueios e artefatos de testes de alerta são enviados para `status.md`.
2. **Testes de alerta.** Todas as regras de alerta estão livres com uma entrada `dashboards/alerts/tests/*.test.yml` depois que o CI executa `promtool test rules` quando o guardrail evolui.
3. **Preuves d'audit.** Após as repetições `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE`, o on-call captura os resultados das solicitações Prometheus, o histórico de alertas e as saídas de scripts pertinentes (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para sinais correlatos) e estoque com artefatos routed-trace.
4. **Escalada.** Se um guarda-corpo for fechado fora de uma janela repetida, a equipe proprietária emitirá um ticket de incidente Nexus como referência a este plano, incluindo o instantâneo da métrica e as etapas de mitigação antes de repender as auditorias.

Com esta matriz publicada - e referência de `roadmap.md` e `status.md` - o item de roteiro **B2** satisfaz a manutenção dos critérios de aceitação "responsabilidade, verificação, alerta, verificação".