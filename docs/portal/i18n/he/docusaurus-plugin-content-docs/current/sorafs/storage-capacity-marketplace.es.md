---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/storage-capacity-marketplace.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: storage-capacity-marketplace
כותרת: Mercado de capacidad de almacenamiento de SoraFS
sidebar_label: Mercado de capacidad
תיאור: תוכנית SF-2c ל-Mercado de Capacidad, Ordnes de Replicacion, Telemetria y Hooks de gobernanza.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/storage_capacity_marketplace.md`. Mantén ambos lugares alineados mientras la documentación heredada siga active.
:::

# Mercado de capacidad de almacenamiento de SoraFS (Borrador SF-2c)

הפריט של מפת הדרכים SF-2c מציגה את השוק של הספקים
almacenamiento declaran capacidad comprometida, reciben ordenes de replicacion y
ganan fees proporcionales a la disponibilidad entregada. Este documento delimita
los entregables requeridos para la primera release y los divide en tracks accionables.

## אובייקטיביות

- הסבר פשרה של נפח (סכמי בתים, מגבלות מסלול, תפוגה)
  en una forma חומר מתכלה ניתן לאימות por gobernanza, transporte SoraNet y Torii.
- Asignar pins entre providers segun capacidad declarada, stake y restricciones de
  politica mientras se mantiene comportamiento determinista.
- אמצעי אחסון (יציאה לשכפול, זמן פעולה, הוכחות אינטגרליות) y
  טלמטריה יצואנית להפצת עמלות.
- Proeer processos de revocacion y disputa para que providers deshonestos sean
  penalisados או removidos.

## מושגי שליטה

| קונספטו | תיאור | Inicial נמרץ |
|--------|-------------|------------------------|
| `CapacityDeclarationV1` | מטען Norito הוא מתאר את מזהה הספק, תעודת פרופיל של chunker, פתרונות GiB, מגבלות ליין, מחירים מסלולים, פשרה על הימור ותפוגה. | Esquema + validador en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | הוראה פורסמה פור gobernanza que asigna un CID de manifest a uno o mas ספקים, כולל nivel de redundancia y metricas SLA. | Esquema Norito compartido con Torii + API של חוזה חכם. |
| `CapacityLedger` | רישום על שרשרת/מחוץ לשרשרת que rastrea declaraciones de capacidad actives, orderes de replicacion, מדדי רנדימיינטו ואגרות צבירת. | מודול חוזה חכם או דף שירות מחוץ לשרשרת עם קביעת תמונת מצב. |
| `MarketplacePolicy` | Politica de gobernanza que להגדיר את הימור מינימו, דרישות אודיטוריה y curvas de penalizacion. | Struct de config en `sorafs_manifest` + documento de gobernanza. |

### Esquemas implementados (estado)

## Desglose de trabajo

### 1. רישיון רישום| טארא | אחראי(ים) | Notas |
|------|----------------|------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Storage / Gobernanza | Usar Norito; כולל גרסאות סמנטיות ורפרנציות ליכולות. |
| מודולים מיושמים של מנתח + validador en `sorafs_manifest`. | Equipo de Storage | תעודות זהות מסמלות, מגבלות כושר, דרישות סיכון. |
| מטא נתונים מרחיב של chunker registry עם `min_capacity_gib` לפרופיל. | Tooling WG | איודה ולקוחות מצריכים מינימלי חומרה לפרופיל. |
| עורך את המסמך `MarketplacePolicy` עם מעקות בטיחות לכניסה ולוח שנה לעונשים. | מועצת ממשל | Publicar en docs junto con defaults de politica. |

#### Definiciones de esquema (implementadas)- `CapacityDeclarationV1` תופסים פשרה על ידי ספק, כולל ידיות קנוניות של chunker, נקודות עזר, כוונות אופציונליות למסלול, תמחור פיסטות, תקינות ומטא נתונים. La validacion asegura stake no cero, מטפל ב-canonicos, כינויים deduplicados, caps por lane dentro del total declarado y contabilidad de GiB monotónica.【ארגזים/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` enlaza manifests con asignaciones emitidas por gobernanza que incluyen objetivos de redundancia, umbrales de SLA y garantias por asignacion; los validadores imponen מטפל ב-canonicos, ספקים unicos y restricciones de deadline antes de que Torii o el registry consuman la orden.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` אקספרס צילומי מצב של תקופה (GiB declarados vs usados, contadores de replicacion, porcentajes de uptime/PoR) que alimentan la distribucion de fees. Las validaciones mantienen el uso dentro de la declaracion y los porcentajes dentro de 0-100%.【ארגזים/sorafs_manifest/src/capacity.rs:476】
- חברות עוזרות (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de lane/asignacion/SLA) הוכחו אימות תקינות המפתחות ודיווחי השגיאה לגבי CI y tooling downstream pueden reutilizar.【ארגזים/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` אזהרה להמחיש את תמונת המצב על השרשרת דרך `/v2/sorafs/capacity/state`, שילוב של הצהרות ספק וכניסות עמלות עם Norito JSON determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La cobertura de validacion ejercita enforcement de handles canonicos, deteccion de duplicados, limites por lane, guardas de asignacion de replicacion y checks de rango de telemetria para que las regresiones aparezcan en CI de inmediato.【crates/sorafs_manifest.【rates/sorafs_manifest:9
- כלי עבודה עבור מפעילים: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convierte especificaciones readibles en payloads Norito canonicos, blobs base64 y resúmenes JSON para que los operadores readyn fixtures de `/v2/sorafs/capacity/declare` ordenes de `/v2/sorafs/capacity/declare`, replicacion con validacion local.【ארגזים/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Los fixtures de referencia viven en `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, I100NI040 via generan) `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. אינטגרציה של שליטה בשטח| טארא | אחראי(ים) | Notas |
|------|----------------|------|
| מטפלי אגרגר Torii `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` עם מטענים Norito JSON. | צוות Torii | Reflejar la logica del validador; reutilizar helpers Norito JSON. |
| תמונת מצב של `CapacityDeclarationV1` מטא נתונים של לוח התוצאות של אורקסטדור ומטוסים להבאת שער. | Tooling WG / Equipo de Orchestrator | Extender `provider_metadata` עם רפרנסים של נפח עבור ניקוד ריבוי מקורות הפוגה גבולות לנתיב. |
| Inyectar ordenes de replicacion en clientes de orquestador/gateway para guiar asignaciones y hints de failover. | Networking TL / Gateway צוות | El Builder del לוח תוצאות לצרוך סדרes firmadas por gobernanza. |
| כלי עבודה CLI: מאריך `sorafs_cli` עם `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | הוכח את ההגדרה של JSON + לוח התוצאות. |

### 3. Politica del marketplace y gobernanza

| טארא | אחראי(ים) | Notas |
|------|----------------|------|
| אישור `MarketplacePolicy` (מינימום הימור, ריבוי עונשין, קדנציה של אודיטוריה). | מועצת ממשל | Publicar en docs, capturar historical de revisiones. |
| Agregar hooks de gobernanza para que el Parlamento pueda aprobar, renovar y revocar declaraciones. | מועצת ממשל / צוות חוזה חכם | Usar eventos Norito + ingesta de manifests. |
| יישום חוק העונשין (הפחתת עמלות, חיתוך אג"ח) ליגדו ל-Violaciones de SLA telegrafiadas. | מועצת ממשל / האוצר | Alinear con outputs de settlement de `DealEngine`. |
| דוקומנטרי אל תהליך דה דיספוטה y la matriz de escalamiento. | מסמכים / ממשל | Vincular al runbook de disputa + helpers de CLI. |

### 4. Medicin y distributioncion de fees

| טארא | אחראי(ים) | Notas |
|------|----------------|------|
| הרחב את המדידה ב-Torii עבור ה-`CapacityTelemetryV1`. | צוות Torii | Validar GiB-hour, exito PoR, uptime. |
| אקטואליזר את צינור המדידה של `sorafs_node` עבור שימוש דיווח עבור סדר + estadisticas SLA. | צוות אחסון | Alinear con ordenes de replicacion y handles de chunker. |
| צינור ההתיישבות: המרת טלמטריה + נתונים העתקים ב-Pagos denominados en XOR, מפיק רשימות רשומות עבור גוברננזה ורשם חשבונות חשבונות. | צוות אוצר / אחסנה | Conectar con exportaciones de Deal Engine / Treasury. |
| ייצוא לוחות מחוונים/אזהרות למתן מדידה (פיגור של אינסטה, טלמטריה מיושן). | צפייה | Extender el pack de Grafana התייחסות ל-SF-6/SF-7. |- Torii ahora expone `/v2/sorafs/capacity/telemetry` y `/v2/sorafs/capacity/state` (JSON + Norito) para que operadores envien snapshots de telemetria por epoca y los inspectores can recuperen o audit el ledger recuperen evidencia.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- La integracion `PinProviderRegistry` asegura que las ordenes de replicacion sean accesibles por el mismo endpoint; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) ahora validan y publican telemetria desde ejecuciones automatizadas con hashing determinista y resolucion de aliases.
- תמונת מצב של מדידה מיוצרים entradas `CapacityTelemetrySnapshot` fijadas al תמונת מצב `metering`, y los exports Prometheus alimentan el tablero Grafana listo para importar en Norito para importar en Norito facturacion monitoreen la acumulacion de GiB-hour, fees nano-SORA proyectados y cumplimiento de SLA en tiempo real.【crates/iroha_torii/src/routing.rs:5143】【grafing_sor/source.
- Cuando el smoothing de metering esta habilitado, el snapshot incluye `smoothed_gib_hours` y `smoothed_por_success_bps` para que operadores comparen valores con EMA frente a contadores crudos que la gobernanza usa para pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. Manejo de disputas y revocaciones

| טארא | אחראי(ים) | Notas |
|------|----------------|------|
| הגדר את המטען `CapacityDisputeV1` (ביקוש, הוכחה, מטרת ספק). | מועצת ממשל | Esquema Norito + validador. |
| Soporte de CLI para iniciar disputas y responder (con adjuntos de evidencia). | Tooling WG | Asegurar hashing determinista del bundle de evidencia. |
| Agregar בודק אוטומטית ל-SLA repetidas (auto-escalado a disputa). | צפייה | Umbrales de alerta y hooks de gobernanza. |
| דוקומנטרי על פנקס המשחקים של ריבוקציון (פריודו דה gracia, evacuacion de datas pineados). | צוות מסמכים / אחסון | Vincular a documento de politica y runbook de operadores. |

## דרישות בדיקות CI

- בדיקות יחידות פאר טוdos los validadores de esquema nuevos (`sorafs_manifest`).
- Tests de integracion que simulan: declaracion -> orden de replicacion -> מדידה -> תשלום.
- זרימת עבודה של CI עבור הצהרות מחודשות/טלמטריה של נפחים וחיבורים למערכת ההפעלה (Extender `ci/check_sorafs_fixtures.sh`).
- בדיקות אחסון עבור API של הרישום (10,000 ספקים דומים, 100,000 סדרות).

## לוחות מחוונים של Telemetria y

- לוחות מחוונים:
  - Capacidad declarada לעומת ניצול עבור ספק.
  - Backlog de ordenes de replicacion y demora promedio de asignacion.
  - Cumplimiento de SLA (זמן פעילות, % זמן יציאה, PoR).
  - עמלות צבירת עונשין של תקופה.
- התראות:
  - ספק פור debajo de la capacidad minima comprometida.
  - Orden de replicacion atascada > SLA.
  - Fallas en el pipeline de meterering.

## מעוררי תיעוד- Guia de Operator להכרזה על כושר, שיפוץ פשרה ושימוש ב-monitorear.
- Guia de gobernanza para aprobar declaraciones, emitir ordenes y manejar disputas.
- Referencia de API עבור נקודות קצה של קיבולת ופורמט סדר דה רפליקיון.
- שאלות נפוצות על השוק עבור מפתחים.

## רשימת רשימת מוכנות עבור GA

El item de roadmap **SF-2c** bloquea el rollout in produccion sobre evidencia concreta
עזרת גישה, ניהול מחלוקת וכניסה למטוס. Usa los artefactos suientes
לקריטריונים של אישור וסינכרון עם יישום.

### קבל לילה ופיוס XOR
- ייצוא תמונת מצב של נכסים ויצוא של ספר חשבונות XOR para la misma
  ventana, luego ejecuta:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  El helper sale con codigo no cero si hay settlements o penalizaciones faltantes/excesivas y
  emit un resumen de Prometheus בפורמט קובץ טקסט.
- La alerta `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  הפערים בדיווח על פיוס המטרידים; los dashboards viven en
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiva el resumen JSON y los hashes en `docs/examples/sorafs_capacity_marketplace_validation/`
  junto con los paquetes de gobernanza.

