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

---

## Re-check of SOUNDNESS.md revision 2 (2026-11-18)

**Updated verdict.**
- **G1: accept, conditionally.** The argument and its restated assumption are now acceptable. G1 can close once two code findings are fixed and tested and the owner accepts the open items below. The code findings are 4 (unbound io leaves) and the canonicality half of 7.
- **G2: accept, conditionally.** The proofs and bounds are acceptable. G2 can close once finding 5 (build-time LogUp phase check) and finding 9 (forgery refusal through the complete uniform builders) land, and DESIGN.md and TODO.md are synchronized as revision 2 §10 requires.

What I checked, read-only:
- `research/SOUNDNESS.md` as committed in `8672d5ee88` ("Revise … after its independent review").
- Its source citations are pinned to `d4ba3c1a6a`. Between that commit and `8672d5ee88`, `src/` differs only by the new `unload.rs` and `mod unload;` at `lib.rs:56`. I spot-checked citations against HEAD.
- I did not re-review code findings 4, 5, 7 or 9, except for judging the author's partial disagreement on 7. No builds or tests were run.

### Findings on the document

| Finding | Status | Reason |
|---|---|---|
| **1. Scope of P2-RKFS** | **Resolved** (two minor clarifications, R1 below) | §4.2 now has two model assumptions: A, straight-line algebraic representation access on the pinned bases, recorded when an element first enters a native or recursively evaluated RP57 evaluation; and B, idealization of concrete recursive RP57, with ε_RKFS left unquantified and said to be so. A explicitly does *not* assert that representations satisfy relations, that witnesses agree, or that accumulators are true. Prefix knowledge, two-field correspondence, accumulation closure, polynomial extraction cost and query charging are now derived (§4.2 items 1–5). Causal closure is removed. §6.1 no longer uses rewinding over γ. |
| **2. Security experiment** | **Resolved** (residual R2) | §2.1 has the challenger running honest wallets and recording their witnesses and randomness. Node identity is by descriptor plus canonical statement, not proof bytes. The extractor outputs a witness or a pointer to an honest record for the same statement. Each proof occurrence's own obligations are still checked. §2.2 states that no non-malleability is assumed and keys credit uniqueness on the Request-derived id (`receive_lineage.rs:188-215`). Mauled honest proofs are covered because A lets the extractor substitute the challenger's openings. |
| **3. Closure and conservation** | **Resolved** (residual R2) | §4.4 replaces induction with worklist closure over a finite set V, closed under the required parent edges. Truth propagates forward from the native terminal decisions. That is a reachability argument, valid with cycles, and I checked it. §4.3 roots each own chain at Bootstrap through the u128 counter. §4.6 states the corollary Σ S_w = L − F − U ≤ L − F under no forks, no second Bootstrap per credential *or wallet id*, credit uniqueness and authorized funding. I re-derived it: credit raises the balance; burn raises balance and adjusted burned total equally; Send debits principal plus fee against the prior adjusted burn (`send_arithmetic.rs:97-125`, carried forward at `send.rs:247-251`); the Request names payer wallet and ordinal, so each credit maps injectively to one Send under the hypotheses. |
| **6. Restructuring checked only in-circuit** | **Resolved** | §6.8 now says native code decides only the exported claim. Its circuit audit matches `generator_claim_points.rs:160-273`: the digest is taken over the relation, the ordered source points (input cells tied to shared limbs) and their challenge limbs, and all 1+2ℓ masks; D is built from the digest limbs and m₀ and absorbed before μ; C* is built from the same source-point cells and absorbed before the evaluation point and ζ; L and R come before each challenge; the final H-corrected equality is a hard constraint. Comparing own sources with the proof's g_final is a soft equality, made hard for the predecessor through `require_true`. |
| **8. Round-by-round lemma** | **Resolved** (residual R1) | Checked in the next subsection. |
| **10. Degree declarations** | **Resolved** | §6.6, §9 and §10.2 now make the true degree of q − P∘Γ authoritative. Under-declaration is a completeness and advertised-bound issue, not a bypass of exact native evaluation. The new formal-degree checks exist at HEAD (`program_artifact.rs:476-482`, `engine.rs:445-452`). |
| **11. Citations and records** | **Resolved** (editorial residual R3) | Citations are pinned to `d4ba3c1a6a`. The ones I checked are accurate there and at HEAD, except that `lib.rs` references are off by one at HEAD because of `mod unload;`. §10 lists the record updates, including TODO's "3-4-bit" wording, but they are not done yet: DESIGN.md:697 and :747 and TODO.md:21 and :26 still describe DAG induction, causal extraction and a "~3-4-bit loss". This blocks closure but is not a defect in SOUNDNESS.md. |

