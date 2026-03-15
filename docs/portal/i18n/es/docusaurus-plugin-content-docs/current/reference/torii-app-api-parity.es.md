---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridad
título: Auditoría de paridad de la API de la aplicación de Torii
descripción: Espejo de la revisión TORII-APP-1 para que los equipos de SDK y plataforma confirmen la cobertura pública.
---

Estado: Completado 2026-03-21  
Responsables: Plataforma Torii, Líder del Programa SDK  
Referencia del roadmap: TORII-APP-1 - auditoria de paridad de `app_api`

Esta página refleja la auditoria interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que los lectores fuera del mono-repo puedan ver que superficies `/v1/*` están cableadas, probadas y documentadas. La auditoria rastrea las rutas reexportadas a través de `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` y `add_connect_routes`.

## Alcance y método

La auditoria inspecciona las reexportaciones públicas en `crates/iroha_torii/src/lib.rs:256-522` y los constructores de rutas con feature gating. Para cada superficie `/v1/*` del roadmap verificamos:

- Implementación del handler y definiciones DTO en `crates/iroha_torii/src/routing.rs`.
- Registro del enrutador bajo los grupos de características `app_api` o `connect`.
- Pruebas de integracion/unitarias existentes y el equipo responsable de la cobertura a largo plazo.

Las listas de activos/transacciones de cuentas y los listados de titulares de activos aceptan parámetros de consulta `asset_id` opcionales para el prefiltrado, además de los límites existentes de paginación/backpression.## Autenticación y firma canónica

- Los puntos finales GET/POST orientados a aplicaciones aceptan encabezados opcionales de solicitud canónica (`X-Iroha-Account`, `X-Iroha-Signature`) construidos desde `METHOD\n/path\nsorted_query\nsha256(body)`; Torii los envuelve en `QueryRequestWithAuthority` antes de la validación del ejecutor para que reflejen `/query`.
- Los helpers de SDK se entregan en todos los clientes principales:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` desde `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Ejemplos:
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

## Inventario de puntos finales

### Permisos de cuenta (`/v1/accounts/{id}/permissions`) - Cubierto
- Controlador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: `crates/iroha_torii/tests/accounts_endpoints.rs:126` y `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propietario: Plataforma Torii.
- Notas: La respuesta es un body JSON Norito con `items`/`total`, que coincide con los ayudantes de paginación de los SDK.### Evaluación OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Cubierto
- Controlador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Enlace de enrutador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Pruebas: pruebas inline del handler (`crates/iroha_torii/src/lib.rs:9945-9986`) mas cobertura de SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propietario: Plataforma Torii.
- Notas: La superficie de respuesta impone hexadecimal determinístico e identificadores de backend; Los SDK consumen el DTO.

### Eventos de prueba SSE (`GET /v1/events/sse`) - Cubierto
- Manejador: `handle_v1_events_sse` con soporte de filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mas el cableado del filtro de prueba.
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: suites SSE especificas de prueba (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y prueba de humo de SSE del ducto
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propietario: Plataforma Torii (runtime), GT de Pruebas de Integración (fixtures).
- Notas: Las rutas de filtros de prueba se validan de extremo a extremo; la documentacion vive en `docs/source/zk_app_api.md`.### Ciclo de vida de contratos (`/v1/contracts/*`) - Cubierto
- Controladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: suites router/integracion `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propietario: Smart Contract WG con Plataforma Torii.
- Notas: Los endpoints encolan transacciones firmadas y reutilizan métricas compartidas de telemetría (`handle_transaction_with_metrics`).

### Ciclo de vida de claves de verificación (`/v1/zk/vk/*`) - Cubierto
- Controladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propietario: ZK Working Group con soporte de plataforma Torii.
- Notas: Los DTO se alinean con los esquemas Norito referenciados por los SDK; el límite de velocidad se impone vía `limits.rs`.### Nexus Conectar (`/v1/connect/*`) - Cubierto (característica `connect`)
- Controladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Enlace de enrutador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Pruebas: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, ciclo de vida de sesión, handshake WS) y
  cobertura de matriz de características del enrutador (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propietario: Nexus Conectar WG.
- Notas: Las claves de límite de tasa se rastrean vía `limits::rate_limit_key`; los contadores de telemetria alimentan las metricas `connect.*`.

### Telemetria de relevo Kaigi - Cubierto
- Controladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Encuadernación del enrutador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Pruebas: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: El stream SSE reutiliza el canal global de transmisión mientras aplica el gating del perfil de telemetría; los esquemas de respuesta se documentan en `docs/source/torii/kaigi_telemetry_api.md`.

## Resumen de cobertura de pruebas- Las pruebas smoke del router (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantizan que las combinaciones de características registradas en cada ruta y que la generación de OpenAPI se mantiene en sincronización.
- Las suites especificas de endpoints cubren consultas de cuentas, ciclo de vida de contratos, claves de verificación ZK, filtros de prueba SSE y comportamientos de Nexus Connect.
- Los arneses de paridad SDK (JavaScript, Swift, Python) ya consumen Alias ​​VOPRF y endpoints SSE; no se requiere trabajo adicional.

## Mantener este espejo actualizado

Actualiza esta página y la fuente de auditoría (`docs/source/torii/app_api_parity_audit.md`) cuando cambia el comportamiento de la aplicación API de Torii para que los propietarios de SDK y los lectores externos sigan alineados.