**The Pixel results establish performance of particular kernels, not security of the proposed payment system. Path 2 is a credible research direction, but the supplied construction is not yet justified for real money.** The most urgent cryptographic issue is whether the single IPA binds **each earlier sumcheck commitment to its own polynomial before its challenges**. Proving only an aggregate commitment does not establish that property.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

There are also two independent limits: today’s monetary model trusts the wallet/OS to prevent state forks, and today’s public payment fields reveal identifiers that zero-knowledge proofs cannot hide.

I read the selected design, security specifications and referenced implementation/test paths. This was read-only; I changed nothing and ran no builds or test suites. I evaluated (a)–(d) as supplied, without a located implementation of that spike.

Below, **established** means a source fact or a conditional mathematical result; **conjectured** means an explicit cryptographic assumption; **open** means a proof or qualification obligation remains.

**1. Threat model and the monetary guarantee**

The cryptographic adversary must be allowed to:

- Control its prover, witnesses, randomness and ordinary keys.
- Generate arbitrary QR bytes, statements, commitments, transcripts and proofs.
- Choose inputs adaptively, collude with counterparties, replay evidence, and grind through failed proof attempts.
- Interrupt execution, restart, restore backups, submit concurrent operations and exploit ambiguous storage outcomes.

An honest verifier must nevertheless establish authorization, integer conservation, correct state transitions, consumed-credit uniqueness, and every predecessor/incoming-proof obligation before releasing spendable value.

However, **arbitrary control of an enrolled phone and its payment key is stronger than today’s monetary threat model**. The specification assumes correct released wallet code, an uncompromised stock OS, protected storage and a hardware-backed nonexportable key. It expressly excludes modified wallet executables and compromised operating systems. It also states that lineage proofs cannot discover another valid fork. [Trust assumptions, lines 147–166](../../../specs/kagemusha_single_design_proposal.md)

A concrete counterexample needs no proof-system failure: copy a funded state and its signing capability; produce two individually valid Sends from that predecessor to different recipients. Each proof can establish conservation along its own branch while aggregate spending exceeds the original balance. The specification explicitly acknowledges this, including that nonexportability alone does not prevent forks. [Forking limitation, lines 333–356](../../../specs/kagemusha_single_design_proposal.md)

There is a similar funding boundary: current Load verifies BLS finality **natively before Advance**; the recursive relation binds the receipt and monetary effect but does not prove that finality check. Funding assurance consequently depends on the trusted application/provider. [Load boundary, lines 1597–1609](../../../specs/kagemusha_single_design_proposal.md)

Therefore the security claim must distinguish:

| Guarantee | Necessary assumptions |
|---|---|
| No acceptance of an invalid proved transition | Knowledge soundness, exact relation, authenticated parameters and complete verification |
| No spending of unproven value | Release ordering and durable state machine, plus complete verification |
| No duplicate Receive or ledger payout | Authenticated replay state, atomic updates and once-only settlement |
| No competing offline spends from one state | Trusted anti-fork custody mechanism; proofs alone do not provide this |
| Valid funded origin | Authenticated issuance/finality, including today’s native-only Load boundary |

If the required guarantee includes a fully malicious enrolled phone with usable keys, the current custody/finality assumptions must change.

For Path 2, preserve REPORT4’s stronger ordering:

> preparation proof → serialized Advance/receipt → completed primary and secondary proofs → full native verification → durable exact output bytes → release.

There is no authorization for local-credit shortcuts or outstanding proof work after payout. [REPORT4:32](REPORT4.md)

Privacy must also be stated relative to an explicit leakage function. Today’s public lineage exposes wallet/key/credential identifiers, policy facts and several roots; Request/Payment expose the current amount and counterparties’ identifiers. Thus the attainable claim is hiding balances and **additional historical lineage/counterparties**, conditional on those public fields—not anonymity of the current payment or unlinkability of stable wallet identifiers. [Public lineage, line 830](../../../specs/kagemusha_single_design_proposal.md), [wire fields, line 917](../../../specs/kagemusha_wallet_wire_v1.md)