### Is the restated P2-RKFS minimal, well defined and sufficient?

**Minimal.** Yes. Nothing that a local lemma is meant to prove is assumed. A is needed for straight-line extraction and for the round-by-round state. B is needed for the concrete permutation used inside recursion. Neither can be dropped without supplying a different theorem.

**Well defined.** Yes, with two clarifications (R1):
- Pallas and Vesta allow *oblivious sampling*: pick x, take a square root, and get a point with no known representation. A as written ("the adversary is algebraic") silently excludes such points. State the standard treatment: an obliviously sampled or hash-to-curve point is an extra base with unknown discrete log, so a relation involving it is a DL break.
- B idealizes the duplex transcript as an oracle over its prefix. The actual sponge has a one-element capacity (about 254 bits), so even in the ideal-permutation model there is a generic inner-collision term of about Q²/2^254. List it inside or beside ε_RKFS. It is negligible at Q ≤ 2^64, but it is the only part of B that can be quantified.

**Sufficient for the closure argument.** Yes, given the listed premises:
- A supplies representations for every required parent's commitments. They enter at least the child's recursively evaluated transcript, so they are recorded before that parent's challenges.
- The local lemmas then give each node's witness once its own matrix and generator claims are known true. Those claims are true by propagation from the native terminal decisions.
- The pair link gives two-field agreement.
- Memoization by statement bounds the work by N.
- Encoding divergence between native and circuit (findings 4 and 7) affects equivalence, not this argument: an aliased in-circuit replay is a different oracle input under B. The document rightly keeps equivalence as a separate premise.

**Sufficient for the conservation corollary.** Yes, as a conditional statement (R2):
- Hypotheses 1, 2 and 4 range over *every* wallet in the accounting set, including adversarial counterparties. A receiver's native admission checks the payer's Send pair, not the Load finality or fork-freedom of wallets deeper in the payer's history.
- So the corollary reads: no spendable value beyond finalized Loads unless some joined wallet's custody or native engine fails. That matches the specification's trust model, but §4.6 should say it in those words.
- The telescoping also uses continuity of the adjusted burned total across the Core[10] and lineage BURNED_TOTAL boundary. Send's continuity is source-visible (`send.rs:171-177`, `247-251`); cite Load's and Receive's exports the same way.

### Round-by-round lemma (§8.1) and query bounds (§8.2)

**The lemma is sound at sketch level.** The state is defined in A's recorded view before each fresh reply, and the per-reply events match the protocol order:
- a violated sumcheck recurrence persists until α;
- a recurrence-consistent false claim survives only at a root of degree ≤ 26;
- α then γ;
- IPA, folds and the link;
- LogUp on the joint distribution of its challenge pair.

Charging a shared reply its combined bound is correct, and two halves of one squeeze are not treated as independent.

Residual (R1): write the "doomed" predicate explicitly as a function of (transcript prefix, recorded representations), so the Q·δ* step reads as an instance of the standard theorem. Sketch level is acceptable for G1, but that is the form a later proof will need.

**Bounds checked.**
- δ* ≤ (22,187,384 + 186)·λ*, which is 2^-101.27. It combines the Receive P LogUp term with the matrix-fold β drawn from the same phase-two reply. Every other reply group is smaller at this profile; the next largest is S Receive LogUp at 11.7M·λ*.
- Q·δ* + Σ_v ε_v gives 2^-61.14 at Q = 2^40, N = 2^36, and 2^-37.27 at Q = 2^64. I reproduced both.
- Adding the node sum on top of Q·δ* double-counts the final transcripts. That is harmless and correctly called conservative.
- The second inequality, (Q+N)·max_k ε_k, holds because δ* ≤ max_k ε_k and |V| ≤ N.

