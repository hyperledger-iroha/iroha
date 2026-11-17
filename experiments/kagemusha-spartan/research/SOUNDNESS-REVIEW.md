<!-- G3 independent review of research/SOUNDNESS.md, written 2026-11-18 by Claude (Opus 5.5) as reviewer outside the G1/G2 lane. Read-only on everything except this file; no builds or tests run. -->

# G3 review of the Path 2 soundness argument (SOUNDNESS.md)

**Verdict. G1: accept with required changes (findings 1–4; 4 is a code fix). G2: accept with required changes (findings 5, 6 and 9). Neither closes before those changes land and the owner decisions at the end are taken.**

Count: 0 critical, 4 major, 5 minor, 2 editorial.

## Scope and what I used

- I reviewed the committed source at iroha `optimizations` HEAD `9ee63f379a` (`git show HEAD:<path>`). The working tree differs from HEAD in, among others, `spartan.rs`, `uniform.rs`, `engine.rs`, `native.rs`, `full.rs`, `r1cs.rs` and `program_artifact.rs`, because other engineers are editing them. All `file:line` references below are HEAD lines under `experiments/kagemusha-spartan/src/` unless stated otherwise.
- SOUNDNESS.md was written at `60309bba27`. Between that commit and HEAD, `engine.rs` changed substantially; `full.rs`, `program_artifact.rs`, `deferral.rs`, `receive.rs` and `verifier.rs` changed slightly. The document's `engine.rs` citations match `60309bba27`, not HEAD (finding 11).
- Checked against the code and found correct (not repeated as findings):
  - challenge distributions: §5.2 atoms (4H+1)/r, 5/(4H+1) and the joint 5/r (`fs.rs:111-129`); endoscalar injectivity and nonzeroness (§5.3, `fs.rs:228-254`); full-width maps (`fs.rs:136-144`, `verifier.rs:2152-2154`);
  - γ is drawn after every row, round commitment, `v` and α (`spartan.rs:1515`, `1658-1659`; verifier `1945-1963`);
  - the Laurent degree 2m and α degree m+1 (`spartan.rs:381-399`, `469-555`, `936-946`);
  - the eq-weighted outer recurrence, with no division (`spartan.rs:481-505`);
  - the Kronecker degree 26 per three-variable block, and the block splits (the short block is last, `r1cs.rs:51-63`);
  - the r, r², r³ test (`spartan.rs:1308-1314`);
  - the IPA folding and Schnorr equations, and the native own-generator MSM (`ipa.rs:437-488`);
  - the matrix fold: q is first-phase; only Γ(β) and q(β) are phase-2 and are defined by linear rows (`deferral.rs:495-631`); padding coordinates of fresh claims are pinned to 0 (`deferral.rs:249-269`, `full.rs:4810-4895`); incoming accumulators are corrected (`full.rs:4682`, `4767-4785`);
  - the pair-link ordering and its native check (`spartan.rs:213-233`, `engine.rs:720-751`);
  - generator sources and corrections (`generator_integration.rs:85-112`, `284-375`);
  - the own-chain counter (`monetary_state.rs:92`, `send_arithmetic.rs:121-126`, `receive.rs:331-338`, `load.rs:351-356`).
- I recomputed the whole numeric ledger from the stated profile:
  - P main proof: 184 + 164 + 3 + 32 + 38 = 421 λ0 plus 21 λ1; S: 395 λ0 plus 21 λ1. The two-proof subtotal is 868.5/H, which is 116.2376 bits.
  - Matrix fold degrees: D_P = 186, D_S = 174. Generator folds: 58 and 55.
  - Link n: I re-derived the 139 verification words per pair by counting the shared values (P proof 71: 49 challenge limbs, 18 scalar limbs, 2 late-digest limbs and 2 R limbs; S proof 68). That gives 783 for Send and Load and 1,539 for Receive.
  - LogUp coefficients, C_k and bits: 104.017 / 101.610 / 101.441 / 100.665.
  - Query example: 2^-60.58, and 2^-36.66 at Q = 2^64.
  - **No arithmetic or counting error found.**

## Findings

### 1. Major: P2-RKFS mixes what it assumes with what the lemmas are meant to prove, and clause 1 is not well defined (§4.2)

**Claim.** Clauses 1–6 together form "the" recursive assumption.

