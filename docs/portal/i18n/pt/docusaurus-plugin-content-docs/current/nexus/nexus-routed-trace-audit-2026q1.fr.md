---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-routed-trace-audit-2026q1
título: Relatório de auditoria roteado-traço 2026 Q1 (B1)
descrição: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, informa os resultados trimestrais das repetições de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Gardez as duas cópias alinhadas até que as traduções restantes cheguem.
:::

# Relatório de auditoria Routed-Trace 2026 Q1 (B1)

O item de roteiro **B1 - Routed-Trace Audits & Telemetry Baseline** exige uma revisão trimestral do programa Routed-Trace Nexus. Este relatório documenta a janela de auditoria do primeiro trimestre de 2026 (janvier-marte) para que o conselho de governo possa validar a postura de telemetria antes das repetições do lançamento do segundo trimestre.

## Porta e calendário

| ID de rastreamento | Fenetre (UTC) | Objetivo |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Verifique os histogramas de admissão de pistas, o rastreamento de arquivos e o fluxo de alertas antes da ativação de múltiplas pistas. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Valide o replay OTLP, a parte do bot diff e a ingestão do SDK de telemetria antes dos servidores AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Confirme os deltas `iroha_config` aprovados pelo governo e pela preparação para a reversão antes do corte RC1. |

Cada repetição gira em torno de uma topologia próxima à produção com a instrumentação Routed-Trace ativa (telemetria `nexus.audit.outcome` + computadores Prometheus), as regras do Alertmanager são cobradas e os preços exportados em `docs/examples/`.

## Metodologia

1. **Coleta de telemetria.** Todas as noites emis a estrutura de evento `nexus.audit.outcome` e as métricas associadas (`nexus_audit_outcome_total*`). O ajudante `scripts/telemetry/check_nexus_audit_outcome.py` segue o log JSON, valida o status do evento e arquiva a carga útil sob `docs/examples/nexus_audit_outcomes/`. [scripts/telemetria/check_nexus_audit_outcome.py:1]
2. **Validação de alertas.** `dashboards/alerts/nexus_audit_rules.yml` e seu equipamento de teste garantem que seus resultados de ruído e modelos de cargas úteis permaneçam coerentes. CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` uma modificação chaque; os memes regles ont ete exercem manualmente pingente cada fenetre.
3. **Captura de painéis.** Os operadores exportam os painéis roteados a partir de `dashboards/grafana/soranet_sn16_handshake.json` (sante handshake) e os painéis de telemetria globais para correlacionar a segurança dos arquivos com os resultados da auditoria.
4. **Notas dos leitores.** O secretário de governo envia as iniciais, a decisão e os tickets de mitigação nas [notas de transição Nexus](./nexus-transition-notes) e no rastreador de deltas de configuração (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##Constatações

| ID de rastreamento | Resultado | Preúves | Notas |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | Passe | Captura alerta de incêndio/recuperação (garantia interna) + replay `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diferenças de registro de telemetria em [notas de transição Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | O P95 de admissão de arquivo tem duração de 612 ms (cível <=750 ms). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | Passe | Arquivo de carga útil `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais hash de repetição OTLP registrado em `status.md`. | As versões do SDK de redação correspondem à base Rust; le diff bot um sinal zero delta. |
| `TRACE-CONFIG-DELTA` | Aprovado (mitigação encerrada) | Entrada do rastreador de governo (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + perfil de manifesto TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifesto do pacote de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | A reexecução Q2 a hashe le perfil TLS aprovou e confirmou zero retardatários; a telemetria do manifesto registra a placa dos slots 912-936 e a semente de carga de trabalho `NEXUS-REH-2026Q2`. |

Todos os vestígios foram produzidos pelo menos um evento `nexus.audit.outcome` em suas janelas, satisfazendo os guardrails Alertmanager (`NexusAuditOutcomeFailure` estão restantes no trimestre).

## Suívis

- O anexo routed-trace foi adicionado hoje com o hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; a mitigação `NEXUS-421` está encerrada nas notas de transição.
- Continue juntando os replays OTLP brutos e os artefatos de diff Torii ao arquivo para reforçar a pré-visualização de parite para as revistas Android AND4/AND7.
- Confirme que os ensaios prochaines `TRACE-MULTILANE-CANARY` reutilizam o meme helper de telemetria para que a validação Q2 beneficie du workflow valide.

## Índice de artefatos| Ativo | Colocação |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alerta | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Exemplo de resultado da carga útil | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Rastreador de deltas de configuração | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Planejamento e notas roteadas-traço | [Notas de transição Nexus](./nexus-transition-notes) |

Este relatório, os artefatos ci-dessus e as exportações de alertas/telemetria devem ser anexados ao jornal de decisão de governo para o fechamento do B1 do trimestre.