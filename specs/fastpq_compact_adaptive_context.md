# Adaptive-context extension of the typed whole-tape compiler

2026-09-06. Internally reviewed conditional extension. This extends [the typed compiler](fastpq_compact_typed_compiler.md); its reviewed source artifact had SHA-256 `b37a7632017c633167543482c819e17faee5ea2ad43c453010dd347c6686db9f`. It does not change FASTPQ source, query profiles, cryptographic primitives or admission behavior.

**Claim.** The candidate bound can cover an adversary that chooses its final public context after arbitrary counted oracle queries, without multiplying the bound by the number of possible contexts, under the explicit family-wide premises below. The proof uses one global database property from the start. It is not a union bound over separately proved fixed-context security statements.

## 1. Exact additional premises

Fix a finite, canonically encoded context universe `Ctx` before sampling the ideal functions. Finiteness follows from fixed input-size limits; the universe can be extremely large. Let `Admissible(c)` and `False(c)` be deterministic, oracle-independent predicates. Each admissible c determines an immutable formal IOP instance x(c), and a deterministic state function `state_c` satisfying the same empty/prover/full-transcript clauses as in the original candidate. For all admissible false contexts and every good fixed prefix, the next whole verifier move j must have bad-transition probability at most one common epsilon_j. The state may depend on every bit of c.

This extension holds within one fixed compiler profile: common k, tape types and lengths R_j, output alphabets, input/resource caps and typed Merkle conventions. The word and statement encodings are fixed, not selected after a tape is obtained. Extending simultaneously across profiles with different types or alphabets needs the corresponding explicit supremum and query model; it is not asserted here.

Each valid G_j input explicitly contains the **complete canonical c** and the short predecessor state: `(c,sigma)`. Each relevant H chain, parent and leaf input explicitly includes that same complete c together with its required role, round, level/position and data fields. The encoding is injective across all these roles. The extractor checks exact context equality on every inverse it follows. Context bytes are data, not implicit hash pointers.

The initial anchor is fixed or a deterministic oracle-independent function of c. The candidate's common fixed anchor is sufficient. An oracle-derived anchor adds a dependency and is outside this statement.

Use independent uniform ideal functions H and G_j over all these contexts, with the same global output alphabets as the original candidate. Invalid-but-queryable encodings may receive uniform outputs but cannot become proper endpoints or valid extracted nodes. All oracle-dependent preprocessing and retries count toward T; initial advice is independent of the sampled functions. The final proof and its selected context are classical output; the adversary may query in superposition across contexts before producing them.

A verifier expansion for `(c,proof)` must carry all required typed oracle input/output constraints, the full raw G tapes, all chain links and all authenticated reads, and must reject if c is inadmissible. Its ordinary checks and the false-instance winning predicate are oracle-independent once the full expansion is given. K_weight is maximized over every successful false-context expansion allowed by the common resource policy.

## 2. One global database property

Keep the original global collision property B: repeated H output at distinct H inputs, or repeated G_j output at distinct inputs of the same j, including different contexts. Collisions between separate G types are not compared.

For a collision-free D, define `P_(c,j,sigma)(D)` exactly when:

1. c is admissible and false;
2. the typed entry `D_j(c,sigma)=tau` is present;
3. `tr=E(D,c,j−1,sigma)` is an anchored extraction;
4. `state_c(tr)=0`, tau decodes successfully, and `state_c(tr || Decode_(c,j)(tau))=1`.

Define

```
P_all(D) = existence of (c,j,sigma) satisfying those clauses;
R_all = B union P_all.
```

Only entries already in D_j can supply endpoints. Therefore a fixed database has at most `sum_j |D_j|` candidate endpoint tuples, regardless of `|Ctx|`. The predicates can be inefficient; they only define a mathematical database projector. Empty D is outside R_all.

Define S_H(D) and S_j(D) using parsed inputs **across all contexts**. The same cardinalities hold:

```
|S_H(D)| <= 2*n_H+n_G,
|im(D_H)| <= n_H,
|S_j(D)|+|im(D_j)| <= #H_chain_j+|D_j| <= |D|.
```

Adding raw context bytes to inputs adds no inverse edge: the extractor compares those bytes and never inverts each short-looking substring within them. In particular, these counts do not multiply by the number or length of contexts.

## 3. H insertion, both directions

Fix any old D and any undefined H input x. Suppose D and `D'=D+[x->y]` are collision-free and R_all membership differs.

H insertion changes no G entry. If it creates a P_all witness, that witness `(c,j,sigma)` already had its G endpoint in D. If it destroys P_all, choose one old witness; its endpoint remains present in D'. Admissibility, formal falsity, the state definition and that endpoint's stored tape cannot change with H insertion. The membership flip therefore requires changed extraction at `(c,j−1,sigma)`.

Apply the candidate's typed stability lemma for that particular c. The new output is in `S_H(D) union {sigma}`. Because sigma is already a parsed predecessor field in the old G endpoint, it belongs to S_H(D). Taking the possible witness over all contexts does not enlarge this global set. New collisions require `y in im(D_H)`; old collisions cannot disappear. Either flip direction therefore has probability at most

```
(|S_H(D)|+|im(D_H)|)/p^6 <= 3*|D|/p^6.
```

No assertion that c was fixed before the adversary's interaction is used.

## 4. G_j insertion, both directions

Fix an undefined typed input `x=(c_star,sigma_star)` to G_j. Its full c_star is now fixed as part of x before sampling the new tape tau, even if an attacker selected x after inspecting all prior queries.

For any old endpoint witness, the endpoint's stored tape is unchanged by insertion at another input. Its context predicates are also unchanged. Any destruction or creation through that old endpoint therefore requires extraction to change, hence `tau in S_j(D)`. A collision requires `tau in im(D_j)`.

