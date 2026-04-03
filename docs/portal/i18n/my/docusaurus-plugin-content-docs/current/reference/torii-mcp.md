<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: my
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
description: Torii ၏ဇာတိ Model Context Protocol တံတားကို အသုံးပြုခြင်းအတွက် အကိုးအကားလမ်းညွှန်။
---

Torii သည် `/v1/mcp` တွင် မူရင်း Model Context Protocol (MCP) တံတားကို ဖော်ထုတ်သည်။
ဤအဆုံးမှတ်သည် အေးဂျင့်များအား ကိရိယာများကို ရှာဖွေတွေ့ရှိစေပြီး JSON-RPC မှတစ်ဆင့် Torii/Connect လမ်းကြောင်းများကို ခေါ်ဆိုနိုင်စေပါသည်။

## အဆုံးမှတ်ပုံသဏ္ဍာန်

- `GET /v1/mcp` သည် လုပ်ဆောင်နိုင်စွမ်း မက်တာဒေတာကို ပြန်ပေးသည် (JSON-RPC ထုပ်ပိုးထားခြင်းမဟုတ်ပါ)။
- `POST /v1/mcp` သည် JSON-RPC 2.0 တောင်းဆိုချက်များကို လက်ခံသည်။
- `torii.mcp.enabled = false` ဆိုလျှင်၊ လမ်းကြောင်းနှစ်ခုကို ထိတွေ့မည်မဟုတ်ပါ။
- `torii.require_api_token` ကို ဖွင့်ထားလျှင် JSON-RPC ပေးပို့ခြင်းမပြုမီ ပျောက်ဆုံး/မမှန် တိုကင်ကို ပယ်ချပါမည်။

## ဖွဲ့စည်းမှု

`torii.mcp` အောက်တွင် MCP ကိုဖွင့်ပါ-

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

အဓိက အပြုအမူ-

- `profile` သည် တူးလ်မြင်နိုင်မှုကို ထိန်းချုပ်သည် (`read_only`၊ `writer`၊ `operator`)။
- `allow_tool_prefixes`/`deny_tool_prefixes` တွင် အမည်အခြေခံသည့် မူဝါဒကို ကျင့်သုံးပါ။
- `rate_per_minute`/`burst` MCP တောင်းဆိုမှုများအတွက် တိုကင်ပုံးကန့်သတ်ချက်ကို သက်ရောက်သည်။
- `tools/call_async` မှ Async အလုပ်အခြေအနေသည် `async_job_ttl_secs` နှင့် `async_job_max_entries` ကို အသုံးပြု၍ မမ်မိုရီတွင် ထိန်းသိမ်းထားသည်။

## ဖောက်သည်စီးဆင်းမှုကို အကြံပြုထားသည်။

1. `initialize` ကိုခေါ်ဆိုပါ။
2. `tools/list` နှင့် cache `toolsetVersion` ကိုခေါ်ဆိုပါ။
3. ပုံမှန်လုပ်ဆောင်မှုများအတွက် `tools/call` ကို အသုံးပြုပါ။
4. ပိုမိုကြာကြာလုပ်ဆောင်မှုများအတွက် `tools/call_async` + `tools/jobs/get` ကို အသုံးပြုပါ။
5. `listChanged` သည် `true` ဖြစ်သောအခါ `tools/list` ကို ပြန်ဖွင့်ပါ။

tool catalog အပြည့်အစုံကို hardcode မလုပ်ပါနဲ့။ runtime တွင်ရှာဖွေပါ။

## နည်းလမ်းများနှင့် ဝေါဟာရများ

ပံ့ပိုးထားသော JSON-RPC နည်းလမ်းများ-

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

