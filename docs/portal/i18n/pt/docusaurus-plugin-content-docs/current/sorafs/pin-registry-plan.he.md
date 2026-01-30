---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 80e02147ea520c5eec9fb4d25e64f9c1deb6efeb710275ba02c660ba1e3ced30
source_last_modified: "2025-11-14T04:43:22.047605+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canonica
Esta pagina reflete `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas enquanto a documentacao herdada permanecer ativa.
:::

# Plano de implementacao do Pin Registry do SoraFS (SF-4)

O SF-4 entrega o contrato do Pin Registry e os servicos de apoio que armazenam
compromissos de manifest, impoem politicas de pin e expoem APIs para Torii,
gateways e orquestradores. Este documento amplia o plano de validacao com
tarefas de implementacao concretas, cobrindo a logica on-chain, os servicos do
host, os fixtures e os requisitos operacionais.

## Escopo

1. **Maquina de estados do registry**: registros definidos por Norito para manifests,
   aliases, cadeias sucessoras, epocas de retencao e metadados de governanca.
2. **Implementacao do contrato**: operacoes CRUD deterministicas para o ciclo de vida
   dos pins (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Fachada de servico**: endpoints gRPC/REST sustentados pelo registry que Torii
   e os SDKs consomem, incluindo paginacao e atestacao.
4. **Tooling e fixtures**: helpers de CLI, vetores de teste e documentacao para manter
   manifests, aliases e envelopes de governanca sincronizados.
5. **Telemetria e ops**: metricas, alertas e runbooks para a saude do registry.

## Modelo de dados

### Registros centrais (Norito)

| Struct | Descricao | Campos |
|--------|-----------|--------|
| `PinRecordV1` | Entrada canonica de manifest. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Mapeia alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucao para providers fixarem o manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Confirmacao do provider. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Snapshot de politica de governanca. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementacao: veja `crates/sorafs_manifest/src/pin_registry.rs` para os
esquemas Norito em Rust e os helpers de validacao que sustentam esses registros. A
validacao espelha o tooling de manifest (lookup do chunker registry, pin policy gating)
para que o contrato, as fachadas Torii e a CLI compartilhem invariantes identicas.

Tarefas:
- Finalizar os esquemas Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- Gerar codigo (Rust + outros SDKs) usando macros Norito.
- Atualizar a documentacao (`sorafs_architecture_rfc.md`) quando os esquemas estiverem prontos.

## Implementacao do contrato

| Tarefa | Responsavel(is) | Notas |
|--------|-----------------|-------|
| Implementar armazenamento do registry (sled/sqlite/off-chain) ou modulo de smart contract. | Core Infra / Smart Contract Team | Fornecer hashing deterministico, evitar ponto flutuante. |
| Entry points: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | Aproveitar `ManifestValidator` do plano de validacao. O binding de alias agora flui via `RegisterPinManifest` (DTO do Torii) enquanto `bind_alias` dedicado segue planejado para atualizacoes sucessivas. |
| Transicoes de estado: impor sucessao (manifest A -> B), epocas de retencao, unicidade de alias. | Governance Council / Core Infra | Unicidade de alias, limites de retencao e checks de aprovacao/retirada de predecessores vivem em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop e o bookkeeping de replicacao permanecem em aberto. |
| Parametros governados: carregar `ManifestPolicyV1` de config/estado de governanca; permitir atualizacoes via eventos de governanca. | Governance Council | Fornecer CLI para atualizacoes de politica. |
| Emissao de eventos: emitir eventos Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | Definir esquema de eventos + logging. |

Testes:
- Testes unitarios para cada entry point (positivo + rejeicao).
- Testes de propriedades para a cadeia de sucessao (sem ciclos, epocas monotonicamente crescentes).
- Fuzz de validacao gerando manifests aleatorios (limitados).

## Fachada de servico (Integracao Torii/SDK)

| Componente | Tarefa | Responsavel(is) |
|------------|--------|-----------------|
| Servico Torii | Expor `/v1/sorafs/pin` (submit), `/v1/sorafs/pin/{cid}` (lookup), `/v1/sorafs/aliases` (list/bind), `/v1/sorafs/replication` (orders/receipts). Fornecer paginacao + filtragem. | Networking TL / Core Infra |
| Atestacao | Incluir altura/hash do registry nas respostas; adicionar estrutura de atestacao Norito consumida pelos SDKs. | Core Infra |
| CLI | Estender `sorafs_manifest_stub` ou um novo CLI `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Gerar bindings de cliente (Rust/Go/TS) a partir do esquema Norito; adicionar testes de integracao. | SDK Teams |

Operacoes:
- Adicionar camada de cache/ETag para endpoints GET.
- Fornecer rate limiting / auth consistentes com as politicas do Torii.

## Fixtures e CI

- Diretorio de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assinados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot e falha se houver diffs, mantendo os fixtures de CI alinhados.
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) exercitam o fluxo feliz mais a rejeicao de alias duplicado, guards de aprovacao/retencao de alias, handles de chunker incompativeis, validacao de contagem de replicas e falhas de guardas de sucessao (ponteiros desconhecidos/preaprovados/retirados/autorreferencias); veja casos `register_manifest_rejects_*` para detalhes de cobertura.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop quando a maquina de estados chegar.
- JSON golden para eventos usados pelos pipelines de observabilidade.

## Telemetria e observabilidade

Metricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- A telemetria existente de providers (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece em escopo para dashboards end-to-end.

Logs:
- Stream de eventos Norito estruturados para auditorias de governanca (assinados?).

Alertas:
- Ordens de replicacao pendentes excedendo o SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (manifest nao renovado antes de expirar).

Dashboards:
- O JSON do Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totais do ciclo de vida dos manifests, cobertura de alias, saturacao do backlog, razao de SLA, overlays de latencia vs slack e taxas de ordens perdidas para revisao on-call.

## Runbooks e documentacao

- Atualizar `docs/source/sorafs/migration_ledger.md` para incluir atualizacoes de status do registry.
- Guia do operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publicado) cobrindo metricas, alertas, deploy, backup e fluxos de recuperacao.
- Guia de governanca: descrever parametros de politica, workflow de aprovacao, tratamento de disputas.
- Paginas de referencia de API para cada endpoint (docs Docusaurus).

## Dependencias e sequenciamento

1. Completar tarefas do plano de validacao (integracao do ManifestValidator).
2. Finalizar esquema Norito + defaults de politica.
3. Implementar contrato + servico, conectar telemetria.
4. Regenerar fixtures, rodar suites de integracao.
5. Atualizar docs/runbooks e marcar itens do roadmap como completos.

Cada checklist do SF-4 deve referenciar este plano quando houver progresso.
A fachada REST agora entrega endpoints de listagem com atestacao:

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` retornam manifests com
  bindings de alias, ordens de replicacao e um objeto de atestacao derivado do
  hash do ultimo bloco.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` expoem o catalogo de
  alias ativo e o backlog de ordens de replicacao com paginacao consistente e
  filtros de status.

A CLI encapsula essas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que operadores possam automatizar auditorias do
registry sem tocar APIs de baixo nivel.
