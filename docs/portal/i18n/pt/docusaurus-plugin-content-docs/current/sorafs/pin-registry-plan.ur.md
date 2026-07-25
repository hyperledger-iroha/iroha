---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: SoraFS Pin Registry نفاذی منصوبہ
sidebar_label: Pin Registry منصوبہ
descrição: SF-4 نفاذی منصوبہ جو registro کی máquina de estado, fachada Torii, ferramentas e observabilidade کو کور کرتا ہے۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

Registro SF-4 Pin
políticas de pin نافذ کرتی ہیں, اور Torii, gateways e orquestradores کے لیے APIs ظاہر کرتی ہیں۔
یہ دستاویز plano de validação کو ٹھوس tarefas de implementação سے بڑھاتی ہے، جس میں lógica on-chain،
serviços do lado do host, luminárias, e outros serviços de hospedagem

## دائرہ کار

1. **máquina de estado de registro**: registros definidos por Norito برائے manifestos, aliases, cadeias de sucessores,
   épocas de retenção e metadados de governança.
2. **کنٹریکٹ نفاذ**: ciclo de vida do pino کے لیے operações CRUD determinísticas (`ReplicationOrder`,
   `Precommit`, `Completion`, despejo).
3. **fachada **: endpoints gRPC/REST e registro سے بیکڈ ہوں اور Torii e SDKs انہیں استعمال کریں
   جن میں paginação e atestado شامل ہے۔
4. **ferramentas e acessórios**: auxiliares CLI, vetores de teste, documentação, manifestos, aliases e
   envelopes de governança
5. **telemetria e operações**: registro de métricas, alertas e runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)

| Estrutura | وضاحت | فیلڈز |
|----|-------|-------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | alias -> mapeamento CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | provedores کو pin de manifesto کرنے کی ہدایت. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | provider acknowledgement. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | instantâneo da política de governação. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Calendário do CI- Diretório de luminárias: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` میں manifesto/alias/instantâneos de pedido assinados محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے regenerar ہوتے ہیں۔
- Etapa CI: `ci/check_sorafs_fixtures.sh` regeneração de snapshot کرتا ہے اور diff ہونے پر fail کرتا ہے تاکہ CI fixtures alinhados رہیں۔
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) caminho feliz کے ساتھ rejeição de alias duplicados, aprovação de alias/guardas de retenção, identificadores de chunker incompatíveis, validação de contagem de réplicas, اور falhas de proteção de sucessão (desconhecido/pré-aprovado/aposentado/auto ponteiros) کور کرتے ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` casos دیکھیں۔
- Testes de unidade اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں validação de alias, guardas de retenção, e verificações de sucessor کور کرتے ہیں؛ detecção de sucessão multi-hop تب آئے گا جب máquina de estado
- Pipelines de observabilidade para eventos JSON dourados۔

## Telemetria e Observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Telemetria do provedor موجودہ (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) painéis de ponta a ponta کے لیے escopo میں رہے گی۔

Registros:
- auditorias de governança کے لیے fluxo de eventos Norito estruturado (assinado?).

Alertas:
- SLA سے زیادہ ordens de replicação pendentes.
- limite de expiração do alias سے کم.
- violações de retenção (renovação manifesta وقت سے پہلے نہ ہو).

Painéis:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` totais do ciclo de vida do manifesto, cobertura de alias, saturação de pendências, proporção de SLA, latência vs sobreposições de folga, taxas de pedidos perdidos e revisão de plantão کے لیے دکھاتا ہے۔

## Runbooks e documentação

- `docs/source/sorafs/migration_ledger.md` کو atualizações de status do registro شامل کرنے کے لیے اپڈیٹ کریں۔
- Guia do operador: métricas `docs/source/sorafs/runbooks/pin_registry_ops.md` (referências), alertas, implantação, backup, fluxos de recuperação e fluxos de recuperação کور کرتا ہے۔
- Guia de governança: parâmetros de política, fluxo de trabalho de aprovação, tratamento de disputas بیان کریں۔
- ہر endpoint کے لیے páginas de referência da API (documentos Docusaurus).

## Dependências e Sequenciamento

1. tarefas do plano de validação مکمل کریں (integração do ManifestValidator).
2. Esquema Norito + padrões de política
3. contrato + serviço نافذ کریں اور fio de telemetria کریں۔
4. fixtures regeneram کریں اور suítes de integração چلائیں۔
5. docs/runbooks اپڈیٹ کریں اور itens de roteiro کو مکمل مارک کریں۔

Lista de verificação SF-4 کے ہر
Fachada REST اب endpoints de listagem atestados کے ساتھ آتی ہے:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` no catálogo de alias
  backlog de pedidos de replicação کو paginação consistente اور filtros de status کے ساتھ ظاہر کرتے ہیں۔

CLI não chama کو wrap کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) تاکہ operadores کم سطحی APIs کو چھوئے بغیر auditorias de registro خودکار بنا سکیں۔