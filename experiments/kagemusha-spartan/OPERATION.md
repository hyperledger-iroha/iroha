# Gate 1c: the operation's own constraints (2026-11-08)

Historical research record. See [README.md](README.md) for the current scope, reproduction commands and unresolved production gaps.

Run: `zk-spartan operation` (code: `src/operation.rs`, `src/p256.rs`, `run_operation` in `src/main.rs`).
Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v4.log` (after b04a57's review, research/REPORT-b04a57-census-review.md; v3 was before it). The old `census` mode is unchanged (`../../output/kagemusha-100x-20261108/spike/runs/zks-census-v5.log` equals v4).

**Verdict: pass.** With a lookup argument, the Send and Receive relations are about a third of
the reference that gates 1 and 1b used. Both primaries drop one size class: Send to 2^19 and
Receive to 2^20. The modelled Pixel prover cores are 2.4 s (Send) and 3.4 s (Receive). Before
this, the cores were 3.1 / 4.7 s.

| | Send | Receive |
|---|---:|---:|
| Own relation, constraints with LogUp | 171k | 297k |
| Gate 1b reference (REPORT4 census) | 559k | 877k |
| Primary circuit (own + recursion overhead) | 2^19 | 2^20 |
| Secondary circuit | 2^18 | 2^18 |
| Pair bytes (limits 8,458 / 9,600) | 5,056 | 6,624 |
| Modelled Pixel prover core | **2.37 s** | **3.44 s** |
| Largest variant (Send+quota / Receive+renewed) | 2.43 s | 3.86 s |

The REPORT4 reference already included recursion work, so the gate 1b fixed point counted
recursion twice.

## Update, evening: lever 1 and the verifier both synthesized, Pixel costs adopted

Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v6-{census,synth,lean}.log` (operation mode with `ZKS_VERIFIER=synth`,
`ZKS_SCHEDULE=lean`); verifier synthesis in VERIFIER.md (gate 1d).

Fastest shape within both byte limits, Send / Receive, with Pixel lane commits and the measured IPA
(b04a57, PIXEL.md):

| Verifier model | Gate 1c signatures | Lever 1: own key over comb tables | Lever 1 + lookup SHA-256 (lever 5) |
|---|---|---|---|
| Gate 1b census formula | 2.28 / 3.24 s | 2.06 / 2.62 s | 1.88 / 2.50 s |
| **Gate 1d synthesized (real proofs)** | **2.46 / 3.48 s** | **2.37 / 3.29 s** | **2.38 / 3.30 s** |
| **Synthesized, lean challenge schedule (gate 1e)** | **2.13 / 3.13 s** | **1.96 / 2.60 s** | **1.89 / 2.51 s** |
| Same, with the pairing link costed (gate 1f) | 2.13 / 3.14 s | 2.07 / 2.63 s | **1.89 / 2.52 s** |
| **Same, with the Pixel IPA after the GLV fix (current)** | **1.93 / 2.95 s** | — | **1.68 / 2.31 s** |

IPA after the GLV fix (b04a57, iroha `8ca1643b3f`). With endoscalar challenges the Pixel medians are 0.216 /
0.332 / 0.525 / 0.929 s at 8k / 16k / 32k / 64k columns. That is L/R MSMs, fold and the rest; the model adds a
0.01 s allowance for the transcript and the final Schnorr step. The earlier full-width table was 0.283 / 0.493 /
0.855 / 1.473 s, and `ZKS_IPA=fullwidth` reproduces it. Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v12-ipa-glv.log`.
- At P 16k / S 8k, an ordinary Receive is P-recv 1.47 s (commit 0.76, IPA 0.34) plus S-recv 0.84 s.
- Largest variants with today's renewal rule: 1.87 / 2.99 s. With credential continuity (committed, `00fc09fcea`):
  1.73 / 2.32 s.

Lookup SHA-256 columns: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v7-{census,synth,lean}.log`.

Largest variants (Send+quota / Receive+renewed), synthesized: 2.51 / 3.79 s and 2.41 / 3.51 s.

