<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
id: torii-mcp
title: Torii MCP API
description: מדריך עזר לשימוש בגשר ה-Model Context Protocol המקורי של Torii.
---

Torii חושף גשר מקורי של Model Context Protocol (MCP) ב-`/v1/mcp`.
נקודת קצה זו מאפשרת לסוכנים לגלות כלים ולהפעיל מסלולי Torii/Connect דרך JSON-RPC.

## צורת נקודת קצה

- `GET /v1/mcp` מחזיר מטא נתונים של יכולות (לא עטוף ב-JSON-RPC).
- `POST /v1/mcp` מקבל בקשות JSON-RPC 2.0.
- אם `torii.mcp.enabled = false`, אף מסלול לא חשוף.
- אם `torii.require_api_token` מופעל, אסימון חסר/לא חוקי נדחה לפני שליחת JSON-RPC.

## תצורה

אפשר MCP תחת `torii.mcp`:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

התנהגות מפתח:

- `profile` שולט בנראות הכלים (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` להחיל מדיניות נוספת מבוססת שמות.
- `rate_per_minute`/`burst` החל הגבלת דלי אסימון עבור בקשות MCP.
- מצב עבודה אסינכרוני מ-`tools/call_async` נשמר בזיכרון באמצעות `async_job_ttl_secs` ו-`async_job_max_entries`.

## זרימת לקוח מומלצת

1. התקשר ל-`initialize`.
2. התקשר ל-`tools/list` ושמור את המטמון `toolsetVersion`.
3. השתמש ב-`tools/call` לפעולות רגילות.
4. השתמש ב-`tools/call_async` + `tools/jobs/get` לפעולות ארוכות יותר.
5. הפעל מחדש את `tools/list` כאשר `listChanged` הוא `true`.

אל תקוד קשיח את קטלוג הכלים המלא. גלה בזמן ריצה.

## שיטות וסמנטיקה

שיטות JSON-RPC נתמכות:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

הערות:- `tools/list` מקבל גם `toolset_version` וגם `toolsetVersion`.
- `tools/jobs/get` מקבל גם `job_id` וגם `jobId`.
- `tools/list.cursor` הוא היסט מחרוזת מספרית; ערכים לא חוקיים נופלים בחזרה ל-`0`.
- `tools/call_batch` הוא המאמץ הטוב ביותר לכל פריט (שיחה אחת שנכשלה אינה מכשילה שיחות אחים).
- `tools/call_async` מאמת רק את צורת המעטפה באופן מיידי; שגיאות ביצוע מופיעות מאוחר יותר במצב העבודה.
- `jsonrpc` צריך להיות `"2.0"`; הושמט `jsonrpc` מקובל עבור תאימות.

## אימות והעברה

שליחת MCP אינה עוקפת את הרשאת Torii. שיחות מבצעות מטפלי מסלול רגילים ובדיקות אימות.

Torii מעביר כותרות נכנסות הקשורות לאישור עבור שליחת הכלים:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

לקוחות יכולים גם לספק כותרות נוספות לכל שיחה באמצעות `arguments.headers`.
מתעלמים מ-`content-length`, `host` ו-`connection` מ-`arguments.headers`.

## מודל שגיאה

שכבת HTTP:

- `400` JSON לא חוקי
- אסימון API `403` נדחה לפני הטיפול ב-JSON-RPC
- מטען `413` עולה על `max_request_bytes`
- `429` מוגבל בתעריף
- `200` עבור תגובות JSON-RPC (כולל שגיאות JSON-RPC)

שכבת JSON-RPC:- `error.data.error_code` ברמה העליונה יציב (לדוגמה `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, OpenAPI, I000NI).
- כשלים בכלי צצים כתוצאות של כלי MCP עם `isError = true` ופרטים מובנים.
- כשלים בכלי שנשלחו במסלול ממפים את סטטוס HTTP ל-`structuredContent.error_code` (לדוגמה `forbidden`, `not_found`, `server_error`).

## מתן שמות לכלי

כלים שמקורם ב-OpenAPI משתמשים בשמות מבוססי מסלול יציבים:

- `torii.<method>_<path...>`
- דוגמה: `torii.get_v1_accounts`

כינויים שנאספו נחשפים גם תחת `iroha.*` ו-`connect.*`.

## מפרט קנוני

החוזה ברמת החוט המלא נשמר ב:

- `crates/iroha_torii/docs/mcp_api.md`

כאשר התנהגות משתנה ב-`crates/iroha_torii/src/mcp.rs` או `crates/iroha_torii/src/lib.rs`,
עדכן את המפרט באותו שינוי ולאחר מכן שיקף כאן את הנחיות השימוש במפתח.