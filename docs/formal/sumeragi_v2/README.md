# Sumeragi v2 formal verification

This directory is the first-release formal corridor for the production
Sumeragi v2 consensus protocol. There is no legacy Sumeragi proof corridor.
The model fixes protocol revision 3 and is parameterized over arbitrary finite
frozen rosters; production separately enforces the release limit of 128
validators. Mechanization status is recorded per obligation in the proof
ledger. The first-release implementation likewise has one canonical decoder:
an omitted `proposal_round` is invalid rather than interpreted as the vote or
certificate round.

## Modules

- `SumeragiV2Quorums.tla` and `SumeragiV2QuorumProofs.tla` define and prove
  strict count-and-power quorum intersection.
- `SumeragiV2Availability.tla`, `SumeragiV2CrashRecovery.tla`, and
  `SumeragiV2Reconfiguration.tla` define durable-body, WAL, restart, and frozen
  height-context boundaries. `SumeragiV2VocabularyProofs.tla` checks their
  small helper facts separately, keeping the executable vocabulary theorem-free
  for exact parameterized instancing in the indexed chain refinement.
- `SumeragiV2Core.tla` models addressed asynchronous delivery, durable intents,
  locks and highest PrepareQCs, grouped timeout certificates, future-view
  recovery, old-view CommitQCs, body recovery, decisions, and application.
- `SumeragiV2TimeoutDurability.tla`,
  `SumeragiV2TimeoutSigningInvariant.tla`,
  `SumeragiV2TimeoutViewInvariant.tla`, and
  `SumeragiV2TimeoutWireAuthorization.tla` prove the timeout-envelope frontier
  from WAL ownership through signing and honest transport. Every honest timeout
  is bound to the frozen voter roster, context and height, carries an
  authenticated PrepareQC high reference, and satisfies `highRank <= view`.
  These envelope modules do not by themselves discharge the historical
  TC-lock authorization or the dependent direct-or-installed-authorization
  timeout wrapper; the full action induction below discharges both.
- `SumeragiV2SafetyLemmas.tla`, `SumeragiV2AgreementLemmas.tla`,
  `SumeragiV2Inductive.tla`, `SumeragiV2InductiveProofs.tla`, and
  `SumeragiV2Proofs.tla` contain the action-by-action safety induction and its
  end-to-end theorems.
- `SumeragiV2ChainEpoch.tla`, `SumeragiV2ChainEpochProofs.tla`, and
  `SumeragiV2ChainEpochRefinement.tla` model prefix-comparable per-validator
  histories and frozen epoch routing from exact durable CommitQC decisions and
  exact local application receipts. The refinement contains both the selected-
  height safety product and an indexed family of dormant, admissible
  `AsyncSpecAt` instances. Exact application receipts join successors one node
  at a time through an ordered activation pipeline. The node joins only when an
  Applied or exact durable-tip Recovered path publishes a full-context token.
  In the production refinement, an internal State application maps to that
  Applied boundary only after the canonical lane-ownership completion gate;
  proving this delayed mapping remains explicit proof debt.
  Validators absent from an old roster use the production-shaped authenticated
  historical CommitQC/body service and explicit decision, body-recovery, store,
  validation, and application stages. Terminal observers record known
  application without creating a successor or join. Certification and local
  application do not use a global all-node barrier. The production trace
  mapping and temporal multi-height induction remain explicit proof debt.
- `SumeragiV2EffectiveLockAcquisition.tla` is an executable, height-scoped
  locked-body owner. It keeps a physical load ID and subject immutable across
  same-lock consumer rebinds, defers a higher different-subject replacement
  until the active load terminates, classifies stale/future/wrong/duplicate
  completions, and retries only after the exact certified body becomes durable.
  `SumeragiV2EffectiveLockAcquisitionProofs.tla` owns the separate deductive
  model obligation; the ordinary Rust worker/runtime refinement is a distinct
  asynchronous proof obligation.
