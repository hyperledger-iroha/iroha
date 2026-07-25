# Sumeragi v2 liveness contract and release gate

Sumeragi v2 does not promise unconditional termination. An unbounded network
partition, the absence of a responsive dual quorum, or local disk, signing,
validation, and application work which never completes can prevent progress.
The first-release target is therefore conditional:

> After GST, with a responsive dual quorum and terminating local work, every
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
node which has no Commit intent for that proposal origin. That node does not sign merely
because the TC was installed: it recovers, durably stores, and deterministically
validates the exact locked body at its proposal origin, then appends a
`LockAndCommit` for the active finality round which retains that origin. Only
the successful WAL acknowledgement releases the Commit signature.

A second TC for the immediately preceding timed-out round is progress only when
its selected Prepare origin strictly exceeds both the installed highest PrepareQC
and lock. Its durable installation upgrades that lock while leaving the lifecycle
view unchanged; equal, lower, and no-high replacements are rejected.

This is a narrow exception to the timeout fence. It authorizes only the exact
proposal origin and subject of the active TC-promoted lock; unrelated Commit
origins and all old-round Prepare votes remain fenced. Any higher proposal-origin
local Prepare intent or known PrepareQC blocks the exception, including one for
the same subject bytes. Same-subject evidence at a later origin neither relabels
the locked origin nor permits reconstruction of its later-finality Commit.
Replay admits that `LockAndCommit` only
after an earlier durable `InstallTimeout` has
advanced the view while leaving that exact PrepareQC as the active lock. A
missing or mismatched lock fails closed instead of reviving an unrelated old
round. Each authenticated Commit vote whose `proposal_round` and subject
exactly match the active lock may
cross semantic delivery admission once in each active consumer epoch. A TC
generation change or a newly acknowledged exact lock advances that consumer;
equivocation fingerprints are tracked independently from delivery records and
retained for one roster rotation (plus the active exact lock exception).

The exact-lock rule also applies to current-view Commit votes. A vote delivered
before the node acknowledges the matching `LockAndCommit` is ignored
recoverably rather than creating an authority-free pool. The acknowledgement
first makes the later-finality same-origin intent durable, then retires every
older same-origin Commit pool, and only then releases the current Commit
signature. That ordering keeps the old reconstruction pool intact if
persistence fails. The adapter's
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
frame, block header, and validation receipt. The CommitQC round identifies the
later finality vote.
The owner tag must equal the reducer's independently observed current tag; it
has no ordering relation to either wire round. Every vote and QC authenticates
an explicit `proposal_round` in its signed preimage. Prepare evidence requires
that origin to equal its voting round; Commit evidence requires the same
context and height and permits the finality view to be later. A later-view
CommitQC can therefore finalize an exact body locked and validated in an
earlier view without relabelling its manifest or durable receipt. Body fetch,
application, and startup recovery require the manifest, durable frame, and
validation receipt to equal the authenticated proposal origin exactly. A local
lock is only a consistency/cache input and is never used to guess a missing
origin. Conflicting body, manifest, frame, or execution-commitment identities
fail closed, while a lockless validator can request the exact origin directly
from any certified signer. Revision 3 has no legacy decoder which fills a
missing origin from the finality round.

The canonical height-one genesis header is the only header-origin exception.
Its authenticated bytes fix `view_change_index = 0` before consensus timers
start, so an initial timeout may move the first proposal and its Prepare origin
to a later view without rewriting the genesis block hash. Finality and bridge
validation accept that exact height-one, parentless, view-zero header shape;
all ordinary blocks still require the header view to equal the authenticated
proposal origin exactly.

Once a proposal origin is locked, equal block bytes cannot be proposed again
under a later origin. A TC-selected PrepareQC installs the exact old origin and
the active reducer round drives Commit signing for that lock directly. Each
later Commit intent and vote therefore has a current finality round plus the
unchanged proposal origin. Durable recovery selects only the newest Commit
intent for the active `(proposal_round, subject)`. Acknowledging that intent
retires every older same-origin Commit pool before the new signature is
released, so only the latest durable finality pool is active.

