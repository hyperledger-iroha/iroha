# Gate 1d: verifier synthesis (2026-11-08)

Historical research record. See [README.md](README.md) for the current scope, reproduction commands and unresolved production gaps.

Run: `zk-spartan verifier` (code: `src/verifier.rs`; formula `census::verify_cost_synth`; transcript `src/fs.rs`).
Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-verifier-v1.log`. Tests: `verifier::tests` (3, all pass); the tamper suite still passes.

**Verdict: the census verifier formula undercounts. CENSUS.md's gate fails as stated.**
- The in-circuit verifier of one P-proof and one S-proof is synthesized and satisfied by real proofs, with inherited
  claims folded in clear. Every constraint is checked on insertion. Every in-circuit challenge, point and scalar
  equals the native verifier's.
- The scalar side matches the census within 1-4%.
- The points side (transcript and group work) is 11-20% above the census.
- About twice as many values cross between the circuits.
- Gate (CENSUS.md): P-verify ≤ 75k in C_S and S-verify ≤ 70k in C_P.
  - P-verify is 78-91k: fail.
  - S-send's 68.7k passes; S-recv's 75.8k fails.
- With the synthesized costs, Receive's primary no longer fits 2^19 under lever 1 (OPERATION.md, "Lever 1
  synthesized"). A leaner challenge schedule would more than recover it, but that is an estimate.

## Gate 1f: the cross-circuit binding, synthesized (2026-11-08)

Run: `ZKS_SCHEDULE=lean zk-spartan binding` (code: `spartan::Link`, the binding section of `src/verifier.rs`,
`operation::rlc_chain`). Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-binding-v1.log`. Tests: `pairing_link_binds_partners`,
`binding_rlc_refusals`, `verifier_cells_are_constrained`.

**Verdict: pass.** The binding works as designed and costs what the model charged, plus a small link term.

**Construction.** A P-proof (Vesta, circuit C_P) and an S-proof (Pallas, circuit C_S) of one operation share
the values their two halves of the predecessor's verification exchange: every challenge and scalar, the claim
points, and the link words.
- Each circuit computes R = μ·ρ^n + Σ v_j·ρ^(n−1−j) over Fq of its own copies. C_S does this natively; C_P does
  it non-natively. μ is a fresh mask both circuits commit.
- **The link** (`spartan::Link`) orders the challenge after both commitments:
  - P squeezes its phase-1 digest d after its rows.
  - S absorbs d and its own rows, then squeezes ρ.
  - P absorbs ρ, and both transcripts absorb R.
  - So ρ follows both proofs' phase-1 commitments.
- Whoever verifies the pair replays both transcripts with one R: the next operation's circuits, or the payee
  natively.
- R crosses as a shared value in-circuit. It is 32 bytes on the wire for the payee's native check: the Send pair
  becomes 5,008 B and the Receive pair 6,016 B, both within their limits.

**Measured on real linked pairs at the fixed point's shapes** (P-recv 2^19, 32 × 2^14, and S-recv 2^18,
32 × 2^13, each with two inherited claims):
- **Link replay:** +527 constraints per proof on the points side, none on the scalar sides, and six more shared
  limbs per pair (d, ρ, R).
- **R chains:** over the pair's 133 shared values, C_P costs 12,286 constraints at chunk 11 (92.4 per value),
  equal to the measured binding step the model charges (83.4 per value at chunk 13). C_S costs 133 rows.
- Both chains equal the R that the next pair's linked transcripts absorbed.
- **Model:** `verify_cost_synth` and `verify_cost_lean` now add the link (527 rows, three shared limbs per proof).
  The fixed point becomes 1.89 / 2.52 s for the comb with lookup SHA-256 (P-recv 2^19, 3.2% headroom; 6.0% at
  P 16k / S 16k for 2.08 / 2.65 s). The largest variant is 3.20 s.
  Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v9-lean-paired.log`.

**Refusals (REPORT5's pairing and binding cases), each rejected:**
- **A P or an S from another pair, natively.** The partner's transcript replays with the wrong d or ρ and fails.
- **A different R.** It is absorbed by both transcripts.
- **ρ depends on S's rows.** Another S witness gives another ρ for the same P.
- **A differing copy of one shared value**, and reordered, omitted or duplicated copies: R changes.
- **ρ known before the copies are committed** (the forgery the schedule prevents). 2D lattice reduction finds
  δ·ρ + ε ≡ 0 (mod q) with |δ|, |ε| < 2^128. Shifting two adjacent copies by (δ, ε) keeps R unchanged, and both
  copies stay within the 128-bit range. A fresh ρ refuses them.
- **Assigned-cell mutation.** Every witness cell of both verifier sides, on both curves, linked and unlinked, is
  perturbed alone, and some constraint fails.
  - The test found two dead cells per scalar side: the link word was allocated but never used. They are removed.
  - It does not catch multi-cell aliasing that only the counted range checks prevent.

**Not covered:**
- **R inside the proven circuits.** The spike's toy circuit cannot hold the R chains, so they are synthesized as
  gadgets on the real shared values. The pairing link itself is in the real proofs: each proof verifies only with
  its partner. (The uniform circuits hold both chains since 2026-11-11: A2-UNIFORM.md, "The pair binding inside
  the circuits".)
- **Modulus aliases.** The canonicity comparisons and lookup range checks are counted, not synthesized.
- **Wrong-width claims** have no explicit test. Fabricated claims are refused.
- **The burn branch's** total verifier.

## Gate 1e: the lean challenge schedule, synthesized (2026-11-08)

Run: `ZKS_SCHEDULE=lean zk-spartan verifier`. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-verifier-lean-v2.log` and
`../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v8-lean.log`. Under `ZKS_SCHEDULE=lean`, the tamper suite (all 8 combinations) and
`verifier::tests` pass, refusals included.

