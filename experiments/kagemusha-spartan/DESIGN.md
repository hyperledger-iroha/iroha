# KAGEMUSHA Path 2: the design

One current description of the Path 2 prover and wallet chain, consolidated on 2026-11-13 from this experiment's
records and from the source at iroha `optimizations` e562d91f81, then corrected after a read-only review by Codex
astra (ultra reasoning) the same day. The older records stay as history (§14). Authority is split: the specs in
`specs/` govern protocol rules, the source at the named revision governs behaviour, and the run logs govern results.
This file summarizes them and points to each.

This is a research design. It changes nothing installed: the running PoC, its wallet generation, Taira and the
production prover are untouched, and nothing here is a production parameter set. The crate is now the experimental
four-operation backend. Its [README.md](README.md), kept by the active writer, says how to build and run it; this
file explains how the design works and where each part stands.

**Status words.** Each part below carries one:

- **Validated:** built and passing in the last full checkpoint (2026-11-12: all 126 tests, and the eight-hop fixed point
  with two wallets, on the Mac). That checkpoint ran the research chain: Bootstrap was exact, but Send and Receive
  still had synthetic relations of the right size.
- **In code:** committed at HEAD and covered by focused tests. The complete chain over it has no recorded run yet.
- **In progress:** uncommitted work by the experiment's active writer (the Codex takeover of 2026-11-12).
- **Open:** not built yet, or waiting on a decision.

## 1. Summary

**Goal.** An offline Receive in about 3 s on a Pixel 6, against about 300 s with today's prover. Payments still travel
as one QR code of at most 12,000 bytes. Every proof is checked natively before value moves. No trusted setup.

**Approach.**

1. Every wallet operation (Bootstrap, Load, Send, Receive) proves its own step directly, with one zero-knowledge
   Spartan proof pair over the Pasta curve cycle: a P proof over Fp committed on Vesta, and an S proof over Fq
   committed on Pallas.
2. Each pair verifies the wallet's previous pair inside its circuits. A Receive also verifies the payer's Send pair,
   which the Payment carries. A wallet is a chain of pairs that starts at its Bootstrap.
3. Circuits are compiled from uniform arrays, so every circuit's matrices have a short evaluation program. That lets
   the costly matrix check of each verified proof be deferred: it is folded into one accumulated claim per side,
   which travels with the proofs and is decided natively in milliseconds. The IPA's generator checks are deferred
   too, and settled natively by an MSM.
4. One circuit per operation and side serves every wallet. A wallet's key tables enter as a committed row whose
   commitment the issuer certifies in the credential.
5. Acceptance is always native: both proofs verify, the pair is linked, and the accumulated matrix claims and the
   generator claims hold.

**Where it stands.**

- **The recursion is validated on the research chain.** The six circuits (P and S for Receive, Send and Bootstrap)
  reach a fixed point. Two wallets with different keys run every order of operations from Bootstrap on one set of
  circuits, and every accumulator holds.
- **The complete backend is built and under validation.** Bootstrap is exact and validated. Complete Send, Receive
  and Load relations, the Payment codec and the native engine are in code. The SDK surface and the physical-device
  fixtures are in progress. The first full-chain run (11-13) verified a generation-0 Receive pair of 4,992 + 4,032 B.
  It then stopped where serializing the public programs exceeded its 128 MiB bound, with a 3.1 GB peak RSS on the
  Mac.
- **No phone run since 11-10.** The uniform pair then measured 2.08 s on the Pixel. On the kernel model, a Receive pair
  costs 2.21 s in the 64-row layout (9,952 B of proofs) and about 2.64 s in the 11-12 byte-saving layout (7,264 B).
  Both figures are from the research chain. The last time the model was checked against the Pixel, it ran about 9%
  low.

**The decision that matters most now** is time against bytes (§10.3, §13).

## 2. Requirements

| | Requirement | Source |
|---|---|---|
| Speed | Receive ≈ 3 s on a Pixel 6: "it needs to be 3s, so we need 100x speedup" | owner, 2026-11-08 |
| Carrier | One QR; the complete Payment at most 12,000 bytes: "we can go to 12 kb QR" | owner |
| Proof first | Every proof is verified natively before value moves or bytes are released | owner, spec |
| Finality | Offline payments are final; only Load and Unload cross the online boundary | spec |
| Keys | The payment key is a persistent, nonexportable hardware P-256 key (StrongBox or TEE, Secure Enclave); Send is confirmed by hardware-backed biometrics or device credential | spec |
| Setup | No trusted setup; discrete log on Pallas and Vesta | design |
| Privacy | Balances and witness values stay hidden; linking which payment fed which is accepted | owner, 2026-11-08 |
| Rollout | Ships only as a deliberately chosen future wallet generation (credential and state formats change); never touches the PoC's data or Taira | owner |

## 3. Why this design

| Option | Measured or projected | Outcome |
|---|---|---|
| Today's stack: 12 PLONK/IPA k16 proofs per Receive | ~23 s each, ~300 s per Receive; with FFT, MSM and IPA all free, ~100 s still remain | No engine-only 100x exists |
| The folding path (8 half-steps, deciders, 4 wraps) | ~292 s on the Pixel; the four wraps alone 168 s | Too slow |
| Hash-based recursion per operation (Plonky3 FRI or WHIR) | ~1.5-5 s per layer (projected to the Pixel), 166-341 KB proofs | Too slow, and far too large for a QR |
| Path 1: small-field accumulation with a compact export | An export of at most 5 KB in about 1 s is out of reach: Pasta-Poseidon Merkle trees cost 45-180 s on the Pixel, emulating Poseidon2 costs 12-112 M constraints | Fails its gate; would need NFC or Nearby carriers |
| **Path 2: curve-based Spartan over Pasta, a pair per operation, deferred claims decided natively** | Gate 1 bytes passed (toy Send pair 4,544 B); recursion built; Receive models at 2.2-2.6 s | **Chosen** (owner: keep the QR) |

