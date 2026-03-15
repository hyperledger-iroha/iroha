---
lang: he
direction: rtl
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# יעד אבטחה של Android SDK — יישור ETSI EN 319 401

| שדה | ערך |
|-------|-------|
| גרסת מסמך | 0.1 (2026-02-12) |
| היקף | Android SDK (ספריות לקוח תחת `java/iroha_android/` בתוספת סקריפטים/מסמכים תומכים) |
| בעלים | ציות ומשפט (סופיה מרטינס) |
| סוקרים | מוביל תוכנית אנדרואיד, הנדסת שחרור, ממשל SRE |

## 1. תיאור הבוהן

יעד ההערכה (TOE) כולל את קוד ספריית Android SDK (`java/iroha_android/src/main/java`), משטח התצורה שלו (`ClientConfig` + Norito הטמעה), ואת הכלים התפעוליים המוזכרים ב-`roadmap.md` עבור `roadmap.md` עבור AND2.

רכיבים ראשוניים:

1. **הטמעת תצורה** — שרשורי `ClientConfig` Torii נקודות קצה, מדיניות TLS, ניסיונות חוזרים וטלמטריה מהמניפסט `iroha_config` שנוצר ואוכפים אי-שינוי שלאחר האתחול (Sigstore00).
2. **ניהול מפתחות / StrongBox** — חתימה מגובה חומרה מיושמת באמצעות `SystemAndroidKeystoreBackend` ו-`AttestationVerifier`, עם מדיניות מתועדת ב-`docs/source/sdk/android/key_management.md`. לכידת/אימות אישור משתמש ב-`scripts/android_keystore_attestation.sh` וב-CI helper `scripts/android_strongbox_attestation_ci.sh`.
3. **טלמטריה ועיבוד** - מכשור עובר דרך הסכימה המשותפת המתוארת ב-`docs/source/sdk/android/telemetry_redaction.md`, ייצוא סמכויות גיבוב, פרופילי התקנים ב-bucketed ועקוף ווים לביקורת שנאכפו על ידי ה- Support Playbook.
4. **פנקסי פעולות הפעלה** - `docs/source/android_runbook.md` (תגובת מפעיל) ו-`docs/source/android_support_playbook.md` (SLA + הסלמה) מקשיחים את טביעת הרגל התפעולית של ה-TOE עם עקיפות דטרמיניסטיות, תרגילי כאוס ולכידת ראיות.
5. **מקור שחרור** - בנייה מבוססת Gradle משתמשת בתוסף CycloneDX בתוספת דגלי בנייה שניתנים לשחזור כפי שנלכדו ב-`docs/source/sdk/android/developer_experience_plan.md` וברשימת הבדיקה של תאימות AND6. חפצי שחרור חתומים ומוצלבים ב-`docs/source/release/provenance/android/`.

## 2. נכסים והנחות

| נכס | תיאור | מטרת אבטחה |
|-------|-------------|------------------------|
| מניפסטים של תצורה | תצלומי מצב Norito שמקורם ב-`ClientConfig` המופצים עם אפליקציות. | אותנטיות, יושרה וסודיות במנוחה. |
| חתימה על מפתחות | מפתחות שנוצרו או מיובאים דרך ספקי StrongBox/TEE. | העדפת StrongBox, רישום אישורים, ללא ייצוא מפתח. |
| זרמי טלמטריה | עקבות/יומנים/מדדים של OTLP מיוצאים ממכשור SDK. | פסאודונימיזציה (סמכויות גיבוב), ממוזער PII, עוקף ביקורת. |
| אינטראקציות פנקס | עומסי Norito, מטא נתונים של כניסה, Torii תעבורת רשת. | אימות הדדי, בקשות עמידות בשידור חוזר, ניסיונות חוזרים דטרמיניסטיים. |

הנחות:

- מערכת הפעלה ניידת מספקת ארגז חול סטנדרטי + SELinux; מכשירי StrongBox מיישמים את ממשק keymaster של גוגל.
- מפעילים מספקים נקודות קצה Torii עם תעודות TLS החתומות על ידי רשויות CA מהימנות.
- בניית תשתית מכבדת את דרישות הבנייה הניתנות לשחזור לפני הפרסום ב-Maven.