**Evidence.**
- Clause 2 (two-field closure "except for the pair-link failure event bounded below") and clause 3 (accumulation extraction "unless a bounded false-to-true event occurred") restate exactly what §6.6–§6.8 claim to prove. Assuming them makes those lemmas circular. As a result, the assumption is nearly the theorem.
- Clause 1 says openings are "attributable to the transcript prefix at which the associated commitment was fixed". Rows, rounds, L/R and D are perfectly hiding Pedersen commitments, so no prefix information-theoretically fixes an opening. The clause has meaning only relative to a stated extractor view: algebraic representations, or observed oracle queries.
- Clause 5 (expected time (T+Q+NB)·poly with no c^D factor) is plausible only for a straight-line extractor. For any rewinding extractor it is an open problem, which §4.2 admits for "repeated application". As written, clause 5 is a bare assertion that the hard part is solved.

**Required fix.** Restate P2-RKFS minimally, for example:
- (A) Straight-line extraction in the style of the algebraic group model. Every Pallas or Vesta element that the adversary outputs and that later enters an accepted transcript (rows, round commitments, L/R, Schnorr A, restructuring D, L/R, claim points) comes with a representation over the pinned (g_i, H, U). The extractor reads that representation at the first RP57 transcript evaluation containing the element.
- (B) The concrete RP57 transcript, evaluated natively and in-circuit, may be treated as an ideal oracle for the round-by-round experiments of §5–§7, with Q queries.

Then derive the following from (A), (B), DL on both curves and the lemmas:
- clause 1 (from (A));
- clause 2 (from (A) and §6.7);
- clause 3 (from (A) and §6.6/§6.8);
- clause 5 (straight-line extraction composes as N·poly);
- clause 6.

Clause 4 is dropped (finding 3). This leaves one decision for the owner: algebraic adversary plus concrete Poseidon in recursion. That is the same heuristic class as Halo/Nova-style deployments, instead of a bespoke six-part statement.

### 2. Major: the security experiment is undefined, in particular honest wallets and their proofs (§2, §4.2 clause 4, §4.4)

**Claim.** §2 says an accepted pair has an extractable history. Clause 4 mentions "a previously supplied honest proof with an already recorded history".