Sources: [research/DESIGN-100X.md](research/DESIGN-100X.md) and the reports beside it.

## 4. Architecture

### 4.1 A wallet is a chain of proof pairs

```text
             predecessor          predecessor           predecessor
 Bootstrap ─────────────► Load ─────────────► Send ─────────────► Receive ──► …
 (no input)                                    │                     ▲
                                               │ Payment             │ the payer's Send pair,
                                               └─► another wallet    │ from the Payment it received
```

- **Four kinds**, carried as two public kind bits at io 60 and 61: Receive (0, 0), Send (1, 0), Bootstrap (0, 1),
  Load (1, 1).
- **Bootstrap verifies nothing.** Every other kind verifies exactly one predecessor pair: the wallet's last operation,
  of any kind. A Receive also verifies the payer's Send pair.
- **Every chain starts at a Bootstrap.** Only Bootstrap has no predecessor, so walking back along any valid chain ends
  at one. Bootstrap authenticates the wallet's credential and fixes its zero state. There is no "genesis" flag that
  would let a wallet drop its history.

### 4.2 What one pair contains

- **P** (over Fp, committed on Vesta): the operation's own relation (state transition, signatures, maps, policy), the
  scalar side of every verified P proof, the points side of every verified S proof, the P-side claim fold and P's
  half of the pair link.
- **S** (over Fq, committed on Pallas): the scalar side of every verified S proof, the points side of every verified P
  proof, the S-side claim fold and S's half of the link. S has no own relation; S-boot is almost empty.
- **Why split this way:** a proof's commitments are points whose coordinates live in the other field, so the curve
  arithmetic of a P proof is native in S and that of an S proof is native in P (§7.1).
- **The pair link** (§7.2) ties the two halves to the same proofs.

### 4.3 What a Payment carries, and who checks what

- **The Payment** carries the payer's completed Send pair, its public inputs, the signed originals (Request, receipts)
  and companion data, in the IPM1 transport (§8.7).
- **The verdict comes from the bytes.** Within an admitted transport profile, the receiver's Receive relation derives
  its verdict from the exact Payment bytes. A valid payment credits. An invalid one takes the burn branch: the Core
  balance is credited and the same amount is added to the adjusted burned value, so spendable value does not grow,
  and the credit id stays consumed. Neither the receiver nor the host chooses the branch (spec
  `kagemusha_lambda_omega_v1.md` §2.7, OQ-3).
- **At HEAD, native admission still rejects an invalid incoming Payment** (`engine.rs` requires `valid == 1`). The
  relation keeps the burn branch for an already selected historical Receive. Turning invalid candidates into a native
  burn is in progress (uncommitted).
- **Acceptance is native.** Before any pair's bytes are released, native code verifies both proofs, the link, both
  accumulated matrix claims and the own and inherited generator claims. A Spartan verifier accepting a proof is not,
  by itself, acceptance.

| Case | Outcome |
|---|---|
| Transport, framing or reconstruction fails (length, header, profile) | Rejected before any verdict |
| Incoming pair, claims and Request signature valid | Accept: credit; the credit id is inserted |
| Incoming proof, claims or signature invalid, within the admitted profile | The relation derives burn. At HEAD, native admission rejects instead; native burn is in progress |
| A produced pair fails native verification, or its accumulators are false | Not released |

## 5. One proof

- **Relation:** R1CS, `Az ∘ Bz = Cz` with `z = (w, io)`.
- **Witness commitment:** Hyrax rows. Row r is committed as `T_r = <w_r, G> + ρ_r H`; padded rows are authenticated
  zeros.
- **Sumchecks:** the outer sumcheck proves `Σ_x eq(τ, x)(Az·Bz − Cz)(x) = 0`, and the inner sumcheck runs over z.
  - Each round binds k variables (k = 3 in the uniform circuits) and is committed as one point `D_i` of its values
    on the grid {0,1,2}^k.
  - No round polynomial and no witness evaluation is revealed.
- **Evaluations:** `v_A, v_B, v_C` are masked by two isolated multiplication gates.
- **One linear relation:** every round check, both final checks and the hidden `w~(r_y)` are batched by α and proved
  by one zero-knowledge IPA: Bulletproofs folding with blinded L and R, and a Schnorr final.
- **γ separation:** γ, drawn after α, separates the rows' and the rounds' slots in the IPA opening. Opening the plain
  sum was unsound: the CancelRound and AdaptiveWitness forgeries were accepted, and the regression suite keeps both.
- **Lean challenge schedule:** one challenge r gives `r_A, r_B, r_C = r, r², r³`. A verified proof's matrix check is
  then the single value `P(r, r_x, r_y) = rA + r²B + r³C`, the claim that §7.3 defers.
- **Commit order:**
  1. the header and the first-phase rows;
  2. the pair-link word;
  3. the link-phase rows;
  4. the second-phase challenges α and β (LogUp), the late io (the folded accumulator), the second-phase rows and the
     fixed rows;
  5. the link's R.
