**I would prototype a 4,992-byte Omega built from a small-field zero-knowledge proof, a rectangular IPA compressor, and one accumulated claim that every verifier decides natively.** Private proofs over the relevant large fields handle the expensive curve computations.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

The key change to round 1 is that **the compressor need not independently provide zero knowledge**: its entire private input can be an already zero-knowledge proof of exactly the statement being published.

This is a concrete research construction, not a demonstrated three-second implementation. Its byte arithmetic works under explicit circuit-size limits. Its timing remains conditional on two measurements below. I read the [round‑1 report](REPORT.md) and made no repository changes.

The design has four parts.

1. **A completed KoalaBear core proof.** This proves the operation, its authorizations, state changes, predecessor, and incoming-payment obligations. It uses the hiding construction from round 1. The wallet retains this proof locally.
2. **Private native-field coprocessor proofs.** These establish P‑256 verification, the incoming compact proof’s Pasta scalar computations, and its curve equations. The KoalaBear core verifies these proofs completely.
3. **A final hash-proof layer with Pasta-native hashing.** This proves verification of the completed core proof, using Poseidon over Pasta Fp for its Merkle commitments and transcript. Its arithmetic remains explicitly bounded small-field arithmetic.
4. **A compact Spartan-style argument over Fp**, using Vesta commitments arranged as 16 rows of 8,192 scalars and one length‑8,192 IPA.

The last two layers see only proof bytes, public inputs, and deterministic verifier scratch. Application secrets never enter their witnesses directly.

