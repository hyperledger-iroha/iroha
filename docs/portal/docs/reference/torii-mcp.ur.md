<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii MCP API
description: Torii کا مقامی ماڈل سیاق و سباق پروٹوکول برج استعمال کرنے کے لیے حوالہ گائیڈ۔
---

Torii `/v1/mcp` پر ایک مقامی ماڈل سیاق و سباق پروٹوکول (MCP) برج کو بے نقاب کرتا ہے۔
یہ اختتامی نقطہ ایجنٹوں کو ٹولز دریافت کرنے اور JSON-RPC کے ذریعے Torii/کنیکٹ روٹس کی درخواست کرنے دیتا ہے۔

## اختتامی نقطہ کی شکل

- `GET /v1/mcp` صلاحیتوں کا میٹا ڈیٹا واپس کرتا ہے (JSON-RPC لپیٹے ہوئے نہیں)۔
- `POST /v1/mcp` JSON-RPC 2.0 کی درخواستیں قبول کرتا ہے۔
- اگر `torii.mcp.enabled = false`، کوئی بھی راستہ سامنے نہیں آتا ہے۔
- اگر `torii.require_api_token` فعال ہے، تو JSON-RPC ڈسپیچ سے پہلے غائب/غلط ٹوکن کو مسترد کر دیا جاتا ہے۔

## کنفیگریشن

`torii.mcp` کے تحت MCP کو فعال کریں:

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

کلیدی طرز عمل:

- `profile` ٹول کی مرئیت کو کنٹرول کرتا ہے (`read_only`, `writer`, `operator`)۔
- `allow_tool_prefixes`/`deny_tool_prefixes` نام پر مبنی اضافی پالیسی کا اطلاق کریں۔
- `rate_per_minute`/`burst` MCP درخواستوں کے لیے ٹوکن-بکٹ کی حد بندی کا اطلاق کریں۔
- `tools/call_async` سے Async ملازمت کی حالت `async_job_ttl_secs` اور `async_job_max_entries` کا استعمال کرتے ہوئے ان میموری کو برقرار رکھا جاتا ہے۔

## تجویز کردہ کلائنٹ کا بہاؤ

1. `initialize` پر کال کریں۔
2. `tools/list` کو کال کریں اور `toolsetVersion` کو کیش کریں۔
3. عام کاموں کے لیے `tools/call` استعمال کریں۔
4. طویل آپریشنز کے لیے `tools/call_async` + `tools/jobs/get` استعمال کریں۔
5. `tools/list` کو دوبارہ چلائیں جب `listChanged` `true` ہو۔

مکمل ٹول کیٹلاگ کو ہارڈ کوڈ نہ کریں۔ رن ٹائم پر دریافت کریں۔

## طریقے اور الفاظ

تعاون یافتہ JSON-RPC طریقے:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

نوٹس:- `tools/list` `toolset_version` اور `toolsetVersion` دونوں کو قبول کرتا ہے۔
- `tools/jobs/get` `job_id` اور `jobId` دونوں کو قبول کرتا ہے۔
- `tools/list.cursor` ایک عددی سٹرنگ آفسیٹ ہے۔ غلط اقدار `0` پر واپس آتی ہیں۔
- `tools/call_batch` فی آئٹم بہترین کوشش ہے (ایک ناکام کال بہن بھائی کی کالوں کو ناکام نہیں کرتی ہے)۔
- `tools/call_async` فوری طور پر صرف لفافے کی شکل کی توثیق کرتا ہے۔ عمل درآمد کی غلطیاں بعد میں ملازمت کی حالت میں ظاہر ہوتی ہیں۔
- `jsonrpc` `"2.0"` ہونا چاہئے؛ چھوڑ دیا گیا `jsonrpc` مطابقت کے لیے قبول کیا جاتا ہے۔

## تصدیق اور آگے بڑھانا

MCP ڈسپیچ Torii کی اجازت کو نظرانداز نہیں کرتا ہے۔ کالز نارمل روٹ ہینڈلرز اور توثیق کی جانچ پڑتال کرتی ہیں۔

Torii ٹول ڈسپیچ کے لیے ان باؤنڈ تصنیف سے متعلق ہیڈرز کو آگے بھیجتا ہے:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

کلائنٹ `arguments.headers` کے ذریعے اضافی فی کال ہیڈرز بھی فراہم کر سکتے ہیں۔
`content-length`، `host`، اور `connection` سے `arguments.headers` کو نظر انداز کر دیا گیا ہے۔

## خرابی کا ماڈل

HTTP پرت:

- `400` غلط JSON
- JSON-RPC ہینڈلنگ سے پہلے `403` API ٹوکن مسترد کر دیا گیا
- `413` پے لوڈ `max_request_bytes` سے زیادہ ہے
- `429` شرح محدود
- JSON-RPC جوابات کے لیے `200` (بشمول JSON-RPC کی خرابیاں)

JSON-RPC پرت:- اعلی سطحی `error.data.error_code` مستحکم ہے (مثال کے طور پر `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I10072X, I10072)۔
- `isError = true` اور ساختی تفصیلات کے ساتھ MCP ٹول کے نتائج کے طور پر ٹول کی ناکامی سطح۔
- روٹ سے بھیجے گئے ٹول کی ناکامی HTTP اسٹیٹس کو `structuredContent.error_code` پر نقشہ کرتی ہے (مثال کے طور پر `forbidden`, `not_found`, `server_error`)۔

## ٹول کا نام دینا

OpenAPI سے ماخوذ ٹولز مستحکم روٹ پر مبنی نام استعمال کرتے ہیں:

- `torii.<method>_<path...>`
- مثال: `torii.get_v1_accounts`

کیوریٹ شدہ عرفی نام بھی `iroha.*` اور `connect.*` کے تحت سامنے آئے ہیں۔

## کیننیکل تفصیلات

مکمل تار کی سطح کا معاہدہ اس میں برقرار ہے:

- `crates/iroha_torii/docs/mcp_api.md`

جب سلوک `crates/iroha_torii/src/mcp.rs` یا `crates/iroha_torii/src/lib.rs` میں تبدیل ہوتا ہے،
اسی تبدیلی میں اس قیاس کو اپ ڈیٹ کریں اور پھر کلیدی استعمال کی رہنمائی کو یہاں آئینہ دیں۔