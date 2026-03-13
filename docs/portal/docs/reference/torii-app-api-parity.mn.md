---
lang: mn
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

Төлөв: 2026-03-21-нд дууссан  
Эзэмшигч: Torii платформ, SDK хөтөлбөрийн удирдагч  
Замын зургийн лавлагаа: TORII-APP-1 — `app_api` паритын аудит

Энэ хуудас нь дотоод `TORII-APP-1` аудитыг (`docs/source/torii/app_api_parity_audit.md`) толилуулж байна.
Ингэснээр моно-репогийн гадна байгаа уншигчид `/v2/*` гадаргуу нь утастай, туршилт,
болон баримтжуулсан. Аудит нь `Torii::add_app_api_routes`-ээр дамжуулан дахин экспортолсон маршрутуудыг хянаж,
`add_contracts_and_vk_routes`, `add_connect_routes`.

## Хамрах хүрээ ба арга

Аудит нь `crates/iroha_torii/src/lib.rs:256-522` болон олон нийтийн реэкспортыг шалгадаг.
онцлог хаалгатай маршрут бүтээгчид. Замын зураг дээрх `/v2/*` гадаргуу бүрийн хувьд бид дараахыг баталгаажуулсан:

- `crates/iroha_torii/src/routing.rs` дахь зохицуулагчийн хэрэгжилт ба DTO тодорхойлолтууд.
- `app_api` эсвэл `connect` онцлог бүлгүүдийн дагуу чиглүүлэгчийн бүртгэл.
- Одоо байгаа интеграцийн/нэгжийн туршилтууд болон урт хугацааны хамрах хүрээг хариуцах эзэмшигчийн баг.

Дансны хөрөнгө/гүйлгээ болон хөрөнгө эзэмшигчийн жагсаалт нь нэмэлт `asset_id` асуулгын параметрүүдийг хүлээн зөвшөөрдөг
урьдчилж шүүлтийн хувьд одоо байгаа хуудаслалт/буцах даралтын хязгаараас гадна.

## Батламж ба каноник гарын үсэг

- Аппликэйшнд зориулагдсан GET/POST төгсгөлийн цэгүүд нь `METHOD\n/path\nsorted_query\nsha256(body)`-ээс бүтээгдсэн нэмэлт каноник хүсэлтийн толгой хэсгийг (`X-Iroha-Account`, `X-Iroha-Signature`) хүлээн авдаг; Torii тэдгээрийг гүйцэтгэгч баталгаажуулахаас өмнө `QueryRequestWithAuthority` руу ороож, `/query`-ийг тусгадаг.
- SDK туслахууд нь бүх үндсэн үйлчлүүлэгчдэд хүргэдэг:
  - JS/TS: `canonicalRequest.js`-аас `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Хурдан: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Жишээ хэсгүүд:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v2/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v2/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v2/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v2/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## Төгсгөлийн цэгийн бүртгэл

### Дансны зөвшөөрөл (`/v2/accounts/{id}/permissions`) — Хамгаалагдсан
- Ажиллагч: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Чиглүүлэгчийн холболт: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Туршилтууд: `crates/iroha_torii/tests/accounts_endpoints.rs:126` болон `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Эзэмшигч: Torii платформ.
- Тайлбар: Хариулт нь `items`/`total` бүхий Norito JSON их бие бөгөөд SDK хуудасны туслахуудтай таарч байна.

### Alias OPRF үнэлгээ (`POST /v2/aliases/voprf/evaluate`) — Хамгаалагдсан
- Ажиллагч: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Чиглүүлэгчийн холболт: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Туршилтууд: Inline handler tests (`crates/iroha_torii/src/lib.rs:9945-9986`) болон SDK хамрах хүрээ
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Эзэмшигч: Torii платформ.
- Тайлбар: Хариултын гадаргуу нь детерминистик hex болон backend identifiers-ийг хэрэгжүүлдэг; SDK нь DTO-г ашигладаг.

### Нотлох үйл явдлууд SSE (`GET /v2/events/sse`) — Хамгаалагдсан
- Ажиллагч: `handle_v1_events_sse` шүүлтүүрийн дэмжлэгтэй (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) нэмэлт шүүлтүүрийн утас.
- Чиглүүлэгчийн холболт: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Туршилтууд: баталгаат тусгай SSE багц (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) болон дамжуулах хоолойн SSE утааны туршилт
  (`integration_tests/tests/events/sse_smoke.rs`).
