---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-refactor-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-refactor-plan
כותרת: Plan de refactorizacion del Ledger Sora Nexus
תיאור: Espejo de `docs/source/nexus_refactor_plan.md`, que detalla el trabajo de limpieza por phases para el codebase de Iroha 3.
---

:::שימו לב פואנטה קנוניקה
Esta page refleja `docs/source/nexus_refactor_plan.md`. Manten ambas copias alineadas hasta que la edicion multilingue llegue al portal.
:::

# Plan de refactorizacion del ספר החשבונות Sora Nexus

Este documento captura el map inmediato para el refactor del Sora Nexus Ledger ("Iroha 3"). רפלג'ה הפריסה האמיתית של המאגר ו-Regresiones Observadas ב-Contabilidad de Genesis/WSV, consenso Sumeragi, טריגרים של חוזים חכמים, ייעוץ לצילומי מצב, קישורים ל-host pointer-ABI ו-codec Norito. El objetivo es converger en una arquitectura coherente y comprobable sin intentar aterrizar todas las correcciones en un unico parche monolitico.

## 0. Principios guia
- Preservar el comportamiento determinista en heterogeneo חומרה; usar aceleracion solo con תכונה דגלים הצטרפות y fallbacks identicos.
- Norito es la capa de serializacion. Cualquier cambio de estado/esquema debe כולל קידוד/פענוח Norito הלוך ושוב ומכשירים אקטואליים.
- La configuracion fluye por `iroha_config` (משתמש -> בפועל -> ברירות מחדל). Eliminar מחליפה אד-הוק de entorno in paths de produccion.
- La politica ABI sigue en V1 y no es negocable. Los מארח deben rechazar pointer types/syscalls desconocidos de forma determinista.
- `cargo test --workspace` y los golden tests (`ivm`, `norito`, `integration_tests`) סיואן סינדו להמחשב בסיס עבור קאדה היטו.

## 1. תמונת מצב של מאגר הטופולוגיה
- `crates/iroha_core`: שחקנים Sumeragi, WSV, loader de genesis, צינורות (שאילתה, שכבת על, מסלולי zk), דבק מארח של חוזים חכמים.
- `crates/iroha_data_model`: esquema autoritativo para datas y queries on-chain.
- `crates/iroha`: API של לקוחות ארה"ב עבור CLI, בדיקות, SDK.
- `crates/iroha_cli`: מפעילי CLI, ממשקי API מספריים ב-`iroha`.
- `crates/ivm`: VM de bytecode Kotodama, נקודות פתיחה של אינטגרציה מצביע-ABI של המארח.
- `crates/norito`: Codec de serializacion עם התאמה של JSON y backends AoS/NCB.
- `integration_tests`: הצהרות חוצות רכיבים que cubren genesis/bootstrap, Sumeragi, טריגרים, עמודות וכו'.
- Los docs ya delinean metas del Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), אבל אם כן, יש חלוקה ל-Parcialmente al codigeto respecto.

## 2. Pilares de refactor y hitos### שלב א' - Fundaciones y observabilidad
1. **Telemetria WSV + תמונת מצב**
   - Establecer una API canonica de snapshots en `state` (תכונה `WorldStateSnapshot`) usada por queries, Sumeragi y CLI.
   - משתמש `scripts/iroha_state_dump.sh` להפקת צילומי מצב קבועים דרך `iroha state dump --format norito`.
2. **Determinismo de Genesis/Bootstrap**
   - Refactorizar la ingesta de genesis para que pase por un unico pipeline con Norito (`iroha_core::genesis`).
   - Agregar cobertura de integracion/regresion que reprocesa genesis mas el primer bloque y afirma roots WSV identicos entre arm64/x86_64 (seguido en `integration_tests/tests/genesis_replay_determinism.rs`).
3. **בדיקות de fixity cross-arte**
   - הרחב את `integration_tests/tests/genesis_json.rs` ל-WSV, צינור ו-ABI ולרתמה סולו.
   - היכרות עם פיגום `cargo xtask check-shape` que hasa panic ante סחיפה (seguido bajo el backlog de tooling DevEx; ver el action in `scripts/xtask/README.md`).

### שלב B - WSV y superficie de queries
1. **Transacciones de state storage**
   - Colapsar `state/storage_transactions.rs` en un adaptador transaccional que imponga order de commits y deteccion de conflictos.
   - Los unit tests ahora verifican que modificaciones de assets/world/triggers Hagan Rollback ante Fallos.
2. **Refactor del modelo de queries**
   - מעביר את הלוגיקה/הסמן לרכיבים שניתן להשתמש בהם מחדש באחו `crates/iroha_core/src/query/`. ייצוג ליניארי Norito ו-`iroha_data_model`.
   - שאילתות תמונת מצב אגרגר עבור טריגרים, נכסים ותפקידים קובעים (seguido via `crates/iroha_core/tests/snapshot_iterable.rs` para la cobertura actual).
3. **קונסיסטנציה של תמונות מצב**
   - Asegurar que `iroha ledger query` CLI השתמש ב-la misma ruta de snapshot que Sumeragi/מחזירים.
   - Los tests de regresion de snapshots en CLI viven en `tests/cli/state_snapshot.rs` (לנטוס לריצות עם תכונות משוערות).

