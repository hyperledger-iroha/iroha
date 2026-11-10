**I would replace per-operation proof recursion with local accumulation, and move ZK to export. But I withdraw the round-2 three-second cold-Send claim.** The revised design makes a three-second **local Receive** plausible; a three-second **Send including finalization** still needs a new, measured terminal prover.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

The most promising revision also removes round 2’s separate hash-recursion bridge:

```text
Private operation relations + accumulation-update checks
                         ↓
             Private, non-ZK accumulator
                         ↓  on export
       ZK terminal decider with Pasta-native PCS hashes
                         ↓
              Non-ZK compact argument
                         ↓
        Omega + complete native verification
                         ↓
              Durable output / payout
```

This is a proposed construction, not an implemented performance result. I read both reports, the folding implementation and measurement logs; I made no changes.

**The measurements invalidate the previous allocation, without establishing a lower bound for every possible decider.**

Using your calibration:

- Non-ZK aggregation: \(24.0/15.5 \approx 1.55\) seconds.
- ZK aggregation at blowup 3: \(66.8/15.5 \approx 4.31\) seconds.
- ZK aggregation at blowup 4: \(137/15.5 \approx 8.84\) seconds.

These are **whole-run proxies**, not isolated recursive-verifier costs. The non-ZK example aggregates two approximately 57 KB proofs and includes construction, preprocessing and verification. Nevertheless, it decisively undermines my allocation of roughly 0.6 seconds to four embedded verifications. [Non-ZK log](../../../output/kagemusha-100x-20261108/spike/runs/agg-nozk-b3p20.log), [ZK instruction count](../../../output/kagemusha-100x-20261108/spike/runs/agg-b3p20.log).

The 66-bit WHIR configuration is not an admissible shortcut. Neither are 124-bit defaults automatically evidence of the required composed security level.

**The accumulator must carry validity obligations, not merely a history digest.**

For each operation, I would accumulate:

1. The actual transition relation: state continuity, conservation, maps, policy and authorizations.
2. The accumulation-update verifier, binding the predecessor accumulator and authenticated starting state.
3. Any incoming compact-proof verification obligations.
4. The generator claims deferred by that compact verifier.

At export, the terminal proof must close every active relation and obligation. This resembles the ownership structure already present in [`ivc.rs`](../../../crates/iroha_fold/src/ivc.rs), where distinct relation slots have distinct eventual decisions. The current [decider contract](../../../crates/iroha_fold/src/decider_v2/mod.rs) illustrates what “decide” actually entails: commitment openings, copies, lookups and the accumulated constraint identity.

Replacing Pasta commitments is substantial protocol work. ProtoGalaxy uses their linear homomorphism; Merkle roots cannot replace them in the existing folding equation. WARP/Arc or SuperNeo must replace that argument, including its update verifier.

