<!-- Written by Codex astra (gpt-6-astra, ultra reasoning), read-only, for TODO G1/G2. Revision 2, 2026-11-18, at iroha d4ba3c1a6a, answering research/SOUNDNESS-REVIEW.md; prompts and logs in ../../../output/kagemusha-100x-20261108/astra-g1g2-20261118/ and astra-g1g2-rev-20261118/ (local, git-ignored). Saved unedited. -->

<!-- Revised read-only on 2026-11-18 against iroha HEAD d4ba3c1a6af58065c74bbd8b5fab6ae698afb7ab, in response to research/SOUNDNESS-REVIEW.md. No repository files were changed and no builds or tests were run. -->

# KAGEMUSHA Path 2: conditional soundness argument and release obligations

**Status: not approved for wallet use.** This document supplies a conditional recursive argument and explicit local algebraic bounds. It does not establish security of the complete implementation from discrete logarithm hardness alone. Acceptance of the recursive model, implementation corrections, compiler correspondence, finite security budgets, and custody qualification remain required before G1–G3 can close.

## 1. Scope and evidence

This revision was prepared read-only on 2026-11-18 in `/Users/takemiyamakoto/dev/iroha`. The final inspected HEAD was:

```text
d4ba3c1a6af58065c74bbd8b5fab6ae698afb7ab
Add the independent review of the KAGEMUSHA Path 2 soundness argument
```

Source inspection began at `911ebf37abd006bf1d1ace37d9557ce3bc196016`. The subsequent commit above changed only `DESIGN.md`, `TODO.md`, and `research/SOUNDNESS-REVIEW.md`; the inspected source and its line numbers are identical. All source citations below refer to the final HEAD, not to in-progress working-tree edits.

No files were changed, builds performed, or test suites executed. Source and existing evidence were inspected, and the numerical ledger was recalculated. Unless explicitly rooted at the repository, paths below are relative to `experiments/kagemusha-spartan/`.

The evidence includes:

- `TODO.md`, particularly G1–G3.
- `DESIGN.md`.
- `research/SOUNDNESS-REVIEW.md`, whose verdict is acceptance of G1 and G2 with required changes.
- `research/REPORT-b04a57-soundness.md`.
- `research/CONSTRUCTION.md`.
- Existing complete-backend artifacts and logs.

The numerical evaluation retains the **2026-11-17 complete-backend `p18-s17` candidate**: artifacts under `output/full-backend-run-20261117T051751Z/` and log `output/s2/chain-20261117T051751Z.log`. The independent review recomputed this ledger without finding an arithmetic or counting error. Correspondence of that retained candidate to current HEAD remains **UNVERIFIED**.

Current HEAD contains checks absent from the original document’s source revision: authenticated generator/table profile checks and formal matrix-degree checks. These are identified as source-verified below. Findings 4, 7, and 9 are not represented as fixed by work occurring outside this frozen commit.

The following distinctions apply throughout:

- **Proved conditional lemma:** a mathematical implication under explicitly stated hypotheses.
- **Source-verified:** an implementation fact established by source inspection.
- **Assumption:** a model or security property not proved here.
- **UNVERIFIED:** a required correspondence, test result, or qualification not established by this review.

## 2. Security experiment, claim, and exclusions

### 2.1 Experiment

Fix an authenticated profile \(\Pi\), the native accepting predicate of §3, and the intended four operation relations.

The challenger installs the profile and operates honest wallets. The adversary may request honest Bootstrap, Load, Send, and Receive operations, choose permitted inputs and counterparties, obtain public outputs and proofs, and schedule Payment delivery. Honest Load operations require the native funding-finality predicate. The challenger records the witnesses, proof randomness, authenticated statements, and parent references of every honest operation.

The adversary may also construct arbitrary statements and proof attempts within the algebraic-adversary model of §4. This includes malformed objects, failed attempts, replay, forks, and adaptations of honest proofs. Corruptions follow the specified custody model. The conservation corollary applies only to executions satisfying its explicit custody hypotheses.

A logical proof node is identified by its **authenticated descriptor and complete canonical public statement**: the profile, operation kind, public operation inputs, and exported matrix and generator claims needed to specify its obligations. Identity is **not by proof bytes**. Different proof byte strings for the same descriptor and statement identify one logical node. A pointer to an honest-oracle record is permitted only for that same descriptor and statement.

The extractor must output a finite set \(V\), with each node accompanied by either:

1. A satisfying witness for its selected operation relation and references to its required parents; or
2. A pointer to the challenger’s recorded honest output for the same descriptor and statement.

The set is closed under hard own-predecessor edges and incoming-payer edges required for successful credit. Invalid incoming evidence assigned to burn is retained as evidence; it is not promoted to a valid payer node. Honest-record pointers may abbreviate extraction, but their recorded histories are available and included whenever closure or monetary accounting requires them.

Every encountered proof occurrence still requires its actual transcript and accumulator obligations to be checked. Statement memoization does not excuse a modified proof’s different proof-specific generator claim or other verification obligations. Proof occurrences and grinding are included in the query and computation budgets even when their statements have already been recorded.

The adversary wins if it produces a natively accepted pair for which extraction fails, exceeds the stated resource bounds, or leaves a required node without a satisfying witness or same-statement honest pointer, or fails the required closure and correspondence checks. This defines the extraction experiment; it does not assume the existence of the required witnesses.

### 2.2 Intended conditional claim

> Subject to P2-RKFS, the stated cryptographic assumptions, and the implementation hypotheses, a natively accepted pair has a finite, efficiently obtainable, parent-closed set of operation witnesses or recorded honest statements. Every hard predecessor and successful incoming-credit obligation is supported by that set. Deferred obligations cannot disappear except through the bounded local algebraic events.

Section 8 bounds the experiment’s failure probability conditionally. The implementation hypotheses are **not established for current HEAD**; §9 includes concrete outstanding findings.

Honest proofs are not extracted from public zero-knowledge proof bytes alone. Their witnesses are available because the challenger generated them. No proof non-malleability assumption is made.

Credit uniqueness uses the **Request-derived credit identifier**, not a proof hash. The relation searches the consumed map using that identifier and constrains a successful credit’s insertion to the same key (`src/receive_lineage.rs:188–215`). Changing proof bytes therefore does not create a new replay key.

### 2.3 Exclusions

The extracted-history claim alone does not establish:

1. **Global uniqueness of spending.** Two Sends from one funded predecessor can each satisfy the relation. Preventing such forks requires trusted execution and custody through concurrency, restart, rollback, and recovery.
2. **In-circuit Load finality.** The relation binds the receipt and monetary effect; native code checks funding finality.
3. **Globally unique Bootstrap.** Enrollment uniqueness, including refusal of a second Bootstrap for the same credential or wallet identifier, is a provider/custody obligation.
4. **Anonymity or unlinkability.** No joint zero-knowledge theorem is established here for recursive exports, private generator restructuring, or retries.
5. **Security for infinite attempts.** Every quantitative statement has finite history, computation, and query budgets.

The checked debit equations are at `src/send_arithmetic.rs:90–147`. The Load carve-out is explicit at `src/load.rs:1–7`, and native finality checks are at `src/engine.rs:1386–1406`. Custody responsibilities are explicit at `src/native.rs:1–6`.

## 3. Construction and accepting predicate

### 3.1 Fields, curves, and authenticated programs

Write \(p<q\) for the Pasta primes:

```text
p = 0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001
q = 0x40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001
```

P proofs have scalar field \(\mathbb F_p\) and Vesta commitments, whose coordinates lie in \(\mathbb F_q\). S proofs have scalar field \(\mathbb F_q\) and Pallas commitments, whose coordinates lie in \(\mathbb F_p\). Definitions are at repository-root `crates/iroha_pasta/src/field/fp.rs:7–16` and `fq.rs:7–16`.

The four kinds are Receive, Send, Bootstrap, and Load. Two Boolean selectors choose their programs, including the bilinear fourth-corner term for Load (`src/points.rs:1058–1124`; `src/full.rs:4747–4758`).

Generators are derived by curve-specific hash-to-curve from `kagemusha-zks-spike/gens`: \(g_i\) at index \(i\), \(H\) at capacity \(N\), and \(U\) at \(N+1\). Prefixes retain the parent’s \(H,U\), so capacities and prefix rules are security parameters (`src/ipa.rs:24–26`, `50–81`).

**Source-verified:** the current manifest names the eight program digests, matrix dimensions, generator namespace and derivation profile, capacities, derived-set digests, and table construction/known answer. Admission recomputes and compares these identities (`src/engine.rs:53–84`, `198–228`, `430–456`). Generated operation programs are also compared with the installed programs (`src/engine.rs:1751–1754`).

These checks authenticate the selected implementation. They do not prove that its compiler and circuits implement the intended monetary relation.

### 3.2 Native acceptance

The central accepting predicate is `src/engine.rs:720–765`. It:

