---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridade
title: Torii API API برابری کا آڈٹ
description: TORII-APP-1 جائزے کی نقل تاکہ SDK اور پلیٹ فارم ٹیمیں عوامی کوریج کی تصدیق کر سکیں۔
---

Data: Maio 2026-03-21  
Nome: Plataforma Torii, líder do programa SDK  
Solução de problemas: TORII-APP-1 — `app_api` Instalação

یہ صفحہ اندرونی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ مونو-ریپو سے O cartão de crédito do `/v1/*` é um produto de alta qualidade دستاویزی ہیں۔ یہ آڈٹ ان راستوں کو ٹریک کرتا ہے جو `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes` کے ذریعے دوبارہ ایکسپورٹ ہوتے ہیں۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی ری ایکسپورٹس اور feature gating والے روٹ بلڈرز کا جائزہ لیتا ہے۔ روڈمیپ میں ہر `/v1/*` سطح کے لئے ہم نے درج ذیل تصدیق کی:

- `crates/iroha_torii/src/routing.rs` میں manipulador کی نفاذ اور DTO تعریفات۔
- `app_api` یا `connect` فیچر گروپس کے تحت روٹر رجسٹریشن۔
- موجودہ انٹیگریشن/یونٹ ٹیسٹس اور طویل مدتی کوریج کے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن فہرستیں اور اثاثہ ہولڈر لسٹنگز موجودہ paginação/contrapressão حدود کے علاوہ پری فلٹرنگ کے لئے اختیاری Consulta `asset_id` پیرامیٹرز قبول کرتی ہیں۔

## تصدیق اور کینونیکل دستخط

- ایپ کے لئے GET/POST اینڈپوائنٹس اختیاری کینونیکل ریکوئسٹ ہیڈرز (`X-Iroha-Account`, `X-Iroha-Signature`) قبول کرتے ہیں ou `METHOD\n/path\nsorted_query\nsha256(body)` سے بنائے جاتے ہیں؛ Executor Torii کی عکاسی کریں۔
- SDK ہیلپرز تمام بنیادی کلائنٹس میں دستیاب ہیں:
  - JS/TS: `canonicalRequest.js` ou `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Como:
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

## اینڈپوائنٹ فہرست

### اکاؤنٹ اجازتیں (`/v1/accounts/{id}/permissions`) — کورڈ
- Manipulador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Proprietário: Plataforma Torii.
- Notas: Corpo Norito JSON ہے جس میں `items`/`total` ہے، جو auxiliares de paginação SDK سے مطابقت رکھتا ہے۔

### Alias OPRF تشخیص (`POST /v1/aliases/voprf/evaluate`) — کورڈ
- Manipulador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Ligação do roteador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Testes: manipulador کے testes inline (`crates/iroha_torii/src/lib.rs:9945-9986`) کے ساتھ SDK کوریج
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Proprietário: Plataforma Torii.
- Notas: جواب کی سطح hexadecimal determinístico e identificadores de back-end نافذ کرتی ہے؛ SDKs para DTO e SDKs disponíveis### Prova SSE ایونٹس (`GET /v1/events/sse`) — کورڈ
- Manipulador: `handle_v1_events_sse` فلٹر سپورٹ کے ساتھ (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) E fiação de filtro à prova.
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: prova de suítes SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste de fumaça SSE de tubulação
  (`integration_tests/tests/events/sse_smoke.rs`).
- Proprietário: Plataforma Torii (runtime), GT de Testes de Integração (fixtures).
- Notas: filtro de prova راستے ponta a ponta توثیق شدہ ہیں؛ دستاویزات `docs/source/zk_app_api.md` میں ہیں۔

### کنٹریکٹ لائف سائیکل (`/v1/contracts/*`) — کورڈ
- Manipuladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: roteadores/suítes de integração `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Proprietário: Smart Contract WG کے ساتھ Plataforma Torii.
- Notas: اینڈپوائنٹس سائن شدہ ٹرانزیکشنز کو قطار میں ڈالते ہیں اور مشترکہ ٹیلیمیٹری میٹرکس (`handle_transaction_with_metrics`) کو دوبارہ استعمال کرتے ہیں۔

### ویریفائنگ کی لائف سائیکل (`/v1/zk/vk/*`) — کورڈ
- Manipuladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) ou `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Proprietário: ZK Working Group کے ساتھ Plataforma Torii سپورٹ۔
- Notas: Esquemas DTOs Norito کے مطابق ہیں جنہیں SDKs ریفرنس کرتے ہیں؛ limitação de taxa `limits.rs` کے ذریعے نافذ ہے۔

### Nexus Connect (`/v1/connect/*`) — کورڈ (recurso `connect`)
- Manipuladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Ligação do roteador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Testes: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, سیشن لائف سائیکل, WS handshake) اور
  matriz de recursos do roteador کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Proprietário: Nexus Connect WG.
- Notas: chaves de limite de taxa `limits::rate_limit_key` کے ذریعے ٹریک ہوتے ہیں؛ Contadores ٹیلیمیٹری `connect.*` میٹرکس کو فیڈ کرتے ہیں۔

### Relé Kaigi ٹیلیمیٹری — کورڈ
- Manipuladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
Ligação do roteador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testes: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: fluxo SSE عالمی transmissão چینل کو دوبارہ استعمال کرتا ہے جبکہ ٹیلیمیٹری پروفائل gating نافذ کرتا ہے؛ esquemas de resposta `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی ہیں۔

## ٹیسٹ کوریج خلاصہ- Testes de fumaça do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs`) یقینی بناتے ہیں کہ combinações de recursos ہر rota رجسٹر کریں اور sincronização de geração OpenAPI میں رہے۔
- Endpoint مخصوص suites اکاؤنٹ consultas, ciclo de vida do contrato, chaves de verificação ZK, filtros SSE de prova اور Nexus Connect رویوں کو کور کرتے ہیں۔
- Chicotes de paridade SDK (JavaScript, Swift, Python) پہلے سے Alias ​​VOPRF اور endpoints SSE استعمال کرتے ہیں؛ اضافی کام درکار نہیں۔

## اس مرآۃ کو اپ ٹو ڈیٹ رکھنا

A API do aplicativo Torii é uma API de aplicativo que pode ser usada para criar uma API de aplicativo (`docs/source/torii/app_api_parity_audit.md`). O SDK do SDK é uma opção de software livre e confiável