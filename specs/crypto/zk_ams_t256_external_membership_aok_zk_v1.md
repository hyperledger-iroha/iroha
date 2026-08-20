# ZK-AMS T256 external-membership AoK/ZK V1 precursor

## Status and authority

This document is a fail-closed, game-based specification precursor. It is **not a security proof**,
an argument-of-knowledge theorem, a zero-knowledge theorem,
an independent review, or a certificate. It assigns no security bits, creates
no certificate digest, and authorizes no release gate. Every security
obligation identified below remains open until a later proof is independently
reviewed against the exact pinned implementation.

The smallest candidate theorem is deliberately restricted to the two fixed
release membership circuits described below. It does not cover the generic
`ArithmeticCircuitStatement` API, scalar commitments, an arbitrary number of
external vector commitments, or any other caller of the generalized
Bulletproof backend.

## Source snapshot

The precursor was derived from the following read-only snapshot. These hashes
identify source inputs; they are not evidence that any theorem has been proved.

| Artifact | SHA-256 |
| --- | --- |
| `Cargo.lock` | `0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222` |
| `crates/iroha_zkp_halo2/src/vega.rs` | `60b31745a00c20a5ae356037dc5c10c1581704ac3170f55bb0cb936657a84fee` |
| `crates/iroha_zkp_halo2/src/vega/masked_relaxed.rs` | `262b8189f6ddb59b069913cd53eeeb159d352339115f389b3c0a5fbebbb776d9` |
| `crates/iroha_zkp_halo2/src/vega/sponge.rs` | `6eb48cb13dc9afb1a5a088d11ec918235d584234d8fad87170dd606e2eb5a81c` |
| `crates/iroha_zkp_halo2/src/generalized_bulletproof.rs` | `f0acea2500c3649bd1c54ea40fadcc2d4a4a7a3627e4abcadb26ad85766b5ab7` |
| `crates/iroha_zkp_halo2/src/generalized_bulletproof/exact_small_coefficient_source_v1.rs` | `1f9d45b7673ef85e92b56d9a442b61cddf7bee87cc5398d009d4085e6ed26533` |
| `crates/iroha_zkp_halo2/src/vega/bulletproof_t256.rs` | `f51b5045c2a9b9e1c486338a30d50b3c58d0c4d4bbf04a142ccb88eb7d8e122d` |
| `crates/iroha_zkp_halo2/src/vega/bulletproof_t256_transcript_v1.rs` | `83fd77d2277392bc4d4b5ac317ff23319e8f996d5af4f2dcf7ce7583cc12e2f2` |
| `crates/iroha_zkp_halo2/src/vega/bulletproof_t256_workspace_lease_v1.rs` | `5fb06c38b792d251496665c338d3d442a992c379e69dae671ddd7c99575013a4` |
| `crates/iroha_zkp_halo2/src/vega/curve.rs` | `6bd3b34fdaf502718443a2da907af44896f0195435713596c1b1133d252b0f80` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe.rs` | `6a0629632ae1d94592fbb724dcc66370faba819fbb8be34ca5cc8d46a308d5dd` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/manifest.rs` | `1ecb4b590a07e99311987a3a6f8190791ff4e75e5399e0325572d95fd9d8235d` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/direct_object_transport.rs` | `cf01063325a40eb8dcdc725e5171806a308eda788874e9467344e4d1ad58ccda` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/wire.rs` | `2ec2e7e1ce56ff1eec5e34a98653dfec3a4df5a4907f9961ca6418e3f765255c` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/cpk_ceremony.rs` | `fe85cc7a1c5b443c1d00b1c0ce4340c88c0e0eecae0ea0955f31e39475615ae9` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/cpk_relation.rs` | `6629406e1916332adeca2af4c66adb6fe51e06bf4e26eb296375ae8c7a393e8a` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/cpk_relation_common_a.rs` | `e049575a3b8a8b0f75a260a02a0815b612cf3bcb83266b38360cb6b40301a8f1` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/cpk_relation/state_owned_creator_v1.rs` | `5c674e63e7d8f9c1d1fdd51be2e76de4f3e6686730be083bb528ae3f07e1d57c` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/cpk_relation/state_owned_secret_adapter_v1.rs` | `e06a9ccd292c7769cdee2568996c3af2d8326978baaaa9d44d0341e419feea92` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/persistent_decryption_equality.rs` | `62467cea1893604d430fb787c23b4b2df1c77d724738db20e6dd113528ca2f97` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/exact_eight_chunk_membership.rs` | `f0eb5c67ca88a6787303e12f502cb9fc7f03487501c3c6b20616c374eeaf81f9` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/persistent_membership_evidence.rs` | `12ed3a665992776625673d7dfb3abdadb23a3d68dea7c26fcdc5ecde9440cc13` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective.rs` | `4e2b545df7303a3680be73dd15a705c6f56b05361213544e7f78457eee0b34bc` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/borrowed_product.rs` | `0fd55027ee0c8adf024022e840f9d1b8657b9f3d9accb2404832333348eb9c1a` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/persistent_direct_opening_v1.rs` | `e6da14a973093b0751d07342b55a083001e2bd3cac837b6f7530d3402fa7dc87` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/party_local_rkg_ephemeral_v1.rs` | `07348243c3b2a986ca7b61626d140dac8193701efab920640adfb24eb749d5da` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/direct_rkg_one_candidate_v1.rs` | `58c49c41a3e819348cb535d1f53ca06ed96233e77d34bffb1ef9b67217df6f66` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/direct_rkg_one_creator_v2.rs` | `f5b982d52b82e16e3575dd1d99ac77e2306f123eafd90e7c8e3e3a3c60078a41` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/direct_rkg_one_publication_v1.rs` | `88b76057d3fbd5e9d3339c8985f4ebf6f4d4a8bda05b9f650b9a183dbed62b69` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/direct_rkg_one_publication_v1/direct_rkg_one_lifecycle_v2.rs` | `399865581d9863d2aad3323a0798b43e9f1a78cb7fa3c99d0c7d29e89b2d9978` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/direct_rkg_one_publication_v1/direct_rkg_one_lifecycle_record_v2.rs` | `fb02a926fdb8ebceb6882cec1e79d4f957d75c67beb1224c99966d3c65f65091` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/direct_rkg_one_sealed_candidate_v1.rs` | `cff01c1b791c0320b7949cdf83dfd6cdc7743458aa18be1c51745b94d5bd2715` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/direct_rkg_ephemeral_membership.rs` | `d001681b891574a124783951133a069954f96c07a66346ca93df2bb69cf4c5be` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active.rs` | `6057b745b92bceeb17f8372bde6b5aab4a943bac34668ba923c92d5df19ef0ae` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_common_a_v1.rs` | `953916ba1c94edc69acbb41c71dd4be09666c611cfeaa2caf8275945260c49ed` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_common_a_v1/creator_replay_v1.rs` | `b1524d148bb20bff22144de04a7eb877d27949cad944ff853f71fd898fb5c6a2` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1.rs` | `c63e85c714da2de177d007248c5f2151d0d4d93d26d63bdfcf8ff22665301bc9` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/response_commitment_v1.rs` | `c0e428e9f10073b309ca287044e87d7a38f223824c053e7c45c9561f68145536` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/rkg_one_creator_response_v1.rs` | `13d6daff96419ee413e90567a58ee63c1f42149f87cfe7855dde5e25f57502c5` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/rkg_one_creator_response_v1/rns_first_messages_v1.rs` | `f956cacd1680a20eecbb67e44a8ae88838506b2c01d652695f2b784d7ce31620` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/rkg_one_creator_membership_v1.rs` | `990f1eadb3f9199a0c33f627bb942cfd1fdf267de269b0806860b6c8271d81c0` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs` | `f1aa4ba944f929ea322ea7f3d710a32e6a933a331fb9441859b757835c8a49fe` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1/transcript_v1.rs` | `658e7aae564868ce3997240b87dc27e8add6b2f583b0895e08ba9a98961cdafb` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/statement_v1.rs` | `cb28a30ff5f4069f20638aec702beae5103abae41870c88660cf1d25a1bb4f76` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/statement_v1/rkg_one_h0_h1_replay_v1.rs` | `e435eb280a9da280231008cf24e581a7a660989cd6239b4f5db5c9a2401a29c6` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/statement_v1/rkg_one_creator_core_v1.rs` | `8c038b0b5304e09dcb9aa99aaef33fad4d25c18f2dc7af3e44871153449f97bf` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_rkg_one_creator_adapter_v1.rs` | `5d0e4f671ad0ba31bfc781a8892565f03c30f36f57ff27274f863e26e7e46ea7` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/predecode_v1.rs` | `43eb5fa77d9985c92a5f7fc8d7b7e2c73834ee5600b20b786f811a8dd5b4c7b1` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/predecode_v1/rkg_one_semantic_verifier_v1.rs` | `25be7c86805756a1855421a2350c4c509afcd9d8633e70b5f4dde115ec9b99b9` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/direct_collective_eval_ceremony.rs` | `04c0fa63fa22587509abbef9d2145f86b4a0ebd77cc36ff13e09010054095581` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding.rs` | `1c5cf1c9db73a76bb7cb4356cb5b5ffabe187ae21b1a8ccdfac49f5cc42f4db8` |

This table is exhaustive for the production Rust inputs used by the five
source-refinement facts below. Test-only children and production siblings
unrelated to those facts—including Galois, streaming, incremental,
prepared-public-A, and release-admission paths—are outside this snapshot
unless listed explicitly.

`Cargo.lock` pins `halo2curves` 0.9.0, `keccak` 0.1.6,
`sha3` 0.10.9, and `tiny-keccak` 2.0.2. It does not pin an upstream
Bulletproof implementation from which an exact theorem can be inherited.

## Deterministic execution and acceleration status

The fixed-order scalar CPU implementation is the V1 correctness reference.
The generalized backend's feature-gated Rayon path collects independent MSM
chunks and public-point folds in canonical input order, but the fixed T256 suite
sets its parallel-prover policy to false and the process-local workspace lease
serializes membership operations. The MKHE NTT and fixed T256 membership paths
currently qualify no SIMD/NEON, Metal, CUDA/GPU, or `halo2curves` assembly
backend.

This is an open implementation obligation, not an acceleration claim or a
security certificate. Any future backend must be feature-gated, retain the
scalar fallback, preserve commitments, proof bytes, transcript digests, and
verification decisions for identical inputs and entropy, pass a secret-handling
review, and close implementation-derived peak-RSS evidence within the governed
160 MiB workspace ceiling. Until those gates close, acceleration qualification
remains fail closed.

## Fixed algebra and CRS

Let

```text
q = 0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff.
```

The implementation uses the prime-order, cofactor-one T256 group with scalar
field `F_q`. The full generalized-Bulletproof basis is

```text
g, h, G_0, ..., G_65535, H_0, ..., H_65535.
```

The direct response commitment uses `G_0, ..., G_16383` and `h`. The adapter
derives the four basis families independently by applying SHAKE256 to these
labels and mapping each consecutive 32-byte block through the T256
`hash_to_curve("from_uniform_bytes")` suite:

```text
iroha.generalized-bulletproof.t256.g.v1
iroha.generalized-bulletproof.t256.h.v1
iroha.generalized-bulletproof.t256.G.v1
iroha.generalized-bulletproof.t256.H.v1
```

The ordered digest includes `g`, `h`, all 65,536 `G` points, and all 65,536
`H` points. Runtime construction checks shape and nonidentity, but it does not
establish pairwise distinctness or the absence of efficiently known linear
relations. A later theorem must state an exact hash-to-curve model and a
full-basis multi-representation assumption; the basis digest and its KAT are
only implementation facts.

## Exact fixed relations

For `b` equal to one or two, the public input is a nonidentity point `C`, the
complete transcript context described below, and the pinned basis. The witness
relation is

```text
C = sum_{i=0}^{16383} v_i G_i + r h,
r in F_q,
v_i in {-1, 0, 1}                         when b = 1,
v_i in {-2, -1, 0, 1, 2}                 when b = 2.
```

The verifier relation permits `r = 0`. The native prover wrapper rejects a
zero input blinding, but that API restriction must not be added to the theorem
statement.

For bound one, two multiplication gates per coefficient enforce two Boolean
values and the linear equation

```text
b_plus - b_minus - v_i = 0.
```

For bound two, three multiplication gates per coefficient enforce three
Boolean values and

```text
b_low + b_high - 2 b_negative - v_i = 0.
```

Every committed coordinate from index 16,384 through `n - 1` is separately
constrained to zero. The exact fixed shapes are:

| Bound | Padded gates `n` | Constraints `K` | IPA rounds | Core proof bytes | Framed chunk bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| one | 32,768 | 98,304 | 15 | 1,447 | 1,494 |
| two | 65,536 | 163,840 | 16 | 1,513 | 1,560 |

The generalized statement contains one external vector commitment and zero
scalar commitments. With the constraint weights aggregated by powers of `z`
and `Y = (1, y, ..., y^(n-1))`, the implemented polynomials specialize to

```text
l(X) = v
     + X   (w_R * Y^-1 + a_L)
     + X^2 a_O
     + X^3 s_L,