- **Lever 1 works on its own terms:** Send's relation shrinks 28% and Receive's 31%. Its time saving depends on
  P-recv fitting 2^19. Under the census formula it does, with 1.4% witness headroom: −0.22 / −0.62 s. Under the
  synthesized verifier it does not: −0.09 / −0.19 s.
- **The verifier synthesis is the larger correction:** +0.18 / +0.24 s on gate 1c's signatures and +0.31 /
  +0.67 s on the comb (VERIFIER.md).
- **Below 3 s for an ordinary Receive:** the lean challenge schedule is synthesized (gate 1e, VERIFIER.md). The
  comb with lookup SHA-256 reaches 1.89 / 2.51 s, with P-recv at 2^19 and 3.8% headroom (6.6% at P 16k / S 16k,
  2.08 / 2.64 s). The largest variant, Receive+renewed, is 3.19 s.

### The renewal path: credential continuity brings the renewed Receive under 3 s

Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v10-renewal.log` (synthesized lean verifier with the pairing link, comb tables,
lookup SHA-256).

The largest Receive variant is the one where the Request quotes the receiver credential from before a renewal.
Under R10 (lean lineage, owner decision 2026-10-17), Λ_recv then re-verifies that quoted credential: the issuer
signature with a variable key, plus the issuer certificate under the root. That is two more signed messages,
and they push P-recv to 2^20. One circuit per operation type is sized for its largest variant, so every Receive
pays for it.

| Renewed-Request handling | Own relation | Send / Receive (largest variants) |
|---|---:|---:|
| Today: re-verify issuer signature and certificate | 281,514 | 2.08 / 3.20 s |
| Issuer key over fixed comb tables, certificate verified | 236,093 | 2.08 / 3.09 s |
| Issuer keys as circuit constants, certificate checked at circuit build | 204,338 | 2.08 / 3.02 s |
| Credential continuity, last four digests in the core (bounded) | 172,018 | 1.94 / 2.52 s |
| **Credential continuity, depth-16 prior-credential array, root in the core (drafted)** | **175,834** | **1.94 / 2.53 s** |
| Credential continuity, the same array with its root in the rest digest | 180,370 | 1.94 / 2.54 s |
| (Ordinary Receive, for reference) | 168,482 | 1.89 / 2.52 s |

- **Applied to the spec: iroha `00fc09fcea`** (owner: "Yes, draft spec change", then "Apply and commit"). Rationale:
  `../../research/SPEC-CHANGE-credential-continuity.md`. The draft takes a depth-16 prior-credential array indexed by
  `renewal_sequence`, with its root in the core. Unlike a window of four, it keeps a Request receivable after any
  number of renewals (up to 65,536), for +0.01 s. Log: `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v11-continuity.log`.
- **The continuity rule.** The quoted pre-renewal credential was already verified in-circuit, signature chain
  included, by the step that introduced it (Bootstrap or RefreshCredential).
  - The state keeps the digests of the wallet's last four credentials (RefreshCredential shifts them).
  - Λ_recv computes the quoted credential's digest as today, opens that window (a depth-2 opening), and binds
    wallet_id, payment_key and the receiver account digest from it by continuity. No signature is verified.
  - This applies the lean-lineage principle (verify a credential once, carry it by continuity) to the one case
    that still re-verifies.
- **Effect.** P-recv stays at 2^19 with 2.5% headroom (P 16k / S 8k; 5.4% at P 16k / S 16k for 2.66 s). Every
  variant is then under 3 s: Send 1.89-1.94 s, Receive 2.52 s.
- **What it changes.**
  - The spec: R10 and the Λ_recv bullet in §3.2 ("a Request's renewed receiver credential").
  - A state field holding the last four credential digests.
  - A Request quoted before more than four renewals stops being receivable. Leases are disabled by default (R8).
  - Native verification by every consuming party (R10) is unchanged.
  - This is an owner decision.
- **The issuer-table options stay over 3 s.** Making issuer keys circuit constants also changes the trust anchor:
  a new issuer key would need a circuit update.

### Lever 1, synthesized: own-key ECDSA over comb tables

With the wallet's tables precomputed, the circuit does not need R' recovery or half-size scalars.
- It computes P = u1·G + u2·Q directly and checks x(P) = r + flag·n with x(P) < p.
- Each scalar is written u = 2B − (2^256 − 1) (mod n) with B = (u + 2^256 − 1)/2 mod n. That gives signed
  one-bit digits: no zero digit and no parity condition.
- The 256 digits split into m chunks over the bases B_k = 2^(256k/m)·X.
- Each table combines 8 bases: entry(idx) = Σ_k (2·bit_k(idx) − 1)·B_(8j+k).
- One loop of 256/m windows then costs a double-add and m/4 − 1 additions per window, with both slope
  equations at every addition.

Measured (`operation::tests`):

| m (bases) | Checks per signature | ECDSA constraints with LogUp (chunk 11) | Per-wallet table entries | Bootstrap, one time |
|---:|---:|---:|---:|---:|
| 8 | 321 | ~31k | 256 | 156k constraints |
| **16** | **289** | **~28.5k** | **512** | **253k constraints** |
| 32 | 273 | ~28k | 1,024 | 441k constraints |

Gate 1c's variable-key ECDSA is 821 checks and 74k constraints. The pass gate (≤ 400 checks) holds at every m.
m = 16 ties m = 32 on time at about half the Bootstrap cost. Send and Receive must share one m, since a wallet
has one table set.

| Relation (m = 16, chunk 11) | Gate 1c | Lever 1 |
|---|---:|---:|
| Send | 170,540 | 122,130 |
| Receive | 297,130 | 206,184 |
| Receive+renewed (issuer key stays variable; root over fixed comb tables) | 489,863 | 358,966 |

**Requirements:**
- Q never appears in the per-operation circuit.
- The Bootstrap proof derives the tables from Q: on-curve check, 2^(16k)·Q by doublings, each table along a Gray
  code. Enrollment binds Q to the attested key.
- The table rows are dedicated and unblinded. Their commitment is fixed at Bootstrap, so there is no commitment
  work per proof.
- Every verifier compares it with the wallet's state. That is two permutations per state and a four-limb compare
  per verified P-proof, counted.
- The incoming Send's statement must expose it.

**Linkability:** the unblinded table commitment is a per-incarnation constant, a function of Q. Anyone who
verifies the proofs can link a wallet's proofs to each other, and holders of Q can link them to Q. That fits the
owner's "we don't really care about linkable/unlinkable". A fixed per-incarnation blind would hide Q but not the
linkage.

**Tests:**
- valid own-key and root signatures accepted for m = 8/16/32, with exactly the predicted check counts;
- a signature on another digest, or with a wrong r, leaves a constraint unsatisfied.

### Lever 5, synthesized: SHA-256 with spread lookups

One fixed table of (d, spread(d)) for d < 2^11 (2,048 entries) serves two purposes: chunk lookups, and range
checks of narrower pieces (looking up d·2^(11−b) with its spread). A sum of up to three spread words has base-4
digits 0..3. Splitting it into even bits (XOR) and odd bits (AND, majority) is therefore unique: one linear row
plus three chunk lookups per half.
- Σ0, Σ1, σ0, σ1 decompose the word at their rotation and shift cuts.
- Maj and Ch are odd halves of spread sums. Ch is the sum of two of them, which never overlap.
- Additions mod 2^32 allocate the result with boolean carries. Each result is range-checked where it is next
  decomposed.
- Message words are linear in the input's canonical bits. The output words stay bits for the ECDSA digest.
- Test (`lookup_sha256_matches_native`): equals native SHA-256.

| One 32-byte SHA-256 block | Bit-level (gate 1c) | Spread lookups |
|---|---:|---:|
| Rows | 25,976 | 2,709 |
| Lookups | 0 | 3,392 (fixed table, shared per operation) |
| Constraints per signature, tables excluded | 25,976 | 6,101 |

- **Own relations with lever 1:** Send 122,130 → 104,303 and Receive 206,184 → 168,482. Gate 1c's are 170,540
  and 297,130.
- **Per entry it is not cheaper.** Each lookup adds a full-width LogUp helper, where the bit-level gadget's
  entries are 1-bit. The lever pays only when it moves a proof down a size class:
  - census formula: P-recv keeps 2^19 with 2.9-6.3% headroom (was 1.4%), and Send's secondary drops a class:
    1.88 / 2.50 s;
  - synthesized verifier, today's challenge schedule: P-recv stays at 2^20, about 51k entries over, so no gain
    (+0.01 s);
  - lean schedule (estimate): P-recv at 2^19 with 3.6% headroom (6.5% at P 16k / S 16k), 1.89 / 2.51 s. Without
    lookup SHA-256, the headroom was 0.2%.
- On gate 1c's signatures it costs 0.01-0.02 s, since no size class changes.

### Fixed-point search

Gate 1c started the iteration at 2^19 / 2^20 primaries, which finds the largest stable shapes. Every size grows
with the predecessor's shape, so starting small finds the least stable (self-consistent) shapes. The run now
tries both starts and keeps the smaller.
- For gate 1c this changes nothing at the fastest shape.
- For lever 1 it finds P-recv at 2^19, which the high start missed (3.18 s at the high start vs 2.75 s under
  the model costs).

### Commit and IPA costs

The Pixel calibration replaces the model by default:
- lane commits, 16-row batches;
- the measured IPA at 8k-64k columns.

`ZKS_COMMIT=rowloop` gives the per-row loop's calibration, and `ZKS_COMMIT=model` / `ZKS_IPA=model` give gate 1c's
model. Under the model, `ZKS_COMMIT=model ZKS_IPA=model` with the census verifier reproduces 2.37 / 3.44 s for
gate 1c (`../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v5-model.log`).

## What each relation proves

Path 2's completed proof is the step relation plus that step's lineage checks: σ and Λ for one
operation. The sources are `kagemusha_single_design_proposal.md` §§3.1-3.2 and the
`kagemusha_wallet_wire_v1.md` rows for Request, Payment and the receipt.

| Component | Send | Receive | Receive+renewed |
|---|---:|---:|---:|
| P-256 ECDSA with SHA-256 of the signed 32-byte digest | 1 (τ_send) | 2 (τ_recv, Request) | 3 + 1 fixed-key (credential, certificate) |
| RP57 permutations outside trees | 118 | 117 | 133 |
| Depth-32 indexed-tree insertions (4 paths, 6 leaves) | 1 (pending outgoing) | 2 (consumed credit, credit digest) | 2 |
| Full-field order comparisons | 2 | 4 | 4 |
| u128 range checks | 12 | 10 | 10 |
| Field elements re-encoded as bytes (`P_bytes`) | 0 | 5 (Payment transcript) | 5 |
| Selector, equality and branch rows | 2,000 | 3,000 | 3,500 |

Send+quota adds 4 touched quota windows: 84 permutations and 12 range checks.

Outside the trees, the permutations are:
- the predecessor and successor heads, P(core(33) ‖ rest digest), 17 each;
- the statement, 10;
- operation_id, 3;
- the Request's signed terms m = P(19), credit_id = P(24) and request_digest = P(5): 10 + 12 + 3;
- the chain append, 4;
- the receipt message, 6;
- the credential opening behind the account-digest checks, 12 (b04a57's review; proposal 804-807).

Send also opens the rest digest for its fee policy (20, an estimate) and the fee schedule (5).
Receive also computes the Payment digest over the 163-byte transcript (4) and the package digest
(4).

Signatures:
- Send verifies only its own receipt, because the Request and receiver credential are checked
  natively before Advance (§3.2).
- Receive no longer re-proves τ_send, since the Payment carries the completed Send pair.
- Every signature signs the 32-byte encoding of a Poseidon digest, so each costs one SHA-256 block.

## Measured gadgets (each synthesized on real values, every constraint checked)

| Gadget | Rows | Lookups | Notes |
|---|---:|---:|---|
| Non-native check a·b = r (mod p), r fresh, plus allocating a and b | 9 | 119 at 13-bit chunks | 5 evaluation products, 3 carry rows, 1 native row |
| ECDSA P-256, variable key | 9,540 | 56,306 at 13 bits | 821 non-native checks, both slope equations at every addition |
| ECDSA P-256, fixed key | 9,128 | 53,971 | no Q table, no on-curve check |
| SHA-256 of a 32-byte digest, with canonical bits | 25,976 | 0 | bit-level; constant padding folded |
| Cross-circuit binding step (Horner over Fq in C_P) | 9 | 74 | 83 constraints with LogUp; gate 1b assumed 700 |
| RP57 permutation | 243 | 0 | from gate 1b |

The non-native representation is three 88-bit limbs. A product is checked over the integers by a
carry chain mod 2^264 plus one native row mod Fp, and the code asserts the CRT bound
|E| < 2^264·Fp/2. The quotient, the carries and every fresh value are range-checked in chunks.

ECDSA uses three techniques:
- **R recovery.** x(R') = r + flag·n, checked over the integers, with R' on the curve.
- **Half-size scalars.** Lattice reduction gives v = v'·u2 (mod n) with |v|, |v'| ≤ 2^130, both
  odd, and the circuit checks v'·R' − v·Q − w·G = O. That halves the doublings.
- **One Straus loop.** It has 33 windows of 4-bit odd digits. R' and Q use 16-entry dynamic
  tables (± odd multiples). G and 2^128·G share one fixed 256-entry table, so each window costs
  4 doublings and 3 additions.

Every addition is asserted against native P-256, and the final point against −K·G.

### No free slope (exception-freeness)

b04a57's review found that the first version used one chord equation per addition. If the two
inputs are the same point, λ·0 = 0 leaves λ a free witness. The prover picks R' through r, and
R' = αQ + βG with known α and β puts every accumulator and table point in a basis the prover
controls. So a coincidence can be forced at any chosen addition, in later windows too, not only
the first.

Every addition (and the first slope of each double-add) now checks both chord-slope equations
(`slope_checks`):

    λ (x_c − x_a) = y_c − y_a
    λ (y_a + y_c) = x_a² + x_a x_c + x_c² − 3

- The first equation pins λ unless x_a = x_c. Then it forces y_a = y_c, and the second pins λ to
  the tangent, since y ≠ 0 on P-256 (prime order, no 2-torsion).
- c = −a is unsatisfiable rather than free.
- Doublings pin λ through 2λy = 3x² − 3 (y ≠ 0).
- The second slope of double-add pins λ₂ through (λ₁ + λ₂)(x_acc − x_r) = 2y_acc.

**Soundness.** By induction, every intermediate is the exact group operation on curve points.
The loop therefore computes v'R' − vQ − wG exactly. With the range and canonical checks, the
circuit accepts exactly what native ECDSA accepts.

**Completeness.** An honest prover fails only if some step adds a point to its negation, which
has negligible probability. The prover checks this natively and re-chooses the lattice vector if
it happens.

**Tests** (`operation::tests`):
- a forced equal-point addition computes the doubling;
- double-add with equal inputs computes 3·acc;
- wrong slopes, for equal and for distinct points, are rejected.

**Cost:** 112 more checks per signature (+13%).

The second equation relies on both inputs being on the curve. That holds by induction: R' and Q
are checked on-curve, and table entries come from pinned operations or constants.

**Fallback, not adopted.** b04a57 estimates how hard a coincidence is to force at window k.
r = x(aQ + bG) feeds u1 and u2, and so the digits. A prover who fixes (a, b) from a chosen digit
prefix must then grind s until about 16k prefix bits match (v' and v at 4 bits per window, w at
8), with about 64 decompositions per s. That is roughly 2^(16k−6) work: windows 1-3 are
reachable, but window 8 onward is beyond 2^122.

Pinning the second equation only in the top 10 windows, plus the structurally safe tables,
would recover about two thirds of the +112 checks: about 7k constraints per signature. That
rests on a grinding argument (digit pseudo-randomness, the decomposition count), so it stays a
fallback, behind lookup-based SHA-256, with the argument written down if it is ever used.

## Lookup argument

LogUp with committed inverse helpers, in a second witness phase. The main rows are committed,
then α and β are drawn, then the helper rows are committed.
- Each lookup adds one row, h·(α − f) = 1, and one full-width helper.
- Each table entry adds the same, plus a small multiplicity.
- One linear row checks the sum.
- No extra sumcheck rounds or proof bytes are needed. The verifier evaluates α-dependent matrix
  entries, which is cheap on the scalar side.

Phases (soundness needs this order):
- Phase 1 holds the witness, the range chunks, the multiplicities and the dynamic table entries
  (the computed R' and Q multiples), all committed before α and β. Multiplicities chosen after α
  would make the sum forgeable.
- Phase 2 holds h_i = 1/(α − f_i) and g_j = m_j/(α − t_j), so the sum row stays linear.
- Each phase occupies whole Hyrax rows. The fixed point reserves one spare row, and the proof
  bytes count every row (32 B each).

Tables:
- A range table of 2^b entries. Partial chunks take two lookups: the chunk and its left shift.
- Dynamic 16-entry tables for R' and Q.
- The fixed 256-entry table for G and 2^128·G.
- A value table for wiring between uniform blocks, which keeps the matrix MLE succinct for the
  recursive verifier. It is estimated at 3 copy lookups per non-native check and 6 per
  permutation.

The chunk width is swept from 10 to 16 bits; Send is fastest at 11 and Receive at 13.

**LogUp-GKR is rejected.** It would save Receive's helper commitments (about 0.29 s), but it
adds about 60 committed rounds (about 2 KB) per proof. That puts about 54k constraints into
every secondary that verifies the proof, and Receive's secondary then reaches 2^19. The two
effects cancel.

## Totals per relation (best chunk width)

| Relation | Constraints | Witness | Lookups | Table entries | 1-bit | 2-20 bit | 71-180 bit | Full width |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Send (11-bit chunks) | 170,540 | 228,839 | 67,004 | 4,326 | 25,603 | 63,393 | 6,841 | 133,002 |
| Send+quota | 192,864 | 250,547 | 67,652 | 4,578 | 25,603 | 63,789 | 6,841 | 154,314 |
| Receive (13-bit chunks) | 297,130 | 401,246 | 120,678 | 12,085 | 52,762 | 111,743 | 13,682 | 223,059 |
| Receive+renewed | 489,863 | 688,259 | 235,878 | 14,610 | 105,319 | 211,182 | 27,084 | 344,674 |

ECDSA dominates: it is 74k constraints per signature with LogUp. Next come RP57 (61k for Send,
94k for Receive) and SHA-256 (26k per signature). About half the full-width entries are LogUp
helpers.

## Fixed point with this census

Construction:
- Inherited claims are folded in clear, the owner's decision.
- Binding is the masked RLC at the measured 83 constraints per value.
- The matrix-MLE budget is 8,000 per verified proof; gate 1b used 2,000.
- Endoscale, squeeze and scalar-multiplication bits are committed as 1-bit entries.

All four operation orders, P columns 8,192-65,536, S columns 4,096-16,384:

| P / S cols | Send C_P / C_S overhead | Receive C_P / C_S | Shapes | Pairs | Pixel Send / Receive |
|---|---|---|---|---:|---:|
| 16,384 / 16,384 | 105k / 137k | 203k / 241k | P-send 2^19 32 rows, S 2^18 16, P-recv 2^20 64, S 2^18 16 | 5,056 / 6,624 B | **2.37 / 3.44 s** |
| 32,768 / 8,192 | 117k / 112k | 213k / 204k | 2^19 16, 2^17 16, 2^20 32, 2^18 32 | 4,544 / 6,112 B | 2.38 / 3.48 s |
| 16,384 / 8,192 | 144k / 137k | 254k / 241k | 2^19 32, 2^18 32, 2^20 64, 2^19 64 | 5,488 / 8,064 B | 2.34 / 3.62 s |
| 65,536 / 8,192 | 118k / 100k | 213k / 187k | 2^19 8, 2^17 16, 2^20 16, 2^18 32 | 4,368 / 5,696 B | 2.90 / 3.95 s |

Breakdown of the 16,384 / 16,384 point, in seconds:

| Proof | Outer | Inner | Commit (of which full width) | Density | IPA | Total |
|---|---:|---:|---:|---:|---:|---:|
| P-send | 0.14 | 0.21 | 0.60 (0.55) | 0.03 | 0.47 | 1.44 |
| S-send | 0.07 | 0.10 | 0.28 (0.28) | 0.00 | 0.47 | 0.93 |
| P-recv | 0.27 | 0.42 | 1.08 (0.98) | 0.05 | 0.47 | 2.29 |
| S-recv | 0.07 | 0.10 | 0.50 (0.48) | 0.01 | 0.47 | 1.15 |

Full-width commitments and the four IPAs are about two thirds of each operation.

## What is modelled or estimated

- **Commit cost per width class** (`commit_us` in main.rs). These are anchors from b04a57's
  2^18 kernel, scaled to each row length with the measured spike mix (0.72 µs at 65,536,
  1.13 µs at 8,192). Full width is 2.04 µs at 65,536 columns, scaled down to shorter rows by the
  MSM planner.
  - `zk-spartan commit-classes` measures it directly.
  - Mac reference: `../../output/kagemusha-100x-20261108/spike/runs/zks-commit-classes-mac-16t.log`, run at load 21, so noisy.
  - The Pixel run is the calibration that counts.
- **Wiring:** 3 copy lookups per non-native check and 6 per permutation. The real count depends
  on the block layout.
- **Matrix-MLE budget:** 8,000 per verified proof, assuming about ten uniform block types and
  preprocessed index columns.
- **Counted parts:**
  - rest-digest size (20 permutations);
  - logic rows (2,000-3,500);
  - comparisons, at 5 canonical chunk sets each.
- **Protocol choice (a wire change at wire:1134):** Path 2's proof digest and package digest
  are taken over the verified pair's transcript digest. They are not P_bytes over the proof
  bytes, which would add about 100 permutations and 6k byte lookups to Receive.
  - The transcript digest must absorb both proofs' z1 and z2; the spike's IPA returns them
    unabsorbed (ipa.rs:116-119). That is one permutation.
  - It must also bind both proofs, the statement, the version and the lengths.
- **Not included:**
  - the preparation proof before Advance (or a native constraint check in its place);
  - witness generation;
  - native verification;
  - durable completion.

## Levers (not booked)

The first lever gets these requirements from b04a57's review; nobody has found a flaw yet:
- Prove the table for Q once at Bootstrap: about 1,300 point operations, ~4k checks, ~350k
  constraints, one time. Q is fixed per incarnation, and renewals keep it (proposal 810-812).
- The table rows are dedicated, whole and unblinded. They are a public function of the public
  Q, so zero knowledge holds.
- C_table is carried in the state and compared in the next secondary, where Vesta points are
  native, at about 83 constraints.
- Dirichlet guarantees 0 < |v'| ≤ 2^32 with |v'u2 mod n| ≤ 2^224, and parity takes a ±1 fix.
- All base signatures are own-key: τ_send, τ_recv, and the Request under the receiver's key.
- Exception-freeness must be re-argued with the extra bases. The both-equations rule above
  carries over.

1. **Per-wallet key tables: synthesized, see "Lever 1, synthesized" above.** The direct comb (no R'
   recovery) beats this half-scalar version: 289 checks, not about 440. The original estimate follows.
   Q is fixed for a wallet's incarnation.
   - Choose |v'| ≤ 2^32 for the fresh R'.
   - Split v (about 2^224) across precomputed 2^32k·Q, combined in 256-entry per-wallet pair
     tables (G likewise, in fixed pair tables).
   - The loop then has 8 windows instead of 33: about 440 non-native checks instead of 821 per
     own-key signature, both slope equations included.
   - That saves about 61k constraints and 121k witness entries on Receive.
   - P-recv's constraints would fit 2^19 (439k of 524k), but its witness lands about 2% over 32
     rows × 16,384 (507k against 498k). Reaching 2^19 also needs a few percent off, for example
     lever 5 or a smaller tail. If it gets there, it is worth about 0.6 s.
   - The table rows must be bound once, for example a public per-wallet row commitment checked
     by each successor.
   - Estimate only; it needs synthesis.
2. **Sparse io half in the inner sumcheck.** z is padded to 2^(sw+1); about −0.1 s Send and
   −0.2 s Receive.
3. **One Q table for Receive's two own-key signatures.** Subsumed by lever 1: one table set per wallet.
4. **IPA:** parallel L/R MSMs and the 0.17 s fixed cost (b04a57's suggestion).
5. **Lookup-based SHA-256: synthesized, see "Lever 5, synthesized" above.** 2,709 rows plus 3,392
   lookups per block, against 25,976 rows: −20k constraints per signature. It pays only where it
   moves a size class.
6. **Owner decision: sign the Poseidon message directly**, with no SHA-256 inside the signature
   (Digest NONE / X962SHA256 over the 32 bytes as the digest).
   - It drops the SHA-256 block: −26k Send, −52k Receive.
   - Raw ECDSA's key-only forgery gives a random e, which would need a Poseidon preimage, so it
     looks sound.
   - But wire:95-100 forbids no-digest modes.
   - It needs new key parameters (new incarnations), enrollment and attestation updates, SDK
     changes and a StrongBox Digest NONE check.
   - Not proposed unless the owner wants it.

## Next measurements

Done on 2026-11-08:
1. Pixel `commit-classes`: failed the 25% gate (full width 1.56x, 13-bit 1.81x at 16,384 columns). Lane commits
   and the measured IPA replace the model (b04a57, PIXEL.md).
2. Lever 1 synthesized: 289 checks (gate ≤ 400). P-recv is 2^19 under the census formula, but not under the
   synthesized verifier.
3. Verifier synthesis: VERIFIER.md (gate 1d).

Next:
1. **Lean challenge schedule: done (gate 1e, VERIFIER.md).** The comb's P-recv is 2^19 under the synthesized model.
   The soundness note (Schwartz-Zippel degrees) is written.
2. **Lookup-based SHA-256: done (lever 5).** Under the lean estimate it lifts P-recv's headroom at 2^19 from 0.2%
   to 3.6-6.5%. Under today's schedule it changes nothing.
3. **The cross-circuit binding: done (gate 1f, VERIFIER.md).** Real linked pairs, R chains on the real shared
   values, the pairing and binding refusals, and every verifier cell mutation-tested. Still open: modulus aliases
   (counted comparisons) and R inside a proven circuit (needs the real operation circuit).
4. **The burn branch:** if Path 2 keeps it (proposal 892), the incoming Send-pair verifier must be total, emitting
   soft verdict bits instead of asserting. It needs counting.
6. **Renewal path: measured.** Credential continuity (a spec change, owner decision) puts the renewed Receive at
   2.52 s. Without it, the largest Receive variant stays at 3.02-3.20 s.
5. **Endoscalar IPA challenges in the prover's fold.** The spike's u is now a·ζ + b with ~64-bit halves; measure
   whether iroha_pasta's fold exploits it (b04a57's estimate is −0.25 s per operation).
