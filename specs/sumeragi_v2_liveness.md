# Sumeragi v2 liveness contract and release gate

Sumeragi v2 does not promise unconditional termination. An unbounded network
partition, the absence of a responsive `2f + 1` equal-vote quorum, or local disk, signing,
validation, and application work which never completes can prevent progress.
The first-release target is therefore conditional:

> After GST, with an exact `n = 3f + 1` roster of 4 through 31 voting peers, a
> responsive `2f + 1` equal-vote quorum, and deterministic terminating local work, every
> height eventually decides and every responsive validator eventually applies
> the decision and activates its successor height.

This remains the conditional protocol target and paper argument until the
proof ledger reports `machine_checked_completion: true`.

The runtime premise is per validator. Each non-crashing responsive validator
in the active-height or exact historical-recovery corridor must have an
advancing local monotonic clock, regain its serialized height-runner turn after
each finite wait within the declared service bound, and finish admitted local
work within its declared bound. The formal service-deadline vector is ghost
bookkeeping for that trusted premise, not shared production state. Source
fidelity seals the narrower implementation facts: the height loop is
serialized, completion/ingress/runtime/sidecar work is serviced in finite
batches, the watchdog is polled on every loop edge, and idle/continue paths use
the finite 10 ms `IDLE_POLL`. Those checks do not prove host scheduling or I/O
latency.

The release claim is exercised only by networks with at least four voting
peers. Single-peer and undersized fixtures are not representative Sumeragi v2
networks and cannot satisfy the production-network release gate, even when a
bounded local model is useful as mutation evidence.

For the first release, a “responsive validator” is a voting Sumeragi v2 node
running on the validator-storage platform contract implemented and exercised
by the release corridor: Linux or macOS with stable filesystem object
identities, descriptor-relative no-follow rename and unlink operations, and
durable POSIX file and directory `fsync`. The source-bound corridor is
platform-pinned to Linux x86_64 and macOS arm64; published production validator
artifacts are Linux. Other platforms are restricted to non-voting observer or
development use and are outside the validator-liveness quantifier; complete
observer application and lane-retirement behavior there is not
release-certified. A platform which cannot perform progress-sidecar promotion,
lane-geometry movement, or authenticated archive deletion fails
voting-validator startup and cannot satisfy “terminating local work.”

The formal liveness contract makes both quantifier boundaries explicit. An
undecided execution must either decide before another rotation is needed or
reach a view in which the responsive honest scheduled leader itself is active;
that leader state must then lead to decisions at all responsive validators.
Application is per validator: once GST holds and one responsive validator has
a durable decision, that validator must eventually recover, validate, and
apply the certified body even if another responsive validator has not yet
decided. The aggregate all-decided-to-all-applied clause is retained only as a
composition target for successor-height activation.

## Progress ownership

The reducer and its adapter enforce one structural rule at persistence and
view-change boundaries: volatile progress state may be cleared only while a
durable source retains a fairly scheduled reconstruction path. In particular,
a timeout certificate may clear a volatile vote pool, but it does not clear an
active PrepareQC lock or an existing corresponding durable Commit intent. A TC
may also promote its selected highest PrepareQC to the active durable lock at a
node which has no Commit intent for that proposal origin. That node does not
sign merely because the TC was installed: it recovers, durably stores, and
deterministically validates the exact locked body at its old origin, but that
validation cannot mint a split-round Commit. An already-durable old-round
Commit intent may resume unchanged. Otherwise the retained bytes supply the
safe value for a later same-subject re-proposal, whose manifest, PrepareQC,
Commit intent, Vote, and CommitQC all name one new same round.

A second TC for the immediately preceding timed-out round is progress only when
its selected Prepare origin strictly exceeds both the installed highest PrepareQC
and lock. Its durable installation upgrades that lock while leaving the lifecycle
view unchanged; equal, lower, and no-high replacements are rejected.

This is a narrow exception to the timeout fence. It authorizes only an
already-durable exact old-round Commit whose subject still matches the active
TC-promoted lock; unrelated Commit origins and old-round Prepare creation
remain fenced. Same-subject evidence at a later origin never relabels the old
intent: it is a new proposal which must pass the complete same-round Prepare
and Commit path. A missing or mismatched lock fails closed instead of reviving
an unrelated old round. Each authenticated Commit vote must satisfy
`proposal_round == round` and exactly match its same-round durable Prepare lock
before it may cross semantic delivery admission once in each active consumer
epoch. A TC generation change or a newly acknowledged exact lock advances that
consumer; equivocation fingerprints are tracked independently from delivery
records and retained for one roster rotation.

The exact-lock rule also applies to current-view Commit votes. A vote delivered
before the node acknowledges the matching `LockAndCommit` is ignored
recoverably rather than creating an authority-free pool. The acknowledgement
first makes the exact same-round intent durable and only then releases the
current Commit signature. The adapter's
locked-Commit consumer epoch changes at this boundary, so the previously
ignored exact vote may cross admission once after the lock exists without
weakening the independently tracked, rotation-bounded equivocation fingerprint
history. Complete authenticated conflict-pair persistence for penalties remains
separate future work.

Durable-intent refinement treats a WAL record's intrinsic round and the
reducer's lifecycle owner tag as distinct identities. The owner tag authorizes
the current transition; the pending projection and `Persist` effect retain the
round encoded by the WAL record and satisfy that record kind's round-order
relation instead of being forced to equal the owner view. In particular, an
`InstallTimeout` may carry a certificate round ahead of the current view. When
that certificate contains a highest Prepare, its Prepare subject is projected
through the boundary, pending record, and persistence effect; a timeout without
a highest Prepare retains the canonical no-Prepare subject sentinel. The same
boundary separately binds the primary WAL proposal origin and the auxiliary
proposal origin of any embedded PrepareQC. Begin and acknowledgement compare
both against the pending record, so changing both requested and granted
capabilities cannot substitute another origin.

Application keeps three round domains separate. The reducer lifecycle owner
tag authorizes which process-local incarnation may start or complete work. The
proposal-origin round identifies the exact locked proposal manifest, durable
frame, and validation receipt. The certification round identifies the Vote or
QC slot. The owner tag must equal the reducer's independently observed current
tag; it has no ordering relation to the signed wire round. Every vote and QC
authenticates an explicit `proposal_round` in its signed preimage, and both
Prepare and Commit require that origin to equal their certification round.
Body fetch, application, and startup recovery require the manifest, durable
frame, and validation receipt to equal that authenticated round exactly. A
local lock is only a consistency/cache input and is never used to guess a
missing origin. Conflicting body, manifest, frame, or execution-commitment
identities fail closed, while a lockless validator can request the exact
certified round directly from any signer. Revision 4 has no legacy decoder
which fills a missing proposal origin.

The canonical height-one genesis header is the only header-origin exception.
Its authenticated bytes fix `view_change_index = 0` before consensus timers
start, so an initial timeout may move the first proposal and its Prepare origin
to a later view without rewriting the genesis block hash. Finality and bridge
validation accept that exact height-one, parentless, view-zero header shape;
all ordinary blocks still require the header view to equal the authenticated
proposal origin exactly.

Once a proposal origin is locked, an already-durable Commit intent can still
complete only in that exact old round. If it does not, a later leader may
re-propose the same block subject and bytes under a new origin. The new
manifest is stored and validated for that round, then a same-round PrepareQC
and `LockAndCommit` create the only authority for its Commit. No old receipt or
vote is retargeted to the later round.

Reducer transitions also check executable progress witnesses. A durable locked
Commit intent must be represented by signing work, the exact local vote pool,
recovery ownership, or a decision. Retained outbound control is a peer-delivery
source, but it is not a sufficient local witness because broadcast excludes the
sender. A TC-promoted lock without an intent must retain exact body-recovery
ownership through later unchanged re-proposal or legitimate supersession; old
origin validation alone cannot append a historical split-round
`LockAndCommit`. A durable decision awaiting application must retain its exact
body pipeline and may not refer to a body which deterministic validation marked
invalid.

There is one serialized race exception: validation of a TC-promoted lock may
complete after the current finality view has durably timed out. The exact
acknowledged timeout for that current view then owns recovery, but never grants
permission to append or sign a Commit in the closed view. The following TC
retains the same safe subject and restarts body recovery; successful validation
of the old origin still cannot create a Commit in the new view. Only an
unchanged later-view re-proposal can establish the new same-round manifest,
PrepareQC, and Commit authority. The source-shared progress kernel rejects
stale, wrong-signer, volatile-only, and non-exact timeout substitutions.

Decision retransmission reconstructs the next missing owner from the durable
body stage instead of assuming a fetch is always sufficient. A missing body
restarts recovery, an available body restarts `StoreBody`, a durable body
restarts `ValidateBody`, and a validated body restarts `Apply`. Dropping any
one volatile owner therefore leaves a deterministic reconstruction path from
the durable Decision and body-stage record.

Adapter candidate admission has the same exact-owner boundary. A first
semantic lifecycle moves its owner count from zero to one; an exact retry may
remain one-to-one only when it carries that immutable incumbent owner. A
different owner for the same semantic lifecycle fails closed before the
effect-to-candidate refinement projection is constructed. Monotone certified
Fetch, Store, and Validate authority upgrades remain separate: their stronger
phase/commitment identity can retain the one physical stage lineage only after
the typed authority relation is checked and re-projected.

Application retransmission has the matching terminal rule. While `Apply` is
in flight, an exact rediscovery coalesces with the incumbent work identifier
and lifecycle owner. After Kura returns the typed finality receipt and exact
artifact, the executor retains their original reducer tag as the completion
tombstone: later exact rediscovery is absorbed, while any tag, subject,
context, or CommitQC drift fails closed. Draining the work queue therefore
cannot recreate the same logical application at its old stage. This is a
source-level production repair and does not by itself promote the
`application-liveness` or production-refinement ledger entries.

Decision installation is also a terminal ownership boundary for global reducer
input and losing carriers. Before its `FetchBody` effect can reserve capacity,
the executor retires every competing fetch, store, validation, signing,
proposal, candidate, outbound, and retransmission owner. It preserves the exact
decided body pipeline and its exact merge-sidecar deferral until application
starts. A CommitQC discovery response is reducer-producing because it unwraps
to a QC, so it is discarded once Decision is installed; a Decision formed
mid-ingress batch returns to the runner before another global occurrence can be
admitted. Unmatched payload chunks and certified current-height body requests
for a losing subject are also discarded, while exact outstanding decided-body
transport and historical serving remain available.

Lane completion has a narrower terminal rule. The adapter reconstructs the
winning same-height lane proposals from the canonical Kura block's lane
ownerships and permits only those exact proposals, votes, QCs, and certificates
to continue after global Decision. A canonical block with no lane ownerships
has no lane durability debt, including result-bearing genesis or external-only
blocks; rollover must not reinterpret their external entries as a malformed
lane plan. Losing lane carriers are purged before any post-Decision relay or
retransmission turn. Late I/O therefore cannot recreate losing proposal or lane
work, but the decided carrier can still form and persist the CommitQC which the
canonical block requires.

Those witnesses also cover recovery boundaries. WAL replay after durable
`LockAndCommit` restores the exact pending Commit signature and broadcast. For
a historical record, replay first requires the same exact lock to be active
after the preceding TC installation; `InstallTimeout` alone never synthesizes
a signature. A validated body retained under a lock survives leader loss.
After leader rotation, the retained body and the existing or reconstructed
exact old-round Commit intent can rebuild that old quorum; otherwise the
retained body feeds the later unchanged re-proposal path instead of leaving a
lock with no executable owner.

Proposal replay joins two independently fsynced authorities before startup
effects run: the safety WAL supplies the exact proposal intent and the body
store supplies its canonical body plus deterministic execution commitment.
Startup rejects any identity or commitment drift, otherwise restores the
commitment into the replayed registry so the same already-authorized proposal
origin can resume signing and proceed directly to its Prepare vote even when
the durable intent belongs to a nonzero view. Replay never creates a later
origin for those bytes.

Under the same post-GST, responsive-dual-quorum, terminating-local-work
assumptions, certified lane-block and application-receipt progress sidecars
always cross data, index, immediate-directory, and bottom-up ancestor
durability barriers through the authenticated Kura root. These barriers apply
even when ordinary Kura persistence uses batched `fsync`; a page-cache-readable
record is not treated as durable progress. Authoritative reads re-attest the
data, index, sidecar, and root binding and fail closed on drift. The writer's
optimistic receipt observation is structural only and performs neither sidecar
recovery nor a durability barrier. On the write path, both are deferred to the
subsequent mutation phase after it holds the geometry and sidecar locks;
authoritative readers retain their independent attestation path. Pending
alternate QCs and proofs of possession are fully authenticated and validated
before the corresponding same-proposal owner can be retired, so invalid
alternates cannot erase the remaining progress witness.

Ordinary progress-sidecar appends publish a bounded, canonical Norito intent
before mutating either main file. The intent binds the exact data suffix by a
domain-separated digest and carries bounded old/new index windows, allowing
restart to roll an exact postimage forward or restore the exact preimage.
Unpublished builds are discarded, while malformed, conflicting, non-regular,
or multiply linked authorities fail closed. Recovery preserves a separate
retryable-I/O classification, so an interrupted open, read, metadata, or
durability operation does not masquerade as permanent corruption. Lane
retirement runs the same descriptor-bound recovery for all six fixed progress
pairs before freezing its immutable directory snapshot, so a crash residue
cannot be mistaken for either live work or an admissible empty lane.

The release claim is platform-scoped, not a portable-filesystem claim. Windows
and other non-Unix progress promotion and authenticated lane-archive garbage
collection remain unsupported. Those fail-closed errors do not count as
terminating local work, and no unsupported-platform validator release receipt
may be emitted.

The serialized runtime and adapter each reserve completion and progress
capacity. Their cyclic `completion -> progress -> normal` service order keeps
FIFO order within a class and records eligible service skips only for the
oldest item in a non-selected class. Exact locked Commit votes authenticate
through the progress reserve even when ordinary ingress is saturated. The
unauthenticated wire-shape predicate is a production capacity hint only;
authenticated exact-lock matching independently authorizes the Progress class
before enqueueing.
Production `BoundedIngress::pop_next` calls the same fixed-width selector that
Verus checks: every selected class is ready, an invalid cursor selects no work,
and three continuously ready classes are each selected once per three runtime
invocations. This is not a temporal fairness claim. The post-GST argument still
requires the host to invoke the runtime fairly and every admitted external
operation to terminate or report failure.

Reducer effects are also structurally bounded. The largest flattened
persistence macro-step contains four effects, below the fixed limit of eight
effects per serialized adapter macro-step. When adapter debt is serviceable,
each runtime turn services exactly one deferred adapter macro-step before timers
or newer ingress; this decreases that debt without allowing one turn to
monopolize the runtime.
The adapter cannot report terminal readiness while any deferred Completion,
Progress input, or ordinary input remains.

Certified-body Serve traffic has a separate atomic handoff at the outer runner.
When the final member of one frozen Serve batch retires, the queue records one
producer episode as due under the same lock that removes the selected owner.
Fresh Serve admission remains Busy while that episode is due or active, so a
new response replenishment cannot reserve the released slot before proposal or
other ordinary producer work receives one bounded turn. Beginning the episode
atomically consumes `due` and acquires `active`; dropping its local lease clears
`active` and reopens Serve admission. Neither a rejected replenishment nor the
handoff itself allocates a scheduler or semantic-lifecycle ordinal. This finite
handoff prevents an endless sequence of individually finite Serve batches from
starving proposal production; it does not treat replenishment as consensus
progress and does not add a network fairness premise.

Effect admission has two retryable capacity boundaries. General pending-work
exhaustion is retryable for ordinary effects. Independently, certified-request
capacity pressure is retryable only for a `FetchBody` effect and retains its
exact causal suffix without partially acquiring either pending-work or request
ownership. Every other effect-admission failure, including a malformed or
oversized effect shape and an impossible `Busy` after the serviceability fence,
fails closed.