**Evidence.**
- Honest payers' Send pairs are public (Payments) and are routinely verified as Receive's second parent. Their witnesses are hidden by zero knowledge and cannot be extracted.
- §2 and the §4.4 induction table never introduce an honest-wallet oracle, never say who runs the honest wallets in the reduction, and never treat honest outputs as leaves.
- Spartan/IPA proofs are not argued to be non-malleable. A mauled honest proof is a new byte string for the same statement. A memoization or identity rule based on proof bytes (clause 5's "canonical proof/statement identities") needs this case defined.

**Required fix.** Define the game:
- The challenger (and so the extractor) runs the honest wallets and knows their witnesses and proof randomness. The adversary gets an honest Bootstrap/Load/Send/Receive oracle and corrupts wallets only within the custody model.
- Success means native acceptance of a pair whose extracted node set contains a node that is neither a satisfying witness nor an honest-oracle statement.
- The extractor outputs, per node, a witness or a pointer to an honest-oracle output for the same statement. Identity is by statement and descriptor, not proof bytes.

Note in the text that credit uniqueness keys on the Request-derived credit id (`receive_lineage.rs:208-210`, `479`), not on proof bytes, so proof malleability cannot double-credit.

### 3. Major: causal closure (clause 4) and a "global rank" are not needed, and the monetary conclusion is never stated (§4.3, §4.4, §9 row 2, §11 item 1)

**Claim.**
- §4.3: the missing rank on payer edges "is a missing proof hypothesis".
- §4.4: induction "in topological order".

**Evidence.**
- With memoized extraction (already in clause 5), the extracted node set V is finite and closed under parent edges, whether or not a payer-edge cycle exists.
- Every claim in §2 is local to a node (each obligation is supported by an extracted node satisfying its relation), except rootedness. Rootedness follows from own edges alone: Core[11] is u128-bounded (`monetary_state.rs:92`) and strictly increases (`send_arithmetic.rs:121-126`, `receive.rs:331-338`, `load.rs:351-356`). So following own-predecessor edges inside the finite V must end at a Bootstrap.
- Value conservation over V needs no acyclicity:
  - Σ final balances = Σ Loads + Σ credits − Σ debits − fees, summed over each wallet's own chain rooted at a zero-balance Bootstrap.
  - Credits map injectively to Send nodes in V ∪ (honest leaves): through the Request-bound recipient and the per-chain consumed map.
- A payer-edge cycle (an earlier Receive crediting a Send whose ancestry includes a later Send of the same wallet) is a "time-travel" credit. It cannot create value under these equations, because every credit is matched to a real debit in V.
- Meanwhile the document never states the theorem the owner actually needs: no spendable value beyond Loads (net of fees and burns).

**Required fix.**
- Replace the induction with a closure argument over V, followed by a stated conservation corollary. Its hypotheses: no forks among extracted nodes, **including no second Bootstrap for the same credential or wallet id** (a re-Bootstrap starts an empty consumed map, so it is a fork for replay purposes; §2 item 3 must feed into this hypothesis); per-chain replay nonmembership; recipient binding; native Load finality.
- Delete clause 4.
- Remove "no rank on incoming-payer edges" as an open item; no global rank needs to be added.

### 4. Major: in-circuit io leaves at unbound io slots are unconstrained, so a receiver can force the burn branch on a valid Payment, and native and in-circuit acceptance diverge (§4.5, §11 item 4)

**Claim.** §4.5: "Receive's incoming validity is a constrained conjunction, not a caller-selected permission to burn". DESIGN §4.3: "Neither the receiver nor the host chooses the branch."

**Evidence** (derived from source at HEAD; not yet confirmed by a test).

How a verified proof's io leaves are bound:
- The verifier circuit holds a verified proof's io as the leaves of the scalar side's io fold, assigned from `io_full` (`scalar.rs:221`, `790`).
- `io_full` = ordinary io + link io + late io, zero-filled in between (`verifier.rs:898-906`, `spartan.rs:288-297`, `r1cs.rs:237-246`).
- Leaves are bound only by:
  - the ordinary io digest over slots `0..inst.io.len()` and the late digest over `late_slots` (`full.rs:3895-3922`, `4970-5045`);
  - the link ties at slots 124–127 (`full.rs:4129-4166`);
  - zero padding only for slots ≥ `io_len` (`scalar.rs:779-787`).

Which slots are left free:
- P: ordinary io is 64 words (the retained carrier logs "ordinary io 2048 + 1984" B). The matrix accumulator is late io 64..118 (n = 54 plus its value, `deferral.rs:676-713`, `full.rs:4900`). The generator claim is 160..181 (`generator_integration.rs:208-237`). So **slots 119–123 and 128–159 are free cells** for every verified P proof.
- S: similarly free, for example slots 62–63, 118–123, 125, 127 and 128–159 (S's link io is single cells at 124 and 126, `full.rs:5190-5191`).

Consequences:
- No matrix column reads these slots: reads must hit present instances (`uniform.rs:1700-1716`). So a proof made over z with nonzero entries there proves the same R1CS. There is **no false-statement acceptance**.
- But changing such a leaf changes io~(r_y), hence κ, hence the soft Schnorr equality of the total verifier (`points.rs:369-386`). The bit `tr_p.accepts` then becomes 0 for a valid payer pair, and that bit is bound into the verdict (`full.rs:4241-4248`, `4278-4302`; `receive.rs:232`).
- So a receiver can select burn for a valid Payment. Native admission credits that Payment (`engine.rs`, derived verdict), so the circuit and native verdicts disagree. The hard predecessor slot likewise accepts proofs that native verification rejects.

This is not value creation: the burned amount stays out of spendable value. It does falsify the OQ-3 property that SOUNDNESS.md and DESIGN.md assert, and it is a concrete counterexample to §11 item 4.

**Required fix.**
- Constrain every io-fold leaf in `[0, io_len)` that no digest or link tie covers to zero: generalize `scalar.rs:779-787` to the complement of the covered slots. Alternatively, make the digests cover all of `io_full`.
- Add a mutation test: set one uncovered leaf of a valid incoming pair to 1 in a total verification. The witness must become unsatisfiable, not verdict 0.

### 5. Minor: the LogUp phase invariant holds by inspection but nothing enforces it, and the lemma's hypothesis is stated loosely (§6.9, §9 row 6)

**Audit result at HEAD.**
- The only phase-2 cells are:
  - lookup helpers (`uniform.rs:1533-1541`);
  - table helper cell 1 (`1630`);
  - copy-entry helper cell 1 (`1524-1526`);
  - the matrix fold's Γ(β) and q(β) cells (`deferral.rs:606-609`).
- Table and copy multiplicities are first-phase, or link-phase for link tables (`uniform.rs:1630-1638`, `2579-2622`).
- Copy addresses and read on/off masks are constant columns.
- Γ(β) and q(β) are read by no lookup or copy tuple (only by linear late-io ties).
- So **no violation was found**.

**On wrap and zero denominators.**
- A queried tuple outside the table has residue equal to its query count, which is below the characteristic. Prover-chosen multiplicities therefore cannot cancel it, and multiplicity "widths" are not constraints (`spartan.rs:662-664`), which membership semantics do not need.
- A table-side zero denominator makes h_t free when m_t = 0. That event is the s·H term already counted.

**Gaps.**
- No build-time assertion forbids a lookup, export or import tuple from reading a phase-2 cell or a late-io array. A copy tuple on phase-2 cells would not even break honest completeness, because copy multiplicities are structural. Two adaptive values then defeat the single LogUp equation.
- The `ChallengeRows` doc invariant ("every cell a row reads with a challenge factor must be fixed by the first phase", `uniform.rs:211-213`) is already broken, harmlessly, by the Horner row reading q(β)'s own cells with factor −β (`deferral.rs:594-602`).
- The lemma's hypothesis should exclude phase-2 cells that are *determined* by the challenges, not only free ones.

**Required fix.**
- Add the assertion in `UniformCircuit::build_with_copies_at`. Allow challenge-factor reads of phase-2 cells only in rows that triangularly define those cells.
- Restate §6.9's hypothesis accordingly, and record the audit.

### 6. Minor: the generator restructuring is checked only in-circuit, but §6.8 says native replay checks it (§6.8)

**Claim.** "Native replay checks the digest opening and these equations (`generator_claim.rs:358–408`)."

**Evidence.**
- `gc::trace` (`generator_claim.rs:342-408`) needs the private masks. It is the prover's reconstruction.
- Native acceptance only decides the exported claim (`engine.rs:786-814`).
- The restructuring's soundness hypotheses therefore rest entirely on the same pair's circuits (`generator_claim_points.rs`, `generator_claim_scalar.rs`), which §6.8 cites but does not audit. Those hypotheses are: D absorbed before μ, every mask inside the digest, C* before r, ζ, and the exact H-mask recurrence.

**Required fix.**
- Correct the sentence.
- Make the in-circuit restructuring an explicit audit item. In particular, confirm that the input digest hashed in-circuit is computed from the very source cells tied to the proof ports (`generator_integration.rs:284-375`).
- Keep §9's field-semantics note for its limbs.

### 7. Minor: equivalence of native and in-circuit decoding is unverified for two concrete encodings (§4.5, §6.7, §11 item 4)

**Evidence.**
1. **Full-width scalar limbs.** The nine full-width scalars per proof (io digest, v_A, v_B, v_C, κ′, …) cross circuits as two 128-bit limbs. The digest row checks only lo + 2^128·hi ≡ d modulo the scalar field (`full.rs:5024-5045`). I found no canonical check (lo + 2^128·hi below the modulus) for these limbs, as exists for challenge words (`points.rs:1624-1786`) and claim coordinates (`1522-1621`). If they are absent, the in-circuit transcript of a verified proof can absorb aliased limbs that native never absorbs. That gives up to about 4^9 alternative transcripts per verified proof, which is extra grinding and divergence, not a direct forgery.
2. **Identity points.** Native parsing accepts identity points: only the IPA's A is refused (`ipa.rs:483-485`). The in-circuit decoder admits identity (`proof_bytes.rs:573-618`), but the points side "cannot allocate" (0,0) (`fs.rs:18-19`). A malicious payer can make a valid Payment with an identity row commitment, for example an empty row with blind 0. Its native and circuit verdicts may then differ.

**Required fix.** Establish canonicality for every cross-field full-width limb pair (or charge the alias factor in §8), define identity handling identically on both sides, and add both cases to the native-versus-circuit equivalence tests.

### 8. Minor: the query charge in §8 presupposes round-by-round soundness that is not stated (§8)

**Claim.** Adv ≤ … + (Q+N)·max_k ε_k.

**Evidence.** This is a valid, if loose, Fiat–Shamir bound only if the interactive protocol is round-by-round sound with the per-challenge errors of §5–§7. That property itself needs the straight-line knowledge of finding 1(A) to define the state function over commitments. A per-query charge should multiply Q by the largest single-challenge error (here the Receive P LogUp test, about 2^-101.3), not by the per-kind sum. The result is numerically similar here because LogUp dominates.

**Required fix.** State the round-by-round lemma and its dependence on 1(A), and quote both forms.

### 9. Minor: the CancelRound/AdaptiveWitness evidence covers the native verifier and a toy synthesized verifier only (§6.1)

**Evidence.**
- The regression `lib.rs:163-180` uses `spartan::verify` with the toy circuit.
- The in-circuit refusal test (`verifier.rs:2344-2461`) uses a small toy shape through the synthesis path.
- §3.4 itself says census and synthesis machinery is not the uniform builders that `full.rs` instantiates.

G2's "must stay refused" applies equally to the recursive verifier.

**Required fix.** Add refusal of both forgeries as verified proofs inside the uniform complete builders: the points side's γ-Horner over round commitments and the scalar side's slot weights.

### 10. Editorial: the severity of imported degree declarations is overstated (§9 row 4, §10.2)

q is committed before β and native decision evaluates the authenticated program exactly. An under-declared degree breaks honest folds (completeness), and soundness degrades only to (true degree)·λ_β. The soundness-relevant item is program identity, which is already digest-pinned, not the declared degree. Restate the row.

### 11. Editorial: stale citations and records (header, §3.2, §4.5)

- The `engine.rs` line numbers refer to `60309bba27`. At HEAD:
  - `verify_pair` is `engine.rs:720-765`;
  - `matrix_accumulator` is `771-779`;
  - `decide_accumulators` is `786-814`.
- Re-cite every reference against the frozen candidate (§11 item 2).
- TODO G2's "~3-4-bit loss" should quote §6.3's figure: 2.115 bits relative to independent coordinates.

## What must change before G1 and G2 can close

1. SOUNDNESS.md:
   - restate P2-RKFS minimally and derive clauses 1–3, 5 and 6 (finding 1);
   - define the experiment with honest wallets and identity by statement (finding 2);
   - replace the induction with closure over V plus a stated conservation corollary whose custody hypothesis includes no second Bootstrap per credential, and delete clause 4 (finding 3);
   - add the round-by-round lemma (finding 8);
   - correct §6.8 (finding 6) and §9 rows 2 and 4 (findings 3 and 10).
2. Code, then tests:
   - pin the unbound io-fold leaves (finding 4);
   - enforce the LogUp phase invariant at build time (finding 5);
   - canonical cross-field limbs and identity handling (finding 7);
   - in-circuit forgery refusals on the uniform builders (finding 9).
3. Re-run the ledger on a frozen candidate equal to HEAD; the §7 numbers are unaffected by these fixes. Then re-quote it in DESIGN.md §5, §7.2, §7.3 and §11.

## Owner decisions

- **Accept the restated P2-RKFS**: an algebraic adversary on Pallas and Vesta, plus concrete RP57 as an oracle in recursion. Alternatively, fund a proof. Without this, G1 cannot close: no DL-only theorem is available.
- **Lifetime and query budgets, and a numeric target.**
  - At the current schedule the per-operation bound is LogUp-dominated, at about 2^-100.7 (Receive).
  - That gives about 2^-60.6 at Q = 2^40, N = 2^36, but only about 2^-36.7 at Q = 2^64.
  - If that is insufficient, one option is to draw the LogUp challenges A and B from full-width words, as τ and ρ already are. The LogUp term would drop to about (d+1)·M/p ≈ 2^-229, and the main proofs would dominate at about 2^-115.3. The in-circuit cost is unmeasured.
- **The custody hypothesis in the conservation corollary**: no forks, and no re-Bootstrap of a credential or wallet id. These are custody and provider obligations, not proof properties.
- **Interim status of finding 4**: until it is fixed, the "receiver cannot choose burn" statement in DESIGN.md §4.3 and SOUNDNESS.md §4.5 should be withdrawn, or qualified as holding only for the native verdict.
