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
| `crates/iroha_zkp_halo2/src/generalized_bulletproof.rs` | `0d20ee5c5cca0b0f75d9ec02b54fe4e875b6a8826f7eee60a03a50b115265387` |
| `crates/iroha_zkp_halo2/src/vega/bulletproof_t256.rs` | `23399573b38fecf641004dff7219d42eb72b10bb35063fefb1b7fb48c440007f` |
| `crates/iroha_zkp_halo2/src/vega/curve.rs` | `c61fba4c0bb88c12b29380fe9589721e894cc4774e48d97ce8cb975114041c61` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/exact_eight_chunk_membership.rs` | `45035dd5f0e4ce8b7cf3e928100a82f515ebfc68e12a48036f9ae65ba462be23` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/direct_rkg_ephemeral_membership.rs` | `97a402017d8f34b650e3ebe7fd7021c791a21c26e4887aee082658cdebe19853` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1.rs` | `d570871a0747378bf3f50c81b5214de3d5914edac8106b18cfc769fff3be6a7e` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/predecode_v1.rs` | `a8efe9eb555491430975d7660609ee0e1f3ae93ee92db0baa5ef829e8592688c` |
| `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/active_exact_binding/direct_relation_wire_v1/predecode_v1/rkg_one_semantic_verifier_v1.rs` | `7bfd0586fd2bd76b89610357b9c5b04cf94cac3b0dca02b3660ad982a3a99841` |

`Cargo.lock` pins `halo2curves` 0.9.0, `keccak` 0.1.6,
`sha3` 0.10.9, and `tiny-keccak` 2.0.2. It does not pin an upstream
Bulletproof implementation from which an exact theorem can be inherited.

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
extractor must consume those global representations and the proof transcript
without rewinding, and must return all required openings or a full-basis
relation.

Calling only the later proof algorithm “algebraic” is not enough. The theorem
must define representation validity for the SHAKE/hash-to-curve CRS, every
group input and output, state handoff, corruption timing, and any honest-party
commitment that enters the game.

### Opaque external commitment and later independent prover game

If a later independent prover receives `C` as an opaque external group input,
its AGM metadata may express proof points relative to the input handle `C`
without revealing a representation of `C` in the CRS. No straight-line
opening extractor follows from that metadata alone. This game remains
blocked unless a separate proof extracts relative to an opaque `C`, the
commitment protocol exports an independently justified extractable state, or
the protocol is replaced. A theorem for the joint creator/prover game must not
be cited for this independent-prover game.

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
