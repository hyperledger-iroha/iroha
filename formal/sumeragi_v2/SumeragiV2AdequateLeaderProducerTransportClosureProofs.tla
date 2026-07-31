---- MODULE SumeragiV2AdequateLeaderProducerTransportClosureProofs ----
EXTENDS SumeragiV2AsyncCandidateProducerContinuationProofs,
        SumeragiV2AdequateLeaderRetainedProducerClosureProofs

(***************************************************************************
Exact adequate-leader producer/transport boundary.

The occurrence-indexed source below is intentionally identical to the source
of `AdequateLeaderTargetProducerTransportOccurrenceClosureProperty`.  Its
terminal is intentionally identical too: Decision, a strictly lower
occurrence rank, the exact named-owner corridor-exit handoff, or a genuinely
new frozen owner.  In particular, count-increasing replenishment is not a
terminal and an arbitrary occurrence frontier is not accepted as descent.

The base `AdequateLeaderTargetProducerResidual` is a negative state
classification.  It says that no semantic frontier, rebroadcast residual,
overdue blocked packet, runner residual, or response-capacity residual is
visible.  It does not say which immutable producer owns the next action.  A
ready or not-yet-due wire item can still be recovered from the concrete state;
the final arm below records the remaining case separately instead of turning
the complement itself into a fair owner.

The scheduled-producer projection exposes an existing bounded lifecycle
reservation before physical drain; after drain, the stage-exact continuation
inherits the same ordinal.  A source-qualified packet before admission, or a
retained nonterminal Serve attempt after atomic admission, is now an explicit
finite-universe producer owner too.  It does not prove that an otherwise empty
producer residual contains such an owner.  That remaining debt still needs a
real durable semantic-handoff producer reservation (or an unreachability
proof); this module keeps it explicit instead of manufacturing a producer.
***************************************************************************)

AdequateLeaderTargetProducerTransportOccurrenceSource(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, budget, owner) ==
  /\ AdequateLeaderTargetNonDescentEpisodeAtBudget(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, budget)
  /\ AdequateLeaderTargetOccurrenceOwnerCarried(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)
  /\ AdequateLeaderTargetProtocolSubjectSource(
       target, leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetProducerTransportResidualAtOccurrence(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)

AdequateLeaderTargetProducerTransportOccurrenceGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner) ==
  \/ AdequateLeaderTargetOccurrenceRankServiceExitGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)
  \/ AdequateLeaderTargetCarriedNonDescentEpisodeResidual(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)

(***************************************************************************
Concrete immutable owner exposure.

The first arm retains the exact leader PersistDecision/rebroadcast request.
The second arm retains one exact frozen wire item and an actual lifecycle
owner: retained control, packet, ingress, reducer candidate, or bounded
control-service slot.  `ItemHasPacket` includes both ready and not-yet-due
packets; requiring only the existing overdue-blocked residual would lose the
fixed packet occurrence before the clock/rank proof can select it.

Consumer milestones are terminals of the wire lifecycle, not producer
owners, and therefore do not appear in this predicate.  They must first be
projected to the occurrence goal by an exact semantic handoff theorem.
***************************************************************************)
AdequateLeaderTargetScheduledProducerOriginOwner(
    target, leaderContext, leader, leaderView, subject) ==
  \E candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
    /\ candidate.node \in {target, leader}
    /\ candidate.consumerContext = leaderContext
    /\ candidate.height = leaderContext.height
    /\ candidate.view = leaderView
    /\ candidate.subject = subject
    /\ candidate.kind \in AsyncCandidateServiceTrackedKinds
    /\ AsyncCandidateScheduledProducerOriginReservation(candidate)

AdequateLeaderTargetDurableBodyTerminalOwner(
    target, leaderContext, leader, leaderView, subject) ==
  \E owner \in
       AdequateLeaderFrozenCandidateOwnerUniverse(
         target, leaderContext, leader, leaderView, subject):
    AdequateLeaderTargetProducerOriginDurableBodyTerminal(
      target, leaderContext, leader, leaderView, subject, owner)

AdequateLeaderTargetDurableReplayOriginOwner(
    target, leaderContext, leader, leaderView, subject) ==
  \E candidate \in AsyncCandidateSet,
     rank \in AdequateLeaderTargetSemanticRankCarrier:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, rank, target, leaderContext,
         leader, leaderView, subject)
    /\ candidate.causalOrigin
         \in AsyncCandidateLifecycleDurableReplayOriginsForNode(
              candidate.node)

