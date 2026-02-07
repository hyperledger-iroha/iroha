---
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridade
título: Auditoria de paridade da API de app do Torii
description: Espelho da revisão TORII-APP-1 para que as equipes de SDK e plataforma confirmem a cobertura pública.
---

Situação: Concluído em 21/03/2026  
Responsável: Plataforma Torii, líder do programa SDK  
Referência do roadmap: TORII-APP-1 - auditorias de paridade `app_api`

Esta página reflete os auditórios internos `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) para que os leitores fora do mono-repo vejam quais superfícies `/v1/*` estão conectadas, testadas e documentadas. A auditoria acompanha as rotas reexportadas via `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` e `add_connect_routes`.

## Escopo e método

As auditorias funcionam como reexportações públicas em `crates/iroha_torii/src/lib.rs:256-522` e os construtores de rotas com feature gating. Para cada superfície `/v1/*` do roadmap verificamos:

- Implementação do handler e definições de DTO em `crates/iroha_torii/src/routing.rs`.
- Registro do roteador nos grupos de recursos `app_api` ou `connect`.
- Testes de integração/unitários existentes e uma equipe responsável pela cobertura de longo prazo.

As listas de ativos/transações da conta e de titulares de ativos aceitam sessões de consulta `asset_id` adicionais para pré-filtragem, além dos limites existentes de paginação/contrapressão.

## Autenticação e assinatura canônica

- Endpoints GET/POST secundários a apps aceitam cabeçalhos de requisição canônica (`X-Iroha-Account`, `X-Iroha-Signature`) construídos de `METHOD\n/path\nsorted_query\nsha256(body)`; o Torii envolve em `QueryRequestWithAuthority` antes da validação do executor para espelhar `/query`.
- Helpers de SDK existem em todos os clientes principais:
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

### Permissões de conta (`/v1/accounts/{id}/permissions`) - Coberto
- Manipulador: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: `crates/iroha_torii/tests/accounts_endpoints.rs:126` e `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Proprietário: Plataforma Torii.
- Notas: A resposta e um corpo JSON Norito com `items`/`total`, alinhado aos helpers de paginação dos SDKs.

### Avaliação OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - Coberto
- Manipulador: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Ligação do roteador: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Testes: testes inline do handler (`crates/iroha_torii/src/lib.rs:9945-9986`) mais cobertura de SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Proprietário: Plataforma Torii.
- Notas: A superfície de resposta reforca hex determinística e identificadores de backend; os SDKs consomem o DTO.### Eventos de prova SSE (`GET /v1/events/sse`) - Coberto
- Handler: `handle_v1_events_sse` com suporte a filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) mais o cabeamento de filtro de prova.
- Ligação do roteador: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Testes: suítes SSE especificações de prova (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) e teste de fumaça SSE do pipeline
  (`integration_tests/tests/events/sse_smoke.rs`).
- Proprietário: Plataforma Torii (runtime), GT de Testes de Integração (fixtures).
- Notas: Os caminhos de filtro de prova foram validados ponta a ponta; a documentação fica em `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - Coberto
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
- Notas: Os endpoints enfileiram transações assinadas e reutilizaram métricas de telemetria compartilhadas (`handle_transaction_with_metrics`).

### Ciclo de vida de chaves de verificação (`/v1/zk/vk/*`) - Coberto
- Manipuladores: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Ligação do roteador: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Testes: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Proprietário: ZK Working Group com suporte da Plataforma Torii.
- Notas: Os DTOs se alinham aos esquemas Norito referenciados pelos SDKs; limitação de taxa e aplicada via `limits.rs`.

### Nexus Conectar (`/v1/connect/*`) - Coberto (recurso `connect`)
- Manipuladores: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Ligação do roteador: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Testes: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, ciclo de vida de sessão, handshake WS) e
  cobertura da matriz de recursos do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Proprietário: Nexus Connect WG.
- Notas: Chaves de taxa limite são rastreadas via `limits::rate_limit_key`; contadores de telemetria alimentados como métricas `connect.*`.

### Telemetria de relé Kaigi - Coberto
- Manipuladores: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
Ligação do roteador: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Testes: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notas: O stream SSE reutiliza o canal global de transmissão enquanto aplica o gate do perfil de telemetria; os esquemas de resposta estão documentados em `docs/source/torii/kaigi_telemetry_api.md`.## Resumo de cobertura de testículos

- Testes smoke do roteador (`crates/iroha_torii/tests/router_feature_matrix.rs`) garantem que combinações de recursos registrem todas as rotas e que a geração de OpenAPI fique sincronizada.
- Suites específicas de endpoints cobrem consultas de contas, ciclo de vida de contratos, chaves de verificação ZK, filtros de prova SSE e comportamentos do Nexus Connect.
- Chicotes de paridade de SDK (JavaScript, Swift, Python) e consomem Alias ​​VOPRF e endpoints SSE; não há trabalho adicional.

## Manter este espelho atualizado

Atualize esta página e a fonte de auditoria (`docs/source/torii/app_api_parity_audit.md`) quando o comportamento da app API de Torii mudar para que os proprietários de SDK e leitores externos fiquem alinhados.