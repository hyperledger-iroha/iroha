---
lang: he
direction: rtl
source: docs/portal/docs/nexus/lane-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-lane-model
כותרת: Modelo de lanes de Nexus
תיאור: טקסונומיה לוגיקה דה ליין, גיאומטריה ותצורה ורגלס דה מיזוג עולמי עבור Sora Nexus.
---

# Modelo de lanes de Nexus y particionado de WSV

> **Estado:** NX-1 מרתקת - טקסונומיה של נתיבים, גיאומטריה של תצורה ופריסה של רשימות אחסון ליישום.  
> **בעלים:** Nexus Core WG, Governance WG  
> **Referencia de roadmap:** NX-1 en `roadmap.md`

Esta page del Portal refleja el brief canonico `docs/source/nexus_lanes.md` para que operadores de Sora Nexus, בעלים של SDK y revisores puedan leer la guia de lanes sin intrar en el arbol mono-repo. La arquitectura objetivo mantiene el state world determinista mientras permite que que space data (lanes) individuales ejecuten conjuntos de validadores publicos o privados con looms work aislados.

## מושגים

- **Lane:** לוגיקה של רסיס של Nexus עם ערכת אישורים וגיבוי של פירוק. זיהוי על ידי `LaneId` יציב.
- **מרחב נתונים:** דלי הממשל של מסלולים שונים המתייחסים למדיניות הציות, ניתוב ההתנחלות.
- **מניפסט הנתיב:** מטא-נתונים בקרת ממשל המתארים אישורים, פוליטיקה של DA, אסימון גז, התיישבות ואישורים לניתוב.
- **מחויבות גלובלית:** הוכחה ל-una lane que resume nuevos roots de estado, datas de settlement and transferencias cross-lane optiones. El anillo NPoS התחייבויות סדרנה עולמיות.

## טקסונומיה דה ליין

Los tipos de lane described de forma canonica su visibilidad, superficie de governance y hooks de settlement. La geometria de configuracion (`LaneConfig`) תופסים את המאפיינים עבור que nodos, SDKs y tooling puedan razonar sobre el layout and logica bespoke.

| טיפו דה ליין | Visibilidad | Membresia de validadores | חשיפה WSV | Gobernanza por defecto | פוליטיקה דה התנחלות | Uso tipico |
|----------------------------------------------------|
| `default_public` | ציבורי | ללא רשות (הימור גלובלי) | Replica de estado completa | SORA הפרלמנט | `xor_global` | בסיס לדג'ר פובליקו |
| `public_custom` | ציבורי | חסרי רשות או מגודרים | Replica de estado completa | Modulo ponderado por stake | `xor_lane_weighted` | תפוקת אפליקציונים דה אלטו |
| `private_permissioned` | מוגבל | Set fijo de validadores (התלהבות של ממשל) | התחייבויות והוכחות | מועצה פדרציה | `xor_hosted_custody` | CBDC, עומסי עבודה de consorcio |
| `hybrid_confidential` | מוגבל | ממברזיה מיקסטה; לקפוף הוכחות ZK | התחייבויות + גילוי סלקטיבי | Modulo de Dinero ניתן לתכנות | `xor_dual_fund` | Dinero ניתן לתכנות עם שימור פרטיות |

טוdos los tipos de lane deben מצהיר:- כינוי של מרחב נתונים - אגרופאציון קריא ל-Humanos que vincula politicas de compliance.
- Handle de governance - זיהוי resuelto דרך `Nexus.governance.modules`.
- Handle de Settlement - Identicador Consumido por el Settlement Router para debitar buffers XOR.
- מטא-נתונים אופציונליים של טלמטריה (תיאור, יצירת קשר, דומיניום ניהולי) באמצעות `/status` y לוחות מחוונים.

## Geometria de configuracion de lanes (`LaneConfig`)

