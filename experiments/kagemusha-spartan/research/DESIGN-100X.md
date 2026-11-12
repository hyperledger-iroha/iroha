# KAGEMUSHA 100x: research, measurements and the proposed design (2026-11-08)

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

Owner target: "it needs to be 3s, so we need 100x speedup" (a Receive is ~300 s on a Pixel 6 today).
Inputs: three Codex gpt-6-astra consults (REPORT.md, REPORT2.md, REPORT3.md in this folder), web research,
Mac spikes on Plonky3 / Plonky3-recursion (spike/, spike/runs/), Pixel 6 Plonky3 runs by Claude b04a57
(pixel-plonky3/RESULTS.md).

## Why today is slow

- A Receive is 12 k16 PLONK/IPA proofs (Q0, Q1, A1..A5, W0..W3, Omega), ~23 s each on a Pixel 6.
  Five of the twelve are recursion/compression, not statement.
- Inside one proof: ~40% FFT, 16-20% MSM, 6% IPA, 21-32% row evaluation. Making all three kernels
  free still leaves ~100 s: no engine-only 100x exists.
- The folding path built this week (8 half-steps + deciders + 4 wraps) is ~292 s on the Pixel; its
  four wraps alone are 168 s.
- The useful statement is small: ~0.6-0.75 M active cells in today's gadgets (2-3 P-256 checks,
  3 SHA-256 blocks, ~1,000 Poseidon permutations, two indexed-tree updates).

## Measurements

Pixel 6 (b04a57, Plonky3 0d803737b6, KoalaBear + Poseidon2-16, 8 threads, 35 °C):

| run | time | proof | peak |
|---|---|---|---|
| plain prove 2^18 x 100 | 1.48 s | 339 KiB | 0.26 GB |
| plain prove 2^20 x 100 | 5.52 s | 432 KiB | 1.31 GB |
| zk prove 2^18 x 100 | 12.1 s | 564 KiB | 2.32 GB |

About 200x today's prover per cell, but zero knowledge costs ~9x and proofs are 340-650 KiB.

Mac, Plonky3-recursion 2-to-1 aggregation, one layer, 4 threads (projected to Pixel at 15.5 G instr/s):

| config | layer | proof | Pixel projection |
|---|---|---|---|
| FRI zk defaults | 3.12 s | 341 KB | ~4.5-5 s |
| FRI zk, blowup 8, PoW 20 | 4.73 s | 231 KB | ~4.3 s |
| FRI non-zk, blowup 8, PoW 20 | 0.95 s | 169 KB | ~1.5 s |
| WHIR, 66-bit conjectured | 3.09 s | 166 KB | ~3.6 s |

Per-operation recursion of hash-based proofs is too slow and too large: rejected.
Plonky3 example defaults are ~59 bits proven / ~100-124 conjectured; a 128-bit target costs more.

Published anchors: Longfellow proves 1/2/3 P-256 ECDSA in 80/120/152 ms on a Pixel 6 Pro (native
P-256-field sumcheck + Ligero, ~200 KB proofs, 116/117-bit profiles); UltraHonk P-256 1.1 s on a
Pixel 8a; WHIR 2^22 commit+open 1.2 s, 63 KiB; WARP (2025/753) hash-based linear-time accumulation.

## Proposed design (Codex round 3, checked against the measurements above)

1. **Per operation: small-field accumulation, no proof recursion.** Each operation's relation
   (transition, maps, policy, authorizations, the accumulation-update check, incoming-Omega
   obligations) folds into a private, non-zero-knowledge accumulator with hash/RS commitments (WARP)
   or small-field Ajtai commitments (SuperNeo). The phone never proves a FRI/WHIR verifier per step.
2. **Payment carries the payer's completed Send proof.** No separate sigma_send; the receiver no
   longer re-proves the payer's receipt signature (3 -> 2 P-256 checks per Receive).
3. **Zero knowledge only at export.** When a proof leaves the phone (Send's Omega, Unload, optional
   proof-bearing Credited), one terminal ZK decider with Pasta-native PCS hashes closes the
   accumulator, then a non-ZK compact Spartan-style argument over Fp (16 Vesta row commitments of
   8,192, one length-8,192 IPA) gives a **4,992-byte Omega**. Its deferred generator claims are
   decided natively by every verifier, as today's Omega does. No trusted setup; Pasta assumptions.
4. **Curves:** P-256 and the incoming Pasta scalar/curve checks as native-field accumulators (or
   12-bit limbs in KoalaBear; prototype both), every instance bound to the operation's statement.
