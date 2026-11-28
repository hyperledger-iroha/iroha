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

## 0. Status at iroha `optimizations` 79d42d60da (2026-11-15)

**Control decision, 2026-11-18 (`1e2afdc338`).** The next experimental catalog keeps four P/S pairs. Bootstrap remains in the recurring matrix polynomial. The standalone relation in `dd38d1348c` shares the recovery insertion across Load, RefreshTimeAnchor and Unload. Recursive integration still needs one shared Control pair. The two mode bits are authenticated ordinary inputs. The standalone Refresh relation hard-checks its anchor signature, with a fixed public dummy signer for the other modes. Qualification still requires a combined build, all eight exact program comparisons, native recovery, signed device measurements and the owner decisions. [Integration decision and gates](research/E4-REWEIGH.md)

**Update 2026-11-18** (HEAD 60309bba27; supersedes the Payment-size and Burn bullets below):

- **Complete chain on the Mac, 11-17** (`output/s2/chain-20261117T051751Z.log`, HEAD 1def5379c7 plus the then
  uncommitted device-fixture work, layout p18-s17). Programs reported stable by digest from generation 2-4 on. Final
  generation, linked pairs verified: Receive 29.35 s, Send 10.75 s, Load 10.99 s, Bootstrap 9.96 s, every pair
  4,832 + 4,704 B. **Complete IPM1 Payment 11,597 B** (pair 9,610, envelope 10,748, companion 749): 403 B under the
  12,000 B experimental budget. A malformed-R Receive burned under the same program. The process ended during the
  native containment stage without an exit line, so the run has no peak-RSS line; its bundle
  (`output/full-backend-run-20261117T051751Z/native-device`) is complete.
- **Native Burn, end to end** (`output/r26/containment-20261117T175154Z.log`, from that bundle, 1,669 s wall,
  5.67 GB peak RSS): `authenticated_native_containment_from_bundle` passed. Fifteen independently corrupted Payments
  (P and S matrix claims, own and inherited generator claims, group and scalar equations, malformed proof words, the
  link R, a Request signature with corrupted bindings, an isolated bad Request signature with a valid Send proof) each
  burned 10, kept the unrelated 100, and let the next Send10 spend, leaving 90. A valid Payment could not be forced to
  burn and credited without burn. TODO G4 is done.
- **Library suite** at 1def5379c7: 196 passed, 0 failed, 11 ignored.

This section supersedes the dated status statements later in this file (§1 "Where it stands", and the notes on the
128 MiB stop, native rejection of invalid Payments, the unmeasured Payment and "uncommitted" work). The rest of the
design text is unchanged. Everything below was read from committed source and the run logs in `output/`.

- **Committed.** The 11-13 and 11-14 work, including the SDK surface and the device fixtures, was swept into
  1f9a8fc086, 105b96b8a7 and 14e5a5e1f4. Nothing here is "in progress (uncommitted)" any more.
- **Complete chain on the Mac (run5, `output/full-backend-chain-run-20261113-5.log`).** All eight programs are stable
  at generation 6 (log lines 384-391). Fresh chain, linked pairs verified: Receive 32.95 s, Send 23.13 s, Bootstrap
  about 20 s, Load 84.45 s (outlier on a loaded Mac); every pair 4,832 + 4,704 B. Peak RSS 6.64 GB (footprint
  7.88 GB). The 128 MiB program-serialization stop of run1 is gone (packed program artifact, 1f9a8fc086). Run5 then
  panicked in the native Send/Receive flow with "malformed program bundle frame"; that is fixed in 14e5a5e1f4
  (`native.rs`), and the focused bundle test passes. **No complete-chain run since that fix.**
- **Complete Payment size (Mac, host software signatures).** Run5's IPM1 Payment: **12,050 B, 50 B over the
  12,000 B carrier budget** (pair 9,610, envelope 11,100, companion 850; `output/full-backend-chain/payment-size.txt`).
  The fresh native Send in `output/native-containment-20261114-2.log` wrote 11,698 B
  (`output/native-baseline-h2/complete-payment-11698.json`). The companion compaction committed in 14e5a5e1f4 has no
  complete-Payment re-measurement. This 12,000 B budget is the experiment's; the production 10,034 B envelope limit
  is unchanged.
- **Native admission now derives Burn.** `engine.rs` (around line 786) computes `valid` from parsing plus the exact
  full verifier and continues with `valid = false` instead of rejecting; resource and availability errors still
  return. Run5 verified a malformed-R Receive Burn (adjusted burn 10). Containment-2 finalized the invalid
  Request-signature and P-matrix Burns.
- **Post-Burn successor: fixture defect found 2026-11-15.** The Send10 after a Burn failed "native monetary
  preparation constraints", and its focused relation test failed at a completed-Request row with left 1, right 0.
  Cause: the fixture built the successor Request with `request_self`, which copies the payer's wallet and account
  into the receiver words. `request::append_completed` refuses payer == receiver (`equal[0]·equal[1] = 0`, since
  441496899d). The successor now pays the sender's wallet (`device_fixture::request_to`/`send10_to`); with that,
  every completed-Request row holds (`output/post-burn-successor-relation-e02b88-run.log`). The test still fails,
  next at "Send checked arithmetic and state preservation" row 44 (left nonzero, right 0): the fixture rewrites
  `after_core[10]` and the statement after `send10_to` built them. Open. The phone fixture's Bootstrap → Load100 →
  Send10 → `receive_self` flow is the same self-transfer and is refused at Send10; a device run needs a two-wallet
  fixture first.
- **Full library suite** (`output/fixed-tail-full-lib-suite.log`, before 14e5a5e1f4): 191 passed, 3 failed
  (`bootstrap::tests::recursive_natural_layout_profiles`, `full::tests::bootstrap_pair_carries_the_empty_claim`,
  `points::tests::points_side_holds_proves_and_refuses`), 10 ignored.
