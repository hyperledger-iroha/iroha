---
lang: he
direction: rtl
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-09T07:05:10.922933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# אישורי JDG: שמירה, סיבוב ושימור

הערה זו מתעדת את שומר האישורים v1 JDG שנשלח כעת ב-`iroha_core`.

- **מניפסטים של הוועדה:** חבילות `JdgCommitteeManifest` מקודדות Norito נושאות סיבוב לכל מרחב נתונים
  לוחות זמנים (`committee_id`, חברים שהוזמנו, סף, `activation_height`, `retire_height`).
  מניפסטים עמוסים ב-`JdgCommitteeSchedule::from_path` ואוכפים עלייה קפדנית
  גבהי הפעלה עם חפיפת חסד אופציונלית (`grace_blocks`) בין פרישה להפעלה
  ועדות.
- **שומר אישורים:** `JdgAttestationGuard` אוכף כריכת מרחב נתונים, תפוגה, גבולות מיושנים,
  התאמה של מזהה ועדה/סף, חברות חותם, תוכניות חתימה נתמכות ואופציונלי
  אימות SDN באמצעות `JdgSdnEnforcer`. מכסי גודל, פיגור מקסימלי וסכימות חתימה מותרות
  פרמטרים של קונסטרוקטור; `validate(attestation, dataspace, current_height)` מחזיר את הפעיל
  ועדה או טעות מובנית.
  - `scheme_id = 1` (`simple_threshold`): חתימות לכל חותם, מפת סיביות לחתום אופציונלית.
  - `scheme_id = 2` (`bls_normal_aggregate`): חתימה אחת מצטברת מראש BLS-רגילה על פני
    hash של אישור; מפת סיביות של חתם אופציונלית, ברירת המחדל היא לכל החותמים באישור. BLS
    אימות מצטבר דורש PoP תקף לכל חבר ועדה במניפסט; חסר או
    PoPs לא חוקיים דוחים את האישור.
  הגדר את רשימת ההיתרים באמצעות `governance.jdg_signature_schemes`.
- **חנות שמירה:** `JdgAttestationStore` עוקב אחר אישורים לכל מרחב נתונים עם הגדרה שניתן להגדיר
  מכסה לכל מרחב נתונים, גיזום הערכים הישנים ביותר בהוספת. התקשר ל-`for_dataspace` או
  `for_dataspace_and_epoch` כדי לאחזר חבילות ביקורת/השמעה חוזרת.
- **מבחנים:** כיסוי יחידה מפעיל כעת בחירת ועדה חוקית, דחיית חתם לא ידוע, מעופש
  דחיית אישור, מזהי סכימה לא נתמכים וגיזום שמירה. ראה
  `crates/iroha_core/src/jurisdiction.rs`.

השומר דוחה תוכניות מחוץ לרשימת ההיתרים שהוגדרה.