Body-pipeline completion ownership spans both runtime ingress and the
adapter's Busy-deferred queues. The deferred owner retains the full manifest
and non-forgeable durable or validation receipt, including validation
success/failure polarity. A retry coalesces only when that complete evidence
is equal; conflicting evidence or more than one owner fails closed. The check
runs before a `BodyAvailable` completion can prune conflicting queued
proposals, so a non-exact retry cannot mutate consumer state and then hide as
a duplicate. Before either serialized enqueue path can construct a tagged
command or allocate an admission ordinal,
`preflight_runtime_command_admission` validates the complete callback and
classifies its phase. Exact already-applied `BodyAvailable`, `BodyStored`,
`ValidationSucceeded`, and `SignatureCompleted` retries stutter without a
physical owner or new ordinal. Conflicting validation evidence or opposite
success/failure polarity fails closed. A malformed callback rejects even when
it carries a stale tag, because complete callback validation precedes
stale-tag coalescing; a well-formed stale incarnation is discarded without
installing a marker. The finite TLA+ mutation matrix covers only the
evidence-bearing `BodyStored` and `ValidationSucceeded` Busy/applied phases.
The four-phase Rust test extends exact-retry suppression—not the matrix's
conflict or Busy-owner claim—to `BodyAvailable` and `SignatureCompleted`.

An admitted `FetchBody` becomes passive while it waits on the network. The
executor retains its exact lifecycle owner, but does not publish that wait as a
runnable actor-global scheduler minimum. Otherwise a missing response can
block the timeout, retransmit, Proposal, or QC which supplies or supersedes the
same acquisition. Before retiring the fetch, the executor reserves and commits
`BodyAvailable` with the original owner; that concrete completion therefore
re-enters scheduling at the exact old ordinal, ahead of unrelated later work.

That passive-to-runnable transition is also a reviewed exact-Serve boundary.
A selected Serve ticket may legitimately finish an older-owner turn while the
Fetch is still passive; a later authenticated reconstruction must not be
stranded behind the completed turn. The runtime therefore retains, for the
current process-local Serve target only, whether a strictly older runnable
prefix is physically present and a monotone predecessor-episode ordinal. An
observed `no older -> older` transition issues one non-serialized witness over
the Serve ordinal, the minimum predecessor ordinal, and the next exact episode.
Repeated observation of the same continuous prefix returns the same witness;
retry-unadmitted pressure keeps the physical-presence latch set and cannot mint
another episode. The worker accepts episode one or the checked exact successor
of its retained witness, rejects regressions, gaps, target drift, and changed
same-episode evidence, and changes `Complete` back to `Ready` exactly once for
that new witness. The runner publishes and observes this evidence before the
claim and after each bounded predecessor recheck. It can therefore drain the
late Fetch descendant under its original ordinal and still preserve the
selected Serve position and the final producer handoff. Passive Fetch itself
remains intentionally absent from the runnable minimum, so an unresponsive
body source cannot veto timeout or view progress. Witness replenishment is
finite local bookkeeping, not consensus progress or a new fairness premise.

Authenticated consensus-message ownership likewise spans the runtime queue and
the adapter's Busy-deferred lanes through one generic production path. The
adapter compares the complete canonical envelope with the retained
`authenticated_wire_identity`, and runtime ingress repeats the exact comparison
after authentication. Thus an exact CommitQC embedded in a
commit-certificate response may cross a saturated Progress boundary only to
coalesce with that exact envelope owner; a distinct certificate remains
backpressured. The QC-named adapter lookups are `cfg(test)` conveniences for the
focused regression, not reducer-event-only production evidence. One logical
exact QC may retain a bounded aggregate of independently authenticated carriers from
different sources, with direct-QC and commit-certificate-response forms tracked
separately. Coalescing therefore preserves admitted source carriers without
creating a second runtime command up to the protocol bound. Another occurrence
of the same semantic request must merge its exact route and accounting history;
a same-owner conflict fails closed instead of masquerading as a distinct
carrier. Genuinely disjoint carriers remain individually named up to the
maximum validator count, including their semantic origin and authenticated
source identities. The next carrier receives recoverable backpressure rather
than being summarized without its identity or failing the process. The raw
capacity hint is recomputed after authentication, and any disagreement fails
the serialized runtime closed instead of authorizing a conflicting owner.

The immutable admission identity of each carrier binds its semantic delivery,
source, route set, and cursors, but excludes a reply route's mutable active
flag. Retiring a transport tenure therefore cannot retroactively invalidate an
already admitted carrier. Delivery still requires a live route, and explicit
route maintenance prunes inactive routes and publishes the changed retained
set in the ownership projection.

The executor also calls one source-linked typed identity kernel at every
`FetchBody -> BodyAvailable -> StoreBody -> ValidateBody` owner boundary. A
certified Fetch may fill its initially absent manifest identity exactly once;
after that, tag, round, subject, and manifest are immutable. Certified-view
rebind changes only the consumer tag, requires both view and generation to
move monotonically at the same height: the view cannot regress and the
generation must strictly increase. This includes an alternate certificate for
the same timed-out round that installs a strictly higher PrepareQC in a new
generation without changing the lifecycle view. The exact body identity stays
immutable.
Supersession computes reconstructed/retained and pending-store residual bytes
with a checked sequential subtraction relation shared with Verus, avoiding an
overflowing combined-retirement sum. Before either EnterView cleanup or direct
locked-body reconciliation, a global preflight also requires the counters to
equal the complete serialized ready/retained/store owner sets; exact lock
repetition cannot bypass that check. The theorem covers these serialized safety
transitions; map extraction, cryptographic manifest identity, service
acknowledgement, and eventual scheduling remain explicit production/temporal
boundaries.

`LocalProposalReady` is trusted completion work, not ordinary proposal ingress.
It uses the runtime Completion reservation and, if the reducer reports Busy,
the adapter's Busy-deferred Completion reservation. Saturating Normal ingress
therefore cannot strand a locally built, durably stored, validated proposal.

Cryptographic authentication also checks commitment-bearing consensus evidence
before the envelope receives serialized runtime ownership. An individual
signed Vote cannot establish execution-commitment authority: its exact round,
subject, and commitment must already be bound by a local validated receipt,
verified safety-WAL replay, or quorum-authenticated QC evidence. An unbound Vote
is rejected without closing the runtime and may be retried after an authoritative
binding arrives. QCs, TCs, proposal justifications, and certificate transport
are checked for conflicts both against existing authority and within the one
authenticated envelope.

`BodyAvailable` rebind is authorized only after the reducer has installed its
destination tag. Before mutation, the runtime inventories the exact source and
destination owners across ingress and Busy-deferred state. One exact source may
move into an empty destination or coalesce into one exact destination owner;
an uninstalled destination is a recoverable caller-contract rejection with no
mutation. Conflicting destination evidence or duplicate source/destination
ownership fails closed without changing either side. Body-pipeline and Decision
retirement use the same transactional preflight: every owner count and evidence
invariant is checked before any queue entry is removed.

Higher-lock, Decision, and certified-view cleanup also treat an external
cancellation failure as a restart boundary, never as a successful retirement.
The executor keeps the exact fetch/store/validation owner until all required
service cancellations acknowledge; a failure latches the process-wide
restart-required guard while the safety WAL or durable Decision remains the
reconstruction source. Adversarial tests inject cancellation failure at each
of these three cleanup boundaries while an exact certified fetch is pending
and require that fetch owner, signed-request hash, and accounting projection to
remain exact. Cleanup preflights both certified-request indexes before runtime
or service mutation and commits their removal as one infallible serialized
step. Separate corrupt-index tests require lock, view, and Decision cleanup to
fail closed before issuing even the first external cancellation. Direct
locked-body reconciliation closes itself on a service/runtime cleanup failure
instead of relying solely on the outer runner to terminate.
Multi-fetch view cleanup also completes every ordered service callback before
committing any executor or certified-request retirement; failure on the second
callback therefore leaves the complete executor projection unchanged and
forces restart recovery.
Decision cleanup similarly defers detaching an exact decided local
store/validation consumer until all losing service owners acknowledge
cancellation, so a failed losing fetch cannot orphan the decided local
pipeline.
If the final status publication fails after cleanup commits, the replacement
lock and retired certified indexes remain the authoritative executor state and
the node still fails closed; retries cannot resurrect the superseded owner.
Across later view-cleanup categories, an already acknowledged signature,
store, or validation cancellation is not rolled back if a subsequent callback
fails. The fail-closed process must restart; WAL replay restores the TC, lock,
and Commit intent, while the locked QC recreates any required body fetch.
Unprotected stale work is not a progress witness in the installed view.

Remote certificate conflicts at the body-recovery transport boundary are
nonfatal. A certified-body request whose QC conflicts with local authority is
rejected without closing the runtime, and a conflicting Commit-certificate
response leaves discovery outstanding so another authenticated peer can be
retried.

Recovery-scoped CommitQC discovery also has an urgency rule. A process which
starts with durable v2 height ownership or an interrupted applied tip may issue
the first discovery request immediately instead of first spending an ordinary
live-round timeout. That urgency reaches the next height only when an
authenticated `CommitCertificateResponse` yields a discovered CommitQC which
is admitted to, or coalesced with, serialized reducer ownership; authentication
rejection or runtime backpressure cannot manufacture the carry. The outstanding
request's `Some`-to-`None` transition proves only that ownership
handoff. It does not prove reducer execution, Decision, durability, or that the
certificate came from historical Kura. Ordinary live finality clears the hint,
and an ordinary height retains its configured quiet round before discovery.
This changes when the existing request is scheduled, not its request identity,
authentication, exact frozen context, certificate checks, or reducer
transition. It therefore avoids one full quiet-round delay per missing height
during sequential catch-up without creating permanent normal-height fanout.

Certified-body request capacity is independent from the retryable general
pending-work boundary. When that request bound alone is full, only a
`FetchBody` producer may retain the exact causal suffix and retry. The executor
installs one bounded FIFO owner for the exact task, authority, and lifecycle
ordinal, while the rejected attempt acquires no partial pending-work, request,
or transport ownership. After capacity releases, retrying that same FIFO head
acquires pending-work and request ownership atomically. A genuinely new Fetch
then drains the successful head; an existing ordinary Fetch retains it as the
exact completion barrier after the request-authority upgrade. An authenticated exact
`CertifiedBodyResponse` is transport-only and may cross retained
reducer-effect debt to retire the exact blocking work/request pair. A
`CommitCertificateResponse` is different: it exposes its authenticated
CommitQC to reducer ingress and therefore remains reducer-ordered. Before the
executor boundary, each validator has one shared outer TransportCompletion
slot and full-envelope byte reserve for either a `CertifiedBodyResponse` or a
`PayloadChunk`; generic reducer traffic and TimeoutVote ownership cannot spend
either reserve. The source-sealed finite matrix for this boundary now contains
6 models and 33 configurations: 10 repaired cases and 23 mutants. The 22
unchanged cases pin 131 generated and 130 distinct states; the revised
certified-request cases pin their semantic actions and violations without
preserving stale aggregate state counts. The retained owner binds exact task,
authority, and lifecycle ordinal; drop, substitution, duplication, overtaking,
partial P/Q install, and loss of the existing-owner retry barrier each have a
dedicated mutant. Its weak-fairness result
requires a responsive certified source plus terminating transport and body
work. Those are explicit premises; the finite evidence promotes no
proof-ledger obligation.

The adapter's deferred Progress reserve is partitioned by consumer ownership:
one locked-Commit slot and one independent TimeoutVote slot per frozen
validator, plus one slot for each PrepareQC, CommitQC, and TC class. Its exact
capacity is therefore `2 * roster_len + 3`. Exact retransmissions coalesce
before this capacity check. Vote ownership is signer-injective: the one
TimeoutVote slot is shared by the current and adjacent-future rounds, so a
distinct Commit or TimeoutVote from the same signer cannot consume a second
slot or displace the admitted owner and becomes admissible only after that
owner's slot is serviced. Once a progress item is admitted, later equal- or
higher-ranked traffic cannot displace it; a full class rejects the new item
while preserving the already admitted vote, reconstruction, or certificate
owner.

The semantic-admission table applies the same partition before the reducer is
called. A full ordinary-history budget cannot reject a TimeoutVote in the
bounded current/adjacent-future window: at most two keys per frozen signer
bypass that budget, remain available for equivocation/delivery checks while
their rounds are in the window, and are retired when they fall behind it. The
exact live semantic bound is therefore `1024 + 3 * roster_len`: ordinary
history, one locked-Commit set, and two TimeoutVote sets. On TC installation,
authenticated shares and retained local TimeoutVote control are filtered
against the newly installed current/adjacent window, so an early adjacent
share becomes immediately useful instead of being discarded. Thus the single
deferred TimeoutVote slot remains reachable without allowing unbounded future
rounds even when ordinary semantic history is saturated.

The roster-aware transport ingress also prevents auxiliary I/O backpressure
from becoming a per-validator head-of-line stall. On each source's fair turn it
removes the oldest message which the downstream consumer can currently admit;
earlier blocked messages remain owned in their original order. A certified-body
request waiting for auxiliary I/O capacity therefore cannot hide a later
proposal, QC, TC, body response, or payload chunk from the same authenticated
validator. The source still consumes only one turn, and the retained head is
selected first as soon as it becomes admissible.

An empty validator lane reserves an ordinary first-message slot, an ordinary
Progress slot, a certified-fence-escape slot, a distinct TimeoutVote slot, and one shared
TransportCompletion slot for either a payload chunk or certified-body
response. Short non-empty lanes retain the continuation potential needed to
restore all five reservations after any service step. Each simultaneously
materialized authenticated non-validator lane has three owners: one generic
message slot, one certified-fence-escape slot, and one TransportCompletion slot for either a current-roster
completion forwarded through that source or a proof-carrying historical lane
response from an authenticated predecessor signer. The anonymous lane has the
same two owners when the roster is non-empty, while its no-roster diagnostic
geometry needs only the generic slot because no anonymous completion can be
valid. If `H` is the
configured maximum number of simultaneously materialized authenticated
non-validator lanes, the exact non-empty-roster count minimum is therefore
`5 * roster_len + 3 * H + 2`; the no-roster minimum is `3 * H + 1`.
`H` is independent of exact-output reply-route capacity `R`, which is derived
from the effective `network.max_total_connections`. Root validation resolves
the selected lane profile before deriving `R`, so `lane_profile = "home"` with
an omitted explicit maximum uses `R = 32`, and rejects any configuration with
`H > R`.
The canonical Sumeragi shared-config projection is format version 6. `H`
remains fingerprint-bound, so incompatible source geometries cannot share a
handshake fingerprint.
Authenticated non-validator lanes are created on demand and removed when empty.
A semantic duplicate carrying an alternate authenticated reply route is merged
into the existing request before a new-lane `H` admission check; only a
semantically distinct request requiring a new source lane consumes another
lane. Ordinary
Progress includes Commit votes, PrepareQCs, certified-body requests, and both
Commit-certificate request/response directions. Proposals, Prepare votes, and
manifests remain ordinary; TimeoutVote uses its own signer-bounded corridor;
TC, direct CommitQC, and a Commit-certificate response carrying CommitQC use a
separate certified-fence-escape corridor;
`PayloadChunk`, `CertifiedBodyResponse`, and
`LaneHistoricalRecoveryResponse` share TransportCompletion. The first two
require a current-roster semantic origin. The historical response may instead
come from an authenticated predecessor signer, but the lane adapter admits it
only for an outstanding exact request whose frozen CommitQC or READY
certificate authorizes that signer. Relay
delivery keeps semantic origin separate from authenticated transport via: the
reducer and equivocation checks use origin, while count, bytes, and fair service
are charged exclusively to the via lane. Pending exact retransmissions
coalesce only for the same transport sender and canonical envelope hash, and
the coalescing authority ends when the consumer removes the queued occurrence.

Count bounds are paired with canonical-wire byte bounds. The
`sumeragi.queues.body_source_bytes` quota isolates each frozen-roster validator,
each of the `H` authenticated non-validator source lanes, and the anonymous
lane, while `sumeragi.queues.body_bytes` bounds their aggregate ownership at no
less than `(roster_len + H + 1) * body_source_bytes`. Roster installation fails
closed if the aggregate cannot provide every source partition. Each
authenticated source retains an isolated certified-fence-escape byte reserve;
each validator additionally retains an isolated timeout-vote byte reserve, and
every source retains an independent full TransportCompletion
envelope reserve, so ordinary traffic cannot consume the capacity required to
advance a view or finish body recovery. Height activation derives the latter
with checked arithmetic from the frozen DA layout and the exact bare-Norito
framing of the largest valid `PayloadChunk`, `CertifiedBodyResponse`, and
bounded historical lane response; an
overflow or undersized source partition fails closed before ingress opens.
Height activation also checks the exact maximal TC, direct QC, and CommitQC-response
wire ceiling against the certified reserve. These count, byte, Progress,
certified, timeout, and completion reservations prevent one
authenticated source from consuming another validator's recovery capacity or
turning the count-bounded queue into multi-gigabyte memory ownership.

### Production transport geometry