Reducer transitions also check executable progress witnesses. A durable locked
Commit intent must be represented by signing work, the exact local vote pool,
recovery ownership, or a decision. Retained outbound control is a peer-delivery
source, but it is not a sufficient local witness because broadcast excludes the
sender. A TC-promoted lock without an intent must retain exact body-recovery
ownership until validation; once validated, the pending historical
`LockAndCommit` append is the required witness. A durable decision awaiting
application must retain its exact body pipeline and may not refer to a body
which deterministic validation marked invalid.

There is one serialized race exception: validation of a TC-promoted lock may
complete after the current finality view has durably timed out. The exact
acknowledged timeout for that current view then owns recovery, but never grants
permission to append or sign a Commit in the closed view. The following TC
rebinds the same proposal origin to a new generation and restarts its body
pipeline; successful validation may create the later-finality Commit only in
that new open view. The source-shared progress kernel rejects stale,
wrong-signer, volatile-only, and non-exact timeout substitutions.

Decision retransmission reconstructs the next missing owner from the durable
body stage instead of assuming a fetch is always sufficient. A missing body
restarts recovery, an available body restarts `StoreBody`, a durable body
restarts `ValidateBody`, and a validated body restarts `Apply`. Dropping any
one volatile owner therefore leaves a deterministic reconstruction path from
the durable Decision and body-stage record.

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
exact later-finality Commit intent can therefore rebuild the Commit quorum for
the immutable origin instead of leaving a lock with no executable owner.

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
persistence macro-step contains five effects, below the fixed limit of eight
effects per serialized adapter macro-step. When adapter debt is serviceable,
each runtime turn services exactly one deferred adapter macro-step before timers
or newer ingress; this decreases that debt without allowing one turn to
monopolize the runtime.
The adapter cannot report terminal readiness while any deferred Completion,
Progress input, or ordinary input remains.

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
a duplicate.

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
`FetchBody` producer may retain the exact causal suffix and retry; the rejected
attempt changes neither work nor request ownership, and a later successful
drain acquires both without exposing a partial allocation. An authenticated
exact `CertifiedBodyResponse` is transport-only and may cross that retained
reducer-effect debt to retire the blocking Fetch/request pair. A
`CommitCertificateResponse` is different: it exposes its authenticated
CommitQC to reducer ingress and therefore remains reducer-ordered. Before the
executor boundary, each validator has one shared outer TransportCompletion
slot and full-envelope byte reserve for either a `CertifiedBodyResponse` or a
`PayloadChunk`; generic reducer traffic and TimeoutVote ownership cannot spend
either reserve. The source-sealed finite matrix for this boundary now contains
6 models and 28 configurations: 10 repaired cases and 18 mutants, producing
147 generated and 146 distinct states. Its weak-fairness result requires a
responsive certified source plus terminating transport and body work. Those
are explicit premises; the finite evidence promotes no proof-ledger
obligation.

The adapter's deferred Progress reserve is partitioned by consumer ownership:
one locked-Commit slot and one independent TimeoutVote slot per frozen
validator, plus one slot for each PrepareQC, CommitQC, and TC class. Its exact
capacity is therefore `2 * roster_len + 3`. Exact retransmissions coalesce
before this capacity check. Vote ownership is signer-injective: a distinct
Commit or TimeoutVote from the same signer cannot consume a second slot or
displace the admitted owner, and becomes admissible only after that owner's
slot is serviced. Once a progress item is admitted, later equal- or
higher-ranked traffic cannot displace it; a full class rejects the new item
while preserving the already admitted vote, reconstruction, or certificate
owner.

The semantic-admission table applies the same partition before the reducer is
called. A full ordinary-history budget cannot reject a current-view
TimeoutVote: at most one key per frozen signer bypasses that budget, remains
available for equivocation/delivery checks while the view is current, and is
retired when the view advances. Thus the reserved deferred slot is reachable
even when ordinary semantic history is saturated.

