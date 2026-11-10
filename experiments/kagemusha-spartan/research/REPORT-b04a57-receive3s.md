**In-clear inheritance removes a major cost, but it does not get the current Path 2 shape to 2.55 s Send or 2.95 s Receive.** Row precommitment helps; reaching those caps also requires a smaller recursive relation or a different proof schedule.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

**Hybrid H has the stronger structural opportunity for Receive.** It is admissible only if its completed local IVC proof is fully decided before payout. Checking a folding step alone is insufficient. Its other unresolved gate is Send: the complete, private export—not just its IPA—must fit the time and byte budgets.

I made no changes and ran no builds or prover benchmarks. Below, **M** means measured in the supplied artifacts, **D** means calculated from those measurements or source formulas, and **A** means an assumption or proposed acceptance threshold. Derived estimates inherit their stated assumptions.

**1. The revised in-clear critical path**

The owner’s linkage decision supersedes my earlier requirement to mask inherited ancestry tags. I therefore charge **no private restructuring IPA**. The latest census already incorporates that saving; subtracting it again would double-count it. Its selected geometry is:

| Item | Send | Receive |
|---|---:|---:|
| Primary geometry | 32 × 32,768 | 64 × 32,768 |
| Secondary geometry | 32 × 8,192 | 32 × 8,192 |
| Modelled pair size | 5,600 B D | 7,168 B D |
| Modelled prover core | 3.09 s D | 4.65 s D |

These are conditional census results, not measured recursive proofs. The implementation still lacks the complete recursive relations, and REPORT4’s reference counts are witness counts rather than a synthesized application constraint census. [CENSUS.md:84](../CENSUS.md), [RESULTS.md:108](../RESULTS.md)

The required sequence remains:

`input acceptance → complete preparation proof → Advance/receipt → completed proof(s) → full native verification → durable exact output → release`

For Receive, input acceptance includes full incoming Payment verification **before mutation**. [REPORT4.md:42](REPORT4.md), [wallet wire:975](../../../specs/kagemusha_wallet_wire_v1.md)

| Work | Payment-dependent work | Safe idle work after the previous operation |
|---|---|---|
| Own predecessor | Bind the exact selected predecessor into the new statement | Parse and fully verify its exact bytes; construct its scalar/group-verifier witness |
| Authenticated state | Current policy/time checks, changed leaves, successor state | Decode state, prepare unchanged authenticated data and reusable tree material |
| Inherited claims | Bind the incoming sources and derive the final combination challenges | Prepare head-only source descriptors, tensor data and fixed arithmetic |
| Preparation | Finish the proof for this exact operation | Prepare only genuinely head-dependent witness components |
| Advance/receipt | Entire serialized operation and receipt-dependent work | None |
| Witness commitments | Commit incoming, successor, receipt and challenge-dependent rows | Commit whole head-only rows using single-use blinding |
| Sumchecks | Current transcript challenges and reductions | Structural tables; possibly fixed matrix-times-head contributions |
| Own IPA | Current combined vector, weights and transcript | Parameters and randomness provisioning; no completed opening |
| Native verification | Incoming Payment before Advance; completed output afterward | Reuse exact previously accepted predecessor verification |
| Persistence/release | Exact selected operation and completed output | Storage provisioning, not monetary completion |

Thus my earlier statement needs narrowing: **Receive still cannot precompute its final merged proof or challenge-dependent opening, but it can now prepare substantial head-only witness and commitment work without an intervening restructuring proof.**

The final combination depends on the incoming Payment. Removing privacy restructuring does not make that Payment known earlier.

One-time setup comprises authenticated parameters, circuit/layout data and optional fixed-base tables. Recorded Vesta parameter derivation is **0.988 s M at 8,192**, **7.3 s M at 65,536**, and **31.8 s M at 262,144**. The selected 32,768-column setup is unmeasured. Load/cache these parameters; do not hide installation or process-restoration costs inside a “prepared” result. [Pixel Pasta results:9](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md), [:41](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md)

The source’s fitted model decomposes as follows. [main.rs:292](../src/main.rs)