- `SumeragiV2AsyncNetwork.tla`,
  `SumeragiV2AsyncFairnessRefinementProofs.tla`,
  `SumeragiV2LivenessProofs.tla`,
  `SumeragiV2CertifiedRequestHashAuthorityProofs.tla`,
  `SumeragiV2DurableDecisionRecoveryProofs.tla`, and
  `SumeragiV2AsyncLivenessProofs.tla`
  model the production scheduler and transport abstractions, own the typed
  fair-action-to-`AsyncNext` refinement proof, and state the conditional
  progress obligations after GST. The two recovery modules separate the exact
  generation-free signed-request/hash authority from the generation-scoped
  executor candidate and prove the Commit-only durable Decision lifecycle.
  The volatile vote pool is the delivery-epoch witness: an exact vote crosses
  once while present, and TC installation clears the local pool. If the
  resulting lock has the node's exact durable Commit intent, acknowledgement
  queues that intent for re-signing; signature completion reconstructs the
  local pool before sending only to the other voters. Peer retransmission of
  that exact locked Commit enters reserved progress admission in the new
  reducer generation. Every Commit delivery, including a current-view one,
  additionally requires the recipient's exact durable lock. A pre-lock current
  Commit is a recoverable ignore; after `LockAndCommit` acknowledgement applies
  the lock, the adapter advances its locked-Commit consumer epoch and may admit
  that exact vote once. The acknowledgement prunes the superseded historical
  pool before releasing the current Commit signature. Retained outbound control
  remains peer-delivery evidence, not a sufficient local witness because
  broadcast excludes the sender. The protected deferred lane has one slot per
  locked-Commit signer, one slot per TimeoutVote signer, and one class-wide
  PrepareQC, CommitQC, and TC slot each: exactly
  `2 * |ValidatorIds| + 3` owners. Exact duplicates coalesce, while a distinct
  same-owner item retries without displacing another protected slot. Immutable
  authenticated history remains separate from this consumer state. Fair
  transport ingress requires at least `4 * |ValidatorIds| + 1` entries. The
  potential separately reserves an empty or non-timeout-progress-deficient
  validator source, every validator's missing TimeoutVote, every validator's
  missing shared TransportCompletion owner, and the continuation required when
  servicing a lane would recreate any reservation; anonymous and non-roster
  senders share the final untrusted slot. The
  inductive invariant also records that every individual source lane is at
  most the aggregate ingress capacity, matching the runtime admission gate;
  this makes one-item removal decrease the counted depth by exactly one even
  when preservation is checked from an arbitrary invariant state. Each
  authenticated validator source also leaves a distinct configured 64 KiB
  timeout-vote reserve unavailable to ordinary traffic (in addition to body
  envelope headroom). That isolated region exceeds the conservative 4 KiB
  maximum valid timeout-vote envelope, including a 128-signer PrepareQC. A
  validator lane owns at most one distinct queued TimeoutVote in that region;
  transport copies coalesce while that lane owns the exact envelope, and the
  authenticated delivery record continues coalescing while the corresponding
  candidate is deferred, causal, queued, or completion-owned. A new occurrence
  starts only after scheduler ownership ends. Timeout delivery additionally
  requires the complete canonical envelope domain before inspecting its
  authorization fields; the augmented-record counterexample and typed async
  adapter refinement are TLAPS-proved. Thus auxiliary byte saturation or a
  field-compatible non-canonical record cannot invalidate the message-level
  timeout admission boundary. `PayloadChunk` and `CertifiedBodyResponse` share
  a separate one-entry validator owner and a full canonical-envelope byte
  partition. Its checked bound mirrors bare Norito framing from the frozen DA
  layout; overflow or an undersized partition rejects height activation before
  the ingress opens, while generic Progress and TimeoutVote traffic cannot
  spend the completion partition. Production now routes every live lane-local
  message through this same fair owner: lane executable payloads and handoffs
  use TransportCompletion, while lane votes, proposals, QCs, certificates, and
  new-view traffic use Progress. Their exact wire ceilings are four MiB and one
  MiB, respectively. The byte-abstract asynchronous model uses its existing
  completion and progress representatives; proving the concrete class and byte
  mapping remains part of the production-refinement obligation.
  Resource lanes are keyed by the authenticated transport hop (`via`), while
  validation, response routing, and exact-wire coalescing retain the semantic
  origin. `SumeragiV2ReplyRouteOwnership` composes canonical semantic identity
  with one bounded attempt per authenticated source. Actor-global delivery
  ordinals remain distinct from source connection tenure. Exact and later
  same-tenure deliveries preserve message/chunk cursors, and a reconnect
  preserves the source's current cursor while replacing its writer tenure; a
  newly observed alternate source starts at zero. Network admission tickets
  bind tenure plus semantic identity rather than delivery ordinal. A
  per-semantic round-robin cursor isolates source progress while one immutable
  payload carrier is shared. When one semantic attempt advances the
  source-scoped tenure, later deliveries rebind each retained semantic attempt
  for that source independently; a new-tenure rebind clears only its old
  ticket and preserves that attempt's cursor. The route progress formula now
  requires eventual stable-responsive service eligibility and a strict cursor
  advance or completion; losing a
  ticket no longer satisfies it. This remains an obligation, not a promoted
  proof. The cross-tool refinement must still
  prove the production origin/`via` projection and runner/worker/sidecar call
  paths; the abstract transition kernel and its cursor-reset/alternate-source-
  replacement TLC mutations do not establish that mapping.
  The production transport refinement uses exact checked canonical geometry at
  every layer. With `F(x)` denoting compact-length framing, the manifest bound
  is `F(8 + C * F(32)) + 228`; the proposal adds the maximal grouped TC,
  separately carried highest PrepareQC, and signature. The recommended
  128-validator proposal is 232,541 bare bytes, and the recommended maximum
  transport completion is 16,811,581 bare bytes. `CertifiedBodyRequest`
  carries a maximal PrepareQC, `CommitCertificateRequest` carries the actual
  frozen chain id, and `CommitCertificateResponse` carries a maximal CommitQC.
  Control takes the maximum of proposal and Commit-certificate response.
  Requesters, rotated responders, P2P origins, and direct targets use the
  protocol-wide 8,258-byte raw public-key payload ceiling rather than a roster
  sample. That feature-independent bound includes the largest accepted SM2
  distinguishing identifier and point. The checked refinement then covers
  `BlockMessage::V2`, header-framed `BlockMessageWire`, boxed
  `NetworkMessage::SumeragiBlock`, the direct relay and complete
  `Message::Data`, 28 AEAD bytes, and the four-byte encrypted-frame queue
  prefix. Although the wire body prefix can represent `u32::MAX`, production
  uses a deterministic 2,147,483,643-byte runtime/configuration ceiling so the
  prefix plus body fits a contiguous `i32::MAX`-byte buffer on 32-bit and 64-bit
  hosts. Startup rejects a larger global cap before binding and rechecks each
  encrypted length with checked arithmetic before encryption. Prefix-inclusive
  queue charge is an optional checked value; `None` rejects activation even at
  a `usize::MAX` configured queue rather than acting as an equal sentinel.
  The abstract packet-publication step also collapses production actor
  admission, encoding, frame and batch ownership, socket write, and flush into
  one transition. The reliable production path must retain the exact source,
  target or remaining broadcast cursor, byte owner, and attempt identity until
  the matching writer flush acknowledgement. This actor-to-flush trace and its
  decreasing service rank remain explicitly `specified_unproved`. A writer
  flush is only a local transport-attempt witness: it does not acknowledge
  final-target receipt through a relay, subscriber consumption, or application.
  A reliable broadcast snapshots the actor-accepted relay-aware topology and
  acquires each ordinary `(target, class)` lane independently. It may coalesce
  with an existing target child only for the identical canonical request digest
  in the same membership tenure. A distinct payload or direct/broadcast
  cross-kind collision retains an exact per-target FIFO ticket with the caller;
  there is no class-wide parent. Explicit topology removal cancels only the old
  broadcast tenure, remove/re-add creates a new generation, and direct-post
  ownership survives. This closes the known local parent-residual obstruction,
  while subscriber consumption, relay/final-application acknowledgement, true
  target-geometry exhaustion, and direct-post ownership after target removal
  remain separate production-refinement debt. Broadcast starvation freedom
  remains unproved.
  Configure and open both reject overflow or an undersized ingress, topic,
  global-frame, or high-queue owner. The default instantiation uses
  17 MiB global/consensus/block-sync settings, 2 MiB control, a 128 MiB
  high-priority byte queue, and `4 * 128 + 2 = 514` outer-ingress entries.
  Kagami and the Taira renderer reject rosters above 128 and preserve one
  aggregate source partition for every validator plus the shared untrusted
  lane by scaling body bytes to at least `(N + 1) * body_source_bytes`.
  The production reducer's shared Rust/Verus refinement gate checks the local
  EnterView selection boundary: the persisted TC, pre-install lock,
  post-install durable lock, effect-carried lock, and immediately following
  recovery fetch must agree on the effective maximum lock. The executable
  acquisition owner makes the body-rebind and recovery state machine explicit,
  and the pinned strict TLAPS run proves all 1,258 obligations in the complete
  `SumeragiV2EffectiveLockAcquisitionProofs` module. The ledger therefore marks
  `EffectiveLockAcquisitionModelObligation` as `tlaps_proved`, covering abstract
  type closure, acquisition progress, and stable repeated delivery. This does
  not prove that every production executor, runtime, worker, request, byte, and
  queue owner refines and fairly services that model; that remains the separate
  `EffectiveLockBodyAcquisitionProductionRefinementObligation`, also
  `specified_unproved`.
  Generation-scoped vote delivery, concrete runner scheduler preservation, and
  the dependent one-height `AsyncSpecAt` type-closure wrapper are ledgered
  `tlaps_proved`; the fresh hash-guarded strict evidence is summarized below.
  Deadlock freedom now requires an enabled
  productive step that grows height evidence, consumes concrete deadline debt,
  or decreases/exits a protected candidate or Serve-occurrence rank. The weaker
  scheduler-enabled lemma cannot discharge it, so the productive obligation
  remains explicit proof debt. `StarvationFreedomObligation` likewise remains
  proofless: its conditional precursor lemmas have source proof bodies, but the
  release-facing theorem cannot be closed ahead of its still-unproved
  service-rank prerequisite. The durable progress
  witness and the remaining stable-suffix liveness declarations are likewise
  explicit debt, so this is not a machine-checked completion claim.
  The Core vote-delivery relation and normalized trace replay encode the exact
  durable-lock Commit gate and post-WAL pool pruning. A recorded strict run
  made before the current edits discharged all 7,826 induction obligations and
  all 565 downstream Core safety obligations. Those historical submodule
  results close only the historical TC-lock and timeout-protection ledger
  entries; the asynchronous liveness
  proof-premise repairs remain outstanding.
  Logical views are unbounded in the deductive liveness abstraction; finite
  TLC configurations remain counterexample searches only.
- `proof_coverage.json` is the checked-in theorem/trust-boundary status
  declaration, not independent proof authority. The structural checker binds
  every status to the exact source theorem, and release status additionally
  requires fresh generated evidence under `target/formal/sumeragi_v2/`.
  Backend obligation counts belong only in that generated evidence.
