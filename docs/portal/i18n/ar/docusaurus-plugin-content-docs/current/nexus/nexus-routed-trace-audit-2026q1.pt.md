---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-routed-trace-audit-2026q1
title: Relatorio de auditoria routed-trace 2026 Q1 (B1)
description: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, cobrindo os resultados trimestrais das revisoes de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Fonte canonica
Esta pagina reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenha as duas copias alinhadas ate que as traducoes restantes cheguem.
:::

# Relatorio de auditoria Routed-Trace 2026 Q1 (B1)

O item do roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** exige uma revisao trimestral do programa routed-trace do Nexus. Este relatorio documenta a janela de auditoria Q1 2026 (janeiro-marco) para que o conselho de governanca possa aprovar a postura de telemetria antes dos ensaios de lancamento Q2.

## Escopo e cronograma

| Trace ID | Janela (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verificar histogramas de admissao de lane, gossip de fila e fluxo de alertas antes da habilitacao multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Validar replay OTLP, paridade do diff bot e ingestao de telemetria de SDK antes dos marcos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas de `iroha_config` aprovados pela governanca e prontidao de rollback antes do corte RC1. |

Cada ensaio rodou em topologia semelhante a producao com a instrumentacao routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), regras do Alertmanager carregadas e evidencia exportada para `docs/examples/`.

## Metodologia

1. **Coleta de telemetria.** Todos os nos emitiram o evento estruturado `nexus.audit.outcome` e as metricas associadas (`nexus_audit_outcome_total*`). O helper `scripts/telemetry/check_nexus_audit_outcome.py` fez tail do log JSON, validou o status do evento e arquivou o payload em `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Validacao de alertas.** `dashboards/alerts/nexus_audit_rules.yml` e seu harness de teste garantiram que os limiares de ruido e o templating do payload permanecessem consistentes. O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` a cada mudanca; as mesmas regras foram exercitadas manualmente durante cada janela.
3. **Captura de dashboards.** Operadores exportaram os paineis routed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (saude de handshake) e os dashboards de visao geral de telemetria para correlacionar a saude das filas com os resultados de auditoria.
4. **Notas de revisao.** A secretaria de governanca registrou iniciais dos revisores, decisao e tickets de mitigacao em [Nexus transition notes](./nexus-transition-notes) e no tracker de delta de configuracao (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Achados

| Trace ID | Resultado | Evidencia | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Capturas de alerta fire/recover (link interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados em [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admissao da fila permaneceu em 612 ms (alvo <=750 ms). Sem follow-up. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Payload arquivado `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais hash de replay OTLP registrado em `status.md`. | Os salts de redaction do SDK bateram com a base Rust; o diff bot reportou zero deltas. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Entrada no tracker de governanca (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest do pacote de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | A rerun Q2 hashou o perfil TLS aprovado e confirmou zero stragglers; o manifest de telemetria registra o intervalo de slots 912-936 e o workload seed `NEXUS-REH-2026Q2`. |

Todos os traces produziram ao menos um evento `nexus.audit.outcome` dentro de suas janelas, satisfazendo os guardrails do Alertmanager (`NexusAuditOutcomeFailure` permaneceu verde no trimestre).

## Follow-ups

- O apendice routed-trace foi atualizado com o hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; a mitigacao `NEXUS-421` foi encerrada nas transition notes.
- Continuar anexando replays OTLP brutos e artefatos de diff do Torii ao arquivo para reforcar a evidencia de paridade para revisoes Android AND4/AND7.
- Confirmar que as proximas rehearsals `TRACE-MULTILANE-CANARY` reutilizem o mesmo helper de telemetria para que o sign-off de Q2 se beneficie do workflow validado.

## Indice de artefatos

| Ativo | Local |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Payload de outcome de exemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de delta de configuracao | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma e notas routed-trace | [Nexus transition notes](./nexus-transition-notes) |

Este relatorio, os artefatos acima e os exports de alertas/telemetria devem ser anexados ao log de decisao de governanca para fechar o B1 do trimestre.
