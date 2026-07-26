---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ede2caf753a381139467f00048f8f12d41ae7e26ef6d0838af789f864a8362d
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-plan
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

SF-4 Pin Registry کنٹریکٹ اور معاون سروسز فراہم کرتا ہے جو manifest commitments محفوظ کرتی ہیں،
pin policies نافذ کرتی ہیں، اور Torii، gateways اور orchestrators کے لیے APIs ظاہر کرتی ہیں۔
یہ دستاویز validation plan کو ٹھوس implementation tasks سے بڑھاتی ہے، جس میں on-chain logic،
host-side services، fixtures، اور عملیاتی تقاضے شامل ہیں۔

## دائرہ کار

1. **registry state machine**: Norito-defined records برائے manifests، aliases، successor chains،
   retention epochs، اور governance metadata.
2. **کنٹریکٹ نفاذ**: pin lifecycle کے لیے deterministic CRUD operations (`ReplicationOrder`,
   `Precommit`, `Completion`, eviction).
3. **سروس facade**: gRPC/REST endpoints جو registry سے بیکڈ ہوں اور Torii و SDKs انہیں استعمال کریں،
   جن میں pagination اور attestation شامل ہے۔
4. **tooling اور fixtures**: CLI helpers، test vectors، اور documentation تاکہ manifests، aliases اور
   governance envelopes ہم آہنگ رہیں۔
5. **telemetry اور ops**: registry صحت کے لیے metrics، alerts، اور runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)

| Struct | وضاحت | فیلڈز |
|--------|-------|-------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | alias -> manifest CID mapping. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | providers کو manifest pin کرنے کی ہدایت. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | provider acknowledgement. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | governance policy snapshot. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Fixtures اور CI

- Fixtures directory: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` میں signed manifest/alias/order snapshots محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے regenerate ہوتے ہیں۔
- CI step: `ci/check_sorafs_fixtures.sh` snapshot regenerate کرتا ہے اور diff ہونے پر fail کرتا ہے تاکہ CI fixtures aligned رہیں۔
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) happy path کے ساتھ duplicate-alias rejection، alias approval/retention guards، mismatched chunker handles، replica-count validation، اور succession-guard failures (unknown/pre-approved/retired/self pointers) کور کرتے ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` cases دیکھیں۔
- Unit tests اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں alias validation، retention guards، اور successor checks کور کرتے ہیں؛ multi-hop succession detection تب آئے گا جب state machine دستیاب ہوگی۔
- Observability pipelines کے لیے golden JSON events۔

## Telemetry اور Observability

Metrics (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- موجودہ provider telemetry (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) end-to-end dashboards کے لیے scope میں رہے گی۔

Logs:
- governance audits کے لیے structured Norito event stream (signed?).

Alerts:
- SLA سے زیادہ pending replication orders.
- alias expiry threshold سے کم.
- retention violations (manifest renewal وقت سے پہلے نہ ہو).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` manifest lifecycle totals، alias coverage، backlog saturation، SLA ratio، latency vs slack overlays، اور missed-order rates کو on-call review کے لیے دکھاتا ہے۔

## Runbooks اور Documentation

- `docs/source/sorafs/migration_ledger.md` کو registry status updates شامل کرنے کے لیے اپڈیٹ کریں۔
- Operator guide: `docs/source/sorafs/runbooks/pin_registry_ops.md` (اب شائع شدہ) metrics، alerting، deployment، backup، اور recovery flows کور کرتا ہے۔
- Governance guide: policy parameters، approval workflow، dispute handling بیان کریں۔
- ہر endpoint کے لیے API reference pages (Docusaurus docs).

## Dependencies اور Sequencing

1. validation plan tasks مکمل کریں (ManifestValidator integration).
2. Norito schema + policy defaults کو حتمی بنائیں۔
3. contract + service نافذ کریں اور telemetry wire کریں۔
4. fixtures regenerate کریں اور integration suites چلائیں۔
5. docs/runbooks اپڈیٹ کریں اور roadmap items کو مکمل مارک کریں۔

SF-4 کے ہر checklist آئٹم میں پیش رفت پر اس منصوبے کا حوالہ ہونا چاہیے۔
REST facade اب attested listing endpoints کے ساتھ آتی ہے:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` فعال alias catalogue اور
  replication order backlog کو consistent pagination اور status filters کے ساتھ ظاہر کرتے ہیں۔

CLI ان calls کو wrap کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) تاکہ operators کم سطحی APIs کو چھوئے بغیر registry audits خودکار بنا سکیں۔
