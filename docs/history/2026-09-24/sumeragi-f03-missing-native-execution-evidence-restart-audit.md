# F03 missing Native execution evidence after restart — source audit

Scope: `/Users/takemiyamakoto/devstuff/iroha` on the existing `optimizations`
branch, 2026-09-24. This is a source audit, not an executed four-peer result.

The mandatory release identity
`nexus_autoscale_native_recovers_missing_execution_evidence_after_restart`
is absent from `integration_tests/tests/nexus/autoscale_localnet.rs`. The
multilane ledger reserves that exact non-ignored identity under G-4P, and
`scripts/run_sumeragi_v2_release_gates.sh` checks the test inventory before
qualification. Adding a placeholder, renaming an existing ordinary drain test,
or carrying a synthetic receipt would not exercise the required recovery.

The current production `ValidBlock::validate_execution_context_header` in
`crates/iroha_core/src/block.rs` rejects any carrier containing
`native_lane_decisions` with `native lane economic carrier is not active`. The
current Native reducer/driver has real signed RS16 and Decision tests, but its
economic output remains held as an unapplied effect. The source-recovery test
`native_driver_source_recovery_rejoins_original_owner_after_foreign_refusal`
recovers a **resultless first-admission body** and explicitly verifies that
recovery does not create Ready or Apply. That is a different dependency from a
globally finalized carrier's missing **result-bearing execution image**.

The funded Apply service has the relevant fail-closed boundary:
`V2ApplyService::revalidate_recovered_candidate` in
`crates/iroha_core/src/sumeragi/v2_apply.rs` refuses a finalized marker when
the verified Kura read lacks its canonical execution image, and refuses an
already-applied State without exact finalized evidence. The focused
`finalized_marker_recovery_requires_canonical_execution_image` test covers
both pre- and post-State publication for an ordinary Apply fixture. Its test
does not prove the Native four-validator daemon/restart path. Native validation
retains `AwaitingSource`, `Executed`, and `Published` phases in one candidate
owner; `try_publish` requires the original evidence-ready executed phase.
Those ownership types are necessary, but they do not replace the missing
end-to-end test or the currently closed Native admission gate.

The required test can be implemented once the sole Native execution and Apply
consumer is active. It must use four real validators with signed revision-4
RS16 availability, produce an accepted Native transaction and one finalized
carrier, then stop one peer at a durable boundary while its canonical
result-bearing body is absent but the exact Kura finality/QC and State prefix
remain. On restart it must first observe a typed recovery wait, with no
reexecution against an already-applied State and no marker promotion from a
resultless proposal or cached digest. Recovery must fetch the exact canonical
body from authenticated holders, check its subject, execution commitment,
first-input binding, and finality against the same candidate, then finish
application exactly once. A second restart must preserve the same transaction
result, State root, Queue ownership, lane incarnation, and four-peer
convergence. Tampered/forked body and finality, wrong holder, stale incarnation,
and duplicated recovery responses must leave the original owner available for
retry or fail-stop without manufacturing another execution.

No production gate or release ledger status changes as a result of this audit.
The G-4P test identity remains absent and the release gate remains open.
