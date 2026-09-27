# In-flight first-release Apalache VC diagnostic — 2026-09-23

Correction: [the runtime VC mapping follow-up](multilane-apalache-runtime-vc-mapping-correction.md)
shows that VCGen emission order does not equal Apalache's runtime query order.
Use that follow-up for the source expressions behind runtime VCs 10, 33 and 43.

The 18-step in-flight Apalache gate is **open**. This inspection and a private
encoding experiment used only the `optimizations` checkout. The canonical
model and fixed config were not changed and retain SHA-256
`f4d30d3227a5294338ff8af3a4abb2ff75a46bd118a682e05689f396ded5a7f7`
and `8174cb1d329c40dbc9eb34456d85953f1446739c48ff5828e29eb123c6e85128`,
respectively, matching the prior exact attempt. The pinned runner still checks
all 20 named invariants at length 18 with incremental search and `after`
invariant scheduling.

The earlier exact run ended with exit 143 after 411,980.3 seconds, having
completed only state 15, invariant 9. It never reported `NoError` or
`EXITCODE: OK`. Its receipt and raw log are at
`dist/multilane-validation-20260907/scaling-apalache18-after-join-20260916/`;
the indexed diagnostic packet is
`target/first-release-apalache-18-diagnostic-20260923/diagnostic.json`.

The pinned 0.52.2 `VCGen` log reports 77 internal verification conditions from
the 20 named invariants. Numbering those conditions in the logged emission
order and following the conjunctions in
`formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla` identifies the three
recurrent slow conditions:

| Internal condition | Source condition | Prior 60-second-capped diagnostic |
| --- | --- | --- |
| 10 | `FirstReleaseTypeInvariant`: `session` record type | 1.44 s at state 7, 5.39 s at 8, 34.92 s at 9, then TIMEOUT at 10–12 |
| 33 | `MLCommitAndReleaseRetainExactScope`: lane-commit scope equals `BindingA` when present | 3.44 s at state 9, 9.42 s at 10, then TIMEOUT at 11–12 |
| 43 | `MLPostCarrierCommitCleanupOrder`: the third canonical commit/tombstone/forget key-prefix condition | TIMEOUT at state 12 |

Those timings were parsed from
`target/first-release-apalache-quantified-20260920-02/check18.out/SumeragiV2InFlightFirstRelease.tla/*/detailed.log`.
The earlier uncapped run demonstrates the cost of continuing the same encoding:
condition 10 completed state 10 at 16:16:10, state 11 at 16:38:59, state 12 at
18:17:04, state 13 at 04:12:05, and state 14 at 12:11:14 in its raw log; state
15 never completed condition 10. The rapid growth occurs across structurally
different properties. The capped diagnostics returned `TIMEOUT` and assumed
those conditions, so they are troubleshooting data, not valid safety evidence.

The model has 30 `Next` transitions, including two explicitly named stutters.
`RecoverReservationSnapshot` and `RepairPostCarrierEvidence` are bound to
production source seams; deleting them merely to reduce the solver formula
would require a revised source-correspondence argument. Replacing the dynamic
validator domain alone was already tried and timed out on the same conditions.
The experimental arrays encoding failed at step-seven enabledness. No
equivalence-backed finite-state re-encoding has yet been identified, and the
same-input multi-day rerun is not a completion strategy.

The retained `08_OutVCGen.json` shows condition 10 as
`SET_IN(session, RECORD_SET(...))` with three `SET_POWERSET(Validators)`
operands for the volatile sets. Condition 33 is the simple lane-commit-scope
implication; its late cost may come from the accumulated transition context,
not that predicate alone. Both retained `log0.smt` files only say SMT logging
was disabled, so they did not isolate solver clauses. A candidate rewrite of
the session type check needs exact record-domain equality plus componentwise
subset/Boolean checks to preserve the predicate; field checks alone would
weaken it. The pinned checker supports `--debug` for `log.smt` and `--smtprof`
for `profile.csv`.

