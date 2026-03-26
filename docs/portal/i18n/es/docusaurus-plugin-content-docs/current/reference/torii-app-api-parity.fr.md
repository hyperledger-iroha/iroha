---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridad
título: Auditoría de paridad de la API de aplicación Torii
descripción: Espejo de la revista TORII-APP-1 para los equipos SDK y plataformas que confirman la cobertura pública.
---

Estatuto: Terminar 2026-03-21  
Responsables: Plataforma Torii, Líder del Programa SDK  
Referencia de hoja de ruta: TORII-APP-1 - auditoría de paridad `app_api`

Esta página refleja la auditoría interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) a fin de que los lectores en dehors du mono-repo puedan ver las superficies `/v1/*` sont cables, testees y documentees. La auditoría se adapta a las rutas reexportadas a través de `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Portée et methode

La auditoría inspecciona las reexportaciones públicas en `crates/iroha_torii/src/lib.rs:256-522` y los constructores de rutas soumis au feature gating. Para cada superficie `/v1/*` de la hoja de ruta, debemos verificar:

- Implementación del controlador y definiciones de DTO en `crates/iroha_torii/src/routing.rs`.
- Registro del enrutador en los grupos de funciones `app_api` o `connect`.
- Pruebas de integración/unidades existentes y equipos responsables de la cobertura a largo plazo.

Las listas de activos/transacciones de cuenta y las listas de discontinuadores de activos aceptan los parámetros de solicitud `asset_id` facultativos para el prefiltrado, además de los límites de paginación/contrapresión existentes.

## Autenticación y firma canónica- Los puntos finales GET/POST exponen aplicaciones auxiliares que aceptan encabezados opcionales de solicitud canónica (`X-Iroha-Account`, `X-Iroha-Signature`) construidas a partir de `METHOD\n/path\nsorted_query\nsha256(body)`; Torii los sobres en `QueryRequestWithAuthority` antes de la validación del ejecutor hasta el reflector `/query`.
- Los ayudantes SDK son proveedores de todos los clientes principales:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` después de `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Ejemplos:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "soraカタカナ...", method: "get", path: "/v1/accounts/soraカタカナ.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/soraカタカナ.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "soraカタカナ...",
                                                  method: "get",
                                                  path: "/v1/accounts/soraカタカナ.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("soraカタカナ...", "get", "/v1/accounts/soraカタカナ.../assets", "limit=5", ByteArray(0), signer)
```

## Inventario de puntos finales

### Permisos de cuenta (`/v1/accounts/{id}/permissions`) - Cubierto
- Controlador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: `crates/iroha_torii/tests/accounts_endpoints.rs:126` y `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propietario: Plataforma Torii.
- Notas: La respuesta es un cuerpo JSON Norito con `items`/`total`, conforme a los ayudantes de paginación del SDK.### Evaluación OPRF d'alias (`POST /v1/aliases/voprf/evaluate`) - Couvert
- Controlador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Enlace de enrutador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Pruebas: pruebas en línea del controlador (`crates/iroha_torii/src/lib.rs:9945-9986`) más SDK de cobertura
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propietario: Plataforma Torii.
- Notas: La superficie de respuesta impone un hexadecimal determinante y los identificadores de backend; El SDK contiene el DTO.

### Eventos de prueba SSE (`GET /v1/events/sse`) - Couvert
- Controlador: `handle_v1_events_sse` con soporte de filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) más el cableado a prueba de filtro.
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: suites SSE prueba específica (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y prueba de humo SSE du tubería
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propietario: Plataforma Torii (runtime), GT de Pruebas de Integración (fixtures).
- Notas: Les chemins de filtreproof sont valides de bout en bout; La documentación se encuentra en `docs/source/zk_app_api.md`.### Ciclo de vida de contratos (`/v1/contracts/*`) - Couvert
- Controladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: suites enrutador/integración `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propietario: Smart Contract WG con plataforma Torii.
- Notas: Los puntos finales están guardados en el archivo de transacciones firmadas y reutilizados de mediciones de telemetría partagees (`handle_transaction_with_metrics`).

### Ciclo de vida de las celdas de verificación (`/v1/zk/vk/*`) - Couvert
- Controladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propietario: ZK Working Group con soporte para la plataforma Torii.
- Notas: Los DTO están alineados con las referencias de esquemas Norito par les SDK; La limitación de velocidad se impone a través de `limits.rs`.### Nexus Connect (`/v1/connect/*`) - Cubierto (característica `connect`)
- Controladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Enlace de enrutador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Pruebas: `crates/iroha_torii/tests/connect_gating.rs` (activación de funciones, ciclo de vida de sesión, protocolo de enlace WS) y
  Cobertura de matriz de características del enrutador (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propietario: Nexus Conectar WG.
- Notas: Les cles de rate limit sont suivies via `limits::rate_limit_key`; les compteurs de telemetrie alimentent les metrics `connect.*`.

### Telemetría de relevo Kaigi - Couvert
- Controladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Encuadernación del enrutador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Pruebas: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: Le flux SSE reutilise le canal global de broadcast tout en appliquant le gating du profil de telemetrie; Los esquemas de respuesta están documentados en `docs/source/torii/kaigi_telemetry_api.md`.

## Resumen de la cobertura de pruebas.- Las pruebas de humo del enrutador (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantizan que las combinaciones de funciones se registran en cada ruta y que la generación OpenAPI permanece sincronizada.
- Las suites de puntos finales incluyen las solicitudes de cuentas, el ciclo de vida de los contratos, las llaves de verificación ZK, los filtros a prueba de SSE y los comportamientos Nexus Connect.
- Los arneses de paridad SDK (JavaScript, Swift, Python) tienen el alias VOPRF y los puntos finales SSE; aucun trabajo suplementario requis.

## Garder ce miroir a day

Agregue este día a esta página y a la fuente de auditoría (`docs/source/torii/app_api_parity_audit.md`) cuando el comportamiento de la aplicación API Torii cambie según los propietarios del SDK y los lectores externos que se alinean.