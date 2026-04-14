<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: uz
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
id: torii-mcp
title: Torii MCP API
description: Torii ning mahalliy Model kontekst protokoli ko'prigidan foydalanish bo'yicha ma'lumotnoma.
---

Torii `/v1/mcp` da mahalliy Model kontekst protokoli (MCP) ko'prigini ochib beradi.
Bu soʻnggi nuqta agentlarga asboblarni topish va JSON-RPC orqali Torii/Connect marshrutlarini chaqirish imkonini beradi.

## Oxirgi nuqta shakli

- `GET /v1/mcp` imkoniyatlar metama'lumotlarini qaytaradi (JSON-RPC o'ralgan emas).
- `POST /v1/mcp` JSON-RPC 2.0 so'rovlarini qabul qiladi.
- Agar `torii.mcp.enabled = false` bo'lsa, hech bir marshrut ko'rinmaydi.
- Agar `torii.require_api_token` yoqilgan bo'lsa, JSON-RPC jo'natmasidan oldin etishmayotgan/yaroqsiz token rad etiladi.

## Konfiguratsiya

`torii.mcp` ostida MCPni yoqing:

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

Asosiy xatti-harakatlar:

- `profile` asbob ko'rinishini boshqaradi (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` nomga asoslangan qoʻshimcha siyosatni qoʻllaydi.
- `rate_per_minute`/`burst` MCP so'rovlari uchun token-paqir cheklovini qo'llaydi.
- `tools/call_async` dan asinxron ish holati `async_job_ttl_secs` va `async_job_max_entries` yordamida xotirada saqlanadi.

## Tavsiya etilgan mijozlar oqimi

1. `initialize` raqamiga qo'ng'iroq qiling.
2. `tools/list` va `toolsetVersion` keshiga qo'ng'iroq qiling.
3. Oddiy operatsiyalar uchun `tools/call` dan foydalaning.
4. Uzunroq operatsiyalar uchun `tools/call_async` + `tools/jobs/get` dan foydalaning.
5. `listChanged` `true` bo'lganda `tools/list` ni qayta ishga tushiring.

To'liq asboblar katalogini qattiq kodlamang. Ishlash vaqtida kashf qiling.

## Usullar va semantika

Qo'llab-quvvatlanadigan JSON-RPC usullari:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Eslatmalar:- `tools/list` ikkala `toolset_version` va `toolsetVersion` ni qabul qiladi.
- `tools/jobs/get` ikkala `job_id` va `jobId` ni qabul qiladi.
- `tools/list.cursor` - raqamli qator ofset; yaroqsiz qiymatlar `0` ga qaytadi.
- `tools/call_batch` - har bir element uchun eng yaxshi harakat (bitta muvaffaqiyatsiz qo'ng'iroq aka-uka qo'ng'iroqlari muvaffaqiyatsiz tugamaydi).
- `tools/call_async` faqat konvert shaklini darhol tasdiqlaydi; bajarish xatolari ish holatida keyinroq paydo bo'ladi.
- `jsonrpc` `"2.0"` bo'lishi kerak; o'tkazib yuborilgan `jsonrpc` moslik uchun qabul qilinadi.

## Avtorizatsiya va yo'naltirish

MCP jo'natilishi Torii avtorizatsiyasini chetlab o'tmaydi. Qo'ng'iroqlar oddiy marshrut ishlovchilari va autentifikatsiya tekshiruvlarini amalga oshiradi.

Torii asboblarni jo'natish uchun kirish autentsiyasi bilan bog'liq sarlavhalarni yo'naltiradi:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Mijozlar `arguments.headers` orqali qo'shimcha qo'ng'iroq sarlavhalarini ham taqdim etishlari mumkin.
`content-length`, `host` va `arguments.headers` dan `connection` hisobga olinmaydi.

## Xato modeli

HTTP qatlami:

- `400` yaroqsiz JSON
- `403` API tokeni JSON-RPC bilan ishlashdan oldin rad etildi
- `413` foydali yuki `max_request_bytes` dan oshadi
- `429` tezligi cheklangan
- JSON-RPC javoblari uchun `200` (jumladan, JSON-RPC xatolari)

JSON-RPC qatlami:- Yuqori darajadagi `error.data.error_code` barqaror (masalan, `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I010X, I010).
- Asbobning nosozliklari MCP asbobi natijasida `isError = true` va tuzilgan tafsilotlar bilan yuzaga keladi.
- Yoʻnalish boʻyicha joʻnatilgan asboblar nosozliklari HTTP holatini `structuredContent.error_code` ga koʻrsatadi (masalan, `forbidden`, `not_found`, `server_error`).

## Asbobga nom berish

OpenAPI-dan olingan asboblar barqaror marshrutga asoslangan nomlardan foydalanadi:

- `torii.<method>_<path...>`
- Misol: `torii.get_v1_accounts`

Tanlangan taxalluslar `iroha.*` va `connect.*` ostida ham ochiladi.

## Kanonik spetsifikatsiya

To'liq simli darajadagi shartnoma quyidagilarda saqlanadi:

- `crates/iroha_torii/docs/mcp_api.md`

`crates/iroha_torii/src/mcp.rs` yoki `crates/iroha_torii/src/lib.rs` da xatti-harakatlar o'zgarganda,
o'sha o'zgarishda ushbu spetsifikatsiyani yangilang va keyin bu erda kalitdan foydalanish bo'yicha ko'rsatmalarni aks ettiring.