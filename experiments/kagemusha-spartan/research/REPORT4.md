**I would choose a direct Pasta-cycle prover: one primary operation proof and one secondary proof, with rectangular commitments and native decisions of accumulated IPA claims.** It is the strongest candidate for fast QR payments without a trusted setup.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

However, **the evidence does not yet establish a complete ZK construction fitting the 12 KB Payment**. The promising timing is approximately **8-second Send and 10–11-second Receive**, conditional on the byte and circuit gates below—not three seconds. Treating two plain Spartan transcripts as two private proofs would repeat round 2’s mistake.

I read the four reports and relevant implementation paths. I made no repository changes or new performance measurements.

**Count the complete Payment, including its companion.**

I use **12 KB = 12,000 binary bytes**, before QR redundancy and text encoding, without assuming compression or cached credentials.

\[
B_{\rm proofs}
=12,000-1,394-84-16-2,048
=\boxed{8,458\ {\rm bytes}}.
\]

The terms are Payment’s fixed envelope overhead, IPM1 header, companion framing and maximum payer companion. At 12 KiB, the allowance becomes 8,746 bytes. Omitting the companion gives 10,506 bytes, but the current protocol requires it. [Envelope arithmetic](../../../specs/kagemusha_wallet_wire_v1.md), [carrier framing](../../../specs/kagemusha_wallet_wire_v1.md).

These are the proposed admission budgets:

| Message | Proof objects | Proof allowance | Complete binary message |
|---|---|---:|---:|
| Payment | Completed **Send primary + secondary**, including all exposed claims | **8,458 B maximum** | **12,000 B maximum**, with maximum companion |
| Credited::Receive | Completed **Receive primary + secondary** | **9,600 B target** | 679 + 9,600 + 84 = **10,363 B** |
| Credited::Status | Same completed pair, with status opening | **9,600 B target** | 2,188 + 9,600 + 84 = **11,872 B** |
| Request | No lineage proof | — | Measured 1,792-byte envelope + maximum account companion/framing/header = **5,988 B** |

The Receive pair need not fit Payment’s 8,458-byte allowance: Payment carries the **Send** pair. The existing proof-role caps and authenticated proof descriptors nevertheless need replacement.

“QR” here means the existing animated stream. A maximum Payment body needs 47 data shards and 24 parity shards, plus repeated headers. Base45 and redundancy exceed 12,000 displayed bytes; a literal limit on *all displayed characters* would be a different, substantially smaller payload allowance. [QR specification](../../../specs/kagemusha_wallet_wire_v1.md).

**The selected architecture completes both proofs before release.**

Use a uniform, low-degree Spartan/CCS relation over the Pasta cycle:

- **Primary over Fp, committed on Vesta:** transition, authorizations, maps, exact byte bindings, predecessor/incoming primary scalar verification, and native Pallas arithmetic needed for secondary verification.
- **Secondary over Fq, committed on Pallas:** Vesta group equations, secondary scalar-verifier work and the required claim-update equations.
- Both certify the same operation identity and authenticated task list. They verify predecessor/incoming proof bytes; neither hashes the other’s complete current proof, avoiding a circular construction.
- Payment contains the completed Send pair. Receive proves its own completed successor, including incoming-payment verification. Removing the separate transported sigma permits eliminating the duplicate payer-receipt verification, provided the completed Send statement establishes that exact obligation.
- Each field carries one inherited generator claim. Every accepting verifier decides **both own opening claims and both inherited claims**.

The ordering remains:

> preparation proof → serialized Advance/receipt → completed proof pair → full native verification → durable exact output bytes → release.

No local-credit shortcut, background finalization debt or deferred payout proof is assumed.

