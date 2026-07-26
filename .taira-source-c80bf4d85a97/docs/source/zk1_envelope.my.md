---
lang: my
direction: ltr
source: docs/source/zk1_envelope.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ada31202e840ecc1a58da3cef6f9956db8e90c3a72714c566d8f53a4f6e50398
source_last_modified: "2025-12-29T18:16:36.240859+00:00"
translation_last_reviewed: 2026-02-07
---

# ZK1 Envelope Format (Proof/Verifying-Key Containers)

This document specifies the ZK1 envelope used in tests and helper utilities to carry
opaque proofs, verifying keys and public instances in a backend-agnostic way. ZK1 is a
TLV container with a 4-byte magic header and a sequence of typed records. Parsers are
deterministic and enforce size bounds for safety.

## Container

- Magic: ASCII `ZK1\0` (4 bytes)
- Followed by zero or more TLVs:
  - `tag[4]` (ASCII)
  - `len[u32 LE]`
  - `payload[len]`

## Recognized TLVs

- `PROF`: raw proof transcript bytes (opaque to ZK1). The backend’s verifier interprets
  the payload.
- `IPAK`: Halo2 IPA parameters for Pasta (transparent). Payload is `u32 k`, the exponent
  of the domain size `N = 2^k`. The verifier derives `Params::<EqAffine>::new(k)`.
- `H2VK`: Halo2 Verifying Key bytes for the selected circuit (processed form preferred).
- `I10P`: Instance column block for Pasta Fp. Layout: `cols[u32] || rows[u32] || rows*cols * 32`.

Notes:
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

let mut prf_env = zk1::wrap_start();
zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
zk1::wrap_append_instances_pasta_fp(&[public_scalar], &mut prf_env);
```


## Negative Cases (tests)

ZK1 decoders and verifiers reject malformed envelopes. Examples include backend/tag
mismatches, truncated instance blocks, and non-canonical field elements in instance TLVs.
