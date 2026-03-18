---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : torii-app-api-parité
titre : Auditorium de parité de l'API de l'application de Torii
description : En particulier la révision TORII-APP-1 pour que les équipes de SDK et la plate-forme confirment la couverture publique.
---

État : Terminé 2026-03-21  
Responsables : Plateforme Torii, responsable du programme SDK  
Référence de la feuille de route : TORII-APP-1 - salle de parité de `app_api`

Cette page reflète l'auditoire interne `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) pour que les lecteurs hors du mono-repo puissent voir que la surface `/v1/*` est câblée, testée et documentée. L'auditoire rastrea les routes réexportées à travers les `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` et `add_connect_routes`.

## Alcance et méthode

L'auditoire inspecte les réexportations publiques en `crates/iroha_torii/src/lib.rs:256-522` et les constructeurs de routes avec fonctionnalité de contrôle. Pour chaque surface `/v1/*` de la feuille de route vérifiée :

- Implémentation du gestionnaire et définitions DTO en `crates/iroha_torii/src/routing.rs`.
- Enregistrez le routeur sous les groupes de fonctionnalités `app_api` ou `connect`.
- Essais d'intégration/unités existantes et équipe responsable de la couverture sur une grande place.

Les listes d'actifs/transactions de comptes et les listes de titulaires d'actifs acceptent les paramètres de consultation `asset_id` optionnels pour le préfiltrage, en plus des limites existantes de pagination/contre-pression.## Autentification et entreprise canonique

- Les points de terminaison GET/POST sont orientés vers les applications acceptant les en-têtes optionnels de sollicitude canonique (`X-Iroha-Account`, `X-Iroha-Signature`) construits à partir de `METHOD\n/path\nsorted_query\nsha256(body)` ; Torii est envoyé en `QueryRequestWithAuthority` avant la validation de l'exécuteur pour refléter `/query`.
- Les assistants du SDK sont intégrés à tous les clients principaux :
  - JS/TS : `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` à partir de `canonicalRequest.js`.
  -Swift : `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  -Android (Kotlin/Java) : `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemples :
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v1/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v1/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v1/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## Inventaire des points de terminaison

### Permis de compte (`/v1/accounts/{id}/permissions`) - Cubierto
- Gestionnaire : `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO : `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propriétaire : Plateforme Torii.
- Notes : La réponse est un corps JSON Norito avec `items`/`total`, qui coïncide avec les aides de pagination du SDK.### Évaluation OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Cubierto
- Gestionnaire : `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO : `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Liaison du routeur : `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests : tests en ligne du gestionnaire (`crates/iroha_torii/src/lib.rs:9945-9986`) avec couverture du SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propriétaire : Plateforme Torii.
- Remarques : La surface de réponse implique des identifiants et des identifiants hexadécimaux de backend ; Le SDK utilise le DTO.

### Événements de preuve SSE (`GET /v1/events/sse`) - Cubierto
- Gestionnaire : `handle_v1_events_sse` avec support de filtres (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO : `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) pour le câblage du filtre de preuve.
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : suites SSE spécifiques de preuve (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) et vérification de la fumée du SSE du pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propriétaire : Plateforme Torii (runtime), Integration Tests WG (fixations).
- Remarques : Les itinéraires des filtres de preuve sont validés de bout en bout ; la documentación vive en `docs/source/zk_app_api.md`.### Cycle de vie de contrat (`/v1/contracts/*`) - Cubierto
- Gestionnaires : `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO : `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests : suites routeur/intégration `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propriétaire : Smart Contract WG avec la plateforme Torii.
- Notes : Les points finaux encolan transactions firmadas et réutilisent les metrics compartimentés de télémétrie (`handle_transaction_with_metrics`).

### Cycle de vie des clés de vérification (`/v1/zk/vk/*`) - Cubierto
- Gestionnaires : `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO : `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Essais : `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propriétaire : ZK Working Group avec support de plateforme Torii.
- Notes : Les DTO sont alignés avec les schémas Norito référencés par le SDK ; La limitation du débit s'impose via `limits.rs`.### Nexus Connect (`/v1/connect/*`) - Cubierto (fonctionnalité `connect`)
- Gestionnaires : `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO : `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Liaison du routeur : `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests : `crates/iroha_torii/tests/connect_gating.rs` (feature gating, cycle de vie de session, handshake WS) et
  couverture de la matrice des fonctionnalités du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propriétaire : Nexus Connect WG.
- Remarques : Les touches de limite de taux sont rastrées via `limits::rate_limit_key` ; les contadores de télémétrie alimentant les mesures `connect.*`.

### Télémétrie du relais Kaigi - Cubierto
- Gestionnaires : `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO : `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Liaison du routeur : `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Essais : `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes : Le flux SSE réutilise le canal mondial de diffusion en appliquant le portail du profil de télémétrie ; les questions de réponse sont documentées en `docs/source/torii/kaigi_telemetry_api.md`.

## Résumé de la couverture des essais- Les essais du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantissent que les combinaisons de fonctionnalités sont enregistrées chaque fois et que la génération de OpenAPI est maintenue en synchronisation.
- Les suites spécifiques des points de terminaison contiennent des requêtes de comptes, un cycle de vie de contrats, des clés de vérification ZK, des filtres de preuve SSE et des composants de Nexus Connect.
- Le SDK de parité (JavaScript, Swift, Python) est utilisé pour utiliser l'alias VOPRF et les points de terminaison SSE ; cela ne nécessite pas de travail supplémentaire.

## Maintenir cet article actualisé

Actualisez cette page et l'auditoire source (`docs/source/torii/app_api_parity_audit.md`) lorsque vous modifiez le comportement de l'API de l'application Torii pour que les propriétaires du SDK et les lecteurs externes soient connectés.