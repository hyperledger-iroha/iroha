---
lang: my
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

အခြေအနေ- 2026-03-21 တွင် အပြီးသတ်ခဲ့သည်။  
ပိုင်ရှင်များ- Torii ပလပ်ဖောင်း၊ SDK ပရိုဂရမ် ဦးဆောင်  
လမ်းပြမြေပုံရည်ညွှန်း- TORII-APP-1 — `app_api` တူညီမှုစာရင်းစစ်

ဤစာမျက်နှာသည် အတွင်းပိုင်း `TORII-APP-1` စာရင်းစစ် (`docs/source/torii/app_api_parity_audit.md`) ကို ထင်ဟပ်စေသည်
Mono-repo အပြင်ဘက်ရှိ စာဖတ်သူများသည် မည်သည့် `/v1/*` မျက်နှာပြင်များကို ကြိုးတပ်၍ စမ်းသပ်ထားသည်၊
မှတ်တမ်းတင်ထားသည်။ စာရင်းစစ်သည် `Torii::add_app_api_routes` မှတစ်ဆင့် ပြန်လည်တင်ပို့သည့် လမ်းကြောင်းများကို ခြေရာခံခြင်း၊
`add_contracts_and_vk_routes` နှင့် `add_connect_routes`။

## နယ်ပယ်နှင့် နည်းလမ်း

စာရင်းစစ်သည် `crates/iroha_torii/src/lib.rs:256-522` တွင် အများသူငှာ ပြန်လည်တင်ပို့မှုများကို ကြည့်ရှုစစ်ဆေးသည်၊
feature-gated လမ်းကြောင်းတည်ဆောက်သူများ။ လမ်းပြမြေပုံရှိ `/v1/*` မျက်နှာပြင်တိုင်းအတွက် ကျွန်ုပ်တို့ အတည်ပြုထားသည်-

- `crates/iroha_torii/src/routing.rs` ရှိ Handler အကောင်အထည်ဖော်မှုနှင့် DTO အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ။
- `app_api` သို့မဟုတ် `connect` အင်္ဂါရပ်အုပ်စုများအောက်တွင် Router မှတ်ပုံတင်ခြင်း။
- လက်ရှိ ပေါင်းစပ်/ယူနစ် စမ်းသပ်မှုများနှင့် ရေရှည်လွှမ်းခြုံမှုအတွက် တာဝန်ရှိသော ကိုယ်ပိုင်အဖွဲ့။

အကောင့်ပိုင်ဆိုင်မှု/ငွေပေးငွေယူများနှင့် ပိုင်ဆိုင်မှုကိုင်ဆောင်သူစာရင်းများသည် စိတ်ကြိုက်ရွေးချယ်နိုင်သော `asset_id` မေးမြန်းမှုဘောင်များကို လက်ခံသည်
ကြိုတင်စစ်ထုတ်ခြင်းအတွက်၊ ရှိပြီးသား pagination/backpressure ကန့်သတ်ချက်များအပြင်။

## Auth & canonical လက်မှတ်ထိုးခြင်း။

- App-facing GET/POST endpoints သည် ရွေးချယ်နိုင်သော canonical တောင်းဆိုချက် ခေါင်းစီးများ (`X-Iroha-Account`, `X-Iroha-Signature`) ကို `METHOD\n/path\nsorted_query\nsha256(body)` မှ လက်ခံပါသည်။ Torii သည် ၎င်းတို့အား executor validation မလုပ်မီ `QueryRequestWithAuthority` တွင် ထည့်သွင်းထားသောကြောင့် ၎င်းတို့သည် `/query` ကို ထင်ဟပ်စေသည်။
- SDK အကူအညီပေးသူများသည် ပင်မဖောက်သည်များအားလုံးတွင် ပို့ဆောင်သည်-
  - JS/TS- `canonicalRequest.js` မှ `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`။
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`။
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`။
- ဥပမာ အတိုအထွာများ
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<katakana-i105-account-id>", method: "get", path: "/v1/accounts/<katakana-i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<katakana-i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<katakana-i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<katakana-i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<katakana-i105-account-id>", "get", "/v1/accounts/<katakana-i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## အဆုံးမှတ်စာရင်း

