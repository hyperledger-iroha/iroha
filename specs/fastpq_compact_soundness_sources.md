# Compact FASTPQ: primary-source soundness map

Reviewed 2026-09-06 against the [implemented contract](fastpq_compact_protocol_contract.md) and [profile analysis](fastpq_compact_profile_analysis.md). This is a bounded applicability review of five papers, not a security reduction or parameter qualification. No protocol, profile, admission limit or production caller changes are proposed here.

The target is the exact one-delta prototype: `N=65,536`, evaluation domain `D=aG` for a subgroup `G` of size `L=8N`, 342 base columns, 923 numerator slots, challenges in `K=F_p[u]/(u^4-7)`, mixed trace bound `<N`, quotient/joint bound `<2N`, 17 binary folds and the entire four-element terminal vector checked at degree `<1`. There are 136 distinct initial queries. The newly bounded sampler returns an error after at most 1,088 digest draws; its implementation does not establish a distributional security bound.

## Five applicable references and their limits

### 1. Original FRI: a starting IOPP, not a complete AIR theorem

Ben-Sasson, Bentov, Horesh and Riabzev, [*Fast Reed-Solomon Interactive Oracle Proofs of Proximity*](https://drops.dagstuhl.de/storage/00lipics/lipics-vol107-icalp2018/LIPIcs.ICALP.2018.14/LIPIcs.ICALP.2018.14.pdf), ICALP 2018: **Theorem 2 and the smooth-code remark in §1.1.3; §2.1**.

The smooth multiplicative-domain construction uses the two-to-one square map and random linear combination of even/odd parts. This matches FASTPQ's `(x,-x)` fold algebra; scaling a nonzero coset and embedding its points in `K` are the required coordinate identifications. The joint oracle has rate `1/4`.

The theorem bounds rejection of a word already known to be a specified distance from the Reed–Solomon code. It does not establish that a false FASTPQ statement produces such a word. Its proven distance range is explicitly limited; Conjecture 3 is not a replacement bound. The paper's terminal/protocol conventions must also be reconciled with the exact four-entry check. Separate commit-phase failure from query-phase failure before repetition: an unconditional one-query error cannot simply be raised to the 136th power when commitments/challenges are shared.

### 2. Correlated agreement: the relevant missing row-batching tool

Ben-Sasson, Carmon, Haböck, Kopparty and Saraf, [*On Proximity Gaps for Reed–Solomon Codes*](https://www.math.toronto.edu/swastik/rs-proximity-gaps-2025.pdf), author manuscript dated November 11, 2025: **Theorem 1.5; §4.1; Theorems 4.2–4.3; Corollary 4.4; Theorem 4.5**. The PDF title/date, rather than the URL's older filename, identify the reviewed version.

These results concern joint agreement of several words with low-degree polynomials, including agreement on specified sets and weighted/subspace variants. They are candidates for recovering one consistent row assignment from post-commitment mixtures. Their degree parameter `k` is inclusive, so FASTPQ's bounds translate to `k=N-1` or `2N-1`.

The explicit curve theorems use `u_0+z*u_1+...+z^M*u_M`; FASTPQ samples independent `lambda_j` and independent `rho,sigma`. §4.1 discusses general linear combinations, but its constants and hypotheses still need instantiation for this sampling and the coupled words `Q,T,X^N*T`. No near-capacity conjecture or theorem for randomly chosen evaluation sets applies automatically to our fixed subgroup coset. This paper is a tool for the missing reduction, not that reduction.

### 3. DEEP-FRI/DEEP-ALI: useful comparison with unmet protocol steps

Ben-Sasson, Goldberg, Kopparty and Saraf, [*DEEP-FRI: Sampling Outside the Box Improves Soundness*](https://drops.dagstuhl.de/storage/00lipics/lipics-vol151-itcs2020/LIPIcs.ITCS.2020.5/LIPIcs.ITCS.2020.5.pdf), ITCS 2020: **§4.1/Theorem 4; §4.2/Theorem 8; §5.2/Theorem 15 and Protocol 17**.

The original-FRI discussion separates commit randomness from repeated query tests. DEEP-FRI and DEEP-ALI then introduce a verifier-selected field point and claims used to quotient the original words; Protocol 17 checks linked witness/constraint claims through those derived proximity tests.

FASTPQ has no such out-of-domain challenge/opening or DEEP quotient. Its division by the fixed trace vanishing polynomial `X^N-1` is a different operation. Therefore Theorem 8's improved DEEP-FRI guarantee and Theorem 15's DEEP-ALI composition bound cannot be assigned to this implementation. These sections identify a concrete missing hypothesis whenever a DEEP bound is suggested; adopting that protocol would be a separate reviewed change, not a parameter calculation.

### 4. IOP-to-QROM compilation: the strongest directly relevant candidate

Chiesa, Manohar and Spooner, [*Succinct Arguments in the Quantum Random Oracle Model*](https://eprint.iacr.org/2019/834.pdf), full version dated January 14, 2020: **§8.2/Remark 8.2; Definitions 8.3–8.4; Theorem 8.6; §8.6**.

Theorem 8.6 gives the BCS compiler a bound of the form `O(t^2*epsilon + t^3/2^lambda)`, with verifier work included in the oracle-query accounting. Its premise is round-by-round soundness of the underlying IOP. Here `t` is an adversary/oracle-work parameter, not FASTPQ's 136 query positions.

The full version distinguishes a chain containing verifier messages from the original BCS chain; §8.6 requires stronger soundness for the latter. FASTPQ updates its full transcript state on every challenge, but a correspondence for its extra append hashes, vector challenges, framed domains and sampler has not been proved. The conditional [IOP state function](fastpq_compact_round_by_round.md) is now recorded for row mixing, quotient aggregation and FRI; it still needs an exact concrete-compiler mapping. The theorem assumes an ideal random oracle: it does not justify treating the concrete six-lane sponge as one, or replacing its range by 384 uniform bits.

### 5. Generic multi-round Fiat–Shamir: an alternative with different losses

Don, Fehr and Majenz, [*The Measure-and-Reprogram Technique 2.0: Multi-Round Fiat-Shamir and More*](https://eprint.iacr.org/2020/282.pdf): **Definition 9, Definition 11/Remark 12, and Corollary 13 in §5; Remark 14**.

Corollary 13 covers adaptive adversaries against the specified multi-round public-coin transformation. Its simulator success has factor `n!/(2q+n+1)^(2n)` and an additive term summing to `n!/|C|`; `q` counts adversarial random-oracle queries and `n` counts challenge rounds. This is not a universal quadratic-loss theorem for an arbitrary number of rounds.

FASTPQ must first be expressed as the underlying public-coin protocol with a proved quantum soundness guarantee. Merkle-root messages alone do not supply that guarantee. The paper's uniform challenge space `C` and specified hash chain also need a mapping to field-valued vector challenges and bounded rejection/dedup sampling. No round count or concrete loss is assigned here by treating each internal hash invocation, or each FRI query, as a protocol round without justification.

## Concrete mathematical work remaining

These are obligations inferred from the code and the hypotheses above, not claims made by the papers about FASTPQ:

1. **Review the concrete relation against its formal definition.** The conditional AIR reduction now fixes public-statement-dependent numerators and oracle chronology with arbitrary malicious row/`T`/`Q`/FRI words. Verify the source's semantics and numerator degree ledger against that relation. Caller authority and source-root finality remain external premises.
2. **Externally review correlated recovery and linkage.** The reduction derives a common row set, base-field recovery, pre-alpha uniqueness, coupled `Q,T,X^N*T` proximity, and enough current/next overlap at agreement `11/16`. Separate internal reviews found no defect under its premises; this is not external cryptographic qualification.
3. **Map the ideal state to the concrete compiler.** The [round-by-round lemma](fastpq_compact_round_by_round.md) supplies an explicit state satisfying CMS Definitions 8.3–8.4 for the ideal grouped-message IOP, including partial prover words. The actual sequential hash expansion produces vectors and query subsets through many calls; these cannot be identified with one digest output, and splitting the sampler calls into rounds loses the lemma's small final-query bound. A compiler-compatible expansion or exact new reduction is still required.
4. **Analyze exact query randomness and commitment representation.** Derive the distribution of the first 136 distinct accepted indices, the bounded sampler's abort event and all shared descendant groups. Prove shared multiproofs/terminal duplication preserve the relevant oracle checks. Count commitment failures separately from algebraic/proximity errors.
5. **Complete the compiler and concrete hash accounting.** Map the exact transcript to one theorem, including full digest state versus four-lane challenges, and count oracle work, adaptive statements, repeated attempts and multiple proof/profile targets. An ideal range of `F_p^6`, a concrete digest assumption and a 384-bit string oracle are distinct claims. Only then combine explicit constants against the aggregate 128-bit qROM target.

The downloaded primary PDFs and searchable text are retained locally in `target/fastpq-production-validation/soundness-paper-text/` for theorem-pointer review; this document depends on the linked primary publications, not those optional local artifacts. No tests, benchmark timings, digest width or increased query count discharge these obligations.

## Conditional components now recorded

The [row-linkage lemma](fastpq_compact_row_linkage.md) supplies deterministic base-field/shift/intersection/root-count arguments and conditional numerator batching. The [sampler lemma](fastpq_compact_query_sampler.md) supplies exact uniform-subset/abort calculations under iid ideal draws. The [formal AIR interactive reduction](fastpq_compact_air_bound.md) supplies the missing proximity and extraction chronology under explicit degree/semantic and ideal-oracle premises, with a reproducible 237-query conditional bound. These components do not establish concrete-oracle premises, replace the compiler mapping, or qualify a profile.


The separately derived [typed whole-tape compiler](fastpq_compact_typed_compiler.md)
now supplies explicit ideal-model constants under its stated extractor,
state and expansion premises. Its [adaptive-context extension](fastpq_compact_adaptive_context.md)
and [projected raw-XOF construction](fastpq_compact_projected_xof.md) address
additional ideal-model interfaces. These are new internally reviewed arguments,
not constants attributed to the cited papers. The
[375-position candidate arithmetic](fastpq_compact_typed_profile.md) retains the
explicit adversary/verifier query accounting and 54-target union. Current
multi-call challenge expansion, concrete primitives and production admission
remain outside that qualification.
