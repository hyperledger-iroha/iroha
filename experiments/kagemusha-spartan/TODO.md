# KAGEMUSHA Path 2 (Spartan): TODO to finish

Owner request, 2026-11-17: list what remains before the Spartan backend can replace today's prover. Sources:
[DESIGN.md](DESIGN.md) §0, §11-§13; Codex astra's design review
(`../../output/kagemusha-100x-20261108/astra-design-review-20261113/answer.md`); the soundness checklist
[research/REPORT-b04a57-soundness.md](research/REPORT-b04a57-soundness.md); the post-Burn report
[output/post-burn-row44-REPORT.md](output/post-burn-row44-REPORT.md).

Today's prover keeps moving money until **every Gate item** is done. Path 2 ships only as a deliberately chosen future
wallet generation (credential and state formats change); it never touches the PoC's data or Taira. Make no 3 s or 100x
claim before item E5 measures it.

## Gate: security, before any wallet uses Path 2

- [ ] **G1. Recursive soundness argument.** State the exact Path 2 recursive assumption (or check a PCD composition
  theorem against this construction): extraction through joined histories (a Receive has two parents), accumulated
  matrix and generator claims, recursive Fiat-Shamir, well-founded state progression and a finite lifetime/query
  budget. "Only Bootstrap has no predecessor" is not the argument.
  *Done when:* a written argument in `research/` covers every recursive kind, and G3's reviewer accepts it.
  *2026-11-18:* written by Codex astra: [research/SOUNDNESS.md](research/SOUNDNESS.md) §4 (explicit assumption
  P2-RKFS, induction over the extracted DAG, well-foundedness from the implemented state, finite budgets in §8). It
  is conditional and says so; it waits for G3.
  *Revision 2 (8672d5ee88):* minimal P2-RKFS (algebraic adversary + ideal recursive RP57), honest-wallet experiment,
  closure over the extracted node set (no global rank), conservation corollary. The reviewer's re-check: accept,
  conditionally on code findings 4 and 7 (canonicality) and the owner's decisions (adopt P2-RKFS, budgets, custody
  hypotheses).
- [ ] **G2. Written proofs and exact bounds.** Prove γ separation (CancelRound/AdaptiveWitness must stay refused) and
  the deferred matrix claims. Take each bound over the challenge's real distribution (β is a raw 126-bit integer; γ is
  an endoscalar from 126 bits; the pair-link challenge comes from S's Fp transcript), include the lean schedule's
  ~3-4-bit loss, and state the total bits of soundness per operation.
  *Done when:* the proofs are written and DESIGN.md §5/§7.3/§11 quote the corrected bounds.
  *2026-11-18:* written in SOUNDNESS.md §5-§7 (per-operation local algebraic bounds 104.0 / 101.6 / 101.4 / 100.7 bits
  for Bootstrap / Load / Send / Receive, LogUp-dominated, conditional); DESIGN.md §5, §7.2, §7.3 and §11 now quote
  them (the link ρ is a whole Fp word: `n/p`, not `n/2^126`). Closes with G3.
  *Revision 2:* the "~3-4-bit loss" is replaced by exact per-term accounting and a round-by-round lemma (largest
  single-reply charge about 2^-101.3; about 2^-61 total at Q = 2^40, N = 2^36, and 2^-37 at Q = 2^64). The reviewer's
  re-check: accept, conditionally on code findings 5 and 9.
