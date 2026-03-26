---
id: torii-app-api-parity
lang: az
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Vəziyyəti: Tamamlanıb 21-03-2026  
Sahiblər: Torii Platforması, SDK Proqram Rəhbəri  
Yol xəritəsi arayışı: TORII-APP-1 — `app_api` paritet auditi

Bu səhifə daxili `TORII-APP-1` auditini əks etdirir (`docs/source/torii/app_api_parity_audit.md`)
Beləliklə, mono-repodan kənar oxucular hansı `/v1/*` səthlərinin simli, sınaqdan keçirildiyini,
və sənədləşdirilmişdir. Audit `Torii::add_app_api_routes` vasitəsilə yenidən ixrac edilən marşrutları izləyir,
`add_contracts_and_vk_routes` və `add_connect_routes`.

## Əhatə dairəsi və metodu

Audit `crates/iroha_torii/src/lib.rs:256-522`-də ictimai təkrar ixracı yoxlayır və
xüsusiyyətli marşrut qurucuları. Yol xəritəsindəki hər `/v1/*` səthi üçün biz təsdiq etdik:

- `crates/iroha_torii/src/routing.rs`-də işləyicinin tətbiqi və DTO tərifləri.
- `app_api` və ya `connect` xüsusiyyət qrupları altında marşrutlaşdırıcının qeydiyyatı.
- Mövcud inteqrasiya/vahid testləri və uzunmüddətli əhatə dairəsinə cavabdeh olan sahib komandası.

Hesab aktivləri/əməliyyatları və aktiv sahibi siyahıları isteğe bağlı `asset_id` sorğu parametrlərini qəbul edir
mövcud səhifələmə/geri təzyiq məhdudiyyətlərinə əlavə olaraq, əvvəlcədən filtrləmə üçün.

## Doğrulama və kanonik imzalama

- Tətbiqə baxan GET/POST son nöqtələri `METHOD\n/path\nsorted_query\nsha256(body)`-dən qurulmuş isteğe bağlı kanonik sorğu başlıqlarını (`X-Iroha-Account`, `X-Iroha-Signature`) qəbul edir; Torii, icraçı yoxlamadan əvvəl onları `QueryRequestWithAuthority`-ə bükür ki, onlar `/query`-i əks etdirir.
- SDK köməkçiləri bütün əsas müştərilərə göndərilir:
  - JS/TS: `canonicalRequest.js`-dən `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Sürətli: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Nümunə parçaları:
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

## Son nöqtə inventar

### Hesab icazələri (`/v1/accounts/{id}/permissions`) — Əhatə olunur
- İşləyici: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO-lar: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Routerin bağlanması: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testlər: `crates/iroha_torii/tests/accounts_endpoints.rs:126` və `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Sahib: Torii Platforması.
- Qeydlər: Cavab SDK səhifələşdirmə köməkçilərinə uyğun gələn `items`/`total` ilə Norito JSON gövdəsidir.

### Ləqəb OPRF qiymətləndirməsi (`POST /v1/aliases/voprf/evaluate`) — Örtülüdür
- İşləyici: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO-lar: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Routerin bağlanması: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Testlər: daxili işləyici testləri (`crates/iroha_torii/src/lib.rs:9945-9986`) və SDK əhatə dairəsi
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Sahib: Torii Platforması.
- Qeydlər: Cavab səthi deterministik hex və arxa uç identifikatorlarını tətbiq edir; SDK-lar DTO-nu istehlak edir.

### Proof hadisələri SSE (`GET /v1/events/sse`) — Örtülüdür
- İşləyici: filtr dəstəyi ilə `handle_v1_events_sse` (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO-lar: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) üstəgəl sübut filtr naqilləri.
- Routerin bağlanması: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testlər: sübut üçün xüsusi SSE dəstləri (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) və boru kəməri SSE tüstü testi
  (`integration_tests/tests/events/sse_smoke.rs`).
- Sahib: Torii Platforması (işləmə vaxtı), İnteqrasiya Testləri WG (qurğular).
- Qeydlər: Sübut filtr yolları başdan sona doğrulanmış; sənədlər `docs/source/zk_app_api.md` altında yaşayır.

### Müqavilənin həyat dövrü (`/v1/contracts/*`) — Əhatə olunur
- İşləyicilər: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO-lar: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Routerin bağlanması: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testlər: marşrutlaşdırıcı/inteqrasiya dəstləri `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Sahib: Torii Platforması ilə Ağıllı Müqavilə WG.
- Qeydlər: Son nöqtələr imzalanmış əməliyyatları növbəyə çəkir və paylaşılan telemetriya ölçülərini təkrar istifadə edir (`handle_transaction_with_metrics`).

### Açarın həyat dövrünün yoxlanılması (`/v1/zk/vk/*`) — Örtülüdür
- İşləyicilər: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) və `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO-lar: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Routerin bağlanması: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testlər: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Sahib: Torii Platforma dəstəyi ilə ZK İşçi Qrupu.
- Qeydlər: DTO-lar SDK-lar tərəfindən istinad edilən Norito sxemləri ilə uyğunlaşdırılır; dərəcə məhdudiyyəti `limits.rs` vasitəsilə tətbiq edilir.

### Nexus Qoşulun (`/v1/connect/*`) — Örtülü (`connect` xüsusiyyəti)
- İşləyicilər: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO-lar: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Routerin bağlanması: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Testlər: `crates/iroha_torii/tests/connect_gating.rs` (xüsusiyyət qapısı, sessiyanın həyat dövrü, WS əl sıxması) və
  marşrutlaşdırıcının xüsusiyyətləri matrisinin əhatə dairəsi (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Sahib: Nexus Connect WG.
- Qeydlər: `limits::rate_limit_key` vasitəsilə izlənilən tarif limiti düymələri; telemetriya sayğacları `connect.*` ölçülərini verir.

### Kaigi relay telemetriyası — Örtülü
- İşləyicilər: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO-lar: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Routerin bağlanması: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testlər: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Qeydlər: SSE axını tətbiq edərkən qlobal yayım kanalından yenidən istifadə edir
  telemetriya profil qapısı; cavab sxemləri sənədləşdirilmişdir
  `docs/source/torii/kaigi_telemetry_api.md`.

## Test əhatə dairəsi xülasəsi

- Router tüstü testləri (`crates/iroha_torii/tests/router_feature_matrix.rs`) xüsusiyyət birləşmələrinin hər dəfə qeydiyyatdan keçməsini təmin edir
  marşrut və həmin OpenAPI nəsli sinxron qalır.
- Son nöqtə üçün xüsusi paketlər hesab sorğularını, müqavilənin həyat dövrünü, ZK doğrulama açarlarını, SSE sübut filtrlərini və Nexus-ni əhatə edir.
  Davranışları birləşdirin.
- SDK paritet qoşquları (JavaScript, Swift, Python) artıq Alias ​​VOPRF və SSE son nöqtələrini istehlak edir; əlavə iş yoxdur
  tələb olunur.

## Bu güzgünün aktual olması

Həm bu səhifəni, həm də mənbə auditini yeniləyin (`docs/source/torii/app_api_parity_audit.md`)
Torii tətbiqinin API davranışı dəyişdikdə SDK sahibləri və xarici oxucular uyğunlaşdırılır.