The cryptographic ingredients come from [Spartan](https://eprint.iacr.org/2019/550), [Hyrax](https://www.pepper-project.org/hyrax-oakland18.pdf), [WHIR](https://eprint.iacr.org/2024/1586), and existing claim accumulation. Their proposed composition and parameters require a new argument and implementation.

**The proposed Omega is exactly 4,992 bytes, provided the final verifier fits the selected shape.**

Let its R1CS contain at most \(2^{17}\) private witness coordinates and \(2^{18}\) constraints. Pad the complete assignment, including public inputs, to \(2^{18}\).

| Component | Encoding | Bytes |
|---|---|---:|
| Header and statement binding | Fixed, including scheme/relation identity and statement digest | 128 |
| Witness commitments | 16 compressed Vesta points | 512 |
| First sumcheck | 18 rounds × 3 Fp elements | 1,728 |
| Terminal A/B/C evaluations | 3 Fp elements | 96 |
| Second sumcheck | 18 rounds × 2 Fp elements | 1,152 |
| Witness evaluation | 1 Fp element | 32 |
| IPA | 13 L/R pairs and final scalar | 864 |
| Own folded-generator point | 1 compressed point | 32 |
| Inherited generator claim | 1 point and 13 challenges | 448 |
| **Complete Omega** | | **4,992** |

There are **128 spare bytes** under the existing cap. The curve, generator set, dimensions and claim interpretation are fixed by the authenticated suite; there are no variable-length auxiliary proofs hidden outside this count.

Rectangular commitments are essential. Committing the same witness as one vector would require a length‑131,072 IPA. Sixteen row commitments cost 512 bytes but reduce the opening to length 8,192.

The verifier evaluates the fixed R1CS matrices itself. This avoids adding Spartan’s matrix-commitment proof. **That work is still real:** native ledger verification pays it, and recursive verification must prove it through the Fp coprocessor. I am not assuming arbitrary matrix evaluation becomes logarithmic.

The proposed serialization is an arithmetic specification, not an implemented codec result.

**Privacy can be supplied once, inside the compressor.**

If the completed core proof \(\pi\) is zero knowledge for public statement \(x\), then computing another proof from \((x,\pi)\) need not expose application secrets: simulate \(\pi\), then run the compressor on the simulated proof. The intermediate hash-proof layer can use the same reasoning.

This requires composable simulation with domain-separated transcripts. The compressor must also be **knowledge sound**, so an accepting compressed proof yields an accepting inner proof; ordinary soundness for the existential statement “some accepting proof exists” is insufficient.

This does **not** justify exposing today’s non-ZK folded wallet witness. The existing [decider-v2 explicitly lacks zero knowledge](../../../crates/iroha_fold/src/decider_v2/mod.rs). Here its replacement’s witness contains only an already-private proof.

**The accumulated claim closes the recursion without making the next phone prove an 8,192-base MSM.**

Write a generator claim as

\[
A=(G,u_0,\ldots,u_{12}),\qquad
\operatorname{Decide}(A):
G=\langle s(u),g_0,\ldots,g_{8191}\rangle .
\]

Every public verifier performs:

\[
\operatorname{SuccinctVerify}(\Omega,x)
\land \operatorname{Decide}(A_{\rm own})
\land \operatorname{Decide}(A_{\rm inherited}).
\]

All three are mandatory before accepting value.

The induction is:

- **Bootstrap:** start with the pinned trivial claim.
- **Send:** verify the retained predecessor core proof and preserve its inherited claim.
- **Receive:** verify the retained predecessor core proof; verify the incoming Omega’s succinct checks; accumulate the predecessor’s inherited claim, the incoming inherited claim, and the incoming Omega’s own generator claim.
- **Unload:** publish the resulting Omega and decide everything natively.

The local predecessor’s optional transport wrapper introduces no obligation because the phone retains and verifies its underlying core proof.

This follows the existing distinction between an accumulated obligation and completed verification. The repository even documents how a false statement can satisfy succinct verification by choosing its folded-generator point appropriately; the native decision is what rejects it. [Explicit forgery and acceptance boundary.](../../../specs/plonk_ipa_v1.md)

Accumulation itself is not free. It constructs another length‑8,192 IPA and proves its masked update correctly. The proposed 13-round private fold body is approximately 928 bytes; it stays inside the core witness. Its construction must preserve the [current input binding, masks and fold equation](../../../specs/kagemusha_recursion_soundness_v1.md).

Invalid incoming evidence must still produce the prescribed burn outcome. Preserve the existing **corrected-claim** mechanism: prove an ordinary verification failure, or authenticate one corrected generator point and prove that it differs from the supplied point. Accumulate the corrected claim, with other incoming slots trivial. The final native decision authenticates the correction. Keep transcripts bound to the original incoming bytes; never silently substitute corrected bytes. The predecessor obligation remains mandatory. [Current total-verification policy.](../../../specs/kagemusha_lambda_omega_v1.md)

**Private coprocessors are necessary for the next-hop cost.**

Directly verifying this compact proof inside KoalaBear would be expensive. Sixteen row commitments and 26 IPA points produce roughly 42 variable bases before constants. A rough shared-window calculation gives thousands of curve operations, or approximately **13–20 million 12-bit limb products** before carry, range and scalar checks.

Deferring the long generator MSM does not remove that smaller variable-base MSM.

Instead, the phone privately produces:

- an **Fp proof** of scalar transcript checks, sumchecks and fixed-matrix evaluations;
- an **Fq proof** of Vesta point decoding, group equations and accumulation updates;
- a **P‑256-field proof** of the operation’s hardware-signature checks.

Use sumcheck plus a hash-based PCS for these, with KoalaBear hashes so their authentication paths are inexpensive in the core. Every lane binds exact source bytes, authorized relation identity, challenges, coefficients and results. The core proves their complete verification—not a native Boolean verdict.

These proofs need not themselves hide their witnesses because they remain private inputs to the hiding core. Their soundness must meet the full composed security target.

Longfellow’s 152 ms result therefore becomes relevant as evidence for native-field signature work, but it is not a transferable timing guarantee. Its current [audit gives 116/117-bit profiles](https://github.com/longfellow-zk/longfellow-zk/blob/main/formal/AUDIT.md), which cannot be adopted unchanged. Nor is a Ligero verifier merely logarithmic foreign-field work: sampled-column checks still process substantial large-field data. WHIR is the stronger candidate, but its terminal vector and final linear-form obligations must also be checked.

**The messages carry completed terminal proofs. There is no transported sigma.**

The following are proposed hard budgets. The proof size is specified above; metadata budgets require codec verification. Current measured fixed overheads are 1,394 bytes for Payment and 679 bytes for Credited::Receive. [Wire arithmetic.](../../../specs/kagemusha_wallet_wire_v1.md)

| Message | Proof objects and contents | Envelope budget | Complete transport budget |
|---|---|---:|---:|
| Request | Signed Request, fee schedule and authority material; no lineage proof | 2,048 B | **6,244 B**, including up to 4,096 B AccountId companion, 16 B framing and 84 B header |
| Payment | Completed **Send Omega**, Request binding, payment terms, receipt and public state claim; sigma absent | 1,664 + 4,992 = **6,656 B** | **8,804 B**, including up to 2,048 B payer companion, 16 B framing and 84 B header |
| Receipt/Credited | Completed **Receive Omega**, exact Payment digest and credited outcome; sigma absent | 768 + 4,992 = **5,760 B** | **5,844 B**, including 84 B header |

The metadata allowances accommodate the revised digest and statement fields; exceeding them fails the design rather than silently enlarging the carrier.

At the current chunk sizes, Payment requires **three NFC chunks** and at most **35 data shards** of 256 bytes. QR parity and header frames must still be counted in the actual layout. The complete Payment is smaller than the quoted existing 10,034-byte envelope, even after including its companion and transport header; this gives room to preserve the owner’s existing first-screen presentation. It does not establish a physical-camera qualification result.

The ordering remains:

> preparation proof → serialized Advance and receipt → completed core proof → compact Omega → full native verification → durable exact-byte output → release.

Payment’s digest binds the complete canonical terminal Omega and all Payment fields. Retries return frozen bytes. Credit identity remains independent of proof randomness. The existing [Advance and recovery contract](../../../specs/kagemusha_single_design_proposal.md) remains the authority boundary.

**The three-second allocation is possible on paper, but has essentially no contingency.**

These are engineering acceptance budgets for Pixel 6, **not predictions or measurements**. “Cold” means no operation-specific readiness proof; installed parameters are available. Human confirmation, scanning and radio transfer are additional.

| Critical-path work | Cold Send | Cold Receive | Prepared Receive |
|---|---:|---:|---:|
| Witness preparation and pre-Advance proof | 0.15 s | 0.20 s | 0.05 s |
| Native-field coprocessor proving | 0.10 s | 0.35 s | 0.30 s |
| Application work and core recursive-verifier processing | 0.40 s | 0.65 s | 0.55 s |
| Core hiding commitments and remaining proof work | 0.45 s | 0.55 s | 0.55 s |
| Masked claim accumulation | — | 0.20 s | 0.20 s |
| Final hash layer plus compact compressor | 0.80 s | 0.80 s | 0.80 s |
| Native decisions, hardware receipt and durability | 0.25 s | 0.25 s | 0.25 s |
| **Total target** | **2.15 s** | **3.00 s** | **2.70 s** |

Prepared Send targets approximately **2.0 seconds**. A sequential cold Send→Receive exchange consequently targets approximately **5.15 seconds plus transport and user interaction**, not three seconds for both phones.

There is some local calibration for the compressor. Existing measurements report approximately 40,200 instructions per full-width committed scalar and 25.14 billion instructions for a length‑65,536 IPA. Applying round 1’s rough phone calibration gives:

\[
T_{\rm commit}(2^{17})\approx0.34\text{ s},\qquad
T_{\rm IPA}(2^{13})\approx0.20\text{ s}.
\]

That leaves only **0.26 seconds** of the 0.8-second allocation for the final hash layer, witness construction, sumchecks, transcript work and verification. Linear scaling and cross-machine instruction calibration are uncertain. [Measurement source.](../../../output/kagemusha-nova-folding-20261106/RESULTS.md)

The circuit-size limit is equally severe. Roughly 500 Poseidon permutations with 80 fifth-power S-boxes each already imply around **120,000 multiplication intermediates**, before small-field reductions, canonical parsing and statement binding. “A few hundred hashes” is not sufficient evidence that the wrapper fits.

The final layer must use reviewed Pasta-native hash parameters and bounded integer gadgets for KoalaBear operations. Representing a 31-bit value in Fp does not make arithmetic modulo KoalaBear’s prime free.

**No background compaction is required for correctness or continued spending.**

Optional background work consists of public table preparation, authenticated key-table preparation, and readiness proofs for the current head and Request. A conflicting operation invalidates readiness work. Fresh masks remain single-use.

There is no delta backlog and no limit on consecutive payments imposed by stale compact checkpoints. Every released operation finishes its own compact proof.

The worst case is explicit: if finalization takes ten seconds, the operation remains pending for ten seconds. The protocol never releases value on a promise of later compression. A cold application launch also adds parameter loading and initialization, whose time is unmeasured.

Memory targets are:

- core prover: **≤768 MiB**;
- one active coprocessor or compressor phase: **≤128 MiB** additional;
- public tables, retained proofs and wallet state: remaining margin;
- **≤1 GiB whole-process peak**, with scratch reused between phases.

The compressor’s raw \(2^{17}\)-element Fp witness is only 4 MiB; matrices, proving buffers and tables dominate its remaining allocation. Coprocessor proof bytes can be hundreds of kilobytes locally without affecting QR. Concurrent full prover instances are excluded from this memory budget.

Sustained, thermally stabilized runs must meet the targets. A cool three-second result that becomes 4.8 seconds under repeated payments fails.

The other proposed directions explain why I chose this combination:

| Direction | Assessment |
|---|---|
| Old compact Omega plus delta | Sound only if the delta proves every intervening transition, including incoming lineage, or verifies the latest hash proof. The latter recreates the finalizer. A bounded delta introduces stalls. |
| Precompacted spending tickets | Can provide fast prepared payments with reserved liquidity. Received value and change need compaction before reuse; cold and immediate onward spending lose the three-second property. |
| Deferred native claims | Useful and selected here. Every obligation must be accumulated soundly and decided before acceptance. The update still includes an IPA and proved group equations. |
| Pasta-native final hash and IPA | Selected, with rectangular commitments and privacy inherited from the core proof. Circuit census is the decisive sizing gate. |
| Receiver challenge | Useful for binding and preparation, not public soundness against colluding payer and receiver. Keep Fiat–Shamir challenges derived after the relevant commitments. |
| Native P‑256 lane | Selected as a private coprocessor, with its complete verifier included in the core and strengthened soundness parameters. |

The approximately 0.7-second current sigma figure also needs careful labeling: the repository calls it a **phone estimate**, not a qualifying measurement of the proposed delta. [Timing qualification.](../../../specs/kagemusha_lambda_omega_v1.md)

Ledger verification installs the new authenticated proof suite and checks the compact argument, its own generator claim, and its inherited claim. Unload, fee claims and status evidence use that same complete predicate. Native verification includes fixed-matrix evaluation and two length‑8,192 generator decisions; none is an optional cache assertion.

Load preserves its current funding boundary: ordinary Sumeragi finality is authenticated natively before Advance. This design does not claim to remove an in-circuit BLS verifier that the current construction does not have. Monetary conservation, load ordinals, unload nullifiers and exactly-once credit handling remain unchanged.

The security argument is an induction over completed operations: knowledge soundness extracts the final hash proof and then the hiding core proof; the core establishes application correctness, authorizations, predecessor verification and complete coprocessor verification; sound masked accumulation carries every unresolved generator obligation to a claim that the public verifier actually decides. Corrected claims establish invalid-input outcomes without trusting supplied verdicts. Privacy follows from the core’s zero knowledge, the masked accumulator’s privacy, and simulation-preserving compression. This uses no trusted setup and retains the current Pasta discrete-log assumptions and provider trust boundary. **The complete composition, concrete soundness budget and privacy simulation still need review; the byte layout alone does not establish them.**

The first two measurements should decide whether to continue:

1. **Compile and time the complete finalizer before implementing wallet logic.**
   Generate a representative completed core proof at the intended security parameters, construct the Pasta-hashed final layer, and synthesize its exact verifier—including parsing, reductions, public-claim and accumulator bindings. Require **≤131,072 private coordinates, ≤262,144 constraints, exactly 4,992 transport bytes, and ≤0.8 seconds on Pixel 6 including the final hash layer**. A witness census alone can cheaply reject the selected shape.

2. **Measure one complete compressed-proof hop.**
   Exercise compact Send → private Fp/Fq coprocessors → KoalaBear Receive → masked accumulation → compact Receive. Include valid input, malformed input and a solved false-generator claim. Require the coprocessor provers plus their full embedded verification and accumulation to fit their allocated budget, and complete cold Receive to meet **three seconds at thermal steady state**. This rejects a fast compressor whose output is too expensive for the next phone.

The proposed invention is therefore a **4,992-byte, publicly decided proof composition with zero knowledge concentrated in the core**. It supplies a concrete route to test the missing piece. Until those two gates pass, I would describe three seconds as the design target—not as a solved result.
