---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridade
título: تدقيق تكافؤ واجهة تطبيق Torii
description: نسخة مرآة لمراجعة TORII-APP-1 حتى تتمكن فرق SDK والمنصة من تأكيد التغطية العامة.
---

Data: Maio 2026-03-21  
Nome: Plataforma Torii, líder do programa SDK  
مرجع خارطة الطريق: TORII-APP-1 — تدقيق تكافؤ `app_api`

Você pode usar o código `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para obter mais informações. O produto é um produto `/v2/*` que pode ser usado e usado. Para obter mais informações, verifique `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## النطاق والمنهج

Verifique o valor do produto em `crates/iroha_torii/src/lib.rs:256-522` e verifique o valor do produto. A solução `/v2/*` é a seguinte:

- Verifique o DTO e o DTO em `crates/iroha_torii/src/routing.rs`.
- Verifique o valor do código `app_api` e `connect`.
- اختبارات التكامل/الوحدة الموجودة والفريق المسؤول عن التغطية طويلة الاجل.

قوائم أصول/معاملات الحساب وقوائم حاملي الأصول تقبل معاملات استعلام `asset_id` اختيارية للتصفية المسبقة, بالإضافة إلى حدود الترقيم/الضغط العكسي الحالية.

## المصادقة والتوقيع القياسي

- نقاط النهاية GET/POST الموجهة للتطبيقات تقبل رؤوس طلب قياسية اختيارية (`X-Iroha-Account`, `X-Iroha-Signature`) Nome de `METHOD\n/path\nsorted_query\nsha256(body)`; O Torii é executado no `QueryRequestWithAuthority` para executar o executor do `/query`.
- Baixe o SDK do site:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` em `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Como:
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

## جرد نقاط النهاية

### اذونات الحساب (`/v2/accounts/{id}/permissions`) — مغطى
- Nome: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Código de configuração: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Nomes: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Nome: Plataforma Torii.
- ملاحظات: الاستجابة هي جسم JSON Norito como `items`/`total`; Instale o software no SDK.

### تقييم OPRF للاسماء المستعارة (`POST /v2/aliases/voprf/evaluate`) — مغطى
- Nome: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Código de configuração: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Configuração: Inline للمعالج (`crates/iroha_torii/src/lib.rs:9945-9986`) بالاضافة الى تغطية SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Nome: Plataforma Torii.
- ملاحظات: واجهة الاستجابة تفرض hex محدد وهوية backend; Instale o SDK no DTO.

### Prova de prova de SSE (`GET /v2/events/sse`) — مغطى
- Nome: `handle_v1_events_sse` de acordo com o padrão (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) é uma prova de segurança.
- Código de configuração: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Requisitos: حزم SSE خاصة بالproof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`).
  (`integration_tests/tests/events/sse_smoke.rs`).
- Nome: Plataforma Torii (tempo de execução), GT de testes de integração (acessórios).
- ملاحظات: تم التحقق من مسارات فلتر prova طرفا لطرف؛ Você pode usar o `docs/source/zk_app_api.md`.### دورة حياة العقود (`/v2/contracts/*`) — مغطى
- Nomes: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Código de configuração: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Configurações: roteador/integração `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Nome: Smart Contract WG na plataforma Torii.
- ملاحظات: نقاط النهاية تضع المعاملات الموقعة في قائمة انتظار وتعيد استخدام مقاييس Código de erro (`handle_transaction_with_metrics`).

### دورة حياة مفاتيح التحقق (`/v2/zk/vk/*`) — مغطى
- Nomes: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Código de configuração: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Nome: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Nome: ZK Working Group baseado na plataforma Torii.
- Método: DTOs de terceiros com Norito para SDKs; A limitação de taxa é definida como `limits.rs`.

### Nexus Connect (`/v2/connect/*`) — Recurso (recurso `connect`)
- Nomes: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Código de configuração: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Nomes: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, دورة حياة الجلسة, handshake WS) e
  Verifique o valor do dispositivo (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Nome: Nexus Connect WG.
- ملاحظات: يتم تتبع مفاتيح limite de taxa عبر `limits::rate_limit_key`؛ Use o código `connect.*`.

### تليمترية مرحلات Kaigi — مغطى
- Nomes: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Código de identificação: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Nome: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: يعيد بث SSE استخدام القناة العامة للبث مع فرض بوابة ملف تليمترية؛ Você pode fazer isso em `docs/source/torii/kaigi_telemetry_api.md`.

## ملخص تغطية الاختبارات

- A lâmpada de fumaça (`crates/iroha_torii/tests/router_feature_matrix.rs`) é usada para remover fumaça e fumaça OpenAPI Não há problema.
- تغطي الحزم الخاصة بنقاط النهاية استعلامات الحسابات ودورة حياة العقود ومفاتيح التحقق ZK Verifique a prova de SSE e Nexus Connect.
- Use o SDK (JavaScript, Swift, Python) para usar o Alias ​​VOPRF e SSE; E isso não é verdade.

## الحفاظ على تحديث هذه المرآة

A solução de problemas de hardware (`docs/source/torii/app_api_parity_audit.md`) está disponível para uso em Torii. O SDK e o SDK não estão disponíveis.