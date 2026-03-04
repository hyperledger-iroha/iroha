---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-routed-trace-audit-2026q1
título: Atualizado em routed-trace no primeiro trimestre de 2026 (B1)
description: Verifique se o `docs/source/nexus_routed_trace_audit_report_2026q1.md` está configurado para não causar problemas.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note المصدر القانوني
Verifique o valor `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Verifique se há algum problema com isso.
:::

# تقرير تدقيق Routed-Trace no primeiro trimestre de 2026 (B1)

Você pode usar **B1 - Routed-Trace Audits & Telemetry Baseline** para usar o Routed-Trace no Nexus. No primeiro trimestre de 2026 (يناير-مارس) O segundo trimestre.

## النطاق والجدول الزمني

| ID de rastreamento | النافذة (UTC) | الهدف |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | A faixa de rodagem, a fofoca e a faixa múltipla são possíveis. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Você pode usar o OTLP, e o bot diff, e usar o SDK no conjunto AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Verifique se o `iroha_config` está no lugar certo e se está conectado ao RC1. |

Você pode usar o Routed-Trace (`nexus.audit.outcome` + عدادات) Prometheus) e o Alertmanager pode ser usado para `docs/examples/`.

## المنهجية

1. **جمع التليمتري.** Use o código `nexus.audit.outcome` e o código `nexus_audit_outcome_total*`. O `scripts/telemetry/check_nexus_audit_outcome.py` é definido como JSON e o `docs/examples/nexus_audit_outcomes/`. [scripts/telemetria/check_nexus_audit_outcome.py:1]
2. **التحقق من التنبيهات.** ضمنت `dashboards/alerts/nexus_audit_rules.yml` e واداة الاختبار الخاصة بها بقاء عتبات ضوضاء التنبيه وقوالب الحمولة متسقة. Use CI como `dashboards/alerts/tests/nexus_audit_rules.test.yml` para obter mais informações E não se esqueça de fazer isso.
3. **التقاط لوحات المراقبة.** قام المشغلون بتصدير roteed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (صحة المصافحة) e Certifique-se de que o dispositivo esteja funcionando corretamente.
4. **ملاحظات المراجعين. ** [Notas de transição Nexus](./nexus-transition-notes) e as notas de transição (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج

| ID de rastreamento | النتيجة | الدليل | الملاحظات |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | Passe | لقطات تنبيه disparar/recuperar (رابط داخلي) + اعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; O código de transição foi criado em [Nexus notas de transição](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | A duração do P95 é de 612 ms (até <=750 ms). Não há problema em fazê-lo. |
| `TRACE-TELEMETRY-BRIDGE` | Passe | Você pode usar `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` para reproduzir o arquivo OTLP em `status.md`. | تطابقت salts الخاصة بتنقيح SDK مع خط Rust الاساس؛ O diff bot está no lugar certo. |
| `TRACE-CONFIG-DELTA` | Aprovado (mitigação encerrada) | سجل متتبع الحوكمة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifesto como TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifesto حزمة التليمتري (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | O segundo trimestre deve ser usado no TLS e no seu site. سجل manifest التليمتري نطاق الفتحات 912-936 وبذرة الحمل `NEXUS-REH-2026Q2`. |

انتجت جميع الـ traces على الاقل حدثا واحدا `nexus.audit.outcome` ضمن نوافذها, بما يلبي حواجز Alertmanager (`NexusAuditOutcomeFailure` بقي اخضر طوال الربع).

## المتابعات

- تم تحديث ملحق routed-trace ببصمة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; Use o `NEXUS-421` nas notas de transição.
- O software de configuração OTLP e o diff do Torii estão disponíveis para download Android AND4/AND7.
- Você pode usar o `TRACE-MULTILANE-CANARY` para obter mais informações sobre o produto. No segundo trimestre do ano passado.

## Artefatos فهرس

| الاصل | الموقع |
|-------|----------|
| مدقق التليمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Produtos e serviços | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Resultado do resultado | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Máquinas de lavar roupa | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Roteed-trace e equipamentos | [Notas de transição Nexus](./nexus-transition-notes) |

Faça o download do seu cartão de crédito e verifique o valor do seu cartão de crédito. O B1 está disponível.