- `SumeragiV2ResumeVoteWitness.tla` and
  `resume_locked_commit_witness.cfg` ask TLC to produce the bounded
  lock-and-Commit recovery trace: a timed-out view-0 Commit remains the exact
  active lock, a TC advances the node to view 1, a crash/restart contributes the
  second generation increment, and `ResumeVote` reconstructs the historical
  signing request. The deliberately violated negated predicate is a regression
  witness, not a release invariant or deductive proof.

## Exact protocol abstractions

`ContextRecord` binds the chain and protocol identities, semantic parent
finality, height, epoch, canonical roster and powers, lane/DA commitments, and
the already-computed production leader start. Certificates must satisfy both
`3 * signer_count > 2 * voter_count` and
`3 * signer_power > 2 * total_power`; observers never pad either threshold.

The source-shared reducer separates three round domains. Its lifecycle owner
tag is the current `(height, view, generation)` authorized to make a local
transition. The signed `proposal_round` is the immutable origin of the proposal
manifest, block header, body, and validation receipt. The Vote/QC `round` is
the certification or finality round. Prepare requires equal proposal and vote
rounds. Commit requires equal context and height and permits only
`proposal_round.view <= round.view`. The owner tag is neither a proposal-origin
field nor an inferred certificate round.

Honest Proposal, Prepare, Commit, and Timeout signatures require their matching
acknowledged WAL intent. A timeout vote carries the full highest durable
PrepareQC, and a TC contains disjoint signer groups whose union independently
satisfies both quorum thresholds. Installing a TC may move one validator to
`tc.view + 1`; it does not require other validators to install first. Prepare
intent replay remains current-view and timeout-fenced. An already-durable
Commit intent may resume signing after a timeout or later TC only when it still
matches the validator's exact active lock proposal origin and subject;
unrelated proposal origins and superseded Commit finality intents remain
fenced. TC acknowledgement performs the same
exact-origin check before queuing a Commit re-sign in the active finality round.
If the TC instead promotes an exact lock for which this node lacks a Commit
intent, installation does not sign. After exact body storage and
current-generation validation, the normal `BeginLockCommit`/WAL path may create
that later-finality intent only when no higher local Prepare origin or known
PrepareQC exists. A higher same-subject Prepare still names a different
proposal origin and therefore fences the historical Commit. The local
signature completion inserts the vote directly into the new volatile pool
because the P2P broadcast excludes its sender. An old-view CommitQC remains
decisive after a view change. All received Commit votes require the exact active
durable lock origin and subject. A premature current-view Commit stutters
recoverably. Once the matching `LockAndCommit` acknowledgement makes a later
finality intent durable, it prunes every older Commit pool for that same origin
before signing and advances the adapter's consumer epoch so the exact vote may
enter once. Recovery and progress witnessing select only the newest durable
Commit intent for the active `(proposal_round, subject)`.

An installed lock is committed directly. Equal canonical bytes proposed in a
later view would have a different proposal origin and are rejected; a selected
higher PrepareQC installs its own exact origin instead of authorizing another
re-proposal.

The source-shared refinement projection carries a pending WAL record's primary
proposal origin independently from its lifecycle owner. When the record embeds
another certificate, it also carries that certificate's auxiliary proposal
origin. The begin boundary, acknowledgement boundary, requested `Persist`
capability, and independently reconstructed grant must match both origins and
the pending record. There is no correlated requested/granted substitution path
which can rename either origin.

A certified chain slot is created only from the exact valid CommitQC in a
durable decision receipt for the canonical parent context. Each validator
fetches, reconstructs, validates, durably applies, and advances independently
from its own exact receipt. Lagging validators remain on their certified
prefix and cannot be advanced by another node's receipt.

The asynchronous model includes pre-GST loss, duplication, reordering, crash,
and replay. After GST it models bounded authenticated per-source transport,
normal/progress/completion ingress reserves, view-indexed absolute timeout
priority, periodic retransmission, command-service debt, cyclic class-aware
dispatch with FIFO order inside each class, stale-completion rejection,
manifest and chunk recovery, validation, and independent application. The
scheduler choice matches the source-linked production kernel:

1. the current view's absolute timeout;
2. an owed class-aware command;
3. one periodic retransmission;
4. the first command selected by the cyclic class cursor; or
5. idle service.

Textual TLA+ disjunction order is never treated as priority; the selected-work
operator makes the branches mutually exclusive.

Local admission has a separate two-source cursor for producer completions and
causal work. If a producer turn is taken while causal work is waiting, the
model records sticky causal-admission debt and advances the cursor. The debt is
cleared only by selecting the causal source; once its head is admissible, debt
makes that source the deterministic preference under the existing fair
`RunNode` action. Thus a continuously replenished producer cannot erase the
causal source's turn.

Body transport is typed syntactically over all `Subjects`, not only
`ValidSubjects`. This keeps authenticated-but-invalid reconstructed bodies in
the adversarial state space. Deterministic validation is the semantic boundary:
a valid proposal body records current-view/current-generation validation,
whereas an invalid proposal body follows `RejectBody` and cannot authorize a
Prepare vote. An invalid body for an already durable decision remains a
fail-closed error, matching the production progress-witness check.

Every available, durable, validated, and invalid body record binds its exact
proposal origin. `retainedLockedBodies` preserves that origin across lifecycle
generation and finality-view changes. Recovery may move ownership to a new
process-local consumer, but it cannot rewrite the manifest or validation
receipt to the consumer's view. Retained authority alone cannot authorize
voting or application: the body, deterministic execution commitment, and
validation receipt must match the CommitQC's authenticated `proposal_round`.
The canonical header view-change index also equals that proposal-origin view,
not a later Commit certification view.

The production authority boundary is stricter than signature validity alone.
An individual Vote may consume an execution commitment only after the exact
round and subject are bound by a local validated receipt, verified WAL replay,
or quorum-authenticated QC evidence; a signed Vote cannot create that binding.
Unbound Votes remain recoverable input rather than local fail-stop evidence.
Body-availability rebind requires the installed destination tag, preflights the
source and destination ownership sets, and either moves one exact source or
coalesces it into one exact destination. An uninstalled destination is a
recoverable caller-contract rejection with no mutation. Conflicting or
duplicate ownership fails closed, and every pipeline or Decision retirement
invariant is checked transactionally before mutation. Conflicting certificate
evidence received through body-recovery request/response transport is rejected
nonfatally and leaves retry ownership available.

The timeout service budget charges at most three class-aware dispatches for
each of the `AsyncQueueCapacity` same-class positions of a protected admitted
occurrence.  Each dispatch is separated by the conservative runner-cycle bound
`AsyncQueueCapacity + 2 * AsyncIngressCapacity + 3`; the extra ingress term is
the sound over-approximation implied by the scalar runner-budget type rather
than an assumption that every reachable phase starts at its tight reset value.

Shared-config projection version 2 binds this pacemaker rule into the handshake
fingerprint. A retired fixed-timeout binary therefore cannot silently
participate in the same height and supply premature timeout votes against the
view-growing liveness argument.

## Theorem scope and FLP boundary