r(X) = (w_O - Y)
     + X   (w_L + a_R * Y)
     + X^2 w_C
     + X^3 (s_R * Y),

t(X) = <l(X), r(X)>.
```

Here `*` denotes coordinatewise multiplication where vectors are involved.
The degree of `t` is six. The proof commits `t_0`, `t_1`, `t_3`, `t_4`,
`t_5`, and `t_6`; the center coefficient at index two is omitted and enforced
through the polynomial and external-commitment equations. The verifier checks
the polynomial MSM and IPA MSM independently.

## Exact transcript and retry semantics

The initial state is the unambiguous fixed-length concatenation

```text
"iroha.generalized-bulletproof.t256.transcript.v1"
|| context_digest_32
|| generator_basis_digest_32
|| chunk_ordinal_u16be
|| coefficient_bound_u8
|| commitment_33.
```

The fixed release wrapper supplies coefficient count 16,384 and reconstructs
the exact circuit. The V1 transcript does not independently absorb `n`, `K`,
the coefficient count, commitment counts, or a circuit digest. Consequently,
any V1 theorem must be scoped to this wrapper and these two shapes. A future
V2 may absorb an explicit theorem-suite and shape digest, but doing so does not
by itself establish knowledge soundness.

Scalars are appended as tag `0` followed by their canonical 32-byte
little-endian encoding. Nonidentity points are appended as tag `1` followed by
their canonical 33-byte encoding. An accepted challenge is appended as tag
`2`, its big-endian `u32` ordinal, the successful attempt byte, and the
canonical little-endian scalar.

For challenge ordinal `k` and attempt `a` in `0..128`, the transcript makes
two distinct Keccak-256 queries:

```text
H(challenge_domain || state || k_u32be || a_u8 || 0)
H(challenge_domain || state || k_u32be || a_u8 || 1).
```

Their 64 output bytes are concatenated in that order and reduced by
`from_uniform_le_bytes`. A zero result is rejected; after 128 zero results the
transcript fails. The theorem must model the variable query schedule, the
modulo-reduction bias, conditioning on nonzero, and exhaustion. The loose
per-attempt variation term `q / 2^512` is only an obligation input, not a
completed bound. Prover scalars are separately sampled by canonical 32-byte
rejection with a 128-attempt ceiling; zero is allowed there.

The exact proof schedule is

```text
AI, AO, S
  -> y, z
