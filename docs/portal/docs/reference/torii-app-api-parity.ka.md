---
lang: ka
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

სტატუსი: დასრულებული 2026-03-21  
მფლობელები: Torii პლატფორმა, SDK პროგრამის წამყვანი  
საგზაო რუკის მითითება: TORII-APP-1 — `app_api` პარიტეტული აუდიტი

ეს გვერდი ასახავს შიდა `TORII-APP-1` აუდიტს (`docs/source/torii/app_api_parity_audit.md`)
ასე რომ, მონო-რეპოს გარეთ მყოფმა მკითხველებმა დაინახეს, რომელი `/v1/*` ზედაპირებია სადენიანი, შემოწმებული,
და დოკუმენტირებული. აუდიტი აკონტროლებს `Torii::add_app_api_routes`-ით რეექსპორტირებულ მარშრუტებს,
`add_contracts_and_vk_routes` და `add_connect_routes`.

## სფერო და მეთოდი

აუდიტი ამოწმებს საჯარო რეექსპორტს `crates/iroha_torii/src/lib.rs:256-522`-ში და
ფუნქციური კარიბჭე მარშრუტების შემქმნელები. საგზაო რუქის ყოველი `/v1/*` ზედაპირისთვის ჩვენ დავადასტურეთ:

- დამმუშავებლის დანერგვა და DTO განმარტებები `crates/iroha_torii/src/routing.rs`-ში.
- როუტერის რეგისტრაცია `app_api` ან `connect` ფუნქციების ჯგუფებში.
- არსებული ინტეგრაციის/ერთეულის ტესტები და გრძელვადიანი გაშუქებაზე პასუხისმგებელი გუნდი.

ანგარიშის აქტივები/ტრანზაქციები და აქტივების მფლობელთა სიები იღებენ არასავალდებულო `asset_id` მოთხოვნის პარამეტრებს
წინასწარი გაფილტვრისთვის, გარდა არსებული პაგინაციის/უკუწნევის ლიმიტების გარდა.

## ავტორიზაცია და კანონიკური ხელმოწერა

- აპისკენ მიმართული GET/POST ბოლო წერტილები იღებენ არჩევით კანონიკურ მოთხოვნის სათაურებს (`X-Iroha-Account`, `X-Iroha-Signature`), რომლებიც აგებულია `METHOD\n/path\nsorted_query\nsha256(body)`-დან; Torii ახვევს მათ `QueryRequestWithAuthority`-ში შემსრულებლის ვალიდაციამდე, რათა ისინი აირეკლონ `/query`.
- SDK დამხმარეები იგზავნება ყველა ძირითად კლიენტში:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` `canonicalRequest.js`-დან.
  - სვიფტი: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- ფრაგმენტების მაგალითები:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "soraカタカナ...", method: "get", path: "/v1/accounts/soraカタカナ.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/soraカタカナ.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "soraカタカナ...",
                                                  method: "get",
                                                  path: "/v1/accounts/soraカタカナ.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("soraカタカナ...", "get", "/v1/accounts/soraカタカナ.../assets", "limit=5", ByteArray(0), signer)
```

## საბოლოო წერტილის ინვენტარი

