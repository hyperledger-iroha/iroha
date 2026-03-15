<!-- Hebrew translation of docs/source/connect_architecture_strawman.md -->

---
lang: he
direction: rtl
source: docs/source/connect_architecture_strawman.md
status: complete
translator: manual
---

<div dir="rtl">

# תרשים ארכיטקטורה לסשן Connect (Swift / Android / JS)

המסמך מציג הצעת strawman לעיצוב המאוחד של תזרימי Nexus Connect בכלי ה-SDK עבור Swift, ‎Android‎ ו-JavaScript. הוא נכתב כהכנה לסדנת ה-SDK הצולבת בפברואר ‎2026 ומטרתו לרכז סימני שאלה טרם תחילת הפיתוח.

> עודכן לאחרונה: ‎2026-01-12  
> מחברים: מוביל SDK ל-Swift, ראש צוות רשתות Android, מוביל JS  
> סטטוס: טיוטה לסקירת מועצה

## יעדים

1. ליישר את מחזור החיים של סשן ארנק ↔‎ dApp, כולל bootstrap, אישורים, בקשות חתימה וסגירה.
2. לקבע סכמת Norito משותפת (open/approve/sign/control) לכל ה-SDK-ים ולוודא פריות מול ‎`connect_norito_bridge`‎.
3. להפריד אחריות בין שכבת התמסורת (WebSocket/WebRTC), שכבת ההצפנה (מסגרות Connect + החלפת מפתחות) ושכבת היישום (פאסדות SDK).
4. להבטיח התנהגות דטרמיניסטית בין דסקטופ ומובייל, כולל באפרים לא־מקוונים וחיבור מחדש.

## מחזור חיים של סשן (תמונה עליונה)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## סכמת מעטפת / Norito

כל ה-SDK-ים חייבים להשתמש בסכמת Norito הקנונית המוגדרת ב-`connect_norito_bridge`:

- `EnvelopeV1` (open / approve / sign / control)
- `ConnectFrameV1` (מסגרות מוצפנות עם מטען AEAD)
- קודי שליטה:
  - `open_ext` (מטא-נתונים והרשאות)
  - `approve_ext` (חשבון, הרשאות, הוכחות, חתימה)
  - `reject`, `close`, ‏`ping/pong`, ‏`error`

בעבר Swift השתמשה במקודדי JSON זמניים (`ConnectCodec.swift`), אך מאז אפריל 2026 `ConnectCodec` מחייב את גשר Norito ונכשל אם ה-XCFramework חסר; הסקשן נשאר כאן כדי לתעד את הדרישה המקורית:

| פונקציה | תיאור | סטטוס |
|---------|-------|--------|
| `connect_norito_encode_control_open_ext` | פריים פתיחה של dApp | ממומש בגשר |
| `connect_norito_encode_control_approve_ext` | פריים אישור של ארנק | ממומש |
| `connect_norito_encode_envelope_sign_request_tx/raw` | בקשות חתימה | ממומש |
| `connect_norito_encode_envelope_sign_result_ok/err` | תוצאות חתימה | ממומש |
| `connect_norito_decode_*` | ניתוח פריימים לארנקים/dApp | ממומש |

### משימות נדרשות

- **Swift:** להחליף את Helper-י ה-JSON בגשר, להציג עטיפות מסוג `ConnectFrame`/`ConnectEnvelope` המבוססות על טיפוסי Norito משותפים. ✅ (אפריל 2026)
- **Android/JS:** להגדיר עטיפות תואמות, ליישר קודי שגיאה ומפתחות מטא-נתונים.
- CI: ריצות Swift מפעילות `make swift-ci` כדי לאמת פריות פיקסצ'רים, פידי דשבורד ומטא-דאטה של Buildkite `ci/xcframework-smoke:<lane>:device_tag`, ולשמור על תאימות מול טלמטריית Android/JS.
- **משותף:** לתעד הצפנה (החלפת מפתחות X25519 + AEAD), להגדיר גזירת מפתחות אחידה ולהוסיף בדיקות אינטגרציה המשתמשות בגשר Rust.

## חוזה התמסורת