### Finding 7: the identity-point disagreement

**The author is right about identity points; I withdraw that half of finding 7.**
- My inference relied on the stale comment at `fs.rs:18-19` and on the non-total path.
- In the complete builders, `generator_fold` sets the builder-wide flag `self.total = true` (`generator_claim_points.rs:168`); so does `verify_keyed_total` (`points.rs:381`).
- With that flag, build uses an input check that admits identity, y² = x³ + b·(1 − [x=0][y=0]), with zero and inverse witnesses (`points.rs:998-1033`).
- Points that must be non-identity are masked to finite: the hard Schnorr A (`points.rs:762-768`, `1029-1033`), matching native `ipa.rs:483-485`.
- Additions, endoscalings and mixed multiplications switch to their complete forms (`points.rs:1176`, `1217`, `1311`, `1346`).
- Both the native transcript and the in-circuit transcript absorb identity as (0,0).
- So I have no forced-burn counterexample from identity rows, and the remaining question is equivalence testing, as revision 2 says.

Residual (R4, editorial/robustness):
- Identity semantics of the hard-predecessor path currently follow from a side effect: whether this circuit happens to run a private generator fold.
- If owner decision D2 switches to clear folding and `generator_fold` disappears, hard verification of Send and Load predecessors reverts to the incomplete path.
- That reversion is harmless for soundness but reintroduces the divergence between native and circuit.
- Make total arithmetic an explicit builder parameter for every recursive verifier, and fix the comment at `fs.rs:18-19`.

**The canonicality half of finding 7 stands.** The author agrees: full-width scalar limbs crossing circuits are recomposed only modulo the field (`full.rs:2846-2875`, `5031-5044`). It remains open and is not re-reviewed here.

### New residual items from revision 2

- **R1 (minor):** in A, treat oblivious sampling and hash-to-curve points as extra bases with unknown discrete log. In B, list the generic duplex inner-collision term (about Q²/2^254). In §8.1, write the doomed-state predicate explicitly.
- **R2 (minor):** in §4.6, state that custody, no-fork and funding hypotheses quantify over all joined wallets, adversarial counterparties included. Cite continuity of the adjusted burned total for Load and Receive as for Send.
- **R3 (editorial):**
  - In §2.1, "exceeds the stated resource bounds" should say the *extractor* exceeding its time bound counts as failure; the adversary's T, Q, N, B are premises.
  - `lib.rs` citations are off by one at `8672d5ee88`.
- **R4 (editorial/robustness):** make identity-capable total arithmetic explicit, as described above.

### Before G1 and G2 close (updated)

1. Code, with tests, outside this document:
   - finding 4: bind every io leaf, and add the unsatisfiable-mutation test;
   - finding 7: canonical cross-field limbs, plus tests of identity ports against native acceptance;
   - finding 5: build-time phase invariant for LogUp;
   - finding 9: CancelRound and AdaptiveWitness refused through the complete uniform builders on both curves.
2. SOUNDNESS.md: R1–R3, which are wording only.
3. DESIGN.md §5, §7.2, §7.3 and §11, and TODO.md G1/G2: synchronize per revision 2 §10, replacing the DAG and causal-extraction text and the "3-4-bit" wording.
4. Re-quote the ledger on one frozen candidate equal to the source then current.

### Owner decisions (unchanged in substance)

- Accept P2-RKFS A and B: an algebraic adversary on Pallas and Vesta, and concrete RP57 idealized inside recursion. Alternatively, fund a replacement theorem.
- Choose history and query budgets and a numeric target. At this schedule: about 2^-61 at Q = 2^40, about 2^-37 at Q = 2^64. Drawing LogUp's challenges from full-width words remains the lever if that is insufficient.
- Accept the corollary's custody hypotheses as obligations of custody and the provider, for every wallet: no forks, no second Bootstrap per credential or wallet id, native Load finality.
