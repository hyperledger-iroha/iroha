---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
כותרת: Plano de implementacao do Pin Registry do SoraFS
sidebar_label: Plano do Pin Registry
תיאור: Plano de implementacao SF-4 cobrindo a maquina de estados do registry, a fachada Torii, tooling e observabilidade.
---

:::שים לב Fonte canonica
Esta pagina reflete `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas enquanto a documentacao herdada permanecer ativa.
:::

# Plano de implementacao do Pin Registry do SoraFS (SF-4)

O SF-4 entrega o contrato do Pin Registry e os servicos de apoio que armazenam
פשרה של מניפסט, Impoem Politicas de Pin e Expoem APIs para Torii,
שערים e orquestradores. Este documento amplia o plano de validacao com
tarefas de implementacao concretas, cobrindo a logica on-chain, os servicos do
מארח, אביזרי OS ו-OS Requisitos Operacionais.

## אסקופו

1. **Maquina de estados do registry**: registros definidos por Norito para manifests,
   כינויים, קדנציות המשך, epocas de retencao e metadados de governanca.
2. **Implementacao do contrato**: operacoes CRUD deterministicas para o ciclo de vida
   פיני דוס (`ReplicationOrder`, `Precommit`, `Completion`, פינוי).
3. **Fachada de servico**: נקודות קצה gRPC/REST sustentados pelo registry que Torii
   e OS SDKs consomem, incluindo paginacao e atestacao.
4. **מכשירים אלקטרוניים**: עוזרים של CLI, וטריות ותעודות מסמכים למטרות
   מניפסטים, כינויים e envelopes de governanca sincronizados.
5. **Telemetria e Ops**: מדדים, התראות e runbooks para a saude do registry.

## Modelo de Dados

### רישום מרכזי (Norito)

| מבנה | תיאור | קמפוס |
|--------|--------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Mapeia alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucao para providers fixarem o manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | ספק Confirmacao do. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | תמונת מצב של פוליטיקה דה גוברננקה. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## גופי CI- מדריך מתקנים: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` ארמזנה צילומי מצב assinados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o תמונת מצב ו-falha se hover diffs, mantendo os fixtures de CI alinhados.
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) exercitam o fluxo feliz mais a rejeicao de alias duplicado, guards de aprovacao/retencao de alias, handles de chunker incompativeis, validacao de contagem de suurlicas de guard eas desconhecidos/preaprovados/retirados/autorreferencias); veja casos `register_manifest_rejects_*` לפרטי קוברטורה.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop quando a maquina de estados chegar.
- JSON golden para eventos usados ​​pelos pipelines de observabilidade.

## Telemetria e Observabilidade

מדדים (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- קיימת קיימת טלמטריה של ספקים (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) מתמשכת עם לוחות מחוונים מקצה לקצה.

יומנים:
- Stream de eventos Norito estruturados para auditorias de governanca (אסינאדוס?).

התראות:
- Ordens de replicacao pendentes excedendo o SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (מניפסט נאו renovado antes de expirar).

לוחות מחוונים:
- O JSON do Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totais do ciclo de vida dos manifests, cobertura de alias, saturacao do backlog, razao de SLA, overlays de latencia vs slack e taxas de ordenalls perdidas on- revis.

## ספרי הפעלה ומסמכים

- Atualizar `docs/source/sorafs/migration_ledger.md` כולל את הסטטוס של רישום.
- מפעיל פעולות: `docs/source/sorafs/runbooks/pin_registry_ops.md` (יא מפרסמים) מדדי קוברינדו, התראות, פריסה, גיבוי והחלמה.
- Guia de governanca: מסירת פרמטרים פוליטיים, זרימת עבודה של אפרובאקאו, טראטמנטו דה מחלוקת.
- Paginas de referencia de API para cada end point (docs Docusaurus).

## Dependencias e sequenciamento

1. Completar tarefas do plano de validacao (integracao do ManifestValidator).
2. Finalizar esquema Norito + ברירות מחדל של פוליטיקה.
3. קונטרטו מיושם + סרוויקו, קונקטר טלמטריה.
4. אביזרי Regenerar, Rodar Suites de integracao.
5. Atualizar docs/runbooks e marcar itens do מפת דרכים como completos.

רשימת בדיקה עבור SF-4 deve referenciar este plano quando howver progresso.
A Fachada REST agora entrega נקודות קצה de listgem com atestacao:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` תערוכת או קטלוג
  כינוי ativo e o backlog de ordens de replicacao com paginacao consistente e
  filtros de status.A CLI encapsula essas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que operadores possam automatizar auditorias do
Registry Sem tocar APIs de baixo nivel.