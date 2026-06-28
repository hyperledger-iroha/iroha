---
lang: fr
direction: ltr
source: docs/source/sorafs_pdp_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9282c522d94aab6cbc286ac4de9ee8780050b76d409fc4ce3f4aa369c9010282
source_last_modified: "2026-06-25T17:38:19+00:00"
translation_last_reviewed: 2026-06-25
---

# Sora-PDP Hot Storage Proofs

## Status

SF-13 defines Sora-PDP hot-storage proofs. The local repository now has the
schema and accounting foundations, but the provider protocol is not production
ready yet. Torii therefore rejects PDP proof-stream requests with `400 Bad
Request` until real provider proof generation, signature verification, and
governance archival are implemented.
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
- `sorafs-validate pdp` validates committed PDP commitments, challenges, and
  proofs, including commitment/challenge binding and challenge/proof binding
  for manifest digest, provider id, epoch id, challenge id, response deadline,
  segment coverage, and hot-leaf coverage.
- The PDP rollout evidence gate requires payload-free provider-transport,
  proof-generation, validator-replay, governance/repair, observability, and
  governance-approval artifacts before reporting `ready`, and it requires
  replay, governance/repair, observability, and governance approval evidence to
  carry a `proof_summary_digest_hex` matching a valid proof-generation artifact
  in the same bundle. Proof-summary mismatches are recorded on the offending
  artifact in the JSON summary before required-kind validity is reported.
- `fixtures/sorafs_manifest/pdp/` now contains canonical PDP commitment,
  challenge, and proof `.to`/JSON pairs plus negative fixtures for duplicate
  hot-leaf challenge material and missing proof signatures. The fixture bundle
  validator discovers these payloads from a clean checkout.
- `sorafs_car::ChunkStore` derives deterministic PDP hot-leaf and segment roots
  from the same two-level tree used by PoR sampling, exposing
  `pdp_hot_root`, `pdp_segment_root`, `pdp_hot_leaf_count`, and
  `pdp_segment_count`.
- `ProofStreamRequestV1` and the CLI request layer understand
  `proof_kind=pdp` as a sample-count proof kind, allowing external PDP-capable
  gateways to be exercised by `sorafs_cli proof stream --proof-kind=pdp`.
- Capacity telemetry, penalty policy, reputation scoring, and proof-health
  dashboards already reserve PDP counters so governance can account for PDP
  success/failure once provider submissions are live.

Fail-closed surfaces:

- Torii `/v1/sorafs/proof/stream` accepts PoR and PoTR only. It parses `pdp`
  but returns `400 Bad Request` so clients do not mistake PoR samples for PDP
  provider proofs.
- The public OpenAPI description documents `proof_kind=pdp` as reserved.
- `sorafs_cli proof stream --proof-kind=pdp` is a client interoperability path
  for PDP-capable gateways, not proof that the embedded Torii gateway serves
  PDP today.

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

## Provider Protocol Gates

Do not remove the Torii fail-closed PDP guard until these local gates exist:

1. Provider challenge queue:
   `POST /sorafs/pdp/challenge`, `GET /sorafs/pdp/next`, and
   `POST /sorafs/pdp/proof` or their governed Torii equivalents.
2. Deterministic proof generation from stored payloads, including segment and
   hot-leaf witness material bound to `PdpCommitmentV1` roots.
3. Provider signature verification over canonical PDP proof bytes with
   governance-controlled key material.
4. Deadline, manifest digest, provider id, epoch, challenge id, and sample-set
   replay checks in the live provider-submission pipeline. The reference
   validator already covers these checks for committed PDP fixtures.
5. Governance DAG archival for accepted PDP proofs and PDP failure reports.
6. Repair pipeline handoff for `pdp_failure` events.
7. Portal/OpenAPI update that moves `proof_kind=pdp` from reserved to shipped.

## CLI And SDK Surface

Shipped today:

- `sorafs_cli proof stream --proof-kind=pdp --samples=<n>` serializes a
  `ProofStreamRequestV1` with `proof_kind=pdp` and consumes NDJSON responses
  from an external PDP-capable gateway.
- `sorafs-validate pdp --commitment <commitment.to> --challenge <challenge.to>
  --proof <proof.to>` validates the reference fixture shape and pair binding.

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
- Chunk-store tests that cover PDP commitment roots as part of the PoR tree.
- CLI proof-stream tests that verify PDP request serialization against mocked
  gateways.
- Torii test coverage that PDP proof-stream requests are rejected as unsupported
  while the provider protocol is absent.
- Canonical `fixtures/sorafs_manifest/pdp/` commitment/challenge/proof samples.
- Negative PDP fixtures for duplicate hot-leaf challenges and missing proof
  signatures. The fixture generator now also emits deterministic negative
  proof cases for missing segment Merkle paths, missing hot-leaf Merkle paths,
  late proofs, wrong providers, wrong manifests, and witness coverage
  mismatches once `generate_pdp_fixtures` is rerun.
- Fail-closed PDP rollout evidence checker, dry-run-visible collection runner,
  focused tests, and operator argfile templates for reviewed deployed evidence,
  including cross-artifact proof-summary digest binding.

Required before production enablement:

- Regenerate and commit the expanded negative PDP fixture artifacts for bad
  paths, deadline overruns, wrong provider ids, wrong manifest digests, and
  witness coverage mismatches once the workspace is free for fixture generation.
- Storage-node integration tests that generate PDP proofs from persisted
  payloads and validate them against commitment roots.
- Torii endpoint tests for challenge issuance, proof submission, governance
  archival, repair handoff, and telemetry counters.
- SDK parity tests that verify the same PDP fixture bundle across Rust,
  JavaScript/TypeScript, Python, Swift, Kotlin/JVM, Java Android, and C#.

## Observability

Reserved telemetry names should stay stable:

- `sorafs_pdp_challenges_total{result}`.
- `sorafs_pdp_response_latency_seconds_bucket`.
- `sorafs_pdp_duplicates_total`.
- `sorafs_pdp_slash_proposals_total`.
- Proof-health gauges such as `torii_sorafs_proof_health_pdp_failures`.

Dashboards may continue to show empty or telemetry-derived PDP panels before
provider protocol rollout, but release evidence must call out that embedded
Torii proof streaming is still fail-closed for PDP.

## Rollout Status

Completed local foundations:

- Define PDP commitment, challenge, sample, and proof schemas.
- Add structural validators and unit tests.
- Derive PDP hot/segment commitment roots from stored payload trees.
- Reserve proof-stream request and telemetry labels.
- Generate canonical PDP fixture bundle and initial negative fixtures.
- Add reference validator and `sorafs-validate pdp` coverage for PDP binding.
- Reject empty segment and hot-leaf Merkle paths in `PdpProofV1` and cover late
  proof, wrong provider, wrong manifest, and witness coverage mismatch paths in
  focused validator tests.
- Extend `generate_pdp_fixtures` so the expanded negative PDP fixture set is
  reproducible when fixture regeneration can run.
- Keep the fail-closed PDP rollout evidence gate and collection planner covered
  with proof-summary digest binding.

Remaining production gates:

- Implement provider challenge/proof transport.
- Verify provider signatures and PDP inclusion witnesses.
- Regenerate and commit the expanded negative fixture `.to`/JSON artifacts for
  bad paths, deadline overruns, wrong provider ids, wrong manifest digests, and
  witness coverage mismatches.
- Archive PDP verdicts/failures in Governance DAG and wire repair handoff.
- Collect deployed provider-transport, proof-generation, validator-replay,
  governance/repair, observability, and governed-approval evidence that passes
  the SF-13 rollout gate with replay/governance/observability evidence bound to
  the same proof-generation summary digest and any binding failure marked on the
  offending artifact in the emitted summary.
- Ship operator CLI commands and SDK validators.
- Update OpenAPI/portal docs and remove the Torii PDP fail-closed guard.
