<!-- Hebrew translation of docs/connect_config.md -->

---
lang: he
direction: rtl
source: docs/connect_config.md
status: complete
translator: manual
---

<div dir="rtl">

## תצורת Torii Connect

Torii של Iroha חושף נקודות קצה אופציונליות בסגנון WalletConnect ונתיב ריליי מינימלי בתוך הצומת כאשר מאפיין ה-Cargo ‏`connect` פעיל (ברירת המחדל). ההתנהגות בזמן הריצה נשלטת דרך קובץ התצורה.

- הגדרת `connect.enabled=false` משביתה את כל מסלולי Connect (`/v2/connect/*`).
- השארת הערך `true` (ברירת המחדל) מפעילה את נקודות הקצה של WS ואת `/v2/connect/status`.

עקיפות באמצעות משתני סביבה (תצורת משתמש → תצורה בפועל):

- `CONNECT_ENABLED` ‏(bool; ברירת מחדל: `true`)
- `CONNECT_WS_MAX_SESSIONS` ‏(usize; ברירת מחדל: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` ‏(usize; ברירת מחדל: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` ‏(u32; ברירת מחדל: `120`)
- `CONNECT_FRAME_MAX_BYTES` ‏(usize; ברירת מחדל: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` ‏(usize; ברירת מחדל: `262144`)
- `CONNECT_PING_INTERVAL_MS` ‏(duration; ברירת מחדל: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` ‏(u32; ברירת מחדל: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` ‏(duration; ברירת מחדל: `15000`)
- `CONNECT_DEDUPE_CAP` ‏(usize; ברירת מחדל: `8192`)
- `CONNECT_RELAY_ENABLED` ‏(bool; ברירת מחדל: `true`)
- `CONNECT_RELAY_STRATEGY` ‏(string; ברירת מחדל: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` ‏(u8; ברירת מחדל: `0`)

הערות:

- ‏`CONNECT_SESSION_TTL_MS` ו-`CONNECT_DEDUPE_TTL_MS` משתמשים בערכי משך בתצורת המשתמש וממופים לשדות `session_ttl` ו-`dedupe_ttl` בתצורה האקטואלית.
- כאשר Connect מושבת בזמן הריצה (`connect.enabled=false`), מסלולי ה-WS והסטטוס אינם נרשמים; בקשות ל-`/v2/connect/ws` ו-`/v2/connect/status` יחזירו 404.
- אכיפת heartbeat קושרת את האינטרוול לתקרה עבור דפדפנים (`ping_min_interval_ms`), ממתינה ל-`ping_miss_tolerance` ניטורים לפני ניתוק, ומעדכנת את המטריקה `connect.ping_miss_total`.
- השרת דורש מזהה `sid` מסופק על ידי הלקוח עבור `/v2/connect/session` (base64url או הקס, 32 בתים). יצירת `sid` חלופי כבר אינה מתבצעת.

ראו גם: ‏`crates/iroha_config/src/parameters/{user,actual}.rs` והברירות המחדל ב-`crates/iroha_config/src/parameters/defaults.rs` (מודול `connect`).
</div>
