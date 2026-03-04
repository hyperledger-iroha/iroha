---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-routed-trace-audit-2026q1
כותרת: Relatorio de auditoria מנותב-מעקב 2026 Q1 (B1)
תיאור: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, cobrindo os resultados trimestrais das revisoes de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::שים לב Fonte canonica
Esta pagina reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenha as duas copias alinhadas ate que as traducoes restantes cheguem.
:::

# Relatorio de auditoria Routed-Trace 2026 Q1 (B1)

O item do map **B1 - Routed-Trace Audits & Telemetry Baseline** exige uma revisao trimestral do programa routed-trace do Nexus. Este relatorio documenta a janela de auditoria Q1 2026 (janeiro-marco) para que o conselho de governanca possa aprovar a postura de telemetria antes dos ensaios de lancamento Q2.

## Escopo e Cronograma

| מזהה מעקב | ג'נלה (UTC) | אובייקטיבו |
|--------|-------------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | אימות היסטוגרמות של אדמיסאו דה ליין, רכילות דה פילה e fluxo de alertas antes da habilitacao multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | תקף שידור חוזר של OTLP, פרידה לעשות הבדל בוט e ingestao de telemetria de SDK antes dos marcos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas de `iroha_config` aprovados pela governanca e prontidao de rollback antes do corte RC1. |

Cada ensaio rodou em topologia semelhante a producao com a instrumentacao routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), regras do Alertmanager carregadas e evidencia1070X para I1000NI.

## מתודולוגיה

1. **Coleta de telemetria.** Todos os nos emitiram o evento estruturado `nexus.audit.outcome` e as metricas associadas (`nexus_audit_outcome_total*`). O helper `scripts/telemetry/check_nexus_audit_outcome.py` fez tail do log JSON, validou o status do evento e arquivou oload pay em `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. ** Validacao de alertas.** `dashboards/alerts/nexus_audit_rules.yml` e seu רתמה de teste garantiram que os limiares de ruido e o templating do מטען מטען קבוע עקבי. O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` a cada mudanca; as mesmas regras foram exercitadas manualmente durante cada janela.
3. **Captura de Dashboards.** Operadores exportaram os paineis routed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (saude de לחיצת יד) e os dashboards de visao geral de telemetria para correlacionar a saude das filas com os resultados de auditoria.
4. **Notas de revisao.** A secretaria de governanca registrou iniciais dos revisores, decisao e tickets de mitigacao em [Nexus הערות מעבר](./nexus-transition-notes) e no tracker de delta de configuracao (00100NI).

## אחאדוס| מזהה מעקב | תוצאות | Evidencia | Notas |
|--------|--------|--------|-------|
| `TRACE-LANE-ROUTING` | לעבור | Capturas de alerta fire/recover (link interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados em [Nexus הערות מעבר](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admissao da fila permaneceu em 612 ms (alvo <=750 ms). מעקב סם. |
| `TRACE-TELEMETRY-BRIDGE` | לעבור | קובץ טעינה מטען `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` יש צורך ברישום חוזר של OTLP ב-`status.md`. | Os salts de redaction לעשות SDK bateram com בסיס חלודה; o הבדל בוט מדווח אפס דלתות. |
| `TRACE-CONFIG-DELTA` | מעבר (הקלה סגורה) | Entrada no tracker de governanca (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + Manifest de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + Manifest do Pacote de Telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | שידור חוזר של Q2 hashou o perfil TLS aprovado e confirmou zero tragglers; o manifest de telemetria registra o intervalo de slots 912-936 e o ​​seed עומס עבודה `NEXUS-REH-2026Q2`. |

Todos os traces produziram ao menos um evento `nexus.audit.outcome` dentro de suas janelas, satisfazendo os frans guard do Alertmanager (`NexusAuditOutcomeFailure` permaneceu verde no trimestre).

## מעקבים

- O apendice מנותב-trace foi atualizado com o hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; a mitigacao `NEXUS-421` foi encerrada nas הערות מעבר.
- שידורים חוזרים מתמשכים OTLP brutos e artefatos de diff do Torii ao arquivo para reforcar a Evidencia de paridade para revisoes Android AND4/AND7.
- אשר את החזרות הקרובות ל-`TRACE-MULTILANE-CANARY` reusal o mesmo helper de telemetria para que o sign-off Q2 to beneficie do workflow validado.

## אינדיס דה ארטפטוס

| אטיו | מקומי |
|-------|--------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| מטען תוצאת דוגמה | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de delta de configuracao | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma e notas routed-trace | [הערות מעבר Nexus](./nexus-transition-notes) |

Este relatorio, os artefatos acima e os exports de alertas/telemetria devem ser anexados ao log de decisao de governanca para fechar o B1 do trimestre.