**Verdict: pass.**
- The points side drops 21% below the per-variable schedule, and 12% below the census formula.
- The scalar side is unchanged (+1-4% vs the census).
- About 60 values cross the circuits instead of 100.
- The fitted formula (`census::verify_cost_lean`) reproduces every synthesized count. The scalar side keeps
  about 500 extra rows on purpose: range checks in C_P that the synthesis leaves out.

**The schedule** (`fs::lean_schedule`; the prover, the native verifier and the circuit all follow it):
- τ_i = t^(2^i) for one full-width squeezed word t, which crosses as two limbs.
- Each committed round's k challenges are r^(3^j) for one 126-bit challenge.
- r_A, r_B, r_C = ρ, ρ², ρ³.
- The inner rounds that hold r_sel and the row bits keep per-variable challenges, since the row bits are
  endoscalars for the Hyrax fold.
- A squeeze whose challenges are all raw is range-checked by lookups (about 44 rows) instead of 255 bits; the
  τ word costs about 41.

**Soundness (Schwartz-Zippel degrees, per proof):**
- **τ.** The outer sumcheck reduces "Az∘Bz = Cz on the cube" to F(τ) = Σ_x eq(τ, x)·e(x) = 0, and F is
  multilinear in τ. Under τ_i = t^(2^i), distinct monomials map to distinct powers (binary representation), so
  F(t) is nonzero whenever e ≠ 0. Its degree is below 2^sx, and t is a full scalar: error ≤ 2^20 / 2^254.
- **Rounds.** A committed round polynomial has degree at most 2 per variable. Under x_j = r^(3^j), distinct
  monomials map to distinct powers (base 3; base 2 would let x₁² and x₂ collide). The difference of two round
  polynomials therefore stays a nonzero univariate of degree ≤ 3^k − 1 = 80, so the error is ≤ 80 / 2^126 per
  round. Over the 7-8 Kronecker rounds of a proof that is about 2^−116.8. The per-variable schedule was about
  2^−120: roughly a 3-4 bit loss.
- **r_A, r_B, r_C:** ≤ 3 / 2^126.
- **Unchanged:** α, γ (separation), ξ, ζ, u_j, c, and the kept rounds.
- **Fiat-Shamir** stays REPORT5's stated assumption.

| P 16k / S 16k shapes | Points side: census / per variable / **lean** | Scalar side, lean (census) | Shared: per variable / **lean** |
|---|---:|---:|---:|
| P-send 2^19, 32 × 2^14 | 78,056 / 87,414 / **68,702** | 26,651 (25,536) | 102 / **60** |
| S-send 2^17, 8 × 2^14 | 57,262 / 68,694 / **51,490** | 25,625 (25,084) | 96 / **57** |
| P-recv 2^19, 32 × 2^14 | 78,888 / 88,114 / **69,402** | 31,188 (30,374) | 106 / **64** |
| S-recv 2^18, 16 × 2^14 | 65,528 / 75,796 / **58,592** | 30,370 (30,028) | 103 / **64** |

Where P-recv's points side goes: transcript 31,132 (82 permutations, 32 squeezes, 11 lookup-checked), group
36,000, scalar bits 2,048, on-curve checks 222.

**Fixed points** (Pixel lane commits and the measured IPA, synthesized lean verifier):