The roster-aware transport ingress also prevents auxiliary I/O backpressure
from becoming a per-validator head-of-line stall. On each source's fair turn it
removes the oldest message which the downstream consumer can currently admit;
earlier blocked messages remain owned in their original order. A certified-body
request waiting for auxiliary I/O capacity therefore cannot hide a later
proposal, QC, TC, body response, or payload chunk from the same authenticated
validator. The source still consumes only one turn, and the retained head is
selected first as soon as it becomes admissible.

An empty validator lane reserves an ordinary first-message slot, a non-timeout
Progress slot, a distinct TimeoutVote slot, and one shared
TransportCompletion slot for either a payload chunk or certified-body
response. Short non-empty lanes retain the continuation potential needed to
restore all four reservations after any service step. Each simultaneously
materialized authenticated non-validator lane has two owners: one generic
message slot and one TransportCompletion slot for a roster-origin completion
forwarded through that source. The anonymous lane has the same two owners when
the roster is non-empty, while its no-roster diagnostic geometry needs only the
generic slot because no roster-origin completion can be valid. If `H` is the
configured maximum number of simultaneously materialized authenticated
non-validator lanes, the exact non-empty-roster count minimum is therefore
`4 * roster_len + 2 * H + 2`; the no-roster minimum is `2 * H + 1`.
`H` is independent of exact-output reply-route capacity `R`, which is derived
from the effective `network.max_total_connections`. Root validation resolves
the selected lane profile before deriving `R`, so `lane_profile = "home"` with
an omitted explicit maximum uses `R = 32`, and rejects any configuration with
`H > R`.
The canonical Sumeragi shared-config projection is format version 3 because
`H` is fingerprint-bound, so incompatible source geometries cannot share a
handshake fingerprint.
Authenticated non-validator lanes are created on demand and removed when empty.
A semantic duplicate carrying an alternate authenticated reply route is merged
into the existing request before a new-lane `H` admission check; only a
semantically distinct request requiring a new source lane consumes another
lane. Non-timeout
Progress includes Commit votes, QCs, TCs, certified-body requests, and both
Commit-certificate request/response directions. Proposals, Prepare votes, and
manifests remain ordinary; TimeoutVote uses its own signer-bounded corridor;
`PayloadChunk` and `CertifiedBodyResponse` share TransportCompletion. Relay
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
authenticated validator source also retains an
isolated timeout-vote byte reserve and an independent full TransportCompletion
envelope reserve, so ordinary traffic cannot consume the capacity required to
advance a view or finish body recovery. Height activation derives the latter
with checked arithmetic from the frozen DA layout and the exact bare-Norito
framing of the largest valid `PayloadChunk` and `CertifiedBodyResponse`; an
overflow or undersized source partition fails closed before ingress opens.
These count, byte, Progress, timeout, and completion reservations prevent one
authenticated source from consuming another validator's recovery capacity or
turning the count-bounded queue into multi-gigabyte memory ownership.

### Production transport geometry

Height activation sizes every progress-relevant envelope from the frozen
context rather than from a representative fixture. Let `F(x)` be `x` plus its
canonical compact-length prefix. For a layout admitting `C` chunk hashes, the
exact bare manifest ceiling is `M(C) = F(8 + C * F(32)) + 228`. The proposal
ceiling then includes that manifest, a maximum-size signature, a timeout
certificate with one non-empty group per validator and a full PrepareQC in
every group, plus the separately carried highest PrepareQC. The recommended
layout at the protocol maximum of 128 validators is exactly 232,541 bare bytes;
the minimum one-validator, one-chunk layout is 2,302 bytes. For the recommended
16 MiB layout, the larger of the maximum `PayloadChunk` and
`CertifiedBodyResponse` envelopes is exactly 16,811,581 bare bytes.

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
hosts. Both `irohad --check-config` and normal startup reject a larger
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
514 outer-ingress entries (`4 * 128 + 2`). The effective consensus and
block-sync plaintext ceiling is the 17 MiB global cap minus the 28-byte AEAD
expansion.

