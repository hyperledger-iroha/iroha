**The measured byte gate passes. The recursive sizing gate has not passed.** A concrete two-curve construction is possible, but the supplied artifacts do not establish stable Send/Receive circuits at the proposed sizes.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

The main findings are:

- Hash each proof’s transcript in its **scalar field**, absorbing foreign points through canonical byte limbs. This avoids non-native transcript permutations.
- Bind the two circuits through an existing Hyrax row commitment. This can add **zero wire bytes**, but substantial circuit work.
- The proposed \(2^{16}\)/\(2^{17}\) secondary budgets fail the conservative construction below.
- Wider columns produce byte-compatible candidates, but their projected Pixel times are substantially above the measured short-shape cores.
- Round 4 supplies **proposed witness-coordinate counts, not an application R1CS constraint census**. Consequently, an unconditional four-circuit census and a certified recursive fixed point cannot honestly be extracted from those reports.

I made no changes and ran no new prover benchmarks.

**A concrete two-curve construction**

Use the following division of work. “Input proofs” means the predecessor pair for Send, and the predecessor plus incoming Send pairs for Receive.

| Work | \(P_i\), circuit over Fp | \(S_i\), circuit over Fq |
|---|---|---|
| Operation statement | Transition, authorization, state and incoming-payment bindings | Matching operation/interface identity |
| Input P proofs | Transcript and scalar verifier | Vesta group equations |
| Input S proofs | Pallas group equations | Transcript and scalar verifier |
| Vesta inherited claims | Scalar half of restructuring | Group half |
| Pallas inherited claims | Group half of restructuring | Scalar half |
| Pair binding | Constrain the canonical shared descriptor | Recompute its designated Vesta row commitment |

The shared descriptor must identify ordered tasks, not just an operation label. It contains the exact predecessor/incoming proof bytes and claims, their parameter and shape identities, reconstructed IPA starting points, cross-field MSM coefficients, and the private claim-restructuring interface.

The current prototype does not implement these recursive relations. Its verifier and serializer are at [spartan.rs:546](../src/spartan.rs) and [spartan.rs:37](../src/spartan.rs).

**Transcript placement and field crossings**

Use:

\[
H_P=\operatorname{Poseidon}_{Fp},\qquad
H_S=\operatorname{Poseidon}_{Fq},
\]

with pinned RP57 parameters: width 3, rate 2, eight full rounds, 57 partial rounds, \(x^5\). These parameters already exist over both fields. This replaces the spike’s BLAKE3 transcript; it is a protocol change, not an implementation detail. [Poseidon parameters](../../../crates/iroha_pasta/src/poseidon/mod.rs), [prototype transcript](../src/fs.rs).

For every foreign point:

1. Absorb its exact 32-byte compressed encoding as two unsigned 128-bit limbs.
2. In the group-native circuit, constrain decoding, coordinate bounds, curve membership, sign and the specified identity policy.
3. Bind those limbs to the scalar circuit’s transcript through the shared descriptor.

Thus, hashing a Vesta point in Fp requires native Poseidon plus byte-limb constraints. It does **not** require evaluating the Vesta curve equation non-natively in Fp.

Derive every MSM coefficient in its proper scalar field. Transfer its canonical integer representation, with

\[
0\le a<p\quad\text{or}\quad0\le a<q,
\]

and use those integer bits for multiplication on the other circuit’s native curve. **Do not reduce the coefficient modulo the other circuit’s field.** Challenges remain full-field challenges; neither 128-bit truncation nor an endoscalar map is assumed in the census.

Preserve the prototype’s ordering: rows before \(\tau\); each round commitment before its challenges; terminal evaluations before \(r_A,r_B,r_C\); \(\alpha\) before \(\gamma\); L/R before each IPA challenge; final generator and Schnorr commitment before the final challenge.

One easily missed dependency is the IPA starting point:

\[
P_0=\sum_r\operatorname{eq}(r_{\rm row},r)T_r+
       \sum_j\gamma^{j+1}D_j.
\]

The prototype **hashes \(P_0\)** before deriving \(\zeta\). Therefore the interface must carry its encoding, and the group circuit must check its reconstruction separately from the final IPA equation. Flattening everything into one final equation while leaving the hashed \(P_0\) unconstrained is unsound. [ipa.rs:56](../src/ipa.rs).

**Binding alternatives**