T0, T1, T3, T4, T5, T6
  -> x
tau_x, u, t_hat
  -> ip_x
(L_j, R_j -> xi_j) for log2(n) rounds
a, b.
```

There are 19 scalar challenges for bound one and 20 for bound two. All encoded
proof points, including `AI`, `AO`, `S`, the six `T` points, and every `L/R`
point, must be nonidentity. Point-identity errors abort rather than retrying the
individual point.

## Universal-language obstruction

Because T256 is prime order and `h` is nonidentity, `h` generates the whole
group. For every admissible `C` and every fixed bounded vector `v`, there is a
mathematical scalar `r` satisfying

```text
r h = C - sum_i v_i G_i.
```

Therefore the ordinary language “there exists a bounded opening” contains
every nonidentity point. False-statement soundness for that language is
vacuous. The required property is an argument of knowledge: from an accepting
efficient prover, an efficient extractor must return an efficiently known
bounded opening `(v, r)` and satisfying gate wires, or produce a precisely
defined full-basis multi-representation break.

Computational representation binding is a distinct later step. Given two
efficiently known, unequal openings of the same `C`, their difference is a
nonzero relation among `G_0, ..., G_16383, h`. The implementation has no
statistical binding: the one-point commitment has a `q`-element codomain while
the bounded-vector domains alone have `3^16384` or `5^16384` elements.

## Knowledge games and the external-commitment boundary

### Joint globally algebraic creator/prover game

A possible straight-line AGM route may be considered only in a joint game in
which one globally algebraic adversary creates every challenged commitment
and later produces its membership and direct-relation proofs. The game must
retain algebraic representations across the complete commitment lifecycle,
including persistent commitments created in an earlier protocol phase. The
provenance extractor must consume those global representations without
rewinding and expose CRS-closed representations for every challenged
commitment. Only a separate membership-extraction theorem may turn those
representations and the proof transcript into required openings or a
full-basis relation.

Calling only the later proof algorithm “algebraic” is not enough. The theorem
must define representation validity for the SHAKE/hash-to-curve CRS, every
group input and output, state handoff, corruption timing, and any honest-party
commitment that enters the game.

#### Six commitment-creation events

The candidate game `G_JCP-RKG1-Prov` is restricted to the pinned local,
single-session RKG1 creator corridor. Each row below denotes eight distinct
per-chunk creation records. A slot/chunk key is unique even if two resulting
points happen to be equal.

| Slot | Opening | Bound | First creation event | Required later link |
| ---: | --- | --- | --- | --- |
| 0 | persistent `s` | one | After the CPK share proof, when the persistent blindings are sampled and `PersistentDirectOpeningOwnerV1::new_unverified` computes and retains the eight canonical points. | The installed CPK binding, retained wire, direct membership frame, and logged points must agree exactly. |
| 1 | ephemeral `u` | one | During party-local ephemeral preparation, before its membership proof, when `u`, its eight blindings, points, and retained wire are created. | The retained owner, preparation evidence, direct membership frame, and logged points must agree exactly. |
| 2 | `e_0` | two | When the direct bound-two membership evidence first materializes each chunk commitment. | The membership frame and outer response reconstruction use that exact point. |
| 3 | `e_1` | two | When the direct bound-two membership evidence first materializes each chunk commitment. | The membership frame and outer response reconstruction use that exact point. |
| 4 | forced-zero vector | two | When its direct bound-two membership evidence first materializes each chunk commitment with its own blinding. | The membership frame, forced-zero equation, and outer reconstruction use that exact point. |
| 5 | forced-zero vector | two | When its distinct direct bound-two membership evidence first materializes each chunk commitment with its own blinding. | The membership frame, forced-zero equation, and outer reconstruction use that exact point. |

For slots two through five, chunk creation is sequential: a chunk point need
only be logged before the challenges of its own membership transcript, not
before challenges for an earlier chunk. Slots zero and one are earlier points
recomputed from the same retained openings. Their later contexts and proof
frames do not constitute new creation events.

#### Typed extractor log and CRS closure

The log below is reduction-only ghost state. It is not a Rust field, runtime
audit facility, serialized receipt, verifier input, or wire extension.

```text
B = (g, h, G_0, ..., G_65535, H_0, ..., H_65535)

