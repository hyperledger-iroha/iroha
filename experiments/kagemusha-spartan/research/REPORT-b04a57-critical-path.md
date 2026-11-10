**The strongest next experiment is shorter IPAs plus a joint opening for inherited claims. Safe precomputation helps Send substantially, but cannot remove Receive’s payment-dependent restructuring. Three seconds remains unproven.**

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

There is also a significant sizing risk: increasing Hyrax rows makes the *next operation’s recursive verifier* larger. The byte optimization must survive that feedback before its timing can be trusted.

I made no changes or new measurements. `pixel-plonky3/RESULTS.md` and `spike/zk-spartan` are absent from this checkout; I used DESIGN’s Plonky3 summary and your supplied ZK construction. Below, **M** means measured, **D** derived, and **A** assumed. Derived timings inherit their stated assumptions.

**1. What can move off the critical path**

The required order stays:

`complete preparation proof → Advance/receipt → complete primary+secondary → full native verification → durable exact bytes → release`.

That is the selected architecture’s explicit boundary. Its primary and secondary certify the same operation and task list without circular hashing of their current complete proofs. [REPORT4.md:34](REPORT4.md)

| Work | Send critical dependency | Receive critical dependency | Safe idle work after the previous operation |
|---|---|---|---|
| Parse/authenticate previous head and proof pair | Confirm the selected predecessor is unchanged | Same | Parse, fully verify, cache exact accepted bytes and derived verifier values |
| Predecessor scalar/group-verifier witness | Bind it into the new operation | Bind it alongside incoming-payment verification | Compute predecessor-only arithmetic |
| Fold inherited generator claims | Any sources introduced by this operation remain online | Incoming Payment changes the source set and fold challenges | Fold the **head-only** source set |
| Restructure inherited claims | Can use a cached head-only result if that is the complete inherited set | Final merged accumulator depends on incoming Payment | Complete and verify head-only restructuring |
| State, policy and map witness | Request, amount, fee, time/quota checks, successor | Exact Payment, credit identity, insertion paths, successor | Decode authenticated state, cache unchanged tree/state data |
| Preparation proof | Actual Send statement and successor | Actual Payment and Receive successor | Only predecessor/Request-independent subproofs or witness prefixes |
| Advance and receipt | Exact preparation and serialized operation | Same, including Payment digest | None |
| Final primary/secondary witness | Receipt and current operation bindings | Receipt, incoming proof checks and invalid-input selection | Static and predecessor-only components |
| Fresh witness commitments | Unknown coordinates and final fresh blinding | Same | Partial MSMs over fixed coordinates; single-use randomness |
| Outer sumcheck and masked terminal evaluations | Current commitments and transcript challenges | Same | Structural tables only |
| Inner sumcheck, matrix evaluation, batched linear relation | Earlier challenges and terminal claims | Same | Structural constants, not challenge-dependent answers |
| Own opening IPA | Final combined vector and transcript | Same | Bases and fresh random tape; not witness-dependent L/R messages |
| Native verification | Both own and inherited claims | Incoming pair **and** outgoing pair | Only exact previously accepted predecessor checks |
| Completion and release | Complete verified output | Complete verified output | None |

Three qualifications matter.

First, **a complete Receive preparation proof cannot precede the Payment**. The native Receive rule requires full Payment verification before mutation; the recovery body retains the exact Payment and reconstructs the successor. REPORT4’s “prepared Receive” must therefore mean a prefix/subproof, not the finished operation-specific preparation proof. [wallet wire:975](../../../specs/kagemusha_wallet_wire_v1.md), [wallet wire:1025](../../../specs/kagemusha_wallet_wire_v1.md)

Second, **pre-restructuring the receiver’s head does not pre-restructure its eventual merged accumulator**. The fold binds incoming claims before choosing challenges; restructuring binds the resulting accumulator digest, `u`, `beta`, and `e`. An unseen Payment changes those inputs. [claims.rs:182](../../../crates/iroha_fold/src/claims.rs), [claims.rs:435](../../../crates/iroha_fold/src/claims.rs)

