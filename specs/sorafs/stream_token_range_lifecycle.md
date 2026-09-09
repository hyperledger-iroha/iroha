# Stream-token range lease lifecycle

Owners: Torii `stream_token_enforcement`, `stream_token_cleanup` and
`api/stream_token_body`. This local lifecycle does not authenticate a lease;
`StreamTokenAdmissionCaptureV1` retains exact canonical request, qualification,
record, callback and acknowledgement validation. The production provider and
independent custody/state/observer integrations require separate qualification.

All synchronous serving custody checks and terminal admission/callback work run
through the configured query/heavy physical worker. Cancellation of the awaiting
request retains those permits until the physical call ends. Before an Accepted
external admission, Torii reserves one cleanup ticket. Static rejections have no
lease and need no ticket; external quota rejections return the unused ticket.
A returned Accepted record is owned before fallible window checks or telemetry.

`admission_max_pending` additionally bounds local reserved/queued cleanup tickets
(K, default 65,536). This is a separate local resource ceiling, not a change to
token cardinality, stream quotas or external outbox capacity. One receiver runs
one physical release, so at most K queued/reserved records plus one in flight
exist. Bodies retain tickets through application consumption, error or drop;
query/heavy permits are not held for the stream lifetime. Drop only transfers
the known record into its reserved queue slot; it does not call the provider,
create tasks or acquire a blocking mutex.

The receiver is a Torii critical worker registered in startup rollback and
shutdown joining. It owns minimal queue/health state and work's capture, never
`AppState`. Shutdown rejects new tickets and closes admission to the queue while
draining pre-existing tickets and joining physical releases. An unresolved
release makes exactly one attempt, fences local Accepted admission and signals
supervised shutdown; existing records continue draining. Unexpected receiver
loss fences immediately, including loss before its first poll. Pending records
are accounted once; an already claimed physical operation alone settles its
actual acknowledgement or failure. A valid late acknowledgement remains valid
without restoring the failed receiver's health.

The body's immutable window contains the exact record's validation and exclusive
expiry times plus a conservative monotonic deadline anchored before the worker's
wall-clock validation sample. Slow admission cannot start a fresh relative
lifetime at response creation. Body production requires wall time at least the
validated time and below expiry, and monotonic time below that retained deadline.
The response owner stops producing further application bytes at detected expiry
or shutdown. Already handed-off Hyper/socket bytes cannot be retracted; this is
not a global hard real-time guarantee. An unpolled body conservatively retains
its ticket until consumption or drop.

A provider may commit admission and then fail callback/acknowledgement without
returning its record. Existing durable reconciliation and independently
authenticated lease expiry remain that case's recovery bound; no local release
or successful acknowledgement is invented. Expiry is also the bound for known
unresolved releases. Arbitrary synchronous providers can hang, so this contract
does not promise finite shutdown or an end-to-end ceremony deadline. Queue
capacity and release throughput require real deployment load qualification;
signed test fixtures and bounded gates do not establish it.