Height activation sizes every progress-relevant envelope from the frozen
context rather than from a representative fixture. Let `F(x)` be `x` plus its
canonical compact-length prefix. For a layout admitting `C` chunk hashes, the
exact bare manifest ceiling is `M(C) = F(8 + C * F(32)) + 228`. The proposal
ceiling then includes that manifest, a maximum-size signature, a timeout
certificate with one non-empty group per validator and a full PrepareQC in
every group, plus the separately carried highest PrepareQC. The production
calculation covers every valid `3f + 1` committee from 4 through the protocol
maximum of 31 validators; no one-validator or non-`3f + 1` envelope is
admissible. The implementation derives the exact proposal and recovery
ceilings from the frozen context at activation instead of relying on one stale
representative byte count.

Recovery sizing is equally structural. A maximum QC contains all `N` signer
indices and a 256-byte aggregate signature. `CertifiedBodyRequest` carries the
round, subject, maximum PrepareQC, requester, and maximum signature;
`CommitCertificateRequest` carries the actual frozen chain id, context, height,
requester, and maximum signature; and `CommitCertificateResponse` carries the
request hash, maximum CommitQC, responder, and maximum signature. The
control requirement is the larger of the proposal and Commit-certificate
response ceilings, while the ordinary ingress requirement is the largest of
body-envelope headroom, that control envelope, and either recovery request.

Requester and responder identities are not bounded by the frozen roster:
observers may request bodies and a rotated validator may serve historical
finality. The sizing path therefore uses the feature-independent protocol
ceiling of 8,258 raw public-key payload bytes, excluding the one-byte algorithm
tag. This is the largest accepted SM2 envelope: a two-byte distinguishing-
identifier length, up to 8,191 identifier bytes because SM2 stores that length
in bits, and a 65-byte uncompressed SEC1 point. Under the canonical unpacked
layout, the checked embedded `PeerId` calculation is
`F(F(8 + 2 * (8,258 + 1)))`; it is used for
requesters, responders, relay origins, and direct targets even when the active
roster uses smaller BLS identities.

The bare bound is not compared directly with a transport cap. Production first
nests it in `BlockMessage::V2`, the 40-byte-header `BlockMessageWire`, and
`NetworkMessage::SumeragiBlock`, including every compact prefix and alignment
byte. The P2P layer then computes the exact direct `RelayMessage` with
protocol-maximum origin and target identities and wraps it in the complete
header-framed `Message::Data`; direct delivery dominates broadcast. Finally,
the global encrypted-frame check reserves the 28-byte ChaCha20-Poly1305
nonce/tag expansion, and the outbound high-priority byte-owner check also
includes the four-byte encrypted-frame length prefix. In other words, a
plaintext P2P frame of `P` bytes requires `P + 28` global encrypted bytes and
`P + 28 + 4` high-queue bytes, in addition to fitting its plaintext topic cap.
The encrypted body itself must also fit the wire prefix, whose inclusive body
ceiling is `u32::MAX`. Production uses the stricter architecture-independent
`network.max_frame_bytes` ceiling of 2,147,483,643 bytes so the four-byte prefix
plus body fits a contiguous `i32::MAX`-byte buffer on both 32-bit and 64-bit
hosts. Both `iroha3d --check-config` and normal startup reject a larger
configured value before listener binding, and the sender independently derives
the encrypted length with checked arithmetic and a checked `u32` conversion
before encryption or frame allocation. Queue-charge arithmetic returns an
explicit checked result, so overflow rejects height activation even if an
operator configured `usize::MAX` queue bytes; no maximum-valued sentinel can
compare equal and pass. The sender also counts the exact canonical frame before
materializing its output buffer, while the receiver grows toward an admitted
declared length in bounded increments.

Both roster configuration and the later ingress-open boundary fail closed on
arithmetic overflow or if any exact count, ordinary, timeout, completion,
aggregate, consensus-topic, control-topic, block-sync-topic, or outbound-high
requirement is undersized. Production defaults are 17 MiB for the global
encrypted cap and the configured consensus and block-sync topic caps, 2 MiB
for control, 128 MiB per peer for the high-priority encrypted-frame queue, and
130 outer-ingress entries (`4 * 31 + 2 * 2 + 2`). The effective consensus and
block-sync plaintext ceiling is the 17 MiB global cap minus the 28-byte AEAD
expansion.

Kagami-generated localnets inherit those 17/17/2/17 MiB frame caps, the
130-entry count bound, and a 33 MiB per-source byte partition. They accept only
exact `3f + 1` rosters from 4 through 31 validators and set aggregate
body-ingress bytes from the validator, authenticated non-validator, and
anonymous source partitions, retaining the reviewed four-validator floor.
The Taira template carries the same per-source baseline; its bundle renderer
enforces the same 4, 7, 10, …, 31 geometry and raises the aggregate to at least
`(validator_count + authenticated_non_validator_sources + 1) *
body_source_bytes`.

The peer sender extends that ownership boundary through encrypted stream I/O.
It retains at most one bounded plaintext retry in each safety, ordinary-high,
and low pool when encrypted-frame capacity is full. Safety and ordinary-high
encrypted frames share one configured aggregate high count/byte envelope, while
the safety plaintext retry remains a dedicated owner with the first retry rank;
ordinary traffic cannot evict or consume it. A write that is cancelled after its bytes reach the
stream but before flush completion retains the non-empty batch as a pending
flush witness, resumes the flush before staging later work, and never writes
the batch twice. One read/write arbiter polls both reliable streams and both
outbound senders, alternating equally ready high/low work. Direct post intake
has a finite burst; on exhaustion a non-cancelable checkpoint gives reliable
stream I/O first refusal before intake reopens, so continuously ready
best-effort datagrams cannot starve consensus traffic.

Reliable consensus, block-sync, and genesis messages keep their network-actor
byte/count owner until every selected peer writer reports a complete local
stream write and flush. Missing or replaced sessions retain the same opaque
actor item; only failed targets re-enter the sorted, source-fair retry cursor,
so one unavailable target does not hold a responsive direct target behind it.
That flush acknowledgement is a local transport-attempt witness, not a remote
application acknowledgement. In spoke/assist routing it also is not an
end-to-end acknowledgement from the final target: durable protocol
retransmission or committed-state recovery remains responsible for another
attempt after a hub accepts but cannot forward a frame.

An exact authenticated reply additionally owns an immutable writer deadline
from its first network-actor dispatch, before it can enter the peer-writer
queue. Repeated full-queue dispatch retries cannot reset that deadline and do
not use the best-effort overflow knob. The actor polls the writer completion
first, so a full flush already published at the boundary wins route retirement,
connection replacement, and exact-deadline observation. An empty or closed
completion cannot keep a stale route alive or terminate its replacement.
Otherwise expiry marks the exact tenure unwritable, retires only the same
connection if it is still current, releases actor ownership, and reports
`TimedOut`, never `Flushed`.

The exact-output worker retains one saturating `u8` timeout generation per
semantic target and current item. Only `TimedOut` increments it. An ordinary
writer close and an authenticated reconnect preserve it, while successful
flush and cursor advance reset it to zero. Attempt `a` uses the checked,
saturating interval `base * 2^a`. The actor receipt retains its admitted
attempt independently; worker installation, terminal polling, and finality
handoff all require target, pending owner, and receipt attempts to agree before
cursor advance. The same field remains lossless in the sidecar worker trace,
lane-application projection, and their two-phase equality kernel. This progress
argument remains conditional on publication of a ready exact receipt. The
current `ReplyWriterDeadlineModelObligation` proof script targets local actor
termination, and `ConditionalResponsiveWriterCursorLiveness` targets cursor
advance under `ResponsiveWriterReceiptAssumption`; both have source proof
bodies and are deductively classified support for the still-unpromoted
production progress refinement. Deriving that assumption from
`ResponsiveReplyWriterSpec` is the SANY-clean
`ResponsiveStrongFairnessToReceiptResidual` proof target. Its chain retains
outstanding ownership, applies weak fairness to reconnect and first dispatch,
then applies strong fairness to writer admission and publication across
fragmented eligibility intervals. It likewise has a source proof body and is
not an independent ledger row. A fresh strict TLAPS run over the final
dependency closure is still required; responsive TLC remains bounded evidence
rather than proof authority. The finite
`u8` attempt and saturating `Duration` make this a qualitative termination
statement, not a fixed operational wall-clock SLA: later exponential deadlines
can be extremely long.
A recovered responsive writer may still flush as soon as it is serviced,
before even a long current deadline. A Byzantine or permanently stalled target
can make only its own isolated deadline grow and cannot consume a sibling
target's reservation or cursor.

Certified merge-sidecar output retains a bounded, byte-free identity of the
current per-source chunk across response-byte release, route pruning, and
reconnect. Every actor-minted writer-flush identity also owns one process-local
application claim shared by all of its clones. Worker-to-lane handoff accepts
only an acknowledgement carrying that exact shared claim, not an independently
rebuilt identity with equal ticket and delivery fields. Its cross-tool evidence
binds the opaque source key, exact admitted delivery route, writer-claim
occurrence, and admitted timeout attempt. The three opaque identities are
process-local, and none of these fields is serialized, persisted, or admitted
to consensus state. The server validates the exact source, tenure occurrence,
ticket, timeout attempt, request, chunk hashes, fixed cursors, and any
still-materialized bytes before consuming that claim. A
duplicate or losing late receipt is therefore a terminal no-op, and a receipt
already applied to an expired rate-gate cannot advance a later byte-identical
rematerialization. A
genuine old-writer flush which has not yet been applied may still complete the
same source's retained current chunk once while a reconnect retries it; sibling
sources keep their independent cursors and reservations.

Response materialization is selected internally, not owned by the network
delivery which happened to arrive most recently. Before the shared lane queue,
Request and Close require requester/sender identity plus an active exact return
route; the authenticated relay carrying that route need not be a validator.
The serialized adapter then admits either a current frozen-roster requester or
an absent requester whose entry, finality, historical frozen roster, holder
selection, carrier tuple, and retained compact reference all match Kura.
For current roster size `N`, historical bound `H = 31`, per-requester gate
bound `S`, and authenticated reply-source bound `W`, the responder retains at
most `N + H` streams, `(N + H) * S` logical request gates, and
`(N + H) * S * W` route attempts. At most `H` retained stream identities may
be outside the current roster, so even a complete disjoint predecessor
committee cannot consume any of the `N` live-roster reservations. Older
archival demand beyond that bounded corridor fails closed; it cannot delay
current consensus. A same-roster restart may monotonically expand a durable
V3 lifecycle snapshot from the predecessor `N`-stream geometry to `N + H`;
it preserves the generation, streams, gates, and attempts and republishes the
expanded bound before returning. A shrink or any in-process geometry drift
still fails closed.

Historical serving does not require a locally retained block body. Before
finality publication or eviction, Kura extracts the exact compact merge
reference from the canonical body and stores it in the immutable retained
block record beside the canonical header and proposal/executed-wire hashes.
The combined reader revalidates finality and those wire bindings after
remote-only eviction. This witness is bounded Kura-local serving authority,
not an inclusion proof exported to consensus: every requester independently
matches the reference and certified entry to its own canonical carrier and
rejects a substituted or non-holder response.
Canonical version-2 retained records remain restart-readable. They expose no
merge witness because none was stored. If their exact body remains available,
the same persistence operation that precedes eviction replaces the legacy
record atomically with version 3 and verifies the replacement before eviction
may continue. A bodyless legacy record therefore fails historical merge
service closed instead of manufacturing authority. The version-3 byte ceiling
is the sum of the complete version-2 envelope, the independently bounded
reference, and explicit Norito option/struct framing headroom.
An unknown current-generation Close is acknowledged statelessly and consumes
none of those tables.

A durable two-level scheduler rotates first across semantic requesters and then
selects that requester's lowest stream/sequence/request identity. It persists
the requester cursor before granting one exact Kura lookup authority. The lane
must materialize the request and route returned by that scheduler, even when a
different request triggered the poll. One lookup's immutable bytes can satisfy
all pending live sources already attached to the same logical gate. Route loss
and outbound count/byte pressure clear only process-local authority and leave
the pending source retryable. Kura absence or read failure, request/reference
metadata mismatch, and a non-holder serving decision instead durably retire
that exact gate; a later authenticated replay may acquire a fresh bounded gate
if authoritative state has changed. Any other enqueue invariant failure is
fail-stop rather than silently reclassified as requester input.

Before any newly materialized chunk leaves lane work, its cursor and byte-free
pending identity are durably journaled. An inactive or reply-unwritable writer
first publishes its retained current-chunk cursor, then releases the ephemeral
outbound attempt, queue position, and shared response bytes. This distinction
matters after an exact writer deadline: inbound delivery authority may remain
active while the old writer is already draining and cannot accept output. The
gate remains byte-free and retryable for a freshly authenticated route.
Periodic lane service polls the same fair scheduler even without new ingress,
so progress does not depend on an adversary delivering another Request.

On receive, the three safety/high/low count shares are keyed by authenticated
`PeerId`, not by authenticated transport tenure. Retired-tenure dispatch workers
close their senders and drain already accepted reliable work before normal teardown.
A closed subscriber returns its actor-side pending safety/progress backlog to
the network owner and a replacement subscriber receives that backlog in
per-peer, per-class FIFO order. An item which already crossed into the old
subscriber's channel is not covered by that transfer; until an application
acknowledgement/in-flight ledger exists, its recovery depends on the durable
upstream retransmission source. The weak identity registry is bounded by the
configured total-connection geometry, prunes dead owners, and rejects a new
authenticated handoff if every owner slot remains live.

The Sumeragi exact-output corridor applies the same isolation above the P2P
actor. It preserves FIFO order for each target while round-robin service lets
later responsive targets and fan-outs proceed during another target's
backpressure. Completion and reducer work continue to run while such output is
pending. If the corridor cannot accept an exact fanout and returns
`SourceRetained`, the reducer keeps the retransmittable semantic source and an
active proposal keeps its producer fence; only exact service acceptance may
release that fence. A timeout therefore cannot prune a proposal merely because
its first fanout met bounded corridor pressure. The corridor freezes
`roster × {Safety, Lane, Bulk}` reservations for
the height, one `SidecarTopologyProgress` Lane reservation for topology-routed
Request/Close traffic, and one independent `SidecarReplyControl` Lane
reservation for exact-reply CloseAck/GenerationHint traffic at every frozen
target. It also adds a distinct non-zero shared fanout budget. A deterministic
maximum matching assigns at most one unique frozen target/class/kind
reservation to each retained fanout and is recomputed after every attempt.
Parked ordinary output for the same target and a saturated shared pool cannot
consume either progress reservation. Alternate authenticated routes for one
semantic Hint coalesce as attempts of the same reserved occurrence rather than
multiplying reservations. Non-roster reply identities and repeated output for
an already-owned target/class/kind are confined to shared capacity, while
partial multi-target completion reopens the exact finished reservation. This
prevents authenticated observer identity churn from filling validator
safety/lane reserves. Once Kura has returned the exact applied-height receipt
and matching
finality artifact, the runner applies one atomic preflight to every retained
fan-out. It rechecks the pinned hash of every network message and accepts only a
typed claim created in the exact Decision context and height. A historical
CommitQC response is a singleton target-bound claim whose source height,
context, responder, certificate, response hash, signature, and chain are
revalidated against an independently reread Kura finality artifact. A
historical certified-body response likewise rereads the source finality
artifact and canonical Kura block, then rechecks its historical responder,
signature, subject, body bytes, payload hash, manifest, target, and response
hash. A historical lane-certificate response rereads the exact certified Kura
lane artifact and requires the same lane/height, proposal, PrepareQC, CommitQC,
target, and certificate hash. Cached claims alone are never sufficient.