- **Transcript:** an RP57 Poseidon duplex sponge over the base field of the proof's points, so that a circuit can
  replay it.
  - Points are absorbed as (x, y); scalars are embedded or absorbed as 128-bit halves; the io is hashed to one digest.
  - Every P circuit uses one label (`P-uniform`) and every S circuit another (`S-uniform`), so a circuit's key does not
    depend on which kind of proof it verifies. The kind stays bound through the io.
- **Endoscalars:** challenges that multiply points are Halo endoscalars.
- **Sizes:** a proof is its row commitments, round commitments, masked evaluations, IPA L/R pairs, folded generator,
  Schnorr final and late io.

  | Layout | P proof | S proof |
  |---|---:|---:|
  | 11-12 research checkpoint: P 32 rows × 65,536, S 16 rows × 32,768 | 3,840 B | 3,424 B |
  | Earlier: P 64 rows × 16,384, S 64 rows × 8,192 | 5,024 B | 4,928 B |
  | Complete backend, generation-0 Receive (11-13) | 4,992 B | 4,032 B |

## 6. Circuits: the uniform compiler

**Arrays and families.** A circuit is a list of arrays.

- An array is a template of a few constraint rows over a few cells, repeated over instances `p` (and over `ρ` sub-rows
  per instance).
- Arrays of one family share the instance index.
- Terms read the instance's own cells, cells of another array of the family, global arrays, sums over all instances,
  constants, or challenge factors.

**Placement.** Cells go into the witness in aligned blocks of `2^m`, banded by width. Constraint rows are packed at
the lowest aligned base where the present rows do not collide. A family whose count sits just past a power of two
(`2^k < n < 1.5·2^k`) is split at `2^k`, so its blocks are not rounded up to twice the size.

**Lookups and copies.**

- Range and fixed tables use LogUp, with α and β as the second-phase challenges.
- The copy argument moves values between arrays with preprocessed index columns.
- A table's multiplicity column takes its width from the number of lookups into the table, never from the witness, so
  a build depends only on the circuit's structure.

**Widths.** Every cell has a width class (one bit, a chunk, or full width). Commitments use pay-per-bit MSMs, so narrow
values are cheap to commit.

**The matrix program.** `A~, B~, C~` at `(r_x, r_y; α, β)` compile to a straight-line program of closed forms over
blocks. Sizes at the 11-12 checkpoint, before the complete Send and Receive relations:

| Circuit | Products in the program |
|---|---:|
| P-recv | 11,472 |
| P-send | 9,438 |
| P-boot (authenticated Bootstrap) | 53,964 |

Native code runs the program for deferred claims (§7.3).

**Gadgets.**

- the RP57 Poseidon permutation, with materialized rounds;
- elliptic-curve ladders: variable-base, fixed-base and endoscaling;
- P-256 ECDSA by comb tables (m = 16, two tables of 256 points per key), with non-native limbs of 88, 88 and 79-80
  bits;
- SHA-256 with spread lookups;
- depth-32 indexed maps;
- canonical byte encodings and non-native field checks.

**Determinism.** Building a circuit twice gives the same shape and sizes and the same matrix evaluations at a random
point (`ZKS_DETERMINISM=1`): a probabilistic identity check. Exact identity rests on canonical program digests, which
the native engine compares before it proves an operation (`engine.rs`).

## 7. Recursion

### 7.1 Verifying a proof inside a circuit

**Points side**, in the circuit of the other field:

- the transcript sponge;
- on-curve checks of every proof point;
- the Hyrax row fold, with R − 1 endoscalings for R rows;
- the round commitments by Horner in γ;
- the IPA fold;
- the Schnorr check.

**Scalar side**, in the circuit of the proof's own field:

- the endoscalar values;
- the io digest;
- the round weights and the IPA tail;
- κ and the closed-form `b_final`;
- `io~(r)`;
- the Schnorr scalars.

The matrix evaluation is not done in-circuit: it becomes a fresh claim (§7.3).

**Exception-free additions.** Additions of independently chosen points check both chord-slope equations. Incomplete
additions remain only where the inputs provably differ.

### 7.2 The pair link

The pair link makes P and S two halves of one statement.

1. P's first phase yields a digest d. S absorbs d and squeezes ρ; P absorbs ρ.
2. Both circuits compute the chain `acc_(r+1) = acc_r·ρ + v_r` from `acc_0 = μ` over the pair's shared values, ending
   at R. The chain sits in the link phase, after ρ and before α and β.
3. ρ and R are public link io at 124-127, set from the transcript and the link, so proofs do not carry them. A chain
   that does not end at the link's R fails its own circuit.
4. Each verifier circuit ties every verified proof's link io to the pair's shared values.

μ is committed before ρ, so if P's and S's vectors `(μ, v)` differ, the two chains agree with probability at most
about `n/2^126`, since ρ is a 126-bit challenge. The native engine draws μ from fresh OS entropy; the research fixtures derive it deterministically. A
swapped partner (P-recv linked with S-send) is refused.

### 7.3 Deferred matrix claims

**The combined polynomial.** The kinds a circuit verifies share one polynomial over
`(c0, ρ, α, β, x, y, c1)`:

`P = Σ_k eq((c0, c1), corner_k) · pad_k(x, y) · (ρ A_k + ρ² B_k + ρ³ C_k)(α, β; x, y)`