PersistentCpkKey = (
    profile, security_certificate, roster, key_material, epoch,
    cpk_transcript, party_index, party, public_share
)

EphemeralKey = (
    PersistentCpkKey, persistent_binding_identity, direct_context,
    evaluated_key_ordinal, digit, record_index, secret_lineage_identity
)

DirectMembershipKey = (
    EphemeralKey, relation, statement_core, slot
)

CreationRecord = (
    typed_lifecycle_key, slot, chunk, bound, exact_canonical_point,
    crs_representation, creation_ordinal, predecessor_state, successor_state
)
```

A record for `P` is valid only if its representation `rho` satisfies

```text
P = <rho, B>.
```

If an algebraic representation refers to earlier point handles, the extractor
must recursively substitute their logged representations until only elements
of `B` remain. An unknown or opaque root handle, a cycle, a missing predecessor,
or a representation that does not flatten to `B` invalidates provenance. An
honestly computed membership commitment has the sparse representation given by
its coefficients on the first 16,384 `G` points and its coefficient on `h`, but
that opening remains ghost state and is never exported by the implementation.
The log need not choose a unique representation, and it must not assume CRS or
commitment points are pairwise distinct.

The persistent key deliberately contains no future direct-context or digit
axis: slot zero is created before those axes exist. The later `EphemeralKey`
links it to one direct session through the installed binding identity and exact
point equality. Digests may locate typed records, but they cannot substitute
for exact point equality without an explicit collision bad event.

#### Game `G_JCP-RKG1-Prov`

1. The challenger fixes the exact CRS, hash-to-curve suite, random oracles,
   governed roster, CPK axes, and direct-ceremony axes from the pinned snapshot.
2. One globally algebraic creator/prover controls the target party continuously
   from persistent-commitment creation through the final direct proof. Every
   adversarial group output is accompanied by an algebraic representation; an
   honestly computed group output is recorded with its known sparse
   representation.
3. The challenger records the slot-zero points at persistent CPK-owner creation,
   the slot-one points at ephemeral preparation, and slot-two through slot-five
   points as their membership chunks are created. Each record precedes the
   corresponding membership transcript's first challenge.
4. The pinned move-only state transitions assemble one
   `DirectRkgOneProverSessionV1`. Slots zero and one must equal their earlier
   records point for point; later direct frames, outer responses, and semantic
   replay must retain the fixed six-slot/eight-chunk ordering.
5. The game accepts only if the exact pinned semantic verifier accepts the
   completed proof. Define `BadProv` as acceptance with any decoded membership
   point lacking one unique, valid, pre-challenge, CRS-closed creation record.
6. The candidate provenance theorem must bound `Pr[BadProv]` under the stated
   global-algebraicity, lifecycle, corruption, hash, and source-refinement
   assumptions. This precursor supplies the game, not that bound or its proof.

Given a successful provenance game, a later extractor may read the 48
CRS-closed representations. Only a separately proved membership extractor can
turn acceptance into bounded openings and satisfying gate wires. Comparing
those openings with the creator representations may then yield the full-basis
multi-representation relation required by the outer reduction.

#### Corruption, abort, and source-refinement rules

The minimal game uses static corruption. Adaptive corruption is admissible only
if the extractor retains every pre-corruption honest creation record, or the
challenger itself records the honest sparse representation. Corruption must not
turn an earlier opaque point into a retrospectively algebraic output.

Only one uninterrupted local session is in scope. The persistent owner may
cross the CPK-to-RKG phase boundary, but its ghost record must cross that same
boundary. The ephemeral owner is consumed once. An error, panic, failed local
check, or aborted proof marks every partial successor/log segment unusable and
emits no accepted candidate; it cannot be restored, spliced into a later
session, or used to satisfy `BadProv`'s record predicate.

The pinned source-refinement obligation is limited to these data-flow facts:

- persistent construction retains exact canonical points, and binding
  admission and the post-CPK guard recompute and compare those points;
- ephemeral preparation retains an owner and canonical wire, and compares its
  first membership evidence to them before installing the one-use state;
- the prover session consumes that owner, rechecks the persistent binding and
  both retained wires, and uses the same openings and blindings for membership
  and outer responses;
- the creator emits six ordered membership frames, while predecode compares
  slots zero and one to the capability's exact point arrays; and
- semantic verification consumes all 48 membership proofs and uses their exact
  points in the outer commitment reconstruction.

These are implementation/refinement obligations, not cryptographic evidence.
In particular, `DirectRkgOneProverSessionV1`, the verified binding, retained
wire, commitment-set digest, semantic handoff, and move-only ownership are not
creator provenance, extractor metadata, receipts, admissions, or security
certificates.

The restricted claim is only that a successful execution of this pinned local
single-session corridor in `G_JCP-RKG1-Prov` gives the extractor one exact,
pre-challenge, CRS-closed record for each of the 48 membership commitments. It
does not establish bounded-opening or gate-wire extraction, membership AoK,
membership ZK, CRS/MRep security, composite-ROM forking, full-ceremony
composition, security bits, verifier authority, or release admission.

### Opaque external commitment and later independent prover game

If a later independent prover receives `C` as an opaque external group input,
its AGM metadata may express proof points relative to the input handle `C`
without revealing a representation of `C` in the CRS. No straight-line
opening extractor follows from that metadata alone. This game remains
blocked unless a separate proof extracts relative to an opaque `C`, the
commitment protocol exports an independently justified extractable state, or
the protocol is replaced. A theorem for the joint creator/prover game must not
be cited for this independent-prover game.

This is a structural blocker for the public verifier, which accepts proof bytes
that need not have traversed the pinned local creator corridor. Closing that
broader claim requires one of:

1. a separate proof that extracts relative to an opaque external `C`;
2. a persistent, independently trusted algebraic attestation with its new trust
   and corruption assumptions stated explicitly; or
3. a protocol or membership-backend redesign with an extractable commitment or
   independently established commitment-creation proof.

Slot zero is the hardest temporal boundary because it predates RKG1, and slot
one also predates the direct proof. The existing CPK membership/binding
capability cannot bootstrap their provenance by citing the same still-unproved
membership AoK; that would be circular. A runtime log containing only points or
hashes is not extractor metadata, while a runtime log containing the actual
openings leaks secrets and changes the threat model.

### Excluded executions and required negative cases

A later theorem must reject, exclude from its statement, or account for each of
the following without silently mapping it into the joint game:

- an independent prover that receives an opaque commitment or raw proof bytes;
- imported or restarted state containing only points, encodings, or digests;
- creator/prover separation without one persistent extractor-visible algebraic
  log;
- adaptive corruption for which earlier representations are unavailable;
- an algebraic expression rooted in an opaque handle or not flattenable to the
  full pinned CRS;
- a digest-only record, or use of a commitment-set/lineage digest as if it were
  an algebraic representation;
- cross-profile, roster, key, epoch, transcript, party, digit, record, slot, or
  chunk splicing, reordering, duplication, or replay;
- reuse of a partial log after error, panic, failed proof generation, or owner
  burn;
- test-only injection or fixture corridors;
- treating a verified binding, semantic completion, move-only session, receipt,
  admission, or release gate as provenance;
- circular use of the current membership proof's desired AoK to certify the
  commitment that is its external input;
- coalescing slots four and five because both coefficient vectors are zero, or
  assuming independently keyed commitments must have distinct point values;
- assuming honest sampling in the adversarial game; and
- either hashes of secret openings, which do not expose a CRS representation,
  or actual runtime openings, which violate the intended secrecy boundary.

### Plain-ROM black-box route

The recursive IPA has 15 or 16 sequential fold challenges. The direct
black-box special-soundness construction uses an accepting binary tree with
32,768 or 65,536 leaves to recover a complete opening. A direct proof would
require 2,621,440 accepting IPA leaves to extract all 48 chunks, before the
earlier `y`, `z`, `x`, and `ip_x` obligations. This observation is not an
impossibility proof. It means that a release theorem choosing plain-ROM
black-box extraction must supply an explicit multi-forking algorithm, runtime,
query bound, knowledge error, and its composition with the outer fork; a
single ordinary forking-lemma citation is insufficient.

## Front-end degree accounting only

The following values account only for visible front-end challenge
polynomials. They are not membership soundness, AoK, IPA, or end-to-end
security bounds:

```text
D1 = K1 + (n1 - 1) + 6
   = 98,304 + 32,767 + 6
   = 131,077,