### Evidencia de disputa y slashing
- Presenta disputas באמצעות `sorafs_manifest_stub capacity dispute` (בדיקות:
  `cargo test -p sorafs_car --test capacity_cli`) para mantener מטענים canonicos.
- Ejecuta `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` y las suites de
  penalizacion (`record_capacity_telemetry_penalises_persistent_under_delivery`) para probar que
  disputas y slashes se reproduceren de manera determinista.
- Sigue `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidencia y escalamiento;
  enlaza las aprobaciones de strikes en el reporte de validacion.

### כניסה של ספקים ויציאה מבחני עשן
- Regenera artefactos declaracion/telemetria con `sorafs_manifest_stub capacity ...` y
  reejecuta los tests de CLI antes del submit (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Envialos via Torii (`/v2/sorafs/capacity/declare`) y luego captura `/v2/sorafs/capacity/state` mas
  צילומי מסך של Grafana. Sigue el flujo de salida en `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiva artefactos firmados y outputs de reconciliacion dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencias y secuenciacion

1. Terminar SF-2b (פוליטיקה של קבלה) - השוק תלוי בתוקף ספקים.
2. esquema Implementar + capa de registry (este doc) antes de la integracion de Torii.
3. השלם את צינור המדידה לפני התשלומים.
4. Paso final: habilitar distribucion de fees controlada por gobernanza una vez que los datas
   de metering se verifiquen en staging.

El progreso debe rastrearse en el מפת הדרכים עם רפרנסים למסמך זה. אקטואליזה אל
מפת דרכים una vez que cada seccion ראש העיר (esquema, plano de control, שילוב, מדידה,
manejo de disputas) תכונת alcance estado הושלמה.