---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-routed-trace-audit-2026q1
título: Relatório de auditorias de routed-trace 2026 Q1 (B1)
description: Espejo de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, que coleta os resultados da revisão trimestral de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenha ambas as cópias alinhadas até que você leia as traduções restantes.
:::

# Relatório de auditoria do Routed-Trace 2026 Q1 (B1)

O item do roteiro **B1 - Routed-Trace Audits & Telemetry Baseline** requer uma revisão trimestral do programa Routed-Trace de Nexus. Este documento informa a janela de auditoria do primeiro trimestre de 2026 (ano-março) para que o conselho de governo possa aprovar a postura de telemetria antes dos ensaios de lançamento do segundo trimestre.

## Alcance e linha de tempo

| ID de rastreamento | Ventana (UTC) | Objetivo |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Verifique histogramas de admissão de pista, fofoca de colas e fluxo de alertas antes de ativar multi-pista. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Validar replay OTLP, paridade do bot diff e ingestão de telemetria do SDK antes dos sucessos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Confirme os deltas de `iroha_config` aprovados pelo governo e a preparação de reversão antes do corte RC1. |

Cada ensaio é executado em topologia de tipo produção com a instrumentação routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), regras de Alertmanager carregadas e evidências exportadas em `docs/examples/`.

## Metodologia

1. **Recolha de telemetria.** Todos os nós emitiram o evento estruturado `nexus.audit.outcome` e as métricas acompanhantes (`nexus_audit_outcome_total*`). O auxiliar `scripts/telemetry/check_nexus_audit_outcome.py` hizo tail del log JSON, valida o estado do evento e arquiva a carga útil em `docs/examples/nexus_audit_outcomes/`. [scripts/telemetria/check_nexus_audit_outcome.py:1]
2. **Validação de alertas.** `dashboards/alerts/nexus_audit_rules.yml` e seu conjunto de testes garantem que os limites de ruído de alertas e os modelos de carga útil sejam mantidos consistentes. CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` em cada mudança; as mismas devem ser ejetadas manualmente durante cada janela.
3. **Captura de painéis.** Os operadores exportam os painéis de rastreamento roteado de `dashboards/grafana/soranet_sn16_handshake.json` (saúde de handshake) e os painéis de telemetria geral para correlacionar a saúde de cola com os resultados de auditoria.
4. **Notas de revisores.** A secretaria de governo registra iniciais de revisores, decisões e tickets de mitigação em [Nexus notas de transição](./nexus-transition-notes) e o rastreador de deltas de configuração (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Hallazgos

| ID de rastreamento | Resultado | Evidência | Notas |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | Passe | Capturas de alerta fire/recover (enlace interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diferenças de telemetria registradas em [notas de transição Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admissão de cola se mantuvo em 612 ms (objetivo <=750 ms). Não é necessário acompanhamento. |
| `TRACE-TELEMETRY-BRIDGE` | Passe | Carga útil arquivada `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json`, mas o hash de replay OTLP registrado em `status.md`. | Os sais de redação do SDK coincidiram com a base Rust; el diff bot reporta zero deltas. |
| `TRACE-CONFIG-DELTA` | Aprovado (mitigação encerrada) | Entrada no rastreador de governo (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifesto de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifesto de pacote de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | A reejecução Q2 hashio perfil TLS aprovado e confirmado com zero rezagados; o manifesto de telemetria registra a faixa de slots 912-936 e a semente de carga de trabalho `NEXUS-REH-2026Q2`. |

Todos os traços foram produzidos, menos um evento `nexus.audit.outcome` dentro de suas janelas, satisfazendo os guardrails do Alertmanager (`NexusAuditOutcomeFailure` se mantuvo em verde durante o trimestre).

## Acompanhamentos

- Se atualizar o apêndice routed-trace com o hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; a mitigação `NEXUS-421` foi cerrada nas notas de transição.
- Continuar adicionando replays OTLP sem processamento e artefatos de diferença de Torii ao arquivo para reforçar a evidência de paridade para revisões do Android AND4/AND7.
- Confirmar que os ensaios próximos `TRACE-MULTILANE-CANARY` reutilizam o mesmo auxiliar de telemetria para que a assinatura do Q2 se beneficie do fluxo validado.

## Índice de artefatos| Ativo | Localização |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Carga útil do resultado do exemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Rastreador de delta de configuração | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Agendar e notas de routed-trace | [Notas de transição Nexus](./nexus-transition-notes) |

Este relatório, os artefatos anteriores e as exportações de alertas/telemetria devem ser adjuntos ao registro de decisão de governo para encerrar o B1 do trimestre.