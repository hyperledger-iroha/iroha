**I would replace the twelve-proof Pasta pipeline with a direct, zero-knowledge small-field recursive prover, and change Payment to carry the sender’s completed Send proof.** That removes the Q/A/W staging, the separate transported sigma, duplicate signature verification, and the four-proof compression tail.

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

This is a credible route toward **3 seconds**, but it is not an established result. The evidence does **not** support promising all three simultaneously today:

- 3-second Pixel 6 completion;
- complete, private recursive lineage before payout;
- ≤5,120-byte Omega.

My proposed implementation targets the first two with **64–128 KiB Omega as an initial engineering budget**. Keeping the existing transport requires a separately demonstrated compact finalizer. I would test that boundary first, before funding another folding implementation.

I treated your measurements as supplied evidence, inspected the checkout and its measurement artifacts, and researched primary sources available through November 8, 2026. I made no repository changes or new performance measurements.

**The 300 seconds mostly pays for the proof architecture.**

| Current Receive work | Proofs | Approximate cost at 23 seconds/proof |
|---|---:|---:|
| Incoming sigma verifier | 1 | 23 s |
| Three-signature leaf | 1 | 23 s |
| Business/verification stages A1–A5 | 5 | 115 s |
| Recursion joins W0–W3 | 4 | 92 s |
| Final Omega | 1 | 23 s |
| Total, before miscellaneous overhead | **12** | **276 s** |

The [compiled schedule](../../../crates/iroha_kagemusha_proof/src/a_relation/schedule/compiled.rs) and [construction specification](../../../specs/kagemusha_lambda_omega_v1.md) confirm this structure. These are not twelve independently necessary cryptographic statements. Several exist because a k16 A circuit over one Pasta field cannot fit everything or directly verify another A proof natively.

At roughly 1,000 length-65,536 FFTs per proof, the pipeline performs approximately:

\[
12 \times 1000 \times \frac{65536}{2}\times16
\approx 6.3\text{ billion FFT butterflies}.
\]

That is an architectural expense, not a lower bound for offline payments.

Your profile also rules out an engine-only solution. With representative shares of 40% FFT, 20% MSM and 6% IPA, making **all three free** would still leave roughly 100 seconds. The current implementation already contains specialized CIOS arithmetic, optimized FFT scheduling and shared-expression evaluation; “add NEON” or “use Pippenger” is not an unexplored 100× lever. See [field arithmetic](../../../crates/iroha_pasta/src/field/cios.rs), [FFT implementation](../../../crates/iroha_pasta/src/fft.rs), and [quotient evaluation](../../../crates/iroha_plonk/src/prover/quotient.rs).

The folding experiment has an even clearer floor:

\[
27.8+37.2+51.7+51.4=\boxed{168.1\text{ seconds}}
\]

for S→R→P→Omega alone. Free folding and free native deciders would not remove that floor. These four wraps explicitly bridge scalar-field and curve-group verification across Pasta’s cycle. [Their source contract explains each crossing.](../../../crates/iroha_kagemusha_proof/src/fold_wrap/mod.rs)

Exporting the accumulator instead is not a solution: [`iroha_fold` says it does not itself produce a succinct zero-knowledge argument](../../../crates/iroha_fold/src/lib.rs), and the [v2 deciders are explicitly not zero knowledge](../../../crates/iroha_fold/src/decider_v2/mod.rs).

**The useful statement is much smaller than its current proving workload.**

There is no known universal “minimum number of constraints” for this statement. Counts depend on the field, custom gates, lookup argument, commitment scheme and what is made public. The useful distinction is between measured implementation counts, a reconstructed semantic workload, and targets for a new compilation.