Safety is asynchronous: it permits arbitrary delay, loss, duplication,
reordering, Byzantine messages within authenticated identities, and crashes at
effect boundaries. The safety argument and release obligations cover durable
sign-once behavior, external validity, certified-body availability, lock
protection, agreement, absence of conflicting CommitQCs, crash/restart
preservation, chain-prefix safety, epoch-context isolation, and both the
grouped-timeout kernel and the corrected timeout disjunction: a formed TC
either directly protects a potential Commit quorum or exposes an honest
timeout/Commit intersection witness with durable installed-TC authorization.
Their exact mechanization status is recorded per obligation in
`proof_coverage.json`;
inclusion in this scope is not itself a claim that the obligation is proved.
Crash/restart preservation is a behavior-scoped temporal contract: under the
selected core specification, every crash or restart preserves the durable
projection, interrupted writes stay unacknowledged, and stale generations are
rejected.

Ten arbitrary-context Core safety wrappers are TLAPS-proved over
`CoreSpecAt(initialContext)`: durable-vote uniqueness, lock monotonicity,
external validity, certified-body availability, certificate uniqueness,
agreement, conflicting-CommitQC exclusion, crash recovery, historical TC-lock
Commit authorization, and the dependent direct-or-installed-authorization
timeout wrapper. The narrower grouped-timeout kernel for Commit intents already
present at timeout remains proved as well. The authoritative
receipt-driven chain model also TLAPS-proves chain-prefix comparability and
epoch-boundary isolation; those two obligations are not redirected to the
one-height Core wrapper.

Liveness is necessarily conditional. FLP rules out unconditional deterministic
consensus termination in a fully asynchronous network. The post-GST paper
argument therefore has explicit premises: a non-crashing honest set
independently meets both quorum thresholds; authenticated retransmissions and
serialized service have declared finite representable bounds; the monotonic
clock and run loop continue; and admitted fsync, signature, reconstruction,
deterministic validation, and local application work terminate. The immutable view-zero
deadline grows linearly as `base * (view + 1)`, while retransmission retains its
fixed base interval. Consequently some post-GST view exceeds the complete
bounded service rank without assuming in advance that one configured fixed
deadline is already adequate. Under those premises, failed views form and
install TCs. An undecided execution either decides early or reaches a view in
which the responsive honest scheduled leader itself is active, and that leader
state leads to responsive decisions. Each responsive validator's durable
decision independently leads to certified-body recovery, validation, and
application, after which its local chain advances.

The scheduler-owned protected rank includes Completion and Progress work plus
a canonical constructor-shaped Normal proposal/Prepare slice: initial or
post-TC `AssembleBody`, causal `BeginPrepare`, and the frozen Normal delivery
shape for Proposal, PrepareVote, and CommitVote items. Reachable delivery
ownership originates at authenticated ingress, but the admitted class remains
protected after view movement even if dynamic classification would now call the
same CommitVote historical Progress. Authenticated TimeoutVote/DeliverTimeout
uses its own signer-keyed protected Progress slot. Each accepted certified-body
or Commit-certificate recovery request receives a fresh live Serve nonce whose
FIFO position is its occurrence-level rank, so equal request values remain
distinct. This intentionally over-approximates reachable constructor families
without promising service to authenticated junk. The composite rank and
starvation obligations remain `specified_unproved`.

All 18 weak-fairness targets are complete transition actions, not inner
scheduler fragments. Four exact outer-frame categories bind the Core height
context, crash/recovery state, and runner service deadline according to the
variables each action may change. `AsyncFairActionAt` inventories the same
quantified actions as `AsyncFairnessAt`. `AsyncFairActionsRefineAsyncNext` is
the typed source claim; the dedicated
`SumeragiV2AsyncFairnessRefinementProofs!AsyncFairActionsRefineAsyncNextObligation`
theorem proves that every member is one canonical `AsyncNext` transition at a
Core-plus-scheduler typed state. A recorded strict submodule run was green at
1,143/1,143 obligations; it is not current aggregate release evidence. The
Core transition relation is deliberately not
conjoined inside each `WF` target: doing so makes TLC re-search unrelated Core
branches while evaluating `ENABLED`. The structural checker pins the four
frames, all 18 action classifications, both quantifier inventories, the typed
claim, and the exact dedicated theorem inventory. The finite TLC specs and
deductive specs share the same `AsyncAllVars` and `AsyncFairnessAt`; no
TLC-only fairness relation exists. This promotes the fair-action refinement
entry. Independently, the complete post-Decision timeout-frontier induction
described below is also `tlaps_proved`, as is the exact durable Commit-Decision
crash/restart/replay lifecycle. Runner scheduler preservation and the dependent
async type invariant are now `tlaps_proved`. Fresh hash-guarded strict TLAPS
slices exited 0 for transport/runner closure (186/186 and 204/204), the recovery
execution hierarchy (305/305), its strong caller and bracket (63/63), the exact
type obligation (16/16), and the named always-strong wrapper (10/10).
After the TLAPM repin, a strict `--nofp` range run over the complete
`AsyncSpecAlwaysProgressOwnershipInvariant` proof exited 0 at 15/15
obligations. A fresh dependency-ordered proof wave then checked the current
`SumeragiV2AsyncLivenessProofs.tla` source at SHA-256
`2fe00973f8f983fd7682667baadbccbbe422a6c34dec02205cfe9cfe697f95ec`
with pinned TLAPM `3ab43c7`, strict mode, disabled fingerprints, and complete
theorem ranges. Progress ownership proved 15/15 obligations at lines
33324--33348, Serve FIFO proved 15/15 at 42007--42049, Stage 4 proved 9/9 at
46313--46342, and Stage 5 proved 9/9 at 46344--46373; every invocation exited
zero. Declaration-only `--line` attempts selected zero obligations and exited
12, so they were rejected as evidence.
The abstract protected-rank prerequisites are now isolated as
`async-progress-ownership-invariant`,
`protected-service-rank-stage4-ready-causal`,
`protected-service-rank-serve-fifo`, and
`protected-service-rank-stage5-consensus-fifo`. All four were
`tlaps_proved` for the superseded transition relation. Progress ownership
consumed the then-proved async type closure; Stage 4 and Stage 5 consumed
progress ownership, and the three rank leaves used their exact fair-action
prerequisites. The new higher-origin fence, no-high proposal admission,
same-round TC lock upgrade, and durable-timeout witness invalidate those
Core/Async receipts until a fresh strict rerun. The
aggregate `protected-service-rank` obligation depends on every one of these
leaves, without conflating the abstract model with production admission,
runtime, ingress, or actor-to-flush ownership. The 54-entry ledger therefore
has 10 `tlaps_proved`, 37 `specified_unproved`, 6 `trusted_contract`, and 1
`out_of_scope` entries and keeps `machine_checked_completion: false`.

The target statement is exactly: after GST, with a responsive dual quorum and
terminating local work, every height eventually decides and every responsive
validator eventually applies it. It makes no termination claim during an
unbounded partition, without a responsive dual quorum, or while admitted disk,
signing, reconstruction, validation, or application work does not terminate.

This is the conditional consensus-height progress target, not a completed
machine-checked liveness theorem and not a transaction-fairness claim. A valid
empty heartbeat can satisfy progress. Transaction inclusion, mempool fairness,
and censorship resistance are explicitly out of scope in the proof ledger. It
must remain described as a paper argument while
`machine_checked_completion` is false.

