---
lang: he
direction: rtl
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# טקסונומיה של שגיאות חיבור (קו בסיס מהיר)

הערה זו עוקבת אחר IOS-CONNECT-010 ומתעדת את טקסונומיית השגיאות המשותפת עבור
Nexus Connect SDKs. סוויפט מייצאת כעת את העטיפה הקנונית `ConnectError`,
אשר ממפה את כל הכשלים הקשורים ל-Connect לאחת משש קטגוריות, כך שטלמטריה,
לוחות מחוונים, ועותק UX נשארים מיושרים בין הפלטפורמות.

> עדכון אחרון: 2026-01-15  
> בעלים: Swift SDK Lead (דייל טקסונומיה)  
> סטטוס: יישומי Swift + Android + JavaScript **נחתו**; שילוב תור בהמתנה.

## קטגוריות

| קטגוריה | מטרה | מקורות אופייניים |
|--------|--------|----------------|
| `transport` | כשלי הובלה של WebSocket/HTTP שמסיימים הפעלה. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | כשלים בסריאליזציה/גשר בזמן קידוד/פענוח מסגרות. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | כשלי TLS/אישור/מדיניות הדורשים תיקון משתמש או מפעיל. | תגובות `URLError(.secureConnectionFailed)`, Torii 4xx |
| `timeout` | תפוגות סרק/לא מקוונות וכלבי שמירה (תור TTL, פסק זמן של בקשה). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO מאותת לחץ אחורי כך שאפליקציות יכולות להשיל את הטעינה בחן. | `ConnectQueueError.overflow(limit:)` |
| `internal` | כל השאר: שימוש לרעה ב-SDK, גשר Norito חסר, יומנים פגומים. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

כל SDK מפרסם סוג שגיאה התואם את הטקסונומיה וחושף
תכונות טלמטריה מובנית: `category`, `code`, `fatal`, ואופציונלי
מטא נתונים (`http_status`, `underlying`).

## מיפוי מהיר

Swift מייצאת את `ConnectError`, `ConnectErrorCategory` ופרוטוקולי עוזר ב
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. כל שגיאת החיבור הציבורי
הסוגים תואמים ל-`ConnectErrorConvertible`, כך שאפליקציות יכולות להתקשר ל-`error.asConnectError()`
ולהעביר את התוצאה לשכבות טלמטריה/רישום.| שגיאה מהירה | קטגוריה | קוד | הערות |
|------------|--------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | מציין כפול `start()`; טעות של מפתח. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | מורם בעת שליחה/קבלה לאחר סגירה. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket סיפק מטען טקסטואלי תוך ציפייה לבינארי. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | הצד שכנגד סגר את הזרם באופן בלתי צפוי. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | האפליקציה שכחה להגדיר מקשים סימטריים. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito חסרים שדות חובה. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | מטען עתידי שנראה על ידי SDK ישן. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | גשר Norito חסר או נכשל בקידוד/פענוח בתים של מסגרת. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | גשר לא זמין או אורכי מפתח לא תואמים. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | אורך התור הלא מקוון חרג מהגבול שהוגדר. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | משטחים `URLSessionWebSocketTask`. |
| `URLError` מארזי TLS | `authorization` | `network.tls_failure` | כשלים במשא ומתן ATS/TLS. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | פענוח/קידוד JSON נכשל במקומות אחרים ב-SDK; ההודעה משתמשת בהקשר של מפענח Swift. |
| כל `Error` אחר | `internal` | `unknown_error` | תופסת מובטחת; מראות הודעות `LocalizedError`. |

בדיקות יחידה (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) ננעלות
המיפוי כך שגורמים עתידיים לא יכולים לשנות בשקט קטגוריות או קודים.

### שימוש לדוגמה

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## טלמטריה ודשבורדים

ה-SDK של Swift מספק `ConnectError.telemetryAttributes(fatal:httpStatus:)`
מה שמחזיר את מפת התכונות הקנונית. היצואנים צריכים להעביר אותם
תכונות לאירועי `connect.error` OTEL עם תוספות אופציונליות:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

לוחות מחוונים מתאמים מונים `connect.error` עם עומק תור (`connect.queue_depth`)
ולחבר מחדש היסטוגרמות כדי לזהות רגרסיות בלי להשמיע יומנים.

## מיפוי אנדרואידה-SDK של Android מייצא את `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions`, וכלי העזר תחת
`org.hyperledger.iroha.android.connect.error`. עוזרים בסגנון Builder להמיר כל `Throwable`
למטען תואם טקסונומיה, להסיק קטגוריות מחריגות תחבורה/TLS/codec,
ולחשוף תכונות טלמטריה דטרמיניסטיות כך ש-OpenTelemetry/ערימות דגימה יוכלו לצרוך את
תוצאה ללא מתאמים מותאמים אישית.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect:errors㑑
`ConnectQueueError` כבר מיישמת את `ConnectErrorConvertible`, פולטת את ה-CuueOverflow/פסק זמן
קטגוריות לתנאי גלישה/תפוגה, כך שמכשור בתור לא מקוון יכול להתחבר לאותה זרימה.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
ה-Android SDK README מתייחס כעת לטקסונומיה ומראה כיצד לעטוף חריגים בהובלה
לפני פליטת טלמטריה, שמירה על הנחיית dApp מיושרת עם קו הבסיס של Swift.【java/iroha_android/README.md:167】

## מיפוי JavaScript

לקוחות Node.js/browser ייבוא `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory`, ו
`connectErrorFrom()` מ-`@iroha/iroha-js`. העוזר המשותף בודק קודי מצב HTTP,
קודי שגיאה של צומת (שקע, TLS, פסק זמן), שמות `DOMException` וכישלונות קודקים לפליטת
אותן קטגוריות/קודים המתועדות בהערה זו, בעוד שהגדרות TypeScript מדגימות את הטלמטריה
עקיפות תכונה כך שהכלים יכולים לפלוט אירועי OTEL ללא ליהוק ידני.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
ה-SDK README מתעד את זרימת העבודה ומקשר חזרה לטקסונומיה זו כך שצוותי יישומים יכולים
העתק את קטעי המכשור מילה במילה.【javascript/iroha_js/README.md:1387】

## השלבים הבאים (חוצה SDK)

- **שילוב תור:** ברגע שהתור הלא מקוון נשלח, ודא לוגיקה של יציאת תור/הסרה
  משטחים ערכי `ConnectQueueError` כך שהטלמטריה על גדותיה נשארת אמינה.