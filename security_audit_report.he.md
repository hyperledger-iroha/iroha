<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# דוח ביקורת אבטחה

תאריך: 26-03-2026

## תקציר מנהלים

ביקורת זו התמקדה במשטחי הסיכון הגבוהים ביותר בעץ הנוכחי: Torii תזרימי HTTP/API/אישור, תעבורה P2P, ממשקי API לטיפול בסתר, מגני הובלה של SDK ונתיב לחיטוי הקבצים המצורפים.

מצאתי 6 בעיות שניתן לבצע:

- 2 ממצאים בחומרה גבוהה
- 4 ממצאים בחומרה בינונית

הבעיות החשובות ביותר הן:

1. Torii רושם כעת כותרות של בקשות נכנסות עבור כל בקשת HTTP, מה שיכול לחשוף אסימוני נושאות, אסימוני API, אסימוני הפעלה/אתחול של מפעיל, וסמני mTLS מועברים ליומנים.
2. מספר מסלולי Torii ו-SDKs ציבוריים עדיין תומכים בשליחת ערכי `private_key` גולמיים לשרת כך ש-Torii יוכל לחתום בשם המתקשר.
3. מספר נתיבים "סודיים" מטופלים כאל גופי בקשות רגילים, כולל גזירת זרעים סודית ואישור בקשה קנוני בחלק מה-SDKs.

## שיטה

- סקירה סטטית של נתיבי טיפול בסתר Torii, P2P, קריפטו/VM ו-SDK
- פקודות אימות ממוקדות:
  - `cargo check -p iroha_torii --lib --message-format short` -> מעבר
  - `cargo check -p iroha_p2p --message-format short` -> מעבר
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> מעבר
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> מעבר, אזהרות בגרסה כפולה בלבד
- לא הושלם במעבר הזה:
  - סביבת עבודה מלאה לבנות/בדיקה/קליפי
  - חבילות בדיקה של Swift/Gradle
  - אימות זמן ריצה CUDA/Metal

## ממצאים

### SA-001 גבוה: Torii מתעד כותרות בקשות רגישות ברחבי העולםהשפעה: כל פריסה ששולחת מעקב אחר בקשות עלולה להדליף אסימוני נושא/API/מפעיל וחומר אישור קשור ליומני האפליקציה.

עדות:

- `crates/iroha_torii/src/lib.rs:20752` מאפשר את `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` מאפשר את `DefaultMakeSpan::default().include_headers(true)`
- שמות כותרות רגישים נמצאים בשימוש פעיל במקומות אחרים באותו שירות:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

למה זה חשוב:

- `include_headers(true)` מתעד ערכי כותרת נכנסת מלאים לטווחי מעקב.
- Torii מקבל חומרי אימות בכותרות כמו `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` ו-`x-forwarded-client-cert`.
- פשרה של יומן רישום, איסוף יומני ניפוי באגים או חבילת תמיכה יכולים להפוך לאירוע גילוי אישורים.

תיקון מומלץ:

- הפסק לכלול כותרות בקשות מלאות בטווחי הייצור.
- הוסף עריכה מפורשת עבור כותרות רגישות לאבטחה אם עדיין יש צורך ברישום כותרות לצורך איתור באגים.
- התייחס לרישום בקשות/תגובות כאל נושא סודי כברירת מחדל, אלא אם כן הנתונים רשומים באופן חיובי.

### SA-002 גבוה: ממשקי API ציבוריים Torii עדיין מקבלים מפתחות פרטיים גולמיים לחתימה בצד השרת

השפעה: מעודדים לקוחות להעביר מפתחות פרטיים גולמיים דרך הרשת כדי שהשרת יוכל לחתום בשמם, וליצור ערוץ חשיפת סוד מיותר בשכבות ה-API, SDK, פרוקסי ושכבות זיכרון השרת.

עֵדוּת:- תיעוד מסלול ניהול מפרסם במפורש חתימה בצד השרת:
  - `crates/iroha_torii/src/gov.rs:495`