For current-height output, global V2 traffic validates its protocol version and
binds to the exact finality artifact. Winning lane output requires its exact
durable Kura certificate and application receipt; an alternate vote, QC, or
certificate proof variant is revalidated against the durable winning proposal
and proof material. A structurally valid same-height lane output for a
non-winning proposal is explicitly superseded by the finality authority rather
than described as reconstructible winning output. The winning set is derived
from the canonical finalized block's lane ownerships, never from the volatile
output queue. An empty canonical ownership set is already complete, including
for a result-bearing genesis block with external entries. Missing lane
certificate or receipt evidence does not keep the finalized global height
active. Rollover first canonicalizes every bounded unfinished winner against
Kura, rejects conflicting quorum evidence, prunes proposal-only losers, and
moves the exact remaining session cache, signing locks, autonomous payload and
NewView cursors, historical recovery ownership, and retry cursors into the one
immediate successor. Completed certificates keep their Kura-first source;
unfinished certificates keep one move-only volatile owner. A block synchronized
after adapter construction therefore retains its exact proposal as the request
source for a peer's durable lane certificate while global successor progress
continues. Duplicate, conflicting, unbounded, or non-canonical evidence fails
closed. Native AMX
output binds the creation scope, embedded consensus round, and exact message
hash. A merge share binds its creation scope and exact share hash. Certified
sidecar requests and chunks bind creation scope, exact target and
requester/responder roles,
complete transfer identity, and exact request or response hash. Before the
handoff, finalized-sidecar pruning leaves winning bytes reconstructible from
the globally committed merge log and explicitly supersedes losing pending
sidecar work. Any failed check retains the complete fan-out. Manual claims,
wrong identities, payload substitutions, and otherwise untyped `Exact` output
remain owned and fail closed. This rollover seam prevents a dead target holding
typed, reconstructible or explicitly superseded applied-height output from
blocking successor activation; it does not stand in for the still-required
four-validator QC-to-body-to-application and catch-up regression.

The final rollover pass then seals the empty worker corridor under the same
mutex used by every exact-output enqueue. The one-shot
`DurableExactOutputHandoffReceipt` binds the exact finality artifact,
predecessor context, and a private process-local owner nonce shared only with
that height's retained merge-sidecar transport. A byte-identical receipt from
another service cannot authorize it, no output can enter after the seal, and
finalized cleanup without the seal latches restart. The lane adapter consumes
both its transport endpoint and that move-only receipt only after every
committed lane output and undispatched effect has crossed the worker boundary.
It then binds the retained transport to one exact immediate successor context.
An error at any of these post-finality checks is fail-stop.

A successor adapter treats every moved earlier-height proposal, vote, QC,
certificate, executable payload, and NewView artifact as historical work. Each
ingress and retransmission rechecks the embedded predecessor height, frozen
committee, canonical Kura carrier, exact body, and durable cursor; it cannot
borrow the successor context's committee or carrier. A configured validator
removed from the successor global roster runs global consensus as an observer,
but retains lane authority only where an exact frozen descriptor names its key.
That descriptor may belong to moved historical work or to an independently
pinned current-height Nexus lane committee, which need not be a subset of the
successor global roster. Authenticated non-roster ingress is bounded
separately, so old committee members can finish their exact obligation without
acquiring successor-global voting power. Once historical recovery is durable,
exact request handling uses the separately typed Kura-backed response claims
described above.

Startup first completes any interrupted global application at the durable tip.
After that strict boundary it activates the verified successor and hydrates
bounded unfinished canonical lane evidence from Kura as historical work. It
does not require the globally applied tip to wait for every lane certificate
or application receipt. Historical artifacts are admitted only through the
exact route/incarnation and canonical-carrier checks, so a later lifecycle
cannot revive a retired lane generation.

Broadcast and relay ownership are deliberately not claimed complete. A reliable
broadcast snapshots the actor-accepted relay-aware topology and attempts each
`(target, class)` lane independently, so one occupied target cannot retain a
class-wide parent or suppress admission to responsive targets. A target child
coalesces only with the identical canonical request during the same topology
membership tenure. Distinct payloads and direct/broadcast cross-kind collisions
retain exact per-target caller ownership and FIFO tickets. Removing a target
cancels only that old broadcast tenure; re-adding the same peer creates a new
generation, while direct-post ownership survives the topology transition. This
closes the known local parent-residual obstruction, but true reliable-target
geometry exhaustion, direct-post ownership for a removed target, a lost
second-hop hub forward, remote consumption, and final application
acknowledgement remain open production-refinement obligations. Local
writer-flush and admission-rank tests cannot promote end-to-end delivery or
broadcast starvation freedom.

QUIC applies the same checked process geometry before endpoint construction.
In addition to active send/receive flow-control credit and configured datagram
buffers, every admitted pending `Incoming` owns one 64 KiB post-first-packet
buffer and a separate 64 KiB reserve for the first packet Quinn retains outside
that buffer. Both regions are multiplied by the checked incoming-connection
cap and included in the combined endpoint byte bound; fixed Quinn object and
allocator metadata remains count-bounded by that cap.

## Liveness snapshot

`GET /v1/sumeragi/status` and `/v1/sumeragi/status/sse` expose the same required
`liveness` object in the canonical `SumeragiV2Status` payload. It contains:

- the reducer generation and exact Prepare, Commit, and timeout partial pools,
  including distinct signer count plus signed/total unit-vote projections that
  validation requires to equal that count and the frozen roster length;
- durable outbound proposal, vote, QC, timeout-vote, and TC intents, with their
  persistence, signature, queue, or sent stage;
- candidate, body recovery/store, validation, application, and successor
  activation work. Candidate work reflects an actually owned candidate load or
  artifact and is never inferred merely from the local validator being leader;
- retained semantic-admission occupancy and, separately, the live bounded
  transport-to-runner `network_ingress`, adapter, and runtime queue depth,
  capacity, oldest age, and service debt. Semantic-admission age is diagnostic
  history and is not treated as scheduler debt. Queue age remains diagnostic
  context and never proves scheduler starvation by itself. Queue-based
  starvation requires eligible-skip debt covering a complete three-class
  service rotation. Network ingress publishes zero synthetic debt and instead
  maintains a private service clock: while the queue is non-empty, a completed
  admission scan resets that clock even if every item remains blocked. Only a
  watchdog interval with no scan classifies network-ingress starvation;
- the bounded worker-to-executor `EffectCompletion` handoff, including its
  depth, capacity, oldest retained age, and service debt, so completed local I/O
  cannot remain invisible behind a full runtime completion lane;
- the bounded reducer-to-executor `EffectDispatch` causal suffix, including its
  depth, fixed source bound, and oldest retained age. This reserved FIFO is
  attempted before another runtime transition and therefore publishes zero
  scheduler-skip debt; ordinary pending-work exhaustion is diagnosed through
  the exact body, validation, application, or local-control owner instead. A
  full-capacity body Fetch remains reducer-owned reconstruction debt rather than
  entering this FIFO, so an unresponsive Byzantine body source cannot sit ahead
  of a durable signing intent. Certified-request-only pressure is different: it
  retains the exact Fetch causal suffix in this FIFO, while authenticated
  transport completions remain admissible to release the blocking request;
- the last semantic transition, its age, the height no-progress age, and every
  reducer ignore-reason count.

The watchdog deadline is derived from the configured, view-aware round timeout
plus one retransmission interval. Its height-wide progress rank is bounded by
the greatest semantic stage reached and the maximum Prepare/Commit signer count
observed at that height. View and reducer generation are not
rank components. Timeout-vote admission, TC installation, and reconstruction of
the same locked Commit pool update current diagnostics but cannot reset the
height-progress clock; repeated view/generation cycles are not treated as
height progress. After the deadline, a snapshot classifies the current delay as
exactly one of:

- `missing_proposal`
- `body_unavailable`
- `prepare_quorum_missing`
- `commit_quorum_missing`
- `timeout_certificate_missing`
- `scheduler_starvation`
- `application_pending`
- `successor_activation_pending`
- `local_control_pending`

`successor_activation_pending` identifies a durably applied predecessor whose
verified successor construction, service startup, or authenticated handoff has
not completed.

`local_control_pending` distinguishes a reducer blocked on safety-WAL
persistence or consensus signing from scheduler starvation. Queued outbound
work and formed-but-unconsumed QC or TC transitions remain structural scheduler
witnesses even when no queue-debt sample is available.

Merge-sidecar ownership is classified by its actual consumer. Validation work
waiting for the exact sidecar remains validation/body-unavailable work, whereas
a durable `Apply` waiting for that sidecar is `application_pending`; the
aggregate retained-work counter is not used to collapse these two stages.

Successor activation is likewise tied to runner ownership rather than inferred
from application alone. Application and global finality are necessary but not
sufficient when the canonical block owns lane payloads: the Kura-first lane
completion gate above must also be complete. A block with no lane ownerships
crosses that gate without synthesizing lane evidence. Once application,
finality, and any non-empty lane-completion gate are complete, the runner
publishes successor construction as `Running` before building the verified
context. The successor adapter defers its initial status while the runtime,
services, lane work, and startup effects are still fallible. Only after the
successor clocks are armed and authenticated ingress is open does the runner
publish the predecessor handoff as `Complete` and install the successor status
with `SuccessorHeightActivated` bound to the successor's own height,
generation, context, and view. The one-shot activation token also carries the
exact verified successor `HeightContextId`; a same-height snapshot for another
chain/roster/quorum/seed context is rejected without mutating the applied
predecessor. A constructor or startup failure therefore leaves `Running`
visible at the finalized height. For a valid checked Running-to-failure
projection, the local snapshot latches `restart_required` before the runner
exits without claiming activation. If a corrupted local snapshot makes that
projection invalid, the checked gate rejects it without mutation; the runner
has already latched the process-wide output guard, and the public status
overlay still reports `restart_required` while preserving the rejected local
evidence for diagnosis. The
predecessor progress clock retains its owner-bound watchdog deadline while the
successor effect executor is being built. An aged `Running` handoff therefore
remains classified as `application_pending`; successor-owned effect,
completion, and ingress overlays cannot erase that classification or be
attributed to the predecessor. The predecessor must carry an exact `Applied`
body/application marker before either
handoff transition is accepted. Effect, completion, and network-ingress
overlays are tagged by height and context, so fallible successor startup cannot
be attributed to the still-visible predecessor. Complete-tip and audited
snapshot recovery reconstruct the same deferred activation token from their
authenticated durable parent; they do not publish `RecoveryReplayed` as a
substitute for a live successor. The adapter itself owns the activation marker,
so a later ignored/retransmit publication cannot erase it. The marker starts
the new height at progress rank zero and cannot mask a later missing-proposal
stall.

Recovered CompleteTip has an additional disk-ownership prerequisite. Its
retired predecessor authority must first bind the exact unlaunched H+1
lifecycle owner. Authenticated height recovery is also the sole source of its
move-only storage authority: the capability fixes the live Kura instance,
verified context, context-addressed lifecycle/body roots, and exact body
signature policy. Owner construction cannot accept those components
independently, and launch rechecks the same process-local Kura identity before
moving either adapter or body ownership. That bound join can enter the existing
owner-to-runtime, executor, and I/O launch only through a consuming transition whose opaque
result retains the running H+1 stack together with the retired-H authority. It
has no generic owner, parts, status, or activation accessor. The serialized
runner still must consume this complete wrapper in the one-shot activation
transaction above; until then, the recovered CompleteTip path remains
fail-closed before status publication.

Asynchronous work from the predecessor can still arrive after this handoff, as
can work tagged for a non-current view or an older reducer generation. The
reducer and its production refinement gates accept such wrong-height,
wrong-view, or stale-generation events only as canonical exact stutters: they
claim and grant no boundary, emit no effect, leave every durable, pending,
volatile, application, and owner-tag observation unchanged, and retain the
event's typed ignore reason. These zero-boundary, zero-effect stutters never own
current progress, reset a progress deadline, or authorize persistence, signing,
body-pipeline, application, or successor-activation work. A mismatched-tag event
which attempts any boundary, effect, or state change still fails closed.

Late `Persisted` and `PersistenceFailed` events keep their nonzero WAL
identifier as typed completion payload while obeying the same exact-stutter
rule. A stale lifecycle tag cannot consume or alter a current pending record;
with a current tag and no pending record, the completion is likewise an exact
`NoMatchingWork` stutter. With a current tag and a pending record, however, an
identifier mismatch remains a fatal persistence-acknowledgement error.

The classification is diagnostic. It does not weaken reducer safety checks or
manufacture a progress event.

The serialized production runner also polls an edge-triggered operator
watchdog on every scheduling turn, including retry paths. It rebuilds the full
live overlay only when the retained semantic deadline is due or the published
height owner changes. The first classified blocker and each later blocker
change emit one structured warning with height, context, view, generation,
leader, exact relevant quorum, oldest/debted queue, and local-work context; an
unchanged blocker is not repeated. A recovery notice requires a strict gain in
the height-wide semantic high-water, so a fresh view or repeated reconstruction
alone cannot claim recovery. Successor activation and explicit status clear
reset the edge state instead of attributing the predecessor's alert to another
height.

## Implementation evidence and pending inventory

The following focused checks were recorded through 2026-07-18:

- all 248 `iroha_p2p` library tests, including bounded plaintext retention,
  cancellation-safe flush, read/write arbitration, and direct-post exhaustion;
- all 19 fair outer-ingress tests;
- three exact locked-Commit generation/readmission tests, one exact WAL replay
  witness test, and one locked-body leader-crash recovery test;
- terminal Decision cleanup across executor, runtime, runner, lane, candidate,
  outbound, and completion ownership, plus a real nonzero-view WAL/body-store
  proposal replay through production signing and Prepare continuation;
- all 55 serial `sumeragi::v2_lane_work::tests`, including the production
  Native-AMX committee, PrepareQC-retention, Decision cleanup, and lane-session
  ownership boundaries;
- the status contract accepting all 12 distinct ignore reasons and rejecting a
  thirteenth entry, with tag-11 `unsafe_proposal` parity in the maintained SDKs;
- a fresh exact four-validator genesis run with networking required and one
  permitted startup attempt, which committed and activated the successor height
  on all validators in 458.47 seconds including the separate source-bound
  daemon build;
- the then-current formal-ledger checker suite and clean SANY
  syntax/semantic analysis;
- one exact, no-retry four-validator genesis attempt using a freshly built
  `iroha3d`, which committed on every validator in 59.09 seconds. The daemon's
  SHA-256 was
  `0bff8b990a6f653b69b26b039ac26a4abdcba6cbb8f85c9fef252b81cdbab0df`;
- an exact-evidence four-validator genesis reproduction which passed its first
  attempt in 351.76 seconds, with retained logs under
  `target/sumeragi-v2-genesis-recheck-exact-evidence-20260715/irohad_test_network_3XUobQ`;
- the complete eleven production-liveness modules at 56/56 reducer/core,
  10/10 refinement, 9/9 reducer source-link, 55/55 adapter, 26/26 apply,
  115/115 effects, 55/55 lane work, 38/38 runtime, 19/19 runner, 66/66 worker,
  and 14/14 watchdog tests (463 passed, 0 failed, 0 ignored). The broader
  serialized Sumeragi v2 namespace passed 582/582 tests, and release discovery
  confirmed all 124 explicitly required production tests are present and
  non-ignored.