Kagami-generated localnets inherit those 17/17/2/17 MiB frame caps, the
514-entry count bound, and a 33 MiB per-source byte partition. They reject more
than 128 validators and set aggregate body-ingress bytes to at least
`(validator_count + 1) * 33 MiB`, retaining the 165 MiB four-validator default
floor. The Taira template carries the same 165 MiB aggregate and 33 MiB
per-source baseline; its bundle renderer accepts 4 through 128 validators and
raises the aggregate to at least `(validator_count + 1) * body_source_bytes`.

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

Certified merge-sidecar output retains a bounded, byte-free identity of the
current per-source chunk across response-byte release, route pruning, and
reconnect. Every actor-minted writer-flush identity also owns one process-local
application claim shared by all of its clones. Worker-to-lane handoff accepts
only an acknowledgement carrying that exact shared claim, not an independently
rebuilt identity with equal ticket and delivery fields. Its cross-tool evidence
binds the opaque source key, exact admitted delivery route, and writer-claim
occurrence as three typed process-local identities; none is serialized,
persisted, or admitted to consensus state. The server validates the exact
source, tenure occurrence, ticket, request, chunk hashes, fixed cursors, and any
still-materialized bytes before consuming that claim. A
duplicate or losing late receipt is therefore a terminal no-op, and a receipt
already applied to an expired rate-gate cannot advance a later byte-identical
rematerialization. A
genuine old-writer flush which has not yet been applied may still complete the
same source's retained current chunk once while a reconnect retries it; sibling
sources keep their independent cursors and reservations.

Response materialization authority is one-shot, but transient failure does not
make a pending semantic request permanently terminal. A failed local lookup,
loss of the current route, or outbound count/byte pressure clears the
process-local authority and leaves that exact pending source retryable; a
completed race or successful immutable-byte installation consumes
retryability. Before any newly materialized chunk leaves lane work, its cursor
and byte-free pending identity are durably journaled. Restart can therefore
retry the same semantic source without reconstructing a route capability or
forgetting a chunk already handed to actor output.

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
pending. The corridor freezes `roster × {Safety, Lane, Bulk}` reservations for
the height and adds a distinct non-zero shared fanout budget. A deterministic
maximum matching assigns at most one unique frozen target/class reservation to
each retained fanout and is recomputed after every attempt. Non-roster reply
identities and repeated output for an already-owned target/class are confined
to shared capacity, while partial multi-target completion reopens the exact
finished reservation. This prevents authenticated observer identity churn from
filling validator safety/lane reserves. Once Kura has returned the exact
applied-height receipt and matching
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
for a result-bearing genesis block with external entries. Missing certificate
or receipt evidence for a non-empty set leaves ingress open and the same height
active so its exact lane session can finish; duplicate, conflicting, or
non-canonical evidence fails closed. Only a complete Kura-first set authorizes
the atomic output handoff and successor construction. Each rollover attempt
rehydrates bounded, unapplied canonical ownership from Kura before collecting
completed sessions. A block synchronized after adapter construction therefore
retains its exact proposal as the request source for a peer's durable lane
certificate instead of waiting forever with global application already
complete. Native AMX
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

A successor adapter may ingest an earlier-height lane certificate only as local
historical-recovery work. Earlier-height proposals, votes, and QCs are never
emitted through the successor's generic `Lane(current_scope)` fanout: doing so
would attach the successor context to a predecessor artifact and defer the
contradiction until applied-height rollover. Once historical recovery is
durable, exact request handling uses the separately typed Kura-backed response
claims described above. Generic lane fanout is therefore restricted to output
whose embedded proposal height equals the adapter's immutable height context.

