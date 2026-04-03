<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: kk
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
description: Torii түпнұсқалық үлгі контекстік протокол көпірін пайдалануға арналған анықтамалық нұсқаулық.
---

Torii `/v1/mcp` мекенжайында жергілікті үлгі контекстік протоколы (MCP) көпірін көрсетеді.
Бұл соңғы нүкте агенттерге құралдарды табуға және JSON-RPC арқылы Torii/Connect маршруттарын шақыруға мүмкіндік береді.

## Соңғы нүкте пішіні

- `GET /v1/mcp` мүмкіндіктер метадеректерін қайтарады (JSON-RPC оралмаған).
- `POST /v1/mcp` JSON-RPC 2.0 сұрауларын қабылдайды.
- `torii.mcp.enabled = false` болса, маршруттардың ешқайсысы да көрінбейді.
- `torii.require_api_token` қосылған болса, JSON-RPC жіберу алдында жоқ/жарамсыз таңбалауыш қабылданбайды.

## Конфигурация

`torii.mcp` астында MCP қосу:

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

Негізгі мінез-құлық:

- `profile` құралдың көрінуін басқарады (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` қосымша атқа негізделген саясатты қолданады.
- `rate_per_minute`/`burst` MCP сұраулары үшін таңбалауыш шелек шектеуін қолданады.
- `tools/call_async` нұсқасындағы синхронды тапсырма күйі `async_job_ttl_secs` және `async_job_max_entries` көмегімен жадта сақталады.

## Ұсынылатын клиент ағыны

1. `initialize` нөміріне қоңырау шалыңыз.
2. `tools/list` және `toolsetVersion` кэшіне қоңырау шалыңыз.
3. Қалыпты әрекеттер үшін `tools/call` пайдаланыңыз.
4. Ұзағырақ әрекеттер үшін `tools/call_async` + `tools/jobs/get` пайдаланыңыз.
5. `listChanged` `true` болғанда `tools/list` қайта іске қосыңыз.

Толық құралдар каталогын қатаң кодтамаңыз. Орындалу уақытында ашыңыз.

## Әдістер мен семантика

Қолдау көрсетілетін JSON-RPC әдістері:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Ескертулер:- `tools/list` `toolset_version` және `toolsetVersion` екеуін де қабылдайды.
- `tools/jobs/get` `job_id` және `jobId` екеуін де қабылдайды.
- `tools/list.cursor` – сандық жолдың ығысуы; жарамсыз мәндер `0` мәніне қайта оралады.
- `tools/call_batch` - бір элемент үшін ең жақсы күш (бір сәтсіз қоңырау бауырлас қоңырауларды орындамайды).
- `tools/call_async` бірден конверт пішінін ғана тексереді; орындау қателері тапсырма күйінде кейінірек пайда болады.
- `jsonrpc` `"2.0"` болуы керек; қабылданбаған `jsonrpc` үйлесімділік үшін қабылданады.

## Авторизация және қайта жіберу

MCP жіберу Torii авторизациясын айналып өтпейді. Қоңыраулар қалыпты маршрут өңдеушілерін және аутентификацияны тексеруді орындайды.

Torii құралды жіберу үшін кіріс аутентификацияға қатысты тақырыптарды бағыттайды:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Клиенттер сонымен қатар `arguments.headers` арқылы әрбір қоңырауға қосымша тақырыптарды бере алады.
`content-length`, `host` және `arguments.headers` ішінен `connection` еленбейді.

## Қате үлгісі

HTTP деңгейі:

- `400` жарамсыз JSON
- `403` API таңбалауышы JSON-RPC өңдеуден бұрын қабылданбады
- `413` пайдалы жүктемесі `max_request_bytes` асады
- `429` жылдамдығы шектелген
- JSON-RPC жауаптары үшін `200` (соның ішінде JSON-RPC қателері)

JSON-RPC қабаты:- Жоғарғы деңгейлі `error.data.error_code` тұрақты (мысалы, `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I01072X, I010).
- Құрал ақаулары `isError = true` және құрылымдық мәліметтермен MCP құралының нәтижелері ретінде пайда болады.
- Бағыт бойынша жіберілген құрал ақаулары HTTP күйін `structuredContent.error_code` (мысалы, `forbidden`, `not_found`, `server_error`) салыстырады.

## Құрал атауы

OpenAPI туынды құралдары тұрақты маршрутқа негізделген атауларды пайдаланады:

- `torii.<method>_<path...>`
- Мысалы: `torii.get_v1_accounts`

Таңдалған бүркеншік аттар `iroha.*` және `connect.*` астында да көрсетіледі.

## Канондық спецификация

Толық сым деңгейіндегі келісімшарт келесіде сақталады:

- `crates/iroha_torii/docs/mcp_api.md`

`crates/iroha_torii/src/mcp.rs` немесе `crates/iroha_torii/src/lib.rs` түріндегі мінез-құлық өзгерген кезде,
сол өзгерісте сол спецификацияны жаңартыңыз, содан кейін осы жерде кілтті пайдалану нұсқауларын қайталаңыз.