Those retained serial counts predate the native-AMX, successor-activation,
source-linked body-kernel, ingress-ownership, transport, and active-watchdog
additions. The preceding source-bound inventory contained 406 exact tests
across 24 modules, including the authoritative outer ingress, merge sidecar,
historical block sync, Kura progress-witness durability, P2P actor/writer
ownership, daemon route-control and Hold/Release bridges, and watchdog
modules. The geometry closure added 14 names and two `iroha_config` modules
without retiring a name, producing the historical 420-test checkpoint across
26 modules. The route-lifecycle closure added three P2P names without
retiring a name, producing the historical 423-test checkpoint across the same
26 modules. They
bind a delivery capability to its original minting tenure after bounded
retired-source tombstone churn, reject a second rehydrated route rather than
overwriting the first capability, and prove normal-exit/`Drop` teardown retires
every actor-owned route and cancels only that actor's waiters. Mechanical
source-to-inventory reconciliation now adds 16 distinct, non-ignored tests and
three owning modules, producing 439 exact tests across 29 modules. The delta
pins Kura replay-metadata preflight; five successor/durable-intent refinement
projections; discovered-CommitQC reducer admission; successor recovery,
runner, and watchdog failure boundaries; two P2P source/waiter geometry
modules; and four daemon genesis producer/fanout geometry boundaries. The
renamed lane-relay saturation regression replaces its obsolete
`saturated_relay_owner_returns_sixty_fifth_exact_envelope` inventory entry with
`saturated_relay_owner_returns_sixty_fifth_without_actor_ticket`; it does not
increase that module's cardinality. A follow-up sidecar seam audit adds
`later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit`,
bringing that inventory to 440 tests while retaining the same 29
modules and 52 corridor legs. It proves that a same-tenure route update cannot
emit a second concurrent copy of a chunk which still awaits its exact writer-
flush receipt. Three worker regressions then produce the historical 443-test
checkpoint without changing the module or corridor geometry. They prove that a
same-tenure replay cannot cross actor admission again while its writer flush
is pending or flushed-but-unapplied, that a terminal source remains a
zero-reservation route-history member while two live sibling sources progress,
and that a same-source reconnect preserves either the unflushed current item or
the terminal cursor without acquiring another reservation. A closed writer
never advances the cursor, so its replacement tenure retries the retained
current item through the ordinary route update. A successful writer flush is
terminal and cannot be reinterpreted as a newly materialized cursor zero, even
when another source keeps the fanout live and the shared corridor is full.
The source-authority, immutable-sidecar, runner-race, daemon-corridor,
shared-byte-budget, cached-Arc-admission, and executable-refinement closure adds 22 exact
regressions. It also moves two peer tests to their actual shared-byte-budget
module, yielding the 465-test checkpoint across 30 modules and 53 pre-network
legs. The authenticated-non-validator source-cap regression and the
alternate-route-before-lane-cap regression add two exact names without adding
a module or corridor leg, yielding the 467-test checkpoint. Three daemon
Hold/Release controller regressions, one layered daemon ownership regression,
and two root configuration geometry regressions add six exact names, yielding
the 473-test checkpoint without changing the module or corridor-leg counts. One
configuration fingerprint, two historical-recovery kernel, and one shared
authenticated source-credit regression add four exact names, yielding the
historical 477-test checkpoint with the same module and corridor-leg counts.
Relative
to the preceding 298-name inventory, the 406-name closure's 110 additions and two
retired names produced the exact net increase of 108. The current additions
pin exact-envelope deferred service, semantic-origin separation, source-
swapped response/chunk rejection, alternate-source runtime and orphan-chunk
ownership, actor-global ordinals across tenures, and checked source/capacity
geometry. Together, the route closures pin semantic request coalescing,
per-source route ownership and cursor progress.
The proposal-origin, multi-carrier, and persistence-failure closure adds 41
exact regressions and retires nine superseded selectors, producing the
509-test, 38-module, 61-leg checkpoint. The final successor/recovery closure
adds six exact regressions without adding a module. Six source-sealed format,
legacy-codec, build, Clippy, workspace-test, and daemon-test legs plus the
G-SCALE tooling preflight produced the historical 515-test checkpoint. The
crash-safe response handoff and same-delivery capacity-retry regressions add
two sidecar cases. The per-source route-attempt, exact PrepareQC recovery,
locked-body reproposal, runner/worker, sidecar, and daemon closure, plus the
certified sidecar control-bucket regression, produced the 585-test checkpoint.
The unsent-request restoration and fairness-cursor retry regressions produced
the 588-test checkpoint. The durable semantic-peer-history regression produced
the 589-test checkpoint. Mechanical source-to-inventory reconciliation then
adds 115 net regressions: 3 authoritative-ingress, 57 merge-sidecar, 17
lane-work, 3 runner, 17 worker, and 18 P2P-network tests; the daemon
network-relay rename is cardinality neutral.
The routed-Hint and crash-safe V3 lifecycle closure adds 26 exact regressions
and retires eight obsolete route-free/V2 selectors. Rejecting replay of a
proposal superseded by a same-round lock adds one exact reducer regression.
Preserving that proposal's tag, round, and subject through runner startup adds
one exact runner regression. Two cross-platform lifecycle V3 crash regressions
cover state replacement before directory sync and root replacement before
predecessor cleanup. Rejecting a matching CommitQC from a foreign height
context before Apply schedules work adds one exact `v2_effects` regression.
Five exact-Serve lifecycle regressions cover Pending/Reserved rollback,
shutdown rollback, route-neutral tombstone replay, and cached replay after the
singular future-slot barrier, producing the 738-test checkpoint. Another 35
exact CertifiedServe ingress and worker regressions bind gate ordering,
immutable admission ordinals, frozen predecessors, coalesced retries, durable
restart, terminal replay, owner replacement, and anti-resurrection behavior.
One four-peer leader-wire lifecycle-store regression binds the full
origin/phase/chunk slot product and restart-stable terminal coalescing,
producing the 774-test checkpoint. Eight runtime/effect/runner regressions bind
Decision/lock retirement of orphaned leader-wire owners, same-turn terminal
consumption across live and recovery capacity retries, and fail-closed
authenticated semantic-only Coalesce defense.
Subsequent source reconciliation and exact-ingress lifecycle, restart,
provenance, and invalid-QC quarantine regressions bind one actor-global
logical owner across its physical retries.
Seven physical-cut, adapter-capability, aggregate-rebase, and ineligible-driver
regressions produce the 813-test checkpoint. Five admission/coalescing, Busy
pre-runtime ownership, and reconstructed-chunk terminality regressions bring
the 818-test checkpoint. Thirteen exact admission, retry, tombstone, and
high-water regressions bring the inventory to the 831-test checkpoint. Retiring
five obsolete peer-genesis protocol regressions brings the 826-test checkpoint.
Replacing one obsolete restart selector with its two raw/coalesced
crash boundaries and restoring two implemented certified-ingress regressions
brings the inventory to the 829-test checkpoint. Autonomous-lifecycle
terminal-outcome and startup-recovery coverage plus final source reconciliation
bring the 837-test, 39-module checkpoint. Ten deterministic network simulations
bring the 847-test, 40-module checkpoint. The source-bound terminal-sweep
partition regression brings the source-bound inventory to the 848-test
checkpoint. The late passive-Fetch completion and one-shot completed-Serve
reopening regressions bring the 850-test checkpoint. Seven Native AMX finality-
bound merge-projection regressions bring the 857-test, 40-module checkpoint.
Three Kura recovery regressions and the governance-unlock audit bring the 861-
test, 41-module checkpoint. The production-adapter activation guard and two
deferred-canonical-carrier completion regressions produced the historical
864-test, 41-module checkpoint. Retiring the duplicate inline network-simulation
rows brings the current
source-bound inventory to 855 exact tests across 40 modules and 88 pre-network
legs.
The exact Apply regression also drains the typed Kura completion and verifies
that its immutable finality artifact and original reducer tag absorb a later
identical periodic rediscovery even after live tag authority is relinquished,
without allocating a new work ID; tag drift or a conflicting post-completion
certificate still fails closed. This extends an existing named regression and
therefore does not change the inventory cardinality.
Its canonical module/test TSV inventory SHA-256 is
`a40a9d7ef0dafcad2a6e3eb710d550a7f80f905c378117ef9a52b39a86d77b1e`.
Nine of those legs execute the separate 525-test G-UNIT focus inventory. Its
canonical source-derived inventory contains 526 TSV lines and has SHA-256
`dc428b5bb9054495ef88aacd5b07a0f932ba2ada9da0c015dc45f36edbdf1352`.
The 319-test core group includes grouped Native prevote-budget rejection before
Kura/WSV mutation, historical source-bundle authentication, crash-safe latest-
index and prune-V2 recovery, cross-route manifest-barrier isolation, durable
Native signing-boundary drift rejection, atomic grouped reservation commit,
exact QueuePlan obligation authentication, ApplyCarrier authorization, and
canonical historical autonomous recovery into exactly-once merge application.
It also binds the borrowed exact finalized carrier hash representation, the
autonomous pristine and exact-height/empty post-block/pre-vote carrier surfaces,
and rejection of event-surface drift both before publication and at finality.
This source-derived inventory does not claim execution evidence.
Together, the closures bind proposal-origin reducer/deferred identity,
equivocation evidence, aggregate signatures, finality/header geometry, compact
offline QCs, parent height-context identity, source-scoped sidecar limits,
worker-to-network chunk-admission receipts, runner route
preservation, worker backpressure, actor-global deferred capabilities,
scheduler ownership handoff, opaque delivery ordinals, and daemon Hold/Release
failure behavior.

The first-release queue recovery order installs the lane-reservation journal
before the pending QueuePlan journal. A replayed reservation Commit therefore
authenticates its exact global admission binding from the still-live V4 plan
record, durably tombstones that exact record, and only then forgets the Commit
barrier. The semantic request identity is reconstructed from the durable chain
digest and transaction entrypoint by one pure kernel at construction, journal
replay, certificate validation, core admission, and Torii. The reservation
commit additionally binds the compatibility queue hash, exact routing-plan
digest, canonical binding hash, coordinator leg, and coordinator-lane
incarnation. Reservation scope validation uses the same consensus route
geometry, including only the canonical `SINGLE`/`UNIVERSAL` route when Nexus
is disabled; an arbitrary dataspace cannot inherit that authority. A stale or
noncanonical identity, an ordinary non-global record,
retargeting, and a same-plan ABA replacement all fail closed without appending
either the plan tombstone or `ForgetCommit`. A successful reconciliation
republishes the terminal queue-pressure snapshot so restored ownership cannot
leave phantom backpressure. Startup validates every restored Commit barrier
against one content-bound replay and writes all matching tombstones in one
atomic `RemoveBatch` frame, so the number of journal scans is constant rather
than proportional to the barrier count. Exact prior tombstones make a crash
before `ForgetCommit` idempotent; a missing, duplicate, mismatched, torn, or
ABA-replaced member rejects the batch without a partial logical removal. There
is no journal-disabled configuration or completion path: production startup
always installs the QueuePlan journal and cannot discard source-bound pending
ownership.

The rollover tests cover
historical Kura CommitQC, body, and lane-certificate rereads; current global
V2; and lane proof/supersession, Native AMX, merge-share, certified-sidecar,
and untyped fail-closed boundaries. The network tests distinguish identical-
delivery deduplication from later same-source delivery, reconnect, and a newly
observed alternate authenticated source for the same semantic request. An
explicit proposal-origin slice binds reducer and deferred identities,
equivocation evidence, aggregate signatures, finality/header geometry, compact
offline QCs, and parent height-context identity to the authenticated origin.
The genesis header-binding case additionally accepts canonical view-zero bytes
whose first proposal origin is later; the regression's whole-item token
SHA-256 is
`bfbd01d093f38fa8c96fb17fe38b6ec1132e6ffbb0d09367a298299394bdce4f`.
The integration restart regression fixes the cadence-derived view-zero deadline
at a contention-tolerant 20 seconds; its whole-item token SHA-256 is
`13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f`.
The same four-validator restart scenario covers sequential missing-height
discovery and catch-up. A diagnostic run showed that the restarted validator
paid that entire 20-second interval once per missing height; the
recovery-scoped eager-discovery correction makes those recovery attempts
eligible without the repeated wait while retaining the ordinary-height
deadline. The fresh exact run of
`sumeragi_v2_runner::authoritative_v2_finalizes_through_validator_restart`
passed 1/1 in 79.82 seconds; all four peers shut down gracefully with empty
stderr. This is focused network regression evidence and does not promote a
formal obligation or complete the broader release corridor.
The successor-boundary regression preserves the predecessor CommitQC context
through wire-to-core conversion; its whole-item token SHA-256 is
`ee773b00e696822c6d2ba998fb88201bb6e2a06eac749a2c700edec70dbbdf74`.
The extended cryptographic-parent regression now carries the same certificate
through authenticated admission and is sealed at
`1cb4736b2e4b499403c870cc3dd5ab8ccd361d51887efad4178ed7d39a9e0225`.
An inactive or reply-unwritable sidecar source first durably publishes its
incomplete chunk cursor, then releases its process-local outbound attempt,
queue position, and shared response bytes. A delayed reconnect therefore
creates a freshly authenticated route and rematerializes bytes at the retained
cursor rather than restarting at chunk zero; a newly authenticated alternate
source still starts at chunk zero. The journaled logical gate and cursor remain
bounded and retryable without letting a dead or draining writer pin
output-byte capacity. Completed attempts remain terminal until an authenticated
cumulative Close retires their contiguous semantic prefix.
The requester sequence/floor state and responder gate, source-budget, cursor,
and pending-chunk identities are atomically journaled under the authenticated
Kura root. The journal wraps that canonical Norito payload in its exact typed
hash and rejects a stale digest before interpreting any recovered floor or
cursor. Restart rebinds only a freshly authenticated process-local route; it
never reconstructs a capability from disk. A lifecycle-journal failure
latches the process-wide output guard before an allocated request, queued
chunk, timeout rotation, Close, or CloseAck can publish later consensus output.
`next_stream_epoch` advances with checked arithmetic and is persisted before a
requester uses the allocated epoch. Completion and restart never make an epoch
eligible for reuse; exhaustion returns `Capacity` without publishing work.
Wire protocol version 1 is updated in place for the first release. Every
canonical request identity binds that version, the responder's durable
`NonZeroU64` `service_generation`, the requester's `NonZeroU64`
`stream_epoch`, the per-stream `NonZeroU64` `semantic_sequence`, the immutable
payload or reference, and the requester/responder peers. It excludes only the
cumulative `closed_through` floor. A non-regressing floor on the same
occurrence therefore keeps the same request identity and advances without
rematerializing its response. Generation, epoch, and sequence form a strict
lexicographic order: a cumulative close cancels queued and actor-admitted
occurrences only when that ordered prefix covers them, so a late writer receipt
from an older generation or epoch cannot advance its successor.
For one requester, the prefix covers every older generation, every older epoch
within the same generation, and sequences through the floor only within the
exact same generation and epoch. The applicable coordinates are retained by
Request, Close, CloseAck, Chunk, attempts, gates, flush identities, trace
projections, and refinement state.

A lower-generation, otherwise canonical Request or Close is a stateless
authenticated probe. The responder returns an exact canonical
`GenerationHint`, bound to the observed and current generations and the exact
triggering message hash, without allocating stream, gate, cursor, or other
server-lifecycle state and without rewriting the lifecycle journal. An
unadvertised future generation is rejected without a Hint or server-state
mutation. Like CloseAck and Chunk, the Hint retains the exact authenticated
reply route of its triggering Request or Close. Lane, runner, worker, daemon,
topic selection, and exact-output ownership preserve that capability without
topology fallback. A duplicate semantic Hint coalesces alternate authenticated
sources as independent attempts, and a same-source refresh cannot erase a
sibling. Its dedicated `SidecarReplyControl` Lane reservation remains
available even when ordinary output for the same target is parked and shared
capacity is full. The requester
accepts a Hint only from the expected responder when it names the exact hash of
an outstanding Request or Close and strictly advances the current generation.
It first persists that generation and a fresh stream epoch. Only after that
durability barrier succeeds may it discard old partial chunks and reschedule
the affected work. A failed Hint write retains the old occurrence and latches
restart before any further output drains.

The canonical mutation runner checks that lifecycle twice. The route trace
exhausts 7 generated and 7 distinct states at depth 7. Its pipeline companion
threads the old and successor identities through enqueue, durable reset, and
stale-flush rejection, exhausting 11 generated and 10 distinct states at depth
10. The capacity-overflow trace checks that active ownership survives rejected
nonterminal compaction, requester-epoch exhaustion, and responder-generation
exhaustion; it exhausts 5 generated and 5 distinct states at depth 5. Its
pipeline companion retains the source-owned pending attachment and queued item,
exhausting 8 generated and 7 distinct states at depth 7. These are bounded
regression results, not deductive promotion.

The asynchronous product boundary is structurally machine checked. Pinned
strict TLAPS proves 54/54 obligations showing that every reply branch refines
the complete V2 action, both interleaving brackets project, and the composed
spec projects to both `AsyncSpecAt` and `ReplyRouteV2Spec`. The separately named
V2 inductive-safety, successor-isolation, and temporal-product operators remain
unproved; the structural result does not imply network or consensus liveness.

The responder owns one bounded unified `server_streams` table and one bounded
request-gate table; attempts are bounded within their gates. Equal-roster
rehydration always preserves responder ownership and generation, including a
retained current chunk; a new same-roster requester against a full table
receives fail-atomic `Capacity`. Ordinary changed-roster transition requires
authenticated terminality for every predecessor stream, gate, transfer, and
flush. Two private durable paths may instead fence active predecessor responder
state: a move-only `DurableMergeSidecarRolloverAuthority` obtained after the
exact-output corridor is durably sealed, or a restart-only fence minted after a
V3 snapshot passes complete semantic restoration. Both require a lifecycle
journal and authorize only a certified changed-roster geometry. Gate pressure,
unauthorized active-state replacement, and generation-counter overflow return
`Capacity` atomically without advancing a floor, clearing a table, or emitting
a Hint.

Every authorized changed-roster transition checks the successor generation,
constructs the complete empty responder projection, and commits it before
changing memory or emitting `GenerationHint` on the triggering authenticated
reply route. The force-fenced paths retire predecessor streams, gates, outbound
state, and process-local closure-handoff debt, but do not synthesize
requester-authenticated close prefixes for sequences that were never closed.
The sole durable schema is `MergeSidecarLifecycleSnapshotV3`: its
integrity-bound canonical Norito payload contains geometry,
`next_stream_epoch`, responder generation, requester streams, unified server
streams, request gates, and the journal root generation.