- **Absent:** Unload and RefreshTimeAnchor have statement tags only (`statement.rs`), with no recursive kind and no
  native transition. The fee-schedule signer check now exists natively (RegulatoryPolicy role, 2026-11-18), but no
  transition installs a schedule yet. The written recursive soundness argument now exists (research/SOUNDNESS.md);
  its independent review is open.
- **Devices.** Hardware probes only (11-13): Android StrongBox key and iOS Secure Enclave key, neither requiring user
  confirmation. Five Swift SDK tests passed on an iPhone 17 Pro Max. The iOS runtime fixture installed but did not
  launch (phone locked); Android only built. No device proving or payment run exists. The planned confirmation prompts
  (`LAContext`, `BiometricPrompt` without a CryptoObject) are not bound to the key.
- **Linked into the HEAD bridge, not into any shipped app.** Since c5511ebfc4, `crates/iroha_core_zk` depends on this
  crate with no feature gate, `connect_norito_bridge` compiles its Spartan module, and
  `check_native_sdk_artifact.py` requires the eight `connect_norito_kagemusha_spartan_*_v1` exports. The released
  Native (8df03eb77a: Android v88, iOS 20261110141615) has no such edge. A wallet switches to this generation only
  through an explicit Install with a root-signed manifest (`open.rs`); nothing switches by default.

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

**What the proofs do not guarantee.** Read this before relying on them (§11 has the detail):

- **No protection against double spending across forks.** Each proof shows that an operation follows the rules from
  the wallet's previous state. Two Sends made from the same funded state each follow the rules, so the proofs alone
  accept both. Only hardware custody, protected state and trusted wallet execution prevent this. They have to hold
  through concurrency, restart, rollback and backup recovery on both phones; that qualification is open (TODO G7).
- **Load finality is checked by native code**, not inside the relation.
- **Payments are linkable.** Balances and witness values stay hidden, but a wallet's key-table commitment is not
  blinded and ties all of its proofs together, and with generator claims folded in clear (§7.4) a Payment also links
  to the payments that funded it. The owner accepted this for the research design on 2026-11-08. Production needs the
  owner's confirmation, and the apps' privacy text has to say it (TODO G9).

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
  and the credit id stays consumed. Neither the receiver nor the host is meant to choose the branch (spec
  `kagemusha_lambda_omega_v1.md` §2.7, OQ-3). Review finding 4 identified unconstrained leaves in the fold of
  verifier public inputs. Commit `6c345ed8d0` pinned those leaves in both proof halves. The regression exercises
  both recursive verifiers. [Regression test](src/io_binding_tests.rs) · [Review status](TODO.md)
- **Native admission derives the verdict from the bytes** (`engine.rs`, `valid` from parsing plus the exact full
  verifier; resource and availability errors still return). Fifteen independently corrupted Payments each burn on
  the Mac (2026-11-18, §0), and natively a valid Payment can neither be forced to burn nor fail to credit.
- **Acceptance is native.** Before any pair's bytes are released, native code verifies both proofs, the link, both
  accumulated matrix claims and the own and inherited generator claims. A Spartan verifier accepting a proof is not,
  by itself, acceptance.

| Case | Outcome |
|---|---|
| Transport, framing or reconstruction fails (length, header, profile) | Rejected before any verdict |
| Incoming pair, claims and Request signature valid | Accept: credit; the credit id is inserted |
| Incoming proof, claims or signature invalid, within the admitted profile | Burn: the relation and native admission both derive it; unrelated value is kept and the next operation proceeds |
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
- **Challenge distributions:** the v4 candidate draws LogUp α and β from two successive full-width squeezes. Each
  uses the first word of a fresh permutation. τ and the pair-link ρ also consume full words. Other squeezes can supply
  two 126-bit raw or endoscalar challenges. The transcript domain is `kagemusha-spartan/full-logup/v1`. Transcript
  tags are ignored, so domain separation is the fixed positional schedule. Soundness bounds are taken over these
  distributions, including the correlation between the two halves of one squeeze, not over `|F|`
  ([research/SOUNDNESS.md](research/SOUNDNESS.md) §5). The local lemmas alone do not give prefix knowledge or
  recursive Fiat-Shamir security (§11).
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

μ and the shared values are committed before ρ, so if P's and S's vectors `(μ, v)` differ, the two chains agree with
probability at most about `n/p`: ρ is the whole canonical Fp word squeezed by S's transcript (`spartan.rs`
`link_word`), embedded injectively into Fq, not a 126-bit challenge ([research/SOUNDNESS.md](research/SOUNDNESS.md)
§6.7). n is 783 shared words for Send and Load and 1,539 for Receive. The native engine draws μ from fresh OS entropy
(Bootstrap's mask is deterministic); the research fixtures derive it deterministically. A swapped partner (P-recv
linked with S-send) is refused.

### 7.3 Deferred matrix claims

**The combined polynomial.** The kinds a circuit verifies share one polynomial over
`(c0, ρ, α, β, x, y, c1)`:

`P = Σ_k eq((c0, c1), corner_k) · pad_k(x, y) · (ρ A_k + ρ² B_k + ρ³ C_k)(α, β; x, y)`

`pad_k` zeroes the variables kind k does not have.

**Fresh claims.** Each verified proof contributes one fresh claim:

- its point: the kind's corner, its r, its α and β, its `r_x` and `r_y` (padded);
- its value: the value its verifier checks.

The wallet prover reads the three weighted matrix values from the scalar verifier's witness and sums them for the
fresh claim. The fold independently checks that sum against its restriction polynomial at the fresh claim's node.
A one-proof step reuses the same scalar evaluation for both fresh slots.

**The fold.** A step folds four claims (two incoming accumulators and two fresh claims) into its own accumulator.

