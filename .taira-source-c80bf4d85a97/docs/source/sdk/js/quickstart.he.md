<!-- Hebrew translation of docs/source/sdk/js/quickstart.md (Connect section) -->

---
lang: he
direction: rtl
source: docs/source/sdk/js/quickstart.md
status: draft
translator: LLM (Codex)
translation_last_reviewed: 2026-05-02
---

<div dir="rtl">

# JS/TS מדריך התחלה מהירה (קטע מתורגם)

לעת עתה תורגם רק הסעיף “Connect Sessions & Queueing”. לפרקים האחרים עיינו במסמך האנגלי.

## Connect סשנים וניהול תורים

עזרי Connect משקפים את הסטרומאן המתועד ב־`docs/source/connect_architecture_strawman.md`. הדרך הקלה ביותר להכין סשן ידידותי להדגמות היא לקרוא ל־`bootstrapConnectPreviewSession`, שמחברת בין יצירת SID/URI דטרמיניסטית לבין ממשק הרישום של Torii.

```js
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(torii, {
  chainId: "sora-mainnet",
  node: "https://torii.nexus.example",
  // אופציונלי: החלפת ה-node שבו מתבצע הרישום
  sessionOptions: { node: "https://torii.backup.example" },
});

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- ברירת המחדל של `bootstrapConnectPreviewSession` היא לרשום את הסשן ב-Torii. כאשר דרושות רק URI דטרמיניסטיות עבור QR/קישורי עומק, העבירו `register: false`.
- מאחורי הקלעים ההלפר מפעיל את `createConnectSessionPreview` ואחריה `ToriiClient.createConnectSession`, כך שאפשר להרכיב גם זרימות רב-שלביות עם ניסיונות חוזרים או אחסון מותאם.
- `generateConnectSid` ממשיכה להיות זמינה כאשר נדרש רק SID דטרמיניסטי ללא הנפקת URI (למשל שמירת SID בהארנס CI).
- המפתחות הדירקשונליים ומעטפות הצופן נוצרים על ידי הגשר הנייטיבי. כאשר הגשר אינו זמין, ה-SDK חוזר לקודק JSON וזורק `ConnectError.bridgeUnavailable`.
- חוצצי אופליין נשמרים כבלובי Norito `.to` ב-IndexedDB. עקבו אחר מצב התור באמצעות החריגות `ConnectQueueError.overflow(limit)` ו-`.expired(ttlMs)`, וחברו את הטלמטריה כדי לשדר את `connect.queue_depth`, ‏`connect.replay_success_total`, ו-`connect.resume_latency_ms` בהתאם לרודמפ.

</div>