The root is first atomically published and fsynced as a generation-zero
bootstrap sentinel with no snapshot hash, before the state directory is
created; the first state candidate is generation one. If that first state
survives beside the sentinel, startup semantically validates it and rechecks
the live pair before allowing the root to adopt it. Later commits write and
fsync the inactive alternating slot before atomically replacing and fsyncing
the root marker, so the root selects the predecessor before publication and
the successor afterward. Startup validates the complete selected candidate,
including bounds, uniqueness, monotonic floors, gate/stream correspondence,
and pending-chunk identity; rechecks the live pair; and validates known temp
artifact types before deleting a temp or unselected slot. Unknown or
non-regular artifacts fail closed. Before committed recovery deletes any
predecessor or inactive artifact, it re-syncs both the selected state directory
and the root-marker directory, so a second crash cannot retire the only
rollback-safe copy before the preceding replacements are durable. Filesystem
aliases, including Windows reparse-point files or directories, fail closed.
State and root replacement are native atomic replacements on Unix and Windows,
while platforms without durable directory synchronization fail closed. V1/V2,
corrupt, and unknown bytes are unsupported; there is no migration.

The root marker is the local trust anchor, not an external monotonic counter.
Replacing or rolling it back—including restoring the bootstrap sentinel—or
rolling back the whole store/root pair is outside this rollback guarantee.
Compaction does not weaken response authority: live requests still require the
current exact context, and historical serving independently rereads canonical
Kura data and matching finality before returning a sidecar.

The required adversarial coverage spans nonzero generation/epoch/sequence
roundtrips, checked epoch allocation across restart, stale and future messages,
forged or uncorrelated Hints, monotonic piggybacked close floors without
rematerialization, generation and epoch overflow, crash-before/after
persistence, malformed or legacy snapshots, and fail-atomic capacity
rejection. It also requires stale chunks, CloseAcks, flush receipts, and
cancellation prefixes to be unable to affect successor generations or epochs,
plus Norito roundtrips and topic assertions for all five sidecar variants.
These are release gates, not a claim that the current Rust source has completed
the corresponding test runs.

The exact writer boundary is pinned separately by
`ready_exact_reply_flush_wins_route_retirement`,
`ready_exact_reply_flush_wins_connection_replacement`,
`nonready_exact_reply_ack_cannot_keep_stale_route_alive`,
`adaptive_reply_attempt_flushes_between_base_and_doubled_deadline`,
`full_exact_writer_queue_times_out_closes_route_and_releases_actor_budget`,
`topology_writer_full_retry_does_not_acquire_exact_reply_deadline`, and
`stale_reply_writer_deadline_does_not_terminate_replacement`.
The retained runner seam explicitly inventories
`runner_dispatch_preserves_durable_lane_certificate_reply_routes`,
`runner_dispatch_preserves_certified_sidecar_chunk_reply_routes`,
`runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route`, and
`runner_dispatch_rejects_durable_response_without_reply_routes`. The sidecar
boundary retains the fixed four-gate/two-session/16-MiB source contract plus
the same-hub fifth-gate, third-session, and byte-overflow rejections while an
independent authenticated hub progresses. These names were already present at
the 423-test checkpoint and remain exact release-contract entries. The durable
requester/responder restart, cross-height source-cursor, and lifecycle
fail-stop regressions are pinned alongside them in the current inventory.
Daemon saturation now returns the exact occurrence to its durable remote
source under an explicit source-release disposition; the former route-era
"reconstruction" names are retired and no capability is synthesized.
The current inventory retains the five-per-validator, three-per-materialized
authenticated-non-validator, and two-anonymous owners (`5N+3H+2` total)
capacity-negative boundary and the exact
PrepareQC equal-vote quorum regressions. Its four integration tests run
together under their module filter; the complete pre-network corridor now has
88 legs, including the governance-unlock audit module, the autonomous
lifecycle-recovery module, separate exact
status and atomic lane-certificate decode
contracts, nine G-UNIT execution-receipt legs, the source-attested Native AMX
fixture check, two `iroha_config` geometry modules, the two new `iroha_p2p`
geometry modules, the shared-byte-budget module, plus source-sealed workspace
formatting, the legacy-codec guard, workspace
build, Clippy, workspace tests, and feature-enabled `irohad` command-success
legs, the G-SCALE runner/validator preflight, plus three proposal-origin
data-model module legs. Immediately before completion publication, the runner
also revalidates the source-bound localnet binary bundle. The data-model modules are
discovered and executed against `iroha_data_model`; they cannot fall through to
the `iroha_core` runner.
The current 855-test inventory is a mechanically checked
source contract, not execution evidence; the
complete inventory must still run as one clean committed, detached,
source-sealed release leg before it becomes release evidence.

The preceding source-shared formal harness passed 118/118
unit/reducer/WAL/refinement tests and 8/8 model-trace replay tests. The current
harness inventories 137 runnable reducer tests and still requires a fresh
source-sealed run. The earlier fast-network mode passed all nine named
deterministic network simulations. Those unsealed results do not replace the
source-sealed release leg, and the pinned Verus receipt described by the formal
documentation predates these proposal-origin source changes.

At the earlier recorded checkpoint, the then-present focused additions were
green. The generation-fencing, typed-handoff, and reply-writer additions above
have not completed a source-current Cargo run. The gate names nine
completion-ownership regressions from that earlier checkpoint: exact
ingress/Busy-deferred
coalescing, conflicting manifest and receipt evidence, conflicting local and
validated receipts, production Busy transfer, cross-queue retirement,
transactional duplicate-owner failure, and three installed/destination-rebind
cases. It also names the two production certificate-transport conflict tests
and the runtime unbound-Vote authority test, plus the reducer exact-lock and
adapter consumer-epoch tests. An earlier exact one-attempt four-validator
genesis rerun passed on all validators in 456.76 seconds, with evidence retained
under
`target/sumeragi-v2-genesis-final-hardening-20260716/irohad_test_network_7sJJmM`.

A current-working-tree diagnostic then ran the exact authoritative
four-validator genesis regression with networking required, network-start
retries disabled,
and its exact deterministic seed. The single permitted startup attempt passed
1/1. Live snapshots crossed the original reset boundary through view 10 while
retaining the exact view-9 locked Commit intent; the regression then observed
genesis applied on every validator and a common awaiting-proposal successor
height. All four peers exited with status 0, and libtest completed in 1192.57
seconds including the cold re-entrant network binary build. The temporary log
is `/tmp/iroha-root-genesis.1T17yY/run.log`, and the temporary localnet is
`/tmp/iroha-root-genesis.1T17yY/irohad_test_network_FbEl6A`. This mutable-source
diagnostic is not a clean signed or checkout-manifest-bound source-attested
release receipt and does not complete any remaining release gate.

This is implementation and regression evidence, not a release-completion
claim. The executable effective-lock acquisition model now covers immutable
physical load identity across consumer rebinds, higher-lock replacement after
terminating local work, fail-closed completion classification, certified-store
waiting, retry, and delivery. Its 15,472 generated / 5,910 distinct-state TLC
search is green to depth 15. Paired mutations expose reload-per-view,
no-retry-after-store, and future-completion bugs; these are bounded regression
witnesses, not deductive proof. The model obligation and the separate
production worker/runtime refinement obligation both remain
`specified_unproved`. Two production regressions additionally reject an
unissued future completion without replacing its owner and preserve the latest
consumer while a missing body waits for durable recovery and retries.

The earlier `SumeragiV2TypedRolloverHandoffProofs` strict and bounded receipts
predate the authority-gated active-state fence, the V3 two-slot
compaction/persistence relation, bootstrap adoption, validation-before-cleanup
ordering, and the root trust boundary, so they are not current evidence. The
control partition and `NoRolloverFailure` remain specification structure only.
The conditional rollover declaration is not promoted while its final
persistence relation and downstream rotating-leader dependency remain
unproved. Both typed-handoff declarations remain source-bound support leaves
consumed transitively by the top-level successor-activation production-
refinement debt; they are not independent ledger rows and do not establish
filesystem or Rust refinement, recovery, eventual finality validation, network
or writer progress, repeated rollover, or end-to-end liveness. Fresh strict
safety-and-liveness validation remains pending.

The strict proof-run counts in the following paragraphs are retained
historical submodule evidence, not current aggregate source-manifest-bound
release evidence. The canonical 54-entry top-level proof ledger currently
reports 35 `tlaps_proved`, 12 `specified_unproved`, 6 `trusted_contract`, and 1
`out_of_scope` entry, with `machine_checked_completion: false`. Sixteen
source-bound decomposition leaves remain checked transitively through their
reviewed consumers and are not independent ledger rows. The legacy-named
locked-body-reproposal entry denotes the exact three-arm progress obligation:
old-round Commit, unchanged later-view same-round re-proposal, or legitimate
Decision/higher-Prepare supersession. It and the production cross-tool refinements remain explicitly
unproved; no bounded model or source-fidelity check promotes them. The
aggregate temporal module gives
`AdequateLeaderExactClosureResidualObligation` and
`ExactDecisionOffSchedulerResidualConvergenceObligation` pinned source proof
bodies and classifies them as deductively proved support leaves. They are not
independent ledger rows; downstream wrappers cannot promote their consumers
without fresh strict evidence for the complete dependency closure.

The adequate-leader residual is target-local rather than aggregate: another
validator's Decision is not terminal for the indexed target. Its occurrence
rank counts every distinct target/leader owner at the frozen semantic rank,
preventing one serviced owner from hiding another. Equal-count replacement
and count-increasing replenishment remain explicit non-progress cases and
require a prior finite or coalesced producer argument.

The exact-Decision producer audit narrows causal replenishment to reachable
local debt setters; Serve-capacity growth to ordinary or historical request
drain, fresh causal Completion admission, or local Control enqueue; and
priority growth to exact network-claim admission or the same archive's normal,
recovery, or historical runner. Each classification is action-local. The five
exact off-scheduler convergence leaves have source proof bodies over immutable
owner identity, finite prefixes, admission/coalescing, and the nonphysical
response gate. Their composition does not count replenishment itself as
progress; fresh strict TLAPS is still required before their consumer is
promoted.
Independently, `ResponsiveStrongFairnessToReceiptResidual` and the conditional
cursor theorem have source proof bodies and remain source-bound support for the
unpromoted production progress refinement; a stale receipt cannot attest the
final dependency closure. An honest
validator outside `Responsive` may retain activation queued before GST;
neither the formal fairness premise nor the conditional release target promises
its local-worker progress. The action-by-action safety
induction is strict-green at 7,826/7,826 obligations, and the downstream
`SumeragiV2Proofs` module is strict-green at 565/565; historical TC-lock
authorization and its dependent direct-or-installed-authorization timeout
theorem are therefore ledgered `tlaps_proved`. The remaining debt is the
asynchronous ownership/fairness and multi-height liveness composition.
The protected Stage-4 service-rank slice is strict-green at exactly 196/196
TLAPS obligations. It establishes scheduler-wide exact ownership, coalesced
causal successors, class-specific causal-capacity debt, independent fair
Commit-certificate discovery, Consensus-only I/O indexing, and exact
ready-completion rank. A source-bound mutation gate requires the old refill,
replacement, discovery, and I/O-index variants to expose their exact
counterexamples, while the repaired variants pass; a separate bounded
ownership check exhausts 42,817 generated / 6,208 distinct states to depth 45.
The expanded graph reflects the independent non-timeout-progress and
TimeoutVote ingress reservations.

The asynchronous fairness boundary is also source-pinned. All 18 weak-fairness
targets carry one of four exact outer frames, so TLC sees a value for every
primed variable when it evaluates `ENABLED`. The exact quantified
`AsyncFairActionAt` inventory is stated by the typed
`AsyncFairActionsRefineAsyncNext` operator and proved by the dedicated
`AsyncFairActionsRefineAsyncNextObligation`. A recorded decomposed default-limit
strict run proved all 1,143 obligations, including typed command execution,
the 18 Core projections, and all runner/non-runner/recovery outer frames. The
checker mutation-tests action deletion, quantifier drift, frame
misclassification, claim or theorem weakening, unreviewed helper theorems, and
any TLC-only duplicate variable or fairness relation. The full Core `Next`
disjunction is intentionally kept out of individual fairness targets to avoid
re-evaluating unrelated Core branches. This closes only the fair-action
refinement entry; it does not complete the 18 remaining deductive obligations.
This is not the complete protected-rank proof: progress-relevant Normal
proposal/Prepare work and a productive, decreasing deadlock theorem are still
missing, so no ledger status is promoted. Formal entrypoints now share a
working-Java resolver which rejects an invalid explicit runtime and skips the
macOS stub; the selected canonical binary remains hash-bound in release
evidence.
A 2026-07-21 working-tree 100,000-height permissioned/NPoS chaos run preserved
both 50,000-height chain prefixes, completed 400,000 validator finalizations
with zero failures, and finished in 91.29 seconds; the release profile must
still reproduce it under the checkout-manifest-bound evidence root. This is a
certificate-supplied reducer/WAL simulation (`certificate_source` is the
external deterministic fixture), not a four-daemon production-network chaos
run. The gate now additionally queues local adapter work and rotates
deterministic restart at the Decision-WAL, body-fetch, body-store, validation,
and application boundaries, rejecting each old-generation completion. Because
that expansion postdates the older recorded run, fresh evidence was required.
The expanded 320-height smoke and exact schema-v2 100,000-height harness run are
green; the latest run matched every pinned counter. The source-attested wrapper
intentionally rejects the current dirty worktree, so checkout-manifest-bound
evidence still requires a signed clean commit. The fresh full strict
TLAPS/proof-ledger run against the settled source,
clean source-sealed production corridor, checkout-manifest-bound 100,000-height
reducer/WAL chaos run, and fully pinned 24-hour Taira-profile soak also remain
outstanding. The complete PR corridor is not claimed green here until those
gates finish.

## Deterministic and production gates

The PR corridor runs four fixed seeds for each of five four-validator scenarios:
genesis, restart, timeout rotation, locked-origin direct Commit with rejection
of an equal-byte later proposal origin, and a
causally staged distinct-subject PrepareQC split. The distinct-subject schedule
isolates subject A at the actual view-zero leader, advances the other three
validators with no-high-QC timeout evidence, certifies subject B at two
receivers, and only then delivers stale A evidence to a fourth receiver. This
constructs a 2+2 split over highest PrepareQC references without pretending
that two disjoint honest locks are possible. Explicit releases are increasing
captured subsequences, not FIFO prefixes; only the final drain is a FIFO fence.
The separate ignored four-validator-plus-five-observer slow-reader stress test
does not alter this acceptance contract: it remains independent coverage, not a
sixth matrix scenario. The release profile therefore attests exactly 32 seeds
for each of these five scenarios, or 160 real-network runs.
The new fifth leg is statically validated but still requires its focused Cargo
and real-network execution before it reduces release debt:

```bash
bash scripts/run_sumeragi_v2_release_gates.sh --pr
```