`pad_k` zeroes the variables kind k does not have.

**Fresh claims.** Each verified proof contributes one fresh claim:

- its point: the kind's corner, its r, its α and β, its `r_x` and `r_y` (padded);
- its value: the value its verifier checks.

**The fold.** A step folds four claims (two incoming accumulators and two fresh claims) into its own accumulator.

1. Take the cubic curve γ through the four points at t = 0..3.
2. Commit `q = P∘γ` in the first phase. Its degree is `3·(selector bits + X + Y) + 9 + 3·(challenge degrees)`, 165
   for P in the 64-row layout.
3. Check `q(t) = v_t` at t = 0..3.
4. The new claim is `(γ(β), q(β))`, with β the second-phase challenge. It is published as late io.

A step with one input proof (Send, Load) repeats its two claims, which is sound. A false input claim makes the
folded claim false, except with probability about `deg q/2^126`: β is a raw 126-bit challenge. That is before
Fiat-Shamir, grinding and composition losses.

**Native decision.** An accumulated claim is checked by running each kind's matrix program once at its point: about
0.2 ms per uniform kind on the Mac.

**The prover's native q.**

- q needs P at the curve's points. Each kind's program runs only at its own degree + 1 points, in groups of four
  points per instruction over all cores; equispaced extension fills in the rest.
- Every batched q equals the pointwise q.
- Every fold evaluates every kind on its own degree-sized grid, so the largest program dominates. P-boot's program is
  4.7 times P-recv's.
- Mac medians of seven interleaved rounds:

  | Run | P-recv | S-recv | P-send | S-send |
  |---|---:|---:|---:|---:|
  | 11-10 fixed point, two kinds, 16 threads | 12.9 ms (73.8 ms one point at a time) | 9.3 ms (37.7) | | |
  | 11-12 checkpoint, three kinds with exact Bootstrap, 4 threads | 196.6 ms (657.9) | 32.2 ms (88.4) | 179.9 ms (672.2) | 48.9 ms (82.1) |

  That is about 229 ms of native q per Receive pair on four Mac threads. Four kinds and the phone are unmeasured.

### 7.4 Generator claims

An IPA ends at a folded generator `G_final = <s(u), G>`, which a verifier would compute with an MSM as long as the row.
Instead the claim `(G_final, u)` is carried: each proof's own claim, plus the claims inherited from the proofs it
verifies. The circuit batches these claims, and native code decides what remains with one MSM per claim.

At HEAD, four ordered claims are batched by a private, committed-mask restructuring before one fresh claim is
exported (`generator_claim.rs`, bound to the proof ports by `generator_integration.rs`). On 2026-11-08 the owner chose
to fold inherited claims in clear, accepting linkability. That dropped the private restructuring, which then cost
about +1.2 s per Send and +1.6 s per Receive. Which form ships is an open decision (§13), and the code's form has not
been timed inside a pair.

### 7.5 Kinds, keys and one layout per side

- **One layout per side.** A verifier slot's structure depends on the verified circuit's layout: constraint
  variables, Hyrax rows, first-phase rows and fixed rows. Every kind is built at one shared layout per side
  (`build_with_copies_at`).
  - The first phase is padded with empty rows.
  - Padding fixed rows hold a single 1, so their commitment is a generator rather than the identity.
- **The key follows the kind.**
  - Each circuit pins its own kind bits in io.
  - A predecessor slot reads the verified proof's two kind bits from its io, holds them boolean, and ties them to two
    shared values of the pair and to the fresh claim's selector coordinates.
  - It pins the verified proof's fixed rows to the key those bits select (`points::KeySelect`).
  - The payer's slot in a Receive is a constant Send.
- **Tested refusals:** a changed shared kind bit or claim selector breaks a row on both sides.

### 7.6 The base case

Bootstrap has no predecessor slot. It verifies no proof and folds nothing.

Its accumulator is the **empty claim**, pinned as constants: `c0 = c1 = 1`, every other coordinate 0 (so ρ = 0), and
value 0. Because ρ = 0 zeroes `ρA + ρ²B + ρ³C` for every kind, the empty claim holds on every hop's polynomial, and
the prover chooses nothing in it. Load's corner is (1, 1), so the zero comes from ρ, not from the corner.

### 7.7 The fixed point

Circuits that verify proofs of circuits like themselves need keys that depend on their own size. They are built by
iteration ("hops"):

1. Hop 1 builds the circuits over toy pairs.
2. Each later hop builds them over the previous hop's pairs.
3. Sizes settle first. Matrices settle one hop later, because keys enter as constants.

In the last full run the matrices agree at random points from hop 4 on, and all 32 sampled rebuild comparisons pass.
The linked verifier also accepts template artifacts whose accumulators are false. So every accepting native boundary
must decide the accumulators, and should pin the final programs by digest.

The template-building hops start from toy proofs, so their accumulators are false. They are construction inputs, never
valid payments. The final chains start at Bootstrap pairs, and there every accumulator holds.

## 8. The wallet relations

### 8.1 Identity and authority

- **Enrollment certificate** (108-byte body + 64-byte signature):
  - verified under the fixed scheme-root key in the verification key;
  - the expected scheme is a trusted public input, which avoids an identity cycle through the verification-key
    catalogue;
  - outputs the issuer key and the certificate digest. ([CERTIFICATE.md](CERTIFICATE.md))
