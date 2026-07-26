---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
título: Traza enrutada del primer trimestre de 2026 آڈٹ رپورٹ (B1)
descripción: `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ، جو سہ ماہی ٹیلیمیٹری ریہرسل کے نتائج کا احاطہ کرتا ہے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی تراجم آنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# 2026 Q1 Seguimiento enrutado آڈٹ رپورٹ (B1)

روڈ میپ آئٹم **B1 - Línea base de telemetría y auditorías de seguimiento enrutado** Nexus پروگرام کی سہ ماہی جائزے کا تقاضا کرتا ہے۔ یہ رپورٹ Q1 2026 (جنوری-مارچ) کی آڈٹ ونڈو دستاویز کرتی ہے تاکہ گورننس کونسل Q2 لانچ ریہرسل سے پہلے ٹیلیمیٹری پوزیشن کی منظوری دے سکے۔

## دائرہ کار اور ٹائم لائن

| ID de seguimiento | ونڈو (UTC) | مقصد |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | multicarril فعال کرنے سے پہلے histogramas de admisión de carriles, chismes de colas اور flujo de alertas کی تصدیق۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Hitos AND4/AND7, repetición de OTLP, paridad de bot de diferencias e ingestión de telemetría de SDK |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 cortó سے پہلے deltas `iroha_config` aprobados por la gobernanza y preparación para la reversión کی تصدیق۔ |

ہر ریہرسل topología similar a producción پر instrumentación de seguimiento enrutado کے ساتھ چلائی گئی (telemetría `nexus.audit.outcome` + contadores Prometheus), reglas de Alertmanager لوڈ تھے، اور evidencia `docs/examples/` میں ایکسپورٹ ہوا۔

## طریقہ کار1. **ٹیلیمیٹری کلیکشن۔** تمام نوڈز نے estructurado `nexus.audit.outcome` ایونٹ اور متعلقہ métricas (`nexus_audit_outcome_total*`) emiten کیں۔ ayudante `scripts/telemetry/check_nexus_audit_outcome.py` y cola de registro JSON, configuración de carga útil y carga útil `docs/examples/nexus_audit_outcomes/` کیا۔ [scripts/telemetría/check_nexus_audit_outcome.py:1]
2. **الرٹ ویلیڈیشن۔** `dashboards/alerts/nexus_audit_rules.yml` اور اس کا arnés de prueba یہ یقینی بناتے رہے کہ umbrales de ruido de alerta اور plantillas de carga útil مسلسل رہیں۔ CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ یہی رولز ہر ونڈو میں دستی طور پر بھی چلائے گئے۔
3. **ڈیش بورڈ کیپچر۔** آپریٹرز نے `dashboards/grafana/soranet_sn16_handshake.json` (salud del protocolo de enlace), paneles de seguimiento enrutado y paneles de control de descripción general de telemetría ایکسپورٹ کیے تاکہ estado de la cola کو resultados de auditoría کے ساتھ se correlacionan کیا جا سکے۔
4. **ریویو نوٹس۔** گورننس سیکرٹری نے iniciales del revisor, فیصلہ اور entradas de mitigación کو [notas de transición Nexus] (./nexus-transition-notes) اور config delta tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) میں لاگ کیا۔

## نتائج| ID de seguimiento | نتیجہ | ثبوت | نوٹس |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pase | Alerta de incendio/recuperación اسکرین شاٹس (اندرونی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` repetición؛ diferencias de telemetría [notas de transición Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ | Admisión a cola P95 612 ms پر رہا (ہدف <=750 ms)۔ فالو اپ درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | Pase | Carga útil de resultados archivados `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` اور OTLP hash de reproducción `status.md` میں ریکارڈ۔ | Sales de redacción del SDK Línea base de Rust سے coincidencia تھے؛ diff bot نے cero deltas رپورٹ کیے۔ |
| `TRACE-CONFIG-DELTA` | Pasa (mitigación cerrada) | Entrada del rastreador de gobernanza (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifiesto de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifiesto del paquete de telemetría (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Repetición del segundo trimestre con hash de perfil TLS y sin rezagados con cero rezagados manifiesto de telemetría نے ranuras 912-936 اور semilla de carga de trabajo `NEXUS-REH-2026Q2` درج کیا۔ |

تمام rastros نے اپنی ونڈوز کے اندر کم از کم ایک `nexus.audit.outcome` ایونٹ پیدا کیا، جس سے Alertmanager guardrails پورے ہوئے (`NexusAuditOutcomeFailure` پورے کوارٹر میں گرین رہا)۔

## Seguimientos- Apéndice de seguimiento enrutado کو TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ اپ ڈیٹ کیا گیا؛ mitigación `NEXUS-421` notas de transición میں بند کیا گیا۔
- Revisiones de Android AND4/AND7 کے لئے evidencia de paridad مضبوط کرنے کی خاطر repeticiones OTLP sin procesar اور Torii artefactos de diferenciación کو آرکائیو کے ساتھ منسلک کرتے رہیں۔
- تصدیق کریں کہ آنے والی `TRACE-MULTILANE-CANARY` ensayos y ayudante de telemetría دوبارہ استعمال کریں تاکہ Flujo de trabajo validado de aprobación del segundo trimestre سے فائدہ اٹھائے۔

## Artefacto انڈیکس

| Activo | مقام |
|-------|----------|
| Validador de telemetría | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Reglas y pruebas de alerta | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Carga útil de resultados de muestra | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Configurar rastreador delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Calendario y notas de seguimiento de rutas | [Notas de transición Nexus](./nexus-transition-notes) |

یہ رپورٹ، اوپر دیے گئے artefactos اور exportaciones de alertas/telemetría کو registro de decisiones de gobernanza کے ساتھ منسلک کیا جانا چاہئے تاکہ اس کوارٹر کے لئے B1 بند ہو جائے۔