| Alternative | Wire addition | Circuit consequence |
|---|---:|---|
| Public deferred values | Thousands of bytes | Simple public equality, but still requires exact source binding |
| Recompute a designated Hyrax row | **0 B** | Fixed-base MSM over the shared descriptor |
| Multiphase RLC, linked through existing rows | **0 B** | Foreign-field Horner evaluation on one side |
| Multiphase masked RLC with public evaluation | **32 B/pair** | Removes one binding row and its small commitment check; still needs foreign arithmetic |

For **public deferral**, an explicit uncompressed interface adds, per source proof,

\[
32(R+m+2\ell+3)
\]

bytes for the reconstructed starting point and coefficients not already present in the source proof, where \(m=O+I\).

For Send following the measured short-IPA Receive, that is

\[
32[(128+12+26+3)+(16+10+26+3)]
=\boxed{7,168\text{ B}}.
\]

This is already more than Send’s 2,410-byte headroom, before adding private restructuring interfaces or a sound common-source binding. Publishing the entire descriptor would be larger and would expose ancestry. This is the cost of this explicit interface, not an information-theoretic lower bound.

For the **zero-byte construction I would implement first**, reserve one existing P witness row for descriptor \(D\):

\[
T_*=\sum_j D_jG_j+\rho H.
\]

- P constrains the designated row to contain \(D\), followed by authenticated zeros.
- S witnesses the same descriptor and a blind, recomputes this Vesta commitment natively, and checks equality to \(T_*\).
- The native verifier obtains \(T_*\) from the designated position in the actual P proof and supplies it as an S public input.
- No second copy of \(T_*\) is serialized.

Commitment binding connects the two descriptors. The descriptor excludes current proof commitments, so this introduces no current-proof cycle: construct the shared descriptor and its row commitment, then produce the proofs.

It consumes **one whole active P row**, not merely \(N\) cells. Unused cells cannot simultaneously contain unrelated application witnesses.

A useful descriptor count, before current-operation framing and private restructuring, is

\[
N_{\rm source}
=4(R+m+2\ell)+2L+26
\]

128-bit limbs, where \(L\) is the inherited claim’s challenge count. It includes source proof/header/claim bytes and the deferred task interface. At the measured short shapes:

- Send source pair: **676 limbs**.
- Receive source pair: **976 limbs**.
- Receive after Receive: **1,652 limbs**, before private restructuring.

“No bytes” therefore does not mean negligible constraints.

The **multiphase RLC alternative** is also concrete:

1. Commit descriptor rows \(A_P,A_S\) in both proofs.
2. Derive
   \[
   r=H_P(\text{domain, shapes, operation},A_P,A_S).
   \]
3. Both circuits compute \(h=\sum_jr^jD_j\) in Fp; S does this non-natively.
4. Link their evaluations through another existing P row
   \[
   B_P=hG_0+\rho H,
   \]
   recomputed in S.
5. Construct remaining rows, then run the ordinary Spartan transcript.

The challenge must follow **both** descriptor commitments. Conditional on committed unequal descriptors, the equality test has polynomial-identity error at most \((N-1)/p\), before commitment, extraction and Fiat–Shamir losses.

A potentially better trade is to transmit \(h\), adding 32 bytes and saving \(B_P\)’s row and commitment check. To avoid exporting an ancestry fingerprint, commit a shared fresh private mask \(\delta\) before \(r\) and use

\[
h=\sum_jr^jD_j+r^N\delta.
\]

This variant still needs a joint privacy argument; the mask must not be chosen after \(r\).

**Inherited generator claims**

Use a bounded structured-source restructuring first, rather than assuming the own IPA can discharge inheritance for free.

Each curve has:

- Send: two sources—predecessor own and inherited claims.
- Receive: four sources—own and inherited claims from predecessor and incoming Send.

Fix their ordered \((G_j,u_j,\ell_j)\), including generator identities and padding conventions. Bind the sources and fresh masks before deriving \(\mu\). Form

\[
C_*=\sum_j\mu^jG_j,\qquad
s_*=\sum_j\mu^js(u_j).
\]

After fixing \(C_*\), derive an MLE point \(r\), and compute

\[
v=\sum_j\mu^j
  \prod_a(1-r_a+r_au_{j,a}),
\]

with the appropriate zero-padding factors for shorter source vectors.

Privately verify a masked restructuring IPA opening \(C_*\) at \(\operatorname{eq}(r,\cdot)\). Export only its fresh structured claim \((G_{\rm new},u_{\rm new})\). Keep old sources, intermediate commitments and masks private.

