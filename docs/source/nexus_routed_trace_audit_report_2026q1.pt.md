<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: pt
direction: ltr
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

# Relatorio de auditoria Routed-Trace 2026 Q1 (B1)

O item de roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** exige uma
revisao trimestral do programa routed-trace do Nexus. Este relatorio documenta
a janela de auditoria Q1 2026 (janeiro-marco) para que o conselho de governanca
aprove a postura de telemetria antes dos ensaios de lancamento Q2.

## Escopo e cronograma

| Trace ID | Janela (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verificar histogramas de admissao de lanes, gossip de filas e fluxo de alertas antes de habilitar multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Validar replay OTLP, paridade do bot de diff e ingestao de telemetria do SDK antes dos marcos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas `iroha_config` aprovadas por governanca e prontidao de rollback antes do corte RC1. |

Cada ensaio rodou em topologia similar a producao com a instrumentacao
routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores
Prometheus), regras de Alertmanager carregadas e evidencias exportadas para
`docs/examples/`.

## Metodologia

1. **Coleta de telemetria.** Todos os nos emitiram o evento estruturado
   `nexus.audit.outcome` e as metricas associadas (`nexus_audit_outcome_total*`). O helper
   `scripts/telemetry/check_nexus_audit_outcome.py` acompanhou o log JSON, validou o status do
   evento e arquivou o payload em `docs/examples/nexus_audit_outcomes/`
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **Validacao de alertas.** `dashboards/alerts/nexus_audit_rules.yml` e seu harness de testes
   garantiram que os limites de ruido e o templating do payload permanecessem consistentes. CI
   executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` em cada mudanca; as mesmas regras
   foram exercitadas manualmente durante cada janela.
3. **Captura de dashboards.** Operadores exportaram os paineis routed-trace de
   `dashboards/grafana/soranet_sn16_handshake.json` (saude de handshake) e os dashboards de
   telemetria para correlacionar saude de filas com os outcomes de auditoria.
4. **Notas de revisao.** A secretaria de governanca registrou iniciais de revisores, decisao e
   tickets de mitigacao em `docs/source/nexus_transition_notes.md` e no tracker de config delta
   (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Achados

| Trace ID | Resultado | Evidencia | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Capturas de alerta fire/recover (link interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados em `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule`. | P95 de queue-admission ficou em 612 ms (meta <=750 ms). Sem follow-up. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Payload arquivado `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais hash de replay OTLP registrado em `status.md`. | Sais de redacao do SDK bateram com a baseline Rust; o bot de diff reportou zero deltas. |
| `TRACE-CONFIG-DELTA` | Pass (mitigacao fechada) | Entrada no tracker de governanca (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest do pack de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | A repeticao Q2 fez hash do perfil TLS aprovado e confirmou zero stragglers; o manifest de telemetria registra o range de slots 912-936 e a seed de workload `NEXUS-REH-2026Q2`. |

Todos os traces produziram pelo menos um evento `nexus.audit.outcome` dentro de
suas janelas, satisfazendo os guardrails do Alertmanager (`NexusAuditOutcomeFailure`
permaneceu verde no trimestre).

## Follow-ups

- O apendice routed-trace foi atualizado com o hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  (ver `nexus_transition_notes.md`); mitigacao `NEXUS-421` fechada.
- Continuar anexando replays OTLP brutos e artefatos de diff do Torii ao arquivo para
  reforcar evidencia de paridade nas revisoes AND4/AND7.
- Confirmar que os proximos ensaios `TRACE-MULTILANE-CANARY` reutilizem o mesmo helper de
  telemetria para que o sign-off Q2 aproveite o fluxo validado.

## Indice de artefatos

| Ativo | Localizacao |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alerta | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Payload de resultado exemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma e notas routed-trace | `docs/source/nexus_transition_notes.md` |

Este relatorio, os artefatos acima e os exports de alerta/telemetria devem ser
anexados ao log de decisao de governanca para fechar B1 no trimestre.
