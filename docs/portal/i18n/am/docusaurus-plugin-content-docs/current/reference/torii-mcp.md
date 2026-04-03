<!-- Auto-generated stub for Amharic (am) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: am
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
description: የTorii ቤተኛ የሞዴል አውድ ፕሮቶኮል ድልድይ ለመጠቀም የማጣቀሻ መመሪያ።
---

Torii ቤተኛ የሞዴል አውድ ፕሮቶኮል (MCP) ድልድይ በ`/v1/mcp` ያጋልጣል።
ይህ የመጨረሻ ነጥብ ወኪሎች መሳሪያዎችን እንዲያገኙ እና Torii/በJSON-RPC በኩል መንገዶችን እንዲያገናኙ ያስችላቸዋል።

## የመጨረሻ ነጥብ ቅርፅ

- `GET /v1/mcp` የችሎታዎችን ሜታዳታ ይመልሳል (JSON-RPC ተጠቅልሎ አይደለም)።
- `POST /v1/mcp` የ JSON-RPC 2.0 ጥያቄዎችን ይቀበላል።
- `torii.mcp.enabled = false` ከሆነ, የትኛውም መንገድ አይጋለጥም.
- `torii.require_api_token` ከነቃ፣ ከJSON-RPC መላክ በፊት የጎደለ/ልክ ያልሆነ ማስመሰያ ውድቅ ተደርጓል።

## ማዋቀር

በ `torii.mcp` ስር MCPን አንቃ፡

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

ቁልፍ ባህሪ፡

- `profile` የመሣሪያ ታይነትን ይቆጣጠራል (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` ተጨማሪ ስም-ተኮር ፖሊሲን ይተግብሩ።
- `rate_per_minute`/`burst` ለኤምሲፒ ጥያቄዎች ማስመሰያ-ባልዲ መገደብ ይተግብሩ።
- ከ `tools/call_async` Async የስራ ሁኔታ `async_job_ttl_secs` እና `async_job_max_entries` በመጠቀም በማህደረ ትውስታ ውስጥ እንዲቆይ ተደርጓል።

## የሚመከር የደንበኛ ፍሰት

1. `initialize` ይደውሉ።
2. `tools/list` እና መሸጎጫ `toolsetVersion` ይደውሉ።
3. ለመደበኛ ስራዎች `tools/call` ይጠቀሙ።
4. ለረጅም ስራዎች `tools/call_async` + `tools/jobs/get` ይጠቀሙ።
5. `listChanged` `true` ሲሆን `tools/list`ን እንደገና ያሂዱ።

ሙሉውን የመሳሪያ ካታሎግ ሃርድ ኮድ አታድርጉ። በሂደት ጊዜ ያግኙ።

## ዘዴዎች እና ትርጓሜዎች

የሚደገፉ የJSON-RPC ዘዴዎች፡-

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

ማስታወሻዎች፡-- `tools/list` ሁለቱንም `toolset_version` እና `toolsetVersion` ይቀበላል።
- `tools/jobs/get` ሁለቱንም `job_id` እና `jobId` ይቀበላል።
- `tools/list.cursor` የቁጥር ሕብረቁምፊ ማካካሻ ነው; ልክ ያልሆኑ እሴቶች ወደ `0` ይመለሳሉ።
- `tools/call_batch` ምርጥ-ጥረት በንጥል ነው (አንድ ያልተሳካ ጥሪ የወንድም እህት ጥሪዎችን አያጠፋም)።
- `tools/call_async` ወዲያውኑ የፖስታ ቅርጽ ብቻ ያረጋግጣል; የአፈፃፀም ስህተቶች ከጊዜ በኋላ በስራ ሁኔታ ውስጥ ይታያሉ.
- `jsonrpc` `"2.0"` መሆን አለበት; የተተወ `jsonrpc` ለተኳሃኝነት ተቀባይነት አለው።

## ማረጋገጥ እና ማስተላለፍ

የኤምሲፒ መላኪያ የTorii ፍቃድ አያልፍም። ጥሪዎች መደበኛ የመንገድ ተቆጣጣሪዎችን እና የማረጋገጫ ፍተሻዎችን ያከናውናሉ።

Torii ከውስጥ ከውት ጋር የተገናኙ ራስጌዎችን ለመሳሪያ መላክ ያስተላልፋል፡

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

ደንበኞች እንዲሁ ተጨማሪ የጥሪ ራስጌዎችን በ`arguments.headers` በኩል ማቅረብ ይችላሉ።
`content-length`፣ `host`፣ እና `connection` ከ `arguments.headers` ችላ ተብለዋል።

#የስህተት ሞዴል

HTTP ንብርብር፡-

- `400` ልክ ያልሆነ JSON
- `403` API token ከJSON-RPC አያያዝ በፊት ተቀባይነት አላገኘም።
- `413` ክፍያ ከ`max_request_bytes` ይበልጣል
- `429` ተመን-የተገደበ
- `200` ለJSON-RPC ምላሾች (የJSON-RPC ስህተቶችን ጨምሮ)

JSON-RPC ንብርብር፡-- ከፍተኛ-ደረጃ `error.data.error_code` የተረጋጋ ነው (ለምሳሌ `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I107000000000).
- የመሣሪያ አለመሳካቶች እንደ MCP መሣሪያ ውጤቶች በ`isError = true` እና የተዋቀሩ ዝርዝሮች።
- መንገድ የተላከ መሳሪያ አለመሳካቱ የኤችቲቲፒ ሁኔታን ወደ `structuredContent.error_code` (ለምሳሌ `forbidden`፣ `not_found`፣ `server_error`)።

## መሳሪያ መሰየም

OpenAPI-የተገኙ መሳሪያዎች የተረጋጋ መንገድ ላይ የተመሰረቱ ስሞችን ይጠቀማሉ፡-

- `torii.<method>_<path...>`
- ምሳሌ: `torii.get_v1_accounts`

የተሰየሙ ተለዋጭ ስሞች በ`iroha.*` እና `connect.*` ስር ተጋልጠዋል።

## ቀኖናዊ መግለጫ

የሙሉ ሽቦ ደረጃ ውል በሚከተሉት ውስጥ ተጠብቆ ይቆያል፡-

- `crates/iroha_torii/docs/mcp_api.md`

ባህሪ በ `crates/iroha_torii/src/mcp.rs` ወይም `crates/iroha_torii/src/lib.rs` ሲቀየር፣
ያንን ዝርዝር በተመሳሳይ ለውጥ ያዘምኑ እና ከዚያ ቁልፍ የአጠቃቀም መመሪያን እዚህ ያንጸባርቁ።