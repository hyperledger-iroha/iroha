---
lang: he
direction: rtl
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T17:01:56.615899+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# אנדרואיד SDK Operations Runbook

ספר הפעלה זה תומך במפעילים ובמהנדסי תמיכה המנהלים SDK של Android
פריסות עבור AND7 ואילך. התאם ל-Android Support Playbook עבור SLA
הגדרות ומסלולי הסלמה.

> **הערה:** בעת עדכון נהלי אירוע, רענן גם את המשותף
> מטריצת פתרון בעיות (`docs/source/sdk/android/troubleshooting.md`) כך
> טבלת תרחישים, SLAs והפניות לטלמטריה נשארים מיושרים עם ספר ההפעלה הזה.

## 0. התחלה מהירה (כאשר מפעילים בימונים)

שמור את הרצף הזה בהישג יד עבור התראות Sev1/Sev2 לפני שאתה צולל לתוך המפורט
סעיפים להלן:

1. **אשר את התצורה הפעילה:** לכידת את סכום הבדיקה של המניפסט `ClientConfig`
   הנפלט בעת הפעלת האפליקציה והשווה אותו למניפסט המוצמד
   `configs/android_client_manifest.json`. אם ה-hashs מתפצלים, עצור שחרורים ו
   הגש כרטיס סחיפה של תצורה לפני נגיעה בטלמטריה/עקיפות (ראה §1).
2. **הפעל את שער הבדל של הסכימה:** הפעל את `telemetry-schema-diff` CLI נגד
   תמונת המצב המקובלת
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   התייחס לכל פלט `policy_violations` כ-Sev2 וחסום יצוא עד ל
   אי התאמה מובנת (ראה §2.6).
3. **בדוק לוחות מחוונים + מצב CLI:** פתח את ה-Android Telemetry Redaction ו
   לוחות בריאות היצואן, ואז הפעל
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. אם
   הרשויות נמצאות מתחת לרצפה או שגיאה בייצוא, לצלם צילומי מסך ו
   פלט CLI עבור מסמך האירוע (ראה §2.4–§2.5).
4. **החליטו על עקיפות:** רק לאחר השלבים הנ"ל ועם אירוע/בעלים
   נרשמו, תוציאו דריסה מוגבלת דרך `scripts/android_override_tool.sh`
   והכנס אותו ל-`telemetry_override_log.md` (ראה §3). תוקף ברירת המחדל: <24h.
5. **הסלמה לכל רשימת אנשי קשר:** עמוד ב-Android on-call ו- Observability TL
   (אנשי קשר ב-§8), ולאחר מכן עקוב אחר עץ ההסלמה ב-§4.1. אם אישור או
   אותות StrongBox מעורבים, משוך את החבילה האחרונה והפעל את הרתמה
   בדיקות מ-§7 לפני הפעלת הייצוא מחדש.

## 1. תצורה ופריסה

