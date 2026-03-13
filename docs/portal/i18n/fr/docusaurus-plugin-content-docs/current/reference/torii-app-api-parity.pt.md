---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : torii-app-api-parité
titre : Auditorium de parité de l'API de l'application Torii
description : Nous avons révisé TORII-APP-1 pour que les équipes du SDK et la plate-forme confirment la couverture publique.
---

Statut : Concludo 2026-03-21  
Responsaveis: Plateforme Torii, responsable du programme SDK  
Référence de la feuille de route : TORII-APP-1 - salle de parité `app_api`

Cette page s'affiche dans l'auditoire interne `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) pour que les lecteurs des forums mono-repo voient la surface `/v2/*` connectée, testée et documentée. Un auditoire accompagne les rotations réexportées via `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` et `add_connect_routes`.

## Escopo et méthode

Un auditoire inspecte les réexportations publiques dans `crates/iroha_torii/src/lib.rs:256-522` et les constructeurs de rotations avec fonctionnalité de contrôle. Pour chaque surface `/v2/*`, nous vérifions la feuille de route :

- Implémentation du gestionnaire et définition du DTO dans `crates/iroha_torii/src/routing.rs`.
- Enregistrez le routeur dans nos groupes de fonctionnalités `app_api` ou `connect`.
- Testes d'intégration/unités existantes et équipe responsable de la couverture de longo prazo.

En tant que liste des agents/transactions du contenu et des titulaires des agents, ainsi que des paramètres de consultation `asset_id` optionnels pour le pré-filtrage, il existe également des limites de page/contre-pression.

## Autenticacao e assinatura canonica- Les points de terminaison GET/POST sont connectés aux applications ainsi qu'aux en-têtes sélectionnés de manière standard (`X-Iroha-Account`, `X-Iroha-Signature`) construits par `METHOD\n/path\nsorted_query\nsha256(body)` ; o Torii est impliqué dans `QueryRequestWithAuthority` avant la validation de l'exécuteur pour exécuter `/query`.
- Les aides du SDK existent dans tous les clients principaux :
  - JS/TS : `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` de `canonicalRequest.js`.
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

## Inventaire des points de terminaison

### Permis de contact (`/v2/accounts/{id}/permissions`) - Coberto
- Gestionnaire : `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO : `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propriétaire : Plateforme Torii.
- Notes : La réponse est un corps JSON Norito avec `items`/`total`, ajouté aux aides de pagination des SDK.

### Avaliação OPRF de alias (`POST /v2/aliases/voprf/evaluate`) - Coberto
- Gestionnaire : `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO : `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Liaison du routeur : `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests : tests en ligne du gestionnaire (`crates/iroha_torii/src/lib.rs:9945-9986`) plus cobertura du SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propriétaire : Plateforme Torii.
- Remarques : Une surface de réponse renforcée par un déterministe hexadécimal et des identifiants de backend ; Les SDK du système d'exploitation utilisent le DTO.### Événements de preuve SSE (`GET /v2/events/sse`) - Coberto
- Gestionnaire : `handle_v1_events_sse` avec prise en charge des filtres (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO : `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mais le câblage du filtre de preuve.
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : suites SSE spécifiques de preuve (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) et test de fumée SSE du pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propriétaire : Plateforme Torii (runtime), Integration Tests WG (fixations).
- Remarques : Os caminhos de filtro de proof foram validés de bout en bout ; un document fica em `docs/source/zk_app_api.md`.

### Cycle de vie de contrat (`/v2/contracts/*`) - Coberto
- Gestionnaires : `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO : `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests : suites routeur/intégração `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propriétaire : Smart Contract WG avec plateforme Torii.
- Notes : Les points finaux enregistrent les transactions effectuées et réutilisent les mesures de télémétrie partagées (`handle_transaction_with_metrics`).### Cycle de vie de chaves de vérification (`/v2/zk/vk/*`) - Coberto
- Gestionnaires : `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO : `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Essais : `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propriétaire : ZK Working Group avec support de la plateforme Torii.
- Notes : Les DTO sont associés aux schémas Norito référencés par les SDK ; limitation de débit et appliquée via `limits.rs`.

### Nexus Connect (`/v2/connect/*`) - Coberto (fonctionnalité `connect`)
- Gestionnaires : `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO : `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Liaison du routeur : `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests : `crates/iroha_torii/tests/connect_gating.rs` (fonctionnalité, cycle de vie de session, poignée de main WS) et
  couverture de la matrice de fonctionnalités du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propriétaire : Nexus Connect WG.
- Notes : Chaves de rate limit sao rastreadas via `limits::rate_limit_key` ; contadores de telemetria alimentaire as metricas `connect.*`.### Télémétrie du relais Kaigi - Coberto
- Gestionnaires : `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO : `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Liaison du routeur : `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Essais : `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes : Le flux SSE réutilise le canal mondial de diffusion en ce qui concerne l'application du portail du profil de télémétrie ; Les schémas de réponse sont documentés dans `docs/source/torii/kaigi_telemetry_api.md`.

## CV de couverture des testicules

- Les tests de fumée du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantissent que les combinaisons de fonctionnalités enregistrent toutes les rotations et que la gestion de OpenAPI est synchronisée.
- Suites spécifiques aux points de terminaison concernant les requêtes de contenu, le cycle de vie des contrats, les éléments de vérification ZK, les filtres de preuve SSE et les composants Nexus Connect.
- Exploitation de la parité du SDK (JavaScript, Swift, Python) et du consommateur Alias ​​VOPRF et des points de terminaison SSE ; nao ha trabalho supplémentaire.

## Manter cet article actualisé

Actualisez cette page et la police d'auditoire (`docs/source/torii/app_api_parity_audit.md`) lorsque le comportement de l'API de l'application Torii est modifié pour les propriétaires du SDK et les lecteurs externes.