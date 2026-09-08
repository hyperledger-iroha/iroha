# KAGEMUSHA V1 proving-key artifact codec

All KAGEMUSHA proving-key artifact roles use Halo2 compact-v1. Verifying keys retain
`SerdeFormat::Processed`; transparent parameters retain canonical `ParamsIPA::write`.
The first release has one proving-key codec: loaders reject Processed proving-key
artifacts rather than guessing a format or retaining a compatibility decoder.

The authenticated release manifest remains the authority for role, SHA-256 and
exact byte length. Every changed proving-key artifact needs a regenerated binding,
manifest and signed release evidence. The codec header cannot authorize a circuit,
a role, a hardware provider, or monetary use.

## Exact frame

| Field | Encoding |
| --- | --- |
| Codec magic/version | 16 bytes, `Halo2CompactPK1` followed by NUL |
| Curve domain | 32-byte BLAKE2b digest of the codec, curve equation, generator, field moduli and field representations; see `curve_domain` in the vendor codec |
| Complete frame length | u64 little endian, checked against the externally authenticated length |
| Embedded verifying key | Unchanged Processed verifying-key encoding |
| Three coefficient masks | Existing Processed polynomial encoding, in l0/l_last/l_active_row order |
| Fixed Lagrange polynomials | Existing checked u32-count polynomial-vector encoding |
| Permutation Lagrange polynomials | Existing checked u32-count polynomial-vector encoding |

For Pasta, with domain `n = 2^k`, materialized fixed count F, permutation count P,
and exact Processed VK length V, the complete size is:

`56 + V + (3 + F + P) * (32*n + 4) + 8`.

The trusted circuit role selects k and circuit parameters before decoding: mint
SHA shards use k12; claims and state carriers use their prescribed k16 profiles.
The authenticated frame length bounds reads before parsing. The checked embedded
VK fixes vector dimensions before polynomial allocation. Unknown magic/version,
wrong curve/domain/length, noncanonical points/scalars, malformed counts and
incomplete frames are errors. The outer authenticated reader also requires full
stream consumption and exact SHA-256. Canonical re-encoding streams into a bounded
digest sink; embedded VK equality with the separately authenticated VK remains
mandatory.

## Ownership and resource meaning

The codec stores one Lagrange basis per fixed/permutation polynomial and rebuilds
its coefficient basis in the exact VK domain. Writers first check that the existing
bases agree. Borrowed writing preserves keys needed by mandatory qualifying proofs.
Consuming writing releases coefficient/evaluator storage before output, then releases
serialized polynomial buffers. No proof equation, VK, transcript or key byte cap changes.

Compact storage does not shrink the reconstructed in-memory key or prove compliance
with the 128 MiB RSS or latency limits. Minimum Claim storage can fit its 64 MiB key
cap while its dense advice arrays alone exceed the RSS cap. Full witness geometry
and all immutable artifacts still require actual proof/resource qualification.

Implementation: `vendor/halo2-axiom/src/plonk/compact_key.rs`; production boundary:
`crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs`.