The mechanization boundary is narrower than the argument above. The universal
historical TC-lock authorization and its dependent direct-or-installed-
authorization timeout wrapper are now ledgered `tlaps_proved`, together with
the narrower grouped-timeout kernel for Commit intents already present when
timeout votes were made. The independent post-Decision frontier is also
`tlaps_proved`: its action-by-action induction covers every Core branch,
including crash and durable timeout replay through `ResumeTimeout`, brackets
Core-stuttering scheduler steps, and lifts the invariant through `AsyncNext`
and the temporal asynchronous specification. That proof does not assume or
consume `AsyncTypeInvariantObligation`. The durable Decision recovery proof
separately derives same-node/same-context Decision uniqueness and pending-
persistence exclusion by a complete Core action induction. Its authority is an
unapplied durable Commit Decision only: Prepare certificates are explicitly
rejected. Crash and authenticated restart preserve the generation-free logical
certified-request identity, restart advances only the executor generation, and
replay clears the old registration before installing one exact current-
generation `FetchBody` candidate. Generic body/validation/application stage
preservation and the Rust-to-TLA trace mapping remain in the independent
progress-witness debts. The universal `AsyncTypeInvariantObligation` and its
concrete runner-preservation prerequisite are now ledgered `tlaps_proved` under
the fresh strict slices summarized below; the timeout-view, locked-origin
direct-commit, rotating-leader, and application liveness declarations remain
`specified_unproved` as well. The rotating-leader declaration is a two-stage
claim: reach a view where the responsive honest scheduled leader itself is
active (or decide first), then decide from that leader state. The application
ledger entry now names the proofless per-validator
`ApplicationCompletionProgressObligation`: after GST, each responsive
validator's own durable decision must lead to its own durable application.
The legacy-named `LockedBodyReproposalProgressObligation` rejects vacuous view
movement. Its first-release statement must be read as locked-origin progress:
every stable available retained lock must eventually be committed directly at
that immutable proposal origin, be decided, or be superseded by a higher
certified Prepare lock. Re-proposing the same bytes at another origin is not an
allowed implementation step. Renaming and restating that still-unproved TLA+
symbol is part of the fresh formal-source closure.
`ApplicationLivenessObligation` derives the aggregate clause used by height
composition from that premise using durable application monotonicity, the
frozen responsive-voter set, and finite induction over validator prefixes. It
does not add a global apply barrier, discharge the per-validator pipeline, or
promote the ledger entry before a fresh pinned strict proof. The concrete
genesis chain product separately
records its first-successor handoff, when a successor height exists, as
`GenesisHeightSuccessorHandoffObligation`. That theorem also has a source proof
body but remains `specified_unproved` until the strict proof succeeds after
rotating-leader, application liveness, and the explicit
`SuccessorActivationStarvationFreedomObligation` and
`SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation`
are proved. At the terminal finite
horizon the handoff is explicitly vacuous rather than manufacturing a
successor instance. The activation obligation uses the minimal scalar carrier
`0..19`: the pre-failure path adds 9 to the remaining pipeline distance, so a
fail-closed reset at any stage still strictly descends before authenticated
recovery resumes the one-shot suffix. Its rank, enabledness, fairness, and
starvation clauses range over responsive validators only. An honest validator
outside `Responsive` may retain work queued before GST without violating the
conditional production target; the model does not manufacture local-worker
fairness for that validator. The release-facing activation theorem now has a
source proof body that composes its six exact structure, rank, non-orphaning,
stability, progress, and starvation theorems. It remains ledgered
`specified_unproved` until the complete module passes the pinned strict TLAPS
runner; source composition and SANY parsing are not a machine-checked
discharge.
The separate production-refinement seam is also intentionally stronger than
the already proved abstract successor invariant. Its release theorem conjuncts
`ProductionSuccessorAndExactRecoveryTraceRefinement`, an exact six-boolean
inventory for Applied and Recovered publication, fail-closed startup failure,
authenticated historical-certificate import, the ordinary historical body
pipeline, and terminal application without activation. Those booleans are
unassigned: source-order checks, adversarial Rust tests, stale-token mutations,
and source-manifest binding constrain the corresponding traces but do not
prove refinement. The theorem therefore has no proof body and remains
`specified_unproved` until machine-checked cross-tool trace evidence establishes
all six claims; the abstract model theorem cannot be reused as its discharge.
The chain refinement now models an indexed family of authoritative
`AsyncSpecAt` instances and exposes the exact
`SumeragiV2ChainEpochRefinement!HeightLivenessObligation`. The conditional
indexed composition immediately above it has a source proof body covering
responsive-validator instance activation and exact-action fairness on the
all-joined suffix, authenticated exact recovery for responsive validators
absent from an old roster, and finite-height temporal induction. The
release-facing theorem itself remains proofless, explicit
`specified_unproved` debt until its prerequisites are discharged and a fresh
pinned strict proof succeeds
after rotating-leader, application liveness, successor-activation starvation,
and that production-refinement seam are proved. Its exact completion
predicate requires application
at terminal `MaxHeight`; at every nonterminal context it requires each
responsive validator to advance into a successor context. Exact recovery opens
one per-instance scheduler target and imports only an authenticated canonical
CommitQC into the ordinary reducer. It then follows the same decision,
body-fetch, store, validation, and application transitions as live consensus;
it cannot create finality or skip a consumer boundary. Production may make the
first discovery attempt immediately when startup recovers durable v2 ownership
or an interrupted applied tip, and may carry that urgency only when an
authenticated Commit-certificate response yields a discovered CommitQC which
is admitted to, or coalesced with, serialized reducer ownership. The
outstanding-request `Some`-to-`None` transition proves only that ownership
handoff, not reducer execution, Decision, durability, or historical-Kura
provenance. Ordinary live finality clears the hint. This alters scheduling
only: the request, authentication, frozen context, exact CommitQC admission,
and serialized reducer path are unchanged, and ordinary heights do not acquire
permanent discovery fanout. The concrete scheduling mapping remains part of
the `specified_unproved` production refinement. Nonterminal application
queues the ordinary activation pipeline without joining in the abstract
indexed model, while terminal application uses `RecordKnownApplication` and
creates no successor work. Production's earlier internal State application
maps to that abstract boundary only after canonical lane completion; this
includes bounded rehydration of exact canonical ownership which block sync may
install after adapter construction. This delayed mapping remains part of the
unproved production refinement. Dormant
non-genesis instances
retain their exact `InitAt` parent receipt internally, but only current-context
receipts enter the global ChainEpoch projection, so the indexed genesis is
non-vacuous. The first-release model does not restore a
favourable-network relation, global asynchronous shadow state, or a second
consensus transition relation to stand in for that proof.

The ledger also names the previously implicit intermediate obligations.
Generation-scoped delivery is ledgered `tlaps_proved`. The executable
height-scoped owner now gives the body-rebind/recovery kernel its own
`EffectiveLockAcquisitionModelObligation`; the complete pinned strict TLAPS
module proves all 1,258 obligations and the ledger records it as
`tlaps_proved`. Bounded TLC remains complementary regression evidence. The
end-to-end executor/runtime/worker/request/byte/queue mapping and fair-service
composition remains separately ledgered as
`EffectiveLockBodyAcquisitionProductionRefinementObligation`, which remains
`specified_unproved`. Historical TC-lock Commit authorization and the
direct-or-installed-authorization timeout induction are `tlaps_proved` from
the full action induction. Post-GST deadlock freedom excludes a bare clock,
runner, or view-change step: it requires current-height evidence growth,
concrete deadline-debt decrease, or protected-rank decrease/exit. Its remaining
stage, packet, and zero-deadline cases are still `specified_unproved`. Durable
progress-witness preservation, strict protected-service-rank decrease, and
starvation freedom intentionally remain `specified_unproved`; adding the
vocabulary is not machine proof. The release-facing witness now uses
`AsyncDurableCommitProgressWitness`: when a responsive crash clears
`signVotes`, only the exact recovery phase, node, and generation may carry the
durable Commit until WAL replay reconstructs an ordinary carrier. The source
decomposition covers that Commit carrier and protected deferred ownership;
historical TC-lock reconstruction and exact decision-pipeline preservation
remain explicit proof gaps inside the same proofless obligation. A bounded
mutation shows that the volatile-only witness fails on the crash transition
while the exact authority-aware witness survives repaired replay; it is a
counterexample regression, not deductive discharge.