Before those longer scenarios, the PR gate inventories 855 exact production
liveness tests and executes all 40 owning Rust modules serially. The release
profile additionally records nine G-UNIT legs executing a separate 525-test
focus inventory. The
inventory includes the reducer exact-lock and adapter consumer-epoch
regressions, plus five lane-work tests which pin the native-AMX signing guard's
explicit runtime bound, exact hard boundary, above-bound fail-closed behavior,
record/anchor byte ceilings, and production-like adapter limits. It also pins
adapter-owned successor activation, runner ingress
handoff, watchdog predecessor/successor separation, and recovery-derived
successor identity. The worker leg also pins rejection of an unissued future
physical acquisition and exact latest-consumer rebind across unavailable-body
recovery. The authoritative ingress leg pins `5N+3H+2` count potential, the
certified-fence-escape, TimeoutVote, and TransportCompletion byte reserves, frozen-layout wire-size
activation, cross-validator isolation, and fair service; the
adapter/runtime legs pin the independent `2N+3` Busy-deferred partitions and
runtime Progress admission. They also pin the four-effect maximum flattened
persistence macro-step below the eight-effect bound, exactly one serviceable
deferred adapter step per runtime turn, and refusal of terminal readiness while
any deferred Completion, Progress input, or ordinary input remains. The adapter
leg also realizes the complete
`1024 + 3N` semantic-admission bound, retains current/adjacent-future signer
keys, retires out-of-window TimeoutVote delivery records, and exercises
non-poisoning same-owner retry across TC installation. The block-sync leg pins
reducer-enqueue ownership, strictly sequential context catch-up, and canonical
Kura body service by a frozen-roster archive. The archive need not have signed
the historical QC: the QC authenticates the exact subject, the archive signs
the response, and outside-roster or forged evidence still fails closed. Nine tests pin the
completion-ownership seam described above. It then inventories and executes the exact Rust
positive/negative cross-SDK wire-fixture tests and
the maintained JavaScript and Python authoritative-status parser tests. The
parser inventory pins normalization, `local_control_pending`,
`unsafe_proposal`, the full 12-reason ignore bound, and the distinction between
a remote partial timeout pool, a durable local timeout path, and same-/older-view
locked Commit recovery; a missing or ignored
Rust fixture test or a missing named SDK parser test fails the gate. The
prior inventory and serial 11-module execution are green historical evidence.
The preceding mutable-source discovery found all 168 then-required names among
6,750 library tests with none missing or ignored, and direct exact execution
was green at 168/168. Two same-runtime-step Decision reconciliation tests raised
the inventory to 170, and the exact timeout-path classifier regression raised
it to 171. Eleven Kura progress-witness durability regressions, two lane-geometry
durability regressions, and the lane-work alternate-certificate retirement
regression raised the inventory to 185 tests across 16 modules. Six exact
lane-retirement recovery/substitution regressions and the lane-work plus runner
platform-role gates raised the inventory to 193. The isolated-runner timing
contract which proves the view-zero wait covers its deadline without masking
view-one observation raised the inventory to 194. Six bounded effect-dispatch
and preemption regressions, one source-faithful serialized runtime trace, plus
its canonical watchdog lane regression raised the inventory to 202. Two exact
post-decision boundary regressions raised it to 204; fourteen scheduler,
adapter, and effect regressions raised it to 218. Four final adversarial
regressions pin post-WAL oversized-continuation fail-close and exact replay,
terminal readiness with queued Busy-deferred work, signature completion before
a deferred timeout and newer ingress, and production Completion-reserve
saturation in which an exact certified response releases one request before
the same retained FIFO Fetch acquires both owners atomically. They raised the
preceding inventory to 222.
Six outer TransportCompletion-corridor regressions raise it to 228; explicitly
pinning the then-current four-per-validator plus two shared relay-lane owners
(`4N+2` total, before authenticated non-validator lanes were separated)
capacity-negative raises
it to 229; and the four-validator exact PrepareQC equal-vote quorum test
raises it to 230. Two exact locked-Commit progress-witness regressions then
raised the baseline to 232. The next 32-regression delta added atomic
lane-certificate and semantic-origin ownership, P2P source/flush/reconnect
ownership, authenticated-via relay isolation, and active-watchdog coverage,
bringing that baseline to 264. Thirty-seven positive additions—10 exact-output/
typed-rollover, 2 peer flush/generation, 20 ticket/topology/broadcast/
subscriber-network regressions, 1 runtime/Busy-deferred exact CommitQC
coalescing regression, and 4 Nexus lane-relay ownership/fairness regressions—
are offset by removal of the obsolete adapter cursor alias and two superseded
network broadcast-residual tests, bringing that inventory up by a net 34 to
298 tests across 21 modules. The bounded per-source route, writer-flush, and
transport-route construction closure added 82 exact regressions and retired
two obsolete target or broadcast-residual names, yielding 378 tests across 24
modules. The subsequent ownership-carrier closure adds 28 exact adapter,
effects, runtime, ingress, sidecar, lane-work, worker, P2P, and daemon
regressions without removing a name, yielding 406 tests across 24 modules. The
current-source geometry closure adds 14 exact ingress, sidecar, adapter,
effects, runtime, worker, P2P, and `iroha_config` regressions without removing a
name, yielding the historical 420-test checkpoint across 26 modules. The
route-lifecycle closure added three exact P2P regressions without removing a
name, yielding the historical 423-test checkpoint across the same 26 modules.
Those tests pin immutable
delivery-to-minting-tenure binding through bounded tombstone churn, reject
second-route overwrite at the rehydration boundary, and isolate actor-exit/`Drop`
cancellation to the actor's own routes and waiters.
The historical reconciliation added 16 exact tests, two P2P geometry modules,
and a daemon peer-genesis module that is now retired. It separately replaced
one obsolete lane-relay name in place, yielding the historical total of 439
tests across 29 modules. At that checkpoint, its Kura,
refinement, effects, recovery, runner, watchdog,
P2P, and peer-genesis entries pinned replay metadata, successor authority/lifecycle,
discovered CommitQC admission, source geometry, and clone-safe producer/fanout
ownership. The five peer-genesis regressions are not part of the current
inventory. The lane-relay saturation test was renamed in place, so the module
still contributes four tests. The subsequent in-flight sidecar redelivery
regression raises that total to 440 without adding a module or corridor leg
and binds one exact writer-flush owner per source chunk. Three subsequent
worker regressions produce the historical 443-test checkpoint. They bind same-tenure
pending/unapplied flush deduplication, mixed-source terminal-route history, and
same-source reconnect preservation for both the unflushed current item and a
terminal zero-reservation cursor.
The subsequent 22-test source-authority and shared-payload closure raises the
inventory to the 465-test checkpoint across 30 modules and 53 pre-network legs.
The authenticated-non-validator source-cap and alternate-route-before-lane-cap
regressions then raise the inventory to the 467-test checkpoint without changing the module
or corridor-leg counts. The four daemon and two root-configuration regressions
then raise the inventory to the 473-test checkpoint, again without changing those counts.
The configuration-fingerprint, two recovery-kernel, and shared-source-credit
regressions raise the inventory to the 477-test checkpoint.
The proposal-origin, multi-carrier, and persistence-failure closure then adds
41 regressions and retires nine superseded selectors, yielding the 509-test
checkpoint across 38 modules and 61 pre-network legs. The final
successor-parent, lane-rollover, tip-recovery, terminal-ingress, genesis-origin,
and restart-deadline closure adds six tests without adding a module. Six
source-sealed command legs and the G-SCALE runner/validator preflight yielded
the historical 515-test inventory across 38 modules and 65 pre-network legs.
The per-source route-attempt and locked-body completion adds 57 exact names,
one owning module, and seven corridor legs. Nine G-UNIT execution legs plus the
source-attested Native AMX fixture-check leg complete the 584-test checkpoint;
the certified sidecar Close/CloseAck critical-bucket regression produces the
585-test checkpoint. The unsent-request restoration and fairness-cursor retry
regressions produce the 588-test checkpoint; the durable semantic-peer-history
regression produces the 589-test checkpoint. Mechanical reconciliation adds
115 net ingress, merge-sidecar, lane-work, runner, worker, P2P-network, and
daemon-relay changes, producing the 704-test checkpoint. The runner
close-prefix failed-suffix handoff regression adds one exact name, bringing the
historical inventory to 705 tests. The routed-Hint and crash-safe V3 lifecycle
closure adds 26 exact regressions and retires eight obsolete route-free/V2
selectors, producing the 723-test checkpoint across 38 modules and 81 legs.
The replay-safe locked-proposal authorization regression produces the 724-test
checkpoint. The exact runner replay-owner regression produces the 732-test
checkpoint. The foreign-context CommitQC Apply rejection produces the
733-test checkpoint. Five exact-Serve lifecycle regressions bring the current
inventory to the 738-test checkpoint. Another 35 exact CertifiedServe ingress
and worker regressions bring the inventory to 773 tests without adding a
module or leg. One four-peer leader-wire lifecycle-store regression brings the
inventory to the 774-test checkpoint, adds its owning module, and adds one
corridor leg. Eight leader-wire retirement, terminal-consumption, and
authenticated-Coalesce defense regressions produced the 782-test checkpoint
without adding another module or leg. Subsequent source reconciliation and
exact-ingress lifecycle, restart, provenance, and quarantine regressions bring
the inventory to the 806-test checkpoint without adding another module or leg.
Seven physical-cut, adapter-capability, aggregate-rebase, and ineligible-driver
regressions produce the 813-test checkpoint. Five admission/coalescing, Busy
pre-runtime ownership, and reconstructed-chunk terminality regressions bring
the 818-test checkpoint. Thirteen exact admission, retry, tombstone, and
high-water regressions bring the inventory to the 831-test checkpoint, again
without adding a module or leg. Retiring five obsolete peer-genesis protocol
regressions brings the inventory to the 826-test checkpoint and removes one
owning module and one leg overall. Replacing one obsolete restart selector with its two distinct
crash boundaries and restoring two implemented certified-ingress regressions
then produces the 829-test checkpoint. Autonomous-lifecycle terminal-outcome
and startup-recovery coverage plus final source reconciliation bring the
837-test, 39-module, 86-leg checkpoint. Ten deterministic network simulations
then bring the 847-test checkpoint across 40 modules and 87 legs. The
source-bound terminal-sweep partition regression brings the current inventory
to the 848-test checkpoint. The two late-predecessor reopening regressions bring
the 850-test checkpoint. Seven Native AMX finality-bound merge-projection
regressions bring the 857-test checkpoint across the same 40 modules and 87
legs. Three Kura recovery regressions and the governance-unlock audit bring the
861-test checkpoint across 41 modules and 89 legs. The production-adapter
activation guard and two deferred-canonical-carrier completion regressions
produced the historical 864-test, 41-module, 89-leg checkpoint. After retirement
of the duplicate inline network-simulation rows, the current inventory contains
855 tests across 40 modules and 88 legs.
The rollover slice covers
historical Kura CommitQC, body, and lane-certificate rereads; current global
V2; lane proof/supersession; Native AMX; merge-share, certified-sidecar, and
untyped fail-closed boundaries. The route slice pins semantic deduplication,
one independent attempt per authenticated source, actor-global delivery
ordinals, connection-tenure-bound tickets, source-owned non-regressing cursors,
and bounded route-set capacity. Canonical request identity binds wire version
1, the positive responder service generation, requester stream epoch, semantic
sequence, immutable payload or reference, and both peers. It excludes only the
cumulative close floor, which can advance monotonically on that same occurrence
without rematerialization. Completed requests have no wall-clock expiry: an
authenticated cumulative close advances only over a contiguous terminal prefix
and is the sole mechanism that retires the covered server output. Admission
capacity is preflighted before either the close floor or server stream state can
advance.
A later delivery changes only its source's route and preserves that source's
current immutable payload, cursor, FIFO age, and reservations. A reconnect
keeps the source's FIFO identity, clears the retired tenure-bound ticket, and
retries the source's retained current item or chunk through fresh tenure
admission while leaving sibling-source progress untouched. A newly observed
alternate source starts independently at zero.
The durable requester-stream table and unified responder-stream table are
bounded independently of the smaller concurrent reply-source and active-gate
geometry. Request gates form the second bounded responder table. Crash recovery
restores exactly those bounds from the marker-selected V3 snapshot.
These newest tests pin local typed retirement, ownership, and fail-closed
behavior; they do not claim end-to-end relay/application acknowledgement or
unbounded broadcast admission. The integration filter remains a five-test
module leg, while separate P2P, daemon, status, Nexus lane-relay, and atomic
lane-certificate contracts brought that historical aggregate pre-network
corridor to 61 legs. The current source-bound inventory is the separately
audited 88-leg, 855-production-test corridor plus 525 G-UNIT tests; execution
against a signed clean candidate remains required before release promotion.

The current reconnect changes supersede older mutable-tree diagnostics that
assumed writer continuity across connection tenure. Fresh focused worker,
sidecar, lane, and transport-route tests plus the complete independently mirrored
source-sealed pre-network corridor remain required before release evidence may
be promoted.

The same pre-network gate inventories and executes four exact, non-ignored
Taira release-profile validators, the Rust summary-JSON schema contract, and
the strict all-validator restart/catch-up contract: six Taira contracts total.
It then requires exactly 43 passing mocked soak launcher/evidence tests. Those
tests reject profile drift, zero-test success, concurrent evidence ownership,
source or artifact mismatch, malformed JSON, weakened acceptance bounds,
inconsistent counters, invalid provisional evidence, and inconsistent status
classifications. They validate the release machinery; they are not substitutes
for the 24-hour validator soak.

Both profiles require one clean committed source identity before their first
Cargo command, bind the ignored workspace `Cargo.lock` as an explicit build
input, and reject unresolved index entries and every active merge,
cherry-pick, revert, mailbox-apply, rebase, sequencer, or bisect operation.
Administrative paths are resolved through `git rev-parse --git-path`, so an
operation in one linked worktree is not mistaken for an operation in another.
Each outer invocation reproduces the identity in an independent clone made
with no local object sharing, hardlinks, or alternates, then runs build-capable
children only from that clone with private target, Cargo-home, home, temporary,
cache, and artifact roots. The PR profile is developer validation rather than
signed release evidence; production additionally requires the authenticated
bootstrap, source seal, signatures, and aggregate receipt. The seed runner
compares the cloned source digest after test inventory, before and after every
scenario, and on both sides of completion publication; PR also recomputes it
after the formal harness. Any drift leaves only partial evidence and fails the
corridor.

Before any network attempt, the gate requires the complete source-manifest and
seal contract suite to pass. It covers content and ordering, deletions,
symlinks, executable modes, ignored `Cargo.lock` drift, unresolved entries,
every active Git operation, linked-worktree locality, clean HEAD/index/worktree
identity, missing or symlinked lockfiles, detached-source reproduction, and a
cooperative ordinary-write attempt against a read-only source tree. Source
symlinks may not escape the sealed root or enter writable output, and regular
source files may not retain external hard-link aliases; internal dangling links
remain valid only beneath sealed internal ancestors.

Seed evidence is isolated per invocation beneath a manifest-addressed root. An
atomic directory lock rejects a concurrent writer to the same root, and a
unique invocation directory preserves earlier and failed runs instead of
deleting them. `invocation.tsv` records the source identity, fixed run count,
and internal harness deadlines; `summary.tsv` binds every command row to that
source digest and records the SHA-256 of that run's full Cargo log. The
aggregate receipt re-hashes all 160 logs and independently requires one exact
`--nocapture` scenario/seed diagnostic followed by its standalone `ok`, one
`running 1 test`, and one passing libtest summary per row. The seed completion
also binds exact HEAD, tree, and `Cargo.lock`, not only the sealed source
manifest.
Only an exact complete matrix receives an atomically published
`COMPLETED.tsv`, which includes the SHA-256 of the summary. A zero-test result,
ambiguous libtest output, command failure, source change, or interrupted/stuck
invocation has no completion record. A hard-killed runner deliberately
leaves its evidence lock for operator inspection.

The matrix pins the integration harness's bounded build, subprocess, network
permit, startup, synchronization, status, and shutdown waits. Repository
process-safety policy neither observes or logs ambient process state nor
signals or controls unrelated Cargo, rustc, or validator processes. It
serializes only this invocation's Cargo calls with an owner-private directory
lock below the authenticated external artifact root and fails closed on lock
contention. Without an outer watchdog it cannot promise a wall-clock bound if
an execution path escapes all of those internal deadlines. Such a pathological
run may remain active, but it remains fail-closed evidence because it cannot
publish `COMPLETED.tsv`; it must be inspected and resolved before retrying.

The production corridor accepts only one clean committed candidate: HEAD and
its tree must resolve, the index tree must equal HEAD, tracked files must be
unchanged, and no non-ignored untracked path may exist. It recreates that exact
commit in an independent local clone made with no local object sharing, no
hardlinks, and no alternates; verifies that the mirror shares no Git-object
inode with the candidate; copies and re-hashes the ignored `Cargo.lock`; and
requires the pre-seal mirror identity to equal the original candidate identity.
It rechecks the original identity after mirror creation, redirects `target`,
temporary files, caches, evidence, and retained localnets outside the source,
and removes source write permission before re-entering the complete release
script. No linked-worktree metadata is created in the candidate. This chmod
seal makes ordinary editor, build-tool, and accidental writes fail; it is a
cooperative integrity control, not an adversarial same-UID security boundary.
The owning UID can chmod, write, and restore modes between checkpoints.
Identity and seal checks run after each major leg, while the independent
committed mirror removes the caller's mutable checkout from the execution path.

For every enumerated file or symlink entry, the checkout manifest includes all
permission bits. Directory modes are checked by the separate source-seal walk;
the manifest does not claim to inventory directory modes. Therefore the
original committed candidate manifest and the read-only sealed manifest may
differ and are never silently substituted for one another. Git HEAD/tree
continues to bind content and executable-bit semantics. Every child build and
completion record uses the sealed manifest actually compiled. The final
aggregate receipt records both manifests and binds the original HEAD, tree,
index tree, and `Cargo.lock` digest.

