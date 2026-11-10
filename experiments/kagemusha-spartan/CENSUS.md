# Gate 1b: recursive census of the two-curve verifier circuits (2026-11-08)

Historical research record. See [README.md](README.md) for the current scope, reproduction commands and unresolved production gaps.

Run: `cargo run --release --offline -- census` (code: `src/gadgets.rs`, `src/census.rs`, `run_census` in `src/main.rs`). Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-census-v3.log`.

**Verdict: conditional pass.** Inherited claims are folded in clear (owner decision, choice 4).

**The verifier formula below undercounts (gate 1d, VERIFIER.md, 2026-11-08).** Synthesized on real proofs, the
points side is 11-20% higher than `verify_cost`, the scalar side within 1-4%, and about twice as many values cross
the circuits; `verify_cost_synth` reproduces the synthesis and is used with `ZKS_VERIFIER=synth`.

**Superseded in part by gate 1c (OPERATION.md, 2026-11-08).** That census counts the operation's own relation with a lookup argument (171k Send / 297k Receive constraints, against the 559k / 877k reference used here), measures the binding step at 83 constraints instead of 700, and raises the matrix-MLE budget to 8,000. Result: primaries 2^19 / 2^20, secondaries 2^18, pairs 5,056 / 6,624 B, modelled Pixel cores 2.37 / 3.44 s. The tables below are the gate 1b record.

- **Under the construction below, the recursion closes and fits both byte limits.**
  - Primary: 2^20 for Send, 2^21 for Receive, including research/REPORT4.md's operation census as a reference.
  - Secondary: 2^17 to 2^18.
  - Pairs: 4.7-7.2 KB.
  - Modelled Pixel prover cores: about 3.1-3.3 s for Send and 4.7-4.9 s for Receive.
- **Codex round 5 (research/REPORT5.md) disagrees.** Its conservative construction does not close at round 4's 2^16/2^17 secondaries; its widened candidates land at 2^21-2^22 and 7-12 s. The difference comes from five design choices, listed below.
- **Status of those choices.** One is now validated natively; two need a construction or proof first. The operation's own constraint census remains the largest unknown.

## Construction counted

Notation:
- P: the Fp circuit with Vesta commitments.
- S: the Fq circuit with Pallas commitments.
- C_P, C_S: the circuits that prove them.

Operation i's circuits verify the predecessor pair; Receive also verifies the incoming Send pair.

1. **Transcripts are hashed where the proof's points are native.**
   - P's transcript is RP57 over Fq inside C_S. Its points are absorbed as (x, y); its five Fp scalars are embedded, since p < q; its challenges are squeezed there.
   - S's transcript is RP57 over Fp inside C_P.
   - Only about 50 values per verified proof cross circuits: challenges, κ', the Schnorr fixed-base scalar, the five proof scalars, a public-input digest, and four limbs per inherited claim point.
   - No points cross. Round 5 instead hashes in the scalar field, which moves every point across as limbs (676-1,652 limbs per source).
2. **Point-multiplying challenges are 128-bit endoscalars,** as Halo 2 does. This covers the row-bit challenges, γ, ζ, ξ, the IPA u_j and the Schnorr c. Each costs a 583-constraint endoscale.
   - The row combination is a tensor fold: R − 1 endoscales.
   - Round commitments use Horner in γ.
   - The IPA fold is `u(P + uL) + R`: two endoscales per round.
   - Only the Schnorr `z1·G_f` is a full 255-bit variable-base multiplication, plus three fixed-base terms.
   - Round 5 prices every coefficient at full width, 1,555 each.
3. **The scalar verifier uses the closed form**
   `b_final = -coef·(Π_j(1 - r_j + r_j u_j) - Π_high(u_j r_j)·Σ s_low·eq_low) + Π_high(u_j)·Σ s_low·b_tail`,
   which is O(log C + 2^t), not the literal O(C) fold (3C - T - 2 ≈ 196k at 65,536 columns).
   - **Validated:** `verify` asserts this equals the O(C) fold on every proof. It holds on the whole regression suite (both curves, k = 1..4) and all seven benchmark shapes.
4. **Inherited claims: folded in clear (owner decision, 2026-11-08).** b04a57 relayed the owner's words: they first chose "Keep unlinkable", then wrote "we don't really care about linkable/unlinkable tbh"; the later message stands, so speed wins. Private masked restructuring and the fused masked opening are dropped. Soundness is unchanged (every claim is still decided); the ZK theorem's leakage function must name the ancestry tag (research/REPORT-b04a57-soundness.md:44). The analysis that led to the decision follows. These are b04a57's points. Today's construction re-randomizes with create_restructure's committed-mask opening, which keeps payment histories unlinkable across wallets. Folding the claims into the own opening in clear lets anyone who holds two proofs test which earlier payment fed which later one. For example, a payer holding the receiver's Receive proof could recognize the receiver's later Send. `payer_wallet_id` reveals only the direct counterparty. The spec leaves linkability open (C12) rather than ruling it out. Both options are priced (`zk-spartan census`, log `../../output/kagemusha-100x-20261108/spike/runs/zks-census-v4.log`):

   | P 32,768 / S 8,192, Rlc | Send core | Receive core | Pairs | Circuits |
   |---|---:|---:|---:|---|
   | Fold in clear (links histories) | 3.09 s | 4.65 s | 5,600 / 7,168 B | secondary 2^18 / 2^18 |
   | Private masked restructuring (today) | 4.26 s | 6.25 s | 5,600 / 7,680 B | secondary 2^18 / 2^19; primary +99k / +97k |

   What restructuring costs:
   - **Proving:** 1.17 s per operation for one restructuring IPA per curve. That is 0.84 s for Vesta at 32,768 columns and 0.34 s for Pallas at 8,192, each including its full-width mask commitment.
   - **Circuits:** the in-circuit verification adds +35k / +33k constraints to the secondary and +99k / +97k to the primary.
   - **Native decisions:** two more natively decided claims, about +0.18 s.
   - **Receive's secondary:** it crosses 2^18 by 6k constraints. Not decomposing the non-endoscalar challenges would bring it back under, to about 5.9 s.
   - **Fused masked opening:** no sound construction yet. A free mask point lets the prover cancel G_old, and the own circuit cannot compute its own proof's opening point. It could save most of the 1.17 s, but nothing is booked.

   With claims folded in clear, each proof opens `P + Σ ξ^j G_old_j` against `κ + Σ ξ^j <s(u_old_j), b>`, using the same closed form. Its soundness argument is still unwritten.
5. **κ stays in-circuit.** κ contains a random combination of the predecessor's public inputs (the previous payment's data), so the deferred checks cannot go to the payee.
   - The two circuits bind their shared values with a **masked random linear combination over Fq**: native in C_S, non-native Horner in C_P (budgeted at 700 constraints per value, an estimate). It costs zero bytes.
   - Alternative: transmit the public-safe challenges and commit κ. That adds 1.6 KB to Send and 3.1 KB to Receive; Receive then reaches 9,360 B, barely under its 9,600 B limit.

## Measured gadget costs

Each gadget is synthesized on real values, every constraint is checked, and the output matches iroha_pasta.

| Gadget | R1CS constraints |
|---|---:|
| 128-bit endoscale, variable base (Halo algorithm 1) | 583 |
| 255-bit variable-base multiplication (signed digits) | 1,782 |
| 255-bit fixed-base multiplication (2-bit windows) | 768 |
| Incomplete affine addition / on-curve check | 3 / 3 |
| RP57 permutation (t = 3, x^5, 8 + 57 rounds) | 243 |
| Canonical squeeze decomposition (two 127-bit challenges) | 511 |

The less-than-modulus part of the squeeze decomposition is counted as one constraint per bit, not synthesized.

## Verifier work per proof

At P 8 x 65,536 and S 8 x 8,192:

| Verified proof | Points-native circuit | Scalar-native circuit | Shared values |
|---|---|---|---:|
| P (Send) | C_S: transcript 30,307 + group 39,303 | C_P: 20,328 | 50 |
| S (Send) | C_P: transcript 27,077 + group 35,201 | C_S: 19,075 | 46 |

## Fixed points

The fixed points are computed over all four orders (Send/Receive after Send/Receive), with the Rlc binding.

- Each circuit is sized for the larger predecessor.
- The primary adds research/REPORT4.md's whole-relation census (559,072 Send / 876,608 Receive) as a reference.
- Constraints are next powers of two, and rows are sized to the witness after the tail.
- Pixel time is a prover-core model fitted to PIXEL.md:
  - outer sumcheck 0.26 µs per constraint;
  - inner sumcheck 0.2 µs per z entry;
  - commitments 0.68-1.1 µs per entry;
  - IPA 0.17 s + 18.3 µs per column.

| P cols | S cols | Send C_P / C_S overhead | Receive C_P / C_S | Shapes P-send, S-send, P-recv, S-recv | Send pair | Receive pair | Pixel Send / Receive |
|---:|---:|---|---|---|---:|---:|---:|
| 65,536 | 8,192 | 179k / 109k | 329k / 198k | 2^20 16 rows, 2^17 16 rows, 2^21 32 rows, 2^18 32 rows | 4,656 B | 6,240 B | 3.30 / 4.92 s |
| 32,768 | 8,192 | 178k / 134k | 344k / 235k | 2^20 32, 2^18 32, 2^21 64, 2^18 32 | 5,600 B | 7,168 B | **3.09 / 4.65 s** |
| 65,536 | 16,384 | 167k / 110k | 313k / 198k | 2^20 16, 2^17 8, 2^21 32, 2^18 16 | 4,480 B | 5,824 B | 3.43 / 5.02 s |
| 16,384 | 16,384 | 181k / 187k | 334k / 312k | 2^20 64, 2^18 16, 2^21 128, 2^19 32 | 6,112 B | 9,216 B | 2.87 / 4.79 s |
| 8,192 (critical-path proposal) | 8,192 | — | — | the secondary reaches 2^19 | 9,536 B | 14,144 B | fails the byte limits |

- **Fastest within both limits:** P 32 x 32,768 with S 32 x 8,192. Its Send secondary sits just over 2^17 (134k); not decomposing the non-endoscalar challenges (about 13 per proof, ~3k) would bring it back under.
- **Short primary IPAs fail.** 64-128 rows grow the next secondary through the row fold and the transcript. This is the critical-path consult's feedback loop, now quantified.

## Disagreement with Codex round 5, by cause

| Choice | This census | Round 5 | Effect on the secondary |
|---|---|---|---|
| Transcript placement | Group-native; ~50 values cross | Scalar-native; hundreds of limbs cross, and S recomputes a row commitment over them (448 per limb) | Round 5 adds 0.3-0.7M |
| Point coefficients | 128-bit endoscalars (583) | Full width (1,555) | ~2.7x on group work |
| Scalar verifier | Closed form, validated | Literal O(C) | ~196k per proof in round 5 |
| Inherited claims | Joint opening | Private restructuring IPAs | ~110k per circuit in round 5 |
| Canonicality | Per squeeze | +78k allowance per source pair | 78k in round 5 |

## Still open

- **The operation's own constraints.** research/REPORT4.md counts witness values, not R1CS constraints (round 5 is right). Without lookups, R1CS range checks cost one constraint per bit: naively ~3.8M for Send's bounded values. A lookup argument in the committed-round sumcheck (for example LogUp-GKR) is likely required. This is the largest unknown.
- **The non-native Horner step** (700 per value) is estimated, not synthesized.
- **Endoscalar challenges** are a protocol choice. The native prover and verifier must use the same mapping, as Halo 2 does.
- **Written proofs** are owed for the γ separation, the joint opening and the masked binding.
- **PIXEL.md's commit note is wrong.** It says "the commit phase here is full-width", but the spike's witness uses public width classes: 64-, 16-, 80- and 1-bit rows plus two full-width masking columns.

## Next falsifying measurement

**Count-only synthesis of the in-circuit verifier.** Synthesize one P-proof and one S-proof with choices 1-5 in the gadget builder (`src/gadgets.rs`), satisfied by real proofs from this spike.
- **Pass:** P-verify ≤ 75k in C_S, S-verify ≤ 70k in C_P, and every refusal case in research/REPORT5.md's list rejected.
- **Then:** the operation census with a lookup argument.
