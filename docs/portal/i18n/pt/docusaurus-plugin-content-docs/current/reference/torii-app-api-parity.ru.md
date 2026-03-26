---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridade
título: Аудит паритета API приложения Torii
description: Você instalou o TORII-APP-1, seus comandos SDK e plataformas podem ser usados para abrir a rede pública.
---

Status: Final 2026-03-21  
Usuários: Plataforma Torii, líder do programa SDK  
Ссылка в дорожной карте: TORII-APP-1 — аудит паритета `app_api`

Esta página é uma auditoria independente `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), que é a chave para a monoreposição Veja, como usar o `/v1/*` para proteger, proteger e proteger. Аудит отслеживает маршруты, переэкспортируемые через `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Область e método

Аудит проверяет публичные переэкспорты в `crates/iroha_torii/src/lib.rs:256-522` e построители маршрутов с feature gating. Para obter o `/v1/*` no cartão completo, eu provei:

- Realize o manipulador e execute o DTO em `crates/iroha_torii/src/routing.rs`.
- Registre a rota no recurso de grupo `app_api` ou `connect`.
- Certifique-se de que haja integração/configuração e comando que permitam uma boa operação.

Списки активов/транзакций аккаунта и списки держателей активов принимают необязательные query-parâmetros `asset_id` para filtros de pré-venda, limitando a página/pagamento de dados.

## Autenticação e canonicidade

- GET/POST эндпойнты для приложений принимают опциональные заголовки канонического запроса (`X-Iroha-Account`, `X-Iroha-Signature`), instalado em `METHOD\n/path\nsorted_query\nsha256(body)`; Torii foi executado em `QueryRequestWithAuthority` para validar o executor, чтобы они соответствовали `/query`.
- Kits de suporte do SDK disponíveis em nossos clientes:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` ou `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemplos:
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

## Inventar эндпойнтов

### Conta confiável (`/v1/accounts/{id}/permissions`) — Fechar
- Manipulador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Proprietário: Plataforma Torii.
- Notas: Ответ — Norito JSON com `items`/`total`, совпадает с SDK хелперами пагинации.

### OPRF оценка alias (`POST /v1/aliases/voprf/evaluate`) — Покрыто
- Manipulador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Ligação do roteador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Testes: manipulador de testes inline (`crates/iroha_torii/src/lib.rs:9945-9986`) mais SDK покрытие
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Proprietário: Plataforma Torii.
- Notas: Поверхность ответа принуждает детерминированный hex e идентификаторы backend; SDK usa DTO.### Prova de segurança SSE (`GET /v1/events/sse`) — Покрыто
- Manipulador: `handle_v1_events_sse` com filtros adicionais (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mais prova de filtro de fiação.
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
-Testes: prova-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste de fumaça SSE пайплайна
  (`integration_tests/tests/events/sse_smoke.rs`).
- Proprietário: Plataforma Torii (runtime), GT de Testes de Integração (fixtures).
- Notas: Маршруты prova фильтра проверены ponta a ponta; documentação em `docs/source/zk_app_api.md`.

### Contrato de ciclo fechado (`/v1/contracts/*`) — Fechar
- Manipuladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
-DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: roteador/integração сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Proprietário: Smart Contract WG fornecido pela plataforma Torii.
- Notas: Эндпойнты ставят подписанные транзакции в очередь и переиспользуют общие метрики телеметрии (`handle_transaction_with_metrics`).

### Жизненный цикл ключей проверки (`/v1/zk/vk/*`) — Покрыто
- Manipuladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
-DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Proprietário: ZK Working Group por plataforma Torii.
- Notas: DTO согласованы со схемами Norito, на которые ссылаются SDK; limitação de taxa aplicada через `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Покрыто (recurso `connect`)
- Manipuladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Ligação do roteador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Testes: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, жизненный цикл сессии, WS handshake) и
  покрытие матрицы feature роутера (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Proprietário: Nexus Connect WG.
- Notas: Limite de taxa Ключи отслеживаются через `limits::rate_limit_key`; A fiação elétrica é métrica `connect.*`.

### Телеметрия реле Kaigi — Покрыто
- Manipuladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
-DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
Ligação do roteador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testes: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: SSE поток переиспользует глобальный canal de transmissão канал и применяет gating профиля телеметрии; Os itens foram descritos em `docs/source/torii/kaigi_telemetry_api.md`.

## Сводка покрытия тестами- Smoke тесты роутера (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантируют, что комбинации feature регистрируют каждый маршрут и генерация OpenAPI possui sincronização.
- Эндпойнт-специфичные сьюты покрывают запросы аккаунтов, жизненный цикл контрактов, ZK ключи provерки, prova SSE filtros e alimentação Nexus Connect.
- Chicotes de paridade SDK (JavaScript, Swift, Python) уже потребляют Alias ​​VOPRF e SSE эндпойнты; Os trabalhos de conclusão não são necessários.

## Você pode colocar a chave na configuração atual

Обновляйте эту страницу исходный аудит (`docs/source/torii/app_api_parity_audit.md`), когда меняется поведение Torii app API, чтобы instale o SDK e instale a solução instalada.