This retains the inherited wire allowance

\[
32(\ell+1)\text{ bytes per curve}.
\]

The existing restructuring code demonstrates the required statement/mask commitment and exact blinding correction, but its security argument does not automatically cover this proposed batching variant. [restructure.rs:15](../../../crates/iroha_plonk_recursion/src/restructure.rs).

A joint own/inherited opening remains attractive. However, the own IPA’s vector contains grid-check weights in the reserved tail. Sharing an MLE point is insufficient: the joint relation must account for

\[
\sum_{j\in\mathrm{tail}}
s_j\,[b_j-\operatorname{eq}(r,j)],
\]

and bind the resulting commitment without publicly exposing reversible ancestry information. I would book **no elimination of the restructuring IPA** until that relation is implemented and proved.

The payee must verify both current proofs, their shared operation and derived cross-proof inputs, and decide **four current generator claims**: own and inherited on each curve. Succinct IPA equations alone are not acceptance.

**What the prototype actually makes the next circuit verify**

Let

\[
R=2^r,\quad C=2^\ell,\quad M=2^x,\quad z=r+\ell+1.
\]

The extra bit in \(z\) selects witness versus public inputs. For maximum group size \(k\),

\[
O=\lceil x/k\rceil,\quad I=\lceil z/k\rceil,\quad
T=\sum_{\text{outer and inner blocks}}3^{k_j}.
\]

The exact serialization and usable witness capacity are

\[
B=32(R+O+I+2\ell+7),\qquad
W_{\max}=R(C-T).
\]

These come directly from [r1cs.rs:38](../src/r1cs.rs).

| Measured shape | \(x\) | Rows × columns | Outer / inner | Tail | IPA rounds | Variable-base scalar multiplications¹ |
|---|---:|---:|---:|---:|---:|---:|
| Send P | 20 | 8 × 65,536 | 5 / 5 | 810 | 16 | 52 |
| Send S | 16 | 8 × 8,192 | 4 / 5 | 651 | 13 | 45 |
| Receive P | 21 | 16 × 65,536 | 6 / 6 | 816 | 16 | 64 |
| Receive S | 17 | 16 × 8,192 | 5 / 5 | 660 | 13 | 56 |
| Short Send P | 20 | 64 × 8,192 | 5 / 5 | 810 | 13 | 102 |
| Short Receive P | 21 | 128 × 8,192 | 6 / 6 | 816 | 13 | 170 |

¹ Across the starting-point MSM and final IPA MSM, before inherited-claim work; additionally two fixed-base terms and unit-coefficient point operations. Coefficients are generally **255 bits**, even if the original challenges were shortened.