| Candidate | What it offers | Remaining obstacle |
|---|---|---|
| **WARP** | Hash-based accumulation, logarithmic update verification and unbounded depth. Best initial candidate for retaining hash-based assumptions. | Encoding, Merkle work and the embedded update verifier remain. Its linear-prover result does not make arbitrary RS encoding linear. Terminal decision still needs compression. [Paper](https://eprint.iacr.org/2025/753), [authors’ construction](https://swisscryptoday.github.io/assets/2025/epfl/08%20-%20Fenzi%20Giacomo%20-%20Warp.pdf). |
| **Arc** | Direct accumulation of polynomial relations and RS-proximity claims. | Hash-verifier circuits can remain substantial. SuperNeo’s comparison estimates roughly **1.6 million R1CS constraints** for an illustrative 128-bit Arc configuration—an analytical comparison, not a phone benchmark. [Arc](https://eprint.iacr.org/2024/1731), [comparison](https://www.microsoft.com/en-us/research/wp-content/uploads/2026/10/Neo-and-SuperNeo.pdf). |
| **SuperNeo** | General constraint systems, small-field extension sumcheck and commitments whose cost benefits from narrow witnesses. More suitable than Neo’s SIMD restriction. | New Module-SIS assumptions and concrete norm/challenge parameters. Published examples use 61/64-bit fields; this is not a demonstrated KoalaBear replacement. [Paper](https://eprint.iacr.org/2026/242). |
| **LatticeFold+** | Improved range checking and commitments relative to LatticeFold. | The reported 5–10× comparison is against LatticeFold, not this Pasta implementation. It supplies no Pixel finalization measurement. [Paper](https://eprint.iacr.org/2025/247). |

I would compare WARP and SuperNeo on the **complete augmented step**, not select between them from commitment throughput alone. SuperNeo’s illustrative lattice-security estimate also cannot substitute for the full composed error budget.

**There is a credible saving from removing MSMs, but no justified “255-bit to 31-bit” speedup multiplier.**

For the supplied 33-column half-step:

\[
33\cdot 2^{16}=2{,}162{,}688\text{ scalar slots}.
\]

Using the measured approximately 40,200 instructions per full-width committed scalar:

\[
I_{\rm commitments}\approx86.9\text{ G},
\qquad T_{\rm commitments}\approx5.61\text{ s}.
\]

That explains much of the seven-second half-step. However, the measured genuine Q1 fold spends **15.91 G instructions outside commitments**, approximately 1.03 seconds under your calibration. Constraint evaluation, lookups and witness construction survive a commitment replacement. [MSM measurements](../../../output/kagemusha-nova-folding-20261106/RESULTS.md), [Q1 breakdown](../../../output/kagemusha-nova-folding-20261106/RESULTS.md).

A useful sensitivity model is:

\[
T_{\rm step}
=
\frac{N_{\rm effective}c_{\rm processing}+I_{\rm update/bridges}}
     {15.5\cdot10^9}.
\]

Here, effective cells include helpers and foreign-arithmetic expansion; processing includes commitment encoding/hashing or Ajtai work, constraint passes and lookups. Update/bridge work must be counted either there or separately, never omitted.

For an **assumed**, unmeasured 700–1,500 instructions per effective cell:

| Effective cells | Processing projection, before separately counted bridges |
|---:|---:|
| 8 million | 0.36–0.77 s |
| 16 million | 0.72–1.55 s |
| 32 million | 1.45–3.10 s |

Thus a **roughly 0.5–1.9-second complete accumulation step is a plausible engineering hypothesis only if the actual census and kernels support it**. It is not a measurement-backed prediction.

There is an immediate warning against the smaller census: round 2 estimated **13–20 million limb products just for incoming compact-proof curve arithmetic**, before carries and range checks. A unified small-field Receive cannot simply inherit an eight-million-cell budget. [Previous arithmetic estimate](REPORT2.md).

**The coprocessors can be accumulated, but their mathematical relations must remain proved.**

There are two legitimate approaches:

| Approach | Per-operation work | Export consequence |
|---|---|---|
| **One common small-field relation** | Constrain P-256, Fp and Fq using canonical limbs, carries, reductions and range checks. Accumulate everything together. | One relation family, but potentially substantial witness expansion. |
| **Separate native-field accumulators** | Accumulate signature, scalar and curve relations in their respective fields. Prove exact links to the control transition. | Joint terminal decision must close every field’s accumulator and all links. Cross-field work remains. |

For the second approach, every instance must bind its authorized relation, operation ordinal, exact input bytes, outputs and transcript challenges. Matching unconstrained digests or accumulating `native_verify == true` is insufficient.

This removes the **three complete coprocessor proof verifiers** from each operation. It does not permit algebraically folding Pasta or P-256 elements into KoalaBear as if they belonged to the same field. My preference is to prototype native-field accumulators alongside the common-field version, because the existing compact verifier makes limb expansion a serious risk.

Incoming Omega still requires complete native verification before local acceptance. Its succinct checks must also enter the accumulated relation, since a malicious wallet can skip native checks. The repository explicitly demonstrates a false generator claim that passes succinct verification but fails decision. [Acceptance boundary](../../../specs/plonk_ipa_v1.md).

**Generator claims provide another saving: defer restructuring, not verification.**

Receive can retain a relaxed local generator-claim accumulator. It need not construct a fresh structured claim through a length-8,192 IPA on every operation.

The current implementation already distinguishes cheap claim folding from expensive restructuring. [Claim accumulation](../../../crates/iroha_fold/src/claims.rs).

Scaling the existing k16 measurements to a **new k13 implementation** gives:

- Three native claim folds: approximately **0.006 seconds**.
- One restructuring IPA at export: approximately **0.203 seconds**.

These are linear extrapolations; the current implementation fixes k16, and the proved update/binding work is additional. [Measurements](../../../output/kagemusha-nova-folding-20261106/RESULTS.md).

Consequently, Receive saves approximately the restructuring IPA, but **Send inherits that cost** whenever pending claims need restructuring. It is separate from the compact finalizer’s own IPA.

**Privacy should live in the terminal decider.**

The local accumulator, witnesses and folding transcripts can be non-ZK under the existing private-wallet custody boundary. That is already the explicit contract of [`iroha_fold`](../../../crates/iroha_fold/src/lib.rs).

At export:

- The **terminal ZK proof** hides the accumulated witnesses, private roots and auxiliary transcripts.
- The compact proof may remain non-ZK if its private inputs contain only that simulatable ZK proof, authorized public inputs and verifier scratch.
- Exposed inherited generator claims still need their masking/privacy argument.

“Local-only” does not make an exported accumulator root private. ZK hides witnesses, not public inputs; receipts, recovery exports and diagnostics must respect that distinction. Repeated exports also need joint privacy, rather than separate single-proof arguments.

Putting ZK in the compact wrapper instead is possible in principle, but **the existing 4,992-byte layout does not establish that variant**. Its clear sumcheck messages cannot be made ZK merely by blinding row commitments. The 128 spare bytes are not evidence that the necessary changes fit.

The measured roughly threefold HidingFRI overhead is configuration-specific. It justifies avoiding repeated hiding proofs; it is not a universal multiplier to divide out.

**Send needs a different terminal construction, not merely postponed round-2 recursion.**

I would try a **single ZK proof of the terminal decision, using Pasta-native PCS hashes from the outset**, followed directly by the compact argument. This removes the separate proof whose job was to convert a KoalaBear-hashed proof into a Pasta-hashed one.

It preserves the round-2 privacy rationale while eliminating one recursive layer. However, it introduces two explicit gates:

1. The complete terminal prover must be fast, including Pasta hashing, hiding, all accumulator decisions and field links.
2. Its exact verifier must fit the compact circuit: **≤\(2^{17}\) private coordinates and ≤\(2^{18}\) constraints**.

Only under those bounds does the proposed 4,992-byte arithmetic remain applicable. [Layout and limits](REPORT2.md).

The calibrated compact subtotal is:

\[
T_{\rm row\ commitments}\approx0.340\text{ s},
\qquad
T_{\rm IPA}\approx0.203\text{ s}.
\]

**Their sum, 0.543 seconds, excludes witness construction, sumchecks, transcripts and other compression work.** The new export additionally owes approximately 0.203 seconds for pending-claim restructuring.

Retaining an extra non-ZK recursion layer would add a **1.55-second benchmark proxy**. Together those three components consume approximately 2.29 seconds before the terminal ZK decider, operation update, preparation and durability. That version has no credible established three-second budget.

Nor can we reuse the current Pasta decider unchanged: its measured Q+A proving and opening work totals approximately 157 G instructions, or **10.1 seconds** under this calibration, before wrappers. [Current decider measurements](../../../output/kagemusha-nova-folding-20261106/RESULTS.md).

**Receive can omit finalization only when it exports no new proof.**

A receiver may fully verify the incoming completed Send Omega, durably accept the credit and accumulate its own successor obligations. Any subsequent Send or Unload closes those obligations before releasing value.

But round 2 also promised a completed **Receive Omega in Credited**. Producing that evidence is an export and must pay finalization. [Previous message contract](REPORT2.md).

I would explicitly change this:

- Fast Receive produces durable local credit.
- An immediate acknowledgement does not substitute for proof-bearing Credited evidence.
- Proof-bearing Credited/CreditStatus triggers finalization; until then, the sender retains the required outgoing evidence.
- Immediate onward spending pays the Send export cost.

The current specification already makes Credited optional and distinguishes acknowledgement from evidence. That supports this separation, but does not make the protocol change disappear. [Delivery evidence semantics](../../../specs/kagemusha_single_design_proposal.md).

The pre-Advance preparation proof, serialized receipt, exact-byte recovery and full verification before monetary release remain required.

**The corrected budget must expose the two unknown export costs.**

Below is a concrete **acceptance budget**, not another claimed forecast.

- **A:** explicit assumption to test.
- **P:** projection from existing measurements.
- \(D\): complete proposed terminal ZK prover.
- \(U\): compact proving work beyond its commitment/IPA subtotal.

“Cold” means no operation-specific preparation, with installed parameters available. Application startup and transport are additional. Prepared cases retain fresh receipt-dependent work.

| Critical-path work, seconds | Cold Send | Cold local Receive | Prepared Send | Prepared local Receive | Basis |
|---|---:|---:|---:|---:|---|
| Witness preparation and pre-Advance proof | 0.20 | 0.30 | 0.05 | 0.10 | A |
| Complete application/accumulation update, including field links | 0.65 | 1.40 | 0.55 | 1.20 | A: respectively about 10.1/21.7 G instruction budgets for cold cases |
| Native local claim folding | — | 0.01 | — | 0.01 | P, rounded; proved binding charged above |
| Native incoming or outgoing Omega verification | 0.15 | 0.15 | 0.15 | 0.15 | A, partially anchored below |
| Hardware receipt and durable completion | 0.20 | 0.20 | 0.20 | 0.20 | A |
| Pending-claim restructuring IPA | 0.20 | — | 0.20 | — | P |
| Compact witness commitments and own IPA | 0.54 | — | 0.54 | — | P |
| Terminal ZK decision | \(D\) | — | \(D\) | — | Unmeasured |
| Remaining compact proving work | \(U\) | — | \(U\) | — | Unmeasured |
| **Total** | **\(1.94+D+U\)** | **2.06** | **\(1.69+D+U\)** | **1.66** | Conditional |

The native-verification allowance has some grounding: scaling the measured 4.15 G k16 generator decision to k13 gives about **0.067 seconds for two decisions**. Parsing, matrix evaluation, sumchecks and other group equations must fit the remaining allowance. [Decision measurement](../../../output/kagemusha-nova-folding-20261106/RESULTS.md).

This table makes the conclusions testable:

- **Local Receive:** three seconds is plausible if the complete accumulated relation meets the 1.40-second allocation. Full limb expansion may invalidate it.
- **Cold Send:** requires **\(D+U\le1.06\) seconds**, approximately 16.4 G instruction equivalents. Nothing measured establishes this.
- **Prepared Send:** requires **\(D+U\le1.31\) seconds**. Preparation cannot pre-finalize an unknown incoming credit or a fresh receipt.
- **Receive exporting Omega immediately:** approximately **\(2.95+D+U\) seconds**, including restructuring, compact work and outgoing self-verification. This budget does not support three seconds.

If no new claims need restructuring, Send can reuse its existing structured claim and save that approximately 0.20 seconds. Immediate onward spending after Receive generally cannot take that saving.

**The two cheapest falsifying measurements are now specific.**

1. **Measure the export construction first.**
   Build a representative accumulated terminal state and the proposed direct Pasta-hashed ZK decider. Synthesize its complete compact verifier, including canonical parsing, all field links, claim restructuring and statement binding. Reject immediately if it exceeds the \(2^{17}/2^{18}\) shape or 5,120-byte carrier. If it fits, measure \(D+U\) separately: the cold-Send gate above is **≤1.06 seconds on Pixel 6**, subject to replacing the other assumptions with measurements. A verifier census can kill this design before implementing the wallet.

2. **Measure one genuine augmented Receive update.**
   Include actual signatures, incoming compact-proof arithmetic, maps, lookup helpers, accumulation-update verification, native Omega decisions and claim folding. Compare common-field limbs against native-field accumulators. Record instruction counts by phase, effective witness size and peak memory; then measure on Pixel. Include malformed encodings and the solved-false-generator case. Repeat at different history lengths to expose deferred work that grows with history. The table’s complete update gate is **≤21.7 G instruction equivalents**, followed by end-to-end three-second validation.

I would proceed with that accumulation prototype, preserving the 5,120-byte cap and proof-before-payout. I would describe **three-second local Receive as a falsifiable hypothesis, and three-second cold Send as an unresolved terminal-prover requirement**—not as a result that follows from replacing Pasta MSMs.