- **Credential** (approved future 532-byte body + signature):
  - signed by the issuer the certificate names; issuers rotate under one root key;
  - carries the payment key and, at offset 195, the 64-byte payment key-table commitment;
  - its evidence, renewal and regulatory rules are enforced on the signed bytes. ([CREDENTIAL.md](CREDENTIAL.md))
- **Certified key tables:**
  - The payment key's two comb tables (512 points as 3,584 field values) are the circuit's first fixed row. That row
    is committed with no blind and left out of proofs.
  - Its commitment comes from io 56-59, and every verifier substitutes it.
  - The issuer computes the commitment from the attested key before signing: about 780 point operations and one MSM
    over 3,584 values. The wallet recomputes it at intake.
  - A proof checked under another wallet's commitment is refused.
  - Spec change: iroha 2180d7956b, [research/SPEC-CHANGE-key-table-commitment.md](research/SPEC-CHANGE-key-table-commitment.md).
- **Credential continuity:** a depth-16 prior-credential array, whose root is in the core, lets a Receive accept a
  pre-renewal receiver credential by membership. Spec change: iroha 00fc09fcea,
  [research/SPEC-CHANGE-credential-continuity.md](research/SPEC-CHANGE-credential-continuity.md).

### 8.2 State, statement and continuity

- **State:** the future Core of 30 fields, the extra one being the prior-credential root at index 8, plus a Rest of
  6. The commitment is `P(kgwcore1, core[30] || P(kgwrest1, rest[6]))`.
- **Statement:** 26 fields (`kgwstmt1`).
- **Head continuity:**
  - The head is public at P io 4.
  - Each operation's statement absorbs the predecessor proof's head and its own kind, and every signature is over a
    later digest of that chain.
  - Bootstrap's predecessor is zero.
  - A Receive's payer proof cannot stand in for the wallet's predecessor.
  - A re-signed statement with a wrong predecessor is refused by the recursion.
- **P io profile:**

  | io | Contents |
  |---|---|
  | 4 | head |
  | 16-28 | scheme, relation, asset, wallet, credential digest, payment-key halves |
  | 32-37 | lifecycle, policy epoch, controls, burned value, lineage roots |
  | 56-59 | table commitment |
  | 60-61 | kind bits |
  | 64 on | late io (the accumulator) |
  | 124-127 | link io |

  The transport reconstructs all 126 ordinary inputs.

### 8.3 Bootstrap (validated)

Bootstrap composes, in one relation ([BOOTSTRAP.md](BOOTSTRAP.md)):

- the certificate and the credential, with the issuer's comb table derived from its authenticated key;
- the credential policy;
- the exact initial 30-field state;
- the 26-field statement;
- the receipt: a 338-byte body with a low-S P-256 signature under the payment key;
- the binding of the signed table commitment to the omitted wallet row.

Different wallets and rotated issuers use the same relation key.

### 8.4 Load (in code)

An ordinary, finalized Load (`load.rs`).

- Native code verifies the genesis-rooted Sumeragi finality original before it asks for the hardware-signed Advance.
- The relation proves the exact ordinary receipt, the value, ordinal and recovery effects, and that signed Advance.
- It accepts no host finality Boolean and does not claim to prove BLS finality.

### 8.5 Send (in code)

The complete relation (`send.rs`) irreversibly debits the amount plus fee and binds one receiver. It composes all of
these under the sealed, verified predecessor:

- the state;
- the completed Request (418-byte body + signature) and its terms;
- the held fee schedule: a 191-byte body, an integer basis-point fee with clamping, and zero fee when no schedule is
  held;
- the arithmetic: `gross = amount + fee` and `old balance = gross + adjusted burn + remaining`, with every unrelated
  field preserved;
- the time and quota controls;
- the pending and fee-map effects;
- the own receipt.

([MONETARY_COMPONENTS.md](MONETARY_COMPONENTS.md) describes the components.)

### 8.6 Receive (in code)

The complete relation (`receive.rs`, `receive_lineage.rs`, `receive_originals.rs`) derives a total verdict on the
incoming Payment from its exact bytes, within an admitted transport profile (framing failures are rejected first),
using:

- the original objects;
- the Request signature (a total P-256 verdict);
- the full Payment identity;
- a total verification of the payer's pair (points, scalars and proof words).

It then applies:

- the accept or burn branch;
- the consumed-credit (OQ-3) and credit-digest map effects;
- burn accounting;
- prior credentials;
- the own receipt.

A Receive with an invalid incoming proof or signature uses the same matrices as a valid one. On the burn branch the
Core balance is credited and the lineage adds the same amount to the adjusted burned value. The consumed-credit root
either stays unchanged or takes a structurally valid insertion of a key proven absent (OQ-3), and existing
credit-digest records are preserved.

### 8.7 The Payment and its transport (in code)

- **The commitment** (`payment.rs`) hashes every byte of the completed Send pair, including the final IPA responses.
  The signed preparation receipt stays in the package and cannot name the proof that contains it.
- **The wire** (`payment_wire.rs`) is the experimental IPM1 profile.
  - Its framing and hashing match the SDK transport.
  - It is uncompressed, so the measured size is the real size.
  - It reconstructs the omitted originals and all 126 ordinary inputs, and refuses any discrepancy.
  - Decoding is not admission.
- **Proof bytes** (`proof_bytes.rs`): both circuits bind the exact compressed proof bytes through the pair link, and
  the primary hashes the whole pair.