`LaneConfig` הוא התוצאה של זמן ריצה גיאומטריה של קטלוג הנתיבים. אין reemplaza los manifests de governance; en su lugar provee identificadores deterministas אחסון y pistas de telemetria para cada lane configurada.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` recalcula la geometria cuando se carga la configuracion (`State::set_nexus`).
- Los aliases se sanitizan a slugs en minusculas; caracteres no alfanumericos consecutivos se colapsan en `_`. Si el alias produce un slug vacio, usamos `lane{id}`.
- `shard_id` נגזרת מפתח מטא-נתונים `da_shard_id` (באמצעות `lane_id`) ותעביר את יומן הסמן של הרסיס לשמירה על השידור החוזר של ה-DA לקבוע אתחול/החלפה מחדש.
- Los prefijos de clave aseguran que el WSV mantenga rangos de claves por lane disjuntos aun cuando se comparte el mismo backend.
- Los nombres de segmentos Kura son deterministas entre hosts; אודיטורים בודקים את הוראות הסגמנטים וגילויי החטאים בהתאמה אישית.
- Los segmentos merge (`lane_{id:03}_merge`) guardan las ultimas roots de merge-hint y commitments de estado global para esa lane.

## מפלגת המדינה העולמית- El logico state world de Nexus es la union de espacios de estado por lane. Las lanes publicas persisten estado completo; לאס מסלולים פרטיות/שורשי ייצוא סודיים Merkle/מחויבות לפנקס המיזוג.
- El almacenamiento MV prefija cada clave con el prefijo de 4 bytes de `LaneConfigEntry::key_prefix`, generando claves como `[00 00 00 01] ++ PackedKey`.
- Las tablas compartidas (חשבונות, נכסים, טריגרים, registros de governance) almacenan entradas agrupadas por prefijo de lane, manteniendo los range scans deterministas.
- La metadata del Merge-ledger refleja el mismo layout: cada lane escribe roots de merge-hint y roots de estado global reducido en `lane_{id:03}_merge`, הותרת שמירה או פינוי הדרכה cuando una lane se retira.
- מדדים חוצי נתיב (כינויים של חשבונות, רישומי נכסים, מניפסטים של ממשל) almacenan prefijos de lane explicitos para que los operadores reconcilien entradas rapidamente.
- **Politica de retencion** - lanes publicas retienen cuerpos de bloque completos; לאס ליין סולו דה התחייבויות pueden compactar cuerpos antiguos despues de checkpoints porque los התחייבויות בן autoritativos. Las lanes guardan journals cifrados en segmentos dedicados para no bloquear otros עומסי עבודה.
- **כלי עבודה** - שימושי עזר (`kagami`, מנהלי מערכת של CLI) כתובות את מרחב השמות עם מדד ה-exponer slug, stag Prometheus או כל חלקי הארכיון.

## ניתוב y APIs

- נקודות קצה Torii REST/gRPC מקובלות ללא `lane_id` אופציונליים; la ausencia implica `lane_default`.
- Los SDKs exponen selectores de lane y mapean aliases amigables a `LaneId` usando el catalogo de lanes.
- לאס reglas de routing אופרה sobre el catalogo validado y pueden elegir lane y dataspace. `LaneConfig` הוכח כינויים מתאימים עבור טלמטריה ולוחות מחוונים ויומנים.

## עמלות הסדר y

- Cada lane paga עמלות XOR al set global de validadores. Las lanes pueden cobrar tokens de gas nativos pero deben hacer escrow de equivalentes XOR junto con התחייבויות.
- לאס הוכחות דה התנחלות כולל מונטו, מטא נתונים של המרה y prueba de escrow (לפי דוגמה, העברה אל כספת גלובל דה fees).
- El settlement router unificado (NX-3) debita buffers usando los mismos prefijos de lane, asi la telemetria de settlement se alinea con la geometria de storage.

## ממשל

- Las lanes declaran su modulo de governance באמצעות el catalogo. `LaneConfigEntry` lleva el alias y slug originales para mantener קריא la telemetria y los trails audit.
- רישום ה-Nexus הפצה של נתיב המניפסטים כולל את `LaneId`, מחייב את מרחב הנתונים, טיפול בממשל, טיפול בהתנחלות ומטא נתונים.
- שדרוג זמן ריצה לאחר יישום פוליטיקה של ממשל (`gov_upgrade_id` פור defecto) ורישום הבדלים דרך גשר הטלמטריה (אירועים `nexus.config.diff`).

## סטטוס טלמטריה- `/status` כינויי חשיפה של ליין, חיבורי מרחב נתונים, טיפול בממשל ופרפילי יישוב, תוצאות קטלוג ו-`LaneConfig`.
- Las metricas del scheduler (`nexus_scheduler_lane_teu_*`) כינויים/שבלולים של מוסטרן עבור צבר פעולות מאפין y presion de TEU rapidamente.
- `nexus_lane_configured_total` מספר נקודות לחיצה על נתיב וחישוב מחדש. La telemetria emite diffs firmados cuando cambia la geometria de lanes.
- מדדי הצטברות של מרחב הנתונים כוללים מטא-נתונים כינוי/תיאור עבור איודר או מפעילי מערכת א-חברתית של קולה עם דומיניום.

## תצורה וטיפים Norito

- `LaneCatalog`, `LaneConfig`, y `DataSpaceCatalog` viven en `iroha_data_model::nexus` y estructuras en formato Norito para manifests y SDKs.
- `LaneConfig` vive en `iroha_config::parameters::actual::Nexus` y se deriva automaticamente del catalogo; אין צורך בקידוד Norito הוא זמן ריצה פנימי עוזר.
- La configuracion de cara al usuario (`iroha_config::parameters::user::Nexus`) לאחר תיאורי הצהרות של ליין y dataspace; el parseo ahora deriva la geometria y rechaza aliases invalidos o IDs de lane duplicados.

## Trabajo pendiente

- עדכונים משולבים של נתב התיישבות (NX-3) עם גיאומטריה נובעת לריבוי נקודות חישוב XOR להגדיר את הגינונים של slug de lane.
- Extender Tooling Admin עבור משפחות עמודות רשימות, רצועות קומפקטיות ובדיקת יומני בלוקים לפי נתיב Usando El namespace with Slug.
- סיים את המיזוג (הזמנה, גיזום, זיהוי קונפליקטים) ורכיבי רגרסיה משלימים עבור שידור חוזר חוצה נתיב.
- Agregar hooks de compliance עבור רשימות הלבנות/רשימות שחורות ופוליטיקה ניתנת לתכנות (seguido bajo NX-12).

---

*עקוב אחרי NX-1 עם NX-2 עם NX-18. Por favor exponga preguntas abiertas en `roadmap.md` o en el tracker de governance para que el portal siga alineado con los docs canonicos.*