- תמסורת עיקרית: WebSocket ‏(`‎/v2/connect/ws?sid=<session_id>`‏).
- אופציה עתידית: WebRTC (מחוץ להיקף טיוטה זו).
- אחריות ה-SDK:
  - לשמר heartbeat מסוג ping/pong ולחסוך בסוללה במובייל.
  - לתעדף פריימים יוצאים בעת Offline (תור מוגבל, התמדה עבור dApp).
  - לספק API לזרם אירועים (Swift Combine `AsyncStream`, ‏Android Flow, ‏JS async iterator).
  - לחשוף Hooks של התחברות מחדש ולאפשר הרשמה חוזרת ידנית.

## הצפנה וניהול מפתחות

- עקומה: X25519 עבור החלפת מפתחות (סוד משותף → HKDF → מפתחות AEAD).
- AEAD: ‏ChaCha20-Poly1305 (כמו ב-Norito).
- Nonce: מונה חד־כיווני (`sequence`) לכל כיוון סשן.
- רוטציה: אופציונלית; בטיוטה זו נשמר מפתח יחיד לכל סשן.
- אפליקציות JS דורשות WebCrypto בהקשר מאובטח; סביבת Node מסתמכת על גשר Rust מקומי `iroha_js_host` (נבנה עם `npm run build:native`) או על תוסף HSM.
- StrongBox / Secure Enclave:
  - ארנקים יעדיפו אחסון מפתחות בחומרה כאשר קיים.
  - אישורים חתומים יכולים לכלול נתוני attestation (Android StrongBox, ‏iOS Secure Enclave).
  - יש לתעד fallback להתקנים ללא תמיכה חומרתית.

## הרשאות והוכחות

