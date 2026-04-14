# ZK Envelopes (Norito)

This page specifies Norito-encoded envelopes used by native verifiers in the
Iroha 2 codebase. Envelopes are versioned, deterministic, and designed to be
portable between components (clients, IVM, node).

Scope (current)
- IPA (transparent; no trusted setup): Polynomial opening proofs used for
  Halo2-like flows via a native verifier. Envelope type: `OpenVerifyEnvelope`.
- STARK (FRI-style): Multi-fold consistency proofs over a 2^k domain using
  SHA-256 Merkle commitments and a deterministic transcript.

Backends (tags)
- Standalone native IPA verifier entrypoint: `halo2/ipa/poly-open`
  - The envelope selects the concrete curve/backend with `curve_id`
    (`1 = Pallas`, `2 = Goldilocks`, `20 = BN254`).
- STARK (native): `stark/fri-v1/<profile>` (e.g., `stark/fri-v1/sha256-goldilocks-v1`)

General notes
- Norito encoding is used for the envelopes and their nested payloads. Unless
  otherwise specified, scalars are little-endian and sized as per struct types.
- Determinism: challenges are derived from fixed transcript labels and byte
  sequences; hashing is specified (SHA-256 for STARK envelopes; SHA3 for IPA).
- Size limits and validation: implementers must bound vector sizes and reject
  malformed payloads early (see code for current limits). The native verifier
  applies `StarkVerifierLimits` (envelope byte budget, domain/tag length, fold
  arity, queries, Merkle depth, auxiliary terms) inside
  `verify_stark_fri_envelope_with_limits`, with defaults used by the standard
  `verify_stark_fri_envelope` entrypoint.

## IPA: Polynomial Opening Envelope

Wire types (as implemented in `crates/iroha_zkp_halo2`)

- `IpaParams` (transparent parameters)
  - `version: u16` — format version, currently 1
  - `curve_id: u16` — backend identifier (`1 = Pallas`, `2 = Goldilocks`)
  - `n: u32` — vector length (power of two)
  - `g: Vec<[u8; 32]>` — generator vector encoded as compressed curve points
  - `h: Vec<[u8; 32]>` — generator vector encoded as compressed curve points
  - `u: [u8; 32]` — generator encoding as compressed curve point

- `IpaProofData`
  - `version: u16` — format version, currently 1
  - `l: Vec<[u8; 32]>` — per-round L commitments (compressed curve points)
  - `r: Vec<[u8; 32]>` — per-round R commitments
  - `a_final: [u8; 32]` — final reduced scalar for witness vector
  - `b_final: [u8; 32]` — final reduced scalar for public vector

- `PolyOpenPublic`
  - `version: u16` — format version, currently 1
  - `curve_id: u16` — backend identifier
  - `n: u32` — vector length
  - `z: [u8; 32]` — evaluation point (canonical field encoding)
  - `t: [u8; 32]` — claimed evaluation f(z)
  - `p_g: [u8; 32]` — commitment to coefficients under `g`

- `OpenVerifyEnvelope`
  - `params: IpaParams`
  - `public: PolyOpenPublic`
  - `proof: IpaProofData`
  - `transcript_label: String` — bound by both prover and verifier
  - `vk_commitment: Option<[u8; 32]>` — optional outer VK commitment bound into the transcript
  - `public_inputs_schema_hash: Option<[u8; 32]>` — optional schema hash bound into the transcript
  - `domain_tag: Option<[u8; 32]>` — optional caller-defined domain separator bound into the transcript

Verifier behavior (native IPA)
- Re-derives the public vector b = [1, z, z^2, …, z^{n-1}].
- Binds the claimed statement before the first Fiat-Shamir challenge:
  `transcript_label`, backend/`n`, `z`, `t`, `p_g`, and any present optional
  metadata fields (`vk_commitment`, `public_inputs_schema_hash`, `domain_tag`).
- Replays transcript rounds to fold generators and update Q.
- Checks the final relation holds with `(a_final, b_final)`.
- Deterministic transcript over SHA3-256 under crate-defined DST.
- Batch helpers: `iroha_zkp_halo2::batch::verify_open_batch` verifies multiple
  envelopes with default settings, while
  `verify_open_batch_with_options` accepts `BatchOptions` to force sequential
  execution or cap rayon parallelism via `Parallelism::{Sequential, Auto, Limited}`.