Third, current claim code is fixed at **k16 on both curves**. One measured restructuring is:

\[
17.9\text{ ms}+1.495\text{ s}=1.513\text{ s}\quad\textbf{D}.
\]

Two unchanged-code restructurings are therefore approximately **3.026 s A**, assuming Pallas matches Vesta. REPORT4’s approximately 1.82-second pair assumes a generalized k16/k13 implementation. Shortening current proof IPAs alone does not shorten these inherited openings. [claims.rs:44](../../../crates/iroha_fold/src/claims.rs), [restructure.rs:107](../../../crates/iroha_plonk_recursion/src/restructure.rs), [Pixel results:49](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md)

For every idle artifact, the security contract should be:

- **Exact binding:** parameter/shape identity, relation/VK, wallet, head/sequence, predecessor proof bytes, ordered source list and relevant policy originals. At payment time, authenticate these and constrain their inclusion in the new statement.
- **Freshness:** rerun current time, quota, blacklist and payment-dependent checks. A changed head or relevant context invalidates affected cached work.
- **Randomness:** private partial commitments may be retained, but each new exported proof receives its prescribed fresh, single-use coins. Partial transcripts cannot be repurposed for another operation.
- **Restart:** authenticate restored artifacts against the selected durable head; otherwise recompute. After the durable operation boundary, recover the exact selected operation/output—never turn missing cache or completion data into authorization for a replacement payment.

This moves computation without moving authorization. Existing acceptance memoization binds exact proof/context inputs, while durable session continuation requires unchanged authenticated originals and exact checkpoint prefixes. Neither is a persistent unqualified “verified” flag. [soundness:156](../../../specs/kagemusha_recursion_soundness_v1.md), [soundness:1206](../../../specs/kagemusha_recursion_soundness_v1.md), [completion/recovery:1029](../../../specs/kagemusha_wallet_wire_v1.md)

One-time setup comprises authenticated generator derivation/loading, structural compiler/VK data and optional fixed-base tables. Vesta derivation costs **0.988 s M at k13**, **7.3 s M at k16**, and **31.8 s M at k18**. Two k13 fields would be approximately **1.98 s A** if Pallas matches. Ship authenticated public parameters or cache them; fresh installation cannot silently exclude this work from its latency claim. Fixed-base tables cost **0.441 s M at k13**, where they did not help, versus **3.4 s M at k16**, where they did. [Pixel results:9](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md), [Pixel results:41](../../../output/kagemusha-100x-20261108/pixel-pasta/RESULTS.md)

**2. A priced critical-path schedule**

Distinguish three cases:

- **Unprepared operation:** parameters and an authenticated previous head exist; no idle artifact is credited.
- **Idle-prepared operation:** authenticated head-dependent work has completed.
- **Process-cold operation:** add artifact loading, restoration authentication and any necessary head re-verification. Those costs are unmeasured.

The following ledger uses **8,192-length own and inherited IPAs on both fields**, sequentially priced on the measured eight-thread phone. It assumes the recursive circuit census survives the shape change—a substantial unresolved condition discussed below.

| Payment-time work | Send, seconds | Receive, seconds | Basis |
|---|---:|---:|---|
| Parsing/witness construction | 0.30 | 0.50 | A, REPORT4 allocation |
| Fresh bounded commitments | 0.75 | 1.12 | A, proposed census priced from kernels |
| Full-width helper commitments | 0.34 | 0.51 | A, retained REPORT4 allowance |
| Sumchecks, matrix and recursive arithmetic | 1.00 | 1.80 | A; not all represented by the 0.28-second kernel |
| Two new ZK own IPAs | 0.54 | 0.54 | D/A below |
| Two generalized k13 inherited restructurings | 0.61 | 0.61 | A, extrapolated from opening kernels |
| Complete native verification | 0.18 | 0.36 | D/A; Receive includes incoming and outgoing |
| Advance/receipt and durable completion | 0.20 | 0.20 | A |
| Complete preparation proof | \(P_S\) | \(P_R\) | Unmeasured |
| Additional ZK/grid/general-vector overhead | \(Z_S\) | \(Z_R\) | Unmeasured |
| **Conditional total** | **3.92 + \(P_S+Z_S\)** | **5.64 + \(P_R+Z_R\)** | D |

