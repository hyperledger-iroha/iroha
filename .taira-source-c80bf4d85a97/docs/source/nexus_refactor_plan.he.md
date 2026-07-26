<!-- Hebrew translation of docs/source/nexus_refactor_plan.md -->

---
lang: he
direction: rtl
source: docs/source/nexus_refactor_plan.md
status: complete
translator: manual
---

<div dir="rtl">

# תכנית ריפקטור ל-Sora Nexus Ledger

המסמך מסכם את מפת הדרכים המיידית לריפקטור של Sora Nexus Ledger ("Iroha 3"). הוא משקף את מבנה המאגר הנוכחי ואת הרגרסיות שנצפו בניהול Genesis/WSV, בקונצנזוס Sumeragi, בטריגרים של חוזים חכמים, בשאילתות snapshot, בביינדינג של pointer-ABI ובקודקים של Norito. היעד: ארכיטקטורה קוהרנטית וניתנת לבדיקה, ללא נחיתה של פאטצ' מונוליטי.

## 0. עקרונות מנחים
- לשמר דטרמיניזם על פני חומרה הטרוגנית; האצה רק דרך פיצ'רים opt-in עם fallback זהה.
- Norito היא שכבת הסריאליזציה. שינויי מצב/סכימה מחייבים בדיקת round-trip ו-fixture עדכני.
- תצורה עוברת דרך `iroha_config` (משתמש → actual → defaults). להסיר toggles סביבתיים ממסלולי פרודקשן.
- מדיניות ABI נשארת V1. Hosts דוחים pointer types/syscalls לא מזוהים בצורה דטרמיניסטית.
- `cargo test --workspace` והגולדן-טסטים (`ivm`, `norito`, `integration_tests`) הם שער בסיס לכל מיילסטון.

## 1. מבנה המאגר (Snapshot)
- `crates/iroha_core`: שחקני Sumeragi, WSV, רוטינת genesis, pipelines (שאילתות, overlays, מסלולי zk), הדבקת host.
- `crates/iroha_data_model`: סכימת on-chain ורשיונות שאילתות.
- `crates/iroha`: API לקוח עבור CLI, בדיקות, SDK.
- `crates/iroha_cli`: CLI לאופרציות (משכפל חלקית את `iroha`).
- `crates/ivm`: VM ל-Kotodama, נקודת הכניסה לפוינטר ABI.
- `crates/norito`: קודק סריאליזציה (JSON adapters, AoS/NCB).
- `integration_tests`: בדיקות אינטגרציה ל-genesis, Sumeragi, טריגרים, pagination וכו'.
- דוקומנטים (`nexus.md`, ‏`new_pipeline.md`, ‏`ivm.md`) מפרטים את היעדים אך המימוש חלקי ומיושן בחלקו.

## 2. עמודי ריפקטור ומיילסטונים

### שלב A — יסודות ותצפיתיות
1. **טלמטריית WSV + Snapshots**
   - Trait `WorldStateSnapshot` קנוני במודול `state`; משותף לשאילתות/Sumeragi/CLI.
   - השתמשו ב-`scripts/iroha_state_dump.sh` כדי להפיק snapshot דטרמיניסטי באמצעות `iroha state dump --format norito`.
2. **דטרמיניזם Genesis/Bootstrap**
   - צינור Norito יחיד (`iroha_core::genesis`) לטעינת ה-genesis.
   - בדיקת אינטגרציה `integration_tests/tests/genesis_replay_determinism.rs` מריצה את הג׳נסיס ואת הבלוק הראשון על arm64 ו-x86_64, משווה את Root ה-WSV, ושומרת את טביעות ה-state (`state_root`, `world_state_hash`) להשוואה בין ארכיטקטורות.
3. **בדיקות cross-crate**
   - הרחבת `integration_tests/tests/genesis_json.rs` ל-WSV/Pipeline/ABI.
   - להוסיף שלד `cargo xtask check-shape` שמזהה סטיית סכימה ומפיל panic (במעקב ב-backlog של DevEx, ראו משימת `scripts/xtask/README.md`).

### שלב B — WSV וממשק שאילתות
1. **עסקאות אחסון מצב**
   - איחוד `state/storage_transactions.rs` לאדפטר טרנזקציות עם סדר commit וגילוי קונפליקטים.
   - בדיקות יחידה לוודא rollback בעת כשל.
2. **ריפקטור מודל שאילתות**
   - העברת לוגיקת pagination/cursor לרכיבים ב-`iroha_core/src/query/`; יישור ייצוג Norito ב-`iroha_data_model`.
   - להוסיף שאילתות snapshot לטריגרים, נכסים ותפקידים עם סדר דטרמיניסטי (מנוטר באמצעות `crates/iroha_core/tests/snapshot_iterable.rs`).
