---
lang: he
direction: rtl
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2026-01-03T18:07:58.674563+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! רשימת משוב של Connect Architecture

רשימת בדיקה זו לוכדת את השאלות הפתוחות מארכיטקטורת Connect Session
איש קש שדורשים קלט מהלידים של אנדרואיד ו-JavaScript לפני ה-
פברואר 2026 סדנת חוצה SDK. השתמש בו כדי לאסוף הערות באופן אסינכרוני, עקוב אחר
בעלות, ולבטל את חסימת סדר היום של הסדנה.

> העמודה סטטוס / הערות תפסה תגובות סופיות מהפניות של Android ו-JS נכון לתאריך
> הסנכרון של פברואר 2026 לפני הסדנה; לקשר נושאי מעקב חדשים בתוך החלטות
> להתפתח.

## מחזור חיים ותחבורה

| נושא | בעל אנדרואיד | בעל JS | סטטוס / הערות |
|-------|--------------|--------|----------------|
| WebSocket אסטרטגיית ביטול חזרה לחיבור מחדש (אקספוננציאלי לעומת ליניארי מכסה) | Android Networking TL | JS Lead | ✅ מוסכם על ביטול אקספוננציאלי עם ריצוד, מוגבל ל-60; JS משקף את אותם קבועים עבור שוויון דפדפן/צומת. |
| ברירות מחדל של קיבולת מאגר לא מקוונת (קש נוכחי: 32 פריימים) | Android Networking TL | JS Lead | ✅ ברירת מחדל של 32 מסגרות מאושרת עם עקיפה של תצורה; אנדרואיד ממשיך דרך `ConnectQueueConfig`, JS מכבד את `window.connectQueueMax`. |
| הודעות חיבור מחדש בסגנון דחיפה (FCM/APNS לעומת סקר) | Android Networking TL | JS Lead | ✅ אנדרואיד יחשוף וו FCM אופציונלי עבור אפליקציות ארנק; JS נשאר מבוסס סקרים עם גיבוי אקספוננציאלי, תוך שימת לב לאילוצי דחיפה של הדפדפן. |
| מעקות בטיחות פינג/פונג עבור לקוחות ניידים | Android Networking TL | JS Lead | ✅ פינג סטנדרטי של שנות ה-30 עם סובלנות פספוס של 3×; אנדרואיד מאזנת את ההשפעה של Doze, JS נצמד ל-≥15 שניות כדי למנוע מצערת דפדפן. |

## הצפנה וניהול מפתחות

| נושא | בעל אנדרואיד | בעל JS | סטטוס / הערות |
|-------|--------------|--------|----------------|
| ציפיות לאחסון מפתח X25519 (StrongBox, WebCrypto בהקשרים מאובטחים) | Android Crypto TL | JS Lead | ✅ אנדרואיד מאחסנת את X25519 ב-StrongBox כאשר זמין (נופל בחזרה ל-TEE); JS מחייב WebCrypto בהקשר מאובטח עבור dApps, נופל בחזרה לגשר `iroha_js_host` המקורי ב-Node. |
| ChaCha20-Poly1305 שיתוף לא ניהולי בין ערכות SDK | Android Crypto TL | JS Lead | ✅ אמץ `sequence` מונה API משותף עם מגן גלישה של 64 סיביות ובדיקות משותפות; JS משתמש במונים של BigInt כדי להתאים להתנהגות חלודה. |
| סכימת עומס אישורים מגובה חומרה | Android Crypto TL | JS Lead | ✅ הסכימה הסתיימה: `attestation { platform, evidence_b64, statement_hash }`; JS אופציונלי (דפדפן), Node משתמש בהוק של תוסף HSM. |
| זרימת שחזור עבור ארנקים שאבדו (לחיצת יד לסיבוב מפתח) | Android Crypto TL | JS Lead | ✅ לחיצת יד לסיבוב ארנק מקובלת: dApp מנפיקה שליטה ב-`rotate`, תשובות בארנק עם מפתח פאב חדש + אישור חתום; JS מפתח מחדש חומר WebCrypto באופן מיידי. |

## הרשאות וחבילות הוכחה| נושא | בעל אנדרואיד | בעל JS | סטטוס / הערות |
|-------|--------------|--------|----------------|
| סכימת הרשאות מינימלית (שיטות/אירועים/משאבים) עבור GA | Android Data Model TL | JS Lead | ✅ GA בסיס: `methods`, `events`, `resources`, `constraints`; JS מיישר את סוגי TypeScript עם Rust Manifest. |
| מטען דחיית ארנק (`reason_code`, הודעות מקומיות) | Android Networking TL | JS Lead | ✅ קודים סופיים (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) בתוספת `localized_message` אופציונלי. |
| שדות אופציונליים של חבילת הוכחה (תאימות/קבצי KYC) | Android Data Model TL | JS Lead | ✅ כל ערכות ה-SDK מקבלים אופציונליים `attachments[]` (Norito `AttachmentRef`) ו-`compliance_manifest_id`; לא נדרש שינוי התנהגותי. |
| יישור על סכימת Norito JSON לעומת מבנים שנוצרו על ידי גשר | Android Data Model TL | JS Lead | ✅ החלטה: העדפת מבנים שנוצרו על ידי גשרים; נתיב JSON נשאר לניפוי באגים בלבד, JS שומר על מתאם `Value`. |

