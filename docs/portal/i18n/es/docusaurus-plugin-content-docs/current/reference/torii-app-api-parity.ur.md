---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridad
título: Torii ایپ API برابری کا آڈٹ
descripción: TORII-APP-1 جائزے کی نقل تاکہ SDK اور پلیٹ فارم ٹیمیں عوامی کوریج کی تصدیق کر سکیں۔
---

حیثیت: مکمل 2026-03-21  
Nombre: Plataforma Torii, Líder del programa SDK  
روڈمیپ حوالہ: TORII-APP-1 — `app_api` برابری آڈٹ

یہ صفحہ اندرونی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ مونو-ریپو سے باہر کے قارئین دیکھ سکیں کہ کون سی `/v1/*` سطحیں وائرڈ، ٹیسٹ شدہ اور دستاویزی ہیں۔ یہ آڈٹ ان راستوں کو ٹریک کرتا ہے جو `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` اور `add_connect_routes` کے ذریعے دوبارہ ایکسپورٹ ہوتے ہیں۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی ری ایکسپورٹس اور función de puerta والے روٹ بلڈرز کا جائزہ لیتا ہے۔ روڈمیپ میں ہر `/v1/*` سطح کے لئے ہم نے درج ذیل تصدیق کی:

- `crates/iroha_torii/src/routing.rs` Controlador de software کی نفاذ اور DTO تعریفات۔
- `app_api` یا `connect` فیچر گروپس کے تحت روٹر رجسٹریشن۔
- موجودہ انٹیگریشن/یونٹ ٹیسٹس اور طویل مدتی کوریج کے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن فہرستیں اور اثاثہ ہولڈر لسٹنگز موجودہ paginación/contrapresión حدود کے علاوہ پری فلٹرنگ کے لئے اختیاری `asset_id` consulta پیرامیٹرز قبول کرتی ہیں۔

## تصدیق اور کینونیکل دستخط- ایپ کے لئے GET/POST اینڈپوائنٹس اختیاری کینونیکل ریکوئسٹ ہیڈرز (`X-Iroha-Account`, `X-Iroha-Signature`) قبول کرتے ہیں جو `METHOD\n/path\nsorted_query\nsha256(body)` سے بنائے جاتے ہیں؛ Torii Ejecutor ویلیڈیشن سے پہلے `QueryRequestWithAuthority` میں لپیٹتا ہے تاکہ یہ `/query` کی عکاسی کریں۔
- SDK ہیلپرز تمام بنیادی کلائنٹس میں دستیاب ہیں:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- مثالیں:
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
- Controlador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: `crates/iroha_torii/tests/accounts_endpoints.rs:126` y `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propietario: Plataforma Torii.
- Notas: Cuerpo Norito JSON ہے جس میں `items`/`total` ہے، جو Ayudantes de paginación SDK سے مطابقت رکھتا ہے۔

### Alias OPRF تشخیص (`POST /v1/aliases/voprf/evaluate`) — کورڈ
- Controlador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Enlace de enrutador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Pruebas: controlador کے pruebas en línea (`crates/iroha_torii/src/lib.rs:9945-9986`) کے ساتھ SDK کوریج
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propietario: Plataforma Torii.
- Notas: جواب کی سطح hexadecimal determinista اور identificadores de backend نافذ کرتی ہے؛ SDK اس DTO کو استعمال کرتے ہیں۔### Prueba SSE ایونٹس (`GET /v1/events/sse`) — کورڈ
- Controlador: `handle_v1_events_sse` فلٹر سپورٹ کے ساتھ (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) y cableado de filtro a prueba.
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: prueba مخصوص SSE suites (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y prueba de humo SSE de tubería
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propietario: Plataforma Torii (runtime), GT de Pruebas de Integración (fixtures).
- Notas: filtro de prueba راستے de extremo a extremo توثیق شدہ ہیں؛ دستاویزات `docs/source/zk_app_api.md` میں ہیں۔

### کنٹریکٹ لائف سائیکل (`/v1/contracts/*`) — کورڈ
- Controladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: enrutador/suites de integración `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propietario: Smart Contract WG کے ساتھ Plataforma Torii.
- Notas: اینڈپوائنٹس سائن شدہ ٹرانزیکشنز کو قطار میں ڈالते ہیں اور مشترکہ ٹیلیمیٹری میٹرکس (`handle_transaction_with_metrics`) کو دوبارہ استعمال کرتے ہیں۔### ویریفائنگ کی لائف سائیکل (`/v1/zk/vk/*`) — کورڈ
- Controladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propietario: ZK Working Group کے ساتھ Torii Platform سپورٹ۔
- Notas: Esquemas DTO Norito کے مطابق ہیں جنہیں SDK ریفرنس کرتے ہیں؛ limitación de velocidad `limits.rs` کے ذریعے نافذ ہے۔

### Nexus Connect (`/v1/connect/*`) — کورڈ (característica `connect`)
- Controladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Enlace de enrutador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Pruebas: `crates/iroha_torii/tests/connect_gating.rs` (activación de funciones, protocolo de enlace WS)
  matriz de funciones del enrutador کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propietario: Nexus Conectar WG.
- Notas: teclas de límite de velocidad `limits::rate_limit_key` کے ذریعے ٹریک ہوتے ہیں؛ ٹیلیمیٹری contadores `connect.*` میٹرکس کو فیڈ کرتے ہیں۔### Relevo Kaigi ٹیلیمیٹری — کورڈ
- Controladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Encuadernación del enrutador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Pruebas: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: transmisión SSE عالمی transmisión چینل کو دوبارہ استعمال کرتا ہے جبکہ ٹیلیمیٹری پروفائل gating نافذ کرتا ہے؛ esquemas de respuesta `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی ہیں۔

## ٹیسٹ کوریج خلاصہ

- Pruebas de humo del enrutador (`crates/iroha_torii/tests/router_feature_matrix.rs`) یقینی بناتے ہیں کہ combinaciones de funciones ہر ruta رجسٹر کریں اور OpenAPI sincronización de generación میں رہے۔
- Endpoint مخصوص suites اکاؤنٹ consultas, ciclo de vida del contrato, claves de verificación ZK, filtros SSE de prueba اور Nexus Connect رویوں کو کور کرتے ہیں۔
- Arneses de paridad SDK (JavaScript, Swift, Python) Alias ​​VOPRF y puntos finales SSE استعمال کرتے ہیں؛ اضافی کام درکار نہیں۔

## اس مرآۃ کو اپ ٹو ڈیٹ رکھنا

Esta es la API de la aplicación Torii. مالکان اور بیرونی قارئین ہم آہنگ رہیں۔