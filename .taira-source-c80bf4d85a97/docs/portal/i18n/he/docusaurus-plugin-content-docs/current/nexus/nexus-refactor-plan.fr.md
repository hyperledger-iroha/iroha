---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-refactor-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-refactor-plan
כותרת: Plan de refactorisation du Ledger Sora Nexus
תיאור: Miroir de `docs/source/nexus_refactor_plan.md`, detaillant le travail de nettoyage par phases pour la base de code Iroha 3.
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/nexus_refactor_plan.md`. Gardez les deux copies alignees jusqu'a ce que l'edition multilingue arrive dans le portal.
:::

# Plan de refactorisation du ledger Sora Nexus

Ce לכידת מסמכים למפת הדרכים מיידית du refactor du Sora Nexus Ledger ("Iroha 3"). הטופולוגיה האקטואלית והרגרסיונית נצפית ב-WSV התאימות, הקונצנזוס Sumeragi, הטריגרים של חוזים חכמים, בקשת התמונות, חיבורי ה-host pointer-ABI ו-Codecs Norito. L'objectif est de converger vers une architecture coherente et testable sans tenter de livrer toutes les corrections dans un patch monolithique.

## 0. מנהלי Principes
- Preserver un comportement deterministe sur du materiel heterogene; שימוש בייחודיות של תאוצה באמצעות דגלים של מאפיינים הצטרפות עם זיהויים נמוכים.
- Norito est la couche de serialisation. כל שינוי בסכימה כולל את הבדיקות הלוך ושוב Norito קידוד/פענוח ותקשורת.
- La configuration transite par `iroha_config` (משתמש -> בפועל -> ברירות מחדל). Supprimer les switches d'environnement ad-hoc des paths de production.
- La politique ABI reste V1 et non negocable. המארחים אינם שוללים את ההגדרה לסוגי המצביעים/מערכות ההפעלה אינם מודעים.
- `cargo test --workspace` et les golden tests (`ivm`, `norito`, `integration_tests`) restent le gate de base pour chaque jalon.

## 1. תמונת מצב של טופולוגיה דו דיפו
- `crates/iroha_core`: שחקנים Sumeragi, WSV, יצירת מטעין, צינורות (שאילתה, שכבת על, נתיבי zk), מארח דבק של חוזים חכמים.
- `crates/iroha_data_model`: schema autoritatif pour les donnees et requetes on-chain.
- `crates/iroha`: משתמש לקוח API לפי CLI, בדיקות, SDK.
- `crates/iroha_cli`: מפעיל CLI, ממשק ממשק ממשק API של `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, points d'entree d'integration pointer-ABI du host.
- `crates/norito`: Codec של סדרה עם מתאמי JSON ו-backends AoS/NCB.
- `integration_tests`: קביעות חוצה רכיבים קוברנט בראשית/אתחול, Sumeragi, טריגרים, עימוד וכו'.
- Les docs decrivent deja les objectifs du Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`).

## 2. Piliers de refactor et jalons### שלב A - Fondations et observabilite
1. **Telemetrie WSV + Snapshots**
   - Etablir une API canonique de snapshots dans `state` (תכונה `WorldStateSnapshot`) משתמש בשאילתות נקובות, Sumeragi ו-CLI.
   - משתמש `scripts/iroha_state_dump.sh` pour produire des snapshots deterministes דרך `iroha state dump --format norito`.
2. **Determinisme Genesis/Bootstrap**
   - Refactoriser l'ingestion genesis pour passer par un pipeline בסיס ייחודי sur Norito (`iroha_core::genesis`).
   - Ajouter une couverture d'integration/regression qui rejoue genesis plus le premier bloc et verifié des racines WSV identiques entre arm64/x86_64 (suivi dans `integration_tests/tests/genesis_replay_determinism.rs`).
3. **בדיקות de fixity cross-arte**
   - Etendre `integration_tests/tests/genesis_json.rs` pour valider les invariants WSV, pipeline and ABI dans un seul harness.
   - היכרות עם פיגום `cargo xtask check-shape` כדי להיווצר פאניקה על סחף סכימת (שמפרט את הצטברות DevEx tooling; עבור פריט הפעולה ב-`scripts/xtask/README.md`).

### שלב ב' - WSV ושטח הבקשות
1. **עסקאות של אחסון המדינה**
   - Collapser `state/storage_transactions.rs` ב-`state/storage_transactions.rs` הוא מתאם טרנסקנסנל qui applique l'ordre de commit et la detection de conflits.
   - Des unit tests מאמת desormais que les modifications asset/world/triggers font rollback in cas d'echec.
2. **Refactor du modele de requetes**
   - Deplacer la logique de pagetion/cursor dans des composants reutilisables sous `crates/iroha_core/src/query/`. Aligner les representations Norito dans `iroha_data_model`.
   - Ajouter des snapshot queries pour triggers, assets and roles with un order deterministe (suivi via `crates/iroha_core/tests/snapshot_iterable.rs` pour la couverture actuelle).
3. **Consistance des snapshots**
   - S'assurer que le CLI `iroha ledger query` להשתמש ב-le meme chemin de snapshot que Sumeragi/מחזירים.
   - Les tests de regression de snapshots.

### שלב ג' - צינור Sumeragi
1. **Topologie et gestion des epoques**
   - Extraire `EpochRosterProvider` עם תכונה של יישומים בסיסיים על תמונות מצב של WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` fournit un constructeur פשוט וידידותי ללעג בספסלים/בדיקות.