5. **Unchanged:** proof before payout (the pre-Advance preparation proof, serialized receipt,
   complete native verification and durable exact bytes before release), credit identity,
   Load's native Sumeragi finality boundary, the 5,120-byte transport.
6. **Release condition for local credit (owner decision):** a Receive records durable local credit
   only after the incoming completed Send proof is verified natively in full (succinct checks and
   generator-claim decisions) and the operation's own relation is accumulated. That credit cannot be
   paid onward, unloaded or exported as proof-bearing evidence until an export has decided the whole
   accumulator. No value leaves the wallet on an undecided proof. This must be confirmed against
   "you must always do the proof first before payout".

Acceptance budget on a Pixel 6 (assumptions A, projections P; D = terminal ZK decider,
U = remaining compact work, both unmeasured):

| seconds | cold Send | cold local Receive |
|---|---|---|
| witness + pre-Advance proof | 0.20 (A) | 0.30 (A) |
| accumulation update incl. field links | 0.65 (A) | 1.40 (A, ~21.7 G instr) |
| native claim folds / Omega verification | 0.15 | 0.16 |
| receipt + durability | 0.20 | 0.20 |
| claim restructuring + compact commitments/IPA | 0.74 (P, unmeasured on a phone; b04a57's k16 proxy suggests 0.8-1 s) | - |
| D + U | must be <= 1.06 | - |
| **total** | **1.94 + D + U** | **~2.1** |

Status: a ~2 s local Receive is plausible (b04a57's 2^18 x 100 plain run in 1.48 s supports the
accumulation budget). A 3 s Send depends on an export prover nobody has measured; with today's
~9x zk overhead it would be several seconds. A Receive that must export a proof immediately is
~2.95 + D + U.

## Export spike result (2026-11-08): the 4,992-byte export fails its gate

The compact Pasta argument must prove "the terminal hash-based proof verifies" inside an Fp circuit
of at most 2^18 constraints, in about 1 s on the Pixel 6. Measured and counted:

- A FRI verifier (Plonky3-recursion, KoalaBear d4, blowup 8, PoW 20, ~35 queries) needs about
  4,000 Poseidon2 permutations and ~13,000 extension-field operations per verified proof (the 2-to-1
  aggregation circuit: Poseidon2 table 8,192 rows, ALU 13,241 rows x 2 lanes; spike/runs/agg-nozk-b3p20.log).
  Large traces need more (deeper trees).
- One native KAGEMUSHA Pasta Poseidon permutation (t = 3, x^5, 8 + 57 rounds) costs 166k
  instructions, ~22 µs of CPU on this Mac (spike/pasta-bench): ~830 full-width field
  multiplications, about 100x a KoalaBear Poseidon2 permutation.

| route to <= 5 KB | cost | gate |
|---|---|---|
| terminal proof Merkle-hashed with Pasta Poseidon (2^16-2^18 trace, blowup 8) | 4-17 M permutations, ~0.7-2.8 T instructions, ~45-180 s on the Pixel | fails (~1 s) |
| terminal proof keeps Poseidon2-KoalaBear; Fp circuit emulates it | ~4,000 x 3,000-28,000 constraints = 12-112 M constraints | fails (262k) |
| shrink in the small field first, Pasta-hash a tiny final layer | ~1.5 M Pasta permutations (~16 s) plus the shrinking layers | fails |

So a hash-based core cannot be compressed to today's 5,120-byte transport in about a second on a
phone with these constructions. Two paths remain:

- **Path 1, larger offline carrier.** Keep the small-field design end to end and transport the
  hash-based export (100-500 KB) over NFC or Nearby (BLE/Wi-Fi); the one-screen QR cannot carry it.
  Receive ~2 s stays plausible; Send depends on the terminal zero-knowledge proof (today ~12 s at
  2^18 x 100 on the Pixel; it needs the ~9x zk overhead cut to ~2-3x).
- **Path 2, keep the 5 KB QR.** Prove each operation directly with a curve-based argument over Pasta
  (Spartan-style sumcheck with Hyrax/IPA commitments, accumulated claims decided natively as today),
  with no hash-based inner proof. Codex projected 5-15 s (20-60x); the open lever is cheap
  commitments to mostly small witness values (pay-per-bit MSMs). Needs its own spike.

## Path 2 (owner: keep QR; "we can go to 12 kb QR"), Codex round 4 + Pixel kernels

Owner chose to keep QR payments and allows a 12 KB QR. Proof allowance in a 12,000-byte Payment:
12,000 - 1,394 (envelope) - 84 (header) - 16 (framing) - 2,048 (payer companion) = **8,458 B**.

Design (REPORT4.md): a direct Pasta-cycle prover. A primary proof over Fp (Vesta commitments) and a
secondary over Fq (Pallas commitments) per operation, a uniform low-degree Spartan/CCS relation
(uniform circuits keep matrix evaluation succinct for recursion), rectangular Hyrax commitments,
an IPA per proof, and one inherited generator claim per field decided natively by every verifier.
Payment carries the completed Send pair; proof before payout unchanged; no trusted setup.

Pixel 6 kernels (b04a57, pixel-pasta/RESULTS.md, 8 threads):
- 16 secret MSMs of 2^13 + a length-8,192 IPA opening: ~0.6 s.
- Pay-per-bit secret MSMs (msm_secret_bounded_cancellable) at 2^18: 555 ms (255-bit), 76 ms
  (16-bit), 49 ms (8-bit), 31 ms (1-bit).
- Params derivation: 7 s (2^16), 32 s (2^18), so ship or cache them.

Three more Pixel 6 kernels (b04a57, same file, 34 °C):
- Today's hiding IPA opening at length 65,536 (1,120 B): 1.52-1.56 s.
- One inherited-claim restructuring at k16 (restructure_prove + create_restructure):
  17.9 ms + 1.49 s, peak 46 MB.
- Degree-3 sumcheck prover over 2^20 rows of Fp: 0.28 s.
- 8 Vesta commitments of 65,536 full-width secret scalars: 1.07 s, or 0.87 s with fixed-base
  tables.

The two biggest lines come in about 15% under REPORT4.md's allocations.

### Gate 1 (bytes) passes: ZK Spartan with committed sumcheck rounds (spike/zk-spartan)

A working prototype exists over Vesta and Pallas. Details and logs:
../RESULTS.md, runs/zks-all-16t.log.

How it hides the transcript and stays small:
- Each sumcheck round binds 4 variables and sends one hiding commitment (32 B) to the round
  polynomial's values on {0,1,2}^4. These occupy reserved tail columns of the Hyrax opening.
- One alpha-batched linear relation covers every round check, both final checks and the hidden
  witness evaluation. A single ZK IPA proves it.
- v_A, v_B, v_C are revealed, masked by two random multiplication gates.
- The IPA blinds every L and R and ends with a Schnorr final (A, z1, z2). No masking vector
  is committed.

Measured bytes, exact serializations:

| Proof | Bytes |
|---|---:|
| Send primary (2^20 constraints, 8 x 65,536) | 1,824 |
| Send secondary (2^16, 8 x 8,192) | 1,600 |
| Receive primary | 2,144 |
| Receive secondary | 1,888 |

Pair totals with REPORT4.md's header and inherited-claim lines:
- **Send 4,544 B against 8,458 B** (4,080 B with 128-bit inherited challenges).
- **Receive 5,152 B against 9,600 B.**

The ZK pairs are smaller than round 4's non-ZK count (8,416 / 9,120 B): a committed round costs
32 B per 4 variables, against 64 B per variable sent in clear.

Soundness bug and fix:
- The first version opened the plain sum of the round commitments. A later commitment can then
  rewrite an earlier round, or the witness columns, after its challenges.
- Two forgeries for an unsatisfied witness were accepted. One is the round cancellation that
  b04a57's soundness consult also found (REPORT-b04a57-soundness.md §6(b)); the other adapts the
  witness after tau.
- Fix: open sum_r eq(r_row,r) T_r + sum_i gamma^i D_i, with gamma drawn after alpha, and scale
  slot i's weights by gamma^-i. Off-slot terms then land on nonzero powers of gamma.
- Both attacks stay in the regression suite (curves x k = 1..4) and are rejected. The suite also
  covers single-bit flips of every element, truncation, wrong public input and three unsatisfied
  witnesses.
- A written proof of the separation argument is still owed.

Mac timing (16 threads, toy census, indicative only):
- Send primary prove 0.37 s, of which the IPA phase is 0.23 s; secondary 0.08 s.
- Receive primary 0.49 s; secondary 0.12 s.
- Verify 0.03 s; peak 506 MB.
- Pixel 6 run (b04a57, ../PIXEL.md, 8 threads, 285 MB peak): the regression suite
  passes on the phone.
- Pixel prover cores:

  | Shape | Prove | Of which IPA |
  |---|---:|---:|
  | Send primary 8 x 65,536 | 2.21 s | 1.33 s |
  | Send secondary | 0.46 s | |
  | Receive primary 16 x 65,536 | 2.96 s | |
  | Receive secondary | 0.52 s | |
  | Send primary 64 x 8,192 | 1.39 s | 0.30 s |
  | Receive primary 128 x 8,192 | 2.24 s | |

  The phone is 4.3-5.5x slower than the Mac.

Byte headroom buys speed. These are the critical-path consult's 8,192-column own IPAs on both
fields, measured:

| Pair | Total | Limit | Spare |
|---|---:|---:|---:|
| Send (64 x 8,192 primary) | 6,048 B | 8,458 B | 2,410 B |
| Receive (128 x 8,192 primary) | 8,448 B | 9,600 B | 1,152 B |

The IPA phase is about 4x faster. Small row MSMs commit less efficiently, so the commit time goes
up.

### Gate 1b (recursion census): conditional pass (../CENSUS.md, REPORT5.md)

I measured the gadget costs in R1CS, each synthesized and checked against iroha_pasta:

| Gadget | Constraints |
|---|---:|
| 128-bit endoscale | 583 |
| 255-bit variable-base multiplication | 1,782 |
| Fixed-base multiplication | 768 |
| RP57 permutation | 243 |
| Squeeze decomposition (2 challenges) | 511 |

The census applies them to this prototype's verifier and iterates row counts over all four operation
orders. Construction counted:
- Transcripts are hashed where the proof's points are native; about 50 values per proof cross circuits.
- Point-multiplying challenges are 128-bit endoscalars.
- The scalar verifier uses the closed-form `b_final`, now validated against the O(C) fold on every
  verified proof.
- Inherited claims: **folded in clear** (owner decision 2026-11-08, relayed by b04a57: "we don't
  really care about linkable/unlinkable tbh"). Today's create_restructure re-randomizes them,
  keeping payment histories unlinkable across wallets; folding them in clear links them.
  - Fold in clear: 3.1 s Send / 4.7 s Receive (gate 1b reference; gate 1c below supersedes).
  - Private masked restructuring per curve: **4.3 / 6.3 s**. That is 1.17 s of restructuring IPAs,
    +~100k primary and +~35k secondary constraints, and Receive's secondary at 2^19 (5.9 s with one
    small cut).
  - A fused masked opening has no sound construction yet.
- kappa stays in-circuit, because it contains the predecessor's public inputs. The two circuits bind
  their shared values by a masked random linear combination, at zero bytes.

Result, with REPORT4.md's whole-relation census as the operation reference:
- At P 32 x 32,768 and S 32 x 8,192:
  - Recursion overhead: Send primary 178k / secondary 134k; Receive 344k / 235k.
  - Circuit sizes: primary 2^20 (Send) / 2^21 (Receive), secondary 2^18.
  - Pairs: **5,600 B Send, 7,168 B Receive**.
  - Modelled Pixel prover cores: **3.1 s Send, 4.7 s Receive**.
- At P 65,536 columns: pairs 4,656 / 6,240 B, prover cores 3.3 / 4.9 s.
- The 8,192-column primary fails: its 64-128 rows push the next secondary to 2^19 and the bytes
  to 9.5 / 14 KB.

Codex round 5 (REPORT5.md) counts a more conservative construction:
- scalar-field transcripts, so points cross as limbs and S recomputes a row commitment over them;
- full-width coefficients;
- the literal O(C) scalar verifier;
- private restructuring IPAs.

On that basis, its secondaries reach 2^21-2^22, its pairs 6.5-8.4 KB, and its cores 7-12 s.

Open items:
- The operation's own R1CS census, the biggest unknown. Round 4 counted witness values, not
  constraints, and range checks without lookups cost a constraint per bit. A lookup argument is
  likely needed.
- The synthesized non-native binding (700 per value, estimated).
- Written proofs of gamma separation and the joint opening.
- Next: count-only synthesis of the in-circuit verifier. Gates: P-verify ≤ 75k in C_S, S-verify
  ≤ 70k in C_P, REPORT5's refusal list rejected.

### Consults on this construction (b04a57, REPORT-b04a57-{soundness,critical-path}.md)

Soundness review:
- (a) holds with eq genuinely factored out and all four challenges drawn after the commitment.
- (c) gives statistical HVZK for (v_A, v_B, v_C) at distance < 1/|F|, provided the gates are
  isolated, the masks fresh, the mask inputs otherwise unexposed and the MLE weights nonzero.
- (d) has a sound ancestor (Halo). It still needs full-round extraction, the folded generator
  decided, the challenge bound to the whole statement, and nonces never reused.
- Inherited claims must be re-randomized for unlinkability; a reused (G, u) is an ancestry tag.
  The owner accepted the linkage (fold in clear), so this becomes a named leakage of the ZK
  theorem, not a soundness requirement.
- Unbounded depth needs a PCD theorem checked against this construction, or a stated recursive
  Fiat-Shamir assumption, as today's spec has.
- System points, already true today: proofs cannot stop forks from a cloned key plus state, so
  anti-forking rests on custody. ZK hides nothing the Payment exposes publicly.

Critical path:
- 8,192/8,192 IPAs and Pixel kernel prices give conditional critical paths of Send ≈ 4.3 s + Z_S
  and Receive ≈ 6.2 s + Z_R, down from 8.1 / 10.7.
- Idle-time head preparation brings Send to about 3.1 s + residual preparation + Z_S. Receive
  cannot precompute its merged accumulator.
- Root-cause cut: open the inherited claim inside the own IPA (joint sumcheck, tail correction).
  It saves about 0.6-1.8 s, unbooked until proved and measured.
- Risk: more rows grow the next operation's recursive verifier, by 117k-175k secondary gates for
  Send after Receive. Round 4's Send census (623k) already exceeds the nominal capacity, so a
  recursive census must close the loop.
- GPU MSMs are worth about 0.2 s.
- A 3 s attempt needs phase caps of Send 2.55 s and Receive 2.95 s.

### Gate 1c (operation census): pass (../OPERATION.md)

The Send and Receive relations themselves, with the large gadgets synthesized on real values and
every constraint checked:
- P-256 ECDSA in the Fp circuit: three 88-bit limbs, a CRT check mod 2^264 and mod Fp, R recovery,
  half-size scalars (v = v' u2 mod n, |v|, |v'| <= 2^130) and one Straus loop. That is 821
  non-native checks per signature: 9,540 rows plus 56,306 range lookups.
  - Every addition checks both chord-slope equations, so no slope is a free 0/0 witness.
  - b04a57's review found the gap: with one equation, a prover choosing R' = aQ + bG could force
    equal points at any step.
  - Tests cover forced equal-point additions and wrong slopes.
- SHA-256 of the signed 32-byte digest: 25,976 rows.
- RP57, indexed trees, comparisons and u128 checks are counted from the specification.
- Lookups: LogUp with committed inverse helpers in a second witness phase. No extra rounds or
  bytes. LogUp-GKR is rejected because its ~60 extra rounds would push Receive's secondary to 2^19.

| | Send | Receive |
|---|---:|---:|
| Own relation with LogUp | 171k constraints, 229k witness | 297k, 401k |
| Gate 1b reference (REPORT4, which already contained recursion work) | 559k | 877k |
| Circuits (primary / secondary) | 2^19 / 2^18 | 2^20 / 2^18 |
| Pairs | 5,056 B | 6,624 B |
| **Modelled Pixel prover core** | **2.37 s** | **3.44 s** |
| Largest variant (Send+quota / Receive+renewed) | 2.43 s | 3.86 s |

Construction behind the numbers:
- Shape: P 16,384 columns (32 or 64 rows), S 16 x 16,384.
- Inherited claims are folded in clear.
- The binding step is measured at 83 constraints (gate 1b assumed 700).
- The matrix-MLE budget is 8,000 per verified proof.

Where the time goes: full-width commitments and the four IPAs are about two thirds.

Still a model, to be calibrated:
- the commit cost per width class (`zk-spartan commit-classes` on the Pixel);
- the wiring lookups between uniform blocks.

Unbooked levers:
- **Unbalanced half-scalars with per-wallet key tables.** About 440 instead of 821 checks per
  own-key signature. P-recv lands about 2% over the 2^19 boundary; if it crosses, that is worth
  about 0.6 s.
- **Lookup-based SHA-256:** about −17k constraints per signature.
- **A sparse io half in the inner sumcheck:** about 0.1 / 0.2 s.
- **Signing the Poseidon message without SHA-256** is an owner decision (below).

So QR at 12 KB now looks like **~2.4 s (Send) and ~3.4 s (Receive) of modelled prover core**. That
is about 3-4 s end to end with witness generation, native checks and durability, or 75-100x. This
holds only if the Pixel commit calibration and the verifier synthesis confirm the counts. A 3 s
Receive needs the ECDSA lever or idle-time preparation.

### Gates 1c+ and 1d (2026-11-08 evening): lever 1 and the verifier, both synthesized

Records: ../OPERATION.md ("Update, evening") and ../VERIFIER.md.

- **Pixel costs adopted.** Commit costs are b04a57's lane commits and the IPA is measured (PIXEL.md). With them,
  gate 1c is 2.28 / 3.24 s.
- **Lever 1 synthesized: pass.** Own-key ECDSA over per-wallet comb tables.
  - No R' recovery: P = u1·G + u2·Q from signed one-bit digits, 16 bases, two 256-entry tables.
  - 289 checks instead of 821. Send's relation goes 171k → 122k and Receive's 297k → 206k constraints.
  - The tables cost a one-time Bootstrap proof (253k constraints). Their commitment is a per-incarnation
    constant, so a wallet's proofs become linkable. That fits the owner's linkability decision.
- **Verifier synthesized: the census undercounts (gate fails as stated).**
  - The in-circuit verifier is satisfied by real proofs, every constraint checked, and the refusal tests pass.
  - The scalar side matches within 1-4%.
  - The points side is 11-20% higher, and about twice as many values cross the circuits.
  - Cause: challenges are per variable and each IPA round takes its own squeeze; the census assumed Kronecker
    powers.

| Verifier model | Gate 1c signatures | Lever 1 comb tables |
|---|---|---|
| Census formula | 2.28 / 3.24 s | 2.06 / 2.62 s |
| **Synthesized** | **2.46 / 3.48 s** | **2.37 / 3.29 s** |
| Synthesized + lean challenge schedule (estimate) | 2.13 / 3.13 s | 1.96 / 2.60 s |

Comparison:
- Under the synthesized verifier, the comb's P-recv misses 2^19 by about 45k witness entries, so lever 1 is
  worth only −0.09 / −0.19 s there.
- The verifier correction itself costs +0.18-0.31 s Send and +0.24-0.67 s Receive.
- **Lookup SHA-256 (lever 5), synthesized:** 2,709 rows plus 3,392 lookups per block, against 25,976 rows. It
  pays only where it moves a size class:
  - census formula 1.88 / 2.50 s;
  - synthesized 2.38 / 3.30 s (no change, P-recv stays 2^20);
  - lean estimate 1.89 / 2.51 s, with P-recv at 2^19 and 3.6% headroom instead of 0.2%.
- **Lean challenge schedule, synthesized (gate 1e, VERIFIER.md): pass.**
  - τ is drawn as powers of one full-width challenge, and each round's challenges as base-3 Kronecker powers of one
    126-bit challenge. The row-bit rounds stay per variable. Raw-only squeezes are range-checked by lookups.
  - On real proofs, every constraint is checked and the refusal tests and tamper suite pass.
  - The points side drops 21% (P-recv 88.1k → 69.4k), and about 60 values cross instead of 100.
  - Soundness: about a 3-4 bit loss on the Kronecker rounds (degree 80 / 2^126 per round); τ is negligible
    (2^20 / 2^254).
  - **Modelled with Pixel-measured costs: the comb plus lookup SHA-256 reaches 1.89 s Send and 2.51 s Receive**
    (P-recv 2^19, 3.8% headroom; 6.6% at P 16k / S 16k for 2.08 / 2.64 s). The largest variant is 2.08 / 3.19 s.
- **Cross-circuit binding, synthesized (gate 1f, VERIFIER.md): pass.**
  - The pairing link puts ρ after both proofs' phase-1 commitments: P's digest d → S's rows → ρ → P absorbs ρ;
    both transcripts absorb R.
  - On real linked pairs: each proof verifies only with its partner. A different R is refused. The R chains over
    the pair's 133 shared values cost 92.4 per value in C_P (the model's binding step) and 1 per value in C_S, and
    equal the absorbed R.
  - An early ρ admits a lattice forgery (shown), which the schedule prevents.
  - Every verifier cell passes a single-cell mutation test. It found and removed two dead cells.
  - Adding the link (+527 rows per proof, six limbs per pair) gives **1.89 s Send / 2.52 s Receive**, largest
    variant 3.20 s. R adds 32 B per pair on the wire.
  - Open: modulus aliases (counted comparisons), R inside a proven circuit (needs the real operation circuit),
    the burn branch.