## 3. איומים ובקרות| איום | שליטה | עדות |
|--------|--------|--------|
| מניפסטים של תצורה משותפת | `ClientConfig` מאמת מניפסטים (hash + סכימה) לפני היישום ויומן דחיית טעינות מחדש באמצעות `android.telemetry.config.reload`. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| פשרה של חתימה על מפתחות | מדיניות נדרשת של StrongBox, רתמות אישורים וביקורות מטריצות מכשיר מזהות סחיפה; עוקפים מתועדים לכל אירוע. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| דליפת PII בטלמטריה | רשויות גיבוב של Blake2b, פרופילי מכשירים עם דליים, השמטת ספק, ביטול רישום. | `docs/source/sdk/android/telemetry_redaction.md`; תמיכה ב-Playbook §8. |
| הפעל מחדש או שדרוג לאחור ב-Torii RPC | בונה הבקשות `/v1/pipeline` אוכף הצמדת TLS, מדיניות ערוצי רעש ונסה שוב תקציבים עם הקשר סמכות גיבוב. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (מתוכנן). |
| מהדורות לא חתומות או לא ניתנות לשחזור | CycloneDX SBOM + Sigstore אישורים מסודרים על ידי רשימת AND6; RFCs לשחרור דורשים ראיות ב-`docs/source/release/provenance/android/`. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| טיפול לא שלם באירוע | ספר הפעלה + ספר הפעלה מגדירים דרישות, תרגילי כאוס ועץ הסלמה; עקיפות טלמטריה דורשות בקשות חתומות של Norito. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. פעילויות הערכה

1. **סקירת עיצוב** — תאימות + SRE מוודא שתצורה, ניהול מפתחות, טלמטריה ובקרות שחרור ממפות את יעדי האבטחה של ETSI.
2. **בדיקות יישום** — בדיקות אוטומטיות:
   - `scripts/android_strongbox_attestation_ci.sh` מאמת חבילות שנלכדו עבור כל מכשיר StrongBox הרשום במטריצה.
   - `scripts/check_android_samples.sh` ו-Managed Device CI מבטיחים שאפליקציות לדוגמה מכבדות חוזי `ClientConfig`/טלמטריה.
3. **אימות תפעולי** - תרגילי כאוס רבעוניים לכל `docs/source/sdk/android/telemetry_chaos_checklist.md` (עריכה + תרגילי עקיפה).
4. **שימור ראיות** - חפצי אמנות המאוחסנים תחת `docs/source/compliance/android/` (תיקיה זו) והפנייה מ-`status.md`.

## 5. מיפוי ETSI EN 319 401| EN 319 401 סעיף | בקרת SDK |
|------------------------|--------|
| 7.1 מדיניות אבטחה | מתועד ביעד אבטחה זה + ספר תמיכה. |
| 7.2 אבטחה ארגונית | RACI + בעלות כוננית ב- Support Playbook §2. |
| 7.3 ניהול נכסים | יעדי נכסי תצורה, מפתח וטלמטריה שהוגדרו ב-§2 לעיל. |
| 7.4 בקרת גישה | מדיניות StrongBox + עקיפה של זרימת עבודה המחייבת חפצי Norito חתומים. |
| 7.5 בקרות קריפטוגרפיות | דרישות יצירת מפתחות, אחסון ואישור מ-AND2 (מדריך לניהול מפתחות). |
| 7.6 אבטחת תפעול | גיבוב טלמטריה, חזרות כאוס, תגובה לאירועים ושחרור עדויות. |
| 7.7 אבטחת תקשורת | `/v1/pipeline` מדיניות TLS + רשויות גיבוב (מסמך עריכת טלמטריה). |
| 7.8 רכישת / פיתוח מערכת | בניית Gradle, SBOM ושערי מוצא ניתנים לשחזור בתוכניות AND5/AND6. |
| 7.9 קשרי ספקים | אישורי Buildkite + Sigstore נרשמו לצד SBOM של צד שלישי. |
| 7.10 ניהול אירועים | הסלמה של ספר הפעלה/Playbook, עקיפת רישום, מוני כישלון טלמטריה. |

## 6. תחזוקה

- עדכן מסמך זה בכל פעם שה-SDK מציג אלגוריתמים קריפטוגרפיים חדשים, קטגוריות טלמטריה או שינויים באוטומציה לשחרור.
- קשר עותקים חתומים ב-`docs/source/compliance/android/evidence_log.csv` עם תקצירי SHA-256 וחתימות מבקרים.