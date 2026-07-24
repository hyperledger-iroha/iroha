---
lang: fr
direction: ltr
source: docs/source/sorafs_pdp_plan.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 081d0b3c5ed447615ee16364f76176d2eae91f7419938d4b8d24b5d2ffd833e5
source_last_modified: "2026-07-06T19:29:26.838406+00:00"
translation_last_reviewed: 2026-07-06
source_mtime: 2026-07-06T19:29:26.838406+00:00
---

# Sora-PDP Hot Storage Proofs

## Status

SF-13 defines Sora-PDP hot-storage proofs. The local cryptographic layer builds
the canonical trees, generates byte-backed witnesses, verifies both commitment
roots, enforces exact governed coverage and deadlines, and verifies provider
signatures against council-verified admission records. Torii ships the
authenticated `/v1/sorafs/pdp/challenge`, `/next`, `/proof`, `/status`, and
`/export` protocol family. `/v1/sorafs/proof/stream` also accepts `pdp` only
when the caller supplies the existing non-zero governed challenge ID; it
rejects client-selected PDP sample counts and seeds. Persisted storage retains
canonical PDP trees and produces witnesses through verified no-follow chunk
reads.

This protocol is not production-ready while rejected or expired challenges
still converge on the process-local repair coordinator. The native repair
ledger exists, but PDP terminal handoff, retry reconciliation, and readback
must use its finalized committed task/event projection before SF-13 can be
promoted. Genuine multi-provider transport, restart, key-rotation, metrics,
archive, and repair evidence also remains external rollout work.
`scripts/check_sorafs_pdp_rollout_evidence.py` now provides the fail-closed
SF-13 rollout evidence gate for deployed PDP promotion, and
`scripts/run_sorafs_pdp_rollout_evidence.py` provides the matching
reviewed-evidence collection planner.

Implemented locally:

- `sorafs_manifest::pdp` exports `PdpCommitmentV1`, `PdpChallengeV1`,
  `PdpSampleV1`, `PdpProofV1`, `PdpProofLeafV1`, `PdpHotLeafProofV1`, and
  structural validators for version, non-zero identifiers, sample sets,
  duplicate hot leaves, non-zero digests, non-empty segment and hot-leaf Merkle
  paths, timestamps, and signatures.
- `crates/sorafs_manifest/tests/pdp.rs` covers the structural validators for
  commitments, challenges, and proofs.
- `sorafs-validate pdp` performs exhaustive diagnostic validation of committed
  commitments, challenges, proof signatures, byte witnesses, both Merkle roots,
  and all manifest/provider/epoch/deadline/coverage bindings. A standalone
  fixture triple reports `SFS-PDP-DIAG-000` and never authorizes production
  acceptance; only the admission-bound verifier can emit `SFS-OK-000`, after a
  council-verified active provider record supplies the trusted signing key.