| | Send | Receive | P-recv at 2^19: witness / capacity |
|---|---:|---:|---|
| Gate 1c signatures | 2.13 s | 3.13 s | (2^20) |
| Lever 1 comb tables | 1.96 s | 2.60 s | 474,689 / 475,904 (0.26% headroom) |
| **Lever 1 + lookup SHA-256** | **1.89 s** | **2.51 s** | 481,228 / 500,096 (3.8%) |
| Lever 1 + lookup SHA-256 at P 16k / S 16k | 2.08 s | 2.64 s | 467,185 / 500,096 (6.6%) |

- The fastest point is P 16,384 / S 8,192. There P-recv is 1.62 s (commit 0.75, IPA 0.49) and S-recv 0.89 s.
- Pairs are 4,976 / 5,984 B.
- Largest variants (Send+quota / Receive+renewed): 2.08 / 3.19 s.

## The spike's protocol is now recursion-friendly

These changes are needed for the circuit to replay real proofs:
- **Transcript.** An RP57 duplex sponge (rate 2) over the base field of the proof's points.
  - Points are absorbed as (x, y).
  - Scalars are embedded for Vesta proofs (Fp into Fq). For Pallas proofs they are absorbed as 128-bit halves.
  - The public inputs are hashed to one digest in their own field.
  - One squeeze gives two 126-bit challenges.
- **Endoscalars.** Point-multiplying challenges are Halo endoscalars: the row bits r_y[1..=row_bits], γ, ξ, ζ, the
  IPA's u_j and the Schnorr c. The others are the 126-bit integers.
- **Inherited claims in the IPA.** The proof opens P + Σ ξ^(j+1) G_old_j against κ + Σ ξ^(j+1) ⟨s(u_old_j), b⟩.
- **Tamper suite.** All 8 curve and round-size combinations still accept the honest proof and reject every
  tampering and both forgery attacks.

## What the circuit does