- **Sizes measured so far,** on the 11-12 research chain, not the complete backend: a Bootstrap pair's Norito
  envelope is 11,731 B, broken down below. Reconstructing the ordinary inputs from supplied context brings it to
  7,699 B. The complete Payment has not been measured; the complete-chain test measures it.

  | Part | Bytes |
  |---|---:|
  | Proofs (P + S) | 3,840 + 3,424 |
  | Ordinary inputs | 4,032 |
  | Link R | 32 |
  | Framing and profile | 403 |

## 9. The native side

- **Programs:** eight public verifier programs, one per kind and side, holding instructions, constants and geometry
  and no witness data (`program_artifact.rs`). A root-signed manifest pins them (`engine.rs`).
- **The engine** verifies every output against its program, checks both matrix accumulators and the generator claims,
  and takes fresh OS entropy for every proof's masks and the pair's μ.
- **Custody:**
  - It belongs to the canonical Advance provider: durable selection, hardware signing, redundant copies, anchors and
    exact-byte retry.
  - There is no second journal and no caller-supplied acceptance Boolean (`native.rs`).
  - Send prepares an irreversible amount-plus-fee debit.
  - Retries resend the original Payment bytes and never re-prove a released operation.
- **SDK surface** (in progress, `NATIVE_BACKEND.md`), identical in Swift and Kotlin:
  1. install the programs;
  2. bind the existing Ed25519 account to Native's slot and the four exact originals;
  3. prepare a review bound to one immutable capsule;
  4. confirm through hardware-backed biometrics or the device credential;
  5. execute;
  6. read the output (Unknown, Pending, Archived or DeliveryLoss).
- **Not built:** Unload and RefreshTimeAnchor.

## 10. Performance

### 10.1 Measured on the Pixel 6 (2026-11-10)

| Pair | Time |
|---|---:|
| Stage A's real Receive pair | 1.99 s (2.43 s before the review lane's kernels) |
| The uniform pair | 2.08 s; the model said 1.91 s |

Peak memory was about 1 GB for a stage A pair. Nothing has been measured on a phone since, and nothing at the current
layout.

### 10.2 Modelled per pair (kernel-calibrated model, 11-10 kernels)

| Layout | Receive | Send | Bootstrap | Proofs per pair | Own relations then |
|---|---:|---:|---:|---:|---|
| P 64 × 16,384, S 64 × 8,192 (11-11, 11-12) | 2.21 s | 1.55 s | 1.05 s | 9,952 B | synthetic Send and Receive, stand-in Bootstrap |
| P 32 × 65,536, S 16 × 32,768 (11-12 research checkpoint) | ≈ 2.64 s | ≈ 2.20 s | ≈ 2.28 s | 7,264 B | exact Bootstrap, synthetic Send and Receive |

Each total is P plus S:

| Layout | Receive | Send | Bootstrap |
|---|---|---|---|
| 64-row | 1.35 + 0.86 s | 0.96 + 0.59 s | 0.72 + 0.33 s |
| 11-12 checkpoint | 1.76 + 0.88 s | 1.48 + 0.72 s | 1.76 + 0.52 s |

Where P-recv's time goes in the 11-12 layout:

| Term | Time |
|---|---:|
| IPA | 0.71 s |
| Commit | 0.56 s |
| Inner sumcheck | 0.37 s |
| Outer sumcheck | 0.10 s |

The IPA term was 0.26 s at 16,384 columns.

### 10.3 Time against bytes

Each witness row costs one 32-byte commitment in the proof. Fewer, longer rows therefore save bytes, but they lengthen
the IPA and the inner sumcheck.

- **The 64-row layout** is faster: 2.21 s, about 2.41 s at the observed 2.08/1.91 calibration. Its 9,952 B of
  proofs leave no room in 12,000 B: with the historical envelope overhead the pair already accounts for at least
  13,526 B.
- **The 11-12 layout** fits better: the Bootstrap envelope measured 11,731 B. But its Receive models at about 2.64 s,
  about 2.88 s at the same calibration, leaving about 0.12 s.
- **Neither is a given.** P and S may use different layouts. The right choice is the fastest complete layout whose
  complete Payment fits 12,000 B, measured, not modelled.
- **Not yet counted:**
  - the growth of the complete Send and Receive relations (the complete generation-0 Receive pair is 4,992 + 4,032 B);
  - native q (§7.3: about 229 ms per Receive pair on four Mac threads);
  - the generator-claim restructuring (§7.4);
  - the native decisions;
  - the receipt and durability steps.

### 10.4 Native work

- **The prover's native q:** see §7.3.
- **The final decisions:** about 0.2 ms per kind per accumulated claim, and one MSM per generator claim.
- **Memory:** a stage A pair peaked at about 1 GB on the Pixel. The complete-chain driver peaked at 3.1 GB RSS on the
  Mac (compiler and prover together). The complete backend's per-operation memory on a phone is unmeasured.

None of these has been timed on a phone.

## 11. Security

**Assumptions:**

- discrete log on Pallas and Vesta;
- Fiat-Shamir with the Poseidon sponge as a random oracle;
- for unbounded recursion, a PCD composition theorem checked against this construction, or an explicitly stated
  recursive Fiat-Shamir assumption, as today's spec has.

**Arguments by mechanism:**

