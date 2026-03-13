---
lang: he
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-11-11T10:26:38.026921+00:00"
translation_last_reviewed: 2026-01-01
---

# סקירה כללית של Norito-RPC

Norito-RPC הוא תעבורה בינארית עבור ממשקי Torii. הוא משתמש באותם נתיבי HTTP כמו `/v2/pipeline` אבל מחליף מטענים ממוסגרי Norito שכוללים hashes של סכמה ו-checksums. השתמשו בו כאשר נדרשות תשובות דטרמיניסטיות ומאומתות או כאשר תגובות JSON של pipeline הופכות לצוואר בקבוק.

## למה לעבור?
- מסגור דטרמיניסטי עם CRC64 ו-hashes של סכמה מפחית שגיאות פענוח.
- Helpers של Norito המשותפים בין SDKs מאפשרים שימוש חוזר בטיפוסי מודל נתונים קיימים.
- Torii כבר מתייג סשנים של Norito בטלמטריה, כך שמפעילים יכולים לעקוב אחרי האימוץ עם הדשבורדים שסופקו.

## ביצוע בקשה

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. בצעו סיריאליזציה של המטען עם codec Norito (`iroha_client`, helpers של ה-SDK או `norito::to_bytes`).
2. שלחו את הבקשה עם `Content-Type: application/x-norito`.
3. בקשו תגובת Norito באמצעות `Accept: application/x-norito`.
4. פענחו את התגובה באמצעות helper של ה-SDK המתאים.

הנחיות לפי SDK:
- **Rust**: `iroha_client::Client` מנהל משא ומתן על Norito אוטומטית כאשר מגדירים את הכותרת `Accept`.
- **Python**: השתמשו ב-`NoritoRpcClient` מתוך `iroha_python.norito_rpc`.
- **Android**: השתמשו ב-`NoritoRpcClient` וב-`NoritoRpcRequestOptions` ב-SDK של Android.
- **JavaScript/Swift**: ה-helpers מנוטרים ב-`docs/source/torii/norito_rpc_tracker.md` ויגיעו כחלק מ-NRPC-3.

## דוגמת קונסולת Try It

פורטל המפתחים כולל פרוקסי Try It כדי שריביוורים יוכלו לשחזר מטעני Norito בלי לכתוב סקריפטים ייעודיים.

1. [הפעילו את הפרוקסי](./try-it.md#start-the-proxy-locally) והגדירו `TRYIT_PROXY_PUBLIC_URL` כדי שהווידג'טים ידעו לאן לשלוח תעבורה.
2. פתחו את כרטיס **Try it** בעמוד זה או את הפאנל `/reference/torii-swagger` ובחרו endpoint כמו `POST /v2/pipeline/submit`.
   For MCP/agent flows, use `/reference/torii-mcp`.
3. החליפו את **Content-Type** ל-`application/x-norito`, בחרו את עורך **Binary**, והעלו `fixtures/norito_rpc/transfer_asset.norito` (או כל payload שמופיע ב-`fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. ספקו bearer token דרך הווידג'ט OAuth device-code או שדה הטוקן הידני (הפרוקסי מקבל overrides של `X-TryIt-Auth` כאשר מוגדר `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. שלחו את הבקשה ואמתו ש-Torii מחזיר את `schema_hash` שמופיע ב-`fixtures/norito_rpc/schema_hashes.json`. hashes תואמים מאשרים שהכותרת Norito שרדה את מעבר הדפדפן/פרוקסי.

לצורך ראיות roadmap, צרפו צילום מסך של Try It עם הרצה של `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. הסקריפט עוטף את `cargo xtask norito-rpc-verify`, כותב את תקציר ה-JSON ל-`artifacts/norito_rpc/<timestamp>/`, ולוכד את אותם fixtures שהפורטל צרך.

## פתרון תקלות

| תסמין | היכן מופיע | סיבה אפשרית | תיקון |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | תגובת Torii | כותרת `Content-Type` חסרה או שגויה | הגדירו `Content-Type: application/x-norito` לפני שליחת המטען. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | גוף/כותרות תגובת Torii | hash הסכמה של fixtures שונה מה-build של Torii | צרו מחדש fixtures עם `cargo xtask norito-rpc-fixtures` ואשרו את ה-hash ב-`fixtures/norito_rpc/schema_hashes.json`; חזרו ל-JSON אם ה-endpoint עדיין לא הפעיל Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | תגובת פרוקסי Try It | הבקשה הגיעה ממקור שאינו מופיע ב-`TRYIT_PROXY_ALLOWED_ORIGINS` | הוסיפו את מקור הפורטל (למשל `https://docs.devnet.sora.example`) למשתנה הסביבה והפעילו מחדש את הפרוקסי. |
| `{"error":"rate_limited"}` (HTTP 429) | תגובת פרוקסי Try It | מכסת IP חרגה מתקציב `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | הגדילו את המגבלה לבדיקות עומס פנימיות או המתינו לאיפוס החלון (ראו `retryAfterMs` בתגובת ה-JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) או `{"error":"upstream_error"}` (HTTP 502) | תגובת פרוקסי Try It | Torii נתקע או שהפרוקסי לא הצליח להגיע ל-backend המוגדר | ודאו ש-`TRYIT_PROXY_TARGET` נגיש, בדקו את מצב Torii או נסו שוב עם `TRYIT_PROXY_TIMEOUT_MS` גדול יותר. |

מידע נוסף על Try It וטיפים ל-OAuth נמצאים ב-[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## משאבים נוספים
- RFC תעבורה: `docs/source/torii/norito_rpc.md`
- תקציר מנהלים: `docs/source/torii/norito_rpc_brief.md`
- Tracker פעולות: `docs/source/torii/norito_rpc_tracker.md`
- הוראות פרוקסי Try-It: `docs/portal/docs/devportal/try-it.md`