- **מיקור ClientConfig:** ודא שלקוחות אנדרואיד יטענו נקודת קצה Torii, TLS
  מדיניות, ונסה שוב כפתורים ממניפסטים שמקורם ב-`iroha_config`. אימות
  ערכים במהלך הפעלת האפליקציה וסיכום בדיקת יומן של המניפסט הפעיל.
  התייחסות ליישום: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  חוטים `TelemetryOptions` מ-`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (בתוספת ה-`TelemetryObserver` שנוצר) כך שרשויות גיבוב נפלטות אוטומטית.
- **טעינה חוזרת חמה:** השתמש בשומר התצורה כדי לאסוף את `iroha_config`
  עדכונים ללא הפעלה מחדש של האפליקציה. טעינות מחדש שנכשלו אמורות לפלוט את
  אירוע `android.telemetry.config.reload` והפעל ניסיון חוזר עם אקספוננציאלי
  גיבוי (מקסימום 5 ניסיונות).
- **התנהגות החזרה:** כאשר התצורה חסרה או לא חוקית, חזור אל
  ברירות מחדל בטוחות (מצב קריאה בלבד, ללא הגשת תור ממתינה) ומציאת משתמש
  הנחיה. רשום את האירוע למעקב.

### 1.1 אבחון טעינה מחדש של הגדרות- ה-config watcher פולט אותות `android.telemetry.config.reload` עם
  שדות `source`, `result`, `duration_ms` ושדות `digest`/`error` אופציונליים (ראה
  `configs/android_telemetry.json` ו
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  צפו לאירוע `result:"success"` בודד לכל מניפסט מוחל; חזר על עצמו
  רישומי `result:"error"` מצביעים על כך שהצופה מיצה את 5 ניסיונות הגיבוי שלו
  החל מ-50ms.
- במהלך תקרית, לכוד את אות הטעינה מחדש האחרון מהאספן
  (חנות OTLP/span או נקודת הקצה של סטטוס העריכה) ורישום את `digest` +
  `source` במסמך האירוע. השווה את העיכול ל
  `configs/android_client_manifest.json` ומניפסט ההפצה שהופץ אל
  מפעילים.
- אם הצופה ממשיך לפלוט שגיאות, הפעל את הרתמה הממוקדת כדי להתרבות
  כישלון הניתוח עם המניפסט החשוד:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  צרף את פלט הבדיקה ואת המניפסט הכושל לחבילת האירועים כך SRE
  יכול להבדיל את זה מול סכימת התצורה האפויה.
- כאשר חסרה טלמטריית טעינה מחדש, ודא שה-`ClientConfig` הפעיל נושא
  כיור טלמטריה וכי אספן OTLP עדיין מקבל את
  `android.telemetry.config.reload` מזהה; אחרת התייחס לזה כטלמטריה של Sev2
  רגרסיה (זהה נתיב כמו §2.4) ושחרור הפסקה עד שהאות חוזר.

### 1.2 חבילות ייצוא מפתחות דטרמיניסטיות
- ייצוא תוכנה פולט כעת חבילות v3 עם מלח לכל יצוא + nonce, `kdf_kind` ו-`kdf_work_factor`.
  היצואן מעדיף את Argon2id (64 MiB, 3 איטרציות, מקביליות = 2) ונופל בחזרה ל
  PBKDF2-HMAC-SHA256 עם רצפת איטרציה של 350 קילו כאשר Argon2id אינו זמין במכשיר. צרור
  AAD עדיין נקשר לכינוי; ביטויי סיסמה חייבים להיות לפחות 12 תווים עבור ייצוא v3 וה-
  היבואן דוחה כל אפס זרעי מלח/nonce.
  `KeyExportBundle.decode(Base64|bytes)`, ייבא עם ביטוי הסיסמה המקורי וייצא מחדש ל-v3 ל
  עבור לפורמט הקשה לזיכרון. היבואן דוחה צמדי מלח/לא-נוס בשימוש חוזר או אפס; תמיד
  סובב חבילות במקום לעשות שימוש חוזר בייצוא ישן בין מכשירים.
- בדיקות נתיב שלילי ב-`ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests`
  דחייה. נקה מערכי תווים של ביטויי סיסמה לאחר השימוש וללכוד גם את גרסת החבילה וגם את `kdf_kind`
  בהערות אירועים כאשר השחזור נכשל.

## 2. טלמטריה ועיבוד

> הפניה מהירה: ראה
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> עבור רשימת הפקודה/הסף המעובה המשמשת במהלך ההפעלה
> הפעלות וגשרי אירועים.- **מלאי אותות:** עיין ב-`docs/source/sdk/android/telemetry_redaction.md`
  לרשימה המלאה של טווחים/מדדים/אירועים הנפלטים ו
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  לפרטי בעלים/אימות ופערים בולטים.
- **הבדל בסכימה קנונית:** תמונת המצב המאושרת של AND7 היא
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  יש להשוות כל ריצת CLI חדשה מול חפץ זה כדי שהבודקים יוכלו לראות
  שה-`intentional_differences` ו-`android_only_signals` המקובלים עדיין
  להתאים את טבלאות המדיניות המתועדות ב
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. ה-CLI מוסיף כעת
  `policy_violations` כאשר כל הבדל מכוון חסר א
  `status:"accepted"`/`"policy_allowlisted"` (או כאשר רשומות אנדרואיד בלבד מאבדות
  הסטטוס המקובל שלהם), לכן התייחס להפרות שאינן ריקות כאל Sev2 והפסק
  יצוא. קטעי `jq` למטה נשארים כבדיקת שפיות ידנית בארכיון
  חפצי אמנות:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  התייחס לכל פלט מפקודות אלה כאל רגרסיית סכימה שזקוק ל-
  באג מוכנות AND7 לפני המשך ייצוא הטלמטריה; `field_mismatches`
  חייב להישאר ריק לפי `telemetry_schema_diff.md` §5. העוזר כותב כעת
  `artifacts/android/telemetry/schema_diff.prom` באופן אוטומטי; לעבור
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (או סט
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) כאשר הוא פועל על מארחי בימוי/הפקה
  אז מד `telemetry_schema_diff_run_status` מתהפך ל-`policy_violation`
  באופן אוטומטי אם ה-CLI מזהה סחיפה.
- **מסייע CLI:** `scripts/telemetry/check_redaction_status.py` בודק
  `artifacts/android/telemetry/status.json` כברירת מחדל; להעביר את `--status-url` ל
  בימוי שאילתות ו-`--write-cache` כדי לרענן את העותק המקומי למצב לא מקוון
  תרגילים. השתמש ב-`--min-hashed 214` (או בסט
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) כדי לאכוף את הממשל
  קומה על רשויות גיבוב במהלך כל סקר סטטוס.
- **גיבוב של רשות:** כל הרשויות עוברות גיבוב באמצעות Blake2b-256 עם
  מלח סיבוב רבעוני מאוחסן בכספת הסודות המאובטחת. סיבובים מתרחשים על
  ביום שני הראשון של כל רבעון בשעה 00:00 UTC. ודא שהיצואן אוסף
  המלח החדש על ידי בדיקת המדד `android.telemetry.redaction.salt_version`.
- **דליים של פרופיל מכשיר:** רק `emulator`, `consumer` ו-`enterprise`
  שכבות מיוצאות (לצד הגרסה העיקרית של SDK). לוחות מחוונים משווים את אלה
  סופר נגד קווי הבסיס של חלודה; שונות מעל 10% מעלה התראות.
- **מטא נתונים של הרשת:** אנדרואיד מייצאת דגלי `network_type` ו-`roaming` בלבד.
  שמות ספקים לעולם אינם נפלטים; מפעילים לא צריכים לבקש מנוי
  מידע ביומני אירועים. תמונת המצב המחוברת נפלטת כ-
  אירוע `android.telemetry.network_context`, אז ודא שאפליקציות ירשמו א
  `NetworkContextProvider` (או דרך
  `ClientConfig.Builder.setNetworkContextProvider(...)` או הנוחות
  `enableAndroidNetworkContext(...)` עוזר) לפני הוצאת קריאות Torii.
- **מצביע Grafana:** לוח המחוונים `Android Telemetry Redaction` הוא
  בדיקה חזותית קנונית עבור פלט CLI למעלה - אשר את
  לוח `android.telemetry.redaction.salt_version` מתאים לתקופת המלח הנוכחית
  והווידג'ט `android_telemetry_override_tokens_active` נשאר באפס
  בכל פעם שלא פועלים תרגילים או תקריות. הסלמה אם אחד הפאנלים נסחפים
  לפני שסקריפטי ה-CLI מדווחים על רגרסיה.

### 2.1 זרימת עבודה של צינור ייצוא1. **הפצת תצורה.** `ClientConfig.telemetry.redaction` מושחל מ
   `iroha_config` ונטען מחדש על ידי `ConfigWatcher`. כל טעינה מחדש מתעדת את
   עידן הגילוי בתוספת מלח - לכיד את הקו הזה בתקריות ובמהלך
   חזרות.
2. **מכשירים.** רכיבי SDK פולטים טווחים/ערכים/אירועים לתוך
   `TelemetryBuffer`. המאגר מתייג כל מטען עם פרופיל המכשיר ו
   תקופת המלח הנוכחית כך שהיצואנית יכולה לאמת תשומות גיבוב באופן דטרמיניסטי.
3. **מסנן רדוקציה.** `RedactionFilter` גיבובים `authority`, `alias`, ו
   מזהי מכשיר לפני שהם עוזבים את המכשיר. כישלונות פולטים
   `android.telemetry.redaction.failure` וחסום את ניסיון הייצוא.
4. **יצואן + אספן.** מטענים מחוטאים נשלחים דרך האנדרואיד
   יצואנית OpenTelemetry לפריסת `android-otel-collector`. ה
   יציאות מאווררי אספן לעקבות (טמפו), מדדים (Prometheus) ו-Norito
   כיורי עץ.
5. **ווי צפייה.** קריאות `scripts/telemetry/check_redaction_status.py`
   מוני אספנים (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) ומייצר את חבילת הסטטוס
   הפניה לאורך ספר ההפעלה הזה.

### 2.2 שערי אימות

- **הבדל סכימה:** הפעלה
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  בכל פעם שמתבטא שינוי. לאחר כל ריצה, אשר כל
  הערך `intentional_differences[*]` ו-`android_only_signals[*]` מוטבע
  `status:"accepted"` (או `status:"policy_allowlisted"` עבור hashed/bucketed
  שדות) כפי שהומלץ ב-`telemetry_schema_diff.md` §3 לפני חיבור ה-
  חפץ לאירועים ודוחות מעבדת כאוס. השתמש בתמונת המצב המאושרת
  (`android_vs_rust-20260305.json`) כמעקה בטיחות ומוך את הנפלטים הטריים
  JSON לפני שהוא מתויק:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  השווה בין `$LATEST`
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  כדי להוכיח שרשימת ההיתרים נותרה ללא שינוי. חסר או ריק `status`
  ערכים (לדוגמה ב-`android.telemetry.redaction.failure` או
  `android.telemetry.redaction.salt_version`) מטופלים כעת כאל רגרסיות ו
  יש לפתור לפני שהסקירה יכולה להיסגר; ה-CLI משטח את המקובל
  ציין ישירות, כך שההפניה המוצלבת של §3.4 ידנית חלה רק כאשר
  הסבר מדוע מופיע סטטוס שאינו `accepted`.

  **אותות AND7 קנוניים (תמונת מצב של 2026-03-05)**| אות | ערוץ | סטטוס | הערת ממשל | וו אימות |
  |--------|--------|--------|----------------|----------------|
  | `android.telemetry.redaction.override` | אירוע | `accepted` | מראות עוקפות מניפסטים וחייבות להתאים לערכי `telemetry_override_log.md`. | צפה ב-`android_telemetry_override_tokens_active` וארכיון מניפסטים לפי §3. |
  | `android.telemetry.network_context` | אירוע | `accepted` | אנדרואיד מבטל בכוונה את שמות הספקים; רק `network_type` ו-`roaming` מיוצאים. | ודא שאפליקציות רושמות `NetworkContextProvider` ומאשרות שנפח האירועים עוקב אחר תעבורת Torii ב-`Android Telemetry Overview`. |
  | `android.telemetry.redaction.failure` | מונה | `accepted` | פולט בכל פעם ש-hashing נכשל; ממשל דורש כעת מטא-נתונים מפורשים של סטטוס ב- schema diff artefact. | לוח המחוונים של `Redaction Compliance` ופלט CLI מ-`check_redaction_status.py` חייבים להישאר באפס למעט במהלך תרגילים. |
  | `android.telemetry.redaction.salt_version` | מד | `accepted` | מוכיח שהיצואן משתמש בתקופת המלח הרבעונית הנוכחית. | השווה את יישומון המלח של Grafana עם עידן הכספת של סודות והבטח שהריצות של הבדלים בסכימה ישמרו על ההערה `status:"accepted"`. |

  אם ערך כלשהו בטבלה שלמעלה מפיל `status`, החפץ ההבדל חייב להיות
  מחדשים **ו** `telemetry_schema_diff.md` עודכנו לפני ה-AND7
  חבילת ממשל מופצת. כלול את ה-JSON המרענן ב
  `docs/source/sdk/android/readiness/schema_diffs/` וקשר אותו מה-
  אירוע, מעבדת כאוס או דוח הפעלה שהפעילו את ההרצה החוזרת.
- **כיסוי CI/יחידה:** `ci/run_android_tests.sh` חייב לעבור לפני
  הוצאה לאור בונה; החבילה אוכפת התנהגות גיבוב/עקיפה על ידי אימון
  יצואניות הטלמטריה עם מטענים לדוגמה.
- **בדיקות שפיות מזרק:** שימוש
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` לקראת החזרות
  כדי לאשר כישלון הזרקת עובד וזה מתריע על אש בעת גיבוש שומרים
  מועדים. נקה תמיד את המזרק עם `--clear` לאחר אימות
  משלים.

### 2.3 נייד ↔ רשימת בדיקה לשוויון טלמטריית חלודה

