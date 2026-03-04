---
lang: ur
direction: rtl
source: docs/source/governance_vote_tally.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ebff8477d06e2aac8840988d31762704d05ded353d3f900a87db3ea5091e718
source_last_modified: "2026-01-03T20:01:33.034634+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Governance ZK Vote Tally
---

## Overview

Iroha’s governance tally flow relies on a Halo2/IPA circuit that verifies a bit vote commitment and its membership in the eligible voter set. This note captures the circuit parameters, public inputs, and auditing fixtures so reviewers can regenerate the verifying key and proofs used in tests.

## Circuit Summary

- **Circuit identifier**: `halo2/pasta/vote-bool-commit-merkle8-v1`
- **Implementation**: `VoteBoolCommitMerkle::<8>` in `iroha_core::zk::depth`
- **Domain size**: `k = 6`
- **Backend**: Transparent Halo2/IPA over Pasta (ZK1 envelope: `IPAK` + `H2VK` for VKs, `PROF` + `I10P` for proofs)
- **Witness shape**:
  - ballot bit `v ∈ {0,1}`
  - randomness scalar `ρ`
  - eight sibling scalars for the Merkle path
  - direction bits (all zero in the reference witnesses)
- **Merkle compressor**: `H(x, y) = 2·(x + 7)^5 + 3·(y + 13)^5 (mod p)` where `p` is the Pasta scalar modulus
- **Public inputs**:
  - column 0: `commit`
  - column 1: Merkle root
  - exposed via the `I10P` TLV (`cols = 2`, `rows = 1`)

### Circuit layout

- **Advice columns**:
  - `v` – ballot bit constrained to be boolean.
  - `ρ` – blinding scalar used in the vote commitment.
  - `sibling[i]` for `i ∈ [0, 7]` – Merkle path element at depth `i`.
  - `dir[i]` for `i ∈ [0, 7]` – direction bit selecting left (`0`) or right (`1`) branch.
  - `node[i]` for `i ∈ [0, 7]` – Merkle accumulator after depth `i`.
- **Instance columns**:
  - `commit` – public commitment published by the voter.
  - `root` – Merkle root of the eligible voter set.
- **Selector**: `s_vote` enables the gate on the single populated row.

All advice cells are assigned in the first (and only) row of the region; the circuit uses a `SimpleFloorPlanner`.

### Gate system

Let `H` be the compressor defined above and `prev_0 = H(v, ρ)`. The gate enforces:

1. `s_vote · v · (v - 1) = 0` – boolean ballot bit.
2. `s_vote · (H(v, ρ) - commit) = 0` – commitment consistency.
3. For each depth `i`:
   - `s_vote · dir[i] · (dir[i] - 1) = 0` – boolean path direction.
   - `left = H(prev_i, sibling[i])`
   - `right = H(sibling[i], prev_i)`
   - `expected = (1 - dir[i]) · left + dir[i] · right`
   - `s_vote · (node[i] - expected) = 0`
   - `prev_{i+1} = node[i]`
4. `s_vote · (prev_8 - root) = 0` – accumulator equals the public Merkle root.

The compressor uses quintic shapes only; no lookup tables are required. All arithmetic is performed in the Pasta scalar field, and the row count `k = 6` allocates `2^k = 64` rows — only row zero is populated.

### Canonical fixture

The deterministic harness (`zk_testkit::vote_merkle8_bundle`) populates the witness with:

- `v = 1`
- `ρ = 12345`
- `sibling[i] = 10 + i` for `i ∈ [0, 7]`
- `dir[i] = 0`
- `node[i] = H(node[i-1], sibling[i])` with `node[-1] = H(v, ρ)`

This produces the public values:

```text
commit = 0x20574662a58708e02e0000000000000000000000000000000000000000000000
root   = 0xb63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817
```

The `public_inputs_schema_hash` recorded in the verifying-key registry is `blake2b-256(commit_bytes || root_bytes)` with the least significant bit forced to `1`, yielding:

```text
public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3
```

### Verifying key record

Governance registers the verifier under:

- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `backend tag = BackendTag::Halo2IpaPasta`
- `curve = "pallas"`
- `public_inputs_schema_hash = 0xfae4…64d3`
- `commitment = sha256(backend || vk_bytes)` (32-byte digest)

The canonical bundle includes an inline verifying key (`key = Some(VerifyingKeyBox { … })`) together with the proof envelope. `vk_len`, `max_proof_bytes`, and optional metadata URIs are populated from the generated artefacts.

