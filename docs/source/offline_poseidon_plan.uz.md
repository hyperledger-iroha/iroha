---
lang: uz
direction: ltr
source: docs/source/offline_poseidon_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f9b2bf0c56c214e6e95584550e3d8bc13f402cf60f801c2f3f25b363ab3ea15
source_last_modified: "2026-01-05T18:22:23.405206+00:00"
translation_last_reviewed: 2026-02-07
title: Offline Poseidon & FASTPQ Plan
---

## 1. Goals

1. Finish the Poseidon receipt-tree contract (parameters, leaf schema, determinism rules) shared by
   all SDKs and ledger components.
2. Capture the Norito/ledger updates required to make `AggregateProofEnvelope` first-class so Torii,
   Iroha Core, and clients speak the same bytes.
3. Stage the FASTPQ circuit + witness work so SDKs can ship builders in parallel with prover/verifier
   plumbing.

This memo acts as the agreement artifact referenced by `roadmap.md` (OA13/OA14) and `docs/source/offline_allowance.md`.

## 2. Poseidon Profile (OA13)

| Parameter | Value | Notes |
|-----------|-------|-------|
| Field `q` | `2^64 - 2^32 + 1 = 0xffff_ffff_0000_0001` (Goldilocks) | Matches FASTPQ Standard(3) |
| Width `t` | 3 | 2 rate elements + 1 capacity |
| Rate `r` | 2 | Leaves absorb six field elements by streaming pairs |
| Capacity `c` | 1 | Keeps sponge aligned with FASTPQ Poseidon2 |
| S-box | `x ↦ x^5` | Full rounds apply to all lanes; partial rounds only mutate lane 0 |
| Full rounds `R_F` | 8 | Four at the start, four at the end |
| Partial rounds `R_P` | 57 | Spread across the center of the permutation |
| Domain tag | `b"iroha.offline.receipt.merkle.v1"` | Absorbed before every leaf/node hash |
| Constants/MDS | `artifacts/offline_poseidon/constants.ron` | Mirrors `crates/fastpq_isi/src/poseidon.rs` |

Action items (OA13.1):

1. Regenerate `constants.ron` + `vectors.json` with `cargo xtask offline-poseidon-fixtures`
   (overrides available via `--constants`, `--vectors`, and `--tag`).
2. Add a CI check (`cargo test -p iroha_data_model poseidon_vectors_match`) that recomputes the
   vectors from source to guard against drift.
3. Mirror the constant bundle into Swift/Android SDK fixtures so mobile teams can fuzz their
   Poseidon kernels without relying on Rust artifacts at runtime. The repo now carries synced copies
   under `IrohaSwift/Fixtures/offline_poseidon/` and
   `java/iroha_android/src/test/resources/offline_poseidon/`.

## 3. Leaf Schema & Determinism

Canonical leaf tuple:

```
tx_id: Hash
amount: Numeric { mantissa: i128, scale: u32 }
counter: u64
receiver_hash: Hash // blake2b32(Norito(AccountId))
invoice_hash: Hash  // blake2b32(invoice bytes)
platform_proof_hash: Hash // blake2b32(Norito(OfflinePlatformProof))
```

Encoding rules:

1. Serialize `AccountId` and `OfflinePlatformProof` via Norito (`norito::to_bytes`).
2. Hash the serialized bytes with BLAKE2b-256 before reducing modulo `q`.
3. Reduce mantissa/scale/counter limbs with the helpers in
   `crates/iroha_data_model/src/offline/poseidon.rs`.
4. Absorb the six field elements in the fixed order
   `[tx_id, amount.mantissa, amount.scale, counter, receiver_hash, invoice_hash, platform_proof_hash]`.
5. Sort receipts by `(counter, tx_id)` before building the tree; ties use the secondary sort to keep
   multi-receipt batches deterministic.
6. Pad odd levels with the zero field element (`PoseidonDigest::zero()`).

Deliverables (OA13.2):

* Rust: `OfflineReceiptLeaf` + `OfflineReceiptMerkleBuilder` + `compute_receipts_root()` (already
  merged) with regression tests that cover zero padding, ordering, and fuzzed invoice sizes.
* Swift/Android: mirror the builder API so SDKs can recompute the root locally when preparing bundles.
* Fuzzing: add ABI-stable fixtures (`fixtures/offline_poseidon/*.json`) proving parity between Rust
  and SDK builds.

## 4. Norito & Ledger Updates (OA13.a)

Required changes (partially merged, track to completion):

1. **Norito schema:** ensure `norito.md` documents `AggregateProofEnvelope`, Poseidon leaf encoding,
   and the new rejection reasons (`AggregateProofVersionUnsupported`, `AggregateProofRootMismatch`,
   `AggregateProofHashError`).
2. **Data model:** `crates/iroha_data_model/src/offline/mod.rs` must expose the new structs and
   Norito derives so RPC clients can serialize/deserialize envelopes. Add schema hashes to the
   `iroha_schema` registry.