- [ ] **G3. Independent cryptographic review.** A reviewer outside this lane, working from
  research/REPORT-b04a57-soundness.md, the written G1/G2 arguments and the code.
  *Done when:* every finding is fixed or explicitly accepted by the owner.
  *2026-11-18:* review done by an independent reviewer (Claude Opus 5.5, outside this lane's writers):
  [research/SOUNDNESS-REVIEW.md](research/SOUNDNESS-REVIEW.md). G1 and G2: accept with required changes; 0 critical,
  4 major, 5 minor, 2 editorial; the §7 ledger recomputed without error. Code findings 4 (unbound io-fold leaves let
  a receiver force burn), 7 and 9 are being fixed; finding 5 waits for the prover-memory work in `uniform.rs`; the
  document findings go back to its author. Owner decisions listed at the end of the review.
  *Later 2026-11-18:* document findings resolved in SOUNDNESS.md revision 2 (8672d5ee88; re-check accepts G1 and G2
  conditionally). Code findings 4, 7 and 9 fixed in 6c345ed8d0 (finding 4 confirmed by test first; every unbound
  io-fold leaf pinned to zero, no rows added); re-check item R4 in 740b30764f. Open: finding 5 (LogUp phase invariant
  enforced at build time, after the prover-memory work leaves `uniform.rs`), a fresh chain on the new programs, and
  the owner's decisions.
- [x] **G4. Native burn, end to end** — done 2026-11-18. `engine.rs` derives `valid` from parsing plus the exact
  full verifier; OQ-3 is applied in `specs/kagemusha_lambda_omega_v1.md` §2.7. `authenticated_native_containment_from_bundle`
  passed on the 11-17 bundle (`output/r26/containment-20261117T175154Z.log`): 15 independently corrupted cases each
  burned 10, kept 100 and let Send10 spend; a valid Payment could not be forced to burn and credited (DESIGN.md §0).
- [ ] **G5. Native acceptance hardening.** Reject construction artifacts whose template accumulators are false at
  every accepting native boundary (linked verification accepted them:
  `../../output/kagemusha-100x-20261108/spike/runs/codex-20261112/bootstrap-resume-fixed-point-8.log` line 346).
  Pin program identity by canonical digest at every accepting boundary.
  *Done when:* those refusals are tested, and all eight complete programs are shown exactly equal (not the 32
  sampled matrix-evaluation comparisons).
  *2026-11-18 (iroha a96dd1f3f4):* every accepting boundary decides both matrix and both inherited generator
  accumulators through `engine::decide_accumulators`; imported tapes get their challenge degrees derived and refused
  above the declaration (all eight saved programs pass); the chain test compares the exact canonical bytes of all
  eight programs and refuses its template pairs at pair, head, Payment and Receive verdict. Open: that heavy chain run.
- [x] **G6. Issuance parameters.** Pin generator namespace, capacity, column order and table construction in
  authenticated parameters (research/CONSTRUCTION.md).
  *Done when:* a correctly signed credential that carries a wrong table commitment is refused, by test.
  *Done 2026-11-18 (a96dd1f3f4):* manifest v2 pins namespace, hash-to-curve, capacities, derived-set digests, table
  construction and a known-answer table commitment; install recomputes them; credential intake recomputes the table
  commitment. `correctly_signed_credential_with_wrong_table_commitment_is_refused` and
  `generator_and_table_profile_is_pinned_by_authenticated_parameters` pass.
- [ ] **G7. Trust model stated where it binds.** Double-spend prevention rests on hardware custody, protected state
  and trusted wallet execution, not on the proofs (two Sends from one funded state each satisfy local conservation);
  Load finality is verified natively, not inside the relation.
  *Done when:* the spec says so prominently, and concurrency, restart, rollback and backup recovery are qualified at
  the custody boundary on both phones.
  *2026-11-18:* stated prominently in DESIGN.md §1 ("What the proofs do not guarantee") and §11. Phone qualification
  open (needs E5/E6 devices).
- [ ] **G8. Constant-time secret MSMs**, or the owner accepts the same side-channel exposure as today's prover, in
  writing.
- [ ] **G9. Linkability.** The owner accepted on 2026-11-08 that payments can be linked to the payments that funded
  them; a wallet's unblinded table commitment also ties its proofs together.
  *Done when:* the spec and the apps' privacy text say this, and the owner confirms it for production.
  *2026-11-18:* DESIGN.md §1 says it. Open: the owner's production confirmation, then the apps' privacy text when a
  Path 2 generation ships (D4); today's apps do not use Path 2, so their text stays as is.

## Engineering: to a complete operation on a phone

- [x] **E1. The three failing library tests** — fixed at their causes, iroha 1def5379c7 (suite 196 passed, 0 failed).
- [ ] **E2. Fresh complete chain on the Mac** (none since 14e5a5e1f4's bundle-frame fix): times per kind, peak RSS,
  and the complete IPM1 Payment size against the 12,000 B experimental budget (11-13 run5: 12,050 B; the 11-15
  two-wallet run: 11,597 B).
  *2026-11-17/18:* chain run `output/s2/chain-20261117T051751Z.log` gave per-kind times and a complete Payment of
  11,597 B (403 B under budget), but ended without a peak-RSS line. Rerun once after E3 and G5 land, for peak RSS and
  the exact program-digest equality.
- [ ] **E3. Memory.** The complete chain peaked at 6.64 GB RSS (11-13) against the 0.68 GiB proving admission for
  4 GB phones. *Done when:* every kind proves within the admission, measured.
  *2026-11-18 (2b3671cae3):* prover cut about 3x with byte-identical proofs; every kind except P-Receive fits by the
  accounting (P-Receive 869 MB, +139 MB); process peak 5.26 → 2.94 GB RSS; every proof faster (P-Receive 1.58 →
  1.32 s Mac). In progress: the engine side (release private values, no private digest recompile, chunked
  preparation check) and the real-RSS measurement of whole native operations.
- [ ] **E4. Shrink P-boot's matrix program** (53,964 products, about 72% of the three P programs). Taking Bootstrap out
  of the per-step polynomial needs an authenticated discharge of its matrix obligation, not a one-time native check.
  *2026-11-18:* construction chosen in [research/E4-F1-F2-DESIGN.md](research/E4-F1-F2-DESIGN.md) §3 (Codex astra):
  discharge Bootstrap's fresh matrix claims inside the first successor's circuits under pinned Bootstrap keys, then
  drop Bootstrap from the recurring polynomial. Not implemented; its cost is unmeasured.
  *Step 1 (af4cc93556):* the in-circuit evaluator of the real Bootstrap tapes is exact and refuses false claims, but
  it costs about 678K constraint rows and 1.18M committed cells per recurring P proof, likely a P geometry step and
  about +200 B per Payment, and saves only about 3.8% of P's native q: P-Bootstrap is no longer 72% of it (the
  53,964 figure predates the complete relations; P-Receive's tape now dominates). E4 is being re-weighed against
  other ways to give Unload a corner before any integration.
- [ ] **E5. One complete Pixel 6 Receive**: retained incoming bytes to a verified, durable, releasable output, with
  phase timings, carrier bytes and peak RSS from the same run; then Send, Load and Bootstrap. The Pixel operator runs
  it only with the owner's go typed in the operator's own chat. *Done when:* the 3 s target is judged on these
  numbers.
  *2026-11-18:* runner and package scripts ready (`output/e5-pixel-package/`: build with source-drift checks, run,
  manifest template); no phone binary yet. Build after E3 lands, from a committed HEAD; if E3 changes a program, take
  the bundle from a fresh chain run.
- [ ] **E6. iPhone measurement** with the Secure Enclave / App Attest device fixture (iOS and Android must behave the
  same).
- [x] **E7. Two-wallet device runner** (the phone fixture's Bootstrap → Load100 → Send10 → `receive_self` flow is a
  self-transfer and is refused at Send10).
  *Done 2026-11-18 (670a26616d):* launch 1 runs A Bootstrap → Load100 → Send10 to software wallet B, B receives and
  sends 10 back; launch 2 (fresh process) receives B's retained Payment to a verified, durable, credited output;
  launch 3 finds it retained. Mac host run passes (`output/e7/host-run-20261117T211302Z`, contended: Receive 85.5 s,
  peak 2.93 GB, Payment 11,597 B).

## Missing functionality

- [ ] **F1. Unload** (offline to online) as a recursive kind with its Core path. The proof-before-payout hold on
  suffix-Unload stays.
  *2026-11-18:* relation, Core acceptance and exactly-once storage specified in E4-F1-F2-DESIGN.md §4. The Core side
  is future-generation code and ships with D4.
  *2026-11-18 (14e14627e6):* the exact Unload relation is in `src/unload.rs`, standalone (147,620 rows, 11 tests incl.
  native parity with `iroha_data_model`'s nullifier and redeem leaf). Not yet a recursive kind: it needs E4 to free
  the (0, 1) corner, then the full.rs/native/engine integration listed in that commit's checklist.
- [ ] **F2. RefreshTimeAnchor** as a recursive kind (only statement tags exist in `statement.rs`).
  *2026-11-18:* E4-F1-F2-DESIGN.md §5 puts it in one Control program shared with Load (no fifth kind; ten programs
  in all with E4 and F1). Not implemented.
  *2026-11-18 (32245ce8ae):* `src/time_anchor.rs` and `src/control.rs` hold the standalone Control relation (Load in
  mode 0, exactly Load's constraints; RefreshTimeAnchor in mode 1 with a root-certified TimeAnchor-role signer and the
  floor as the exact maximum). Cost: 2^20 constraints against 2^18 for Load alone, almost all TimeAnchor signature
  work. Levers need a decision: check the anchor signature hard against a public dummy signer in Load mode (about
  −80k rows, departs from the design's total predicate), or certify a table commitment for TimeAnchor keys (spec
  change, about −50k rows). Integration needs E4 and the geometry census.
- [ ] **F3. The remaining policy signer roles**, including the fee-schedule signer check.
  *2026-11-18 (a96dd1f3f4):* certificates authenticate exactly one role; manifests are signed by a root-certified
  Artifact-role key (wire spec §1, §3.1); the fee-schedule check (RegulatoryPolicy role) exists but no transition
  installs a schedule yet. Open: TimeAnchor (with F2), the Request certificate set (`kgwcset1`), and install paths
  for the other RegulatoryPolicy objects.
- [ ] **F4. Production codecs**: the 532-byte credential and the 30-field Core, per the credential-continuity and
  key-table spec changes (iroha 00fc09fcea, 2180d7956b).
- [ ] **F5. SDK and apps**: Kotlin and Swift surfaces, the explicit root-signed Install that switches a wallet, and the
  wallet-generation plan.

## Owner decisions

- [ ] **D1. Layout: time against bytes** (DESIGN.md §10.3), chosen on E2's measured Payment and E5's Pixel run, not
  on the model.
- [ ] **D2. Generator claims:** fold in clear (decided 2026-11-08; going back needs a reviewed joint-opening argument)
  or keep the private restructuring now in code (one more full-length IPA per side, historically about +1.17 s per
  operation).
- [ ] **D3. Optional Digest-NONE payment signatures** (−26k constraints in Send, −52k in Receive); only if other
  levers fall short, since it changes key parameters, enrollment and attestation.
- [ ] **D4. When a Path 2 wallet generation ships**, after the Gate.

## Records to correct

- [x] **R1. DESIGN.md corrections** from astra's 11-13 review: challenge bounds (G2), native burn status (G4), the
  "total from exact bytes" domain (within an admitted transport profile), burn accounting (Core credit plus adjusted
  burned value), the measured native q (196.6 / 32.2 / 179.9 / 48.9 ms, three-kind Mac), "sampled" rather than
  "identical" matrices, and research-checkpoint numbers labelled as such.
  *2026-11-18:* done. Burn status corrected in §4.3, §11, §12; challenge bounds from SOUNDNESS.md in §5, §7.2, §7.3,
  §11; the other items were already fixed.