| Receive component | Source-backed workload |
|---|---|
| Three P-256 verifications | Approximately **347,361 assigned advice cells**, excluding SHA/message conversion |
| Three SHA-256 blocks and message conversions | Approximately **70,263 additional cells** |
| Complete Q1 leaf, including bindings | **421,908 assigned cells**, 43,237 maximum assigned row |
| Consumed-credit insertion and credit-digest insertion | Approximately **530–534 Poseidon permutations** |
| Separate nonmembership check | Approximately **66–68 permutations**; potentially shared with insertion |
| Predecessor/successor state openings | Approximately **40 permutations** |
| Credit ID, receive-chain append, statement digest | Approximately **30 permutations** |
| Other object hashes, proof-byte hashes, byte binding and arithmetic | Hundreds of permutations, byte checks, and bounded integer operations |
| Incoming sigma verifier Q0 | **301,653 assigned cells** |
| Entire measured eleven-stage chain, excluding final Omega | **7,489,708 assigned cells** |

The signature numbers are documented in the [P-256 gadget](../../../crates/iroha_plonk_gadgets/src/p256/mod.rs). The genuine stage census is in the [November 6 prototype measurements](../../../output/kagemusha-nova-folding-20261106/RESULTS.md). The 66-column figure describes the later signature-plus-fold-frame relation; the underlying signature circuit has 17 advice columns.

A reasonable reconstruction of ordinary Receive’s useful application statement is:

- three P-256 verifications;
- three SHA-256 compression blocks;
- approximately **900–1,200 current Poseidon permutations**;
- two indexed-tree updates and their ordering/nonmembership checks;
- bounded balance, sequence, policy and identifier checks;
- exact byte and statement binding.

That is approximately **0.6–0.75 million useful active cells in today’s gadgets**, excluding recursive verification and staging overhead. It is an engineering reconstruction, not a measured monolithic circuit. The local folding study independently brackets a stripped construction, still including incoming proof verification and folding machinery, at **2.2–3.0 million large-field cells**. [Derivation.](../../../output/kagemusha-nova-folding-20261106/RESULTS.md)

The maps are also concrete. An indexed-tree insertion performs approximately four depth-32 traversals, plus leaf/value hashing. Two inserts and separate nonmembership therefore represent about **nine logical depth-32 traversals**, not an enormous general database computation. [IMT implementation.](../../../crates/iroha_plonk_gadgets/src/imt.rs)

For P-256, source inspection gives approximately **1.25–1.45 thousand modular-multiplication-equivalent blocks per variable-key verification**, including its point-operation chains but before all range/canonicality overhead. The fixed-key path is roughly six times smaller. The current implementation already uses windowing and carefully constrained incomplete arithmetic; those optimizations cannot be counted again. [Scalar multiplication](../../../crates/iroha_plonk_gadgets/src/p256/mod.rs), [point operations](../../../crates/iroha_plonk_gadgets/src/p256/point.rs).

A safe initial small-field implementation could use **22 limbs of 12 bits**. Schoolbook multiplication then has 484 limb products, giving approximately **0.60–0.70 million small-field pair-products per variable signature**, before carries, range checks and lookups. Three signatures mean roughly 1.8–2.1 million pair-products. Naively using 16-bit limbs in a 31-bit field is unsound: even one product exceeds the field, and convolution sums are larger still.

Thus several million 32-bit witness cells are plausible. An **8-million-cell total including recursion is a target to test**, not a fact established by the current code.