## Bounded private session-type experiment

At HEAD `b45ec0457eb0664e46c2cdfd8a4b619caefd02b9` on `optimizations`,
after the external merge cleared, a private copy under
`target/first-release-apalache-session-type-20260923/inputs/` replaced only
the `session` record-set membership conjunct of `FirstReleaseTypeInvariant`
with exact `DOMAIN session` equality and four field membership/subset checks.
The private model SHA-256 is
`8687dfddc1f50a34e5bd259cf498ce1e3f4dd08a985064240c91d674084eff05`;
the fixed config and canonical model retain the hashes above. This is a
mathematical equivalence of record typing, including exact fields. `Init`, all
30 `Next` arms, the other 19 named invariants, the 18-step bound and the
`after` schedule were unchanged. The private experiment does not amend the
canonical release model. The merge commit reports signature status `N`, so
this HEAD is not a signed release candidate.

Pinned Apalache 0.52.2 typechecking passed. TLC on the private copy and fixed
config returned 0 with its no-error marker in 19.77 seconds. All 25 configured
mutation runs returned status 12 with their expected invariant marker and
counterexample trace. Exact commands and results are in
`target/first-release-apalache-session-type-20260923/tlc/results.json` and
`tlc/full_mutants/results.json`; the equivalence argument is in
`equivalence.md`. These are useful correspondence checks, not a proof of the
18-step Apalache obligation.

The bounded Apalache command is recorded verbatim in
`target/first-release-apalache-session-type-20260923/apalache-command.json`:
incremental `check`, 18 steps, all 20 fixed-config invariants, `after`
scheduling, `--timeout-smt=60`, `--debug`, `--smtprof`, and soft/hard CPU
limits of 600/610 seconds. The pinned launcher and JAR SHA-256 were
`bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7`
and `1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a`.
Its result at 2026-09-23 09:09:27 UTC was **failure**: process return code
`-24` (CPU limit), 586.25 seconds elapsed, six per-query `TIMEOUT` conditions,
and only state 13 invariant 9 completed. It had no `NoError`, full step 18,
or `EXITCODE: OK`. The exact result and stdout SHA-256
`8f2a088786eda69ef2c0d07260d0ec921c8bdf3c1365fce7e3d9e41a3772a347`
are in `apalache-result.json`.

The rewrite expanded `FirstReleaseTypeInvariant` from 14 to 18 internal
conditions and all named invariants from 77 to 81. The candidate's internal
condition 10 still timed out at states 10, 11, and 12; condition 35 and 47
also timed out at state 12, and condition 5 at state 13. Apalache assumed each
timed-out condition, so its later checks cannot establish safety. `--debug`
captured a 13,672,268-byte `log0.smt` (SHA-256
`6c79830dafe522f5d802cc348bfacedcdc7cb8457ebbb13e39f680043d1ee270`)
with 1,307 `check-sat` commands; the final incomplete query had 4,123 arena
cells. The 118,482-byte `profile.csv` (SHA-256
`2c1879cf8969e06d5f3375d1c86ee1da76f215ee9b54da958d33105df57cb70f`)
places its highest weights in model spans 711–722 and 676–687, which cover
authenticated carrier ownership and history typing, not solely the rewritten
`session` check. The
conjunctive type rewrite therefore did not relieve the growing incremental
solver context and is not suitable for canonical integration.

The next diagnosis should map the captured condition-10 query and the
candidate's condition-35/47 queries through a retained candidate `08_OutVCGen`
and compare their assertion
dependency slices and accumulated arena growth at states 9–13. A proposed
finite-state encoding or decomposition must first establish equivalence to the
full fixed `Next` relation and all 20 named invariants, then repeat TLC/mutation
correspondence before the complete pinned 18-step `after` gate. A longer run
of this candidate would not repair the observed timeouts. No current result
closes that gate.