Startup applies the same rule only at the durable tip. A tip with a complete
checkpoint, manifest, and global finality artifact but incomplete canonical
lane evidence reopens that exact terminal height with global reducer traffic
closed and exact decided-lane transport enabled. It does not derive or persist
the successor until every canonical ownership has its certificate and
application receipt. Historical lane sidecars are not re-audited because
canonical lifecycle changes may legitimately retire an older incarnation; the
live crash boundary is the tip.

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
  including distinct signer count and signed/total voting power;
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
and signed power observed at that height. View and reducer generation are not
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
- `local_control_pending`

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
predecessor. A constructor or startup failure therefore
leaves `Running` visible at the finalized height, retains that snapshot with
`restart_required`, and exits fail-closed without claiming activation. The
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
and that a closed writer's replacement tenure reacquires the current sidecar
item only after exact-output capacity is available. The last transition is a
capacity-checked reactivation, not a cursor reset: the old test which treated
an unflushed reconnect as terminal is renamed to assert the required retry.
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
certified sidecar control-bucket regression, brings the current source-bound
inventory to 585 exact tests across 39 modules and 82 pre-network legs.
Its canonical module/test TSV inventory SHA-256 is
`b6dc528ea6e07d75a243a5b4ead89c83561aa9d0a74b9ac941cdf1a744078acd`.
Nine of those legs execute the separate 256-test G-UNIT focus inventory; its
canonical source-derived TSV SHA-256 is
`f45d6752f3dc449415446abc7340a3531ed48db5e9e744bccc6c1bba0d7a8957`.
Together, the closures bind proposal-origin reducer/deferred identity,
equivocation evidence, aggregate signatures, finality/header geometry, compact
offline QCs, parent height-context identity, source-scoped sidecar limits,
worker-to-network chunk-admission receipts, runner route
preservation, worker backpressure, actor-global deferred capabilities,
scheduler ownership handoff, opaque delivery ordinals, and daemon Hold/Release
failure behavior. The rollover tests cover
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
An inactive sidecar source retains its authenticated-source session and byte
reservation together with its incomplete chunk cursor; neither reconnect nor a
wall-clock TTL can erase that ownership. A delayed reconnect therefore
rematerializes shared bytes at its retained chunk rather than restarting at
chunk zero, while a newly authenticated alternate source starts at chunk zero.
Completed attempts remain terminal until an authenticated cumulative Close
retires their contiguous semantic prefix.
The requester sequence/floor state and responder gate, source-budget, cursor,
and pending-chunk identities are atomically journaled under the authenticated
Kura root. Restart rebinds only a freshly authenticated process-local route;
it never reconstructs a capability from disk. A lifecycle-journal failure
latches the process-wide output guard before an allocated request, queued
chunk, timeout rotation, Close, or CloseAck can publish later consensus output.
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
The current inventory retains the four-per-validator, two-per-materialized
authenticated-non-validator, and two-anonymous owners (`4N+2H+2` total)
capacity-negative boundary and the exact
PrepareQC count-and-power quorum regressions. Its four integration tests run
together under their module filter; the complete pre-network corridor now has
82 legs, including separate exact status and atomic lane-certificate decode
contracts, nine G-UNIT execution-receipt legs, the source-attested Native AMX
fixture check, two `iroha_config` geometry modules, the two new `iroha_p2p`
geometry modules, the shared-byte-budget module, the daemon genesis module,
plus source-sealed workspace formatting, the legacy-codec guard, workspace
build, Clippy, workspace tests, and feature-enabled `irohad` command-success
legs, the G-SCALE runner/validator preflight, plus three proposal-origin
data-model module legs. Immediately before completion publication, the runner
also revalidates the source-bound localnet binary bundle. The data-model modules are
discovered and executed against `iroha_data_model`; they cannot fall through to
the `iroha_core` runner.
The current 585-test inventory is a mechanically checked
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

The focused source-bound additions above are green. The gate names nine
completion-ownership regressions: exact ingress/Busy-deferred
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

The strict proof-run counts in the following paragraphs are retained
historical submodule evidence, not current aggregate source-manifest-bound
release evidence. The canonical 54-entry proof ledger currently reports 33
`tlaps_proved`, 14 `specified_unproved`, 6 `trusted_contract`, and 1
`out_of_scope` entry, with `machine_checked_completion: false`. The legacy-named
locked-body-reproposal entry now denotes locked-origin direct-commit progress;
it and the production cross-tool refinements remain explicitly
unproved; no bounded model or source-fidelity check promotes them. An honest
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

