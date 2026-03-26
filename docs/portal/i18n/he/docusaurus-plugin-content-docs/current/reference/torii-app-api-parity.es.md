---
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: torii-app-api-parity
כותרת: Auditoria de paridad de la API de la app de Torii
תיאור: Espejo de la revision TORII-APP-1 para que los equipos de SDK y plataforma confirmen la cobertura publica.
---

Estado: Completado 2026-03-21  
אחראים: Torii Platform, מוביל תוכנית SDK  
מפת דרכים: TORII-APP-1 - Auditoria de paridad de `app_api`

Esta page refleja la auditoria interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que los lectores fura del mono-repo puedan ver que superficies `/v1/*` estan cableadas, probadas y documentadas. La auditoria rastrea las rutas reexportadas a traves de `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` y `add_connect_routes`.

## Alcance y methodo

La auditoria inspecciona las reexportaciones publicas en `crates/iroha_torii/src/lib.rs:256-522` y los constructores de rutas con feature gating. עבור שטחי `/v1/*` לאימות מפת הדרכים:

- יישום המטפל וההגדרות DTO en `crates/iroha_torii/src/routing.rs`.
- רישום הנתב תכונות קבוצתיות `app_api` או `connect`.
- Pruebas de integracion/unitarias existentes y el equipo responsable de la cobertura a largo plazo.

רשימה של פעילים/טרנזקציות ותוספות ורשימת כותרות של אקטיביות אקפטניות של ייעוץ `asset_id` אופציונליות עבור קדם פילטרים, גבולות קיומיים/לחץ גב.

## Autenticacion y firma canonica

- Los נקודות הקצה GET/POST orientados a acceptan headers opcionales de solicitud canonica (`X-Iroha-Account`, `X-Iroha-Signature`) construidos desde `METHOD\n/path\nsorted_query\nsha256(body)`; Torii los envuelve en `QueryRequestWithAuthority` antes de la validacion del executor para que reflejen `/query`.
- Los helpers de SDK se entregan en todos los clientes principales:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` לפי `canonicalRequest.js`.
  - סוויפט: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - אנדרואיד (קוטלין/ג'אווה): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- דוגמאות:
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

## ממצאי נקודות קצה

### Permisos de cuenta (`/v1/accounts/{id}/permissions`) - Cubierto
- מטפל: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: `crates/iroha_torii/tests/accounts_endpoints.rs:126` y `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- בעלים: Torii Platform.
- הערות: La respuesta es un body JSON Norito con `items`/`total`, que coincide con los helpers de pagecion de los SDK.

### Evaluacion OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Cubierto
- מטפל: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- כריכת נתב: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- בדיקות: pruebas inline del handler (`crates/iroha_torii/src/lib.rs:9945-9986`) mas cobertura de SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- בעלים: Torii Platform.
- הערות: La superficie de respuesta impone hex deterministico e identificadores de backend; los SDK consumen el DTO.### Eventos de proof SSE (`GET /v1/events/sse`) - Cubierto
- מטפל: `handle_v1_events_sse` con soporte de filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mas el wiring del filter de proof.
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: סוויטות SSE especificas de proof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y prueba smoke de SSE del pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- בעלים: פלטפורמת Torii (זמן ריצה), בדיקות אינטגרציה WG (מתקנים).
- הערות: Las rutas de filtros de proof se validan מקצה לקצה; la documentacion vive en `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - Cubierto
- מטפלים: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: חבילות נתב/אינטגרציה `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- בעלים: Smart Contract WG con Torii Platform.
- הערות: נקודות קצה עזר להן את נקודות הקצה והשימושיות של נקודות הקצה (`handle_transaction_with_metrics`).

### Ciclo de vida de claves de verificacion (`/v1/zk/vk/*`) - Cubierto
- מטפלים: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- בעלים: ZK Working Group con soporte de Torii Platform.
- הערות: Los DTOs se alinean con los esquemas Norito referenciados por los SDK; מגבלת קצב באמצעות `limits.rs`.

### Nexus Connect (`/v1/connect/*`) - Cubierto (תכונה `connect`)
- מטפלים: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- כריכת נתב: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- בדיקות: `crates/iroha_torii/tests/connect_gating.rs` (שער תכונה, ciclo de vida de sesion, לחיצת יד WS) y
  cobertura de matriz de features del router (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- בעלים: Nexus Connect WG.
- הערות: Las claves de rate limit se rastrean via `limits::rate_limit_key`; los contadores de telemetria alimentan las metricas `connect.*`.### Telemetria de relay Kaigi - Cubierto
- מטפלים: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- כריכת נתב: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- בדיקות: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- הערות: El stream SSE reutiliza el canal global de broadcast mientras aplica el gating del perfil de telemetria; los esquemas de respuesta se documentan en `docs/source/torii/kaigi_telemetry_api.md`.

## קורות חיים של קוברטורה דה פרובאס

- Las pruebas smoke del router (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantizan que las combinaciones de features registren cada ruta y que la generacion de OpenAPI se mantenga en sincronizacion.
- הסוויטות המיוחדות לנקודות קצה קוביות שאילתות קונטרס, ציוני מידע, אימות ZK, מסנני הוכחה SSE ו-Comportamientos de Nexus Connect.
- Los רתמות של SDK (JavaScript, Swift, Python) צורכים כינוי VOPRF ypoints SSE; no se requiere trabajo adicional.

## רמת המציאות

Actualiza esta page y la auditoria fuente (`docs/source/torii/app_api_parity_audit.md`) cuando cambie el comportamiento de la app API de Torii עבור בעלי ה-SDK y los lectores externos signan alineados.