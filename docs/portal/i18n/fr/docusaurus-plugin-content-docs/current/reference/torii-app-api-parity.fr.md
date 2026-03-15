---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : torii-app-api-parité
titre : Audit de parité de l'API d'application Torii
description : Miroir de la revue TORII-APP-1 pour que les équipes SDK et plateforme confirment la couverture publique.
---

Statut : Résiliation le 2026-03-21  
Responsables : Plateforme Torii, responsable du programme SDK  
Référence de feuille de route : TORII-APP-1 - audit de parite `app_api`

Cette page reflète l'audit interne `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) afin que les lecteurs en dehors du mono-repo puissent voir quelles surfaces `/v2/*` sont câblées, testées et documentées. L'audit suit les routes réexportées via `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` et `add_connect_routes`.

## Portee et méthode

L'audit inspecte les réexportations publiques dans `crates/iroha_torii/src/lib.rs:256-522` et les constructeurs de routes soumis au feature gating. Pour chaque surface `/v2/*` du roadmap, nous avons vérifié :

- Implémentation du handler et définitions de DTO dans `crates/iroha_torii/src/routing.rs`.
- Enregistrement du routeur sous les groupes de fonctionnalités `app_api` ou `connect`.
- Tests d'intégration/unitaires existants et equipe responsable de la couverture à long terme.

Les listes d'actifs/transactions de compte et les listes de détenteurs d'actifs acceptent les paramètres de requête `asset_id` facultatifs pour le pré-filtrage, en plus des limites de pagination/backpression existantes.

## Authentification et signature canonique- Les points de terminaison GET/POST exposent aux applications acceptant les en-têtes optionnels de requête canonique (`X-Iroha-Account`, `X-Iroha-Signature`) construits à partir de `METHOD\n/path\nsorted_query\nsha256(body)` ; Torii l'enveloppe dans `QueryRequestWithAuthority` avant la validation de l'exécuteur afin de refléter `/query`.
- Les helpers SDK sont fournis dans tous les clients principaux :
  - JS/TS : `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` depuis `canonicalRequest.js`.
  -Swift : `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  -Android (Kotlin/Java) : `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemples :
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

### Autorisations de compte (`/v2/accounts/{id}/permissions`) - Couvert
- Gestionnaire : `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO : `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Essais : `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propriétaire : Plateforme Torii.
- Notes : La réponse est un corps JSON Norito avec `items`/`total`, conforme aux helpers de pagination du SDK.### Évaluation OPRF d'alias (`POST /v2/aliases/voprf/evaluate`) - Couvert
- Gestionnaire : `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO : `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Liaison du routeur : `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests : tests inline du handler (`crates/iroha_torii/src/lib.rs:9945-9986`) plus SDK couverture
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propriétaire : Plateforme Torii.
- Notes : La surface de réponse impose un déterministe hexadécimal et des identifiants de backend ; le SDK consomme le DTO.

### Événements de preuve SSE (`GET /v2/events/sse`) - Couvert
- Gestionnaire : `handle_v1_events_sse` avec support de filtres (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO : `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) plus le câblage du filtre proof.
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : suites de preuves spécifiques SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) et test fumée SSE du pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propriétaire : Plateforme Torii (runtime), Integration Tests WG (fixations).
- Notes : Les chemins de filtre proof sont valides de bout en bout ; la documentation se trouve dans `docs/source/zk_app_api.md`.### Cycle de vie des contrats (`/v2/contracts/*`) - Couvert
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
- Propriétaire : Smart Contract WG avec plateforme Torii.
- Notes : Les points de terminaison mettent en fichier des transactions signées et réutilisent des métriques de télémétrie partagées (`handle_transaction_with_metrics`).

### Cycle de vie des clés de vérification (`/v2/zk/vk/*`) - Couvert
- Gestionnaires : `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO : `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Essais : `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propriétaire : ZK Working Group avec support Torii Platform.
- Notes : Les DTO s'alignent sur les schémas Norito références par les SDK ; le limitation de débit est imposée via `limits.rs`.### Nexus Connect (`/v2/connect/*`) - Couvert (fonctionnalité `connect`)
- Gestionnaires : `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO : `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Liaison du routeur : `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests : `crates/iroha_torii/tests/connect_gating.rs` (feature gating, cycle de vie de session, handshake WS) et
  couverture de matrice de fonctionnalités du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propriétaire : Nexus Connect WG.
- Notes : Les clés de limite de taux sont suivies via `limits::rate_limit_key` ; les compteurs de télémétrie alimentent les métriques `connect.*`.

### Télémétrie de relais Kaigi - Couvert
- Gestionnaires : `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO : `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Liaison du routeur : `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Essais : `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes : Le flux SSE réutilise le canal global de diffusion tout en appliquant le gating du profil de télémétrie ; les schémas de réponse sont documentés dans `docs/source/torii/kaigi_telemetry_api.md`.

## Resume de la couverture de tests- Les tests smoke du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantissent que les combinaisons de fonctionnalités enregistrées à chaque itinéraire et que la génération OpenAPI reste synchronisée.
- Les suites d'endpoints couvrent les requêtes de comptes, le cycle de vie des contrats, les clés de vérification ZK, les filtres proof SSE et les comportements Nexus Connect.
- Les harnais de parité SDK (JavaScript, Swift, Python) consomment déjà Alias ​​VOPRF et les endpoints SSE ; aucun travail supplémentaire requis.

## Garder ce miroir à jour

Mettez à jour cette page et l'audit source (`docs/source/torii/app_api_parity_audit.md`) lorsque le comportement de l'app API Torii change afin que les propriétaires SDK et les lecteurs externes restent alignés.