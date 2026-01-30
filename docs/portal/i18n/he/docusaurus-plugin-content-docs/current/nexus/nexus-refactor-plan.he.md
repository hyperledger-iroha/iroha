---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6670a48836cddc39f2f61b57a80fa6a1bd3254396f6f0b3440baf80e62e07aa3
source_last_modified: "2025-11-14T04:43:20.463888+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/nexus_refactor_plan.md`. שמרו על יישור שתי הגרסאות עד שהמהדורה הרב-לשונית תגיע לפורטל.
:::

# תוכנית רפקטור ללדג'ר Sora Nexus

מסמך זה מתעד את מפת הדרכים המיידית לרפקטור של Sora Nexus Ledger ("Iroha 3"). הוא משקף את פריסת המאגר הנוכחית ואת הרגרסיות שנצפו בניהול החשבונאי של genesis/WSV, קונצנזוס Sumeragi, טריגרים של חוזים חכמים, שאילתות snapshot, חיבורים של host pointer-ABI וקודקים של Norito. המטרה היא להתכנס לארכיטקטורה עקבית ובת-בדיקה בלי לנסות לנחות את כל התיקונים בפאץ' מונוליתי אחד.

## 0. עקרונות מנחים
- לשמר התנהגות דטרמיניסטית על פני חומרה הטרוגנית; להשתמש בהאצה רק דרך feature flags opt-in עם fallbacks זהים.
- Norito היא שכבת הסריאליזציה. כל שינויי state/schema חייבים לכלול בדיקות round-trip ל-Norito encode/decode ועדכוני fixtures.
- הקונפיגורציה זורמת דרך `iroha_config` (user -> actual -> defaults). להסיר toggles סביבתיים אד-הוק ממסלולי פרודקשן.
- מדיניות ABI נשארת V1 ואינה נתונה למו"מ. hosts חייבים לדחות pointer types/syscalls לא מוכרים בצורה דטרמיניסטית.
- `cargo test --workspace` וה-golden tests (`ivm`, `norito`, `integration_tests`) נשארים שער הבסיס לכל אבן דרך.

## 1. צילום טופולוגיית הריפו
- `crates/iroha_core`: שחקני Sumeragi, WSV, טוען genesis, pipelines (query, overlay, zk lanes), ו-glue של host לחוזים חכמים.
- `crates/iroha_data_model`: סכימה סמכותית לנתונים ושאילתות on-chain.
- `crates/iroha`: API לקוח בשימוש CLI, tests, SDK.
- `crates/iroha_cli`: CLI למפעילים, משקף כיום APIs רבים ב-`iroha`.
- `crates/ivm`: VM לבייטקוד Kotodama, נקודות כניסה לאינטגרציית pointer-ABI של host.
- `crates/norito`: codec סריאליזציה עם מתאמי JSON ו-backends של AoS/NCB.
- `integration_tests`: assertions בין רכיבים שמכסות genesis/bootstrap, Sumeragi, triggers, pagination ועוד.
- המסמכים כבר משרטטים את יעדי Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), אבל המימוש מקוטע וחלקית מיושן ביחס לקוד.

## 2. עמודי התווך של הרפקטור ואבני דרך

### שלב A - יסודות ותצפיתיות
1. **טלמטריית WSV + Snapshots**
   - לקבע API קנוני ל-snapshot ב-`state` (trait `WorldStateSnapshot`) המשמש לשאילתות, Sumeragi ו-CLI.
   - להשתמש ב-`scripts/iroha_state_dump.sh` כדי להפיק snapshots דטרמיניסטיים דרך `iroha state dump --format norito`.
2. **דטרמיניזם של Genesis/Bootstrap**
   - לרפקטור את ingest של genesis כך שיעבור דרך pipeline יחיד מבוסס Norito (`iroha_core::genesis`).
   - להוסיף כיסוי אינטגרציה/רגרסיה שמריץ מחדש genesis יחד עם הבלוק הראשון ומאשר roots של WSV זהים בין arm64/x86_64 (נעקב ב-`integration_tests/tests/genesis_replay_determinism.rs`).