Example (Rust)
```rust
use iroha_zkp_halo2::{
    Params, PolyOpenTranscriptMetadata, Polynomial, PrimeField64, Transcript,
    norito_helpers as nh,
};
let n = 8; let params = Params::new(n).unwrap();
let coeffs = (0..n).map(|i| PrimeField64::from((i+1) as u64)).collect();
let poly = Polynomial::from_coeffs(coeffs);
let p_g = poly.commit(&params).unwrap();
let z = PrimeField64::from(3u64);
let mut tr = Transcript::new("IROHA-TEST-IPA");
let metadata = PolyOpenTranscriptMetadata {
    vk_commitment: Some([0x11; 32]),
    public_inputs_schema_hash: Some([0x22; 32]),
    domain_tag: Some([0x33; 32]),
};
let (proof, t) = poly.open_with_metadata(&params, &mut tr, z, p_g, metadata).unwrap();
let env = iroha_zkp_halo2::OpenVerifyEnvelope {
    params: nh::params_to_wire(&params),
    public: nh::poly_open_public::<iroha_zkp_halo2::backend::pallas::PallasBackend>(
        n,
        z,
        t,
        p_g,
    ),
    proof: nh::proof_to_wire(&proof),
    transcript_label: "IROHA-TEST-IPA".into(),
    vk_commitment: metadata.vk_commitment,
    public_inputs_schema_hash: metadata.public_inputs_schema_hash,
    domain_tag: metadata.domain_tag,
};
let bytes = norito::to_bytes(&env).unwrap();
```

Example (JSON-like, annotated)
```jsonc
{
  // OpenVerifyEnvelope
  "params": {
    "version": 1,
    "curve_id": 1,           // backend (Pallas)
    "n": 8,
    "g": ["0x...", "0x..."],        // compressed curve points
    "h": ["0x...", "0x..."],
    "u": "0x..."
  },
  "public": {
    "version": 1,
    "curve_id": 1,
    "n": 8,
    "z": "0x...",           // evaluation point encoding
    "t": "0x...",           // claimed evaluation f(z)
    "p_g": "0x..."          // commitment encoding
  },
  "proof": {
    "version": 1,
    "l": ["0x...", "0x..."], // per-round L commitments
    "r": ["0x...", "0x..."],
    "a_final": "0x...",
    "b_final": "0x..."
  },
  "transcript_label": "IROHA-TEST-IPA",
  "vk_commitment": "0x...",             // optional, null when omitted
  "public_inputs_schema_hash": "0x...", // optional, null when omitted
  "domain_tag": "0x..."                 // optional, null when omitted
}
```

## STARK: FRI-Style Multi-Fold Envelope

Hashing and transcript
- Leaves: `LEAF || u64_le(value)` hashed with SHA-256.
- Internal nodes: SHA-256(left || right).
- Per-layer challenge `r_k = H(label || params || root_k)` mapped to field, where `params` =
  `version || n_log2 || blowup_log2 || fold_arity || merkle_arity || hash_fn || queries ||
  len(domain_tag) || domain_tag`. The same prefix is used for query sampling.
- Field: Goldilocks-like prime `p = 2^64 - 2^32 + 1` (test backend). Hash selector `hash_fn`
  currently supports SHA-256 (`1`); Poseidon2 (`2`) is reserved and rejected by the native
  verifier.

Wire types (as implemented in `iroha_core::zk_stark`)

- `StarkFriParamsV1`
  - `version: u16` — format version, currently 1
  - `n_log2: u8` — log2 of evaluation domain size
  - `blowup_log2: u8` — log2 blowup before FRI folding (e.g., 3 for 8×)
  - `fold_arity: u8` — FRI arity (power-of-two; current backend supports 2)
  - `queries: u16` — expected query count (must match `proof.queries.len()`)
  - `merkle_arity: u8` — Merkle branching factor (binary only in v1)
  - `hash_fn: u8` — hash selector (`1 = SHA-256`, `2 = Poseidon2` reserved)
  - `domain_tag: String` — domain separator baked into the transcript/sampler

- `MerklePath`
  - `dirs: Vec<u8>` — direction bits (packed, low bit = lowest level)
  - `siblings: Vec<[u8; 32]>` — sibling hashes from leaf to root

- `StarkCommitmentsV1`
  - `version: u16`
  - `roots: Vec<[u8; 32]>` — layer roots from 0…L
  - `comp_root: Option<[u8; 32]>` — optional Merkle root over composition leaves (one per query index)

- `FoldDecommitV1`
  - `j: u32` — index at this layer
  - `y0: u64`, `y1: u64` — inputs at positions (2*j, 2*j+1)
  - `path_y0: MerklePath`, `path_y1: MerklePath`
  - `z: u64` — folded value `y0 + r_k*y1`
  - `path_z: MerklePath` — Merkle path for `z` under `roots[k+1]`

- `StarkProofV1`
  - `version: u16`
  - `commits: StarkCommitmentsV1`
  - `queries: Vec<Vec<FoldDecommitV1>>` — one chain per query
  - `comp_values: Option<Vec<StarkCompositionValueV1>>` — optional composition leaves