- JSON ההרשאות צריך לכלול:
  - תחומי הרשאה (חתימת טרנזקציה, חתימת בייטים גולמיים וכו').
  - מגבלות (chain_id, סינון חשבונות, TTL).
  - מטעני ציות אופציונליים (KYC, הסכמה).
- אישור ארנק יכלול:
  - חשבון שנבחר (בארנקים מרובי חשבונות).
  - חבילת הוכחות (ZK או attestation).
  - סיבת דחייה אופציונלית כאשר נדחה.

## פאסדות SDK

| SDK | API מוצע | הערות |
|-----|----------|-------|
| Swift | ‎`ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval`‎ | להחליף placeholders בעטיפות טיפוסיות וזרמים אסינכרוניים. |
| Android | קורוטינות + sealed classes לפריימים | ליישר עם מבנה Swift לשם ניידות. |
| JS | Async iterators + enums של TypeScript | לספק SDK מתאים לדפדפן ול-Node. |

### התנהגויות משותפות

- `ConnectSession` מנהל את מחזור החיים:
  1. חיבור WebSocket וביצוע handshake.
  2. חילוף פריימים ‎open/approve‎.
  3. טיפול בבקשות חתימה ותוצאות.
  4. הפצת אירועים לשכבת האפליקציה.
- עזרי high-level:
  - ‎`requestSignature(tx, metadata)`‎
  - ‎`approveSession(account, permissions)`‎
  - ‎`reject(reason)`‎
- טיפול בשגיאות: מיפוי קודי Norito לשגיאות SDK, כולל קודים דומייניים לממשק המשתמש.

## עבודה Offline וחיבורים מחדש

### חוזה יומן

כל SDK מנהל יומן append-only לכל סשן כדי ש־dApp וארנק יוכלו לתור פריימים כאשר החיבור מנותק, לחזור ללא אובדן נתונים ולהציג חומרים ראייתיים לטלמטריה. החוזה משקף את טיפוסי Norito של הגשר, כך שאותו ייצוג בינארי נשמר על פני כל הפלטפורמות.

- היומנים נשמרים תחת מזהה סשן מגובב (`sha256(sid)`) ומיוצרים כשני קבצים לכל סשן: ‎`app_to_wallet.queue`‎ ו־‎`wallet_to_app.queue`‎. Swift משתמשת בעטיפת קבצים בספריית ‎Application Support‎, Android כותבת דרך ‎Room/FileChannel‎ מוצפן, ו-JS שומר ב-IndexedDB; כל הפורמטים בינאריים ויציבים.
- כל רשומה מסדרת את ‎`ConnectJournalRecordV1`‎:
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]` (גיבוב Blake3 של הצופן והכותרות)
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (המסגרת המוצפנת בדיוק כפי שנשמרה)
- היומנים שומרים את המטען המוצפן כפי שהוא. אין הצפנה מחדש; AEAD כבר מאמת את מפתחות הכיוון ולכן הפעולה מסתכמת ב‑fsync לרשומה שנוספה.
- מבנה ‎`ConnectQueueState`‎ בזיכרון משקף את המטא-דאטה (עומק, נפח בתים, רצף ישן/חדש) ומזין את יצואי הטלמטריה ואת מנגנון ה‑FlowControl.
- ברירת המחדל: 32 פריימים או ‎1 MiB‎ לכל היותר; חריגה מפנה את הרשומה הוותיקה ומסמנת ‎`reason=overflow`‎. הפרמטר ‎`ConnectFeatureConfig.max_queue_len`‎ מאפשר התאמה לdeployment.
- שימור הנתונים עומד על ‎24h‎ (`expires_at_ms`), ומנגנון ה-GC ברקע מוחק מקטעים שפג תוקפם כדי למנוע נפח חופשי שגדל ללא בקרה.
- שרידות קריסה: קודם כותבים, מבצעים fsync ומעדכנים את המראה בזיכרון לפני שמחזירים שליטה למתקשר. בעת עלייה, SDK-ים סורקים את הספרייה, מאמתים checksum ומבנים מחדש את ‎`ConnectQueueState`‎; רשומות פגומות מדולגות, מקבלות דגל טלמטריה ואופציונלית מבודדות לצוות התמיכה.
- מכיוון שהצופן כבר מקיים את מעטפת הפרטיות, נרשם רק מזהה הסשן המגובב. אפליקציות יכולות להפעיל ‎`telemetry_opt_in = false`‎ כדי לשמר יומנים אך להסתיר מדדי עומק ולחסום ייצוא ‎`sid`‎ ללוגים.
- SDK-ים חושפים ‎`ConnectQueueObserver`‎ כדי שאפליקציות יוכלו לצפות בעומק, ניקוז ו‑GC; ה-hook מזין UI סטטוס ללא צורך בניתוח לוגים.

### סמנטיקת השמעה חוזרת ורזום

1. בעת התחברות מחדש, ה-SDK שולח ‎`Control::Resume`‎ עם ‎`{seq_app_max, seq_wallet_max, queued_app, queued_wallet, journal_hash}`‎. הגיבוב הוא Blake3 של היומן, כך שניתן לזהות סטייה בין הצדדים.
2. הצד המקבל משווה את המטען למצב שלו, מבקש שידור חוזר אם קיימים חורים ומאשר פריימים שנשלחו מחדש בעזרת ‎`Control::ResumeAck`‎.
3. פריימים שמושמעים מחדש נשמרים לפי סדר הכנסת ‎(sequence ואז זמן כתיבה)‎. ארנק חייב להפעיל back-pressure באמצעות ‎`FlowControl`‎ כדי למנוע מה-dApp להציף את התור בזמן שהוא Offline.
4. מכיוון שהיומן שומר את המידע כ־ciphertext, השמעה מחדש פשוט מזרים את הבתים דרך התמסורת והמפענח; אסור לבצע קידוד מחדש בצד ה-SDK.

### זרימת התחברות מחדש

1. שכבת התמסורת פותחת מחדש WebSocket ומסכמת מרווחי ping חדשים.
2. ה-dApp מזרים מחדש את הפריימים לפי סדר, בהתאם ל-token-ים של FlowControl שמגיעים מהארנק (`ConnectSession.nextControlFrame()`).
3. הארנק מפענח תוצאות, בודק מונוטוניות רצף ומשמיע אישורים/תוצאות שהמתינו.
4. שני הצדדים משדרים ‎`resume`‎ המסכם ‎`seq_app_max/seq_wallet_max`‎ ועומקי תור לצורכי טלמטריה.
5. פריימים כפולים (אותו ‎`sequence`‎ + ‎`payload_hash`‎) מאושרים ומושלכים; קונפליקטים מפיקים ‎`ConnectError.Internal`‎ ומכריחים פתיחה של ‎`sid`‎ חדש.

### מצבי כשל

- כאשר הסשן נחשב מיושן (`offline_timeout_ms`, ברירת מחדל 5 דקות) הפריימים נמחקים וה-SDK מרים ‎`ConnectError.sessionExpired`‎.
- פגם ביומן מפעיל ניסיון תיקון אחד באמצעות Norito; אם נכשל, היומן נמחק ומסומן ב־`connect.queue_repair_failed`.
- אי התאמה במספור רצפים מפיקה ‎`ConnectError.replayDetected`‎ ומכריחה התחלה חדשה עם ‎`sid`‎ אחר.

### תכנית באפרים לא־מקוונים ושליטה תפעולית

Deliverable הסדנה דורש תכנית מפורטת כך שכל ה-SDK-ים יספקו אותה התנהגות, שליטה והתממשקות ראייתית. הטבלה משותפת ל‑Swift (`ConnectSessionDiagnostics`), Android (`ConnectDiagnosticsSnapshot`) ו-JS (`ConnectQueueInspector`):

| מצב | טריגר | תגובה אוטומטית | עקיפה ידנית | תג טלמטריה |
|-----|-------|----------------|-------------|------------|
| `Healthy` | שימוש בתור < ‎`disk_watermark_warn`‎ (‎60 %‎) וה‑TTL בתוקף | ללא פעולה | — | `connect.queue_state="healthy"` |
| `Throttled` | שימוש ≥ ‎`disk_watermark_warn`‎ או יותר מ‑5 ניסיונות חיבור לדקה | עצירת בקשות חתימה חדשות, פליטת FlowControl בחצי הקצב | `clearOfflineQueue(.app|.wallet)` מנקה צד אחד וה‑SDK מסתנכרן מול העמית | `connect.queue_state="throttled"`, מד הגובה `connect.queue_watermark` |
| `Quarantined` | שימוש ≥ ‎`disk_watermark_drop`‎ (‎85 %‎), שתי שגיאות corruption או חריגה מ-`offline_timeout_ms` | עצירת באפרינג, ‎`ConnectError.QueueQuarantined`‎ ודורש אישור מפעיל | `ConnectSessionDiagnostics.forceReset()` מוחק לאחר יצוא | `connect.queue_state="quarantined"`, מונה `connect.queue_quarantine_total` |

- הספים מוגדרים ב־`ConnectFeatureConfig` (`disk_watermark_warn`, `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). אם הערך לא מסופק, ה-SDK מחזיר ברירת מחדל ומדפיס אזהרה כדי שצוותי תפעול יוכלו לאמת מהטלמטריה.
- כלי האבחון זמינים בכל פלטפורמה:
  - Swift: ‎`ConnectSessionDiagnostics.snapshot()`‎ מחזיר `{state, depth, bytes, reason}` ו‎`exportJournalBundle(url:)`‎ יוצר ארכיון לשירות התמיכה.
  - Android: ‎`ConnectDiagnostics.snapshot()`‎ ו־‎`exportJournalBundle(path)`‎.
  - JS: ‎`ConnectQueueInspector.read()`‎ מחזיר מבנה זהה וידית Blob שניתן להעלות לכלי Torii.