1. Take the cubic curve γ through the four points at t = 0..3.
2. Commit `q = P∘γ` in the first phase. Its degree is `3·(selector bits + X + Y) + 9 + 3·(challenge degrees)`: 186
   for P and 174 for S at the 11-17 complete p18-s17 layout (SOUNDNESS.md §7.3); 165 belonged to the 64-row research
   layout.
3. Check `q(t) = v_t` at t = 0..3.
4. The new claim is `(γ(β), q(β))`, with β the second-phase challenge. It is published as late io.

A step with one input proof (Send, Load) repeats its two claims, which is sound. `q` is committed before β, and the
four input equalities make any false input imply a nonzero `q − P∘γ`, so one fold fails with probability at most
`deg q · λ_β`. In the v4 candidate β is a fresh field word with maximum atom 2/q on P and 1/p on S. The v3 estimate
used the second 126-bit half of a shared word. [Three-mode soundness review](research/CONTROL-SOUNDNESS.md). The bound
is before Fiat-Shamir, grinding and composition losses. Declared challenge degrees are program metadata: native
import has to verify them or rely on authenticated compiler output (TODO G5).

**Native decision.** An accumulated claim is checked by running each kind's matrix program once at its point: about
0.2 ms per uniform kind on the Mac. A kind whose selector times padding is zero at the point adds zero and is not run.

**The prover's native q.**

- q needs P at the curve's points. Each kind's program runs only at its own degree + 1 points, in groups of four
  points per instruction over all cores; equispaced extension fills in the rest.
- One-proof Load and Send steps have claim points `[A, B, A, B]`. Their cubic is
  `γ(t) = A + (B − A) h(t)`, where `h = L₁ + L₃` in the four-node Lagrange basis.
  The prover first computes `R(u) = P(A + (B − A)u)` on the line. A matrix program of line degree
  `d` takes `d + 1` samples there. Composing with `h` recovers every coefficient of the original
  `q(t) = R(h(t))`. The two-proof Receive uses the cubic through its four input points.
- Every batched q equals the pointwise q.
- A fold evaluates every kind whose selector times padding is nonzero somewhere on its curve, each on its own
  degree-sized grid, so the largest such program dominates; a kind zero at every point adds nothing and is not run.
  With five kinds ArchiveSent's selector is `c2`. Every fresh claim of the other kinds has `c2 = 0`, and so does every
  accumulator folded only from them, so a history without ArchiveSent never runs ArchiveSent's program. In the 968f
  bundle that program is the largest: 3,049,142 of P's 7,545,038 instructions. The skip depends only on the public
  claim points; a published accumulator already shows its `c2`.
- Mac medians of seven interleaved rounds:

  | Run | P-recv | S-recv | P-send | S-send |
  |---|---:|---:|---:|---:|
  | 11-10 fixed point, two kinds, 16 threads | 12.9 ms (73.8 ms one point at a time) | 9.3 ms (37.7) | | |
  | 11-12 checkpoint, three kinds with exact Bootstrap, 4 threads | 196.6 ms (657.9) | 32.2 ms (88.4) | 179.9 ms (672.2) | 48.9 ms (82.1) |

  That is about 229 ms of native q per Receive pair on four Mac threads.
- Five kinds, 968f bundle (`native_q_on_the_installed_bundle`, M4 Max, six threads, load average about 300, process
  CPU): P's five programs hold 7,545,038 instructions and run at 157 points each, about 16 ns of CPU per instruction
  and point on one thread. The arithmetic dominates: P-Bootstrap's 8,398-register program costs the same per
  instruction as P-recv's 44,188-register one, and operand prefetch or cache-line-aligned registers changed it by
  under 6%. P's `restrict` took 20.6 s CPU for random claims and 12.1 s for claims with `c2 = 0`; S's took 1.46 s
  and 0.95 s. In back-to-back `two_wallet_round_trip_from_bundle` runs, the skip cut the primary wiring's CPU by
  71% for Load and the first Send (their claims also keep `c1 = 1`, so Receive and Send vanish too) and by 30-41%
  for the later Send and both Receives. Finalize CPU fell 9-23% per operation. The phone is unmeasured.
- The installed evaluator classifies public scale coefficients once per program. A coefficient whose magnitude fits
  in a `u64` uses a one-word Pasta product. Larger coefficients use a full field product. On the 968f bundle,
  the sum of median CPU times across all ten programs fell from 2.144 s to 2.017 s (6.0%) for 16 points per program.
  The primary Receive program took 0.500 s versus 0.529 s with full products. The benchmark used four lanes on one
  M4 Max worker, alternated execution order across four paired samples, and checked each result against scalar evaluation.
