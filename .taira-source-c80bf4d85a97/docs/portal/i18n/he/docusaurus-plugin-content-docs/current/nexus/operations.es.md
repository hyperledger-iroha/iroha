---
lang: he
direction: rtl
source: docs/portal/docs/nexus/operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operations
כותרת: Runbook de operaciones de Nexus
תיאור: רשימות קורות חיים עבור אל קמפו דל flujo de trabajo del operador de Nexus, que refleja `docs/source/nexus_operations.md`.
---

Usa esta page como el hermano de referencia rapida de `docs/source/nexus_operations.md`. קורות חיים לרשימה אופרטיבית, אוספי התנועה והדרישות של קוברטורה טלמטריה que los operations de Nexus deben seguir.

## Lista de ciclo de vida

| אטאפה | Acciones | Evidencia |
|-------|--------|--------|
| Pre-vuelo | Verifica hashes/firmas de lanzamiento, confirma `profile = "iroha3"` y prepara plantillas de configuracion. | Salida de `scripts/select_release_profile.py`, רישום בדיקת סכום, חבילה דה manifiestos firmado. |
| Alineacion del catalogo | Actualiza el catalogo `[nexus]`, la politica de enrutamiento y los umbrales de DA segun el manifiesto emitido por el consejo, y luego captura `--trace-config`. | Salida de `irohad --sora --config ... --trace-config` almacenada עם כרטיס העלייה למטוס. |
| Pruebas de humo y corte | Ejecuta `irohad --sora --config ... --trace-config`, ejecuta la prueba de humo del CLI (`FindNetworkStatus`), valida las exportaciones de telemetria y solicita admision. | Log de test-smoking + confirmacion de Alertmanager. |
| Estado estable | לפקח על לוחות מחוונים/אזהרות, סיבובי סיבובים של קדנציה וגוברננזה ותצורות/ספרי הפעלה של סינקרוניזה. | דקות של עדכון טרימסטרלי, קלטות של לוחות מחוונים, מזהי כרטיסים לסיבוב. |

El onboarding detallado (reemplazo de claves, plantillas de enrutamiento, pasos del perfil de lanzamiento) permanece en `docs/source/sora_nexus_operator_onboarding.md`.

## Gestion de cambios

1. **Actualizaciones de lanzamiento** - sigue anuncios en `status.md`/`roadmap.md`; תוספת לרשימה לכניסה למערכת יחסי ציבור לשחרור.
2. **Cambios de manifiestos de lane** - Verifica Bunles firmados del Space Directory y Archivados bajo `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuracion** - cada cambio en `config/config.toml` דורש כרטיס que reference lane/dataspace. Guarda una copia redactada de la configuracion efectiva cuando los nodos se unan o se actualicen.
4. **Simulacros de rollback** - ensaya trimestralmente procedimientos de stop/restore/smoke; רשם תוצאות ב-`docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprobaciones de compliance** - lanes privadas/CBDC deben asegurar el visto bueno de compliance antes de modificar la politica de DA o los knobs de redaccion de telemetria (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetria y SLOs- לוחות מחוונים: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, מראות especificas de SDK (לדוגמה, `android_operator_console.json`).
- התראות: `dashboards/alerts/nexus_audit_rules.yml` y regglas de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- מדדים כמשמר:
  - `nexus_lane_height{lane_id}` - alerta si no hay progreso durante tres slots.
  - `nexus_da_backlog_chunks{lane_id}` - התראה פור אנצימה de umbrales por lane (פורם 64 ציבורי / 8 פרטי).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta cuando el P99 supera 900 ms (ציבורי) או 1200 ms (פרטי).
  - `torii_request_failures_total{scheme="norito_rpc"}` - התראה על טעות של 5 דקות על 2%.
  - `telemetry_redaction_override_total` - Sev 2 inmediato; asegura que las anulaciones tengan tickets de compliance.
- Ejecuta la checklist de remediacion de telemetria en el [plan de remediacion de telemetria de Nexus](./nexus-telemetry-remediation) al menos trimestralmente y adjunta el formulario completado a las notas de revision de operaciones.

## Matriz de incidentes

| Severidad | הגדרה | תשובה |
|--------|------------|--------|
| סב 1 | Brecha de aislamiento de data-space, paro de settlement>15 דקות o corrupcion de voto de gobernanza. | Pagear a Nexus הנדסת שחרור ראשונית + ציות, קבלה קונגררית, חפצים חוזרים, תקשורת פומבית <=60 דקות, RCA <=5 ימים. |
| סב' 2 | הצטברות SLA של צבר ליין, נקודת ציון של טלמטריה מעל 30 דקות, השקה של מניפיסטוס. | Pagear a Nexus Primary + SRE, מתיחה <=4 שעות, מעקבים של רשם ו-2 טיפולים. |
| סוו 3 | Deriva no bloqueante (מסמכים, התראות). | רשם en el tracker y planificar el arreglo dentro del sprint. |

Los tickets de incidentes deben registrar IDs de lane/data-space afectadas, hashes de manifiesto, linea de tiempo, metricas/logs de soporte y tareas/owners de seguimiento.

## Archivo de evidencias

- חבילות Guarda/manifiestos/exportaciones de telemetria bajo `artifacts/nexus/<lane>/<date>/`.
- Conserva configs redactadas + salida de `--trace-config` עבור מהדורת קאדה.
- Adjunta minutas del consejo + decisiones firmadas cuando aterricen cambios de config o manifiesto.
- Conserva צילומי מצב של Prometheus רלוונטיים למטרות של Nexus למשך 12 חודשים.
- Registra ediciones del runbook en `docs/source/project_tracker/nexus_config_deltas/README.md` עבור ביקורי אודיטורים ספאן קואנדו קמביארון לאס אחריות.

## relacionado חומרי

- קורות חיים: [סקירה כללית Nexus](./nexus-overview)
- ספציפיקציה: [מפרט Nexus](./nexus-spec)
- Geometria de lanes: [דגם נתיב Nexus](./nexus-lane-model)
- Transicion y shims de routing: [Nexus הערות מעבר](./nexus-transition-notes)
- העלאת מפעילים: [הכנסת מפעיל Sora Nexus](./nexus-operator-onboarding)
- Remediacion de telemetria: [Nexus תוכנית תיקון טלמטריה](./nexus-telemetry-remediation)