| Prover phase | Send | Receive |
|---|---:|---:|
| Primary commitments | 0.869 s D | 1.738 s D |
| Secondary commitments | 0.264 s D | 0.264 s D |
| Outer sumchecks | 0.341 s D | 0.613 s D |
| Inner sumchecks | 0.524 s D | 0.944 s D |
| Own IPA pair | 1.090 s D | 1.090 s D |
| **Total** | **3.088 s D** | **4.649 s D** |

For a concrete idle-prepared example, assume **16 primary + 24 secondary rows A** are reusable for Send, and **16 primary + 16 secondary rows A** for Receive. These are hypotheses for a dependency census, not established reusable counts.

| Complete operation, seconds | Unprepared Send | Idle-prepared Send | Unprepared Receive | Idle-prepared Receive |
|---|---:|---:|---:|---:|
| Input/witness construction | 0.30 A | 0.10 A | 0.50 A | 0.25 A |
| Complete preparation proof | 0.40 A | 0.40 A | 0.60 A | 0.60 A |
| Online row commitments | 1.133 D | 0.500 D/A | 2.002 D | 1.435 D/A |
| Outer + inner sumchecks | 0.865 D | 0.865 D | 1.557 D | 1.557 D |
| Own IPAs | 1.090 D | 1.090 D | 1.090 D | 1.090 D |
| Complete native verification | 0.20 A | 0.20 A | 0.35 A | 0.35 A |
| Advance, encoding, durability | 0.20 A | 0.20 A | 0.20 A | 0.20 A |
| **Conditional total** | **4.19 D/A** | **3.36 D/A** | **6.30 D/A** | **5.48 D/A** |

This is a screening ledger, not a prediction interval. Real witness construction, helpers, cross-field binding and matrix evaluation can invalidate it. In particular, the measured synthetic commitments already exploit narrow public width classes; they cannot receive another blanket small-scalar speedup. [PIXEL.md:41](../PIXEL.md)

An especially useful screen is to pretend **every commitment can move to idle time**:

- Remaining core: **1.955 s Send / 2.647 s Receive D**.
- With prepared witness allowances **0.10/0.25 s A**, preparation **0.25/0.30 s A**, native verification **0.20/0.35 s A**, and durability **0.20 s A**, totals remain **2.70/3.75 s D/A**.

That impossible best-cache case already misses both caps. **Precommitment alone cannot rescue this geometry under these remaining allocations.**

**2. Row-level precommitment is sound in principle, with strict dependency and restart rules**

Use a fixed layout containing:

- Head-only rows.
- Payment/receipt-dependent rows.
- Current-transcript-dependent rows.

For a head-only row, compute privately during idle time:

\[
T_r=\langle w_r,G_{\rm active}\rangle+\rho_rH.
\]

At payment time, complete the other rows, absorb the actual operation header and **every witness-row commitment**, and only then derive the first witness-dependent challenge.

That matches the spike’s ordering: header, row commitments, absorption, then \(\tau\). Subsequent round commitments precede their own challenges. “All commitments before any challenge” must be interpreted per phase; later sumcheck messages necessarily depend on earlier challenges. [spartan.rs:424](../src/spartan.rs), [:488](../src/spartan.rs), [:528](../src/spartan.rs)

Three restrictions matter:

1. **Classify actual dependencies.** Cross-field RLC inputs may be head-only, but Horner outputs depending on the current binding challenge are online. That challenge must follow both descriptor commitments. Lookup helpers can have the same dependency. [REPORT5.md:124](REPORT5.md)
2. **Preserve uniform wiring.** Use fixed blocks and selectors, authenticated zeros and bounded exceptions. Arbitrarily packing “convenient” wires into rows can destroy the succinct matrix evaluator. Each additional serialized row costs **32 B D**, plus work in the next recursive verifier. [REPORT4.md:52](REPORT4.md)
3. **Leave the reserved tail empty.** Precommitment must not overlap the committed-round slots. The tail reduces actual witness capacity. [r1cs.rs:64](../src/r1cs.rs)

The idle artifact should bind the exact wallet/head/sequence, predecessor bytes, layout/parameter identities, row positions and authenticated source context.

Consume its randomness ticket durably before using it in a challenge-bearing attempt. An abandoned attempt burns its row blinds, masking-gate values, round blinds and Schnorr nonces. A changed transcript requires fresh coins. The deterministic head-row MSM contribution may remain cached; adding a new independent \(\rho H\) is cheap. This does **not** authorize reusing challenged blinding.

