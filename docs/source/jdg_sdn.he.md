---
lang: he
direction: rtl
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% אישורי JDG-SDN וסיבוב

הערה זו לוכדת את מודל האכיפה עבור אישורי צומת נתונים סודיים (SDN).
בשימוש על ידי זרימת Data Guardian (JDG).

## פורמט התחייבות
- `JdgSdnCommitment` קושר את ה-scope (`JdgAttestationScope`), את המוצפן
  Hash של מטען, והמפתח הציבורי של SDN. חותמות הן חתימות מודפסות
  (`SignatureOf<JdgSdnCommitmentSignable>`) מעל המטען המתויג בדומיין
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- אימות מבני (`validate_basic`) אוכף:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - טווחי בלוק חוקיים
  - חותמות לא ריקות
  - שוויון היקף מול האישור בעת הפעלתו
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- מניעת כפילויות מטופלת על ידי מאמת האישורים (חתם + מטען גיבוב
  ייחודיות) כדי למנוע התחייבויות נמנעות/כפולות.

## מדיניות רישום ורוטציה
- מפתחות SDN חיים ב-`JdgSdnRegistry`, עם מפתחות `(Algorithm, public_key_bytes)`.
- `JdgSdnKeyRecord` מתעד את גובה ההפעלה, גובה פרישה אופציונלי,
  ומפתח אב אופציונלי.
- הסיבוב נשלט על ידי `JdgSdnRotationPolicy` (נכון לעכשיו: `dual_publish_blocks`
  חלון חפיפה). רישום מפתח ילד מעדכן את פרישת ההורה ל
  `child.activation + dual_publish_blocks`, עם מעקות:
  - הורים נעדרים נדחים
  - ההפעלה חייבת להיות מוגברת בקפדנות
  - חפיפות החורגות מחלון החסד נדחות
- עוזרי הרישום מציגים את הרשומות המותקנות (`record`, `keys`) לסטטוס
  וחשיפת API.

## זרימת אימות
- `JdgAttestation::validate_with_sdn_registry` עוטף את המבני
  בדיקות אישור ואכיפת SDN. שרשורי `JdgSdnPolicy`:
  - `require_commitments`: אכיפת נוכחות עבור מטענים אישיים אישיים/סודיים
  - `rotation`: חלון חסד בשימוש בעת עדכון פרישת ההורים
- כל התחייבות נבדקת עבור:
  - תוקף מבני + התאמה להיקף אישור
  - נוכחות מפתח רשומה
  - חלון פעיל המכסה את טווח הבלוקים המאושר (כבר גבולות הפרישה
    כולל את החסד של פרסום כפול)
  - חותם תקף על גוף ההתחייבות המתויג בדומיין
- שגיאות יציבות משטחות את האינדקס לראיות מפעיל:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  או כשלים מבניים של `Commitment`/`ScopeMismatch`.

## ספר הפעלה של מפעילים
- **הגשה:** רשום את מפתח ה-SDN הראשון עם `activated_at` ב- או לפני
  גובה הבלוק הסודי הראשון. פרסם את טביעת האצבע של המפתח למפעילי JDG.
- **סובב:** צור את המפתח העוקב, רשום אותו עם `rotation_parent`
  מצביע על המפתח הנוכחי, ואשר את פרישת ההורים שווה
  `child_activation + dual_publish_blocks`. אטום מחדש התחייבויות מטען עם
  המפתח הפעיל במהלך חלון החפיפה.
- **ביקורת:** חשיפת תמונות מצב של הרישום (`record`, `keys`) באמצעות Torii/סטטוס
  משטחים כך שמבקרים יכולים לאשר את המפתח הפעיל ואת חלונות הפרישה. התראה
  אם הטווח המאושר נופל מחוץ לחלון הפעיל.
- **שחזור:** `UnknownSdnKey` ← ודא שהרישום כולל את מפתח האיטום;
  `InactiveSdnKey` ← לסובב או להתאים את גבהי ההפעלה; `InvalidSeal` ←
  לאטום מחדש מטענים ולרענן אישורים.## עוזר זמן ריצה
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) חבילות מדיניות +
  רישום ומאמת אישורים באמצעות `validate_with_sdn_registry`.
- ניתן לטעון רישום מחבילות `JdgSdnKeyRecord` מקודדות Norito (ראה
  `JdgSdnEnforcer::from_reader`/`from_path`) או מורכב עם
  `from_records`, אשר מיישם את מעקות הבטיחות הסיבוביים במהלך הרישום.
- מפעילים יכולים להתמיד בצרור Norito כהוכחה ל-Torii/סטטוס
  משטחים בזמן שאותו מטען מזין את האכיפה המשמש את הודאה ו
  שומרי קונצנזוס. ניתן לאתחל אכיפה גלובלית אחת בעת ההפעלה באמצעות
  `init_enforcer_from_path`, ו-`enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  לחשוף את המדיניות החיה + רשומות מפתח עבור משטחי סטטוס/Torii.

## מבחנים
- כיסוי רגרסיה ב-`crates/iroha_data_model/src/jurisdiction.rs`:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, לצד הקיים
  מבחני אישור מבני/בדיקת SDN.