## Evidence and release gate

The operator-facing conditional guarantee, liveness snapshot, watchdog
classifications, and executable PR/release commands are documented in
[`../../source/sumeragi_v2_liveness.md`](../../source/sumeragi_v2_liveness.md).

The release gate uses TLAPM commit
`3ab43c7ff31db4ced850619d4746fa4c841a7681`, the immutable TLA2Tools 1.7.4
release, and Verus `0.2026.05.31.5dd6d83`. The rolling TLA2Tools 1.8.0
pre-release is deliberately not used because upstream replaces its release
asset with master builds, which makes a fixed archive checksum unreproducible.
The TLAPM installer likewise addresses the exact GitHub release-asset object
IDs recorded by the upstream
[build run](https://github.com/tlaplus/tlapm/actions/runs/29682668751), then
checks the platform archive SHA-256 and the embedded commit identity. TLAPM's
rolling-release workflow deletes each superseded object, so an unavailable
pinned object is an explicit availability failure; the installer never falls
through to the newer same-name asset. Operators retaining the exact original
archive may pass its path with `TLAPM_ARCHIVE_PATH`; the same digest and tool
identity checks remain mandatory.
The TLC and replay entry points also verify the exact SHA-256 digests of the
`Functions.tla` and `Folds.tla` files from that pinned TLAPM commit before
adding them to `TLA-Library`; an inherited alternate standard library cannot
change bounded-search semantics.
The strict TLAPS runner checks every deductive module,
then generates evidence bound to the exact ordered module list, every proof
log, the pinned tool identity, and a SHA-256 manifest of every TLA+ source.
The checked-in ledger cannot contain stale tool-run counts.

The structural checker rejects top-level TLA+ assumptions/axioms, unledgered
omitted proofs, Verus assume/admit/trusted-body escapes, non-theorem ledger
targets, retired Sumeragi paths, and the former favourable-network liveness
corridor. A proofless release theorem is accepted only at its exact pinned
module and symbol while the ledger records it as `specified_unproved`.
Every validation mode rejects `machine_checked_completion=true` while any such
entry remains. Promotion order is also explicit: async type closure depends on
proved runner scheduler preservation; Decision restart-recovery depends on
type closure; the aggregate progress witness depends on type closure,
generation-scoped delivery, the independently proved post-Decision frontier,
Decision restart-recovery, and the production refinement seam; deadlock
freedom depends on proved type closure; starvation freedom depends on the
proved service-rank theorem; rotating-leader progress depends on the legacy-
named locked-origin direct-commit obligation; and
genesis handoff and indexed height liveness each depend on proved
rotating-leader, application liveness, successor-activation starvation, and the
exact-recovery production refinement.
Release mode additionally requires fresh source-bound evidence.

Before network startup, the executable wrapper inventories 444 named tests
across 32 Rust modules. The preceding 298-name inventory was produced from the
264-name inventory by adding
37 positive regressions: 10 bind per-target exact-output scheduling and typed
historical/current applied-height rollover; 2 bind peer-writer flush and
old-generation dispatch-worker custody; 20 bind exact progress-ticket identity
and rank, relay-aware topology geometry, explicit removal semantics, generation
replacement, identical-retry coalescing, exact per-target FIFO ownership for
distinct/cross-kind collisions, and actor-side subscriber backlog transfer; and
1 binds exact CommitQC coalescing across the runtime queue and Busy-deferred
adapter owner; and 4 bind exact Nexus lane-relay ownership and source fairness.
Removal of the obsolete adapter cursor alias and two superseded network
broadcast-residual tests made that net delta 34. Relative to the resulting
298-name inventory, the current closure has a net increase of 146 and an exact
total of 444. The canonical module/test TSV inventory SHA-256 is
`1c1bb3bc1ec30704e2944707db653e53519695712cbdd0f0670468e45f891512`. The
additions bind semantic request identity, per-source route/cursor ownership,
writer-flush identity, sidecar source limits, runner/worker route preservation,
daemon Hold/Release failure handling, actor-global deferred capabilities,
scheduler ownership handoff, and fail-closed ordinal/debt boundaries.
The newest proposal-origin slice binds reducer and deferred identities,
equivocation evidence, aggregate signatures, finality/header geometry, compact
offline QCs, and parent height-context identity to the authenticated origin.
Its genesis regression additionally permits canonical view-zero bytes to make
their first proposal appearance in a later certified round. The integration
runner regression fixes the restart cadence's derived view-zero deadline at a
contention-tolerant 20 seconds; its whole-item token SHA-256 is
`13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f`.
That four-validator scenario also owns sequential missing-height discovery and
catch-up. A diagnostic run observed the full 20-second delay at each missing
height. The fresh exact run of
`sumeragi_v2_runner::authoritative_v2_finalizes_through_validator_restart`
against recovery-scoped eager discovery passed 1/1 in 79.82 seconds; all four
peers shut down gracefully with empty stderr. This is focused regression
evidence only and does not promote the formal ledger.
The successor-boundary regression preserves the predecessor CommitQC context
through wire-to-core conversion; its whole-item token SHA-256 is
`ee773b00e696822c6d2ba998fb88201bb6e2a06eac749a2c700edec70dbbdf74`.
The extended authenticated-admission regression is sealed at
`1cb4736b2e4b499403c870cc3dd5ab8ccd361d51887efad4178ed7d39a9e0225`.
These tests deliberately do not claim remote application acknowledgement,
relay second-hop completion, or unbounded
broadcast admission. The 264-name baseline added 32 regressions for atomic lane
certificates, semantic-origin/authenticated-via ownership, P2P source fairness,
daemon relay quotas, and the active watchdog. The 232-name baseline already
included two exact locked-Commit
progress-witness regressions and six outer TransportCompletion-corridor
regressions. The current
geometry pins four owners per validator plus two aggregate-untrusted owners
(`4N+2` total), including a roster-origin completion relayed through an
untrusted authenticated hop, and retains the capacity-negative boundary. It
also retains one four-validator exact PrepareQC count-and-power quorum
regression. The five integration names execute under one module-filtered leg;
the complete pre-network corridor now spans 55 legs, including separate exact
data-model status and atomic lane-certificate decode contracts. Its finality,
offline compact-QC, and height-context proposal-origin modules each use a
dedicated `iroha_data_model` leg. The inventory
executes the `iroha_p2p` library with its empty default feature set. It does not
claim the feature-gated QUIC first-packet geometry tests as part of those
thirty-two modules or fifty-five legs. The inventory includes five native-AMX lane-work
capacity regressions, adapter/runner/watchdog successor-activation boundaries,
exact recovery-derived successor identity, authenticated exact historical
recovery, post-decision timeout/TC quiescence, and the exact
`4N+2`/`2N+3` admission boundaries in addition to exact-lock,
completion-ownership, future-acquisition rejection, rebound durable retry, and
executor-batch boundaries. Those adapter boundaries pin a maximum flattened
persistence macro-step of five effects within the reducer's eight-effect bound,
service at most one Busy-deferred adapter macro-step per serialized runtime
turn, and require the Completion, Progress, and Normal deferred queues all to
be empty before terminal readiness. A production-default capacity regression
saturates the 256 certified-request owners, 640 Normal ingress slots, and the
128-slot reserved Progress increment while retaining the 256-slot Completion
reserve; an exact authenticated `CertifiedBodyResponse` with a still-live
matching logical request registration retires its old request owner. The
durable reducer source then retransmits the still-reconstructible Fetch, which
acquires both owners atomically without an executor-retained partial owner.
The wrapper also runs exact mocked contracts for active Git operation
rejection, detached source sealing, the 160-run matrix launcher, the
source-bound 100,000-height chaos receipt, provisional Taira evidence
promotion, and the aggregate release receipt. These execution contracts are
not deductive proof. A fresh pinned strict whole-module aggregate release TLAPS
run, the complete clean source-sealed PR corridor, the source-bound chaos run,
and the 24-hour Taira-profile soak remain pending.

The formal mutation gate also runs one source-sealed post-Decision model with
nine configurations. The repaired trace completes with status 0. Eight
single-seam mutants—one for each of `BeginTimeout`, `ResumeTimeout`, `FormTC`,
and `BeginInstallTC`, two receive-pool admission branches, and two
causal-successor branches—must fail at their named invariant with status 12.
This finite matrix binds the executable model to the reducer/source-fidelity
contract. The separate strict action induction discharges the post-Decision
timeout frontier. The independently checked durable Decision module discharges
the exact Commit-only crash/restart/replay handoff; neither finite matrix
discharges the production-trace refinement sentinel required by the aggregate
progress witness.

A second source-sealed registration matrix runs one certified-response model
with five configurations. Three repaired duplicate, authenticated-restart,
and historical-catch-up traces require a response to match an exact currently
registered certified request. Two missing-guard mutants accept either the
second fan-out response after the logical request is retired or a delayed
pre-replay response after restart clears volatile request ownership; both must
fail their named invariants. This matrix pins rejection of unsolicited and
replayed signed responses while retaining the intended historical recovery
corridor. It is bounded regression evidence complementing, but not replacing,
the strict deductive crash/restart/replay authority proof.

A third source-sealed lifecycle matrix executes the exact proof split directly.
Its repaired seven-step trace keeps durable Commit authority and logical
request registration generation-free while advancing a separate executor
generation, then clears the registration and installs one exact current-
generation `FetchBody`. Eight single-seam mutants reject duplicate same-node/
same-context Decisions, generation or recipient fan-out in the logical
registration, replay retention, a dropped FetchBody, stale executor generation,
Prepare authority, and a non-singleton replay queue. Across the nine configs,
TLC generates 42 states, all distinct; the fixed case returns status 0 and each
mutant returns status 12 at its named invariant. This is regression evidence
for the strict theorem, not production trace refinement.

Production release execution accepts only a clean committed HEAD and must be
entered through the operator-authenticated out-of-tree bootstrap under a
protected `python3 -I -S`; direct candidate-runner entry fails before another
candidate helper. The bootstrap authenticates and privately archives its exact
tool/helper/policy inputs and the candidate's SSH-signed identity before it
launches the bound runner under a closed environment. It imposes no outer
runner timeout or output-capture bound and never signals the runner process
group.

The runner reproduces the candidate in a detached read-only worktree and
records both the original checkout manifest and the permission-aware sealed
manifest. Manifest modes cover enumerated file/symlink entries; a separate seal
walk checks directories and rejects source symlink escapes, writable-output
targets, and hard-linked regular files. Child builds and evidence bind the
sealed manifest actually compiled. The canonical aggregate receipt additionally
binds original HEAD/tree/`Cargo.lock`, all 55 pre-network legs and the exact
444-test inventory, the pinned harness lock and resolved toolchain, the formal
ledger/evidence/log, all matrix logs, chaos log, and exact-identity soak
evidence. Its no-clobber, file/directory-`fsync` publication has no mutable
pointer; after success the external bootstrap independently validates it and
publishes a separate no-clobber completion marker. The complete operator
contract is documented in
[`../../source/sumeragi_v2_liveness.md`](../../source/sumeragi_v2_liveness.md).

This authenticates the signed candidate and runner relative to the operator's
protected inputs, but is not remote host attestation. The host image,
pre-Python dynamic loader, same UID, trusted ancestor owners, and correct
storage `fsync` semantics remain external prerequisites. Malformed, incomplete,
cross-source, semantically mismatched, or digest-mismatched evidence is
rejected. Cargo/rustc are
resolved to the repository-pinned 1.93.1 toolchain, run with sanitized semantic
environment overrides and an isolated configuration-free `CARGO_HOME`, and
their exact paths, versions, and hashes are retained in the corridor receipt.
Java resolution rejects a non-working launcher such as the macOS
`/usr/bin/java` stub, honors an explicit `JAVA_BIN` only when it executes, and
otherwise selects a canonical working JDK from `JAVA_HOME`, `PATH`, the
repository-local JDK, or stable package-manager links. The selected binary's
canonical path and SHA-256 remain receipt-bound.

Run the full gate from the repository root:

```bash
bash scripts/formal/install_sumeragi_v2_tlapm.sh
bash scripts/formal/install_sumeragi_v2_tla2tools.sh
bash scripts/formal/install_sumeragi_v2_verus.sh
bash ci/check_sumeragi_formal.sh
```

TLC requires exact exhaustive completion from two bounded configurations, one
exact deterministic-simulation transcript from each of four configurations,
and the exact deliberately violated invariant from the locked-Commit recovery
witness; none of these outcomes changes proof status. Its liveness configuration uses
finite `65535` timeout and view ceilings, above the configured complete service
budget and within the
pinned TLC 1.7.4 integer evaluator; the deductive model keeps these constants
symbolic. The ChainEpoch simulation uses a separate full-state harness:
`ChainEpochTlcInit` initializes inherited Core state, `ChainEpochTlcVars`
subscripts it with the receipt variables, and `ChainEpochTlcNext` freezes Core
while a directly constructed receipt relation advances. The deductive
`ChainEpochNext` and `ChainEpochSpec` are unchanged, and
`ChainEpochTlcReceiptNextRefinesChainEpochNext` checks that every optimized
harness receipt is an ordinary deductive step. A recorded focused strict slice
was green at 5/5 obligations; it is not current aggregate release evidence.
Simulation
transcripts must contain one seeded header, one initial-state marker, at least
one progress marker, one exact single- or multi-unit duration footer, status
zero, and no TLC error. Before those searches, the gate runs the original eight
explicit scheduler mutation/repair pairs:

1. equal-value replacement versus exact queued-envelope coalescing;
2. deferred-owner replacement versus scheduler-wide coalescing;
3. strict deferred-class priority versus the cyclic deferred-class cursor;
4. Busy Completion requeue without cursor advance versus cursor advance;
5. same-source head-only ingress versus oldest-admissible indexed removal;
6. aggregate-only ingress capacity versus the explicit per-lane bound;
7. conflated pending-work/completion capacity versus separate capacities; and
8. producer-first local admission versus sticky causal debt and the alternating
   local-source cursor.

The protected-rank follow-up checks five additional adversarial families:

9. four causal-capacity refill lassos versus sticky class-specific debt, plus
   the exact duplicate fast path;
10. blind causal-successor replacement versus scheduler-wide exact coalescing;
11. recurring Commit-certificate discovery inside `RunNode` versus its own
    fairly scheduled auxiliary action; and
12. indexing every physical I/O job versus indexing only Consensus owners; and
13. a multiplier-one causal FIFO rank versus the doubled FIFO position that
    strictly dominates a simultaneous local-source cursor reset.

The last family is an intentionally two-state Stage-6 arithmetic slice. With
`<<Earlier, Target>>` and Causal preferred, removing `Earlier` while resetting
the cursor to Producer changes the fixed rank from `2 * 2 + 0 = 4` to
`2 * 1 + 1 = 3`. The multiplier-one mutation remains at two and produces the
pinned invariant counterexample. This establishes only the local FIFO/cursor
calculation; it does not prove that Completion causal capacity eventually
opens and does not discharge or promote the temporal service-rank obligation.
The executable continuation kernel is additionally source-bound to a Verus
sequence projection: reverse/push-front preserves continuation-before-tail,
and the projected scheduler filter excludes prior owners, retains every fresh
emitted identity exactly once in stable order, and conditionally appends them
after an unchanged old queue.
The condition is material: the supplied owner set must include the old queue
and every scheduler owner. No theorem yet maps concrete effect identities and
that complete owner union to TLA+ candidates, so this evidence remains
unpromoted.

The production exact-output seam is also source-bound by comment/literal-free
whole-item digests. The binding covers per-target and cross-fanout FIFO heads,
round-robin admission, pinned returned-post payload identity, atomic
applied-height preflight, and the exact creation scope of every typed claim.
The bounded corridor derives an immutable reservation set from the height
roster crossed with the Safety, Lane, and Bulk classes. Its physical bound is
that exact set plus a separate non-zero shared fanout capacity. Deterministic
maximum matching assigns at most one distinct frozen reservation to each
retained fanout and recomputes assignments after every delivery attempt;
non-roster reply targets and repeated same-target/class output can consume only
shared slots. Thus identity churn cannot consume unopened validator output
reservations, and partial multi-target progress immediately releases the
completed target/class reservation.
Historical CommitQC, certified-body, and lane-certificate response claims are
single-target exact identities whose finality artifact, canonical body, or
certified lane artifact is independently reread from Kura at handoff; responder,
signature, body/manifest, proposal/QCs, and response hashes are revalidated as
applicable. Current-height global V2 messages validate their protocol and bind
to the exact receipt/finality artifact. Winning lane messages require an exact,
independently readable durable Kura certificate and application receipt;
alternate vote/QC/certificate proof variants are revalidated, and structurally
valid same-height non-winning lane messages are explicitly superseded. The
winning set is reconstructed from canonical finalized-block ownership rather
than volatile output. Missing certificate or receipt evidence keeps the exact
terminal height active; conflicting evidence fails closed; successor handoff
requires the complete Kura-first set. A canonical empty ownership set is
complete even when a result-bearing genesis or external-only block contains
external entries; those entries are not lane durability obligations. Startup
audits that boundary at the tip only and reopens an incomplete tip for exact
decided-lane traffic without re-entering global reducer input. Historical
lifecycle sidecars may already be
retired and are not re-audited. Native
AMX claims pin scope, embedded round, and message hash; merge-share claims pin
scope and share hash. Certified sidecar request/chunk claims pin scope, target
roles, transfer identity, and exact request/response hash. Finalized-sidecar
pruning leaves winning data in the committed merge log and supersedes losing
pending work before handoff. Manual, wrong-identity, substituted, or otherwise
untyped `Exact` output remains owned and fails closed. This is production
source fidelity, not a proof of the QC-to-application pipeline or end-to-end
catch-up. These executable contracts remain `specified_unproved`; they do not
promote the production refinement or starvation obligations.

The gate next runs the separately source-sealed effect-capacity ownership
matrix: 6 compact models, 28 exact configurations, and one standalone runner.
The 10 repaired configurations finish with TLC status 0; the 18 mutants must
fail with their pinned invariant or temporal status. Across the complete
matrix TLC generates 147 states and finds 146 distinct states. The cases cover
the two-Fetch/full-capacity TimeoutVote-Sign prefix, protected and terminating
owner retirement, reconstructible full-capacity Fetch rejection, stable
`(class, work_id)` preemption with decided-owner exclusion, and retained-suffix
FIFO/bound/overtaking/Decision filtering. The fifth model separates the sole
certified-request slot from two general work slots. A `FetchBody` blocked only
by that independent request capacity retains its exact causal owner and is the
only producer for which that capacity result is retryable; the failed attempt
allocates neither partial work nor partial request ownership. An authenticated
exact `CertifiedBodyResponse` with a still-live matching logical request
registration remains transport-only and may cross retained reducer-effect debt
to retire the blocking request, after which the durable producer reconstructs
and retransmits the Fetch so it atomically acquires both owners.
`CommitCertificateResponse` remains
reducer-ordered because it unwraps a CommitQC into reducer ingress. The sixth
model starts both a certified response and a payload chunk behind saturated
generic count and byte ownership. Both must share a dedicated per-validator
TransportCompletion slot and byte reserve; independent classification mutants
stall each kind before the executor. The finite weak-fairness witness assumes
a responsive certified source and terminating transport and body work; it does
not establish either premise. Crash/restart reconstruction is explicitly
delegated to `SumeragiV2CrashReplayMutation`; it is not silently abstracted as
part of these live-process models. This
exhaustive finite matrix is mutation and regression evidence only. It supplies
no deductive liveness proof, changes no proof-ledger status, and promotes no
obligation.

An exhaustive one-validator ownership configuration separately checks
`AsyncProgressOwnershipInvariant` over 983,041 generated states (99,328 distinct,
depth 49). The expanded graph covers the independent non-timeout-progress and
TimeoutVote ingress reservations. The deliberately broken configurations must produce their exact
counterexamples; all repaired configurations must complete without error.

The producer-first buggy configuration has the pinned three-state fair lasso and exits
TLC with status 13; the repaired alternating configuration exhausts its
seven-state bounded graph with no error and exits with status 0. The ingress
capacity mutation separately exposes the two-state invariant failure that its
per-lane bound rejects. These bounded mutations are regression witnesses and
counterexample searches, not deductive proof and not grounds for changing any
ledger status.

Two focused seam models exercise the stronger vocabulary. An unprotected
Normal proposal/Prepare candidate has a fair starvation counterexample, and a
dynamic delivery-class mutation loses a stored CommitVote after TC; the frozen
constructor inventory closes both. Separately, a scheduler-only deadlock claim
accepts a bare tick, whereas the productive claim rejects it until a concrete
deadline, evidence, rank, or decision repair exists. These bounded checks do
not discharge the productive release obligation.

The model-trace replayer drives the exact production reducer API. For the
current proposal-origin source, the isolated source-shared harness passed
118/118 reducer/WAL/refinement tests, all 8 model-trace replay tests, and all 9
named fast-network simulations. A 2026-07-21 100,000-height chaos run completed
the permissioned and NPoS 50,000-height prefixes, 400,000 validator
finalizations, and zero failures in 91.29 seconds. These are unsealed
implementation results, not deductive evidence. The checked-in pinned Verus
receipt predates the proposal-origin changes and was not rerun for this source;
it must not be cited as discharge of the changed obligations.

## Trusted computing boundary

The proof ledger keeps the remaining premises explicit:

- signature authenticity and collision resistance;
- faithful complete-frame `fsync` acknowledgement;
- deterministic reconstruction, validation, and execution;
- a responsive honest dual quorum after GST;
- bounded post-GST authenticated transport; and
- continuing clock/run-loop service plus termination of admitted local work.

TLAPM, its backends, Verus, vstd, the SMT solver, the Rust compiler, the
production-to-proof extraction code, the operating system, and the hardware
remain part of the implementation proof TCB. Generated evidence records the
cooperative, hash-bound claim that the configured tools accepted the exact
sources; it does not authenticate either the host or the runner that produced
that claim.
