<!-- Written by Codex astra (gpt-6-astra, ultra reasoning), read-only, on 2026-11-18 at iroha 60309bba27, for TODO G1/G2; prompt and run log in ../../../output/kagemusha-100x-20261108/astra-g1g2-20261118/ (local, git-ignored). Saved unedited. G3 (independent review) has not happened yet. -->

# KAGEMUSHA Path 2: conditional soundness argument and release obligations

**Status: not approved for wallet use.** This document supplies a conditional recursive argument and explicit local algebraic bounds. It does not establish security of the complete implementation from discrete logarithm hardness alone. The recursive knowledge assumption, compiler obligations, authenticated parameter identity, and findings below require independent review before G1–G3 can close.

## 1. Scope and evidence

This review was performed read-only on 2026-11-18 in `/Users/takemiyamakoto/dev/iroha`, with HEAD:

```text
60309bba27e8205c6196bb0c6b040b954f6f584e
```

No files were changed, builds performed, or test suites executed. Existing source, artifacts, and logs were inspected. Unless explicitly rooted at the repository, `src/`, `output/`, and document paths below are relative to `experiments/kagemusha-spartan/`.

The starting records were:

- `TODO.md`, particularly G1–G3.
- `DESIGN.md` §§4, 5, 7.1–7.7, 8, and 11.
- `research/REPORT-b04a57-soundness.md`.
- `research/CONSTRUCTION.md`.
- Repository-root `output/kagemusha-100x-20261108/astra-design-review-20261113/answer.md`.

Some findings in those records have since been addressed. In particular, current native code derives an incoming validity verdict and supports the burn path, checks program digests, and decides both kinds of accumulator. Those improvements must not be confused with completion of the recursive security argument.

The numerical evaluation uses the retained **2026-11-17 complete-backend `p18-s17` candidate**. Its artifacts are under `output/full-backend-run-20261117T051751Z/`; its log is `output/s2/chain-20261117T051751Z.log`. Correspondence of all retained artifacts to the inspected HEAD is **UNVERIFIED**.

The following distinctions apply throughout:

- **Proved conditional lemma:** a mathematical implication under explicitly stated hypotheses.
- **Source-verified:** an inspected implementation fact.
- **Assumption:** a property not proved here.
- **UNVERIFIED:** a required correspondence, invariant, or qualification not established by this review.

## 2. Security claim and exclusions

The intended claim is:

> A natively accepted Path 2 pair has an efficiently extractable, finite history of correctly linked operation witnesses. Every hard predecessor obligation and every accepted incoming-credit obligation is supported by that history. Deferred matrix and generator obligations cannot disappear except through the explicitly bounded algebraic failure events or a failure of the stated cryptographic assumptions.

This claim concerns the authenticated relation and its extracted history. It does **not** imply:

1. **Global uniqueness of spending.** Two Sends from the same funded predecessor can each satisfy the relation. Preventing those forks requires trusted wallet execution, protected state, and custody that remains correct through concurrency, restart, rollback, and recovery.
2. **In-circuit Load finality.** The Load relation binds the receipt and its monetary effect. Native code verifies ledger finality before preparation.
3. **Globally unique Bootstrap.** Bootstrap authenticates a credential and initial state; uniqueness of enrollment and installation remains a provider obligation.
4. **Anonymity or unlinkability.** This document proves no joint zero-knowledge theorem for the complete recursive export, generator restructuring, or retry behavior.
5. **Security for infinite attempts.** Every quantitative statement has finite operation, history, and query budgets.

The local conservation equations are in `src/send_arithmetic.rs:90–147`; native Load finality is in `src/engine.rs:1028–1048`, with the relation’s exclusion stated at `src/load.rs:1–7`. Provider custody responsibilities are explicit at `src/native.rs:1–6`.

## 3. Construction and accepting predicate

### 3.1 Fields, curves, and authenticated programs

Write \(p<q\) for the Pasta primes:

```text
p = 0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001
q = 0x40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001
```

P proofs have scalar field \(\mathbb F_p\) and Vesta commitments, whose coordinates lie in \(\mathbb F_q\). S proofs have scalar field \(\mathbb F_q\) and Pallas commitments, whose coordinates lie in \(\mathbb F_p\). The definitions are repository-root `crates/iroha_pasta/src/field/fp.rs:7–16` and `fq.rs:7–16`.

The four kinds are Receive, Send, Bootstrap, and Load. Two Boolean selectors choose their programs, including the bilinear fourth-corner term for Load. The implemented fourth-key selection and Boolean checks are at `src/points.rs:1058–1124`; four-kind matrix-selector handling is at `src/full.rs:4740–4751`.

Generators are derived by curve-specific hash-to-curve from `kagemusha-zks-spike/gens`, with \(g_i\) at index \(i\), \(H\) at capacity \(N\), and \(U\) at \(N+1\). Prefixes retain the parent’s \(H,U\). Thus capacity and prefix behavior are security parameters, not incidental allocation choices (`src/ipa.rs:24–26`, `50–81`).

The native manifest authenticates eight program digests and matrix dimensions. Regenerated operation programs are compared against those identities (`src/engine.rs:166–184`, `1393–1396`). The additional generator and compiler obligations are recorded in §9.

### 3.2 What native acceptance must check

The central accepting predicate is `src/engine.rs:419–489`. It:

1. Selects the authenticated programs for the operation.
2. Verifies both linked Spartan proofs.
3. Checks the reconstructed pairing digest and challenge.
4. Evaluates both final accumulated matrix claims against the installed combined polynomials.
5. Decides both inherited generator claims.

Each linked Spartan verification also decides its **own** IPA generator claim by recomputing the public MSM (`src/spartan.rs:1973–1976`; `src/ipa.rs:469–481`).

Consequently, the terminal pair has:

- Two main proof verifications.
- Two exact matrix-accumulator decisions.
- Two exact own-generator decisions.
- Two exact inherited-generator decisions.
- The pair-link and authenticated-profile checks.

A successful lower-level linked proof verification is insufficient if the accumulated matrix or inherited-generator decisions are omitted.

The inspected native paths include head verification, Payment verification, fresh output verification, output revalidation, and recovery (`src/engine.rs:528–559`, `1140–1157`, `1179–1235`, `1451–1463`, `1525–1559`). Complete coverage of every exported caller and failure/recovery path is **UNVERIFIED**.

### 3.3 Commitment and challenge ordering

The main proof schedule is:

1. Absorb shape and ordinary-public-input digest.
2. Absorb first-phase row commitments.
3. Derive the pair-link word.
4. Absorb link-phase row commitments.
5. Derive the two phase-two challenges.
6. Absorb late-output digest, remaining rows, fixed rows, and link \(R\).
7. Derive the initial sumcheck point.
8. For each outer round, absorb its commitment before its challenge.
9. Absorb \(v_A,v_B,v_C\), then derive their batching challenge.
10. For each inner round, absorb its commitment before its challenge.
11. Derive equation-batching \(\alpha\), then separation \(\gamma\).
12. Run the IPA, absorbing each message before its associated challenge.

