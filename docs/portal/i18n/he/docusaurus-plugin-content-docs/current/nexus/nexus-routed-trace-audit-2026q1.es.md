---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-routed-trace-audit-2026q1
כותרת: Informe de auditoria de routed-trace 2026 Q1 (B1)
תיאור: Espejo de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, que cubre los resultados de la revision trimestral de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Manten ambas copias alineadas hasta que lleguen las traducciones restantes.
:::

# Informe de auditoria de Routed-Trace 2026 Q1 (B1)

הפריט של מפת הדרכים **B1 - ביקורת מעקב מנותב ובסיס טלמטריה** דורשת עדכון טרימסטרלי של תכנית מעקב מנותב של Nexus. Este informe documenta la ventana de auditoria Q1 2026 (enero-marzo) para que el consejo de gobernanza pueda aprobar la postura de telemetria antes de los ensayos de lanzamiento Q2.

## Alcance y linea de tiempo

| מזהה מעקב | ונטנה (UTC) | אובייקטיבו |
|--------|-------------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | אימות היסטוגרמות של כניסת נתיב, רכילות של קולות ושפעת אזהרות לפני מספר נתיבים. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | תקף שידור חוזר של OTLP, פרידה דל הבדל בוט וכניסה של טלמטריה של SDK לפני התקדמות AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas de `iroha_config` aprobados por gobernanza y la preparacion de rollback antes del corte RC1. |

Cada ensayo se ejecuto en topologia tipo produccion con la instrumentacion מנותב-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), regglas de Alertmanager cargadas y evidencia exportada en I0000170X.

## מתודולוגיה

1. **Recoleccion de telemetria.** Todos los nodos emitieron el evento estructurado `nexus.audit.outcome` y las metricas acompanantes (`nexus_audit_outcome_total*`). העוזר `scripts/telemetry/check_nexus_audit_outcome.py` הוא זנב של לוג JSON, תקף לאירועים וארכיון מטען ב-`docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. ** Validacion de alertas.** `dashboards/alerts/nexus_audit_rules.yml` y su רתום de pruebas aseguraron que los umbrales de ruido de alertas y el templating del payload se mantuvieran consistentes. CI ejecuta `dashboards/alerts/tests/nexus_audit_rules.test.yml` en cada cambio; las mismas reglas se ejercitaron manualmente durante cada ventana.
3. **Captura de Dashboards.** Los Operatores Exportaron Los Paneles Routed Trace de `dashboards/grafana/soranet_sn16_handshake.json` (הסימון לחיצת יד) y Los Dashboards de Telemetria General para correlacionar la salud de colas con los resultados de auditoria.
4. **Notas de revisores.** La secretaria de gobernanza registro iniciales de revisores, decision y tickets de mitigacion en [Nexus הערות מעבר](./nexus-transition-notes) y el tracker de deltas de configuracion (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`00).

## חלאזגוס| מזהה מעקב | תוצאות | Evidencia | Notas |
|--------|--------|--------|-------|
| `TRACE-LANE-ROUTING` | לעבור | Capturas de alerta fire/recover (enlace interno) + שידור חוזר של `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados en [Nexus הערות מעבר](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admision de cola se mantuvo ב-612 אלפיות השנייה (מטרה <=750 אלפיות השנייה). אין דרישה לסירוגין. |
| `TRACE-TELEMETRY-BRIDGE` | לעבור | ארכיון מטען `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mas el hash de replay OTLP registrado en `status.md`. | Los salts de redaccion de SDK coincidieron con la base Rust; el diff bot reporto cero deltas. |
| `TRACE-CONFIG-DELTA` | מעבר (הקלה סגורה) | Entrada en el tracker de gobernanza (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + מניפסט של TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + מניפסט של טלמטריה (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La reejecucion Q2 hashio el perfil TLS aprobado y confirmo cero rezagados; el manifest de telemetria registra el rango de slots 912-936 y el load work seed `NEXUS-REH-2026Q2`. |

Todos los traces produjeron al menos un evento `nexus.audit.outcome` dentro de sus ventanas, satisfaciendo los guards de Alertmanager (`NexusAuditOutcomeFailure` se mantuvo en verde durante el trimestre).

## מעקבים

- Se actualizo el apendice routed-trace con el hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; la mitigacion `NEXUS-421` עם הערות מעבר.
- שידורים חוזרים מתמשכים של OTLP בעיבודים של חפצי אמנות שונים של Torii אל ארכיון עבור רפורזאר להוכחות פרידה עבור גרסאות של אנדרואיד AND4/AND7.
- אישור חזרות לאס פרוקסימאס `TRACE-MULTILANE-CANARY` reutilicen el mismo helper de telemetria para que el sign-off de Q2 se beneficie del flujo validado.

## מדד חפצים

| Activo | Ubicacion |
|-------|--------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Reglas y tests de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| מטען תוצאה דה דוגמה | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de delta de configuracion | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| לוח זמנים y notas de routed-trace | [הערות מעבר Nexus](./nexus-transition-notes) |

Este informe, los artefactos anteriores y las exportaciones de alertas/telemetria deben adjuntarse al registro de decision de gobernanza para cerrar B1 del trimestre.