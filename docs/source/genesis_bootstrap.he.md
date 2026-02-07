---
lang: he
direction: rtl
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Bootstrap מאת Trusted Peers

עמיתים Iroha ללא `genesis.file` מקומי יכולים להביא בלוק בראשית חתום מעמיתים מהימנים
באמצעות פרוטוקול bootstrap המקודד Norito.

- **פרוטוקול:** עמיתים מחליפים `GenesisRequest` (`Preflight` עבור מטא נתונים, `Fetch` עבור מטען) ו
  מסגרות `GenesisResponse` עם מפתחות `request_id`. המגיבים כוללים את מזהה השרשרת, המפתח לחתום,
  hash, ורמז לגודל אופציונלי; מטענים מוחזרים רק ב-`Fetch`, ומזהי בקשות כפולים
  לקבל `DuplicateRequest`.
- **שומרים:** מגיבים אוכפים רשימת היתרים (`genesis.bootstrap_allowlist` או עמיתים מהימנים
  סט), התאמת שרשרת/מזהה/מפתח/hash, מגבלות תעריפים (`genesis.bootstrap_response_throttle`), ו-
  מכסה גודל (`genesis.bootstrap_max_bytes`). בקשות מחוץ לרשימת ההיתרים מקבלים `NotAllowed`, ו
  מטענים החתומים על ידי מפתח שגוי מקבלים `MismatchedPubkey`.
- **זרימת המבקשים:** כאשר האחסון ריק ו-`genesis.file` אינו מוגדר (ו
  `genesis.bootstrap_enabled=true`), הצומת מבצע בדיקה מוקדמת של עמיתים מהימנים עם האפשרות האופציונלית
  `genesis.expected_hash`, ואז מביא את המטען, מאמת חתימות באמצעות `validate_genesis_block`,
  ונמשך `genesis.bootstrap.nrt` לצד Kura לפני החלת הבלוק. Bootstrap מנסה שוב
  לכבד את `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval`, ו
  `genesis.bootstrap_max_attempts`.
- **מצבי כישלון:** בקשות נדחות בגין החמצות ברשימת ההיתרים, אי התאמה של שרשרת/מפתחות/hash, גודל
  הפרות מכסים, מגבלות תעריף, יצירה מקומית חסרה או מזהי בקשות כפולים. חשיש סותרים
  בין עמיתים לבטל את האחזור; אין מגיבים/פסקי זמן נופלים חזרה לתצורה המקומית.
- **שלבי מפעיל:** ודא שלפחות עמית מהימן אחד נגיש עם התחלה חוקית, הגדר
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` וכפתורי הניסיון החוזר, וכן
  אופציונלי להצמיד את `expected_hash` כדי למנוע קבלת מטענים לא תואמים. מטענים מתמשכים יכולים להיות
  נעשה שימוש חוזר במגפיים הבאים על ידי הצבעה של `genesis.file` ל-`genesis.bootstrap.nrt`.