D2 = K2 + (n2 - 1) + 6
   = 163,840 + 65,535 + 6
   = 229,381.
```

The terms provisionally account for `z` aggregation, the `y`-weighted gate
identity, and the degree-six `x` polynomial. A later proof must replace this
bookkeeping with an exact derivation and add the complete IPA algebra. No
security-bit field may be derived from `D1` or `D2` alone.

One direct proof contains 16 bound-one chunks and 32 bound-two chunks:

```text
membership chunks              = 48
membership wire bytes          = 75,858
front-end numerator             = 16 D1 + 32 D2
                               = 9,437,424
scalar challenge derivations   = 16*19 + 32*20
                               = 944
no-retry Keccak challenge calls = 1,888.
```

At the fixed ceremony cap of 10,336 direct proofs:

```text
bound-one chunks                = 165,376
bound-two chunks                = 330,752
all chunks                      = 496,128
front-end numerator             = 97,545,214,464
scalar challenge derivations   = 9,757,184
no-retry Keccak challenge calls = 19,514,368.
```

If a future theorem gives per-chunk extraction failures `kappa_1` and
`kappa_2`, the union terms are

```text
per direct proof: 16 kappa_1 + 32 kappa_2,
full ceremony:    165376 kappa_1 + 330752 kappa_2.
```

A reduction that embeds one challenge instance by guessing its target index
must also include the corresponding averaging loss: 165,376 possible
bound-one targets, 330,752 possible bound-two targets, or 496,128 targets if
the two shapes share one game. A genuinely global straight-line extractor may
avoid target guessing only if its game and reduction prove that fact; it must
not silently omit the multi-instance term.

## Outer composite-ROM and representation obligations

The outer challenge is not one direct 128-bit random-oracle output. The code
first queries Keccak-256 on a master frame containing all statement axes,
ordered commitment and membership roots, lineage, and all four RNS and
commitment first-message digests. It then makes four domain-separated queries
on

```text
coordinate_domain || seed_32 || ordinal_u8
```

and truncates each result to a `u32`. A composite-ROM theorem must specify
all five queries and show:

1. the master query is identifiable and fresh at the fork point;
2. all four coordinate inputs are distinct and fresh for the forked seed;
3. prequeries, repeated seeds, and master/coordinate domain collisions are
   included in the bad-event term;
4. the four coordinates are jointly uniform in the ideal model;
5. two independently programmed seeds yield the same four-coordinate vector
   with probability `2^-128`;
6. the adversary's master, coordinate, membership, digest, and generator-oracle
   query caps are stated separately or combined without double counting; and
7. equal digest roots imply equal underlying first messages unless the
   reduction outputs a Keccak collision.

For direct false-relation acceptance probability `epsilon` and a justified
master-query cap `Q_out`, the intended outer precursor has the form

```text
F_out >= epsilon * (epsilon / Q_out - 2^-128) - beta_composite_rom.
```

This formula is an obligation, not a proved bound.

After straight-line extraction of a bounded opening `(v, r)`, two accepted
outer forks with the same actual commitment first message and a differing
coordinate `d = c_j - c'_j` give