| Mechanism | Argument |
|---|---|
| γ separation | A rewinding argument over a Laurent polynomial in γ: off-slot terms land on nonzero exponents, so a forgery needs γ to hit one of `2m+1` roots. γ is an endoscalar from 126 challenge bits, so the bound must be taken over that distribution, not `|F|`. Written proof pending. |
| Deferred matrix claims | A false input claim survives the fold with probability about `deg q/2^126` (β is a raw 126-bit challenge), and native decision is mandatory. |
| Pair link | μ is committed before ρ, so two different chains agree with probability about `n/2^126`. |
| Curve arithmetic | Exception-free: complete additions wherever inputs may coincide. |
| Encodings | Canonical: byte ranges, and comparisons against the modulus. |

**Tested refusals** (regression suite and fixed-point runner):

- forged rounds, and single-bit flips in every proof element;
- truncation and wrong public inputs;
- swapped pair partners;
- changed kind bits, empty-claim coordinates or table-commitment limbs;
- a proof checked under another wallet's commitment;
- a re-signed statement with a wrong predecessor;
- high-S signatures;
- a wrong scheme, issuer or digest;
- tampered state, statement or receipt;
- malformed incoming proofs and signatures, which derive an invalid verdict inside the relation; at HEAD, native
  admission then rejects the Payment.

**Trust assumptions that proofs do not cover:**

- **No double spending across forks.** Head continuity links one chain. It does not make a spend globally unique:
  two Sends from the same funded state can each satisfy local conservation. Preventing that rests on trusted wallet
  execution, protected state and hardware custody, which have to hold through concurrency, restart, rollback and
  backup recovery.
- **Load finality.** Native code verifies Load's finality; the relation does not prove BLS finality.
- **Issuance.** The issuer derives the certified table commitment, and the wallet recomputes it at intake. The
  recursion relies on that for every earlier counterparty. Generator namespace, capacity, column order and table
  construction have to be pinned in authenticated parameters.

**Known gaps:**

- the recursive soundness theorem for Path 2 is not stated or proved: extraction through joined histories (a Receive
  has two parents), accumulated claims, and recursive Fiat-Shamir;

- an independent soundness review ([research/REPORT-b04a57-soundness.md](research/REPORT-b04a57-soundness.md) is the
  checklist);
- written proofs for γ separation and the deferral;
- secret MSMs are not constant-time, as in today's prover;
- the deterministic `SpikeRng` is for tests only.

## 12. Status

| Area | State | Evidence |
|---|---|---|
| ZK Spartan pair: committed rounds, γ separation, ZK IPA | Validated | [RESULTS.md](RESULTS.md), regression suite |
| Uniform compiler: lookups, copies, packing, family splitting | Validated | [A2-UNIFORM.md](A2-UNIFORM.md) |
| In-circuit verification (points and scalar sides) | Validated | [VERIFIER.md](VERIFIER.md) |
| Pair link inside both circuits | Validated | A2-UNIFORM.md, "The pair binding inside the circuits" |
| Deferred matrix claims, batched native q | Validated for three kinds; four kinds (Load) in code | A2-UNIFORM.md; `deferral.rs` tests |
| Fixed point: Receive, Send, Bootstrap, every order, two wallets | Validated (11-12, eight hops, research chain) | `bootstrap-resume-fixed-point-8.log` (run evidence, 11-12) |
| Certified key tables and their spec change | Validated; spec committed | A2-UNIFORM.md; iroha 2180d7956b |
| Predecessor-head continuity | Validated | A2-UNIFORM.md, "Research predecessor-head continuity" |
| Exact Bootstrap | Validated | [BOOTSTRAP.md](BOOTSTRAP.md) |
| Monetary components (state, Request, statement, fee, arithmetic, controls, indexed map) | Validated as components | [MONETARY_COMPONENTS.md](MONETARY_COMPONENTS.md), [INDEXED_MAP.md](INDEXED_MAP.md) |
| Complete Send, Receive (total verdict) and Load inside the recursion | In code. The first full-chain run verified a generation-0 Receive pair, then stopped at the 128 MiB program-serialization bound (11-13) | `send.rs`, `receive*.rs`, `load.rs`, `backend_chain_tests.rs`; `codex-20261113/PROOF_VALIDATION.md` |
| Native burn of invalid incoming Payments | In progress; HEAD rejects them natively | `engine.rs` (working tree) |
| Private generator-claim restructuring | In code; decision to confirm | `generator_claim*.rs` |
| Payment commitment, IPM1 wire, proof-byte binding | In code | `payment.rs`, `payment_wire.rs`, `proof_bytes.rs` |
| Native engine, program artifacts, custody contracts | In code | `engine.rs`, `native.rs`, `program_artifact.rs` |
| SDK surface, proving-memory accounting | In progress | `NATIVE_BACKEND.md` (uncommitted) |
| Device fixtures: Android hardware attestation; iPhone App Attest and Secure Enclave | In progress | `device/` (uncommitted) |
| Phone measurement of the current design; complete Payment size | Open | |
| Unload, RefreshTimeAnchor, the remaining policy signer roles | Open | |
| Production codecs: 532-byte credential, 30-field Core | Open | [research/CONSTRUCTION.md](research/CONSTRUCTION.md) |
| Independent soundness review | Open | |

## 13. Open decisions and next steps

**For the owner:**

1. **Layout: time against bytes** (§10.3). Choose the fastest complete layout, P and S separately, whose complete
   Payment fits 12,000 B. Neither historical layout is a given; shorter IPA rows and deduplicated or reconstructed
   inputs are the levers. Decide on a measured complete Payment and a Pixel run, not on the model.