- `StarkCompositionValueV1`
  - `leaf: u64` — Merkle leaf recorded under `comp_root`
  - `constant: u64` — constant term
  - `z_coeff: u64` — coefficient applied to the final folded `z`
  - `aux_terms: Vec<StarkCompositionTermV1>` — additional wire/value contributions
  - `path: MerklePath` — inclusion proof under `comp_root`

- `StarkCompositionTermV1`
  - `wire_index: u32` — caller-defined wire ordering (must be strictly increasing)
  - `value: u64` — auxiliary value contributed by this wire
  - `coeff: u64` — coefficient multiplied with the value

- `StarkVerifyEnvelopeV1`
  - `params: StarkFriParamsV1`
  - `proof: StarkProofV1`
  - `transcript_label: String`

Limits and validation
- Bounds enforced by the native verifier: `n_log2 ≤ 24`, `queries ≤ 32`, `layers ≤ 32`,
  `merkle depth ≤ 32`, `aux_terms ≤ 64`, `domain_tag` length ≤ 64 bytes. `hash_fn` must be
  `1 (SHA-256)` and `merkle_arity`/`fold_arity` must both be `2`.
- Query sampling and per-round challenges are domain-separated by `domain_tag`, hash selector,
  blowup, fold arity, query count, and roots; mismatched headers or roots are rejected.
- Bad roots, broken Merkle paths, tampered folds, non-canonical field encodings, and
  query-count/hash-selector mismatches are covered by regression tests in
  `crates/iroha_core/tests/zk_stark.rs` (`stark_single_fold_roundtrip_ok_and_fail`).

Verifier behavior (native STARK)
- For each query, replays all folds:
  - Verifies `y0`, `y1` Merkle openings under `roots[k]` and `z` under `roots[k+1]`.
  - Checks `z == y0 + r_k*y1` in the field.
- If `comp_root` present, verifies the composition leaf/path and checks it matches
  `constant + z_coeff * z_final + Σ coeff_i * value_i`. Auxiliary terms must appear
  in strictly increasing `wire_index` order.
