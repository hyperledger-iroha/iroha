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
- [ ] **G2. Written proofs and exact bounds.** Prove γ separation (CancelRound/AdaptiveWitness must stay refused) and
  the deferred matrix claims. Take each bound over the challenge's real distribution (β is a raw 126-bit integer; γ is
  an endoscalar from 126 bits; the pair-link challenge comes from S's Fp transcript), include the lean schedule's
  ~3-4-bit loss, and state the total bits of soundness per operation.
  *Done when:* the proofs are written and DESIGN.md §5/§7.3/§11 quote the corrected bounds.
- [ ] **G3. Independent cryptographic review.** A reviewer outside this lane, working from
  research/REPORT-b04a57-soundness.md, the written G1/G2 arguments and the code.
  *Done when:* every finding is fixed or explicitly accepted by the owner.
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
- [ ] **G6. Issuance parameters.** Pin generator namespace, capacity, column order and table construction in
  authenticated parameters (research/CONSTRUCTION.md).
  *Done when:* a correctly signed credential that carries a wrong table commitment is refused, by test.
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
- [ ] **E4. Shrink P-boot's matrix program** (53,964 products, about 72% of the three P programs). Taking Bootstrap out
  of the per-step polynomial needs an authenticated discharge of its matrix obligation, not a one-time native check.
- [ ] **E5. One complete Pixel 6 Receive**: retained incoming bytes to a verified, durable, releasable output, with
  phase timings, carrier bytes and peak RSS from the same run; then Send, Load and Bootstrap. The Pixel operator runs
  it only with the owner's go typed in the operator's own chat. *Done when:* the 3 s target is judged on these
  numbers.
- [ ] **E6. iPhone measurement** with the Secure Enclave / App Attest device fixture (iOS and Android must behave the
  same).
- [ ] **E7. Two-wallet device runner** (the phone fixture's Bootstrap → Load100 → Send10 → `receive_self` flow is a
  self-transfer and is refused at Send10).

## Missing functionality

- [ ] **F1. Unload** (offline to online) as a recursive kind with its Core path. The proof-before-payout hold on
  suffix-Unload stays.
- [ ] **F2. RefreshTimeAnchor** as a recursive kind (only statement tags exist in `statement.rs`).
- [ ] **F3. The remaining policy signer roles**, including the fee-schedule signer check.
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

- [ ] **R1. DESIGN.md corrections** from astra's 11-13 review: challenge bounds (G2), native burn status (G4), the
  "total from exact bytes" domain (within an admitted transport profile), burn accounting (Core credit plus adjusted
  burned value), the measured native q (196.6 / 32.2 / 179.9 / 48.9 ms, three-kind Mac), "sampled" rather than
  "identical" matrices, and research-checkpoint numbers labelled as such.
  *2026-11-18:* all but the challenge bounds are now in DESIGN.md (burn status corrected in §4.3, §11, §12; the other
  items were already fixed). The bounds wait for G2.