Outside those sets, only the newly inserted endpoint itself could create P_all. If c_star is inadmissible or true, or its preceding extraction fails or is already bad, it cannot do so. Otherwise,

```
tr_star = E(D,c_star,j−1,sigma_star)
```

is fixed independently of tau. Strict round typing means that it uses only G rounds below j, so it is equal to its extraction in D' even though the new query is at the same context. The uniform-in-c every-prefix state hypothesis bounds the bad raw-tape set by `epsilon_j*2^R_j`. Successful-decoder conditioning and permanent abort have the same treatment as in the original proof.

Thus both membership directions are bounded by

```
epsilon_j + (|S_j(D)|+|im(D_j)|)/2^R_j
 <= epsilon_j + |D|/2^R_j.
```

A destructive flip does not need the epsilon term. Wrong-context or malformed inputs can still collide with images, but they cannot be the new proper endpoint.

This proves the same two-sided delta(T) for the single R_all property. The supremum over all undefined x already permits adaptively selected contexts; applying a separate context union bound here would count the same possibility again.

## 5. Quantum lifting and final adaptive context selection

Use the finite-abelian one-cell proof unchanged, but replace R by R_all everywhere. For each query-input block x (which contains its context), outside database E and character, set `b=R_all(E)` and let A be the output values flipping that property. Sections 3–4 bound `|A|/s` uniformly. The same row/column calculation gives crossing norm at most `sqrt(6 delta(T))` for that block. The full query is a direct sum over x, including coherent superpositions across contexts, so its norm is the maximum of block norms, not their sum.

R_all is fixed before oracle interaction and depends only on the database. Arbitrary adversary operations that compute or select the eventual output context commute with its projector. The projected-state recurrence therefore still yields

```
Pr[measured D in R_all] <= min(1,6*T^2*delta(T)).
```

For the acceptance comparison, include c in the classical output label w along with the proof expansion. Winning labels are precisely admissible contexts with False(c) and an accepting expansion. Each such w determines a selected-cell projector Q_w. The same `Q_w J_w Q_w = product_i(1−1/s_i) Q_w` calculation applies. Classical w labels, including distinct contexts, are orthogonal. Taking the maximum K_weight across them gives the same weighted error term without any context multiplicity factor.

If a winning expansion is present in a collision-free D, extract the transcript using its selected c. Exact typed context checks make all actually read values consistent with the expansion. Its full transcript accepts, so its state is one; its first zero-to-one verifier move supplies an endpoint already in D and witnesses P_all. Consequently the final bound is

```
Pr[adversary outputs an admissible false context accepted by the verifier]
 <= min(1, (sqrt(6*T^2*delta(T)) + sqrt(2*K_weight))^2),

delta(T) = min(1, max {3*(T−1)/p^6,
                       max_j [epsilon_j+(T−1)/2^R_j]}).
```

The original two-group-query simulation per canonical binary XOR query remains valid. Both adversary queries and the selected context's verifier expansion queries count. This addresses adaptive context selection directly, rather than using the earlier fixed-context theorem as though its quantifiers already did so.

## 6. What this extension does not permit

- **A context digest substituted silently for c.** A many-to-one projection can allow an attacker to choose a context after seeing a response shared by several contexts. A generic example is a one-message family indexed by c in an s-element alphabet, all instances false, where acceptance is `message=c`. Every fixed instance has error 1/s. If the G input omits c and all contexts use one shared anchor query, the attacker queries once and chooses c equal to that response, winning with probability one. The endpoint premise has then failed. This is a counterexample to dropping the context-binding premise, not an attack on the candidate or current FASTPQ. A cryptographic context commitment may be usable, but its input/output binding and any additional extraction pointers/collision terms must be proved separately.
- **Oracle-dependent or mutable context truth.** Admissibility and formal falsity cannot change merely because a new oracle entry was inserted. A changing external ledger state must be represented by enough immutable authenticated context to make the intended predicate fixed; this argument supplies no such authentication.
- **A nonuniform hidden profile change.** All epsilon_j, tape lengths, domains and resource maxima must cover the admissible family. An attacker-selected profile with another state error is outside the common bound.
- **Free oracle-dependent advice or ignored attempts.** Earlier queries used to select a context still count. There is no separate context-count factor, but there is still the adversary's total query cost and the T-squared/T-cubed losses.
- **Reinterpreting 54 release targets.** This extension does not change the independently recorded 54-target accounting. It neither identifies those targets with contexts nor removes any separate release requirement.
- **Concrete instantiation or implementation qualification.** The current FASTPQ transcript, context commitment mapping, pair-leaf/shared-multiproof verifier expansion, SHAKE and frozen six-lane digest have not been qualified by this derivation.

## 7. Independent controls

`../scripts/fastpq/check_compact_adaptive_context.py` extends the independent extractor test model to two false contexts and one true context in one shared database. Every H/G input carries its exact context; H outputs are globally distinct in the base data, and G outputs are distinct within each type. It checks wrong-context extraction, true-context exclusion from P_all, both insertion exceptional sets and the fresh endpoint's own bad-transition alternative.

Result: PASS over 547 database subsets and 527,060 insertions, observing 2,825 extraction changes and 79,087 property flips. The JSON records the script hash. This finite search can expose a mistake in that test model; it does not prove the universal extension. The mathematical argument above received separate internal review; this does not supply external qualification or an implementation mapping.

Run `python3 scripts/fastpq/check_compact_adaptive_context.py` from the
repository root (`--output PATH` is optional). The deterministic finite model
requires no retained target artifacts or network access. Its certificate records
full checker and specification hashes; these identify the reviewed inputs and
are not a proof of source equivalence.