The sequence remains: incoming checks and preparation; Advance; remaining receipt-dependent pair construction; full outgoing verification; durable completion. The table books **no unmeasured parallel overlap**.

The scaling assumptions are explicit:

1. **Own IPA:** k13 hiding opening is **0.304 s M**. Removing its full-length masking commitment subtracts approximately **0.036 s D**, giving roughly **0.268 s A**, or **0.54 s A for two**. The generic combined public vector and Schnorr final still need measurement. Current code exploits a geometric evaluation vector; the proposed vector is more general. [prover.rs:127](../../../crates/iroha_plonk/src/pcs/ipa/prover.rs), [prover.rs:154](../../../crates/iroha_plonk/src/pcs/ipa/prover.rs)
2. **Restructuring:** approximately 0.30 seconds per k13 opening is an assumption about new generalized code, not a measurement of current restructuring.
3. **Native decisions:** the historical k16 decision measurement is 4.15 G instructions. Using REPORT4’s 15.5 G instructions/s and linear dimension scaling gives \(4(4.15/15.5)/8≈0.134\) seconds for four k13 decisions, plus approximately 0.05 seconds for remaining checks: approximately **0.18 s A per pair**. [decision measurement:286](../../../output/kagemusha-nova-folding-20261106/RESULTS.md)
4. Pallas/Vesta equality, unchanged witness census and unchanged remaining arithmetic are assumptions.

Using REPORT4’s preparation allocations, \(P_S=0.4\), \(P_R=0.6\) seconds, produces **4.32 + \(Z_S\)** and **6.24 + \(Z_R\)** seconds. These are conditional allocations, not demonstrated bounds.

For idle preparation:

- Head restructuring: approximately **0.61 s A**.
- Native validation of its newly produced claims: approximately **0.08 s A**.
- Fixed predecessor/state witness preparation: approximately **0.20–0.25 s A**.

Thus a useful background head-preparation job is approximately **0.9 s A**, plus any worthwhile partial commitments.

A Send whose inherited set is entirely head-derived can remove approximately **0.81 s A** from the ledger: **3.11 + residual preparation + \(Z_S\)**. Receive can remove the fixed witness portion, approximately **0.25 s A**, but retains its final payment-dependent reduction: **5.39 + residual preparation + \(Z_R\)**.

These timings start when the complete required peer message is available; camera acquisition and user confirmation are not priced.

**3. Shapes and the actual byte constraints**

The supplied ZK baseline is approximately **4,544 B A** for Send. Keeping its header and inherited-claim allowances unchanged gives the calibrated estimate

\[
B_S=4544+32(R_p+R_s-16)+64(k_p+k_s-29).
\]

For Receive, use doubled assignment counts and approximately **96 B A** more committed-round messages. Exact serialization must resolve the remaining baseline details.

A reserved 1,024-column tail changes capacity to

\[
R_i\ge \left\lceil\frac{N_i}{2^{k_i}-1024}\right\rceil,
\]

not \(N_i/2^{k_i}\). REPORT4 assumes Send assignments \(N_p=2^{19},N_s=2^{16}\), with both doubled for Receive. [REPORT4.md:87](REPORT4.md)

All bytes below are **D conditional on that census and the supplied ZK baseline**.