1. Selects the authenticated operation programs.
2. Verifies both linked Spartan proofs.
3. Checks the reconstructed pairing digest and challenge.
4. Decides both accumulated matrix claims.
5. Decides both inherited generator claims.

Matrix decision exactly evaluates the installed combined polynomial (`src/engine.rs:771–778`). Both matrix and inherited-generator decisions are mandatory (`src/engine.rs:786–811`).

Each linked Spartan verification also decides its **own** IPA generator claim by recomputing the public MSM (`src/spartan.rs:1973–1976`; `src/ipa.rs:469–481`).

Thus terminal acceptance includes two main proofs, two matrix decisions, two own-generator decisions, two inherited-generator decisions, and pair-link/profile checks. A lower-level linked-proof success alone is insufficient.

Inspected paths include head decoding and verification, Payment verification, fresh output acceptance, output revalidation, and recovery (`src/engine.rs:856–887`, `1498–1515`, `1537–1593`, `1605–1634`, `1809–1821`, `1883–1918`). Complete adversarial qualification of every exported caller and failure/recovery path remains **UNVERIFIED**.

### 3.3 Commitment and challenge order

The main proof schedule is:

1. Absorb shape and ordinary-public-input digest.
2. Absorb first-phase row commitments.
3. Derive the pair-link word.
4. Absorb link-phase row commitments.
5. Derive the two phase-two challenges.
6. Absorb late-output digest, remaining rows, fixed rows, and link \(R\).
7. Derive the initial sumcheck point.
8. Absorb each outer-round commitment before its challenge.
9. Absorb \(v_A,v_B,v_C\), then derive their batching challenge.
10. Absorb each inner-round commitment before its challenge.
11. Derive equation-batching \(\alpha\), then separation \(\gamma\).
12. Run the IPA, absorbing messages before their associated challenges.

Sources: `src/spartan.rs:299–304`, `1449–1515`, `1551–1575`, `1593–1597`, `1633–1659`; verifier replay at `1914–1976`.

Late IO follows the phase-two challenges because it includes the resulting accumulator. Its correctness rests on a previously committed fold polynomial and constrained evaluation.

### 3.4 Recursive implementation

Scalar arithmetic and group/transcript arithmetic are split across the two circuits. The actual uniform builders are `src/scalar.rs` and `src/points.rs`; `src/verifier.rs` also contains tracing and census machinery.

Relevant actual constraints include:

- Gamma inversion, alpha powers, and \(r,r^2,r^3\): `src/scalar.rs:808–846`.
- Canonical squeezed transcript words: `src/points.rs:1624–1786`.
- Canonical generator-coordinate limbs: `src/points.rs:1522–1621`.
- Shared verification values: `src/verifier.rs:2046–2100`.

These checks do not establish canonicality for every full-width scalar crossing the circuits; see finding 7 in §9.

Counting a check in a census is not evidence that the uniform circuit constrains it. The counted comparisons at `src/verifier.rs:245–257`, `362–416` cannot substitute for actual-builder inspection and testing.

## 4. G1: recursive model and closure argument

### 4.1 No composition theorem is imported implicitly

No named PCD theorem has been checked hypothesis by hypothesis for this implementation.

