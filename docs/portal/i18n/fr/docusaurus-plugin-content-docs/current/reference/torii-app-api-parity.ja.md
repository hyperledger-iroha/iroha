---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94162d6d8d5c0e22f942d8a6328cd8c7d00f8c1b4972a9c023ff96392f080c60
source_last_modified: "2026-01-30T09:29:51+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: torii-app-api-parity
title: Audit de parite de l'API d'application Torii
description: Miroir de la revue TORII-APP-1 pour que les equipes SDK et plateforme confirment la couverture publique.
---

Statut: Termine 2026-03-21  
Responsables: Torii Platform, SDK Program Lead  
Reference de roadmap: TORII-APP-1 - audit de parite `app_api`

Cette page reflete l'audit interne `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) afin que les lecteurs en dehors du mono-repo puissent voir quelles surfaces `/v2/*` sont cablees, testees et documentees. L'audit suit les routes reexportees via `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` et `add_connect_routes`.

## Portee et methode

L'audit inspecte les reexportations publiques dans `crates/iroha_torii/src/lib.rs:256-522` et les constructeurs de routes soumis au feature gating. Pour chaque surface `/v2/*` du roadmap, nous avons verifie:

- Implementation du handler et definitions de DTO dans `crates/iroha_torii/src/routing.rs`.
- Enregistrement du routeur sous les groupes de features `app_api` ou `connect`.
- Tests d'integration/unitaires existants et equipe responsable de la couverture a long terme.

Les listes d'actifs/transactions de compte et les listes de détenteurs d'actifs acceptent des paramètres de requête `asset_id` facultatifs pour le pré-filtrage, en plus des limites de pagination/backpressure existantes.

## Authentification et signature canonique

- Les endpoints GET/POST exposes aux apps acceptent des headers optionnels de requete canonique (`X-Iroha-Account`, `X-Iroha-Signature`) construits a partir de `METHOD\n/path\nsorted_query\nsha256(body)`; Torii les enveloppe dans `QueryRequestWithAuthority` avant la validation de l'executor afin de refleter `/query`.
- Les helpers SDK sont fournis dans tous les clients principaux:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` depuis `canonicalRequest.js`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemples:
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

## Inventaire des endpoints

### Permissions de compte (`/v2/accounts/{id}/permissions`) - Couvert
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notes: La reponse est un body JSON Norito avec `items`/`total`, conforme aux helpers de pagination des SDK.

### Evaluation OPRF d'alias (`POST /v2/aliases/voprf/evaluate`) - Couvert
- Handler: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests: tests inline du handler (`crates/iroha_torii/src/lib.rs:9945-9986`) plus couverture SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Owner: Torii Platform.
- Notes: La surface de reponse impose un hex deterministe et des identifiants de backend; les SDK consomment le DTO.

### Evenements de proof SSE (`GET /v2/events/sse`) - Couvert
- Handler: `handle_v1_events_sse` avec support de filtres (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) plus le wiring du filtre proof.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: suites SSE specifique proof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) et test smoke SSE du pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notes: Les chemins de filtre proof sont valides de bout en bout; la documentation se trouve dans `docs/source/zk_app_api.md`.

### Cycle de vie des contrats (`/v2/contracts/*`) - Couvert
- Handlers: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: suites router/integration `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Owner: Smart Contract WG avec Torii Platform.
- Notes: Les endpoints mettent en file des transactions signees et reutilisent des metriques de telemetrie partagees (`handle_transaction_with_metrics`).

### Cycle de vie des cles de verification (`/v2/zk/vk/*`) - Couvert
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group avec support Torii Platform.
- Notes: Les DTOs s'alignent sur les schemas Norito references par les SDK; le rate limiting est impose via `limits.rs`.

### Nexus Connect (`/v2/connect/*`) - Couvert (feature `connect`)
- Handlers: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, cycle de vie de session, handshake WS) et
  couverture de matrice de features du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Owner: Nexus Connect WG.
- Notes: Les cles de rate limit sont suivies via `limits::rate_limit_key`; les compteurs de telemetrie alimentent les metriques `connect.*`.

### Telemetrie de relay Kaigi - Couvert
- Handlers: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router binding: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Tests: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes: Le flux SSE reutilise le canal global de broadcast tout en appliquant le gating du profil de telemetrie; les schemas de reponse sont documentes dans `docs/source/torii/kaigi_telemetry_api.md`.

## Resume de la couverture de tests

- Les tests smoke du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantissent que les combinaisons de features enregistrent chaque route et que la generation OpenAPI reste synchronisee.
- Les suites d'endpoints couvrent les requetes de comptes, le cycle de vie des contrats, les cles de verification ZK, les filtres proof SSE et les comportements Nexus Connect.
- Les harnesses de parite SDK (JavaScript, Swift, Python) consomment deja Alias VOPRF et les endpoints SSE; aucun travail supplementaire requis.

## Garder ce miroir a jour

Mettez a jour cette page et l'audit source (`docs/source/torii/app_api_parity_audit.md`) lorsque le comportement de l'app API Torii change afin que les owners SDK et les lecteurs externes restent alignes.