- On the unmasked 968f bundle, a one-proof Load or Send follows the line `A + (B − A) h(t)`.
  This line restriction relies on the whole curve. A quartic mask adds `t(t−1)(t−2)(t−3) s`, which
  vanishes at all four claim nodes but changes the intermediate evaluations. A masked prover must restrict
  the matrix polynomial along its masked curve even when it repeats the same two claims.
  On the signed 968f bundle, the one-proof fold evaluated each active primary matrix program at 53 points
  instead of 157. On six M4 Max workers, primary and secondary restriction together used 1.33 s of process CPU
  for a first Load, down from 3.79 s, and 3.80 s for a later Send, down from 10.93 s. Each number is the mean
  of two runs with alternating order on the same public points. The benchmark compared all 187 primary and
  175 secondary coefficients in each pair.
  Four signed two-wallet runs (baseline, candidate, candidate, baseline) all completed the seven operations.
  The primary circuit wiring for Load and both Sends averaged 21.08 s of CPU on the baseline and 11.03 s on
  the candidate. The full round trip averaged 408.56 s and 436.26 s of process CPU respectively on the loaded Mac.
  A final pair ran after the other local tests finished. The baseline and candidate used 19.92 s and 10.11 s for
  the same wiring, 394.44 s and 387.99 s for the whole process, and 172.80 s and 219.03 s of elapsed time.
  The wiring improvement held across all three comparisons. Section 10.5 measures overall latency with independent wallets.

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
  and no witness data (`program_artifact.rs`). A manifest (v2) signed by a root-certified Artifact-role key pins the
  eight canonical program digests, the accumulator dimensions and the generator/table profile (namespace,
  hash-to-curve suite, capacities, derived-set digests, table construction and its known answer). Installation
  recomputes each and derives every program's matrix-tape challenge degrees instead of trusting the declared ones
  (`engine.rs`).
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
- **Memory (2026-11-18, iroha 2b3671cae3, p18-s17, Mac):** the prover reads its caller's witness rows in place and
  copies only the rows it writes; the first outer round streams Az, Bz, Cz; `M(r_x, ·)` uses a split equality table,
  plans built one group at a time and pages written on demand; the matrix value comes from the folded table, so no
  tape runs in the prover. Proofs are byte-identical. Accounted peaks: S 105-216 MB; P 345-432 MB for Bootstrap,
  Load and Send; P-Receive 869 MB, 139 MB over the 0.68 GiB admission. The native operation's process peak fell
  from 5.26 to 2.94 GB RSS and is now set by the engine (its program-digest compile and preparation check). Phone
  memory is unmeasured.

None of these has been timed on a phone.

### 10.5 Signed two-wallet round trip

`native::device_fixture::tests::two_wallet_round_trip_from_bundle` times both signed parameter installations,
two Bootstraps, a Load of 100, and two transfers of 10, one in each direction. Each wallet owns its engine and
installation. Each recipient verifies the sender's exact Payment bytes with an empty entry for that Payment
before proving its Receive. The timer includes signing with software fixture keys and ends when A completes its
final Receive. The test checks final balances of 100 and 0, both credit roots, zero Burn and the QR size of each
Payment. Physical-device signing, transport and durable storage require separate measurements.

On an M4 Max with six Rayon workers, all 16 fresh release processes using the signed 968f bundle completed
the seven operations and independently verified both 10,231-byte Payments. Both baselines ran the identical
two-owner fixture. The baseline ran first in pairs 1 and 4 of the initial comparison and in pairs 2 and 3 of
the shipping comparison. Stage profiling was disabled for all eight pairs.

| Comparison | Pairs | Mean elapsed, baseline → optimized | Mean process CPU, baseline → optimized |
|---|---:|---:|---:|
| Initial parent `f058094475` | 4 | 54.875 → 52.858 s | 189.481 → 174.661 s |
| Shipping parent `8f377e8f60` | 4 | 56.976 → 56.302 s | 202.440 → 193.732 s |
| Both | 8 | 55.926 → 54.580 s | 195.960 → 184.196 s |

Across the eight pairs, mean elapsed time fell 2.4% and mean CPU fell 6.0%. The optimized code finished sooner
in four pairs and used less CPU in six. A breakdown of the recorded operation boundaries shows where the elapsed
savings occurred. From the start of the first Send through completion of the final Receive, mean time fell from
38.582 to 35.180 s against the shipping parent and from 38.425 to 34.359 s across both comparisons. This interval
includes both recipient verifications and excludes setup, funding and construction of the first Request.
The primary proof was the largest finalization stage in both separately instrumented profiles.

The linked verifier returns the public inputs it checked on both curves so the engine can decide the inherited
generator claims directly. A Send reuses that successful verification for its outgoing Payment only when its
installed programs, projected inputs, link word and both proof encodings match. Receive computes its two matrix
values and incoming verdicts once for both circuits. Generator folding uses four independent prefix products
per batch inversion.

For a predecessor with checked source blocks, the prover obtains the first four IPA challenges from the usual
L/R messages before reducing the generator basis. Each output is a sum of 16 generators whose weights are the
subset products of those challenges. A single GLV Straus multiplication shares doublings across the 15 weighted
terms. At `9116d8e860`, a verified two-parent Receive still derived later L/R messages from its intermediate
bases. At 262,144 input generators, four paired six-worker comparisons on the M4 Max measured
42% less CPU for the four Pallas folds and 44% less for Vesta. Every output point matched the sequential folds.

The native RP57 Poseidon permutation also uses a cheaper representation for its 57 partial rounds. Write the
MDS matrix in blocks `[m a; v B]`. For passive state `Y`, the coordinates `p = aY` and `q = aBY` give
`aB²Y = tr(B)q − det(B)p` by Cayley–Hamilton. This reduces the linear layer from nine field multiplications to
five per partial round, with one basis conversion at each end. The public parameters determine the transformed
constants. Custom matrices with dependent basis rows use the dense calculation. In eight paired batches of
16,384 complete permutations, process CPU fell 24% on each Pasta field. The pinned hash vectors agree on both.

Four fresh-process pairs compared these two arithmetic changes together at `9116d8e860` with their parent
`ac3aced67b` on the unmasked 968f bundle. The baseline ran first in pairs one and four. Mean full elapsed
time rose from 60.142 to 62.120 s, and mean process CPU rose from 210.095 to 211.122 s. The first Send
through final Receive averaged 39.849 and 39.621 s respectively. In a separate instrumented candidate run,
primary proving accounted for 81.1 CPU-seconds, including 44.9 in the opening. Generator-claim folds took
40.0 CPU-seconds across the seven operations.

Four more pairs compared the integrated arithmetic commit `39d6b21d92` with its shipping parent `3efd797b23`
on the same build lane. Mean elapsed fell from 220.449 to 205.936 s and mean process CPU from 378.343 to
311.549 s. The integrated commit finished sooner in three pairs. Its runs ranged from 71.410 to 574.175 s
as host load and swap varied, so these measurements do not establish a repeatable wall-time gain.