For the Pixel 6, I would optimize for its four faster CPU cores, with NEON and bounded helper work on the efficiency cores. Its eight cores are not eight equivalent proving workers. The device has 8 GB RAM, but compatibility with 4 GB phones requires a much smaller application footprint—approximately **0.8–1.0 GiB peak**, not “under 4 GB.” [Google’s Pixel 6 specifications.](https://storage.googleapis.com/pixel-goog-prod-files/downloads/pixel-6-brochure-en_GB.pdf)

**Published results show that primitive proving can be fast; they do not establish a small recursive payment proof.**

| System or technique | Relevant evidence | Consequence for KAGEMUSHA |
|---|---|---|
| **Plonky3: BabyBear/KoalaBear/M31, Poseidon2** | Native small-field arithmetic, SIMD implementations, lookup and recursive-verifier building blocks. Current recursion supports FRI and experimental WHIR paths. | Strong implementation foundation. Its defaults are not automatically your security profile: current recursion documentation lists different per-error targets, and its ZK switch currently requires FRI. [Repository.](https://github.com/Plonky3/Plonky3-recursion) |
| **Stwo / Circle STARK** | StarkWare reported **over 600,000 Poseidon2 hashes proved per second on a 12-core M3 Pro**, at stated 128-bit parameters. | Strong evidence that application hashing is not intrinsically expensive. This is a large hash-throughput workload, not a phone measurement or recursive Receive. [Benchmark.](https://starkware.co/blog/starkware-new-proving-record/) |
| **WHIR** | Published example: degree \(2^{22}\), **1.2 s commit-and-open, 63 KiB, 360 μs verification**, at **100-bit** security. | Excellent direction for cheap recursive verification. It is a PCS result, not a 128-bit ZK application proof, and already exceeds Omega’s cap. [Paper.](https://eprint.iacr.org/2024/1586) |
| **FRI / BaseFold / DeepFold** | FRI supplies low-degree testing; BaseFold supports field-agnostic multilinear commitments. DeepFold reports approximately **3× smaller proofs than BaseFold** with similar prover time. | Useful alternatives for the commitment layer. Proof size, hiding and recursive verifier cost must be measured together. [DeepFold.](https://www.usenix.org/conference/usenixsecurity25/presentation/guo-yanpei) |
| **Blaze** | Commitment dominated by **8n additions plus a Merkle tree**; evaluation proof by **6n additions and 5n multiplications** over its binary field. | Removes expensive group commitments, but does not make the complete private recursive proof 5 KB. [Paper.](https://eprint.iacr.org/2024/1609.pdf) |
| **Ligerito** | \(2^{24}\) coefficients over a 32-bit binary field: **1.3 s on an M1 MacBook Pro, 255 KiB proof**. | Excellent commitment throughput; poor match to the current carrier. Again, this measures a PCS. [Authors’ paper.](https://angeris.github.io/papers/ligerito.pdf) |
| **Binius64 / binary towers** | June 2026 CSP result for SHA-256 over 2,048 bytes: **67.3 ms**, about **260 MB peak**, **424,720-byte proof**, on M1; configured at **96-bit security**. | Bitwise arithmetic can be extremely cheap. This result cannot be transplanted as equal-security recursion or compact transport. [Raw benchmark.](https://github.com/ethereum/csp-benchmarks/blob/main/results/collected_benchmarks_28226856406.json) |
| **LogUp-GKR / Expander** | Expander reported **4,500 Keccak permutations proved per second on M3 Max**. | Useful algorithms for lookup multiplicities and structured arithmetic. The current Ethereum benchmark repository disables Expander pending upstream soundness issues; its throughput is not deployment qualification. [Benchmark](https://blog.polyhedra.network/introducing-expander-the-fastest-gkr-proof-system-to-date/), [current status](https://github.com/ethereum/csp-benchmarks). |
| **Jolt-style lookups** | Lookup-centric arithmetization avoids expensive instruction-by-instruction arithmetic emulation; commitment choice controls much of the resulting cost. | Borrow the lookup techniques for a fixed payment circuit. A general zkVM adds work this application does not need. [Jolt paper.](https://eprint.iacr.org/2023/1217.pdf) |
| **LatticeFold+** | Reports a **5–10×** improvement over LatticeFold; candidate folding-proof sizes around **95 KB**, with concrete implementation left as future work. | Serious research, but neither a measured mobile prover nor a complete compact Omega. [Paper](https://eprint.iacr.org/2025/247), [full-text mirror](https://core.ac.uk/download/pdf/646079161.pdf). |
| **Neo / SuperNeo** | The 2026 work supports small-field extension sumcheck and pay-per-bit Ajtai commitments; SuperNeo removes Neo’s SIMD restriction. | Promising for narrow witnesses, with Module-SIS assumptions and substantial implementation work. No demonstrated three-second phone finalization found. [Paper.](https://eprint.iacr.org/2026/242) |
| **Arc / WARP** | Hash-based accumulation with unbounded depth; WARP has linear prover and logarithmic update-verifier complexity. | Accumulation still requires a terminal decision. A compact private public proof of that decision must finish before each payout. [Arc](https://eprint.iacr.org/2024/1731), [WARP](https://eprint.iacr.org/2025/753). |

The distinction between native verification and recursive verification is decisive. A verifier taking 360 μs natively can still require substantial work when every hash, parser operation and extension-field equation must be proved.

P-256-specific evidence is particularly encouraging:

| Result | Numbers | What it actually proves |
|---|---|---|
| **Longfellow, original mobile measurements** | Pixel 6 Pro: **80.5 / 120 / 152 ms** for one/two/three ECDSA signature proofs | Native-P-256-field sumcheck/Ligero, excluding the complete KAGEMUSHA statement and recursion. [Paper, Table 10.](https://eprint.iacr.org/2024/2010.pdf) |
| **Longfellow, updated primitive benchmarks** | M4 single-thread: **16.7 ms** for one signature, **38.3 ms** for three; **5.3 ms** for one SHA block | Demonstrates the primitive’s potential, not its cost inside a KoalaBear circuit. [Official benchmarks.](https://longfellow-zk.github.io/longfellow-zk/docs/benchmarks/) |
| **UltraHonk P-256 on Android, November 2026** | Pixel 8a **1.1 s / 274 MiB**; Galaxy A06 **3.3 s / 250 MiB** | Real application measurements for a signature circuit; no recursive lineage. [Ethereum Foundation measurements.](https://publicsector.ethereum.foundation/articles/mobile-zk-benchmarks/) |
| **Native P-256 device-binding proof** | **1.47 KB**, **208 ms on M3**; PLONK variant **4.28 KB / 696 ms** | Fresh public-challenge proof of possession, with additional curve assumptions. Not an arbitrary hidden historical-signature proof. [2026 paper.](https://brave.com/research/files/DeviceBinding.pdf) |
| **Mopro** | Pixel 6 Noir examples around **1.1–1.3 s**; Pixel 6 Pro Halo2 Keccak **23.9 s** | Native mobile integration helps, but circuit and backend selection dominate. Different example workloads are not interchangeable speed comparisons. [Benchmarks.](https://zkmopro.org/docs/performance/) |

Longfellow’s signature proofs are roughly **190–204 KB** in the reviewed implementation. Its current repository also states conditional 116/117-bit bounds and an open full V2 joint-view ZK obligation. Those are material differences from an assumed 128-bit, fully private replacement. [Review](https://google.github.io/longfellow-zk/reviews/Longfellow_report_2025_08_18.pdf), [current security statement](https://github.com/longfellow-zk/longfellow-zk).

I found no reproducible primary Pixel 6 P-256-only benchmark for ZKPassport/OpenPassport that should override these numbers. Passport presentation timings include different signature schemes, document hashing, parsing and sometimes preprocessing.

**The concrete design I would build uses one completed Send proof as the payment certificate.**

The main protocol change is to transport **Omega after Send**, rather than predecessor Omega plus sigma_send as separate proofs.

A terminal Send proof establishes:

1. The predecessor lineage is valid.
2. The complete Send transition is valid: balance, fees, controls, ordinals, receiver/Request binding and map effects.
3. The hardware receipt authorizes that exact transition.
4. The exposed payment claim is the precise Send effect the receiver consumes.

Receive then verifies two recursive proofs: its own predecessor and the incoming terminal Send proof. The incoming proof already proves the payer’s receipt signature and Send relation. Receive does not separately prove those facts again.

Ordinary Receive consequently falls from **three P-256 checks to two**: its Request signature and its own receipt. Send retains one. The payer’s hardware signature remains mandatory and is proved inside terminal Send.

This is a change to the induction boundary, not an assumption that native verification happened elsewhere.

I would **retain the local pre-Advance preparation proof**. The current provider contract requires it before irreversible head selection. The new flow is:

```text
prove and verify local preparation
    → serialized Advance and hardware receipt
    → prove complete terminal lineage
    → self-verify, durably freeze exact output, release value
```

The preparation proof uses the new fast backend and stays local. Terminal lineage inlines its transition relation, as today’s lean lineage already does; it need not recursively verify another proof of the same transition.

There is no circular signature over the final proof. The receipt can retain its existing binding to predecessor Omega and the local preparation proof. Terminal Omega is generated afterwards. The new Payment digest must **directly hash the complete canonical terminal proof and all Payment fields**, because the old transitive hash path through predecessor Omega and sigma no longer authenticates the transported bytes. [Current receipt ordering and recovery contract](../../../specs/kagemusha_single_design_proposal.md), [current Payment binding](../../../specs/kagemusha_single_design_proposal.md).

Stable `credit_id` continues to enforce one credit. Proof randomness must never create another credit. Honest retries return the frozen original bytes; a different proof for an already consumed credit remains conflicting evidence. ArchiveSent must consume the same exact-byte credit evidence.

**For the prover, I would use KoalaBear arithmetic, structured sumcheck and a hiding hash-based PCS.**

The implementation would have these properties:

- A fixed application circuit family, with specialized P-256, SHA, integer, Poseidon2 and recursive-verifier components.
- LogUp/GKR-style lookup checking, with shared range/bitwise tables.
- Multilinear sumcheck instead of the current high-degree PLONK quotient machinery.
- A hiding WHIR-based commitment/opening construction as the preferred candidate; a ZK FRI implementation as the comparison baseline.
- The same small-field hash and arithmetic family for application work and recursive verification.
- A complete proof at every operation boundary. No exported undecided accumulator and no later compression stage.

This is **new integration work**, not a claim that current Plonky3 exposes this exact combination as a qualified API. In particular, current Plonky3 recursion’s WHIR path does not simply inherit its FRI ZK support. The hiding construction, recursive verifier and application must be composed and reviewed together. [Current recursion limitations](https://github.com/Plonky3/Plonky3-recursion), [existing full-ZK Spartan-WHIR implementation](https://github.com/ethereum/spartan-whir).

For security parameters, I would start conservatively with an **octic challenge extension**, then reduce only if the composed error analysis permits it. A roughly 124-bit quartic field does not by itself provide 128-bit soundness after polynomial-degree and composition losses. The analysis must account for lookups, proximity testing, Fiat–Shamir, recursion and the supported lifetime.

Similarly, eight 31-bit hash elements have only about 248 bits of output space. I would use at least nine field elements for internal digests and sufficient sponge capacity, with independently reviewed Poseidon2 parameters. A new 36-byte signed digest still fits in **one SHA-256 compression block** and works with standard hardware SHA256withECDSA. This is an explicit first-release encoding change, not a silent reinterpretation of the current 32-byte format.

All protocol-owned hashes must move together. Keeping Pasta Poseidon inside a small-field circuit would reintroduce expensive foreign-field arithmetic. Platform attestation formats and external identifiers retain their existing algorithms.

The native verifier and in-circuit verifier must implement the same total predicate. For an incoming terminal proof:

\[
b=\operatorname{VerifyTotal}(\text{authorized relation},\text{exact bytes},\text{expected claim}).
\]

The circuit derives \(b\); it is not a witness supplied by native code. Canonical parsing, transcript replay, every opening and every final equation contribute to that result. A false result forces the specified burn branch. Malformed input uses constrained safe representations without making the entire circuit unsatisfiable.

That preserves the important distinction between **invalid incoming evidence** and **local resource/prover failure**.

**P-256 and SHA remain hardware-compatible and fully proved.**

I would initially implement P-256 directly in the small-field circuit:

- bounded 12-bit limbs with explicit carry proofs;
- specialized reduction for the P-256 base modulus;
- separately correct scalar-order reduction;
- fixed-generator tables;
- constrained variable-key window tables;
- full canonical-coordinate, curve, scalar-range, low-S and infinity checks;
- exact soft-result computation.

SHA-256 uses byte/nibble lookups and bounded word arithmetic. The design has only a few compression blocks per operation, so SHA is not the primary optimization target.

Authenticated own-key tables are a worthwhile second step. The Request and Receive receipt use the same receiver key. A predecessor/readiness proof can certify a table once and later proofs can authenticate its use. Merely caching a table accelerates witness generation; it does **not** remove table-correctness constraints unless the new proof authenticates that certification.

I would not initially introduce a separate Longfellow proof lane. Its native-field arithmetic is compelling, but recursively verifying its large proof over KoalaBear could cost more than directly proving two signatures. It deserves a benchmark, not an assumed 152 ms budget.

**Pre-proving is useful, but the current open Request does not reveal enough to pre-prove the whole Receive.**

Before any payer information arrives, the wallet can prepare:

- predecessor validity and state openings;
- policy and key continuity;
- its quote/Request signature;
- authenticated key-table facts;
- public engine preprocessing.

It cannot yet know the exact credit ID or IMT insertion path. The current Request omits payer identity and send ordinal until Payment completes them. [The provider contract states this explicitly.](../../../specs/kagemusha_single_design_proposal.md)

My optional readiness proof would bind:

\[
(\text{exact head},\ \text{Request},\ \text{policy},\ \text{authorized circuit identity}).
\]

Receive could verify this readiness proof **instead of** its ordinary predecessor proof, because readiness already proves that predecessor and the Request authorization. The incoming payer proof remains the other recursive input. That moves the Request signature and some state work earlier without adding another recursive input.

A payer-intent phase could additionally reveal the payer/ordinal early and allow map preparation. Its time must be included in the interaction; it is not “payment-independent” work.

Templates are invalidated by conflicting local commits. They do not reserve a global exclusive receive slot or freeze other spending. Single-use masks cannot be reused across different final statements.

I would report three separate numbers: prepared Receive latency, cold Receive latency, and sustained payments per minute. A three-second interaction followed by thirty seconds of hidden preparation is not a 100× total-work improvement.

**The Pixel 6 budget is aggressive and falsifiable.**

The following is my allocation for an optimized ordinary Receive with the fused terminal-Send format. These are **engineering targets**, not extrapolated measurements. The proving phases are charged separately to avoid counting commitment work twice.

| Phase | Target time | Incremental working-memory budget |
|---|---:|---:|
| Parse, native incoming checks, witness preparation | 0.20 s | 64–96 MiB |
| Local preparation proof and verification | 0.20 s | 64–128 MiB |
| P-256/SHA constraint and lookup processing | 0.45 s | 128–192 MiB |
| Maps, application hashes, bounded arithmetic | 0.15 s | 32–64 MiB |
| Two recursive-verifier computations inside the proof | 0.60 s | 192–256 MiB |
| Shared hiding commitments, remaining sumchecks and terminal opening | 0.90 s | 512–768 MiB total prover peak |
| Hardware receipt, final self-verification and durable completion | 0.25 s | Wallet working set |
| Contingency | 0.25 s | — |
| **Total** | **3.00 s** | **≤1 GiB whole-process peak** |

The critical experimental question is whether the complete recursive proof can stay around **6–10 million active base-field cells**, with sufficiently cheap extension-field work and commitments. The application arithmetic is plausibly inside that budget; the recursive verifier and hiding PCS are the uncertain parts.

Send has one predecessor proof and one signature, so I would target **1.8–2.5 seconds**, leaving margin for enabled controls. Renewal-heavy Receive has additional credential/certificate obligations: **cold three-second completion is not supported by this budget**. It needs those obligations incorporated into a completed readiness proof, or another roughly 0.5–1.5 seconds of measured optimization. Quota-enabled and renewed cases must have separate acceptance measurements.

For memory, base witness arrays remain 32-bit. Extension-field scratch must be streamed or reduced between rounds; blindly expanding the whole witness into eight-limb extension elements destroys the budget. Process one commitment phase at a time, reuse buffers, retain only necessary Merkle levels, and avoid concurrent whole-proof instances.

The memory target is for a normal Android application with other applications present. A 4 GB device must still work without swap-dependent proving. It may be slower than Pixel 6; hardware-key eligibility does not imply equal CPU performance.

The three-second gate should be measured after thermal stabilization. If the new system reproduces the current approximately 1.6× slowdown, a cool three-second result is a failure.

**The transport change is substantial; I cannot honestly call 64 KiB a proven minimum.**

For an illustrative uncompressed Merkle opening, 40 queries × 20 levels × 32-byte hashes is already **25,600 bytes**, before field openings and other transcript messages. Multiproofs, packing and WHIR reduce this, but not by a guaranteed factor sufficient for 5 KB.

I would investigate these two explicit envelopes:

| Candidate | Omega budget | Other Payment/carrier allowance | Total budget |
|---|---:|---:|---:|
| Aggressive | 64 KiB | 4 KiB | **68 KiB** |
| Initial implementation ceiling | 128 KiB | 4 KiB | **132 KiB** |

At 256 payload bytes per shard, those are approximately **272 or 528 data shards**, before redundancy. At 4 KiB per NFC chunk, approximately **17 or 33 chunks**. The exact framing must be rebuilt and measured. This sacrifices the current short QR exchange; NFC or a larger animated-QR protocol would be needed.

Neither ceiling is established until the complete hiding recursive proof is serialized at the selected security parameters. The actual minimum is that measured proof plus required metadata—not a number available from asymptotics.

There is also a checkout discrepancy: the current wire spec caps Payment’s envelope at **10,034 bytes**, with Omega≤5,120 and sigma≤3,520, and now adds a companion yielding a **12,098-byte complete carrier**. I have not treated that as permission to relax your tighter quoted transport target. [Envelope caps](../../../specs/kagemusha_wallet_wire_v1.md), [complete carrier](../../../specs/kagemusha_wallet_wire_v1.md).

If ≤5,120 bytes is immovable, the required additional result is concrete:

> A complete, zero-knowledge recursive finalizer, including all claims and bindings, producing ≤5,120 bytes in roughly **0.5–0.8 seconds on Pixel 6**, within the same memory budget.

I found no source demonstrating that result for this statement. It is not a theorem-level impossibility, but it is the missing invention—not an implementation detail to leave until the end.

**Load and Unload use the same terminal proof system.**

Iroha would verify the new proof natively against authenticated scheme/relation definitions and exact public claims. It need not receive an Ethereum-compatible pairing wrapper.

Load must preserve the actual funding authority boundary. Current code authenticates ordinary Sumeragi finality **natively before Advance**; the recursive relation proves the receipt, transition, predecessor and recovery insertion. It does not independently prove BLS finality in-circuit. The replacement should preserve that contract rather than claim a new optimization from removing BLS. [Current Load construction.](../../../specs/kagemusha_lambda_omega_v1.md)

Unload submits its fully completed terminal proof, with the same conservation, nullifier and replay requirements. Enlarging proof transport also means changing ledger admission limits and verification accounting. Nothing on the offline path requires a network.

I would reuse the operation relations, native witness semantics, authenticated store, hardware interfaces, replay/recovery machinery and adversarial fixtures. I would replace the Q/A/W/Omega stage schedule and its proof-specific codecs. The new authenticated scheme must have one explicit encoding and verifier definition; first-release policy provides no reason to retain a slow compatibility path.

**Three serious alternatives remain worth comparing.**

| Alternative | Likely performance relative to today | Advantages | Main obstacle |
|---|---|---|---|
| **Direct small-field recursive design above** | **100× Receive target**, unproven | Removes both large-field engine cost and staged recursion; transparent; CPU-oriented | New ZK/recursion integration, larger transport, independent security review |
| **Direct Spartan/sumcheck + IPA over Pasta** | Rough **5–15+ s research projection**, around 20–60× | Reuses more existing arithmetic and commitment code; compact proofs remain plausible | Linear MSM/opening cost and scalar/base-field recursion mismatch |
| **Fast hash proof plus Groth16/KZG-style compact wrapper** | Fast inner proof plausible; total currently unsubstantiated | Very small final transport; mature verifier techniques | Wrapper must run every payment; trusted setup/additional assumptions; foreign-field verifier circuit |
| **SuperNeo or WARP/Arc accumulation plus per-payment final proof** | No defensible Pixel 6 total yet | Potentially excellent repeated-step cost | Terminal decision, hiding and compact transport remain; highest research risk |

The Pasta alternative has unusually useful local evidence. One current k16 IPA opening costs approximately **25 billion Mac retired instructions**. Calibrating very roughly against your 356-billion-instruction proof taking 23 seconds on Pixel gives about **1.6 seconds per opening**. Two openings already spend the target before commitments, sumchecks or application work. This is a current-kernel projection, not a physical lower bound. [Measurements.](../../../output/kagemusha-nova-folding-20261106/RESULTS.md)

Flattening millions of cells into one IPA vector reduces commitment-message count but increases the linear opening work. Keeping many short columns avoids that increase but makes the proof larger. A new Spartan construction must resolve that tradeoff rather than assume both advantages.

The pairing-wrapper alternative is viable only if its setup and security assumptions are explicitly acceptable. It cannot automatically count as “no weakening,” and a familiar BN254 implementation should not silently substitute for the required security level. Even with acceptable assumptions, the wrapper’s phone timing remains a mandatory gate.

**I would run these five spikes, in this order.**

1. **Measure complete recursive closure before implementing payment logic.**
   On Mac, construct a fixed relation that verifies two prior proofs, exercises the total invalid-input path, and emits a fully hiding recursively reusable proof. Then run it in a Pixel 6 application. Use the intended security parameters, not library defaults.
   **Pass:** closure-only proving ≤1.5 s, peak RSS≤700 MiB, proof≤64 KiB preferred or ≤128 KiB maximum, with stable size/cost across repeated recursion. For the existing transport branch, require ≤5,120 bytes and ≤0.8 s. Failure cheaply rejects the compact-finalizer assumption.

2. **Compile and prove the exact P-256/SHA workload.**
   Compare three-signature current semantics, two-signature fused Receive, and one-signature Send. Include malformed points, scalar boundaries, low-S, hard/soft modes and byte encoding. Run native Longfellow as a separate baseline, without confusing its timings with recursive integration.
   **Pass:** two-signature component ≤0.7 s on Pixel, incremental memory≤250 MiB, and all canonicality/soft-verdict mutations rejected correctly. Failure means the signature arithmetic or field choice needs redesign.

3. **Measure the actual application statement without recursion.**
   Use genuine consumed/credit IMT witnesses, full Payment-byte hashing, policies, controls and local preparation proof. Count live cells, lookup entries, encoded bytes and memory traffic on Mac; measure time in the phone app.
   **Pass:** application work including preparation ≤0.6 s, whole component≤300 MiB, with a credible combined-cell budget. No synthetic Poseidon-only substitute qualifies.

4. **Run complete Send and Receive, including durability.**
   Use hardware P-256, terminal Send transport, local pre-Advance proof, final self-verification and exact-byte persistence. Measure cold, prepared and thermally stabilized runs separately. Exercise duplicate credit, modified proof, invalid signature, interrupted signing and restart.
   **Pass:** ordinary Send and Receive p95≤3.0 s, RSS≤1 GiB, no swap dependence, no hidden preparation backlog. Renewed and quota-enabled paths get explicit results rather than inheriting the ordinary-case claim.

5. **Prove composition correctness and ledger closure on the same candidate.**
   Run A→B→C→Unload, burn/no-op paths, stale readiness artifacts, alternate randomized proofs for one credit, and ArchiveSent recovery. Mutate every binding between receipt, statement, predecessor, Payment bytes and verifier identity.
   **Pass:** independent ledger verification succeeds for valid chains; every named mutation fails or takes the mandated burn/no-op path; privacy analysis covers the complete exposed transcript and retained-state model.

These gates determine whether the proposal earns a **100× claim**, needs a larger carrier or a longer latency budget, or should be abandoned before another costly prover rewrite.
