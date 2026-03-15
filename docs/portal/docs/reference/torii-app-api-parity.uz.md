---
lang: uz
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

Holati: Tugallangan 2026-03-21  
Egalari: Torii platformasi, SDK dasturi rahbari  
Yoʻl xaritasi maʼlumotnomasi: TORII-APP-1 — `app_api` paritet auditi

Bu sahifa ichki `TORII-APP-1` auditini aks ettiradi (`docs/source/torii/app_api_parity_audit.md`)
Shunday qilib, mono-repodan tashqaridagi o'quvchilar qaysi `/v2/*` sirtlari simli, sinovdan o'tganligini ko'rishlari mumkin,
va hujjatlashtirilgan. Audit `Torii::add_app_api_routes` orqali qayta eksport qilingan marshrutlarni kuzatib boradi,
`add_contracts_and_vk_routes` va `add_connect_routes`.

## Qamrov va usul

Audit `crates/iroha_torii/src/lib.rs:256-522` da davlat reeksportlarini tekshiradi va
xususiyatga ega marshrut quruvchilar. Yo'l xaritasidagi har bir `/v2/*` yuzasi uchun biz quyidagilarni tekshirdik:

- `crates/iroha_torii/src/routing.rs` da ishlov beruvchini amalga oshirish va DTO ta'riflari.
- `app_api` yoki `connect` xususiyatlar guruhlari ostida marshrutizatorni ro'yxatdan o'tkazish.
- Mavjud integratsiya/birlik testlari va uzoq muddatli qamrov uchun mas'ul bo'lgan egalik guruhi.

Hisob aktivlari/tranzaksiyalari va aktiv egalari roʻyxati ixtiyoriy `asset_id` soʻrov parametrlarini qabul qiladi
mavjud sahifalash/orqa bosim chegaralariga qo'shimcha ravishda oldindan filtrlash uchun.

## Tasdiqlash va kanonik imzolash

- Ilovaga qaragan GET/POST so'nggi nuqtalari `METHOD\n/path\nsorted_query\nsha256(body)` dan tuzilgan ixtiyoriy kanonik so'rov sarlavhalarini (`X-Iroha-Account`, `X-Iroha-Signature`) qabul qiladi; Torii ijrochi tekshiruvidan oldin ularni `QueryRequestWithAuthority` ichiga o'rab oladi, shuning uchun ular `/query`ni aks ettiradi.
- SDK yordamchilari barcha asosiy mijozlarga yuboriladi:
  - JS/TS: `canonicalRequest.js` dan `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Tezkor: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Misol parchalari:
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

## Oxirgi nuqta inventarizatsiyasi

### Hisob ruxsatnomalari (`/v2/accounts/{id}/permissions`) — Qoplangan
- Ishlovchi: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO'lar: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router ulanishi: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testlar: `crates/iroha_torii/tests/accounts_endpoints.rs:126` va `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Egasi: Torii platformasi.
- Eslatmalar: Javob Norito JSON korpusi boʻlib, `items`/`total` boʻlib, SDK sahifalash yordamchilariga mos keladi.

### taxallus OPRF baholash (`POST /v2/aliases/voprf/evaluate`) - Qoplangan
- Ishlovchi: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOlar: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router ulanishi: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Sinovlar: inline ishlov beruvchi testlari (`crates/iroha_torii/src/lib.rs:9945-9986`) va SDK qamrovi
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Egasi: Torii platformasi.
- Eslatmalar: Javob yuzasi deterministik hex va backend identifikatorlarini amalga oshiradi; SDKlar DTO ni iste'mol qiladi.

### Proof voqealari SSE (`GET /v2/events/sse`) - Qoplangan
- Ishlovchi: `handle_v1_events_sse` filtr qo'llab-quvvatlashi bilan (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO'lar: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) va filtrli simlar.
- Router ulanishi: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Sinovlar: isbot uchun maxsus SSE to'plamlari (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) va quvur liniyasi SSE tutun sinovi
  (`integration_tests/tests/events/sse_smoke.rs`).
- Egasi: Torii platformasi (ish vaqti), Integratsiya testlari WG (fiksatorlar).
- Izohlar: filtri yo'llarining uchdan uchiga tasdiqlangan isboti; hujjatlar `docs/source/zk_app_api.md` ostida ishlaydi.

### Shartnomaning ishlash davri (`/v2/contracts/*`) — Qoplangan
- Ishlovchilar: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOlar: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router ulanishi: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Sinovlar: marshrutizator/integratsiya to'plamlari `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Egasi: Torii platformasi bilan aqlli kontrakt WG.
- Eslatmalar: Endpoints imzolangan tranzaktsiyalarni navbatga qo'yadi va umumiy telemetriya ko'rsatkichlarini qayta ishlatadi (`handle_transaction_with_metrics`).

### Kalitning ishlash davrini tekshirish (`/v2/zk/vk/*`) — Qoplangan
- Ishlovchilar: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) va `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOlar: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router ulanishi: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testlar: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Egasi: Torii platformasini qo'llab-quvvatlaydigan ZK ishchi guruhi.
- Eslatmalar: DTOlar SDK tomonidan havola qilingan Norito sxemalariga mos keladi; tarif cheklovi `limits.rs` orqali amalga oshiriladi.

### Nexus Connect (`/v2/connect/*`) — Qoplangan (`connect` xususiyati)
- Ishlovchilar: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOlar: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router ulanishi: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Sinovlar: `crates/iroha_torii/tests/connect_gating.rs` (xususiyatlar chegarasi, seansning hayotiy tsikli, WS qoʻl siqish) va
  router xususiyati matritsasi qamrovi (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Egasi: Nexus Connect WG.
- Eslatmalar: `limits::rate_limit_key` orqali kuzatilgan tarif chegarasi kalitlari; telemetriya hisoblagichlari `connect.*` ko'rsatkichlarini ta'minlaydi.

### Kaigi relay telemetriyasi — Yopiq
- Ishlovchilar: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOlar: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router ulanishi: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testlar: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Eslatmalar: SSE oqimi amal qilish vaqtida global eshittirish kanalidan qayta foydalanadi
  telemetriya profilini o'rnatish; javob sxemalari hujjatlashtirilgan
  `docs/source/torii/kaigi_telemetry_api.md`.

## Test qamrovi xulosasi

- Routerning tutun sinovlari (`crates/iroha_torii/tests/router_feature_matrix.rs`) har bir xususiyat birikmalarining ro'yxatga olinishini ta'minlaydi
  marshrut va OpenAPI avlodi sinxronlashtiriladi.
- Endpoint-maxsus to'plamlar hisob so'rovlari, shartnomaning amal qilish davri, ZK tasdiqlash kalitlari, SSE proof filtrlari va Nexusni qamrab oladi.
  Xulq-atvorni bog'lash.
- SDK paritet jabduqlari (JavaScript, Swift, Python) allaqachon Alias ​​VOPRF va SSE so'nggi nuqtalarini iste'mol qiladi; qo'shimcha ish yo'q
  talab qilinadi.

## Ushbu oynani yangilab turish

Bu sahifani ham, manba auditini ham yangilang (`docs/source/torii/app_api_parity_audit.md`)
Torii ilovasi API xatti-harakati o'zgarganda, SDK egalari va tashqi o'quvchilar bir xilda qoladilar.