Accumulation and split accumulation provide relevant frameworks, but their use requires the appropriate knowledge and accumulation properties for the actual protocols. See [BCMS, *Proof-Carrying Data from Accumulation Schemes*](https://eprint.iacr.org/2020/499) and [BCLMS, *Proof-Carrying Data without Succinct Arguments*](https://eprint.iacr.org/2020/1618).

The recursive circuits evaluate a concrete hash permutation. An ordinary black-box random-oracle argument does not establish security for that concrete recursive use. [*Halo Infinite*](https://eprint.iacr.org/2020/1536) likewise identifies the non-black-box concrete Fiat–Shamir step in this methodology as heuristic.

The following model is therefore an explicit additional assumption requiring owner acceptance.

### 4.2 Minimal assumption P2-RKFS

A frozen profile \(\Pi\) identifies:

- Fields, curves, generators, capacities, prefix rules, and table ordering.
- Eight exact programs and their interpretation.
- Proof/carrier encodings.
- RP57 constants, actual transcript domains, positional schedules, and challenge maps.
- Matrix and generator accumulation.
- Native, hard-predecessor, and total-incoming verification predicates.

Use finite resource parameters:

- \(T\): adversarial computation.
- \(Q\): relevant oracle-query/reply budget, including grinding, failed attempts, proof variants, and recursively encountered evaluations.
- \(N\): distinct reachable statement nodes, including relevant recorded honest history.
- \(B\): maximum encoded node, witness, and transcript size under \(\Pi\).

**P2-RKFS consists of two model assumptions.**

**A. Straight-line algebraic representation access.** The adversary is algebraic with respect to the pinned Pallas and Vesta commitment bases. Every adversarial group element entering a relevant native or recursively evaluated transcript has an algebraic representation over the appropriate authenticated bases. This includes witness-row and round commitments, IPA \(L,R\), Schnorr \(A\), restructuring \(D,L,R\), and claim points.

The extractor receives and records the representation when the element first enters the corresponding RP57 evaluation, before the resulting challenge. Honest proof elements and authenticated inputs are handled using the challenger’s records and known construction. Representations are composed straight-line; no replay with changed challenges is assumed.

This specifies the extractor’s view. It does **not** assert that an arbitrary representation is a satisfying witness, that unwanted basis coefficients vanish, that two circuit witnesses agree, or that source accumulators are true. Those conclusions require the accepting equations and local lemmas, excluding nontrivial relations among the authenticated commitment bases.

**B. Concrete recursive RP57 idealization.** The exact native and in-circuit RP57 evaluations of \(\Pi\) may be analyzed as ideal-oracle evaluations for the round-by-round experiments below. This preserves repeated-input consistency, positional transcript structure, and the correlations between halves of one squeeze.

Write the additional concrete-instantiation discrepancy as

\[
\epsilon_{\mathrm{RKFS}}(\Pi,T,Q,N,B).
\]

No numerical bound for this term is established here. Ignored textual tags do not acquire domain separation through this assumption; the actual positional schedule must be analyzed.

This model is not a DL-only theorem. In particular, B explicitly includes the heuristic treatment of a concrete permutation inside recursion.

The following are **derived conditional conclusions**, rather than additional assumptions:

1. **Prefix knowledge.** A records algebraic coefficients before the relevant challenge. The IPA and commitment equations identify the required openings, except for the local algebraic events or a nontrivial generator relation. Perfectly hiding Pedersen commitments do not information-theoretically determine an opening. “Fixed before the challenge” refers to the recorded extractor view and computational consistency.
2. **Two-field correspondence.** Section 6.7 forces equality of the relevant extracted field coefficients except for its pair-link event. Byte-level correspondence additionally requires canonical encoding and the IO-binding obligations in §9.
3. **Accumulation closure.** Sections 6.6 and 6.8 make a true output imply true required source obligations except for their false-to-true events. Exact native terminal decisions start this propagation.
4. **Polynomial extraction cost.** Record representations, process proof occurrences, and memoize logical statements in a worklist. Under the stated size budgets, this takes
   \[
   (T+Q+NB)\operatorname{poly}(\lambda,\Pi)
   \]
   time, including relevant honest-record processing. There is no depth-exponential rewinding factor. A wallet counter does not itself establish these resource budgets.
5. **Adaptive-query accounting.** B and the round-by-round lemma of §8 justify charging bad challenge replies and applying a union bound.

There is no causal-closure clause and no global-rank assumption.

### 4.3 Own-chain rootedness

`Core[11]` is constrained to an unsigned 128-bit integer (`src/monetary_state.rs:91–93`). Bootstrap initializes it to zero (`src/bootstrap.rs:260–278`). Send, Receive, and Load increase it by one (`src/send_arithmetic.rs:121–126`; `src/receive.rs:332–337`; `src/load.rs:351–356`). The before-state is joined to the authenticated own predecessor (`src/operation_auth.rs:333–345`).

The range bounds exclude field wrap. Every own-predecessor edge therefore decreases a nonnegative integer. In a parent-closed set, following these edges must end at Bootstrap, the only operation without an own-predecessor obligation.

Incoming-payer edges need not decrease the receiver’s sequence (`src/receive_originals.rs:424–439`). The combined graph can contain payer-edge cycles without invalidating the closure or conservation arguments below.

This does not establish that cyclic proofs can be constructed or that they respect real-time causality. It establishes that this theorem does not need an additional global rank.

### 4.4 Closure over the extracted node set

Condition on:

- P2-RKFS.
- Correct authenticated parameters and compiler semantics.
- The stated native/circuit correspondence.
- No cryptographic binding, hash, or signature failure.
- No local algebraic failure event counted below.

Start from the natively accepted terminal pair and its exact matrix and generator decisions. Read its recorded algebraic representations. The local Spartan argument supplies the selected relation witness, and pair linking supplies correspondence between the two field computations.

For each required parent occurrence, propagate the accumulator obligations and apply the local proof argument. Record an extracted witness or a same-statement honest pointer. Add previously unseen statements to a worklist; reuse previously recorded statements while still checking each occurrence’s actual obligations.

The result is a finite parent-closed set \(V\) within the resource budget. Cycles revisit an existing node; they do not require induction over a topological order.

| Kind | Local facts required at the node |
|---|---|
| **Bootstrap** | Authenticated root/issuer/credential and initial state; zero initial balance and counters; required empty roots; constant true matrix and inherited-generator outputs. |
| **Load** | Valid own predecessor; exact receipt; correct amount, ordinal, recovery effects, and signed successor. Funding finality remains a native premise. |
| **Send** | Valid own predecessor; authenticated Request and applicable controls/maps; checked integer debit and fee; state preservation and recipient binding. |
| **Receive, credit** | Valid own predecessor and payer Send node; original-object matching; Request authentication; consumed-map nonmembership; correct credit transition. |
| **Receive, burn** | Valid own predecessor; exact retained incoming evidence; constrained burn accounting and map behavior. Invalid incoming evidence need not have a valid payer history. Correct classification additionally requires native/circuit equivalence. |

Application anchors are `src/bootstrap.rs:232–319`, `392–468`, `488–553`; `src/load.rs:305–414`; `src/send.rs:163–244`; `src/send_arithmetic.rs:90–179`; `src/receive.rs:165–249`, `327–380`; `src/receive_originals.rs:336–475`; and `src/receive_lineage.rs:186–253`.

Truth propagates from terminal obligations through every reachable required edge using the local no-false-to-true implications. This is closure under implications, not induction on graph acyclicity. Section 4.3 separately establishes that each wallet’s own chain is rooted at Bootstrap.

The extracted-history claim follows **conditionally**. Its implementation premises are not all satisfied or qualified at current HEAD.

### 4.5 Receive containment and native finality

The Receive relation combines incoming proof, object, and signature verdicts, then includes replay nonmembership (`src/receive.rs:229–249`; `src/receive_lineage.rs:186–207`).

That conjunction alone does **not** prove that the receiver cannot choose burn. At current HEAD, uncovered interior IO-fold leaves are unconstrained. A receiver can alter such a leaf, change the verifier’s scalar result, and force the soft acceptance bit to zero for an otherwise valid Payment. The source supports finding 4; its concrete mutation test was not executed in this review. The unqualified in-circuit “receiver cannot choose burn” claim is withdrawn.

Native admission derives its verdict from Payment parsing and full verification, distinguishing resource/unavailability errors from invalidity (`src/engine.rs:1108–1162`, especially `1133–1141`). This native fact does not establish circuit equivalence.

Burn accounting raises the Core balance and adjusted burned value by the same amount, adding no spendable value. Existing credit-digest records are retained. The invalid branch permits an optional structurally valid consumed-map insertion; it does not universally require insertion of the incoming descriptor (`src/receive_lineage.rs:204–253`).

Incoming matrix correction retains the original point and accumulates a corrected value; equality with the original contributes to validity (`src/full.rs:4680–4685`, `4772–4788`). Generator correction retains original challenges and compares original/corrected points (`src/generator_integration.rs:85–112`, `297–355`). A successful credit requires these comparisons to hold. Hard own-predecessor obligations are not optional corrections.

Load finality is checked natively before preparation (`src/engine.rs:1386–1406`). An accepted Load relation alone is not a proof of ledger finality. Funding security additionally depends on the native/provider and signing boundaries.

### 4.6 Monetary conservation corollary

Fix one scheme and asset. Expand honest-record pointers to their recorded histories. For accounting, identify repeated proofs of the same authenticated application transition; different proof-specific accumulator auxiliaries must not cause a Load or Send to be counted twice.

In addition to the closure theorem, assume:

1. **No forks:** all included application states for each wallet lie on one own-predecessor chain. Custody admits no competing successor, rollback replay, or inconsistent recovery.
2. **No second Bootstrap:** the same credential **or wallet identifier** cannot start another Bootstrap chain. Restarting with a fresh empty consumed map is a fork for this purpose.
3. **Credit uniqueness:** recipient binding and consumed-map nonmembership/insertion enforce at most one successful credit for each Request-derived credit identifier.
4. **Authorized funding:** every counted Load satisfies native finality and receipt authorization, together with its chain’s ordinal/recovery rules.

These are substantive hypotheses. The proof system alone does not enforce the first two.

For each wallet chain, let \(S_w\) be the final spendable balance: Core balance minus authenticated adjusted burned total. Across the accounting set, let:

- \(L\): total Load amounts.
- \(C\): total successful Receive credits.
- \(D\): total Send principal.
- \(F\): total Send fees.

Bootstrap contributes zero. The integer transition equations telescope to

\[
\sum_w S_w=L+C-D-F.
\]

Receive increases Core balance by the incoming amount; burn increases adjusted burned value equally (`src/receive.rs:328–337`; `src/receive_lineage.rs:222–227`). Send debits principal and fee and does not debit burned value twice (`src/send_arithmetic.rs:97–125`). Load adds the receipt amount (`src/load.rs:351–360`). These are integer equations under the range constraints, not merely a global equality modulo a field.

Every successful credit maps to a recipient-bound Send in the closed set or expanded honest history. The no-fork, no-second-Bootstrap, and consumed-map hypotheses make that mapping injective. Therefore \(C\le D\). Defining \(U=D-C\ge0\),

\[
\boxed{\sum_w S_w=L-F-U\le L-F.}
\]

Thus there is no spendable value beyond finalized Loads, net of fees and Send principal not successfully credited.

\(U\) includes in-flight or unclaimed principal and principal lost to burn. It is not the sum of arbitrary invalid/replayed burn Receive amounts: those amounts already cancel between Core balance and adjusted burned total.

Payer-edge cycles do not change this equation. Every successful credit still has a matching debit. No global rank is required for conservation.

This is a conditional corollary, not a release claim for current HEAD.

## 5. Actual challenge distributions

### 5.1 RP57 model

The transcript uses width three, rate two, eight full rounds, and 57 partial rounds, with pinned field-specific constants (`crates/iroha_pasta/src/poseidon/mod.rs:42–58` at repository root; `src/fs.rs:15–47`).

The probability calculations assume an ideal uniform squeezed field word at a fresh transcript prefix. They are not statistical claims about a fixed deterministic Poseidon invocation. Concrete recursive use belongs to P2-RKFS B.

Absorption discards the spare half (`src/fs.rs:63–71`). Two consecutive challenges without absorption can share one squeezed word (`src/fs.rs:111–129`).

### 5.2 Raw 126-bit challenges

Set \(H=2^{126}\). Each Pasta modulus \(r\in\{p,q\}\) satisfies

\[
r=4H^2+\delta_r,\qquad 0<\delta_r<H.
\]

For uniform \(Z\in\{0,\ldots,r-1\}\), the first raw challenge is \(Z\bmod H\), with exact maximum atom

\[
\lambda_0(r)=\frac{4H+1}{r}.
\]

Conditional on that first half, the maximum atom of the second half is

\[
\lambda_1=\frac{5}{4H+1}.
\]

For first half \(a<\delta_r\), there are \(4H+1\) compatible words and the second-half value zero has five preimages. Otherwise there are \(4H\) compatible words, four per second-half value.

A conservative common bound is

\[
\boxed{\lambda_*=\frac{5}{4H}.}
\]

The joint distribution of both halves has maximum atom \(5/r\). LogUp uses this joint bound, not independent uniform sampling.

Extraction/conversion is at `src/fs.rs:121–158`, `216–222`; actual circuit decomposition and original-modulus checks are at `src/points.rs:1624–1786`.

### 5.3 Endoscalars are injective and nonzero

The endoscalar map consumes 63 bit pairs. Starting from \(a=b=2\), each pair doubles both coordinates and adds \(+1\) or \(-1\) to one coordinate. It returns

\[
e=a\zeta+b,\qquad \zeta^2+\zeta+1=0
\]

in the scalar field (`src/fs.rs:224–254`).

After 63 steps,

\[
2^{63}+1\le a,b\le3\cdot2^{63}-1.
\]

The integer encoding is injective. In reverse, the odd coordinate identifies which coordinate received the increment. Except for the final reverse step, the preceding coordinate sum is odd, so the current sum modulo four identifies the sign. The remaining step is determined by the initial pair.

If two outputs coincide modulo scalar prime \(r\), then

\[
\Delta a\zeta+\Delta b=0
\quad\Longrightarrow\quad
r\mid\Delta a^2-\Delta a\Delta b+\Delta b^2.
\]

The differences have magnitude below \(2^{64}\), so this nonnegative norm is below \(3\cdot2^{128}<r\). It must be zero, giving \(\Delta a=\Delta b=0\). The same norm argument for positive \(a,b\) proves nonzeroness.

Thus endoscalars preserve the source bits’ atom bounds. They are not uniform over the scalar field, but incur no additional map-collision or zero-challenge penalty.

### 5.4 Full-width cross-field challenges

Full-width challenges recombine the entire canonical squeezed word in the scalar field (`src/fs.rs:136–143`, `178–182`).

| Use | Source and destination | Maximum atom |
|---|---|---:|
| P initial sumcheck challenge | Uniform \(\mathbb F_q\) reduced into \(\mathbb F_p\) | \(2/q\) |
| S initial sumcheck challenge | Uniform \(\mathbb F_p\) embedded into \(\mathbb F_q\) | \(1/p\) |
| Pair-link challenge | S’s full \(\mathbb F_p\) word used in \(\mathbb F_q\) | \(1/p\) |

The pair-link challenge is **not 126 bits** (`src/spartan.rs:222–226`). Since \(p<q\), its conversion is injective (`src/verifier.rs:2151–2154`).

## 6. G2: local soundness lemmas

All lemmas require the relevant coefficients to be fixed in the extractor view before the analyzed challenge. Ordinary computational binding alone does not supply that timing property.

### 6.1 Gamma separation and alpha batching

Let \(m\) be the number of committed outer and inner rounds. After fixing the prefix before \(\gamma\), including \(\alpha\), write

\[
C(\gamma)=C_0+\sum_{i=1}^{m}\gamma^iD_i,
\qquad
b(\gamma)=b_0+\sum_{j=1}^{m}\gamma^{-j}b_j.
\]

Here \(C_0\) is the Hyrax row aggregate, \(b_0\) is supported on active witness columns, and \(b_j\) is supported on round \(j\)’s intended slot (`src/spartan.rs:328–339`, `467–554`, `632–650`, `1674–1715`).

**Straight-line representation step.** P2-RKFS A records the individual row and round representations at their first transcript use. Their known linear combinations give vectors \(W_0,\ldots,W_m\), where \(W_0\) is the row aggregate and \(W_i\) is the generator-vector part of round commitment \(D_i\).

These representations are not asserted to satisfy the protocol. The accepted IPA equations, including the blinding and value-base components, must agree with them, except for §6.5’s algebraic events or a nontrivial relation among the authenticated bases. No Vandermonde rewinding is used to establish prefix knowledge or recursive extraction cost.

The scalar residual is

\[
L(\gamma)=
\left\langle
\sum_{i=0}^{m}\gamma^iW_i,\,
b_0+\sum_{j=1}^{m}\gamma^{-j}b_j
\right\rangle-\kappa.
\]

Its Laurent exponents lie in \([-m,m]\). If nonzero, \(\gamma^mL(\gamma)\) is a nonzero polynomial of degree at most \(2m\). Endoscalars are nonzero, so

\[
\boxed{\Pr[L(\gamma)=0\text{ accidentally}]\le2m\lambda_\gamma.}
\]

The constant coefficient is

\[
\langle W_0,b_0\rangle+
\sum_{i=1}^{m}\langle W_i,b_i\rangle-\kappa.
\]

This is the intended alpha-batched diagonal-slot relation. Off-slot contributions have nonconstant powers. The lemma does not require every malicious commitment to have zero off-slot coordinates; it prevents those coordinates from repairing a nonzero intended constant coefficient except at a root.

There are \(m+2\) scalar consistency/final equations, with alpha powers through \(m+1\) (`src/spartan.rs:481–543`). For a fixed nonzero discrepancy vector,

\[
\boxed{\Pr[\text{alpha batch vanishes}]\le(m+1)\lambda_\alpha.}
\]

The combined conditional error is

\[
2m\lambda_\gamma+(m+1)\lambda_\alpha.
\]

A degree-\(2m\) polynomial has at most \(2m\) roots; \(2m+1\) evaluations would force an identity. These are different statements.

**CancelRound and AdaptiveWitness.** CancelRound places an earlier round’s replacement opening in a later commitment. AdaptiveWitness places a witness correction in a round commitment after the witness’s earlier challenge. Their off-slot terms acquire nonzero gamma exponents and cannot replace the intended constant coefficient except through the events above (`src/spartan.rs:1554–1569`).

Existing refusal evidence covers the native toy verifier (`src/lib.rs:168–175`) and a toy synthesized verifier (`src/verifier.rs:2344–2461`). It does not establish refusal through the complete uniform builders. Finding 9 remains open.

### 6.2 Initial sumcheck test

For Boolean constraint indices \(x\), let

\[
e_x=(Az)_x(Bz)_x-(Cz)_x.
\]

A false committed assignment has a nonzero error table. Its multilinear extension is

\[
F(\tau)=\sum_x\operatorname{eq}(\tau,x)e_x.
\]

The lean schedule sets \(\tau_i=t^{2^i}\). Distinct multilinear monomials become distinct powers, preserving nonzeroness and giving degree at most \(2^{s_x}-1\). Therefore

\[
\epsilon_{\tau,P}\le\frac{2(2^{s_x}-1)}q,
\qquad
\epsilon_{\tau,S}\le\frac{2^{s_x}-1}p.
\]

Sources: `src/fs.rs:185–203`; `src/spartan.rs:1290–1296`, `1513–1515`.

### 6.3 Committed sumcheck rounds

For the outer sumcheck, define

\[
Q_i(y)=
\sum_{z\in\{0,1\}^{\mathrm{remaining}}}
\operatorname{eq}(\tau_{\mathrm{remaining}},z)
E(r_{\mathrm{prefix}},y,z),
\]

where \(E=\widetilde{Az}\widetilde{Bz}-\widetilde{Cz}\).

The implemented recurrence is

\[
\sum_{y\in\{0,1\}^{k_i}}
\operatorname{eq}(\tau_{\mathrm{block}},y)Q_i(y)
=Q_{i-1}(r_{\mathrm{previous}}),
\]

ending at \(v_Av_B-v_C\). Each \(Q_i\) has individual degree at most two (`src/spartan.rs:481–505`, `578–629`, `1542–1547`).

If an incoming claim is false, either the recurrence is violated or the claimed-minus-true polynomial is nonzero. No division by an equality-factor evaluation occurs, so no extra equality-factor-zero event is needed.

The inner sumcheck also has individual degree at most two and ends at the matrix/witness/public-input relation (`src/spartan.rs:507–543`).

For a lean block of \(k\) variables, substitution

\[
(r,r^3,r^{3^2},\ldots)
\]

maps degree-two monomials injectively to exponents. The degree is at most \(3^k-1\), giving

\[
\boxed{\epsilon_{\mathrm{block}}\le(3^k-1)\lambda_0.}
\]

Early inner blocks use separate coordinates, including endoscalar row coordinates, and contribute at most twice the sum of their conditional atom bounds (`src/spartan.rs:306–325`).

For \(k=3\), the degree is 26. Relative to three independent degree-two coordinates, the loss is

\[
\log_2(26/6)\approx2.115\text{ bits}.
\]

Relative to one 126-bit challenge, the degree loss is \(\log_2 26\approx4.700\) bits. The comparative losses for \(k=4,5\) are approximately 3.322 and 4.597 bits. The current schedule should not be described simply as a “3–4-bit loss.”

### 6.4 The \(r,r^2,r^3\) matrix combination

After \(v_A,v_B,v_C\) are fixed, one raw challenge supplies \(r,r^2,r^3\) (`src/spartan.rs:1308–1314`, `1593–1597`).

For a fixed nonzero discrepancy vector,

\[
r\Delta_A+r^2\Delta_B+r^3\Delta_C
\]

is a nonzero polynomial of degree at most three. Hence

\[
\boxed{\epsilon_{\mathrm{ABC}}\le3\lambda_0.}
\]

The root \(r=0\) is included; raw challenges are not assumed nonzero.

### 6.5 Main zero-knowledge IPA

The folding equations are

\[
x'=ux_{\mathrm{lo}}+x_{\mathrm{hi}},\quad
b'=b_{\mathrm{lo}}+ub_{\mathrm{hi}},\quad
G'=G_{\mathrm{lo}}+uG_{\mathrm{hi}},
\]

\[
P'=uP+u^2L+R.
\]

The commitment/value precede randomized-\(U\) separation; each \(L,R\) precedes its folding challenge; the final generator and Schnorr commitment precede the Schnorr challenge (`src/ipa.rs:253–257`, `318–419`).

In the recorded representation view, excluding nontrivial generator relations:

- Randomized-\(U\) separation contributes \(\lambda_0\).
- Each of \(\ell\) folds contributes at most \(2\lambda_0\).
- The Schnorr final contributes \(\lambda_0\).

Thus

\[
\boxed{\epsilon_{\mathrm{IPA}}\le(2\ell+2)\lambda_0.}
\]

This is conditional algebraic knowledge-error accounting, not a standalone concrete Fiat–Shamir reduction.

It requires

\[
G_{\mathrm{final}}=\langle s(u),g\rangle,\qquad
s_i(u)=\prod_j u_j^{\operatorname{bit}_j(i)}.
\]

Native verification decides this equality (`src/ipa.rs:423–434`, `469–481`); recursion carries the obligation forward. A freely chosen, undecided \(G_{\mathrm{final}}\) would invalidate the argument.

### 6.6 Four-claim matrix accumulation

The combined matrix polynomial is

\[
P(c_0,c_1,r,a,b,x,y)=
\sum_k
\operatorname{eq}((c_0,c_1),\operatorname{corner}_k)
\operatorname{pad}_k(x,y)
\bigl(rA_k+r^2B_k+r^3C_k\bigr)(a,b;x,y).
\]

Sources: `src/deferral.rs:112–172`, `219` onward.

Let \(\Gamma(t)\) be the coordinatewise cubic through input points \(z_0,\ldots,z_3\) at \(t=0,1,2,3\), with claimed values \(v_0,\ldots,v_3\).

The prover commits to coefficients of \(q\), of degree at most

\[
D_M=3(2+X+Y)+9+3d_a+3d_b.
\]

The coefficients are first-phase. Constraints check \(q(j)=v_j\) for all four inputs. Only \(\Gamma(\beta)\) and the Horner computation of \(q(\beta)\) are second-phase (`src/deferral.rs:496–626`).

If any input claim is false,

\[
E(t)=q(t)-P(\Gamma(t))
\]

is nonzero, since \(E(j)=v_j-P(z_j)\ne0\) for that input. Assuming the actual program degrees satisfy the bound, \(\deg E\le D_M\), and

\[
\boxed{\epsilon_{\mathrm{matrix\ fold}}\le D_M\lambda_\beta.}
\]

There is **no factor four**: there is one nonzero error polynomial and one root event. Repeating predecessor claims for Send or Load does not change the argument (`src/full.rs:4647–4687`).

The challenge is the second raw phase-two challenge, `ch[1]`, not a full-field uniform value or endoscalar (`src/deferral.rs:615–626`).

Input ties, including fresh-claim padding, are at `src/full.rs:4761–4895`; output exposure is at `4898–4900`. Native decision exactly evaluates the installed polynomial (`src/engine.rs:771–778`) and adds no random error.

The true degree of \(q-P\circ\Gamma\), rather than metadata alone, controls soundness. Under-declaration can prevent honest folding and invalidate the advertised numerical bound; it does not bypass exact native evaluation. Current HEAD checks formal degrees on import and admission (`src/program_artifact.rs:478–482`; `src/engine.rs:447–452`).

**Bootstrap.** Its empty matrix claim has \(r=0\) and value zero. This makes every matrix term vanish; it does not rely on an unused selector corner (`src/deferral.rs:158–172`; `src/full.rs:5678–5685`, `5726–5733`).

### 6.7 Pair link

Both circuits constrain, over \(\mathbb F_q\),

\[
R=\mu\rho^n+\sum_{j=0}^{n-1}v_j\rho^{n-1-j}.
\]

The shared values and mask \(\mu\) are first-phase. The Horner chain is link-phase and tied to public \(R\) (`src/full.rs:5136–5162`, `5250–5298`). P-side foreign-field arithmetic and ranges are implemented in `src/nonnative.rs:2194`, `2308` onward.

P derives its first-phase digest; S absorbs it and derives full-width \(\rho\) after its own first-phase commitments (`src/spartan.rs:191–226`).

If the two fixed vectors \((\mu,v)\) differ as field elements, their difference polynomial is nonzero of degree at most \(n\). Since \(\rho\) is uniform over canonical \(\mathbb F_p\) words embedded into \(\mathbb F_q\),

\[
\boxed{\epsilon_{\mathrm{link}}\le n/p.}
\]

The mask must be fixed before \(\rho\); its randomness is unnecessary for this soundness lemma. Bootstrap uses a deterministic mask (`src/full.rs:5678–5685`, `5726–5733`).

The conclusion is equality of field coefficients. Equality of transcript encodings additionally requires canonical representations and binding of every relevant IO leaf. Findings 4 and 7 cannot be discharged by the link lemma.

### 6.8 Private generator accumulation

A source claim is

\[
G_i=\langle s(u_i),g\rangle.
\]

The private construction hashes the four ordered source claims, relation identity, and all masks, then commits to the digest as \(D\). It absorbs \(D\) before batching endoscalar \(\mu\), and absorbs

\[
C_*=\sum_{i=0}^{3}\mu^iG_i
\]

before evaluation challenges and \(\zeta\) (`src/generator_claim.rs:169–195`, `276–300`).

Under fixed source representations, binding of the digest and its preimage, and the recorded challenge schedule:

1. Source batching contributes \(3\lambda_E\).
2. A differing vector’s multilinear evaluation contributes at most \(\ell\lambda_{\mathrm{raw}}\).
3. Evaluation separation contributes \(\lambda_E\).
4. The \(\ell\) IPA folds contribute \(2\ell\lambda_E\).

Therefore

\[
\boxed{
\epsilon_{\mathrm{generator}}
\le\ell\lambda_{\mathrm{raw}}+(4+2\ell)\lambda_E.
}
\]

The exact mask correction is

\[
T_{j+1}=u_jT_j+u_j^2L_j+R_j,
\]

\[
h_0=0,\qquad
h_{j+1}=u_jh_j+u_j^2m_{2j+1}+m_{2j+2}.
\]

The final equation includes \(h_\ell H\). Mask \(m_0\) blinds \(D\); it is not an initial \(H\)-coefficient of \(T_0\).

**The restructuring is checked in-circuit.** `generator_claim::trace` uses private masks to reconstruct the prover’s witness; it is not the native accepting verifier (`src/generator_claim.rs:342–408`). Native acceptance decides only the exported claim by its exact MSM (`src/engine.rs:786–811`; `src/generator_claim.rs:235–243`).

**Source-verified circuit audit:**

- The points builder hashes the relation identifier, ordered source points, source-challenge limbs, and all \(1+2\ell\) masks from the shared stream (`src/generator_claim_points.rs:170–188`).
- It constructs \(D\) from that digest and \(m_0\), absorbs its constrained readback before \(\mu\), constructs \(C_*\) from the same source-point cells, and absorbs its readback before the evaluation challenges and \(\zeta\) (`189–224`).
- Each \(L,R\) precedes its challenge. The group recurrence and final \(H\)-correction are constrained (`232–273`). Readbacks and final equalities become actual imports and coordinate constraints (`src/points.rs:1161–1165`, `1228–1245`).
- The scalar builder uses the same source/mask limbs and constrains evaluations, output challenges, the exact \(h_j\) recurrence, and final scalar products (`src/generator_claim_scalar.rs:166–222`).
- These are the proof-port source cells: own challenges are tied to verifier \(U\) cells, inherited challenges to IO leaves, and inherited point limbs are tied or compared by the incoming iff predicate (`src/generator_integration.rs:297–355`).
- Own source points are compared with the actual verified proof’s `g_final`; hard equality is required, while incoming equality contributes to validity (`src/generator_claim_points.rs:40–49`; `src/full.rs:3844–3859`, `3988–4007`, `5439–5454`, `5559–5578`).
- Relation identifiers are tied to predecessor context, and outputs are exposed as late IO (`src/generator_integration.rs:361–378`, `408–434`).

This establishes the inspected wiring and ordering. Compiler/copy correctness, pair-link agreement, extraction, and hash hypotheses remain necessary.

Private source-scalar and mask limbs have field-element semantics where recomposed modulo the scalar field. This audit does not prove unique integer encodings for all private limbs (`src/generator_claim_scalar.rs:94–97`, `166–179`).

Bootstrap exposes the exact constant claim \((g_0,(0,\ldots,0))\), for which \(s=(1,0,\ldots,0)\) (`src/generator_integration.rs:239–279`).

### 6.9 Compiler LogUp obligations

Spartan proves the instantiated R1CS. Separate reasoning is needed to establish the intended semantics of randomized copy and lookup constraints.

Tuples are compressed as

\[
f_i(B)=\operatorname{tableid}_i+\sum_{j=1}^{d}B^jv_{ij},
\]

using inverse equations and a sum relation (`src/uniform.rs:1640–1675`, `2718–2775`, `2796–2855`).

**Hypothesis:** every semantic tuple value, address, mask, and multiplicity is fixed before phase-two challenges \(A,B\). Lookup/export/import tuples must not depend on phase-two or late-IO cells, even when those cells are uniquely determined by the challenges rather than freely chosen. Separately justified triangular equations may define phase-two helper values.

Combine identical formal tuple polynomials. If the field-valued multiset relation is false with a nonzero residue,

\[
R(A,B)=\sum_i\frac{c_i}{A-f_i(B)}
\]

is nonzero over \(\mathbb F(B)\). Clearing its \(s\) distinct denominators gives a nonzero numerator of total degree at most \(d(s-1)\). Denominator-zero events occupy at most \(sH\) points of the \(H\times H\) grid.

Using joint maximum atom \(5/r\),

\[
\epsilon_{\mathrm{LogUp}}
\le[d(s-1)+s]\frac{5H}{r}.
\]

For \(s\le M\), with \(M\) a constraint-row bound,

\[
\boxed{\epsilon_{\mathrm{LogUp}}\le(d+1)M\lambda_*.}
\]

For membership, an absent queried tuple has nonzero residue: its positive query count is below the characteristic and cannot be canceled by a table multiplicity for that absent formal tuple. Multiplicity width metadata is not a range constraint (`src/spartan.rs:662–664`). A table-side zero denominator with zero multiplicity is included in the denominator-zero event.

**Source-verified phase audit:** current phase-two cells are lookup/copy/table helpers and the matrix fold’s \(\Gamma(\beta)\) and \(q(\beta)\) cells. Multiplicities precede phase two; link-table multiplicities occupy the link phase. Copy addresses and read masks are fixed (`src/uniform.rs:834–916`, `1522–1553`, `1630–1638`, `2579–2621`; `src/deferral.rs:582–609`). The matrix evaluation cells are triangularly defined and exposed through linear late-IO ties; they do not enter current lookup/copy tuples (`src/deferral.rs:695–707`). No violation was found in the inspected builders.

**Missing enforcement:** `UniformCircuit::build_with_copies_at` does not reject future lookup/export/import tuples that read phase-two or late-IO values. Its current storage/presence checks are insufficient (`src/uniform.rs:808–820`, `1461–1464`, `1701–1735`).

The comment requiring every challenge-factor read to be first-phase is too broad (`src/uniform.rs:210–213`): the triangular Horner definition legitimately reads its own phase-two cells with factor \(-\beta\) (`src/deferral.rs:587–609`).

The required builder invariant must prohibit challenge-dependent semantic LogUp inputs while permitting explicitly justified triangular defining equations. The ledger remains conditional on that invariant and compiler semantics.

## 7. Concrete bounds at the retained complete layout

### 7.1 Profile

| Parameter | P | S |
|---|---:|---:|
| Constraint bits \(s_x\) | 22 | 22 |
| Hyrax row bits | 5 | 4 |
| IPA column bits \(\ell\) | 18 | 17 |
| Inner variables \(s_z\) | 24 | 22 |
| Variables per committed block | 3 | 3 |
| Outer committed rounds | 8 | 8 |
| Inner committed rounds | 8 | 8 |
| Matrix accumulator dimensions \((X,Y)\) | \((24,25)\) | \((24,24)\) |
| Maximum matrix challenge degrees | \((1,7)\) | \((1,4)\) |

The configured profile is at `src/backend_chain_tests.rs:18–34`. Retained shapes appear at `output/s2/chain-20261117T051751Z.log:431`, `443`, `449`, `455`.

The original inspection checked the eight retained public artifact headers and conservatively propagated formal degrees through their instruction tapes. The independent review recomputed the ledger. Neither establishes equality of the retained candidate to current HEAD.

Current artifact schema, geometry fields, and opcode parsing are at `src/program_artifact.rs:8–29`, `276–292`, `441–468`.

### 7.2 Main Spartan proof ledger

Let \(\lambda_{0P}=\lambda_0(q)\) and \(\lambda_{0S}=\lambda_0(p)\).

| Term | P | S |
|---|---:|---:|
| Initial equality test | \(2(2^{22}-1)/q\) | \((2^{22}-1)/p\) |
| Outer rounds | \(184\lambda_{0P}\) | \(184\lambda_{0S}\) |
| Inner rounds | \(164\lambda_{0P}+4\lambda_1\) | \(140\lambda_{0S}+4\lambda_1\) |
| \(r,r^2,r^3\) batching | \(3\lambda_{0P}\) | \(3\lambda_{0S}\) |
| Alpha batching | \(17\lambda_1\) | \(17\lambda_1\) |
| Gamma separation | \(32\lambda_{0P}\) | \(32\lambda_{0S}\) |
| Main IPA | \(38\lambda_{0P}\) | \(36\lambda_{0S}\) |
| **Subtotal excluding initial test** | **\(421\lambda_{0P}+21\lambda_1\)** | **\(395\lambda_{0S}+21\lambda_1\)** |

The outer count is \(7\cdot26+2=184\). Early inner blocks use separate coordinates; later blocks use the Kronecker schedule. Alpha consumes a spare half, while gamma begins a fresh squeeze (`src/spartan.rs:1637–1659`).

The two-proof subtotal is approximately \(868.5/H\), or **116.2376 local algebraic bits**. It excludes accumulation, LogUp, linking, cryptographic/model failures, and lifetime/query losses.

### 7.3 Accumulation and link ledger

\[
D_P=3(2+24+25)+9+3(1+7)=186,
\]

\[
D_S=3(2+24+24)+9+3(1+4)=174.
\]

Generator bounds are \(58\lambda_*\) for P and \(55\lambda_*\) for S.

| Additional term | Bootstrap | Load | Send | Receive |
|---|---:|---:|---:|---:|
| P matrix fold | 0 | \(186\lambda_*\) | \(186\lambda_*\) | \(186\lambda_*\) |
| S matrix fold | 0 | \(174\lambda_*\) | \(174\lambda_*\) | \(174\lambda_*\) |
| P generator restructuring | 0 | \(58\lambda_*\) | \(58\lambda_*\) | \(58\lambda_*\) |
| S generator restructuring | 0 | \(55\lambda_*\) | \(55\lambda_*\) | \(55\lambda_*\) |
| Pair link | \(1/p\) | \(783/p\) | \(783/p\) | \(1539/p\) |
| Exact native decisions | 0 | 0 | 0 | 0 |

The link lengths are:

- One parent pair: 139 verification words, two kind bits, four table limbs, totaling 145.
- Private generator layouts: \(39+16\ell\) per side, totaling \(327+311=638\).
- Send/Load: \(145+638=783\).
- Receive: \(290+638+601+10=1539\), including two parents, admitted incoming bytes, and verdict words.
- Bootstrap: one shared constant.

Sources: `src/verifier.rs:2046–2100`; `src/generator_claim.rs:71–95`; `src/full.rs:3783–3798`, `4209–4275`, `5381–5395`, `5658–5661`; `src/proof_bytes.rs:47–55`.

### 7.4 Conditional total per operation

Replacing raw/endoscalar atom bounds by \(\lambda_*=5/(4H)\), the main-proof coefficient is \(442+416=858\); non-Bootstrap accumulation adds 473.

| Kind | P rows \(M_P\) | S rows \(M_S\) | \(d_P\) | \(d_S\) |
|---|---:|---:|---:|---:|
| Bootstrap | 414,240 | 163 | 7 | 1 |
| Load | 1,398,002 | 1,281,024 | 7 | 4 |
| Send | 1,670,642 | 1,281,024 | 7 | 4 |
| Receive | 2,773,423 | 2,334,464 | 7 | 4 |

These are retained `research-kind-*.norito` artifact fields, interpreted using `src/program_artifact.rs:276–292`.

Define

\[
E_{\mathrm{full}}=
\frac{2(2^{22}-1)}q+\frac{2^{22}-1}p.
\]

Under all local hypotheses,

\[
\boxed{\epsilon_k\le C_k\lambda_*+E_{\mathrm{full}}+\frac{n_k}{p}.}
\]

| Kind | Main proofs | Matrix folds | Generator folds | LogUp upper coefficient | \(C_k\) | Local algebraic bits, approximately |
|---|---:|---:|---:|---:|---:|---:|
| Bootstrap | 858 | 0 | 0 | 3,314,246 | **3,315,104** | **104.01** |
| Load | 858 | 360 | 113 | 17,589,136 | **17,590,467** | **101.60** |
| Send | 858 | 360 | 113 | 19,770,256 | **19,771,587** | **101.44** |
| Receive | 858 | 360 | 113 | 33,859,704 | **33,861,035** | **100.66** |

LogUp uses \((d_P+1)M_P+(d_S+1)M_S\), a conservative bound rather than an exact distinct-denominator count. Bits are rounded down; the probability formula is authoritative.

Without LogUp, conservative subtotals are about 115.93 bits for Bootstrap and 115.29 bits for other kinds. These are **not complete operation bounds**.

The numbers are incremental node costs. A Receive’s closed history includes both required parent histories. Shared statement nodes are counted once; additional proof attempts remain query/computation costs.

**Established overall security bits for current Path 2: UNVERIFIED for every kind.** This ledger is conditional algebraic accounting for a retained profile, not a numerical bound on concrete recursive RP57, discrete logarithms, application hashes, signatures, compiler defects, or custody failures.

## 8. Round-by-round soundness and finite-query accounting

### 8.1 Round-by-round lemma

Work in P2-RKFS A’s recorded representation view and B’s idealized transcript experiment, with the compiler-phase and correspondence hypotheses.

Before each fresh challenge reply, record the relevant coefficients and unresolved obligation residuals. The state is determined before that reply, even when the protocol checks a residual later.

A false obligation can reach an accepting continuation only through a local event bounded in §§5–7:

- A false committed assignment passes the initial equality test only at a root.
- In a sumcheck round, a false incoming claim either leaves a violated recurrence, retained for later alpha batching, or yields a nonzero claimed-minus-true polynomial whose evaluation error disappears only at a root.
- The final discrepancy vector is fixed before alpha. Its batch disappears only at the alpha event. Once alpha is fixed, gamma cannot replace a nonzero intended diagonal constant coefficient by off-slot terms except at its degree-\(2m\) root event.
- IPA equations, matrix/generator folds, and pair linking propagate their corresponding obligations unless their bounded events occur.
- LogUp uses the joint distribution of its challenge pair, with semantic inputs fixed before that pair.

This argument respects the actual protocol order; it does not pretend that delayed sumcheck equations are checked immediately.

Repeated oracle inputs provide no new randomness. A fresh reply supplying several tested quantities must be charged their combined error bound, or a directly proved joint bound. In particular, two halves of one squeeze are not independent.

Therefore the protocol is round-by-round sound in this extractor view: a false obligation cannot reach accepted terminal equations without a charged local event or cryptographic/model failure.

This lemma depends on A to define the fixed residuals. It is not a claim about arbitrary hiding commitments without representation access, nor does it discharge the open implementation findings.

### 8.2 Bounds

For a fixed execution without additional grinding,

\[
\Pr[\text{local algebraic failure}]
\le\sum_{v\in V}\epsilon_{\operatorname{kind}(v)}.
\]

Let \(\delta_*\) be the largest error bound for a fresh relevant oracle reply, combining all tests sharing that reply. Count repeated proof variants and recursively encountered evaluations in \(Q\); statement memoization does not make their challenges free. Final representative-node processing is conservatively covered by the node sum.

Then

\[
\begin{aligned}
\operatorname{Adv}_{\mathrm{invalid}}
\le{}&
\epsilon_{\mathrm{RKFS}}
+\epsilon_{\mathrm{binding}}
+\epsilon_{\mathrm{application\ hashes}}
+\epsilon_{\mathrm{signatures}}\\
&+Q\delta_*+
\sum_{v\in V}\epsilon_{\operatorname{kind}(v)}\\
\le{}&
\epsilon_{\mathrm{RKFS}}
+\epsilon_{\mathrm{binding}}
+\epsilon_{\mathrm{application\ hashes}}
+\epsilon_{\mathrm{signatures}}\\
&+(Q+N)\max_k\epsilon_k.
\end{aligned}
\]

The binding term covers both curves and authenticated commitment-base relations. Cryptographic reductions must use the extractor’s running time, not silently the original adversary’s time. Deterministic implementation defects are premises to repair, not small probabilities hidden in these terms.

The largest elementary challenge test at the retained layout is Receive P LogUp:

\[
8(2{,}773{,}423)\lambda_*
=22{,}187{,}384\lambda_*
\approx2^{-101.2748}.
\]

If the budget counts elementary tested challenge opportunities, this is the maximum elementary-test charge.

If \(Q\) counts fresh oracle replies, the same phase-two reply also supplies matrix-fold \(\beta\). A conservative dominant reply bound is therefore

\[
\delta_*\le
(22{,}187{,}384+186)\lambda_*
=22{,}187{,}570\lambda_*.
\]

The other reply groups are smaller at this profile. This distinction preserves the shared-reply accounting without changing §7.

### 8.3 Illustrative budgets

| Budget | Example |
|---|---:|
| Operations per wallet generation | \(L=2^{20}\) |
| Wallets whose histories may join | \(W=2^{16}\) |
| Distinct relevant extracted nodes | \(N\le2^{36}\) |
| Relevant oracle replies, including grinding | \(Q\le2^{40}\) |

These are **illustrative assumptions, not installed or approved limits**. The bound on \(N\) must cover relevant adversarial forks and honest histories; it does not follow merely by multiplying honest-wallet usage estimates. A wallet counter does not bound off-device grinding.

The retained conservative form gives an algebraic contribution of approximately

\[
2^{-60.58},
\]

or, rounding the exponent conservatively downward, \(2^{-60.57}\). At \(Q=2^{64}\), it gives about \(2^{-36.66}\).

Using the tighter reply bound gives about \(2^{-61.14}\) and \(2^{-37.27}\), respectively. All these figures exclude the unquantified cryptographic and concrete-recursion terms.

Neither 255-bit fields nor 126-bit challenges establish overall “128-bit soundness.” A deployment must select finite budgets and a numerical target, accept or replace the recursive model, and reassess the challenge schedule if the resulting bound is insufficient. No bound survives infinitely many independent attempts.

## 9. Implementation findings and unresolved obligations

This table records the inspected findings and their disposition. It is not a claim of exhaustive audit coverage.

| Finding or obligation | Evidence at HEAD | Disposition and required property |
|---|---|---|
| **Recursive model remains an assumption** | Commitment/challenge order at `src/spartan.rs:1551–1659`; aggregation at `1674–1715`. | Explicitly accept algebraic representation access and concrete recursive RP57 idealization, or supply a checked replacement theorem. Two-field agreement, accumulation closure, extraction cost, and query charging are derived as above, not assumed separately. |
| **Custody uniqueness remains external** | Own counter at `src/monetary_state.rs:91–93`; Bootstrap resets state/maps at `src/bootstrap.rs:260–278`; replay handling at `src/receive_lineage.rs:186–215`. | Require no forks and no second Bootstrap for the same credential or wallet identifier. No global rank on payer edges is required for the stated closure/conservation theorem. |
| **No enforced finite security budget** | The implemented sequence bound is unsigned128 (`src/monetary_state.rs:91–93`). | Select justified history/query budgets and an overall target. Resource limits and sequence width are not cryptographic lifetime bounds. |
| **Degree declarations versus actual degree — review 10** | Formal propagation at `src/program_artifact.rs:30–128`; import check at `478–482`; admission at `src/engine.rs:447–452`; program identity at `430–435`, `1751–1754`. | Agree with the review’s distinction. Actual degree controls soundness. Under-declaration threatens completeness and the advertised bound, not exact native evaluation. Guards are source-verified at HEAD; execution and retained-candidate correspondence remain **UNVERIFIED**. |
| **Unbound interior IO-fold leaves — review 4** | Tail-only zeroing at `src/scalar.rs:779–790`; explicit digest projections at `src/full.rs:3895–3922`, `4970–5044`; link ties at `4129–4166`. | Agree. Every IO-fold leaf must equal native reconstruction, including zero holes inside `[0,io_len)`. The retained P layout leaves 119–123 and 128–159 uncovered. A mutation of such a leaf must make the circuit unsatisfiable, not merely change incoming validity to zero. Cover hard predecessors too. **Not fixed at reviewed HEAD.** |
| **LogUp phase invariant lacks builder enforcement — review 5** | Current placement at `src/uniform.rs:1522–1553`, `1630–1638`; builder metadata at `1461–1464`; matrix definitions at `src/deferral.rs:582–609`. | Agree. Current builders satisfy the inspected phase arrangement. Add build-time rejection of phase-two/late-IO dependencies in semantic lookup/copy inputs, including challenge-determined cells. Permit only justified triangular defining rows to read phase-two cells with challenge factors. |
| **Full-width cross-field scalar canonicality — review 7** | Modular recomposition at `src/full.rs:2846–2875`, `5031–5044`; full scalar exports at `src/verifier.rs:2073–2095`. | Agree. Constrain each relevant limb pair to the unique canonical integer of its source field, including limb ranges and a modulus comparison. Existing challenge-word and claim-coordinate checks do not cover all these values. Add alias-mutation tests. **Not claimed fixed.** No complete alias-grinding factor has been established. |
| **Identity-point equivalence — review 7** | Complete builders enable `generator_fold` at `src/full.rs:3844–3851`, `5439–5446`; it sets total arithmetic at `src/generator_claim_points.rs:168`. Identity-capable constraints and additions are at `src/points.rs:998–1033`, `1217–1227`. | Disagree with the blanket inference that complete recursive verification cannot allocate identity. Both complete hard and incoming paths enable identity-capable arithmetic. Hard Schnorr \(A\) nonidentity is enforced at `src/points.rs:762–768`, `1029–1033`; incoming refusal is included at `1467–1492`. The comment at `src/fs.rs:18–19` is stale. No identity-row forced-burn counterexample is established here. Test every relevant identity port against native acceptance; comprehensive equivalence remains **UNVERIFIED**. |
| **Private generator restructuring — review 6** | Circuit audit in §6.8; native exported-claim decisions at `src/engine.rs:786–811`. | Agree with the correction. Native code does not replay the private restructuring. Ordering, digest/source ties, and exact mask recurrence are source-verified in the circuits; compiler/copy and encoding premises remain necessary. |
| **Uniform-builder forgery refusal coverage — review 9** | Existing native toy test at `src/lib.rs:168–175`; toy synthesis at `src/verifier.rs:2344–2461`; actual gamma Horner at `src/points.rs:680–686` and slot weights at `src/scalar.rs:465–486`. | Agree. Exercise both forgeries inside actual complete uniform builders on both curves. Hard predecessor verification must be unsatisfiable. Total incoming verification must force invalidity and prohibit credit while preserving hard transcript/group computations. Native preflight refusal is insufficient. **Not fixed at reviewed HEAD.** |
| **Transcript tags are ignored** | `src/fs.rs:26`, `74–105`, `138`, `152`, `157`. | Analyze the actual positional schedule and authenticated profile. Do not credit ignored textual tags with domain separation. |
| **Low-level clear claim folding omits source challenge vectors locally** | `src/ipa.rs:262–285`. | Do not treat it as independently binding arbitrary source tuples. The private construction binds ordered inputs separately (`src/generator_claim.rs:169–195`). Any alternative use needs its own argument. |
| **Some private limbs have field semantics** | `src/generator_claim_scalar.rs:94–97`, `166–179`. | Preserve that precise semantics or add canonical checks before claiming unique byte encodings. This is distinct from the public verifier transcript canonicality gap. |
| **Compiler and candidate correspondence** | Program construction/import and fixed-point comparisons at `src/backend_chain_tests.rs:924–981`. | Establish exact source/program correspondence and rerun relevant checks on one frozen candidate. Retained-layout bounds do not qualify current source automatically. |

The following checks must **not** be reported as absent:

- Native matrix and inherited-generator decisions: `src/engine.rs:771–811`.
- Own IPA generator decision: `src/ipa.rs:469–473`.
- Native program-digest pinning: `src/engine.rs:430–435`, `1751–1754`.
- Generator/table profile authentication and recomputation: `src/engine.rs:53–84`, `198–228`, `454–456`.
- Certified table recomputation: `src/engine.rs:675–684`.
- Formal program-degree checks: `src/program_artifact.rs:478–482`, `575–581`; `src/engine.rs:447–452`.
- Four-kind selection with Boolean selectors: `src/points.rs:1058–1124`.
- Native canonical proof/scalar parsing and carrier re-encoding: `src/spartan.rs:88–143`; `src/carrier.rs:286–331`. These do not settle every recursive limb representation.
- Eight-program digest and canonical-byte fixed-point comparisons in source: `src/backend_chain_tests.rs:924–981`, especially `969–976`. Execution against this HEAD was not performed here.

No prematurely derived challenge was found in the inspected main committed-round, pair-link, matrix-fold, or private-generator schedules. This conclusion is narrower than full transcript/constraint equivalence.

## 10. Required synchronization of DESIGN.md and related records

These are required document updates, not edits performed by this read-only revision.

### 10.1 One-proof argument

DESIGN.md §5 and its security table should describe straight-line recorded representations rather than gamma rewinding as the extraction mechanism.

Retain:

- Degree two per committed sumcheck variable.
- Degree 26 for the current three-variable Kronecker block.
- Initial degree \(2^{s_x}-1\).
- Degree-three matrix combination.
- Degree-\(2m\) gamma separation and degree-\(m+1\) alpha batching.
- Actual challenge atoms and shared-squeeze correlations.

Explain the weighted outer recurrence without division. Update `TODO.md:26` to **2.115 bits relative to three independent degree-two coordinates**.

### 10.2 Deferred matrix claims

DESIGN.md §7.3 should state

\[
\Pr[\text{false-to-true fold}]
\le \deg(q-P\circ\Gamma)\lambda_\beta,
\]

with the retained bounds 186 and 174 when the true-degree hypotheses hold. There is no factor four.

Distinguish authenticated program identity, declared degree, and actual degree. Current import/admission formal-degree checks are present; they should not remain described as missing. Execution and compiler semantics remain separate qualifications.

Bootstrap emptiness follows from \(r=0\); Load uses corner \((1,1)\).

### 10.3 Linking and generator restructuring

DESIGN.md §7.2 should retain the full canonical Fp source for \(\rho\), the \(n/p\) link bound, and the fixed-before-\(\rho\) requirement for values and mask.

The generator section should identify the same pair’s circuits as the verifier of private restructuring, including the source-bound digest and exact mask recurrence. Native acceptance decides the exported claim only.

### 10.4 Recursive security and monetary claim

DESIGN.md §11 and `TODO.md:20–22` should replace DAG induction and causal-extraction language with:

- Minimal P2-RKFS A/B.
- Closure over a finite extracted statement set.
- Own-chain rootedness.
- The explicit conservation corollary, including no forks and no second Bootstrap.
- Round-by-round query accounting.

Remove the proposed global-rank obligation. Keep native Load finality, custody, unique enrollment, and joint zero knowledge as separate boundaries.

Current DESIGN.md §4.3 already records finding 4. Other statements that a valid Payment cannot be forced to burn must remain scoped to native behavior until the circuit fix and equivalence tests pass. Do not claim the identity-point counterexample from the stale `fs.rs` comment.

`research/CONSTRUCTION.md:57–86` still needs reconciliation with four kinds, current mask behavior, and complete monetary relations. The residual selector comment at `src/full.rs:4728–4730` also needs correction.

## 11. Conditions for independent acceptance

G1 and G2 remain conditional on resolving or explicitly accepting:

1. P2-RKFS’s algebraic-adversary model and concrete recursive RP57 idealization, or a checked replacement theorem.
2. A frozen candidate with authenticated parameters, exact program/source correspondence, and justified actual-degree bounds.
3. Enforced LogUp phase invariants and intended compiler semantics.
4. Correct IO-leaf binding, canonical cross-field transcript values, and qualified native/hard/total verification equivalence.
5. CancelRound and AdaptiveWitness refusal through the actual complete uniform builders.
6. Finite lifetime/query budgets and a target including cryptographic and concrete-recursion terms.
7. Custody preventing forks and second Bootstrap, together with native funding finality and receipt authorization.

The supported result remains a **conditional security argument with quantified local algebraic losses**, not approval for wallets to use Path 2.

---

## Response to review

1. **Accepted.** Reduced P2-RKFS to representation access and concrete recursive RP57 idealization. Derived prefix knowledge, linking, accumulation closure, polynomial extraction cost, and query accounting. Removed gamma rewinding as the extraction mechanism.
2. **Accepted.** Added the honest-wallet experiment, challenger records, statement/descriptor identity, and honest-output pointers. Preserved checks of each proof occurrence. Replay keys use the Request credit identifier (`src/receive_lineage.rs:188–215`).
3. **Accepted.** Replaced DAG induction with finite-set closure, removed causal closure/global rank, and added conservation with no forks and no second Bootstrap. Own-chain descent is supported by `src/monetary_state.rs:91–93` and the operation increments.
4. **Accepted; open.** Verified uncovered IO leaves and withdrew the unconditional circuit burn claim. Specified complete leaf binding and an unsatisfiable-mutation test (`src/scalar.rs:779–790`; `src/full.rs:4970–5044`).
5. **Accepted; enforcement open.** Recorded the current phase audit, tightened the lemma, and specified builder rejection of challenge-dependent semantic tuples (`src/uniform.rs:1461–1464`, `1522–1553`; `src/deferral.rs:582–609`).
6. **Accepted.** Corrected native-versus-circuit responsibility and audited digest/source ties, ordering, and mask recurrence (`src/generator_claim_points.rs:170–273`; `src/generator_integration.rs:297–355`).
7. **Partly accepted.** Confirmed missing full-scalar canonicality (`src/full.rs:2846–2875`, `5031–5044`). The identity inference is unsupported for complete builders: generator folding enables identity-capable arithmetic (`src/generator_claim_points.rs:168`; `src/points.rs:998–1033`). Comprehensive equivalence testing remains open.
8. **Accepted.** Added round-by-round soundness with delayed residuals and both query bounds. Charged LogUp and matrix folding jointly when they share one reply.
9. **Accepted; open.** Distinguished toy evidence from complete uniform-builder coverage and specified hard refusal versus total-incoming invalidity (`src/lib.rs:168–175`; `src/verifier.rs:2344–2461`).
10. **Accepted.** Made actual degree authoritative and distinguished completeness from soundness. Also recorded newly committed degree guards (`src/program_artifact.rs:478–482`; `src/engine.rs:447–452`).
11. **Accepted.** Updated provenance and citations to final HEAD, removed resolved gap descriptions, retained §7 unchanged numerically, and identified remaining record corrections. Current acceptance anchors are `src/engine.rs:720–811`.