- יישום המסלול מנתח את המפתח הפרטי שסופק וחותם בצד השרת:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- ערכות SDK מסדרות באופן פעיל את `private_key` לתוך גופי JSON:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

הערות:

- דפוס זה אינו מבודד למשפחת מסלול אחת. העץ הנוכחי מכיל את אותו מודל נוחות בכל ניהול, מזומנים לא מקוונים, מנויים ו-DTOs אחרים מול אפליקציות.
- בדיקות תעבורה ב-HTTPS בלבד מפחיתות העברה מקרית של טקסט רגיל, אך הן אינן פותרות טיפול בסוד בצד השרת או סיכון חשיפת רישום/זיכרון.

תיקון מומלץ:

- הוצא משימוש כל DTOs הבקשה הנושאים נתוני `private_key` גולמיים.
- לדרוש מהלקוחות לחתום באופן מקומי ולהגיש חתימות או עסקאות/מעטפות חתומות במלואן.
- הסר דוגמאות `private_key` מ-OpenAPI/SDKs לאחר חלון תאימות.

### SA-003 בינוני: גזירת מפתח סודי שולחת חומר זרע סודי ל-Torii ומהדהדת אותו בחזרה

השפעה: ה-API החסוי של גזירת מפתחות הופך חומר ראשי לנתונים רגילים של בקשה/תגובה, ומגדיל את הסיכוי לחשיפת סיד באמצעות פרוקסי, תוכנת ביניים, יומנים, עקבות, דוחות קריסה או שימוש לרעה של לקוח.

עֵדוּת:- הבקשה מקבלת חומר זרעים ישירות:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- סכימת התגובה מהדהדת את הזרע בחזרה גם ב-hex וגם ב-base64:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- המטפל מקודד מחדש באופן מפורש ומחזיר את הזרע:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- ה-SDK של Swift חושף את זה כשיטת רשת רגילה וממשיך את הזרע המהדהד במודל התגובה:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

תיקון מומלץ:

- העדיפו גזירת מפתח מקומי בקוד CLI/SDK והסר לחלוטין את מסלול הגזירה המרוחק.
- אם המסלול חייב להישאר, לעולם אל תחזיר את הזרע בתגובה וסמן גופים נושאי זרע כרגישים בכל שומרי התחבורה ובנתיבי הטלמטריה/רישום.

### SA-004 בינוני: לזיהוי רגישות לתחבורה SDK יש נקודות עיוורות עבור חומר סודי שאינו `private_key`

השפעה: ערכות SDK מסוימות יאכפו HTTPS עבור בקשות גולמיות של `private_key`, אך עדיין יאפשרו לחומר בקשות רגיש אחר לאבטחה לעבור דרך HTTP לא מאובטח או למארחים לא תואמים.

עֵדוּת:- Swift מתייחסת לכותרות אישור בקשות קנוניות כרגישות:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- אבל סוויפט עדיין תואמת רק גוף ב-`"private_key"`:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- קוטלין מזהה רק כותרות `authorization` ו-`x-api-token`, ואז חוזר לאותה היוריסטית גוף `"private_key"`:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- ל-Java/Android יש את אותה מגבלה:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- חותמי הבקשות הקנוניות של Kotlin/Java מייצרים כותרות אישור נוספות שאינן מסווגות כרגישות על ידי שומרי התחבורה שלהם:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

תיקון מומלץ:

- החלפת סריקת גוף היוריסטית בסיווג בקשה מפורשת.
- התייחס לכותרות אימות קנוניות, שדות זרעים/ביטוי סיסמה, כותרות מוטציות חתומות וכל שדות עתידיים הנושאים סוד כרגישים לפי חוזה, לא לפי התאמת מחרוזת משנה.
- שמור על כללי הרגישות מיושרים על פני Swift, Kotlin ו-Java.