The wallet verifier can also retain the partial sums it checks. Split the IPA challenge vector into a four-word
prefix and a suffix. For each contiguous generator block `G_j`, compute `B_j = ⟨s(suffix), G_j⟩` and require
`Σ_j s(prefix)[j] B_j` to equal the claimed generator. The wallet retains 16 points per claim, about 4 KiB
of points for the own and inherited claims on both curves. A successor matches the claim, proof, wallet owner
and installation before consuming the blocks. A verified Receive can then compute its leading L/R messages
from all four sources. Once the fold matches the incoming Send's own and inherited claims on both curves,
the witness reuses those four generator verdicts. It evaluates both matrices and parses the incoming bytes
independently. The verifier and recursive trace also evaluate the folded opening weight directly from
its public slot factors. The prover still materializes the column weights needed for the individual L/R sums.

Four fresh-process pairs compared the retained blocks and factored openings at `67caea8965` with their
parent `39d6b21d92`. Average CPU fell from 309.736 to 277.983 s (10.25%). Average elapsed rose from
111.037 to 121.082 s (9.05%). The first two pairs averaged 141.005 to 124.045 s. After a host restart,
the last two averaged 81.069 to 118.121 s. Each pair ran entirely within one host session, and each
recipient independently verified its incoming Payment in all eight runs.

For admitted P-256 keys, the host verifier computes the ECDSA point `[z/s]G + [r/s]Q` in projective
coordinates and normalizes it once. The comb witness uses the two verified scalars and checks its own
independently constructed point against both affine coordinates and the required x-coordinate residue.
In eight alternating single-thread M4 Max batches of 80 public verifications, mean process CPU fell from
2.268 s to 0.031 s. The two implementations agreed on all 13 accepted and seven rejected inputs.

Four six-worker round-trip pairs compared the published wallet at `50b25af7b6` with the same source tree
using the previous two ECDSA implementation files. The published wallet finished sooner in all four pairs.
Mean full elapsed fell from 46.182 to 43.259 s (6.33%) and mean CPU from 160.162 to 154.306 s (3.66%).
The first Send through final Receive fell from 29.929 to 27.997 s, including both independent Payment
verifications. Primary proving remained the largest finalization stage in a separate profiled run.
It used 47.36 CPU-seconds across the seven operations, including 24.40 CPU-seconds in the opening.

To measure the cumulative change against `39d6b21d92` under shared host load, four more pairs launched
both versions together with three workers each. Kernel process starts were within 4.5 ms for every pair.
Mean full elapsed fell from 108.136 to 99.019 s (8.43%) and process CPU from 203.112 to 188.206 s (7.34%).
The published version finished sooner and used less CPU in every pair. The first Send through final Receive
fell from 75.843 to 68.144 s (10.15%). The two Receive finalizations together fell from 39.977 to 35.066 s
(12.28%). This comparison includes all intervening branch changes and uses the signed unmasked 968f bundle.

The ordinary IPA opening reuses public generators across proofs under one wallet installation. Its first round
computes `G_lo + [u]G_hi` for the public transcript challenge `u`. Write each signed GLV component of `u`
in two pieces at bit 32. A table of `[2^32]G_hi` lets all four pieces share the same doubling steps, using
the curve endomorphism for the second component. Later rounds operate on the newly folded basis. At the signed
wallet dimensions the table occupies 8 MiB on Vesta and 4 MiB on Pallas per installation. The engine prepares
a table when the early and per-stage memory checks show enough headroom and retains it for the lifetime of that
installation. A cold wallet below the preparation threshold continues with the ordinary fold.

Against `f7a213db1c`, a fixed-entropy differential at both wallet widths reproduced all four opening proofs
byte for byte. Each curve covered a first opening with no inherited claim and a second opening with one claim.
The verifier checked each proof, and the next transcript challenge and blinding-stream scalar also matched.

**First-round generator table on the signed 968f wallet.** Four fresh-process pairs compared
`f7a213db1c` with `c484a75259` on an M4 Max. Both installations, seven operations and both recipient
verifications are included. Each version ran first in two of the six-worker pairs. Stage profiling was off.
Elapsed savings are relative to the baseline mean.

| Full round trip | Mean elapsed, baseline → candidate | Elapsed saved | Mean process CPU, baseline → candidate | Candidate faster |
|---|---:|---:|---:|---:|
| Six workers, sequential | 73.714 → 75.042 s | -1.80% | 239.169 → 237.975 s | 2/4 |
| Three workers per version, simultaneous | 100.970 → 99.737 s | +1.22% | 214.653 → 210.988 s | 4/4 |

| Elapsed per pair in seconds, baseline / candidate | Pair 1 | Pair 2 | Pair 3 | Pair 4 |
|---|---:|---:|---:|---:|
| Six workers, sequential | 97.237 / 71.661 | 73.888 / 73.629 | 63.575 / 85.150 | 60.156 / 69.728 |
| Three workers per version, simultaneous | 108.942 / 107.574 | 101.770 / 101.253 | 83.071 / 82.123 | 110.096 / 107.998 |

Two additional pairs profiled both versions simultaneously with three workers each and reversed the launch
order. Each entry sums the seven openings on that curve, then averages the two runs. The candidate includes
table preparation for both wallets. Both recipients independently verified their 10,231-byte Payments in every run.

| Profiled opening work | Mean elapsed, baseline → candidate | Elapsed saved |
|---|---:|---:|
| Primary first generator fold | 3.915 → 3.152 s | +19.48% |
| Complete primary IPA | 12.935 → 12.397 s | +4.16% |
| Complete secondary IPA | 5.977 → 5.611 s | +6.13% |

