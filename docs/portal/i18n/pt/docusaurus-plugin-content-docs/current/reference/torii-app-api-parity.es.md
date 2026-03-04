---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridade
título: Auditoria de paridade da API do aplicativo de Torii
description: Espejo da revisão TORII-APP-1 para que os equipamentos de SDK e plataforma confirmem a cobertura pública.
---

Estado: Concluído 2026-03-21  
Responsáveis: Plataforma Torii, líder do programa SDK  
Referência do roteiro: TORII-APP-1 - auditorias de paridade de `app_api`

Esta página reflete os auditórios internos `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que os leitores fora do mono-repo possam ver que as superfícies `/v1/*` estão cabeadas, testadas e documentadas. Os auditórios rastreiam as rotas reexportadas através de `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Alcance e método

Os auditórios inspecionam as reexportações públicas em `crates/iroha_torii/src/lib.rs:256-522` e os construtores de rotas com recurso de gate. Para cada superfície `/v1/*` do roadmap verificamos:

- Implementação do manipulador e definições DTO em `crates/iroha_torii/src/routing.rs`.
- Registro do roteador abaixo dos grupos de recursos `app_api` ou `connect`.
- Testes de integração/unitarias existentes e o equipamento responsável pela cobertura no largo pátio.

As listas de ativos/transações de contas e a lista de titulares de ativos aceitam parâmetros de consulta `asset_id` opcionais para o pré-filtrado, além dos limites existentes de paginação/contrapressão.

## Autenticação e firma canônica

- Os endpoints GET/POST orientados para aplicativos aceitam cabeçalhos opcionais de solicitação canônica (`X-Iroha-Account`, `X-Iroha-Signature`) construídos a partir de `METHOD\n/path\nsorted_query\nsha256(body)`; Torii é enviado para `QueryRequestWithAuthority` antes da validação do executor para que `/query` seja refletido.
- Os ajudantes do SDK são entregues a todos os clientes principais:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` de `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Exemplos:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "ih58...", method: "get", path: "/v1/accounts/ih58.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/ih58.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "ih58...",
                                                  method: "get",
                                                  path: "/v1/accounts/ih58.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("ih58...", "get", "/v1/accounts/ih58.../assets", "limit=5", ByteArray(0), signer)
```

## Inventário de endpoints

### Permissões de conta (`/v1/accounts/{id}/permissions`) - Cubierto
- Manipulador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Proprietário: Plataforma Torii.
- Notas: A resposta é um corpo JSON Norito com `items`/`total`, que coincide com os auxiliares de paginação do SDK.

### Avaliação OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Cubierto
- Manipulador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Ligação do roteador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Testes: testes inline do manipulador (`crates/iroha_torii/src/lib.rs:9945-9986`) mas cobertura do SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Proprietário: Plataforma Torii.
- Notas: A superfície de resposta impone hexadecimal determinístico e identificadores de backend; o SDK consome o DTO.### Eventos de prova SSE (`GET /v1/events/sse`) - Cubierto
- Manipulador: `handle_v1_events_sse` com suporte de filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mas a fiação do filtro de prova.
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: suítes SSE especificações de prova (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste de fumaça de SSE do gasoduto
  (`integration_tests/tests/events/sse_smoke.rs`).
- Proprietário: Plataforma Torii (runtime), GT de Testes de Integração (fixtures).
- Notas: As rotas de filtros de prova são válidas de ponta a ponta; a documentação vive em `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - Cubierto
- Manipuladores: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: suítes roteador/integração `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Proprietário: Smart Contract WG com Plataforma Torii.
- Notas: Los endpoints encolan transações firmadas e reutilizan métricas compartidas de telemetria (`handle_transaction_with_metrics`).

### Ciclo de vida de chaves de verificação (`/v1/zk/vk/*`) - Cubierto
- Manipuladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Proprietário: ZK Working Group com suporte para a plataforma Torii.
- Notas: Os DTOs são alinhados com os esquemas Norito referenciados pelo SDK; A limitação de taxa é imposta via `limits.rs`.

### Nexus Connect (`/v1/connect/*`) - Cubierto (recurso `connect`)
- Manipuladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Ligação do roteador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Testes: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, ciclo de vida de sessão, handshake WS) e
  cobertura de matriz de recursos do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Proprietário: Nexus Connect WG.
- Notas: As chaves de limite de taxa são rastreadas via `limits::rate_limit_key`; os contadores de telemetria alimentam as métricas `connect.*`.### Telemetria de relé Kaigi - Cubierto
- Manipuladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
Ligação do roteador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testes: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: El stream SSE reutiliza o canal global de transmissão enquanto aplica o gate do perfil de telemetria; Os esquemas de resposta estão documentados em `docs/source/torii/kaigi_telemetry_api.md`.

## Resumo da cobertura de testes

- As verificações de fumaça do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantem que as combinações de recursos sejam registradas em cada rota e que a geração do OpenAPI seja mantida em sincronização.
- As suítes específicas de endpoints contêm consultas de contas, ciclo de vida de contratos, chaves de verificação ZK, filtros de prova SSE e comportamentos de Nexus Connect.
- Os recursos de paridade SDK (JavaScript, Swift, Python) que consomem Alias ​​VOPRF e endpoints SSE; não é necessário trabalho adicional.

## Manter este espejo atualizado

Atualiza esta página e a fonte de auditoria (`docs/source/torii/app_api_parity_audit.md`) quando o comportamento da API do aplicativo de Torii muda para que os proprietários do SDK e os leitores externos sigam alinhados.