- Эзэмшигч: Torii платформ (ажиллуулах хугацаа), Интеграцийн туршилтын ажлын хэсэг (бэхэлгээ).
- Тэмдэглэл: Шүүлтүүрийн замыг эцэс төгсгөл хүртэл баталгаажуулсан; баримт бичиг нь `docs/source/zk_app_api.md` дор амьдардаг.

### Гэрээний амьдралын мөчлөг (`/v2/contracts/*`) — Хамгаалагдсан
- Ажиллагчид: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Чиглүүлэгчийн холболт: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Туршилтууд: чиглүүлэгч/интеграцийн багцууд `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Эзэмшигч: Torii платформтой Ухаалаг гэрээний ажлын хэсэг.
- Тэмдэглэл: Төгсгөлийн цэгүүд гарын үсэг зурсан гүйлгээг дараалалд оруулж, хуваалцсан телеметрийн хэмжигдэхүүнийг дахин ашигладаг (`handle_transaction_with_metrics`).

### Түлхүүр амьдралын мөчлөгийг баталгаажуулж байна (`/v2/zk/vk/*`) — Хамгаалагдсан
- Ажиллагчид: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) болон `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Чиглүүлэгчийн холболт: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Туршилтууд: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Эзэмшигч: Torii платформын дэмжлэгтэй ZK Ажлын хэсэг.
- Тэмдэглэл: DTO нь SDK-ийн иш татсан Norito схемтэй нийцдэг; хурдны хязгаарлалтыг `limits.rs`-ээр хэрэгжүүлсэн.

### Nexus Холбох (`/v2/connect/*`) — Хамгаалагдсан (`connect` онцлог)
- Ажиллагчид: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Чиглүүлэгчийн холболт: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Туршилтууд: `crates/iroha_torii/tests/connect_gating.rs` (онцлогын гарц, сессийн амьдралын мөчлөг, WS гар барих) ба
  чиглүүлэгчийн онцлог матрицын хамрах хүрээ (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Эзэмшигч: Nexus WG-г холбоно.
- Тайлбар: `limits::rate_limit_key`-ээр хянагдсан ханшийн хязгаарын товчлуурууд; телеметрийн тоолуурууд `connect.*` хэмжүүрүүдийг тэжээдэг.

### Кайги релей телеметри — Хамгаалагдсан
- Ажиллагчид: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Чиглүүлэгчийн холболт: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Туршилтууд: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Тайлбар: SSE урсгалыг хэрэгжүүлэхийн зэрэгцээ дэлхийн өргөн нэвтрүүлгийн сувгийг дахин ашигладаг
  телеметрийн профайлын гарц; -д баримтжуулсан хариултын схемүүд
  `docs/source/torii/kaigi_telemetry_api.md`.

## Туршилтын хамрах хүрээний хураангуй

- Чиглүүлэгчийн утааны туршилт (`crates/iroha_torii/tests/router_feature_matrix.rs`) нь функцын хослолыг бүртгэх боломжийг олгодог
  чиглүүлэлт ба тэр OpenAPI үеийнх нь синхрончлолд үлддэг.
- Төгсгөлийн цэгт зориулсан багцууд нь дансны асуулга, гэрээний амьдралын мөчлөг, ZK баталгаажуулах түлхүүр, SSE баталгаажуулалтын шүүлтүүр, Nexus зэргийг хамардаг.
  Зан үйлийг холбох.
- SDK паритын бэхэлгээ (JavaScript, Swift, Python) аль хэдийн Alias ​​VOPRF болон SSE төгсгөлийн цэгүүдийг ашигладаг; нэмэлт ажил байхгүй
  шаардлагатай.

## Энэ толийг байнга шинэчилж байна

Энэ хуудас болон эх сурвалжийн аудитыг хоёуланг нь шинэчлэх (`docs/source/torii/app_api_parity_audit.md`)
Torii програмын API-н үйл ажиллагаа өөрчлөгдөх бүрд SDK эзэмшигчид болон гадны уншигчид зэрэгцэж байх болно.