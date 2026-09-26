# F03 canonical recovery output boundary — 2026-09-24

Scope: the existing `optimizations` checkout at
`/Users/takemiyamakoto/devstuff/iroha`. This narrows one live interrupted-tip
recovery output path; it is not the first-release lane protocol cutover.

The [2026-09-23 production call-graph audit](../2026-09-23/sumeragi-f03-native-runner-source-and-evidence-audit.md)
identifies `dispatch_canonical_executed_block_recovery_effects` as a live caller
of the generic `dispatch_lane_work_effect`/`post_lane_block` corridor. The latter
still accepts old lane-local wire variants. The canonical recovery owner emits
only certificate-free `CanonicalExecutedBlock` requests and
`CanonicalExecutedBlockChunk` responses. The runner dispatcher now checks those
exact effect shapes before passing either to the unchanged generic output
service. An unexpected old proposal, other lane-work effect, or historical
request for another kind stays owned and returns the existing local service
error; it cannot borrow this recovery-only production path.

The new `canonical_body_recovery_dispatch_rejects_old_lane_and_unrelated_output_effects`
test covers both permitted shapes and substitutions with an old NewView vote,
an unrelated QueuePlan output effect, and a wrong-kind historical request.
The first combined Core binary compiled, but this test stopped in an unrelated
heavy history fixture with `MissingInsertBlock` before reaching its assertions.
That fixture was replaced by lightweight variant-only values. The corrected
selector passed 1/1 from the next combined Core binary
`target/debug/deps/iroha_core-4c145bf78d09a1ce`; five adjacent
`canonical_body_recovery` selectors passed 5/5 from the first binary. These
focused results do not establish the Native cutover or distributed recovery.

This change does not delete the old adapter, signer, wire tags, generic output
service, or current historical recovery protocol. F03 remains open for the
single V1 retirement cut, full Native runner-to-Apply/restart evidence, and the
four-/seven-validator fault and lifecycle qualification in the prior audit.