2. **Generator claims** (§7.4). Fold in clear, as decided on 2026-11-08, or keep the private restructuring now in
   code.
   - The private path runs another full-length IPA per side.
   - Historically, restructuring cost 1.17 s per operation (Receive 6.25 s against 4.65 s in clear, on the 11-08
     census model).
   - Going back to clear folding needs a reviewed joint-opening argument, not just deleted checks.
3. **Optional: Digest-NONE payment signatures.** They would save 26k constraints in Send and 52k in Receive. Raise
   only if the other levers fall short, because it needs new key parameters, enrollment and attestation updates.

**Engineering:**

- Fix the public-program serialization that stopped the full chain at its 128 MiB bound, without raising the bound;
  bring the 3.1 GB driver peak down. Then finish validating the complete chain (the active writer), and record the
  complete circuits' sizes and the complete Payment's size.
- **The next measurement:** one complete Pixel Receive, from retained incoming bytes to a verified, durable,
  releasable output, with phase timings, the complete carrier bytes and peak RSS from the same run. This needs the
  owner's go in the phone operator's chat.
- Test native burn end to end. Independently corrupt P and S matrix claims, own and inherited generator claims,
  proof words, group and scalar equations, and Request signatures. Each admitted invalid case must produce a verified
  burn, preserve unrelated value and allow the next operation; a valid Payment must never burn.
- Profile and shrink P-boot's matrix program (53,964 products, about 72% of the three P programs' total). Taking
  Bootstrap out of the per-step polynomial would need an authenticated discharge of its matrix obligation, not a
  one-time native check.
- Pin program identity by digest at every accepting boundary, and test a correctly signed credential that carries a
  wrong table commitment.
- State the Path 2 recursive soundness assumption and its extraction argument. Get an independent soundness review
  and written proofs.
- Build Unload, RefreshTimeAnchor and the remaining signer roles; write the production codecs.

## 14. Map of the code and the records

**Code** (`src/`):

| Area | Files |
|---|---|
| Proof system | `spartan.rs`, `ipa.rs`, `fs.rs`, `kern.rs`, `grid.rs`, `r1cs.rs`, `prof.rs` |
| Compiler | `uniform.rs`, `wired.rs`, `blocks.rs` |
| Gadgets | `ladders.rs`, `nonnative.rs`, `sha.rs`, `points_complete.rs`, `indexed_map.rs`, `signature.rs` |
| Recursion | `verifier.rs`, `points.rs`, `scalar.rs`, `deferral.rs`, `generator_claim*.rs`, `generator_integration.rs`, `full.rs`, `proof_bytes.rs`, `incoming_codec.rs` |
| Wallet relations | `certificate.rs`, `credential.rs`, `credential_policy.rs`, `bootstrap*.rs`, `monetary_state.rs`, `statement.rs`, `request.rs`, `fee_schedule.rs`, `send*.rs`, `receive*.rs`, `load.rs`, `receipt.rs`, `operation_auth.rs` |
| Payment | `payment.rs`, `payment_wire.rs`, `carrier.rs` |
| Native | `engine.rs`, `native.rs`, `native_witness.rs`, `program_artifact.rs`, `chain_fixture.rs` |
| Census and models | `census.rs`, `operation.rs`, `gadgets.rs`, `sparse.rs` |
| Runner | `lib.rs` (benchmarks and the fixed-point runner; `main.rs` calls it) |

**Records**, history with their own dates:

- [A2-UNIFORM.md](A2-UNIFORM.md): the compiler, fixed point, kinds, binding, Bootstrap, tables and head continuity;
- [RESULTS.md](RESULTS.md): gate 1, bytes;
- [CENSUS.md](CENSUS.md), [OPERATION.md](OPERATION.md) and [VERIFIER.md](VERIFIER.md): gates 1b-1f;
- [REAL-RECEIVE.md](REAL-RECEIVE.md): stage A;
- [PIXEL.md](PIXEL.md): Pixel kernels;
- [BOOTSTRAP.md](BOOTSTRAP.md), [CERTIFICATE.md](CERTIFICATE.md), [CREDENTIAL.md](CREDENTIAL.md),
  [MONETARY_COMPONENTS.md](MONETARY_COMPONENTS.md) and [INDEXED_MAP.md](INDEXED_MAP.md);
- [research/](research/): the 100x design, construction record, spec changes and consults;
- Codex astra's review of this file, 2026-11-13 (local, git-ignored):
  `../../output/kagemusha-100x-20261108/astra-design-review-20261113/answer.md`.

**Reproduce:** see [README.md](README.md), "Reproduce".

## Glossary

| Term | Meaning |
|---|---|
| P, S | The two proofs of a pair: P over Fp (Vesta commitments), S over Fq (Pallas commitments) |
| Kind | The operation a circuit proves: Receive, Send, Bootstrap or Load, as two public bits |
| Fresh claim | A verified proof's matrix check, `P(r, r_x, r_y)`, left for later |
| Accumulator | The one claim per side a proof carries, folding all earlier claims; decided natively |
| Empty claim | Bootstrap's accumulator: ρ = 0, so it holds on every polynomial |
| Late io | Public values fixed after the second-phase challenges, here the accumulator |
| Shared values | The values both halves of a pair commit to and the link checks |
| Wallet row | The fixed row holding the wallet key's comb tables, committed by the certified commitment |
| Hop | One rebuild of the circuits over the previous rebuild's proofs, while finding the fixed point |
