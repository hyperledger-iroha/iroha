---
lang: he
direction: rtl
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/runbooks/connect_session_preview_runbook.md -->

# ראנבוק תצוגה מקדימה של סשני Connect (IOS7 / JS4)

ראנבוק זה מתעד את התהליך מקצה לקצה להכנה, אימות וסגירה של סשני תצוגה מקדימה של Connect כנדרש באבני הדרך **IOS7** ו-**JS4** (`roadmap.md:1340`, `roadmap.md:1656`). פעלו לפי שלבים אלו בכל פעם שמדגימים את ה-strawman של Connect (`docs/source/connect_architecture_strawman.md`), בודקים את ה-hooks של תור/טלמטריה שהובטחו במפות הדרכים של ה-SDK, או אוספים ראיות עבור `status.md`.

## 1. רשימת בדיקות מקדימה

| פריט | פרטים | הפניות |
|------|---------|------------|
| נקודת קצה Torii + מדיניות Connect | אשרו את כתובת הבסיס של Torii, את `chain_id` ואת מדיניות Connect (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). שמרו snapshot JSON בכרטיס הרנבוק. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| גרסאות fixture + bridge | רשמו את hash ה-fixture של Norito ואת build ה-bridge שבו תשתמשו (Swift דורש `NoritoBridge.xcframework`, JS דורש `@iroha/iroha-js` >= הגרסה שסיפקה `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| לוחות טלמטריה | ודאו שלוחות שמציגים `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` ועוד נגישים (לוח Grafana `Android/Swift Connect` + snapshots שהופקו מ-Prometheus). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| תיקיות ראיות | בחרו יעד כמו `docs/source/status/swift_weekly_digest.md` (דיג'סט שבועי) ו-`docs/source/sdk/swift/connect_risk_tracker.md` (מעקב סיכונים). שמרו לוגים, צילומי מדדים ואישורים תחת `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. אתחול סשן התצוגה המקדימה

1. **אימות מדיניות ומכסות.** קראו:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   הכשילו את הריצה אם `queue_max` או TTL שונים מהקונפיג שתכננתם לבדוק.
2. **צרו SID/URI דטרמיניסטיים.** העוזר `bootstrapConnectPreviewSession` של `@iroha/iroha-js` קושר את יצירת SID/URI לרישום הסשן ב-Torii; השתמשו בו גם כאשר Swift מפעיל את שכבת ה-WebSocket.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - הגדירו `register: false` לתרחישי QR/deep-link ב-dry-run.
   - שמרו את `sidBase64Url` שהוחזר, כתובות ה-deeplink, ואת blob ה-`tokens` בתיקיית הראיות; סקירת ה-governance מצפה לארטיפקטים אלו.
3. **הפצת סודות.** שתפו את URI ה-deeplink עם מפעיל הארנק (דוגמת dApp ב-Swift, ארנק Android, או harness QA). לעולם אל תדביקו טוקנים גולמיים בצ'אט; השתמשו בכספת המוצפנת המתועדת בחבילת ה-enablement.

## 3. הרצת הסשן

1. **פתחו WebSocket.** לקוחות Swift משתמשים לרוב:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   ראו `docs/connect_swift_integration.md` להגדרות נוספות (imports של bridge, מתאמי concurrency).
2. **אישורים וחתימות.** dApps קוראים ל-`ConnectSession.requestSignature(...)`, בעוד שהארנקים מגיבים דרך `approveSession` / `reject`. כל אישור חייב לרשום alias מגובב + הרשאות כדי להתאים לאמנת ה-governance של Connect.
3. **בדקו נתיבי תור וחידוש.** החליפו קישוריות רשת או השעו את הארנק כדי לוודא שהתור המוגבל ו-hooks של replay רושמים אירועים. ה-SDKs של JS/Android מפיקים `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` כאשר הם משליכים frames; Swift אמור לראות אותו הדבר לאחר שיגיע שלד התור של IOS7 (`docs/source/connect_architecture_strawman.md`). אחרי לפחות reconnect אחד, הריצו
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (או העבירו את ספריית היצוא שמוחזרת מ-`ConnectSessionDiagnostics`) וצירפו את הטבלה/JSON לכרטיס הרנבוק. ה-CLI קורא את אותו זוג `state.json` / `metrics.ndjson` שמייצר `ConnectQueueStateTracker`, כך שבודקי ה-governance יכולים לעקוב אחרי ראיות drill בלי כלי מיוחד.

## 4. טלמטריה ותצפיתיות

- **מדדים לאיסוף:**
  - `connect.queue_depth{direction}` gauge (יש לשמור מתחת לתקרת המדיניות).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (לא אפס רק בזמן הזרקת תקלות).
  - `connect.resume_latency_ms` histogram (רשמו p95 לאחר forcing של reconnect).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Exports של Swift: `swift.connect.session_event` ו-`swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **דשבורדים:** עדכנו את סימניות לוח Connect עם סמני אנוטציה. צרפו צילומים (או JSON exports) לתיקיית הראיות יחד עם snapshots של OTLP/Prometheus שנמשכו דרך CLI של exporter הטלמטריה.
- **התראות:** אם ספי Sev 1/2 מופעלים (ראו `docs/source/android_support_playbook.md` סעיף 5), פנו ל-SDK Program Lead ותעדו את מזהה התקרית של PagerDuty בכרטיס הרנבוק לפני שממשיכים.

## 5. ניקוי וגלגול לאחור

1. **מחקו סשנים ב-staging.** מחקו תמיד סשני preview כדי שהתרעות עומק התור יישארו משמעותיות:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   בריצות Swift בלבד, קראו לאותו endpoint דרך helper של Rust/CLI.
2. **נקו יומנים.** מחקו כל יומן תור שנשמר (`ApplicationSupport/ConnectQueue/<sid>.to`, מאגרי IndexedDB וכו') כדי שהריצה הבאה תתחיל נקיה. תעדו את hash הקובץ לפני מחיקה אם צריך לאבחן בעיית replay.
3. **תעדו הערות אירוע.** סכמו את הריצה ב:
   - `docs/source/status/swift_weekly_digest.md` (בלוק deltas),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (נקו או הורידו CR-2 לאחר שהטלמטריה זמינה),
   - changelog של SDK JS או המתכון אם אומת התנהגות חדשה.
4. **הסלמת כשלים:**
   - הצפת תור ללא תקלות מוזרקות => פתחו באג נגד ה-SDK שהמדיניות שלו חורגת מ-Torii.
   - שגיאות חידוש => צרפו snapshots של `connect.queue_depth` + `connect.resume_latency_ms` לדוח האירוע.
   - אי התאמות governance (שימוש חוזר בטוקנים, חריגה מ-TTL) => הסלימו ל-SDK Program Lead וסמנו זאת ב-`roadmap.md` בעדכון הבא.

## 6. רשימת ראיות

| ארטיפקט | מיקום |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| אישור ניקוי (Torii delete, journal wipe) | `.../cleanup.log` |

השלמת הרשימה הזו עומדת בקריטריון היציאה "docs/runbooks updated" עבור IOS7/JS4 ומספקת לבודקי ה-governance עקבה דטרמיניסטית לכל סשן preview של Connect.

</div>
