# Completed experiment resource borrower

`ResourceExperiment.from_completed(owner)` accepts the exact originating
`FixedExperimentCustody`. Only that owner creates and registers one borrower,
with its original token, ten ordered run scopes, physical custody and native
proof authority. Direct construction fails. No completed receipt or reopened
bundle can create this authority.

The owner initializes `RunReplayScope(pair_index, variant, capture_directory,
journal_path, journal_sha256, peers, geometry, allocation)` for pairs 1 through 5,
one lane then four lanes. The borrower owns copies of these bounded public
values. Its pure `_scope_identity()` returns current primitive values; the owner
pins the original instance, token identity and scope identity independently.

`collect_replay()` enters `replaying`, calls `_replay_before`, then passes each
full actual resource replay to `_replay_accept` in order. The owner reconciles
that run with its original completed native authority. The borrower compares
the returned public resource projection with its independently reduced data,
retains only `RunResourceResult(pair_index, variant, resources, maxima)`, and
drops signed request bodies before the next run. It calls `_replay_finish` in
`finishing` with its exact stored ten-result tuple, then becomes `complete`.
`verify()` invokes `_verify_replay` and returns that same tuple. `close()` calls
`_release_replay` once and closes no original file descriptor. Any failed scope,
callback, replay, comparison, verification or premature release notifies
`_reject_replay` and permanently fails the borrower and originating owner.

Native trial deadlines remain unchanged. Original timed/private validation and
public custody transfer occur before each trial expires; final public replay
is guarded by the same experiment owner and its original experiment deadline.
The borrower grants no deadline extension, release verdict or control-reading
API. Manifest, raw summary and report publication stay with the original owner.

The maxima math remains unchanged: whole-run maxima include preflight and all
actual captures; interval maxima use inclusive overlap of collection brackets.
The retained pure `_reconcile_run_resources` helper consumes the immutable
resource snapshot and original geometry. It does not establish provenance.

The first-release migration removes `RunReplayInput`, the fresh-bundle
constructor, `read_control`, and `reconcile_run`; no aliases remain. The old
validator and constructor-based experiment/control/reconciliation tests must
be replaced by the originating-owner integration. Their historical assertions
remain preserved in the private migration context, not treated as passing
against the retired API. The focused new tests exercise ten actual raw capture
replays with explicitly controlled owner hooks, mutation/failure handling and
measurement math. Full ten-trial authority/report composition belongs to the
fixed experiment owner; these tests do not qualify native execution.