```text
sum_i (Delta z_i - d v_i) G_i + (Delta rho - d r) h = 0.
```

The reduction must either conclude coefficient equality under its exact MRep
game or output this nonzero relation. The response bounds give the corrected
integer-lift inequality

```text
2B + S = 288,230,367,494,668,290
       < 2^58
       = 288,230,376,151,711,744,

|d v_i| <= (2^32 - 1)*2 = 8,589,934,590.
```

These bounds are below T256 wrap and the required RNS half-modulus range. For
the same actual RNS first message,

```text
A Delta z - d u = 0 mod q_l.
```

Since `0 < |d| < 2^32 < q_l`, `d` is invertible in every release RNS limb.
Only after membership extraction, MRep binding, digest equality, and integer
lifting have all been justified may the reduction conclude `A v = u`.

The full composition theorem must state a bound of the form

```text
Adv_relation + Adv_MRep
  >= F_out
     - membership_extraction_failure
     - beta_Keccak
     - beta_H2C
     - beta_challenge_bias
     - beta_abort
     - beta_multi_instance,
```

with every term defined by a game. None of these terms is currently
certified.

## Zero-knowledge obligations

The required property is zero knowledge of the bounded opening of a fixed
external `C`, not merely hiding of a freshly generated commitment. A later
simulator must cover:

- the exact `AI/AO/S`, six-`T`, and recursive-IPA equations;
- programming all 19 or 20 scalar challenges with the exact two-query,
  nonzero-retry encoding;
- the 512-bit reduction bias and the 128-attempt exhaustion event;
- canonical-scalar rejection in prover randomness;
- all nonidentity proof-point aborts and whether the theorem is conditioned on
  success or proves witness-independent abort behavior;
- the nonidentity external-commitment precondition;
- ordered proof-set and verifier-transcript digests;
- the fact that simulated membership proof bytes feed the outer master seed;
- composite programming of the master and four coordinate queries; and
- multi-proof simulation for all 48 chunks and the 10,336-proof cap.

Functional round trips, byte-mutation tests, constraint exactness, and a pinned
generator digest do not establish any of these zero-knowledge properties.

## Required open gates

Release admission must keep these six obligations independently false:

1. external-commitment provenance is covered by the extraction game;
2. the full-basis MRep and CRS/H2C model is certified;
3. exact fixed-shape membership AoK is certified;
4. exact fixed-shape membership ZK is certified;
5. the composite master-seed plus four-coordinate ROM fork is certified; and
6. composition across 10,336 direct proofs is certified.

Backend implementation and the generator-basis KAT remain useful facts, but
they do not close any of these security obligations. This precursor must not
be hashed into a security-certificate field or treated as an independent
review artifact.