AdequateLeaderTargetConcreteProducerTransportOwner(
    target, leaderContext, leader, leaderView, subject) ==
  \/ AdequateLeaderTargetScheduledProducerOriginOwner(
       target, leaderContext, leader, leaderView, subject)
  \/ AsyncCandidateProducerContinuationExactOwner(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetCommitQcRebroadcastResidual(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetDurableBodyTerminalOwner(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetDurableReplayOriginOwner(
       target, leaderContext, leader, leaderView, subject)
  \/ \E item \in AsyncNetworkItems:
       /\ AdequateLeaderTargetWireIdentity(
            item, target, leaderContext, leader, leaderView, subject)
       /\ LeaderWireLogicalServiceActive(item)
       /\ \/ item \in asyncRetainedControl
          \/ ItemHasPacket(item)
          \/ LeaderWireIngressOwned(item)
          \/ LeaderWireCandidateOwned(item)
          \/ LeaderWireLiveControlServiceOwner(item)

\* Keep the retained ingress producer on a distinct boundary until the
\* source-derived journal rank is supplied.  Widening the legacy concrete
\* predicate would let the older synthetic ordinal-ceiling route consume it
\* without proving the request/source episode.
AdequateLeaderTargetConcreteRetainedProducerTransportOwner(
    target, leaderContext, leader, leaderView, subject) ==
  \/ AdequateLeaderTargetConcreteProducerTransportOwner(
       target, leaderContext, leader, leaderView, subject)
  \/ AdequateLeaderTargetRetainedProducerTransportOwner(
       target, leaderContext, leader, leaderView, subject)

THEOREM AdequateLeaderScheduledProducerOriginUsesBoundedLifecycleToken ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
    /\ AsyncCandidateLifecycleStageIdentityInvariant
    /\ AdequateLeaderTargetScheduledProducerOriginOwner(
         target, leaderContext, leader, leaderView, subject)
      => \E candidate \in
           QueuedCandidates \cup DeferredCandidates
             \cup CausalCandidates \cup TrackedWorkCandidates:
          /\ candidate.node \in {target, leader}
          /\ candidate.consumerContext = leaderContext
          /\ candidate.view = leaderView
          /\ candidate.subject = subject
          /\ AsyncCandidateScheduledProducerOriginReservation(candidate)
          /\ (AsyncCandidateScheduledProducerOriginReservationToken(candidate))
               .ordinal = AsyncCandidateLifecycleOrdinal(candidate)
BY AsyncCandidateSchedulerCoverageExposesBoundedProducerOrigin, Isa
   DEF AdequateLeaderTargetScheduledProducerOriginOwner

AdequateLeaderTargetUnownedProducerOriginDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, budget, owner) ==
  /\ AdequateLeaderTargetProducerTransportOccurrenceSource(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, budget, owner)
  /\ ~AdequateLeaderTargetConcreteProducerTransportOwner(
       target, leaderContext, leader, leaderView, subject)

THEOREM AdequateLeaderProducerTransportOccurrenceSourceIsOwnedOrExactDebt ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget, owner:
    AdequateLeaderTargetProducerTransportOccurrenceSource(
      target, leaderContext, leader, leaderView,
      subject, sourceOccurrenceRank, known, budget, owner)
      => \/ AdequateLeaderTargetConcreteProducerTransportOwner(
               target, leaderContext, leader, leaderView, subject)
         \/ AdequateLeaderTargetUnownedProducerOriginDebt(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget, owner)
BY Isa
   DEF AdequateLeaderTargetUnownedProducerOriginDebt

(***************************************************************************
Receipt-backed producer exposure.

This is a state split only.  A scheduled reservation, inherited continuation,
active exact leader wire, durable body terminal, or frozen replay origin is a
concrete owner supplied to the legacy temporal closure provider.  The
source-qualified retained ingress owner is composed only by the strengthened
boundary below.  An authority-bound corridor exit is already the exact
occurrence goal.  The service receipt itself is never a producer and
count-increasing replenishment is never called progress.
***************************************************************************)
THEOREM AdequateLeaderProducerOriginReceiptClosesExactDebt ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, budget, owner:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderTargetProducerTransportOccurrenceSource(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, budget, owner)
    => \/ AdequateLeaderTargetConcreteProducerTransportOwner(
             target, leaderContext, leader, leaderView, subject)
       \/ AdequateLeaderTargetProducerTransportOccurrenceGoal(
             target, leaderContext, leader, leaderView,
             subject, sourceOccurrenceRank, known, owner)
BY AdequateLeaderProducerOriginSourceNamesExactReceiptOrPhysicalTerminal,
   AdequateLeaderProducerOriginReceiptExposesExactReplacement,
   IsaT(900)
   DEF AdequateLeaderTargetProducerTransportOccurrenceSource,
       AdequateLeaderTargetProducerTransportOccurrenceGoal,
       AdequateLeaderTargetConcreteProducerTransportOwner,
       AdequateLeaderTargetScheduledProducerOriginOwner,
       AdequateLeaderTargetDurableBodyTerminalOwner,
       AdequateLeaderTargetDurableReplayOriginOwner,
       AdequateLeaderTargetProducerOriginReceiptSource,
       AdequateLeaderTargetProducerOriginScheduledWitness,
       AdequateLeaderTargetProducerOriginDurableBodyTerminal,
       AdequateLeaderTargetProducerOriginAuthorityExit,
       AdequateLeaderTargetProducerOriginReceiptWitness,
       AdequateLeaderTargetOccurrenceRankServiceExitGoal,
       AsyncCandidateProducerContinuationExactOwner,
       AsyncCandidateProducerSemanticHandoffReservation,
       AsyncLeaderWireLifecycleActive,
       AdequateLeaderTargetWireIdentity,
       LeaderWireLogicalServiceActive

(***************************************************************************
Temporal interfaces below the concrete-owner provider.

Origin exposure is state preservation, not a fairness assumption. The
scheduled projection strengthens the set of concrete witnesses but does not
close `AdequateLeaderTargetUnownedProducerOriginDebt`; the property below
remains an explicit required provider. Keeping it separate makes a mutation
which substitutes aggregate Decision convergence or the desired occurrence-
service theorem visible to the source-fidelity checker.
***************************************************************************)
AdequateLeaderTargetProducerOriginExposureProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             subject \in Subjects,
             sourceOccurrenceRank \in
               AdequateLeaderTargetOccurrenceRankCarrier,
             known \in
               SUBSET AdequateLeaderFrozenOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject),
             budget \in Nat,
             owner \in
               AdequateLeaderFrozenCandidateOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject):
            AdequateLeaderTargetProducerTransportOccurrenceSource(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget, owner)
              => AdequateLeaderTargetConcreteProducerTransportOwner(
                   target, leaderContext, leader, leaderView, subject))

AdequateLeaderTargetProducerOriginExposureOrGoalProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             subject \in Subjects,
             sourceOccurrenceRank \in
               AdequateLeaderTargetOccurrenceRankCarrier,
             known \in
               SUBSET AdequateLeaderFrozenOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject),
             budget \in Nat,
             owner \in
               AdequateLeaderFrozenCandidateOwnerUniverse(
                 target, leaderContext, leader, leaderView, subject):
            AdequateLeaderTargetProducerTransportOccurrenceSource(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget, owner)
              => \/ AdequateLeaderTargetConcreteProducerTransportOwner(
                       target, leaderContext, leader, leaderView, subject)
                 \/ AdequateLeaderTargetProducerTransportOccurrenceGoal(
                       target, leaderContext, leader, leaderView,
                       subject, sourceOccurrenceRank, known, owner))

THEOREM AdequateLeaderProducerOriginInvariantsProvideExposureOrGoal ==
  \A specification:
    /\ (specification => []AsyncStrongTypeInvariant)
    /\ (specification => []AsyncProgressOwnershipInvariant)
    /\ (specification
          => []AsyncCandidateServiceTombstoneLifecycleInvariant)
    => AdequateLeaderTargetProducerOriginExposureOrGoalProperty(
         specification)
BY AdequateLeaderProducerOriginReceiptClosesExactDebt, PTL
   DEF AdequateLeaderTargetProducerOriginExposureOrGoalProperty

AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat,
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject):
         /\ AdequateLeaderTargetProducerTransportOccurrenceSource(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, budget, owner)
         /\ AdequateLeaderTargetConcreteProducerTransportOwner(
              target, leaderContext, leader, leaderView, subject)
           ~> AdequateLeaderTargetProducerTransportOccurrenceGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner)

THEOREM AdequateLeaderConcreteProducerOriginAndClosureProvideOccurrenceClosure ==
  \A specification:
    /\ AdequateLeaderTargetProducerOriginExposureProperty(specification)
    /\ AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty(
         specification)
    => AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
         specification)
BY PTL
   DEF AdequateLeaderTargetProducerOriginExposureProperty,
       AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetProducerTransportOccurrenceSource,
       AdequateLeaderTargetProducerTransportOccurrenceGoal

THEOREM AdequateLeaderReceiptExposureAndConcreteClosureProvideOccurrenceClosure ==
  \A specification:
    /\ AdequateLeaderTargetProducerOriginExposureOrGoalProperty(
         specification)
    /\ AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty(
         specification)
    => AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
         specification)
BY PTL
   DEF AdequateLeaderTargetProducerOriginExposureOrGoalProperty,
       AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetProducerTransportOccurrenceSource,
       AdequateLeaderTargetProducerTransportOccurrenceGoal

(***************************************************************************
Retained-producer/occurrence composition seam.

The first conjunct closes the existing candidate/wire occurrence corridor.
The second closes only the source-qualified ingress replenishment episode.
Keeping the pair explicit prevents a downstream rotating-leader consumer from
silently using the older occurrence theorem while omitting the new monotone
producer journal.  The composition theorem is conditional on the concrete
retained-producer step provider; it introduces no additional fairness.
***************************************************************************)
AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty(
    specification) ==
  /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
       specification)
  /\ AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty(
       specification)

THEOREM AdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose ==
  \A specification:
    /\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
         specification)
    /\ AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty(
         specification)
    => AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty(
         specification)
BY AdequateLeaderFiniteRetainedProducerBudgetClosesNonDescentEpisode
   DEF AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty

(***************************************************************************
Authority-bound fixed-deadline carry interface.

The numeric fixed-corridor module records a ghost receipt at the synchronized
self leader, while this producer module reasons about any exact target in the
same frozen authority corridor.  Two obligations are therefore kept
separate:

  1. a live frozen corridor acquires the matching self-leader receipt (or the
     exact target has already decided); and
  2. while that matching receipt and frozen target corridor remain active,
     the exact target decides.

The second obligation includes target-local CommitQC dissemination after the
self leader decides.  Neither obligation may be discharged by aggregate
Decision convergence, the producer closure being proved, or the occurrence
service theorem.  In particular,
`AdequateLeaderFixedCorridorDeadlineServiceProperty` alone is not this
interface: its source is only the fresh self-leader arming instant and it has
no temporal-history carrier for an arbitrary later target corridor.

The fixed-deadline provider must prove the acquisition/carry pair below from
the immutable receipt, the configured cumulative clock budget, and exact
target dissemination.  Until then the following theorems are conditional
compositions only; they are not live-provider or ledger-promotion theorems.
***************************************************************************)

AdequateLeaderAuthorityBoundFixedDeadlineReceipt(
    target, leaderContext, leader, leaderView, receipt) ==
  LET authority ==
        AdequateLeaderCorridorAuthorityReceipt(
          target, leaderContext, leader, leaderView)
  IN /\ receipt \in AsyncFixedCorridorDeadlineReceipts
     /\ authority.target = target
     /\ authority.context = leaderContext
     /\ authority.leader = leader
     /\ authority.view = leaderView
     /\ receipt.target = authority.leader
     /\ receipt.context = authority.context
     /\ receipt.view = authority.view
     /\ receipt.deadline =
          receipt.armedAt + AsyncFixedCorridorServiceBudget + 1
     /\ receipt.armedAt <= asyncNow
     /\ asyncNow < receipt.deadline

AdequateLeaderTargetAuthorityBoundActiveReceiptSource(
    target, leaderContext, leader, leaderView) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ ~NodeHasDecision(target)
  /\ \E receipt \in AsyncActiveFixedCorridorDeadlineReceipts:
       AdequateLeaderAuthorityBoundFixedDeadlineReceipt(
         target, leaderContext, leader, leaderView, receipt)

AdequateLeaderAuthorityBoundReceiptAcquisitionProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views:
         AdequateLeaderFrozenTargetCorridor(
           target, leaderContext, leader, leaderView)
           ~> (NodeHasDecision(target)
                \/ AdequateLeaderTargetAuthorityBoundActiveReceiptSource(
                     target, leaderContext, leader, leaderView))

AdequateLeaderAuthorityBoundActiveReceiptServiceProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views:
         AdequateLeaderTargetAuthorityBoundActiveReceiptSource(
           target, leaderContext, leader, leaderView)
           ~> NodeHasDecision(target)

AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty(
    specification) ==
  /\ AdequateLeaderAuthorityBoundReceiptAcquisitionProperty(specification)
  /\ AdequateLeaderAuthorityBoundActiveReceiptServiceProperty(specification)

THEOREM AdequateLeaderAuthorityBoundReceiptCarryClosesBaseProducerTransport ==
  \A specification:
    AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty(
      specification)
      => AdequateLeaderTargetProducerTransportClosureProperty(specification)
BY PTL
   DEF AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty,
       AdequateLeaderAuthorityBoundReceiptAcquisitionProperty,
       AdequateLeaderAuthorityBoundActiveReceiptServiceProperty,
       AdequateLeaderTargetAuthorityBoundActiveReceiptSource,
       AdequateLeaderTargetProducerTransportClosureProperty,
       AdequateLeaderTargetProtocolSubjectSource

THEOREM AdequateLeaderAuthorityBoundReceiptCarryClosesConcreteOccurrenceTransport ==
  \A specification:
    AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty(
      specification)
      =>
        AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty(
          specification)
BY PTL
   DEF AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty,
       AdequateLeaderAuthorityBoundReceiptAcquisitionProperty,
       AdequateLeaderAuthorityBoundActiveReceiptServiceProperty,
       AdequateLeaderTargetAuthorityBoundActiveReceiptSource,
       AdequateLeaderTargetConcreteProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetProducerTransportOccurrenceSource,
       AdequateLeaderTargetProducerTransportOccurrenceGoal,
       AdequateLeaderTargetOccurrenceRankServiceExitGoal,
       AdequateLeaderTargetOccurrenceRankOwnerServiceExitGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal,
       AdequateLeaderTargetProtocolSubjectSource

THEOREM AdequateLeaderAuthorityBoundReceiptCarryClosesOccurrenceTransport ==
  \A specification:
    AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty(
      specification)
      =>
        AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
          specification)
BY PTL
   DEF AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty,
       AdequateLeaderAuthorityBoundReceiptAcquisitionProperty,
       AdequateLeaderAuthorityBoundActiveReceiptServiceProperty,
       AdequateLeaderTargetAuthorityBoundActiveReceiptSource,
       AdequateLeaderTargetProducerTransportOccurrenceClosureProperty,
       AdequateLeaderTargetProducerTransportOccurrenceSource,
       AdequateLeaderTargetProducerTransportOccurrenceGoal,
       AdequateLeaderTargetOccurrenceRankServiceExitGoal,
       AdequateLeaderTargetOccurrenceRankOwnerServiceExitGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal,
       AdequateLeaderTargetProtocolSubjectSource

\* An occurrence rank is not itself a progress claim.  It does, however,
\* retain the exact frozen target/authority corridor from the immutable
\* candidate identity.  This projection is the only rank fact needed by the
\* authority-bound receipt continuation below.
THEOREM AdequateLeaderTargetOccurrenceRankFrontierRetainsFrozenCorridor ==
  \A target, leaderContext, leader, leaderView, subject, occurrenceRank:
    AdequateLeaderTargetOccurrenceRankFrontier(
      target, leaderContext, leader, leaderView, subject, occurrenceRank)
      => AdequateLeaderFrozenTargetCorridor(
           target, leaderContext, leader, leaderView)
BY DEF AdequateLeaderTargetOccurrenceRankFrontier,
       AdequateLeaderTargetRankFrontier,
       AdequateLeaderTargetCandidateIdentity

\* Corridor-exit lineage is not Decision and is not rank descent.  Its
\* still-owned, post-milestone, evidence, discard, tombstone, lifecycle, and
\* stale-consumer arms all retain this exact target/authority identity; a
\* leader-local Decision/Application arm still needs exact-target CommitQC
\* dissemination.  Consequently every arm is closed only by the two explicit
\* authority-receipt obligations below.  The fixed-corridor clock layer must
\* still supply their cross-child receipt acquisition and cumulative service
\* carry; this theorem deliberately does not claim an AsyncLive provider.
THEOREM AdequateLeaderTargetRanksReachIndexedDecision ==
  \A specification:
    /\ AdequateLeaderAuthorityBoundReceiptAcquisitionProperty(specification)
    /\ AdequateLeaderAuthorityBoundActiveReceiptServiceProperty(specification)
    => (specification
          => \A target \in ValidatorIds,
                leaderContext \in ContextRecords,
                leader \in ValidatorIds,
                leaderView \in Views,
                subject \in Subjects,
                occurrenceRank \in
                  AdequateLeaderTargetOccurrenceRankCarrier:
               /\ AdequateLeaderTargetProtocolSubjectSource(
                    target, leaderContext, leader, leaderView, subject)
               /\ AdequateLeaderTargetOccurrenceRankFrontier(
                    target, leaderContext, leader, leaderView,
                    subject, occurrenceRank)
                 ~> NodeHasDecision(target))
BY AdequateLeaderTargetOccurrenceRankFrontierRetainsFrozenCorridor,
   PTL
   DEF AdequateLeaderAuthorityBoundReceiptAcquisitionProperty,
       AdequateLeaderAuthorityBoundActiveReceiptServiceProperty,
       AdequateLeaderTargetAuthorityBoundActiveReceiptSource

=============================================================================