- The PDP rollout evidence gate requires payload-free provider-transport,
  proof-generation, validator-replay, governance/repair, observability, and
  governance-approval artifacts before reporting `ready`, and it requires
  replay, governance/repair, observability, and governance approval evidence to
  carry a `proof_summary_digest_hex` matching a valid proof-generation artifact
  in the same bundle. Proof-generation artifacts also carry
  `policy_digest_hex` and `provider_roster_digest_hex`, valid PDP policy and
  provider-roster digests are published as `valid_policy_digests` and
  `valid_provider_roster_digests`, and governance approval evidence must bind
  its `policy_digest_hex` and `provider_roster_digest_hex` to the matching
  valid proof-generation digests. Provider-transport artifacts also bind
  `route_count` to the unique canonical `routes[].name` inventory and reject
  duplicate or unknown route entries before promotion can report ready. Route
  `latency_ms` evidence must be non-negative integer milliseconds before
  satisfying the route-latency ceiling, and every provider route response must
  include a `body_blake3_hex` digest.
  Proof-generation artifacts also bind `provider_count`, `challenge_count`, and
  `proof_count` to the unique canonical `providers[].name`, `challenges[].name`,
  and `proofs[].name` inventories and reject duplicate provider, challenge, or
  proof entries before promotion can report ready. Provider inventory labels
  must use reviewed lowercase `provider-*` IDs without non-production markers,
  challenge inventory labels must use reviewed lowercase `pdp-challenge-*`
  labels without non-production markers, and proof inventory labels must use
  reviewed lowercase `pdp-proof-*` labels without non-production markers.
  Proof-generation `max_proof_latency_ms` evidence must be positive integer
  milliseconds before satisfying the proof-latency ceiling.
  Observability artifacts also bind `metric_count` to the unique canonical
  `metrics` inventory, require the reviewed PDP metric set, and reject duplicate
  or unknown metric labels before promotion can report ready.
  PDP payload-safety artifacts must explicitly set `response_bodies_included`,
  `raw_challenge_bytes_included`, `raw_proof_bytes_included`,
  `raw_export_included`, `raw_report_included`, and
  `critical_alerts_firing` to `false` before promotion can report ready.
  The summary exports the sorted reviewed `metrics` inventory plus
  `metric_count_values`, and the aggregate production-readiness gate requires
  those fields to match the observability artifact fingerprint before final
  promotion can report ready. Governance/repair artifacts fingerprint
  `repair_handoff_digest_hex`, the summary exports
  `valid_repair_handoff_digests`, and the aggregate production-readiness gate
  requires those values to match governance/repair artifact fingerprints before
  final promotion can report ready. Aggregate promotion also rechecks the
  lane-proven PDP digest relationships: proof-summary-bound artifact
  fingerprints must match `valid_proof_summary_digests`, policy-bound artifact
  fingerprints must match `valid_policy_digests`, provider-roster-bound
  artifact fingerprints must match `valid_provider_roster_digests`, and
  repair-handoff metadata must match `valid_repair_handoff_digests` before
  final promotion can report ready. PDP rollout summaries must expose exactly
  one active proof summary digest, one active policy digest, one active
  provider-roster digest, and one active repair-handoff digest; mixed valid
  anchors fail closed before final promotion can report ready.
  Proof-summary mismatches are recorded on the offending artifact in the JSON
  summary before required-kind validity is reported. Policy and provider-roster
  mismatches are recorded on the offending governance approval artifact through
  the same summary path. The checker exports its required top-level payload
  fields as `EVIDENCE_REQUIRED_FIELDS`, and the collection runner includes the
  checker-backed `evidence_contract` map in dry-run output for the selected
  required kinds, and validates the schema-closed collection plan, required
  kinds, thresholds, external evidence map, evidence contract, and command steps
  before dry-run output or verifier execution. The shared runner plan guard also
  rejects non-canonical nested required-kind, threshold, external-evidence,
  evidence-contract, and command-step shapes before dry-run output or verifier
  execution.
- `scripts/build_sorafs_pdp_canary.py` builds individual payload-free SF-13
  canary artifacts for provider transport, proof generation, validator replay,
  governance/repair, observability, and governance approval evidence. The
  builder requires reviewed deployment context, complete PDP route and metric
  coverage where applicable, rejects duplicate or unknown route and metric
  inputs before writing, proof-summary digest bindings, provider/challenge/
  proof minimum counts, reviewed `provider-*` provider names plus reviewed
  `pdp-challenge-*` challenge names and `pdp-proof-*` proof names whose unique
  inventories match their scalar counts, rejects duplicate or non-production
  provider/challenge/proof names before writing, integer route/proof latency thresholds,
  explicit `--route-body-blake3-hex` evidence for provider transport routes,
  `--repair-handoff-digest-hex` evidence for governance/repair handoff,
  config-backed governance metadata, and reviewed policy and provider-roster
  digest input for proof-generation and governance-approval canaries, then
  validates every generated artifact through
  `scripts/check_sorafs_pdp_rollout_evidence.py` before writing. Checked-in
  response-file examples cover provider transport and proof-generation
  canaries.
- `fixtures/sorafs_manifest/pdp/` now contains canonical PDP commitment,
  challenge, and proof `.to`/JSON pairs plus negative fixtures for duplicate
  hot-leaf challenge material and missing proof signatures. The fixture bundle
  validator discovers these payloads from a clean checkout.
- `sorafs_manifest::PdpMerkleTreeV1` and
  `PdpMerkleTreeBuilderV1` implement the canonical, chunk-boundary-independent
  4 KiB hot-leaf and 256 KiB segment trees. Finished trees retain two exact
  flat node slabs, expose checked allocation-free retained-memory estimates,
  and generate witnesses through a bounded exact-position reader callback.
  The proof constructor rejects duplicate, unordered, or out-of-range samples
  before performing I/O, reads each requested hot leaf once, and verifies the
  returned bytes against the retained commitment before returning a witness.
- With its `manifest` feature, `sorafs_car::ChunkStore` builds that canonical
  PDP tree transactionally while it verifies the CAR plan and whole payload.
  Its PDP roots and counts no longer reuse the incompatible 64 KiB PoR tree,
  and arbitrary CAR chunk splits produce the same PDP roots as a contiguous
  payload. Plan validation applies a checked heap estimate before allocation or
  source/sink I/O.
- `sorafs_node::StorageBackend` persists the commitment and bounded retained
  tree, rehydrates them on restart, and generates exact challenge witnesses
  while holding the manifest read lease. Integration and adversarial tests
  cover restart parity, corrupted chunks, short/mutating reads, symlink and
  hard-link replacement, out-of-range/duplicate samples, and eviction races.
- `ProofStreamRequestV1` and the CLI request layer understand
  `proof_kind=pdp` as a challenge-bound proof kind.
  `sorafs_cli proof stream --proof-kind=pdp
  --challenge-id-hex=<governed-challenge-id>` emits the canonical request;
  client-selected `--samples` and `--sample-seed` are rejected for PDP.
- Capacity telemetry, penalty policy, reputation scoring, and proof-health
  dashboards already reserve PDP counters so governance can account for PDP
  success/failure once provider submissions are live.

Shipped fail-closed surfaces:

- Every provider-protocol route requires canonical application
  authentication. Challenge enqueue binds the exact canonical commitment and
  challenge to the provider's active council admission.
- `/v1/sorafs/proof/stream` requires `challenge_id_hex` for PDP, rejects
  absent/zero IDs and client-controlled sampling fields, and binds the returned
  durable status to the requested manifest and provider.
- The public OpenAPI description documents the five provider routes and
  challenge-bound PDP proof streaming as shipped V1 surfaces.
- Missing protocol runtime, unknown challenge, admission mismatch, malformed
  proof, archive failure, and repair-handoff failure are explicit errors; no
  request falls back to PoR sampling or an unsigned local result.

## Protocol Target

PDP complements PoR by providing higher-frequency integrity attestations for
hot replicas. It uses deterministic challenge material, provider signatures, and
commitment roots derived from hot 4 KiB leaves and 256 KiB segments.

Target payloads:

```norito
struct PdpCommitmentV1 {
    version: u8,
    manifest_digest: Digest32,
    chunk_profile: ChunkingProfileV1,
    commitment_root_hot: Digest32,
    commitment_root_segment: Digest32,
    hash_algorithm: HashAlgorithmV1,
    hot_tree_height: u16,
    segment_tree_height: u16,
    sample_window: u16,
    sealed_at: Timestamp,
}

struct PdpChallengeV1 {
    version: u8,
    challenge_id: Digest32,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    chunk_profile: ChunkingProfileV1,
    seed: Digest32,
    epoch_id: u64,
    drand_round: u64,
    response_deadline_unix: u64,
    samples: Vec<PdpSampleV1>,
}

struct PdpSampleV1 {
    segment_index: u32,
    hot_leaf_indices: Vec<u32>,
    segment_leaf_hash: Digest32,
}

struct PdpProofV1 {
    version: u8,
    challenge_id: Digest32,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    epoch_id: u64,
    proof_leaves: Vec<PdpProofLeafV1>,
    signature: Signature,
    issued_at_unix: u64,
}
```

## Challenge Generation

The production scheduler should share the SF-9 PoR randomness corridor while
remaining domain-separated:

- `seed = BLAKE3("sora:pdp:seed:v1" || drand_randomness || vrf_output || manifest_digest || epoch_id_le)`.
- `challenge_id = BLAKE3("sora:pdp:id:v1" || seed || provider_id || epoch_id_le || drand_round_le)`.
- Sample count should be deterministic from manifest size and policy, bounded
  by governance configuration and duplicate-resampling limits.
- Response deadline defaults should remain in the 4-10 minute policy window and
  be recorded in challenge payloads so validators can replay decisions.

## Provider Protocol Contract

The local V1 protocol gates are shipped:

1. Authenticated durable challenge enqueue, next-work pickup, proof submission,
   status, and sequence-bounded terminal export.
2. Deterministic proof generation from stored payloads, including segment and
   hot-leaf witness material bound to `PdpCommitmentV1` roots.
3. Provider signature verification over canonical PDP proof bytes with
   admission-controlled key material.
4. Deadline, manifest digest, provider ID, epoch, challenge ID, sample-set,
   replay, and terminal-idempotency checks in the provider-submission pipeline.
5. Governance DAG archival for accepted proofs and failure reports.
6. An explicit repair-handoff boundary for `pdp_failure` events.
7. OpenAPI coverage for the five provider routes and challenge-bound proof
   streaming.

The remaining authority gate is the sixth item: its target must be the
finalized native repair ledger rather than `FileRepairStore` or any other
process-local scheduler state.

## CLI And SDK Surface

Shipped today:

- `sorafs_cli proof stream --proof-kind=pdp
  --challenge-id-hex=<hex32>` serializes a challenge-bound
  `ProofStreamRequestV1` and consumes the NDJSON status response. PDP sampling
  remains fixed by the recorded challenge.
- `sorafs-validate pdp --commitment <commitment.to> --challenge <challenge.to>
  --proof <proof.to>` validates the reference fixture shape and pair binding.
- Canonical positive and negative PDP fixtures, the fail-closed rollout
  checker/runner, payload-free canary builder, and operator argfile templates
  are checked in.

Not shipped yet:

- `sorafs pdp challenge --manifest <CID> --provider <ID>`.
- `sorafs pdp fetch --manifest <CID>`.
- `sorafs pdp respond --challenge challenge.to --storage-path <path>`.
- `sorafs pdp verify --challenge challenge.to --proof proof.to --manifest manifest.to`.
- `sorafs pdp status --provider <ID> --limit 20`.
- `sorafs pdp export --since 2026-01-01 --out pdp_export.jsonl`.

Do not document the unshipped `sorafs pdp ...` commands as operator-ready until
they exist in the CLI and have focused tests.

## Testing And Fixtures

Implemented:

- Structural unit tests for `PdpCommitmentV1`, `PdpChallengeV1`, and
  `PdpProofV1`.
- Merkle tests covering 4 KiB/256 KiB boundaries, arbitrary streaming splits,
  odd-node padding, compact-path parity against an independent layered tree,
  corrupt/truncated/extended retained state, allocation overflow, exact reader
  counts, short/long reads, and tampered source bytes.
- CAR chunk-store tests that compare byte and streamed ingestion against an
  independent canonical PDP tree, prove chunk-boundary invariance, and verify
  transactional state preservation on digest, length, sink, and PDP failures.
- CLI proof-stream tests that require the governed PDP challenge ID and reject
  client-selected PDP sampling against mocked gateways.
- Torii tests for authenticated challenge enqueue/next-work/proof/status/export,
  challenge-bound proof streaming, governance archival, repair handoff, and
  telemetry counters.
- Canonical `fixtures/sorafs_manifest/pdp/` commitment/challenge/proof samples.
- Negative PDP fixtures for duplicate hot-leaf challenges, missing proof
  signatures, missing segment Merkle paths, missing hot-leaf Merkle paths,
  late proofs, wrong providers, wrong manifests, and witness coverage
  mismatches. The fixture tests cover every committed negative `.to` payload and
  verify that each commentary JSON `norito_bytes_hex` value matches the encoded
  bytes.
- Fail-closed PDP rollout evidence checker, dry-run-visible collection runner,
  checker-backed evidence-contract export, payload-free canary builder,
  focused tests, and operator argfile templates for reviewed deployed evidence,
  including cross-artifact proof-summary digest binding.

Required before production enablement:

- Replace the local repair coordinator target with finalized native repair
  transactions, committed task/event queries, durable retry reconciliation,
  and cross-peer exactly-once tests.
- SDK parity tests that verify the same PDP fixture bundle across Rust,
  JavaScript/TypeScript, Python, Swift, Kotlin/JVM, Java Android, and C#.

## Observability

Reserved telemetry names should stay stable:

- `sorafs_pdp_challenges_total{result}`.
- `sorafs_pdp_response_latency_seconds_bucket`.
- `sorafs_pdp_duplicates_total`.
- `sorafs_pdp_slash_proposals_total`.
- Proof-health gauges such as `torii_sorafs_proof_health_pdp_failures`.

Dashboards may show empty panels before a provider receives governed
challenges, but release evidence must distinguish zero work from unavailable
protocol, archive, or chain-repair dependencies.

## Rollout Status

Completed local foundations:

- Define PDP commitment, challenge, sample, and proof schemas.
- Add structural validators and unit tests.
- Derive PDP hot/segment commitment roots from stored payload trees.
- Generate PDP witnesses from persisted storage with restart, corruption,
  filesystem-race, exact-read, and eviction adversarial coverage.
- Reserve proof-stream request and telemetry labels.
- Generate canonical PDP fixture bundle and expanded negative fixtures.
- Add reference validator and `sorafs-validate pdp` coverage for PDP binding.
- Reject empty segment and hot-leaf Merkle paths in `PdpProofV1` and cover late
  proof, wrong provider, wrong manifest, and witness coverage mismatch paths in
  focused validator tests.
- Extend `generate_pdp_fixtures` so the expanded negative PDP fixture set is
  reproducible, committed, and covered by fixture inventory tests.
- Keep the fail-closed PDP rollout evidence gate and collection planner covered
  with proof-summary digest binding and rejection of evidence supplied for
  excluded `--require-kind` values.
- Require provider-transport route latency evidence to be non-negative and
  proof-generation max-latency evidence to be positive before either value can
  satisfy the SF-13 rollout threshold.
- Require governance/repair evidence to carry a reviewed
  `repair_handoff_digest_hex` that is fingerprinted, exported as
  `valid_repair_handoff_digests`, and tethered in aggregate production
  readiness.

Remaining production gates:

- Cut PDP terminal repair handoff over to the finalized native repair ledger
  and delete the local-authority path.
- Exercise admission-bound provider signatures, archive publication, restart,
  retry, revocation, and exactly-once repair across multiple providers and
  validators.
- Collect deployed provider-transport, proof-generation, validator-replay,
  governance/repair, observability, and governed-approval evidence that passes
  the SF-13 rollout gate with replay/governance/observability evidence bound to
  the same proof-generation summary digest, governance/repair evidence carrying
  `repair_handoff_digest_hex`, and any binding failure marked on the offending
  artifact in the emitted summary. Provider-transport latency values must be
  non-negative and proof-generation max-latency values must be positive, so
  impossible negative timings cannot satisfy rollout thresholds.
- Ship dedicated operator CLI commands and complete cross-SDK validation
  parity.