**Points side** (C_S for a P-proof, C_P for an S-proof):
- the sponge, with one canonical decomposition per squeeze;
- on-curve checks of every proof point;
- the Hyrax row fold T' = T_lo + ρ(T_hi − T_lo), with R − 1 endoscalings;
- the round commitments by Horner in γ;
- the claims by Horner in ξ;
- (κ'ζ)·U by fixed base, then the IPA fold u(P + uL) + R;
- the Schnorr check z1·G_f + (z1·b_f·ζ)·U + z2·H = A + c·P_fold.

**Scalar side** (C_P for a P-proof, C_S for an S-proof):
- endoscalar values from bits;
- the io digest;
- the round weights and the IPA tail, scaled by γ^−(i+1);
- κ;
- the closed-form b_final and each inherited claim's value;
- the matrix MLE (the toy's, with 8,000 counted for the real circuit);
- io~(r);
- the Schnorr scalars, and z1 = 2c + 2^255 + 1 for the signed-digit form.

**Exception-freeness.** Additions between independently chosen points check both chord-slope equations, as
in the P-256 gadget. Incomplete additions remain only where the inputs provably differ:
- inside endoscaling, by Halo's lemma for one base;
- inside the variable-base multiplication: partial sums stay in [2^i + 1, 3·2^i − 1], and the last three steps
  use complete additions;
- in the fixed-base windows: digits are offset by 4, so partial sums stay below the next entry, and the last two
  windows use complete additions.

The fixed-base gadget in `gadgets.rs` (offset 1) is exceptional for about 1 in 16 scalars, for example digits
3 then 0. Its cost was right, but it is not safe to reuse as is.

## Counts per verified proof

Synthesized on real proofs. Census = gate 1b's `verify_cost` with the 8,000 matrix budget.

| Fixed point | Proof | Points side: synthesized / census | Scalar side: synthesized / census | Shared values: synthesized / census |
|---|---|---:|---:|---:|
| comb16, P 16k / S 16k | P-send 2^19, 32 × 2^14 | 87,414 / 78,056 (+12.0%) | 26,585 / 25,536 (+4.1%) | 102 / 48 |
| | S-send 2^17, 8 × 2^14 | 68,694 / 57,262 (+20.0%) | 25,563 / 25,084 (+1.9%) | 96 / 46 |
| | P-recv 2^19, 32 × 2^14 | 88,114 / 78,888 (+11.7%) | 31,122 / 30,374 (+2.5%) | 106 / 52 |
| | S-recv 2^18, 16 × 2^14 | 75,796 / 65,528 (+15.7%) | 30,309 / 30,028 (+0.9%) | 103 / 51 |
| gate 1c, P 32k / S 16k | P-recv 2^20, 32 × 2^15 | 91,236 / 82,138 (+11.1%) | 32,135 / 31,220 (+2.9%) | 110 / 54 |

P-send in gate 1c's shape is 78,029 / 66,360 (+17.6%). `verify_cost_synth` reproduces every synthesized count
within 7 constraints on the points side and 33 on the scalar side.

Where the points side goes, for P-send 2^19 at 32 × 2^14:
- transcript 49,601: 99 permutations and 50 squeezes;
- group 35,546: 71 endoscalings at 448 and 105 additions at 6;
- the four scalars' bits 2,048;
- on-curve checks 219.

## Why the census was off

1. **Challenges are per variable, and each IPA round takes its own squeeze.** The census assumed one challenge
   for τ and one per round, as Kronecker powers, and two challenges per squeeze. The spike draws one per variable:
   80 challenges and 50 squeezes for P-send, against the census's 36 and 18. Each squeeze costs 243 + 511.
2. **Endoscaling is cheaper than measured, additions dearer.** With existing bits, endoscaling costs 448, not
   583, because the census's measured gadget allocated its own booleans. Additions with both slope equations
   cost 6, not 3.
3. **Full scalars cross as two 128-bit limbs.** Nine full scalars (io digest, v_A, v_B, v_C, κ', κ'ζ, z1's digit
   form, z1·b_f·ζ, z2) cross the circuits. Every challenge crosses too, so C_P pays about twice the binding steps.
   Raw challenges and absorbed-only limbs also need range checks there.

## Effect on the fixed points

Pixel lane commits and the measured IPA (b04a57, PIXEL.md), fastest shape within both byte limits:

| Verifier model | Gate 1c signatures | Lever 1 comb tables (m = 16) |
|---|---|---|
| gate 1b census formula | 2.28 / 3.24 s | 2.06 / 2.62 s (P-recv 2^19) |
| **gate 1d synthesized** | **2.46 / 3.48 s** | **2.37 / 3.29 s** (P-recv 2^20) |
| synthesized, lean schedule (gate 1e, synthesized) | 2.13 / 3.13 s | 1.96 / 2.60 s (P-recv 2^19, 0.26% headroom); with lookup SHA-256 1.89 / 2.51 s (3.8%) |

Largest variants (Send+quota / Receive+renewed): synthesized 2.51 / 3.79 s for gate 1c and 2.41 / 3.51 s for the
comb; with the lean schedule, 2.18 / 3.47 s and 2.07 / 3.17 s. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v6-{census,synth,lean}.log`.

## Levers this measurement exposes

1. **A lean challenge schedule: done, see "Gate 1e" above.** The original estimate follows.
   - Draw τ as powers t^(2^i) of one full-width squeeze (error 2^sx / q).
   - Draw each sumcheck round's challenges as powers of one 126-bit challenge (degree 2(2^k − 1), about 2^−121).
     The inner rounds holding the row bits keep per-variable endoscalars.
   - Draw r_A, r_B, r_C as powers.
   - Range-check raw-only squeezes by lookups (about 50 rows) instead of 255 bits.
   - That is about −0.4 s Send and −0.7 s Receive on the comb (−0.3 / −0.35 s on gate 1c's signatures), and it
     brings the comb's P-recv back to 2^19.
   - It needs the prover change, a re-synthesis and a written soundness note.
2. **Lookup-based SHA-256: synthesized** (OPERATION.md "Lever 5, synthesized", −20k constraints per signature).
   With the lean schedule, the comb's P-recv keeps 2^19 with 3.6-6.5% headroom (1.89 / 2.51 s), where it had
   0.2% before. With today's schedule, P-recv stays at 2^20 (2.38 / 3.30 s).
3. **Endoscalar IPA challenges on the prover.** b04a57 measured that iroha_pasta's fold GLV-splits u into
   ~128-bit halves. The spike's u is now a·ζ + b with ~64-bit a and b. Whether the fold finds the short split by
   itself, or needs (a, b) passed in, is not yet measured.

## Not covered

- **The cross-circuit binding** (masked RLC) is counted at the measured step and the shared-value count, not
  synthesized. So REPORT5's pairing and binding refusals are open: P and S from different operations,
  transplanted binding rows, RLC challenge timing.
- **Canonicity.** Less-than-modulus comparisons are counted (one row per bit), not synthesized. Modulus aliases
  are therefore priced but not rejected by a synthesized constraint.
- **The real circuit's matrix MLE** is the 8,000 budget. The toy's MLE is synthesized at 70-80 rows.
- **The burn branch.** A total (soft-verdict) incoming verifier for proposal 892 is not done.

Refusal cases that are synthesized and rejected by an unsatisfied circuit constraint, not by a harness assertion
(`verifier::tests::synthesis_refuses_tampering`):
- altered row, outer-round, inner-round, L, R, G_f and A points;
- altered v_A, z1 and z2;
- wrong public inputs;
- omitted, duplicated, reordered and fabricated inherited claims;
- the CancelRound and AdaptiveWitness forgeries.