မှတ်စုများ-- `tools/list` သည် `toolset_version` နှင့် `toolsetVersion` နှစ်မျိုးလုံးကို လက်ခံသည်။
- `tools/jobs/get` သည် `job_id` နှင့် `jobId` နှစ်မျိုးလုံးကို လက်ခံသည်။
- `tools/list.cursor` သည် ဂဏန်းစာကြောင်း အော့ဖ်ဆက်တစ်ခုဖြစ်သည်။ မမှန်ကန်သော တန်ဖိုးများသည် `0` သို့ ပြန်ရောက်သွားပါသည်။
- `tools/call_batch` သည် ပစ္စည်းတစ်ခုအတွက် အကောင်းဆုံးကြိုးစားမှုဖြစ်သည် (မအောင်မြင်သောခေါ်ဆိုမှုတစ်ခုသည် ပေါက်ဖော်ခေါ်ဆိုမှုများတွင် မအောင်မြင်ပါ)။
- `tools/call_async` သည် စာအိတ်ပုံစံကိုသာ ချက်ချင်းအတည်ပြုသည်။ နောက်ပိုင်းတွင် အလုပ်အခြေအနေတွင် လုပ်ဆောင်မှုအမှားများ ပေါ်လာသည်။
- `jsonrpc` သည် `"2.0"` ဖြစ်သင့်သည်။ ချန်လှပ်ထားသော `jsonrpc` ကို တွဲဖက်အသုံးပြုနိုင်ရန် လက်ခံပါသည်။

## အထောက်အထားနှင့် ထပ်ဆင့်ပို့ခြင်း။

MCP dispatch သည် Torii ခွင့်ပြုချက်ကို ရှောင်လွှဲမည်မဟုတ်ပါ။ ခေါ်ဆိုမှုများသည် ပုံမှန်လမ်းကြောင်း ကိုင်တွယ်သူများနှင့် အထောက်အထားစစ်ဆေးမှုများကို လုပ်ဆောင်သည်။

Torii သည် tool dispatch အတွက် inbound auth-related headers ကို ထပ်ဆင့်ပို့သည်-

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

ဖောက်သည်များသည် `arguments.headers` မှတစ်ဆင့် နောက်ထပ်ခေါ်ဆိုမှုတစ်ခုအတွက် ခေါင်းစီးများကို ထောက်ပံ့ပေးနိုင်သည်။
`content-length`၊ `host` နှင့် `connection` တို့မှ လျစ်လျူရှုထားသည်။

## မော်ဒယ်အမှား

HTTP အလွှာ-

- `400` မမှန်ကန်သော JSON
- JSON-RPC မကိုင်တွယ်မီ `403` API တိုကင်ကို ပယ်ချခဲ့သည်
- `413` ပါ၀င်မှုမှာ `max_request_bytes` ထက် ကျော်လွန်နေသည်
- `429` နှုန်းထား ကန့်သတ်ထားသည်။
- JSON-RPC တုံ့ပြန်မှုများအတွက် `200` (JSON-RPC အမှားအယွင်းများ အပါအဝင်)

JSON-RPC အလွှာ-- ထိပ်တန်းအဆင့် `error.data.error_code` သည် တည်ငြိမ်သည် (ဥပမာ `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I180NI00)။
- `isError = true` နှင့် ဖွဲ့စည်းတည်ဆောက်ထားသော အသေးစိတ်အချက်များနှင့်အတူ MCP တူးလ်ရလဒ်များအတိုင်း တူးလ်ပျက်ကွက်မှုများ ပေါ်လာသည်။
- လမ်းကြောင်း-ပေးပို့သည့်တူးလ် ချို့ယွင်းချက်များသည် HTTP အခြေအနေကို `structuredContent.error_code` (ဥပမာ `forbidden`၊ `not_found`၊ `server_error`) သို့ မြေပုံညွှန်းပါသည်။

## ကိရိယာအမည်ပေးခြင်း

OpenAPI မှရရှိသော ကိရိယာများသည် တည်ငြိမ်သောလမ်းကြောင်းအခြေခံအမည်များကို အသုံးပြုသည်-

- `torii.<method>_<path...>`
- ဥပမာ- `torii.get_v1_accounts`

ရွေးချယ်ထားသော နာမည်တူများကို `iroha.*` နှင့် `connect.*` အောက်တွင်လည်း ဖော်ထုတ်ထားသည်။

## Canonical သတ်မှတ်ချက်

ဝါယာကြိုးအဆင့် စာချုပ် အပြည့်အစုံကို အောက်ပါအတိုင်း ထိန်းသိမ်းထားသည်။

- `crates/iroha_torii/docs/mcp_api.md`

`crates/iroha_torii/src/mcp.rs` သို့မဟုတ် `crates/iroha_torii/src/lib.rs` တွင် အပြုအမူပြောင်းလဲသောအခါ၊
တူညီသောပြောင်းလဲမှုတွင် ထို spec ကို အပ်ဒိတ်လုပ်ပြီး ဤနေရာတွင် သော့အသုံးပြုမှုလမ်းညွှန်ကို ကြေးမုံပြင်ပါ။