- **Renewal path (OPERATION.md "The renewal path").** The largest Receive variant (a Request quoting the
  pre-renewal receiver credential) re-verifies that credential's issuer signature and certificate. That puts it
  at 3.20 s.
  - Issuer comb tables or issuer keys as circuit constants only reach 3.09 / 3.02 s.
  - **Credential continuity reaches 1.94 / 2.52 s for every variant.** The state keeps the last four credential
    digests (each verified in-circuit when introduced), and Λ_recv checks the quoted credential against that
    window instead of re-verifying signatures.
  - It is a spec change to R10 / §3.2. Owner decision below.
- Those two plus lever 1 would put an ordinary Receive near 2.6 s. The largest variant would be about 3.2 s.

Next falsifying measurements:
1. **Recursive census:** build the Send/Receive verifier circuits (predecessor + incoming pair,
   CycleFold secondary, claim re-randomization) at the chosen row counts. Gate: the circuits fit
   the shapes above and the row count is stable under recursion (Send after Receive included).
2. **Pixel run of spike/zk-spartan** at 8 x 65,536 and 64 x 8,192, plus real-census commitments.
   Gates: own IPA ≤ 0.4 s at 8,192, primary prove core ≤ 1.5 s.
3. **Shared inherited opening** (joint sumcheck + tail correction), then a written soundness proof
   of the separated committed-round relation.

