---
lang: he
direction: rtl
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2026-01-03T18:07:58.049266+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/connect_architecture_followups.md -->

# פעולות המשך לארכיטקטורת Connect

הערה זו מתעדת את משימות ההמשך ההנדסיות שעלו מסקירת ארכיטקטורת Connect בין SDKs.
כל שורה אמורה למפות ל‑issue (כרטיס Jira או PR) לאחר תזמון העבודה.
עדכנו את הטבלה כאשר הבעלים יוצרים כרטיסי מעקב.

| פריט | תיאור | בעלים | מעקב | סטטוס |
|------|-------------|----------|----------|--------|
| קבועי back‑off משותפים | לממש עזרי exponential back‑off + jitter (`connect_retry::policy`) ולחשוף אותם ל‑Swift/Android/JS SDKs. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | הושלם — `connect_retry::policy` נחת עם דגימת splitmix64 דטרמיניסטית; Swift (`ConnectRetryPolicy`), Android ו‑JS SDKs שולחים עזרים מקבילים עם בדיקות golden. |
| אכיפת ping/pong | להוסיף אכיפת heartbeat ניתנת להגדרה עם קצב מוסכם של 30 ש׳ ומינימום דפדפן; להוציא מטריקות (`connect.ping_miss_total`). | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | הושלם — Torii כעת אוכף מרווחי heartbeat ניתנים להגדרה (`ping_interval_ms`, `ping_miss_tolerance`, `ping_min_interval_ms`), חושף את המטריקה `connect.ping_miss_total`, ושולח בדיקות רגרסיה לטיפול בניתוק heartbeat. snapshots של SDK מציגים את ה‑knobs החדשים ללקוחות. |
| התמדה של תור offline | לממש כותבי/קוראי journal של Norito `.to` עבור תורי Connect (Swift `FileManager`, אחסון מוצפן ב‑Android, JS IndexedDB) באמצעות הסכמה המשותפת. | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | הושלם — Swift, Android ו‑JS שולחים כעת את `ConnectQueueJournal` + עזרי diagnostics משותפים עם בדיקות retention/overflow כך שחבילות הראיות נשארות דטרמיניסטיות בין SDKs.【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| מטען attestation של StrongBox | להעביר `{platform,evidence_b64,statement_hash}` דרך אישורי ארנק ולהוסיף אימות ל‑dApp SDKs. | Android Crypto TL, JS Lead | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | ממתין |
| מסגרת בקרה לסבב מפתחות | לממש `Control::RotateKeys` + `RotateKeysAck` ולחשוף `cancelRequest(hash)` / APIs לסבב בכל SDKs. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | ממתין |
| exporters טלמטריה | להפיק `connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`, ומוני replay לתוך צינורות טלמטריה קיימים (OpenTelemetry). | Telemetry WG, SDK owners | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | ממתין |
| gating CI ל‑Swift | לוודא שצינורות Connect מפעילים `make swift-ci` כדי ש‑fixture parity, dashboard feeds, ו‑Buildkite `ci/xcframework-smoke:<lane>:device_tag` יישארו מסונכרנים בין SDKs. | Swift SDK Lead, Build Infra | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | ממתין |
| דיווח תקריות fallback | לחווט את תקריות smoke של XCFramework (`xcframework_smoke_fallback`, `xcframework_smoke_strongbox_unavailable`) לדשבורדים של Connect לנראות משותפת. | Swift QA Lead, Build Infra | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | ממתין |
| העברת attachments תאימות | לוודא ש‑SDKs מקבלים ומעבירים שדות `attachments[]` + `compliance_manifest_id` ב‑payloads של אישור ללא אובדן. | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | ממתין |
| יישור טקסונומיית שגיאות | למפות את ה‑enum המשותף (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`) לשגיאות פלטפורמה עם דוגמאות/תיעוד. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | הושלם — Swift, Android ו‑JS SDKs שולחים מעטפת `ConnectError` משותפת + עזרי טלמטריה עם תיעוד README/TypeScript/Java ובדיקות רגרסיה שמכסות TLS/timeout/HTTP/codec/queue.【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
| יומן החלטות הסדנה | לפרסם מצגת/הערות מתויגות המסכמות החלטות שאושרו בארכיון המועצה. | SDK Program Lead | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | ממתין |

> מזהי מעקב יתמלאו כאשר הבעלים פותחים כרטיסים; עדכנו את עמודת `Status` לצד התקדמות ה‑issue.

</div>
