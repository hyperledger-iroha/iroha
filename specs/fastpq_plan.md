# FASTPQ Prover Work Breakdown

This document captures the FASTPQ-ISI V1 implementation and its release
boundary. V1 deliberately exposes only witnessed transfers and opaque metadata
carriers; unsupported supply, permission, and non-membership variants are not
retained in the wire or trace schema.

Cryptographic qualification note (2026-08-29): the sole canonical parameter
record targets 128-bit aggregate qROM security with six independently generated
Goldilocks digest lanes, degree-four FRI challenges, binary folds, and 136
queries. The exact arithmetic calculator meets that target, but arithmetic is
not a production qualification claim: the protocol-specific qROM reduction and
the final-artifact multi-target digest review remain mandatory. Appendix A
records the calculation and blockers.

## Implemented release boundary

- Public `Prover::prove`, `verify`, and `verify_with_limits` select the
  `TransferStateTransition` profile explicitly. A non-empty batch must contain
  only Transfer rows backed by canonical transfer transcripts and complete
  touched-balance-tree update witnesses. An empty batch is accepted only when
  `old_root == new_root`.
- Transfer rows use exactly eight little-endian bytes for both values and exact
  `asset/<asset-id>/<account>` keys with three non-empty slash-free components.
- Transfer witness validation binds each 32-bit Merkle direction vector to the
  key-derived path, authenticates every sibling/root update, enforces debit and
  credit amounts, and chains the witnessed roots to `PublicIO.old_root` and
  `PublicIO.new_root`.
- The sampled AIR composition is not the complete transfer semantic boundary.
  The verifier deterministically rebuilds the caller-carried batch, transcript,
  SMT witness, full trace, LDE material, and commitments and requires exact
  equality. Key identities, path nodes, and public roots are therefore enforced
  by full verifier replay rather than by every trace column appearing in the
  current residue vector. Removing
  that replay would be a protocol break; a future succinct profile must move
  every accepted-state relation into authenticated public input and AIR.
- MetaSet batches fail closed in the public state-transition profile. The V1
  operation enum contains only Transfer and MetaSet; old experimental mint,
  burn, and role encodings are rejected during decoding. The release wire
  indices are `16` for Transfer and `17` for MetaSet, deliberately disjoint
  from every pre-release index (`0..=5`) so removal cannot relabel an old
  operation as a supported one.
- AXT verification selects `AxtTransferClaim` or `AxtOpaqueEffect` only after a
  canonical outer `AxtFastpqBinding` is authenticated and exactly matched. The
  opaque profile accepts MetaSet carrier rows only; its public roots are
  externally authenticated statement context, not proven state updates.
- Production CoreHost rejects non-null standalone `AXT_VERIFY_DS_PROOF` before
  proof recording or cache mutation. A caller-carried witness cannot serve as
  remote authorization without an authoritative finalized source-state anchor,
  irrespective of the proof commitment width.
- Handle-bound transfer proofs carry canonical remote-spend claim preimages.
  Verification reconstructs their commitments and requires an exact one-to-one
  match with real transfer transcripts across handle identity, dataspace,
  asset, accounts, amount, and cardinality. The issuer signature authenticates
  the capability/asset fields, not the intent, proof, or amount, so this
  specialized path remains outside release qualification while those exact
  facts are not authenticated. Finalized lane-relay and authoritative fee-vault
  paths retain their separate state anchors.
- Test/dev-tools raw-statement helpers check cryptographic transcript and byte
  determinism only. They make no state-validity claim and are absent from normal
  production builds.

## Stage 0 — Foundation
- Deterministic Norito encoding and domain-separated commitments.
- Canonical parameter table provided by `fastpq_isi`.
- A single production STARK backend selected by the canonical prover constructors.

## Stage 1 — Trace Builder Prototype

> **Status:** `fastpq_prover` exposes canonical packing helpers (`pack_bytes`,
> `PackedBytes`) and a full-width, domain-separated BLAKE2b-256 ordering
> commitment. The dense-MDS Poseidon constants used by the STARK are pinned by
> the repository asset and manifest; this construction is not Poseidon2. The
> ordering fixture (`tests/fixtures/ordering_hash.json`) anchors the sorted-row
> regression, while `tests/trace_commitment.rs` covers canonical commitment
> encoding, determinism, and fixture separation. The JSON fixture is regenerated
> only when `FASTPQ_UPDATE_FIXTURES=1` is set explicitly.

### Implemented trace schema
- Each row encodes only the V1 statement surface:
  - `key_limbs[i]`: base-256 limbs (7 bytes, little-endian) of the canonical key path.
  - `value_old_limbs[i]`, `value_new_limbs[i]`: same packing for pre/post values.
  - Selector columns: `s_active`, `s_transfer`, `s_meta_set`.
  - Auxiliary columns: `delta = value_new - value_old` on transfer rows and `metadata_hash_limb_0` through `metadata_hash_limb_7`.
  - Transfer witness projection columns per level `ℓ`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`. These columns exist only when the batch contains a Transfer row; metadata-only proofs do not allocate or commit 128 zero SMT columns.
  - Metadata columns: `dsid`, `slot`.
- **Deterministic ordering.** Stable-sort rows lexicographically by
  `(key_bytes, op_rank)`; rows with an equal key and operation retain their
  supplied order without carrying a redundant ordinal in the wire type.
  `op_rank` mapping: `transfer=0`, `meta_set=1`. Persist the full BLAKE2b-256 hash of
  `fastpq:v1:ordering || canonical_norito(sorted_transitions)`. Canonical
  Norito length framing keeps trailing zero bytes distinct.
- Implemented base residues enforce selector booleanity/relations, transfer row deltas, active-prefix shape, and metadata/dsid/slot stability. Generic Merkle hashing and boundary totals are enforced by deterministic witness replay rather than a succinct AIR relation.
- `N_trace = 2^k` (`pow2_ceiling` of row count); `N_eval = N_trace * 2^b` where `b` is the blowup exponent.
- Provide fixtures and property tests:
  - Packing limb coverage in `fastpq_prover/src/packing.rs` unit tests.
  - Ordering stability hash (`tests/fixtures/ordering_hash.json`).
  - Trace-commitment canonical/determinism coverage (`tests/trace_commitment.rs`).
  - Canonical V1 raw-transcript fixture (`v1_raw_transcript_64.bin`).

### AIR Column Schema
| Column Group      | Names                                                                                  | Description                                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Activity          | `s_active`                                                                               | 1 for real rows, 0 for padding.                                                                                       |
| Main              | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]`                               | Packed Goldilocks elements (little-endian, 7-byte limbs).                                                             |
| Selectors         | `s_transfer`, `s_meta_set`                                                                | 0/1. `s_active` equals their sum.                                                                                      |
| Auxiliary         | `delta`, `metadata_hash_limb_0..7`                                                       | Transfer delta and stable metadata are constrained.                                                                   |
| SMT               | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`                                    | Transfer-batch-only projection of validated witnesses; generic AIR hashing/non-membership is not implemented.         |
| Metadata          | `dsid`, `slot`                                                                           | Constant across rows.                                                                                                 |

The metadata map is canonically Norito-encoded and committed with raw
Blake2b-256 over `u64_le(domain_len) || domain || u64_le(metadata_len) ||
metadata`, using the domain `fastpq:v1:metadata-commitment:blake2b-256`.
The 32 digest bytes are injected without field reduction as eight little-endian
`u32` limbs, and every limb is constrained to remain constant across the trace.
This replaces the collision-prone single Goldilocks-field projection. Because
the trace schema and commitments change, this is a first-release hard cut:
proofs and binary/golden proof artifacts produced with the single
`metadata_hash` column must be regenerated.

### Implemented math and deferred constraints
- **Field packing:** bytes are chunked into 7-byte limbs (little-endian). Each limb `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; reject limbs ≥ Goldilocks modulus.
- **Transfer delta:** the base trace reconstructs the complete fixed-width u64
  pre/post values and constrains `delta = value_new - value_old` in Goldilocks.
- **Padding:** introduce `s_active`. Multiply all row constraints by `s_active` and enforce a contiguous prefix: `s_active[i] ≥ s_active[i+1]`. Padding rows (`s_active=0`) must keep constant values but are otherwise unconstrained.
- **Ordering hash:** full-width BLAKE2b-256 (domain `fastpq:v1:ordering`) over the
  canonical Norito transition encoding; stored in Public IO for auditability.

## V1 — STARK Prover Core

### Objectives
- Build dense-MDS Poseidon Merkle commitments over trace and LDE evaluation vectors. Parameters: Goldilocks `x^7` S-box, rate=2, capacity=1, full rounds=8, partial rounds=57, and the constants pinned by `artifacts/poseidon/constants.ron`.
- Low-degree extension: evaluate each column on the multiplicative coset
  `D = { o · g^i | i = 0 .. N_eval-1 }`, where `N_eval = 2^{k+b}` divides the
  2-adic capacity of Goldilocks, `g` is the pinned root of exact order
  `N_eval`, and `o = omega_coset` is the pinned nonzero offset outside that
  subgroup. The sole compiled parameter record fixes this geometry; the
  parameter identifier is bound into transcript initialization.
- Composition commitment: combine the 16 implemented residues with 16
  independently sampled coefficients. Proving and verifier-side derivation
  first require every residue to vanish on the canonical base trace; the raw
  LDE composition is then committed for low-degree testing.
- FRI with the sole V1 binary arity `r = 2`: for each layer, absorb the
  six-lane 384-bit root with tag `fastpq:v1:fri_layer:<round>`, sample a
  `GoldilocksFp4V1` challenge `β_ℓ` (tag `fastpq:v1:beta:<round>`), and fold
  an opened multiplicative coset using the domain elements for that coset.
  Verifiers must bind every opened value to its Merkle path and evaluation
  point; an x-free linear combination of sibling values is not a valid
  low-degree check.
  The implemented prover/verifier now uses strided multiplicative cosets,
  inverse-subgroup decomposition, the round domain point, and adaptive final
  arity without repeat-last padding. It stops while the complete terminal domain
  still contains 2--16 evaluations, opens that single authenticated leaf, and
  inverse-interpolates it to reject coefficients at or above the verifier-owned
  reduced bound. The initial exclusive composition bound is conservatively
  `2 * N_trace`, matching the maximum quadratic degree of the V1 residue ledger.
- V1 verification deterministically rebuilds the canonical trace, LDE, and
  AIR commitments from the supplied batch before authenticating sampled LDE
  query chunks and per-round FRI openings. The FRI base layer is the AIR
  composition evaluation vector, so sampled constraints are also recomputed
  from opened current/next rows and bound to the corresponding FRI opening.
- Proof object (Norito-encoded):
  ```
  Proof {
      protocol_version: u16,
      parameter: String,
      trace_commitment: GoldilocksDigest384V1,
      public_io: PublicIO,
      trace_root: GoldilocksDigest384V1,
      air_trace_root: GoldilocksDigest384V1,
      air_composition_root: GoldilocksDigest384V1,
      lde_root: GoldilocksDigest384V1,
      lde_domain_size: u32,
      alphas: Vec<u64>,
      betas: Vec<GoldilocksFp4V1>,
      fri_layers: Vec<GoldilocksDigest384V1>,
      queries: Vec<QueryOpening>,
      air_openings: Vec<AirConstraintOpening>,
      fri_queries: Vec<FriQueryOpening>,
  }

  QueryOpening {
      index: u32,
      value: u64,
      chunk_values: Vec<u64>,
      merkle_path: Vec<GoldilocksDigest384V1>,
  }

  FriRoundOpening {
      round: u32,
      index: u32,
      values: Vec<GoldilocksFp4V1>,
      folded_value: GoldilocksFp4V1,
      merkle_path: Vec<GoldilocksDigest384V1>,
  }

  FriQueryOpening {
      initial_index: u32,
      rounds: Vec<FriRoundOpening>,
      final_index: u32,
      final_values: Vec<GoldilocksFp4V1>,
      final_merkle_path: Vec<GoldilocksDigest384V1>,
  }

  AirConstraintOpening {
      index: u32,
      current_row: Vec<u64>,
      next_row: Vec<u64>,
      current_row_path: Vec<GoldilocksDigest384V1>,
      next_row_path: Vec<GoldilocksDigest384V1>,
      composition_value: u64,
      composition_path: Vec<GoldilocksDigest384V1>,
  }
  ```
