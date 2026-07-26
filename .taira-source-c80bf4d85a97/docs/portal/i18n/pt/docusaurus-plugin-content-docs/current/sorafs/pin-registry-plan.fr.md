---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Plano de implementação do Pin Registry de SoraFS
sidebar_label: Registro Plan du Pin
descrição: Plano de implementação SF-4 cobrindo a máquina de registro, a fachada Torii, as ferramentas e a observabilidade.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/pin_registry_plan.md`. Garanta as duas cópias sincronizadas para que a documentação herdada esteja ativa.
:::

# Plano de implementação do Pin Registry de SoraFS (SF-4)

SF-4 libera o contrato Pin Registry e os serviços de aplicativo que você armazena
compromissos de manifesto, aplicação de políticas de fixação e exposição de
API para Torii, gateways auxiliares e orquestradores auxiliares. Este documento estende o plano de
validação com técnicas de implementação concretas, cobrindo a lógica
on-chain, serviços de hospedagem, instalações e exigências operacionais.

## Portée

1. **Máquina de status de registro**: registros Norito para manifestos, aliases,
   cadeias de sucessão, épocas de retenção e metas de governo.
2. **Implementação do contrato**: operações CRUD determinadas para o ciclo de vida
   des pinos (`ReplicationOrder`, `Precommit`, `Completion`, despejo).
3. **Façade de serviço**: endpoints gRPC/REST adicionados ao registro consumido por Torii
   e os SDKs, com paginação e atestado.
4. **Ferramentas e acessórios**: CLI auxiliar, vetores de teste e documentação para
   Garder manifestos, pseudônimos e envelopes de governo sincronizados.
5. **Telemetria e operações**: métricas, alertas e runbooks para a saúde do registro.

## Modèle de données

### Registros principais (Norito)

| Estrutura | Descrição | Campeões |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​do mapa -> CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrução para que os provedores pintem o manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acusado de recepção do provedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantâneo da política de governo. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Luminárias e CI- Dossiê de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assinados por manifesto/alias/pedido registrados via `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa CI: `ci/check_sorafs_fixtures.sh` gera o instantâneo e o eco em caso de diferença, mantendo os equipamentos CI alinhados.
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) cobrem o caminho feliz mais a rejeição de alias duplicados, os guardas de aprovação/retenção, os identificadores de blocos não concordantes, a validação da conta de réplicas e as verificações de garde de sucessão (ponteiros inconnus/pré-approuvés/retirés/auto-référencés); veja o caso `register_manifest_rejects_*` para os detalhes da cobertura.
- Os testes unitários mantêm a validação do alias, as proteções de retenção e as verificações do sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; a detecção de sucessão multi-hop atende à máquina de estados.
- JSON dourado para eventos utilizados pelos pipelines de observabilidade.

## Telemetria e observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- O provedor de telefonia existente (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece no escopo dos painéis de ponta a ponta.

Registros:
- Fluxos de eventos Norito estruturados para auditorias de governo (assinaturas?).

Alertas:
- Ordens de replicação atentas ao SLA.
- Expiração do alias < seuil.
- Violações de retenção (não renovações manifestadas antes do vencimento).

Painéis:
- Le JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` atende a todos os ciclos de vida dos manifestos, à cobertura de alias, à saturação do backlog, à relação SLA, às sobreposições de latência vs folga e às taxas de ordem perdidas para a revista de plantão.

## Runbooks e documentação

- Mettre a jour `docs/source/sorafs/migration_ledger.md` para incluir as mises no dia do status do registro.
- Guia do operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) cobrindo métricas, alertando, implementação, segurança e fluxo de reprise.
- Guia de governança: descreve os parâmetros políticos, o fluxo de trabalho de aprovação e o gerenciamento de litígios.
- API de páginas de referência para cada endpoint (docs Docusaurus).

## Dependências e sequência

1. Termine as etapas do plano de validação (integração ManifestValidator).
2. Finaliser le schéma Norito + defaults de politique.
3. Implemente o contrato + serviço, ramalize a telefonia.
4. Regenere os equipamentos e execute as suítes de integração.
5. Insira novos documentos/runbooks e marque os itens do roteiro como completos.

Cada item da lista de verificação SF-4 deve consultar este plano quando o progresso for registrado.
A fachada REST livre desordenada dos endpoints da listagem atesta:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` expõem o catálogo
  o alias ativo e o backlog das ordens de replicação com uma paginação coerente
  e filtros de status.O encapsulamento CLI desses aplicativos (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para permitir que os operadores automatizem as auditorias
registro sem toucher aux APIs no nível básico.