**Sequential slowdown and arithmetic follow-up.** In the four sequential pairs above, time outside the two-transfer
exchange rose from 23.036 to 26.869 s. The two installations accounted for 1.028 s of that increase.
Bootstrap finalizations accounted for another 1.257 s and Load finalization for 1.254 s. From the first Send through final Receive, elapsed time
fell from 50.678 to 48.173 s. Host load varied across the pairs.

Each new wallet still derives and authenticates its own Pallas and Vesta generators. For groups of 32 public
indices, Montgomery's trick inverts the 64 simplified SWU denominators together. The map then takes square
roots in affine coordinates and applies the prescribed sign rule. The P-256 witness also reuses the tangent
or chord computed for each point and extracts range-check chunks directly from the integer's limbs.
For both Pasta primes, canonical field operands satisfy `−p < a − b < p` with `p < 2²⁵⁵`.
The high bit of the 256-bit difference therefore selects the modular correction after a single carry chain.

A separate six-worker baseline run sampled thread stacks every 10 ms. Of 207,283 samples that ended in
active work, 33.1% were in multiscalar multiplication, 18.8% in generator folds and 16.8% in matrix work.
The paired full-wallet runs use the fixture clocks and operating-system counters. At full wallet dimensions,
all 393,220 compressed generator encodings and both signed manifest digests matched the scalar implementation.
Deriving one full generator set used 10.433 CPU-seconds with the scalar map and 9.103 with the batch map.

Four pairs per cohort compared signed commits `dc415ed6bb` and `bc85468523` on the M4 Max using the same
signed unmasked 968f bundle. The baseline ran or launched first in pairs 1 and 4. Every run started a fresh
process, installed both wallets and verified both unseen 10,231-byte Payments. Wall time ends at the final Receive.

| Full round trip | Mean elapsed, baseline → candidate | Elapsed saved | Mean process CPU, baseline → candidate | CPU saved | Faster pairs |
|---|---:|---:|---:|---:|---:|
| Sequential, six workers | 268.860 → 262.625 s | +2.32% | 356.289 → 324.183 s | +9.01% | 2/4 |
| Simultaneous, three workers per version | 348.530 → 325.457 s | +6.62% | 356.895 → 326.090 s | +8.63% | 4/4 |

| Measurement in seconds, baseline / candidate | Pair 1 | Pair 2 | Pair 3 | Pair 4 |
|---|---:|---:|---:|---:|
| Sequential, six workers, elapsed | 324.033 / 389.170 | 211.278 / 255.571 | 244.053 / 147.338 | 296.074 / 258.420 |
| Sequential, six workers, CPU | 364.941 / 327.647 | 338.813 / 334.041 | 357.289 / 304.018 | 364.112 / 331.024 |
| Simultaneous, three workers per version, elapsed | 423.272 / 401.498 | 263.225 / 248.193 | 260.388 / 232.261 | 447.235 / 419.875 |
| Simultaneous, three workers per version, CPU | 364.638 / 333.521 | 346.929 / 317.093 | 341.414 / 306.461 | 374.598 / 347.286 |

| Unprofiled six-worker stage | Mean elapsed, baseline → candidate |
|---|---:|
| Outside the transfer exchange, including installation and funding | 89.776 → 109.042 s |
| Both installations | 12.189 → 22.837 s |
| Both Bootstrap finalizations | 30.768 → 35.850 s |
| Load finalization | 40.137 → 40.428 s |
| First Send through final Receive | 179.084 → 153.583 s |
| Both Receive finalizations | 101.252 → 74.410 s |

The exchange saved 25.501 s per round trip. Time outside the exchange rose by 19.266 s.
The largest installation difference was in the first pair, 11.198 → 53.375 s for the two installations together.
Host load varied across runs. These are host measurements with software signing.

Two additional six-worker profile pairs ran sequentially, reversing the order for the second pair.
The full profiled runs took 296.409 / 176.759 s and 281.654 / 179.808 s (baseline / candidate).
Values sum each stage across seven operations and average the two runs per version. The stage CPU estimates
interpolate operating-system process samples at the recorded stage boundaries.

| Profiled finalization stage | Mean elapsed, baseline → candidate | Mean sampled CPU, baseline → candidate |
|---|---:|---:|
| Primary proof | 80.375 → 45.925 s | 120.005 → 104.225 s |
| Secondary proof | 37.290 → 25.160 s | 59.570 → 52.355 s |
| Primary witness wiring | 31.625 → 20.435 s | 43.860 → 38.230 s |
| Generator-claim folds | 16.425 → 10.655 s | 29.465 → 24.915 s |

The opening timers below include both cold table preparations on the corresponding curve.

| Profiled opening | Mean elapsed, baseline → candidate |
|---|---:|
| Primary (Vesta) | 38.389 → 18.339 s |
| Secondary (Pallas) | 17.453 → 11.112 s |

At the signed dimensions, the four fixed-entropy openings reproduced every proof byte, the subsequent
transcript challenges and the next blinding scalars. The full-wallet timings use fresh proof entropy each time.
The candidate passed 378 active wallet tests and all four signed-original checks.

**Wallet witness construction and public constraint padding.** Four more paired runs per cohort compared
signed revisions `37ac218541` and `8f373f068b` on the same M4 Max and signed 968f program bundle.
Each process installed the programs in two independent wallets, performed two Bootstraps, one Load and two
transfers, independently verified both 10,231-byte Payments, and checked both balances, lineage roots and zero Burn.
Baseline ran or launched first in pairs 1 and 4. The table reports arithmetic means. The host also ran other workloads.

| Full wallet | Elapsed (s), baseline → candidate | Seconds saved | Elapsed saved | Process CPU (s), baseline → candidate | Faster pairs |
|---|---:|---:|---:|---:|---:|
| Sequential, six workers | 40.8768 → 40.5275 | 0.349 | 0.85% | 143.981 → 144.295 | 3/4 |
| Simultaneous, three workers per version | 65.7695 → 63.8375 | 1.932 | 2.94% | 154.770 → 151.144 | 4/4 |