Source anchors are `src/spartan.rs:299–304`, `1449–1515`, `1551–1575`, `1593–1597`, `1633–1659`, and the verifier replay at `1914–1976`.

Late IO is intentionally sampled after phase-two challenges: it contains the resulting accumulator. Its correctness therefore depends on a previously committed fold polynomial and constrained evaluation, not on pretending that late IO was an ordinary first-phase statement.

### 3.4 Recursive implementation

The verifier is divided between scalar arithmetic in the proof’s scalar field and group/transcript arithmetic in the other circuit. The actual uniform builders are `src/scalar.rs` and `src/points.rs`; `src/verifier.rs` also contains tracing and census machinery.

For example:

- Gamma inversion, alpha powers, and the \(r,r^2,r^3\) equations are constrained at `src/scalar.rs:808–846`.
- Canonical transcript-word decomposition is implemented at `src/points.rs:1624–1786`.
- Canonical generator-coordinate limbs are constrained at `src/points.rs:1522–1621`.
- The shared verification values are enumerated at `src/verifier.rs:2046–2100` and connected by the pair link.

Counting a check in the census is not evidence that a constraint exists. In particular, the counted comparisons at `src/verifier.rs:245–257` and `362–416` must not substitute for review of the actual uniform builders.

## 4. G1: explicit recursive knowledge assumption

### 4.1 No composition theorem is imported implicitly

This document does **not** assert that a named PCD theorem has been checked hypothesis by hypothesis for this construction.

