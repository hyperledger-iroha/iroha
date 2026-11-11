# Gate 1 byte spike: zero-knowledge Spartan with committed sumcheck rounds (2026-11-08)

Historical research record. See [README.md](README.md) for the current scope, reproduction commands and unresolved production gaps.

**Verdict: the byte gate passes.** With research/REPORT4.md's header and inherited-claim lines unchanged:

| Pair | Measured ZK proofs | Header + inherited claims | Total | Limit | Spare |
|---|---:|---:|---:|---:|---:|
| Send | 1,824 + 1,600 | 128 + 544 + 448 | **4,544 B** | 8,458 B | 3,914 B |
| Receive | 2,144 + 1,888 | 128 + 544 + 448 | **5,152 B** | 9,600 B | 4,448 B |

- With 128-bit inherited-claim challenges the totals are 4,080 B and 4,688 B.
- For comparison, research/REPORT4.md's non-ZK count was 8,416 B for Send and 9,120 B for Receive.
- The proof bytes are real serializations. The verifier parses exactly `shape.proof_bytes()` bytes, so nothing reaches it off the wire.

Run it with `CARGO_TARGET_DIR=../../output/kagemusha-100x-20261108/spike/target-zks cargo run --release --offline -- all` from this folder. The log is `../../output/kagemusha-100x-20261108/spike/runs/zks-all-16t.log`.

## Construction

Spartan over the Pasta curves. The primary proof works over Fp with Vesta commitments; the secondary over Fq with Pallas commitments. The code is generic over the curve.

1. **Witness rows (Hyrax).** Each row is committed as `T_r = <w_r, G[0..active)> + rho_r H`. The last `tail` columns of every row are left empty; they carry the round values in the opening.
2. **Outer sumcheck.** It proves `sum_x eq(tau,x) (Az Bz - Cz)(x) = 0`.
   - eq is genuinely factored out.
   - Each round binds k = 4 variables. The prover commits one point to the reduced polynomial's 81 values on `{0,1,2}^4`: `D_i = <t_i, G[slot_i]> + beta_i H`.
   - The round challenges are drawn after `D_i`.
   - The checks never divide by an eq weight, so an eq-weight of zero needs no special case.