3. **בדיקות Fixity בין crates**
   - להרחיב `integration_tests/tests/genesis_json.rs` כדי לאמת invariants של WSV, pipeline ו-ABI ב-harness אחד.
   - להציג scaffold של `cargo xtask check-shape` שמבצע panic על schema drift (נעקב ב-backlog של DevEx tooling; ראו action item ב-`scripts/xtask/README.md`).

### שלב B - WSV ומשטח שאילתות
1. **טרנזקציות של State Storage**
   - לאחד את `state/storage_transactions.rs` לאדפטר טרנזקציוני שמחייב סדר commit וזיהוי קונפליקטים.
   - Unit tests כעת מאמתים ששינויים ב-assets/world/triggers מתבטלים (rollback) בעת כשל.
2. **רפקטור מודל השאילתות**
   - להעביר לוגיקת pagination/cursor לרכיבים שימושיים מחדש תחת `crates/iroha_core/src/query/`. ליישר ייצוגי Norito ב-`iroha_data_model`.
   - להוסיף snapshot queries עבור triggers, assets ותפקידים עם סדר דטרמיניסטי (נעקב דרך `crates/iroha_core/tests/snapshot_iterable.rs` לכיסוי הנוכחי).
3. **עקביות Snapshots**
   - לוודא ש-CLI `iroha ledger query` משתמש באותו נתיב snapshot כמו Sumeragi/fetchers.
   - בדיקות רגרסיה של snapshot ב-CLI נמצאות תחת `tests/cli/state_snapshot.rs` (feature-gated לריצות איטיות).