### Current modelled prover core (2026-11-08, end of day)

Pixel-measured commit (lanes) and IPA (endoscalar challenges after the GLV fix, iroha `8ca1643b3f`). Verifier
synthesized on real proofs (lean schedule, pairing link). Own-key comb tables and lookup SHA-256.

| | Send | Receive |
|---|---:|---:|
| Ordinary | **1.68 s** | **2.31 s** |
| Largest variants, with credential continuity (spec `00fc09fcea`) | 1.73 s | 2.32 s |
| Largest variants, under the old renewal rule | 1.87 s | 2.99 s |

The prover core is below the 3 s target. Today's folded Receive takes about 292 s on the Pixel, so this is about
125x. Witness generation, native checks and durable completion come on top and are not yet measured. These are
modelled, not measured end to end.

### Real Receive circuit, stage A (2026-11-09; ../REAL-RECEIVE.md)

Owner request: "Real Receive circuit". The Receive pair was built at full size from real constraints, then proved
and verified on the Mac:

- P-recv holds the own relation, the synthesized verifiers of both incoming pairs and the binding chain.
- S-recv holds the mirror.
- The prover is general sparse R1CS with a real LogUp argument.

| | P-recv | S-recv |
|---|---:|---:|
| Constraints | 2^19 (368,601; census 363,768) | 2^18 (195,283) |
| Proof | 2,464 B | 2,400 B |
| Mac prove per proof (matrix build excluded) | 0.31 s | 0.17-0.26 s |
| Pixel model for this exact layout | 1.56 s | 0.88 s |
| **Pixel 6 measured, core (medians of two runs)** | **2.11-2.16 s** | **1.18-1.22 s** |

