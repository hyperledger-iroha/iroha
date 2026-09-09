# KAGEMUSHA V1 proving-key artifact codec

All KAGEMUSHA proving-key artifact roles use Halo2 structured-v1. Verifying keys
retain `SerdeFormat::Processed`; transparent parameters retain canonical
`ParamsIPA::write`. The first release has one proving-key artifact codec: loaders
reject Processed and compact-v1 proving-key frames without format inference or
fallback.

The authenticated release manifest remains the authority for role, SHA-256 and
exact byte length. Every changed proving-key artifact needs regenerated bindings,
manifest and signed release evidence. The codec header cannot authorize a circuit,
a role, a hardware provider or monetary use. Selector choices can change under the
new byte accounting; regenerate dependent verifying keys, recursive protocols and
convergence evidence when they do.

## Exact frame

| Field | Encoding |
| --- | --- |
| Codec magic/version | 16 bytes: `Halo2StructPK1` followed by two NUL bytes |
| Curve domain | 32-byte BLAKE2b digest of the codec, curve equation, generator, field moduli and field representations; see `curve_domain` in the vendor codec |
| Complete frame length | u64 little endian, equal to the externally authenticated length |
| Embedded verifying key | Complete Processed verifying-key encoding |
| Three coefficient masks | Processed polynomials in l0/l_last/l_active_row order |
| Fixed column count | u32 big endian, equal to the embedded VK fixed-column count |
| Each fixed column | One mode byte, then the exact payload below |
| Permutation column count | u32 big endian, equal to the trusted circuit permutation count |
| Permutation entries | One u32 little-endian target per cell in original column-major order |

Fixed modes have a unique priority: a constant column uses mode 0 and one canonical
scalar; a nonconstant column containing only zero/one uses mode 1 and
`ceil(n/8)` bytes with rows in low-bit-first order; every other column uses mode 2
and `n` canonical scalars. Constant takes precedence even at tiny domains where a
bitset would be shorter. Unknown modes, redundant encodings and nonzero unused
bitset bits are errors.

A permutation target identifies `column*n + row` for the domain label
`DELTA^column * omega^row`. Targets must lie below `n*P`, and every target must
appear exactly once. The complete cell count must fit u32. The decoder verifies
membership/bijection without retaining a full cell map and reconstructs the exact
Lagrange and coefficient bases; no copy-cycle orientation changes.

For Pasta, domain `n = 2^k`, permutation columns P, exact Processed VK length V,
and disjoint fixed-mode counts C (constant), B (binary), R (raw), the frame size is:

`56 + V + 3*(32*n + 4) + 8 + 4*n*P + 33*C + (1 + ceil(n/8))*B + (1 + 32*n)*R`.

## Admission and early sizing

The trusted role selects k and circuit parameters before decoding: mint SHA shards
use k12; claims and state carriers use their prescribed k16 profiles. Authenticated
frame length bounds parsing. The checked embedded VK fixes dimensions before
polynomial allocation. Wrong codec/curve/domain/length, noncanonical fields or
points, malformed counts, non-bijective targets and incomplete frames fail closed.
The outer reader also requires full same-stream consumption, exact SHA-256 and
outer EOF. Canonical re-encoding streams into a bounded digest sink; equality with
the separately authenticated standalone VK remains mandatory.

Configure-only preflight uses sound optimistic bounds for each selector strategy.
A pass permits synthesis; it never establishes exact artifact feasibility. The
fixed-column payload lower bound is `min(32, ceil(n/8))` plus its mode byte.
After synthesis, exact configured-column modes use normalized `Assigned` field
equality, including rational values with zero denominators. Compressed-selector
modes follow the same deterministic combination plan and actual root values as key
construction, without expanding selector field arrays. The callback checks both
unchanged PK/VK caps before key polynomial expansion. Selection is deterministic:
smallest feasible PK, then VK, then compressed mode on an exact tie. Separate
strategies cannot contribute different halves of a feasible pair. The independent
1,024-advice-column pre-synthesis ceiling remains enforced.

The exact serialized length is checked again before output allocation and after
writing. State PKs remain limited to 48,234,934 bytes, helper PKs to 64 MiB, VKs to
64 KiB and the artifact package to 512 MiB. No resource cap is enlarged by this codec.

## Ownership and qualification

Writers first validate matching polynomial bases and the complete permutation.
Borrowed writing preserves keys needed for mandatory qualifying proofs. Consuming
writing drops coefficient banks and the evaluator before its first output write,
then releases the VK, masks and Lagrange buffers after serializing them. The caller
owns output reservation, flushing and atomic publication; sink failure can leave
partial bytes and must never publish an artifact.

The generic writer derives permutation targets twice using an inverse-label index
with O(n+P) scratch and a validation bitmap that is dropped before output. At
k16/P133 the two passes require about 279 million field squarings, plus searches
and basis-validation FFTs. Canonical authenticated loading invokes this writer, so
actual load timing must be qualified alongside key generation and proving.

Structured storage does not shrink the reconstructed dense runtime key or prove
compliance with the 128 MiB process RSS or latency limits. Minimum Claim dense
advice storage alone still exceeds that RSS limit. Full artifact generation,
seeded proof recovery, immutable release evidence and physical profiles remain
separate required qualification.

Implementation: `vendor/halo2-axiom/src/plonk/structured_key.rs`; production boundary:
`crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs`; early admission:
`crates/iroha_core/src/zk/kagemusha_v1_recursion/artifact_resource_preflight.rs`.