3. **Evaluations.** `v_A, v_B, v_C` are revealed. Two isolated multiplication gates with fresh uniform factors mask them: (v_A, v_B) is uniform and v_C is uniform given them, up to ~2/|F|.
4. **Inner sumcheck.** It runs over `z = (w || io)` and commits its rounds the same way.
5. **One linear relation.** It is batched by `alpha` and covers:
   - every round check (Boolean-cube sum against the previous round's value at its challenges);
   - both final checks;
   - the hidden `w~(r_y)`, through head-column weights `-M(r_x,r_y)(1 - r_sel) eq(r_col, .)`.

   No round polynomial and no witness evaluation is revealed.
6. **Separation.** `gamma` is drawn after `alpha`. The IPA opens `P = sum_r eq(r_row,r) T_r + sum_i gamma^i D_i`, and slot i's weights are scaled by `gamma^-i`.
7. **ZK IPA.** It uses Bulletproofs folding: `x' = u x_lo + x_hi`, `b' = b_lo + u b_hi`, `G' = G_lo + u G_hi`.
   - Every L and R carries a fresh blind.
   - A Schnorr final `(A, z1, z2)` replaces the opening `(a, f)`, so no full-length masking vector is committed.
   - The folded generator is sent. Recursion accumulates it as the own generator claim; the spike decides it natively with one public MSM.

The wire per proof is `32 x (rows + outer rounds + 3 + inner rounds + 2 log2(cols) + 4)` bytes.

## Soundness bug found in the first version, and the fix

The first version opened the plain sum `sum_i D_i`. All commitments share one generator basis, and disjoint slots constrain only an honest prover. A later commitment can carry terms in an earlier round's slot or in the witness columns. Two forgeries for an **unsatisfied** witness (one bit set to 2) were accepted by that version, and the regression suite records both as `unseparated aggregate: ACCEPTED`:

- **CancelRound**, the same attack as b04a57's soundness consult §6(b):
  1. Send `D_0 = rho H`, which commits to nothing.
  2. After `r_0`, pick `q_0` whose Boolean sum is the false claim 0 and whose value at `r_0` is the true value.
  3. Send `D_1 = Com_0(q_0) + Com_1(t_1) - D_0`.
- **AdaptiveWitness:**
  - `D_0` carries `J G[col]`, chosen after `tau` so that the tau-weighted R1CS error of `w + J` vanishes.
  - The opened witness then differs from the rows committed before `tau`, and the outer sumcheck checks the wrong witness.

The fix is the separation in step 6.

- **Argument:** rewinding `gamma` at m + 1 points extracts openings of every `T_r` and `D_i`. Then `<x, b'> = kappa` is a Laurent polynomial in `gamma` with exponents in [-m, m].
- **Where the forgery lands:** off-slot terms sit on nonzero exponents. The `gamma^0` coefficient is exactly the relation over each round's own slot and the rows' witness columns.
- **Bound:** at a random `gamma` this costs (2m + 1)/|F| plus extraction.
- **Still to do:** this is an argument, not a written proof. The soundness checklist (research/REPORT-b04a57-soundness.md) asks for one. `alpha` and `gamma` are separate challenges, `gamma` drawn after `alpha`.

## Regression suite

It runs on both curves for k = 1, 2, 3 and 4, on 2^14 constraints and 8 x 1,024 columns. Results:

- The honest proof is accepted.
- Two single-bit flips in every 32-byte element (86-126 per shape) are rejected.
- Truncation and a wrong public input are rejected.
- Three unsatisfied witnesses are rejected: a product, a bit, and a masking-gate product.
- CancelRound and AdaptiveWitness are rejected by the separated construction and accepted by the unseparated one.

## Bytes and Mac timing

These are exact bytes. Timing is on the Mac with 16 Rayon threads and the toy census described in the next section.

| Shape | Constraints | Rows x cols (active, tail) | Rounds out/in | Bytes | Prove (s) | of which open | Verify (s) |
|---|---:|---|---|---:|---:|---:|---:|
| Send primary (Vesta) | 2^20 | 8 x 65,536 (64,726, 810) | 5/5 | 1,824 | 0.370 | 0.228 | 0.034 |
| Send secondary (Pallas) | 2^16 | 8 x 8,192 (7,541, 651) | 4/5 | 1,600 | 0.083 | 0.052 | 0.009 |
| Receive primary | 2^21 | 16 x 65,536 (64,720, 816) | 6/6 | 2,144 | 0.494 | 0.219 | 0.034 |
| Receive secondary | 2^17 | 16 x 8,192 (7,532, 660) | 5/5 | 1,888 | 0.117 | 0.059 | 0.011 |
| Send primary 64 x 8k | 2^20 | 64 x 8,192 (7,382, 810) | 5/5 | 3,424 | 0.297 | 0.056 | 0.010 |
| Send primary 64 x 8k, k = 2 | 2^20 | 64 x 8,192 (8,012, 180) | 10/10 | 3,744 | 0.289 | 0.054 | 0.009 |
| Receive primary 128 x 8k | 2^21 | 128 x 8,192 (7,376, 816) | 6/6 | 5,536 | 0.510 | 0.056 | 0.010 |

These are the short-IPA pairs the critical-path consult proposed (8,192 columns on both fields):

| Pair | Total | Limit | Spare |
|---|---:|---:|---:|
| Send 64 x 8k | 6,048 B | 8,458 B | 2,410 B |
| Send 64 x 8k, k = 2 | 6,368 B | 8,458 B | 2,090 B |
| Receive 128 x 8k | 8,448 B | 9,600 B | 1,152 B |

The short IPAs make the opening about 4x faster (0.228 s → 0.056 s). Sixty-four small row MSMs commit less efficiently than eight long ones (0.057 s → 0.167 s).

Exact bytes by the number of variables per committed round (k):

| Shape | k=1 | k=2 | k=3 | k=4 | k=5 |
|---|---:|---:|---:|---:|---:|
| Send primary | 2,784 | 2,144 | 1,952 | 1,824 | 1,760 |
| Send secondary | 2,368 | 1,856 | 1,696 | 1,600 | 1,568 |
| Receive primary | 3,104 | 2,464 | 2,208 | 2,144 | 2,080 |
| Receive secondary | 2,688 | 2,144 | 1,952 | 1,888 | 1,824 |

Peak RSS for the whole suite, run sequentially and including the 2^21 Receive primary, was 506 MB.

There is a rough Pixel scale. Today's hiding IPA at length 65,536 takes 1.52 s on the Pixel 6 (b04a57, pixel-pasta/RESULTS.md). This spike's whole open phase at that length takes 0.228 s on the Mac, a ratio of about 6.7x. This is not a Pixel measurement.

## What this spike does not show

- **Toy relation.** The R1CS is uniform with about 3 nonzeros per row. It has a 64-bit x 16-bit product, two identity quarters and a bit quarter. Real circuits need the uniform-CCS compiler plus bounded sparse exceptions.
- **Commitment cost understated.** The witness census is narrower than research/REPORT4.md's (220k full-width values in the Send primary), so commit time here is low.
- **Recursion not built.** The header (64 B) and inherited claims (544 / 448 B) are research/REPORT4.md's counts. None of this is built:
  - in-circuit verification of the predecessor and the incoming payment;
  - the CycleFold secondary relation;
  - inherited-claim re-randomization;
  - the recursive census that sizes row counts against the next verifier (the critical-path consult's feedback loop).
- **The tail costs witness capacity.**
  - Send primary: 8 x 64,726 = 517,808 positions, 98.8% of 2^19.
  - Send secondary: 8 x 7,541 = 60,328, 92% of 2^16.
  - k = 2 cuts the tail to 180 columns for +320 B. Alternatively one more row restores capacity.
  - Padded rows must be implicit, authenticated zeros.
- **Transcript.** It uses BLAKE3. Recursion hashes with Pasta Poseidon; byte counts are unchanged.
- **Randomness.** The spike uses a deterministic stream. A wallet draws blinds, masks and Schnorr nonces from its CSPRNG and never reuses a nonce.
- **Secret MSM timing.** The secret MSM is not constant-time (`iroha_pasta/src/msm/pippenger.rs`), the same as today's prover.
- **Unbounded-depth soundness.** Recursion needs a PCD composition theorem checked against this construction, or an explicitly stated recursive Fiat-Shamir assumption as today's spec has.

## Files

| File | Contents |
|---|---|
| `src/fs.rs` | Transcript and spike RNG |
| `src/grid.rs` | Grid extension, Lagrange weights, eq tables; unit test |
| `src/r1cs.rs` | Toy uniform R1CS, O(log) matrix MLEs, masking gates |
| `src/ipa.rs` | ZK IPA |
| `src/spartan.rs` | Prover, verifier, serialization, batched relation, separation, malicious provers |
| `src/main.rs` | Regression suite, benchmarks, pair totals |