After Advance, restart must recover the selected operation. After completion, retries return the exact stored output bytes. Any continuation using retained randomness must reproduce the identical transcript; ambiguous continuation must fail closed rather than reuse that tape under new challenges. These rules extend the existing exact-byte custody contract. [wallet wire:1029](../../../specs/kagemusha_wallet_wire_v1.md)

The modelled saving is:

\[
\Delta T\approx 0.02716h_P+0.008259h_S\ {\rm seconds}\quad\textbf{D/A},
\]

where the row costs assume the model’s width mixture.

| Head-only rows, assumed | Saving |
|---|---:|
| 6 primary + 18 secondary | 0.312 s D/A |
| 16 primary + 24 secondary | 0.633 s D/A |
| 16 primary + 16 secondary | 0.567 s D/A |

Moving the same rows earlier adds **0 wire bytes D**. Rough private storage is **0.975 MiB per primary row / 0.229 MiB per secondary row D**, excluding metadata.

You can also cache \(Az_{\rm head},Bz_{\rm head},Cz_{\rm head}\). That saves a matrix-application pass, not the subsequent challenge-dependent reductions. Dense caches cost **96 MiB at \(2^{20}\) constraints / 192 MiB at \(2^{21}\) D**. The source does not isolate that pass’s timing, so I book **zero saving**. The final IPA vector still uses current row challenges. [spartan.rs:455](../src/spartan.rs), [spartan.rs:534](../src/spartan.rs)

A separate cheap experiment is **eliminating genuine zero padding**. The census currently sizes witnesses from constraint estimates and charges every padded active cell. Under those estimates, implicit authenticated zero rows could remove approximately **0.33 s Send / 0.68 s Receive D/A** of commitment work. This needs real witness synthesis and a recursive recount; do not count those rows again as idle secret rows. [main.rs:263](../src/main.rs)

**3. Receive’s two proof pairs cannot retrospectively share a transcript**

The own predecessor and incoming Send already have independently fixed Fiat–Shamir transcripts. Their statements, commitments and challenges differ. One new verifier can process them together, but it must faithfully reconstruct both transcripts.

| Proposed sharing | Assessment |
|---|---|
| One replacement transcript/challenge schedule | Invalid for already-created proofs |
| Shared generators, structural constants and code | Valid |
| One matrix evaluation result | Generally invalid: evaluation points differ |
| Batched final scalar/group equations | Possible after all source statements and transcripts are fixed |
| One current IPA handling inherited sources | Already included in census v4 |
| One short tensor descriptor replacing arbitrary source descriptors | Requires another proved reduction |
| Batch incoming and outgoing native verification | Cannot cross the pre-Advance input-acceptance boundary |

For residual batching, derive fresh, domain-separated coefficients after fixing the complete ordered source set, statements, shapes and residual equations. Preserve canonical decoding, group membership and field separation; account for batching error and adaptive Fiat–Shamir soundness. Batching does not eliminate source transcript reconstruction or native generator decisions.

The census allocates only **2,000 constraints A per matrix evaluator**. Even eliminating one evaluator in each field would remove only **4,000 constraints D**, approximately **0.001 s D** of its outer-sumcheck model absent a size-boundary crossing. [census.rs:27](../src/census.rs)

There is also an important correction to “one claim per field”: **the census has one joint opening per field, but Receive still exports two source descriptors per field.** The code sets `exported = sources`, with one source for Send and two for Receive. [main.rs:241](../src/main.rs)

At the selected dimensions, these descriptors cost **512 B Send / 1,024 B Receive D**. Although

\[
G_*=G_0+\xi G_1,
\]

its vector is \(s(u_0)+\xi s(u_1)\), generally not another tensor \(s(u_*)\) with a short challenge list. Retain the source descriptions or prove a further reduction.

The proposed joint opening already handles

\[
P+\sum_j\xi^jG_j,\qquad
\kappa+\sum_j\xi^j\langle s(u_j),b\rangle,
\]

including the reserved-tail contribution. Its written soundness argument remains open. [CENSUS.md:53](../CENSUS.md)

For generic two-pair batching I would book **0 s**, with **0.05 s A** as an exploratory saving target. A measured circuit-size reduction could be more valuable, but that must emerge from synthesis.