- כאשר משתמש מכבה ‎`offline_queue_enabled=false`‎, ה-SDK מרוקן מיד את שני היומנים, מסמן ‎`Disabled`‎ ומשדר אירוע טלמטריה סופי. ההעדפה נרשמת גם במסגרת האישור כדי שהצד השני יידע שאין מה להשמיע מחדש.
- מפעילים מריצים ‎`connect queue inspect --sid <sid>`‎ (פקודת CLI העוטפת את ההוקים של ה-SDK) בזמן תרגילי כאוס; הפלט כולל היסטוריית מצבים, גבהים ורזום כך שסקרי הממשל אינם תלויים בכלי פלטפורמה פרטיים.

### זרימת חבילת ראיות

צוותי תמיכה וציות זקוקים לחומר קבוע בעת ביקורת לחוצה-Offline. כל SDK מספק יצוא תלת-שלבי:

1. ‎`exportJournalBundle(..)`‎ כותב את ‎`{app_to_wallet,wallet_to_app}.queue`‎ ומניפסט עם גרסת build, feature flags וגבהים.
2. ‎`exportQueueMetrics(..)`‎ פולט את 1 000 מדגמי הטלמטריה האחרונים כדי שניתן יהיה לשחזר דשבורדים offline (כולל ‎`sid`‎ מגובב כאשר המשתמש הסכים לכך).
3. כלי ה-CLI אורז את שני הייצוא ומוסיף קובץ Norito חתום (`ConnectQueueEvidenceV1`) כדי שטורי יוכל להעלות את החבילה ל‑SoraFS.

חבילות שלא עומדות באימות נדחות עם ‎`connect.evidence_invalid`‎ כדי שהצוותים יקבלו התראה ויתקנו את המייצא.

## טלמטריה ואבחון