- Validation: query indices derive from the transcript label + params + roots; the verifier
  rejects mismatched `j`, missing folds, bad roots/paths, non-canonical field encodings,
  unsupported hash selectors, and mismatched query-count headers. Depth/size caps guard
  against oversized envelopes.

	Example (Rust)
	```rust
		use iroha_core::zk_stark::*;
		let n_log2 = 3u8; // domain size 8
		// Build layers 0..L (y0/y1 folds) and Merkle roots/paths externally
		let env = StarkVerifyEnvelopeV1 { /* fill params, proof, transcript */ };
		let bytes = norito::to_bytes(&env).unwrap();
		// Verify the raw envelope bytes with `verify_stark_fri_envelope(&bytes)`.
		// Note: `verify_backend(\"stark/fri-v1/*\", ...)` expects a Norito `OpenVerifyEnvelope` wrapper.
		```

	STARK via `OpenVerifyEnvelope` (consensus / `verify_backend`)
	- `ProofBox.bytes` (outer payload): Norito `OpenVerifyEnvelope` with:
	  - `backend = BackendTag::Stark`
	  - `circuit_id = "stark/fri-v1/<profile>:<circuit>"` (application-level identifier)
	  - `vk_hash = sha256(backend || vk_bytes)`
	  - `public_inputs = schema descriptor bytes` (stable policy-defined layout commitment)
	  - `proof_bytes = norito(StarkFriOpenProofV1 { version, public_inputs, envelope_bytes })`
	- `StarkFriOpenProofV1.public_inputs` carries the concrete public input values
	  (column-major 32-byte words) used for circuit/policy checks.
	- `StarkFriOpenProofV1.envelope_bytes` (inner payload): Norito `StarkVerifyEnvelopeV1`
	- `VerifyingKeyBox.bytes` (for `stark/fri-v1/*`): Norito `StarkFriVerifyingKeyV1`
	  containing the expected `circuit_id` and the FRI parameter set (`n_log2`, `blowup_log2`,
	  `fold_arity`, `queries`, `merkle_arity`, `hash_fn`).
	- The verifier enforces that the outer wrapper metadata is bound into the inner STARK
	  envelope (via `domain_tag`) and that the inner envelope parameters match the VK payload.

	Example (JSON-like, annotated)
	```jsonc
	{
  // StarkVerifyEnvelopeV1
  "params": {
    "version": 1,
    "n_log2": 3,             // domain size 8
    "blowup_log2": 3,        // blowup factor 8×
    "fold_arity": 2,         // binary FRI folds
    "queries": 1,            // one query chain
    "merkle_arity": 2,       // binary Merkle trees
    "hash_fn": 1,            // SHA-256 transcript
    "domain_tag": "fastpq:v1:fri"
  },
  "proof": {
    "version": 1,
    "commits": {
      "version": 1,
      "roots": [
        "0xroot_l0...",    // layer-0 root (trace)
        "0xroot_l1...",    // layer-1 root (folded)
        "0xroot_l2..."     // layer-2 root (final)
      ],
      "comp_root": "0xcomproot..." // optional composition root (final layer)
    },
    "queries": [
      [ // query 0 chain (layers 0->1->2)
        {
          "j": 0,
          "y0": 5,
          "y1": 8,
          "path_y0": { "dirs": "AA==", "siblings": ["0xsib0...", "0xsib1..."] },
          "path_y1": { "dirs": "AA==", "siblings": ["0xsib0...", "0xsib1..."] },
          "z": 29, // y0 + r0*y1 (mod p)
          "path_z": { "dirs": "AA==", "siblings": ["0xsib0..."] }
        },
        {
          "j": 0,
          "y0": 29,
          "y1": 42,
          "path_y0": { "dirs": "AA==", "siblings": ["0xsib0..."] },
          "path_y1": { "dirs": "AA==", "siblings": ["0xsib0..."] },
          "z": 113,
          "path_z": { "dirs": "AA==", "siblings": [] }
        }
      ]
    ],
    "comp_values": [
      // Composition leaf: constant + z_coeff * z_final + Σ coeff_i * value_i
      {
        "leaf": 227,             // composition leaf at final layer index
        "constant": 7,           // c
        "z_coeff": 2,            // a0 applied to z_final
        "aux_terms": [
          { "wire_index": 0, "value": 90, "coeff": 3 },
          { "wire_index": 1, "value": 42, "coeff": 5 }
        ],
        "path": { "dirs": "", "siblings": [] } // Merkle path into comp_root (single-leaf example)
      }
    ]
  },
  "transcript_label": "TEST-STARK"
}
```

- All structs carry `version` fields to enable evolution.
- Future updates may add fields or new composition profiles; keep existing
  behavior stable for existing versions.
- Golden vector: `crates/iroha_core/tests/zk_stark.rs::stark_single_fold_envelope_golden_vector`
  encodes the sample envelope into hex and guards the Norito byte layout.
- Query indices are derived deterministically from the transcript label, parameters,
  and commitment roots; the verifier recomputes the index and rejects envelopes whose
  payload `j` values do not match the derived result.

### Governance vote tally (VoteBoolCommitMerkle)

Torii and governance hosts expect vote-tally proofs under:

- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `envelope TLV`:
  - `I10P` (`cols = 2`, `rows = 1`)
  - column 0 = commitment `H(v, ρ)`
  - column 1 = Merkle root of the voter registry
- `public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3`

Verifying key registry entries (`VerifyingKeyRecord`) for the tally circuit use:

| Field                        | Value / Notes                                                |
|-----------------------------|--------------------------------------------------------------|
| `backend`                   | `Halo2IpaPasta`                                              |
| `circuit_id`                | `halo2/pasta/vote-bool-commit-merkle8-v1`                    |
| `curve`                     | `pallas`                                                     |
| `public_inputs_schema_hash` | `0xfae4…64d3` (see above)                                    |
| `commitment`                | `sha256(backend || vk_bytes)`                                |
| `vk_len` / `max_proof_bytes`| Derived from the bundled `.zk1` artefacts                    |
| `key`                       | Inline `VerifyingKeyBox` wrapping the ZK1-encoded verifier   |

`cargo xtask zk-vote-tally-bundle` emits a reproducible bundle without any extra feature flags:

```bash
cargo xtask zk-vote-tally-bundle --out ./artifacts/zk_vote_tally --attestation ./artifacts/vote_tally.attestation.json
```

The command writes `vote_tally_vk.zk1`, `vote_tally_proof.zk1`, and `vote_tally_meta.json`, plus the optional attestation manifest if `--attestation` is supplied. The metadata JSON records:

```jsonc
{
  "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
  "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
  "vk_len": <length of verifying key bytes>,
  "proof_len": <length of proof envelope bytes>,
  "vk_commitment_hex": "<sha256 backend||vk_bytes>",
  "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
  "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
  "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817"
}
```

When `--attestation` is provided the manifest also captures `hash_algorithm = "blake2b-256"`, the bundle summary, and the Blake2b-256 digests/lengths for each artifact so auditors can archive the proofs alongside their checksum record. The `generated_unix_ms` value derives deterministically from the commitment/verifying-key fingerprint, so repeated regenerations remain comparable. During `--verify --attestation`, xtask checks that the manifest’s bundle metadata and artifact lengths match (the verifying-key and metadata digests stay stable; the proof digest changes with each regeneration).

Auditors can recompute the vectors using the xtask helper (which calls the same deterministic generator as the tests) and compare the resulting files and hashes against governance attestations.