Before those longer scenarios, the PR gate inventories 585 exact production
liveness tests and executes all 39 owning Rust modules serially. The release
profile additionally records nine G-UNIT legs executing a separate 256-test
focus inventory. The
inventory includes the reducer exact-lock and adapter consumer-epoch
regressions, plus five lane-work tests which pin the native-AMX signing guard's
explicit runtime bound, exact hard boundary, above-bound fail-closed behavior,
record/anchor byte ceilings, and production-like adapter limits. It also pins
adapter-owned successor activation, runner ingress
handoff, watchdog predecessor/successor separation, and recovery-derived
successor identity. The worker leg also pins rejection of an unissued future
physical acquisition and exact latest-consumer rebind across unavailable-body
recovery. The authoritative ingress leg pins `4N+2H+2` count potential, the
TimeoutVote and TransportCompletion byte reserves, frozen-layout wire-size
activation, cross-validator isolation, and fair service; the
adapter/runtime legs pin the independent `2N+3` Busy-deferred partitions and
runtime Progress admission. They also pin the five-effect maximum flattened
persistence macro-step below the eight-effect bound, exactly one serviceable
deferred adapter step per runtime turn, and refusal of terminal readiness while
any deferred Completion, Progress input, or ordinary input remains. The adapter
leg also realizes the complete
`1024 + 2N` semantic-admission bound, retains current-view signer slots,
retires old-view TimeoutVote delivery records, and exercises non-poisoning
same-owner retry across TC installation. The block-sync leg pins
reducer-enqueue ownership, strictly sequential context catch-up, and canonical
Kura body service by a certified historical signer. Nine tests pin the
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
durable reducer retransmission reconstructs the blocked Fetch. They raised the
preceding inventory to 222.
Six outer TransportCompletion-corridor regressions raise it to 228; explicitly
pinning the then-current four-per-validator plus two shared relay-lane owners
(`4N+2` total, before authenticated non-validator lanes were separated)
capacity-negative raises
it to 229; and the four-validator exact PrepareQC count-and-power quorum test
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
The current reconciliation adds 16 exact tests and two P2P geometry modules
plus the daemon genesis module. It separately replaces one obsolete lane-relay
name in place, yielding a net total of 439 tests across 29 modules. Its Kura,
refinement, effects, recovery, runner, watchdog,
P2P, and genesis entries pin replay metadata, successor authority/lifecycle,
discovered CommitQC admission, source geometry, and clone-safe producer/fanout
ownership. The lane-relay saturation test was renamed in place, so the module
still contributes four tests. The subsequent in-flight sidecar redelivery
regression raises that total to 440 without adding a module or corridor leg
and binds one exact writer-flush owner per source chunk. Three subsequent
worker regressions produce the historical 443-test checkpoint. They bind same-tenure
pending/unapplied flush deduplication, mixed-source terminal-route history, and
capacity-checked reconnect reactivation of the unflushed current item.
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
the certified sidecar Close/CloseAck critical-bucket regression brings the
current inventory to 585 tests across 39 modules and 82 legs. The rollover slice covers
historical Kura CommitQC, body, and lane-certificate rereads; current global
V2; lane proof/supersession; Native AMX; merge-share, certified-sidecar, and
untyped fail-closed boundaries. The route slice pins semantic deduplication,
one independent attempt per authenticated source, actor-global delivery
ordinals, connection-tenure-bound tickets, source-owned non-regressing cursors,
and bounded route-set capacity. Stream sequence and cumulative-close metadata
are excluded from the canonical semantic request identity. Completed requests
have no wall-clock expiry: an authenticated cumulative close advances only
over a contiguous terminal prefix and is the sole mechanism that retires the
covered server output. Admission capacity is preflighted before either the
close floor or server stream state can advance. A later delivery changes only
its source's route and preserves that source's current immutable payload,
cursor, FIFO age, and reservations. A reconnect keeps the source's FIFO
identity, clears the retired tenure-bound ticket, and retries the source's
retained current item or chunk through fresh tenure admission while leaving
sibling-source progress
untouched. A newly observed alternate source starts independently at zero.
These newest tests pin local typed retirement, ownership, and fail-closed
behavior; they do not claim end-to-end relay/application acknowledgement or
unbounded broadcast admission. The integration filter remains a five-test
module leg, while separate P2P, daemon, status, Nexus lane-relay, and atomic
lane-certificate contracts bring the aggregate pre-network corridor to 61
legs. That set needs fresh discovery and execution plus its clean source-sealed
release rerun; the full PR corridor is not claimed passed.

