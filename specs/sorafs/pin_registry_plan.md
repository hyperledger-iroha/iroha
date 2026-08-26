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
3. **Service facade**: REST/query endpoints backed by the registry that Torii
   and SDKs consume, including finalized exclusive-keyset pages and bounded
   exact-record readback.
4. **Tooling & fixtures**: CLI helpers, test vectors, and documentation to keep
   manifests, aliases, and governance envelopes in sync.
5. **Telemetry & ops**: metrics, alerts, and runbooks for registry health.

## Data Model

### Core Records (Norito)

| Struct | Description | Fields |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. `submitted_epoch` and the approval/retirement epochs carried by `status` are evidence derived from block consensus time; they are never supplied by the client. The immutable nullable `approved_epoch` preserves approval history after retirement. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `approved_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor`, `manifest`. |
| `PinManifestSummaryV1` | Bounded list projection that deliberately omits alias proofs, metadata, council envelopes, and fee-payment details while retaining the required nullable approval epoch. | `digest`, `submitted_by`, `submitted_epoch`, `approved_epoch`, `content_length`, `retention_epoch`, `status`, `successor_of`. |
| `PinManifestPageV1` | Finalized exclusive-keyset page with both row and encoded-byte ceilings. | `finalized_cursor`, `charged_usage`, `manifests`, `has_more`, `next_after_digest`. |
| `PinResourceUsage` | Consensus-maintained resource charge for the global registry or one authenticated account. `manifest_count` covers every retained lifecycle record; `content_bytes` covers live replicated content. | `manifest_count`, `content_bytes`. |
| `PinLineageSummaryV1` | Consensus-maintained bounded succession state used without traversing complete manifest history. | `depth`, `direct_successor_count`. |
| `AliasBindingV1` | Maps alias -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Canonical governance order payload for providers to pin one manifest. | `version`, `order_id`, `manifest_cid`, `manifest_digest`, `chunking_profile`, `target_replicas`, `assignments`, `issued_at`, `deadline_at`, `sla`, `metadata`. |
| `ReplicationOrderRecord` | Chain-authoritative order state. Assignment revision starts at one and changes only through exact compare-and-set reassignment. | `order_id`, `manifest_digest`, `manifest_root_cid`, `issued_by`, `issued_epoch`, `deadline_epoch`, `canonical_order`, `assignment_revision`, `provider_completions`, `status`. |
| `ProviderIngestCompletionSignerPolicyV1` | Public identity of the governed provider-ingest signer policy. | `policy_id`, `revision`, `predecessor_digest`, `policy_digest`. |
| `ProviderIngestCompletionAuthorityV1` | Current chain-authoritative completion authority for one provider. | `provider_owner`, `signer_policy`. |
| `ProviderIngestFinalizedAnchorV1` | Exact committed-chain prefix used to prepare a completion. | `height`, `block_hash`. |
| `ReplicationOrderCompletionRecord` | Retained provider-scoped completion context accepted by ledger execution. | `provider_id`, `completed_by`, `completion_epoch`, `assignment_revision`, `completion_authority`, `finalized_anchor`. |
| `ManifestPolicyV1` | Governance policy snapshot. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle, accounting,
and finalized query schemas live in
`crates/iroha_data_model/src/sorafs/pin_registry.rs`. Supporting alias,
replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord`, `PinManifestFinalizedRecordV1`, and
  `PinManifestPageV1` are the first-release manifest registry surface used by
  Core, Torii, fixtures, and reference validators. The obsolete pre-release
  `PinRecordV1` format is removed.
- `RegisterPinManifest`, `ApprovePinManifest`, and `RetirePinManifest` carry no
  caller-selected lifecycle epoch. Core derives the event epoch from the
  executing block timestamp; it also retires due live pins from the
  authenticated expiry index at consensus time. The manifest's
  `retention_epoch` remains the prepaid policy deadline and must be later than
  the derived submission epoch.
- Rust code generation is handled through Norito derives; SDK parity now follows
  the normal SDK guard lanes whenever the schema changes.
- Architecture, migration, manifest-pipeline, CLI, OpenAPI, status, and roadmap
  docs already describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in authenticated Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`, global/per-authority count-and-byte usage, lineage summaries, expiry keys, and lifecycle-status indexes) with deterministic Norito payload hashing, checked arithmetic, and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `ReviseReplicationOrderAssignments`, `SetProviderIngestCompletionAuthority`, `RevokeProviderIngestCompletionAuthority`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Pin registration is a public paid operation for any authenticated transaction authority: no general pin permission token is consulted. Core validates the complete canonical manifest, enforces global/per-authority count-and-byte ceilings plus lineage depth/fanout, charges the submitter, collects a prepaid fee that scales with rounded content bytes, replica count, and retention duration, and derives the submission epoch. Alias attachment separately requires `CanBindSorafsAlias`. Approval derives its epoch and requires the bounded threshold council envelope when governance is enabled; any authenticated account may relay that envelope. Retirement derives its epoch and is restricted to the authenticated submitter. The three lifecycle instructions have no legacy client-epoch fields. Issuance and reassignment require every assigned provider to have a registered owner and a valid, owner-matched completion authority. `CompleteReplicationOrder` is the V1 six-field hard cut: order, provider, completion epoch, expected owner/policy authority, expected assignment revision, and finalized height/hash anchor. Relayers are not trusted completion authorities. Core execution revalidates all expected context atomically in the transaction that records completion; there is no three-field compatibility form. Exact retained replay is idempotent, and the order becomes terminal only after its canonical redundancy target is reached. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, resource charging, signer-policy succession, and replication status changes. | Governance Council / Core Infra | Admission stages quota, usage, lineage, expiry, and status-index writes transactionally. Automatic expiry walks the ordered authenticated expiry index at block consensus time and atomically retires each due pin, releases only its global/per-authority live-content byte charge, and updates the lifecycle index. The retained-record count and retained-successor fanout charge remain while the lifecycle evidence remains in consensus state, so register/retire cycles cannot grow state outside the configured ceilings. Corrupt index/accounting state rejects the complete effect. `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage. Provider signer policies start at revision one; a same-identity successor advances exactly one revision and commits the prior policy digest, while a replacement identity restarts at revision one. Assignment replacement is an exact monotonic compare-and-set on a pending order and is forbidden after the first completion. Replication records retain ordered provider-scoped completion evidence including the accepted assignment revision, owner/policy tuple, and finalized anchor. Partial completion stays pending, late completion is rejected, and only an incomplete order may expire after its inclusive deadline. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | The finalized pin page exposes the consensus-maintained `charged_usage` summary in O(1) state reads. Prometheus lifecycle/replication gauges remain sampled operational telemetry, not authoritative accounting or a substitute for finalized queries. Additional signed event archives can be layered over finalized query results if governance requires them. |

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
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The pin-list route executes `FindSorafsPinManifests` and returns `PinManifestPageV1`; the detail route returns exact native `PinManifestFinalizedRecordV1` JSON. Both accept only an optional paired expected finalized height/hash precondition. The alias/replication projections require canonical-account request signatures and return closed typed V1 response objects because their response pagination follows authoritative-inventory materialization. Each replication-order projection includes `assignment_revision`; each retained completion includes the accepted revision, nested owner/signer-policy identity, and nested finalized height/hash anchor. | Networking TL / Core Infra |
| Finality binding | Every pin page and detail response carries the height/hash from the same immutable finalized view. Clients continue a page with the returned non-zero `next_after_digest` as an exclusive `after_digest_hex` key and repeat that finalized pair; a stale requested cursor fails with HTTP 409. There is no offset or live-list compatibility mode. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the Kotlin/mirrored-Java, JavaScript, Python, Swift, and C# guard lanes mirror the signed `RegisterPinManifest` hard cut. Lifecycle event epochs are readback evidence only, never builder inputs. | SDK Teams |

Operations:
- `GET /v1/sorafs/pin` accepts `limit=1..=256`,
  `max_bytes=1024..=262144`, an optional lowercase non-zero
  `after_digest_hex`, an optional exact lowercase lifecycle `status`, and an
  optional paired `expected_finalized_height` plus
  `expected_finalized_block_hash_hex`. It returns digest-ordered bounded
  summaries, `has_more`, and the exclusive `next_after_digest`; unknown,
  duplicate, empty, percent-encoded, offset, and unpaired-anchor parameters
  are rejected. The response includes O(1) consensus-maintained retained-record
  and live-content `charged_usage` totals and is never produced by materializing the complete
  manifest registry.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  bounded native `manifest`. The retired `limit`, attestation, embedded
  alias/order arrays, counts, truncation fields, and list paging selectors are
  absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for authenticated
  list queries. Their returned pages are bounded, but the current handlers
  materialize the authoritative inventory before applying the page.
- `GET /v1/sorafs/aliases` accepts bounded `limit=1..=500` and canonical
  `u32` `offset` selectors plus optional exact case-sensitive canonical
  lowercase `namespace` and non-zero lowercase 32-byte `manifest_digest`
  filters. Unknown, duplicate, empty, noncanonical, and percent-encoded
  parameters are rejected before the authoritative query. Its response is the
  closed `SorafsAliasListResponseV1` projection, including exact typed lineage,
  cache-decision, and governance assessment objects.
- `GET /v1/sorafs/replication` accepts bounded `limit`/`offset` pagination plus
  `status` and `manifest_digest` filters. Each order emits
  `assignment_revision`. Every `provider_completions[]` entry emits
  `assignment_revision`,
  `completion_authority.provider_owner`,
  `completion_authority.signer_policy.{policy_id_hex,revision,predecessor_digest_hex,policy_digest_hex}`,
  and `finalized_anchor.{height,block_hash_hex}`. These are retained ledger
  facts, not live substitutions from the provider registry.
  Selectors are a strict hard cut: `limit` is `1..=500`, `offset` is a
  canonical `u32`, `status` is exactly lowercase `pending`, `completed`,
  `cancelled`, or `expired`, and `manifest_digest` is a non-zero lowercase
  32-byte digest;
  unknown, duplicate, empty, and alternate parameter names are rejected. The listing
  attestation and world data are derived from one full `StateView` generation.
- Registration is submitted as one canonical `SignedTransaction` containing
  exactly one `RegisterPinManifest`. The authenticated transaction authority
  pays the public-pin fee and consumes its deterministic quota; there is no
  client-supplied submission epoch and no general pin permission. Alias binding
  keeps its dedicated permission, threshold approval keeps its council
  envelope, and submitter-only retirement keeps its ownership check.

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
  and retiring a manifest cancels each pending order at or before its inclusive
  deadline and expires it only when retirement occurs strictly after that
  deadline.
- `ExpireReplicationOrder` closes a still-pending order only when its supplied
  epoch is strictly later than the inclusive completion deadline. It requires
  `CanIssueSorafsReplicationOrder`, accepts only an exact idempotent replay, and
  rejects early, conflicting, already-completed, or corrupt stored records
  without mutation.
- Golden JSON for events used by observability pipelines.

## Telemetry & Observability

Metrics (Prometheus):
- `torii_sorafs_pin_retained_manifests`
- `torii_sorafs_pin_live_content_bytes`
- Existing provider telemetry (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) remains in scope for end-to-end dashboards.

Logs:
- Finalized height/hash-bound query results are the local audit surface. The
  page's `charged_usage` and the two global Prometheus gauges above come from
  consensus-maintained accounting. Torii does not scan registry collections to
  synthesize lifecycle, alias, replication-order, or SLA aggregates. Signed
  governance archives can consume finalized results through the governance DAG
  when an operator rollout requires durable external evidence.

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
   canonical signed-transaction admission, and canonical manifest-derived policy checks are wired.
2. Norito schema, policy defaults, contract state, service facade, telemetry,
   fixtures, and local integration coverage are implemented.
3. Ongoing SF-4 work is rollout evidence: live registry audits, governance archive
   handoff, and operator-specific policy-change transcripts.

Each roadmap checklist item under SF-4 should reference this plan when progress is made.
The REST facade now ships with a finalized bounded list endpoint and native
manifest readback:

- `GET /v1/sorafs/pin` returns `PinManifestPageV1`: bounded summaries at one
  finalized height/hash, an exclusive digest continuation key, and O(1)
  consensus-maintained charged usage. Row and byte ceilings apply to every
  page; offset pagination and complete-registry materialization are absent.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact
  `PinManifestFinalizedRecordV1` JSON with the finalized cursor and native
  bounded manifest record.
- Canonical-account-signed `GET /v1/sorafs/aliases` and
  `GET /v1/sorafs/replication` expose the active alias catalogue and replication
  order backlog with consistent response pagination and status filters. They
  are classified as expensive compute until pagination can precede full
  authoritative-inventory materialization.

The CLI wraps these calls (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) so operators can script registry audits without touching
lower-level APIs.
