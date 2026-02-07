---
lang: he
direction: rtl
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## הפניה לתצורת API של הלקוח

מסמך זה עוקב אחר כפתורי התצורה הפונים ללקוח Torii שהם
משטחים דרך `iroha_config::parameters::user::Torii`. הסעיף למטה
מתמקד בבקרות התחבורה Norito-RPC שהוצגו עבור NRPC-1; עתיד
הגדרות ה-API של הלקוח צריכות להרחיב את הקובץ הזה.

### `torii.transport.norito_rpc`

| מפתח | הקלד | ברירת מחדל | תיאור |
|-----|------|--------|----------------|
| `enabled` | `bool` | `true` | מתג ראשי המאפשר פענוח Norito בינארי. כאשר `false`, Torii דוחה כל בקשת Norito-RPC עם `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | שכבת השקה: `disabled`, `canary`, או `ga`. השלבים מניעים החלטות קבלה ופלט `/rpc/capabilities`. |
| `require_mtls` | `bool` | `false` | אוכף mTLS עבור הובלת Norito-RPC: כאשר `true`, Torii דוחים בקשות Norito-RPC שאינן נושאות כותרת סמן mTLS (למשל I100NI3000). הדגל מוצג דרך `/rpc/capabilities` כך ש-SDKs יכולים להזהיר על סביבות שגויות בתצורה. |
| `allowed_clients` | `array<string>` | `[]` | רשימת ההיתרים הקנרית. כאשר `stage = "canary"`, רק בקשות הנושאות כותרת `X-API-Token` הקיימת ברשימה זו מתקבלות. |

תצורה לדוגמה:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

סמנטיקה בשלבים:

- **מושבת** — Norito-RPC אינו זמין גם אם `enabled = true`. לקוחות
  לקבל `403 norito_rpc_disabled`.
- **כנרית** - בקשות חייבות לכלול כותרת `X-API-Token` התואמת לכותרת אחת
  של `allowed_clients`. כל שאר הבקשות מקבלות `403
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC זמין לכל מתקשר מאומת (בכפוף ל-
  תעריף רגיל ומגבלות טרום אישור).

מפעילים יכולים לעדכן ערכים אלה באופן דינמי באמצעות `/v1/config`. כל שינוי
משתקף באופן מיידי ב-`/rpc/capabilities`, המאפשר SDKs וצפייה
לוחות מחוונים כדי להראות את תנוחת התחבורה החיה.