**2. Knowledge soundness at the composition boundaries**

The acceptance predicate must cover the **whole pair**, not two independently valid proofs.

| Boundary | Failure mode | Required rule |
|---|---|---|
| Primary Fp/Vesta ↔ secondary Fq/Pallas | Valid proofs describe different operations, predecessors or group equations | Both constrain the same authenticated operation context, ordered task list, keys, exact predecessor bytes and accumulator outputs |
| Base case ↔ recursive step | Bootstrap/reset selected midway through a lineage | Authenticate the base case and constrain lifecycle/ordinal progression; no prover-controlled reset |
| Application ↔ CCS | Missing range, copy, authorization or map constraint | Prove the complete integer/state relation; field equality alone does not establish monetary conservation |
| CCS ↔ matrix evaluations | Prover supplies an evaluation of a different matrix, or “fixed VK” substitutes for evaluation | Compute authenticated uniform matrix MLEs exactly, including wiring exceptions, padding and public-input positions |
| Witness ↔ rectangular Hyrax | Wrong row/column order, altered dimensions, unconstrained padding or tail overlap | Pin dimensions and indexing; commit rows before evaluation challenges; derive the exact row-combination and column-evaluation vectors |
| Sumcheck ↔ IPA | Final scalar equations use values unrelated to earlier commitments | Extract openings consistent with **every** committed round and the committed witness, with the correct commitment timing |
| IPA ↔ generator claim | Succinct equation passes with a fabricated folded generator | Decide the exact folded-generator equation against pinned generators |
| Old claims ↔ accumulation/restructuring | Omitted claim, altered source width, unrelated restructuring opening, or unconstrained mask correction | Bind every ordered source and every intermediate statement; prove the restructuring opening and decide its resulting claim |
| Bytes ↔ field/group objects | Modulus aliases, wrong curve, inconsistent lengths or dummy-decoder substitution | Canonical parsing and injective constrained encodings of the original bytes |
| Fiat–Shamir ↔ all preceding objects | Omitted statement/input/commitment permits adaptive substitution | A reviewed, typed, domain-separated transcript schedule binding everything before the challenge that depends on it |

Three boundaries deserve particular emphasis.

