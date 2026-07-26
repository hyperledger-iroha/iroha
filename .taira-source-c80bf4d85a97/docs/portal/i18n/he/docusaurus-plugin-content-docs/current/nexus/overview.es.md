---
lang: he
direction: rtl
source: docs/portal/docs/nexus/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-overview
כותרת: Resumen de Sora Nexus
תיאור: Resumen de alto nivel de la arquitectura de Iroha 3 (Sora Nexus) כולל מסמך קנוניקוס מונו-ריפו.
---

Nexus (Iroha 3) אמפליה Iroha 2 עם פליטת רב-נתיב, ספיציוס של נתונים אקוטאדוס פור גוברננזה ורמיינטאס ארגונית SDK. עמוד קורות חיים חדשים `docs/source/nexus_overview.md` חלק מונו-ריפו עבור פורטל הלקטורים של פורטל אנטיאנדאן מהיר כמו ארקג'אן לאס פייזס דה לה ארכיטקטורה.

## Lineas de Lanzamiento

- **Iroha 2** - despliegues autoalojados para consorcios o redes privadas.
- **Iroha 3 / Sora Nexus** - la red publica multi-lane donde los operadores registran espacios de datas (DS) e heredan herramientas compartidas de gobernanza, liquidacion y observabilidad.
- Ambas lineas compilan desde el mismo workspace (IVM + Toolchain de Kotodama), por lo que las correcciones de SDK, las actualizaciones de ABI y los fixtures Norito סיונדו נייד. Los operadores decargan el paquete `iroha3-<version>-<os>.tar.zst` para unirse a Nexus; consulta `docs/source/sora_nexus_operator_onboarding.md` לרשימה של אימות ב-panalla completa.

## Bloques de construccion| רכיב | קורות חיים | Portal Ganchos del |
|--------|--------|----------------|
| Espacio de datas (DS) | Dominio de ejecucion/almacenamiento definido por gobernanza que posee una o mas lanes, declara conjuntos de validadores, clase de privacidad y politica de tarifas + DA. | Consulta [Nexus מפרט](./nexus-spec) עבור אל esquema del manifiesto. |
| ליין | Fragmento determinista de ejecucion; emite compromisos que ordena el anillo NPoS העולמי. Las classes de lane כולל `default_public`, `public_custom`, `private_permissioned` ו `hybrid_confidential`. | אל [דגם ליין](./nexus-lane-model) תפיסת גיאומטריה, התקדמות תקינות ושמירה. |
| תכנית המעבר | מציין מיקום, שלבי התקדמות ותפעול כפולים בסימן como los despliegues de un solo lane evolucionan hacia Nexus. | Las [notas de transicion](./nexus-transition-notes) תיעוד של שלב ההגירה. |
| Directorio de espacios | Contrato de registro que almacena manifiestos + versiones de DS. Los operadores Concilian las entradas del catalogo contra este directorio antes de unirse. | El rastreador de diffs de manifiestos vive en `docs/source/project_tracker/nexus_config_deltas/`. |
| קטלוג הנתיבים | Seccion `[nexus]` de configuracion que asigna IDs de lane a alias, politicas de enrutamiento y umbrales DA. `irohad --sora --config ... --trace-config` אימפריה אל קטלוג רזולוציה עבור אודיטוריאס. | Usa `docs/source/sora_nexus_operator_onboarding.md` עבור el recorrido de CLI. |
| נתב ליקוי | Orquestador de transferencias XOR que conecta lanes CBDC privadas con lanes de liquidez publicas. | `docs/source/cbdc_lane_playbook.md` פרטים על ידיות פוליטיקה ומחשבי טלמטריה. |
| Telemetria/SLOs | פאנלים + התראות באחו `dashboards/grafana/nexus_*.json` תפסו מסלולים גבוהים, פיגור DA, שחרור ליקוי ועומק של קולה דה גוברננסה. | El [plan de remediacion de telemetria](./nexus-telemetry-remediation) detalla los paneles, alertas y evidencia de auditoria. |

## Instantanea de despliegue

| פאזה | Enfoque | קריטריונים דה סלידה |
|-------|-------|-------------|
| N0 - Beta cerrada | רשם המנהלים של המרכז (`.sora`), שילוב מדריך מפעילים, קטלוג מסלולים אסטטיים. | Manifiestos DS firmados + traspasos de gobernanza ensayados. |
| N1 - Lanzamiento publico | Anade sufijos `.nexus`, subastas, רשם אוטוservicio, cableado de liquidacion XOR. | פרועבאס דה סינקרוניזציה של פתרונות/שערים, פאנלים של פיוס דה פקטורי, סימולציות של מחלוקת. |
| N2 - הרחבה | הצג את `.dao`, APIs de reventa, analitica, portal de disputas, כרטיסי ניקוד של דיילים. | Artefactos de cumplimiento versionados, ערכת כלים של jurado de politicas en linea, מודיע ל-transparencia del tesoro. |
| Puerta NX-12/13/14 | El motor de cumplimiento, פאנלים של טלמטריה ותיעוד deben salir juntos antes de pilotos con socios. | [Nexus סקירה כללית](./nexus-overview) + [Nexus פעולות](./nexus-operations) פרסומים, פאנלים מחוברים, מנוע פוליטיקה. |## אחריות מפעיל

1. **Higiene de configuracion** - manten `config/config.toml` sincronizado con el catalogo publicado de lanes y dataspaces; archiva la salida de `--trace-config` con cada ticket de release.
2. **Seguimiento de manifiestos** - Concilia las entradas del catalogo con el paquete mas reciente del Space Directory antes de unirte o actualizar nodos.
3. **Cobertura de telemetria** - expon los paneles `nexus_lanes.json`, `nexus_settlement.json` y los dashboards relacionados del SDK; conecta alertas a PagerDuty y ejecuta revisies trimestrales segun el plan de remediacion de telemetria.
4. **Reporte de incidentes** - segue la matriz de severidad en [Nexus פעולות](./nexus-operations) y presenta RCAs dentro de cinco dias habiles.
5. **Preparacion de gobernanza** - asiste a las votaciones del consejo Nexus que impactan tus lanes y ensaya instrucciones de rollback trimestralmente (seguido en `docs/source/project_tracker/nexus_config_deltas/`).

## Ver tambien

- קורות חיים קנוניקו: `docs/source/nexus_overview.md`
- פרטים ספציפיים: [./nexus-spec](./nexus-spec)
- Geometria de lanes: [./nexus-lane-model](./nexus-lane-model)
- תוכנית המעבר: [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediacion de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- ספר הפעלה: [./nexus-operations](./nexus-operations)
- Guia de onboarding de operators: `docs/source/sora_nexus_operator_onboarding.md`