## חזיתות SDK וצורת API

| נושא | בעל אנדרואיד | בעל JS | סטטוס / הערות |
|-------|--------------|--------|----------------|
| שוויון ממשקי אסינכרון ברמה גבוהה (`Flow`, איטרטורים אסינכרוניים) | Android Networking TL | JS Lead | ✅ אנדרואיד חושפת את `Flow<ConnectEvent>`; JS משתמש ב-`AsyncIterable<ConnectEvent>`; שניהם מפות ל-`ConnectEventKind` משותף. |
| מיפוי טקסונומיה של שגיאות (`ConnectError`, תת מחלקות מוקלדות) | Android Networking TL | JS Lead | ✅ אמצו את הרשימה המשותפת {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`, עם פרטים ספציפיים לפלטפורמה. |
| סמנטיקה של ביטול לבקשות שלטים בטיסה | Android Networking TL | JS Lead | ✅ הציג שליטה `cancelRequest(hash)`; שני ערכות ה-SDK מציגות קורוטינות/הבטחות הניתנות לביטול המכבדות את אישור הארנק. |
| ווי טלמטריה משותפים (אירועים, שמות מדדים) | Android Networking TL | JS Lead | ✅ שמות מדדים מיושרים: `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total`; יצואנים לדוגמה מתועדים. |

## התמדה לא מקוונת ורישום יומן

| נושא | בעל אנדרואיד | בעל JS | סטטוס / הערות |
|-------|--------------|--------|----------------|
| פורמט אחסון לפריימים בתור (בינארי Norito לעומת JSON) | Android Data Model TL | JS Lead | ✅ אחסן בינארי Norito (`.to`) בכל מקום; JS משתמש ב- IndexedDB `ArrayBuffer`. |
| מדיניות שימור יומן ומכסות גודל | Android Networking TL | JS Lead | ✅ שימור ברירת מחדל 24 שעות ו-1MiB לכל הפעלה; ניתן להגדרה באמצעות `ConnectQueueConfig`. |
| פתרון קונפליקטים כאשר שני הצדדים מפעילים מחדש מסגרות | Android Networking TL | JS Lead | ✅ השתמש ב-`sequence` + `payload_hash`; התעלמו מהכפילויות, התנגשויות מפעילות את `ConnectError.Internal` עם אירוע טלמטריה. |
| טלמטריה לעומק תור והצלחת השידור החוזר | Android Networking TL | JS Lead | ✅ מד פליטת `connect.queue_depth` ומונה `connect.replay_success_total`; שני ערכות ה-SDK מתחברים לסכימת טלמטריה משותפת של Norito. |

## קוצים והפניות ליישום- **מתקני גשר חלודה:** `crates/connect_norito_bridge/src/lib.rs` ובדיקות נלוות מכסים את נתיבי הקידוד/פענוח הקנוניים המשמשים כל SDK.
- **רתמת הדגמה מהירה:** תרגילי `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` חברו בין זרימות של מפגשים לטרנספורט מעוגל.
- **Swift CI gating:** הפעל את `make swift-ci` בעת עדכון חפצי Connect כדי לאמת זוגיות של מתקנים, הזנות של לוח המחוונים ומטא נתונים של Buildkite `ci/xcframework-smoke:<lane>:device_tag` לפני שיתוף עם ערכות SDK אחרות.
- **מבחני אינטגרציה של JavaScript SDK:** `javascript/iroha_js/test/integrationTorii.test.js` מאמתים את עוזרי הסטטוס/הפעלות של Connect מול Torii.
- **הערות חוסן לקוח אנדרואיד:** `java/iroha_android/README.md:150` מתעד את ניסויי הקישוריות הנוכחיים שהיוו השראה לברירות המחדל של התור/חזרה לאחור.

## פריטי הכנה לסדנה

- [x] אנדרואיד: הקצה איש נקודה לכל שורה בטבלה למעלה.
- [x] JS: הקצה איש נקודה לכל שורה בטבלה למעלה.
- [x] אסוף קישורים לעליות יישום או ניסויים קיימים.
- [x] תזמן בדיקה לפני העבודה לפני מועצת פברואר 2026 (מוזמן ל-2026-01-29 15:00 UTC עם Android TL, JS Lead, Swift Lead).
- [x] עדכון `docs/source/connect_architecture_strawman.md` עם תשובות מקובלות.

## חבילה קריאה מראש

- ✅ חבילה מוקלטת תחת `artifacts/connect/pre-read/20260129/` (נוצר באמצעות `make docs-html` לאחר רענון הקש, מדריכי SDK ורשימת הבדיקה הזו).
- 📄 סיכום + שלבי הפצה חיים ב-`docs/source/project_tracker/connect_architecture_pre_read.md`; כלול את הקישור בהזמנה לסדנה בפברואר 2026 ובתזכורת `#sdk-council`.
- 🔁 בעת רענון החבילה, עדכן את הנתיב וה-hash בתוך הערת הקריאה מראש ואחסן את ההודעה בארכיון ב-`status.md` תחת יומני המוכנות של IOS7/AND7.