### Fase C - Pipeline Sumeragi
1. **Topologia y gestion de epocas**
   - Extraer `EpochRosterProvider` תכונה ייחודית עם יישום תוצאות עבור תמונות מצב של הימור ב-WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` קיבלנו בנאי פשוט וניתן להתאמה עבור ספסלים/בדיקות.
2. **הפשטות של הסכמה**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` עם מודולים: `pacemaker`, `aggregation`, `availability`, `witness` עם רכיבי טיפוס בגו `consensus`000.
   - Reemplazar el הודעה העוברת אד-הוק למעטפות Norito tipados e introducir testing property de view-change (seguido en el backlog de mensajeria Sumeragi).
3. **נתיב אינטגרציה/הוכחה**
   - הוכחות לנתיב ליניארי עם התחייבויות של DA y asegurar que RBC porting sea uniforme.
   - בדיקת אינטגרציה מקצה לקצה `integration_tests/tests/extra_functional/seven_peer_consistency.rs` אוורה בדוק את שיטת RBC.### שלב D - חוזים חכמים y hosts pointer-ABI
1. **Auditoria de limite del host**
   - Consolidar los checks de pointer-type (`ivm::pointer_abi`) y los adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - לאס צפויות של טבלת המצביעים y los bindings the host manifest estan cubiertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` y `ivm_host_mapping.rs`, que ejercitan los mappings TLV golden.
2. **ארגז חול של פליטת טריגרים**
   - Refactorizar triggers para ejecutar דרך un `TriggerExecutor` comun que impone גז, validacion de pointers and journaling de eventos.
   - Agregar tests de regresion para triggers de call/time cubriendo paths de fallo (seguido via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alineacion de CLI y client**
   - Asegurar que las operaciones CLI (`audit`, `gov`, `sumeragi`, `ivm`) תלוי בשילובי לקוחות Sumeragi.
   - Los tests de snapshots JSON del CLI viven en `tests/cli/json_snapshot.rs`; mantenlos al dia para que la salida de comandos siga coincidiendo con la referencia JSON canonica.

### שלב E - Endurecimiento del codec Norito
1. **רישום סכימות**
   - צור סכימת רישום Norito bajo `crates/norito/src/schema/` עבור קידודים קנוניים של טיפוס.
   - Se agregaron doc tests que verifican la codificacion de payloads de muestra (`norito::schema::SamplePayload`).
2. **Actualizacion de golden fixtures**
   - Actualizar los golden fixtures de `crates/norito/tests/*` para que coincidan con el nuevo schema WSV cuando aterrice el refactor.
   - `scripts/norito_regen.sh` regenera los golden JSON de Norito de forma determinista via el helper `norito_regen_goldens`.
3. **Integracion IVM/Norito**
   - Validar la serializacion de manifestes Kotodama מקצה לקצה באמצעות Norito, אסיגורנדו que la metadata pointer ABI sea consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene la paridad Norito קידוד/פענוח פר מניפסטים.

## 3. Temas transversales
- **מבחנים אסטרטגיים**: מבחני יחידת קדמה -> מבחני ארגז -> מבחני אינטגרציה. Los tests fallidos capturan regresiones actuales; los nuevos בדיקות evitan que reaparezcan.
- **תיעוד**: שלב אחרון, אקטואליזר `status.md` ופריטים מובילים עברו ל-`roadmap.md`.
- **Benchmarks de rendimiento**: Mantener benches existentes en `iroha_core`, `ivm` y `norito`; agregar medicines base post-refactor para validar que no hay regresiones.
- **דגלים מאפיינים**: Mantener מחליף ארגז ניבל סולו עבור קצה אחורי que requieren toolchains externos (`cuda`, `zk-verify-batch`). נתיבי SIMD de CPU נבנה ובחרו בזמן ריצה; prover fallbacks escalares deterministas para hardware no soportado.## 4. Acciones inmediatas
- פיגומים של Fase A (תכונת תמונת מצב + חיווט של טלמטריה) - מפת הדרכים מוצגת בפועל.
- La auditoria reciente de defectos para `sumeragi`, `state` e `ivm` revelo los suientes pointos:
  - `sumeragi`: קצבאות שידור קוד מת שידור של שינוי צפייה, שידור חוזר של VRF וייצוא של טלמטריה EMA. Estos permanecen gated hasta que la simplificacion del flujo de consenso de la Fase C y los entregables de integracion lane/proof aterricen.
  - `state`: la limpieza de `Cell` y el ruteo de telemetria pasan al track de telemetria WSV de la Fase A, mientras que las notas de SoA/parallel-apply se integran al backlog de optimizacion de pipeline de la Fase C.
  - `ivm`: להחלפת CUDA, להחלפת מעטפות ולקוברטורה של Halo2/Metal. los kernels permanecen en el backlog dedicado de GPU hasta estar listos.
- הכן את תוכנית ה-RFC חוצה צוות לחידוש התוכנית לכניסה ל-Terrizar Cambios de Codigo Invasivos.

## 5. Preguntas abiertas
- Debe RBC seguir siendo optional mas alla de P1, o es obligatorio para lanes del book Nexus? דורש החלטה של ​​בעלי עניין.
- Impulsamos grupos de composabilidad DS en P1 o los mantenemos deshabilitados hasta que maduren las lane proofs?
- Cual es la ubicacion canonica para los parametros ML-DSA-87? מועמד: nuevo crate `crates/fastpq_isi` (creacion pendiente).

---

_עדכון סופי: 2025-09-12_