2. **פישוט קונצנזוס**
   - Reorganizer `crates/iroha_core/src/sumeragi/*` עם מודולים: `pacemaker`, `aggregation`, `availability`, `witness` avec des types partages sous Norito.
   - החזרת ההודעה המועברת באופן אד-הוק למעטפות מסוג Norito והקדמה לבדיקות נכסים לשינוי תצוגה (לפי הודעות ה-backlog Sumeragi).
3. **נתיב אינטגרציה/הוכחה**
   - Aligner les lane proofs avec les engagements DA et garant une porting RBC uniforme.
   - Le test d'integration מקצה לקצה `integration_tests/tests/extra_functional/seven_peer_consistency.rs` ודא תחזוקה le chemin avec RBC פעיל.### שלב D - חוזים חכמים ומארחים מצביע-ABI
1. **מארח Audit de la Frontiere**
   - רכז את בדיקות ה-pointer-type (`ivm::pointer_abi`) ו-les adaptateurs de host (`iroha_core::smartcontracts::ivm::host`).
   - Les attentes de pointer table and les bindings the host manifest sont couverts par `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui exercent les mapppings TLV golden.
2. **Sandbox d'execution des triggers**
   - Refactoriser les triggers pour passer par un `TriggerExecutor` commun qui applique גז, validation de pointers and journaling d'evenements.
   - Ajouter des tests de regression pour les triggers call/time couvrant les chemins d'echec (suivi via `crates/iroha_core/tests/trigger_failure.rs`).
3. **יישור CLI ולקוח**
   - S'assurer que les operations CLI (`audit`, `gov`, `sumeragi`, `ivm`) reposent sur les fonctions client partagees `iroha` le drifte.
   - Les tests de snapshots JSON du CLI vivent dans `tests/cli/json_snapshot.rs`; gardez-les a jour pour que la sortie core continue de correspondre a la reference JSON canonique.

### שלב E - Durcissement du codec Norito
1. **רישום סכימות**
   - Creer un registry de schemas Norito sos `crates/norito/src/schema/` pour sourcer les encodages canoniques des types core.
   - בדיקת Ajout de doc מאמתת l'encodage de payloads d'exemple (`norito::schema::SamplePayload`).
2. **Refresh des golden fixtures**
   - Mettre a jour les golden fixtures `crates/norito/tests/*` pour correspondre au nouveau schema WSV une fois le refactor livre.
   - `scripts/norito_regen.sh` regenere les golden JSON Norito de facon deterministe via le helper `norito_regen_goldens`.
3. **שילוב IVM/Norito**
   - Valider la serialization des manifests Kotodama מקצה לקצה באמצעות Norito, ומבטיח מצביע מטא נתונים ABI קוהרנטי.
   - `crates/ivm/tests/manifest_roundtrip.rs` maintient la parite Norito קידוד/פענוח לשפוך מניפסטים.

## 3. עיסוקים רוחביים
- **אסטרטגיית בדיקות**: בדיקות יחידה של שלב צ'אק -> בדיקות ארגז -> בדיקות אינטגרציה. Les בדיקות ובדיקת תקינות רגרסיות בפועל; les nouveaux tests evitent leur retour.
- **תיעוד**: שלב אפר-צ'אק, מטרה שנייה ב-`status.md` et reporter les items ouverts dans `roadmap.md` tout en supprimant les taches terminees.
- **מדדי ביצועים**: Maintenir les benches existants dans `iroha_core`, `ivm` et `norito`; ajouter des mesures de base post-refactor pour valider l'absence de regressions.
- **דגלי תכונה**: שומר חילופי מחליפים או ייחודיות של ארגז רמה עבור קצה אחורי שדרוש לשרשרת כלים חיצונית (`cuda`, `zk-verify-batch`). Les chemins SIMD CPU sont toujours construits and selectionnes a l'executes; fournir des fallbacks scalaires deterministes pour le materiel non supporte.## 4. פעולות מיידיות
- פיגומים שלב א' (תכונת תמונת מצב + טלמטריה של חיווט) - voir les taches actionnables dans les mises a jour du מפת דרכים.
- L'audit recent des defauts pour `sumeragi`, `state` et `ivm` a revele les points suivants:
  - `sumeragi`: הקצבאות של קוד מת מגן על שידור התצוגה המקדימה, שינוי VRF ו-Export Telemetrie EMA. Ils restent gates jusqu'a ce que la simplification du flux de consensus de la Phase C et les livrables d'integration lane/proof soient livrees.
  - `state`: le nettoyage de `Cell` et le routage telemetrie passat sur la piste telemetrie WSV de la Phase A, tandis que les notes SoA/Basculent-apply basculent in the backlog d'optimization de pipeline de la Phase C.
  - `ivm`: התצוגה של CUDA, אימות המעטפות ו-Halo2/Metal מפותחת את הגבול המארח של Phase D בתוספת נושא ה-GPU של האצה רוחבית; les kernels restent dans la backlog GPU dedie jusqu'a maturite.
- מכין את תוכנית ה-RFC חוצה צוות לסיומה של קוד השינויים.

## 5. שאלות אוברטיות
- RBC doit-il rester optionnel apres P1, ou est-il obligatoire pour les lanes du ledger Nexus? החלטה של ​​הצדדים לפני דרישה.
- Doit-on imposer des groupes de composabilite DS en P1 ou les laisser desactives jusqu'a maturite des lane proofs?
- מקור לוקליזציה קנוני של פרמטרים ML-DSA-87? מועמד: ארגז נובו `crates/fastpq_isi` (יצירה en attente).

---

_Derniere mise a jour: 2025-09-12_