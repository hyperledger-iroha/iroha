# ZK1 Envelope Format (Proof/Verifying-Key Containers)

This document specifies the ZK1 envelope used to carry opaque Halo2 proofs, verifying
keys, and public instances. ZK1 is a TLV container with a 4-byte magic header and one
canonical record order for each payload kind. Parsers enforce exact order, size bounds,
and full input consumption.

## Container

- Magic: ASCII `ZK1\0` (4 bytes)
- Followed by zero or more TLVs:
  - `tag[4]` (ASCII)
  - `len[u32 LE]`
  - `payload[len]`

## Recognized TLVs

- `PROF`: raw proof transcript bytes (opaque to ZK1). The backend’s verifier interprets
  the payload.
- `CID1`: exact portable circuit identifier. Whitespace, aliases, and normalization are
  not accepted.
- `IPAK`: Halo2 IPA parameters for Pasta (transparent). Payload is `u32 k`, the exponent
  of the domain size `N = 2^k`. The verifier derives `Params::<EqAffine>::new(k)`.
- `H2VK`: Halo2 Verifying Key bytes for the selected circuit (processed form preferred).
- `I10P`: Instance column block for Pasta Fp. Layout: `cols[u32] || rows[u32] || rows*cols * 32`.

Notes:
- A production verifying-key container is exactly `IPAK`, `CID1`, `H2VK`, in that order.
- A production proof container is exactly `PROF`, optionally followed by one `I10P`.
- Duplicate, unknown, missing, empty, reordered, or trailing records are rejected.
- Instance blocks (`I10P`) use canonical 32-byte field representations; non-canonical
  values must be rejected by decoders.
- Multiple instance columns should be packed into a single TLV for a proof. Tests may use
  a single column for simplicity.
- ZK1 is backend-agnostic; a separate backend tag accompanies proofs/keys
  (e.g., `halo2/pasta/tiny-add-v1`, `halo2/pasta/tiny-add-public-v1`,
  `halo2/pasta/tiny-add-2rows-v1`).
- Tests generate deterministic fixture proofs/VKs for `tiny-add-v1`,
  `tiny-add-public-v1`, and `tiny-add-2rows-v1`; other circuit IDs use placeholder
  payloads unless real VK/proof bytes are supplied.

## Examples (Rust)

### Pasta/IPA (transparent)

```rust
let mut vk_env = zk1::wrap_start();
zk1::wrap_append_ipa_k(&mut vk_env, 5); // k = 5
zk1::wrap_append_circuit_id(&mut vk_env, "halo2/pasta/ipa/tiny-add-v1");
zk1::wrap_append_vk_pasta(&mut vk_env, &vk);

let mut prf_env = zk1::wrap_start();
zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
zk1::wrap_append_instances_pasta_fp(&[public_scalar], &mut prf_env);
```


## Negative Cases (tests)

ZK1 decoders and verifiers reject malformed envelopes. Examples include backend/tag
mismatches, truncated instance blocks, and non-canonical field elements in instance TLVs.
