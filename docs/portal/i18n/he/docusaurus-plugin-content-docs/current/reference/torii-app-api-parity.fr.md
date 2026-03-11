---
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: torii-app-api-parity
כותרת: Audit de parite de l'API d'application Torii
תיאור: Miroir de la revue TORII-APP-1 pour que les equipes SDK and plateforme confirment la couverture publicque.
---

תקנון: סיום 2026-03-21  
אחראים: Torii Platform, מוביל תוכנית SDK  
הפניה למפת הדרכים: TORII-APP-1 - audit de parite `app_api`

Cette page reflete l'audit intern `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) אפינ que les lectures en dehors du mono-repo puissent voir quelles surfaces `/v1/*` sont cablees, testees and documentees. L'audit suit les מסלולים לייצוא חוזר דרך `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` et `add_connect_routes`.

## Portee et method

L'audit inspecte les reexportations publiques dans `crates/iroha_torii/src/lib.rs:256-522` et les constructeurs de routes soumis au feature gating. יוצקים משטח צ'אקה `/v1/*` du מפת הדרכים, נאוס אוונס לאמת:

- יישום של מטפל והגדרות של DTO ב-`crates/iroha_torii/src/routing.rs`.
- רישום du routeur sous les groupes de features `app_api` או `connect`.
- בדיקות אינטגרציה/יחידות קיימות וציוד אחראי לזמן ארוך.

Les lists d'actifs/transactions de compte et les lists de détenteurs d'actifs acceptent des paramètres de requête `asset_id` facultatifs pour le pré-filtrage, en plus des limites de pagetion/pressure existantes.

## אימות וחתימה קנונית

- נקודות הקצה GET/POST חושפות אפליקציות נוספות המקובלות בכותרות אופציונליות של בקשת קנונית (`X-Iroha-Account`, `X-Iroha-Signature`) בונה חלק מה-`METHOD\n/path\nsorted_query\nsha256(body)`; Torii les enveloppe dans `QueryRequestWithAuthority` avant la validation de l'executor afin de refleter `/query`.
- Les helpers SDK sont fournis dans tous les clients principaux:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` depuis `canonicalRequest.js`.
  - סוויפט: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - אנדרואיד (קוטלין/ג'אווה): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- דוגמאות:
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

## Inventaire des endpoints

### Permissions de compte (`/v1/accounts/{id}/permissions`) - Couvert
- מטפל: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: `crates/iroha_torii/tests/accounts_endpoints.rs:126` et `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- בעלים: Torii Platform.
- הערות: התשובה היא לגוף JSON Norito עם `items`/`total`, תואם aux helpers de pagetion des SDK.

### הערכה OPRF d'alias (`POST /v1/aliases/voprf/evaluate`) - Couvert
- מטפל: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- כריכת נתב: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- בדיקות: בודק את המטפל המוטבע (`crates/iroha_torii/src/lib.rs:9945-9986`) בתוספת SDK לכיסוי כיסוי
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- בעלים: Torii Platform.
- הערות: La surface de reponse impose un hex deterministe et des identifiants de backend; les SDK consomm le DTO.### Evenements de proof SSE (`GET /v1/events/sse`) - Couvert
- מטפל: `handle_v1_events_sse` avec support de filters (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) בתוספת חיווט עמיד למסנן.
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: הוכחה ספציפית ל-SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) ובדיקת עשן SSE du pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- בעלים: פלטפורמת Torii (זמן ריצה), בדיקות אינטגרציה WG (מתקנים).
- הערות: Les chemins de filtere proof sont valides de bout en bout; התיעוד הזה נראה ב-`docs/source/zk_app_api.md`.

### Cycle de vie des contrats (`/v1/contracts/*`) - Couvert
- מטפלים: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: נתב/אינטגרציה סוויטות `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- בעלים: Smart Contract WG avec Torii Platform.
- הערות: Les endpoints mettent en file des trades signees and reutilisent des metriques de telemetrie partagees (`handle_transaction_with_metrics`).

### Cycle de vie des cles de verification (`/v1/zk/vk/*`) - Couvert
- מטפלים: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) et `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- בעלים: ZK Working Group avec support Torii Platform.
- הערות: Les DTOs s'alignent sur les schemas Norito הפניות ל-SDK; הגבלת התעריף משוערת להטיל באמצעות `limits.rs`.

### Nexus Connect (`/v1/connect/*`) - Couvert (תכונה `connect`)
- מטפלים: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- כריכת נתב: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- בדיקות: `crates/iroha_torii/tests/connect_gating.rs` (שער תכונה, מחזור דה וי דה סשן, לחיצת יד WS) et
  couverture de matrice de features du router (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- בעלים: Nexus Connect WG.
- הערות: Les cles de rate limit sont suivies via `limits::rate_limit_key`; les compteurs de telemetrie alimentent les metriques `connect.*`.### Telemetrie de relay Kaigi - Couvert
- מטפלים: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- כריכת נתב: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- בדיקות: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- הערות: Le flux SSE reutilise le canal global de broadcast tout en appliquant le gating du profil de telemetrie; les schemas de reponse sont documentes dans `docs/source/torii/kaigi_telemetry_api.md`.

## קורות חיים למבחנים

- Les tests smoke du routeur (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantissent que les combinaisons de features enregistrent chaque route et que la generation OpenAPI reste Syncee.
- Les suites d'endpoints couvrent les requetes de comptes, le cycle de vie des contrats, les cles de Verification ZK, les cles de Verification SSE et les comportements Nexus Connect.
- Les harnesses de parite SDK (JavaScript, Swift, Python) consomment deja כינוי VOPRF et les endpoints SSE; aucun travail תוספת דרישה.

## Garder ce miroir a jour

Mettez a jour cette page and l'audit source (`docs/source/torii/app_api_parity_audit.md`) lorsque le comportement de l'app API Torii change afin que les בעלים SDK et les lecteurs externes restent alignes.