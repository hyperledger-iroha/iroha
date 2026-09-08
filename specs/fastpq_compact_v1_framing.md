# Compact V1 six-lane framing and field tapes

Source contract: 2026-09-08. The private `compact_v1` module supplies the normal
[offline verifier](fastpq_compact_protocol_contract.md). This document fixes its
source-level bytes and state reuse. It does not qualify production admission,
zero knowledge or the concrete hash. No SHAKE or prototype protocol selector is
accepted by this compact path.

## Canonical owner and complete context

`fastpq_isi::hash_bytes_384_v1` is the one-shot reference. Its owned reusable
prefix delegates to the existing borrowed prefix and permutation, retains the
full domain descriptor, and preserves both rate-buffer positions. No compact
module implements another permutation or substitutes a context digest.

Each call uses `GoldilocksDigestDomainV1` with these exact fields:

| Domain field | H commitments | G tape blocks |
| --- | --- | --- |
| `catalog` | `iroha-privacy-exact12-v1` | Same |
| `protocol` | `fastpq-state-transition-stark-v1` (`FASTPQ_FINAL_V1.name`) | Same |
| `profile` | Complete canonical `ProfileContextV1` frame | Same |
| `role` | `compact-commitment` | `compact-transcript` |
| `phase` | `typed-h` | `whole-field-tape-block` |
| `level: u64` | Body's exact tree level; zero for leaf/chain | Zero |
| `index: u64` | Body's exact leaf/parent position; zero for chain | Zero-based block ordinal |
| `counter: u64` | Body's FRI round or chain-message ordinal | One-based message ordinal |

The canonical digest owner additionally binds its message-frame domain and lane
ordinal, byte-field lengths and ordered payload field count. Each byte field uses
seven-byte little-endian chunks plus the mandatory remainder marker. Domain
integers use their complete eight-byte representation; they are not reduced
modulo the field. Each compact call supplies exactly one payload byte field:
the complete canonical `BodyV1` frame. H directly returns all six field words;
there is no binary squeeze or root-rejection projection.

The profile frame is encoded with `norito::encode_canonical`, nominal schema
`fastpq_prover::compact_v1::ProfileContextV1`. Its ordered fields are:

1. `version: u16 = 1`.
2. `identity: Vec<u8>` containing exactly
   `fastpq:compact:goldilocks-six-lane:h6:g-field-blocks:q375:c401:342cols:923slots:65536rows:8blowup:17folds:v1`.
3. `context: Vec<u8>` containing the complete canonical engine statement frame.

The engine statement schema is `fastpq_prover::compact_v1::EngineStatementV1`.
Its 17 ordered fields are `relation: String`; `trace_rows`, `lde_rows`, `width`,
`constraints: u32`; `base_modulus`, `extension_nonresidue`, `lde_root: u64`;
`lde_log_size: u32`; `coset_offset: u64`; `blowup`, `arity`, `folds`,
`terminal_values`, `terminal_degree`, `queries: u32`; and `statement: Vec<u8>`.
The fixed values are those in `profile::Binding::new` and its exact geometry
check. `relation` and `statement` come from the same caller-fixed AIR.

The context must contain 1..=262,144 bytes. This ceiling includes the engine
statement's enclosing canonical frame and geometry; raw public bytes receive no
additional hidden allowance. All Norito frame headers, schema hashes, flags,
lengths and checksums remain part of the logical digest input. An outgoing
canonical encoder owns these forms. A formal recognizer of arbitrary oracle
queries must additionally prove exact parsing, permitted field combinations and
full consumption; constructing valid inputs alone is not that proof.

The domain `profile` field above is the complete profile-context descriptor.
The artifact's short `FastpqCompactProfileIdV1` is separate metadata: the quantity
route computes SHA-256 over canonical `QuantityArtifactProfileV1`, including
catalog, final protocol, compact geometry identity, lane-parameter digest,
22 tape lengths, value/context schemas, value-hash domain and four relation
identities. That metadata hash is neither H nor G and never replaces context
bytes. Computing the identifier registers no qualified profile.

## Body frame and tree coordinates

Every body has nominal schema `fastpq_prover::compact_v1::BodyV1` and these
ordered fields:

1. `kind: u8`: 1 leaf, 2 parent, 3 chain, 4 tape request.
2. `oracle: u8`: 1 row, 2 mixed, 3 quotient, 4 FRI; zero for chain/tape.
3. `round: u8`: FRI index 0..17, or message 1..22; zero for other trees.
4. `level: u32`: zero for leaves/chain/tape; one-based for parents.
5. `position: u32`: exact leaf/parent position; zero for chain/tape.
6. `output_bytes: u32`: 48 for H, the exact whole-tape length for G.
7. `fields: Vec<Vec<u8>>`: one complete canonical leaf payload; two separately
   ordered full child digests; complete pending tape then next root; or the full
   predecessor digest, respectively.

Rows, mixed values and quotients have 524,288 leaves. FRI round `r<17` has
`524,288 >> (r+1)` pair leaves. Round 17 has one four-value leaf and a parent
that must repeat its child. All supplied leaf words must be canonical. Exact
payload length, index range, parent level/range and singleton duplication are
validated before hashing. A body carries its tree position as `u32`; the digest
owner's separate index is full-width `u64`. For G that index is the block
ordinal while body position remains zero.

## Fixed field-product messages

For message `j`, the body binds `j`, its fixed whole output length and the full
predecessor. Block `b` is the six-lane digest at typed index `b` with that same
body. Concatenation is in ascending block order; each output word is canonical
little-endian. The exact block counts are
`[1, 228, 616, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 67]`.
A complete run has 22 messages, 931 blocks and 44,688 bytes.

Message 1 is a positive-length dummy. Messages 2 and 3 decode respectively 342
and 923 Fp4 coefficients. Message 4 decodes two; messages 5..21 decode one each.
The entire tape is checked for canonical words before the required prefix is
split directly into groups of four. No coefficient-rejection loop survives.
Unused suffix coordinates still enter the next chain hash.

Message 22 takes its first 401 words, rejects `p-1`, maps modulo 524,288 and
returns the first 375 distinct positions sorted ascending. Its final, 402nd word
is not sampled but must be canonical. Insufficient distinct positions reject
permanently; there are no retries, extra blocks or partial success. Before
message 22, every chain H binds the complete previous raw tape and next root.
The initial predecessor is zero; no root can be committed after the final query
message. The dummy and all suffixes therefore have explicit ownership.

## Owned prefix resource contract

One `Arc<[u8]>` holds the complete canonical profile frame. Every context clone
shares it and a fixed array of 462 `OnceLock` slots: 22 H round values times
20 levels, plus 22 G rounds. Each initialized slot owns immutable six-lane
prefix state and the partial rate buffer while sharing the complete domain
bytes. It absorbs through canonical domain tag 7; the per-call full-width index,
remaining domain fields and body are appended through the same existing owner.
There is no mutable process-global context cache and no context clone per hash.

This preserves one-shot logical inputs while bounding prefix state by fixed
geometry. Prefix preparation, frame encoding, state copies, permutation work
and all emitted blocks remain real costs. The cache does not reduce adversarial
oracle-query charges or establish a proof latency/memory guarantee.

## Qualification and test scope

The owned-prefix tests compare one-shot, borrowed, owned and streamed outputs
for empty/multiple fields, both rate positions, full-width coordinates, 256 KiB
context and concurrent reuse. Compact tests compare every G block with an
independent one-shot call, exact H/body framing, canonical suffixes, fixed cache
geometry, late query candidates, permanent abort and terminal singleton behavior.
An independently calculated complete-context dummy/first-chain known answer
checks framing and outputs. These are source tests; a compilation or test
result must retain its actual source/binary/command evidence separately.

An internally reviewed conditional reduction groups ideal `F_p^6` block-oracle
entries into fixed whole-message tuples. Compute, encoded coordinate XOR and
uncompute cost two ideal group queries in total, including the binary interface.
The model needs full contexts, injective reversible framing, disjoint H/G domains,
fixed block counts, an explicitly bounded auxiliary-query domain and the
compiler's every-prefix AIR/FRI premises. Canonical serialized fields are not
uniform 384-bit strings. This reduction does not qualify the concrete six-lane
construction or a public low-level permutation interface; the earlier SHAKE
assumption is not transferred.

TODO: Complete the implementation-to-model mapping, concrete and external review,
full-relation and privacy qualification, immutable hardware evidence and production
integration. The current complete-row DTO has a raw lower bound of 1,050,000 bytes
per segment, exceeding the unchanged 512 KiB proof and 1 MiB AXT ceilings before
framing. This framing change does not solve those admission resource gaps.