שמור על יצואניות אנדרואיד ושירותי צומת Rust מיושרים תוך כיבוד
דרישות עריכה שונות המתועדות ב
`docs/source/sdk/android/telemetry_redaction.md`. הטבלה שלהלן משמשת כ-
רשימת ההיתרים הכפולה המוזכרת בערך מפת הדרכים של AND7 - עדכן אותה בכל פעם
schema diff מציג או מסיר שדות.| קטגוריה | יצואניות אנדרואיד | שירותי חלודה | וו אימות |
|--------|----------------|----------------|----------------|
| הקשר סמכות / מסלול | Hash `authority`/`alias` דרך Blake2b-256 ושחרר שמות מארח גולמיים Torii לפני הייצוא; פולט `android.telemetry.redaction.salt_version` כדי להוכיח סיבוב מלח. | שלח שמות מארחים מלאים Torii ומזהי עמיתים לצורך מתאם. | השווה ערכי `android.torii.http.request` לעומת `torii.http.request` בהבדל הסכימה העדכני ביותר תחת `readiness/schema_diffs/`, ולאחר מכן אשר את `android.telemetry.redaction.salt_version` תואם למלח האשכול על ידי הפעלת `scripts/telemetry/check_redaction_status.py`. |
| זהות מכשיר וחותם | דלי `hardware_tier`/`device_profile`, כינויים של בקר גיבוב, ולעולם אל ייצא מספרים סידוריים. | אין מטא נתונים של המכשיר; הצמתים פולטים אימות `peer_id` והבקר `public_key` מילה במילה. | שיקוף את המיפויים ב-`docs/source/sdk/mobile_device_profile_alignment.md`, בדוק את פלטי `PendingQueueInspector` במהלך מעבדות, וודא שמבחני הגיבוב הכינוי בתוך `ci/run_android_tests.sh` נשארים ירוקים. |
| מטא נתונים של רשת | ייצא רק `network_type` + `roaming` בוליאנים; `carrier_name` נשמט. | Rust שומרת על שמות מארח עמיתים בתוספת מטא נתונים מלאים של נקודות קצה TLS. | אחסן את ה-JSON ההבדל העדכני ביותר ב-`readiness/schema_diffs/` ואשר שצד האנדרואיד עדיין משמיט את `carrier_name`. התראה אם ​​הווידג'ט "הקשר רשת" של Grafana מציג מחרוזות של ספקים. |
| עדות לעקוף / כאוס | פליט אירועים `android.telemetry.redaction.override` ו-`android.telemetry.chaos.scenario` עם תפקידי שחקן רעולי פנים. | שירותי חלודה פולטים אישורי עקיפה ללא מיסוך תפקיד וללא טווחים ספציפיים לכאוס. | בדוק את `docs/source/sdk/android/readiness/and7_operator_enablement.md` לאחר כל תרגיל כדי לוודא שאסימוני ביטול וחפצי תוהו ובוהו מאוחסנים בארכיון לצד אירועי חלודה חשופי המסכה. |

זרימת עבודה זוגית:

1. לאחר כל שינוי מניפסט או ביצואן, הפעל
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   אז חפץ JSON ומדדי השיקוף נוחתים שניהם בחבילת הראיות
   (העוזר עדיין כותב `artifacts/android/telemetry/schema_diff.prom` כברירת מחדל).
2. סקור את ההבדל מול הטבלה לעיל; אם אנדרואיד עכשיו פולט שדה כלומר
   מותר רק ב- Rust (או להיפך), הגישו באג מוכנות AND7 ועדכן
   תוכנית העריכה.
3. במהלך בדיקות שבועיות, הפעל
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   כדי לאשר תקופות מלח להתאים את הווידג'ט Grafana ולציין את העידן ב-
   יומן כוננות.
4. רשמו דלתות כלשהן
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` כך
   ממשל יכול לבקר החלטות זוגיות.

### 2.4 לוחות מחוונים של צפיות וספי התראות

שמור את לוחות המחוונים וההתראות מיושרים עם אישורי הסכימה של AND7 כאשר
סקירת פלט `scripts/telemetry/check_redaction_status.py`:

- `Android Telemetry Redaction` - ווידג'ט של עידן מלח, עוקף מד אסימון.
- `Redaction Compliance` — מונה `android.telemetry.redaction.failure` ו
  לוחות מגמת מזרק.
- `Exporter Health` — `android.telemetry.export.status` התמוטטויות קצב.
- `Android Telemetry Overview` — דליים של פרופיל המכשיר ונפח ההקשר של הרשת.

הספים הבאים משקפים את כרטיס ההפניה המהירה ויש לאכוף אותם
במהלך תגובה לאירוע וחזרות:| מדד / פאנל | סף | פעולה |
|----------------|--------|--------|
| `android.telemetry.redaction.failure` (לוח `Redaction Compliance`) | >0 מעל חלון מתגלגל של 15 דקות | בדוק את האות הכושל, הפעל ניקוי מזרק, רשם פלט CLI + צילום מסך Grafana. |
| `android.telemetry.redaction.salt_version` (לוח `Android Telemetry Redaction`) | שונה מתקופת המלח סודות-כספת | עצירת שחרורים, תיאום עם סיבוב סודות, קובץ AND7 הערה. |
| `android.telemetry.export.status{status="error"}` (לוח `Exporter Health`) | >1% מהיצוא | בדוק את תקינות האספן, לכוד אבחון CLI, הסלמה ל-SRE. |
| `android.telemetry.device_profile{tier="enterprise"}` לעומת שוויון חלודה (`Android Telemetry Overview`) | שונות >10% מקו הבסיס של חלודה | מעקב אחר ניהול קבצים, אימות מאגרי רכיבים, הערות שונות של סכימה. |
| נפח `android.telemetry.network_context` (`Android Telemetry Overview`) | יורד לאפס בזמן שקיימת תעבורת Torii | אשר את הרישום של `NetworkContextProvider`, הפעל מחדש את הסכימה הבדל כדי להבטיח שהשדות לא השתנו. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | חלון עקיפה/קידוח חיצוני לא אפס | קשר אסימון לאירוע, צור תקציר מחדש, בטל באמצעות זרימת עבודה ב-§3. |

### 2.5 מסלול מוכנות והפעלה של המפעיל

פריט מפת הדרכים AND7 קורא לתכנית לימודים ייעודית למפעילים, כך שתומכים, SRE, ו
בעלי עניין בשחרור מבינים את טבלאות הזוגיות שלמעלה לפני שספר ההפעלה יוצא
GA. השתמש בקו המתאר ב
`docs/source/sdk/android/telemetry_readiness_outline.md` ללוגיסטיקה קנונית
(סדר יום, מציגים, ציר זמן) ו-`docs/source/sdk/android/readiness/and7_operator_enablement.md`
לרשימת הבדיקה המפורטת, קישורי הוכחות ויומן פעולות. שמור את הדברים הבאים
שלבים מסונכרנים בכל פעם שתוכנית הטלמטריה משתנה:| שלב | תיאור | צרור ראיות | בעלים ראשיים |
|-------|-------------|----------------|--------|
| הפצה קריאה מוקדמת | שלח את המדיניות שנקראה מראש, `telemetry_redaction.md`, ואת כרטיס ההפניה המהיר לפחות חמישה ימי עסקים לפני התדריך. עקוב אחר אישורים ביומן ההתקשרות של המתאר. | `docs/source/sdk/android/telemetry_readiness_outline.md` (לוגיסטיקת הפעלות + יומן תקשורת) והאימייל הארכיון ב-`docs/source/sdk/android/readiness/archive/<YYYY-MM>/`. | מנהל מסמכים/תמיכה |
| מפגש מוכנות חי | העבירו את ההדרכה של 60 דקות (צלילת מדיניות עמוקה, הדרכה על ספרי ריצה, לוחות מחוונים, הדגמה של מעבדת כאוס) והמשיכו לפעול בהקלטה עבור צופים אסינכרוניים. | הקלטה + שקופיות מאוחסנות תחת `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` עם הפניות שנלכדו ב-§2 של המתווה. | LLM (ממלא מקום הבעלים של AND7) |
| ביצוע מעבדת כאוס | הפעל לפחות C2 (עקיפה) + C6 (השמעה חוזרת בתור) מ-`docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` מיד לאחר ההפעלה החיה וצרף יומנים/צילומי מסך לערכת ההפעלה. | דוחות תרחיש וצילומי מסך בתוך `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` ו-`/screenshots/<YYYY-MM>/`. | Android Observability TL + SRE כוננות |
| בדיקת ידע ונוכחות | אסוף הגשות לחידון, תקן כל מי שצבר פחות מ-90% ורשום סטטיסטיקות נוכחות/חידון. שמרו על שאלות ההפניה המהירה בהתאם לרשימת התיוג השוויונית. | חידון ייצוא ב-`docs/source/sdk/android/readiness/forms/responses/`, סיכום Markdown/JSON שהופק באמצעות `scripts/telemetry/generate_and7_quiz_summary.py`, וטבלת הנוכחות בתוך `and7_operator_enablement.md`. | הנדסת תמיכה |
| ארכיון ומעקבים | עדכן את יומן הפעולות של ערכת ההפעלה, העלה חפצים לארכיון וציין את ההשלמה ב-`status.md`. כל אסימוני תיקון או עקיפה שהונפקו במהלך ההפעלה חייבים להיות מועתקים ל-`telemetry_override_log.md`. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (יומן פעולות), `.../archive/<YYYY-MM>/checklist.md`, ויומן העקיפה המוזכר ב-§3. | LLM (ממלא מקום הבעלים של AND7) |

כאשר תוכנית הלימודים מוצגת מחדש (ברבעון או לפני שינויים בסכימה העיקרית), רענן
המתווה עם תאריך המפגש החדש, שמרו על רשימת המשתתפים עדכנית, וכן
צור מחדש את סיכום החידון חפצי JSON/Markdown כדי שמנות ניהול יוכלו
התייחסות לראיות עקביות. הערך `status.md` עבור AND7 צריך לקשר ל-
תיקיית הארכיון האחרונה לאחר סגירת כל ספרינט הפעלה.

### 2.6 רשימות היתרים של סכימה ובדיקות מדיניות

מפת הדרכים קוראת במפורש מדיניות כפולה של רשימת הרשאות (עריכות בנייד לעומת
שמירת חלודה) שנאכפת על ידי `telemetry-schema-diff` CLI שנמצא תחת
`tools/telemetry-schema-diff`. כל חפץ שונה מתועד ב
`docs/source/sdk/android/readiness/schema_diffs/` חייב לתעד אילו שדות הם
hashed/bucketed באנדרואיד, אילו שדות נשארים ללא hash ב-Rust, והאם
כל אות שאינו ברשימת ההיתרים החליק לתוך המבנה. תפוס את ההחלטות האלה
ישירות ב-JSON על ידי הפעלת:

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```ה-`jq` הסופי מוערך ללא הפעלה כאשר הדוח נקי. לטפל בכל פלט
מהפקודה הזו בתור באג מוכנות של Sev2: `policy_violations` מאוכלס
מערך פירושו שה-CLI גילה אות שאינו נמצא ברשימת אנדרואיד בלבד
ולא ברשימת הפטור לחלודה בלבד המתועדת ב
`docs/source/sdk/android/telemetry_schema_diff.md`. כשזה קורה, עצור
מייצא, הגש כרטיס AND7 והפעל מחדש את ה-diff רק לאחר מודול המדיניות
ותמונות מניפסט תוקנו. אחסן את ה-JSON שנוצר ב
`docs/source/sdk/android/readiness/schema_diffs/` עם סיומת התאריך והערה
הנתיב בתוך האירוע או דוח המעבדה כדי שהממשל יוכל להפעיל מחדש את הבדיקות.

**מטריצת גיבוב ושימור**

| Signal.field | טיפול באנדרואיד | טיפול בחלודה | תג רשימת ההיתרים |
|--------------|----------------|----------------------------|
| `torii.http.request.authority` | Blake2b-256 hashed (`representation: "blake2b_256"`) | מאוחסן מילה במילה לעקיבות | `policy_allowlisted` (hash נייד) |
| `attestation.result.alias` | Blake2b-256 hashed | כינוי טקסט רגיל (ארכיון אישורים) | `policy_allowlisted` |
| `attestation.result.device_tier` | דלי (`representation: "bucketed"`) | מחרוזת שכבה פשוטה | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | נעדר - יצואניות אנדרואיד שומטות את התחום לחלוטין | הצג ללא עריכה | `rust_only` (מתועד ב-§3 של `telemetry_schema_diff.md`) |
| `android.telemetry.redaction.override.*` | אות לאנדרואיד בלבד עם תפקידי שחקן רעולי פנים | לא נפלט אות שווה ערך | `android_only` (חייב להישאר `status:"accepted"`) |

כאשר מופיעים אותות חדשים, הוסף אותם למודול המדיניות של schema diff **ו** את
טבלה למעלה, כך שה-Runbook משקף את היגיון האכיפה שנשלח ב-CLI.
ריצות סכימה נכשלות כעת אם אות כלשהו ל-Android בלבד משמיט `status` מפורש או אם
מערך `policy_violations` אינו ריק, אז שמור את רשימת הבדיקה הזו מסונכרנת עם
`telemetry_schema_diff.md` §3 ותצלומי ה-JSON העדכניים ביותר המוזכרים ב
`telemetry_redaction_minutes_*.md`.

## 3. לעקוף את זרימת העבודה

עקיפות הן אפשרות "לשבור זכוכית" בעת גיבוש רגרסיות או פרטיות
התראות חוסמות לקוחות. החל אותם רק לאחר הקלטת מסלול ההחלטה המלא
במסמך האירוע.1. **אשר סחיפה והיקף.** המתן להתראת PagerDuty או להבדל הסכימה
   שער לאש, ואז לרוץ
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` ל
   להוכיח רשויות לא תואמות. צרף את פלט CLI וצילומי מסך Grafana
   לתיעוד האירוע.
2. **הכן בקשה חתומה.** אכלוס
   `docs/examples/android_override_request.json` עם מזהה הכרטיס, המבקש,
   תפוגה, והצדקה. אחסן את הקובץ ליד חפצי האירוע כך
   תאימות יכולה לבקר את התשומות.
3. **הנפק את העקיפה.** Invoke
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   העוזר מדפיס את אסימון העקיפה, כותב את המניפסט ומוסיף שורה
   ליומן הביקורת של Markdown. לעולם אל תפרסם את האסימון בצ'אט; להעביר אותו ישירות
   לאופרטורים Torii המחילים את העקיפה.
4. **עקוב אחר האפקט.** תוך חמש דקות אמת סינגל
   אירוע `android.telemetry.redaction.override` נפלט, האספן
   נקודת הקצה המצב מציגה `override_active=true`, ומסמך האירוע מפרט את
   תפוגה. צפה בלוח המחוונים "עקוף אסימונים" של ה-Android Telemetry Overview
   פאנל "פעיל" (`android_telemetry_override_tokens_active`) עבור אותו
   ספירת אסימונים והמשך להפעיל את הסטטוס CLI כל 10 דקות עד
   הגיבוב מתייצב.
5. **בטל וארכיון.** ברגע שההקלה נוחתת, רוץ
  `scripts/android_override_tool.sh revoke --token <token>` אז יומן הביקורת
  לוכד את זמן הביטול ולאחר מכן בצע
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  כדי לרענן את תמונת המצב המחוברת שהממשל מצפה לה. צרף את
  מניפסט, תמלול JSON, CLI, תמונות Snapshot Grafana ויומן NDJSON
  מיוצר באמצעות `--event-log` ל
  `docs/source/sdk/android/readiness/screenshots/<date>/` וצלב את
  ערך מ-`docs/source/sdk/android/telemetry_override_log.md`.

עקיפות העוברות על 24 שעות דורשות אישור מנהל SRE וציות
חייב להיות מודגש בסקירת AND7 השבועית הבאה.

### 3.1 ביטול מטריצת הסלמה

| מצב | משך מקסימום | מאשרים | הודעות נדרשות |
|-----------|----------------|----------------|------------------------|
| חקירת דייר יחיד (אי-התאמה של סמכות גיבוב, לקוח Sev2) | 4 שעות | מהנדס תמיכה + SRE כוננות | כרטיס `SUP-OVR-<id>`, `android.telemetry.redaction.override` אירוע, יומן אירועים |
| הפסקת טלמטריה בכל צי או שכפול מבוקש של SRE | 24 שעות | SRE כוננות + מוביל תוכנית | הערת PagerDuty, ביטול רישום יומן, עדכון ב-`status.md` |
| בקשת ציות/זיהוי פלילי או כל מקרה העולה על 24 שעות | עד לביטול מפורש | מנהל SRE + מוביל ציות | רשימת תפוצה של ממשל, יומן ביטול, סטטוס שבועי AND7 |

#### אחריות תפקיד| תפקיד | אחריות | SLA / הערות |
|------|----------------|----------------|
| טלמטריית אנדרואיד בכוננות (מפקד תקרית) | זיהוי כונן, בצע את כלי העקיפה, רשום אישורים במסמך האירוע, וודא שהביטול מתרחש לפני התוקף. | אשר את PagerDuty תוך 5 דקות ותעד את ההתקדמות כל 15 דקות. |
| Android Observability TL (Haruka Yamamoto) | אמת את אות הסחף, אשר את מצב היצואן/אספן, וחתום על מניפסט העקיפה לפני מסירתו למפעילים. | הצטרף לגשר תוך 10 דקות; להאציל לבעלים של אשכול הבמה אם אינו זמין. |
| קשר SRE (ליאם אוקונור) | החל את המניפסט על אספנים, עקוב אחר הצטברות ותיאום עם הנדסת שחרור עבור התקנות בצד Torii. | רישום כל פעולה של `kubectl` בבקשת השינוי והדבק תמלילי פקודות במסמך האירוע. |
| ציות (סופיה מרטינס / דניאל פארק) | אשר דרישות ארוכות מ-30 דקות, אמת את שורת יומן הביקורת וייעץ לגבי הודעות רגולטור/לקוח. | פרסום אישור ב-`#compliance-alerts`; עבור אירועי הפקה, הגש הערת ציות לפני הוצאת העקיפה. |
| מנהל מסמכים/תמיכה (Priya Deshpande) | ארכיון מניפסטים/פלט CLI תחת `docs/source/sdk/android/readiness/…`, שמור על יומן העקיפה מסודר, ותזמן מעבדות מעקב אם צצו פערים. | מאשר שמירת ראיות (13 חודשים) ומתיק מעקבים AND7 לפני סגירת האירוע. |

הסלמה מיידית אם אסימון ביטול כלשהו מתקרב לתוקף שלו ללא א
תוכנית ביטול מתועדת.

## 4. תגובה לאירוע

- **התראות:** שירות PagerDuty `android-telemetry-primary` מכסה עריכה
  כשלים, הפסקות ביצואן וסחף דליים. אישור בתוך חלונות SLA
  (ראה ספר תמיכה).
- **אבחון:** הפעל את `scripts/telemetry/check_redaction_status.py` כדי לאסוף
  בריאות היצואן הנוכחית, התראות אחרונות ומדדי סמכות גיבובים. כלול
  פלט בציר הזמן של האירוע (`incident/YYYY-MM-DD-android-telemetry.md`).