- ייצוא אירועי Norito JSON דרך OpenTelemetry. מדדים חובה:
  - ‎`connect.queue_depth{direction}`‎ (מד עומק) המבוסס על ‎`ConnectQueueState`‎.
  - ‎`connect.queue_bytes{direction}`‎ (מד נפח דיסק).
  - ‎`connect.queue_dropped_total{reason}`‎ למקרים ‎`overflow|ttl|repair`‎.
  - ‎`connect.offline_flush_total{direction}`‎ ו־‎`connect.offline_flush_failed`‎.
  - מונים ‎`connect.replay_success_total`‎ / ‎`connect.replay_error_total`‎.
  - היסטוגרמות ‎`connect.resume_latency_ms`‎ ו־‎`connect.session_duration_ms`‎, ומונה ‎`connect.resume_attempts_total`‎.
  - אירועי ‎`connect.error`‎ עם ‎`code`‎, ‎`fatal`‎ ופרופיל טלמטריה.
- המייצאים מוסיפים ‎`{platform, sdk_version, feature_hash}`‎ כדי שניתן יהיה לפלח דשבורדים; ‎`sid`‎ מגובב משודר רק כאשר המשתמש מאשר.
- ה-SDK מספק hooks ברמת האפליקציה:
  - Swift: ‎`ConnectSession.addObserver(_:) -> ConnectEvent`‎.
  - Android: ‎`Flow<ConnectEvent>`‎.
  - JS: איטרטור אסינכרוני או callback.
- CI: Swift מריץ ‎`make swift-ci`‎, ‏Android את ‎`./gradlew sdkConnectCi`‎ ו‑JS את ‎`npm run test:connect`‎ כדי להבטיח שהטלמטריה ירוקה לפני מיזוג.
- לוגים מובְנים כוללים ‎`sid`‎ מגובב, ‎`seq`‎, עומק תור ו־`sid_epoch`; יומנים שקרסו משדרים ‎`connect.queue_repair_failed{reason}`‎ ונתיב dump אופציונלי.

### קרסי טלמטריה וראיות לממשל

- ‎`connect.queue_state`‎ משמש גם סמן סיכון במפת הדרכים. דשבורדים מתייגים לפי ‎`{platform, sdk_version}`‎ ומציגים זמן בכל מצב כדי שסוקרי הממשל יאספו הוכחות חודשיות.
- ‎`connect.queue_watermark`‎ ו־‎`connect.queue_bytes`‎ מזינים את מדד הסיכון ‎`risk.connect.offline_buffer`‎ שמדליק התרעה כאשר יותר מ‑5 % מהסשנים נמצאים מעל 10 דקות ב־`Throttled`.
- המייצאים מוסיפים ‎`feature_hash`‎ לכל אירוע כדי שכלי הביקורת יוודאו שהגרסה שאושרה היא זו שרצה בפועל; CI מפסיק build אם מתקבל hash לא מוכר.
- כאשר מדדי המדיניות נחצים, ה-SDK משדר ‎`connect.policy_violation`‎ הכולל ‎`sid`‎ מגובב, מצב ופעולת התיקון (`drain|purge|quarantine`).
- ‎`exportQueueMetrics`‎ נשמר באותו מרחב SoraFS כמו חבילות ה-Connect Runbook, כך שסוקרי המועצה יכולים לעקוב אחרי כל Drill בלי לגשת ללוגים פנימיים.

## שאלות פתוחות

1. **גילוי סשן:** האם נדרש QR או handshake חיצוני כמו WalletConnect? (תכנון עתידי)
2. **רב-חתימה:** כיצד לייצג אישורים מרובי חתימות? (הרחבת תוצאת חתימה לכמה חתימות)
3. **ציות:** אילו שדות חובה בזרמי רגולציה (כפי שמוגדר במפת הדרכים)? (הנחיות צוות ציות)
4. **אריזת SDK:** האם עלינו לחלץ קוד משותף (למשל Norito Connect codecs) לקרייט חוצה-פלטפורמות? (טרם הוחלט)

## צעדים הבאים

- להפיץ את הטיוטה למועצת ה-SDK (פברואר ‎2026).
- לאסוף משוב על השאלות הפתוחות ולעדכן את המסמך.
- לתזמן פירוק משימות לכל SDK (Swift IOS7, ‏Android AND7, ‏JS Connect).
- לעקוב אחר ההתקדמות ברשימת ה-hot roadmap ולעדכן את `status.md` לאחר שהטיוטה תאושר.

</div>