3. **Ledger admission:** `crates/iroha_core/src/smartcontracts/isi/offline.rs` now calls
   `verify_aggregate_proof_envelope` before crediting bundles. Thread configuration knobs
   (`offline.proofs.mode`) through `iroha_config` once OA14.3 lands.
4. **Telemetry & Torii:** extend Torii payloads/metrics with `receipts_root` + proof status so
   operators can audit bundles without DB access. Document the new REST surface in the OpenAPI spec.
5. **Docs:** keep `docs/source/offline_allowance.md` and `docs/source/offline_allowance_*`
   translations in sync with the Poseidon spec. Link back to this plan for implementation context.

## 5. FASTPQ Circuits & Witness Builders (OA14)

### 5.1 Scheduling (OA14.s)

| Sprint | Deliverable | Owner(s) | Milestone |
|--------|-------------|----------|-----------|
| S1 | Sum circuit prototype (CPU) + witness API draft | FASTPQ WG | T+4 weeks |
| S2 | Sum circuit Metal/CUDA parity + resource envelope doc | FASTPQ WG | T+8 weeks |
| S3 | Counter circuit + checkpoint witness builder | FASTPQ WG / Core Runtime | T+12 weeks |
| S4 | Replay circuit + receiver log export plumbing | FASTPQ WG / SDK Leads | T+16 weeks |
| S5 | Ledger verifier integration + telemetry | Core Runtime / Telemetry | T+20 weeks |
| S6 | SDK builders (Swift/Android) + CLI fixtures | SDK Leads / Tooling | T+24 weeks |

Dependencies:

* Witness request struct (`offline_proof_request_sum` etc.) containing `receipts_root`,
  `balance_commitments`, `receipt_amounts`, `counter checkpoints`, and HKDF seeds for blindings.
* Deterministic HKDF context string: `b"iroha.offline.fastpq.v1"` with `certificate_id‖counter` as
  salt so every platform derives the same blinding factors.

The shared Norito structs now live in
`crates/iroha_data_model/src/offline/mod.rs::{OfflineProofRequestHeader,OfflineProofRequestSum,OfflineProofRequestCounter,OfflineProofRequestReplay,OfflineProofBlindingSeed}`,
giving the SDK/API layers a canonical schema as soon as the prover endpoints land. The CLI exposes
`iroha offline transfer proof --bundle <PATH> --kind <sum|counter|replay>` (e.g.,
`iroha offline transfer proof --bundle ./bundle.json --kind replay --replay-log-head ... --replay-log-tail ...`)
so builders can fetch the Norito payloads directly from a local bundle payload, and Torii exposes
the same data via `POST /v2/offline/transfers/proof` (transfer payload + kind, optional
checkpoint/log hashes).

### 5.2 Circuit Summaries

1. **Sum circuit (OA14.1):**
   * Inputs: Poseidon leaf hashes, commitments (`C_init`, `C_res`), claimed delta.
   * Constraints: `Σ amount_i = claimed_delta`, `C_res = C_init - Pedersen(Σ amount_i)`.
   * Witness builder reads receipts from the Poseidon builder, reuses the same ordering, and derives
     blindings via HKDF.
   * APIs: `{prove,verify}_offline_sum(request, worker_hint)` returning proof bytes + telemetry.

2. **Counter circuit (OA14.2):**
   * Inputs: `counter_start`, ordered counters, Poseidon root.
   * Constraints: `counter_0 = checkpoint + 1`, `counter_i = counter_{i-1} + 1`.
   * Witness builder reuses allowance checkpoints exported by Torii so SDKs do not need storage
     internals.

3. **Replay circuit (OA14.2):**
   * Inputs: receiver replay-log head/tail, ordered `tx_id`s, Poseidon root.
   * Constraints: hashing each `tx_id` into the prior head yields the reported tail, preventing
     omission or reordering.

### 5.3 Ledger & SDK Integration (OA14.3 / OA14.a)

  `optional`. Failing proofs emit structured errors (reason + receipts root) and Prometheus counters.
* **SDK builders:** Swift/Android maintain rolling Poseidon trees, call the FASTPQ APIs via FFI, and
  attach `AggregateProofEnvelope`. Builders expose telemetry hooks so apps can surface progress.
* **CLI/tooling:** `iroha offline bundle inspect` (implemented in
  `crates/iroha_cli/src/offline.rs`) now loads JSON/Norito bundle fixtures, recomputes the Poseidon
  `receipts_root`, highlights mismatches against any embedded `AggregateProofEnvelope`, and, when
  run with `--proofs`, prints proof byte counts plus metadata keys. Fixture generators now live
  under `scripts/offline_bundle/` (see the README + `spec.example.json`) so operators can produce
  witness-bearing bundles and FASTPQ requests without touching the Rust sources.

## 6. Next Actions

1. Regenerate Poseidon constants/vectors and wire the CI guard (OA13.1).
2. Draft the witness request structs/FFI headers and circulate them with SDK teams (OA14.s).
3. Update `docs/source/offline_allowance.md`/`norito.md` references once the above artifacts land.