- **Receive: 2.44 s** modelled on the Pixel for the real layout, against 2.31 s from the census.
- The gap is density: 15.6 nonzeros per constraint.
- In each proof, three constrained cells set off by one were rejected.
- Stage A wires the circuit directly. The recursive verifier needs the uniform-block compiler that REPORT4
  requires (copy lookups into a value table), which is not built yet. Until then, stage A validates the gadget
  counts and the prover's cost structure, not the uniform layout.
- **Pixel 6, 2026-11-09 (owner: "Yes, run on Pixel"): 3.28 / 3.38 s per Receive pair, 1.34-1.39x the model.**
  - All 12 proofs were accepted. Peak memory was 1.10 GB, flat across proofs.
  - Counting the prover core only, this is about 90x today's ~292 s.
- Where the 0.89 s gap goes:
  - **Commits, +0.40 s.**
    - About 0.1 s of it is lane tails. Lanes commit whole rows single-threaded, so a commit call waits for its last
      row. The window-claim fix (iroha 28ef3cd191) recovers that 0.1 s in b04a57's Pixel A/B.
    - The other ~0.2 s per pair is phase-1 time that the isolated harness does not show. It is not repeated values
      (checked on the Mac). Candidates are serial bound checks and blinding, or the phone's state after assembly.
      A Pixel run with the commit split decides it.
  - **Sparse matrix evaluation inside the opening, about +0.37 s.** The uniform layout removes it.
  - **Sumchecks, +0.10 s.**
  - **Helper inversion, +0.02 s.**
