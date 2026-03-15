---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridad
título: Aplicación de API de partición de auditoría Torii
descripción: Зеркало обзора TORII-APP-1, чтобы команды SDK y plataformas que pueden permitir la descarga de archivos públicos.
---

Estado: Завершено 2026-03-21  
Usuarios: Plataforma Torii, líder del programa SDK  
Ссылка в дорожной карте: TORII-APP-1 — аудит паритета `app_api`

Esta página está disponible para la auditoría `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), cuáles son las principales fuentes de vídeo del monorepositorio. поверхности `/v1/*` подключены, протестированы и задокументированы. La auditoría se realiza en marzo, los puertos deportivos son `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Obligación y método

Audit proporciona pereksports públicos en `crates/iroha_torii/src/lib.rs:256-522` y построители маршрутов con función de puerta. El cable de alimentación `/v1/*` en la tarjeta correspondiente muestra:

- Realización del controlador y operación de DTO en `crates/iroha_torii/src/routing.rs`.
- Registre el enrutador en el grupo con la función `app_api` o `connect`.
- Наличие интеграционных/юнит-тестов и команду, ответственную за долгосрочное покрытие.

Algunas cuentas activas/transmisoras y varias activaciones no configuradas mediante parámetros de consulta `asset_id` para предварительной фильтрации, помимо существующих limитов пагинации/обратного давления.

## Аутентификация и каноническая подпись- Dispositivos GET/POST para el programa de configuración de software opcional (`X-Iroha-Account`, `X-Iroha-Signature`), построенные из `METHOD\n/path\nsorted_query\nsha256(body)`; Torii está instalado en `QueryRequestWithAuthority` antes de que el ejecutor valido, чтобы они соответствовали `/query`.
- SDK de ayuda disponible para todos los clientes actuales:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` y `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Primeros:
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

## Инвентарь эндпойнтов

### Права аккаунта (`/v1/accounts/{id}/permissions`) — Покрыто
- Controlador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: `crates/iroha_torii/tests/accounts_endpoints.rs:126` y `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Propietario: Plataforma Torii.
- Notas: Nota: Norito JSON con `items`/`total`, compatible con páginas de ayuda SDK.

### OPRF оценка alias (`POST /v1/aliases/voprf/evaluate`) — Покрыто
- Controlador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Enlace de enrutador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Pruebas: controlador de pruebas en línea (`crates/iroha_torii/src/lib.rs:9945-9986`) más el archivo SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Propietario: Plataforma Torii.
- Notas: Поверхность ответа принуждает детерминированный hexadecimal e identificadores backend; SDK compatible con DTO.### События prueba SSE (`GET /v1/events/sse`) — Покрыто
- Controlador: `handle_v1_events_sse` с поддержкой фильтров (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) más cableado a prueba de filtro.
- Enlace de enrutador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Pruebas: prueba-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y el panel SSE de prueba de humo
  (`integration_tests/tests/events/sse_smoke.rs`).
- Propietario: Plataforma Torii (runtime), GT de Pruebas de Integración (fixtures).
- Notas: Filtro de prueba de marzo probado de extremo a extremo; documentación en `docs/source/zk_app_api.md`.

### Жизненный цикл контрактов (`/v1/contracts/*`) — Покрыто
- Controladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: enrutador/integración сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Propietario: Smart Contract WG desarrollado en la plataforma Torii.
- Notas: Эндпойнты ставят подписанные транзакции в очередь и переиспользуют общие метрики телеметрии (`handle_transaction_with_metrics`).### Жизненный цикл ключей проверки (`/v1/zk/vk/*`) — Покрыто
- Controladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Enlace de enrutador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Pruebas: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Propietario: Grupo de trabajo ZK de la plataforma Torii.
- Notas: DTO согласованы со схемами Norito, на которые ссылаются SDK; limitación de velocidad aplicada según `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Покрыто (característica `connect`)
- Controladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Enlace de enrutador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Pruebas: `crates/iroha_torii/tests/connect_gating.rs` (activación de funciones, жизненный цикл сессии, protocolo de enlace WS) y
  покрытие матрицы característica роутера (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Propietario: Nexus Conectar WG.
- Notas: Límite de velocidad de Ключи отслеживаются через `limits::rate_limit_key`; телеметрические счетчики питают метрики `connect.*`.### Telemetro de Kaigi — Покрыто
- Controladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Encuadernación del enrutador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Pruebas: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: SSE поток переиспользует глобальный канал и применяет gating профиля телеметрии; схемы ответов описаны в `docs/source/torii/kaigi_telemetry_api.md`.

## Сводка покрытия тестами

- Humo тесты роутера (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantizado, что комбинации característica registro каждый маршрут и генерация OpenAPI остается синхронизированной.
- Dispositivos especializados para el almacenamiento de aplicaciones, contratos de ciclos de trabajo ZK, filtros a prueba de SSE y поведение Nexus Conectar.
- Arneses de paridad SDK (JavaScript, Swift, Python) con alias VOPRF y SSE; дополнительной работы не требуется.

## Поддержание зеркала в актуальном состоянии

Esta página se actualiza y se audita correctamente (`docs/source/torii/app_api_parity_audit.md`), junto con algunas aplicaciones API de la aplicación Torii, SDK y SDK completos. внешние читатели оставались согласованными.