### အကောင့်ခွင့်ပြုချက်များ (`/v1/accounts/{id}/permissions`) — အကျုံးဝင်သည်။
- ကိုင်တွယ်သူ- `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)။
- DTOs- `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)။
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)။
- စမ်းသပ်မှုများ- `crates/iroha_torii/tests/accounts_endpoints.rs:126` နှင့် `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`။
- ပိုင်ရှင်- Torii ပလပ်ဖောင်း။
- မှတ်ချက်များ- တုံ့ပြန်မှုသည် SDK pagination အကူအညီပေးသူများနှင့် ကိုက်ညီသော `items`/`total` ပါသော Norito JSON ကိုယ်ထည်ဖြစ်သည်။

### Alias OPRF အကဲဖြတ်ခြင်း (`POST /v1/aliases/voprf/evaluate`) — ဖုံးအုပ်ထားသည်
- ကိုင်တွယ်သူ- `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)။
- DTO များ- `AliasVoprfEvaluateRequestDto`၊ `AliasVoprfEvaluateResponseDto`၊ `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)။
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)။
- စမ်းသပ်မှုများ- inline handler tests (`crates/iroha_torii/src/lib.rs:9945-9986`) နှင့် SDK လွှမ်းခြုံမှု
  (`javascript/iroha_js/test/toriiClient.test.js:72`)။
- ပိုင်ရှင်- Torii ပလပ်ဖောင်း။
- မှတ်စုများ- တုံ့ပြန်မှုမျက်နှာပြင်သည် အဆုံးအဖြတ်ပေးသော hex နှင့် backend identifiers များကို တွန်းအားပေးသည်။ SDK များသည် DTO ကိုစားသုံးသည်။

### သက်သေဖြစ်ရပ်များ SSE (`GET /v1/events/sse`) — ဖုံးအုပ်ထားသည်။
- ကိုင်တွယ်သူ- `handle_v1_events_sse` (`crates/iroha_torii/src/routing.rs:14008-14133`)။
- DTOs- `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) နှင့် အထောက်အထား စစ်ထုတ်ခြင်း ဝိုင်ယာကြိုးများ။
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)။
- စမ်းသပ်မှုများ- အထောက်အထား-တိကျသော SSE suites (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`၊
  `sse_proof_callhash.rs`၊ `sse_proof_verified_fields.rs`၊ `sse_proof_rejected_fields.rs`) နှင့် ပိုက်လိုင်း SSE မီးခိုးစမ်းသပ်မှု
  (`integration_tests/tests/events/sse_smoke.rs`)။
- ပိုင်ရှင်- Torii Platform (runtime), Integration Tests WG (Fixtures)။
- မှတ်စုများ- အထောက်အထား စစ်ထုတ်ခြင်း လမ်းကြောင်းများကို အဆုံးမှ အဆုံးအထိ အတည်ပြုထားသည်။ စာရွက်စာတမ်းသည် `docs/source/zk_app_api.md` အောက်တွင်ရှိသည်။