- **לוחות מחוונים:** עקוב אחר תיקון הטלמטריה של אנדרואיד, הטלמטריה של אנדרואיד
  סקירה כללית, לוחות מחוונים של תאימות לשינויים ובריאות היצואן. לכידת
  צילומי מסך לרישומי תקריות והערות לכל גרסת מלח או ביטול
  סטיות סמליות לפני סגירת אירוע.
- **תיאום:** צור הנדסת שחרור בנושאי יצואנים, תאימות
  לשאלות עקיפה/PII, והובלת תוכנית לתקריות סב 1.

### 4.1 זרימת הסלמה

תקריות אנדרואיד נבדקות באמצעות אותן רמות חומרה כמו האנדרואיד
תמיכה ב-Playbook (§2.1). הטבלה שלהלן מסכמת את מי יש לדפדף וכיצד
במהירות כל מגיב צפוי להצטרף לגשר.| חומרה | השפעה | מגיב ראשי (≤5 דקות) | הסלמה משנית (≤10 דקות) | הודעות נוספות | הערות |
|--------|--------|------------------------|--------------------------------|------------------------|------|
| Sev1 | הפסקה מול לקוח, הפרת פרטיות או דליפת נתונים | טלמטריית אנדרואיד בשיחה (`android-telemetry-primary`) | Torii כוננות + מוביל תוכנית | ציות + ממשל SRE (`#sre-governance`), בעלי מקבץ ביניים (`#android-staging`) | התחל חדר מלחמה מיד ופתח מסמך משותף לרישום פקודות. |
| Sev2 | השפלה של הצי, ביטול שימוש לרעה או צבר ממושך של הפעלה חוזרת | טלמטריית אנדרואיד בשיחה | Android Foundations TL + Docs/Support Manager | מוביל תכנית, קשר הנדסת שחרור | הסלמה לתאימות אם העקיפות חורגות מ-24 שעות. |
| Sev3 | בעיה של דייר יחיד, חזרה במעבדה או התראה מייעצת | מהנדס תמיכה | אנדרואיד בכוננות (אופציונלי) | מסמכים/תמיכה למודעות | המר ל-Sev2 אם ההיקף מתרחב או אם מספר דיירים מושפעים. |

| חלון | פעולה | בעל/ים | עדות/הערות |
|--------|--------|--------|----------------|
| 0–5 דקות | הכירו את PagerDuty, הקצה מפקד תקרית (IC) וצור `incident/YYYY-MM-DD-android-telemetry.md`. שחרר את הקישור בתוספת סטטוס שורה אחת ב-`#android-sdk-support`. | SRE / מהנדס תמיכה בכוננות | צילום מסך של PagerDuty ack + קטע תקרית שבוצע לצד יומני תקריות אחרים. |
| 5–15 דקות | הפעל את `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` והדבק את הסיכום במסמך האירוע. Ping Android Observability TL (Haruka Yamamoto) ותמיכה מובילה (Priya Deshpande) למסירה בזמן אמת. | IC + Android Observability TL | צרף JSON פלט CLI, שים לב שכתובות האתרים של לוח המחוונים נפתחו וסמן מי הבעלים של אבחון. |
| 15–25 דקות | צור קשר עם בעלי מקבץ בימוי (Haruka Yamamoto לצפייה, ליאם אוקונור עבור SRE) כדי לשכפל ב-`android-telemetry-stg`. טעינת זרעים עם `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` ותלכד השלכות בתור מאמולטור Pixel + כדי לאשר שוויון סימפטומים. | בעלי אשכולות במה | העלה פלט מחוטא `pending.queue` + `PendingQueueInspector` לתיקיית האירוע. |
| 25–40 דקות | החלט על ביטולים, Torii מצערת, או StrongBox fallback. אם יש חשד לחשיפת PII או גיבוב לא דטרמיניסטי, עמוד על תאימות (סופיה מרטינס, דניאל פארק) דרך `#compliance-alerts` והודע לראש התוכנית באותו שרשור אירוע. | IC + תאימות + מוביל תוכנית | אסימוני עקיפה של קישורים, מניפסטים של Norito והערות אישור. |
| ≥40 דקות | ספק עדכוני סטטוס של 30 דקות (הערות PagerDuty + `#android-sdk-support`). תזמן את גשר חדר המלחמה אם הוא עדיין לא פעיל, תיעד את ה-ETA להפחתת זמן, וודא שהנדסת שחרור (Alexei Morozov) נמצאת במצב המתנה כדי לגלגל חפצי אספן/SDK. | IC | עדכונים עם חותמת זמן בתוספת יומני החלטות המאוחסנים בקובץ האירוע ומסוכמים ב-`status.md` במהלך הרענון השבועי הבא. |- כל ההסלמות חייבות להישאר שיקוף במסמך האירוע באמצעות טבלת "הבעלים / שעת העדכון הבא" מ-Android Support Playbook.
- אם אירוע אחר כבר פתוח, הצטרף לחדר המלחמה הקיים וצרף הקשר אנדרואיד במקום ליצור אחד חדש.
- כאשר התקרית נוגעת בפערים בפנקס ההפעלה, צור משימות מעקב באפוס AND7 JIRA ותייגו את `telemetry-runbook`.

## 5. תרגילי כאוס ומוכנות

- בצע את התרחישים המפורטים ב
  `docs/source/sdk/android/telemetry_chaos_checklist.md` מדי רבעון ולפני
  מהדורות מרכזיות. התחבר לתוצאות עם תבנית דוח המעבדה.
- אחסן ראיות (צילומי מסך, יומנים) תחת
  `docs/source/sdk/android/readiness/screenshots/`.
- עקוב אחר כרטיסי תיקון באפוס AND7 עם התווית `telemetry-lab`.
- מפת התרחישים: C1 (תקלת עריכה), C2 (עקיפה), C3 (החלמה של היצואן), C4
  (שער הבדל סכימה באמצעות `run_schema_diff.sh` עם תצורה נסחפת), C5
  (הטיית פרופיל מכשיר מוזנת באמצעות `generate_android_load.sh`), C6 (פסק זמן Torii
  + שידור חוזר בתור), C7 (דחיית אישור). שמור את המספור הזה מיושר עם
  `telemetry_lab_01.md` ורשימת הכאוס בעת הוספת תרגילים.

### 5.1 תרגיל סחיפה ועקיפה של חיתוך (C1/C2)

1. הזרקת כשל גיבוב באמצעות
   `scripts/telemetry/inject_redaction_failure.sh` והמתן ל-PagerDuty
   התראה (`android.telemetry.redaction.failure`). ללכוד את פלט CLI מ
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` עבור
   תיעוד האירוע.
2. נקה את הכשל עם `--clear` ואשר שההתראה נפתרה בתוך
   10 דקות; צרף צילומי מסך Grafana של לוחות המלח/רשות.
3. צור בקשת עקיפה חתומה באמצעות
   `docs/examples/android_override_request.json`, החל אותו עם
   `scripts/android_override_tool.sh apply`, ואמת את המדגם ללא גיבוב על ידי
   בדיקת מטען היצואן בהיערכות (חפש
   `android.telemetry.redaction.override`).
4. בטל את העקיפה עם `scripts/android_override_tool.sh revoke --token <token>`,
   הוסף את ה-hash לעקוף אסימון פלוס הפניה לכרטיס
   `docs/source/sdk/android/telemetry_override_log.md`, ומטביע JSON תקציר
   תחת `docs/source/sdk/android/readiness/override_logs/`. זה סוגר את
   תרחיש C2 ברשימת הכאוס ושומר על עדויות הממשל טריות.

### 5.2 תרגיל בראון ליצואן והפעלה חוזרת בתור (C3/C6)1. קנה קנה מידה של אספן הבמה ('קנה מידה kubectl
   deploy/android-otel-collector --replicas=0`) כדי לדמות יצואן
   חום. עקוב אחר מדדי חיץ דרך ה-CLI הסטטוס ואשר שההתראות מופעלות בשעה
   רף 15 הדקות.
2. שחזר את האספן, אשר את ניקוז ההצטברות ושמור את יומן האספן בארכיון
   קטע המראה את השלמת השידור החוזר.
3. הן בפיקסל והן באמולטור, עקוב אחר ScenarioC6: install
   `examples/android/operator-console`, החלף מצב מטוס, שלח את ההדגמה
   העברות, ולאחר מכן השבת את מצב המטוס וצפה במדדי עומק התורים.
