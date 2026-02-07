---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-routed-trace-audit-2026q1
título: A auditoria roteada do primeiro trimestre de 2026 (B1)
description: Зеркало `docs/source/nexus_routed_trace_audit_report_2026q1.md`, охватывающее итоги квартальных репетиций телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota História Canônica
Esta página contém `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Faça cópias sincronizadas, mas não será possível executá-lo.
:::

# Verificado Routed-Trace no primeiro trimestre de 2026 (B1)

Ponto roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** требует квартального обзора программы routed-trace Nexus. Isso foi obtido no final da auditoria do primeiro trimestre de 2026 (январь-mart), o que significa que você pode usar o telefone готовность перед репетициями запуска Q2.

## Область и таймлайн

| ID de rastreamento | Hoje (UTC) | Cel |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Проверить гистограммы допуска в lane, fofoca очередей и поток алертов перед включением multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Valide o replay OTLP, compartilhe o diff bot e use o SDK de telemetria para seu AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Altere os deltas de governança `iroha_config` e execute o rollback antes do RC1. |

A repetição de repetição é fornecida na topologia do produto com a ferramenta de rastreamento roteado `nexus.audit.outcome` + счетчики Prometheus), instale o Alertmanager e o transporte transferidos para `docs/examples/`.

## Metodologia

1. **Сбор телеметрии.** Você está usando a estrutura de referência `nexus.audit.outcome` e as métricas atualizadas (`nexus_audit_outcome_total*`). O ajudante `scripts/telemetry/check_nexus_audit_outcome.py` configurou o arquivo JSON, validou o status da carga e arquivou a carga útil no `docs/examples/nexus_audit_outcomes/`. [scripts/telemetria/check_nexus_audit_outcome.py:1]
2. **Alertas de segurança.** `dashboards/alerts/nexus_audit_rules.yml` e seu chicote de fios testado estão configurados para carregar a carga útil. CI запускает `dashboards/alerts/tests/nexus_audit_rules.test.yml` при каждом изменении; Você está pronto para realizar a programação no momento certo.
3. **Съемка дашбордов.** O operador transfere o painel roteado de `dashboards/grafana/soranet_sn16_handshake.json` (aperto de mão) e fecha os dados телеметрии, чтобы связать здоровье очередей с результатами аудита.
4. **Remova as alterações.** O segredo para atualizar as permissões iniciais, as regras e os bilhetes para mitigações em [Nexus notas de transição](./nexus-transition-notes) e трекер конфигурационных дельт (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Regulamentos

| ID de rastreamento | Isto | Documentar | Nomeação |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | Passe | Telas de alerta de disparo/recuperação (внутренняя ссылка) + repetição `dashboards/alerts/tests/soranet_lane_rules.test.yml`; телеметрийные дифы зафиксированы em [Nexus notas de transição](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 dura aproximadamente 612 ms (tempo <=750 ms). A configuração atual não é necessária. |
| `TRACE-TELEMETRY-BRIDGE` | Passe | Carga útil de arquivo `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mais hash de repetição OTLP, registrado em `status.md`. | Sais de redação SDK совпали с Linha de base de ferrugem; diff bot não é compatível. |
| `TRACE-CONFIG-DELTA` | Aprovado (mitigação encerrada) | Запись governança-трекера (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifesto de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifesto do pacote de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2 rerun захешировал одобренный perfil TLS e подтвердил отсутствие отстающих; manifesto de telemetria фиксирует диапазон слотов 912-936 e semente de carga de trabalho `NEXUS-REH-2026Q2`. |

Todos os rastreamentos são encontrados no `nexus.audit.outcome` na área de trabalho, onde os guardrails Alertmanager (`NexusAuditOutcomeFailure` são instalados) зеленым весь квартал).

## Acompanhamentos

- Дополнение routed-trace обновлено TLS хэшем `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; mitigação `NEXUS-421` está nas notas de transição.
- Produza replays OTLP e artefatos Torii diff em arquivos, use a partição доказательства для проверок Android AND4/AND7.
- Убедиться, что предстоящие репетиции `TRACE-MULTILANE-CANARY` используют тот же телеметрийный helper, чтобы Q2 sign-off опирался на проверенный fluxo de trabalho.

## Индекс артефактов

| Ativo | Localização |
|-------|----------|
| Validadores de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Alertas de alertas e testes | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Carga útil do resultado do exemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Configuração do Tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Gráficos e gráficos routed-trace | [Notas de transição Nexus](./nexus-transition-notes) |

Isso foi criado, artefactos você e esportes alertam/телеметрии должны быть приложены к журналу решений governança, чтобы закрыть B1 no quarto.