This explains why increasing rows is expensive recursively. CycleFold’s published 1,000–1,500 multiplication-gate figure concerns **one scalar multiplication**, not this entire verifier. [CycleFold](https://eprint.iacr.org/2023/1192).

**Gadget costs and census formulas**

The following are explicit arithmetization choices, not synthesized measurements.

| Gadget | R1CS derivation |
|---|---|
| Native Poseidon permutation | \(81\) quintics × \(3\) multiplications for \(x^2,x^4,x^5\) = **243** |
| \(b\)-bit range | \(b\) Boolean constraints, plus reconstruction |
| Canonical foreign integer | Existing bits plus integer borrow comparison; budget about **260 additional constraints** per 32-byte scalar/coordinate |
| Affine addition with distinct x coordinates | Slope, slope square, y product, nonzero-denominator inverse = **4** |
| Affine doubling | Above plus \(x^2\) = **5** |
| Full-width variable-base multiplication | Signed radix-4: approximately \(255+128(6+4)+20=\mathbf{1,555}\), plus shared doubling/offset work |
| Fixed-base 128-bit limb | \(128+64(1+4)=\mathbf{448}\) |
| Fixed-base 255-bit scalar | Approximately \(255+128(1+4)=\mathbf{895}\) |

The affine model requires constrained exceptional-case handling. A costed option uses nonidentity bases, signed nonzero digit tables and a private on-curve accumulator offset, checking all denominators and cancelling the offset exactly. It must not silently apply incomplete formulas to arbitrary points.

For a foreign multiply, the repository’s three-87-bit-limb construction proves

\[
ab=c+qm
\]

using bounded quotient and carry witnesses plus a native residue equation. Translating its stated bounds to Boolean-range R1CS gives approximately

\[
256+261+4(105)+9\approx946
\]

constraints before canonical-output comparison and linear bookkeeping. This makes an \(N\)-step foreign Horner check materially more expensive than \(N\) native multiplications. The current seven-row lookup gadget is **not seven R1CS constraints**. [Foreign-field representation and bounds](../../../crates/iroha_plonk_gadgets/src/ff/mod.rs).

For transcripts, choose a definite framed encoding:

- point: `[tag, 2, lo128, hi128]`;
- native scalar: `[tag, 1, value]`;
- challenge request: `[domain, counter, tag]`.

Use the evolving sponge state without redundantly reabsorbing its output. With eight public scalars and a preabsorbed fixed header, this gives

\[
\Pi=2R+4x+2z+2O+2I+6\ell+39.
\]

Additional public inputs must be charged. Retaining the spike’s output-reabsorption convention also increases this count.

For the scalar verifier, literal translation of the current code includes

\[
\underbrace{C-1}_{\text{column eq table}}+
\underbrace{C-T}_{\text{head scaling}}+
\underbrace{C-1}_{\text{IPA weight folding}}
=\boxed{3C-T-2}
\]

multiplications.

The grid weights can share \(t^2\); their cost is

\[
\sum_j\left[k_j+\frac{3^{k_j+1}-9}{2}\right].
\]

Add tail scaling by \(\alpha\) and \(\gamma^{-1}\), row weights, powers, terminal arithmetic and the **actual circuit’s matrix-MLE evaluator**. The spike’s logarithmic evaluator describes its toy wiring, not an arbitrary recursive verifier. [Scalar relation](../src/spartan.rs), [IPA folding](../src/ipa.rs).

An optimized tensor-and-tail evaluation could avoid the literal \(O(C)\) scalar work. That is a separate implementation and validation task.

**A worked four-circuit allocation**

Consider this uniform source geometry:

- P: \(x=22,\ R=64,\ C=32,768,\ k=4\).
- S: \(x=21,\ R=32,\ C=32,768,\ k=4\).

It has 413/343 transcript permutations, costing 100,359/83,349 R1CS constraints. Literal scalar verification is approximately 102,000 constraints per input proof, excluding the real matrix evaluator.

Let \(q=1\) for Send and \(q=2\) for Receive. For an illustrative descriptor envelope \(N=900q+600\) 128-bit limbs:

| Component, thousands of constraints | Primary | Secondary |
|---|---:|---:|
| Input transcript permutations | \(100.4q\) | \(83.4q\) |
| Input scalar verifier | \(102q\) | \(102q\) |
| Native group equations | \(130q\) | \(180q\) |
| Private inherited restructuring | 110 | 110 |
| Descriptor binding/ranges | \(0.128N\) | \(0.448N+0.9\) |
| Example canonicality allowance² | \(78q\) | \(78q\) |
| **Send subtotal** | **712k** | **1,226k** |
| **Receive subtotal** | **1,238k** | **2,073k** |

² This allowance represents 300 additional canonical comparisons per source pair. The actual descriptor schema and range-sharing compiler must determine that count.

These are **worked engineering allocations**, not exact synthesized totals. Add:

\[
A_{\text{operation}}
+U_{\text{real matrix evaluation}}
+E_{\text{actual interface/public-input layout}}.
\]

The secondary has no independent application transition in this design, but its remaining interface and matrix work still belongs in \(U+E\).

The proposed \(2^{16}=65,536\) Send secondary and \(2^{17}=131,072\) Receive secondary therefore do **not** fit this construction. This is a finding about the explicit design and gadgets above, not a universal lower bound on every possible optimized MSM argument.

**Why round 4 cannot fill in \(A_{\text{operation}}\)**

Round 4 explicitly labels its numbers a “falsifiable compiler census.” They count proposed witness values:

| Relation | Proposed fresh coordinates |
|---|---:|
| Send P | 428,000 |
| Send S | 64,000 |
| Receive P | 680,000 |
| Receive S | 96,000 |

It also allocates 131,072/196,608 additional helper scalars without a complete field/layout partition. The table does not separate application work from recursion, identify independent range checks, or specify matrix wiring. Treating it as application constraints and adding another complete recursive verifier would risk both undercounting and double-counting. [REPORT4.md:124](REPORT4.md), [helper allowance](REPORT4.md).

For scale, independently range-checking all its primary bounded values would alone cost:

\[
\begin{aligned}
\text{Send}:&\ 64k+8(16k)+16(96k)+64(32k)
=\boxed{3,776,000},\\
\text{Receive}:&\ 128k+8(24k)+16(160k)+64(48k)
=\boxed{5,952,000}.
\end{aligned}
\]

These are **not lower bounds**: existing bits, derived bounds and digit reuse can reduce them. They show precisely why “428k coordinates” cannot establish “fits \(2^{20}\) R1CS constraints.”

**What degree-\(\le5\) CCS changes**

CCS can reduce several gadget counts:

- Poseidon: 81 degree-5 equations instead of 243 R1CS multiplications.
- Four-bit variable-point selection can reduce the illustrative scalar-multiplication cost to roughly 720 equations.
- A base-4 digit can be constrained by one quartic equation rather than two Boolean equations.

But the current 81-value outer grid cannot then be retained automatically. For a generic degree-5 relation after factoring out equality,

\[
T_{\rm CCS}
=\sum_{\rm outer}6^{k_j}
+\sum_{\rm inner}3^{k_j}.
\]

Only the **outer** grid changes to degree five; the inner matrix-times-assignment sumcheck remains quadratic.

At \(x=z=22,k=4\), this is

\[
T_{\rm CCS}=6,516+414=6,930,
\]

versus 828 for the R1CS construction. At \(k=2\), it becomes 495, with more round commitments.

Additional matrix forms can also require additional terminal evaluations and a new masking argument. Thus one cannot simultaneously claim the cheaper CCS gates, the prototype’s three terminal values, its two masking gates and its unchanged measured bytes without constructing that variant.

**The recursive row-count feedback loop**

A genuine fixed point must satisfy both

\[
M_P\le2^{x_P},\quad M_S\le2^{x_S}
\]

and witness-capacity inequalities after accounting for binding rows, masks and reserved tails.

With the direct binding row,

\[
W_{P,\rm other}\le(R_P-1)(C_P-T_P),\qquad
W_S\le R_S(C_S-T_S).
\]

Those witness counts themselves depend on predecessor row counts through MSMs and interface binding. Constraint counts alone do not close the loop.

There is already a concrete capacity failure in the measured short Send geometry:

- P capacity: \(64(8192-810)=472,448\).
- S capacity: \(8(8192-651)=60,328\).

The secondary cannot even hold round 4’s proposed 64,000 fresh coordinates. Combined capacity is 532,776 versus 623,072 fresh-plus-helper coordinates, unless the helper allocation overlaps or is eliminated.

For the full-width inherited claims and existing 64-byte headers, the exact pair-size formula is

\[
B_{\rm pair}
=32\!\left[
R_P+R_S+O_P+I_P+O_S+I_S
+3(\ell_P+\ell_S)+20
\right].
\]

This preserves the measured 4,544/5,152 and 6,048/8,448 totals; it does not reinterpret them as recursive measurements.

The following are useful **conditional fixed-point candidates**, using identical shapes for Send and Receive:

| Candidate | P shape | S shape | Constraint ceilings P/S | Pair | Complete Payment |
|---|---|---|---|---:|---:|
| Lean wider columns | 64 × 32,768 | 32 × 32,768 | \(2^{22}/2^{21}\) | **7,360 B** | **10,902 B** |
| Larger secondary | 64 × 32,768 | 64 × 32,768 | \(2^{22}/2^{22}\) | **8,384 B** | **11,926 B** |
| More column capacity | 32 × 65,536 | 32 × 65,536 | \(2^{22}/2^{22}\) | **6,528 B** | **10,070 B** |

All use \(k=4\). These are exact formula-derived sizes for the proposed layouts, **not newly measured serializations**.

The larger-secondary candidate has 2,044,160 active positions per field; its P nonbinding-row capacity is 2,012,220. The 65,536-column candidate has 2,070,656 active positions per field, with 2,005,948 P nonbinding positions.

Uniform source geometry removes the operation-order ambiguity:

| Order | Verified source pairs | Required sizing case |
|---|---:|---|
| Send after Send | 1 | Send |
| Send after Receive | 1 | Same Send geometry |
| Receive after Send | 2 | Receive |
| Receive after Receive | 2 | Same Receive geometry |

However, **uniform geometry is not proof of closure**. The lean secondary allocation leaves only about 24,000 constraints before its unknown work. The larger-secondary candidate has more constraint room but only 74 bytes of Send headroom, and its witness census remains unmeasured.

Accordingly, **none of these can presently be called an established stable recursive shape**. What must change first is the secondary budget and the concrete application/interface compiler—not merely the IPA column count.

**Pixel 6 timing**

The measured short-shape toy prover cores are:

- Send: \(1.392+0.457=\mathbf{1.849\,s}\).
- Receive: \(2.243+0.524=\mathbf{2.767\,s}\).

They exclude real witness generation, recursive relation construction, inherited restructuring, preparation and durable completion. [PIXEL.md:17](../PIXEL.md).

For larger shapes, I used the measured phase rates:

\[
T_{\rm commit,P}\approx
(0.63\text{–}0.66)\,\mu s\cdot RC+
(3.1\text{–}3.9)\,ms\cdot R,
\]

outer sumcheck proportional to \(2^x\), inner sumcheck proportional to \(2RC\), and interpolation of the measured 8,192/65,536-column IPA times. Pallas long-IPA parity is an additional assumption.

| Candidate | Projected pair core | Including two separate private restructuring IPAs |
|---|---:|---:|
| Lean wider columns | **7.19–8.19 s** | **8.64–9.70 s** |
| Larger secondary | **9.67–11.13 s** | **11.12–12.64 s** |
| 65,536 columns on both fields | **10.78–12.21 s** | **13.43–14.91 s** |

These are extrapolations from synthetic measured stages, not confidence intervals or wallet latency measurements. They exclude preparation, real witness generation, remaining native decisions and durable completion.

Thus widening the circuits conservatively preserves the byte budget but does **not** preserve the earlier 4–6-second expectation.

**Soundness assumption and refusal tests**

Until an applicable composition theorem is proved, state the assumption explicitly:

> This exact paired Pasta/Poseidon argument, its commitment separation, interface binding and claim restructuring are recursively knowledge-sound for finite feasible PCD histories, including concrete transcript evaluation inside the recursive circuits and an efficient extraction/composition mechanism.

Discrete-log hardness plus ordinary single-proof Fiat–Shamir security does not itself establish this. Receive merges histories, so PCD is the appropriate composition model. Halo Infinite explicitly identifies the concrete, non-black-box Fiat–Shamir step as heuristic; the split-accumulation literature supplies frameworks whose hypotheses must still be checked. [Halo Infinite](https://eprint.iacr.org/2020/1536), [split accumulation and PCD](https://eprint.iacr.org/2020/1618).

“No depth cap” must mean finite feasible histories, with finite operation/query budgets—not a fixed failure bound over infinitely many attempts. The prototype’s \(\gamma\)-separation argument also still requires a written extraction proof. [Current soundness boundary](REPORT-b04a57-soundness.md).

The refusal suite must include:

- P and S from different operations, predecessors or incoming payments.
- Altered deferred coefficients or reconstructed \(P_0\).
- Modulus aliases, inconsistent compressed-point signs and malformed original bytes.
- Transplanted binding rows, unconstrained row padding, or RLC challenges sampled before both commitments.
- Omitted, duplicated or reordered inherited sources; wrong widths or generator identities.
- A fabricated generator claim satisfying the succinct equation, rejected by native decision.
- Wrong restructuring mask corrections and unrelated opening commitments.
- Existing `CancelRound` and `AdaptiveWitness` attacks, tail interference, and altered \(\alpha/\gamma\) ordering.
- All four operation orders and assigned-cell mutations, not merely wire-byte tampering.

**The two cheapest falsifying measurements**

1. **Count-only synthesis plus interface mutation tests.** No expensive IPA proving is necessary. For the larger-secondary candidate, require both circuits to stay below **4,194,304 R1CS constraints**, P nonbinding witnesses below **2,012,220**, and S witnesses below **2,044,160**. Require exact pair sizes ≤8,458/9,600 bytes and Payment ≤12,000 bytes, including every claim and binding input. Run all four orders and the refusal cases above. Any omitted matrix-evaluation obligation, extra unpriced proof or capacity overflow is a failure.

2. **Pixel benchmark at the synthesized shape and actual width census.** For the 64 × 32,768 candidate, test the projection with **pair core ≤11.2 s**, each own IPA **≤0.76 s**, and two private restructuring openings together **≤1.52 s**. Include the actual binding relation and compiled matrix structure. Exceeding these thresholds falsifies the timing model. Passing them still does not establish a 4–6-second wallet; that requires a separate complete-boundary measurement.

The immediate decision is therefore to synthesize the linked verifier and descriptor row before optimizing further. The existing measurements establish compact ZK proof serialization; they do not yet establish a compact, fast recursive payment pair.