### שלב C - Pipeline של Sumeragi
1. **טופולוגיה וניהול Epoch**
   - לחלץ `EpochRosterProvider` ל-trait עם מימושים הנתמכים ב-snapshots של stake ב-WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` מספק constructor פשוט וידידותי ל-mock עבור benches/tests.
2. **פישוט זרימת הקונצנזוס**
   - לארגן מחדש את `crates/iroha_core/src/sumeragi/*` למודולים: `pacemaker`, `aggregation`, `availability`, `witness` עם טיפוסים משותפים תחת `consensus`.
   - להחליף העברת הודעות ad-hoc ב-envelopes מטופסים של Norito ולהוסיף property tests ל-view-change (נעקב ב-backlog של הודעות Sumeragi).
3. **אינטגרציית Lane/Proof**
   - ליישר lane proofs עם התחייבויות DA ולוודא שה-gating של RBC אחיד.
   - בדיקת אינטגרציה end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs` מאמתת כעת את המסלול עם RBC פעיל.

### שלב D - חוזים חכמים ו-hosts של Pointer-ABI
1. **ביקורת גבול ה-host**
   - לאחד בדיקות pointer-type (`ivm::pointer_abi`) ואדפטרים של host (`iroha_core::smartcontracts::ivm::host`).
   - ציפיות pointer table ו-bindings של host manifest מכוסות על ידי `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` ו-`ivm_host_mapping.rs`, שמפעילים את מיפויי ה-TLV golden.
2. **Sandbox להרצת Triggers**
   - לרפקטור triggers כך שירוצו דרך `TriggerExecutor` משותף שמחיל gas, אימות pointers ורישום אירועים (journaling).
   - להוסיף בדיקות רגרסיה ל-triggers של call/time המכסות מסלולי כשל (נעקב דרך `crates/iroha_core/tests/trigger_failure.rs`).
3. **יישור CLI ו-client**
   - לוודא שפעולות CLI (`audit`, `gov`, `sumeragi`, `ivm`) מסתמכות על פונקציות הלקוח המשותפות של `iroha` כדי למנוע drift.
   - בדיקות snapshot JSON של CLI נמצאות תחת `tests/cli/json_snapshot.rs`; להשאיר אותן מעודכנות כדי שפלטי פקודות הליבה ימשיכו להתאים לייחוס JSON הקנוני.

### שלב E - הקשחת Codec של Norito
1. **רישום סכימות**
   - ליצור schema registry של Norito תחת `crates/norito/src/schema/` כדי לספק encodings קנוניים לסוגי נתונים ליבה.
   - נוספו doc tests שמאמתים קידוד של payloads לדוגמה (`norito::schema::SamplePayload`).
2. **רענון Golden Fixtures**
   - לעדכן את golden fixtures ב-`crates/norito/tests/*` כדי להתאים לסכמת WSV החדשה כאשר הרפקטור ינחת.
   - `scripts/norito_regen.sh` מייצר מחדש את ה-golden JSON של Norito באופן דטרמיניסטי דרך ה-helper `norito_regen_goldens`.
3. **אינטגרציה בין IVM ל-Norito**
   - לאמת סריאליזציה של Kotodama manifest מקצה לקצה דרך Norito, כדי שהמטא-דטה של pointer ABI תהיה עקבית.
   - `crates/ivm/tests/manifest_roundtrip.rs` שומר על פריטי encode/decode של Norito עבור manifests.

## 3. נושאים חוצי-רוחב
- **אסטרטגיית בדיקות**: כל שלב מקדם unit tests -> crate tests -> integration tests. בדיקות שנכשלות לוכדות רגרסיות נוכחיות; בדיקות חדשות מונעות מהן לחזור.
- **תיעוד**: לאחר שכל שלב נוחת, לעדכן את `status.md` ולהעביר פריטים פתוחים ל-`roadmap.md` תוך גיזום משימות שהושלמו.
- **מדדי ביצועים**: לשמר benches קיימים ב-`iroha_core`, `ivm` ו-`norito`; להוסיף מדידות בסיס לאחר הרפקטור כדי לוודא שאין רגרסיות.
- **Feature flags**: להשאיר toggles ברמת crate רק ל-backends שדורשים toolchains חיצוניים (`cuda`, `zk-verify-batch`). מסלולי SIMD של CPU תמיד נבנים ונבחרים בזמן ריצה; לספק fallbacks סקלריים דטרמיניסטיים לחומרה לא נתמכת.

## 4. פעולות מיידיות
- סקפולדינג של שלב A (snapshot trait + wiring של טלמטריה) - ראו משימות ביצועיות בעדכוני ה-roadmap.
- הבדיקה האחרונה של תקלות ב-`sumeragi`, `state` ו-`ivm` חשפה את ההדגשים הבאים:
  - `sumeragi`: dead-code allowances מגנים על שידור הוכחות view-change, מצב replay של VRF וייצוא טלמטריה של EMA. אלה נשארים gated עד שהפישוט של זרימת הקונצנזוס בשלב C ואינטגרציית lane/proof ינחתו.
  - `state`: ניקוי `Cell` וניתוב טלמטריה עוברים למסלול טלמטריית WSV של שלב A, בעוד שהערות SoA/parallel-apply נכנסות ל-backlog אופטימיזציית pipeline של שלב C.
  - `ivm`: חשיפת toggle CUDA, אימות envelopes וכיסוי Halo2/Metal ממופים לעבודת host-boundary של שלב D יחד עם נושא האצת GPU חוצה; kernels נשארים ב-backlog GPU הייעודי עד שיהיו מוכנים.
- להכין RFC חוצה-צוותים שמסכם את התוכנית לאישור לפני הנחת שינויי קוד חודרניים.

## 5. שאלות פתוחות
- האם RBC צריך להישאר אופציונלי מעבר ל-P1, או שהוא חובה ל-lanes של Nexus ledger? דורש החלטת בעלי עניין.
- האם נאכוף קבוצות קומפוזביליות DS ב-P1 או נשאיר אותן מושבתות עד ש-lane proofs יבשילו?
- מהו המיקום הקנוני לפרמטרים של ML-DSA-87? מועמד: crate חדש `crates/fastpq_isi` (בהקמה).

---

_עודכן לאחרונה: 2025-09-12_