### SA-005 בינוני: "ארגז חול" המצורף הוא רק תת-תהליך בתוספת `setrlimit`השפעה: חומר החיטוי של הקבצים המצורפים מתואר ומדווח כ"ארגז חול", אך היישום הוא רק מזלג/מנהל של הבינארי הנוכחי עם מגבלות משאבים. ניצול מנתח או ארכיון עדיין יבוצע עם אותה משתמש, תצוגת מערכת קבצים והרשאות רשת/תהליך סביבתי כמו Torii.

עדות:

- השביל החיצוני מסמן את התוצאה כארגזי חול לאחר השרצת ילד:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- ברירת המחדל של הילד הוא קובץ ההפעלה הנוכחי:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- תהליך המשנה עובר במפורש חזרה ל-`AttachmentSanitizerMode::InProcess`:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- ההקשחה היחידה המיושמת היא CPU/מרחב כתובות `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

תיקון מומלץ:

- או ליישם ארגז חול אמיתי של מערכת ההפעלה (לדוגמה מרחבי שמות/seccomp/landlock/בידוד בסגנון מאסר, ירידה בהרשאות, ללא רשת, מערכת קבצים מוגבלת) או להפסיק לתייג את התוצאה כ-`sandboxed`.
- התייחס לעיצוב הנוכחי כאל "בידוד תת-תהליכים" במקום "ארגז חול" בממשקי API, טלמטריה ומסמכים עד לקיים בידוד אמיתי.

### SA-006 בינוני: העברות P2P TLS/QUIC אופציונליות משביתות את אימות האישורהשפעה: כאשר `quic` או `p2p_tls` מופעלים, הערוץ מספק הצפנה אך אינו מאמת את נקודת הקצה המרוחקת. תוקף פעיל בנתיב עדיין יכול להעביר או לסיים את הערוץ, ולהביס את ציפיות האבטחה הרגילות שמפעילים מקשרים ל-TLS/QUIC.

עדות:

- QUIC מתעד במפורש אימות אישור מתירני:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- המאמת QUIC מקבל ללא תנאי את אישור השרת:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- התחבורה TLS-over-TCP עושה את אותו הדבר:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

תיקון מומלץ:

- אמת אישורי עמיתים או הוסף כריכת ערוץ מפורשת בין לחיצת היד החתומה בשכבה גבוהה יותר לבין הפעלת התחבורה.
- אם ההתנהגות הנוכחית היא מכוונת, שנה את שם התכונה/תעד את התכונה כהעברה מוצפנת לא מאומתת כדי שהמפעילים לא יטעו בה כאימות עמיתים מלא של TLS.

## צו תיקון מומלץ1. תקן את SA-001 באופן מיידי על ידי תיקון או ביטול רישום כותרות.
2. תכנן ושלח תוכנית הגירה עבור SA-002 כך שמפתחות פרטיים גולמיים יפסיקו לחצות את גבול ה-API.
3. הסר או צמצם את מסלול גזירת המפתח הסודי המרוחק וסווגו גופים נושאי זרעים כרגישים.
4. יישר כללי רגישות לתעבורה של SDK על פני Swift/Kotlin/Java.
5. החלט אם תברואה מצורפת זקוקה לארגז חול אמיתי או שינוי שם כנה / הגדרה מחדש.
6. הבהירו והקשיחו את מודל האיום של P2P TLS/QUIC לפני שהמפעילים יאפשרו את התחבורה הזו המצפים ל-TLS מאומת.

## הערות אימות

- `cargo check -p iroha_torii --lib --message-format short` עבר.
- `cargo check -p iroha_p2p --message-format short` עבר.
- `cargo deny check advisories bans sources --hide-inclusion-graph` עבר לאחר ריצה מחוץ לארגז החול; הוא פלט אזהרות על גרסה כפולה אך דיווח על `advisories ok, bans ok, sources ok`.
- בדיקת Torii ממוקדת עבור המסלול הסודי של ערכת מפתחות נגזרת החלה במהלך ביקורת זו אך לא הושלמה לפני כתיבת הדוח; הממצא נתמך בבדיקת מקור ישירה ללא קשר.