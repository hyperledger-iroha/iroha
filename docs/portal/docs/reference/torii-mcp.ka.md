<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
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
description: საცნობარო სახელმძღვანელო Torii-ის ძირითადი მოდელის კონტექსტური პროტოკოლის ხიდის გამოყენებისთვის.
---

Torii ასახავს ორიგინალური მოდელის კონტექსტური პროტოკოლის (MCP) ხიდს `/v1/mcp`-ზე.
ეს საბოლოო წერტილი აგენტებს საშუალებას აძლევს აღმოაჩინონ ინსტრუმენტები და გამოიძახონ Torii/Connect მარშრუტები JSON-RPC-ით.

## ბოლო წერტილის ფორმა

- `GET /v1/mcp` აბრუნებს შესაძლებლობების მეტამონაცემებს (არა JSON-RPC შეფუთული).
- `POST /v1/mcp` იღებს JSON-RPC 2.0 მოთხოვნებს.
- თუ `torii.mcp.enabled = false`, არც ერთი მარშრუტი არ არის გამოვლენილი.
- თუ `torii.require_api_token` ჩართულია, გამოტოვებული/არასწორი ჟეტონი უარყოფილია JSON-RPC გაგზავნამდე.

## კონფიგურაცია

ჩართეთ MCP `torii.mcp`-ში:

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

ძირითადი ქცევა:

- `profile` აკონტროლებს ხელსაწყოს ხილვადობას (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` გამოიყენებს სახელზე დაფუძნებულ დამატებით პოლიტიკას.
- `rate_per_minute`/`burst` გამოიყენე ტოკენ-თაიგულის შეზღუდვა MCP მოთხოვნებისთვის.
- ასინქრონული სამუშაო მდგომარეობა `tools/call_async`-დან ინახება მეხსიერებაში `async_job_ttl_secs` და `async_job_max_entries`-ის გამოყენებით.

## რეკომენდებული კლიენტის ნაკადი

1. დარეკეთ `initialize`.
2. დარეკეთ `tools/list` და ქეში `toolsetVersion`.
3. გამოიყენეთ `tools/call` ნორმალური ოპერაციებისთვის.
4. გამოიყენეთ `tools/call_async` + `tools/jobs/get` უფრო ხანგრძლივი ოპერაციებისთვის.
5. ხელახლა გაუშვით `tools/list`, როდესაც `listChanged` არის `true`.

ნუ დააკოდებთ ინსტრუმენტების სრულ კატალოგს. აღმოაჩინეთ გაშვების დროს.

## მეთოდები და სემანტიკა

მხარდაჭერილი JSON-RPC მეთოდები:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

შენიშვნები:- `tools/list` იღებს ორივე `toolset_version` და `toolsetVersion`.
- `tools/jobs/get` იღებს ორივე `job_id` და `jobId`.
- `tools/list.cursor` არის რიცხვითი სტრიქონის ოფსეტი; არასწორი მნიშვნელობები ბრუნდება `0`-მდე.
- `tools/call_batch` არის საუკეთესო ძალისხმევა თითო ნივთზე (ერთი წარუმატებელი ზარი არ არღვევს და-ძმის ზარებს).
- `tools/call_async` ამოწმებს მხოლოდ კონვერტის ფორმას დაუყოვნებლივ; შესრულების შეცდომები მოგვიანებით გამოჩნდება სამუშაო მდგომარეობაში.
- `jsonrpc` უნდა იყოს `"2.0"`; გამოტოვებული `jsonrpc` მიღებულია თავსებადობისთვის.

## ავტორიზაცია და გადამისამართება

MCP დისპეტჩერიზაცია არ გვერდს უვლის Torii ავტორიზაციას. ზარები ასრულებენ ნორმალურ მარშრუტებს და ავტორიზაციის შემოწმებას.

Torii აგზავნის შემომავალ ავტორიზაციასთან დაკავშირებულ სათაურებს ხელსაწყოების გაგზავნისთვის:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

კლიენტებს ასევე შეუძლიათ მიაწოდონ დამატებითი სათაურები თითო ზარზე `arguments.headers`-ის საშუალებით.
`content-length`, `host` და `connection` `arguments.headers`-დან იგნორირებულია.

## შეცდომის მოდელი

HTTP ფენა:

- `400` არასწორი JSON
- `403` API ჟეტონი უარყოფილია JSON-RPC დამუშავებამდე
- `413` დატვირთვა აღემატება `max_request_bytes`
- `429` განაკვეთი შეზღუდულია
- `200` JSON-RPC პასუხებისთვის (JSON-RPC შეცდომების ჩათვლით)

JSON-RPC ფენა:- უმაღლესი დონის `error.data.error_code` სტაბილურია (მაგალითად `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I008).
- ხელსაწყოს გაუმართაობა ჩნდება, როგორც MCP ხელსაწყოს შედეგი `isError = true` და სტრუქტურირებული დეტალებით.
- მარშრუტით გაგზავნილი ხელსაწყოების წარუმატებლობები HTTP სტატუსს ასახავს `structuredContent.error_code`-ზე (მაგალითად `forbidden`, `not_found`, `server_error`).

## ხელსაწყოს დასახელება

OpenAPI-დან მიღებული ხელსაწყოები იყენებენ მარშრუტზე დაფუძნებულ სტაბილურ სახელებს:

- `torii.<method>_<path...>`
- მაგალითი: `torii.get_v1_accounts`

კურირებული მეტსახელები ასევე წარმოდგენილია `iroha.*` და `connect.*` ქვეშ.

## კანონიკური სპეციფიკაცია

სრული მავთულის დონის კონტრაქტი შენარჩუნებულია შემდეგში:

- `crates/iroha_torii/docs/mcp_api.md`

როდესაც ქცევა იცვლება `crates/iroha_torii/src/mcp.rs` ან `crates/iroha_torii/src/lib.rs`,
განაახლეთ ეს სპეციფიკა იმავე ცვლილებით და შემდეგ ასახეთ გასაღების გამოყენების მითითებები აქ.