| IPA lengths, primary / secondary | Send rows | Send pair | Receive rows | Receive pair |
|---|---:|---:|---:|---:|
| 65,536 / 8,192 | 9 / 10 | 4,640 B | 17 / 19 | 5,280 B |
| 32,768 / 8,192 | 17 / 10 | 4,832 B | 34 / 19 | 5,760 B |
| 16,384 / 8,192 | 35 / 10 | 5,344 B | 69 / 19 | 6,816 B |
| **8,192 / 8,192** | **74 / 10** | **6,528 B** | **147 / 19** | **9,248 B** |
| 8,192 / 4,096 | 74 / 22 | 6,848 B | 147 / 43 | 9,952 B |
| 8,192 / 2,048 | 74 / 64 | 8,128 B | 147 / 128 | 12,608 B |
| 4,096 / 8,192 | 171 / 10 | 9,568 B | 342 / 19 | 15,424 B |

This explains why **128 × 4,096 is not automatically the winner**. With the tail it holds only 393,216 witness coordinates; REPORT4’s proposed fresh primary census alone is 428,000.

The table also requires **implicit, authenticated zero rows** when padding the row-index domain. Transmitting every power-of-two padded row commitment makes the 8,192/8,192 family approximately **8,448 B Send / 13,152 B Receive D**, failing Receive. The reserved-column layout must preserve succinct uniform matrix evaluation.

There is a promising alternative: **two-variable groups and a 256-column tail**.

Under assumed outer/inner dimensions Send 20/19 and 16/16, Receive 21/20 and 17/17:

- Grid-slot upper bounds are **180/144 Send and 189/162 Receive D**, fitting 256.
- Usable row capacity becomes **7,936 D**.
- Send requires **67/9 rows D**; Receive **133/17 D**.
- Additional round commitments cost **576 B D per pair**.
- Result: **6,848 B Send / 9,312 B Receive D**.

Naive grid-evaluation counts fall from

\[
81\sum_{j\ge1}N/16^j=5.4N
\quad\text{to}\quad
9\sum_{j\ge1}N/4^j=3N.
\]

That is **44% fewer grid evaluations D**, not a measured runtime reduction; additional transcript/group-verification work must be charged.

For **Send alone**, the fastest dimension-based candidate is then:

- Primary **67 × 8,192**.
- Secondary **37 × 2,048**.
- Approximately **7,616 B D**.
- Own IPA pair approximately **0.35–0.42 s A**, versus approximately 0.54 seconds for two k13 IPAs.

Receive can retain the 8,192/8,192 family. Send-after-Receive must still support inherited secondary k13 claims; its shorter current secondary IPA does not shrink the predecessor obligation.

**However, none of these is yet an established optimum.** Two census problems must be resolved first.

