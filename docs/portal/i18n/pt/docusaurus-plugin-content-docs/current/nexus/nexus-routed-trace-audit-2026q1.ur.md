---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-routed-trace-audit-2026q1
título: 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)
description: `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ, جو سہ ماہی ٹیلیمیٹری ریہرسل کے نتائج کا احاطہ کرتا ہے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی تراجم آنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)

روڈ میپ آئٹم **B1 - Routed-Trace Audits & Telemetry Baseline** Nexus Routed-Trace پروگرام کی سہ ماہی جائزے کا تقاضا کرتا ہے۔ No primeiro trimestre de 2026 (جنوری-مارچ) ریہرسل سے پہلے ٹیلیمیٹری پوزیشن کی منظوری دے سکے۔

## دائرہ کار اور ٹائم لائن

| ID de rastreamento | Tóquio (UTC) | مقصد |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | multi-lane فعال کرنے سے پہلے histogramas de admissão de faixa, fofocas de fila e fluxo de alerta کی تصدیق۔ |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Marcos AND4/AND7 سے پہلے replay OTLP, paridade de bot diff e ingestão de telemetria SDK کی توثیق۔ |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Corte RC1 سے پہلے deltas `iroha_config` aprovados pela governança e prontidão para reversão کی تصدیق۔ |

ہر ریہرسل topologia semelhante à produção پر instrumentação de rastreamento roteado کے ساتھ چلائی گئی (telemetria `nexus.audit.outcome` + contadores Prometheus), regras do Alertmanager لوڈ تھے, اور evidência `docs/examples/` میں ایکسپورٹ ہوا۔

## طریقہ کار

1. **ٹیلیمیٹری کلیکشن۔** تمام نوڈز نے estruturado `nexus.audit.outcome` ایونٹ اور متعلقہ métricas (`nexus_audit_outcome_total*`) emitem کیں۔ helper `scripts/telemetry/check_nexus_audit_outcome.py` JSON log tail کیا۔ [scripts/telemetria/check_nexus_audit_outcome.py:1]
2. **الرٹ ویلیڈیشن۔** `dashboards/alerts/nexus_audit_rules.yml` اور اس کا equipamento de teste یہ یقینی بناتے رہے کہ limites de ruído de alerta e modelos de carga útil مسلسل رہیں۔ CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ یہی رولز ہر ونڈو میں دستی طور پر بھی چلائے گئے۔
3. **ڈیش بورڈ کیپچر۔** آپریٹرز نے `dashboards/grafana/soranet_sn16_handshake.json` (handshake health) سے painéis de rastreamento roteado اور painéis de visão geral de telemetria ایکسپورٹ کیے تاکہ fila saúde کو resultados de auditoria کے ساتھ correlacionar کیا جا سکے۔
4. **ریویو نوٹس۔** گورننس سیکرٹری نے iniciais do revisor, فیصلہ اور tíquetes de mitigação کو [Nexus notas de transição](./nexus-transition-notes) اور config delta tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) میں لاگ کیا۔

## نتائج

| ID de rastreamento | Não | ثبوت | Não |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | Passe | Alerta de disparo/recuperação اسکرین شاٹس (اندرونی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` replay؛ diferenças de telemetria [Nexus notas de transição](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ | Admissão na fila P95 612 ms پر رہا (ہدف <=750 ms)۔ فالو اپ درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | Passe | Carga útil de resultado arquivado `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` e hash de repetição OTLP `status.md` میں ریکارڈ۔ | Sais de redação do SDK Linha de base de ferrugem سے correspondência تھے؛ diff bot com zero deltas |
| `TRACE-CONFIG-DELTA` | Aprovado (mitigação encerrada) | Entrada do rastreador de governança (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifesto do perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifesto do pacote de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Reexecução do segundo trimestre نے منظور شدہ Hash de perfil TLS کیا اور zero retardatários کی تصدیق کی؛ manifesto de telemetria نے slots 912-936 اور workload seed `NEXUS-REH-2026Q2` درج کیا۔ |

تمام traces نے اپنی ونڈوز کے اندر کم از کم ایک `nexus.audit.outcome` ایونٹ پیدا کیا, جس سے Alertmanager guardrails پورے ہوئے (`NexusAuditOutcomeFailure` پورے کوارٹر میں گرین رہا)۔

## Acompanhamentos

- Apêndice de rastreamento roteado کو hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ اپ ڈیٹ کیا گیا؛ mitigação `NEXUS-421` notas de transição میں بند کیا گیا۔
- Android AND4/AND7 análises کے لئے evidência de paridade مضبوط کرنے کی خاطر replays OTLP brutos اور Torii artefatos diff کو آرکائیو کے ساتھ منسلک کرتے رہیں۔
- تصدیق کریں کہ آنے والی Ensaios `TRACE-MULTILANE-CANARY` وہی auxiliar de telemetria دوبارہ استعمال کریں تاکہ Aprovação do segundo trimestre fluxo de trabalho validado سے فائدہ اٹھائے۔

## Artefato

| Ativo | مقام |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de alerta | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Exemplo de carga útil de resultado | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Configurar rastreador delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma e notas de rastreamento roteado | [Notas de transição Nexus](./nexus-transition-notes) |

یہ رپورٹ, اوپر دیے گئے artefatos اور exportações de alerta/telemetria کو registro de decisão de governança کے ساتھ منسلک کیا جانا چاہئے تاکہ اس کوارٹر کے لئے B1 بند ہو جائے۔