The current reconnect changes supersede older mutable-tree diagnostics that
assumed writer continuity across connection tenure. Fresh focused worker,
sidecar, lane, and transport-route tests plus the complete detached
source-sealed pre-network corridor remain required before release evidence may
be promoted.

The same pre-network gate inventories and executes four exact, non-ignored
Taira release-profile validators plus the Rust summary-JSON schema contract.
It then requires exactly 39 passing mocked soak launcher/evidence tests. Those
tests reject profile drift, zero-test success, concurrent evidence ownership,
source or artifact mismatch, malformed JSON, weakened acceptance bounds,
inconsistent counters, invalid provisional evidence, and inconsistent status
classifications. They validate the release machinery; they are not substitutes
for the 24-hour validator soak.

Both profiles compute the canonical tracked/untracked checkout manifest before
their first Cargo command, bind the ignored workspace `Cargo.lock` as an
explicit build input, and reject unresolved index entries and every active
merge, cherry-pick, revert, mailbox-apply, rebase, sequencer, or bisect
operation. Administrative paths are resolved through `git rev-parse
--git-path`, so an operation in one linked worktree is not mistaken for an
operation in another. The PR profile may still record intentionally dirty
tracked or non-ignored untracked source; its exact bytes remain part of the
manifest. It exports that digest to the network matrix and uses it as the
build-root identity. The seed runner
compares that parent digest after test inventory, before and after every
scenario, and on both sides of completion publication; the PR profile also
recomputes it after the formal harness. Any drift leaves only partial evidence
and fails the corridor.

Before any network attempt, the gate requires exactly 30 source-manifest and
seal contract tests to pass. They cover content and ordering, deletions,
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
process-safety policy forbids an outer watchdog from signalling Cargo, rustc,
or validator process groups, so it cannot promise a wall-clock bound if an
execution path escapes all of those internal deadlines. Such a pathological
run may remain active, but it remains fail-closed evidence because it cannot
publish `COMPLETED.tsv`; it must be inspected and resolved before retrying.

The production corridor accepts only one clean committed candidate: HEAD and
its tree must resolve, the index tree must equal HEAD, tracked files must be
unchanged, and no non-ignored untracked path may exist. It recreates that exact
commit in a unique detached worktree, copies and re-hashes the ignored
`Cargo.lock`, and requires the detached pre-seal identity to equal the original
candidate identity. It then redirects `target`, temporary files, caches,
evidence, and retained localnets outside the detached source and removes source
write permission before re-entering the complete release script. This chmod
seal makes ordinary editor, build-tool, and accidental writes fail; it is a
cooperative integrity control, not an adversarial same-UID security boundary.
The owning UID can chmod, write, and restore modes between checkpoints.
Identity and seal checks run after each major leg, while the detached committed
worktree removes the caller's mutable checkout from the execution path.

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
transcripts. It resolves the repository-pinned Cargo 1.93.1 and rustc 1.93.1
binaries through rustup, records their paths, versions, and hashes, uses an
isolated `CARGO_HOME` without external configuration, and permits only the
source-bound `.cargo/config.toml`; registry and Git caches remain a cooperative
host prerequisite for offline execution. Java, Git, Python, Node, Bash, TLAPM,
TLA2Tools, Verus, and cargo-verus identities are likewise bound where used.

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
and requires all 39 mocked soak launcher/evidence adversaries to pass. The soak
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
protocol 3, no restart-required node, the complete liveness object, and bounded
queue evidence. Every retained no-progress interval is accepted only when its
canonical classification set exactly matches the blockers in its authoritative
status snapshots. A checkout-manifest checkpoint immediately after the
100,000-height chaos run rejects drift before starting the 24-hour soak. A final
checkout-manifest and proof-evidence check rejects any later source change
during the long corridor.