1. REPORT4’s fresh Send census totals **492,000 coordinates D**, and its helper allowance adds **131,072 D**, totaling **623,072 D**—already above the nominal combined assignment capacity of 589,824. Show that helpers are included/reuse slots, or enlarge the geometry. [REPORT4.md:124](REPORT4.md), [REPORT4.md:159](REPORT4.md)
2. More row commitments enlarge recursive group verification. A Send following the proposed Receive sees **133 instead of 16 predecessor primary row points: 117 extra terms D**. Applying CycleFold’s published approximately 1,000–1,500 gates per standalone scalar multiplication gives an illustrative **117k–175.5k extra gates D**, already exceeding the assumed Send secondary size. This is a screening estimate, not a lower bound on an optimized batched MSM; constraints also need not equal assignment coordinates. [CycleFold](https://eprint.iacr.org/2023/1192)

If that growth makes Send secondary assignments \(2^{18}\), the proposed length-2,048 shape grows to approximately **11,136 B D**, failing. The compiler must solve the full cycle:

`row counts → recursive MSM workload → secondary size → primary verifier workload → row counts`.

Several shorter IPAs generally lose to rectangular Hyrax. For equal total length \(L\), \(t\) independent openings cost

\[
t[64\log_2(L/t)+96]
\]

in IPA messages, versus \(64\log_2L+96\), before extra commitments, evaluations and generator claims.

| Split of one k16 opening | Illustrative added bytes, D | Likely CPU result |
|---|---:|---|
| 2 × k15 | ≈1,088 B | Similar linear work, more overhead |
| 4 × k14 | ≈3,136 B | More overhead and recursive obligations |
| 8 × k13 | ≈6,976 B | Exceeds Send budget from the supplied baseline |

When the openings share a column evaluation vector, random row combination already permits **one shorter IPA**. Independent openings do not provide another eight-core CPU.

Pay-per-bit commitments preserve proof size unless they change the layout. Scaling the measured \(2^{18}\) kernels to \(2^{19}\) values gives:

| Public width | Commitment time, D |
|---|---:|
| Full field | 1.110 s |
| 16 bits | 0.152 s |
| 8 bits | 0.097 s |
| 1 bit | 0.061 s |

These exclude row/blinding overhead. Apply them to fresh bounded coordinates, not randomly combined opening vectors or full-width helpers. The API requires independently public width bounds and checks every scalar; it does not replace circuit range constraints. [msm/mod.rs:326](../../../crates/iroha_pasta/src/msm/mod.rs)

Shorter columns do not make the original commitments proportionally cheaper: 8 × 65,536 full-width commitments cost **1.078 s M**; four measured batches of 16 × 8,192 give **1.164 s D**. The principal saving is in openings and generator decisions.

**4. Parallelism and mobile GPUs**

The current column scheduler already distributes admitted single-task MSMs dynamically under one shared scratch budget. Do not run two eight-thread proof pools and assume additive throughput. [msm/mod.rs:202](../../../crates/iroha_pasta/src/msm/mod.rs)

The CPU experiments worth running are:

- One shared pool with 2, 4, 6 and 8 workers; compare X1-only, four big cores and all cores.
- Dynamic row dispatch rather than equal static partitions that leave A55 stragglers.
- Concurrent L/R MSMs **within a round**, using one bounded resource pool. Current code computes them sequentially; the next round still waits for their transcript challenge and generator fold. [prover.rs:169](../../../crates/iroha_plonk/src/pcs/ipa/prover.rs)
- Primary/secondary pipelining after their actual inputs become available. Absence of circular current-proof hashing permits overlap; it does not prove the two workloads are independent at every phase.

Book **zero additional scheduling gain initially A**. Retain complexity only for a measured **≥10% whole-stage improvement A**.

Published mobile evidence is useful but does not establish Pixel performance:

| Published workload | CPU → accelerated, M-publication | Gain, D |
|---|---:|---:|
| iPhone 15 Pro, BN256 MSM \(2^{16}\) | 85 → 98 ms | Slower |
| Same, \(2^{18}\) | 302 → 200 ms | 1.51× |
| Same, \(2^{20}\) | 1,405 → 787 ms | 1.79× |
| Same, \(2^{21}\) | 3,649 → 1,619 ms | 2.25× |
| M3 MacBook Air, BN254 \(2^{18}\), Arkworks → ICICLE Metal | 245 → 149 ms | 1.64× |
| Same, \(2^{20}\) | 942 → 421 ms | 2.24× |

These are developer-published measurements on different curves and hardware. [Apple implementation report](https://hackmd.io/@guard/BJDmuEODke), [Mopro benchmark repository](https://github.com/zkmopro/gpu-acceleration)

I found no matching published **Mali-G78 or Adreno Pasta MSM/IPA measurements** supporting a numerical projection. The most plausible GPU targets are large resident commitment batches and early IPA rounds; short k11–k13 openings increase the importance of launch, conversion and synchronization costs.

Even **1.5× A** acceleration of an eligible **0.60-second A** stage saves only **0.20 seconds D**. GPU work is therefore a later optimization, not the currently evidenced route from 8–11 seconds to three.

A port must preserve exact Pasta arithmetic, exceptional-point handling, deterministic outputs and native verification. Secret-buffer handling and side-channel behavior need explicit review: the existing “secret” MSM itself has digit-dependent buckets/skips and is **not constant-time**. [pippenger.rs:24](../../../crates/iroha_pasta/src/msm/pippenger.rs)

**5. Root-cause cuts worth pursuing**

The highest-value unresolved cut is **sharing the inherited opening with the own IPA**.

For compatible vectors and a shared final point, bind the inherited commitment \(C_*=\langle s,g\rangle\), then prove a joint sumcheck and open

\[
D=\sum_i\lambda_i C_i+\beta C_*,
\qquad
d=\sum_i\lambda_i w_i+\beta s.
\]

One IPA can discharge the combined obligation if all source claims and evaluations are fixed before the batching challenges. The new relation must constrain the exact predecessor sources, relaxed-error equations and invalid-input selection. Native generator decisions remain mandatory.

There is a concrete obstacle in the proposed ZK construction: its opening vector contains **grid-check coefficients in the reserved tail**, whereas inherited tensor \(s\) occupies those coordinates too. Merely sharing the evaluation point opens \(\langle s,b\rangle\), not \(\widetilde s(r)\).

A possible repair proves

\[
c_{\rm tail}
=\sum_{j\in{\rm tail}}s_j\,[b_j-\operatorname{eq}(r,j)]
\]

from the same authenticated source tensors. For a bounded source set, that adds approximately **tail length × source count scalar work D**, potentially much less than another IPA. A 256-column tail helps this experiment. An arbitrary historical relaxed vector cannot be substituted without proving its tail linkage.

Potential saving: approximately **0.61 s A per operation** at generalized k13, or approximately **1.8 s A** at REPORT4’s assumed k16/k13 lengths, less the added joint-relation cost. **No saving should be booked before that construction is proved and measured.**

Simply retaining the relaxed accumulator does not solve this: current native decision needs its entire witness vector, and sums of tensor vectors are generally not tensors. Publishing the k16 vector costs approximately **2 MiB D**. Lists of structured claims grow through lineage and eventually require compression. [claims.rs:331](../../../crates/iroha_fold/src/claims.rs), [restructure.rs:3](../../../crates/iroha_plonk_recursion/src/restructure.rs)

Other cuts are smaller or compiler-dependent:

| Cut | Saving estimate | Security condition |
|---|---|---|
| Eq factoring / Gruen-style arithmetic | Applying reported 10–25% improvements to 0.28 s suggests roughly 0.03–0.07 s D | Evaluate exactly the same field identity; preserve challenge and degree accounting |
| Small-value sumcheck | Published 2–3× on Jolt’s first Spartan sumcheck would imply 0.14–0.19 s D saved on this kernel, **if applicable** | Public bounds, proven integer overflow bounds, exact reductions; random folds cease to be small |
| Two-variable committed rounds | 44% fewer naive grid evaluations D | Same committed degree constraints and batching order; charge extra recursive transcript work |
| Remove unnecessary copy/lookup helper commitments through the uniform compiler | Up to REPORT4’s 0.34/0.51 s A line | Prove equivalent wiring/range constraints; recount any additional witness coordinates |
| Replace the full masking MSM with the proposed Schnorr-final IPA | Approximately 0.07 s D per two-k13 pair | Full ZK and knowledge-soundness argument; remaining IPA work stays |
| Reuse predecessor-only witness/partial commitments | Actual reusable portion of the commitment/witness ledger | Exact layout/head binding, fresh exported blinding, stale-cache rejection |
| Reuse exact accepted verification | Avoid duplicate checks within the allowed owner/cache | Never replace recursive constraints or final native decisions with a local verdict |

The sumcheck publications support useful kernel improvements, not multiplying all of REPORT4’s 1–1.8-second arithmetic allocation by three. [Dao–Thaler](https://eprint.iacr.org/2024/1210), [Bagad–Dao–Domb–Thaler](https://eprint.iacr.org/2025/1117)

Also, the measured **0.28 s M** implementation uses one-variable cubic rounds with four evaluations. It does not measure grouped committed-grid generation. Naive four-variable quadratic groups have approximately **1.35× D** as many evaluation sites as that cubic benchmark; approximately **0.38 s A** is a more cautious initial kernel allocation before optimizations. [benchmark implementation:244](../../../output/kagemusha-100x-20261108/pixel-pasta/src/main.rs)

The proposed ZK construction itself needs these gates:

- `{0,1,2}^4` is valid only after enforcing degree at most two per variable. Without equality factoring, cubic grids need **256 values per group D**, exceeding the proposed tail.
- Both terminal masking gates must be isolated valid multiplication gates. Two gates plausibly give statistical masking of \(v_A,v_B,v_C\), but require a simulator and a bound covering degenerate evaluation weights and adaptive Fiat–Shamir transcripts.
- Every batched claim must be fixed before its batching challenge.
- Blinded L/R and a Schnorr final require their own complete argument. Calling the construction “Bulletproofs-style” does not establish ZK. [Bulletproofs](https://eprint.iacr.org/2017/1066)

Removing the duplicate payer-receipt check is already in REPORT4’s selected architecture; do not claim its saving again.

**6. Cheapest-kill-first spike plan**

All thresholds below are proposed engineering gates **A**, not achieved measurements.

1. **Resolve algebra and recursive sizing before phone optimization.**
   Serialize the complete ZK pair and synthesize Send-after-Receive and Receive-after-Receive, including incoming Send, all row-point equations, grid commitments, inherited obligations and invalid-input paths. Require Send ≤8,458 B, Receive ≤9,600 B and complete Payment ≤12,000 B. Reject any candidate requiring an omitted auxiliary proof, unproved matrix evaluation or unbounded claim list. Establish the masking/IPA argument independently of tests.

2. **Benchmark the actual generic-vector opening on both curves.**
   Compare k11/k13/k14, including all blinding and Schnorr work. Gate two k13 own openings at **≤0.60 s**; one complete native pair at **≤0.20 s**. If these fail, revise the ledger before wallet integration.

3. **Benchmark actual committed-grid generation and recursive witness construction.**
   Compare four-variable/1,024-tail against two-variable/256-tail. Include commitments, transcript work and the larger recursive verifier. The 0.28-second uncommitted kernel is only a control.

4. **Attempt shared inherited/own opening with the tail correction.**
   Measure total added arithmetic, bytes and recursive witness cost. Retain it only if it removes at least **0.40 s** from the complete k13 operation. Otherwise keep the separate restructuring cost in the three-second calculation.

5. **Use explicit phase caps for the three-second attempt.**

   | Complete phase, including privacy | Send cap | Receive cap |
   |---|---:|---:|
   | Input/state witness work | 0.20 s | 0.25 s |
   | Complete preparation proof | 0.25 s | 0.30 s |
   | All fresh/helper commitments | 0.65 s | 0.80 s |
   | All sumcheck/matrix/recursive work, including fused inheritance | 0.50 s | 0.50 s |
   | Own IPAs | 0.55 s | 0.55 s |
   | All native verification | 0.20 s | 0.35 s |
   | Advance, encoding, durable completion | 0.20 s | 0.20 s |
   | **Sum** | **2.55 s** | **2.95 s** |

   Receive’s caps demand approximately halving the current commitment/helper allocation and reducing the 1.8-second arithmetic allocation substantially. That is the remaining root-cause work; affinity tuning alone cannot satisfy it. Separate inherited IPAs would add approximately **0.61 s A**, failing this Receive budget.

6. **Qualify the complete boundary.**
   Require **≤3.0 s p95** from complete input availability through durable releasable output, with both unprepared and process-cold cases reported separately. Measure thermal-steady repetitions, peak memory ≤896 MiB, and no hidden idle work in the unprepared result. Exercise changed Payment with fixed idle cache, changed head/policy, cancellation, every durable restart boundary, and exact retry.

   Include a forged succinctly valid generator point and require rejection before release: the repository explicitly demonstrates why succinct acceptance alone is insufficient. [plonk_ipa_v1.md:676](../../../specs/plonk_ipa_v1.md)

My implementation priority would be **recursive census → two-variable/short-tail serialization → generic k13 IPA → shared inherited opening**. GPU experiments come after those gates. The first two can invalidate the attractive shape without spending time optimizing a prover that cannot fit its own recursive successor.
