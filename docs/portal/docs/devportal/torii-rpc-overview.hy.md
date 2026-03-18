---
lang: hy
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-12-29T18:16:35.115752+00:00"
translation_last_reviewed: 2026-02-07
title: Norito-RPC Overview
translator: machine-google-reviewed
---

# Norito-RPC ակնարկ

Norito-RPC-ը Torii API-ների երկուական փոխադրումն է: Այն նորից օգտագործում է նույն HTTP ուղիները
որպես `/v1/pipeline`, բայց փոխանակում է Norito շրջանակով օգտակար բեռներ, որոնք ներառում են սխեմա
հեշեր և չեկային գումարներ: Օգտագործեք այն, երբ ձեզ անհրաժեշտ են որոշիչ, վավերացված պատասխաններ կամ
երբ խողովակաշարի JSON պատասխանները դառնում են խցան:

## Ինչու՞ անցնել:
- CRC64-ով և սխեմայի հեշերով դետերմինիստական ​​շրջանակավորումը նվազեցնում է վերծանման սխալները:
- Համօգտագործվող Norito օգնականները SDK-ներում թույլ են տալիս վերօգտագործել առկա տվյալների մոդելների տեսակները:
- Torii-ն արդեն հատկորոշում է Norito սեսիաները հեռաչափության մեջ, որպեսզի օպերատորները կարողանան վերահսկել
ընդունումը տրամադրված վահանակներով:

## հարցում

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. Սերիալացրեք ձեր օգտակար բեռը Norito կոդեկով (`iroha_client`, SDK օգնականներ կամ
   `norito::to_bytes`):
2. Հարցումն ուղարկեք `Content-Type: application/x-norito`-ով:
3. Պահանջեք Norito պատասխան՝ օգտագործելով `Accept: application/x-norito`:
4. Վերծանեք պատասխանը՝ օգտագործելով համապատասխան SDK օգնականը:

SDK-ի հատուկ ուղեցույց.
- **Rust**. `iroha_client::Client`-ը ավտոմատ կերպով բանակցում է Norito-ի հետ, երբ սահմանում եք
  `Accept` վերնագիր:
- ** Python**. օգտագործեք `NoritoRpcClient` `iroha_python.norito_rpc`-ից:
- **Android**. օգտագործեք `NoritoRpcClient` և `NoritoRpcRequestOptions`
  Android SDK.
- **JavaScript/Swift**. օգնականներին հետևում են `docs/source/torii/norito_rpc_tracker.md`-ում
  և վայրէջք կկատարի որպես NRPC-3-ի մաս:

## Փորձեք այն վահանակի նմուշը

Մշակողների պորտալը ուղարկում է Try It վստահված անձ, որպեսզի վերանայողները կարողանան վերարտադրել Norito
ծանրաբեռնվածություն՝ առանց պատվիրված սցենարներ գրելու:

1. [Սկսել վստահված անձը] (./try-it.md#start-the-proxy-locally) և սահմանել
   `TRYIT_PROXY_PUBLIC_URL`, որպեսզի վիջեթներն իմանան, թե ուր ուղարկել թրաֆիկը:
2. Բացեք այս էջում **Փորձեք** քարտը կամ `/reference/torii-swagger`
   For MCP/agent flows, use `/reference/torii-mcp`.
   վահանակ և ընտրեք վերջնակետ, ինչպիսին է `POST /v1/pipeline/submit`:
3. Անցեք **Content-Type**-ը `application/x-norito`-ի, ընտրեք **Երկուական**
   խմբագրել և վերբեռնել `fixtures/norito_rpc/transfer_asset.norito`
   (կամ ցանկացած օգտակար բեռ, որը նշված է
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`):
4. Տրամադրեք կրող նշան OAuth սարքի կոդերի վիդջեթի կամ ձեռքով նշանի միջոցով
   դաշտը (վստահված անձը ընդունում է `X-TryIt-Auth` վերափոխումները, երբ կազմաձևվում է
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`):
5. Ներկայացրեք հարցումը և հաստատեք, որ Torii-ը կրկնում է `schema_hash`-ում նշված
   `fixtures/norito_rpc/schema_hashes.json`. Համապատասխան հեշերը հաստատում են, որ
   Norito վերնագիրը պահպանվել է զննարկիչի/վստահված անձի հոփից:

Ճանապարհային քարտեզի ապացույցների համար զուգակցեք «Փորձիր այն» սքրինշոթը մի շարքի հետ
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Սցենարը փաթաթվում է
`cargo xtask norito-rpc-verify`, գրում է JSON-ի ամփոփագիրը
`artifacts/norito_rpc/<timestamp>/` և գրավում է նույն հարմարանքները, որոնք
պորտալը սպառված.

## Անսարքությունների վերացում

| Ախտանիշ | Որտեղ է հայտնվում | Հավանական պատճառ | Ամրագրել |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii պատասխան | `Content-Type` վերնագիր բացակայում է կամ սխալ է | Սահմանեք `Content-Type: application/x-norito` նախքան օգտակար բեռը ուղարկելը: |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii արձագանքման մարմին/վերնագրեր | Սարքավորումների սխեմայի հեշը տարբերվում է Torii կառուցվածքից | Վերականգնեք հարմարանքները `cargo xtask norito-rpc-fixtures`-ով և հաստատեք հեշը `fixtures/norito_rpc/schema_hashes.json`-ում; վերադարձեք JSON-ին, եթե վերջնակետը դեռ միացրել է Norito-ը: |
| `{"error":"origin_forbidden"}` (HTTP 403) | Փորձեք այն վստահված անձի պատասխանը | Հարցումը եկել է ծագումից, որը նշված չէ `TRYIT_PROXY_ALLOWED_ORIGINS` | Ավելացրեք պորտալի սկզբնաղբյուրը (օրինակ՝ `https://docs.devnet.sora.example`) env var-ին և վերագործարկեք վստահված սերվերը: |
| `{"error":"rate_limited"}` (HTTP 429) | Փորձեք այն վստահված անձի պատասխանը | Մեկ IP-ի քվոտան գերազանցել է `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` բյուջեն | Բարձրացրեք ներքին բեռի փորձարկման սահմանաչափը կամ սպասեք, մինչև պատուհանը վերակայվի (տես JSON պատասխանում `retryAfterMs`): |
| `{"error":"upstream_timeout"}` (HTTP 504) կամ `{"error":"upstream_error"}` (HTTP 502) | Փորձեք այն վստահված անձի պատասխանը | Torii-ի ժամանակը սպառվել է, կամ վստահված սերվերը չի կարողացել հասնել կազմաձևված հետնամասին | Համոզվեք, որ `TRYIT_PROXY_TARGET`-ը հասանելի է, ստուգեք Torii-ի առողջությունը կամ նորից փորձեք ավելի մեծ `TRYIT_PROXY_TIMEOUT_MS`-ով: |

Ավելին Try It-ի ախտորոշման և OAuth-ի խորհուրդները գործում են
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples):

## Լրացուցիչ ռեսուրսներ
- Տրանսպորտային RFC՝ `docs/source/torii/norito_rpc.md`
- Գործադիր ամփոփագիր՝ `docs/source/torii/norito_rpc_brief.md`
- Գործողությունների հետքեր՝ `docs/source/torii/norito_rpc_tracker.md`
- Փորձեք վստահված անձի ցուցումներ՝ `docs/portal/docs/devportal/try-it.md`
