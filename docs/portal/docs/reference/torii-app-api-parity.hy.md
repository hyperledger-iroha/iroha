---
lang: hy
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b8769f4d2a6dd365b851c583d936fd78594ca60087fa3bbdcc6dc8177ee12be6
source_last_modified: "2026-01-30T12:29:10.183882+00:00"
translation_last_reviewed: 2026-02-07
id: torii-app-api-parity
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
---

Կարգավիճակ. Ավարտված է 2026-03-21  
Սեփականատերեր՝ Torii հարթակ, SDK ծրագրի ղեկավար  
Ճանապարհային քարտեզի հղում՝ TORII-APP-1 — `app_api` հավասարության աուդիտ

Այս էջը արտացոլում է ներքին `TORII-APP-1` աուդիտը (`docs/source/torii/app_api_parity_audit.md`)
այնպես որ մոնո-ռեպոից դուրս ընթերցողները կարող են տեսնել, թե որ `/v1/*` մակերեսներն են լարով, փորձարկված,
և փաստաթղթավորված: Աուդիտը հետևում է `Torii::add_app_api_routes`-ի միջոցով վերաարտահանվող երթուղիներին,
`add_contracts_and_vk_routes` և `add_connect_routes`:

## Շրջանակ և մեթոդ

Աուդիտը ստուգում է `crates/iroha_torii/src/lib.rs:256-522`-ի հանրային վերաարտահանումները և
հատուկ դարպասներով երթուղի կառուցողներ. Ճանապարհային քարտեզի յուրաքանչյուր `/v1/*` մակերեսի համար մենք ստուգել ենք.

- Կառավարիչի ներդրում և DTO սահմանումներ `crates/iroha_torii/src/routing.rs`-ում:
- Երթուղիչի գրանցում `app_api` կամ `connect` առանձնահատկությունների խմբերի ներքո:
- Գործող ինտեգրման/միավորի թեստեր և երկարաժամկետ ծածկույթի համար պատասխանատու թիմ:

Հաշվի ակտիվները/գործարքները և ակտիվների սեփականատերերի ցուցակները ընդունում են կամընտիր `asset_id` հարցումների պարամետրերը
նախնական զտման համար՝ ի լրումն առկա էջագրման/հետճնշման սահմանների:

## Հավաստագրում և կանոնական ստորագրություն

- Հավելվածին ուղղված GET/POST վերջնակետերը ընդունում են կամընտիր կանոնական հարցումների վերնագրեր (`X-Iroha-Account`, `X-Iroha-Signature`), որոնք կառուցված են `METHOD\n/path\nsorted_query\nsha256(body)`-ից; Torii-ը դրանք փաթաթում է `QueryRequestWithAuthority`-ի մեջ նախքան կատարողի վավերացումը, որպեսզի նրանք արտացոլեն `/query`-ը:
- SDK օգնականները առաքվում են բոլոր հիմնական հաճախորդներին.
  - JS/TS՝ `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`՝ `canonicalRequest.js`-ից:
  - Swift՝ `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`:
  - Android (Kotlin/Java)՝ `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`:
- Հատվածների օրինակներ.
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<i105-account-id>", method: "get", path: "/v1/accounts/<i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<i105-account-id>", "get", "/v1/accounts/<i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## Վերջնակետի գույքագրում

### Հաշվի թույլտվություններ (`/v1/accounts/{id}/permissions`) — Ծածկված
- Կառավարիչ՝ `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`):
- DTO-ներ՝ `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`):
- Երթուղիչի միացում՝ `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`):
- Թեստեր՝ `crates/iroha_torii/tests/accounts_endpoints.rs:126` և `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`:
- Սեփականատեր՝ Torii հարթակ:
- Նշումներ. Response-ը Norito JSON մարմին է `items`/`total`-ով, որը համապատասխանում է SDK էջագրման օգնականներին:

### Alias OPRF գնահատում (`POST /v1/aliases/voprf/evaluate`) — Ծածկված
- Կառավարիչ՝ `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`):
- DTO-ներ՝ `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`):
- Երթուղիչի միացում՝ `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`):
- Թեստեր. ներկառուցված կարգավորիչի թեստեր (`crates/iroha_torii/src/lib.rs:9945-9986`) գումարած SDK ծածկույթ
  (`javascript/iroha_js/test/toriiClient.test.js:72`):
- Սեփականատեր՝ Torii հարթակ:
- Ծանոթագրություններ. արձագանքման մակերեսը պարտադրում է դետերմինիստական ​​վեցանկյուն և հետնամասի նույնացուցիչները; SDK-ները սպառում են DTO-ն:

### Ապացուցողական իրադարձություններ SSE (`GET /v1/events/sse`) — Ծածկված
- Կառավարիչ՝ `handle_v1_events_sse` ֆիլտրի աջակցությամբ (`crates/iroha_torii/src/routing.rs:14008-14133`):
- DTO-ներ՝ `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) գումարած ֆիլտրի պաշտպանիչ էլեկտրալարեր:
- Երթուղիչի միացում՝ `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`):
- Թեստեր՝ հատուկ SSE փաթեթներ (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) և խողովակաշարի SSE ծխի փորձարկում
  (`integration_tests/tests/events/sse_smoke.rs`):
- Սեփականատեր՝ Torii հարթակ (աշխատանքի ժամանակ), Ինտեգրման թեստեր WG (հարմարանքներ):
- Ծանոթագրություններ. Ապացուցիչ ֆիլտրի ուղիները վավերացված են վերջից մինչև վերջ; փաստաթղթերը գործում են `docs/source/zk_app_api.md`-ի ներքո:

### Պայմանագրի կյանքի ցիկլը (`/v1/contracts/*`) — Ծածկված
- Կառավարիչներ՝ `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`):
- DTO-ներ՝ `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`):
- Երթուղիչի միացում՝ `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`):
- Թեստեր՝ երթուղիչի/ինտեգրման փաթեթներ `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Սեփականատեր՝ Smart Contract WG Torii հարթակով:
- Ծանոթագրություններ. վերջնակետերը հերթագրում են ստորագրված գործարքները և վերօգտագործում ընդհանուր հեռաչափության չափումները (`handle_transaction_with_metrics`):

### Բանալին հաստատող կյանքի ցիկլը (`/v1/zk/vk/*`) — Ծածկված
- Կառավարիչներ՝ `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) և `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`):
- DTO-ներ՝ `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`):
- Երթուղիչի միացում՝ `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`):
- Թեստեր՝ `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Սեփականատեր՝ ZK աշխատանքային խումբ՝ Torii պլատֆորմի աջակցությամբ:
- Ծանոթագրություններ. DTO-ները համընկնում են Norito սխեմաների հետ, որոնք հղում են կատարում SDK-ների կողմից; տոկոսադրույքի սահմանափակում, որն իրականացվում է `limits.rs`-ի միջոցով:

### Nexus Connect (`/v1/connect/*`) — Ծածկված (հատկանիշ `connect`)
- Կառավարիչներ՝ `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`):
- DTO-ներ՝ `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`):
- Երթուղիչի միացում՝ `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`):
- Թեստեր.
  երթուղիչի հատկանիշի մատրիցային ծածկույթ (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`):
- Սեփականատեր՝ Nexus Connect WG:
- Ծանոթագրություններ. Գնահատման սահմանաչափի ստեղները հետևվում են `limits::rate_limit_key`-ի միջոցով; Հեռաչափական հաշվիչները կերակրում են `connect.*` չափումները:

### Kaigi ռելեային հեռաչափություն — Ծածկված
- Կառավարիչներ՝ `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`):
- DTO-ներ՝ `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`):
- Երթուղիչի միացում՝ `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`):
- Թեստեր՝ `crates/iroha_torii/tests/kaigi_endpoints.rs`:
- Ծանոթագրություններ. SSE հոսքը կրկին օգտագործում է գլոբալ հեռարձակման ալիքը կիրառման ընթացքում
  հեռաչափական պրոֆիլի դարպաս; պատասխանների սխեմաներ, որոնք փաստաթղթավորված են
  `docs/source/torii/kaigi_telemetry_api.md`.

## Թեստի ծածկույթի ամփոփում

- Երթուղիչի ծխի թեստերը (`crates/iroha_torii/tests/router_feature_matrix.rs`) ապահովում են հնարավորությունների համակցությունների գրանցում ամեն անգամ
  երթուղին և այդ OpenAPI սերունդը մնում է համաժամանակյա:
- Վերջնակետին հատուկ փաթեթներն ընդգրկում են հաշվի հարցումները, պայմանագրի կյանքի ցիկլը, ZK հաստատող բանալիները, SSE-ի պաշտպանված զտիչները և Nexus
  Միացնել վարքագիծը:
- SDK-ի հավասարաչափ ամրացումները (JavaScript, Swift, Python) արդեն սպառում են Alias ​​VOPRF և SSE վերջնակետերը. ոչ մի լրացուցիչ աշխատանք
  պահանջվում է.

## Այս հայելին արդիական պահեք

Թարմացրեք և՛ այս էջը, և՛ աղբյուրի աուդիտը (`docs/source/torii/app_api_parity_audit.md`)
երբ Torii հավելվածի API-ի վարքագիծը փոխվում է, որպեսզի SDK-ի սեփականատերերն ու արտաքին ընթերցողները մնան համահունչ: