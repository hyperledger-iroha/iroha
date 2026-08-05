# Sumeragi v2 formal verification

This directory is the formal corridor for the production Sumeragi v2
consensus protocol. The authoritative protocol is revision 4. It admits only
unit-vote committees with exact `n = 3f + 1` geometry, `1 <= f <= 10`, and
therefore exactly 4, 7, ..., 31 validators with quorum `q = 2f + 1`. Stake may
affect election or eligibility; it never weights a consensus or timeout vote.

Revision 4 deterministically rotates a height-seeded roster permutation. Set A
contains the first `q` members, with the leader first and proxy tail last; Set B
contains the remaining `f`. The leader sends the proposal manifest
committee-wide and the first full-body dispersal to Set A. Set A votes directly
to the sole proxy-tail collector. The first retransmission deadline activates
same-view fallback, which preserves the proposal, view, lock, and `q` threshold
while expanding body recovery and voting to the full committee. Timeout votes
bypass the proxy tail. A certificate of `q` timeout votes rotates the leader,
proxy tail, and both sets, and resets fallback before the new view starts.

Every honest Prepare signer must first reconstruct the complete canonical body
from the mandatory Reed-Solomon-16 layout, check its hashes, durably store it,
and deterministically validate it. A durable CommitQC plus canonical
application authorizes successor activation. Retryable finalized-height
network output, lane sidecars, and cleanup are reconstructed and repaired
independently; they cannot hold the successor height inactive.
An unfinished historical lane remains governed by its immutable predecessor
descriptor, not by the successor global roster. A configured validator removed
from that roster has no successor global vote, but may continue signing any
exact frozen lane descriptor which names it. This includes unfinished old
descriptors and independently pinned current-height Nexus lane descriptors,
whose committees need not be successor-roster subsets. This adds no safety
requirement for global-roster overlap; conditional lane-output liveness
requires each frozen lane committee's honest threshold to remain responsive
until durable completion.

`SumeragiV2Revision4.tla` is the compact revision-4 model.
`SumeragiV2Revision4.cfg` instantiates the smallest production committee with
one validator designated faulty and two candidate bodies for exhaustive safety
checking. `SumeragiV2Revision4Liveness.cfg` checks the same finite full
rotation under the model's explicit post-GST weak-fairness assumptions. The
model records committee-wide manifest fanout, first-occurrence body/chunk
targets equal to Set A, committee-wide fallback targets, Prepare and Commit
routes to the current proxy tail, and committee-wide timeout routes which do
not depend on that tail. It also checks full-body-before-Prepare and Commit,
agreement, fallback reset, exact decided-body recovery before local apply, and
successor activation while finalized-output debt may remain outstanding.
`SumeragiV2Revision4AdversarialSafety.tla` is the separate non-vacuity kernel
for agreement: in one bounded round, its single Byzantine validator may vote
for both candidate bodies, honest validators may split but durably sign at
most once, and vote, QC, and decision actions remain open after the first QC.
Its exhaustive configuration checks that neither two conflicting CommitQCs
nor two conflicting decisions can become reachable.

The deductive safety and conditional post-GST liveness arguments are in
[`PROOF.md`](PROOF.md). Three proof boundaries matter when interpreting a TLC
run. `CompleteFullBody` and `RecoverDecidedFullBody` atomically abstract
reconstruction, hash checking, durability, and deterministic validation.
`PostGSTSpec` assumes weakly fair service of the honest leader, body, vote,
honest-tail QC, timeout, application, and successor actions, and suppresses a
timeout-certified departure while the leader and proxy tail are both honest;
this represents deadlines which exceed the finite post-GST service bound.
Finally, neither compact transition system enumerates a complete Byzantine
network adversary or proves its production refinement. The adversarial safety
kernel exhausts the critical four-validator/two-body vote-equivocation state
space, while the main safety configuration supplies bounded routing and
availability evidence and the liveness configuration supplies bounded
temporal evidence under the named assumptions. None is a TLAPS proof or an
unconditional termination result.

## Revision-4 files

- `SumeragiV2Revision4.tla` is the authoritative compact revision-4 protocol
  model, routing-invariant surface, and conditional post-GST temporal surface.
- `SumeragiV2Revision4.cfg` is its exhaustive bounded
  four-validator/two-body safety instantiation.
- `SumeragiV2Revision4AdversarialSafety.tla` removes the single-proposal and
  stop-after-QC shortcuts while allowing the one faulty validator to vote for
  both bodies.
- `SumeragiV2Revision4AdversarialSafety.cfg` exhaustively checks conflicting
  CommitQC and decision unreachability for that adversarial kernel.
- `SumeragiV2Revision4Liveness.cfg` uses the same geometry to check
  `ConditionalPostGSTProgress` and
  `FinalizedOutputDebtDoesNotBlockSuccessor` under `PostGSTSpec`.
- `PROOF.md` gives the unit-quorum safety argument, conditional post-GST
  liveness argument, and exact mechanization boundary.

Run only the revision-4 TLC corridor with:

```sh
bash scripts/formal/run_sumeragi_v2_tlc.sh ci revision4_safety revision4_adversarial_safety revision4_liveness
```

The repository proof-ledger checker requires all three revision-4
configurations and registers both models in the formal source manifest used by
generated evidence:

```sh
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
```

## Revision-3 archive

The modules and proof ledger described below record the superseded revision-3
transition relation and remain useful as historical proof engineering. They
are not evidence for revision 4 until their obligations are restated against
`SumeragiV2Revision4.tla` and rerun from the final source. Revision 4 has one
canonical decoder and is a fresh-genesis protocol; revision-3 wire, WAL, and
height-context state are not migrated in place.

### Modules

- `SumeragiV2Quorums.tla` and `SumeragiV2QuorumProofs.tla` define and prove
  the historical revision-3 count-and-power quorum relation.
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
- `SumeragiV2ChainEpoch.tla`, `SumeragiV2ChainEpochProofs.tla`,
  `SumeragiV2ChainEpochRefinement.tla`,
  `SumeragiV2ChainReceiptAgreementProofs.tla`,
  `SumeragiV2SuccessorActivationRefinementProofs.tla`, and
  `SumeragiV2ChainLivenessProofs.tla` model prefix-comparable per-validator
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
  The receipt-agreement bridge covers Decision and Application receipts at
  every exact `context.height + 1` slot, including the terminal
  `MaxHeight + 1` slot, by joined-source ownership and the joined Core
  instance's `DecisionAgreement`; it does not use `decidedAt` or
  `CanonicalCommitForSlot` as agreement assumptions.
  `IndexedChainSpecEstablishesExactPerSlotReceiptAgreement` has a source proof
  body and is classified as a deductively proved support leaf for
  `height-liveness`, not as an independent ledger row. Fresh strict evidence
  for the final dependency closure is still required before its consumer is
  promoted.
- `SumeragiV2EffectiveLockAcquisition.tla` is an executable, height-scoped
  locked-body owner. It keeps a physical load ID and subject immutable across
  same-lock consumer rebinds, defers a higher different-subject replacement
  until the active load terminates, classifies stale/future/wrong/duplicate
  completions, and retries only after the exact certified body becomes durable.
  `SumeragiV2EffectiveLockAcquisitionProofs.tla` owns the separate deductive
  model obligation; the ordinary Rust worker/runtime refinement is a distinct
  asynchronous proof obligation.
- `SumeragiV2ReplyWriterDeadline.tla`,
  `SumeragiV2ReplyWriterDeadlineProofs.tla`, and the paired mutation module
  isolate exact-reply actor termination. The current proof script targets one
  immutable deadline from first dispatch, ready-flush precedence over timeout,
  retirement, and connection replacement, distinct timeout accounting,
  receipt-to-target timeout-attempt equality before cursor advance,
  replacement isolation, topology exclusion, and local actor termination. The
  fixed safety induction enumerates all 15 `Next` branches.
  Its conditional cursor theorem consumes the explicit
  `ResponsiveWriterReceiptAssumption`.
  The SANY-clean `ResponsiveStrongFairnessToReceiptResidual` proof script now
  derives that assumption through persistent exact ownership, weakly fair
  reconnect/dispatch, and strongly fair admission/publication despite
  fragmented `WriterPending` intervals. The finite responsive TLC result,
  eleven state-invariant mutation counterexamples, and one witness-erasure
  action-property counterexample remain bounded evidence only. This is
  qualitative termination, not a fixed operational wall-clock SLA; a
  recovered writer may still flush immediately before its current adaptive
  deadline. The last strict TLAPS receipt predates the final witness origin,
  monotonicity, receipt-attempt, and strong-fairness theorems, so a fresh strict
  run remains pending. The three reply-writer theorems have source proof bodies
  and remain source-bound support for the still-unpromoted production progress
  refinement; they are not independent ledger rows.
- `SumeragiV2TypedRolloverHandoff.tla`,
  `SumeragiV2TypedRolloverHandoffProofs.tla`, the shared mutation module, and
  `SumeragiV2TypedRolloverHandoffRepeatedHandoffMutation.tla` isolate the
  move-only service/transport owner pair, final empty-corridor seal, exact
  predecessor and immediate-successor receipt, retry preservation, and
  late-callback isolation. The source seal covers these four modules, one fixed
  config, and 43 mutation configs (48 artifacts total). The shared matrix owns
  42 mutations; the dedicated repeated-handoff mutant bypasses the one-shot
  predecessor-transport gate after restart restore. The proof module contains
  38 theorem declarations and all 38 retain proof bodies, including the two
  conditional local-liveness theorems. Bootstrap root replacement now binds the selected
  durable snapshot to the exact initial snapshot for the current target roster;
  the existing wrong-bootstrap-projection mutant changes that target while
  publishing the old candidate and is rejected by this exact binding.
  Production ordinary rollover requires a changed
  certified roster and authenticated predecessor terminality. A sealed,
  move-only durable handoff or a semantically validated restart may instead
  fence active predecessor responder state, but only through the durable
  lifecycle journal; it commits the changed-roster generation and empty
  responder projection before clearing memory and never invents an
  authenticated close prefix. Same-roster rehydration preserves generation and
  responder ownership. The earlier strict and bounded receipts predate this
  authority-gated relation, V3 bootstrap adoption, cleanup ordering, and root
  trust boundary, so fresh validation remains pending. The proved typed
  support stays transitively bound to its reviewed `specified_unproved`
  top-level production-refinement consumer; no support row is promoted into
  the ledger.
  Neither the model nor historical bounded evidence proves eventual finality
  validation, network delivery, writer flush, recovery after failure, repeated
  rollover, or Rust-to-TLA refinement.
- `SumeragiV2TerminalIngressLifecycleProofs.tla` isolates process-lifetime
  terminal-ingress safety. Within one terminal runner instance,
  `{TerminalReadOnly, TerminalRetired}` is forward-closed and
  `TerminalRetired` is individually absorbing. The history-service owner
  exists exactly in `TerminalReadOnly`, owner exit atomically retires detached
  and ingress owners without increasing the successful-admission count, and
  successful admission cannot follow owner loss.
  Restart creates a fresh instance, and the module claims neither eventual
  service exit nor a Rust refinement. The existing terminal-application
  successor-suppression trace sentinel does not map these ingress lifecycle
  actions. `TerminalIngressProcessLifetimeAbsorbencyObligation` has a source
  proof body and is a deductively proved support leaf for the successor/exact-
  recovery production refinement. There is no fresh strict TLAPS evidence for
  the final tree, and the support leaf is not an independent ledger row.
- `SumeragiV2AsyncNetwork.tla`,
  `SumeragiV2AsyncFairnessRefinementProofs.tla`,
  `SumeragiV2LivenessProofs.tla`,
  `SumeragiV2CertifiedRequestHashAuthorityProofs.tla`,
  `SumeragiV2DurableDecisionRecoveryProofs.tla`, and
  the bounded `SumeragiV2Async*Proofs.tla` shard chain behind the
  declaration-free `SumeragiV2AsyncLivenessProofs.tla` compatibility façade,
  with
  `SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla` as its child,
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
  transport ingress for a non-empty roster has the exact minimum
  `5 * |ValidatorIds| + 3 * H + 2` entries, where `H` is the configured maximum
  number of simultaneously materialized authenticated non-validator source
  lanes. The potential separately reserves five owners per validator, three
  owners per materialized authenticated non-validator lane, and two anonymous
  owners. With no roster the diagnostic minimum is `3 * H + 1`, because no
  roster-origin TransportCompletion can be valid on the anonymous lane. A
  semantic duplicate carrying a newly authenticated reply route is merged into
  its existing request before the new-lane `H` gate is evaluated. The
  inductive invariant also records that every individual source lane is at
  most the aggregate ingress capacity, matching the runtime admission gate;
  this makes one-item removal decrease the counted depth by exactly one even
  when preservation is checked from an arbitrary invariant state. Each
  authenticated source also leaves a distinct configured 64 KiB certified-fence-escape
  reserve unavailable to ordinary traffic. Each validator additionally leaves a
  64 KiB timeout-vote reserve unavailable to ordinary traffic (in addition to body
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
  Cross-queue duplicate detection is likewise message-generic in production:
  `deferred_authenticated_message_owner` compares the complete canonical
  envelope with the Busy-deferred occurrence's retained
  `authenticated_wire_identity`, and runtime ingress repeats the exact check
  after authentication. QC-specific lookup wrappers exist only under
  `cfg(test)` to keep focused CommitQC regressions readable; they are not a
  narrower production ownership path or proof evidence.
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
  target or remaining broadcast cursor, byte owner, adaptive timeout attempt,
  and actor receipt identity until the matching writer flush acknowledgement.
  Worker and lane projections compare that timeout attempt explicitly in their
  two-phase link. This actor-to-flush trace and its decreasing service rank
  remain explicitly `specified_unproved`. A writer
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
  high-priority byte queue, `H = 2`, and
  `4 * 128 + 2 * 2 + 2 = 518` outer-ingress entries.
  Kagami and the Taira renderer reject rosters above 128 and preserve one
  source partition for every validator, every simultaneously materialized
  authenticated non-validator lane, and the anonymous lane by scaling body
  bytes to at least `(N + H + 1) * body_source_bytes`.
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
  scheduler-enabled lemma cannot discharge it. Both
  `DeadlockFreedomObligation` and `StarvationFreedomObligation` now have
  composed source proof bodies; starvation consumes the finite-runner episode
  closure and protected-service-rank progress. Both remain
  `specified_unproved` until their final dependency cones pass fresh strict
  TLAPS. The durable production progress witness and the remaining
  stable-suffix liveness declarations are likewise unpromoted, so this is not
  a machine-checked completion claim.
  The Core vote-delivery relation and normalized trace replay encode the exact
  durable-lock Commit gate and post-WAL pool pruning. A recorded strict run
  made before the current edits discharged all 7,826 induction obligations and
  all 565 downstream Core safety obligations. Those historical submodule
  results close only the historical TC-lock and timeout-protection ledger
  entries; the asynchronous liveness
  proof-premise repairs remain outstanding.
  Logical views are unbounded in the deductive liveness abstraction; finite
  TLC configurations remain counterexample searches only.
- `SumeragiV2HistoricalRecoveryTemporalClosureProofs.tla` registers exactly
  three temporal support leaves with source proof bodies:
  `IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation`,
  `IndexedHistoricalCertificateRankProgressResidualObligation`, and
  `IndexedHistoricalDecisionRankProgressResidualObligation`. Its proved
  `IndexedHistoricalDecisionStageOwnershipResidualObligation` is a separate
  safety support theorem derived from `IndexedChainSpec`, and both composition
  wrappers obtain ownership through that theorem rather than a temporal
  antecedent. All four leaves are deductively classified, source-bound support;
  their fresh strict evidence and the remaining consumer dependencies are
  still required before top-level `height-liveness` can be promoted.
- `SumeragiV2LockedBodyProposalActionProofs.tla` is a helper-only release
  module. It proves action-frame preservation and exit lemmas used by the
  locked-body corridor, but owns no ledger obligation and cannot discharge or
  promote `LockedBodyReproposalProgressObligation`.
- The theorem-free compatibility shard
  `SumeragiV2AsyncOutstandingLivenessDebt.tla` and exactly two theorem-bearing
  scratch modules are outside the release-proof surface.
  `SumeragiV2HistoricalLockedBodyRecoveryBridgeScratch.tla` and
  `SumeragiV2ProgressWitnessCrossToolScratch.tla` each recheck one
  authoritative bridge. These compatibility/scratch modules are structural
  inventory, not release evidence, and none can promote a ledger entry.
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
- `SumeragiV2AutoscaleLifecycle.tla`,
  `SumeragiV2NativeApplicationEvidence.tla`,
  `SumeragiV2AutonomousReservationCarrier.tla`, and
  `SumeragiV2QueuePlanAdmissionRegistry.tla` are the bounded multilane
  closure kernels. Their fixed configurations check storage-before-activation,
  evidence-aware retirement, fresh incarnation reuse, durable Native
  publication/pruning order, same-route control-only treatment, unchanged
  reservation identity, single ownership, a control-only autonomous anchor,
  durable full-candidate authorization, ordered two-phase release, ABA-safe
  recreation, at-most-once canonical application, exact global QueuePlan CAS,
  certificate-before-acceptance durability, Exact-gated queue eligibility,
  immutable admission tombstones, and cancellation. The closure-ledger
  predicates additionally cover atomic route publication, quorum-bound drain
  certificates, exact-incarnation retirement, V4 Native source claims,
  contiguous active routes, exact grouped application, authenticated
  manifests, startup repair/latest-index exactness, durable reservation
  ownership, route/incarnation-first merge prefixes, canonical re-execution,
  restart ownership partitioning, and observer-only monotonic stage evidence
  derived from durable State/Kura artifacts. Thirty-seven `_bug.cfg` controls
  deliberately weaken one boundary each and must produce the named invariant
  counterexample.
  `multilane_source_bindings.json` binds each kernel to current Rust items and
  semantic tokens; `check_sumeragi_v2_multilane_models.py` validates that
  structure before the default TLC matrix. The Native binding includes the
  QC-authenticated manifest builder, manifest-before-receipt publication,
  atomic latest-index write/readback, bounded latest lookup, and the
  Kura-before-WSV application boundary. The autonomous binding includes
  exclusion-aware FIFO reservation, the durable Queue/Kura release barrier,
  route/incarnation-first canonical source ordering, startup ownership
  reconciliation, and exact full-candidate signing authorization.
  The QueuePlan binding covers the shared V2 binding and coordinator quorum,
  Kura-before-wake-before-WSV public acceptance, immutable registry CAS,
  Exact-gated autonomous ownership, restart/TTL retention, and exact
  authenticated loser cleanup.
  It also binds the bounded autonomous stage projection, its durable-stage
  reducer, and the data-model stage geometry/order validation; diagnostics
  cannot advance beyond revalidated evidence or authorize consensus state.
  The same version-3 ledger machine-maps every conceptual `ML-MUT-*` ID from
  the closure ledger. `tla_counterexample` entries cover every and only the 37
  production-refinement `_bug.cfg` files. Its separate
  `layout_only_no_transition_refinement` contract binds the accepted payload
  schema V2 in `LaneExecutablePayloadV1`, QueuePlan journal V4, reservation
  journal V5, the 4096 entry ceiling, exact queue durability order, and Kura
  execution-input persistence/recovery to
  `SumeragiV2InFlightFirstRelease.tla` and its twenty mutation controls. That
  structural contract detects layout drift but does not claim a total Rust
  transition-refinement theorem. The release receipt requires the four
  refinement rows plus a separately named fifth
  `inflight-first-release-layout` row, preventing bounded layout evidence from
  being promoted into a refinement result. `MLDiagnosticsAreDerived`,
  `MLApiAuthoritySeparation`, `MLSdkAcceptSetEqualsRust`,
  `MLFixtureHasOneCanonicalOwner`, and `MLConsensusLayoutAgreement` are
  explicitly `static_release` or `differential_release` invariants with zero
  TLA mutation configs; their exact unit, endpoint, parity, regeneration, and
  legacy-codec check contracts are source-bound instead. The non-Cargo
  structural checker rejects a missing, duplicate, reclassified, or reassigned
  conceptual map and rejects declaring one of those release-only invariants in
  a TLA+ module; the owning release gate still executes each bound check.

  `run_sumeragi_v2_multilane_apalache.sh` is the second bounded positive
  checker. It accepts no length argument or environment length override,
  requires the exact source-binding check first, validates the pinned Apalache 0.52.2
  launcher and jar hashes, typechecks the four complete refinement modules
  plus the layout-only in-flight carrier module, and
  requires one exact `NoError` result at these reviewed bounds:

  | Kernel | Fixed configuration | `Next` bound |
  | --- | --- | ---: |
  | autoscale lifecycle | `multilane_autoscale_lifecycle_fixed.cfg` | 8 |
  | Native application evidence | `multilane_native_application_evidence_fixed.cfg` | 5 |
  | autonomous reservation/carrier | `multilane_autonomous_reservation_carrier_fixed.cfg` | 10 |
  | QueuePlan admission registry | `multilane_queue_plan_admission_registry_fixed.cfg` | 8 |
  | in-flight carrier (layout-only) | `inflight_first_release_fixed.cfg` | 18 |

  Twelve runner-contract negative controls reject tool-version or checksum
  drift, source-binding bypass, reduced autoscale or QueuePlan bounds, mutation
  substitution, removal of a Native invariant, a reduced in-flight bound or
  in-flight mutation substitution, a weakened success marker, and a length
  override. The default
  `run_sumeragi_v2_tlc.sh` release matrix invokes this Apalache gate after the
  thirty-seven exact refinement-kernel TLC mutation witnesses and the twenty
  exact in-flight layout mutation witnesses. Apalache does not run those
  mutations: their named-counterexample contract is owned by the deterministic
  TLC runners, while the Apalache leg accepts positive `NoError` only.

  Install and run the pinned toolchain with:

  ```sh
  bash scripts/formal/install_apalache.sh 0.52.2
  bash scripts/formal/run_sumeragi_v2_multilane_apalache.sh
  ```

  `APALACHE_INSTALL_ROOT` may relocate the verified installation, and
  `APALACHE_BIN` may point at that relocated launcher; the runner still
  verifies the exact launcher, jar, and reported version. A successful run
  writes current logs under
  `target/formal/sumeragi_v2/multilane_apalache/` and atomically publishes
  `target/formal/sumeragi_v2/multilane_apalache_evidence.tsv`. The evidence
  source-seals every model, configuration, formal runner/checker, binding
  ledger, and bound production source before and after checking, then records
  each model/config/log hash, bound length, and exact `NoError` result. A
  failed or source-drifting run removes or withholds the completion evidence.
  These finite checks constrain four source-bound production-refinement
  declarations consumed transitively by the top-level refinement debts and one
  explicitly layout-only carrier kernel. They are
  not independent ledger rows, TLAPS evidence, or cross-tool proof evidence,
  and they do not establish a Rust transition-refinement theorem.

### Exact protocol abstractions

`ContextRecord` binds the chain and protocol identities, semantic parent
finality, height, epoch, canonical roster and powers, lane/DA commitments, and
the already-computed production leader start. Certificates must satisfy both
`3 * signer_count > 2 * voter_count` and
`3 * signer_power > 2 * total_power`; observers never pad either threshold.

The source-shared reducer separates three round domains. Its lifecycle owner
tag is the current `(height, view, generation)` authorized to make a local
transition. The signed `proposal_round` is the immutable origin of the proposal
manifest, block header, body, and validation receipt. The Vote/QC `round` is
the certification round. Both Prepare and Commit require
`proposal_round == round`; split-round Commit is invalid. The owner tag may be
in a later lifecycle view while resuming an already-durable old-round vote, but
it is neither a proposal-origin field nor an inferred certificate round.

Honest Proposal, Prepare, Commit, and Timeout signatures require their matching
acknowledged WAL intent. A timeout vote carries the full highest durable
PrepareQC, and a TC contains disjoint signer groups whose union independently
satisfies both quorum thresholds. Installing a TC may move one validator to
`tc.view + 1`; it does not require other validators to install first. Prepare
intent replay remains current-view and timeout-fenced. An already-durable
Commit intent may resume signing after a timeout or later TC only when it still
matches the validator's exact active lock proposal origin and subject;
unrelated proposal origins remain fenced. TC acknowledgement performs the same
exact-origin check before queuing that unchanged old-round Commit re-sign.
If the TC instead promotes an exact lock for which this node lacks a Commit
intent, installation does not sign. Exact body recovery retains the safe value
for an unchanged later-view re-proposal. That new proposal must be stored and
validated under its new same-round manifest, acquire a same-round PrepareQC,
and durably append its matching `LockAndCommit` before Commit signing. A higher
same-subject Prepare names that later proposal origin and legitimately
supersedes the historical lock. Local signature completion inserts the vote
directly into its exact-round volatile pool because P2P broadcast excludes the
sender. An old-view CommitQC remains decisive after a view change. All received
Commit votes require an exact same-round durable Prepare lock and subject. A
premature current-view Commit stutters recoverably; matching `LockAndCommit`
acknowledgement advances the adapter's consumer epoch so that exact vote may
enter once.

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

Local admission alternates producer-completion and causal-work sources. The
current proof source gives an Init/full-action induction for
`asyncCausalAdmissionOwed[node] => CausalQueueNonempty(node)` on `AsyncSpecAt`
traces; `AsyncStrongTypeInvariant` alone does not imply this fact. Reachable
causal-debt replenishment therefore reduces to the two local metadata setters,
and owed admissible causal work receives deterministic preference under fair
runner service. This does not bound how often distinct causal heads can
replenish the debt, so it is not yet a temporal convergence proof.

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
coalesces it into one exact destination. Coalescence preserves the sole
persistent producer: a persistent source replaces and retags across an
ordinary destination, an already-persistent destination is validated and
retained before an ordinary volatile source is removed, and two independent
persistent roots fail closed before either side changes. An uninstalled destination is a recoverable caller-contract
rejection with no mutation. Conflicting or duplicate ownership fails closed,
and every pipeline or Decision retirement invariant is checked transactionally
before mutation. Conflicting certificate evidence received through
body-recovery request/response transport is rejected nonfatally and leaves
retry ownership available.

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

### Theorem scope and FLP boundary

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
  one-height Core wrapper. The separate exact per-slot receipt-agreement
  script is only SANY-clean: without a fresh strict TLAPS receipt,
  `IndexedChainSpecEstablishesExactPerSlotReceiptAgreement` remains
  `specified_unproved`.

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
context, crash/recovery state, and local runner-service contract deadline
according to the variables each action may change. `LocalRunnerServiceOwners`
is exactly the union of active responsive voters and exact historical-recovery
targets. Its deadline vector is per-validator ghost bookkeeping for the
ledgered `runtime-after-gst` trusted contract; it is neither wire state nor a
production all-voter scheduler. The source-fidelity guard separately seals the
serialized height loop, bounded runtime/ingress/completion/sidecar turns,
watchdog poll, and four finite `IDLE_POLL` edges. Those structural checks do not
prove OS scheduling or admitted-work latency, which remain explicitly trusted.

`ApplyDecision` and `ApplyDecisionReady` now require
`DecisionCertifiedBodyRecoveryAuthority`: the durable Decision selected for
application must be the exact current-context Commit certificate before
`ExecuteApply` retires that node's historical-recovery target. The serialized
command's `evidence` remains causal provenance and is not overloaded as the
application certificate. The non-regular readiness proof states this authority
on both the executable and readiness sides while preserving their existing
enabledness equivalence. `SumeragiV2ApplyAuthorityMutation.tla` is the paired
bounded regression: its missing-guard branch applies a same-view/same-subject
old-context Decision and loses the non-voter's timed-service owner, while the
repaired branch transfers that ownership to the current application. This is
an action-boundary counterexample, not a claim that the adversarial initial
state is reachable from the full async `Init`; a fresh pinned SANY/TLAPS/TLC
run remains required before citing the new artifacts as machine-checked
evidence.

`AsyncFairActionAt` inventories the same
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
`tlaps_proved`. Progress ownership consumes the proved async type closure;
Stage 4 and Stage 5 consume
progress ownership, and the three rank leaves use their exact fair-action
prerequisites. The
aggregate `protected-service-rank` obligation depends on every one of these
leaves, without conflating the abstract model with production admission,
runtime, ingress, or actor-to-flush ownership. The top-level source ledger now
contains exactly 54 obligations: 35 `tlaps_proved`, 12 `specified_unproved`, 6
`trusted_contract`, and 1 `out_of_scope`. Sixteen proof/evidence decomposition
leaves remain source-bound and transitively checked through reviewed top-level
consumers rather than being promoted into extra ledger rows. Machine-checked
completion is evaluated over the same 54 obligations and must contain 44
`tlaps_proved`, 3 `cross_tool_proved`, 6 `trusted_contract`, and 1
`out_of_scope`; the current ledger keeps `machine_checked_completion: false`.
`AdequateLeaderExactClosureResidualObligation` and
`ExactDecisionOffSchedulerResidualConvergenceObligation` now both have pinned
source proof bodies and are deductively classified support leaves in the
aggregate temporal closure. They are not independent ledger rows; fresh strict
evidence and every remaining consumer dependency are required before
rotating-leader or application liveness can be promoted.

The target-local closure boundary is split across
`SumeragiV2AdequateLeaderServiceClosureProofs.tla`,
`SumeragiV2ExactDecisionStageServiceClosureProofs.tla`, and
`SumeragiV2AsyncTemporalClosureProofs.tla`. The adequate-leader residual does
not treat another validator's Decision as terminal for the indexed target. Its
occurrence rank counts every distinct target/leader owner at the frozen
semantic rank, so servicing one owner cannot hide another. Equal-count
replacement and count-increasing replenishment remain explicit non-progress
cases and require a prior finite or coalesced producer argument.

The exact-Decision producer audit narrows causal replenishment to reachable
local debt setters; Serve-capacity growth to ordinary or historical request
drain, fresh causal Completion admission, or local Control enqueue; and
priority growth to exact network-claim admission or the same archive's normal,
recovery, or historical runner. Each classification is action-local. The five
exact off-scheduler convergence leaves now have source proof bodies that close
their immutable-owner, finite-prefix, admission/coalescing, and response-gate
cases without treating replenishment itself as progress. Fresh strict TLAPS is
still required before their application-liveness consumer is promoted.

The target statement is exactly: after GST, with a representative roster of at
least four voting peers, a responsive dual quorum, and deterministic
terminating local work, every height eventually decides and every responsive
validator eventually applies it. It makes no termination claim during an
unbounded partition, without a responsive dual quorum, in an undersized
production fixture, or while admitted disk, signing, reconstruction,
validation, or application work does not terminate.

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
the fresh strict slices summarized below; the timeout-view, locked-body
reproposal, rotating-leader, and application liveness declarations remain
`specified_unproved` as well. The rotating-leader declaration is a two-stage
claim: reach a view where the responsive honest scheduled leader itself is
active (or decide first), then decide from that leader state. The application
ledger entry now names the per-validator
`AsyncTemporalClosureApplicationCompletionProgressObligation`: after GST, each
responsive validator's own durable decision must lead to its own durable
application. Its source proof composes exact Decision-stage service from the
five off-scheduler leaves and protected finite-runner closure; the entry remains
unpromoted until that dependency cone passes strict TLAPS.
The legacy-named `LockedBodyReproposalProgressObligation` rejects vacuous view
movement. Its first-release statement requires every stable available retained
lock eventually to commit in its old round, be re-proposed unchanged under a
later new same-round origin, or be legitimately decided or superseded by a
higher certified Prepare lock. That still-unproved TLA+ symbol must pass the
fresh strict proof before promotion.
The executable Core action also retains the narrower safe-value guard:
`LocalProposalReproposesJustifiedHigh` requires every nonempty
`LocalProposalJustification` high certificate to name the proposed subject.
The focused inductive theorem proves that projection from
`BeginLocalProposal`. A source-fidelity contract binds it to TC-high promotion
in durable WAL replay, the adapter's exact lock projection, the runner's exact
body load and subject check, executor admission, and the fresh-candidate
rejection in `v2_candidate`; vacuous/disconnected formal guards and weakened
WAL, runner, or candidate seams fail the mutation test. This structural result
does not prove locked-origin temporal progress, so the locked-body ledger entry
remains `specified_unproved`.
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
`SuccessorActivationStarvationFreedomObligation` are proved. The separate
`SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation`
is a required production safety/refinement seam, not temporal exact-recovery
progress and therefore not a genesis-handoff prerequisite. At the terminal finite
horizon the handoff is explicitly vacuous rather than manufacturing a
successor instance. The activation obligation uses the lifecycle-aware scalar
carrier `0..21`: Published predecessor ownership adds an `11`-point tier to
the `0..10` startup distance, while recovered/absent ownership uses the
distance directly. A clean exact-complete-tip restart therefore descends from
the Published tier into absent/Queued rank `10`; snapshot-bootstrap authority
cannot satisfy that recovery credential. Failure may reset an attempt
repeatedly, so history is not a ranking counter; an explicit eventual
failure-free suffix premise lets the terminating local worker traverse the
finite pipeline. Its rank, enabledness, fairness, and starvation clauses range
over responsive validators only. An honest validator
outside `Responsive` may retain work queued before GST without violating the
conditional production target; the model does not manufacture local-worker
fairness for that validator. Failure history is no longer a one-shot ranking
counter: Applied failure preserves Running until restart, Recovered attempts
may fail repeatedly, and `IndexedChainSpec` states an explicit eventual
failure-free suffix for terminating local work. The release-facing activation
theorem now carries an explicit candidate proof: suffix-local weak fairness
exits every rank, well-founded composition reaches publication/supersession,
and a temporal persistence lemma lifts that result across the eventual suffix.
Its progress-preservation leaf explicitly requires the chain-epoch invariant
and proves the canonical next context admissible from exact durable parent
application, node-context typing, certified-prefix validity, and the
no-outrun bound. This prevents a failure latch from retaining an exact recovery
owner outside the admissible indexed product. The repaired caller chain and
fail-closed mutations are SANY-clean, but not yet a strict deductive
discharge.
It remains ledgered `specified_unproved` until strict TLAPS verifies the full
proof; source composition and SANY parsing are not a machine-checked discharge.
The separate production-refinement seam is also intentionally stronger than
the already proved abstract successor invariant. Its release theorem conjuncts
`ProductionSuccessorAndExactRecoveryTraceRefinement`, an exact six-boolean
inventory for Applied and Recovered publication, split startup failure/restart,
authenticated historical-certificate import, the ordinary historical body
pipeline, and the authenticated terminal Apply boundary before successor
construction. The sixth kernel proves exact context, receipt, artifact, block,
and durable-predecessor identity with no pending successor activation. It has
no production `MaxHeight` input; `MaxHeight` remains only a finite-horizon
projection. The startup-failure production seam distinguishes a valid checked
Running failure, which mutates the local snapshot, from an invalid projection,
which preserves local state while the independently prelatched process output
guard remains authoritative in public status. Historical body serving is
authorized by frozen-roster membership rather than historical QC
participation; the QC authenticates the subject and the archive signs the
response. Those booleans are
unassigned: source-order checks, adversarial Rust tests, stale-token mutations,
and source-manifest binding constrain the corresponding traces but do not
prove refinement. The theorem therefore has no proof body and remains
`specified_unproved` until machine-checked cross-tool trace evidence establishes
all six claims; the abstract model theorem cannot be reused as its discharge.
The chain refinement now models an indexed family of authoritative
`AsyncSpecAt` instances. The exact
`SumeragiV2ChainLivenessProofs!HeightLivenessObligation` lives in a child of
the successor-starvation proof module, eliminating the former parent-to-child
dependency. Its conditional kernel derives exact recovery from the already
fair product-level open action plus an explicit source-eligibility leadsto and
two named Async temporal properties: historical target-to-Decision and
responsive Decision-to-application. Eligibility prevents a joined node with
no certified source from being treated as an enabled historical action. They
are predicates, not assumptions or trace booleans. The Async historical child
states their exact post-GST, all-`Responsive` forms. It gives scheduled work an
explicit `HistoricalRecoveryTarget` owner, proves that the shared service rank
is well founded for that owner, and proves the weak-fair historical discovery
rule conditional on exact next-step preservation. Clock/readiness, discovery
preservation, authenticated request-to-Decision delivery, and the historical
Decision/body/application corridor remain named operator premises; the child
does not promote either endpoint from those premises. The checker exposes the
remaining historical corridor as exactly three temporal support theorems with
source proof bodies:
`IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation`,
`IndexedHistoricalCertificateRankProgressResidualObligation`,
and `IndexedHistoricalDecisionRankProgressResidualObligation`. The separate
`IndexedHistoricalDecisionStageOwnershipResidualObligation` is proved as a
safety theorem from `IndexedChainSpec`; downstream exact-recovery composition
derives ownership from it and assumes only the three remaining temporal
residual properties. Those three theorems are source-bound transitively through
the top-level `height-liveness` row rather than promoted into extra claims.
The release-facing height theorem also has a source proof body but remains
explicit `specified_unproved` debt until its prerequisites are discharged and
a fresh pinned strict proof succeeds
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
unproved production refinement. At the production effect boundary, repeated
exact `Apply` rediscovery coalesces with one in-flight work owner. Once the
typed Kura receipt and exact finality artifact complete, their original
reducer tag remains as a terminal tombstone: exact post-drain retries are
absorbed and identity drift fails closed. This prevents old-stage recreation
but is source-level refinement evidence, not a replacement for the outstanding
strict application theorem or cross-tool trace proof. Dormant
non-genesis instances
retain their exact `InitAt` parent receipt internally, but only current-context
receipts enter the global ChainEpoch projection, so the indexed genesis is
non-vacuous. The first-release model does not restore a
favourable-network relation, global asynchronous shadow state, or a second
consensus transition relation to stand in for that proof. The separate
terminal-ingress absorbency script is SANY-clean process-lifetime model safety,
not the missing Rust trace refinement. No fresh strict TLAPS receipt exists for
that abstract obligation, and neither it nor source-fidelity checks promote the
still-unproved terminal Rust refinement.

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
concrete deadline-debt decrease, or protected-rank decrease/exit. The local
runner debt in that decomposition is scoped by `LocalRunnerServiceOwners` and
owned by the trusted runtime contract rather than inferred from a fictional
shared production deadline. Its remaining
stage, packet, and zero-deadline cases are still `specified_unproved`. Durable
progress-witness preservation, strict protected-service-rank decrease, and
starvation freedom intentionally remain `specified_unproved`; adding the
vocabulary is not machine proof. The executable async scheduler now records an
exact deferred handoff on a Busy retry while permitting Completion work to
terminate that Busy owner and rejecting foreign non-Completion re-Busy. The
Stage-2 scratch leaves the concrete rearm/cursor/service temporal induction
explicit; the Stage-3 scratch leaves only the same-node run and all-other-step
action inductions above its finite FIFO kernel. These source changes have no
fresh strict proof evidence and do not promote the ledger. The release-facing witness now uses
`AsyncDurableCommitProgressWitness`: when a responsive crash clears
`signVotes`, only the exact recovery phase, node, and generation may carry the
durable Commit until WAL replay reconstructs an ordinary carrier. The source
decomposition covers that Commit carrier and protected deferred ownership;
historical TC-lock reconstruction and exact decision-pipeline preservation
remain explicit proof gaps inside the same proofless obligation. A bounded
mutation shows that the volatile-only witness fails on the crash transition
while the exact authority-aware witness survives repaired replay; it is a
counterexample regression, not deductive discharge.

### Evidence and release gate

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
The former 57,058-line async proof root is a mechanically sealed chain of 21
physical modules. Twenty proof-bearing shards run separately; the one debt
shard contains exactly `TimeoutViewProgressObligation`,
`LockedBodyReproposalProgressObligation`, and
`RotatingLeaderProgressObligation`. The façade preserves existing qualified
references without declaring anything. The checker rejects missing, reordered,
duplicate, oversized, forward-referencing, or unexpectedly proofless shards;
each shard is capped at 256 KiB, 5,500 lines, and 150 local theorems, and each
theorem at 600 lines and 256 structured steps. Evidence resolves every
façade-facing ledger symbol to its unique provider shard and provider log.

SANY-clean parsing of `SumeragiV2ChainReceiptAgreementProofs` and
`SumeragiV2TerminalIngressLifecycleProofs` is recorded only as source
well-formedness. Neither module currently has fresh strict, source-bound TLAPS
evidence, so both exact targets stay `specified_unproved`. In particular, the
terminal model proof cannot promote the still-unproved Rust trace refinement.

The checked-in ledger cannot contain stale tool-run counts. Before each backend
run, TLAPM performs a strict no-backend summary preflight. Each proof-bearing
shard then runs in a fresh process with one worker, disabled fingerprints, and
its own disposable cache. The complete wave executes inside an owned process
group under a 2 GiB bounded polling ceiling with a target 250 ms cadence. Each `ps` or macOS
`footprint` probe is limited to 2 seconds; an inspection timeout fails closed and
terminates the exact owned process group. The next sample is scheduled from the
completion of the previous probe so scheduler latency cannot create a catch-up
storm. This portable userspace guard is not an operating-system hard allocation
limit. A separate lifeline session cleans
the body after supervisor death, while inherited lock descriptors keep the
per-user heavy-job lock shared with Kagemusha V4 candidate generation until
cleanup finishes. Release receipts bind the resulting JSONL samples and
canonical resource summary. The inherited one-shot launch capability prevents
stale environment markers from skipping the wrapper; it is not a security
boundary against a malicious same-UID process.

The structural checker rejects top-level TLA+ assumptions/axioms, unledgered
omitted proofs, Verus assume/admit/trusted-body escapes, non-theorem ledger
targets, retired Sumeragi paths, and the former favourable-network liveness
corridor. A proofless release theorem is accepted only at its exact pinned
module and symbol while the ledger records it as `specified_unproved`.
Every validation mode rejects `machine_checked_completion=true` while any such
entry remains. The only theorem-bearing non-release modules are
`SumeragiV2AsyncOutstandingLivenessDebt`,
`SumeragiV2HistoricalLockedBodyRecoveryBridgeScratch`, and
`SumeragiV2ProgressWitnessCrossToolScratch`; their declarations or bridge
rechecks are never proof evidence. `SumeragiV2LockedBodyProposalActionProofs`
is a release helper module but has no ledger target of its own, so its action
lemmas cannot promote locked-body liveness. Promotion order is also explicit:
async type closure depends on
proved runner scheduler preservation; Decision restart-recovery depends on
type closure; the pure model progress witness depends on type closure,
generation-scoped delivery, the independently proved post-Decision frontier,
Decision restart-recovery, and the effective-lock model theorem; the production
progress-witness refinement follows that model theorem. Productive deadlock
freedom additionally waits for model-witness preservation and the aggregate
protected-service-rank theorem, and timeout/view liveness waits for productive
deadlock freedom. Starvation freedom depends on the proved service-rank theorem;
rotating-leader progress depends on the locked-body three-arm progress
obligation; and
genesis handoff and indexed height liveness each depend on proved
rotating-leader, application liveness, and successor-activation starvation.
The successor/exact-recovery production bridge remains independently required
for completion, but its safety/refinement invariant is not substituted for the
unnamed temporal exact-recovery-progress theorem needed by indexed height
liveness. Stage-2, Stage-3, and Stage-6 remain scratch-only and have no canonical
ledger IDs, so the checker does not encode fictitious aggregate-rank edges.
Release mode additionally requires fresh source-bound evidence.

Before network startup, the executable wrapper inventories 826 named tests
across 38 Rust modules. The preceding 298-name inventory was produced from the
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
298-name inventory, the bounded per-source closure added 110 exact tests and
removed two superseded names, for a net increase of 108 and an exact total of
406. The preceding current-source geometry closure added 14 exact tests without
removing a name, for an exact total of 420. The route-lifecycle closure added
three exact P2P tests without removing a name, for the historical 423-test
checkpoint. Mechanical source-to-inventory reconciliation then added 16 tests
and three owning modules, producing 439 tests across 29 modules; the renamed
lane-relay saturation regression replaced its obsolete name without changing
cardinality. The in-flight sidecar redelivery regression raised the inventory to
440, and three terminal-flush/reconnect worker regressions produced the
historical 443-test checkpoint. The source-authority, immutable-sidecar,
runner-race, daemon-corridor, shared-byte-budget, cached-Arc-admission, and
executable-refinement closure adds 22 exact regressions, moves two peer tests to their actual owning
module, and yields the 465-test, 30-module, 53-leg checkpoint. The authenticated
non-validator source-cap regression and the alternate-route-before-lane-cap
regression add two more exact names, yielding the 467-test checkpoint
without adding a module or corridor leg. Three daemon Hold/Release controller
regressions, one layered daemon ownership regression, and two root
configuration geometry regressions add six exact names, yielding the historical
473-test checkpoint without adding a module or corridor leg. One configuration
fingerprint, two historical-recovery kernel, and one shared authenticated
source-credit regression add four more exact names, yielding the historical
477-test checkpoint without adding a module or corridor leg.
They bind each delivery capability to its original minting tenure even after
bounded retired-source tombstone churn, reject a second route at rehydration
instead of silently overwriting the first capability, and prove actor
normal-exit/`Drop` teardown retires every actor-owned route while canceling only
that actor's waiters. The geometry additions bind semantic request identity, per-source
route/cursor ownership,
writer-flush identity, sidecar source limits, runner/worker route preservation,
daemon Hold/Release failure handling, actor-global deferred capabilities,
scheduler ownership handoff, and fail-closed ordinal/debt boundaries.
They also bind exact-envelope deferred service, semantic-origin separation,
source-swapped response/chunk rejection, alternate-source runtime and orphan-
chunk ownership, actor-global ordinals across tenures, and checked
`iroha_config` source/capacity geometry at its arithmetic and root-parse
boundaries.
The proposal-origin, multi-carrier, and persistence-failure closure then adds
41 exact regressions and retires nine superseded selectors, yielding the
509-test, 38-module, 61-leg checkpoint. It binds reducer and deferred
identities, equivocation evidence, aggregate signatures, finality/header
geometry, compact offline QCs, and parent height-context identity to the
authenticated origin.

The final successor/recovery closure adds six exact regressions without adding
a module. Crash-safe response handoff and same-delivery retry after transient
capacity pressure add two more sidecar regressions. The lifecycle snapshot now
binds its complete canonical Norito payload with a typed hash, and a focused
regression rejects canonical-but-digest-stale corruption before recovery. The per-source
route-attempt, exact PrepareQC recovery, locked-body reproposal, runner/worker,
sidecar, and daemon closure, plus the certified sidecar control-bucket
regression, produced the 585-test checkpoint. The unsent-request restoration
and fairness-cursor retry regressions produced the 588-test checkpoint. The
durable semantic-peer-history regression produced the 589-test checkpoint.
Mechanical source-to-inventory reconciliation then adds 115 net regressions:
3 authoritative-ingress, 57 merge-sidecar, 17 lane-work, 3 runner, 17 worker,
and 18 P2P network tests; the daemon network-relay rename is cardinality
neutral. The runner close-prefix failed-suffix handoff regression adds one
exact name. The routed-Hint and crash-safe V3 lifecycle closure then adds 26
exact regressions and retires eight obsolete route-free/V2 selectors, for a net
increase of 18. Rejecting replay of a proposal superseded by a same-round lock
adds one exact reducer regression. Preserving that proposal's exact tag, round,
and subject through runner startup adds one exact runner regression. Two
cross-platform lifecycle V3 crash regressions cover state replacement before
directory sync and root replacement before predecessor cleanup. Those changes
produced the 732-test checkpoint. Rejecting a matching CommitQC from a foreign
height context before Apply schedules any work adds one exact `v2_effects`
regression, yielding the 733-test checkpoint.
Five exact-Serve lifecycle regressions pin Pending/Reserved rollback, shutdown
rollback, route-neutral tombstone replay, and cached replay after the singular
future-slot barrier, producing the 738-test checkpoint. Another 35 exact
CertifiedServe ingress and worker regressions bind gate ordering, immutable
admission ordinals, frozen predecessors, coalesced retries, durable restart,
terminal replay, owner replacement, and anti-resurrection behavior. One
four-peer leader-wire lifecycle-store regression binds the full
origin/phase/chunk slot product and restart-stable terminal coalescing. The
resulting checkpoint contained 774 tests. Eight runtime/effect/runner
regressions now bind Decision/lock retirement of orphaned leader-wire owners,
same-turn terminal consumption across live and recovery capacity retries, and
fail-closed rejection of the unreachable semantic-only authenticated
Coalesce branch. Subsequent source reconciliation and exact-ingress lifecycle,
restart, provenance, and quarantine regressions bind the actor-global logical
owner separately from every physical queue occurrence. Seven physical-cut,
adapter-capability, aggregate-rebase, and ineligible-driver regressions produce
the 813-test checkpoint. Five admission/coalescing, Busy pre-runtime ownership,
and reconstructed-chunk terminality regressions bring the 818-test checkpoint.
Thirteen exact admission, retry, tombstone, and high-water regressions bring the
inventory to the 831-test checkpoint. Retiring five obsolete peer-genesis
protocol regressions brings the
current inventory to 826 tests across 38 modules.
Together with the source-sealed command and tooling legs, the pre-network
corridor contains 81 legs. The
G-SCALE runner/validator preflight remains part of that sealed corridor.
The first-release sidecar keeps wire protocol version 1 and uses positive
`NonZeroU64` responder generation, requester epoch, and per-stream semantic
sequence coordinates. Canonical request identity binds the version, all three
coordinates, payload or reference, and requester/responder peers. It excludes
only cumulative `closed_through`, which may advance monotonically on the same
occurrence without rematerializing output. `GenerationHint` carries the
observed/current generations and exact triggering Request or Close hash on the
triggering authenticated reply route. Alternate sources retain independent
attempts, and a later delivery replaces only its own source attempt. Every
frozen target has a dedicated `SidecarTopologyProgress` Lane reservation for
topology-routed Request/Close and an independent `SidecarReplyControl` Lane
reservation for exact-reply CloseAck/GenerationHint. Parked ordinary output and
a saturated shared pool cannot consume either reservation; alternate Hint
routes coalesce without multiplying reservations.
The canonical progress-mutation runner also executes the source-bound
`GenerationEpochFixed` trace. The repaired path persists and installs the next
responder generation, persists a fresh requester epoch before retiring the old
partial occurrence, and rejects future-generation input atomically. Its
complete TLC graph contains exactly 7 generated and 7 distinct states at depth
7. The paired pipeline trace additionally enqueues the old occurrence, discards
it only after the fresh coordinates persist, enqueues its successor, and
rejects the stale flush without mutation; TLC exhausts 11 generated and 10
distinct states at depth 10. Both remain bounded regression evidence rather
than deductive proof. A third fixed trace checks atomic rejection of
nonterminal compaction plus requester-epoch and responder-generation
exhaustion, exhausting 5 generated and 5 distinct states at depth 5. Its
queued-output pipeline companion exhausts 8 generated and 7 distinct states at
depth 7 without losing the pending attachment, item, or occurrence.

The deductive boundary is deliberately narrower than those traces: 13
ownership, 9 pipeline, and 11 asynchronous structural theorems are strict-green
at 15/15, 11/11, and 54/54 backend obligations. They cover canonical
coordinates, fail-atomic local actions, exact V2 action projection, both
product brackets, and both composed-spec projections. Whole-spec induction,
successor isolation, local progress, and the asynchronous temporal product
remain plain operators without proof evidence. They stay `specified_unproved`;
no bounded trace or structural theorem promotes network delivery,
rotating-leader progress, or another liveness claim.

`MergeSidecarLifecycleSnapshotV3` is the sole durable lifecycle schema. It
contains geometry, `next_stream_epoch`, responder generation, requester
streams, the unified bounded server-stream table, request gates, and the root
generation; V1/V2 are unsupported rather than decoded or migrated. Before the
state directory exists, the root is atomically published and fsynced as a
generation-zero bootstrap sentinel with no snapshot hash. A surviving
generation-one candidate beside that sentinel is semantically validated and
rechecked before the root adopts it. Later snapshots alternate between two
immutable state slots. The inactive slot is fsynced first and an independent
root high-water marker is the sole commit point, so restart selects the exact
predecessor before marker publication and the exact successor after it.
Startup validates the selected state, rechecks the live pair, and validates
known temp artifact types before deleting temps or the unselected slot;
unknown or non-regular artifacts fail closed. Committed recovery re-syncs the
selected state and root-marker directories before cleanup, and filesystem
aliases, including Windows reparse-point files or directories, fail closed.
Unix and Windows use native atomic replacement, and unsupported directory-sync
platforms fail closed.
The marker is a local trust anchor rather than an external monotonic counter:
marker replacement or rollback, including restoration of the bootstrap
sentinel, and whole-store rollback are outside the guarantee.

The server-stream and gate tables are the two bounded responder tables, with
attempts bounded inside gates. Ordinary changed-roster rollover requires
authenticated terminality. A sealed durable handoff or validated restart may
force-fence active predecessor responder state after durably committing the
empty successor projection; it clears predecessor responder/output debt
without manufacturing close prefixes. Equal-roster rehydration never advances
generation and preserves retained responder state. A new same-roster requester
against a full table, an unauthorized active-state replacement, or overflow
returns `Capacity` atomically.
The canonical module/test TSV inventory SHA-256 is
`07b5827f8a9203fcf7a8bd7ffe5aafe03da92436c2b7a0c577b364a78a802165`.
The six boundaries preserve the predecessor CommitQC through wire-to-core
conversion, block rollover until the decided lane session is durable, reopen a
globally finalized tip whose lane evidence is incomplete, filter terminal
CommitQC discovery and losing current-body requests, accept canonical view-zero
bytes whose first proposal origin is later, and pin a contention-tolerant
restart view-zero deadline. The genesis finality regression's whole-item token
SHA-256 is
`bfbd01d093f38fa8c96fb17fe38b6ec1132e6ffbb0d09367a298299394bdce4f`,
and the restart-deadline regression's is
`13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f`.
That four-validator scenario also owns sequential missing-height discovery and
catch-up. A diagnostic run observed the full 20-second delay at each missing
height. The fresh exact run of
`sumeragi_v2_runner::authoritative_v2_finalizes_through_validator_restart`
against recovery-scoped eager discovery passed 1/1 in 79.82 seconds; all four
peers shut down gracefully with empty stderr. This is focused regression
evidence only and does not promote the formal ledger. The successor-boundary
regression's whole-item token SHA-256 is
`ee773b00e696822c6d2ba998fb88201bb6e2a06eac749a2c700edec70dbbdf74`;
its extended authenticated-admission companion is sealed at
`1cb4736b2e4b499403c870cc3dd5ab8ccd361d51887efad4178ed7d39a9e0225`.
These tests deliberately do not claim remote application acknowledgement,
relay second-hop completion, or unbounded
broadcast admission. The 264-name baseline added 32 regressions for atomic lane
certificates, semantic-origin/authenticated-via ownership, P2P source fairness,
daemon relay quotas, and the active watchdog. The 232-name baseline already
included two exact locked-Commit
progress-witness regressions and six outer TransportCompletion-corridor
regressions. The current
geometry pins five owners per validator, three owners for each of the `H`
simultaneously materialized authenticated non-validator lanes, and two
anonymous owners (`5N+3H+2` total), including a roster-origin completion relayed
through an authenticated non-validator hop, and retains the capacity-negative
boundary. It
also retains one four-validator exact PrepareQC count-and-power quorum
regression. The five integration names execute under one module-filtered leg;
the complete pre-network corridor now spans 81 legs, including separate exact
data-model status and atomic lane-certificate decode contracts, the two
`iroha_config` geometry modules, three P2P geometry modules, and source-sealed
command-success legs. Its finality, offline compact-QC,
and height-context proposal-origin modules each use a dedicated
`iroha_data_model` leg. The inventory executes the `iroha_p2p` library with its
empty default feature set. It does not claim the feature-gated QUIC first-packet
geometry tests as part of those thirty-eight modules or eighty-one legs. The
inventory includes five native-AMX lane-work
capacity regressions, adapter/runner/watchdog successor-activation boundaries,
exact recovery-derived successor identity, authenticated exact historical
recovery, post-decision timeout/TC quiescence, and the exact
`5N+3H+2`/`2N+3` admission boundaries in addition to exact-lock,
completion-ownership, future-acquisition rejection, rebound durable retry, and
executor-batch boundaries. Those adapter boundaries pin a maximum flattened
persistence macro-step of four effects within the reducer's eight-effect bound,
service at most one Busy-deferred adapter macro-step per serialized runtime
turn, and require the Completion, Progress, and Normal deferred queues all to
be empty before terminal readiness. A production-default capacity regression
saturates the 256 certified-request owners, 639 Normal ingress slots, and the
128-slot reserved Progress increment while retaining the 256-slot Completion
reserve and one separately charged certified-fence slot; an exact authenticated
`CertifiedBodyResponse` with a still-live
matching logical request registration retires its old request owner. The
certified slot is a single retained credit, not a one-certificate limit:
additional distinct TCs, CommitQCs, or CommitQC-carrying recovery responses
consume ordinary Progress capacity while all retained certificates share that
one credit. While an authenticated
certified-body response remains retained, its escape episode is explicitly
`Fresh`, `Charged`, or `Spent`: the Fresh potential admits at most one new
direct TC, CommitQC, or `CommitCertificateResponse` carrying a CommitQC root;
Charged and Spent reject every further fresh root, and only claim retirement
resets the latch.
The claimed-response rank counts
the exact frozen direct roots plus the strictly decreasing trusted causal tail,
so pacemaker priority cannot be replenished indefinitely.
Certificate-first and certificate-last arrival orders preserve the same
ordinary reserves. The
standalone revision-4 kernel also charges an unpublished `BodyAvailable` token
as an ordinary Completion owner and replaces its conflicting proposal owner in
one atomic transition, preserving physical occupancy throughout the swap. The
production source-fidelity gate additionally requires any restart-restored
stage-7 producer to be exact-matched by lifecycle key, first-admission ordinal,
and stage, then removed from process, durable, and dormant state and persisted
before unpublished or whole-pipeline retirement can release the volatile
runtime owner. Busy-deferred producer release uses the same persist-first
batch boundary and restores the complete queue and producer-map image on
failure. A terminal restored Fetch retires that exact stage-7 parent even when
it never reserved a `BodyAvailable` token: its fresh physical Fetch owner
proves the effect binding, while the adapter resolves the old producer from
exact durable round/subject coordinates and, when present, the complete
manifest identity. A manifest-less lookup must be unique. Destination-owned rebind instead
classifies both persistent roots before mutation and retains the sole one;
two independent roots are rejected. Persistence failure rolls every affected
alias back, and the paired second-restart regression excludes resurrection
after unpublished, materialized, and pre-reservation Fetch retirement. On
startup, replayed Reserved producers below the durable view are persistently
pruned before runtime capacity is installed, except for the exact protected
lock body pipeline; the corresponding cut is deliberately not applied to a
live EnterView owner. The
executor then retries the same retained FIFO occurrence, atomically acquiring
pending-work and certified-request ownership without a partial owner. A new
Fetch removes that head; an existing ordinary Fetch retains it as the exact
completion barrier after upgrading certified authority.

The productive leader-wire source-fidelity boundary is now live as well as a
restart rule. Certified EnterView and durable Decision monotonically advance a
process-local authority. The persistent gate proves the exact obsolete
Dormant set, removes it while retaining both ordinal high-watermarks, and
publishes before the fair-ingress mirror is pruned. Failure rolls the gate back
and leaves the mirror unchanged; Ingress and Runtime carriers are outside this
cut, and below-cut admission is rejected rather than reported as capacity
exhaustion. `CertifiedResponse` is the deliberate exception to view and
Decision obsolescence: Decision can be durable before the decided body is
available locally. Its old proposal view therefore remains admissible only as
the bounded historical-completion class, after which the exact outstanding
request, QC, responder signature, subject, canonical body, and DA manifest
checks remain authoritative. Proposal chunks and every control phase stay
view-scoped.
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
binds original HEAD/tree/`Cargo.lock`, all 81 pre-network legs and the exact
826-test inventory, the pinned harness lock and resolved toolchain, the formal
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
witness; none of these outcomes changes proof status. Its liveness configuration
uses finite `1000000` timeout and view ceilings, above the configured complete
service budget and within the
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
zero, and no TLC error. Before those searches, the gate runs nine
explicit scheduler mutation/repair pairs:

1. equal-value replacement versus exact queued-envelope coalescing;
2. deferred-owner replacement versus scheduler-wide coalescing;
3. strict deferred-class priority versus the cyclic deferred-class cursor;
4. Busy Completion requeue without cursor advance versus cursor advance;
5. handoff-free equal-rank re-Busy versus the exact deferred handoff;
6. same-source head-only ingress versus oldest-admissible indexed removal;
7. aggregate-only ingress capacity versus the explicit per-lane bound;
8. conflated pending-work/completion capacity versus separate capacities; and
9. producer-first local admission versus sticky causal debt and the alternating
   local-source cursor.

The protected-rank follow-up checks five additional adversarial families:

10. four causal-capacity refill lassos versus sticky class-specific debt, plus
   the exact duplicate fast path;
11. blind causal-successor replacement versus scheduler-wide exact coalescing;
12. recurring Commit-certificate discovery inside `RunNode` versus its own
    fairly scheduled auxiliary action; and
13. indexing every physical I/O job versus indexing only Consensus owners; and
14. a multiplier-one causal FIFO rank versus the doubled FIFO position that
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
roster crossed with the Safety, Lane, and Bulk reliable classes, plus
`SidecarTopologyProgress` and `SidecarReplyControl` Lane reservations for each
frozen target. Its physical bound is that exact set plus a separate non-zero
shared fanout capacity. Deterministic maximum matching assigns at most one
distinct frozen target/class/kind reservation to each retained fanout and
recomputes assignments after every delivery attempt. Parked ordinary
same-target output and saturated shared capacity cannot consume either sidecar
progress reservation, while alternate authenticated Hint routes coalesce
without multiplying ownership. Non-roster reply targets and repeated
same-target/class/kind output can consume only shared slots. Thus identity churn
cannot consume unopened validator output reservations, and partial multi-target
progress immediately releases the completed reservation.
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
matrix: 6 compact models, 33 exact configurations, and one standalone runner.
The runner requires 10 repaired configurations to finish with TLC status 0 and
23 mutants to fail with their pinned invariant or temporal status. The 22
unchanged configurations retain exact aggregate markers for 131 generated and
130 distinct states; the revised eleven-case certified-request model pins
semantic actions and violations rather than stale state totals. The cases cover
the two-Fetch/full-capacity TimeoutVote-Sign prefix, protected and terminating
owner retirement, reconstructible full-capacity Fetch rejection, stable
`(class, work_id)` preemption with decided-owner exclusion, and retained-suffix
FIFO/bound/overtaking/Decision filtering. The fifth model separates the sole
certified-request slot from two general work slots. A `FetchBody` blocked only
by that independent request capacity is the only producer for which the result
is retryable. It retains the exact task, authority, and lifecycle occurrence as
one bounded FIFO owner; the failed attempt allocates no partial pending-work,
request, or transport ownership. Separate mutants drop, substitute, duplicate,
overtake, partially install P/Q, or drop the existing-owner completion barrier.
An authenticated exact
`CertifiedBodyResponse` with a still-live matching logical request registration
remains transport-only and may cross retained reducer-effect debt to retire the
blocking request. Once the required capacity releases, retrying the exact
retained FIFO head atomically acquires pending-work and certified-request
ownership. A new
Fetch drains that successful head; an existing ordinary Fetch retains it as
the exact retry barrier until asynchronous completion.
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

A separate source-sealed applied-phase admission runner covers one model and
six configurations. Its TLA+ configuration ranges over the evidence-bearing
`BodyStored` and `ValidationSucceeded` phases. Five mutants allocate a
post-apply ordinal, retain a physical owner after apply, coalesce conflicting
validation evidence, hide a malformed callback behind stale-tag coalescing, or
admit a well-formed stale callback as current. The source seal binds the model
to `preflight_runtime_command_admission`,
`command_admission_is_suppressed`, both serialized enqueue paths, and the exact
Busy-owner regression for those two phases. Complete callback validation
precedes stale-tag coalescing; conflicting validation evidence rejects; an
exact applied retry stutters before tagged-command construction or ordinal
allocation. The source seal separately pins the four-phase Rust exact-retry
regression for `BodyAvailable`, `BodyStored`, `ValidationSucceeded`, and
`SignatureCompleted`, without extending the TLA+ conflict/Busy claim to
`BodyAvailable` or `SignatureCompleted`. Busy, unapplied callbacks in the two
modeled phases retain one serviceable owner. This bounded matrix neither proves
crash/restart refinement nor changes a proof-ledger status.

An exhaustive one-validator ownership configuration separately checks
`AsyncTypeInvariant` and `AsyncProgressOwnershipInvariant` over 616,705
generated states (62,464 distinct, depth 37). The bounded checker retains the
independent non-timeout-progress and TimeoutVote ingress reservations, closes
the inherited acquisition state, and uses structurally equivalent finite-search
definitions for powerset-valued production carriers. The deliberately broken
configurations must produce their exact
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
preceding proposal-origin source, the isolated source-shared harness passed
118/118 reducer/WAL/refinement tests, all 8 model-trace replay tests, and all 11
named fast-network simulations. The current harness inventories 137 runnable
reducer tests and requires a fresh source-sealed run. A 2026-07-21 100,000-height chaos run completed
the permissioned and NPoS 50,000-height prefixes, 400,000 validator
finalizations, and zero failures in 91.29 seconds. These are unsealed
implementation results, not deductive evidence. The checked-in pinned Verus
receipt predates the proposal-origin changes and was not rerun for this source;
it must not be cited as discharge of the changed obligations.

### Trusted computing boundary

The proof ledger keeps the remaining premises explicit:

- signature authenticity and collision resistance;
- faithful complete-frame `fsync` acknowledgement;
- deterministic reconstruction, validation, and execution;
- a representative production roster of at least four voting peers;
- a responsive honest dual quorum after GST;
- bounded post-GST authenticated transport; and
- continuing clock/run-loop service plus termination of admitted local work.

TLAPM, its backends, Verus, vstd, the SMT solver, the Rust compiler, the
production-to-proof extraction code, the operating system, and the hardware
remain part of the implementation proof TCB. Generated evidence records the
cooperative, hash-bound claim that the configured tools accepted the exact
sources; it does not authenticate either the host or the runner that produced
that claim.