A production run cannot be started by invoking the candidate runner directly.
The release operator first authenticates an out-of-tree copy of
`bootstrap_sumeragi_v2_release.py`, the protected Python, Git, OpenSSH
`ssh-keygen`, and Bash executables, the manifest and identity helpers, the SSH
allowed-signers and revocation policies, and every expected SHA-256 digest and
signer fingerprint. The protected interpreter must start in isolated, no-site
mode; the evidence parent must already be owner-owned mode `0700`, and the
requested evidence child must not exist. The complete invocation passes those
protected paths and digests explicitly, for example:

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
  --runner-tool-manifest /protected/runner-tools.json \
  --expected-runner-tool-manifest-sha256 <sha256> \
  --bash-bin /protected/bash --expected-bash-sha256 <sha256> \
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
archived protected tools and a private `runner-bin` directory. Every additional
executable must be named in the protected canonical manifest with one absolute
source path and SHA-256 digest; the bootstrap rejects writable or untrusted
ancestors and creates an exact-target symlink in `runner-bin` so platform code
signatures remain valid without reopening ambient path lookup. The runner
independently
validates that marker at entry, after sealing, and at every release-identity
checkpoint before it executes another candidate helper. The bootstrap imposes
no outer runtime or output-capture limit on the runner and never signals its
process group; a runner which escapes its internal deadlines remains visibly
incomplete and cannot publish either completion marker.

Runner stdout and stderr are inherited regular files created directly under
the private evidence directory, mode `0600` while active and `0400` after a
normal exit. Bootstrap output consumers therefore cannot backpressure the
runner. If the bootstrap process alone is interrupted, it does not signal the
runner and preserves the active logs and evidence directory for diagnosis;
without terminal validation it cannot publish external completion.

On success, the runner publishes exactly
`release-runner/output/release/RELEASE_COMPLETED.json` beneath the bootstrap
evidence directory. That receipt binds the 82 pre-network corridor legs and
their exact 585-test production inventory, the separate 256-test G-UNIT
inventory, semantic test names/counts, commands, logs, source-bound localnet
binary attestation, and resolved tool identities; the formal completion, pinned harness lock, formal
toolchain, proof ledger/evidence/log; all 160 matrix logs; the chaos
completion/log; and the exact-identity Taira completion/canonical JSON/full run
log. It independently revalidates matrix, chaos, and Taira libtest markers and
runs the Taira evidence checker against the archived canonical JSON.

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
writer uses an exclusive staged inode, complete-write loops, no-clobber
linking, and cleanup which removes only its own inode. Before publication it
revalidates and synchronizes every bound evidence file, then the complete
evidence-directory closure bottom-up; after publication it repeats that
closure with the terminal receipt included. Any file, directory, or `fsync`
failure is fail-closed. There is no mutable pointer file. The successful runner
retains its sealed source root and sealed identity instead of deleting the code
which produced the evidence. Once the runner returns success, the external
bootstrap validates that retained root, re-synchronizes and re-attests its
permission-aware manifest, then invokes the separately protected archived
receipt validator in exact `--verify-existing` mode against that root. Only
after the protected replay succeeds does it create the separate no-clobber
`BOOTSTRAP_RELEASE_COMPLETED.json` marker. That external marker binds the
terminal receipt, sealed source identity, protected validator, and sealed
runner-log digests. Runner failure is returned unchanged and cannot publish
external completion.

This authenticates the signed candidate and runner relative to the operator's
protected inputs; it is not remote host attestation. The release-host image,
dynamic loader and libraries before Python starts, the owning UID, and trusted
ancestor-directory owners remain external prerequisites. A malicious same-UID
process or trusted ancestor can still attack the pathname namespace between
checks. Durability also assumes the host filesystem and storage honor POSIX
`fsync`. These limitations are recorded in the bootstrap marker rather than
being hidden behind a cooperative-receipt claim.

The release command is intentionally fail-closed while
`docs/formal/sumeragi_v2/proof_coverage.json` contains any
`specified_unproved` obligation or reports
`machine_checked_completion: false`. Bounded TLC searches and convincing paper
arguments do not upgrade that ledger state.