- With the tail fix and the uniform layout, a Receive pair comes to about 2.9 s, or about 2.7 s if the rest of the
  phase-1 commit gap is recovered.
- Next:
  - the uniform layout (stage A2), started 2026-11-09 (`../A2-UNIFORM.md`).
    - The block census shows that no grouping of today's gadgets fits: per-gadget blocks need 310k copy lookups
      for P-recv, and outermost blocks give 2 M template nonzeros, against an ~8,000 budget.
    - So the gadgets are rewritten as uniform steps with shift, carry and tree links, public constant columns,
      and copy lookups only for irregular flows.
    - Permutations (about half of P-recv's rows) go first;
  - the Pixel A/B of the real Receive pair, old against new lanes, timing the commit split;
  - stage B (exact encodings).

## Owner decisions this design needs

- **Credential continuity for renewed Requests (2026-11-08): decided and applied** (iroha `00fc09fcea`, signed, not
  pushed; owner: "Yes, draft spec change", then "Apply and commit").
  - Rationale and measurements: `SPEC-CHANGE-credential-continuity.md`.
  - A depth-16 prior-credential array indexed by `renewal_sequence`, root in the core. Λ_recv accepts a
    pre-renewal receiver credential by membership instead of re-verifying its issuer signature and certificate.
  - Receivability is unbounded (up to 65,536 renewals). The largest Receive variant goes from 3.20 s to 2.53 s.
  - The wallet state format changes, so it ships with the Path 2 wallet generation.
  - Implementations still re-verify until they adopt the new rule.

- **Issuer-certified key tables (2026-11-12): decided and applied** (spec change committed with
  `SPEC-CHANGE-key-table-commitment.md`; owner: "Issuer certifies them", "Yes, draft it", then "Apply and commit").
  - The credential carries `key_table_commitment`, the commitment to the payment key's comb tables. The issuer
    computes it from the attested key, and the wallet recomputes it at enrollment.
  - Every lineage proof checks payment-key signatures against those tables, so one set of circuits serves every
    wallet. Deriving the tables in Bootstrap does not fit (`d21de32e26`); the certified wallet row keeps Send and
    Receive at 1.55 s and 2.21 s per pair (`d39841bd20`).
  - The credential format changes, so it ships with the Path 2 wallet generation.
  - Open: the issuer's and the root's keys need certified tables of their own.

- **Path 2, inherited claims: decided 2026-11-08, fold in clear.** Relayed by b04a57: the owner
  first chose "Keep unlinkable", then wrote "we don't really care about linkable/unlinkable tbh";
  the later message stands. Anyone holding two proofs can link which payment fed which; balances
  and witness values stay hidden. Private masked restructuring is dropped (it cost about +1.2 s
  Send / +1.6 s Receive). See ../CENSUS.md, choice 4.

- **Optional, Path 2: sign the 32-byte Poseidon message with no SHA-256 inside the signature**
  (Digest NONE). It saves the in-circuit SHA-256 block: −26k constraints for Send, −52k for
  Receive. The key-only forgery would need a Poseidon preimage, so it looks sound. But
  wire:95-100 forbids no-digest modes, and it needs new key parameters (new incarnations),
  enrollment and attestation updates, SDK changes and StrongBox checks. Raise it only if the
  remaining levers fall short.

- Protocol: Payment carries the completed Send proof; fast Receive gives durable local credit, and
  proof-bearing Credited evidence becomes a separate export.
- A new proving stack (small-field accumulation + terminal ZK + compact Pasta argument), replacing
  the Q/A/W/Omega schedule and the folding wraps; Plonky3 (or our own small-field crate) as a
  production dependency.
- Security target stated as proven bits (not the libraries' conjectured defaults).
- Fallback if the 4,992-byte export misses its gate: a larger carrier (64-128 KiB over NFC/Nearby),
  which loses the one-screen QR.

## Next spikes (cheapest kill first)

These were the Path 1/export spikes (spike 1 ran and failed; see above). Path 2's current list is
at the end of the Path 2 section.

0. **Pasta compact kernels on the Pixel 6** (cheap, proposed by b04a57): iroha_pasta Vesta MSMs of
   2^13 x 16 and one length-8,192 IPA prove. They replace the 0.74 s projection; if they cost
   0.8-1 s, the export gate for D + U tightens to ~0.8 s.
1. **Export construction:** a representative accumulated state, the terminal ZK decider with
   Pasta-native hashes and the complete compact verifier. Kill if the verifier exceeds 2^17 private
   coordinates / 2^18 constraints or 4,992 bytes; then measure D + U against <= 1.06 s on the Pixel 6
   (less if spike 0 comes in high), and its peak memory against <= 1 GiB (Plonky3 zk at 2^18 x 100
   peaked at 2.32 GB on the Pixel; the accumulation update's 0.26 GB is not the risk).
2. **One genuine augmented Receive update** (real signatures, incoming compact-proof arithmetic,
   maps, update check, claim folds), common-field limbs vs native-field accumulators, by phase:
   gate <= 21.7 G instructions, then end-to-end on the Pixel.
