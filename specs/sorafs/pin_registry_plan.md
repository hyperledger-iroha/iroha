# SoraFS Pin Registry Implementation Plan (SF-4)

SF-4 delivers the Pin Registry contract and supporting services that store
manifest commitments, enforce pin policies, and expose APIs to Torii, gateways,
and orchestrators. This document expands the validation plan with concrete
implementation tasks, covering on-chain logic, host-side services, fixtures,
and operational requirements.

## Scope

1. **Registry state machine**: Norito-defined records for manifests, aliases,
   successor chains, retention epochs, and governance metadata.
2. **Contract implementation**: deterministic CRUD operations for pin lifecycle
   (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Service facade**: gRPC/REST endpoints backed by the registry that Torii
   and SDKs consume, including pagination and attestation.
4. **Tooling & fixtures**: CLI helpers, test vectors, and documentation to keep
   manifests, aliases, and governance envelopes in sync.
5. **Telemetry & ops**: metrics, alerts, and runbooks for registry health.

## Data Model

### Core Records (Norito)

| Struct | Description | Fields |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor`, `manifest`. |
| `AliasBindingV1` | Maps alias -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Canonical governance order payload for providers to pin one manifest. | `version`, `order_id`, `manifest_cid`, `manifest_digest`, `chunking_profile`, `target_replicas`, `assignments`, `issued_at`, `deadline_at`, `sla`, `metadata`. |
| `ReplicationOrderRecord` | Chain-authoritative order state. Assignment revision starts at one and changes only through exact compare-and-set reassignment. | `order_id`, `manifest_digest`, `manifest_root_cid`, `issued_by`, `issued_epoch`, `deadline_epoch`, `canonical_order`, `assignment_revision`, `provider_completions`, `status`. |
| `ProviderIngestCompletionSignerPolicyV1` | Public identity of the governed provider-ingest signer policy. | `policy_id`, `revision`, `predecessor_digest`, `policy_digest`. |
| `ProviderIngestCompletionAuthorityV1` | Current chain-authoritative completion authority for one provider. | `provider_owner`, `signer_policy`. |
| `ProviderIngestFinalizedAnchorV1` | Exact committed-chain prefix used to prepare a completion. | `height`, `block_hash`. |
| `ReplicationOrderCompletionRecord` | Retained provider-scoped completion context accepted by ledger execution. | `provider_id`, `completed_by`, `completion_epoch`, `assignment_revision`, `completion_authority`, `finalized_anchor`. |
| `ManifestPolicyV1` | Governance policy snapshot. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in
`crates/iroha_data_model/src/sorafs/pin_registry.rs`. Supporting alias,
replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the
  first-release manifest registry surface used by core, Torii, fixtures, and
  reference validators. The obsolete pre-release `PinRecordV1` format is
  removed.
- Rust code generation is handled through Norito derives; SDK parity now follows
  the normal SDK guard lanes whenever the schema changes.
- Architecture, migration, manifest-pipeline, CLI, OpenAPI, status, and roadmap
  docs already describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `ReviseReplicationOrderAssignments`, `SetProviderIngestCompletionAuthority`, `RevokeProviderIngestCompletionAuthority`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Issuance and reassignment require every assigned provider to have a registered owner and a valid, owner-matched completion authority. `CompleteReplicationOrder` is the V1 six-field hard cut: order, provider, completion epoch, expected owner/policy authority, expected assignment revision, and finalized height/hash anchor. Relayers are not trusted completion authorities. Core execution revalidates all expected context atomically in the transaction that records completion; there is no three-field compatibility form. Exact retained replay is idempotent, and the order becomes terminal only after its canonical redundancy target is reached. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, signer-policy succession, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage. Provider signer policies start at revision one; a same-identity successor advances exactly one revision and commits the prior policy digest, while a replacement identity restarts at revision one. Assignment replacement is an exact monotonic compare-and-set on a pending order and is forbidden after the first completion. Replication records retain ordered provider-scoped completion evidence including the accepted assignment revision, owner/policy tuple, and finalized anchor. Partial completion stays pending, late completion is rejected, and only an incomplete order may expire after its inclusive deadline. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue, provider-owner completion, partial and target completion,
  conflicting/surplus replay, owner and signer-policy rotation, assignment
  revision and reassignment, finalized-anchor substitution, deadline expiration,
  permissions, duplicate rejection, exact retained replay, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and then runs the parity checks that keep the canonical
  schema surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. Each replication-order projection includes `assignment_revision`; each retained completion includes the accepted revision, nested owner/signer-policy identity, and nested finalized height/hash anchor. | Networking TL / Core Infra |
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
- `GET /v1/sorafs/replication` accepts bounded `limit`/`offset` pagination plus
  `status` and `manifest_digest` filters. Each order emits
  `assignment_revision`. Every `provider_completions[]` entry emits
  `assignment_revision`,
  `completion_authority.provider_owner`,
  `completion_authority.signer_policy.{policy_id_hex,revision,predecessor_digest_hex,policy_digest_hex}`,
  and `finalized_anchor.{height,block_hash_hex}`. These are retained ledger
  facts, not live substitutions from the provider registry.
  Selectors are a strict hard cut: `limit` is `1..=500`, `offset` is a
  canonical `u32`, `status` is exactly lowercase `pending`, `completed`, or
  `expired`, and `manifest_digest` is a non-zero lowercase 32-byte digest;
  unknown, duplicate, empty, and alias parameters are rejected. The listing
  attestation and world data are derived from one full `StateView` generation.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Fixtures & CI

- Fixtures directory: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stores signed manifest/alias/order snapshots regenerated by `cargo run -p iroha_core --example gen_pin_snapshot`.
- CI step: `ci/check_sorafs_fixtures.sh` regenerates the snapshot and fails if diffs appear, keeping CI fixtures aligned.
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) exercise the happy path plus duplicate-alias rejection, alias approval/retention guards, mismatched chunker handles, replica-count validation, governance-policy ceilings/retention/storage allowlists, and successor-guard failures (unknown/pre-approved/retired/self pointers); see `register_manifest_rejects_*` cases for coverage details.
- Unit tests cover alias validation, retention guards, replication order issue/complete/expire, and multi-hop successor-chain cycle rejection in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
- Replication orders bind both distinct commitments: `manifest_cid` must equal
  the registered content root and `manifest_digest` must equal the BLAKE3 digest
  of the canonical manifest envelope. They bound payload/assignment/metadata sizes, require sorted distinct providers and
  positive SLA targets, reject alternate Norito layouts, and only complete
  within their ledger deadline. `CanCompleteSorafsReplicationOrder` is necessary
  but not sufficient: the transaction authority must also equal the provider's
  current registered owner and the completion's exact expected owner, and the
  current signer-policy tuple, assignment revision, and committed-chain anchor
  must still match. One account need not own every provider in a multi-provider
  order. Exact retained completion replays are idempotent even after a later
  authority rotation; stale prepared completions and conflicting replays fail,
  and retiring a manifest expires its pending orders.
- `ExpireReplicationOrder` closes a still-pending order only when its supplied
  epoch is strictly later than the inclusive completion deadline. It requires
  `CanIssueSorafsReplicationOrder`, accepts only an exact idempotent replay, and
  rejects early, conflicting, already-completed, or corrupt stored records
  without mutation.
- Golden JSON for events used by observability pipelines.

## Telemetry & Observability

Metrics (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Existing provider telemetry (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) remains in scope for end-to-end dashboards.

Logs:
- Attested REST snapshots and registry metrics are the local audit surface; signed
  governance archives can consume those snapshots through the governance DAG when
  an operator rollout requires durable external evidence.

Alerts:
- Pending replication orders exceeding SLA.
- Alias expiry < threshold.
- Retention violations (manifest not renewed before expiry).

Dashboards:
- Grafana JSON `specs/grafana_sorafs_pin_registry.json` tracks manifest lifecycle totals, alias coverage, backlog saturation, SLA ratio, latency vs slack overlays, and missed-order rates for on-call review.

## Runbooks & Documentation

- `specs/sorafs/migration_ledger.md`, `specs/sorafs/migration_roadmap.md`, and `roadmap.md` carry registry status updates.
- Operator guide: `specs/sorafs/runbooks/pin_registry_ops.md` covers metrics, alerting, deployment, backup, and recovery flows.
- Governance and dispute flows are documented through the admission policy, alias policy, capacity marketplace, and dispute/revocation runbooks.
- Endpoint behavior is covered by the SoraFS CLI, node-client protocol, and OpenAPI surfaces.

## Dependencies & Sequencing

1. Endpoint/client submission polish, shared validation, governance config mapping,
   Torii `manifest_payload` validation, and canonical manifest-derived policy checks are wired.
2. Norito schema, policy defaults, contract state, service facade, telemetry,
   fixtures, and local integration coverage are implemented.
3. Ongoing SF-4 work is rollout evidence: live registry audits, governance archive
   handoff, and operator-specific policy-change transcripts.

Each roadmap checklist item under SF-4 should reference this plan when progress is made.
The REST façade now ships with attested list endpoints and finalized native
manifest readback:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact
  `PinManifestFinalizedRecordV1` JSON with the finalized cursor and native
  manifest record.
- `GET /v1/sorafs/aliases` and `GET /v1/sorafs/replication` expose the active
  alias catalogue and replication order backlog with consistent pagination and
  status filters.

The CLI wraps these calls (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) so operators can script registry audits without touching
lower-level APIs.
