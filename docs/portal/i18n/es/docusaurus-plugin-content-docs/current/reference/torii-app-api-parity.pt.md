---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridad
título: Auditoría de paridad de la API de la aplicación Torii
descripción: Espelho da revisao TORII-APP-1 para que como equipos de SDK y plataforma confirmen una cobertura pública.
---

Estado: Concluido 2026-03-21  
Responsaveis: Plataforma Torii, Líder del programa SDK  
Referencia de hoja de ruta: TORII-APP-1 - auditoria de paridade `app_api`

Esta pagina espelha a auditoria interna `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que leitores fora do mono-repo vejam quais superficies `/v1/*` estao conectadas, testadas y documentadas. A auditoria acompanha as rotas reexportadas vía `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Escopo y método

Una auditoría inspeccionada como reexportaciones públicas en `crates/iroha_torii/src/lib.rs:256-522` y os constructores de rotas con función de puerta. Para cada superficie `/v1/*`, verificamos la hoja de ruta:

- Implementación del controlador y definiciones DTO en `crates/iroha_torii/src/routing.rs`.
- Registro del enrutador en los grupos de funciones `app_api` o `connect`.
- Testes de integracao/unitarios existentes y un equipo responsable de la cobertura de largo plazo.

Como lista de activos/transacciones de contacto y titulares de activos aceitam parámetros de consulta `asset_id` opcionales para prefiltrado, además de los límites existentes de paginación/contrapresión.

## Autenticacao e assinatura canónica- Endpoints GET/POST voltados a apps aceitam headers opcionais de requisicao canonica (`X-Iroha-Account`, `X-Iroha-Signature`) construidos de `METHOD\n/path\nsorted_query\nsha256(body)`; o Torii os involucran en `QueryRequestWithAuthority` antes de validar el ejecutor para espelhar `/query`.
- Los ayudantes de SDK existen en todos los clientes principales:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` de `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Ejemplos:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<i105-account-id>", method: "get", path: "/v1/accounts/<i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<i105-account-id>", "get", "/v1/accounts/<i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## Inventario de puntos finales

### Permisos de contacto (`/v1/accounts/{id}/permissions`) - Coberto
- Controlador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propietario: Plataforma Torii.
- Notas: Respuesta y un cuerpo JSON Norito con `items`/`total`, añadido a los ayudantes de paginación de dos SDK.

### Avaliacao OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Coberto
- Controlador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Enlace de enrutador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Pruebas: pruebas en línea del controlador (`crates/iroha_torii/src/lib.rs:9945-9986`) más cobertura de SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propietario: Plataforma Torii.
- Notas: A superficie de respuesta reforzada hex determinística e identificadores de backend; Los SDK contienen DTO.### Eventos de prueba SSE (`GET /v1/events/sse`) - Coberto
- Controlador: `handle_v1_events_sse` con soporte a filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) más o cableado de filtro de prueba.
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: suites SSE especificas de prueba (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y prueba de humo SSE para tubería
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propietario: Plataforma Torii (runtime), GT de Pruebas de Integración (fixtures).
- Notas: Os caminhos de filtro deproof foram validados de extremo a extremo; a documentacao fica em `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - Coberto
- Controladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: suites router/integracao `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propietario: Smart Contract WG con plataforma Torii.
- Notas: Los puntos finales enfileiram transacoes assinadas y reutilizam métricas de telemetría compartidas (`handle_transaction_with_metrics`).### Ciclo de vida de chaves de verificacao (`/v1/zk/vk/*`) - Coberto
- Controladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propietario: ZK Working Group con soporte de la plataforma Torii.
- Notas: Los DTO se alinham y los esquemas Norito referenciados pelos SDK; limitación de velocidad y aplicada vía `limits.rs`.

### Nexus Conectar (`/v1/connect/*`) - Coberto (característica `connect`)
- Controladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Enlace de enrutador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Pruebas: `crates/iroha_torii/tests/connect_gating.rs` (activación de funciones, ciclo de vida de sesión, apretón de manos WS) e
  Cobertura de matriz de funciones del enrutador (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propietario: Nexus Conectar WG.
- Notas: Chaves de rate limit sao rastreadas vía `limits::rate_limit_key`; contadores de telemetria alimentan como métricas `connect.*`.### Telemetria de relevo Kaigi - Coberto
- Controladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Encuadernación del enrutador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Pruebas: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: O stream SSE reutiliza o canal global de transmisión en cuanto se aplica o gating do perfil de telemetría; Los esquemas de respuesta están documentados en `docs/source/torii/kaigi_telemetry_api.md`.

## Resumen de cobertura de testículos

- Testes smoke do router (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantiza que combinacoes de feature registrem todas as rotas e que a geracao de OpenAPI fique sincronizada.
- Suites especificas de endpoints cobrem consultas de contactos, ciclo de vida de contratos, chaves de verificacao ZK, filtros de prueba SSE y comportamentos do Nexus Connect.
- Arneses de paridad de SDK (JavaScript, Swift, Python) y consomem Alias ​​VOPRF y endpoints SSE; nao ha trabalho adicional.

## Manter este espelho actualizado

Actualice esta página y una fuente de auditoria (`docs/source/torii/app_api_parity_audit.md`) cuando el comportamiento de la aplicación API de Torii cambie para que los propietarios de SDK y lectores externos fiquem alinhados.