---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : torii-app-api-parité
titre : Torii pour l'API du site
description : TORII-APP-1 est un SDK entièrement disponible en version anglaise.
---

Publié: مکمل 2026-03-21  
مالکان : Plateforme Torii, responsable du programme SDK  
Nom du produit : TORII-APP-1 — `app_api` Nom du produit

یہ صفحہ اندرونی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ مونو-ریپو سے باہر Il s'agit d'un produit `/v1/*` qui s'est avéré être un bon produit pour vous. ہیں۔ یہ آڈٹ ان راستوں کو ٹریک کرتا ہے جو `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` اور `add_connect_routes` کے ذریعے دوبارہ ایکسپورٹ ہوتے ہیں۔

## دائرہ کار اور طریقہ

Le `crates/iroha_torii/src/lib.rs:256-522` est un système de gestion des fonctionnalités et un portail de fonctionnalité et un système de contrôle d'accès Il s'agit d'un `/v1/*` qui est en train de se connecter à votre compte :

- `crates/iroha_torii/src/routing.rs` Gestionnaire de fichiers pour le gestionnaire DTO
- `app_api` et `connect` sont des produits de haute qualité.
- موجودہ انٹیگریشن/یونٹ ٹیسٹس اور طویل مدتی کوریج کے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن فہرستیں اور اثاثہ ہولڈر لسٹنگز موجودہ pagination/contre-pression حدود کے علاوہ پری فلٹرنگ کے لئے اختیاری `asset_id` query پیرامیٹرز قبول کرتی ہیں۔

## تصدیق اور کینونیکل دستخط- Voici la procédure à suivre pour GET/POST (`X-Iroha-Account`, `X-Iroha-Signature`) Il s'agit d'un modèle `METHOD\n/path\nsorted_query\nsha256(body)` pour un achat en ligne Torii exécuteur testamentaire et exécuteur testamentaire `QueryRequestWithAuthority` exécuteur testamentaire `/query` کی عکاسی کریں۔
- Le SDK est un outil de création de contenu complet :
  - JS/TS : `canonicalRequest.js` ou `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  -Swift : `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  -Android (Kotlin/Java) : `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- مثالیں:
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

## اینڈپوائنٹ فہرست

### اکاؤنٹ اجازتیں (`/v1/accounts/{id}/permissions`) — کورڈ
- Gestionnaire : `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO : `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propriétaire : Plateforme Torii.
- Notes : Le corps JSON Norito et les outils `items`/`total` sont également associés aux aides à la pagination du SDK et aux aides à la pagination du SDK.

### Alias OPRF تشخیص (`POST /v1/aliases/voprf/evaluate`) — کورڈ
- Gestionnaire : `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO : `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Liaison du routeur : `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests : gestionnaire de tests en ligne (`crates/iroha_torii/src/lib.rs:9945-9986`) et SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propriétaire : Plateforme Torii.
- Notes : Il s'agit d'un hexadécimal déterministe et d'identifiants backend. SDK pour DTO et les kits de développement logiciel### Preuve SSE ایونٹس (`GET /v1/events/sse`) — کورڈ
- Gestionnaire : `handle_v1_events_sse` فلٹر سپورٹ کے ساتھ (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO : `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) Câblage de filtre à l'épreuve.
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : preuve مخصوص suites SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) pour le test de fumée SSE du pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propriétaire : Plateforme Torii (runtime), Integration Tests WG (fixations).
- Notes : filtre de preuve de bout en bout دستاویزات `docs/source/zk_app_api.md` ہیں۔

### کنٹریکٹ لائف سائیکل (`/v1/contracts/*`) — کورڈ
- Gestionnaires : `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO : `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests : suites routeur/intégration `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propriétaire : Smart Contract WG et plateforme Torii.
- Notes : میٹرکس (`handle_transaction_with_metrics`) pour votre compte bancaire### ویریفائنگ کی لائف سائیکل (`/v1/zk/vk/*`) — کورڈ
- Gestionnaires : `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO : `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Essais : `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propriétaire : ZK Working Group et Plateforme Torii.
- Remarques : les schémas DTO Norito et les SDK sont également disponibles. limitation de débit `limits.rs` کے ذریعے نافذ ہے۔

### Nexus Connect (`/v1/connect/*`) — Ici (fonctionnalité `connect`)
- Gestionnaires : `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO : `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Liaison du routeur : `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests : `crates/iroha_torii/tests/connect_gating.rs` (gestion des fonctionnalités, prise de contact WS) et
  matrice des fonctionnalités du routeur ici (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propriétaire : Nexus Connect WG.
- Remarques : touches de limite de débit `limits::rate_limit_key`. Compteurs `connect.*` pour compteurs### Relais Kaigi ٹیلیمیٹری — کورڈ
- Gestionnaires : `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO : `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Liaison du routeur : `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Essais : `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes : SSE stream عالمی broadcast چینل کو دوبارہ استعمال کرتا ہے جبکہ ٹیلیمیٹری پروفائل gating نافذ کرتا ہے؛ schémas de réponse `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی ہیں۔

## ٹیسٹ کوریج خلاصہ

- Tests de fumée du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs`) pour plusieurs combinaisons de fonctionnalités et pour la synchronisation de la génération OpenAPI.
- Les suites de points de terminaison pour les requêtes, le cycle de vie des contrats, les clés de vérification ZK, les filtres SSE de preuve et Nexus Connect sont compatibles avec les clés de vérification.
- Harnais de parité SDK (JavaScript, Swift, Python) pour les alias VOPRF et les points de terminaison SSE. اضافی کام درکار نہیں۔

## اس مرآۃ کو اپ ٹو ڈیٹ رکھنا

L'API de l'application Torii est disponible en version SDK (`docs/source/torii/app_api_parity_audit.md`) et le SDK est disponible. مالکان اور بیرونی قارئین ہم آہنگ رہیں۔