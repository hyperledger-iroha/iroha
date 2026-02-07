---
lang: he
direction: rtl
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ספר הפעלה סודי של סבב נכסים עם הפניה של `roadmap.md:M3`.

# ספר ריצה של סיבוב נכסים סודי

ספר משחק זה מסביר כיצד מפעילים מתזמנים ומבצעים נכס סודי
סיבובים (ערכות פרמטרים, מפתחות אימות ומעברי מדיניות) תוך כדי
הבטחת ארנקים, לקוחות Torii ושומרי mempool יישארו דטרמיניסטים.

## מחזור חיים וסטטוסים

ערכות פרמטרים סודיות (`PoseidonParams`, `PedersenParams`, מפתחות אימות)
סריג ועוזר המשמשים כדי להסיק את הסטטוס האפקטיבי בגובה נתון חיים ב
`crates/iroha_core/src/state.rs:7540`–`7561`. סריקה של עוזרי זמן ריצה בהמתנה
מעברים ברגע שמגיעים לגובה היעד וכשלים ביומן למועד מאוחר יותר
שידורים חוזרים (`crates/iroha_core/src/state.rs:6725`–`6765`).

הטמעת מדיניות נכסים
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
כך שהממשל יכול לתזמן שדרוגים באמצעות
`ScheduleConfidentialPolicyTransition` ובטל אותם במידת הצורך. ראה
`crates/iroha_data_model/src/asset/definition.rs:320` והמראות Torii DTO
(`crates/iroha_torii/src/routing.rs:1539`–`1580`).

## זרימת עבודה של סיבוב

1. **פרסם חבילות פרמטרים חדשות.** המפעילים שולחים
   הוראות `PublishPedersenParams`/`PublishPoseidonParams` (CLI
   `iroha app zk params publish ...`) לשלב ערכות גנרטורים חדשות עם מטא נתונים,
   חלונות הפעלה/הוצאה משימוש, וסמני סטטוס. מנהל ההוצאה לפועל דוחה
   זיהויים כפולים, גרסאות שאינן מתגברות, או מעברי סטטוס גרועים לכל
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`, וה
   בדיקות הרישום מכסות את מצבי הכשל (`crates/iroha_core/tests/confidential_params_registry.rs:93`–`226`).
2. **עדכוני רישום/אימות מפתח.** `RegisterVerifyingKey` אוכף קצה אחורי,
   מחויבות, ואילוצי מעגל/גרסה לפני שמפתח יכול להיכנס ל
   רישום (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`).
   עדכון מפתח מבטל אוטומטית את הערך הישן ומחק בתים מוטבעים,
   כפי שהופעל על ידי `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1`.
3. **תזמן מעברים של מדיניות נכסים.** ברגע שמזהי הפרמטרים החדשים יהיו פעילים,
   ממשל קורא `ScheduleConfidentialPolicyTransition` עם הרצוי
   מצב, חלון מעבר וגיבוב ביקורת. מנהל ההוצאה לפועל מסרב לסתור
   מעברים או נכסים עם היצע שקוף יוצא מן הכלל. בדיקות כגון
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` ודא ש
   מעברים בוטלו ברור `pending_transition`, בעוד
   `confidential_policy_transition_reaches_shielded_only_on_schedule` בשעה
   lines385–433 מאשר שהשדרוגים המתוזמנים עוברים ל-`ShieldedOnly` בדיוק בשעה
   הגובה האפקטיבי.
4. **יישום מדיניות ושמירה של mempool.** מבצע החסימה מטאטא את כל הממתינים
   מעברים בתחילת כל בלוק (`apply_policy_if_due`) ופולט
   טלמטריה אם המעבר נכשל כדי שהמפעילים יוכלו לתזמן מחדש. במהלך הקבלה
   ה-mempool מסרבת לעסקאות שהמדיניות האפקטיבית שלהן תשתנה באמצע הבלוק,
   הבטחת הכללה דטרמיניסטית על פני חלון המעבר
   (`docs/source/confidential_assets.md:60`).

## דרישות ארנק ו-SDK- Swift ו-SDKs ניידים אחרים חושפים את עוזרי Torii כדי להביא את המדיניות הפעילה
  בנוסף לכל מעבר ממתין, כך שהארנקים יכולים להזהיר משתמשים לפני החתימה. ראה
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) והמשויך
  בדיקות ב-`IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- ה-CLI משקף את אותם מטא נתונים דרך `iroha ledger assets data-policy get` (עוזר ב
  `crates/iroha_cli/src/main.rs:1497`–`1670`), המאפשרת למפעילים לבקר את
  מזהי מדיניות/פרמטרים המחוברים להגדרת נכס מבלי להכתיר את
  חנות בלוק.

## כיסוי בדיקה וטלמטריה

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` מאמת את המדיניות הזו
  מעברים מתפשטים לצילומי מצב של מטא נתונים ומנקים לאחר החלתם.
- `crates/iroha_core/tests/zk_dedup.rs:1` מוכיח שהמטמון `Preverify`
  דוחה הוצאה כפולה/הוכחות כפולות, כולל תרחישי סיבוב שבהם
  ההתחייבויות שונות.
- `crates/iroha_core/tests/zk_confidential_events.rs` ו
  `zk_shield_transfer_audit.rs` כיסוי מגן מקצה לקצה → העברה → unshield
  זורם, מה שמבטיח שמסלול הביקורת ישרוד על פני סיבובי פרמטרים.
- `dashboards/grafana/confidential_assets.json` ו
  `docs/source/confidential_assets.md:401` מתעד את CommitmentTree &
  מדי מטמון מאמת שמלווים כל ריצת כיול/סיבוב.

## בעלות על ספר הפעלה

- **Leads DevRel / Wallet SDK:** לשמור על קטעי SDK + התחלות מהירות שמופיעות
  כיצד להעלות מעברים ממתינים ולהפעיל מחדש את המנטה → העברה → לחשוף
  בדיקות מקומיות (במעקב תחת `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2`).
- **ניהול תוכנית / נכסים סודיים TL:** לאשר בקשות מעבר, לשמור
  `status.md` מעודכן עם סיבובים קרובים, והבטח ויתורים (אם יש)
  רשום לצד ספר הכיול.