The production corridor clears Git resolution overrides, replacement refs,
Rust compiler/wrapper/flag overrides, Cargo target/profile runner and linker
overrides, inherited libtest capture/color controls, and then fixes uncolored
transcripts. The authenticated bootstrap runner-tool manifest supplies the
repository-pinned Cargo 1.93.1 and rustc 1.93.1 binaries directly; the corridor
records their paths, versions, and hashes without reopening rustup discovery, uses an
isolated `CARGO_HOME` without external configuration, and permits only the
source-bound `.cargo/config.toml`. Registry and Git cache inputs are copied to
new private inodes after race-checked traversal; contained relative symlinks are
preserved, while absolute, escaping, and special-file entries fail closed. A
canonical input inventory and digest outside the mutable Cargo home bind that
snapshot. After the last Cargo command, a second bounded canonical inventory
binds every final cache entry and the receipt requires exact tree equality,
including all Cargo-created additions; both snapshots reject configuration,
hardlink, symlink, special-file, size, depth, and cardinality escapes. The
release corridor never writes the caller's cache. Private cache, runtime, and
scaling-bundle construction occurs before any candidate build child starts,
inside a fresh owner-private invocation namespace. That quiescent construction
boundary is required because portable POSIX interfaces do not provide a
`mkdir` operation which also returns the created directory descriptor; the
contract does not claim to tolerate an already-active same-UID writer racing
the initial stage-directory creation. Once each staged root has been bound and
published, descriptor-held identity and the post-child source/destination
inventories cover the active build phase. Java, Git,
Python, Node, Bash, TLAPM,
TLA2Tools, Verus, and cargo-verus identities are likewise bound where used.
Reading cache files can still update access time under host filesystem policy;
the runner deliberately does not attempt a non-portable or racy atime restore,
and it verifies that content, namespace, identity, mode, ownership, link count,
size, modification time, and change time remain stable while copying.

The production corridor raises the real-network matrix to 32 seeds per
scenario, requires fresh source-bound TLAPS/TLC/Verus and trace-replay evidence,
runs the 100,000-height permissioned/NPoS chain-prefix chaos test, and pins the
Taira-profile seed, load, packet loss, churn cadence, acceptance bounds, and
86,400-second duration. Load acceptance and commit rates use the full elapsed
wall time, not a denominator with churn work removed. Churn deadlines stay
anchored to the original schedule; at least 90% of the expected process and
membership cycles must execute, a cleanup leave does not count as a scheduled
cycle, churn work may consume at most 25% of elapsed time, and an in-flight
bounded churn action may overrun the workload deadline by at most 15 minutes.
The strict formal release gate runs before the 32-seed matrix, so an incomplete
proof ledger or stale backend evidence fails before 160 real-network attempts.
The PR profile retains its four-seed matrix followed by the fast formal harness.

The strict formal command is captured through `tee` in a unique
manifest-addressed invocation. Completion requires both pipeline elements to
succeed, one exact final all-legs marker, a final source identity check, and a
second release validation after mutation, TLC, replay, and Verus finish. The
invocation archives and hashes the complete formal log, `proof_coverage.json`,
`proof_evidence.json`, the checked-in harness `Cargo.lock`, and the resolved
formal-toolchain table; its `COMPLETED.tsv` binds those hashes to sealed
HEAD/tree/`Cargo.lock`. The harness uses the pinned lock with
`--locked --offline`; it does not regenerate dependency resolution. Aggregate
receipt generation invokes the official proof
ledger checker again, so a hand-written JSON object claiming
`machine_checked_completion` cannot satisfy the formal leg.

The chaos runner requires the sealed clean identity before and after execution,
accepts only the one exact ignored 100,000-height test, libtest summary, and
explicit schema-v2 50,000-per-mode completion marker. The marker and atomic
completion record bind the exact restart, duplicate, under-quorum, reordered
delivery, deferred-work, and stale-generation counters as well as HEAD, tree,
sealed source manifest, `Cargo.lock`, both mode counts, completed height count,
and the full log SHA-256. This is an accelerated production-reducer
chain-prefix test with externally supplied valid certificates and a finite
deterministic local scheduler, not a real-network quorum-formation or partition
campaign.

The production soak runner clears inherited daemon/Kagami overrides, pins
release-profile binaries, `CARGO_NET_OFFLINE=true`, and `RUST_LOG=info`, and
builds under a SHA-256 checkout-manifest-addressed target. When invoked by the
release corridor, it rejects a checkout digest that differs from the parent's
manifest before inventory or compilation, rather than spending 24 hours on a
candidate the parent must later reject. An atomic lock gives
that source root exactly one soak-evidence writer; a hard-killed run leaves the
lock for explicit operator inspection. Before any real matrix, the release
gate inventories and executes the five exact Rust release-profile/schema tests
plus the strict all-validator restart/catch-up test,
and requires all 43 mocked soak launcher/evidence adversaries to pass. The soak
runner then inventories the exact ignored test and accepts Cargo output only
when exactly one test ran and passed with networking required. The seed matrix
also forces one fresh network startup attempt, so a later harness retry cannot
hide a protocol-induced stall. The fixed retained summary records the Git
revision, checkout manifest, daemon, Kagami, test-binary and generated-config
digests, the complete profile, cadence accounting, and authoritative
initial/final Sumeragi status quorums. The localnet directory is retained, and
the evidence checker independently recomputes the canonical workspace manifest,
requires every binary to remain beneath its release-profile source-bound
directory, and re-hashes those artifacts before the runner reports success.
The test writes to an invocation-local `.partial` path; only successful
independent validation and final identity checks atomically promote it to the
canonical JSON and publish a hash-bound `COMPLETED.tsv`. That completion binds
the exact HEAD, tree, `Cargo.lock`, sealed source manifest, canonical JSON, and
the retained full Cargo/libtest transcript. Failed invocations keep that log
for diagnosis while provisional JSON is removed.
Duplicate object keys and non-finite numbers, including finite-syntax exponent
overflow such as `1e10000`, are rejected recursively before schema validation.
Retained status snapshots preserve their original validator
index and must contain at least three distinct responsive validators, wire
protocol 4, no restart-required node, the complete liveness object, and bounded
queue evidence. Every retained no-progress interval is accepted only when its
canonical classification set exactly matches the blockers in its authoritative
status snapshots. A checkout-manifest checkpoint immediately after the
100,000-height chaos run rejects drift before starting the 24-hour soak. A final
checkout-manifest and proof-evidence check rejects any later source change
during the long corridor.

### Protected release approvals

Production promotion requires four independent operator decisions with the
exact class IDs `offline-toolchain-sdk`, `formal-proof-tools`,
`network-scale-soak`, and `final-bootstrap-publication`. Each decision is one
canonical JSON v1 file whose exact top-level fields are `approval_id`,
`approved_at`, `candidate_oid`, `candidate_tree`, `class_id`,
`evidence_root_id`, `expected_duration_seconds`, `format`, `operations`,
`profile`, `protected_tool_manifest_sha256`, and `schema_version`. Each ordered
operation has exactly `arguments`, `operation_id`, `ordinal`, and `tool_id`.
The four immutable inventories contain 23, 38, 8, and 8 operations,
respectively; the canonical ordered IDs are recorded in the
[multilane rehearsal runbook](runbooks/nexus_multilane_rehearsal.md#protected-release-approval-contract)
and source-bound by `sumeragi_v2_release_approval_contract.py`. The canonical
ordered operation-record SHA-256 values, in the same class order, are
`4124c633d52744528f04a732149adce9d4e94b83437a6b121bf7087b03c95262`,
`eb9f0283898f09d23970f1d6511d250b17107a0ad80fc65e1adbe1ef0b1b19bb`,
`e922b8afbfe4848e8b2b5f858654477a28d710c3afcf7698a77276a8294681cf`,
and `76be51f1583e2d49c8b9ac85f9218a0a0b5a3334f1923dad39aa13ec8e7768fd`.

An approval file must be owner-held, mode `0400`, single-link, bounded,
canonical, and below trusted non-writable ancestry. Its command records use
relative arguments and stable archive/evidence IDs, never caller checkout,
tool, cache, or evidence paths. These files record filesystem-protected
operator decisions; they are not signatures and do not claim cryptographic
approver identity. Bootstrap authenticates and privately archives all four raw
records, validates their exact candidate/tree/tool/evidence/duration bindings,
and publishes only path-free sanitized attestations. The standalone validator
and receipt writer independently replay the four records before acknowledgment
or publication, and bootstrap authenticates the retained path-free result after
private-state pruning. This machinery does not claim that an operator has
granted an approval and does not by itself close a release gate.

A production run cannot be started by invoking the candidate runner directly.
The release operator first authenticates an out-of-tree copy of
`bootstrap_sumeragi_v2_release.py`, the protected Python, Git, OpenSSH
`ssh-keygen`, and Bash executables, the manifest and identity helpers, the SSH
allowed-signers and revocation policies, the receipt validator's localnet
manifest support module, the private-runtime copy/prune helper, and every
expected SHA-256 digest and signer fingerprint. The bootstrap archives that
support module as the receipt validator's exact sibling so the validator can
load it without consulting the working directory or `PYTHONPATH`. The
protected interpreter must start in
isolated, no-site mode; the evidence parent must already be owner-owned mode
`0700`, and the requested evidence child must not exist. The complete
invocation passes those protected paths and digests explicitly, for example:

```bash
/protected/python3 -I -S /protected/bootstrap_sumeragi_v2_release.py \
  --candidate-root /candidate/iroha \
  --evidence-dir /private/preexisting-parent/new-release-evidence \
  --expected-bootstrap-sha256 <sha256> \
  --python-bin /protected/python3 --expected-python-sha256 <sha256> \
  --git-bin /protected/git --expected-git-sha256 <sha256> \
  --ssh-keygen-bin /protected/ssh-keygen \
  --expected-ssh-keygen-sha256 <sha256> \
  --manifest-helper /protected/compute_workspace_source_manifest.py \
  --expected-manifest-helper-sha256 <sha256> \
  --identity-verifier /protected/verify_sumeragi_v2_release_identity.py \
  --expected-identity-verifier-sha256 <sha256> \
  --receipt-validator /protected/write_sumeragi_v2_release_receipt.py \
  --expected-receipt-validator-sha256 <sha256> \
  --receipt-validator-support /protected/sumeragi_v2_localnet_manifest.py \
  --expected-receipt-validator-support-sha256 <sha256> \
  --runtime-helper /protected/copy_sumeragi_v2_release_cargo_cache.py \
  --expected-runtime-helper-sha256 <sha256> \
  --runner-tool-manifest /protected/runner-tools.json \
  --expected-runner-tool-manifest-sha256 <sha256> \
  --bash-bin /protected/bash --expected-bash-sha256 <sha256> \
  --runner-environment \
    IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST=/authenticated/scaling/scaling_evidence.json \
  --runner-environment \
    IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256=<sha256> \
  --runner-environment \
    IROHA_RELEASE_SCALING_CONFIGURATION_SHA256=<sha256> \
  --runner-environment IROHA_RELEASE_SCALING_IROHAD_SHA256=<sha256> \
  --runner-environment IROHA_RELEASE_SCALING_IROHA_CLI_SHA256=<sha256> \
  --expected-signer-fingerprint SHA256:<fingerprint> \
  --ssh-allowed-signers /protected/allowed_signers \
  --expected-ssh-allowed-signers-sha256 <sha256> \
  --ssh-revocation-file /protected/revocation \
  --expected-ssh-revocation-sha256 <sha256>
```

The first-release policy accepts exactly one active SSH allowed-signers line
and rejects certificate-authority, SSH-certificate, `valid-after`, and
`valid-before` policy forms. The bootstrap archives private stable copies of
the trusted inputs, authenticates the candidate's exact signed commit and
release identity, and publishes `BOOTSTRAP_COMPLETED.json` before it launches
the signed runner under a closed environment. Its `PATH` contains only the
archived protected tools and a reviewed runner-tool closure. The protected
input manifest is an exact closed 41-name map whose entries contain only the
source path and expected SHA-256; after private copying, the sanitized marker
derives and binds each archive identifier, mode, size, and digest without
disclosing its source path. The bootstrap rejects writable or untrusted
ancestors. Before candidate build code runs, the outer
runner copies the named language/tool and exact shell-utility runtime closure
to new private inodes and binds exact path-withheld source and
private-destination inventories. The runner independently
validates that marker at entry, after sealing, and at every release-identity
checkpoint before it executes another candidate helper. The bootstrap imposes
no outer runtime or output-capture limit on the runner and never signals its
process group; a runner which escapes its internal deadlines remains visibly
incomplete and cannot publish either completion marker.

Shell harness core utilities are resolved only from that authenticated private
runtime. There is no `/usr/bin:/bin` execution fallback, and an unlisted
external command fails closed before its result can enter release evidence.

Runner stdout and stderr are inherited regular files created directly under
the private evidence directory, mode `0600` while active and `0400` after a
normal exit. Bootstrap output consumers therefore cannot backpressure the
runner. If the bootstrap process alone is interrupted, it does not signal the
runner and preserves the active logs and evidence directory for diagnosis;
without terminal validation it cannot publish external completion.

On success, the private invocation publishes its exact aggregate receipt. That
receipt binds the 88 pre-network corridor legs and
their exact 855-test production inventory, the separate 525-test G-UNIT
inventory, semantic test names/counts, commands, logs, the exact source-bound
prebuilt localnet binary bundle and attestation, and resolved tool identities.
Formal evidence includes the completion, pinned harness lock and toolchain,
proof ledger/evidence/log, multilane Apalache evidence, and the TLAPS resource
JSONL and summary. The validator requires the exact successful resource-guard
event grammar, strict scalar types, identical terminal summaries, and
sample-derived peak accounting rather than accepting those artifacts by digest
alone. Apalache evidence carries separate workspace and multilane source
manifests: receipt replay binds the first to the sealed release identity and
the second to the authenticated production trace-extraction certificate,
rejecting omission or authority substitution. The receipt also carries all 160 matrix logs; exact G-4P completion,
summary, and
four run logs; exact deterministic G-12 seed completion, summary, and ten run
logs; the two-hour G-12 fault-soak completion and log; the closed multilane
scaling bundle, retained validator, four authenticated digest anchors, retained
tool inventory, and repository-root binding; the chaos completion/log; and the
exact-identity Taira completion/canonical JSON/full run log. It independently
revalidates the matrix, G-4P, G-12, scaling, chaos, and Taira evidence, including
replaying the retained scaling and Taira validators against the archived
artifacts.

Every retained matrix localnet also has a canonical descriptor-relative
manifest containing each regular file's relative path, byte length, and
SHA-256. Traversal does not follow symlinks and rejects special files, path
escape, empty trees, and files or directories which change during hashing.
The seed completion schema binds all 160 manifest paths and digests plus the
canonical aggregate-index digest; receipt replay resolves every manifest
inside the archive and recomputes it from the retained localnet. Missing,
legacy, symlinked, reordered, path-escaping, or content-mismatched evidence
fails closed.

The receipt is a canonical owner-owned, single-link, mode-`0400` file. Its
writer uses an exclusive fully synced staged inode, complete-write loops, and
one no-replace rename to the final name. Before publication it
revalidates and synchronizes every bound evidence file, then the complete
evidence-directory closure bottom-up; after publication it repeats that
closure with the terminal receipt included. Any file, directory, or `fsync`
failure is fail-closed. There is no mutable pointer file. The successful runner
retains its sealed source root and sealed identity instead of deleting the code
which produced the evidence. The protected outer runner first invokes the
separately archived receipt validator in exact `--verify-existing` mode. Only
that validator may publish the canonical no-clobber acknowledgment; only after
the acknowledgment exists may the protected runtime helper prune runtime,
Cargo cache, target, home, and temporary state and publish the exact retained
inventory/result. The external bootstrap authenticates all three records, the
terminal receipt, sealed source identity, validator provenance, and sealed
runner-log digests before `BOOTSTRAP_RELEASE_COMPLETED.json`. Runner or
validator failure cannot publish external completion.

This authenticates the signed candidate and runner relative to the operator's
protected inputs; it is not remote host attestation. The release-host image,
dynamic loader and libraries before Python starts, the owning UID, and trusted
ancestor-directory owners remain external prerequisites. A malicious same-UID
process or trusted ancestor can still attack the pathname namespace between
checks. Durability also assumes the host filesystem and storage honor POSIX
`fsync`. These limitations are recorded in the bootstrap marker rather than
being hidden behind a cooperative-receipt claim.

The release command is intentionally fail-closed while
`formal/sumeragi_v2/proof_coverage.json` contains any
`specified_unproved` obligation or reports
`machine_checked_completion: false`. Bounded TLC searches and convincing paper
arguments do not upgrade that ledger state.
