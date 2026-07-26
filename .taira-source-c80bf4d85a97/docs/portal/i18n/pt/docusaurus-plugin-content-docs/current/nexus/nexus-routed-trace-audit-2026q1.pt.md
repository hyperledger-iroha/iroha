---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-routed-trace-audit-2026q1
título: Relatório de auditoria routed-trace 2026 Q1 (B1)
descrição: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, cobrindo os resultados trimestrais das revisões de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenha as duas cópias homologadas até que as traduções restantes cheguem.
:::

# Relatório de auditoria Routed-Trace 2026 Q1 (B1)

O item do roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** exige uma revisão trimestral do programa Routed-Trace do Nexus. Este relatório documenta a janela de auditorias do 1º trimestre de 2026 (janeiro-marco) para que o conselho de governança possa aprovar a postura de telemetria antes dos ensaios de lançamento do 2º trimestre.

## Escopo e cronograma

| ID de rastreamento | Janela (UTC) | Objetivo |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Verifique histogramas de admissão de pista, fofocas de fila e fluxo de alertas antes da habilitação multi-faixa. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Validar replay OTLP, paridade do diff bot e ingestão de telemetria de SDK antes dos marcos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Confirmar deltas de `iroha_config` aprovados pela governança e prontidão de rollback antes do corte RC1. |

Cada ensaio rodou em topologia semelhante à produção com a instrumentação routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), regras do Alertmanager fornecidos e evidências exportadas para `docs/examples/`.

## Metodologia

1. **Coleta de telemetria.** Todos nos emitiram o evento estruturado `nexus.audit.outcome` e as métricas associadas (`nexus_audit_outcome_total*`). O helper `scripts/telemetry/check_nexus_audit_outcome.py` fez tail do log JSON, validou o status do evento e arquivou o payload em `docs/examples/nexus_audit_outcomes/`. [scripts/telemetria/check_nexus_audit_outcome.py:1]
2. **Validação de alertas.** `dashboards/alerts/nexus_audit_rules.yml` e seu chicote de teste garantem que os limites de ruido e o modelo de carga útil permaneçam consistentes. O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` a cada mudança; as mesmas regras foram exercitadas manualmente durante cada janela.
3. **Captura de dashboards.** Os operadores exportaram os paineis routed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (saude de handshake) e os dashboards de visão geral de telemetria para correlacionar a saúde das filas com os resultados de auditorias.
4. **Notas de revisão.** A secretaria de governança registrou inicialmente dos revisores, decisão e tickets de mitigação em [Nexus Transit Notes](./nexus-transition-notes) e no tracker de delta de configuração (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##Achados

| ID de rastreamento | Resultado | Evidência | Notas |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | Passe | Capturas de alerta fire/recover (link interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diferenças de telemetria registradas em [notas de transição Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admissão da fila encontrada em 612 ms (alvo <=750 ms). Sem acompanhamento. |
| `TRACE-TELEMETRY-BRIDGE` | Passe | Payload arquivado `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais hash de replay OTLP registrado em `status.md`. | Os sais de redação do SDK bateram com a base Rust; o diff bot relatou zero deltas. |
| `TRACE-CONFIG-DELTA` | Aprovado (mitigação encerrada) | Entrada no tracker de governança (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifesto de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifesto do pacote de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Uma reexecução do segundo trimestre tem o perfil do TLS aprovado e confirmado com zero retardatários; o manifesto de telemetria registra o intervalo de slots 912-936 e o ​​workload seed `NEXUS-REH-2026Q2`. |

Todos os traces produziram ao menos um evento `nexus.audit.outcome` dentro de suas janelas, satisfazendo os guardrails do Alertmanager (`NexusAuditOutcomeFailure` perguntas verdes no trimestre).

## Acompanhamentos

- O apêndice routed-trace foi atualizado com o hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; a mitigação `NEXUS-421` foi encerrada nas notas de transição.
- Continuar anexando replays OTLP brutos e artefatos de diff do Torii ao arquivo para reforçar a evidência de paridade para revisões Android AND4/AND7.
- Confirmar que as próximas ensaios `TRACE-MULTILANE-CANARY` reutilizam o mesmo helper de telemetria para que o sign-off de Q2 se beneficie do workflow validado.

## Índice de artistas

| Ativo | Locais |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Carga útil de resultado de exemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de delta de configuração | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma e notas routed-trace | [Notas de transição Nexus](./nexus-transition-notes) |Este relatorio, os artistas acima e as exportações de alertas/telemetria devem ser anexados ao log de decisão de governança para fechar o B1 do trimestre.