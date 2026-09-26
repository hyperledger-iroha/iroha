# In-flight Apalache runtime VC mapping correction — 2026-09-23

The pinned 18-step in-flight first-release Apalache gate remains **open**.
This record corrects the source mapping in
`multilane-apalache-vc-diagnostic.md`. That earlier record counted verification
conditions in VCGen's emission order. Apalache optimizes and reorders them
before the incremental checker assigns the `state invariant N` numbers printed
in `detailed.log`. The emitted number is therefore not a source locator for
the runtime query.

## Retained evidence and exact query mapping

The unchanged canonical module and fixed config have SHA-256 hashes
`f4d30d3227a5294338ff8af3a4abb2ff75a46bd118a682e05689f396ded5a7f7`
and `8174cb1d329c40dbc9eb34456d85953f1446739c48ff5828e29eb123c6e85128`.
A new pinned Apalache 0.52.2 `--debug` check with length 1, incremental
algorithm, `after` invariant scheduling, all 20 configured invariants, and
30-second per-query timeout returned `NoError` and `EXITCODE: OK` in 4.1
seconds. Its retained output is
`target/first-release-apalache-vc-map-20260923/`. At state 1, the 68
`Checking state invariant` entries match the 68 consecutive `log0.smt`
`check-sat` calls numbered 107–174. The source text in the respective query
gives the corrected mapping:

| Runtime VC | SMT query | Exact source expression | Syntactic solver dependencies |
| --- | ---: | --- | --- |
| 10 | 115 | `session.bodies \cap session.crashed = {}` in `MLVolatileSessionLostOnCrash` | Set filter/intersection, equality to empty set, `session.bodies`, `session.crashed` |
| 33 | 136 | Full `session \in [bodies: SUBSET Validators, readyAuthorized: SUBSET Validators, crashed: SUBSET Validators, producerAlive: BOOLEAN]` in `FirstReleaseTypeInvariant` | Exact record fields, three validator powersets, all four session fields |
| 43 | 145 | `decision.releaseOwner \in authenticated` where `authenticated == {p \in Validators: payloadBinding[p] = BindingA}` in `MLValidatorCarrierOwnership` | Filtered validator set, `payloadBinding` entries, `decision.releaseOwner` |

The independently retained private session-type experiment has 81 emitted
conditions. At state 12 its 71 runtime checks match consecutive SMT queries
1200–1270. The previously reported private timeouts map as follows:

| Runtime VC | SMT query and line span | Actual predicate |
| --- | --- | --- |
| 10 | 1208, lines 240941–241364 | Same volatile body/crash disjointness predicate |
| 35 | 1231, lines 243056–243285 | `session.bodies \subseteq Validators`, split from the record type check |
| 47 | 1241, lines 246431–247740 | Authenticated `decision.releaseOwner` membership |

These are syntactic dependency slices of the generated queries, not minimal
unsatisfiable cores. `session.bodies` can change in Init, producer fanout, late
body service, Crash, and local Kura rehydration; `session.crashed` changes in
Crash and Recover. The authenticated-owner query depends on payload binding
from Init and ActivateKura (or the non-fixed conflict mutation) and release
ownership from PersistKuraRetirement. Every check also carries the accumulated
12-step incremental `Next` context. In the private state-12 log, the volatile
and release-owner queries span 424 and 1,310 lines, respectively. The profile
weights on carrier ownership and history typing confirm that a local rewrite
cannot be assumed to solve the full context-growth problem.

## Exact but unsuccessful encoding experiments

Two bounded canonical-source experiments kept Init, all 30 `Next` arms,
all 20 named invariants, the fixed config, and `after` scheduling. They used
the elementary identities
`A \cap B = {} <=> (\A p \in A: p \notin B)` and
`x \in {p \in V: P(p)} <=> (\E p \in V: x = p /\ P(p))`.
The second experiment exchanged `A` and `B` to quantify over the crash set.
These rewrites preserve each predicate on every state where its original
expression is defined; they do not rely on a strengthening assumption or drop
a model behavior. The first rewrite passed the complete fixed TLC state graph
(280,818 distinct states, depth 36, no error) and all 25 mutation configs,
each with its expected counterexample. Its retained TLC artifacts are under
`target/sumeragi-v2-tlc-_kz9jayt/`.

The first 11-step Apalache diagnostic, with a 30-second SMT-query cap and
debug/profiling output under
`target/first-release-apalache-disjointness-20260923/`, timed out on runtime VC
10 at states 9, 10, and 11 and on VC 33 at state 11. Its outcome was
`SmtTimeout`, despite a process exit code 0 and an `EXITCODE: OK` footer;
only `NoError` establishes the required check. The second diagnostic under
`target/first-release-apalache-crash-domain-20260923/` passed VC 10 at state 9
but timed out on that VC at states 10 and 11; VC 33 timed out at state 11.
Neither rewrite makes the fixed 18-step gate tractable. The canonical source
was restored exactly to the hash above; no experiment is retained in the
release model.

A structural candidate would encode every validator-indexed set as a fixed
characteristic function over `Validators`, then prove a state bijection and
commutation of Init, every `Next` arm, and all 20 invariants under decoding.
That proof must include the mutation modes, crash/recovery, authenticated
payload ownership, and ReadyQuorum cardinality. Only after the equivalence
proof and TLC/mutation correspondence should its complete 18-step pinned
`after` check run. No such encoding or proof is claimed here.