**4. Hybrid H: promising, but its completed local proof and private export both need construction**

I would investigate Pasta-native CCS/LCCCS accumulation with a CycleFold secondary before trying to preserve the current ProtoGalaxy stage geometry. HyperNova supplies the relevant multi-folding framework; CycleFold’s small secondary estimate concerns the outsourced scalar multiplication, not an entire incoming Send verifier. [HyperNova](https://eprint.iacr.org/2023/573), [CycleFold](https://eprint.iacr.org/2023/1192)

H removes the wallet’s **own predecessor decider verifier**. It does not remove its fold-update verifier, incoming Send verification, state bindings or terminal decisions.

**The specifications permit a private Receive artifact, but do not currently define H’s artifact.**

| Surface | Requirement and consequence |
|---|---|
| Receipt | The receipt binds the exact statement/proof digest. It is custody/authorization evidence, not a substitute for the relation proof. [proposal:676](../../../specs/kagemusha_single_design_proposal.md) |
| Local Receive completion | Existing rules produce and verify an own step proof and release a durable complete package. Today’s stronger owner ordering governs this consult. [proposal:700](../../../specs/kagemusha_single_design_proposal.md) |
| Immediate transport | **Credited is optional.** There is no requirement to transmit a new Receive proof immediately after every Receive. [proposal:1310](../../../specs/kagemusha_single_design_proposal.md) |
| Credited evidence | Receive form carries its proof and receipt; Status form carries a lineage proof and membership opening. H must finalize before exporting either equivalent proof-bearing form. A bare acknowledgement cannot replace it. [wallet wire:920](../../../specs/kagemusha_wallet_wire_v1.md) |
| Recovery/backup | Private witnesses are marker-bound local custody, excluded from backups. They need not be succinct or ZK while retained privately. [proposal:1206](../../../specs/kagemusha_single_design_proposal.md) |
| Core payment reports | I found no independent mandatory export under that name in the requested specifications and searched Core paths. The concrete external proof consumers found include Credited, fee claims, Unload and retirement; I would not infer another mandatory report without its API. |
| Unload | Requires the complete committed package and lineage decision before ledger payout. H must export a completed ZK decider here. [proposal:1627](../../../specs/kagemusha_single_design_proposal.md) |

The enrollment policy defines admission identities and issuer policy; it does not waive payment-proof completion. [enrollment policy:17](../../../specs/kagemusha_wallet_enrollment_policy_v1.md)

**A completed fold is not, by itself, a completed validity proof.**

Current `verify_fold` checks the reduction and constructs the resulting accumulator. Full `decide` additionally checks witness openings, copies, lookup sums and accumulated error. The source explicitly describes the native deciders as test oracles, not compression proofs. [fold.rs:733](../../../crates/iroha_fold/src/fold.rs), [decide.rs:1](../../../crates/iroha_fold/src/decide.rs), [:107](../../../crates/iroha_fold/src/decide.rs)

A correct reduction can carry an unsatisfied instance into an unsatisfied accumulator. Detecting that at the next Send is exactly the deferred-proof failure the owner ruled out.

H therefore has two possible completion standards:

- Produce and verify a succinct decider on every Receive. This substantially restores the cost H intended to remove.
- Define the completed local proof as a **private witness-bearing IVC certificate**, and fully verify that certificate before release. This can be cryptographically meaningful and need not be succinct, but accepting it as the owner’s required proof is an explicit decision—not something implied by the word “fold.”

The second option must check the complete IVC statement, base/step linkage, current state, both fields, all outstanding obligations and terminal witnesses. Merely evaluating a prover-selected relaxed equation is insufficient.

There are two useful implementations of that local decision:

1. **Generic exact reopening.** Recompute every commitment and evaluate the terminal relation.
2. **A new complete verifier with batched openings or authenticated inductive checks.** Avoid independently recommitting every dense row while still checking the entire supplied certificate.

For batched openings, freeze the exact witness, blinds, commitments and instance first, then draw unpredictable verifier coefficients \(\eta_r\). Check

\[
\sum_r\eta_r C_r
=
\operatorname{Com}\left(\sum_r\eta_r w_r,\sum_r\eta_r\rho_r\right),
\]

and independently check all required terminal relations and links. This replaces many row MSMs with a field pass and a row-length MSM. It introduces a quantified probabilistic check. Coefficients derived only from commitments are inadequate if the prover can subsequently choose inconsistent row witnesses to cancel.

This is **proposed engineering**, not current `verify_fold`. A faulty vector update, omitted secondary, false opening, wrong terminal state or unsatisfied fresh step must fail before the release boundary.

**The local decision has a large cost difference.**

For an illustrative accumulator with **\(2^{20}\) primary and \(2^{17}\) secondary field entries A**:

- Witness vectors: **36 MiB D**.
- A secondary relaxed error vector of \(2^{17}\) entries adds **4 MiB D**.
- Reopening the witness vectors independently costs approximately **2.5 s D**, using the measured large full-width MSM rate; including that error vector gives approximately **2.8 s D**.
- Those costs exclude relation evaluation, preparation, folding and persistence.

The existing generic reopening approach therefore fails the Receive budget. [Pixel Pasta results:27](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md), [decide.rs:35](../../../crates/iroha_fold/src/decide.rs)

With batched openings, two 8,192-length secret MSMs contribute approximately **0.065–0.073 s D from measured kernels**, plus dense field passes and terminal relation evaluation. A **0.60 s A complete local-decision gate** is worth testing. It is not an achieved result.

**Send’s export must decide the accumulated relation, including previous Receives.**

A uniform H accumulator is sized for its largest supported operation. Separate operation accumulators do not evade this: export must close every live relation slot and the secondary. The current IVC owner already makes each relation slot separately accountable. [ivc.rs:10](../../../crates/iroha_fold/src/ivc.rs)

A genuine LCCCS terminal decision checks linear evaluation claims and can avoid rerunning a fresh CCS outer zero-check. Using the fitted inner-pass rate and approximately **0.60 s D/M** for two short IPAs gives:

| Allocated vectors, assumed | Inner work | Short IPA pair | Base linear decision |
|---|---:|---:|---:|
| \(2^{19}+2^{17}\) entries | 0.262 s D | ≈0.60 s D/M | ≈0.86 s D/A |
| \(2^{20}+2^{17}\) entries | 0.472 s D | ≈0.60 s D/M | ≈1.07 s D/A |

These exclude randomization, its proof, additional secondary/error obligations and native verification. They are not timings for the spike unchanged.

The byte tradeoff is equally tight. Applying the existing serializer as a **screening skeleton**, with its assumed header and one inherited descriptor per field:

| Skeleton geometry | Usable primary / secondary cells | Skeleton bytes | Remaining allowance |
|---|---:|---:|---:|
| P 64×8,192; S 16×8,192 | 472,448 / 120,512 D | 5,920 B D | 2,538 B D |
| P 128×8,192; S 16×8,192 | 944,128 / 120,512 D | 8,032 B D | 426 B D |
| P 64×16,384; S 16×8,192 | 996,352 / 120,512 D | 6,064 B D | 2,394 B D |

The usable capacities exclude the reserved tails. **None is a complete H serialization.** Accumulator interfaces, secondary error commitments, randomization and the blinding relation must fit or replace explicitly identified components. [r1cs.rs:64](../src/r1cs.rs), [:75](../src/r1cs.rs)

The larger short-IPA skeleton has only **426 B D** spare. The 16,384-column alternative buys bytes but raises the modelled IPA pair to approximately **0.79 s D**, making the base linear decision approximately **1.26 s D/A**. This is a concrete byte/time tradeoff for the spike.

**Randomizing an accumulator is not automatically zero knowledge of its export.**

HyperNova explicitly distinguishes a randomizing CCS fold from a hiding folding transcript. Its privacy construction checks randomization in an auxiliary `blind` circuit and hides that circuit using Nova’s folding construction. Thus “fresh random instance committed during idle time” is an ingredient, not a complete privacy argument. [HyperNova §7 and Appendix D](https://www.andrew.cmu.edu/user/bparno/papers/hypernova.pdf)

For H, either implement that complete construction or give a direct ZK decider relation that hides and checks the entire equivalent interface. Charge its proof bytes, circuit and runtime.

Idle work can generate and commit random instances and prepare their fixed data. It cannot precompute cross terms with an unknown final accumulator or final transcript challenges. A naïve full-vector mask at the illustrative size incurs approximately **2.5–2.8 s D** of commitment work alone. That work belongs to **unprepared Send** when no unused mask is available.

Allowed ancestry tags do not authorize revealing balances, private accumulator evaluations, masks or local folding transcripts. Every new export requires fresh attempt randomness; exact completed retries replay stored bytes. Vega provides a concrete example of the separate relaxed-instance masking and opening obligations. [Vega privacy construction](https://microsoft.github.io/vega-prover/mc/zero-knowledge.html)

**Storage is part of H’s critical path.**

For the illustrative **40 MiB D** accumulator including secondary error data:

- Redundant current-generation copies require **80 MiB D**.
- Keeping old and successor generations during publication requires approximately **160 MiB D**.
- A prepared randomization witness adds another comparable allocation, depending on the construction.
- Proof scratch, fresh witnesses, maps and retained exact messages are additional.

The current capsule cap is **524,288 B**, so this requires a specified authenticated witness sidecar and revised custody schema, not placing the accumulator into the existing capsule. [proposal:1197](../../../specs/kagemusha_single_design_proposal.md)

Before release, authenticate and durably publish the successor certificate, its witness reference, receipt and exact output. Crashes after Advance must resume that operation; missing selected witness data is custody loss, not authorization to reconstruct a different history. Old generations can be collected only after the successor and required recovery material are safely retained.

The observed **285 MB M** spike peak does not cover H. I would budget **512–896 MiB A** for an initial implementation and measure whole-process peak. Durable witness storage and simultaneous in-memory copies must be counted separately. [PIXEL.md:17](../PIXEL.md)

**H still needs a recursive composition argument.**

Own-lineage folding avoids repeated own-decider verification, but incoming Sends join independently generated histories. The payment system remains a PCD composition problem. HyperNova’s concrete cycle construction itself states a non-interactivity assumption; citing its IVC theorem does not validate a new Hyrax, transcript and export composition. [HyperNova, Appendix E](https://www.andrew.cmu.edu/user/bparno/papers/hypernova.pdf), [split accumulation and PCD](https://eprint.iacr.org/2020/1618)

The existing recursive Fiat–Shamir selection is explicitly tied to current parameters, relations and formats. It does not automatically authorize Path 2 or H. Both need an applicable theorem or an explicitly selected assumption for the exact replacement, with finite operation/query budgets. [soundness specification:41](../../../specs/kagemusha_recursion_soundness_v1.md), [Halo Infinite](https://eprint.iacr.org/2020/1536)

**5. Scheduling: useful after the arithmetic closes, with zero gain booked initially**

Use one bounded worker pool. The existing MSM column scheduler already dynamically assigns work and limits shared scratch; two independent eight-thread provers would compete for the same machine. [msm/mod.rs:202](../../../crates/iroha_pasta/src/msm/mod.rs)

On the stated **2 X1 + 2 A76 + 4 A55** topology, test:

- X1 priority for sequential transcript work, short late IPA rounds and dependency-critical tasks.
- Dynamic large row/MSM and field-work chunks across the available pool.
- A55 participation in independent chunks that cannot leave a fixed partition as the final straggler.
- Two-worker, four-big-core and shared-eight-worker configurations.

For Path 2, descriptor commitments and the cross-field binding challenge are synchronization points. Subsequent primary/secondary work may overlap once its inputs are fixed. That permits overlap; it does not justify replacing the sum of their eight-thread timings with their maximum.

For H, CycleFold witness work depends on the primary folding equations and challenges. Independent commitments can precede that boundary; the dependent secondary cannot.

Within an IPA round, L/R MSMs are candidates for bounded parallel execution. The next round still waits for both messages and its challenge. The spike currently computes these MSMs sequentially. [ipa.rs:81](../src/ipa.rs)

Book **0 s A scheduling saving** initially. Keep a scheduling change only if it improves complete-stage wall time by **at least 10% A**, without worsening peak memory or thermal-steady latency.

**6. What would actually reach the caps**

Under the fixed owner instruction, **Path 2 remains the compatible baseline**. H becomes the more promising Receive architecture only after explicit acceptance of a fully verified private witness-bearing IVC certificate as the completed local proof.

For Path 2, the important structural threshold is reducing Receive’s primary from the current shape to **32×32,768 with \(2^{20}\) constraints A**:

- Usable witness capacity: **1,022,560 cells D**.
- Constraint capacity: **1,048,576 D**.
- Modelled core saving from the larger shape: **1.56 s D**, before revising the idle-work allocation.

Against the current approximate reference of \(876,608+344,000\), this requires cutting roughly **198,000 witness cells / 172,000 constraints D**. Merely batching two verifiers does not establish that cut. REPORT4’s application census and the real lookup/compiler design determine whether it exists.

For a prepared Path 2 operation, this is the useful final acceptance ledger:

| Phase | Send cap | Receive cap |
|---|---:|---:|
| Input/state witness | 0.10 s A | 0.25 s A |
| Complete preparation | 0.25 s A | 0.30 s A |
| **All remaining prover work** | **1.80 s A** | **1.85 s A** |
| All native verification | 0.20 s A | 0.35 s A |
| Advance and durable completion | 0.20 s A | 0.20 s A |
| **Total** | **2.55 s A** | **2.95 s A** |

The current prepared example has **2.46/4.08 s D/A** of remaining prover work. That is the gap to close—not another presumed restructuring saving.

For H, a separate falsifiable allocation is:

| Phase | Send cap | Receive cap |
|---|---:|---:|
| State/witness construction | 0.15 s A | 0.25 s A |
| Complete preparation | 0.20 s A | 0.30 s A |
| Fresh step, multi-folding and CycleFold | 0.35 s A | 1.20 s A |
| **Complete private export, including randomization/blinding proof** | **1.40 s A** | — |
| Native outgoing / incoming proof verification | 0.20 s A | 0.20 s A |
| **Complete local accumulator decision** | Covered by verified export | **0.60 s A** |
| Advance and durable checkpoint | 0.25 s A | 0.40 s A |
| **Total** | **2.55 s A** | **2.95 s A** |

This is an acceptance budget, not a forecast. At the larger short-IPA assumption, the base linear export consumes approximately **1.07 s D/A**, leaving only **0.33 s D** for the rest of the complete export. At the 16,384-column alternative, approximately **0.14 s D** remains. Either can cheaply falsify H.

These H caps assume idle work completed. Without a prepared full-vector random instance, Send adds approximately **2.5–2.8 s D** of masking commitments, plus generation and other preparation. Receive’s unprepared increment is the actual omitted head-witness/commitment work; it cannot be priced from the available dependency census.

I would run the spikes in this order:

| Order | Cheapest falsifying experiment | Pass/fail condition |
|---|---|---|
| 1 | Freeze exact algebra and count-only layouts for Path 2 and H | All obligations included; uniform matrix evaluator; all operation orders; no unpriced privacy/interface proof |
| 2 | Serialize H’s **complete** private export | **≤8,458 B A**; full Payment **≤12,000 B A** |
| 3 | Pixel H export, including blinding/link relation | **≤1.40 s A**; otherwise the proposed Send budget fails |
| 4 | Pixel H complete local decision, with corrupted witness/opening/fold/state tests | **≤0.60 s A**, with every fault refused before release |
| 5 | Path 2 real dependency layout, precommitment and authenticated zero-row elimination | Remaining prover **≤1.80 s Send / ≤1.85 s Receive A**; recursive bytes still fit |
| 6 | Genuine H Receive step with incoming verifier, lookups, own fold checks and CycleFold | **≤1.20 s A** prepared |
| 7 | Durable publication and restart campaign | H checkpoint boundary **≤0.25/0.40 s A**; exact retries and no replacement after selection |
| 8 | Scheduling and complete phone qualification | **p95 ≤2.55 s Send / ≤2.95 s Receive A**, thermal steady state; unprepared and process-cold reported separately |

Path 2 needs no renewed decision about ancestry linkage. It needs its exact replacement proof/transcript specification and recursive security premise accepted.

H additionally needs a decision on the **completed local proof standard**, revised accumulator custody, and the handling of optional proof-bearing Credited exports. It must not be approved or benchmarked as “incoming verification plus a fold, final decision later.”

**No complete measured implementation currently reaches either cap. The decisive experiments are a smaller real Path 2 relation, or H’s complete local decision plus a ≤1.40-second, ≤8,458-byte private export.**
