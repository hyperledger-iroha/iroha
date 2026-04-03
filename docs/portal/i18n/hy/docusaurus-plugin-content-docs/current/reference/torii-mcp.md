<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: hy
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
description: Torii-ի մայրենի Model Context Protocol կամուրջն օգտագործելու համար տեղեկատու ուղեցույց:
---

Torii-ը ցուցադրում է հայրենի մոդելի համատեքստային արձանագրության (MCP) կամուրջը `/v1/mcp`-ում:
Այս վերջնակետը գործակալներին հնարավորություն է տալիս հայտնաբերել գործիքներ և կանչել Torii/Connect երթուղիները JSON-RPC-ի միջոցով:

## Վերջնակետի ձև

- `GET /v1/mcp`-ը վերադարձնում է հնարավորությունների մետատվյալները (ոչ JSON-RPC փաթաթված):
- `POST /v1/mcp`-ն ընդունում է JSON-RPC 2.0 հարցումները:
- Եթե `torii.mcp.enabled = false`, ոչ երթուղին չի բացահայտվում:
- Եթե `torii.require_api_token`-ը միացված է, բացակայող/անվավեր թոքենը մերժվում է մինչև JSON-RPC առաքումը:

## Կազմաձևում

Միացնել MCP-ն `torii.mcp`-ում.

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

Հիմնական վարքագիծը.

- `profile`-ը վերահսկում է գործիքի տեսանելիությունը (`read_only`, `writer`, `operator`):
- `allow_tool_prefixes`/`deny_tool_prefixes` կիրառել լրացուցիչ անվանական քաղաքականություն:
- `rate_per_minute`/`burst` կիրառում է «token-bucket» սահմանափակումը MCP հարցումների համար:
- `tools/call_async`-ից համաժամեցված աշխատանքի վիճակը պահվում է հիշողության մեջ՝ օգտագործելով `async_job_ttl_secs` և `async_job_max_entries`:

## Առաջարկվող հաճախորդների հոսք

1. Զանգահարեք `initialize`:
2. Զանգահարեք `tools/list` և քեշեք `toolsetVersion`:
3. Օգտագործեք `tools/call` նորմալ գործողությունների համար:
4. Ավելի երկար գործողությունների համար օգտագործեք `tools/call_async` + `tools/jobs/get`:
5. Կրկին գործարկել `tools/list`, երբ `listChanged`-ը `true` է:

Մի կոշտ կոդավորեք գործիքների ամբողջական կատալոգը: Բացահայտեք գործարկման ժամանակ:

## Մեթոդներ և իմաստաբանություն

Աջակցվող JSON-RPC մեթոդներ.

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Նշումներ:- `tools/list` ընդունում է և՛ `toolset_version`, և՛ `toolsetVersion`:
- `tools/jobs/get` ընդունում է և՛ `job_id`, և՛ `jobId`:
- `tools/list.cursor`-ը թվային լարային օֆսեթ է; Անվավեր արժեքները վերադառնում են `0`:
- `tools/call_batch`-ը լավագույն ջանքերն է յուրաքանչյուր ապրանքի համար (մեկ ձախողված զանգը չի ձախողում եղբայրների և եղբայրների զանգերը):
- `tools/call_async` անմիջապես վավերացնում է միայն ծրարի ձևը. կատարման սխալները հայտնվում են ավելի ուշ աշխատանքի վիճակում:
- `jsonrpc` պետք է լինի `"2.0"`; բաց թողնված `jsonrpc`-ն ընդունվում է համատեղելիության համար:

## Հավաստագրում և վերահասցեավորում

MCP առաքումը չի շրջանցում Torii թույլտվությունը: Զանգերը կատարում են սովորական երթուղի մշակողներ և վավերականության ստուգումներ:

Torii-ն ուղարկում է ներգնա հեղինակության հետ կապված վերնագրեր՝ գործիքների ուղարկման համար.

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Հաճախորդները կարող են նաև տրամադրել լրացուցիչ մեկ զանգի վերնագրեր `arguments.headers`-ի միջոցով:
`content-length`, `host` և `connection` `arguments.headers`-ից անտեսվում են:

## Սխալի մոդել

HTTP շերտ.

- `400` անվավեր JSON
- `403` API նշանը մերժվել է JSON-RPC-ի մշակումից առաջ
- `413` ծանրաբեռնվածությունը գերազանցում է `max_request_bytes`-ը
- `429` տոկոսադրույքը սահմանափակ է
- `200` JSON-RPC պատասխանների համար (ներառյալ JSON-RPC սխալները)

JSON-RPC շերտ.- Վերին մակարդակի `error.data.error_code` կայուն է (օրինակ `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I008):
- Գործիքների խափանումները հայտնվում են, քանի որ MCP գործիքի արդյունքը `isError = true` է և կառուցվածքային մանրամասները:
- Երթուղիով ուղարկված գործիքի խափանումները HTTP-ի կարգավիճակը արտացոլում են `structuredContent.error_code`-ի (օրինակ՝ `forbidden`, `not_found`, `server_error`):

## Գործիքների անվանում

OpenAPI-ից ստացված գործիքներն օգտագործում են կայուն երթուղու վրա հիմնված անուններ.

- `torii.<method>_<path...>`
- Օրինակ՝ `torii.get_v1_accounts`

Համադրված կեղծանունները նույնպես ցուցադրվում են `iroha.*` և `connect.*` տակ:

## Կանոնական ճշգրտում

Լարերի մակարդակի ամբողջական պայմանագիրը պահպանվում է.

- `crates/iroha_torii/docs/mcp_api.md`

Երբ վարքագիծը փոխվում է `crates/iroha_torii/src/mcp.rs` կամ `crates/iroha_torii/src/lib.rs`,
թարմացրեք այդ հատկանիշը նույն փոփոխությամբ և այնուհետև արտացոլեք ստեղնաշարի օգտագործման ուղեցույցը այստեղ: