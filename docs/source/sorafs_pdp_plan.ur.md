---
lang: ur
direction: rtl
source: docs/source/sorafs_pdp_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2693b28e1c34e88cbfd6677b7b6f80d80b08ce45fa17ca97219e21a6ddbde2df
source_last_modified: "2026-01-03T18:08:01.827329+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Sora-PDP Hot Storage Proofs
summary: Final specification for SF-13 covering challenge generation, proof structure, transport, governance integration, and enforcement.
---

# Sora-PDP Hot Storage Proofs

## Goals & Scope
- Deliver a Provable Data Possession (PDP) protocol for the SoraFS hot storage tier, complementing PoR by providing higher-frequency integrity attestations.
- Define challenge derivation, proof structure, transport headers, and response deadlines.
- Integrate PDP with governance (DAG, slashing, billing) and the repair pipeline.
- Provide operator, SDK, and CLI expectations so implementation can proceed.

This document completes **SF-13 — Implement Sora-PDP hot-storage proofs**.

## Protocol Overview
- PDP operates on the hot tier chunk profile (FastCDC target 256 KiB segments with 4 KiB minimum) and reuses PoR randomness (drand + provider VRF).
- Challenges target both fine-grained (4 KiB) and segment-level (256 KiB) leaves to detect tampering while controlling verification cost.
- Providers respond with signed Merkle proofs demonstrating possession of sampled segments; responses are validated by Torii, recorded in Governance DAG, and influence staking/slashing.
- Challenges are issued every 2 epochs (2 hours) per manifest; response deadline is 6 minutes after issue (configurable 4–10 minutes).

## Data Types
Norito structs (to live in `sorafs_manifest::pdp`):
```norito
struct PdpCommitmentV1 {
    version: u8,                     // 1
    manifest_digest: Digest32,
    chunk_profile: ChunkingProfileV1,
    commitment_root_hot: Digest32,   // 4 KiB tree root
    commitment_root_segment: Digest32, // 256 KiB tree root
    hash_algorithm: HashAlgorithmV1, // e.g., blake3-256
    hot_tree_height: u16,
    segment_tree_height: u16,
    sample_window: u16,              // expected samples per epoch
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
    hot_leaf_indices: Vec<u32>,    // 4 KiB indices within the segment
    segment_leaf_hash: Digest32,   // root expectation for the segment
}

struct PdpProofV1 {
    version: u8,
    challenge_id: Digest32,
    manifest_digest: Digest32,
    provider_id: ProviderId,
    epoch_id: u64,
    proof_leaves: Vec<PdpProofLeafV1>,
    signature: Signature,          // Dilithium3 over canonical bytes
    issued_at_unix: u64,
}

struct PdpProofLeafV1 {
    segment_index: u32,
    segment_hash: Digest32,
    segment_merkle_path: Vec<Digest32>,
    hot_leaves: Vec<PdpHotLeafProofV1>,
}

struct PdpHotLeafProofV1 {
    leaf_index: u32,
    leaf_hash: Digest32,
    leaf_merkle_path: Vec<Digest32>,
}
```

## Challenge Generation
- Scheduler piggybacks on PoR infrastructure (SF-9):
  - `seed = BLAKE3("sora:pdp:seed:v1" || drand_randomness || vrf_output || manifest_digest || epoch_id_le)`.
  - `challenge_id = BLAKE3("sora:pdp:id:v1" || seed || provider_id || epoch_id_le || drand_round_le)`.
- Sample selection:
  - Determine manifest size and compute base sample count: `max(32, min(256, ceil(manifest_size / 64 MiB)))`.
  - For each sample, pick segment index via RNG, then choose `k` hot leaves (default 3) inside the segment.
  - Ensure unique hot leaf indices per segment; re-sample collisions up to 8 attempts, else flag `duplicate` (recorded in metrics).
- Challenge cadence: every 2 epochs per manifest (2 hours). Governance can override to hourly for critical manifests.
- Response deadline default 6 minutes; policy override range 4–10 minutes.

## Transport & API
- PDP piggybacks on Torii HTTP endpoints:
  - `POST /sorafs/pdp/challenge` (internal) issues challenge to provider queue (similar to PoR).
  - Providers fetch via WebSocket or REST `GET /sorafs/pdp/next`.
  - Providers submit proof via `POST /sorafs/pdp/proof` with Norito payload.
- HTTP headers:
  - `Sora-PDP-Challenge: <base64url(norito_bytes)>` for gateway fetch responses.
  - `Sora-PDP-Proof: <base64url(norito_bytes)>` optional for streaming proofs; canonical channel remains Norito body.
- SDK adjustments:
  - CLI `sorafs pdp fetch --manifest <CID>` to view latest challenge.
  - `sorafs pdp respond --challenge challenge.to --storage-path <path>` for provider automation.

## Verification Pipeline
1. Torii verifies challenge/proof IDs match, manifest digest matches, and signature is valid.
2. For each `PdpProofLeafV1`:
   - Reconstruct segment Merkle path to `commitment_root_segment`.
   - For each hot leaf, verify path to `commitment_root_hot`.
   - Confirm leaf hashes match manifest chunk digests (via `sorafs_manifest::chunk_digest`).
3. Deadline enforcement: proof `issued_at_unix` must be ≤ `response_deadline_unix`; lag recorded.
4. On success -> log `PdpVerdictV1` to Governance DAG (`GovernancePayloadKind::PdpProof`).
5. On failure/timeouts -> produce `PdpFailureReportV1` and push repair task (`failure_reason = "pdp_failure"`).

## Governance & Slashing Integration
- Failure thresholds (configurable via policy):
  - **Warning**: ≥2 failures in 24h -> freeze 10% stake, notify governance.
  - **Slashing**: ≥5 failures or no proof for 3 consecutive challenges -> propose slashing `min(0.5 * stake, 10_000 XOR)`.
- Slashing proposals appended to Governance DAG with supporting `PdpFailureReportV1`.
- Hedging/billing system notified to adjust invoices (tie-in with `sorafs_hedging_plan.md`).
- Repair pipeline receives PDP failures automatically and attempts remediation.

## Observability
- Metrics:
  - `sorafs_pdp_challenges_total{result}` (issued, verified, failed, timeout).
  - `sorafs_pdp_response_latency_seconds_bucket`.
  - `sorafs_pdp_duplicates_total` for sampling collisions.
  - `sorafs_pdp_slash_proposals_total`.
- Alerts:
  - `SORAfsPdpFailureRateHigh` (>3 failures 6h).
  - `SORAfsPdpNoProof` (no proof in 3 challenges).
  - `SORAfsPdpLatency` (p95 latency > 4m).
- Logs include `challenge_id`, `manifest_digest`, `provider_id`, `result`, `latency_ms`.

## CLI & SDK
- CLI commands (augment reference SDK):
  - `sorafs pdp challenge --manifest <CID> --provider <ID>` (manual trigger; governance signed).
  - `sorafs pdp verify --challenge challenge.to --proof proof.to --manifest manifest.to`.
  - `sorafs pdp status --provider <ID> --limit 20`.
  - `sorafs pdp export --since 2026-01-01 --out pdp_export.jsonl`.
- SDK functions:
  - `PdpValidator::validate(challenge, proof, manifest, commitment)` returning `ValidationOutcome`.
  - `PdpScheduler::generate(seed, manifest_metadata)` (shared with coordinator).

## Testing & Fixtures
- Fixtures under `fixtures/sorafs_manifest/pdp/`:
  - `challenge.json`, `challenge.to`, `proof.json`, `proof.to`, `commitment.json`.
  - Negative fixtures (invalid path, deadline exceeded).
- Unit tests in `crates/sorafs_manifest/tests/pdp.rs` verifying decode/validate.
- Integration tests in `crates/sorafs_node/tests/pdp.rs` simulating challenge issuance and proof verification.
- CLI golden tests comparing JSON output for valid/invalid cases.
- Fuzz tests targeting Merkle path verification and seed derivation.

## Rollout Steps
1. Implement `sorafs_manifest::pdp` module with commitments/challenge/proof structs + validators.
2. Extend PoR scheduler to emit PDP challenges (every 2 epochs).
3. Implement Torii endpoints and provider client hooks.
4. Update governance DAG pipeline to store PDP events (kind `PdpProof`/`PdpFailure`).
5. Add CLI/SDK support per spec.
6. Staging rollout: enable PDP for subset of manifests, monitor metrics.
7. Production rollout with phased enforcement: warning-only then slashing.

## Documentation & Operator Guides
- `docs/source/sorafs/pdp_operator.md` – overview, rollout playbook, error codes.
- Docs portal page `docs/portal/docs/sorafs/proofs/pdp.md` with API examples.
- Update `sorafs_reference_sdk_plan.md` to reference PDP commands/outcomes.
- Governance docs describing policy overrides and failure thresholds.

## Implementation Checklist
- [x] Document PDP commitments, challenge/proof structures, and randomness.
- [x] Define transport headers, Torii APIs, and CLI surfaces.
- [x] Align PDP with governance/repair/slashing flows.
- [x] Capture metrics, alerts, and logging requirements.
- [x] Specify fixtures, tests, and rollout steps.
- [x] Outline documentation artefacts for operators and developers.

This specification enables engineering teams to implement Sora-PDP proofs consistently across the coordinator, provider clients, governance systems, and tooling.
