---
lang: fr
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : torii-app-api-parité
title : Audience de parité API приложения Torii
description : L'outil TORII-APP-1 est disponible pour les commandes SDK et les plates-formes qui peuvent être publiées.
---

Statut : Date 2026-03-21  
Fournisseurs : Plateforme Torii, responsable du programme SDK  
Rechercher sur la carte principale : TORII-APP-1 — audition pariteta `app_api`

Cette page vient de l'auditeur externe `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), qui est dans le monorepositorio la plus visible, comme en témoignent les pouvoirs publics. `/v2/*` sont des compléments, des protestations et des commentaires. L'auditeur s'occupe des marchés, notamment `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` et `add_connect_routes`.

## Fonctionnement et méthode

L'auditeur a vérifié les sports publics dans le cadre de l'`crates/iroha_torii/src/lib.rs:256-522` et les marchés commerciaux avec la fonctionnalité de portail. Pour le numéro de téléphone `/v2/*` sur la carte locale, nous avons vérifié :

- Réalisation du gestionnaire et mise en œuvre du DTO dans `crates/iroha_torii/src/routing.rs`.
- Enregistrement du routeur dans la fonction `app_api` ou `connect`.
- Наличие интеграционных/юнит-testов и команду, ответственную за долгосрочное покрытие.

Les paramètres d'action/de transfert de compte et les paramètres d'activité de travail ne nécessitent pas de paramètres de requête `asset_id` pour le projet précédent Filtrage, selon les limites de la page/durée de fonctionnement.

## Services d'authentification et de canonique- OBTENTIONS GET/POST pour la procédure d'installation opérationnelle (`X-Iroha-Account`, `X-Iroha-Signature`), postérieure à `METHOD\n/path\nsorted_query\nsha256(body)` ; Torii fonctionne avec `QueryRequestWithAuthority` avant la validation de l'exécuteur testamentaire, qui est associé à `/query`.
- Aide au téléchargement du SDK pour tous les clients actuels :
  - JS/TS : `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` ou `canonicalRequest.js`.
  -Swift : `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  -Android (Kotlin/Java) : `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemples :
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

## Rechercher des entreprises

### Compte de banque (`/v2/accounts/{id}/permissions`) — Marché
- Gestionnaire : `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO : `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propriétaire : Plateforme Torii.
- Remarques : Il s'agit d'un JSON Norito avec `items`/`total`, intégré à la page d'aide du SDK.

### Alias de l'OPRF (`POST /v2/aliases/voprf/evaluate`) — Marché
- Gestionnaire : `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTO : `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Liaison du routeur : `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests : gestionnaire de tests en ligne (`crates/iroha_torii/src/lib.rs:9945-9986`) et téléchargement du SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propriétaire : Plateforme Torii.
- Notes : Поверхность ответа принуждает детерминированный hex и IDENTIFICATORS backend ; Le SDK prend en charge DTO.### Preuve SSE (`GET /v2/events/sse`) — Prêt
- Gestionnaire : `handle_v1_events_sse` avec les filtres suivants (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO : `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) et câblage à l'épreuve du filtre.
- Liaison du routeur : `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests : preuve-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) et test de fumée SSE
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propriétaire : Plateforme Torii (runtime), Integration Tests WG (fixations).
- Notes : Маршруты preuve фильтра проверены de bout en bout ; documentation dans `docs/source/zk_app_api.md`.

### Contrats de cycle de vie (`/v2/contracts/*`) — Marché
- Gestionnaires : `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
-DTO : `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests : routeur/intégration сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propriétaire : Smart Contract WG associé à la plateforme Torii.
- Remarques : L'entreprise est en mesure de procéder à des transitions en vue d'obtenir et de suivre des mesures télémétriques (`handle_transaction_with_metrics`).### Clé à molette (`/v2/zk/vk/*`) — Marché
- Gestionnaires : `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
-DTO : `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Liaison du routeur : `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Essais : `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propriétaire : ZK Working Group pour la plate-forme Torii.
- Notes : DTO contient le schéma Norito, pour le SDK ; limitation de débit appliquée depuis `limits.rs`.

### Nexus Connect (`/v2/connect/*`) — Prise (fonctionnalité `connect`)
- Gestionnaires : `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO : `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Liaison du routeur : `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests : `crates/iroha_torii/tests/connect_gating.rs` (gestion des fonctionnalités, sessions de plusieurs cycles, poignée de main WS) et
  покрытие матрицы fonctionnalité роутера (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propriétaire : Nexus Connect WG.
- Notes : La limite de taux clé s'applique à `limits::rate_limit_key` ; Les mesures télémétriques `connect.*`.### Телеметрия реле Kaigi — Покрыто
- Gestionnaires : `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO : `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Liaison du routeur : `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Essais : `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes : SSE поток переиспользует глобальный канал и применяет gating профиля телеметрии ; Les schémas sont fournis par `docs/source/torii/kaigi_telemetry_api.md`.

## La boisson a été testée

- Le routeur de test de fumée (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantit que la fonction combinée enregistre le marché et la génération OpenAPI. синхронизированной.
- Les services spécialisés fournissent des informations sur les comptes, les contrats de plusieurs types, les clés ZK, les filtres à l'épreuve SSE et поведение Nexus Connect.
- Les harnais de parité SDK (JavaScript, Swift, Python) permettent d'utiliser Alias ​​VOPRF et SSE ; Les robots supplémentaires ne sont pas concernés.

## Подддержание зеркала в актуальном состоянии

Consultez cette page et votre audit (`docs/source/torii/app_api_parity_audit.md`) pour découvrir l'API de l'application Torii, qui contient le SDK et le SDK. внешние читатели оставались согласованными.