4. משוך כל תור ממתין (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :core:classes >/dev/null`), and run `java -cp build/classes
   org.hyperledger.iroha.android.tools.PendingQueueInspector --file
   /tmp/.queue --json > queue-replay-.json`. צרף מפוענח
   מעטפות בתוספת גיבוב חוזר ליומן המעבדה.
5. עדכן את דוח הכאוס עם משך הפסקת היצואן, עומק התור לפני/אחרי,
   ואישור ש-`android_sdk_offline_replay_errors` נשאר 0.

### 5.3 תסריט כאוס של אשכולות בימוי (android-telemetry-stg)

בעלי מקבץ בימוי Haruka Yamamoto (Android Observability TL) וליאם אוקונור
(SRE) עקוב אחר התסריט הזה בכל פעם שמתוכננת ריצת חזרות. הרצף נשמר
המשתתפים התיישרו עם רשימת הכאוס של הטלמטריה תוך הבטחה לכך
חפצי אמנות נלכדים לצורך ממשל.

**משתתפים**

| תפקיד | אחריות | צור קשר |
|------|------------------------|--------|
| אנדרואיד כוננות IC | מניע את התרגיל, מתאם הערות PagerDuty, בעל יומן פקודות | PagerDuty `android-telemetry-primary`, `#android-sdk-support` |
| בעלי אשכולות במה (חרוקה, ליאם) | חלונות החלפת שערים, הפעל פעולות `kubectl`, טלמטריית אשכול של תמונת מצב | `#android-staging` |
| מנהל מסמכים/תמיכה (פריה) | רשום עדויות, עקוב אחר רשימת בדיקות מעבדה, פרסם כרטיסי מעקב | `#docs-support` |

**תיאום טרום טיסה**

- 48 שעות לפני התרגיל, הגש בקשת שינוי המפרטת את המתוכנן
  תרחישים (C1–C7) והדבק את הקישור ב-`#android-staging` כדי שבעלי האשכולות
  יכול לחסום פריסות מתנגשות.
- אסוף את ה-hash העדכני ביותר של `ClientConfig` ו-`kubectl ---context staging get pods
  -n android-telemetry-stg` פלט כדי לקבוע את מצב הבסיס, ואז לאחסן
  שניהם תחת `docs/source/sdk/android/readiness/labs/reports/<date>/`.
- אשר את כיסוי המכשיר (פיקסל + אמולטור) והבטח
  `ci/run_android_tests.sh` הרכיב את הכלים ששימשו במהלך המעבדה
  (`PendingQueueInspector`, מזרקי טלמטריה).

**מחסומי ביצוע**

- הכריז על "תחילת כאוס" ב-`#android-sdk-support`, התחל את הקלטת הגשר,
  ולשמור על `docs/source/sdk/android/telemetry_chaos_checklist.md` גלוי כך
  כל פקודה מסופרת לסופר.
- בקש מבעל הבמה לשקף כל פעולת מזרק (`kubectl scale`, יצואן
  הפעלה מחדש, טען מחוללים) כך שגם Observability וגם SRE מאשרים את השלב.
- ללכוד את הפלט מתוך `scripts/telemetry/check_redaction_status.py
  --status-url https://android-telemetry-stg/api/redaction/status` אחרי כל אחד
  תרחיש והדבק אותו במסמך האירוע.

**החלמה**- אל תעזוב את הגשר עד שכל המזרקים יפוקו (`inject_redaction_failure.sh --clear`,
  לוחות המחוונים `kubectl scale ... --replicas=1`) ו-Grafana מציגים מצב ירוק.
- מסמכים/תמיכה בארכיונים של תורים, יומני CLI וצילומי מסך תחת
  `docs/source/sdk/android/readiness/screenshots/<date>/` ומתקתק את הארכיון
  רשימת תיוג לפני סגירת בקשת השינוי.
- רישום כרטיסי מעקב עם התווית `telemetry-chaos` עבור כל תרחיש זה
  נכשלו או הפיק מדדים בלתי צפויים, והתייחס אליהם ב-`status.md`
  במהלך הסקירה השבועית הבאה.

| זמן | פעולה | בעל/ים | חפץ |
|------|--------|--------|----------|
| T-30 דקות | אמת את תקינות `android-telemetry-stg`: `kubectl --context staging get pods -n android-telemetry-stg`, אשר שאין שדרוגים ממתינים וגרסאות אספן הערות. | הארוקה | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20 דקות | עומס בסיס זרעים (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) ונקודות לכידה. | ליאם | `readiness/labs/reports/<date>/load-generator.log` |
| T−15 דקות | העתק את `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` ל-`docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`, רשום תרחישים להפעלה (C1–C7), והקצה סופרים. | פריה דשפנדה (תמיכה) | סימון אירוע בוצע לפני תחילת החזרות. |
| T−10 דקות | אשר את Pixel + אמולטור מקוון, ה-SDK העדכני ביותר מותקן, ו-`ci/run_android_tests.sh` הידור `PendingQueueInspector`. | הארוקה, ליאם | `readiness/screenshots/<date>/device-checklist.png` |
| T−5 דקות | התחל את גשר זום, התחל בהקלטת מסך והכריז על "התחלת כאוס" ב-`#android-sdk-support`. | IC / מסמכים / תמיכה | ההקלטה נשמרה תחת `readiness/archive/<month>/`. |
| +0 דקות | בצע את התרחיש שנבחר מ-`docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (בדרך כלל C2 + C6). שמור על מדריך המעבדה גלוי וקרא קריאות פקודות כשהן מתרחשות. | הארוקה נוהג, ליאם משקף תוצאות | יומנים מצורפים לקובץ האירוע בזמן אמת. |
| +15 דקות | השהה כדי לאסוף מדדים (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) ותפוס צילומי מסך Grafana. | הארוקה | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25 דקות | שחזר את כל הכשלים שהוזרמו (`inject_redaction_failure.sh --clear`, `kubectl scale ... --replicas=1`), הפעל מחדש תורים ואשר סגירת התראות. | ליאם | `readiness/labs/reports/<date>/recovery.log` |
| +35 דקות | תחקיר: עדכן את מסמך האירוע עם עובר/נכשל לפי תרחיש, רשום מעקבים ודחוף חפצים ל-git. הודע ל-Docs/Support שניתן להשלים את רשימת הבדיקה לארכיון. | IC | מסמך האירוע עודכן, `readiness/archive/<month>/checklist.md` מסומן. |

- השאירו את בעלי הביניים על הגשר עד שהיצואנים יהיו בריאים וכל ההתראות יתוקנו.
- אחסן תורים גולמיים ב-`docs/source/sdk/android/readiness/labs/reports/<date>/queues/` והתייחס ל-hash שלהם ביומן האירועים.
- אם תרחיש נכשל, צור מיד כרטיס JIRA שכותרתו `telemetry-chaos` וצלב אותו מ-`status.md`.
- עוזר אוטומציה: `ci/run_android_telemetry_chaos_prep.sh` עוטף את מחולל העומס, תמונות מצב וצנרת ייצוא בתור. הגדר `ANDROID_TELEMETRY_DRY_RUN=false` כאשר גישת הבמה זמינה ואת `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (וכו') כך שהסקריפט מעתיק כל קובץ תור, פולט `<label>.sha256`, ומריץ `PendingQueueInspector` כדי לייצר Grafana. השתמש ב-`ANDROID_PENDING_QUEUE_INSPECTOR=false` רק כאשר יש לדלג על פליטת JSON (לדוגמה, אין JDK זמין). **ייצא תמיד את מזהי המלח הצפויים לפני הפעלת העזר** על ידי הגדרת `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` ו-`ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` כך שהשיחות המוטמעות `check_redaction_status.py` ייכשלו במהירות אם הטלמטריה שנלכדה חורגת מקו הבסיס של Rust.

## 6. תיעוד והפעלה- **ערכת הפעלת מפעיל:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  מקשר את ספר ההפעלה, מדיניות הטלמטריה, מדריך המעבדה, רשימת הבדיקה לארכיון וידע
  צ'קים לחבילה אחת מוכנה ל-AND7. התייחס אליו בעת הכנת SRE
  ממשל מקריאה מראש או תזמון הרענון הרבעוני.
- **הפעלות הפעלה:** הקלטת הפעלה של 60 דקות פועלת בתאריך 2026-02-18
  עם רענון רבעוני. חומרים חיים מתחת
  `docs/source/sdk/android/readiness/`.
- **בדיקות ידע:** על הצוות לקבל ציון של ≥90% באמצעות טופס המוכנות. חנות
  תוצאות ב-`docs/source/sdk/android/readiness/forms/responses/`.
- **עדכונים:** בכל פעם שסכימות טלמטריה, לוחות מחוונים או עוקפים מדיניות
  שנה, עדכן את ספר ההפעלה הזה, את ספר התמיכה ואת `status.md` באותו
  יחסי ציבור.
- **סקירה שבועית:** לאחר כל מועמד לשחרור Rust (או לפחות מדי שבוע), אמת
  `java/iroha_android/README.md` ופנקס ההפעלה הזה עדיין משקפים את האוטומציה הנוכחית,
  נהלי סיבוב משחקים וציפיות ממשל. ללכוד את הביקורת ב
  `status.md` כדי שביקורת אבן הדרך של היסודות תוכל לאתר את רעננות התיעוד.

## 7. StrongBox Attestation Harness- **מטרה:** אימות חבילות אישורים מגובות חומרה לפני קידום מכשירים לתוך
  מאגר StrongBox (AND2/AND6). הרתמה צורכת שרשראות תעודות שנתפסו ומאמתת אותן
  נגד שורשים מהימנים באמצעות אותה מדיניות שקוד הייצור מבצע.
- **הפניה:** ראה `docs/source/sdk/android/strongbox_attestation_harness_plan.md` במלואו
  לכידת API, מחזור חיים כינוי, חיווט CI/Buildkite ומטריצת בעלות. התייחס לתוכנית הזו כאל
  מקור האמת בעת הצטרפות של טכנאי מעבדה חדשים או עדכון חפצי כספים/תאימות.
- **זרימת עבודה:**
  1. אסוף חבילת אישורים במכשיר (כינוי, `challenge.hex` ו-`chain.pem` עם
     עלה → סדר שורש) והעתק אותו לתחנת העבודה.
  2. הפעל את `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` באמצעות הקובץ המתאים
     Google/Samsung root (ספריות מאפשרות לך לטעון חבילות ספקים שלמות).
  3. אחסן את תקציר ה-JSON לצד חומר אישור גולמי ב
     `artifacts/android/attestation/<device-tag>/`.
- **פורמט חבילה:** עקוב אחר `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  עבור פריסת הקובץ הנדרשת (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **שורשים מהימנים:** השג PEM שסופק על ידי הספק מחנות סודות מעבדת המכשירים; לעבור מרובים
  ארגומנטים `--trust-root` או הצבע `--trust-root-dir` לספרייה שמחזיקה את העוגנים כאשר
  השרשרת מסתיימת בעוגן שאינו של גוגל.
- **רתמת CI:** השתמש ב-`scripts/android_strongbox_attestation_ci.sh` כדי לאמת חבילות בארכיון
  על מכונות מעבדה או רצי CI. הסקריפט סורק את `artifacts/android/attestation/**` ומפעיל את
  לרתום לכל ספרייה המכילה את הקבצים המתועדים, כתיבה מרעננת `result.json`
  סיכומים במקום.
- **נתיב CI:** לאחר סנכרון חבילות חדשות, הפעל את השלב Buildkite המוגדר ב
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  העבודה מבצעת `scripts/android_strongbox_attestation_ci.sh`, מייצרת סיכום עם
  `scripts/android_strongbox_attestation_report.py`, מעלה את הדוח ל-`artifacts/android_strongbox_attestation_report.txt`,
  ומציין את המבנה כ-`android-strongbox/report`. בדוק תקלות באופן מיידי ו
  קשר את כתובת האתר לבנייה ממטריצת המכשיר.
- **דיווח:** צרף את פלט JSON לביקורות ניהול ועדכן את ערך מטריצת המכשיר ב
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` עם תאריך האישור.
- **חזרה מדומה:** כאשר החומרה אינה זמינה, הפעל את `scripts/android_generate_mock_attestation_bundles.sh`
  (המשתמש ב-`scripts/android_mock_attestation_der.py`) כדי להטביע חבילות בדיקה דטרמיניסטיות בתוספת שורש מדומה משותף כדי ש-CI ו-docs יוכלו לממש את הרתמה מקצה לקצה.
- **מעקות בטיחות בקוד:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests' מכסים ריקים לעומת מאותגרים
  חידוש אישורים (StrongBox/TEE metadata) ופולט `android.keystore.attestation.failure`
  על אי התאמה של אתגר, כך שרגרסיות מטמון/טלמטריה נתפסות לפני משלוח חבילות חדשות.

## 8. אנשי קשר

- **הנדסת תמיכה בכוננות:** `#android-sdk-support`
- **ממשל SRE:** `#sre-governance`
- **מסמכים/תמיכה:** `#docs-support`
- **עץ הסלמה:** עיין ב-Android Support Playbook §2.1

## 9. תרחישי פתרון בעיותפריט מפת הדרכים AND7-P2 קורא לשלוש מחלקות תקריות שמדפות שוב ושוב את
אנדרואיד בכוננות: Torii/פסקי זמן של רשת, כשלי אישור StrongBox, ו
סחף מניפסט `iroha_config`. עבדו על רשימת הבדיקה הרלוונטית לפני ההגשה
סיוו1/2 מעקבים וארכיון הראיות ב-`incident/<date>-android-*.md`.

### 9.1 Torii ופסקי זמן לרשת

**אותות**

- התראות על `android_sdk_submission_latency`, `android_sdk_pending_queue_depth`,
  `android_sdk_offline_replay_errors`, ושיעור השגיאות Torii `/v1/pipeline`.
- ווידג'טים של `operator-console` (דוגמאות/אנדרואיד) המציגים ניקוז תור תקוע או
  ניסיונות חוזרים תקועים בגיבוי אקספוננציאלי.

**תגובה מיידית**

1. אשר את PagerDuty (`android-networking`) והתחל יומן אירועים.
2. צלם תצלומי מצב Grafana (השהיית הגשה + עומק תור) המכסים את
   30 הדקות האחרונות.
3. רשמו את ה-hash הפעיל של `ClientConfig` מיומני המכשיר (`ConfigWatcher`
   מדפיס את תקציר המניפסט בכל פעם שטעינה מחדש מצליחה או נכשלת).

**אבחון**

- **תקינות התור:** משוך את קובץ התור המוגדר מהתקן בימוי או מהתקן
  אמולטור (`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`). פענח את המעטפות עם
  `OfflineSigningEnvelopeCodec` כמתואר ב
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` כדי לאשר את
  צבר ההזמנות תואם את ציפיות המפעיל. צרף את הגיבובים המפוענחים ל-
  אירוע.
- **מלאי גיבוב:** לאחר הורדת קובץ התור, הפעל את עוזר המפקח
  כדי ללכוד גיבוב/כינויים קנוניים עבור חפצי התקרית:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  צרף את `queue-inspector.json` ואת התווית המודפסת יפה לאירוע
  וקשר אותו מדוח המעבדה של AND7 עבור תרחיש D.
- **קישוריות Torii:** הפעל את רתמת התעבורה של HTTP באופן מקומי כדי לשלול SDK
  רגרסיות: תרגילי `ci/run_android_tests.sh`
  `HttpClientTransportTests`, `HttpClientTransportHarnessTests`, ו
  `ToriiMockServerTests`. כשלים כאן מצביעים על באג לקוח ולא על א
  הפסקת Torii.
- **חזרה על הזרקת תקלות:** בפיקסל (StrongBox) וה-AOSP
  אמולטור, החלף קישוריות כדי לשחזר גידול בתור ממתין:
  `adb shell cmd connectivity airplane-mode enable` → שלח שני הדגמות
  עסקאות דרך קונסולת המפעיל → `adb shell cmd קישוריות מצב מטוס
  disable` → verify the queue drains and `android_sdk_offline_replay_errors`
  נשאר 0. הקלט גיבוב של העסקאות המושמעות מחדש.
- **שוויון התראה:** בעת כוונון ספים או לאחר שינויים ב-Torii, בצע
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` אז כללי Prometheus יישארו
  מיושר עם לוחות המחוונים.

**החלמה**

1. אם Torii פגום, הפעל את ה-Torii ב-Call והמשיך להפעיל מחדש את
   תור פעם אחת `/v1/pipeline` מקבל תעבורה.
2. הגדר מחדש את הלקוחות המושפעים רק באמצעות מניפסטים חתומים של `iroha_config`. ה
   `ClientConfig` צופה בטעינה מחדש חייבת לפלוט יומן הצלחה לפני התקרית
   יכול לסגור.
3. עדכן את התקרית עם גודל התור לפני/אחרי השידור החוזר בתוספת hashes של
   כל עסקאות שנפלו.

### 9.2 StrongBox וכשלים באישור

**אותות**- התראות על `android_sdk_strongbox_success_rate` או
  `android.keystore.attestation.failure`.
- טלמטריית `android.keystore.keygen` מתעדת כעת את המבוקש
  `KeySecurityPreference` והמסלול בשימוש (`strongbox`, `hardware`,
  `software`) עם דגל `fallback=true` כאשר העדפת StrongBox נוחתת ב
  TEE/תוכנה. STRONGBOX_REQUIRED בקשות נכשלות כעת במהירות במקום בשקט
  החזרת מפתחות TEE.
- תמיכה בכרטיסי הפניה למכשירי `KeySecurityPreference.STRONGBOX_ONLY`
  נופל חזרה למפתחות תוכנה.

**תגובה מיידית**

1. אשר את PagerDuty (`android-crypto`) וללכוד את תווית הכינוי המושפעת
   (hash מלוח) בתוספת דלי פרופיל מכשיר.
2. בדוק את ערך מטריצת האישור עבור המכשיר ב
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ו
   לרשום את התאריך המאומת האחרון.

**אבחון**

- **אימות חבילה:** הפעל
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  על האישור הארכיון כדי לאשר אם התקלה נובעת מהתקן
  תצורה שגויה או שינוי מדיניות. חבר את `result.json` שנוצר.
- **Regen Challenge:** אתגרים לא נשמרים במטמון. כל בקשת אתגר מחוללת חדש
  אישור ומטמונים על ידי `(alias, challenge)`; שיחות ללא אתגר משתמשות מחדש במטמון. לא נתמך
- **סיווג CI:** בצע את `scripts/android_strongbox_attestation_ci.sh` כך שכל
  חבילה מאוחסנת מאומתת מחדש; זה שומר מפני בעיות מערכתיות שהוצגו
  על ידי עוגני אמון חדשים.
- **תרגיל מכשיר:** על חומרה ללא StrongBox (או על ידי כפיית האמולטור),
  הגדר את ה-SDK לדרוש StrongBox בלבד, שלח עסקת הדגמה ואשר
  יצואן הטלמטריה פולט את אירוע `android.keystore.attestation.failure`
  עם הסיבה הצפויה. חזור על פיקסל בעל יכולת StrongBox כדי להבטיח את
  שביל שמח נשאר ירוק.
- **בדיקת רגרסיה SDK:** הפעל את `ci/run_android_tests.sh` ושלם
  תשומת לב לסוויטות ממוקדות אישור (`AndroidKeystoreBackendDetectionTests`,
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  `KeystoreKeyProviderTests` להפרדת מטמון/אתגר). כישלונות כאן
  מצביעים על רגרסיה בצד הלקוח.

**החלמה**

1. צור מחדש חבילות אישורים אם הספק החליף אישורים או אם
   המכשיר קיבל לאחרונה OTA גדול.
2. העלה את החבילה המרעננת ל-`artifacts/android/attestation/<device>/` ו
   עדכן את ערך המטריצה בתאריך החדש.
3. אם StrongBox אינו זמין בייצור, עקוב אחר זרימת העבודה לעקוף ב
   סעיף 3 ותעד את משך החזרה; צמצום לטווח ארוך דורש
   החלפת מכשיר או תיקון של ספק.

### 9.2א שחזור ייצוא דטרמיניסטי

- **פורמטים:** היצוא הנוכחי הוא v3 (לכל ייצוא salt/nonce + Argon2id, מתועד כ
- **מדיניות ביטויי סיסמה:** v3 אוכפת ביטויי סיסמה של ≥12 תווים. אם משתמשים מספקים קצר יותר
  ביטויי סיסמה, הורה להם לייצא מחדש עם ביטוי סיסמה תואם; יבוא v0/v1 הם
  פטור אך יש לעטוף אותו מחדש כ-v3 מיד לאחר הייבוא.
- **מגני שיבוש/שימוש חוזר:** מפענחים דוחים אורך מלח אפס/קצר או לא חד וחוזר על עצמו
  זוגות מלח/nonce משטחים כשגיאות `salt/nonce reuse`. צור מחדש את הייצוא כדי לנקות
  השומר; אל תנסה לכפות שימוש חוזר.
  `SoftwareKeyProvider.importDeterministic(...)` כדי לייבש מחדש את המפתח, אז
  `exportDeterministic(...)` יפלוט חבילה v3 כך שכלי שולחן העבודה יתעד את ה-KDF החדש
  פרמטרים.### 9.3 חוסר התאמה של מניפסט ותצורה

**אותות**

- כשלי טעינה מחדש של `ClientConfig`, שמות מארחים לא תואמים של Torii או טלמטריה
  הבדלי סכימה מסומנים על ידי הכלי AND7 diff.
- מפעילים המדווחים על ידיות שונות של ניסיון חוזר/השבתה במכשירים זהה
  צי.

**תגובה מיידית**

1. צלם את תקציר `ClientConfig` המודפס ביומני Android וב-
   תקציר צפוי ממניפסט השחרור.
2. זרוק את תצורת הצומת הפועל לצורך השוואה:
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

**אבחון**

- **Schema diff:** הפעל `scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  כדי ליצור דוח Norito הבדל, רענן את קובץ הטקסט Prometheus, וצרף את
  חפץ JSON בתוספת מדדים ראיות לתקרית ויומן מוכנות לטלמטריה של AND7.
- **אימות מניפסט:** השתמש ב-`iroha_cli runtime capabilities` (או בזמן הריצה
  פקודת audit) כדי לאחזר את ה-hashs crypto/ABI המפורסמים של הצומת ולהבטיח
  הם תואמים למניפסט הנייד. אי התאמה מאשרת שהצומת הוחזר
  מבלי להוציא מחדש את המניפסט של אנדרואיד.
- **בדיקת רגרסיה SDK:** `ci/run_android_tests.sh` מכסים
  `ClientConfigNoritoRpcTests`, `ClientConfig.ValidationTests`, ו
  `HttpClientTransportStatusTests`. כשלים מסמנים שה-SDK שנשלח אינו יכול
  נתח את פורמט המניפסט שנפרס כעת.

**החלמה**

1. צור מחדש את המניפסט דרך הצינור המורשה (בדרך כלל
   `iroha_cli runtime Capabilities` → חתום Norito מניפסט → חבילת תצורה) ו
   לפרוס אותו מחדש דרך ערוץ המפעיל. לעולם אל תערוך את `ClientConfig`
   עוקף את המכשיר.
2. ברגע שמניפסט מתוקן נוחת, צפה ב-`ConfigWatcher` "טעינה מחדש בסדר"
   הודעה בכל שכבת צי ולסגור את האירוע רק לאחר הטלמטריה
   schema diff דוחות זוגיות.
3. רשמו את ה-hash של המניפסט, נתיב החפץ של הסכימה והקישור לאירוע
   `status.md` תחת סעיף אנדרואיד לביקורת.

## 10. תוכנית לימודים להפעלת מפעיל

פריט מפת הדרכים **AND7** דורש חבילת הדרכה שניתן לחזור עליה, כך שהמפעילים,
מהנדסי תמיכה, ו-SRE יכולים לאמץ את עדכוני הטלמטריה/עריכה ללא
ניחושים. חבר את הקטע הזה עם
`docs/source/sdk/android/readiness/and7_operator_enablement.md`, המכיל
רשימת הבדיקה המפורטת וקישורי החפצים.

### מודולים של 10.1 מפגשים (תדרוך של 60 דקות)

1. **ארכיטקטורת טלמטריה (15 דקות).** עברו דרך מאגר היצואן,
   מסנן עריכה, וכלי הבדל סכימה. הדגמה
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` פלוס
   `scripts/telemetry/check_redaction_status.py` כדי שהמשתתפים יראו כמה זוגיות
   נאכף.
2. **ספר ריצה + מעבדות כאוס (20 דקות).** הדגש את הסעיפים 2-9 בספר ההפעלה הזה,
   חזור על תרחיש אחד מ-`readiness/labs/telemetry_lab_01.md`, והראה כיצד
   לארכיון חפצים תחת `readiness/labs/reports/<stamp>/`.
3. **עקיפה + זרימת עבודה של תאימות (10 דקות).** סקירת סעיף 3 עוקפת,
   להדגים `scripts/android_override_tool.sh` (החל/בטל/עכל), ו
   עדכון `docs/source/sdk/android/telemetry_override_log.md` בתוספת העדכנית ביותר
   לעכל JSON.
4. **שאלות ותשובות / בדיקת ידע (15 דקות).** השתמש בכרטיס ההפניה המהיר ב-
   `readiness/cards/telemetry_redaction_qrc.md` כדי לעגן שאלות, אם כן
   לכידת מעקבים ב-`readiness/and7_operator_enablement.md`.### 10.2 קצב נכסים ובעלים

| נכס | קידנס | בעל/ים | מיקום הארכיון |
|-------|--------|--------|----------------|
| הדרכה מוקלטת (זום/צוותים) | רבעוני או לפני כל סיבוב מלח | Android Observability TL + מנהל מסמכים/תמיכה | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (הקלטה + רשימת בדיקה) |
| חפיסת שקופיות וכרטיס עזר מהיר | עדכן בכל פעם שהמדיניות/פנקס ההפעלה משתנה | מנהל מסמכים/תמיכה | `docs/source/sdk/android/readiness/deck/` ו-`/cards/` (ייצוא PDF + Markdown) |
| בדיקת ידע + גיליון נוכחות | לאחר כל סשן חי | הנדסת תמיכה | חסימת נוכחות `docs/source/sdk/android/readiness/forms/responses/` ו-`and7_operator_enablement.md` |
| צבר שאלות ותשובות / יומן פעולות | גִלגוּל; מתעדכן לאחר כל פגישה | LLM (ממלא מקום DRI) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 לולאת ראיות ומשוב

- אחסן חפצי הפעלה (צילומי מסך, תרגילי תקריות, ייצוא חידון) ב-
  אותו ספרייה מתוארכת המשמשת לחזרות כאוס, כך שהממשל יכול לבקר את שניהם
  מסלולי מוכנות ביחד.
- בסיום הפעלה, עדכן את `status.md` (מדור אנדרואיד) עם קישורים אל
  את ספריית הארכיון ושימו לב לכל מעקב פתוח.
- יש להפוך שאלות בלתי נמנעות מהשאלות והתשובות בשידור חי לבעיות או למסמך
  למשוך בקשות תוך שבוע אחד; עיין באפוסי מפת הדרכים (AND7/AND8) ב-
  תיאור הכרטיס כדי שהבעלים יישארו מיושרים.
- סנכרון SRE סוקר את רשימת התיוג של הארכיון בתוספת חפץ הבדל הסכימה המפורט ברשימה
  סעיף 2.3 לפני הכרזת תכנית הלימודים סגורה לרבעון.