**Curve-cycle closure — open for this exact design.** Nguyen, Boneh and Setty’s attack was not a break of Pasta or the underlying folding algebra. The two-curve construction failed to connect the relevant intermediate and terminal instances tightly enough. Their repaired Nova construction proves a particular linked system; it does not establish security of arbitrary “CycleFold-style” composition. Path 2 must explicitly connect every scalar check to its corresponding group equation and every predecessor accumulator to the actual successor accumulator. [Nguyen–Boneh–Setty, 2023, §7.2](https://drops.dagstuhl.de/storage/00lipics/lipics-vol282-aft2023/LIPIcs.AFT.2023.18/LIPIcs.AFT.2023.18.pdf)

REPORT4 requires a shared operation/task identity and avoids circularly hashing the two complete current proofs. Those requirements need a precise acyclic relation and extraction argument. CycleFold’s small secondary circuit estimate concerns a scalar multiplication, not a complete IPA verifier. [REPORT4:34](REPORT4.md), [CycleFold](https://eprint.iacr.org/2023/1192)

**Uniform matrix evaluation — established requirement, implementation open.** An arbitrary fixed sparse matrix can still require linear work to evaluate inside recursion. Either prove its evaluation using a SPARK-like mechanism or compile genuinely uniform wiring with a small exact evaluation circuit. A preprocessing file containing challenge-dependent “answers” is not a substitute. This distinction is explicit in REPORT4 and in the roles of computation commitments/SPARK in Spartan and uniform CCS in SuperSpartan. [REPORT4:52](REPORT4.md), [Spartan](https://eprint.iacr.org/2019/550), [CCS](https://eprint.iacr.org/2023/552)

**Native generator decisions — established and indispensable.** Today’s verifier documentation supplies an explicit attack: write arbitrary well-formed messages, then solve the succinct equation for its unabsorbed generator suffix \(G\). Succinct accumulation returns success although the statement is false. Only checking

\[
G=\langle s(u),g\rangle
\]

closes that obligation. [IPA verifier:20](../../../crates/iroha_plonk/src/pcs/ipa/verifier.rs)

Every Path 2 acceptance must decide **both own opening claims and both inherited claims**. Neither a successful parser, a successful succinct equation, nor a cached unverified claim is acceptance.

Current restructuring first proves a linear relation concerning the relaxed tensor, then opens the **same commitment** at the derived MLE point. Its masked opening binds the private statement and masks through an opened commitment \(D\), applies the exact blinding correction, and returns another undecided claim. Dropping any link breaks the argument. [Claim relation and restructuring:3](../../../crates/iroha_fold/src/claims.rs), [masked restructuring:15](../../../crates/iroha_plonk_recursion/src/restructure.rs), [verification:245](../../../crates/iroha_plonk_recursion/src/restructure.rs)

Canonical encoding must include original-modulus checks on foreign scalars, curve membership, exact limb ranges, lengths and point-role rules. Hashing a reduced scalar instead of its canonical original integer permits aliases. Soft decoding must still bind the malformed original bytes, not merely a substituted valid dummy. [Encoding requirements:78](../../../specs/kagemusha_recursion_soundness_v1.md)

The transcript review must cover relation/version, both curves and parameter identities, dimensions, public inputs, ordered tasks, commitments, evaluations, masks’ binding commitments, L/R messages and final Schnorr commitment. Frozen Heart demonstrates why omitting public inputs is a soundness failure, even when the algebra is otherwise correct. [Trail of Bits’ original analysis](https://blog.trailofbits.com/2022/04/18/the-frozen-heart-vulnerability-in-plonk/)

**3. Unbounded lineage**

**“Every operation verifies its predecessor” is an invariant to prove, not a sufficient knowledge-soundness theorem.**

The desired argument needs:

1. A well-founded base/step relation with authenticated initial funding and no mid-lineage reset.
2. Extraction of the actual predecessor/incoming proofs and state-transition witnesses.
3. Exact two-field closure and secure accumulation of every deferred obligation.
4. A composition theorem whose extractor remains efficient for the claimed execution model.
5. One consistent model for transcript hashing, recursive verification and adaptive adversaries.

Naively applying a single-proof extractor repeatedly can multiply its runtime at every level. Nguyen–Boneh–Setty explicitly warn that constant multiplicative extraction overhead can become superpolynomial after logarithmic depth. Receive also joins histories from multiple wallets, making PCD a more faithful model than a single linear IVC chain. [Their Remark 2](https://drops.dagstuhl.de/storage/00lipics/lipics-vol282-aft2023/LIPIcs.AFT.2023.18/LIPIcs.AFT.2023.18.pdf)

The relevant foundations are [BCMS20 accumulation-based PCD](https://eprint.iacr.org/2020/499) and [BCLMS21 split accumulation](https://eprint.iacr.org/2020/1618). Their hypotheses must be checked for the actual construction. In particular, a proof about a protocol using a random oracle does not automatically cover a recursive circuit that evaluates a fixed concrete implementation of that oracle. [Halo Infinite](https://eprint.iacr.org/2020/1536) identifies this non-black-box Fiat–Shamir issue.

Today’s specification is appropriately explicit:

- It does not claim an arbitrary-depth reduction.
- It selects an additional recursive Pasta/Poseidon Fiat–Shamir heuristic.
- That heuristic supplies neither a quantitative arbitrary-depth bound nor a privacy theorem.
  [Current theorem boundary:18](../../../specs/kagemusha_recursion_soundness_v1.md)

**Path 2 does not inherit that selection automatically:** today’s selection names the existing parameters, relations and formats. The replacement needs either an applicable proved composition theorem or an explicitly stated new recursive-instantiation assumption.

Operationally unbounded should mean **no protocol-imposed depth cap for finite feasible histories**. It cannot mean a fixed nonzero per-attempt failure probability remains bounded over infinitely many attempts. State finite lifetime budgets for operations, adversarial attempts and hash queries.

**4. Zero knowledge and its costs**

The complete exported view must be simulatable from the approved public leakage.

| Object | What an unmasked version can reveal | Required protection |
|---|---|---|
| Sumcheck polynomials | Witness-dependent partial sums and correlations | A proved masking-polynomial construction, or hiding committed rounds with sound opening/extraction |
| Hyrax rows | Deterministic witness fingerprints; tests of low-entropy guesses | Fresh independent Pedersen blinds |
| Terminal \(v_A,v_B,v_C\) | Three witness-dependent evaluations | A proved joint masking construction |
| Witness evaluation | Another linear observation of the witness | Keep it inside a hiding opening/relation, or account for sufficient masking rank |
| IPA L/R and final scalar | Cross terms and a folded witness linear form | Fresh L/R blinds and a ZK final opening |
| Inherited claims | Exact ancestry tags or recoverable old commitments | Private linkage plus fresh, jointly hiding restructuring |
| Recovery/retry artifacts | Deterministic witness-derived identifiers or nonce reuse | Whole-protocol privacy analysis, fresh attempts and exact retained-byte recovery |

The literature supports several approaches, with different costs:

- **Masking-polynomial ZK sumcheck:** established technique, but the chosen mask degree, commitments and extra openings must be specified. A degree increase can invalidate the 81-value grid. There is no defensible exact byte/time price until the variant is fixed.
- **Committed sumcheck messages:** Hyrax explicitly develops hiding committed messages and combined linear checks. Its opening/extraction machinery matters; replacing it with a single new aggregate IPA requires a new argument. [Hyrax, §§4–6](https://www.pepper-project.org/hyrax-oakland18.pdf)
- **NovaBlindFold:** masks a relation through fresh random relaxed-instance data. It can protect a subsequent non-hiding proof, but random witness/error vectors lose small-scalar commitment savings. [HyperNova](https://eprint.iacr.org/2023/573)
- **A masked small verifier circuit:** reduces the amount of full-width random data, but introduces another relation/proof whose bytes, recursive checking and claims must be counted. Vega documents this approach explicitly. [Vega’s privacy construction](https://microsoft.github.io/vega-prover/mc/zero-knowledge.html)

At the proposed sizes:

| Item | Concrete accounting |
|---|---:|
| Ten committed sumcheck groups | 320 B |
| Eight committed groups | 256 B |
| Reserved 1,024 field entries | 32 KiB private storage |
| Schnorr-final IPA, \(N=2^{16}\) | \(64\log_2N+96=1,120\) B |
| Schnorr-final IPA, \(N=2^{13}\) | 928 B |
| Send primary: 8→32 rows and k16→k14 IPA | \(+768-128=+640\) B |

These IPA sizes exclude the separately carried generator claim.

The measured Pixel k16 hiding IPA is approximately **1.55 s**, and the k16 restructuring opening approximately **1.50 s**. A full-width k16 MSM is about **140 ms**. Avoiding one masking MSM therefore suggests a saving around 0.14 s at that size, not elimination of the IPA’s entire cost. The proposed replacement remains unmeasured. [Pixel results:29](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md), [opening/restructuring measurements:48](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md)

For comparison, at the measured approximately 2.1 μs per full-width committed scalar, committing one random \(2^{19}\)-entry witness and one \(2^{20}\)-entry error vector alone would cost about **3.3 s**. This is an illustrative full-vector masking charge, not a measured NovaBlindFold implementation.

The supplied Send estimate totals **4,544 B**, leaving **3,914 B** within the 8,458-byte proof allowance. That is encouraging room for a sound construction, but remains a serialization hypothesis until the commitment-binding mechanism, all evaluations, dimensions and inherited claims are fixed.

The +640 B reshaping calculation applies to the eight-row Send primary. Preserving REPORT4’s doubled Receive witness capacity would require 64 rows at length 16,384, giving a different byte increment. [REPORT4 shapes:87](REPORT4.md)

**5. Concrete security accounting**

“255-bit fields” does not mean 255-bit cryptographic security—or even an exact 128-bit overall guarantee.

| Component | Defensible assessment |
|---|---|
| Pasta discrete logarithms | Group orders are just above \(2^{254}\). Generic square-root work is around \(2^{127}\); accounting for the curves’ automorphisms gives an estimated Pollard-rho cost around \(2^{126}\) group operations |
| Pedersen/Hyrax binding and IPA | Ultimately limited by those discrete-log assumptions, plus the exact extraction/Fiat–Shamir reduction |
| Poseidon RP57 | The current width-3/rate-2 sponge has one field element of capacity; its generic ideal-permutation security scale is roughly 127 bits. Security of the concrete permutation and its recursive use remains an assumption |
| Degree-two, four-variable sumcheck group | At most \(8/|F|\), conditional on a previously fixed polynomial and independent full-field challenges |
| Alpha-power batching of \(m\) fixed errors | At most \((m-1)/|F|\); independent random coefficients instead give the usual \(1/|F|\) linear-combination bound |
| Two-gate masking below | Statistical distance below \(1/|F|\) for the isolated triple, plus degeneracy events |
| Existing truncated fold challenge | Approximately 128 bits of challenge entropy, not full-field entropy |
| Entire proposed recursive system | **No established numerical bound yet** |

The Pasta order and curve definitions are publicly reproducible; Halo discusses the automorphism-adjusted discrete-log estimate. [Pasta parameters](https://github.com/zcash/pasta), [Halo, §5.3](https://www.doc.ic.ac.uk/~ids/dotdot/crypto_papers_etc_worth_reading/coin_mixing/Halo_iacr_2019_1021.pdf)

The repository specifies Poseidon \(t=3\), rate 2, \(x^5\), 8 full and 57 partial rounds. The corresponding generic sponge bound depends on capacity and query count; it is not a proof that concrete Poseidon is a random oracle. [PIPA specification:382](../../../specs/plonk_ipa_v1.md), [Poseidon paper](https://www.usenix.org/system/files/sec21-grassi.pdf)

For the Send shapes, the degree-two round-check contribution across outer/inner sumchecks is approximately

\[
\frac{2(20+19+16+16)}{|F|}
=\frac{142}{|F|}
\approx 2^{-246.85}.
\]

This is **only that algebraic contribution**. Initial zero testing, equality-factor exceptions, matrix/Hyrax reductions, commitment batching, IPA extraction and Fiat–Shamir losses remain separate.

Challenge distributions must be measured by their maximum conditional atom probability \(\lambda\). A degree-\(D\) test contributes \(D\lambda\). Current cross-field conversion has small bias, but subsequent nonzero/square/fourth-power maps have larger fibers; the specification explicitly warns against treating their outputs as uniform. [Challenge laws:87](../../../specs/kagemusha_recursion_soundness_v1.md)

A concrete implementation warning: `iroha_fold` currently obtains claim-fold \(\gamma\) by taking the **low 128 bits** of a sponge output. It is not an endoscalar encoding. [transcript.rs:93](../../../crates/iroha_fold/src/transcript.rs), [claims.rs:275](../../../crates/iroha_fold/src/claims.rs)

Its update includes

\[
G'=(1-\gamma)G+\gamma G_{\rm new}.
\]

Thus \(\gamma=0\) drops the new contribution and \(\gamma=1\) drops the old one. These are algebraic bad-challenge events to account for, not automatically implementation bugs. A degree-two estimate is around \(2/2^{128}\), before attempts and composition losses.

An injective 128-bit endoscalar map can accelerate multiplication, but still has at most \(2^{128}\) outputs. It needs its own image, multiplicity and extraction analysis. Merely representing its result in a 255-bit field does not enlarge its challenge space.

A completed theorem needs a bound of the form

\[
\operatorname{Adv}_{\rm forge}
\le
\operatorname{Adv}_{\rm DL,P}
+\operatorname{Adv}_{\rm DL,V}
+\operatorname{Adv}_{\rm hash}
+\epsilon_{\rm FS/extraction}
+\sum_{\text{all relevant tests}}D_i\lambda_i,
\]

with finite operation/query budgets and explicit reduction losses. This is the required accounting structure, **not a proved bound for the spike**.

The expected computational bottleneck is roughly the Pasta security level; truncated challenges may dominate some finite-budget algebraic bounds. Presently, the missing composition/extraction argument prevents an honest overall “proven X-bit security” label.

**6. Specific review of (a)–(d)**

**(a) Four-variable committed grids: conditionally valid.**

A polynomial of individual degree at most two in four variables is determined by its \(3^4=81\) grid values. Its total degree is at most eight, giving the requested \(kd/|F|=8/|F|\) bound.

The ordinary outer Spartan polynomial includes an equality factor and has individual degree three. The proposed grid works only if the known equality factor is correctly separated while preserving weighted Boolean sums and terminal checks. Otherwise it requires \(4^4=256\) values and a \(12/|F|\) round bound. REPORT4 proposes the factoring, but its multivariable realization needs verification. [REPORT4:65](REPORT4.md)

All four challenges must follow the commitment. Sampling one scalar and reusing it in four coordinates is a different test. Dividing by an equality-factor evaluation also requires a defined zero case; multiplying through avoids undefined arithmetic but still needs soundness accounting.

Ten full groups need 810 coordinates, so 1,024 is enough for those grids. However, the tail must be unavailable to witness rows and constrained accordingly. Reserving it leaves:

- \(8(65,536-1,024)=516,096\) witness positions.
- \(32(16,384-1,024)=491,520\) witness positions.

The compiler must demonstrate sufficient genuine padding or a revised layout.

**(b) One combined IPA: critical open obligation; a natural interpretation is unsound.**

Once fixed errors \(e_j\) exist, alpha batching is straightforward:

\[
\sum_j\alpha^j e_j=0.
\]

But this does not establish that the errors refer to polynomials fixed at their required earlier transcript prefixes.

Suppose the implementation proves only an opening of

\[
C=C_{\rm rows}+\sum_i D_i
\]

over the intended concatenated vector. Allocating disjoint generator slots in the honest prover does not constrain a malicious commitment point to use those slots.

A two-round counterexample is:

1. Send \(D_1=\rho H\), then obtain \(r_1\).
2. Choose \(q_1\) **after seeing \(r_1\)** so its Boolean sum equals a false initial claim but \(q_1(r_1)\) equals the true next-round sum.
3. Choose the genuine conditioned next polynomial \(q_2\), and send
   \[
   D_2=\operatorname{Com}_1(q_1)+\operatorname{Com}_2(q_2)-D_1.
   \]
4. The aggregate opens correctly to \(q_1\Vert q_2\); the scalar consistency and terminal checks can all hold.

No discrete logarithm is solved. The later commitment absorbs off-slot terms that retroactively substitute the first polynomial. Alpha batching cannot detect the problem because the eventual scalar equations are all true.

This is a counterexample to the **fixed-aggregate interpretation**, not a finding that the unspecified prototype necessarily implements it.

The construction needs an extractable multi-commitment opening argument that establishes each round’s support and timing. Fresh commitment-batching coefficients sampled after all commitments can block this cancellation strategy, but require their own support/extraction theorem and coefficient handling. If one alpha serves both commitment binding and equation batching, prove that exact construction and its degree bound.

**(c) Two multiplication gates: a useful conditional statistical-hiding lemma.**

Let two isolated random gates satisfy

\[
z_1=x_1y_1,\qquad z_2=x_2y_2,
\]

with four independent uniform inputs. If their MLE weights are \(a,b\), their contributions are

\[
X=ax_1+bx_2,\quad Y=ay_1+by_2,\quad
Z=ax_1y_1+bx_2y_2.
\]

For \(a,b,S=a+b\ne0\), conditional on \(X,Y\),

\[
Z=\frac{XY}{S}
+\frac{aS}{b}
\left(x_1-\frac XS\right)
\left(y_1-\frac YS\right).
\]

The two shifted factors are independent uniform field elements. Their product is statistically close to uniform, giving

\[
\Delta\!\left((X,Y,Z),U_{\mathbb F^3}\right)
=\frac{|\mathbb F|-1}{|\mathbb F|^2}
<\frac1{|\mathbb F|}.
\]

Thus two gates can statistically hide the three evaluations. **This is not perfect hiding, and not yet a simulator for the complete proof.**

Required conditions include isolated gates, no additional exposed evaluation of their inputs, correct matrix embedding, fresh uniform masks, and treatment of zero weights. Adjacent gate indices can make \(a+b\) a common MLE factor and simplify the exceptional-event analysis.

A clear fourth witness evaluation, reused masks, or correlations with inherited claims can invalidate the isolated-triple argument. Ideal uniform masks and CSPRNG-generated masks also require separate statements.

**(d) Blinded IPA with Schnorr final: established ancestor, new integration proof required.**

After folding, write the required relation as

\[
P'=a'K+\rho'H,\qquad K=G'+b'U.
\]

Sample fresh \(s,t\), send \(A=sK+tH\), derive challenge \(e\), and return

\[
z_1=s+ea',\qquad z_2=t+e\rho'.
\]

Verify

\[
z_1K+z_2H=A+eP'.
\]

The Schnorr step has the standard honest-verifier simulator and extracts \(a',\rho'\) from two accepting responses with the same commitment and distinct challenges. Fresh L/R blinds hide intermediate terms. Halo provides the relevant ancestor; O(1) Labs describes this particular hiding approach explicitly. [Halo](https://eprint.iacr.org/2019/1021), [official IPA explanation](https://o1-labs.github.io/proof-systems/plonk/inner_product/)

However:

- The whole IPA needs extraction across its folding rounds.
- \(G'\) must be independently derived or decided.
- The complete statement, bases and transcript must bind \(e\).
- Exceptional bases/challenges need explicit semantics.
- Nonces must never repeat across fresh attempts.
- ROM zero knowledge does not establish recursive fixed-Poseidon composition.

Today’s prover instead commits a full random root-at-opening polynomial before its IPA. Replacing that mechanism changes the protocol, even if the final byte size remains equal. [prover.rs:127](../../../crates/iroha_plonk/src/pcs/ipa/prover.rs)

**Inherited-claim rerandomization remains mandatory.**

A reused \((G,u)\) is an ancestry tag. A public equation such as

\[
G_{\rm out}=G_{\rm old}+\alpha G_{\rm own}
\]

reveals \(G_{\rm old}\) whenever the other terms are public. Fresh randomness in the own proof does not repair that disclosure.

Restructuring must keep the old claim and linking equations private while proving their correctness, then expose a fresh structured claim. The privacy theorem must cover both fields jointly, colluding counterparties, Send-after-Receive, checkpoints, failures and exact-byte retries. Today’s specification explicitly notes that “\(G\) is determined by public challenges” does not establish that the public tuple is zero knowledge. [Disclosure boundary:237](../../../specs/kagemusha_recursion_soundness_v1.md)

**7. Tests and release evidence**

Today’s `fold_wrap.rs` is useful as a mutation-test pattern, but all three tests are marked `#[ignore]`. Its four-half-step toy invokes mutations; the eight-half-step call does not. This review does not establish that any of them ran against today’s candidate. [fold_wrap.rs:41](../../../crates/iroha_kagemusha_proof/tests/fold_wrap.rs)

The helper mutates cross-proof digests, challenge limbs, generator points and public rows. Path 2 needs corresponding mutations for its actual interfaces, using genuine complete proofs. [Mutation helper:754](../../../crates/iroha_kagemusha_proof/tests/support/fold_wrap_toy.rs)

Required tests include:

| Area | Inputs or behaviors that must fail |
|---|---|
| Canonical decoding | Truncation, trailing bytes, extra/missing rounds, scalar ≥ modulus, wrong field/curve, malformed points, illegal identity encoding, wrong limb/bit order |
| QR transport | Inconsistent lengths, oversized allocations, decompression limits, mixed streams, conflicting shards, wrong profile/schema/hash |
| Statement binding | Changed operation, asset, amount, fee, credential/key, predecessor, successor, policy, matrix identity or task ordering—even after attacker-controlled hashes/signatures are recomputed |
| Cycle closure | Primary/secondary from different operations; valid endpoints with an unrelated intermediate fold; missing/duplicated predecessor or incoming obligation; unauthorized reset |
| Sumcheck | Wrong Boolean sums, wrong individual degree, equality-factor errors, reused coordinate challenge, false final evaluations |
| Committed rounds | The off-slot cancellation attack above; overlapping tail/witness slots; omitted batch equation; batching challenge sampled too early |
| Hyrax/IPA | Wrong row combination, padding, dimensions, generator prefix, L/R, Schnorr commitment/response or opening value |
| Claims | Each false own/inherited claim independently; solved-\(G\) succinct forgery; source-width relabeling; altered restructuring input, mask, \(D\)-opening or correction |
| Value semantics | Field-wrap balances, integer overflow/underflow, stale roots, duplicate credit, changed Payment digest, cross-asset Load reuse, refund through Archive/replay |
| Recovery | Crashes around every selection/signature/proof/verification/persistence/release boundary; concurrent spends; backup restoration; storage read errors; entropy failure; nonce reuse |

Tests must exercise **native bytes and in-circuit assigned cells separately**, and establish matching acceptance semantics.

Zero/identity tests must be role-specific. Current actual IPA challenges are nonzero after their total map; padding zeros have defined meanings; generator claims may legitimately be identity but still require decision. Current fold alpha/z/zeta can be zero. Blanket rejection would misstate the protocol. [Role and accumulation rules:94](../../../specs/kagemusha_recursion_soundness_v1.md)

Likewise, “invalid inputs must be refused” needs the existing containment distinction: own/predecessor obligations are hard failures; incoming malformed evidence may have a constrained burn/no-op representation inside the total relation, while native admission must refuse it before Advance. A receiver must not choose burn for valid evidence or make an invalid inherited claim disappear through a soft branch. [Branch rules:184](../../../specs/kagemusha_recursion_soundness_v1.md)

Before production, require:

1. **A frozen protocol specification:** exact equations, commitment supports, transcript schedule, challenge distributions, generator derivation, codecs and public leakage.
2. **Independent cryptographic review:** the combined-IPA extraction argument, two-field recursive composition, finite-budget bound and joint ZK simulator.
3. **Compiler and implementation review:** uniform-matrix correctness, source-to-key binding, complete arithmetic, constant-time secret operations and public-only width classification.
4. **Executed adversarial coverage:** genuine Send/Receive/Send-after-Receive chains, bad middle proofs, invalid-input paths, all four native decisions, and restart/fault campaigns.
5. **One identified release candidate:** matching source, parameters, keys, native libraries and SDKs; actual serialized sizes and physical-device payment/recovery qualification.

The repository already requires this separation between component evidence and release qualification. Its completion criteria explicitly retain independent proof/privacy review and physical rollback/release validation. [Production completion criteria:2263](../../../specs/kagemusha_single_design_proposal.md)

**For 3077ab, the next security gate should be the committed-round extraction theorem and its cancellation regression.** Until that is resolved, the smaller proof estimate cannot be treated as the size of a sound payment proof.