- Node-facing V1 verification deterministically rebuilds the canonical trace and
  its LDE, AIR-trace, and AIR-composition commitments from the supplied batch
  before accepting proof-carried roots or openings. `VerifyLimits`
  caps proof material, query counts, path depth, transition count, and payload
  size before that work begins; the 1k-row CPU/GPU parity case and 20k-row
  benchmark workloads remain outside the serialized fixture set.
- The release batch, proof, and public-I/O carriers have explicit
  `TransitionBatchV1`, `ProofV1`, and `PublicIOV1` Norito schema identities.
  Pre-release schema headers are rejected, so removed ordinals and
  lookup/version fields cannot shift an old payload into the smaller release
  layouts.

### Implemented residue accounting

| Residues | Count | Implemented check |
|----------|------:|-------------------|
| Selector booleanity | 3 | Every V1 selector is 0 or 1. |
| Selector relations | 1 | Active equals the Transfer-plus-MetaSet selector sum. |
| Active prefix | 1 | An inactive row cannot be followed by an active row. |
| Transfer delta | 1 | Full fixed-width post-minus-pre value on Transfer rows. |
| Stable statement data | 10 | Eight metadata commitment limbs, `dsid`, and `slot` remain stable. |

The 16 residues above are the complete implemented AIR composition schema.
They do not constrain generic SMT node hashing/non-membership or old/new-root
boundary totals; V1 transfer verification deterministically replays those
witness checks from the caller-carried batch.

Padding rows are handled through `s_active`; dummy rows extend the trace to `N_trace` without violating constraints.
Before interpolation, both proving and verifier-side derivation evaluate every
listed residue independently on the canonical base trace and reject any
non-zero residue. The composition polynomial over the extended domain is a
low-degree commitment and is not required to vanish at every LDE point.

## Encoding & Transcript (Global)
- **Byte packing:** base-256 (7-byte limbs, little-endian). Unit tests live beside
  `fastpq_prover/src/packing.rs`.
- **Field encoding:** canonical Goldilocks (little-endian 64-bit limb, reject ≥ p).
  `GoldilocksFp4V1` values encode four canonical limbs in 32 bytes; native-STARK
  commitments and Merkle siblings use `GoldilocksDigest384V1`, which encodes six
  canonical lanes in 48 bytes and rejects alternate representatives during
  construction and decoding.
  Transfer touched-balance roots remain 32-byte `iroha_crypto::Hash` values, not
  Poseidon field elements. After proof-size limits and before transcript
  verification, the verifier walks every remaining proof-carried scalar and
  extension value and rejects alternate representatives; digest carriers have
  already enforced canonicality, and opaque public hashes are intentionally
  excluded from this field preflight.
- **Transcript (Fiat–Shamir):**
  1. Initialize the six-lane Poseidon transcript from the canonical Norito
     encoding of `protocol_version`, `parameter`, and `public_io` under
     `fastpq:v1:init`.
  2. Absorb `trace_root` under `fastpq:v1:trace_root`, then derive one
     `fastpq:v1:column_mix:<i>` base-field challenge per trace column.
  3. Absorb `lde_root`, `trace_root` (`fastpq:v1:roots`).
  4. Derive exactly 16 V1 composition challenges `α_j`
     (`fastpq:v1:alpha:<j>`), one for each current constraint residue. Coefficients
     are never reused across residues.
  5. Absorb `air_trace_root`, `air_composition_root` (`fastpq:v1:air_roots`).
  6. For each nonterminal FRI layer root, absorb
     `fastpq:v1:fri_layer:<round>` and derive the corresponding Fp4 challenge
     `fastpq:v1:beta:<round>`; absorb the terminal root under
     `fastpq:v1:fri:final`.
  7. Derive deduplicated query indices with rejection sampling under
     `fastpq:v1:query_index:<counter>`.

  Tags are lowercase ASCII; verifiers reject mismatches before sampling challenges. The
  `v1_raw_transcript_64.bin` fixture pins the resulting transcript bytes within
  the public verifier's admitted geometry. Larger-scale performance evidence is
  captured separately by the release benchmarks.
- **Versioning:** `protocol_version = 1` is the only release protocol. The
  `parameter` name selects one exact record from the compiled canonical
  catalogue, which the prover and verifier use directly. There is no secondary
  catalogue-version compatibility layer. Changes to the proof schema,
  transcript, or canonical parameters are first-release hard cuts and require
  regenerating the proof fixture.

## Permission commitment status

`perm_root` is authenticated statement context only. V1 has no role operation,
permission witness column, lookup product, or permission-mutation proof type.
Adding one requires a new explicitly constrained first-release schema rather
than reviving the removed experimental lookup scaffolding.

## Transfer touched-balance tree

The release-safe state profile validates the canonical transfer transcript and
its 32-level touched-balance-tree witnesses before deriving the trace. For every
sender/receiver update it:

- derives the path from the exact canonical balance key and requires the four
  witness path bytes to match it exactly;
- hashes the fixed-width balance leaf and every sibling with domain-separated
  `iroha_crypto::Hash` nodes;
- authenticates both pre/post roots, transfer debit/credit arithmetic, update
  ordering, and the chain from public `old_root` to public `new_root`.

The verifier deterministically repeats this validation from the supplied batch.
Trace Merkle columns mirror the validated witness but are not a generic SMT AIR;
arbitrary inserts/deletes and non-membership are not part of V1.

## Soundness Parameters & SLOs

These estimates describe the cryptographic low-degree/query layer only. They do
not compensate for an absent semantic constraint; the explicit profile gate
above is what prevents unsupported operations from reaching production proof
acceptance.
| N_trace | blowup | FRI arity | layers | queries | aggregate arithmetic target | Proof size (≤) | RAM (≤) | P95 latency (≤) |
| ------- | ------ | --------- | ------ | ------- | --------------------------- | --------------- | ------- | ---------------- |
| 2^16    | 8      | 2         | ≤18    | 136     | 128 bits met; qualification blocked pending review | measured by release fixture | measured by release run | measured by release run |

Derivations follow Appendix A. Deterministic verifier-negative tests cover
malformed proofs, and the sole V1 parameter record fixes the domain geometry,
six-lane commitment construction, degree-four challenge field, 128-bit target,
and query count. Independent protocol-specific soundness qualification remains
required before the arithmetic result can be treated as release evidence.

## Public IO Schema
| Field            | Bytes | Encoding                              | Notes                               |
|-----------------|-------|---------------------------------------|-------------------------------------|
| `dsid`           | 16    | little-endian UUID                    | Dataspace ID for the entry's lane (global for default lane), hashed with tag `fastpq:v1:dsid`. |
| `slot`           | 8     | little-endian u64                     | Nanoseconds since epoch.            |
| `old_root`       | 32    | canonical hash bytes                  | Transfer touched-balance pre-root; external context for opaque AXT. |
| `new_root`       | 32    | canonical hash bytes                  | Transfer touched-balance post-root; external context for opaque AXT. |
| `perm_root`      | 32    | opaque commitment bytes               | Authenticated context only; permission membership is not proven in V1. |
| `tx_set_hash`    | 32    | BLAKE2b                               | Sorted instruction identifiers.     |
| `parameter`      | var   | UTF-8 (`fastpq-state-transition-stark-v1`) | Sole V1 parameter set name.         |
| `protocol_version` | 2   | little-endian u16                     | The sole V1 protocol discriminator. |
| `ordering_hash`  | 32    | domain-separated BLAKE2b-256          | Stable hash of sorted rows.         |

Generic deletion and absent-key/non-membership proofs are not implemented in
the release profile.

`FastpqTransitionBatch.public_inputs` is the canonical carrier for `dsid`,
`slot`, and root commitments. Proof-bound metadata carries the canonical AXT
binding mirrors, transfer transcripts, remote-spend claim preimages,
amount/expiry/manifest/DA mirrors, and the batch seal. Metadata never selects a
proof semantics profile.

## Commitment encodings
- Ordering hash: full-width BLAKE2b-256 (tag `fastpq:v1:ordering`).
- Preprocessing trace commitment: six-lane Poseidon-x7 Goldilocks digest over
  typed column leaves, binary nodes, parameter identity, and exact trace shape.

## Stage Definitions of Done (DoD)
- **Stage 1 DoD**
  - Canonical packing unit vectors merged.
  - This implementation-coupled plan records the exact V1 columns and symbolic constraints.
  - Ordering hash recorded in PublicIO and verified via fixtures.
  - Transfer touched-balance witness generation and key-derived path binding implemented.
  - Non-membership, permission mutation, and supply operations are outside V1.
- **V1 Prover DoD**
  - Transcript spec implemented; tag/order unit tests and the binary V1 proof fixture pin the transcript.
  - Dense-MDS Poseidon constants and the Goldilocks `x^7` S-box are pinned in prover and verifier with endianness and former-collision tests across architectures.
  - Canonical parameter geometry checks are active; the sole record declares
    the 128-bit aggregate arithmetic target, while production qualification
    remains unavailable until independent review is registered. Proof
    size/RAM/latency must be measured from release runs.
    **TODO:** land the empirical 16-residue Monte Carlo characterization described in Appendix A.
- **Stage 3 DoD**
  - Scheduler API (`SubmitProofRequest`, `ProofResult`) documented with idempotency keys.
  - Proof artifacts stored content-addressably with retry/backoff.
  - Telemetry exported for queue depth, queue wait time, prover execution latency, retry counts, backend failure counts, and GPU/CPU utilisation, with dashboards and alert thresholds for each metric.

## Stage 5 — GPU Acceleration & Optimisation
- Target kernels: LDE (NTT), Poseidon hashing, Merkle tree construction, FRI folding.
- Determinism: disable fast-math, ensure bit-identical outputs across CPU, CUDA, Metal. CI must compare proof roots across devices.
- Benchmark suite comparing CPU vs GPU on reference hardware (e.g., Nvidia A100, AMD MI210).
- CUDA command/event waits are bounded at 120 seconds. Any non-success status from asynchronous
  event or stream completion handling—including, but not limited to, a timeout—quarantines the
  backend for the process lifetime and deliberately abandons resources whose ownership is
  uncertain. Subsequent operations therefore cannot accumulate or free device buffers that may
  still be in flight. Direct CUDA calls return an error; the proof hashing layer records the
  dispatch failure and uses its deterministic CPU fallback. Mandatory-GPU startup preflight
  remains a separate fail-closed gate.【crates/fastpq_prover/cuda/fastpq_cuda.cu:22】【crates/fastpq_prover/cuda/fastpq_cuda.cu:42】【crates/fastpq_prover/src/trace.rs:1252】