| By pair, seconds (baseline / candidate) | Pair 1 | Pair 2 | Pair 3 | Pair 4 |
|---|---:|---:|---:|---:|
| Sequential elapsed | 41.301 / 41.003 | 40.691 / 40.618 | 40.923 / 39.775 | 40.592 / 40.714 |
| Sequential CPU | 145.475 / 145.720 | 143.396 / 144.800 | 143.913 / 141.684 | 143.140 / 144.977 |
| Simultaneous elapsed | 65.474 / 63.546 | 64.470 / 62.448 | 65.539 / 63.680 | 67.595 / 65.676 |
| Simultaneous CPU | 153.874 / 150.324 | 151.763 / 148.141 | 154.211 / 150.615 | 159.234 / 155.493 |

| Unprofiled full-wallet components | Sequential seconds, baseline → candidate | Simultaneous seconds, baseline → candidate |
|---|---:|---:|
| First Send through final Receive | 27.180 → 26.722 | 43.863 → 42.712 |
| Time outside the exchange | 13.697 → 13.805 | 21.907 → 21.125 |
| Both installations (included above) | 2.950 → 3.014 | 4.531 → 4.402 |

Two additional sequential profile pairs used six workers and reversed the order. Timers sum the seven operations
for each process. The outer timers cover all outer rounds. Installation includes decoding and authentication.

| Profiled work | Mean seconds, baseline → candidate |
|---|---:|
| Both installations | 2.924 → 2.823 |
| Primary witness wiring | 5.495 → 4.690 |
| Primary program check and memory release | 3.645 → 3.505 |
| Primary helper and matrix instantiation | 0.215 → 0.160 |
| Secondary helper and matrix instantiation | 0.020 → 0.015 |
| Primary outer sumcheck | 0.845 → 0.705 |
| Secondary outer sumcheck | 0.305 → 0.225 |
| Primary IPA | 4.408 → 4.314 |
| Secondary IPA | 1.959 → 1.960 |

Complete curve ladders reuse affine coordinates and batch witness inversions. The first outer sumcheck
factors out public padding from its multilinear evaluations. Lookup helpers reuse challenge powers and
resolved masks, and the template compiler avoids temporary coefficient lists. Installation checks coordinate
multilinearity using register bitsets and tracks the degrees of both challenges separately.

In an isolated 16-slot ladder test, Pallas CPU fell from 70.5 to 10.4 ms and Vesta from 63.0 to 9.6 ms.
Witness cells, constraints and results matched the reference for both curves. Checking degrees on the ten
signed tapes took 171.8 → 37.0 ms median CPU per installation.
The candidate passed 382 active wallet tests and all four signed-original checks. The full-wallet measurements use software fixture signing.

**Primary opening generator folds.** The public fold now evaluates a double and its first signed addition
together. For an accumulator P and addend Q, it computes the x coordinate of T = P + Q, then adds P using
the first slope and the x coordinates of P and T. The cached first fold also represents the two GLV components
jointly using signed rotations of P and P − φ(P), with φ the Pasta endomorphism. Both paths use the existing
complete group formulas for exceptional inputs. The retained first-fold table has the same size.

Four pairs per cohort compared signed revisions `e8091736b6` and `422ae8f2b5` on the M4 Max with the
signed unmasked 968f bundle and fresh proof entropy. Each process performed two independent installations,
two Bootstraps, one Load and the two transfers, including both recipients’ verification of unseen 10,231-byte
Payments and final balances. The host also served other workloads. Baseline ran or launched first in pairs
1 and 4. The table shows arithmetic means over all four pairs.

| Full wallet | Elapsed seconds, baseline → candidate | Elapsed saved | Process CPU seconds, baseline → candidate | CPU saved | Faster elapsed pairs |
|---|---:|---:|---:|---:|---:|
| Sequential, six workers | 214.1042 → 226.7875 | -5.92% | 267.328 → 269.700 | -0.89% | 3/4 |
| Simultaneous, three workers per version | 460.8165 → 454.6622 | 1.34% | 274.923 → 270.345 | 1.67% | 4/4 |

| By pair, seconds (baseline / candidate) | Pair 1 | Pair 2 | Pair 3 | Pair 4 |
|---|---:|---:|---:|---:|
| Sequential elapsed | 168.432 / 167.121 | 237.976 / 316.301 | 217.373 / 198.428 | 232.636 / 225.300 |
| Sequential CPU | 252.730 / 266.028 | 267.077 / 265.265 | 273.792 / 273.993 | 275.712 / 273.512 |
| Simultaneous elapsed | 278.501 / 276.783 | 245.141 / 235.304 | 264.498 / 263.740 | 1055.126 / 1042.822 |
| Simultaneous CPU | 261.286 / 258.133 | 263.222 / 256.691 | 278.142 / 274.179 | 297.042 / 292.376 |

In the sequential comparison, the candidate used 2.372 more CPU-seconds on average. Pair 2 added
78.325 s of elapsed time while using 1.811 fewer CPU-seconds. In the simultaneous comparison,
the candidate was faster in all four pairs and saved 4.578 CPU-seconds on average. The slowest
simultaneous pair took over 1,000 s for both versions. All pairs are included.

Run the benchmark from this experiment with `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS` set to the signed bundle directory.

```sh
RAYON_NUM_THREADS=6 cargo test --locked --offline --release --lib \
  native::device_fixture::tests::two_wallet_round_trip_from_bundle -- --exact --ignored --nocapture
```

## 11. Security

**Assumptions:**