3. **עקביות Snapshot**
   - CLI `iroha ledger query` משתמש באותה דרך snapshot כמו Sumeragi.
   - בדיקות רגרסיה ל-CLI (snapshots) נמצאות בקובץ `tests/cli/state_snapshot.rs` ומוגנות ב-feature עבור הרצה איטית.

### שלב C — פייפליין Sumeragi
1. **ניהול טופולוגיה/אפוק**
   - חילוץ `EpochRosterProvider` ל-trait עם מימוש מבוסס WSV.
  - הפונקציה `WsvEpochRosterAdapter::from_peer_iter` מספקת בקלות רוסטר מדומה עבור בנצ'ים/בדיקות.
2. **פישוט זרימת קונצנזוס**
   - רה-ארגון ל-modules: `pacemaker`, ‏`aggregation`, ‏`availability`, ‏`witness`.
   - להחליף מסרים אד-הוק ב-envelopes Norito typed ולהוסיף בדיקות property לסדר החלפת התצוגה (במעקב ב-backlog של מסרי Sumeragi).
3. **שילוב מסלולים/הוכחות**
   - התאמת מסלולי proofs ל-DA ו-RBC.
   - בדיקת אינטגרציה `seven_peer_consistency.rs` מכסה מסלול RBC.

### שלב D — חוזים חכמים ו-ABI
1. **Audit גבול Host**
   - איחוד בדיקות pointer (`ivm::pointer_abi`) ואדאפטרים (`iroha_core::smartcontracts::ivm::host`).
   - גולדני טבלת מצביעים והאוגדנים של ה-host מכוסים במבחנים `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` ו-`ivm_host_mapping.rs`.
2. **Sandbox טריגרים**
   - `TriggerExecutor` אחיד לגז/ולידציה/Journal אירועים.
   - לבדוק שהבדיקות לרגרסיה עבור טריגרים מסוג זמן/קריאה במקרה כשל נשארות יציבות (מכוסה על ידי `crates/iroha_core/tests/trigger_failure.rs`).
3. **יישור CLI/Client**
   - CLI נשען על פונקציות משותפות ב-`iroha`.
   - בדיקות Snapshot לפלט CLI נתמכות ב-`tests/cli/json_snapshot.rs`, וממשיכות לוודא שההדפסה הקאנונית תואמת את ייצוג ה-JSON המודל.

### שלב E — קשיחות קודק Norito
1. **רישום סכימה**
   - `crates/norito/src/schema/` לאחסון אנקודינג קנוני.
   - להוסיף Doc tests שמוודאים את דוגמאות ה-Norito schema (מנוטר ב-backlog של `norito`).
2. **רענון גולדנים**
   - עדכון `crates/norito/tests/*` לאחר שינוי סכימה.
   - `scripts/norito_regen.sh` מפעיל את `norito_regen_goldens` ומרענן דטרמיניסטית את גולדני ה-JSON של Norito.
3. **אינטגרציית IVM/Norito**
   - אימות end-to-end של מניפסט Kotodama דרך Norito.
   - הבדיקה `crates/ivm/tests/manifest_roundtrip.rs` מבטיחה סיבוב Norito מלא של מניפסטים.

## 3. נושאים רוחביים
- **אסטרטגיית בדיקות:** יחידה → crate → אינטגרציה. בדיקות קיימות לוכדות רגרסיות; חדשות מונעות חזרה.
- **תיעוד:** לאחר כל שלב לעדכן את `status.md`, ולהעביר סעיפים שטרם הושלמו ל-`roadmap.md` עם קישור לטיקט המעקב.
- **בנצ'מרקים:** לשמור את הבנצ'ים של `iroha_core`, ‏`ivm`, ‏`norito`; למדוד לאחר הריפקטור.
- **Feature Flags:** רק backend הדורשים toolchain חיצוני (`cuda`, ‏`zk-verify-batch`). SIMD CPU תמיד נבנה עם fallback סקאלרי דטרמיניסטי.

## 4. צעדים מיידיים
- scaffolding לשלב A (Trait Snapshot + טלמטריה).
- הכנת RFC רוחבי לאישור לפני שינויים פולשניים.

## 5. שאלות פתוחות
- האם RBC נשאר אופציונלי אחרי P1 או חובה במסלולי Nexus?
- האם לאכוף קבוצות DS composability ב-P1?
- מיקום קנוני לפרמטרי ML-DSA-87 (מועמד: crate חדש `fastpq_isi`).

---

_עודכן לאחרונה: 12 בספטמבר 2025_

</div>