CycleFold supplies the field/group separation, but its published 1,000–1,500 multiplication gates describe **one scalar multiplication**, not an entire IPA verifier. The secondary must cover every required equation. [CycleFold](https://eprint.iacr.org/2023/1192).

Succinct verification alone remains insufficient: the repository demonstrates how a false folded-generator point can satisfy its succinct equation. Preserve the native decisions, hard predecessor checks, corrected-claim invalid-input handling and burn semantics. [Acceptance boundary](../../../specs/plonk_ipa_v1.md), [masked accumulation contract](../../../specs/kagemusha_recursion_soundness_v1.md).

**Uniform circuits are essential to the byte budget.**

A fixed verification key does not make arbitrary R1CS matrix evaluation succinct inside recursion. There are two legitimate choices:

1. Include SPARK’s matrix-evaluation argument, charging its commitments, sumchecks and openings.
2. Compile structured selectors, wiring and repeated gadgets whose matrix multilinear extensions have small evaluation circuits.

I choose the second. Public preprocessing may generate authenticated generators and structural constants; it cannot substitute a trusted answer for a challenge-dependent matrix evaluation. Arbitrary sparse exceptions must also be evaluated or proved.

Spartan explicitly uses computation commitments/SPARK for generic succinct verification; uniform CCS provides another route. Neither property follows merely from naming the prover “Spartan.” [Spartan](https://eprint.iacr.org/2019/550), [CCS/SuperSpartan](https://eprint.iacr.org/2023/552).

**The plain proof arithmetic nearly fits Send, but does not establish ZK.**

An optimistic R1CS encoding can factor the known equality polynomial out of the outer sumcheck:

\[
g_i(t)=\operatorname{eq}(\tau_i,t)q_i(t),\qquad \deg q_i\le2.
\]

The endpoint relation permits two transmitted field elements per round. This saves bytes; it supplies no privacy.

Using that optimization, 32-byte scalars/points, rectangular commitments, a plain IPA, one own generator point and one inherited tensor claim per field gives:

| Component | Send primary | Send secondary |
|---|---:|---:|
| Header/binding | 64 | 64 |
| Eight row commitments | 256 | 256 |
| Outer sumcheck | 1,280 | 1,024 |
| Inner sumcheck | 1,216 | 1,024 |
| Terminal evaluations | 128 | 128 |
| Plain IPA | 1,056 | 864 |
| Own generator point | 32 | 32 |
| Inherited generator claim | 544 | 448 |
| **Subtotal** | **4,576** | **3,840** |

The assumed shapes are:

- Send primary: \(2^{20}\) constraints, \(2^{19}\) assignment coordinates, \(8\times65,536\) commitments.
- Send secondary: \(2^{16}\) constraints/coordinates, \(8\times8,192\).
- Receive: primary \(2^{20}\) coordinates and secondary \(2^{17}\), doubling each row count.

That produces **8,416 B for Send** and **9,120 B for Receive**, before a matrix argument or complete ZK construction. Send leaves only **42 bytes**.

Even the existing masked IPA format adds a masking point and an additional scalar relative to this plain count: **64 bytes per proof**. Applied directly, Send becomes 8,544 bytes before hiding the sumcheck. [IPA format](../../../crates/iroha_plonk/src/pcs/ipa/prover.rs).

Shared headers and redesigned encodings can recover some bytes. They do not demonstrate that everything fits. Nor does blinding the row commitments hide clear sumcheck messages.

A proper ZK sumcheck or NovaBlindFold-based construction must be selected and serialized. NovaBlindFold is a legitimate ingredient, but masking the whole application introduces full-width random witnesses/error commitments. Masking a smaller verifier circuit is attractive; its additional proof still counts. [HyperNova](https://eprint.iacr.org/2023/573), [Vega’s explicit privacy construction](https://microsoft.github.io/vega-prover/mc/zero-knowledge.html).

**This is the unresolved feasibility gate for the chosen design.**

**Small-scalar commitments help, but much of the saving already exists.**

The measured three-signature Q1 witness is not predominantly bits or 16-bit limbs:

| Nonzero width class | Count |
|---|---:|
| Bits | 21,539 |
| ≤8 bits | 12,092 |
| ≤16 bits | 39,559 |
| ≤32 bits | 50,005 |
| ≤64 bits | 90,264 |
| ≤96 bits | 153,272 |
| ≤128 bits | 17,004 |
| Above 128 bits | 48 |

There are also 730,329 zero positions in that capture. The foreign-field gadget uses 87-bit limbs, wider intermediates and range-check **running sums**. Its 15-bit lookup differences are not its entire committed witness. [Measured census](../../../output/kagemusha-nova-folding-20261106/RESULTS.md), [foreign-field representation](../../../crates/iroha_plonk_gadgets/src/ff/mod.rs).

Measured Q1 advice commitments already cost **5.67 G instructions = 0.366 s**, versus 5.37 G with explicit width bounds. Grouped LogUp commitments add **9.50 G = 0.613 s**. A naïve full-width charge followed by a “255/16 improvement” double-counts savings.

For a new compiler, the useful optimization is committing independent bounded digits and reconstructing linear combinations rather than committing copied wires and running sums. Width classes must come from public circuit structure, not secret-dependent planning.

Here is a **falsifiable compiler census**, not a measured result. “k” means 1,000 values:

| Relation | Bits | Bytes | ≤16-bit | ≤64-bit | Full field |
|---|---:|---:|---:|---:|---:|
| Send primary | 64k | 16k | 96k | 32k | 220k |
| Send secondary | 16k | — | — | 8k | 40k |
| Receive primary | 128k | 24k | 160k | 48k | 320k |
| Receive secondary | 24k | — | — | 8k | 64k |

Charge respectively **1.5k, 3.5k, 5.7k, 12k and 40.2k instructions/value**. The 16-bit and full-width prices are measured; the others are assumptions. This gives:

\[
I_{\rm commit,Send}=11.655G\Rightarrow0.752s,
\quad
I_{\rm commit,Receive}=17.333G\Rightarrow1.118s.
\]

Transcript-dependent lookup/copy helpers and privacy masks need separate accounting. The measured 16-bit secret MSM costs 5.7k instructions, substantially above ideal bit-linear scaling. [Kernel measurements](../../../output/kagemusha-nova-folding-20261106/RESULTS.md).

The secondary census is aggressive: two primary verifiers can expose roughly 96 variable-base multiplication terms before accumulation. Batched MSM arithmetization must justify the count.

**The Pixel budget is dominated by openings and recursive arithmetic, not application hashing.**

At the selected row lengths:

\[
T_{\rm own\,IPAs}
=\frac{25.1G(1+1/8)}{15.5G/s}
=\boxed{1.82s}.
\]

Two inherited-claim restructuring IPAs cost approximately another **1.82 s**. Small input digits do not make these randomly combined opening vectors narrow.

The following is a conditional allocation. **P** means kernel projection; **A** means an explicit engineering assumption. \(Z_S,Z_R\) are additional privacy costs beyond the masked IPA already priced. \(P_S,P_R\) are complete pre-Advance preparation-proof costs.

| Seconds | Cold Send | Prepared Send | Cold Receive | Prepared Receive | Basis |
|---|---:|---:|---:|---:|---|
| Parsing/witness construction | 0.30 | 0.10 | 0.50 | 0.25 | A; Q1 synthesis is 2.74 G ≈0.177 s |
| Fresh bounded commitments | 0.75 | 0.75 | 1.12 | 1.12 | P from proposed census |
| Full-width helper commitments | 0.34 | 0.34 | 0.51 | 0.51 | A: \(2^{17}\)/\(3·2^{16}\) additional scalars at 40.2k |
| Sumchecks, matrix/recursive arithmetic | 1.00 | 1.00 | 1.80 | 1.80 | A: 15.5/27.9 G instruction allocations |
| Own IPAs | 1.82 | 1.82 | 1.82 | 1.82 | P |
| Inherited-claim restructuring | 1.82 | 1.82 | 1.82 | 1.82 | P |
| Native verification | 0.65 | 0.65 | 1.30 | 1.30 | P plus small parsing/check allowance |
| Receipt and durable completion | 0.20 | 0.20 | 0.20 | 0.20 | A |
| Preparation proof | \(P_S\) | already completed | \(P_R\) | already completed | Unmeasured |
| Remaining privacy work | \(Z_S\) | \(Z_S\) | \(Z_R\) | \(Z_R\) | Unmeasured |
| **Total** | **6.88 + \(P_S+Z_S\)** | **6.68 + \(Z_S\)** | **9.07 + \(P_R+Z_R\)** | **8.82 + \(Z_R\)** | Conditional |

Native verification uses the measured **4.15 G k16 generator decision**. Four decisions—own/inherited on each curve—project to approximately 0.60 s at k16/k13. Receive checks incoming and outgoing pairs. [Decision measurement](../../../output/kagemusha-nova-folding-20261106/RESULTS.md).

For an explicit acceptance target, allocate \(P_S≤0.4\), \(P_R≤0.6\), \(Z_S≤0.8\), \(Z_R≤1.0\) seconds. That yields:

- **Cold Send ≤8.1 s; prepared Send ≤7.5 s.**
- **Cold Receive ≤10.7 s; prepared Receive ≤9.8 s.**

These allocations are not measured predictions. Prepared Receive means preparation against the current head/Request before the incoming Payment is known; it still pays incoming-dependent work. Cold means no operation-specific preparation, with parameters and the previously verified head available. Process launch additionally pays loading and any required head re-verification.

At 166k instructions/permutation, 1,000 native Pasta Poseidon permutations cost only about **0.011 s** in witness generation. Proving their intermediate values remains charged above. This differs fundamentally from millions of permutations in a Pasta-hashed terminal proof.

A proved structured claim accumulator might save the 1.82-second restructuring line. I would not book that saving until its additional relation and export costs are measured.

**Peak memory can plausibly fit; it requires a different allocation strategy.**

A sequential implementation should target **≤896 MiB whole-process peak**, leaving margin below 1 GiB:

| Allocation | Budget |
|---|---:|
| Eight \(2^{20}\)-element field buffers | 256 MiB |
| Additional witness/helper/privacy storage | 192 MiB |
| Uniform wiring and bounded sparse exceptions | 128 MiB |
| Generator/precomputation tables | 128 MiB |
| Shared MSM scratch | 64 MiB |
| Wallet/runtime/allocator allowance | 128 MiB |
| **Total** | **896 MiB** |

Reuse buffers between fields and stream rotations. Generic materialized SPARK matrices or several concurrent provers could invalidate this budget. The existing MSM owner already supports bounded shared scratch. [MSM implementation](../../../crates/iroha_pasta/src/msm/mod.rs).

**Reworking ProtoGalaxy is useful, but does not remove its terminal bottleneck.**

The current Receive has eight half-steps, with two more when rebasing a segment. Fuse business stages, reduce relation kinds and avoid repeated frame processing. Those are worthwhile changes. [Schedule](../../../crates/iroha_kagemusha_proof/src/fold_lineage/mod.rs).

They do not make the existing deciders compact:

- The measured simple Q/A pair is approximately **27 KB + 18 KB**, explicitly non-ZK.
- Q’s degree-12, 16-round zero-check alone is **6,144 B**; A’s degree-9 zero-check is **4,608 B**. Shift arguments, evaluations and openings are additional.
- That simple pair’s proving/opening work is **157 G ≈10.1 s**. It is smaller than the complete operation behind the supplied **61-second decider** measurement.
- The supplied **168-second four-wrap** cost disappears only after replacing their actual field/group bindings, not by deleting the wraps.

[Decider contract](../../../crates/iroha_fold/src/decider_v2/mod.rs), [measurements](../../../output/kagemusha-nova-folding-20261106/RESULTS.md), [wrap obligations](../../../crates/iroha_kagemusha_proof/src/fold_wrap/mod.rs).

Fresh advice benefits from small scalars; folded values and reciprocal helpers are generally full width. ProtoGalaxy updates folded commitments homomorphically, so it does not recommit the whole random accumulator each step—but its terminal prover still processes it.

A sufficiently redesigned compact ProtoGalaxy decider converges on the selected low-degree sumcheck architecture. I would reuse its infrastructure rather than preserve its stage geometry.

**Hash-only does not presently supply an admissible ≤10 KB option.**

The measured non-ZK recursive layer is **168,751 B and 24.006 G instructions ≈1.55 s**. Its constant base proof is already 56,892 B. Repeating that layer has not demonstrated convergence to a tiny terminal trace. [Measured log](../../../output/kagemusha-100x-20261108/spike/runs/agg-nozk-b3p20.log).

WHIR’s published 63 KiB result is at **100-bit security**; STIR reports 114 KiB for its cited 128-bit example. Neither establishes a complete private recursive payment proof below 10 KB. [WHIR](https://eprint.iacr.org/2024/1586), [STIR](https://eprint.iacr.org/2024/390).

Higher **code rate** increases the query burden. Higher **blowup**, or inverse rate, reduces queries at increased encoding/memory cost. The pinned WHIR formulas give this optimistic screening calculation at 128 bits and 20 grinding bits:

| Blowup | Johnson-bound queries | Conjectured-capacity queries |
|---|---:|---:|
| 8 | 76 | 37 |
| 64 | 37 | 19 |
| 256 | 28 | 14 |

[Parameter formulas](../../../output/kagemusha-100x-20261108/spike/Plonky3/security/src/assumption.rs).

At blowup 256, the measured recursion’s Poseidon table alone needs:

\[
8192·165·4·256=1.384\ {\rm GB},
\]

before other tables or trees. Twenty-eight openings contain **18,480 bytes of raw table values**, before authentication paths. A narrow sumcheck-WHIR construction could change this geometry; it needs a new measurement.

Grinding also gives poor leverage. Raising 20→30 bits reduces the residual query-security numerator only 108→98 while multiplying expected attempts by 1,024. Under an explicitly assumed 1,660 instructions/attempt, mean grinding time per site is approximately **0.11 s at 20 bits, 1.8 s at 24, 29 s at 28 and 115 s at 30**.

Neither extra queries nor grinding repairs insufficient digest collision strength or earlier algebraic batching errors. Full composed security and ZK must be priced. The supplied ZK Pixel run already takes **12.1 s and 2.32 GB** before a qualifying small final proof.

Thus there is **no defensible ≤10 KB Pixel timing for the hash-only route today**. This rejects the measured candidates, not every possible hash-based construction.

**Reuse the arithmetic and soundness machinery, replace the proof schedule.**

- **`iroha_pasta`:** field/group arithmetic, bounded MSM, generator derivation and Poseidon.
- **`iroha_plonk`:** IPA proving/native decision, MLE machinery, parameter ownership and gadgets as correctness references.
- **`iroha_fold`:** relation compiler concepts, claim equations, transcript binding and adversarial fixtures.
- **KAGEMUSHA:** state semantics, policy checks, hardware receipts, durable recovery, exact-byte retention and invalid-input outcomes.

The selected route uses no trusted setup and retains Pasta discrete-log assumptions. A pairing/KZG wrapper would be a separate owner decision and still require a measured finalizer.

**The two cheapest falsifying measurements should precede wallet integration.**

1. **Complete ZK serialization and recursive synthesis.** Build the proposed Send/Receive relations with actual privacy, uniform matrix evaluation, both claim accumulators and canonical parsing. Exercise ordinary Receive and its invalid-input path.

   **Pass:** Send proof pair **≤8,458 B**, Receive pair **≤9,600 B**, complete Payment **≤12,000 B**, and the proposed witness/secondary-work bounds survive synthesis. No uncounted auxiliary proof, credential omission or non-ZK transcript qualifies. Test a Send following a Receive so the larger predecessor is included.

   **Fail immediately** if the ZK transcript or matrix mechanism exceeds those bounds. This can reject the design without a phone port.

2. **Pixel commitment/opening harness using that actual witness census.** Measure both curves’ bounded commitments, own IPAs, restructuring, native decisions and memory at thermal steady state.

   **Pass thresholds:** fresh commitments **≤0.9 s Send / ≤1.3 s Receive**; own-plus-restructuring IPAs **≤4.0 s**; native pair verification **≤0.7 s**; whole-process peak **≤896 MiB**. Then require complete cold operations, including preparation/privacy/durability, to meet **8.1 s Send / 10.7 s Receive**.

The direct Pasta pair is my choice for round 4. Its byte gate remains unresolved; claiming either a compliant 12 KB construction or a three-second prover before those measurements would exceed the evidence.
