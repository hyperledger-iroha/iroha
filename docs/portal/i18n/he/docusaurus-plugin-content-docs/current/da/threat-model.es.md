---
lang: he
direction: rtl
source: docs/portal/docs/da/threat-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/da/threat_model.md`. Mantenga ambas versiones en
:::

# Modelo de amenazas de Data Availability de Sora Nexus

_גרסה אולטימטיבית: 2026-01-19 -- תוכנית גרסת פרוקסימה: 2026-04-19_

Cadencia de mantenimiento: קבוצת עבודה זמינות נתונים (<=90 dias). קאדה
revision debe aparecer en `status.md` עם קישורים לכרטיסי הפעלה
y artefactos de simulacion.

## הצעת מחיר

El programa de Data Availability (DA) mantiene transmisiones Taikai, blobs de
ליין Nexus y artefactos de gobernanza recuperables ante fallas bizantinas,
de red y de operadores. Este modelo de amenazas ancla el trabajo de ingenieria
para DA-1 (arquitectura y modelo de amenazas) y sirve como baseline para tareas
DA posteriores (DA-2 a DA-10).

רכיבים דטרו דה אלקנס:
- Extension de ingesta DA en Torii y escritores de metadata Norito.
- Arboles de almacenamiento de blobs respaldados por SoraFS (רמות חם/קר) y
  politicas de replicacion.
- Compromisos de bloques Nexus (פורמטים של חוטים, הוכחות, APIs de cliente ligero).
- Hooks deforcement PDP/PoTR especificos para payloads DA.
- Flujos de operadores (הצמדה, פינוי, חיתוך) y pipelines de
  observabilidad.
- Aprobaciones de gobernanza que admiten o expulsan operadores y contenido DA.

Fuera de alcance למסמך זה:
- Modelado Economico Completo (capturado en el workstream DA-7).
- Protocolos base de SoraFS ya cubiertos por el modelo de amenazas de SoraFS.
- Ergonomia de SDK de clientes mas alla de consideraciones de superficie de
  אמנאזה.

## Panorama arquitectonico

1. **Envio:** Los clientes envian blobs via la API de ingesta DA de Torii. אל
   nodo trocea blobs, codifica manifests Norito (טיpo de blob, ליין, עידן,
   flags de codec), y almacena chunks en el tier hot de SoraFS.
2. **Anuncio:** Intents de pin y hints de replicacion se propagan a proveedores
   de almacenamiento דרך el registry (SoraFS marketplace) con tags de politica
   que indican objetivos de retencion חם/קר.
3. **פשרה:** Los secuenciadores Nexus כוללות פשרות של כתם (CID +
   roots KZG opcionales) en el bloque canonico. Clientes ligeros dependen del
   hash de compromiso y la metadata anunciada para validar זמינות.
4. **שכפול:** Nodos de almacenamiento decargan shares/chunks asignados,
   satisfacen desafios PDP/PoTR y promueven datas entre tiers hot y cold segun
   פוליטיקה.
5. **החלמה:** צורכת שחזור נתונים דרך SoraFS או שערים מודעים ל-DA,
   verificando proofs y levantando solicitudes de reparacion cuando desaparecen
   העתקים.
6. **Gobernanza:** Parlamento y el comite de supervision DA Aprueban operadores,
   לוחות זמנים לביצוע אכיפה. Artefactos de gobernanza se
   almacenan por la misma ruta DA para garantizar transparencia del processo.

## פעילויות האחראיםEscala de impacto: **Critico** rompe seguridad/vivacidad del boek; **אלט**
bloquea backfill DA o clientes; **Moderado** Degrada Calidad Pero es
ניתן להחלים; **באחו** אפקטו מוגבל.

| Activo | תיאור | אינטגרידד | Disponibilidad | סודיות | אחראי |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (נתחים + מניפסטים) | Blobs Taikai, lane y gobernanza almacenados en SoraFS | ביקורת | ביקורת | Moderado | DA WG / צוות אחסון |
| Manifests Norito DA | Metadata tipada que describe blobs | ביקורת | אלטו | Moderado | Core Protocol WG |
| Compromisos de bloque | CIDs + roots KZG dentro de bloques Nexus | ביקורת | אלטו | באחו | Core Protocol WG |
| לוחות זמנים PDP/PoTR | Cadencia de enforcement para replicas DA | אלטו | אלטו | באחו | צוות אחסון |
| Registry de Operadores | Proveedores de almacenamiento aprobados y politicas | אלטו | אלטו | באחו | מועצת ממשל |
| Registros de renta e incentivos | Entradas Ledger para rentas y penalidades DA | אלטו | Moderado | באחו | משרד האוצר |
| לוחות מחוונים דה observabilidad | SLOs DA, profundidad de replicacion, alertas | Moderado | אלטו | באחו | SRE / צפיות |
| Intents de reparacion | שידולים para rehidratar chunks faltantes | Moderado | Moderado | באחו | צוות אחסון |

## יריבים

| שחקן | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Enviar blobs malformados, replays de manifests consoletos, intentar DoS and ingesta. | Interrumpir משדר Taikai, inyectar datos invalidos. | החטא קליב פריבילגיאדות. |
| Nodo de almacenamiento bizantino | העתקים של Soltar, הוכחות PDP/PoTR, הוכחות נוספות. | הפחת את השמירה DA, evitar renta, retener datas como rehens. | Posee credenciales validas de operador. |
| Secuenciador comprometido | הסר פשרות, בלוקים מעורפלים, מטא נתונים מחדש של קוביות. | Ocultar envios DA, crear inconsistencia. | Limitado por la Mayoria de consenso. |
| Operador interno | אבוסר גישה להגנה, מניפולציות פוליטיות של שימור, אישורי סינון. | Ganancia Economica, sabotaje. | גישה לתשתית חמה/קרה. |
| Adversario de red | Nodos פרט, העתק דמורר, טרפיקו MITM. | הקטנת זמינות, SLOs מדרדר. | אין פואד רומפר TLS פרו puede soltar/ralentizar קישורים. |
| Atacante de observabilidad | לוחות מחוונים / התראות מניפולאריים, אירועי סופרמיר. | Ocultar caidas DA. | דרוש גישה לצינור טלמטריה. |

## Fronteras de confianza- **Frontera de ingreso:** Cliente a extension DA de Torii. דרוש אישור
  בקשה, הגבלת קצב yvalidacion de payload.
- **Frontera de replicacion:** Nodos de almacenamiento intercambian chunks y
  הוכחות. Los nodos se autentican mutuamente pero pueden comportarse de forma
  ביזנטינה.
- **Frontera del Ledger:** Datos de bloque comprometidos vs almacenamiento
  מחוץ לשרשרת. כל החסות בהסכמה אינטגרלית, אך נדרשת זמינות
  אכיפה מחוץ לשרשרת.
- **Frontera de gobernanza:** Decisiones de Council/Parliment que Aprueban
  מפעילים, מוקדמים וחתכים. Fallas aqui impactan directamente el
  despliegue DA.
- **Frontera de observabilidad:** Recoleccion de metrics/logs exportada a
  לוחות מחוונים / כלי התראה. La manipulacion oculta breaks o ataques.

## אסמכתאות ושליטה

### Ataques en la ruta de ingesta

**תרחיש:** לקוחות מרושע מקפידים על מטענים Norito פגמים או כתמים
sobredimensionados para agotar recursos או contrabandear metadata invalida.

**בקרות**
- Validacion de schema Norito con negociacion estricta de versiones; rechazar
  דגלים desconocidos.
- מגבלת שיעור y autenticacion en el point end de ingesta Torii.
- מגבלות את גודל הנתח y קידוד קבע פורזאדוס עבור el chunker SoraFS.
- El pipeline de admision solo persiste manifestes tras coincidir el checksum de
  integridad.
- Cache de replay determinista (`ReplayCache`) rastrea ventanas `(נתיב, עידן,
  רצף)`, מתמידים בסימני מים גבוהים בדיסקו, y rechaza duplicados/replays
  מיושנים; רתמות de propiedad y fuzz cubren טביעות אצבע מתפצלות y
  envios fuera de orden. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Brechas residuales**
- Torii לצרוך את המטמון של השידור החוזר והכניסה והסמנים מתמשכים
  de sequence a traves de reinicios.
- Los schemas Norito DA ahora tienen un fuzz harness dedicado
  (`fuzz/da_ingest_schema.rs`) לצורכי קידוד/פענוח קבועים; לוס
  לוחות מחוונים de cobertura deben alertar si el target regresa.

### Retencion por holding de replicacion

**תרחיש:** Operadores de almacenamiento bizantinos aceptan asignaciones de
pin pero sueltan chunks, pasando desafios PDP/PoTR via respuestas forjadas o
קולוזיה.

**בקרות**
- לוח הזמנים של ה-PDP/PoTR יימשך זמן רב לטעינה של DA con cobertura por
  תקופה.
- רפליקיון רב-מקורות con umbrales de quorum; el fetch orchestrator detecta
  רסיסים faltantes y dispara reparacion.
- Slashing de gobernanza vinculado a proofs fallidas y replicas faltantes.
- Job de reconciliacion automatizado (`cargo xtask da-commitment-reconcile`)
  השווה קבלות של אינסטה עם פשרות DA (SignedBlockWire, `.norito` o
  JSON), emit un bundle JSON de evidencia para gobernanza, y falla ante כרטיסים
  faltantes o no coincidentes para que Alertmanager page por mismission/tampering.**Brechas residuales**
- אל רתום de simulacion en `integration_tests/src/da/pdp_potr.rs` (cubierto
  por `integration_tests/tests/da/pdp_potr_simulation.rs`) ahora ejercita
  תרחישים של שיתוף פעולה, תקף לגבי לוח הזמנים של PDP/PoTR detecta
  comportamiento bizantino de forma determinista. Siga extendiendolo junto a
  DA-5 להוכחת שטחים.
- La politica de eviction del tier cold requiere un trail audit firmado para
  prevenir טיפות encubiertos.

### מניפולציה של פשרות

**תסריט:** Secuenciador comprometido publica bloques omitiendo o alterando
compromisos DA, causando fallas de fetch o inconsistencias en clientes ligeros.

**בקרות**
- El consenso cruza propuestas de bloques con colas de envio DA; עמיתים rechazan
  propuestas sin compromisos requeridos.
- Clientes ligeros verifican הכללה הוכחות antes de exponer ידיות אחזור.
- השוואת מסלולי ביקורת קבלות de envio con compromisos de bloque.
- Job de reconciliacion automatizado (`cargo xtask da-commitment-reconcile`)
  השווה קבלות של אינסטה עם פשרות DA (SignedBlockWire, `.norito` o
  JSON), emit un bundle JSON de evidencia para gobernanza, y falla ante כרטיסים
  faltantes o no coincidentes para que Alertmanager page por mismission/tampering.

**Brechas residuales**
- Cubierto por el job de reconciliacion + hook de Alertmanager; los paquetes de
  gobernanza ahora ingieren el bundle JSON de evidencia por defecto.

### Particion de red y censura

**תסריט:** Adversario particiona la red de replicacion, evitando que nodos
קבל נתחים מסויימים או עונה על PDP/PoTR.

**בקרות**
- Requisitos de proveedores רב אזורי garantizan paths de red diversos.
- Ventanas de desafio כולל ריצוד וחזרה ל-canales de reparacion fuera
  דה בנדה.
- לוחות מחוונים de observabilidad monitorean profundidad de replicacion, exito de
  desafios y latencia de fetch con umbrales de alerta.

**Brechas residuales**
- Faltan simulaciones de particion para eventos en vivo de Taikai; se requieren
  בדיקות השרייה.
- La politica de reserva de ancho de banda de reparacion aun no esta codificada.

### Abuso interno

**תרחיש:** אופרטור עם גישה למניפולציה פוליטית של הרישום,
whitelistea proveedores maliciosos o superme alertas.

**בקרות**
- Acciones de gobernanza requieren firmas multi-party y registros Norito
  נוטריזאדו.
- Cambios de politica emiten eventos a monitoreo y logs de archivo.
- El pipeline de observabilidad aplica logs Norito שרשור קו חשיש בלבד.
- La automatizacion de revisiones de acceso trimestral
  (`cargo xtask da-privilege-audit`) שחזר את ספרי המניפסט/השידור החוזר
  (mas paths provistos por operadores), marca entradas faltantes/no directorio/
  ניתן לכתיבה בעולם, אתה מוציא את החבילה של JSON ל-Dashboards de gobernanza.

**Brechas residuales**
- La evidencia de tamper ולוחות מחוונים דורשים תמונות בזק.

## Registro de riesgos residuales| ריסגו | הסתברות | Impacto | אחראי | תוכנית מיתוג |
| --- | --- | --- | --- | --- |
| Replay de manifests DA antes de que aterrice el cache de sequence DA-2 | אפשרי | Moderado | Core Protocol WG | מטמון רצף מיושם + validacion de nonce en DA-2; מבחני רגרסיה אגרגרים. |
| Colusion PDP/PoTR cuando >f nodos se comprometen | בלתי סביר | אלטו | צוות אחסון | סדר לוח זמנים חדש עם ספקי מוסטר; validar באמצעות הרתמה דה סימולציה. |
| Brecha de auditoria en eviction del tier cold | אפשרי | אלטו | SRE / צוות אחסון | Adjuntar logs firmados y receips on-chain para evictions; monitorear באמצעות לוחות מחוונים. |
| Latencia de deteccion de omision de secuenciador | אפשרי | אלטו | Core Protocol WG | `cargo xtask da-commitment-reconcile` השוואת קבלות לעומת פשרות (SignedBlockWire/`.norito`/JSON) וכרטיסי הכרטיסים הקודמים אינם מקריים. |
| Resiliencia a particition para streams en vivo de Taikai | אפשרי | ביקורת | רשתות TL | Ejecutar drills de particition; reservar ancho de banda de reparacion; דוקומנטרי SOP de failover. |
| Deriva de privilegios de gobernanza | בלתי סביר | אלטו | מועצת ממשל | `cargo xtask da-privilege-audit` trimestral (מניפסט/שידור חוזר של מנהלים + נתיבים נוספים) עם JSON firmado + gate de dashboard; anclar artefactos de auditoria on-chain. |

## דרישות מעקב

1. סכימות פומביות Norito de ingesta DA y vectores de emplo (se lleva a DA-2).
2. Enhebrar el replay cache en la ingesta DA de Torii y persistir cursores de
   רצף a traves de reinicios de nodos.
3. **Completado (2026-02-05):** El herness de simulacion PDP/PoTR ahora ejercita
   escenarios de colusion + particion con modelado de backlog QoS; ver
   `integration_tests/src/da/pdp_potr.rs` (con tests en
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para la implementacion y
   los resumenes deterministas capturados abajo.
4. **Completado (2026-05-29):** `cargo xtask da-commitment-reconcile` השוואה
   קבלות de ingesta contra compromisos DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, y esta conectado a
   Alertmanager/Paquetes de gobernanza para alertas de mismission/tabuling
   (`xtask/src/da.rs`).
5. **Completado (2026-05-29):** `cargo xtask da-privilege-audit` recorre el spool
   de manifest/replay (mas paths provistos por operadores), marca entradas
   faltantes/no directorio/world-writable, y produce un bundle JSON firmado para
   לוחות מחוונים/תיקונים דה gobernanza (`artifacts/da/privilege_audit.json`),
   cerrando la brecha de automatizacion de acceso.

**Donde mirar מבזה:**- מטמון השידור החוזר y la persistencia de cursores aterrizaron en DA-2. ור לה
  implementacion en `crates/iroha_core/src/da/replay_cache.rs` (לוגיקה מטמון)
  y la integracion Torii en `crates/iroha_torii/src/da/ingest.rs`, que enhebra las
  comprobaciones de טביעת אצבע a traves de `/v2/da/ingest`.
- סימולציות של סטרימינג של PDP/PoTR הינה הבעלים באמצעות הוכחה-זרם לרתום
  en `crates/sorafs_car/tests/sorafs_cli.rs`, cubriendo flujos de solicitud
  PoR/PDP/PoTR y escenarios de falla animados en el modelo de amenazas.
- Los resultados de capacidad y repair soak viven en
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, mientras que la matriz de
  להשרות Sumeragi mas amplia se rastrea en `docs/source/sumeragi_soak_matrix.md`
  (con variantes localizadas). Estos artefactos capturan los drills de larga
  duracion referenciados en el registro de riesgos residuales.
- La automatizacion de reconciliacion + privilege-audit vive en
  `docs/automation/da/README.md` y los nuevos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; להשתמש
  las salidas por defecto bajo `artifacts/da/` al adjuntar evidencia a paquetes
  דה גוברננסה.

## Evidencia de simulacion y modelado QoS (2026-02)

עבור מעקב אחר DA-1 #3, קודיפיקמוס ורתמה לסימולציה PDP/PoTR

determinista bajo `integration_tests/src/da/pdp_potr.rs` (cubierto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). אל רתמה אsigna nodos

a tres regiones, inyecta particiones/colusion segun las probabilidades del
מפת דרכים, Rastrea Tardanza PoTR, alimenta un modelo de backlog de reparacion
que refleja el presupuesto de reparacion del tier hot. Ejecutar el escenario
produjo por defecto (12 עידנים, 18 עידנים PDP + 2 ventanas PoTR por epoch)
המדדים הבאים:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| מטריקה | חיל | Notas |
| --- | --- | --- |
| Fallas PDP detectadas | 48 / 49 (98.0%) | Las particiones aun disparan deteccion; una sola falla no detectada viene de jitter honesto. |
| Latencia media de deteccion PDP | 0.0 עידנים | Las fallas aparecen dentro del epoch de origen. |
| Fallas PoTR detectadas | 28 / 77 (36.4%) | La deteccion se active cuando un nodo pierde >=2 ventanas PoTR, dejando la Mayoria de eventos en el registro de riesgos residuales. |
| Latencia media de deteccion PoTR | 2.0 עידנים | Coincide con el umbral de tardanza de dos epochs incorporado en la escalacion de archivo. |
| Pico de cola de reparacion | 38 מניפסטים | El backlog se dispara cuando las particiones se apilan mas rapido que las cuatro reparciones disponibles por epoch. |
| Latencia de respuesta p95 | 30,068 אלפיות השנייה | Refleja la ventana de desafio de 30 s con jitter de +/-75 ms aplicado para muestreo QoS. |
<!-- END_DA_SIM_TABLE -->

Estos resultados ahora alimentan los prototipos de dashboards DA y satisfacen
קריטריונים לקבלה של "רתמת הדמיה + דוגמנות QoS".

en el מפת דרכים.

La automatizacion ahora vive detras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que llama al harness compartido y emite Norito JSON a
`artifacts/da/threat_model_report.json` על ידי ערוץ. ג'ובים לילה צורכים
este archivo para refrescar las matrices en este documento y alertar ante deriva

en tasas de deteccion, colas de reparacion o muestras QoS.Para refrescar la tabla de arriba para docs, ejecute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, y rescribe esta seccion
דרך `scripts/docs/render_da_threat_model_tables.py`. El espejo `docs/portal`
(`docs/portal/docs/da/threat-model.md`) se actualiza en el mismo paso para que

ambas copias se mantengan en sync.