- Metal backend (Apple Silicon):
  - Build script compiles the kernel suite (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon.metal`, `metal/kernels/bn254.metal`) into `fastpq.metallib` via `xcrun metal`/`xcrun metallib`; install and select full Xcode (standalone Command Line Tools are insufficient). The build probes the optional MetalToolchain but never installs components or clears system caches; missing tools warn and select runtime source compilation.【crates/fastpq_prover/build.rs:30】【crates/fastpq_prover/build.rs:107】
  - Manual rebuild (mirrors `build.rs`) for CI warm-ups or deterministic packaging:
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/poseidon.metal -o "$OUT_DIR/poseidon.air"
    xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/bn254.metal -o "$OUT_DIR/bn254.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon.air" "$OUT_DIR/bn254.air" -o "$OUT_DIR/fastpq.metallib"
    ```
    Release builds let `build.rs` generate the library and embed its Cargo `OUT_DIR` path; packaged binaries whose path is stale use embedded source compilation. `FASTPQ_METAL_LIB` remains a debug/dev-only override rather than production configuration.【crates/fastpq_prover/build.rs:210】【crates/fastpq_prover/src/metal.rs:2475】
  - The LDE kernel now assumes the evaluation buffer is zero-initialised on the host. Keep the existing `vec![0; ..]` allocation path or explicitly zero buffers when reusing them.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - Coset multiplication is fused into the final FFT stage to avoid an extra pass; any changes to LDE staging must preserve that invariant.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - The shared-memory FFT/LDE kernel now stops at the tile depth and hands the remaining butterflies plus any inverse scaling to a dedicated `fastpq_fft_post_tiling` pass. The Rust host threads the same column batches through both kernels and only launches the post-tile dispatch when `log_len` exceeds the tile limit, so queue-depth telemetry, kernel stats, and fallback behaviour stay deterministic while the GPU handles the wide-stage work entirely on-device.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - To experiment with launch shapes, set `FASTPQ_METAL_THREADGROUP=<width>`; the dispatch path clamps the value to the device limit and logs the override so profiling runs can sweep threadgroup sizes without recompiling.【crates/fastpq_prover/src/metal.rs:321】
  - Tune the FFT tile directly: the host derives threadgroup lanes (16 for short traces, 32 once `log_len ≥ 6`, 64 once `log_len ≥ 10`, 128 once `log_len ≥ 14`, and 256 at `log_len ≥ 18`) plus a five-/four-stage tile depth for smaller domains. Because the tile holds 256 words, every heuristic and override is capped at eight radix-2 stages before control passes to the post-tile kernel. Override with `FASTPQ_METAL_FFT_LANES` (power of two between 8 and 256) and `FASTPQ_METAL_FFT_TILE_STAGES` (1–8) to pin specific launch shapes; both values flow through `FftArgs`, get clamped to the supported window, and are logged for profiling sweeps.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT and LDE column batching now derive from the resolved threadgroup width: the host targets roughly 4 096 logical threads per command buffer, fuses up to 64 columns at a time with the circular-buffer tile staging, and only ratchets down through 64 → 32 → 16 → 8 → 4 → 2 → 1 columns as the evaluation domain crosses the 2¹⁶/2¹⁸/2²⁰/2²² thresholds. This keeps the 20 k-row capture at ≥64 columns per dispatch while ensuring long cosets still finish deterministically. The adaptive scheduler still doubles column width until dispatches approach the ≈2 ms target and now halves the batch automatically whenever a sampled dispatch lands ≥30 % over that target, so lane/tile transitions that inflate per-column cost fall back without manual overrides. Poseidon permutations share the same adaptive scheduler and the `metal_heuristics.batch_columns.poseidon` block in `fastpq_metal_bench` now records the resolved state count, cap, last duration, and override flag so queue-depth telemetry can be tied directly to Poseidon tuning. Override with `FASTPQ_METAL_FFT_COLUMNS` (1–64) to pin a deterministic FFT batch size, and use `FASTPQ_METAL_LDE_COLUMNS` (1–64) when you need the LDE dispatcher to honour a fixed column count; the Metal bench surfaces the resolved `kernel_profiles.*.columns` entries in every capture so tuning experiments stay reproducible.【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】
- Multi-queue dispatch is now automatic on discrete Macs: the host inspects `is_low_power`, `is_headless`, and the device location to decide whether to spin up two Metal command queues, only fans out when the workload carries at least 16 columns (scaled by the resolved fan-out), and round-robins the column batches so long traces keep both GPU lanes busy without sacrificing determinism. The command-buffer semaphore now enforces a “two in flight per queue” floor, and queue telemetry records the aggregate measurement window (`window_ms`) plus normalized busy ratios (`busy_ratio`) for the global semaphore and every queue entry so release artefacts can prove both queues stayed ≥50 % busy over the same time span. Override the defaults with `FASTPQ_METAL_QUEUE_FANOUT` (1–4 lanes) and `FASTPQ_METAL_COLUMN_THRESHOLD` (minimum total columns before fan-out); the Metal parity tests force the overrides so multi-GPU Macs stay covered, and the resolved policy is logged alongside the queue-depth telemetry and the new `metal_dispatch_queue.queues[*]` block.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- Metal detection now probes `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` directly, and `FASTPQ_DEBUG_METAL_ENUM` prints the enumerated devices when set so headless CI runs can explain why `FASTPQ_GPU=gpu` still downgraded to the CPU path. Detection reflects usable `MTLDevice` hardware and no longer equates a missing offline metallib with a missing GPU. When the override is set to `gpu` but no accelerator is detected, `fastpq_metal_bench` errors immediately with a pointer to the debug knob instead of silently continuing on the CPU. This narrows the “silent CPU fallback” class called out in WP2‑E and gives operators a knob to capture enumeration logs inside wrapped benchmarks.【crates/fastpq_prover/src/backend.rs:716】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU timings now refuse to treat CPU fallbacks as “GPU” data. `hash_columns_gpu` reports whether the accelerator actually ran, `measure_poseidon_gpu` drops samples (and logs a warning) whenever the pipeline falls back, and the Poseidon microbench child exits with an error if GPU hashing is unavailable. As a result, `gpu_recorded=false` whenever Metal execution falls back, the queue summary still records the failed dispatch window, and dashboard summaries immediately flag the regression. The wrapper (`scripts/fastpq/wrap_benchmark.py`) now fails when `metal_dispatch_queue.poseidon.dispatch_count == 0` so Stage 7 bundles can’t be signed without real GPU Poseidon dispatch evidence.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】
- Poseidon column hashing has one accelerator path in V1. `PoseidonColumnBatch` builds one flattened payload plus checked offset/length descriptors. CUDA submits that batch directly; Metal divides it into adaptive column ranges and overlaps its completion-backed command slots internally. The trace layer then hashes the canonical `(⌈columns / 2⌉)` depth-1 layer through the shared Merkle-pair helper. `ColumnDigests` carries that optional first level and `merkle_root_with_first_level` consumes it immediately.【crates/fastpq_prover/src/trace.rs:970】【crates/fastpq_prover/src/gpu.rs:478】【crates/fastpq_prover/src/metal.rs:3658】【crates/fastpq_prover/cuda/fastpq_cuda.cu:2636】
- `fastpq_metal_bench` now emits a `device_profile` block with the Metal device name, registry id, `low_power`/`headless` flags, location (built-in, slot, external), discrete indicator, `hw.model`, and the derived Apple SoC label (for example, “M3 Max”). Stage 7 dashboards consume this field to bucket captures by M4/M3 vs discrete GPUs without parsing hostnames, and the JSON ships next to the queue/heuristic evidence so every release artefact proves which fleet class produced the run.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】
  - FFT host/device overlap now uses a double-buffered staging window: while batch *n* finishes inside `fastpq_fft_post_tiling`, the host flattens batch *n + 1* into the second staging buffer and only pauses when a buffer must be recycled. The backend records how many batches were flattened plus the time spent flattening versus waiting for GPU completion, and `fastpq_metal_bench` surfaces the aggregated `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` block so release artefacts can prove the overlap instead of silent host stalls. The JSON report now also breaks the totals down per phase under `column_staging.phases.{fft,lde,poseidon}`, letting Stage 7 captures prove whether FFT/LDE/Poseidon staging is host-bound or waiting on GPU completion. Poseidon permutations reuse the same pooled staging buffers, so `--operation poseidon_hash_columns` captures now emit the Poseidon-specific `column_staging` deltas alongside the queue-depth evidence without bespoke instrumentation. The new `column_staging.samples.{fft,lde,poseidon}` arrays record the per-batch `batch/flatten_ms/wait_ms/wait_ratio` tuples, making it trivial to prove that the `COLUMN_STAGING_PIPE_DEPTH` overlap is holding (or to spot when the host starts waiting for GPU completions).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/fastpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】
- Poseidon acceleration caches round constants and MDS rows in threadgroup memory and keeps the full/partial rounds unrolled. Production Goldilocks paths deliberately assign one independent state per lane and size every grid from the actual workload, avoiding the old artificial 4,096-thread floor while preserving CPU/GPU parity. Lane-width tuning remains available for profiling, but effective telemetry reports `states_per_lane = 1`; BN254 Poseidon retains its separately bounded multi-state geometry. Column-batch telemetry reports the submitted `columns`, successful `batches`, and `fallbacks`; actual Metal command overlap remains in queue, staging, and kernel-profile telemetry rather than a synthetic pipeline-depth field.【crates/fastpq_prover/metal/kernels/poseidon.metal:1】【crates/fastpq_prover/src/metal.rs:3115】【crates/fastpq_prover/src/trace.rs:400】
  - LDE tile staging mirrors the FFT contract: the 256-word tile executes no more than eight radix-2 stages, and the post-tiling kernel handles every wider butterfly. Override with `FASTPQ_METAL_LDE_TILE_STAGES` (1–8) whenever you need a deterministic depth; the host only launches the post-tiling dispatch when the heuristic stops early so queue-depth and kernel telemetry stay deterministic.【crates/fastpq_prover/src/metal.rs:827】
  - Kernel micro-optimisation: the shared-memory FFT/LDE tiles reuse per-lane twiddle and coset strides instead of re-evaluating `pow_mod*` for every butterfly. Each lane precomputes `w_seed`, `w_stride`, and (when required) the coset stride once per block, then streams through the offsets to reduce inner-loop multiplications in `apply_stage_tile`/`apply_stage_global`. Quantify the result with a fresh 20 k-row capture; no reference report is checked in.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】
  - The kernel suite now has a dedicated reference (`specs/fastpq_metal_kernels.md`) that documents each entry point, the threadgroup/tile limits enforced in `fastpq.metallib`, and the reproduction steps for compiling the metallib manually.【specs/fastpq_metal_kernels.md:1】
  - The benchmark report now emits a `post_tile_dispatches` object that records how many FFT/IFFT/LDE batches ran in the dedicated post-tiling kernel (per-kind dispatch counts plus the stage/log₂ boundaries). `scripts/fastpq/wrap_benchmark.py` copies the block into `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary`, and the manifest gate refuses GPU captures that omit the evidence so every 20 k-row artefact proves the multi-pass kernel ran on-device.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - Set `FASTPQ_METAL_TRACE=1` to emit per-dispatch debug logs (pipeline label, threadgroup width, launch groups, elapsed time) for Instruments/Metal trace correlation.【crates/fastpq_prover/src/metal.rs:346】
- The dispatch queue is now instrumented: `FASTPQ_METAL_MAX_IN_FLIGHT` caps concurrent Metal command buffers (auto default derived from the detected GPU core count via `system_profiler`, clamped to at least the queue fan-out floor with a host-parallelism fallback when macOS refuses to report the device). The bench enables queue-depth sampling so the exported JSON carries a `metal_dispatch_queue` object with `limit`, `dispatch_count`, `max_in_flight`, `busy_ms`, and `overlap_ms` fields for release evidence, adds a nested `metal_dispatch_queue.poseidon` block whenever a Poseidon-only capture (`--operation poseidon_hash_columns`) runs, and emits a `metal_heuristics` block describing the resolved command-buffer limit plus the FFT/LDE batch columns (including whether overrides forced the values) so reviewers can audit the scheduling decisions alongside the telemetry. Poseidon kernels also feed a dedicated `poseidon_profiles` block distilled from the kernel samples so bytes/thread, occupancy, and dispatch geometry are tracked across artefacts. If the primary run can’t collect queue depth or the LDE zero-fill stats (for example, when a GPU dispatch silently falls back to the CPU), the harness automatically fires a single probe dispatch to gather the missing telemetry and now synthesizes host zero-fill timings when the GPU refuses to report them, so published evidence always includes the `zero_fill` block.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Set `FASTPQ_SKIP_GPU_BUILD=1` to skip offline shader compilation; the warning records the skip, while visible macOS Metal hardware remains usable through embedded runtime source compilation.【crates/fastpq_prover/build.rs:32】【crates/fastpq_prover/src/metal.rs:2348】
  - Runtime detection uses the Metal API to confirm a usable device independently of shader-library location. The build prefers `fastpq.metallib`, warns with an explicit manual install command when the offline compiler is missing, and compiles embedded self-contained MSL 2.4 source when the offline library is absent. Pipeline/preflight failures remain fail-closed; explicit `FASTPQ_METAL_LIB` overrides are limited to debug/dev builds.【crates/fastpq_prover/build.rs:107】【crates/fastpq_prover/src/backend.rs:745】【crates/fastpq_prover/src/metal.rs:2334】
  - Operator checklist (Metal hosts):
    1. Prefer the build-generated `.metallib` for release evidence, but record when the packaged binary's embedded Cargo path is absent or stale and the runtime-source path is intentionally under test.【crates/fastpq_prover/build.rs:210】【crates/fastpq_prover/src/metal.rs:2475】
    2. Run parity tests with GPU lanes enabled: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. This exercises the Metal kernels and falls back automatically if detection fails.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. Capture a benchmark sample for dashboards using the release build's embedded library path (or runtime-source fallback):
      `cargo run -p fastpq_prover --features fastpq-gpu,dev-tools --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       The canonical `fastpq-state-transition-stark-v1` set now pads every capture to 32,768 rows, so the
       JSON reflects both the requested 20 k rows and the padded domain that drives the GPU
       kernels. Upload the JSON/log to the release evidence store; this repository does not ship a
       nightly FastPQ Metal workflow or a reference capture. The report records
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` alongside each operation’s `speedup`, the
     LDE section adds `zero_fill.{bytes,ms,queue_delta}` so release artefacts prove determinism,
     host zero-fill overhead, and the incremental GPU queue usage (limit, dispatch count,
     peak in-flight, busy/overlap time), and the new `kernel_profiles` block captures per-kernel
     occupancy ratios, estimated bandwidth, and duration ranges so dashboards can flag GPU
       regressions without reprocessing raw samples.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Expect the Metal LDE path to stay under 950 ms (`<1 s` target on Apple M-series hardware);
4. Capture row-usage telemetry from a real ExecWitness so dashboards can chart transfer gadget
   adoption. Fetch a witness from Torii
  (`iroha_cli audit witness --binary --out exec.witness`) and decode it with
  `iroha_cli audit witness --decode exec.witness` (optionally add
  `--fastpq-parameter fastpq-state-transition-stark-v1` to assert the expected parameter set; FASTPQ batches
  emit by default; pass `--no-fastpq-batches` only if you need to trim the output).
   Every batch entry now emits a `row_usage` object (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, `meta_set_rows`, and `transfer_ratio`). Archive that JSON snippet to avoid
   reprocessing raw transcripts.【crates/iroha_cli/src/audit.rs:209】 Compare the new capture against
   the previous baseline with `scripts/fastpq/check_row_usage.py` so CI fails if transfer ratios or
   total rows regress:

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_<baseline-date>.json \
     --candidate fastpq_row_usage_<candidate-date>.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```

   Sample JSON blobs for smoke tests live in `scripts/fastpq/examples/`. The comparison helper and
   `ci/check_fastpq_row_usage.sh` produce `fastpq_row_usage_summary.json`, but the repository does
   not contain a dedicated workflow or checked-in execution captures under
   `artifacts/fastpq_benchmarks/`. Generate the baseline and candidate from real witnesses before
   invoking the shell gate, or call `check_row_usage.py` directly with explicit paths.
   Row-usage regression inputs must come from execution-captured V1 batches with
   real transfer SMT witnesses. The old standalone synthetic row generator has
   been removed because it could not validate the sender/receiver root chain.

   Stage 7-3 rollout bundles must also pass `scripts/fastpq/validate_row_usage_snapshot.py`, which
   enforces that every `row_usage` entry contains the selector counts and that
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` calls the helper
   automatically so bundles missing those invariants fail before GPU lanes are mandated.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       the bench manifest gate enforces this via `--max-operation-ms lde=950`, so refresh the
       capture whenever your evidence exceeds that bound.
      When you also need Instruments evidence, pass `--trace-dir <dir>` so the harness
      relaunches itself via `xcrun xctrace record` (default “Metal System Trace” template) and
      stores a timestamped `.trace` file alongside the JSON; you can still override the location /
      template manually with `--trace-output <path>` plus optional `--trace-template` /
      `--trace-seconds`. The resulting JSON advertises `metal_trace_{template,seconds,output}` so
      artefact bundles always identify the captured trace.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Wrap each capture with
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_<date>.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (add `--gpg-key <fingerprint>` if you need to pin a signing identity) so the bundle fails
       fast whenever the GPU LDE mean breaches the 950 ms target, Poseidon exceeds 1 s, or the
       Poseidon telemetry blocks are missing, embeds a `row_usage_snapshot`
      next to the JSON, surfaces the Poseidon microbench summary under `benchmarks.poseidon_microbench`,
      and still carries metadata for runbooks and the Grafana dashboard
    (`dashboards/grafana/fastpq_acceleration.json`). The JSON now emits `speedup.ratio` /
     `speedup.delta_ms` per operation so release evidence can prove GPU vs
     CPU gains without reprocessing the raw samples, and the wrapper copies both the
     zero-fill statistics (plus `queue_delta`) into `zero_fill_hotspots` (bytes, latency, derived
     GB/s), records the Instruments metadata under `metadata.metal_trace`, threads the optional
     `metadata.row_usage_snapshot` block when `--row-usage <decoded witness>` is supplied, and flattens the
     per-kernel counters into `benchmarks.kernel_summary` so padding bottlenecks, Metal queue
     utilisation, kernel occupancy, and bandwidth regressions are visible at a glance without
     spelunking the raw report.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】
     Because the row-usage snapshot now travels with the wrapped artefact, rollout tickets simply
     reference the bundle instead of attaching a second JSON snippet, and CI can diff the embedded
    counts directly when validating Stage 7 submissions. To archive the microbench data on its own,
    run `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    and store the resulting file under `benchmarks/poseidon/`. Keep the aggregated manifest fresh with
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    so dashboards/CI can diff the full history without walking each file manually.
    4. Validate telemetry by curling `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (Prometheus endpoint) or looking for `telemetry::fastpq.execution_mode` logs; unexpected `backend="none"` or failed GPU preflight entries indicate the host did not satisfy explicit GPU readiness.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. Use `zk.fastpq.execution_mode = "cpu"` to document the deterministic CPU operating path during maintenance; production `gpu` mode must pass preflight or stay disabled.【crates/iroha_config/src/parameters/user.rs:3964】【crates/iroha_core/src/fastpq/lane.rs:228】
- Telemetry & readiness:
  - Execution-mode logs (`telemetry::fastpq.execution_mode`) and counters (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) expose explicit GPU readiness and CPU default usage so failed GPU preflight is visible in dashboards.【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - The `FASTPQ Acceleration Overview` Grafana board (`dashboards/grafana/fastpq_acceleration.json`) visualises the Metal adoption rate and links back to the benchmark artefacts, while the paired alert rules (`dashboards/alerts/fastpq_acceleration_rules.yml`) gate rollouts on sustained downgrades.
  - Low-level prover benches still support `FASTPQ_GPU={auto,cpu,gpu}` for developer diagnostics; production node policy is sourced from `zk.fastpq.execution_mode`/CLI and supports only `cpu` or `gpu`.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:3964】
  - GPU parity tests (`cargo test -p fastpq_prover --features fastpq-gpu`) must pass for CUDA and Metal; Metal tests skip only when no device is visible. A missing metallib exercises the runtime-source path, while source/pipeline failures fail the test with their concrete error.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/metal.rs:2331】
  - CUDA now also has focused low-level BN254 FFT/LDE parity coverage via
    `fastpq_bn254_fft(...)` and `fastpq_bn254_lde(...)`, and
    `fastpq_cuda_bench` now promotes those timings into a raw
    `bn254_metrics` block so wrapped CUDA evidence can carry
    `acceleration.bn254_{fft,lde}_ms` directly. When the local BN254 CUDA path
    downgrades at runtime, the bench now keeps the CPU timings and emits
    `bn254_warnings` instead of aborting the capture. The CPU, CUDA, and Metal
    radix-2 decimation-in-time paths bit-reverse coefficient input before their
    butterfly stages. Independent direct-Horner CPU tests cover both FFT and
    coset LDE, and GPU parity uses that tested CPU oracle rather than a duplicate
    transform. The remaining work is the lab rerun / higher-level integration
    side, not the basic bench/report wiring.
  - Metal readiness evidence (archive the artefacts below with every rollout so the roadmap audit can prove determinism, telemetry coverage, and fail-closed GPU behaviour):

    | Step | Goal | Command / Evidence |
    | ---- | ---- | ------------------ |
    | Build metallib | Ensure `xcrun metal`/`xcrun metallib` are available and emit the deterministic `.metallib` for this commit | Compile `metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon.metal`, and `metal/kernels/bn254.metal` into their `.air` files; run `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon.air" "$OUT_DIR/bn254.air" -o "$OUT_DIR/fastpq.metallib"`. Release builds generate and embed their own Cargo `OUT_DIR` path.【crates/fastpq_prover/build.rs:144】【crates/fastpq_prover/build.rs:210】
    | Verify library path | Record whether the build-time library or embedded source path is in use | Archive the relevant `build.rs` output and note whether the packaged binary retained its embedded Cargo `OUT_DIR` library. An absent or stale path selects embedded runtime source compilation and does not disable visible Metal hardware.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/metal.rs:2475】
    | GPU parity suite | Prove kernels execute before shipping production `gpu` mode | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` and store the resulting log snippet that shows `backend="metal"` or an unavailable-backend warning that blocks GPU rollout.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    | Benchmark sample | Capture the JSON/log pair that records `speedup.*` and FFT tuning so dashboards can ingest accelerator evidence | `cargo run -p fastpq_prover --features fastpq-gpu,dev-tools --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; archive the JSON, the timestamped `.trace`, and stdout alongside release notes so the Grafana board picks up the Metal run (the report records the requested 20 k rows plus the padded 32,768-row domain so reviewers can confirm the `<1 s` LDE target).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Wrap & sign report | Fail the release if the GPU LDE mean breaches 950 ms, Poseidon exceeds 1 s, or Poseidon telemetry blocks are missing, and produce a signed artefact bundle | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; ship both the wrapped JSON and the generated `.json.asc` signature so auditors can verify the sub-second metrics without rerunning the workload.【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    | Signed bench manifest | Enforce `<1 s` LDE evidence across Metal/CUDA bundles and capture signed digests for release approval | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; attach the generated manifest + signature to the release ticket so downstream automation can validate the sub-second proof metrics.【xtask/src/fastpq.rs:1】
| CUDA bundle | Keep the SM80 CUDA capture in lock-step with the Metal evidence so manifests cover both GPU classes. | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --features dev-tools --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_<date>.json` on the Xeon + RTX host → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; append the generated wrapped path to `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` and keep the `.json`/`.asc` pair next to the Metal bundle. No seeded CUDA reference bundle is checked in.【scripts/fastpq/wrap_benchmark.py:1】
| Telemetry check | Validate the Prometheus surface reflects `device_class="<matrix>", backend="metal"` or explicit CPU mode | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` and copy the `telemetry::fastpq.execution_mode` log emitted at startup.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    | Explicit CPU drill | Document the deterministic CPU path for SRE playbooks | Run a short workload with `zk.fastpq.execution_mode = "cpu"` and capture the startup log so operators can rehearse the rollback procedure.【crates/iroha_config/src/parameters/user.rs:3964】
    | Trace capture (optional) | When profiling, capture dispatch traces so kernel lane/tile overrides are reviewable later | Rerun one parity test with `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` and attach the produced trace log to your release artefacts.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Archive the evidence with the release ticket and mirror the same checklist in `specs/fastpq_migration_guide.md` so staging/prod rollouts follow an identical playbook.【specs/fastpq_migration_guide.md:1】

### Release checklist enforcement

Add the following gates to every FASTPQ release ticket. Releases are blocked until all items are
complete and attached as signed artefacts.

1. **Sub-second proof metrics** — The canonical Metal benchmark capture
   (`fastpq_metal_bench_*.json`) must prove the 20 000-row workload (32 768 padded rows) finishes in
   <1 s. Concretely, the `benchmarks.operations` entry where `operation = "lde"` and the matching
   `report.operations` sample must show `gpu_mean_ms ≤ 950`. Runs that exceed the ceiling require
   investigation and a recapture before the checklist can be signed.
2. **Signed benchmark manifest** — After recording fresh Metal + CUDA bundles, run
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` to emit
   `artifacts/fastpq_bench_manifest.json` and the detached signature
   (`artifacts/fastpq_bench_manifest.sig`). Attach both files plus the public key fingerprint to the
   release ticket so reviewers can verify the digest and signature independently.【xtask/src/fastpq.rs:1】
3. **Evidence attachments** — Store the raw benchmark JSON, stdout log (or Instruments trace, when
   captured), and the manifest/signature pair with the release ticket. The checklist is only
   considered green when the ticket links to those artefacts and the on-call reviewer confirms the
   digest recorded in `fastpq_bench_manifest.json` matches the uploaded files.

## V1 release hardening and documentation
- The production pipeline ships by default with no backend compatibility path.
- Reproducible builds (pin toolchains, container images).
- **TODO:** extend fuzzing across larger witnessed-transfer batches and malformed
  generic SMT paths without expanding the V1 operation enum.
- Prover-level smoke tests prove witnessed remittance transfers and assert that
  opaque governance metadata, alone or mixed with transfers, fails closed under
  the generic transfer-state profile.【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Runbooks with alert thresholds, remediation procedures, capacity planning guidelines.
- Cross-architecture proof replay (x86_64, ARM64) in CI.

### Bench manifest & release gate

Release evidence now includes a deterministic manifest covering both Metal and
CUDA benchmark bundles. Run:

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```

The command validates the wrapped bundles, enforces latency/speedup thresholds,
emits BLAKE3 + SHA-256 digests, and (optionally) signs the manifest with an
Ed25519 key so release tooling can verify provenance. See
`xtask/src/fastpq.rs`/`xtask/src/main.rs` for the implementation.

> **Note:** Metal bundles that omit `benchmarks.poseidon_microbench` now cause
> the manifest generation to fail. Re-run `scripts/fastpq/wrap_benchmark.py`
> (and `scripts/fastpq/export_poseidon_microbench.py` if you need a standalone
> summary) whenever the Poseidon evidence is missing so release manifests
> always capture the scalar-vs-default comparison.【xtask/src/fastpq.rs:409】

The `--matrix` flag (defaulting to `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
when present) loads the cross-device medians captured by
`scripts/fastpq/capture_matrix.sh`. The manifest encodes the 20 000-row floor and
per-operation latency/speedup limits for every device class, so bespoke
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` overrides are no
longer required unless you are debugging a specific regression.

Refresh the matrix by appending wrapped benchmark paths to the
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` lists and running
`scripts/fastpq/capture_matrix.sh`. The script snapshots the per-device medians,
emits the consolidated `matrix_manifest.json`, and prints the relative path that
`cargo xtask fastpq-bench-manifest` will consume. These capture lists, wrapped
GPU bundles, and the generated matrix are release evidence; none is checked into
this repository, so each release must supply fresh lab captures. The matrix
records the `operation_filters` seen for each device label, and signed bench
manifests carry those lists as `matrix_operation_filters`, preventing a focused
FFT/LDE/Poseidon run from being mistaken for an `all`-operations capture.
The release pipeline turns that manifest into
`fastpq_rollout_summary.{json,md}` whenever it archives a rollout bundle, so
release tickets can attach a compact reviewer view of each archived Metal/CUDA
lane without losing the underlying manifest as the source of truth. That same
archive step now also records the copied rollout bundle roots and summary paths
under `release_manifest.json.evidence.fastpq`, closing the machine-readable
link from the release manifest back to the Stage 7 rollout evidence.【scripts/fastpq/capture_matrix.sh:1】【xtask/src/fastpq.rs:1】【scripts/run_release_pipeline.py:1】

---

## Critique Summary & Open Actions

## Stage 7 — Fleet Adoption & Rollout Evidence

Stage 7 takes the prover from “documented & benchmarked” (Stage 6) to
“default-ready for production fleets”. The focus is on telemetry ingestion,
cross-device capture parity, and operator evidence bundles so GPU acceleration
can be mandated deterministically.

- **Stage7-1 — Fleet telemetry ingestion & SLOs.** Production dashboards
  (`dashboards/grafana/fastpq_acceleration.json`) must be wired to live
  Prometheus feeds with Alertmanager coverage for queue-depth stalls,
  zero-fill regressions, and silent CPU fallbacks. The alert pack stays under
  `dashboards/alerts/fastpq_acceleration_rules.yml` and feeds the same evidence
  bundle required in Stage 6.【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  The dashboard now exposes template variables for `device_class`, `chip_family`,
  and `gpu_kind`, letting operators pivot Metal adoption by the exact matrix
  label (e.g., `apple-m4-max`), by Apple chip family, or by discrete vs.
  integrated GPU classes without editing the queries.
  macOS `iroha3d` nodes built with `--features fastpq-gpu` now emit
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (busy/overlap ratios), and
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (limit, max_in_flight, dispatch_count, window_seconds) so the dashboards and
  Alertmanager rules can read Metal semaphore duty-cycle/headroom directly from
  Prometheus without waiting for a benchmark bundle. Hosts now export
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` and
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` whenever
  the LDE helper zeros GPU evaluation buffers, and Alertmanager gained the
  `FastpqQueueHeadroomLow` (headroom < 1 for 10 m) and
  `FastpqZeroFillRegression` (>0.40 ms over 15 m) rules so queue headroom and
  zero-fill regressions page operators immediately instead of waiting for the
  next wrapped benchmark. A new `FastpqCpuFallbackBurst` page-level alert tracks
  GPU requests that land on the CPU backend for more than 5 % of the workload,
  forcing operators to capture evidence and root-cause transient GPU failures
  before retrying the rollout.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  The SLO set now also enforces the ≥50 % Metal duty-cycle target via the
  `FastpqQueueDutyCycleDrop` rule, which averages
  `fastpq_metal_queue_ratio{metric="busy"}` over a rolling 15-minute window and
  warns whenever GPU work is still being scheduled but a queue fails to keep the
  required occupancy. This keeps the live telemetry contract aligned with the
  benchmark evidence before GPU lanes are mandated.【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Stage7-2 — Cross-device capture matrix.**
  `scripts/fastpq/capture_matrix.sh` builds
  `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` from the per-device
  capture lists under `artifacts/fastpq_benchmarks/matrix/devices/`. The lists,
  wrapped bundles, and matrix are generated release outputs rather than
  repository fixtures. When supplied, `cargo xtask fastpq-bench-manifest` loads
  the matrix, enforces the 20 000-row floor, and applies its per-device
  latency/speedup limits before a release bundle is approved.【scripts/fastpq/capture_matrix.sh:1】【xtask/src/fastpq.rs:1】
Aggregated instability reasons now ship alongside the matrix: pass
`--reason-summary-out` to `scripts/fastpq/geometry_matrix.py` to emit a
JSON histogram of failure/warning causes keyed by host label and source
summary, so Stage7-2 reviewers can see CPU fallbacks or missing telemetry at
a glance without scanning the full Markdown table. The same helper now
accepts `--host-label chip_family:Chip` (repeat for multiple keys) so the
Markdown/JSON outputs include curated host label columns instead of burying
that metadata in the raw summary, making it trivial to filter OS builds or
Metal driver versions when compiling the Stage7-2 evidence bundle.【scripts/fastpq/geometry_matrix.py:1】
Geometry sweeps also stamp ISO8601 `started_at` / `completed_at` fields into the
summary, CSV, and Markdown outputs so capture bundles can prove the window for
each host when Stage7-2 matrices merge multiple lab runs.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` now stitches the geometry matrix together with
`row_usage/*.json` snapshots into a single Stage7 bundle (`stage7_bundle.json`
+ `stage7_geometry.md`), validating transfer ratios via
`validate_row_usage_snapshot.py` and persisting host/env/reason/source summaries
so rollout tickets can attach one deterministic artefact instead of juggling
per-host tables.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Stage7-3 — Operator adoption evidence & rollback drills.** The new
  `specs/fastpq_rollout_playbook.md` describes the artefact bundle
  (`fastpq_bench_manifest.json`, wrapped Metal/CUDA captures, Grafana export,
  Alertmanager snapshot, rollback logs) that must accompany every rollout ticket
  plus the staged (pilot → ramp → default) timeline and forced fallback drills.
  `ci/check_fastpq_rollout.sh` validates a supplied bundle locally or from release
  automation. The release pipeline can pull the same
  bundles into `artifacts/releases/<version>/fastpq_rollouts/…` via
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, ensuring the
  signed manifests and rollout evidence stay together. No reference rollout
  bundle or dedicated FastPQ rollout workflow is checked in; release automation
  must pass the generated bundle path explicitly.

### Stage7 FFT queue fan-out

`crates/fastpq_prover/src/metal.rs` now instantiates a `QueuePolicy` that
automatically spawns multiple Metal command queues whenever the host reports a
discrete GPU. Integrated GPUs keep the single-queue path
(`MIN_QUEUE_FANOUT = 1`), while discrete devices default to two queues and only
fan out when a workload covers at least 16 columns. Both heuristics can be tuned
via the new `FASTPQ_METAL_QUEUE_FANOUT` and `FASTPQ_METAL_COLUMN_THRESHOLD`
environment variables, and the scheduler round-robins FFT/LDE batches across the
active queues before issuing the paired post-tiling dispatch on the same queue
to preserve ordering guarantees.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
Node operators no longer need to export those env vars manually: the
`iroha_config` profile exposes `fastpq.metal_queue_fanout` and
`fastpq.metal_queue_column_threshold`, and `irohad` applies them via
`fastpq_prover::set_metal_queue_policy` before the Metal backend initialises so
fleet profiles stay reproducible without bespoke launch wrappers.【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Inverse FFT batches now stick to a single queue whenever the workload only just
hits the fan-out threshold (e.g., the 16-column lane-balanced capture), which
restores ≥1.0× parity for WP2-D while leaving large-column FFT/LDE/Poseidon
dispatches on the multi-queue path.【crates/fastpq_prover/src/metal.rs:2018】

Helper tests exercise the queue-policy clamps and parser validation so CI can
prove the Stage 7 heuristics without requiring GPU hardware on every builder,
and the GPU-specific tests force fan-out overrides to keep replay coverage in
sync with the new defaults.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Stage7-1 Device Labels & Alert Contract

`scripts/fastpq/wrap_benchmark.py` now probes `system_profiler` on macOS capture
hosts and records hardware labels in every wrapped benchmark so Fleet telemetry
and the capture matrix can pivot by device without bespoke spreadsheets. A
20 000-row Metal capture now carries entries such as:

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

These labels are ingested along with `benchmarks.zero_fill_hotspots` and
`benchmarks.metal_dispatch_queue` so the Grafana snapshot, capture matrix
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`), and Alertmanager
evidence all agree on the hardware class that produced the metrics. The
`--label` flag still allows manual overrides when a lab host lacks
`system_profiler`, but the auto-probed identifiers now cover Apple M1–M4 and
discrete PCIe GPUs out of the box.【scripts/fastpq/wrap_benchmark.py:1】

Linux captures receive the same treatment: `wrap_benchmark.py` now inspects
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi`, and `lspci` so CUDA and OpenCL runs
derive `cpu_model`, `gpu_model`, and a canonical `device_class` (`xeon-rtx-sm80`
for the Stage 7 CUDA host, `neoverse-mi300` for the MI300A lab). Operators can
still override the auto-detected values, but Stage 7 evidence bundles no longer
require manual edits to tag Xeon/Neoverse captures with the correct device
metadata.

At runtime, each host sets `fastpq.device_class`, `fastpq.chip_family`, and
`fastpq.gpu_kind` (or the corresponding `FASTPQ_*` environment variables) to the
same matrix labels that appear in the capture bundle so Prometheus export
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` and
the FASTPQ Acceleration dashboard can filter by any of the three axes. The
Alertmanager rules aggregate over the same label set, letting operators chart
adoption, downgrades, and fallbacks per hardware profile instead of a single
fleet-wide ratio.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

The telemetry SLO/alert contract now ties the captured metrics back to the Stage 7
gates. The table below summarises the signals and enforcement points:

| Signal | Source | Target / Trigger | Enforcement |
| ------ | ------ | ---------------- | ----------- |
| GPU adoption ratio | Prometheus `fastpq_execution_mode_total{requested="gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95 % of per-(device_class, chip_family, gpu_kind) explicit GPU resolutions must land on `resolved="gpu", backend="metal"`; page when any triplet drops below 50 % over 15 m | `FastpqMetalDowngrade` alert (page)【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| Backend gap | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Must remain at 0 for every triplet; warn after any sustained (>10 m) bursts | `FastpqBackendNoneBurst` alert (warning)【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| GPU fail-closed gap | Prometheus `fastpq_execution_mode_total{requested="gpu", backend!="metal", device_class="…", chip_family="…", gpu_kind="…"}` | Must remain at 0 for production GPU cohorts; page when explicit GPU startup cannot resolve to a GPU backend for ≥10 m | `FastpqCpuFallbackBurst` alert (page)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| Metal queue duty cycle | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | Rolling 15 m average must stay ≥50 % whenever GPU jobs are queued; warn when utilisation drops below target while GPU requests persist | `FastpqQueueDutyCycleDrop` alert (warning)【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| Queue depth & zero-fill budget | Wrapped benchmark `metal_dispatch_queue` and `zero_fill_hotspots` blocks | `max_in_flight` must stay at least one slot below `limit` and LDE zero-fill mean must stay ≤0.4 ms (≈80 GB/s) for the canonical 20 000-row trace; any regression blocks the rollout bundle | Reviewed via `scripts/fastpq/wrap_benchmark.py` output and attached to the Stage 7 evidence bundle (`specs/fastpq_rollout_playbook.md`). |
| Runtime queue headroom | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` for every triplet; warn after 10 m without headroom | `FastpqQueueHeadroomLow` alert (warning)【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| Runtime zero-fill latency | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | Latest zero-fill sample must remain ≤0.40 ms (Stage 7 limit) | `FastpqZeroFillRegression` alert (page)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |

The wrapper enforces the zero-fill row directly. Pass
`--require-zero-fill-max-ms 0.40` to `scripts/fastpq/wrap_benchmark.py` and it
will fail when the bench JSON lacks zero-fill telemetry or when the hottest
zero-fill sample exceeds the Stage 7 budget, preventing rollout bundles from
shipping without the mandated evidence.【scripts/fastpq/wrap_benchmark.py:1008】

#### Stage 7-1 alert-handling checklist

Every alert listed above feeds a specific on-call drill so operators gather the
same artefacts that the release bundle requires:

1. **`FastpqQueueHeadroomLow` (warning).** Run an instantaneous Prometheus query
   for `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` and
   capture the Grafana “Queue headroom” panel from the `fastpq-acceleration`
   board. Record the query result in
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   together with the alert ID so the release bundle proves the warning was
   acknowledged before the queue starved.【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (page).** Inspect
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` and, if the metric is
   noisy, rerun `scripts/fastpq/wrap_benchmark.py` on the most recent bench JSON
   to refresh the `zero_fill_hotspots` block. Attach the promQL output,
   screenshots, and refreshed bench file to the rollout directory; this creates
   the same evidence that `ci/check_fastpq_rollout.sh` expects during release
   validation.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (page).** Confirm that
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` exceeds the 5 %
   floor, then sample `irohad` logs for the corresponding downgrade messages
   (`telemetry::fastpq.execution_mode resolved="cpu"`). Store the promQL dump
   plus log excerpts in `metrics_cpu_fallback.prom`/`rollback_drill.log` so the
   bundle demonstrates both the impact and the operator acknowledgement.
4. **Evidence packaging.** After any alert clears, rerun the Stage 7-3 steps in
   the rollout playbook (Grafana export, alert snapshot, rollback drill) and
   revalidate the bundle via `ci/check_fastpq_rollout.sh` before reattaching it
   to the release ticket.【specs/fastpq_rollout_playbook.md:114】

Operators who prefer automation can run
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
to query the Prometheus API for the queue headroom, zero-fill, and CPU fallback
metrics listed above; the helper writes the captured JSON (prefixed with the
original promQL) into `metrics_headroom.prom`, `metrics_zero_fill.prom`, and
`metrics_cpu_fallback.prom` under the chosen rollout directory so those files
can be attached to the bundle without manual curl invocations.

`ci/check_fastpq_rollout.sh` now enforces the queue headroom and zero-fill
budget directly. It parses each `metal` bench referenced by
`fastpq_bench_manifest.json`, inspects
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` and
`benchmarks.zero_fill_hotspots[]`, and fails the bundle when headroom drops
below one slot or when any LDE hotspot reports `mean_ms > 0.40`. This keeps the
Stage 7 telemetry guard in CI, matching the manual review performed on the
Grafana snapshot and release evidence.【ci/check_fastpq_rollout.sh#L1】
As part of the same validation pass the script now insists that every wrapped
benchmark carries the auto-detected hardware labels (`metadata.labels.device_class`
and `metadata.labels.gpu_kind`). Bundles missing those labels fail immediately,
guaranteeing that release artefacts, Stage7-2 matrix manifests, and runtime
dashboards all refer to the exact same device-class names.

The Grafana “Latest Benchmark” panel and associated rollout bundle now quote the
`device_class`, zero-fill budget, and queue-depth snapshot so on-call engineers
can correlate production telemetry with the exact capture class used during sign
off. Future matrix entries inherit the same labels, meaning the Stage7-2 device
lists and the Prometheus dashboards share a single naming scheme for Apple M4,
M3 Max, and upcoming MI300/RTX captures.

### Stage7-1 Fleet telemetry runbook

Follow this checklist before enabling GPU lanes by default so fleet telemetry
and Alertmanager rules mirror the same evidence captured during release prep:

1. **Label capture and runtime hosts.** `python3 scripts/fastpq/wrap_benchmark.py`
   already emits `metadata.labels.device_class`, `chip_family`, and `gpu_kind`
   for every wrapped JSON. Keep those labels in sync with
   `fastpq.{device_class,chip_family,gpu_kind}` (or the
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) inside `iroha_config`
   so runtime metrics publish
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   and the `fastpq_metal_queue_*` gauges with the same identifiers that show up
   in `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. When staging a new
   class, regenerate the matrix manifest via
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   so CI and dashboards understand the additional label.
2. **Verify queue gauges and adoption metrics.** Run `iroha3d` built with `--features fastpq-gpu`
   on the Metal hosts and scrape the telemetry endpoint to confirm live queue
   gauges are exporting:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```

   The first command proves the semaphore sampler is emitting the `busy`,
   `overlap`, `limit`, and `max_in_flight` series and the second shows whether
   each device class is resolving to `backend="metal"` or falling back to
   `backend="cpu"`. Wire the scrape target through Prometheus before
   importing the dashboard so Grafana can plot the fleet view immediately.
3. **Install the dashboard + alert pack.** Import
   `dashboards/grafana/fastpq_acceleration.json` into Grafana (retain the
   built-in Device Class, Chip Family, and GPU Kind template variables) and load
   `dashboards/alerts/fastpq_acceleration_rules.yml` into Alertmanager together
   with its unit test fixture. The rule pack ships a `promtool` harness; run
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   whenever the rules change to prove `FastpqMetalDowngrade` and
   `FastpqBackendNoneBurst` still fire at the documented thresholds.
4. **Gate releases with the evidence bundle.** Keep
   `specs/fastpq_rollout_playbook.md` handy while generating a rollout
   submission so every bundle carries the wrapped benchmarks, Grafana export,
   alert pack, queue telemetry proof, and rollback logs. CI already enforces the
   contract: `make check-fastpq-rollout` (or invoking
   `ci/check_fastpq_rollout.sh --bundle <path>`) validates the bundle, re-runs
   the alert tests, and refuses to sign off when queue headroom or zero-fill
   budgets regress.
5. **Tie alerts back to remediation.** When Alertmanager pages, use the Grafana
   board and the raw Prometheus counters from step 2 to confirm whether
   downgrades stem from queue starvation, CPU fallbacks, or backend=none bursts.
The runbook lives in
this document plus `specs/fastpq_rollout_playbook.md`; update the
release ticket with the relevant `fastpq_execution_mode_total`,
`fastpq_metal_queue_ratio`, and `fastpq_metal_queue_depth` excerpts together
with links to the Grafana panel and the alert snapshot so reviewers can see
exactly which SLO triggered.

### WP2-E — Stage-by-stage Metal profiling snapshot

`scripts/fastpq/src/bin/metal_profile.rs` summarizes the wrapped Metal captures
so the sub-900 ms target can be tracked over time (run
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
The new Markdown helper
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
generates the stage tables below (it prints the Markdown along with a textual
summary so WP2-E tickets can embed the evidence verbatim). Two captures are tracked
right now:

> **New WP2-E instrumentation:** `fastpq_metal_bench --gpu-probe ...` now emits a
> detection snapshot (requested/resolved execution mode, `FASTPQ_GPU`
> overrides, detected backend, and the enumerated Metal devices/registry ids)
> before any kernels run. Capture this log whenever a forced GPU run still
> falls back to the CPU path so the evidence bundle records which hosts see
> `MTLCopyAllDevices` return zero and which overrides were in effect during the
> benchmark.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】

> **Stage capture helper:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> now drives `fastpq_metal_bench` for FFT, LDE, and Poseidon individually,
> stores the raw JSON outputs under per-stage directories, and emits a single
> `stage_profile_summary.json` bundle that records CPU/GPU timings, queue depth
> telemetry, column-staging stats, kernel profiles, and the associated trace
> artefacts. Pass `--stage fft --stage lde --stage poseidon` to target a subset,
> `--trace-template "Metal System Trace"` to pick a specific xctrace template,
> and `--trace-dir` to route `.trace` bundles to a shared location. Attach the
> summary JSON plus the generated trace files to every WP2-E issue so reviewers
> can diff queue occupancy (`metal_dispatch_queue.*`), overlap ratios, and the
> captured launch geometry across runs without manually spelunking multiple
> `fastpq_metal_bench` invocations.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Queue/staging evidence helper (2026-05-09):** `scripts/fastpq/profile_queue.py` now
> ingests one or more `fastpq_metal_bench` JSON captures and emits both a Markdown table and
> a machine-readable summary (`--markdown-out/--json-out`) so queue depth, overlap ratios, and
> host-side staging telemetry can ride alongside every WP2-E artefact. Run, for example,
> `python3 scripts/fastpq/profile_queue.py <poseidon-capture.json> <full-capture.json> --json-out <evidence-dir>/queue.json --markdown-out <evidence-dir>/queue.md` and attach both generated files to the release ticket. No queue-profile capture is checked in.
> The helper also surfaces the Poseidon column-batch telemetry (`columns`, `batches`, and
> `fallbacks`) inside both the Markdown table and the JSON summary. Reviewers use the separate
> queue, staging, and kernel-profile blocks for actual command overlap and use this summary to spot
> top-level batch fallback without opening the raw capture.【scripts/fastpq/profile_queue.py:1】

> **Stage profile summariser (2026-05-30):** `scripts/fastpq/stage_profile_report.py` consumes
> the `stage_profile_summary.json` bundle emitted by `cargo xtask fastpq-stage-profile` and
> renders both Markdown and JSON summaries so WP2-E reviewers can copy evidence into tickets
> without manually transcribing timings. Invoke
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> to produce deterministic tables listing GPU/CPU means, speedup deltas, trace coverage, and
> telemetry gaps per stage. The JSON output mirrors the table and records per-stage issue tags
> (`trace missing`, `queue telemetry missing`, etc.) so governance automation can diff the host
> runs referenced in WP2-E.1 through WP2-E.6.
> **Host/device overlap guard (2026-06-04):** `scripts/fastpq/profile_queue.py` now annotates
> FFT/LDE/Poseidon wait ratios alongside the per-stage flatten/wait millisecond totals and emits an
> issue whenever `--max-wait-ratio <threshold>` detects poor overlap. Use
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> to capture both the Markdown table and the JSON bundle with explicit wait ratios so WP2-E.5 tickets
> can show whether the double-buffering window kept the GPU fed. The plain-text console output also
> lists the per-phase ratios to make on-call investigations easier.
> **Telemetry guard + run status (2026-06-09):** `fastpq_metal_bench` now emits a `run_status` block
> (backend label, dispatch count, reasons) and the new `--require-telemetry` flag fails the run
> whenever GPU timings or queue/staging telemetry are missing. `profile_queue.py` renders the run
> status as a dedicated column and surfaces non-`ok` states in the issue list, and
> `launch_geometry_sweep.py` threads the same state into warnings/classification so matrices can no
> longer admit captures that silently fell back to CPU or skipped queue instrumentation.
> **Poseidon/LDE auto-tuning (2026-06-12):** `metal_config::poseidon_batch_multiplier()` now scales
> with the Metal working-set hints and `lde_tile_stage_target()` records the resolved tile depth,
> capped at the eight radix-2 stages that fit the 256-word threadgroup tile.
> The applied multiplier and tile limit are included in the `metal_heuristics` block of
> `fastpq_metal_bench` outputs and rendered by `scripts/fastpq/metal_capture_summary.py`, so WP2-E
> bundles record the exact internal batch geometry used in each capture without digging through raw JSON.【crates/fastpq_prover/src/metal_config.rs:304】【crates/fastpq_prover/src/metal.rs:824】【scripts/fastpq/metal_capture_summary.py:1】

| Label | Dispatch | Busy | Overlap | Max Depth | FFT flatten | FFT wait | FFT wait % | LDE flatten | LDE wait | LDE wait % | Poseidon flatten | Poseidon wait | Poseidon wait % | Batch columns | Column batches | Batch fallbacks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20 k snapshot (pre-override)

`fastpq_metal_bench_20k_latest.json`

| Stage | Columns | Input len | GPU mean (ms) | CPU mean (ms) | GPU share | Speedup | Δ CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32 768 | 130.986 ms (115.761–167.755) | 112.616 ms (95.335–132.929) | 2.4 % | 0.860× | −18.370 |
| IFFT | 16 | 32 768 | 129.296 ms (111.127–142.955) | 158.144 ms (126.847–237.887) | 2.4 % | 1.223× | +28.848 |
| LDE | 16 | 262 144 | 1 570.656 ms (1 544.397–1 584.502) | 1 752.523 ms (1 548.807–2 191.930) | 29.2 % | 1.116× | +181.867 |
| Poseidon | 16 | 524 288 | 3 548.329 ms (3 519.881–3 576.041) | 3 642.706 ms (3 539.055–3 758.279) | 66.0 % | 1.027× | +94.377 |

Key observations:

1. The GPU total is 5.379 s, which is **4.48 s over** the 900 ms goal. Poseidon
   hashing still dominates the runtime (≈66 %) with the LDE kernel in second
   place (≈29 %), so WP2-E needs to improve the Poseidon dispatch geometry and
   the LDE memory residency/tiling plan before CPU fallbacks disappear.
2. FFT remains a regression (0.86×) even though IFFT is >1.22× over the scalar
   path. We need a launch-geometry sweep
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) to understand
   whether the FFT occupancy can be salvaged without hurting the already-better
   IFFT timings. The `scripts/fastpq/launch_geometry_sweep.py` helper now drives
   these experiments end-to-end: pass comma-separated overrides (for example,
   `--fft-columns 16,32 --queue-fanout 1,2` and
   `--poseidon-lanes auto,256`) and it will invoke
   `fastpq_metal_bench` for every combination, store the JSON payloads under
   `artifacts/fastpq_geometry/<timestamp>/`, and persist a `summary.json` bundle
   describing each run’s queue ratios, FFT/LDE launch picks, GPU vs CPU timings,
   and the host metadata (hostname/label, platform triple, detected device
   class, GPU vendor/model) so cross-device comparisons have deterministic
   provenance. The helper now also writes `reason_summary.json` next to the
   summary by default, using the same classifier as the geometry matrix to roll
   up CPU fallbacks and missing telemetry. Use `--host-label staging-m3` to tag
   captures from shared labs.
   The companion `scripts/fastpq/geometry_matrix.py` tool now ingests one or
   more summary bundles (`--summary hostA/summary.json --summary hostB/summary.json`)
   and emits Markdown/JSON tables that label every launch shape as *stable*
   (FFT/LDE/Poseidon GPU timings captured) or *unstable* (timeout, CPU fallback,
   non-Metal backend, or missing telemetry) alongside the host columns. The
   tables now include the resolved `execution_mode`/`gpu_backend` plus a
   `Reason` column so CPU fallbacks and missing GPU timings are obvious in
   Stage 7 matrices even when timing blocks are present; a summary line counts
   the stable vs total runs. Pass `--operation fft|lde|poseidon_hash_columns`
   when the sweep needs to isolate a single stage (for example, to profile
   Poseidon separately) and keep `--extra-args` free for bench-specific flags.
   The helper accepts any
   command prefix (defaulting to `cargo run … fastpq_metal_bench`) plus optional
   `--halt-on-error` / `--timeout-seconds` guards so performance engineers can
   reproduce the sweep on different machines while collecting comparable,
   multi-device evidence bundles for Stage 7.
3. `metal_dispatch_queue` reported `dispatch_count = 0`, so queue occupancy
   telemetry was missing even though GPU kernels ran. The Metal runtime now uses
   acquire/release fences for the queue/column-staging toggles so worker threads
   observe the instrumentation flags, and the geometry matrix report calls out
   unstable launch shapes whenever FFT/LDE/Poseidon GPU timings are absent. Keep
   attaching the Markdown/JSON matrix to WP2-E tickets so reviewers can see
   which combinations are still failing once queue telemetry becomes available.
   The `run_status` guard and `--require-telemetry` flag now fail the capture
   whenever GPU timings are missing or queue/staging telemetry is absent, so
   dispatch_count=0 runs can no longer slip into WP2-E bundles unnoticed.
   `fastpq_metal_bench` now exposes `--require-gpu`, and
   `launch_geometry_sweep.py` enables it by default (opt out with
   `--allow-cpu-fallback`) so CPU fallbacks and Metal detection failures abort
   immediately instead of polluting Stage 7 matrices with non-GPU telemetry.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Zero-fill metrics previously vanished for the same reason; the fencing fix
   keeps host instrumentation live, so the next capture should include the
   `zero_fill` block without synthetic timings.

#### 20 k snapshot with `FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

| Stage | Columns | Input len | GPU mean (ms) | CPU mean (ms) | GPU share | Speedup | Δ CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32 768 | 79.951 ms (65.645–93.193) | 83.289 ms (59.956–107.585) | 0.3 % | 1.042× | +3.338 |
| IFFT | 16 | 32 768 | 78.605 ms (69.986–83.726) | 93.898 ms (80.656–119.625) | 0.3 % | 1.195× | +15.293 |
| LDE | 16 | 262 144 | 657.673 ms (619.219–712.367) | 669.537 ms (619.716–723.285) | 2.1 % | 1.018× | +11.864 |
| Poseidon | 16 | 524 288 | 30 004.898 ms (27 284.117–32 945.253) | 29 087.532 ms (24 969.810–33 020.517) | 97.4 % | 0.969× | −917.366 |

Observations:

1. Even with `FASTPQ_GPU=gpu`, this capture still reflects CPU fallback:
   ~30 s per iteration with `metal_dispatch_queue` stuck at zero. When the
   override is set but the host can’t discover a Metal device, the CLI now exits
   before running any kernels and prints the requested/resolved mode plus the
   backend label so engineers can tell whether detection, entitlements, or the
   metallib lookup caused the downgrade. Run `fastpq_metal_bench --gpu-probe
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` to capture the enumeration log and
   fix the underlying detection issue before re-running the profiler.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. Zero-fill telemetry now records a real sample (18.66 ms over 32 MiB), proving
   the fencing fix works, but queue deltas remain absent until GPU dispatches
   succeed.
3. Because the backend keeps downgrading, the Stage 7 telemetry gate is still
   blocked: queue headroom evidence and poseidon overlap require a genuine GPU
   run.

These captures now anchor the WP2-E backlog. Next actions: gather profiler
flamecharts and queue logs (once the backend executes on the GPU), target the
Poseidon/LDE bottlenecks before revisiting FFT, and unblock the backend fallback
so Stage 7 telemetry has real GPU data.

### Strengths
- Incremental staging, trace-first design, transparent STARK stack.

### High-Priority Action Items
1. Implement packing/order fixtures and update AIR spec.
2. Design any future state or permission witness as a new fully constrained schema.
3. Keep generic SMT/non-membership examples outside the V1 trace contract.
4. Add Appendix A documenting soundness derivation and CI rejection methodology.

### Resolved Design Decisions
- ZK disabled (correctness-only) in P1; revisit in future stage.
- Permission-table membership, absent-key proofs, and generic delete semantics
  are outside V1; no reserved columns or compatibility variants remain.

Use this document as the canonical reference; update it alongside source code, fixtures, and appendices to avoid drift.

## Appendix A — Soundness Derivation

This appendix records the exact aggregate arithmetic implemented by
`fastpq_isi`. It is a deterministic parameter-selection calculation, not by
itself a security or production-qualification claim.

Implementation qualification gap (2026-08-29): native-STARK commitments and
Fiat--Shamir state use six independently domain-separated Goldilocks lanes, for
a canonical 384-bit encoding. FRI challenges, opened values, and folds use
`GoldilocksFp4V1 = Goldilocks[X]/(X^4 - 7)`. The frozen arithmetic inputs meet
the declared 128-bit target, but the result remains
`Unavailable(MissingProtocolSpecificQromReduction)`. An independently reviewed
reduction connecting this accounting to the complete FASTPQ adversary and a
multi-target review tied to final artifact digests are still required.

### Notation
- `N_trace = 2^k` — trace length after sorting and padding to a power of two.
- `b = 8` — blowup factor (`N_eval = N_trace × b`).
- `r = 2` — sole V1 FRI folding arity.
- `ℓ ≤ 18` — maximum number of binary FRI reductions.
- `q = 136` — deduplicated verifier queries selected by the calculator.
- `T = 54` — portable release proof targets included in the union bound.
- `Q ≤ 2^32` — declared quantum random-oracle query bound.
- `n = 384` — combined width of the six independent digest lanes.

The current protocol declares zero grinding bits. That fact is explicit in the
sole parameter record and contributes no hidden margin to the calculation.

### Analytic bound

For a candidate query count, the implemented exact dyadic upper bound is

```
p_sampling  = T × Q^2 / 2^(q × log2(b) / 2)
p_collision = T^2 × Q^3 / 2^n
p_total     = p_sampling + p_collision
```

The calculator requires `p_total < 2^-128` using integer arithmetic only. For
the frozen inputs and `q = 136`, the sampling denominator exponent is `204`,
its numerator is `54 × 2^64`, and the collision term is
`54^2 × 2^96 / 2^384`. Query count 136 is the least admitted multiple of eight
that passes; 128 queries fail the same exact comparison. This calculation is
necessary parameter evidence, but it does not discharge the independent
protocol-specific reduction or digest review.

### Rejection-sampling follow-up

**TODO:** add the planned Monte Carlo harness for the implemented 16-residue
composition and report its measured behavior as diagnostic evidence. It does
not replace the exact aggregate calculator or independent qualification.
Semantic adversarial coverage is deterministic instead: unsupported
operations/profile confusion, foreign batch-derived roots, challenge
cancellation, noncanonical numeric/key encodings, and alternate transfer paths
must each be rejected by dedicated regression tests.

## Appendix B — Domain-root derivation

Stage 0 pins the trace and evaluation generators to Poseidon-derived constants so all implementations share the same subgroups.

### Procedure
1. **Seed selection.** Absorb the UTF‑8 tag `fastpq:v1:domain_roots` into the Poseidon sponge used elsewhere in FASTPQ (state width = 3, rate = 2, eight full + 57 partial rounds, `x^7` S-box). Inputs reuse the `[len, limbs…]` encoding from `pack_bytes`.【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **LDE generator.** Compute `lde_root = g_base^{(p-1)/2^{lde_log_size}} mod p` and verify `lde_root^{2^{lde_log_size}} = 1` while the half-power is not 1.
3. **Trace generator.** Derive `trace_root = lde_root^blowup_factor`. Verify its exact `2^trace_log_size` order and this equality so an LDE index advance by `blowup_factor` is exactly multiplication by the trace generator.
4. **Coset selection.** Derive a deterministic nonzero `omega_coset` from the
   domain-root seed and reject candidates inside the LDE subgroup. Pin the
   accepted offset with the sole V1 parameter set.
### Reproduction and validation
- Tooling: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` emits either Rust snippets or a Markdown table (see `--format table`, `--seed`, `--filter`).【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- Tests: `sole_profile_has_final_binary_fp4_shape` and
  `domain_roots_are_coherent_and_coset_is_outside_lde_subgroup` keep
  the canonical parameter set aligned with the published constants, exact
  subgroup orders, the trace/LDE blowup relation, outside-subgroup coset,
  binary FRI arity, and degree-four challenge field, so `cargo test -p fastpq_isi`
  catches drift immediately.
  【crates/fastpq_isi/src/params.rs:337】
- Source of truth: update this table, `fastpq_isi/src/params.rs`, and the V1
  fixtures together if the first-release constants change.

## Appendix C — Commitment pipeline details

### Native-STARK commitment flow
V1 uses the same deterministic preparation pipeline in prover and verifier:
1. **Normalise transitions.** `trace::build_trace` consumes the already
   canonicalized batch, pads it to `N_trace = 2^{⌈log₂ rows⌉}`, and emits the
   fixed-order column vectors.【crates/fastpq_prover/src/trace.rs:612】
2. **Commit the preprocessing trace.**
   `digest::trace_commitment_from_trace` hashes each named base-trace column
   into a six-lane digest, folds a typed binary Merkle tree, then binds the
   parameter identity and exact row/padding/column shape into the final typed
   `GoldilocksDigest384V1` stored in `Proof::trace_commitment`.
   【crates/fastpq_prover/src/digest.rs:81】
3. **Prepare polynomial data once.** `trace::derive_polynomial_data`
   interpolates every column and materializes its deterministic CPU LDE. The
   backend reuses these coefficients and evaluations instead of performing a
   second transform for commitment hashing.【crates/fastpq_prover/src/trace.rs:1701】
4. **Commit coefficient columns.** The backend hashes each named coefficient
   vector into a six-lane leaf and folds those leaves under the typed `Trace`
   Merkle role to produce `trace_root`. Optional GPU Merkle dispatch must match
   the scalar result and uses the deterministic CPU fallback after an allowed
   runtime dispatch failure.【crates/fastpq_prover/src/backend.rs:2534】
5. **Bind and commit LDE/AIR material.** After absorbing `trace_root`, the
   transcript derives one base-field column-mix coefficient per LDE column. The
   backend combines those columns row-wise, commits `lde_root`, commits the
   row-major AIR trace and composition vectors, and binds all four 384-bit roots
   before FRI challenges are sampled.【crates/fastpq_prover/src/backend.rs:2595】

The verifier recomputes the same commitments before accepting openings, so
mismatches abort the proof. The
`tests/fixtures/ordering_hash.json` regression pins the sorted-row input, while
`tests/trace_commitment.rs` independently checks canonical encoding,
determinism, and separation of the resulting commitments. The JSON fixture
changes only under an explicit `FASTPQ_UPDATE_FIXTURES=1` regeneration.

### Poseidon fallback controls

- The prover now exposes a dedicated Poseidon pipeline override (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) so operators can mix GPU FFT/LDE with CPU Poseidon hashing on devices that fail to reach the Stage 7 <900 ms target. Supported values mirror the execution-mode knob (`auto`, `cpu`, `gpu`), defaulting to the global mode when unspecified. The runtime threads this value through the lane config (`FastpqPoseidonMode`) and propagates it into the prover (`Prover::canonical_with_modes`) so overrides are deterministic and auditable in config dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- Telemetry exports the resolved pipeline mode via the `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` counter. `sorafs`/operator dashboards can therefore confirm when a rollout is running the batched GPU path (`path="gpu"`) versus forced CPU execution (`path="cpu_forced"`) or a runtime downgrade (`path="cpu_fallback"`). The CLI probe installs automatically in `irohad`, so release bundles and live telemetry share the same evidence stream.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- Mixed-mode evidence is also stamped into every scoreboard via the existing adoption gate: the prover emits the resolved mode + path label for each batch, and the `fastpq_poseidon_pipeline_total` counter increments alongside the execution-mode counter whenever a proof lands. This satisfies WP2-E.6 by making brownouts visible and by providing a clean switch for deterministic downgrades while optimisation continues.【crates/fastpq_prover/src/trace.rs:1684】【specs/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` now parses Prometheus scrapes (Metal or CUDA) and embeds a `poseidon_metrics` summary inside every wrapped bundle. The helper filters the counter rows by `metadata.labels.device_class`, captures the matching `fastpq_execution_mode_total` samples, and fails the wrap when `fastpq_poseidon_pipeline_total` entries are missing so WP2-E.6 bundles always ship reproducible CUDA/Metal evidence instead of ad-hoc notes.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Deterministic mixed-mode policy (WP2-E.6)

1. **Detect GPU shortfall.** Flag any device-class whose Stage 7 capture or live Grafana snapshot shows Poseidon latency keeping the total proof time >900 ms while FFT/LDE stay below target. Operators annotate the capture matrix (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) and page the on-call when `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` stagnates while `fastpq_execution_mode_total{backend="metal"}` still records GPU FFT/LDE dispatches.【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **Flip to CPU Poseidon only for the affected hosts.** Set `zk.fastpq.poseidon_mode = "cpu"` (or `FASTPQ_POSEIDON_MODE=cpu`) in the host-local config alongside the fleet labels, keeping `zk.fastpq.execution_mode = "gpu"` so FFT/LDE continue to use the accelerator. Record the config diff in the rollout ticket and add the per-host override to the bundle as `poseidon_fallback.patch` so reviewers can replay the change deterministically.
3. **Prove the downgrade.** Scrape the Poseidon counter immediately after restarting the node:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   The dump must show `path="cpu_forced"` growing in lock-step with the GPU execution counter. Store the scrape as `metrics_poseidon.prom` next to the existing `metrics_cpu_fallback.prom` snapshot and capture the matching `telemetry::fastpq.poseidon` log lines in `poseidon_fallback.log`.
4. **Monitor & exit.** Keep alerting on `fastpq_poseidon_pipeline_total{path="cpu_forced"}` while optimisation work continues. Once a patch brings the per-proof runtime back under 900 ms on the test host, roll the config back to `auto`, re-run the scrape (showing `path="gpu"` again), and attach the before/after metrics to the bundle to close the mixed-mode drill.

**Telemetry contract.**

| Signal | PromQL / Source | Purpose |
|--------|-----------------|---------|
| Poseidon mode counter | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | Confirms CPU hashing is intentional and scoped to the flagged device-class. |
| Execution mode counter | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Proves FFT/LDE still run on GPU even while Poseidon downgrades. |
| Log evidence | `telemetry::fastpq.poseidon` entries captured in `poseidon_fallback.log` | Provides per-proof proof that the host resolved to CPU hashing with Reason `cpu_forced`. |

The rollout bundle must now include `metrics_poseidon.prom`, the config diff, and the log excerpt whenever mixed-mode is active so governance can audit the deterministic fallback policy alongside the FFT/LDE telemetry. `ci/check_fastpq_rollout.sh` already enforces the queue/zero-fill limits; the follow-up gate will sanity-check the Poseidon counter once mixed-mode lands in release automation.

The Stage 7 capture tooling already handles CUDA: wrap every `fastpq_cuda_bench` bundle with `--poseidon-metrics` (pointing at the scraped `metrics_poseidon.prom`) and the output now carries the same pipeline counters/resolution summary used on Metal so governance can verify CUDA fallbacks without bespoke tooling.【scripts/fastpq/wrap_benchmark.py:1】

### Column order
The hashing pipeline consumes columns in this deterministic order:
1. Selector flags: `s_active`, `s_transfer`, `s_meta_set`.
2. Packed limb columns (each zero-padded to the trace length): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`.
3. Auxiliary scalars: `delta`, `metadata_hash_limb_0` through `metadata_hash_limb_7`, `dsid`, `slot`.
4. For batches containing Transfer rows, sparse Merkle witnesses for every level `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`. This group is absent for metadata-only batches.

`trace::column_hashes` walks the columns in exactly this order. Any schema or
order change is a proof/fixture hard cut and must regenerate the binary proof
fixture.【crates/fastpq_prover/src/trace.rs:474】

### Transcript domain tags
V1 fixes the Fiat–Shamir catalog below to keep challenge generation deterministic:

| Tag | Purpose |
| --- | ------- |
| `fastpq:v1:init` | Initialize from protocol version, parameter set, and `PublicIO`. |
| `fastpq:v1:trace_root` | Commit the trace Merkle root before column-mix challenges. |
| `fastpq:v1:column_mix:<i>` | Sample one base-field LDE column-mix coefficient per trace column. |
| `fastpq:v1:roots` | Commit the LDE and trace Merkle roots, in that order. |
| `fastpq:v1:alpha:<i>` | Sample one composition-polynomial challenge for each of the 16 residues (`i = 0..15`). |
| `fastpq:v1:air_roots` | Commit the AIR-trace and AIR-composition roots, in that order. |
| `fastpq:v1:beta:<round>` | Sample the folding challenge for each FRI round. |
| `fastpq:v1:fri_layer:<round>` | Commit the Merkle root for each FRI layer. |
| `fastpq:v1:fri:final` | Record the final FRI layer before opening queries. |
| `fastpq:v1:query_index:<counter>` | Derive deduplicated verifier query indices with rejection sampling. |