Accumulation-based PCD and split accumulation provide relevant frameworks, but applicability requires the appropriate argument-of-knowledge and accumulation properties for the actual protocols. See [BCMS, *Proof-Carrying Data from Accumulation Schemes*](https://eprint.iacr.org/2020/499) and [BCLMS, *Proof-Carrying Data without Succinct Arguments*](https://eprint.iacr.org/2020/1618).

A further issue is that the recursive circuits evaluate a concrete hash permutation. An ordinary black-box random-oracle proof does not automatically justify this use. [*Halo Infinite*](https://eprint.iacr.org/2020/1536) explicitly describes the corresponding non-black-box Fiat–Shamir step as heuristic.

We therefore state an additional assumption for this exact construction. Accepting it is a substantive security decision.

### 4.2 Assumption P2-RKFS

Let a frozen profile \(\Pi\) identify:

- Both field and curve implementations.
- Generator derivation, capacities, prefix rules, and table ordering.
- The eight exact programs and their interpretation.
- Proof and carrier encodings.
- RP57 constants, transcript domains, absorption schedule, and challenge maps.
- The matrix and generator accumulation protocols.
- The hard-predecessor and total-incoming verification predicates.

Let:

- \(T\): adversary running-time budget.
- \(Q\): total relevant hash/permutation-query budget, including grinding and failed attempts.
- \(N\): maximum number of distinct extracted operation nodes.
- \(D\le N\): maximum extracted dependency depth.
- \(B\): maximum encoded node and witness size under \(\Pi\).
- \(\eta>0\): extraction success slack above the accounted failure probability.

**P2-RKFS\((\Pi,T,Q,N,D,B)\)** assumes the existence of an extractor with the following properties against adaptive classical adversaries. The adversary may choose arbitrary witnesses, group elements, proof bytes, statements, counterparties, forks, and proof attempts; it is not restricted to the honest prover.

1. **Prefix-consistent knowledge.** The extractor obtains consistent openings for witness rows, committed round vectors, fold-polynomial coefficients, and private generator-restructuring inputs. The openings are attributable to the transcript prefix at which the associated commitment was fixed. Later sumcheck, batching, or opening challenges cannot retroactively determine an earlier committed witness.

2. **Two-field closure.** The extracted scalar-side and points-side witnesses describe the same ordered verification tasks, field values, source proofs, kinds, and output accumulators, except for the pair-link failure event bounded below.

3. **Accumulation extraction.** A true terminal accumulator, together with the extracted accumulation relations, yields the corresponding source obligations unless a bounded false-to-true accumulation event occurred. This includes both own and inherited generator claims and both matrix-claim families.

4. **Causal closure.** For every extracted *valid* parent proof, the extractor supplies a dependency index strictly smaller than that of its child, or identifies a previously supplied honest proof with an already recorded history. Invalid incoming evidence assigned to burn is retained as exact evidence, not promoted to a valid parent. This clause excludes self-supporting and mutually supporting proof cycles.

5. **Efficient DAG extraction.** Extraction memoizes distinct nodes using canonical proof/statement identities. Its expected running time is bounded by
   \[
   (T+Q+NB)\operatorname{poly}(\lambda,1/\eta),
   \]
   for a fixed polynomial independent of \(D\). In particular, there is no factor \(c^D\), and repeated occurrences of a shared ancestor do not cause repeated expansion of an exponential tree.

6. **Concrete recursive Fiat–Shamir accounting.** The ideal challenge experiments used in §§5–7 remain applicable to the concrete, recursively evaluated RP57 implementation, with an additional failure term
   \[
   \epsilon_{\mathrm{RKFS}}(\Pi,T,Q,N,D,\eta).
   \]
   Adaptive attempts can be charged to at most \(Q+N\) challenge experiments with the union-bound accounting in §8.

These clauses are **assumptions**, not consequences proved by this document. In particular:

- The Laurent-polynomial lemma below establishes aggregate separation; it does not independently prove clause 1.
- The existing state counter does not independently prove clause 4.
- Repeated application of an ordinary single-proof extractor does not establish clause 5.
- No concrete numerical bound for \(\epsilon_{\mathrm{RKFS}}\) is established here.

The assumption is about extraction of the specified algebraic relations and dependencies. Monetary correctness still requires the application relations, compiler, parameter identity, and custody hypotheses.

### 4.3 Well-foundedness supplied by the implemented state

There is a genuine decreasing measure on **own-predecessor** edges.

`Core[11]` is an unsigned 128-bit sequence (`src/monetary_state.rs:91–93`). Bootstrap initializes it to zero (`src/bootstrap.rs:260–278`). Send, Receive, and Load increment it by one (`src/send_arithmetic.rs:121–125`; `src/receive.rs:331–336`; `src/load.rs:351–356`). The before-state commitment is joined to the authenticated predecessor head (`src/operation_auth.rs:333–345`).

Because both states are range constrained, the increment cannot wrap a Pasta field. Every extracted own-predecessor edge therefore decreases a natural number. An own-wallet path has length at most \(2^{128}-1\).

That is **not** a well-foundedness proof for the joined history.

A Receive also refers to a payer Send. The payer’s sequence is bound to its own statement and receipt, but is not ordered against the receiver’s sequence (`src/receive_originals.rs:424–439`). A path can switch wallets. Local monotonicity does not exclude a cycle formed by such switches.

Accordingly, the present argument needs the causal-closure clause of P2-RKFS. An alternative would be a separately authenticated rank that decreases along both parent edges. No such global rank was found in the current relation. This is a missing proof hypothesis, not a demonstrated cyclic-proof forgery.

### 4.4 Induction over the extracted DAG

Condition on:

- P2-RKFS succeeding.
- Correct authenticated parameters and compiler semantics.
- No cryptographic binding, hash, or signature failure.
- No local algebraic failure event counted below.

The causal indices give a finite topological order. Apply induction in that order.

| Kind | Extracted obligations and resulting statement |
|---|---|
| **Bootstrap** | Authenticate root/issuer/credential, initial state, and own receipt; constrain zero initial balance and counters and the required empty roots. There is no predecessor obligation. Matrix and inherited-generator outputs are fixed true constants. |
| **Load** | Establish a valid own predecessor, exact ordinary receipt, correct amount and ordinal/recovery effects, and signed transition. Funding finality additionally requires the native finality boundary. |
| **Send** | Establish a valid own predecessor, authenticated Request and applicable controls/maps, checked integer debit, preserved unrelated state, and signed successor bound to the recipient. |
| **Receive, credit** | Establish the hard own predecessor and the incoming payer’s exact valid Send history, plus original-object matching, Request authentication, replay nonmembership, and the credit transition. Both ancestor sets enter the induction; shared ancestors are counted once. |
| **Receive, burn** | Establish the hard own predecessor and the correctly derived invalid/replayed incoming verdict. Preserve exact incoming evidence and the required accounting/map behavior. A malformed payer proof is not required to have a valid monetary history. |

The application joins are implemented at:

- Bootstrap: `src/bootstrap.rs:232–319`, `392–468`, `488–553`.
- Load: `src/load.rs:305–414`.
- Send: `src/send.rs:163–244`; `src/send_arithmetic.rs:90–179`.
- Receive: `src/receive.rs:164–248`, `326–379`; `src/receive_originals.rs:336–475`; `src/receive_lineage.rs:186–253`.

For each non-Bootstrap node, the accumulation lemmas transfer the true terminal claims to its hard source obligations. The pair-link lemma then joins the scalar and group computations. The Spartan argument establishes the selected operation relation. The inductive hypotheses establish the parent histories.

This proves the intended extracted-history claim **conditionally**. “Only Bootstrap has no predecessor” would establish none of the extraction, acyclicity, or efficiency steps above.

### 4.5 Receive containment and native finality

Receive’s incoming validity is a constrained conjunction, not a caller-selected permission to burn (`src/receive.rs:228–248`). Replay nonmembership participates in the final verdict (`src/receive_lineage.rs:186–207`).

Burn accounting credits the Core balance and adds the same amount to adjusted burned value, so spendable value does not increase. Existing credit-digest records are retained. The invalid branch permits the documented optional structurally valid consumed-map insertion; it does not universally require insertion of the incoming descriptor (`src/receive_lineage.rs:204–253`).

For incoming matrix claims, the original point is retained while a corrected value is accumulated; equality with the original value contributes to validity (`src/full.rs:4673–4678`, `4754–4781`). Generator correction similarly retains the original challenges and compares original/corrected points (`src/generator_integration.rs:85–110`, `299–355`). The hard own-predecessor obligations must not receive this treatment.

Current native admission derives the incoming verdict by full Payment verification and propagates resource/unavailability errors separately (`src/engine.rs:780–834`). Complete end-to-end equivalence between every native failure category and the total circuit verdict is **UNVERIFIED**.

Load finality is verified natively before preparation (`src/engine.rs:1028–1048`). Thus even a perfect proof-system theorem does not make an arbitrary accepted Load relation a proof of ledger finality. Funding security additionally trusts the released native/provider path and its signing boundary.

## 5. Actual challenge distributions

### 5.1 RP57 model

The transcript uses width three, rate two, eight full rounds and 57 partial rounds, with pinned field-specific constants. See repository-root `crates/iroha_pasta/src/poseidon/mod.rs:42–58`, and `src/fs.rs:15–47`.

The following probability calculations assume an ideal uniform squeezed field word at the relevant fresh transcript prefix. They are not statistical claims about a fixed deterministic Poseidon invocation. Concrete instantiation and recursive use belong to P2-RKFS.

Absorption discards the spare challenge half (`src/fs.rs:63–71`). The next two challenges without intervening absorption may share one squeezed word (`src/fs.rs:111–129`).

### 5.2 Raw 126-bit challenges

Set \(H=2^{126}\). Each Pasta modulus \(r\in\{p,q\}\) satisfies

\[
r=4H^2+\delta_r,\qquad 0<\delta_r<H.
\]

For a uniform \(Z\in\{0,\ldots,r-1\}\), the first raw challenge is \(Z\bmod H\). Its exact maximum atom is

\[
\lambda_0(r)=\frac{4H+1}{r}.
\]

Conditional on that first half, the maximum atom of the next half is

\[
\lambda_1=\frac{5}{4H+1}.
\]

Indeed, when the first half \(a<\delta_r\), there are \(4H+1\) compatible words and the second half zero has five preimages. Otherwise there are \(4H\) compatible words, with four preimages per second-half value.

A convenient conservative common bound is

\[
\boxed{\lambda_*=\frac{5}{4H}}.
\]

The joint distribution of both halves has maximum atom \(5/r\). This joint bound matters for LogUp; one must not replace it by independent uniform sampling.

The extraction and raw-integer conversion are at `src/fs.rs:121–158`, `216–222`; real circuit decomposition and original-modulus checks are at `src/points.rs:1624–1786`.

### 5.3 Endoscalars are injective and nonzero

The endoscalar map consumes 63 bit pairs. Starting from \(a=b=2\), each pair doubles both coordinates and adds \(+1\) or \(-1\) to one coordinate. It returns

\[
e=a\zeta+b,
\]

where \(\zeta^2+\zeta+1=0\) in the scalar field (`src/fs.rs:224–254`).

After 63 steps,

\[
2^{63}+1\le a,b\le3\cdot2^{63}-1.
\]

The integer encoding is injective. In reverse, the odd coordinate identifies which coordinate received the signed increment. Except for the final reverse step, the previous coordinate sum is odd, so the current sum modulo four identifies the sign. The remaining step is determined by the initial pair \((2,2)\).

Suppose two outputs coincide modulo scalar prime \(r\). Then

\[
\Delta a\,\zeta+\Delta b=0
\quad\Longrightarrow\quad
r\mid \Delta a^2-\Delta a\Delta b+\Delta b^2.
\]

The differences have magnitude below \(2^{64}\), so the nonnegative norm is below \(3\cdot2^{128}<r\). It must therefore be zero, implying \(\Delta a=\Delta b=0\). The same norm argument applied to positive \(a,b\) proves that no output is zero.

Hence:

- Endoscalars preserve the source bits’ atom bounds.
- They are not uniform over the scalar field.
- There is no additional collision-multiplicity or zero-challenge penalty for this map.

### 5.4 Full-width cross-field challenges

Full-width challenges recombine the entire squeezed canonical word in the scalar field (`src/fs.rs:136–143`, `178–182`).

Therefore:

| Use | Source and destination | Maximum atom |
|---|---|---:|
| P initial sumcheck challenge | Uniform \(\mathbb F_q\) word reduced into \(\mathbb F_p\) | \(2/q\) |
| S initial sumcheck challenge | Uniform \(\mathbb F_p\) word embedded into \(\mathbb F_q\) | \(1/p\) |
| Pair-link challenge | S’s full \(\mathbb F_p\) word used in \(\mathbb F_q\) | \(1/p\) |

The pair-link challenge is **not 126 bits**. Its exact source is `src/spartan.rs:222–226`. Since \(p<q\), its cross-field conversion is injective (`src/verifier.rs:2150–2153`).

## 6. G2: local soundness lemmas

All lemmas below require the relevant commitments or extracted representations to be fixed before the challenge being analyzed. Ordinary computational binding alone must not silently replace that knowledge/timing hypothesis.

### 6.1 Gamma separation and alpha batching

Let \(m\) be the total number of committed outer and inner rounds. After fixing the prefix preceding \(\gamma\), including \(\alpha\), write

\[
C(\gamma)=C_0+\sum_{i=1}^{m}\gamma^iD_i,
\qquad
b(\gamma)=b_0+\sum_{j=1}^{m}\gamma^{-j}b_j.
\]

Here \(C_0\) is the Hyrax row aggregate; \(b_0\) is supported on active witness columns; \(b_j\) is supported on round \(j\)’s intended slot. Honest slot placement and the inverse scaling are implemented at `src/spartan.rs:328–339`, `467–554`, `632–650`, `1674–1715`.

**Rewinding step.** Extract aggregate IPA openings at \(m+1\) distinct accepting \(\gamma\) values with the same preceding prefix. Vandermonde inversion gives full-vector openings \(W_0,\ldots,W_m\) of \(C_0,D_1,\ldots,D_m\). A further accepting opening inconsistent with

\[
W(\gamma)=\sum_{i=0}^{m}\gamma^iW_i
\]

would give two different openings of the same commitment, hence a nontrivial relation among the commitment generators and blinding base.

The residual is

\[
L(\gamma)=
\left\langle
\sum_{i=0}^{m}\gamma^iW_i,\,
b_0+\sum_{j=1}^{m}\gamma^{-j}b_j
\right\rangle-\kappa.
\]

Its Laurent exponents lie in \([-m,m]\). If nonzero, \(\gamma^mL(\gamma)\) is a nonzero ordinary polynomial of degree at most \(2m\). Since endoscalars are nonzero,

\[
\boxed{\Pr[L(\gamma)=0\text{ accidentally}]\le2m\lambda_\gamma.}
\]

The constant coefficient is

\[
\langle W_0,b_0\rangle+
\sum_{i=1}^{m}\langle W_i,b_i\rangle-\kappa.
\]

This is exactly the intended alpha-batched diagonal-slot relation. Off-slot contributions affect nonconstant powers. The lemma need not establish that every malicious commitment has zero off-slot coordinates; it establishes that those coordinates cannot replace the intended constant coefficient except at a root.

There are \(m+2\) scalar consistency/final equations, with alpha powers through \(m+1\) (`src/spartan.rs:481–543`). If their fixed error vector is nonzero,

\[
\boxed{\Pr[\text{alpha batch vanishes}]\le(m+1)\lambda_\alpha.}
\]

Thus the conditional combined separation/batching error is

\[
2m\lambda_\gamma+(m+1)\lambda_\alpha.
\]

The often quoted \(2m+1\) is not the exact root count here: a degree-\(2m\) polynomial has at most \(2m\) roots; \(2m+1\) evaluations can force an identity.

**CancelRound and AdaptiveWitness.** CancelRound places the earlier round’s replacement opening in a later commitment. AdaptiveWitness places a witness-column correction in a round commitment after the witness’s earlier challenge. Those terms acquire nonzero gamma exponents and cannot repair the constant coefficient except through the events above. The explicit attacks are at `src/spartan.rs:1554–1569`; their regression is at `src/lib.rs:163–180`.

**Limit of the proof.** Late gamma rewinding does not itself prove that \(W_i\) was known before round \(i\)’s own challenge. Nor does it alone extract separate witness rows from \(C_0\), which depends on row-fold challenges. These requirements are precisely the prefix-consistent knowledge obligation in P2-RKFS. The regression tests do not discharge it.

### 6.2 Initial sumcheck test

Let \(e_x=(Az)_x(Bz)_x-(Cz)_x\) on the Boolean constraint cube. A false committed assignment has a nonzero error table. Its multilinear extension is

\[
F(\tau)=\sum_x\operatorname{eq}(\tau,x)e_x.
\]

The lean schedule sets \(\tau_i=t^{2^i}\). Distinct multilinear monomials become distinct powers of \(t\), so a nonzero \(F\) remains nonzero after substitution, with degree at most \(2^{s_x}-1\).

Therefore:

\[
\epsilon_{\tau,P}\le\frac{2(2^{s_x}-1)}q,\qquad
\epsilon_{\tau,S}\le\frac{2^{s_x}-1}p.
\]

The schedule is `src/fs.rs:185–203`; implementation is `src/spartan.rs:1290–1296`, `1513–1515`.

### 6.3 Committed sumcheck rounds

For the outer sumcheck, define the true round polynomial directly by

\[
Q_i(y)=
\sum_{z\in\{0,1\}^{\text{remaining}}}
\operatorname{eq}(\tau_{\text{remaining}},z)
E(r_{\text{prefix}},y,z),
\]

where \(E=\widetilde{Az}\widetilde{Bz}-\widetilde{Cz}\).

The implemented recurrence is

\[
\sum_{y\in\{0,1\}^{k_i}}
\operatorname{eq}(\tau_{\text{block}},y)Q_i(y)
=Q_{i-1}(r_{\text{previous}}),
\]

ending at \(v_Av_B-v_C\). Each \(Q_i\) has individual degree at most two. See `src/spartan.rs:481–505`, `578–629`, `1542–1547`.

If an incoming claimed value is false, the claimed-minus-true round polynomial is nonzero because its weighted Boolean sum is nonzero. No division by an equality-factor evaluation occurs. Consequently there is no additional equality-factor-zero event for this implemented recurrence.

The inner sumcheck has the same individual degree bound and ends at the matrix/witness/public-input relation (`src/spartan.rs:507–543`).

For a lean block of \(k\) variables, substitution

\[
(r,r^3,r^{3^2},\ldots)
\]

maps the degree-two monomials injectively to distinct exponents. The resulting degree is at most \(3^k-1\), giving

\[
\boxed{\epsilon_{\mathrm{block}}\le(3^k-1)\lambda_0.}
\]

Early inner blocks retain separate coordinates, including endoscalar row coordinates, and contribute at most twice the sum of their conditional atom bounds (`src/spartan.rs:306–325`).

At current \(k=3\), the degree is 26. The extra loss relative to three independent degree-two coordinates is

\[
\log_2(26/6)\approx2.115\text{ bits}.
\]

The degree loss relative to one 126-bit challenge is \(\log_2 26\approx4.700\) bits. For \(k=4\), the comparative loss is approximately 3.322 bits; for \(k=5\), 4.597 bits. An undifferentiated “3–4-bit loss” is not an exact description of the current \(k=3\) schedule.

### 6.4 The \(r,r^2,r^3\) matrix combination

After committing to \(v_A,v_B,v_C\), the prover receives one raw challenge \(r\), used as \(r,r^2,r^3\) (`src/spartan.rs:1308–1314`, `1593–1597`).

For a fixed nonzero discrepancy vector,

\[
r\Delta_A+r^2\Delta_B+r^3\Delta_C
\]

is a nonzero degree-at-most-three polynomial. Hence

\[
\boxed{\epsilon_{\mathrm{ABC}}\le3\lambda_0.}
\]

The root \(r=0\) is included. It must not be excluded by assuming that raw challenges are nonzero.

### 6.5 Main zero-knowledge IPA

The implemented folding equations are

\[
x'=ux_{\mathrm{lo}}+x_{\mathrm{hi}},\quad
b'=b_{\mathrm{lo}}+ub_{\mathrm{hi}},\quad
G'=G_{\mathrm{lo}}+uG_{\mathrm{hi}},
\]

\[
P'=uP+u^2L+R.
\]

The commitment/value precede the randomized-\(U\) challenge; each \(L,R\) precedes its folding challenge; the final generator and Schnorr commitment precede the Schnorr challenge (`src/ipa.rs:253–257`, `318–404`, `407–419`).

Under the corresponding representation/knowledge extraction hypothesis:

- Randomized-\(U\) separation contributes \(\lambda_0\).
- Each of \(\ell\) folds contributes at most \(2\lambda_0\).
- The Schnorr final contributes \(\lambda_0\).

Thus

\[
\boxed{\epsilon_{\mathrm{IPA}}\le(2\ell+2)\lambda_0.}
\]

This is a conditional algebraic knowledge-error accounting, not a complete Fiat–Shamir extraction reduction.

It requires the exact generator equation

\[
G_{\mathrm{final}}=\langle s(u),g\rangle,\qquad
s_i(u)=\prod_j u_j^{\operatorname{bit}_j(i)}.
\]

Native verification decides it at `src/ipa.rs:423–434`, `469–481`; recursive verification carries the obligation forward. A freely chosen undecided \(G_{\mathrm{final}}\) invalidates this argument.

### 6.6 Four-claim matrix accumulation

The combined matrix polynomial is

\[
P(c_0,c_1,r,a,b,x,y)=
\sum_k
\operatorname{eq}((c_0,c_1),\operatorname{corner}_k)
\operatorname{pad}_k(x,y)
\bigl(rA_k+r^2B_k+r^3C_k\bigr)(a,b;x,y).
\]

Selectors, padding, fresh points, and the degree bound are implemented at `src/deferral.rs:111–171`, `218` onward.

Let \(\Gamma(t)\) be the coordinatewise cubic through four input points \(z_0,\ldots,z_3\), at \(t=0,1,2,3\). Let the corresponding claimed values be \(v_0,\ldots,v_3\).

The prover commits to coefficients of \(q\), of degree at most

\[
D_M=3(2+X+Y)+9+3d_a+3d_b.
\]

The coefficient array is first-phase. The constraints check all four equations \(q(j)=v_j\). Only \(\Gamma(\beta)\) and the Horner evaluation \(q(\beta)\) are designated second-phase (`src/deferral.rs:495–560`, `561–625`).

If any input claim is false, then

\[
E(t)=q(t)-P(\Gamma(t))
\]

is nonzero: at that input’s interpolation node, \(E(j)=v_j-P(z_j)\ne0\). Its degree is at most \(D_M\). Therefore

\[
\boxed{\epsilon_{\mathrm{matrix\ fold}}\le D_M\lambda_\beta.}
\]

There is **no factor four**. Four claims produce one nonzero error polynomial and one root event. Repeating the two predecessor claims for Send or Load does not change the proof (`src/full.rs:4640–4680`).

The actual challenge is the second raw phase-two challenge, `ch[1]` (`src/deferral.rs:614–625`). It is neither full-field uniform nor an endoscalar.

Input claims are connected to inherited IO, fresh proof challenges, kind selectors, and the combined matrix value at `src/full.rs:4754–4888`. The resulting accumulator is exposed as late IO at `4891–4893`.

Native decision is an exact equality evaluation (`src/engine.rs:451–463`). It contributes no additional random error. The probabilistic event is an earlier false-to-true fold.

**Bootstrap.** Its constant empty claim has \(r=0\), value zero, and fixed coordinates. This makes every term vanish even though selector \((1,1)\) is Load’s corner. The reason is zero \(r\), not an unused corner (`src/deferral.rs:157–171`; `src/full.rs:5666–5671`, `5713–5718`).

### 6.7 Pair link

Both circuits constrain

\[
R=\mu\rho^n+\sum_{j=0}^{n-1}v_j\rho^{n-1-j}
\]

over \(\mathbb F_q\). The shared values and mask \(\mu\) are first-phase witnesses; the Horner chain is a link-phase witness constrained to the public \(R\) (`src/full.rs:5124–5150`, `5238–5286`). The P-side foreign-field arithmetic and shared-value ranges are implemented in `src/nonnative.rs:2194`, `2308` onward.

P derives its first-phase digest; S absorbs it and derives the full-width \(\rho\) after its own first-phase commitments (`src/spartan.rs:191–226`).

If the two committed vectors \((\mu,v)\) differ as \(\mathbb F_q\) elements, their difference polynomial is nonzero of degree at most \(n\). Since \(\rho\) is a canonical \(\mathbb F_p\) word embedded into \(\mathbb F_q\),

\[
\boxed{\epsilon_{\mathrm{link}}\le n/p.}
\]

In general, reduction from uniform source field size \(p\) into target field size \(q\) would give maximum atom \(\lceil p/q\rceil/p\). Here \(p<q\), so conversion is injective.

The mask must be fixed before \(\rho\). Randomness of the mask is not needed for this soundness lemma, though privacy may need it. Bootstrap currently uses a deterministic mask (`src/full.rs:5668`, `5713`).

This proves equality of field coefficients. It does not prove equality of arbitrary noncanonical private integer representatives of those coefficients.

### 6.8 Private generator accumulation

A source claim is

\[
G_i=\langle s(u_i),g\rangle.
\]

The current private construction hashes the four ordered source claims, relation identity, and all masks, then commits to that digest as \(D\). It absorbs \(D\) before the batching endoscalar \(\mu\). It absorbs

\[
C_*=\sum_{i=0}^{3}\mu^iG_i
\]

before deriving the evaluation point and \(\zeta\) (`src/generator_claim.rs:169–195`, `276–300`).

For fixed source discrepancies, the batch discrepancy is a degree-at-most-three polynomial in \(\mu\). Conditional on commitment/representation extraction:

1. Source batching contributes \(3\lambda_E\).
2. A differing coefficient vector’s multilinear evaluation contributes at most \(\ell\lambda_{\mathrm{raw}}\).
3. Evaluation separation contributes \(\lambda_E\).
4. The \(\ell\) IPA folds contribute \(2\ell\lambda_E\).

Therefore

\[
\boxed{
\epsilon_{\mathrm{generator}}
\le \ell\lambda_{\mathrm{raw}}+(4+2\ell)\lambda_E.
}
\]

The masks’ exact correction is essential:

\[
T_{j+1}=u_jT_j+u_j^2L_j+R_j,
\qquad
h_{j+1}=u_jh_j+u_j^2m_{2j+1}+m_{2j+2}.
\]

The final equation includes \(h_\ell H\). Native replay checks the digest opening and these equations (`src/generator_claim.rs:358–408`). Circuit counterparts are at `src/generator_claim_points.rs:170–273` and `src/generator_claim_scalar.rs:166–222`.

Source challenges and inherited coordinates are tied to the actual verified proof ports; the output becomes late IO (`src/generator_integration.rs:299–378`). Native decision recomputes the exact output MSM (`src/generator_claim.rs:235–243`).

Bootstrap instead exposes the exact constant claim \((g_0,(0,\ldots,0))\), for which \(s=(1,0,\ldots,0)\) (`src/generator_integration.rs:239–279`).

### 6.9 Compiler LogUp obligations

Spartan soundness proves the instantiated R1CS. It does not by itself establish that randomized copy and lookup constraints enforce their intended semantics.

The compiler compresses tuples as

\[
f_i(B)=\operatorname{tableid}_i+\sum_{j=1}^{d}B^jv_{ij},
\]

and uses inverse equations and a sum relation. These are implemented at `src/uniform.rs:1640–1675`, `2718–2775`, `2796–2855`.

A conditional bound can be obtained without a quadratic union over pairwise tuple collisions.

Assume all tuple values, masks, and multiplicities are fixed before phase-two challenges \(A,B\). Combine identical formal tuple polynomials. If the intended field-valued multiset relation is false, with a nonzero residue, then

\[
R(A,B)=\sum_i\frac{c_i}{A-f_i(B)}
\]

is a nonzero rational function over \(\mathbb F(B)\). Clearing its \(s\) distinct denominators gives a nonzero numerator of total degree at most \(d(s-1)\). Denominator-zero events occupy at most \(sH\) points of the \(H\times H\) challenge grid.

Using the joint challenge atom \(5/r\),

\[
\epsilon_{\mathrm{LogUp}}
\le[d(s-1)+s]\frac{5H}{r}.
\]

If \(s\le M\), where \(M\) bounds the number of constraint rows,

\[
\boxed{\epsilon_{\mathrm{LogUp}}\le(d+1)M\lambda_*.}
\]

For membership semantics, an absent queried tuple must have nonzero residue; bounded Boolean query multiplicities cannot sum to zero modulo the field when their total is below its characteristic. Width metadata alone is not proof of a range constraint.

The source places helper cells in phase two and ordinary multiplicities earlier (`src/uniform.rs:1522–1553`, `1607–1638`). However, a complete audit that every lookup tuple, copy mask, and multiplicity satisfies the required phase and semantic invariants is **UNVERIFIED**. The operation totals below are expressly conditional on this obligation.

## 7. Concrete bounds at the retained complete layout

### 7.1 Profile

The retained candidate has:

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

The configured profile is at `src/backend_chain_tests.rs:18–34`. Shapes are recorded at `output/s2/chain-20261117T051751Z.log:431`, `443`, `449`, `455`.

The eight retained public artifact headers were inspected, and their instruction tapes were independently checked with conservative formal-degree propagation. This verifies their declared challenge-degree bounds. It does not establish compiler semantics or equality to current HEAD.

The artifact schema and opcode meanings are at `src/program_artifact.rs:8–28`, `161–192`, `341–365`.

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

For example, the outer count is \(7\cdot26+2=184\). Early inner blocks use separate coordinates; later blocks use the Kronecker schedule. Alpha consumes a spare half, while gamma begins a fresh squeeze under this profile (`src/spartan.rs:1637–1659`).

The two-proof subtotal is approximately \(868.5/H\), corresponding to **116.2376 local algebraic bits**. It excludes accumulation, LogUp, linking, cryptographic failures, recursive extraction, and lifetime/query losses.

### 7.3 Accumulation and link ledger

The matrix degree bounds are

\[
D_P=3(2+24+25)+9+3(1+7)=186,
\]
\[
D_S=3(2+24+24)+9+3(1+4)=174.
\]

The generator bounds are \(58\lambda_*\) for P and \(55\lambda_*\) for S.

| Additional term | Bootstrap | Load | Send | Receive |
|---|---:|---:|---:|---:|
| P matrix fold | 0 | \(186\lambda_*\) | \(186\lambda_*\) | \(186\lambda_*\) |
| S matrix fold | 0 | \(174\lambda_*\) | \(174\lambda_*\) | \(174\lambda_*\) |
| P generator restructuring | 0 | \(58\lambda_*\) | \(58\lambda_*\) | \(58\lambda_*\) |
| S generator restructuring | 0 | \(55\lambda_*\) | \(55\lambda_*\) | \(55\lambda_*\) |
| Pair link | \(1/p\) | \(783/p\) | \(783/p\) | \(1539/p\) |
| Exact native decisions | 0 | 0 | 0 | 0 |

The link lengths follow the source’s shared-value layout:

- One parent pair contributes 139 verification words, two kind bits, and four table limbs: 145 words.
- Private generator layouts contribute \(39+16\ell\) words per side, hence \(327+311=638\).
- Send/Load use \(145+638=783\).
- Receive adds two parent contributions, 601 words for admitted incoming pair bytes, and ten verdict words: \(290+638+601+10=1539\).
- Bootstrap uses one shared constant.

Sources: `src/verifier.rs:2046–2100`; `src/generator_claim.rs:71–95`; `src/full.rs:3783–3798`, `4209–4275`, `5369–5383`, `5646–5649`; `src/proof_bytes.rs:47–55`.

### 7.4 Conditional total per operation

For a compact conservative ledger, replace all raw and endoscalar atom bounds by \(\lambda_*=5/(4H)\). Then the main-proof coefficient is \(442+416=858\); non-Bootstrap accumulation adds 473.

The retained artifacts give the following row counts and LogUp degree bounds:

| Kind | P rows \(M_P\) | S rows \(M_S\) | \(d_P\) | \(d_S\) |
|---|---:|---:|---:|---:|
| Bootstrap | 414,240 | 163 | 7 | 1 |
| Load | 1,398,002 | 1,281,024 | 7 | 4 |
| Send | 1,670,642 | 1,281,024 | 7 | 4 |
| Receive | 2,773,423 | 2,334,464 | 7 | 4 |

These are fields of the retained `research-kind-*.norito` artifacts, interpreted according to `src/program_artifact.rs:176–192`.

Define

\[
E_{\mathrm{full}}=
\frac{2(2^{22}-1)}q+\frac{2^{22}-1}p.
\]

Under all local hypotheses, including §6.9,

\[
\boxed{\epsilon_k\le C_k\lambda_*+E_{\mathrm{full}}+\frac{n_k}{p}.}
\]

| Kind | Main proofs | Matrix folds | Generator folds | LogUp upper coefficient | \(C_k\) | Local algebraic bits, approximately |
|---|---:|---:|---:|---:|---:|---:|
| Bootstrap | 858 | 0 | 0 | 3,314,246 | **3,315,104** | **104.01** |
| Load | 858 | 360 | 113 | 17,589,136 | **17,590,467** | **101.60** |
| Send | 858 | 360 | 113 | 19,770,256 | **19,771,587** | **101.44** |
| Receive | 858 | 360 | 113 | 33,859,704 | **33,861,035** | **100.66** |

The LogUp column uses \((d_P+1)M_P+(d_S+1)M_S\). It is a conservative upper bound, not an exact count of distinct denominators. Bits are rounded down; the displayed probability formula is authoritative.

Without LogUp, the corresponding conservative subtotals would be about 115.93 bits for Bootstrap and 115.29 bits for each other kind. Those are **not complete operation bounds**.

These numbers are incremental node costs. For an extracted Receive DAG, the costs of both parent histories must also be included. A shared ancestor is charged once; no assumption of independent parent histories is needed.

**Established overall security bits for current Path 2: UNVERIFIED for every kind.** The table is conditional algebraic accounting for a retained profile. It is not a numerical bound on P2-RKFS, discrete logarithms, signatures, hashes, compiler defects, or custody failures.

## 8. Finite lifetime and query accounting

Let \(\mathcal V\) be the distinct nodes of the extracted history. For a fixed idealized execution without additional grinding, the algebraic contribution is bounded by

\[
\sum_{v\in\mathcal V}\epsilon_{\operatorname{kind}(v)}.
\]

Under P2-RKFS’s adaptive-query charging clause, a conservative overall form is

\[
\begin{aligned}
\operatorname{Adv}_{\mathrm{invalid}}
\le{}&
\epsilon_{\mathrm{RKFS}}
+\epsilon_{\mathrm{binding}}
+\epsilon_{\mathrm{application\ hashes}}
+\epsilon_{\mathrm{signatures}}\\
&+(Q+N)\max_k\epsilon_k .
\end{aligned}
\]

The binding term includes both curves and authenticated commitment-generator relations. Cryptographic reductions and their running-time losses must be evaluated at the extractor’s running time, not silently at the original adversary’s running time.

For an explicit accounting example, take:

| Budget | Example value |
|---|---:|
| Operations per wallet generation | \(L=2^{20}\) |
| Wallets whose histories may join | \(W=2^{16}\) |
| Distinct extracted nodes, including relevant forks | \(N\le2^{36}\) |
| Depth | \(D\le N\) |
| Adversarial hash/permutation queries | \(Q\le2^{40}\) |

These are **illustrative assumptions, not installed or approved limits**. In particular, a selected wallet counter does not bound adversarial forks or off-device grinding.

Using the conservative Receive coefficient, the algebraic contribution is below approximately

\[
2^{-60.57},
\]

before adding the unquantified cryptographic and recursive-assumption terms. Raising the query budget to \(2^{64}\) reduces this algebraic estimate to roughly 36.66 bits.

This is why neither 255-bit fields nor 126-bit challenges justify an overall “128-bit soundness” label. A production security target must select and justify finite budgets, discharge or explicitly accept the recursive assumption, and reassess the challenge schedule if the resulting bound is insufficient.

No bound remains meaningful over infinitely many independent attempts.

## 9. Implementation discrepancies and unresolved obligations

The table records discrepancies identified during this review. It is not a claim that a manual audit has found every possible defect.

| Finding | Evidence | Required treatment |
|---|---|---|
| **Recursive extraction theorem absent** | Prefix timing is security-critical in `src/spartan.rs:1551–1659`; gamma aggregation is at `1674–1715`. | Accept P2-RKFS explicitly with parameters, or supply a checked composition/extraction theorem. Gamma batching alone is insufficient. |
| **No rank on incoming-payer edges** | `src/receive_originals.rs:424–439`; own increments at `src/receive.rs:331–336`. | Retain an explicit causal-extraction assumption or add an authenticated global rank. |
| **No enforced finite lifetime/query profile** | Implemented sequence bound is unsigned128 (`src/monetary_state.rs:91–93`). | Specify deployment lifetime and adversary-query assumptions. Resource limits are not cryptographic lifetime bounds. |
| **Imported polynomial degrees are trusted metadata** | Degree caps at `src/program_artifact.rs:274–283`; arbitrary multiplication tapes at `341–365`; declared degrees installed at `404–408`. | Authenticate correct compiler output and degree certificates, or verify degree bounds on import. Repeated multiplication can exceed a supplied declaration. The retained eight tapes were checked; general import correctness is not established. |
| **Generator/table profile is not explicitly named by the manifest fields** | `src/engine.rs:49–59`, hardcoded reconstruction at `197–198`; generator domain at `src/ipa.rs:24–26`. | Bind namespace, hash-to-curve, capacities, prefix behavior, table derivation, and column order through an authenticated implementation/profile identity. |
| **LogUp phase/semantic invariants incompletely audited** | `src/uniform.rs:1522–1553`, `1607–1638`, `2718–2775`. | Verify every query tuple, mask, and multiplicity is appropriately fixed before the challenge, and that copy/lookup semantics follow. Until then, complete-operation numerical totals remain conditional. |
| **Transcript tags are ignored** | `src/fs.rs:26`, `74–105`, `138`, `152`, `157`. | Describe security as depending on the exact typed positional schedule and authenticated profile. Do not credit ignored textual tags with domain separation. |
| **Low-level clear claim folding does not absorb challenge vectors locally** | `src/ipa.rs:262–285` absorbs claim points but not `claim.us`. | Do not treat this API as independently binding arbitrary source tuples. The current private construction separately binds ordered sources (`src/generator_claim.rs:169–195`); any switch to clear folding needs its own argument. |
| **Some private scalar limbs have field, not unique-byte, semantics** | `src/generator_claim_scalar.rs:94–97`, `166–179`. | These limbs are recomposed modulo the scalar field without a separate original-modulus comparison. Define their private semantics as field elements or add canonical checks before claiming unique encodings. No acceptance exploit is established by this observation alone. |
| **Census checks are not actual uniform constraints** | `src/verifier.rs:245–257`, `362–416`; actual checks at `src/points.rs:1624–1786`. | Audit the actual builders; do not use counted rows as constraint evidence. |
| **Native burn comments are stale** | `src/receive.rs:42` versus `src/engine.rs:780–834`, `1058–1071`. | Describe current bounded admission and derived burn behavior. Complete containment/recovery qualification remains **UNVERIFIED**. |
| **Several degree/distribution comments remain wrong** | `src/deferral.rs:32`, `src/full.rs:4539`, `src/verifier.rs:2115`. | Replace `/|F|` with the appropriate challenge atom; replace pair-link \(n/q\) with \(n/p\) in the ideal-word model. |
| **Three-kind descriptions remain stale** | `src/full.rs:4544`; `research/CONSTRUCTION.md:60–64`. | Describe all four kinds and derive Bootstrap emptiness from zero \(r\), not an unused selector corner. |
| **Fresh pair-mask description has a Bootstrap exception** | `src/full.rs:5668`, `5713`. | State the deterministic Bootstrap mask accurately. This does not invalidate the fixed-before-challenge soundness lemma. |
| **Retained profile is not established equal to current HEAD** | Retained artifacts and `output/s2/chain-20261117T051751Z.log:423–455`. | Freeze and identify the reviewed candidate before using its numerical ledger as release evidence. |
| **Compiler equivalence and all-boundary acceptance are not completely verified** | Program construction/import and native accepting paths cited above. | Require exact source/program correspondence and adversarial coverage of every accepting boundary. |

Several previously reported gaps should **not** be repeated as current missing checks:

- Native matrix and inherited-generator decisions exist (`src/engine.rs:451–487`).
- Own IPA generator decisions exist (`src/ipa.rs:469–473`).
- Native program-digest pinning exists (`src/engine.rs:179–184`, `1393–1396`).
- Four-kind selection includes the fourth coefficient and Boolean selectors (`src/points.rs:1058–1124`).
- Canonical proof/scalar parsing and carrier re-encoding exist (`src/spartan.rs:88–143`; `src/carrier.rs:286–331`).
- Native table recomputation exists (`src/engine.rs:375–381`). A correctly issuer-signed wrong-table regression exists at `src/bootstrap.rs:1090–1129`; execution against the present candidate is **UNVERIFIED**.
- Newer compiler tests compare all eight program digests (`src/backend_chain_tests.rs:85–91`, `924–975`). The retained log records equality at `output/s2/chain-20261117T051751Z.log:423–430`. This is stronger than the older sampled matrix-evaluation evidence.

No prematurely derived challenge was found in the reviewed main committed-round, pair-link, matrix-fold, or private-generator schedules. That conclusion is narrower than a complete transcript/constraint-equivalence audit.

## 10. Required changes to DESIGN.md

### 10.1 Section 5: one proof

Replace the abbreviated soundness description with the following substance:

> Each committed sumcheck block has individual degree at most two. With the current three-variable Kronecker schedule, its univariate error polynomial has degree at most 26. Initial equality testing has degree at most \(2^{s_x}-1\). The matrix combination uses a degree-three test. Challenge bounds use the actual truncated-word or endoscalar distribution, including correlation between halves of one squeeze.
>
> Gamma separation gives a degree-\(2m\) Laurent-polynomial root bound, conditional on consistent commitment-opening extraction. Alpha batches \(m+2\) equations with degree at most \(m+1\). These local facts do not by themselves establish prefix knowledge or recursive Fiat–Shamir security.
>
> Operation-level accounting also includes compiler LogUp, matrix and generator accumulation, linking, and finite-query losses.

Keep the exact commitment order. Explain the weighted outer recurrence directly; do not imply division by an equality-factor evaluation.

### 10.2 Section 7.3: deferred matrix claims

State:

> A step commits to a bounded-degree polynomial \(q\) before the raw phase-two challenge \(\beta\). The four input equalities make any false input imply a nonzero polynomial \(q-P\circ\Gamma\). One fold therefore has error at most \(D_M\lambda_\beta\), with no factor four. Native decision exactly evaluates the authenticated combined polynomial.
>
> For the retained complete `p18-s17` profile, the conservative degree bounds are 186 on P and 174 on S. The older degree 165 belongs to a different layout. Imported degree declarations require authenticated correctness or independent verification.

Clarify that Bootstrap’s claim is zero because its matrix-combination coordinate is zero. Load occupies \((1,1)\).

### 10.3 Section 7.2: necessary related correction

Replace “\(\rho\) is a 126-bit challenge” with:

> The pair-link challenge is the full canonical Fp word squeezed by S’s transcript and embedded injectively into Fq. For a fixed differing length-\(n\) coefficient vector, the ideal-word collision bound is \(n/p\). Both the shared values and mask must be committed before this challenge.

### 10.4 Section 11: security

State prominently:

> Path 2 has conditional local algebraic arguments and an explicit additional recursive knowledge/Fiat–Shamir assumption. No DL-only theorem or quantitative concrete-RP57 recursive reduction is established. Receive joins histories into a DAG; own-wallet sequence monotonicity does not rank incoming-payer edges.
>
> Native acceptance decides both matrix accumulators and all own/inherited generator obligations. The retained-layout algebraic bounds are conditional on compiler and phase invariants and must be multiplied by a finite history/query budget. They are not an overall 128-bit security claim.
>
> Anti-fork spending depends on custody and trusted wallet execution. Load finality is native. Bootstrap does not prove globally unique enrollment. Joint recursive zero knowledge and production custody qualification remain separate obligations.

Update stale native-burn and sampled-only fixed-point statements using source-bound evidence, without representing existing partial logs as completed release qualification.

## 11. Conditions for independent acceptance

G3 should accept this document only after resolving or explicitly accepting:

1. The exact P2-RKFS assumption, especially prefix knowledge, causal closure, and extraction cost independent of exponential depth.
2. A frozen candidate/profile and the relationship between authenticated programs, generators, compiler semantics, and declared degrees.
3. The LogUp phase and semantic invariants required by the operation totals.
4. Exact agreement of native and circuit acceptance, including canonical decoding, incoming corrections, burn, and all accumulator decisions.
5. A finite lifetime/query budget and numerical target that includes cryptographic and recursive-assumption terms.
6. The separately qualified custody and native-finality boundaries.

Until then, the supported conclusion is a **conditional security argument with quantified local algebraic losses**, not approval for wallets to use Path 2.