## Reference Fixtures

Use `cargo xtask zk-vote-tally-bundle --print-hashes` to regenerate the inline verifying key and proof bundle consumed by integration tests (outputs land in `fixtures/zk/vote_tally/` by default). The command prints a short summary (`backend`, `commit`, `root`, schema hash, lengths) and optionally the file hashes so auditors can capture attestation notes. Pass `--summary-json -` to emit the same data as JSON (or supply a path to write it to disk). Pass `--attestation attestation.json` (or `-` for stdout) to write a Norito JSON manifest containing the summary plus Blake2b-256 digests and sizes for every bundle artifact so attestation packets can be archived with the fixtures. When run with `--verify`, providing `--attestation <path>` checks that the manifest’s bundle metadata and artifact lengths match the freshly regenerated bundle (it does not compare the per-run proof digest, which changes with transcript randomness).

Regenerate the canonical fixtures and manifest:

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --print-hashes \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

Verify the checked-in artifacts remain current (requires the fixture directory to contain the baseline bundle):

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --verify \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

Example manifest:

```jsonc
{
  "generated_unix_ms": 3513801751697071715,
  "hash_algorithm": "blake2b-256",
  "bundle": {
    "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
    "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
    "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
    "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817",
    "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
    "vk_commitment_hex": "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e",
    "vk_len": 66,
    "proof_len": 2748
  },
  "artifacts": [
    {
      "file": "vote_tally_meta.json",
      "len": 522,
      "blake2b_256": "5d0030856f189033e5106415d885fbb2e10c96a49c6115becbbff8b7fd992b77"
    },
    {
      "file": "vote_tally_proof.zk1",
      "len": 2748,
      "blake2b_256": "01449c0599f9bdef81d45f3be21a514984357a0aa2d7fcf3a6d48be6307010bb"
    },
    {
      "file": "vote_tally_vk.zk1",
      "len": 66,
      "blake2b_256": "2fd5859365f1d9576c5d6836694def7f63149e885c58e72f5c4dff34e5005d6b"
    }
  ]
}
```

Store the current manifest next to your canonical artifacts (for example at `fixtures/zk/vote_tally/bundle.attestation.json`). The upstream repository keeps this directory empty to avoid committing large binary bundles, so seed it locally before relying on `--verify`.

`generated_unix_ms` is derived deterministically from the commitment/verifying-key fingerprint so it remains stable across regenerations. The generator uses a fixed ChaCha20 transcript, so the metadata, verifying key, and proof envelope hashes are reproducible. Any digest mismatch now indicates drift that must be investigated. Auditors should record the emitted values alongside the artefacts they attest.

Workflow reminder:

1. Run `cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --attestation fixtures/zk/vote_tally/bundle.attestation.json` to seed the bundle locally.
2. Commit or archive the resulting artefacts as needed.
3. Use `--verify` on subsequent regenerations to ensure the attestation matches the canonical bundle.

Internally the task runs the deterministic generator in `xtask/src/vote_tally.rs`, which:

1. Samples the witnesses (`v = 1`, `ρ = 12345`, siblings `10..17`)
2. Runs `keygen_vk`/`keygen_pk`
3. Produces a Halo2 proof and wraps it in a ZK1 envelope (including public instances)
4. Emits the verifying key record with the appropriate `public_inputs_schema_hash`

## Tamper Coverage

`crates/iroha_core/tests/zk_vote_tally_audit.rs` loads the bundle and checks:

- The genuine proof verifies against the bundled inline VK.
- Flipping any byte in the commitment column causes verification to fail.
- Flipping any byte in the root column causes verification to fail.

These regression tests guarantee Torii (and hosts) reject envelopes whose public inputs are tampered after proof generation.

Run the regression locally with:

```bash
cargo test -p iroha_core zk_vote_tally_audit -- --nocapture
```

## Audit Checklist

1. Review `VoteBoolCommitMerkle::<8>` for constraint completeness and constant selection.
2. Re-run `cargo xtask zk-vote-tally-bundle --verify --print-hashes` to reproduce the VK/proof and confirm the recorded hashes.
3. Confirm Torii’s tally handler uses the same backend identifier and envelope layout.
4. Execute the tamper regression to ensure mutated proofs fail verification.
5. Hash and gossip the `bundle.attestation.json` output (Blake2b-256) so reviewers can log the canonical manifest alongside their attestations.
