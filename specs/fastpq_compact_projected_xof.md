# Projected raw-XOF outputs for compact H: lemma and design decision

2026-09-06. **Internally reviewed conditional extension; no production qualification.** No Rust, profile,
build or frozen digest change. The baseline typed ideal-H compiler argument is
[fastpq_compact_typed_compiler.md](fastpq_compact_typed_compiler.md).
Baseline 54-target arithmetic is in
[fastpq_compact_typed_profile.md](fastpq_compact_typed_profile.md).

**Recommendation for the next compact prototype:** use separately framed
SHAKE256 inputs for both compact commitment/chaining H and whole-message G,
with bounded rejection into the existing canonical six-field/48-byte root
container. Keep the frozen legacy digest implementation and its bytes intact;
the new compact profile/algorithm identity must distinguish the new hashes.
This removes dependence on the unsupported bespoke six-lane-combiner claim.
It replaces that assumption with an explicit SHAKE256 ideal-XOF assumption;
it does not make concrete SHAKE mathematically random or discharge independent
cryptographic review.

[The FIPS 202 primary record](https://csrc.nist.gov/pubs/fips/202/final) specifies
SHAKE256 and currently records a planned revision. Standardization is not a
qROM indifferentiability theorem for the fixed Keccak permutation. No concrete
quantum instantiation error is set to zero here. The local
`crates/iroha_crypto/Cargo.toml` already depends on sha3, and its source already
uses Shake256 in FHE and handshake code; a shared helper needs no new crate or
lockfile change.

## 1. H must not be modeled as perfectly uniform merely by conditioning

For a fixed H input, obtain c independent u64 words from one uniform raw
XOF tape. Select the first six values below p, preserve their order and
encode them as six u64 little-endian field elements. If fewer than six
qualify, return ABORT and propagate failure; do not substitute a digest,
silently retry another domain/counter, or loop without a bound.

Set r=(2^32-1)/2^64. The exact failure probability is

```
eta_H(c) = sum_(a=0..5) binom(c,a)*(1-r)^a*r^(c-a).
```

A simple rational upper bound is

```
eta_H(c) <= binom(c,c-5)*r^(c-5).
```

For every root y in C=Fp^6,

```
Pr[Decode_H(U)=y] = (1-eta_H(c))/p^6.
```

The equality follows because accepted coordinate values are iid uniform and
independent of the acceptance indicators, including the event that enough
coordinates were found. The output law on C union {ABORT} has total variation
distance exactly eta_H from the total uniform-C law. Thus conditional
uniformity alone does not supply a total ideal H oracle.

| H candidates | Raw bits / bytes | Certified eta_H upper bound |
| ---: | ---: | ---: |
| 12 | 768 / 96 | <2^-214 |
| 14 | 896 / 112 | <2^-277 |
| 16 | 1,024 / 128 | <2^-339 |
| 18 | 1,152 / 144 | <2^-402 |

Twelve candidates are adequate for the projected-output proof below and have
a very small honest failure bound. **Sixteen candidates are the preferred
implementation candidate here**: their 128 bytes still fit one 136-byte
SHAKE256 output block, while giving a larger abort margin even under the loose
interface-only coupling calculation. This is a design tradeoff, not a measured
performance optimum. Eighteen cross that output-block boundary. No candidate
is installed here.

## 2. Why a decoded-H-only coupling is not enough

For an interface exposing only decoded H results, one can couple uniform
roots to an independent Bernoulli eta_H abort mark per input. Replacing the
marked inputs gives a conservative Q-query quantum distinguishing bound
2*Q*sqrt(eta_H). Proof: in the reference execution, query amplitudes are
independent of the random marks; the expected squared norm of the difference
on any query is at most 4*eta_H. A hybrid and Cauchy-Schwarz give expected
state-vector distance at most 2*Q*sqrt(eta_H), which bounds measurement
probability differences. The reference operation and marks remain independent
because the reference oracle contains no abort marks.

At Q=2^32+44,584, after a conservative 54-target union, exact squared-rational
checks give:

| H candidates | 54*2*Q*sqrt(eta_H) certified below |
| ---: | ---: |
| 12 | 2^-68 |
| 14 | 2^-99 |
| 16 | 2^-131 |
| 18 | 2^-162 |

This is a deliberately loose distinguishing bound, not an attack or actual
soundness loss. Crucially, public SHAKE permits access to the **full raw tape**.
An interface-only coupling cannot simply hide that auxiliary output. Simulating
raw tapes conditioned on their decoded roots in quantum superposition would
need an additional argument. The direct raw-oracle proof below avoids this
gap and includes raw-tape access from the start.

For a different event, an adversary trying to output an H input whose raw tape
aborts, the same marked-input hybrid gives probability at most
(2*Q+1)^2*eta_H: the reference final output is marked with probability eta_H,
and its success amplitude can increase by at most 2*Q*sqrt(eta_H).
This applies as well to raw tapes by coupling each input's tape to independently
sampled successful/failed conditional tapes and its independent abort mark.
It is an availability/search bound, separate from false-proof acceptance.

## 3. Projected-output compiler lemma

Use independent ideal raw binary oracles U_H and U_j. For the simplest finite
model their outputs have R_H and R_j bits, respectively. Define

```
h_D(x) = Decode_H(D_UH(x)), when this exists and is not ABORT
g_Dj(x) = D_Uj(x)                 // the entire R_j-bit protocol tape.
```

The adversary accesses U_H, including unused/rejected words, directly.
The compiler uses h_D's canonical 48-byte projection for all roots and state
pointers. No claim that U_H and its projection are independent is made.

Replace the collision property B in the reviewed proof by:

* two distinct U_H inputs with the same **successful decoded H root**; or
* two distinct U_j inputs with the same complete protocol tape of type j.

Aborted H inputs do not create a decoded root. The extractor inverts h_D,
not the raw H image, and inverts g_Dj for tapes. Outside B these inverses have
at most one input. Roots, contexts, round checks and pointer sets are otherwise
exactly those of the baseline proof. An H entry's raw output is available when
building its inverse table; its input supplies the same typed child/root fields.
The first-difference extraction lemma now says that an insertion changes a
short-endpoint extraction only if its **successful decoded output** hits a
short pointer or endpoint. If it aborts it introduces no inverse entry and
changes no extraction.

For a uniform raw U_H insertion and any set S of roots,

```
Pr[Decode_H(U_H) in S] = (1-eta_H)*|S|/p^6 <= |S|/p^6.
```

Therefore the short collision/attachment case satisfies

```
I_H(R,T) <= 3*(T-1)*(1-eta_H)/p^6 <= 3*(T-1)/p^6.
```

The full-tape G cases retain

```
I_Gj(R,T) <= epsilon_j + (T-1)/2^R_j.
```

Both directions follow as in the baseline proof. Inserting an aborted H value
does not change R; deleting one does not change R. Root collisions are still
insertion-monotone. A later successful entry sharing some raw prefix with an
aborted entry does not attach anything to that aborted entry, since it has no
successful decoded image.

The compressed oracle now acts on the **raw binary output alphabets**. The
finite-group lifting only needs the two-sided insertion probability under
each uniform raw alphabet; it never requires that the extractor store the
whole raw value as its short pointer. Consequently its 6*T^2 bound applies
with the same conservative delta as before.

Verifier expansion must list every actual U_H raw output, as well as U_j
outputs, and check that each claimed root equals its successful H projection.
A winning expansion yields exactly the same decoded inverse table, hence
the same accepting partial IOP transcript outside B. The oracle-to-database
comparison uses the raw alphabet sizes:

```
K_raw <= (# U_H calls)/2^R_H + sum_j (# U_j calls)/2^R_j
Pr[false acceptance] <= (sqrt(6*T^2*delta)+sqrt(2*K_raw))^2.
```

**There is no additive eta_H soundness error in this projected model.**
Aborts cause rejection and their successful-root preimage densities are already
bounded. This statement is different from claiming that finite-tape H is a
total uniform-C oracle. Honest failure and malicious abort-search probabilities
must still be recorded for availability.

Every raw output type is binary, so binary raw-XOF queries are already group
XOR queries: the baseline factor-two simulation is unnecessary for this
stronger raw-access formulation. Retaining it gives a conservative comparison
with the baseline profile arithmetic. Removing it changes T's stated meaning
and requires a separately labeled calculation, not an unnoticed improvement.

## 4. Extra raw XOF suffixes and domain framing

The ideal model must allow the adversary the raw XOF access that a public
SHAKE implementation actually offers. Encoding the intended output length
in the input separates different *framed inputs*, but it does not prohibit
someone squeezing a longer output on exactly the same bytes.

Every byte input outside the recognized protocol-frame domains is assigned
to an independent auxiliary raw-XOF type. This includes arbitrary public
SHAKE queries that the baseline typed model represented by fixed responses
outside its declared types. Auxiliary entries do not supply extraction
inverses, root/tape collision witnesses or parsed protocol pointers; R ignores
their values, so an auxiliary insertion has instability zero. Their queries
still count in T. In the ideal-XOF model their domain is disjoint from the
protocol inputs, so adding this auxiliary type gives the full public input
surface without pretending that those responses are fixed or unavailable.
Oracle-dependent contexts selected using those counted auxiliary queries
remain covered only under the adaptive-context extension's other premises.

For any finite adversarial output cap R_star at least all required protocol
lengths, expose one R_star-bit raw output per framed input. Define U_j's
protocol tape as its R_j-bit prefix, and H as decoding its first R_H bits.
Count G collisions on the protocol-prefix projection and H collisions on
successful decoded roots. Each G prefix has probability 2^-R_j; each short H
root has probability (1-eta_H)/p^6. Thus all insertion bounds above remain
unchanged. The extractor inverts these projected values, and B excludes
collisions in those projections. Verifier expansion may query/list the full
R_star outputs in the mathematical reduction. Its comparison denominator
only improves to 2^R_star; this does not imply actual verifier code must
squeeze/store that unused suffix.

This finite-cap extension accommodates extra raw suffix information without
claiming that it was bound into the chain. The chain binds the complete
**protocol tape**, including all rejected and unused samples within its R_j
bits; it need not bind arbitrary additional oracle output outside that tape.
This is another projected-output argument, not an assertion that one full
raw value has a unique inverse after arbitrary truncation.

Actual query/work accounting must bound coherent input and output operations.
Arbitrarily many output bits at zero cost is not a concrete SHAKE work model.
A variable-length ideal XOF can be represented at each finite resource bound
by a sufficiently large R_star, with input encodings prefix-free and all roles,
profile, context, round and requested protocol output length unambiguous.
For the binary query-count simulation, a shorter prefix-XOR query can use one
full-output phase query: Fourier-transform its response, coherently copy only
the requested prefix frequencies into a zero phase register, query, uncopy,
and undo the transform. The unused frequencies are zero. This remains coherent
when requested prefix lengths are in superposition and adds bit/gate work,
not another ideal-oracle query.

## 5. Exact arithmetic and candidate choice

The companion `../scripts/fastpq/check_compact_projected_xof.py` checks the projection
distribution by enumerating 36,928 small-field raw tapes, including extra
discarded candidates, and certifies the displayed abort bounds with exact
rational arithmetic. At Q=2^32+44,584, the 54-target quantum abort-search
bounds are below 2^-142 for 12 H candidates and 2^-268 for 16 candidates.

A complete prover's same-geometry commitment trees plus chain have at most
4,194,299 logical H invocations: three trees of 2L-1 nodes, the FRI trees of
2*2^d-1 nodes for d=2,...,18, two terminal hashes, and 21 chain hashes.
The honest 54-attempt H-abort union is below 2^-186 for 12 candidates and
2^-312 for 16 candidates. This counts the proposed proof construction, not
debug conversions or repeated self-verification. Extra work must be charged.
The G/query-tape abort bound from the baseline table dominates honest failure.

With **raw binary XOF access** counted directly (factor one), the smallest
position count certified by the same 54-target bound is 371:

```
T=2^32+44,196=4,295,011,492
832/1024 < 54*bound*2^128 < 833/1024.
```

At 370 the interval is (1211/1024,1212/1024), and its unavoidable query term
already exceeds the target for every smaller count. The projected valid wire
upper bound at 371 is 4,284,295 bytes per segment. Retaining the baseline
factor two conservatively keeps the smallest certified count at 375. These
are different explicitly stated query models, not a changed initial-position
meaning or hidden omission of verifier work.

For a first implementation candidate, **375 positions with 16 H candidates**
retains the more conservative arithmetic decision and gives additional ideal
margin in the direct raw-XOF model. It uses the same 4,326,227-byte valid shared
wire projection and the same G tape table. This choice does not allocate away
the unknown concrete SHAKE term or prove optimality; it is ready for review
and test-only implementation, not production acceptance.

## 6. Concrete implementation gap and suggested next action

Under ideal raw XOFs, the previous lemma is a mathematical route to the desired
compact H/G interface with canonical 48-byte roots and bounded failure.
For actual SHAKE256, write an explicit residual term:

```
Pr[false acceptance with concrete SHAKE]
 <= conditional_ideal_bound + Delta_SHAKE(total coherent work, frame family)
```

Here Delta_SHAKE is the distinguishing/instantiation advantage for the exact
joint H/G framed-input construction and all permitted raw-XOF access. This
document supplies **no numerical bound** for it. It must not be inferred from
output width, a single independent-G assumption, or the withdrawn SHA-3
indifferentiability preprint. Separately verifying the projections does not
make H and G independent when their source framing aliases; injective typed
framing is a required input to the idealization.

Keeping the current six-lane digest for compact H leaves both its bespoke
combiner qualification and the G/SHAKE assumption outstanding. Moving the
new compact profile to jointly framed SHAKE H/G gives one standard primitive
to review and a direct raw-oracle theorem route. It does not alter the legacy
digest, and it does not by itself establish concrete security, zero knowledge,
knowledge extraction, or public-state authority.

After independent review of this lemma, a bounded reusable SHAKE helper and
typed compact-specific H wrapper can be staged in existing crates. Require
known-answer/domain-collision tests, canonical decode and abort propagation,
CPU/hardware byte parity, full witness-dropped proof verification and revised
wire/allocation/runtime evidence before admission. A concrete security review
must approve the final joint SHAKE assumption and resource model.

## Reproduction

Run `python3 scripts/fastpq/check_compact_projected_xof.py` from the repository
root. It accepts `--output PATH` and uses the tracked exact profile arithmetic
module, source geometry/layout checks and source hashes. It needs no retained
proof, paper, target-only note or network access. These are conditional
arithmetic and finite distribution controls, not a SHAKE security certificate.