### ანგარიშის ნებართვები (`/v1/accounts/{id}/permissions`) — დაფარული
- დამმუშავებელი: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- როუტერის დაკავშირება: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- ტესტები: `crates/iroha_torii/tests/accounts_endpoints.rs:126` და `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- მფლობელი: Torii პლატფორმა.
- შენიშვნები: Response არის Norito JSON კორპუსი `items`/`total`, რომელიც შეესაბამება SDK პაგინაციის დამხმარეებს.

### Alias OPRF შეფასება (`POST /v1/aliases/voprf/evaluate`) — დაფარული
- დამმუშავებელი: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- როუტერის დაკავშირება: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- ტესტები: შიდა დამმუშავებლის ტესტები (`crates/iroha_torii/src/lib.rs:9945-9986`) პლუს SDK დაფარვა
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- მფლობელი: Torii პლატფორმა.
- შენიშვნები: საპასუხო ზედაპირი ახორციელებს დეტერმინისტულ ექვსკუთხა და უკანა იდენტიფიკატორებს; SDK-ები მოიხმარენ DTO-ს.

### დამადასტურებელი მოვლენები SSE (`GET /v1/events/sse`) — დაფარული
- დამმუშავებელი: `handle_v1_events_sse` ფილტრის მხარდაჭერით (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) პლუს ფილტრის გაყვანილობა.
- როუტერის დაკავშირება: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- ტესტები: დამადასტურებელი სპეციფიკური SSE კომპლექტები (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) და მილსადენის SSE კვამლის ტესტი
  (`integration_tests/tests/events/sse_smoke.rs`).
- მფლობელი: Torii პლატფორმა (გაშვების დრო), ინტეგრაციის ტესტები WG (მოწყობილობები).
- შენიშვნები: მტკიცებულების ფილტრის ბილიკები დამოწმებულია ბოლომდე; დოკუმენტაცია მოქმედებს `docs/source/zk_app_api.md`-ის ქვეშ.

### კონტრაქტის სიცოცხლის ციკლი (`/v1/contracts/*`) — დაფარული
- დამმუშავებლები: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- როუტერის დაკავშირება: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- ტესტები: როუტერი/ინტეგრაციის კომპლექტები `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- მფლობელი: Smart Contract WG Torii პლატფორმით.
- შენიშვნები: ბოლო წერტილების რიგში ხელმოწერილი ტრანზაქციები და ხელახლა გამოიყენე საერთო ტელემეტრიის მეტრიკა (`handle_transaction_with_metrics`).

### გასაღების სასიცოცხლო ციკლის გადამოწმება (`/v1/zk/vk/*`) — დაფარული
- დამმუშავებლები: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) და `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- როუტერის დაკავშირება: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- ტესტები: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- მფლობელი: ZK სამუშაო ჯგუფი Torii პლატფორმის მხარდაჭერით.
- შენიშვნები: DTO-ები შეესაბამება Norito სქემებს, რომლებიც მითითებულია SDK-ებით; განაკვეთის შეზღუდვა ძალაშია `limits.rs`-ის მეშვეობით.

### Nexus დაკავშირება (`/v1/connect/*`) — დაფარული (ფუნქცია `connect`)
- დამმუშავებლები: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- როუტერის დაკავშირება: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- ტესტები: `crates/iroha_torii/tests/connect_gating.rs` (ფუნქციური კარიბჭე, სესიის სასიცოცხლო ციკლი, WS ხელის ჩამორთმევა) და
  როუტერის მახასიათებლების მატრიცის დაფარვა (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- მფლობელი: Nexus Connect WG.
- შენიშვნები: შეფასების ლიმიტის გასაღებები თვალყურის დევნება `limits::rate_limit_key`-ის მეშვეობით; ტელემეტრიული მრიცხველები იკვებებენ `connect.*` მეტრებს.

### კაიგის სარელეო ტელემეტრია — დაფარული
- დამმუშავებლები: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- როუტერის დაკავშირება: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- ტესტები: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- შენიშვნები: SSE ნაკადი ხელახლა იყენებს გლობალურ სამაუწყებლო არხს აღსრულებისას
  ტელემეტრიული პროფილის კარიბჭე; საპასუხო სქემები დოკუმენტირებულია
  `docs/source/torii/kaigi_telemetry_api.md`.

## ტესტის დაფარვის შეჯამება

- როუტერის კვამლის ტესტები (`crates/iroha_torii/tests/router_feature_matrix.rs`) უზრუნველყოფს ფუნქციების კომბინაციების რეგისტრაციას ყოველ
  მარშრუტი და რომ OpenAPI თაობა სინქრონიზებული რჩება.
- ბოლო წერტილის სპეციფიკური კომპლექტები მოიცავს ანგარიშის შეკითხვებს, კონტრაქტის სასიცოცხლო ციკლს, ZK დამადასტურებელ გასაღებებს, SSE proof ფილტრებს და Nexus
  დააკავშირეთ ქცევები.
- SDK პარიტეტული აღკაზმულობა (JavaScript, Swift, Python) უკვე მოიხმარს Alias ​​VOPRF და SSE საბოლოო წერტილებს; დამატებითი სამუშაო არ არის
  საჭირო.

## ამ სარკის განახლება

განაახლეთ როგორც ეს გვერდი, ასევე წყაროს აუდიტი (`docs/source/torii/app_api_parity_audit.md`)
როდესაც Torii აპლიკაციის API ქცევა იცვლება ისე, რომ SDK-ს მფლობელები და გარე მკითხველები დარჩებიან შესაბამისობაში.