### စာချုပ်သက်တမ်း (`/v1/contracts/*`) — အကျုံးဝင်သည်။
- လက်ကိုင်ကိရိယာများ- `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)၊
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)၊
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)၊
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)၊
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)။
- DTO များ- `DeployContractDto`၊ `DeployAndActivateInstanceDto`၊ `ActivateInstanceDto`၊ `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)။
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)။
- စမ်းသပ်မှုများ- router/integration suites `contracts_deploy_integration.rs`၊ `contracts_activate_integration.rs`၊
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`၊
  `contracts_instances_list_router.rs`။
- ပိုင်ရှင်- Torii ပလပ်ဖောင်းဖြင့် Smart Contract WG။
- မှတ်စုများ- Endpoints များသည် လက်မှတ်ထိုးထားသော ငွေပေးငွေယူများကို တန်းစီစောင့်ဆိုင်းပြီး မျှဝေထားသော တယ်လီမီတာမက်ထရစ်များ (`handle_transaction_with_metrics`) ကို ပြန်သုံးပါ။

### သော့ဘဝသံသရာ (`/v1/zk/vk/*`) ကို အတည်ပြုခြင်း — အကျုံးဝင်သည်။
- လက်ကိုင်များ- `handle_post_vk_register`၊ `handle_post_vk_update`၊ `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) နှင့် `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)။
- DTO များ- `ZkVkRegisterDto`၊ `ZkVkUpdateDto`၊ `ZkVkDeprecateDto`၊ `VkListQuery`၊ `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)။
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)။
- စမ်းသပ်မှုများ- `crates/iroha_torii/tests/zk_vk_get_integration.rs`၊
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`၊
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`။
- ပိုင်ရှင်- Torii ပလပ်ဖောင်းပံ့ပိုးမှုဖြင့် ZK အလုပ်အဖွဲ့။
- မှတ်စုများ- DTOs များသည် SDKs မှရည်ညွှန်းထားသော Norito schemas နှင့် ချိန်ညှိသည်။ နှုန်းထားကန့်သတ်ချက်ကို `limits.rs` မှတစ်ဆင့် ပြဋ္ဌာန်းထားသည်။

### Nexus ချိတ်ဆက်မှု (`/v1/connect/*`) — ဖုံးအုပ်ထားသည် (အင်္ဂါရပ် `connect`)
- လက်ကိုင်ကိရိယာများ- `handle_connect_session`၊ `handler_connect_session_delete`၊ `handle_connect_ws`၊
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)။
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)၊
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)။
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)။
- စမ်းသပ်မှုများ- `crates/iroha_torii/tests/connect_gating.rs` (အင်္ဂါရပ်ဂိတ်ပေါက်၊ စက်ရှင်ဘဝသံသရာ၊ WS လက်ဆွဲနှုတ်ဆက်ခြင်း) နှင့်
  router feature matrix လွှမ်းခြုံမှု (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)။
- ပိုင်ရှင်- Nexus WG ချိတ်ဆက်ပါ။
- မှတ်စုများ- `limits::rate_limit_key` မှတစ်ဆင့် ခြေရာခံထားသော အဆင့်သတ်မှတ်ကန့်သတ်သော့များ၊ တယ်လီမီတာ ကောင်တာများသည် `connect.*` မက်ထရစ်များကို ကျွေးမွေးသည်။

### Kaigi relay telemetry — ဖုံးအုပ်ထားသည်။
- ကိုင်တွယ်သူများ- `handle_v1_kaigi_relays`၊ `handle_v1_kaigi_relay_detail`၊
  `handle_v1_kaigi_relays_health`၊ `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)။
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`၊
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`၊
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)။
- Router ချိတ်ဆက်မှု- `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)။
- စမ်းသပ်မှုများ- `crates/iroha_torii/tests/kaigi_endpoints.rs`။
- မှတ်ချက်များ- SSE stream သည် ကမ္ဘာလုံးဆိုင်ရာ ထုတ်လွှင့်သည့်ချန်နယ်ကို ပြဋ္ဌာန်းထားစဉ်တွင် ပြန်လည်အသုံးပြုသည်။
  တယ်လီမီတာ ပရိုဖိုင်ဂိတ်၊ မှတ်တမ်းတင်ထားသော တုံ့ပြန်မှုအစီအစဉ်များ
  `docs/source/torii/kaigi_telemetry_api.md`။

## စမ်းသပ်လွှမ်းခြုံမှု အနှစ်ချုပ်

- Router smoke tests (`crates/iroha_torii/tests/router_feature_matrix.rs`) သည် feature ပေါင်းစပ်မှုများကို မှတ်ပုံတင်တိုင်း သေချာစေသည် ။
  လမ်းကြောင်းနှင့် OpenAPI မျိုးဆက်သည် တစ်ပြိုင်တည်းရှိနေပါသည်။
- Endpoint-specific suites များသည် အကောင့်မေးမြန်းမှုများ၊ စာချုပ်သက်တမ်းစက်ဝန်း၊ ZK အတည်ပြုသော့များ၊ SSE အထောက်အထားစစ်ထုတ်မှုများနှင့် Nexus တို့ကို အကျုံးဝင်သည်
  အပြုအမူများကို ချိတ်ဆက်ပါ။
- SDK တူညီသောကြိုးများ (JavaScript၊ Swift၊ Python) သည် Alias ​​VOPRF နှင့် SSE အဆုံးမှတ်များကို အသုံးပြုထားပြီးဖြစ်သည်။ အပိုအလုပ်မရှိပါ။
  လိုအပ်သည်။

## ဒီကြေးမုံကို ခေတ်မီအောင်ထားပါ။

ဤစာမျက်နှာနှင့် အရင်းအမြစ်စာရင်းစစ် (`docs/source/torii/app_api_parity_audit.md`) နှစ်ခုလုံးကို အပ်ဒိတ်လုပ်ပါ။
Torii အက်ပ် API အပြုအမူ ပြောင်းလဲသည့်အခါတိုင်း SDK ပိုင်ရှင်များနှင့် ပြင်ပစာဖတ်သူများ တစ်ပြေးညီရှိနေမည်ဖြစ်သည်။