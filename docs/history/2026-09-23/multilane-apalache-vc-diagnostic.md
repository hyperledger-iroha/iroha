# In-flight first-release Apalache VC diagnostic — 2026-09-23

The 18-step in-flight Apalache gate is **open**. This inspection used only the
`optimizations` checkout. No new Apalache run or model change was made. The
current model and fixed config retain SHA-256
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
was disabled, so there is no captured query that isolates solver clauses. A
candidate rewrite of the session type check would need exact record-domain
equality plus componentwise subset/Boolean checks to preserve the predicate;
field checks alone would weaken it. The pinned checker supports `--debug` for
`log.smt` and `--smtprof` for `profile.csv`. A CPU-bounded diagnostic with
those options and the unchanged model is the next query-level measurement;
any per-query timeout remains diagnostic failure.

The next qualification attempt must preserve the full fixed `Next` relation,
all 20 named invariants, length 18, and the `after` schedule. Profile the
generated SMT for conditions 10 and 33 in an isolated bounded run, establish
equivalence and TLC mutation parity for any encoding change, then execute the
complete pinned gate. No current result closes that gate.
