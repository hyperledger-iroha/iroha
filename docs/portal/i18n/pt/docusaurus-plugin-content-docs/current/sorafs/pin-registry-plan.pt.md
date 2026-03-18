---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Plano de implementação do Pin Registry do SoraFS
sidebar_label: Registro do Plano do Pin
descrição: Plano de implementação SF-4 cobrindo a maquina de estados do registro, a fachada Torii, ferramental e observabilidade.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as cópias sincronizadas enquanto a documentação herdada permanece ativa.
:::

# Plano de implementação do Pin Registry do SoraFS (SF-4)

O SF-4 entrega o contrato do Pin Registry e os serviços de apoio que armazenam
compromissos de manifesto, imposição de políticas de pin e exposição de APIs para Torii,
gateways e orquestradores. Este documento amplia o plano de validação com
tarefas de implementação concretas, cobrindo a lógica on-chain, os serviços do
host, os equipamentos e os requisitos operacionais.

## Escopo

1. **Máquina de estados do registro**: registros definidos por Norito para manifestos,
   aliases, cadeias sucessoras, épocas de retenção e metadados de governança.
2. **Implementação do contrato**: operações CRUD determinísticas para o ciclo de vida
   dos pinos (`ReplicationOrder`, `Precommit`, `Completion`, despejo).
3. **Fachada de serviço**: endpoints gRPC/REST sustentados pelo registro que Torii
   e os SDKs consumidos, incluindo paginação e atestado.
4. **Ferramentas e acessórios**: ajudantes de CLI, vetores de teste e documentação para manter
   manifestos, aliases e envelopes de governança sincronizados.
5. **Telemetria e operações**: métricas, alertas e runbooks para a saúde do registro.

## Modelo de dados

### Registros Centrais (Norito)

| Estrutura | Descrição | Campos |
|--------|-----------|--------|
| `PinRecordV1` | Entrada canônica do manifesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​do Mapeia -> CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrução para provedores fixarem o manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Confirmação do provedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantâneo de política de governança. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referência de implementação: veja `crates/sorafs_manifest/src/pin_registry.rs` para os
esquemas Norito em Rust e os ajudantes de validação que sustentam esses registros. Um
validação espelha o tooling de manifest (lookup do chunker Registry, pin policy gating)
para que o contrato, as fachadas Torii e o CLI compartilhem invariantes idênticas.

Tarefas:
- Finalizar os esquemas Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- Gerar código (Rust + outros SDKs) usando macros Norito.
- Atualizar a documentação (`sorafs_architecture_rfc.md`) quando os esquemas estiverem prontos.## Implementação do contrato

| Tarefa | Responsavel(é) | Notas |
|--------|-----------------|-------|
| Implementar armazenamento de registro (sled/sqlite/off-chain) ou módulo de contrato inteligente. | Equipe Core de Infra/Contrato Inteligente | Fornecer hash determinístico, evitar ponto flutuante. |
| Pontos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infra principal | Aproveitar `ManifestValidator` do plano de validação. O bind de alias agora flui via `RegisterPinManifest` (DTO do Torii) enquanto `bind_alias` dedicado segue planejado para atualizações sucessivas. |
| Transições de estado: importação sucessão (manifesto A -> B), épocas de retenção, unicidade de alias. | Conselho de Governança / Core Infra | Unicidade de alias, limites de retenção e verificações de aprovação/retirada de antecessores que vivem em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a detecção de sucesso multi-hop e a escrituração de replicação permanecem em aberto. |
| Parâmetros governados: carregar `ManifestPolicyV1` de config/estado de governança; permitir atualizações via eventos de governança. | Conselho de Governança | fornecer CLI para atualizações políticas. |
| Emissão de eventos: emitir eventos Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidade | Definir esquema de eventos + registro. |

Testes:
- Testes unitários para cada ponto de entrada (positivo + rejeição).
- Testes de propriedades para a cadeia de sucessão (sem ciclos, épocas monotonicamente crescentes).
- Fuzz de validação gerando manifestos aleatórios (limitados).

## Fachada de serviço (Integração Torii/SDK)

| Componente | Tarefa | Responsavel(é) |
|------------|--------|-----------------|
| Serviço Torii | Expor `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (pesquisa), `/v1/sorafs/aliases` (listar/vincular), `/v1/sorafs/replication` (pedidos/recebimentos). fornecer paginação + filtragem. | Rede TL / Core Infra |
| Atestação | Incluir altura/hash do registro nas respostas; adicionar estrutura de atestacao Norito consumida pelos SDKs. | Infra principal |
| CLI | Estender `sorafs_manifest_stub` ou um novo CLI `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Ferramentaria |
| SDK | Gerar ligações de cliente (Rust/Go/TS) a partir do esquema Norito; adicionar testes de integração. | Equipes SDK |

Operações:
- Adicione camada de cache/ETag para endpoints GET.
- fornecer rate limiting / auth consistentes com as políticas do Torii.

## Luminárias e CI- Diretorio de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots aceitos de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot e falha se houver diffs, mantendo os fixtures de CI alinhados.
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) exercitam o fluxo feliz mais a rejeição de alias duplicado, guardas de aprovação/retenção de alias, handles de chunker incompativeis, validação de contagem de réplicas e falhas de guardas de sucesso (ponteiros desconhecidos/preaprovados/retirados/autorreferencias); veja casos `register_manifest_rejects_*` para detalhes de cobertura.
- Testes unitários agora cobrem validação de alias, guardas de retenção e verificações de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a detecção de sucesso multi-hop quando a maquina de estados chegar.
- JSON golden para eventos usados ​​pelos pipelines de observabilidade.

## Telemetria e observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- A telemetria existente de provedores (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece em escopo para dashboards end-to-end.

Registros:
- Stream de eventos Norito estruturados para auditorias de governança (assinados?).

Alertas:
- Ordens de replicação pendentes excedendo o SLA.
- Expiração de alias abaixo do limite.
- Violações de retenção (manifestadas não renovadas antes de expirar).

Painéis:
- O JSON do Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totais do ciclo de vida dos manifestos, cobertura de alias, saturação do backlog, razão de SLA, overlays de latência vs slack e taxas de pedidos perdidos para revisão on-call.

## Runbooks e documentação

- Atualizar `docs/source/sorafs/migration_ledger.md` para incluir atualizações de status do registro.
- Guia do operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publicado) cobrindo métricas, alertas, deploy, backup e fluxos de recuperação.
- Guia de governança: descrever parâmetros de política, fluxo de trabalho de aprovação, tratamento de disputas.
- Páginas de referência de API para cada endpoint (docs Docusaurus).

## Dependências e sequenciamento

1. Completar tarefas do plano de validação (integração do ManifestValidator).
2. Finalizar esquema Norito + padrões de política.
3. Implementar contrato + serviço, conectar telemetria.
4. Regenerar luminárias, rodar suítes de integração.
5. Atualizar docs/runbooks e marcar itens do roadmap como completos.

Cada checklist do SF-4 deve referenciar este plano quando houver progresso.
A fachada REST agora entrega endpoints de listagem com atestado:

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` retornam manifestos com
  binds de alias, ordens de replicação e um objeto de atestacao derivado do
  hash do último bloco.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` exposição ou catálogo de
  alias ativo e o backlog de ordens de replicação com paginação consistente e
  Filtros de status.A CLI encapsula essas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que os operadores possam automatizar auditorias do
registro sem tocar APIs de baixo nível.