## Alternatives if the obligations cannot be closed

- A fixed-shape, globally algebraic joint creator/prover theorem can avoid
  nested rewinding without changing the wire, but it does not cover an opaque
  external `C` or a later independent prover unless those cases are separately
  proved.
- A plain-ROM theorem may retain the protocol only if it supplies a concrete
  multi-forking extractor and acceptable composed runtime and advantage.
- Adding transcript shape or theorem-suite fields is useful V2 hygiene but
  does not create opening knowledge.
- Adding extractor metadata to the wire either reveals the opening or creates
  another proof obligation; reduction-only AGM metadata changes no wire.
- A statistically binding one-point commitment cannot encode either fixed
  bounded-vector domain. Changing the commitment while preserving hiding and
  the linear outer reconstruction is a protocol redesign.
- Integrating all six membership vectors into the current backend requires
  `2^21` gates and has an 832 MiB scalar-vector lower bound before constraints,
  generators, or MSM storage, exceeding the governed workspace.
- A fifth outer repetition does not repair membership AoK and increases the
  maximum direct proof by 6,292,992 bytes, exceeding the current governed
  round-contribution proof budget by 4,279,177 bytes.
- If neither the exact AGM scope nor a concrete plain-ROM extractor is
  acceptable, the membership backend must be replaced with a protocol whose
  release theorem covers external commitments and composes straight-line with
  the outer relation.

Until one route is independently proved and reviewed, all six security gates
remain false and the public verifier remains unavailable.