- discrete log on Pallas and Vesta;
- Fiat-Shamir with the Poseidon sponge as a random oracle;
- for the recursion, the explicit assumption **P2-RKFS** ([research/SOUNDNESS.md](research/SOUNDNESS.md) §4.2,
  revision 2): (A) the adversary is algebraic on Pallas and Vesta, with representations recorded when each point
  first enters a transcript, and (B) the RP57 Poseidon transcript used inside the recursion is an ideal oracle. No PCD
  composition theorem is imported, and nothing here is a DL-only theorem.

**The written argument (TODO G1/G2, 2026-11-18, Codex astra, revision 2 after the independent review):**
SOUNDNESS.md defines the security experiment with honest wallets, argues by closure over the finite set of
extracted operation nodes (a Receive joins its own history and the payer's; no global rank is needed, and own chains
are rooted at Bootstrap by the sequence counter), and states a monetary conservation corollary under custody
hypotheses that bind every wallet in the history: no forks, no second Bootstrap per credential or wallet id, and
native Load finality. It gives written local lemmas, a round-by-round lemma for query accounting, and bounds over
the real challenge distributions (raw 126-bit
words, endoscalars from 126 bits, full-width words). At the 11-17 complete layout the conservative local algebraic
bound per operation is about **104.0 bits for Bootstrap, 101.6 for Load, 101.4 for Send and 100.7 for Receive**,
dominated by the compiler's LogUp terms; without LogUp the main proofs, folds and link give about 115.3 bits. These
numbers are conditional on P2-RKFS and the compiler's phase invariants, and they have to be multiplied by a finite
history and query budget: with an illustrative `Q ≤ 2^40` adversary queries and `N ≤ 2^36` extracted nodes the
algebraic term is about `2^-60.6`. **None of this is an overall 128-bit claim**; a production target needs chosen
budgets and the cryptographic terms added (SOUNDNESS.md §7-§8).

**Arguments by mechanism:**

| Mechanism | Argument |
|---|---|
| γ separation | Under the P2-RKFS representation assumption the extracted coefficients are fixed before the fresh γ reply. Multiplying the nonzero Laurent discrepancy by γᵐ gives a polynomial of degree at most `2m`. The probability of a root is bounded using the endoscalar distribution. α batches `m + 2` equations with degree at most `m + 1` (SOUNDNESS.md §6.1). |
| Committed sumcheck rounds | Each block has individual degree at most two; with the three-variable Kronecker schedule its univariate error polynomial has degree at most 26; the initial equality test has degree at most `2^sx − 1`; the matrix combination is a degree-3 test (SOUNDNESS.md §6.2-§6.4). |
| Deferred matrix claims | The first phase commits `q` before β. The survival bound for a nonzero discrepancy is `deg(q − P∘Γ) · λ_β` under the stated degree and reply-distribution hypotheses. Native decides both final claims (§7.3). |
| Pair link | μ and the shared values are committed before ρ, the whole squeezed Fp word; two different chains agree with probability about `n/p`, n = 783 (Send, Load) or 1,539 (Receive) (§7.2). |
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
- malformed incoming proofs and signatures, which derive an invalid verdict inside the relation and burn natively
  (15 independent cases, 2026-11-18, §0).

**Trust assumptions that proofs do not cover:**

- **No double spending across forks.** Head continuity links one chain. It does not make a spend globally unique:
  two Sends from the same funded state can each satisfy local conservation. Preventing that rests on trusted wallet
  execution, protected state and hardware custody, which have to hold through concurrency, restart, rollback and
  backup recovery.
- **Load finality.** Native code verifies Load's finality; the relation does not prove BLS finality.
- **Issuance.** The issuer derives the certified table commitment, and the wallet recomputes it at intake. The
  recursion relies on that for every earlier counterparty. Generator namespace, capacity, column order and table
  construction are pinned in the authenticated parameters; credential intake recomputes the table commitment under
  them, and a correctly signed credential with a wrong commitment is refused (2026-11-18).

**Known gaps:**

- the recursion rests on the explicit P2-RKFS assumption, not a proved theorem; the independent reviewer accepts it
  as minimal, well defined and sufficient, but adopting it is an owner decision (SOUNDNESS-REVIEW.md, re-check);
- the conservation corollary holds only under the custody hypotheses above, for every wallet whose history joins;
- open code findings from the review: unbound io-fold leaves (4), the LogUp phase invariant not enforced at build
  time (5), cross-field limb canonicality (7) and forgery refusals through the complete builders (9);
- no finite lifetime or query budget is enforced (the sequence counter is unsigned 128-bit), so the bounds above
  hold only under chosen budgets;
- an independent soundness review of SOUNDNESS.md (G3; [research/REPORT-b04a57-soundness.md](research/REPORT-b04a57-soundness.md)
  is the checklist), and the open items in SOUNDNESS.md §9 and §11 (imported degree declarations, generator-profile
  binding, LogUp phase invariants, a frozen candidate equal to HEAD);
- side channels: by the owner's 2026-11-24 decision (TODO G8) private commitments (witness rows, round values, the
  IPA L/R vectors and the generator-claim folds) use variable-time secret Pippenger MSMs without tables, so their
  timing and memory accesses depend on the private values; proof bytes are unchanged. The inner sumcheck's
  support reads only public structure (rows the build never wrote, the io, the public `M(r_x, ·)`), never a
  private zero. LogUp multiplicity counting and link-table recount use data-oblivious lookup schedules.
  The installed prover's row hooks take rows by position; only the non-uniform benchmark and test circuits'
  default hooks compare rows by value. The build's copy and absent-instance checks use constant-time field
  equality and branch only on a malformed witness, whose refusal still stops at the first failing slot. Witness
  construction has value-dependent paths;
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
| Native burn of invalid incoming Payments | In code; 15 corrupted cases burn, valid Payments credit (Mac, 11-18) | `engine.rs`, `engine_adversarial_tests.rs`; `output/r26/containment-20261117T175154Z.log` |
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
