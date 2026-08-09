---- MODULE SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs ----
EXTENDS SumeragiV2AdequateLeaderFixedCorridorClockProofs

(***************************************************************************
Fresh-self fixed-corridor deadline boundary.

The quantitative kernel in this module is deliberately restricted to the
fresh synchronized self leader.  It freezes the pure
`AsyncFixedCorridorDeadlineReceipt` value constructed from the universally
quantified `startTime` and `deadline` of
`AdequateLeaderFixedCorridorDeadlineSource`; it does not require that value to
remain in the active ghost-receipt set.  Consequently a later occurrence of a
fresh-looking corridor cannot replenish an earlier temporal obligation, and
no past-time receipt-acquisition invariant is needed.

The charged part is only the self-leader
Proposal/Prepare/Commit/Decision pipeline.  Exact CommitQC dissemination to
other responsive targets is composed qualitatively afterward through
`AsyncLiveProvidesResponsiveDecisionDissemination`, whose dependency cone is
strictly below adequate-leader convergence but explicitly includes the
already-closed `StarvationFreedomObligation` used by the DirectTimeout
delivery residual.  The former six-stage
quantitative dissemination cone is intentionally absent: its qualitative
stage predicates did not dominate every physical prefix and therefore could
not justify a clock charge.

Operators explicitly named `...ProviderProperty` are proof boundaries, not
assumptions hidden inside a release theorem.  They isolate immutable token
ownership, coalesced producer episodes, the frozen global fixed-clock blocker
prefix, exact fair-owner selection, and per-action absolute-ceiling carry.
Weak fairness proves occurrence of the selected concrete action.  Dormant
reactivation, equal-count replacement, count-increasing replenishment, and
unrelated due work are never called pipeline progress; each is charged to a
separate finite episode before the pipeline rank may descend.
***************************************************************************)

(***************************************************************************
Exact configured accounting.
***************************************************************************)

AdequateLeaderFixedProducerActionCharge ==
  AsyncCandidateProducerActionEpisodeBudget

AdequateLeaderFixedRunnerPrefixCharge ==
  AsyncRuntimeCycleBudget

AdequateLeaderFixedDeferredCursorCharge ==
  4 * AsyncDeferredDrainBudget

\* Four copies cover the two ready sources and the physical I/O prefix.
AdequateLeaderFixedIoCarrierCharge ==
  4 * AsyncIoDrainBudget

\* The remaining two copies cover the I/O/ready selector resets.
AdequateLeaderFixedIoSelectorCharge ==
  2 * AsyncIoDrainBudget

AdequateLeaderFixedCandidateCharge ==
  AdequateLeaderFixedProducerActionCharge
    + AdequateLeaderFixedRunnerPrefixCharge
    + AdequateLeaderFixedDeferredCursorCharge
    + AdequateLeaderFixedIoCarrierCharge
    + AdequateLeaderFixedIoSelectorCharge

\* A token may visit every phase-local root slot.  Each newly admitted slot
\* may execute at another node and therefore owns one complete physical
\* episode; the extra unit charges the immutable slot handoff itself.
AdequateLeaderFixedOriginSlotCapacity ==
  AsyncChunkCount + 8

AdequateLeaderFixedPerOriginSlotEpisodeCharge ==
  AdequateLeaderFixedCandidateCharge + 1

AdequateLeaderFixedProtocolTokenCharge ==
  AdequateLeaderFixedOriginSlotCapacity
    * AdequateLeaderFixedPerOriginSlotEpisodeCharge

AdequateLeaderFixedLeaderPipelineActionBudget ==
  4 * N * AdequateLeaderFixedProtocolTokenCharge

AdequateLeaderFixedLeaderPipelineClockBudget ==
  AdequateLeaderFixedLeaderPipelineActionBudget * AsyncDeliveryBound

AdequateLeaderFixedCommitQcTransportCharge ==
  AsyncOneWayTransportBudget

AdequateLeaderFixedCommitQcIoCharge ==
  AsyncIoDrainBudget * AsyncDeliveryBound

AdequateLeaderFixedCommitQcRunnerCharge ==
  2 * AsyncRuntimeCycleBudget * AsyncDeliveryBound

AdequateLeaderFixedCommitQcRetransmitCharge ==
  3 * AsyncRetransmitPeriod

AdequateLeaderFixedCommitQcCompletionCharge ==
  AsyncCompletionReserve

AdequateLeaderFixedCommitQcDisseminationClockBudget ==
  AdequateLeaderFixedCommitQcTransportCharge
    + AdequateLeaderFixedCommitQcIoCharge
    + AdequateLeaderFixedCommitQcRunnerCharge
    + AdequateLeaderFixedCommitQcRetransmitCharge
    + AdequateLeaderFixedCommitQcCompletionCharge

THEOREM AdequateLeaderFixedCandidateChargeIsConfiguredPhysicalBudget ==
  AdequateLeaderFixedCandidateCharge
    = AsyncCandidatePhysicalServiceBudget
BY SMT
   DEF AdequateLeaderFixedCandidateCharge,
       AdequateLeaderFixedProducerActionCharge,
       AdequateLeaderFixedRunnerPrefixCharge,
       AdequateLeaderFixedDeferredCursorCharge,
       AdequateLeaderFixedIoCarrierCharge,
       AdequateLeaderFixedIoSelectorCharge,
       AsyncCandidatePhysicalServiceBudget

\* The configured model now uses the same product.  Keeping the compatibility
\* operators named makes source-fidelity checks reject a later regression to
\* additive accounting, while the equalities below discharge both seams.
AdequateLeaderFixedConfiguredPipelineBudgetCompatibility ==
  AdequateLeaderFixedLeaderPipelineActionBudget
    <= AsyncProposalPipelineBudget

AdequateLeaderFixedConfiguredDeadlineCompatibility ==
  AdequateLeaderFixedLeaderPipelineClockBudget
    + AdequateLeaderFixedCommitQcDisseminationClockBudget
    <= AsyncFixedCorridorServiceBudget

THEOREM AdequateLeaderFixedLeaderPipelineBudgetMatchesConfiguration ==
  AdequateLeaderFixedLeaderPipelineActionBudget
    = AsyncProposalPipelineBudget
BY SMT
   DEF AdequateLeaderFixedLeaderPipelineActionBudget,
       AdequateLeaderFixedProtocolTokenCharge,
       AdequateLeaderFixedOriginSlotCapacity,
       AdequateLeaderFixedPerOriginSlotEpisodeCharge,
       AdequateLeaderFixedCandidateCharge,
       AsyncCandidatePhysicalServiceBudget,
       AsyncProposalPipelineBudget

THEOREM AdequateLeaderFixedDeadlineBudgetMatchesConfiguration ==
  AdequateLeaderFixedLeaderPipelineClockBudget
    + AdequateLeaderFixedCommitQcDisseminationClockBudget
    = AsyncFixedCorridorServiceBudget
BY AdequateLeaderFixedLeaderPipelineBudgetMatchesConfiguration, SMT
   DEF AdequateLeaderFixedLeaderPipelineClockBudget,
       AdequateLeaderFixedCommitQcDisseminationClockBudget,
       AdequateLeaderFixedCommitQcTransportCharge,
       AdequateLeaderFixedCommitQcIoCharge,
       AdequateLeaderFixedCommitQcRunnerCharge,
       AdequateLeaderFixedCommitQcRetransmitCharge,
       AdequateLeaderFixedCommitQcCompletionCharge,
       AsyncFixedCorridorServiceBudget

THEOREM AdequateLeaderFixedConfiguredBudgetCompatibilityIsDischarged ==
  /\ AdequateLeaderFixedConfiguredPipelineBudgetCompatibility
  /\ AdequateLeaderFixedConfiguredDeadlineCompatibility
BY AdequateLeaderFixedLeaderPipelineBudgetMatchesConfiguration,
   AdequateLeaderFixedDeadlineBudgetMatchesConfiguration
   DEF AdequateLeaderFixedConfiguredPipelineBudgetCompatibility,
       AdequateLeaderFixedConfiguredDeadlineCompatibility

THEOREM AdequateLeaderFixedDeadlineBudgetComponentsAreNatural ==
  ModelConfiguration
    => /\ AdequateLeaderFixedProducerActionCharge \in Nat
       /\ AdequateLeaderFixedRunnerPrefixCharge \in Nat
       /\ AdequateLeaderFixedDeferredCursorCharge \in Nat
       /\ AdequateLeaderFixedIoCarrierCharge \in Nat
       /\ AdequateLeaderFixedIoSelectorCharge \in Nat
       /\ AdequateLeaderFixedCandidateCharge \in Nat
       /\ AdequateLeaderFixedOriginSlotCapacity \in Nat \ {0}
       /\ AdequateLeaderFixedPerOriginSlotEpisodeCharge \in Nat \ {0}
       /\ AdequateLeaderFixedLeaderPipelineClockBudget \in Nat
       /\ AdequateLeaderFixedCommitQcDisseminationClockBudget \in Nat
BY SMT
   DEF ModelConfiguration, AsyncConfiguration,
       AdequateLeaderFixedProducerActionCharge,
       AdequateLeaderFixedRunnerPrefixCharge,
       AdequateLeaderFixedDeferredCursorCharge,
       AdequateLeaderFixedIoCarrierCharge,
       AdequateLeaderFixedIoSelectorCharge,
       AdequateLeaderFixedCandidateCharge,
       AdequateLeaderFixedOriginSlotCapacity,
       AdequateLeaderFixedPerOriginSlotEpisodeCharge,
       AdequateLeaderFixedProtocolTokenCharge,
       AdequateLeaderFixedLeaderPipelineActionBudget,
       AdequateLeaderFixedLeaderPipelineClockBudget,
       AdequateLeaderFixedCommitQcDisseminationClockBudget,
       AdequateLeaderFixedCommitQcTransportCharge,
       AdequateLeaderFixedCommitQcIoCharge,
       AdequateLeaderFixedCommitQcRunnerCharge,
       AdequateLeaderFixedCommitQcRetransmitCharge,
       AdequateLeaderFixedCommitQcCompletionCharge,
       AsyncCandidateProducerActionEpisodeBudget,
       AsyncCandidateProducerEpisodeCapacity,
       AsyncCausalCandidateLifecycleCapacity,
       AsyncRuntimeCycleBudget, AsyncRunnerCycleBudget,
       AsyncDeferredDrainBudget, AsyncIoDrainBudget

(***************************************************************************
Fresh immutable source-window value and exact bounded terminals.

The value below is pure proof data.  Membership in either stored ghost-receipt
set is intentionally absent.  The source freezes the lower deadline
parameters and deterministic proposal subject at the state where the temporal
obligation starts, and every later rank frontier carries those same fields.
`deadlineReceipt` remains the exact lower pure receipt value; the outer
record is proof-only identity data and adds no model or wire field.
***************************************************************************)

AdequateLeaderAuthorityDeadlineReceiptSet ==
  [deadlineReceipt: AsyncFixedCorridorDeadlineReceiptSet,
   subject: Subjects]

AdequateLeaderAuthorityDeadlineReceipt(
    leader, leaderContext, leaderView, startTime, subject) ==
  [deadlineReceipt |->
     AsyncFixedCorridorDeadlineReceipt(
       leader, leaderContext, leaderView, startTime),
   subject |-> subject]

\* A pure receipt is authoritative for this frozen corridor only while its
\* immutable deadline is no later than every member's actual timeout
\* deadline.  `AdequateLeaderFixedCorridorDeadlineSource` establishes exactly
\* this relation.  Carrying it here excludes a fabricated future receipt from
\* extending the quantitative window after the real roster deadline.
AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow(
    leaderContext, receipt) ==
  \A node \in AdequateLeaderFrozenResponsiveRoster(leaderContext):
    receipt.deadlineReceipt.deadline <= asyncNodeDeadlines[node]

AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
    target, leaderContext, leader, leaderView, receipt) ==
  /\ target = leader
  /\ receipt \in AdequateLeaderAuthorityDeadlineReceiptSet
  /\ receipt.deadlineReceipt =
       AsyncFixedCorridorDeadlineReceipt(
         leader, leaderContext, leaderView,
         receipt.deadlineReceipt.armedAt)
  /\ AdequateLeaderFrozenTargetCorridor(
       leader, leaderContext, leader, leaderView)
  /\ AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow(
       leaderContext, receipt)
  /\ receipt.deadlineReceipt.armedAt <= asyncNow
  /\ asyncNow < receipt.deadlineReceipt.deadline

\* The pure deadline window survives a subject replacement.  A fixed-subject
\* rank cell does not: it is valid only while the leader's current proposal
\* subject is the immutable episode subject.  Subject replacement is handled
\* by the source-frozen admission-cut episode below before this kernel starts.
AdequateLeaderAuthorityDeadlineFixedSubjectWindowActive(
    target, leaderContext, leader, leaderView, receipt) ==
  /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
       target, leaderContext, leader, leaderView, receipt)
  /\ AsyncProposalSubject(leader) = receipt.subject

\* A frozen fresh-self source window may not exit before its immutable
\* deadline unless the self leader reaches Decision.
AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
         target, leaderContext, leader, leaderView, receipt)
    /\ ~NodeHasDecision(target)
    /\ [AsyncNext]_AsyncAllVars
    /\ asyncNow' < receipt.deadlineReceipt.deadline
    => \/ NodeHasDecision(target)'
       \/ /\ (AdequateLeaderFrozenTargetCorridor(
                 target, leaderContext, leader, leaderView))'
          /\ (AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
                 target, leaderContext, leader, leaderView, receipt))'

AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty(
    specification) ==
  specification
    => [][AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider]_AsyncAllVars

AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider ==
  \A target \in ValidatorIds:
    /\ NodeHasDecision(target)
    /\ [AsyncNext]_AsyncAllVars
    => NodeHasDecision(target)'

AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty(
    specification) ==
  specification
    => [][AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider]_AsyncAllVars

AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty(
    specification) ==
  specification
    => [](\A target \in ValidatorIds,
             leaderContext \in ContextRecords,
             leader \in ValidatorIds,
             leaderView \in Views,
             receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
            /\ AdequateLeaderFrozenTargetCorridor(
                 target, leaderContext, leader, leaderView)
            /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
                 target, leaderContext, leader, leaderView, receipt)
              => [](/\ asyncNow < receipt.deadlineReceipt.deadline
                    /\ ~NodeHasDecision(target)
                      => /\ AdequateLeaderFrozenTargetCorridor(
                               target, leaderContext, leader, leaderView)
                         /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
                               target, leaderContext, leader, leaderView,
                               receipt)))

THEOREM AdequateLeaderAuthorityDeadlineStepCarryPreventsPrematureExit ==
  \A specification:
    /\ AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty(
         specification)
    /\ AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty(
         specification)
      => AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty(
           specification)
BY PTL
   DEF AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty,
       AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty,
       AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider,
       AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty,
       AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider

THEOREM AsyncSpecProvidesAdequateLeaderAuthorityDeadlineDecisionRetention ==
  \A initialContext:
    AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty(
      AsyncSpecAt(initialContext))
BY AdequateLeaderAsyncBracketStepPreservesTargetDecision, PTL
   DEF AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty,
       AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider

AdequateLeaderAuthorityDeadlineFreshSource(
    target, leaderContext, leader, leaderView, receipt) ==
  /\ target = leader
  /\ receipt =
       AdequateLeaderAuthorityDeadlineReceipt(
         leader, leaderContext, leaderView,
         receipt.deadlineReceipt.armedAt,
         AsyncProposalSubject(leader))
  /\ AdequateLeaderFixedCorridorDeadlineSource(
       leader, leaderContext, leaderView,
       receipt.deadlineReceipt.armedAt,
       receipt.deadlineReceipt.deadline)
  /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
       target, leaderContext, leader, leaderView, receipt)

THEOREM AdequateLeaderAuthorityDeadlineFreshSourceOwnsFrozenRosterWindow ==
  \A target, leaderContext, leader, leaderView, receipt:
    AdequateLeaderAuthorityDeadlineFreshSource(
      target, leaderContext, leader, leaderView, receipt)
      => AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow(
           leaderContext, receipt)
BY Isa
   DEF AdequateLeaderAuthorityDeadlineFreshSource,
       AdequateLeaderAuthorityDeadlineFreshSelfWindowActive,
       AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow

AdequateLeaderAuthorityDeadlineStrictCorridorExit(
    target, leaderContext, leader, leaderView) ==
  /\ ~AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ AdequateLeaderTargetAnyCorridorExitHandoff(
       target, leaderContext, leader, leaderView)

AdequateLeaderAuthorityDeadlineTargetDecision(
    target, receipt) ==
  /\ NodeHasDecision(target)
  /\ asyncNow < receipt.deadlineReceipt.deadline

AdequateLeaderAuthorityDeadlineLeaderDecisionSource(
    target, leaderContext, leader, leaderView, receipt, qc) ==
  /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
       target, leaderContext, leader, leaderView, receipt)
  /\ target \in AsyncCurrentResponsiveVoters
  /\ leader \in AsyncCurrentResponsiveVoters
  /\ DecisionSourceAt(leader, qc)
  /\ asyncNow
       <= receipt.deadlineReceipt.armedAt
            + AdequateLeaderFixedLeaderPipelineClockBudget

AdequateLeaderAuthorityDeadlineLeaderPipelineCorridorExit(
    target, leaderContext, leader, leaderView, receipt) ==
  /\ AdequateLeaderAuthorityDeadlineStrictCorridorExit(
       target, leaderContext, leader, leaderView)
  /\ asyncNow
       <= receipt.deadlineReceipt.armedAt
            + AdequateLeaderFixedLeaderPipelineClockBudget

AdequateLeaderAuthorityDeadlineLeaderPipelineGoal(
    target, leaderContext, leader, leaderView, receipt) ==
  \/ AdequateLeaderAuthorityDeadlineTargetDecision(target, receipt)
  \/ AdequateLeaderAuthorityDeadlineLeaderPipelineCorridorExit(
       target, leaderContext, leader, leaderView, receipt)
  \/ \E qc \in QcRecordSet:
       AdequateLeaderAuthorityDeadlineLeaderDecisionSource(
         target, leaderContext, leader, leaderView, receipt, qc)

AdequateLeaderAuthorityDeadlineTargetServiceGoal(
    target, leaderContext, leader, leaderView, receipt) ==
  \/ AdequateLeaderAuthorityDeadlineTargetDecision(target, receipt)
  \/ AdequateLeaderAuthorityDeadlineLeaderPipelineCorridorExit(
       target, leaderContext, leader, leaderView, receipt)

THEOREM AdequateLeaderFreshReceiptOwnsCompatibleConfiguredCeiling ==
  \A target, leaderContext, leader, leaderView, receipt:
    /\ AdequateLeaderFixedConfiguredDeadlineCompatibility
    /\ AdequateLeaderAuthorityDeadlineFreshSource(
         target, leaderContext, leader, leaderView, receipt)
      => /\ receipt.deadlineReceipt.deadline
              = receipt.deadlineReceipt.armedAt
                   + AsyncFixedCorridorServiceBudget + 1
         /\ receipt.deadlineReceipt.armedAt < receipt.deadlineReceipt.deadline
         /\ receipt.deadlineReceipt.armedAt
              + AdequateLeaderFixedLeaderPipelineClockBudget
              + AdequateLeaderFixedCommitQcDisseminationClockBudget
              <= receipt.deadlineReceipt.deadline - 1
BY SMT
   DEF AdequateLeaderAuthorityDeadlineFreshSource,
       AdequateLeaderAuthorityDeadlineReceipt,
       AdequateLeaderAuthorityDeadlineFreshSelfWindowActive,
       AdequateLeaderFixedCorridorDeadlineSource,
       AsyncFixedCorridorDeadlineReceipt,
       AdequateLeaderFixedConfiguredDeadlineCompatibility

(***************************************************************************
Exact missing per-action provider.

The deadline rank is intentionally not `<<clockSlack, physicalRank>>`.
A Tick could consume that rank without servicing the selected owner.  The
primary coordinate is instead the number of unfinished immutable pipeline
tokens.  Inside one token, the live phase-local origin count precedes the
current origin's cumulative action debt and exact selected service deadline:

  <<remaining pipeline tokens,
    <<live origin slots,
      <<per-origin action debt, selected due-service slack>>>>.

Each immutable phase-local origin slot owns one complete physical episode.
The token charge is therefore the product of the finite slot capacity and
the per-slot episode charge.  The current slot's debt adds its frozen causal
cut and physical rank once; a provider must still prove that parent/child
handoff consumes that same episode rather than recharging it.

At a fixed primary and action coordinate, Tick strictly lowers only service
slack.  At zero slack, the selected runner, I/O, reservation, ingress, or
producer action must strictly lower cumulative debt or reach a terminal; it
may reset the next selected service slack by at most `AsyncDeliveryBound`.
Retiring a durable protocol milestone strictly lowers the outer token count
and permits both inner coordinates to reset.  Replenishment is never a goal.
***************************************************************************)

AdequateLeaderFixedPipelinePhaseForSemanticRank(semanticRank) ==
  CASE semanticRank[1] = 4 -> "Proposal"
    [] semanticRank[1] = 3 -> "Prepare"
    [] semanticRank[1] = 2 -> "Commit"
    [] semanticRank[1] = 1 -> "Decision"
    [] OTHER -> "Terminal"

AdequateLeaderFixedSemanticHandoffDebt(semanticRank) ==
  LET stageDebt ==
        IF semanticRank[2] = 0 THEN 0 ELSE semanticRank[2] - 1
  IN stageDebt
       + IF semanticRank[1] = 4 THEN AsyncChunkCount ELSE 0

AdequateLeaderFixedPerTokenCumulativeActionDebt(
    candidate, node, cutoffOrdinal, semanticRank) ==
  AdequateLeaderFixedCutCumulativeActionDebt(node, cutoffOrdinal)
    + AdequateLeaderFixedCandidatePhysicalRank(candidate)
    + 1

THEOREM AdequateLeaderFixedSemanticHandoffDebtFitsOriginSlotCarrier ==
  \A semanticRank \in (1..4) \X (0..9):
    ModelConfiguration
      => /\ AdequateLeaderFixedSemanticHandoffDebt(semanticRank) \in Nat
         /\ AdequateLeaderFixedSemanticHandoffDebt(semanticRank)
              <= AsyncChunkCount + 8
BY SMT
   DEF AdequateLeaderFixedSemanticHandoffDebt,
       ModelConfiguration, AsyncConfiguration

THEOREM AdequateLeaderFixedPerTokenDebtFitsConfiguredCharge ==
  \A candidate \in AsyncCandidateSet,
     node \in ValidatorIds,
     cutoffOrdinal \in Nat,
     semanticRank \in (1..4) \X (0..9):
    /\ ModelConfiguration
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ CandidateScheduled(candidate)
    => /\ AdequateLeaderFixedPerTokenCumulativeActionDebt(
              candidate, node, cutoffOrdinal, semanticRank)
              \in Nat
       /\ AdequateLeaderFixedPerTokenCumulativeActionDebt(
              candidate, node, cutoffOrdinal, semanticRank)
              <= AdequateLeaderFixedPerOriginSlotEpisodeCharge
BY AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget,
   AdequateLeaderScheduledCandidatePhysicalRankIsBounded,
   AdequateLeaderFixedProducerAndPhysicalWindowFitConfiguredBudget,
   SMT
   DEF AdequateLeaderFixedPerTokenCumulativeActionDebt,
       AdequateLeaderFixedPerOriginSlotEpisodeCharge,
       AdequateLeaderFixedCandidateCharge,
       AsyncCandidatePhysicalServiceBudget

AdequateLeaderFixedFairOwner(ownerKind, node, source) ==
  [ownerKind |-> ownerKind, node |-> node, source |-> source, slot |-> 0]

AdequateLeaderFixedTickOwner(node) ==
  AdequateLeaderFixedFairOwner("Tick", node, AsyncUntrustedSource)

AdequateLeaderFixedRetireOwner(slot) ==
  [ownerKind |-> "Retire",
   node |-> 0,
   source |-> AsyncUntrustedSource,
   slot |-> slot]

\* This carrier mirrors the complete post-GST slice of `AsyncFairnessAt`.
\* Pre-GST setup/replay actions are excluded because every fresh-self source
\* already has `gst`; activation, historical open/discovery, leader-wire
\* retirement, and the three producer continuations remain distinct
\* weakly-fair actions.  A union action would not justify service of any one
\* continuation family.
AdequateLeaderFixedSelectedServiceOwnerSet(initialContext) ==
  {AdequateLeaderFixedTickOwner(node):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {AdequateLeaderFixedFairOwner("TickPacket", recipient, source):
     recipient \in Responsive,
     source \in AsyncIngressSources}
  \cup
  {AdequateLeaderFixedFairOwner(
     "Activate", node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner(
     "RunNode", node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {AdequateLeaderFixedFairOwner(
     "OpenHistoricalRecovery", node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner(
     "RunHistoricalRecovery", node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner(
     "RunHistoricalServer", node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner(
     "CommitCertificateDiscovery", node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {AdequateLeaderFixedFairOwner(
     "HistoricalCommitCertificateDiscovery",
     node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner(
     "ServiceIo", node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner(
     "ServiceHistoricalIo", node, AsyncUntrustedSource):
     node \in Responsive}
  \cup
  {AdequateLeaderFixedFairOwner("Admit", recipient, source):
     recipient \in Responsive,
     source \in AsyncIngressSources}
  \cup
  {AdequateLeaderFixedFairOwner(
     "AdmitHistorical", recipient, source):
     recipient \in ValidatorIds,
     source \in AsyncIngressSources}
  \cup
  {AdequateLeaderFixedRetireOwner(slot):
     slot \in AsyncLeaderWireLifecycleSlotSet}
  \cup
  {AdequateLeaderFixedFairOwner(
     "ResolveLocalProducer", node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {AdequateLeaderFixedFairOwner(
     "ServiceConditionalProducer", node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}
  \cup
  {AdequateLeaderFixedFairOwner(
     "ServiceVolatileProducer", node, AsyncUntrustedSource):
     node \in AsyncVotersAt(initialContext)}

AdequateLeaderFixedSelectedServiceOwnerAction(owner) ==
  CASE owner.ownerKind \in {"Tick", "TickPacket"} -> AsyncTick
    [] owner.ownerKind = "Activate" ->
         AsyncActivateServiceNode(owner.node)
    [] owner.ownerKind = "RunNode" ->
         PostGstRunNode(owner.node)
    [] owner.ownerKind = "OpenHistoricalRecovery" ->
         PostGstOpenHistoricalRecovery(owner.node)
    [] owner.ownerKind = "RunHistoricalRecovery" ->
         PostGstRunHistoricalRecoveryNode(owner.node)
    [] owner.ownerKind = "RunHistoricalServer" ->
         PostGstRunHistoricalServer(owner.node)
    [] owner.ownerKind = "CommitCertificateDiscovery" ->
         PostGstCommitCertificateDiscovery(owner.node)
    [] owner.ownerKind = "HistoricalCommitCertificateDiscovery" ->
         PostGstHistoricalCommitCertificateDiscovery(owner.node)
    [] owner.ownerKind = "ServiceIo" ->
         PostGstServiceIoWorker(owner.node)
    [] owner.ownerKind = "ServiceHistoricalIo" ->
         PostGstServiceHistoricalRecoveryIoWorker(owner.node)
    [] owner.ownerKind = "Admit" ->
         PostGstAdmitHiddenPacket(owner.node, owner.source)
    [] owner.ownerKind = "AdmitHistorical" ->
         PostGstAdmitHistoricalRecoveryPacket(
           owner.node, owner.source)
    [] owner.ownerKind = "Retire" ->
         PostGstRetireLeaderWireLifecycleSlot(owner.slot)
    [] owner.ownerKind = "ResolveLocalProducer" ->
         PostGstResolveLocalCandidateProducerContinuation(owner.node)
    [] owner.ownerKind = "ServiceConditionalProducer" ->
         PostGstServiceConditionalTransportProducerContinuation(
           owner.node)
    [] owner.ownerKind = "ServiceVolatileProducer" ->
         PostGstServiceVolatileBodyProducerContinuation(owner.node)
    [] OTHER -> FALSE

THEOREM AdequateLeaderFixedReadyLeaderWireRetirementDisablesTick ==
  \A slot \in AsyncLeaderWireLifecycleSlotSet:
    /\ gst
    /\ AsyncLeaderWireLifecycleRetirementReady(slot)
      => ~ENABLED AsyncTick
BY ExpandENABLED, Isa
   DEF AsyncTick, AsyncTickEnabled,
       AsyncLeaderWireLifecycleRetirementReady

THEOREM AdequateLeaderFixedSelectedOwnerUsesExactAsyncFairness ==
  \A initialContext, owner:
    owner \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext)
      => (AsyncSpecAt(initialContext)
            => WF_AsyncAllVars(
                 AdequateLeaderFixedSelectedServiceOwnerAction(owner)))
BY Isa, PTL
   DEF AdequateLeaderFixedSelectedServiceOwnerSet,
       AdequateLeaderFixedFairOwner,
       AdequateLeaderFixedTickOwner,
       AdequateLeaderFixedRetireOwner,
       AdequateLeaderFixedSelectedServiceOwnerAction,
       AsyncSpecAt, AsyncFairnessAt

\* This is only a named projection of the weak-fair actions already present
\* in `AsyncFairnessAt`.  Making it an explicit parameter prevents a theorem
\* quantified over an arbitrary specification from silently importing the
\* unrelated `AsyncSpecAt(initialContext)` behavior; it adds no fairness arm.
AdequateLeaderFixedSelectedOwnerFairnessProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords:
         \A owner \in
              AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
           WF_AsyncAllVars(
             AdequateLeaderFixedSelectedServiceOwnerAction(owner))

\* Arbitrary-specification service combinators must carry the concrete
\* transition relation whose bracket steps frame their selected owner.  This
\* is behavior, not an additional fairness assumption.
AdequateLeaderAsyncNextBehaviorProperty(specification) ==
  specification => [][AsyncNext]_AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness ==
  \A initialContext:
    AdequateLeaderFixedSelectedOwnerFairnessProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AdequateLeaderFixedSelectedOwnerUsesExactAsyncFairness,
   PTL
   DEF AdequateLeaderFixedSelectedOwnerFairnessProperty

THEOREM AsyncLiveProvidesAdequateLeaderAsyncNextBehavior ==
  \A initialContext:
    AdequateLeaderAsyncNextBehaviorProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec, PTL
   DEF AdequateLeaderAsyncNextBehaviorProperty, AsyncSpecAt

(***************************************************************************
End-to-end peer/phase tokens.

The coarse token may not be owned by `candidate.node`: a delivered vote is
executed at the leader but belongs to the authenticated signer obligation.
Likewise PrepareQC and CommitQC delivery belong to the recipient's next
phase.  The root causal item is immutable across every child, so this mapping
also survives reducer parent/child handoff.

Prepare and Commit do not retire at the sender's local WAL intent.  They
retire only after the exact signed vote is received at the frozen leader, or
an exact QC/Decision makes that vote unnecessary.  A token with a still-live
charged tail remains in the quantitative carrier even if its semantic
milestone is already visible.

One token may legitimately have several live causal roots.  Proposal fanout
has one Proposal root plus `AsyncChunkCount` distinct Chunk roots per
recipient; vote and QC handoffs may overlap their sender-local and
leader/recipient roots.  The accounting below therefore projects every root
to an exact phase-local slot and charges the complete finite slot episode.
It never assumes that the raw live-origin set is a singleton.
***************************************************************************)

AdequateLeaderFixedExactPhaseQc(
    phase, leaderContext, leaderView, subject) ==
  \E qc \in IF phase = "Prepare" THEN prepareQCs ELSE commitQCs:
    /\ qc.context = leaderContext
    /\ qc.view = leaderView
    /\ qc.phase = phase
    /\ qc.subject = subject

AdequateLeaderFixedExactVoteReceivedAtLeader(
    signer, phase, leaderContext, leader, leaderView, subject) ==
  VoteAt(
    leader,
    Vote(leaderContext, leaderView, phase, subject, signer))
    \in receivedVotes

AdequateLeaderFixedAuthorityPipelineTokenCompleted(
    token, leaderContext, leader, leaderView, subject) ==
  CASE token[2] = "Proposal" ->
         \/ AdequateLeaderFixedPipelineProposalSeen(
              token[1], leaderContext, leader, leaderView, subject)
         \/ AdequateLeaderFixedExactPhaseQc(
              "Prepare", leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedExactPhaseQc(
              "Commit", leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineNodeTerminal(
              token[1], leaderContext)
    [] token[2] = "Prepare" ->
         \/ AdequateLeaderFixedExactVoteReceivedAtLeader(
              token[1], "Prepare", leaderContext,
              leader, leaderView, subject)
         \/ AdequateLeaderFixedExactPhaseQc(
              "Prepare", leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedExactPhaseQc(
              "Commit", leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineNodeTerminal(
              token[1], leaderContext)
    [] token[2] = "Commit" ->
         \/ AdequateLeaderFixedExactVoteReceivedAtLeader(
              token[1], "Commit", leaderContext,
              leader, leaderView, subject)
         \/ AdequateLeaderFixedExactPhaseQc(
              "Commit", leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineNodeTerminal(
              token[1], leaderContext)
    [] token[2] = "Decision" ->
         AdequateLeaderFixedPipelineNodeTerminal(
           token[1], leaderContext)
    [] OTHER -> FALSE

AdequateLeaderFixedOriginRootItem(origin) ==
  origin.payload.item

AdequateLeaderFixedLocalOriginProposalPhases ==
  {"AssembleBody", "BeginProposal", "PersistProposal", "SignProposal"}

AdequateLeaderFixedLocalOriginPreparePhases ==
  {"BeginPrepare", "PersistPrepare"}

AdequateLeaderFixedLocalOriginCommitPhases ==
  {"BeginLockCommit", "PersistLockCommit"}

AdequateLeaderFixedLocalOriginDecisionPhases ==
  {"BeginDecision", "PersistDecision"}

AdequateLeaderFixedOriginProtocolPhase(origin) ==
  LET item == AdequateLeaderFixedOriginRootItem(origin)
  IN CASE item.kind \in {"Proposal", "Chunk"} -> "Proposal"
       [] item.kind = "PrepareVote" -> "Prepare"
       [] item.kind = "PrepareQC" -> "Commit"
       [] item.kind = "CommitVote" -> "Commit"
       [] item.kind = "CommitQC" -> "Decision"
       [] /\ item.kind = "NoItem"
          /\ origin.phase
               \in AdequateLeaderFixedLocalOriginProposalPhases ->
            "Proposal"
       [] /\ item.kind = "NoItem"
          /\ origin.phase
               \in AdequateLeaderFixedLocalOriginPreparePhases ->
            "Prepare"
       [] /\ item.kind = "NoItem"
          /\ origin.phase
               \in AdequateLeaderFixedLocalOriginCommitPhases ->
            "Commit"
       [] /\ item.kind = "NoItem"
          /\ origin.phase
               \in AdequateLeaderFixedLocalOriginDecisionPhases ->
            "Decision"
       [] OTHER -> "NoPipelinePhase"

AdequateLeaderFixedOriginProtocolPeer(origin) ==
  LET item == AdequateLeaderFixedOriginRootItem(origin)
  IN IF item.kind \in {"PrepareVote", "CommitVote"}
     THEN item.source
     ELSE origin.target

AdequateLeaderFixedOriginProtocolToken(origin) ==
  <<AdequateLeaderFixedOriginProtocolPeer(origin),
    AdequateLeaderFixedOriginProtocolPhase(origin)>>

AdequateLeaderFixedOriginProtocolSlot(origin) ==
  LET item == AdequateLeaderFixedOriginRootItem(origin)
  IN CASE item.kind = "Chunk" ->
            <<"Chunk", item.envelope.chunk>>
       [] item.kind
            \in {"Proposal", "PrepareVote", "PrepareQC",
                 "CommitVote", "CommitQC"} ->
            <<"Wire", item.kind>>
       [] item.kind = "NoItem" ->
            <<"Local", origin.phase>>
       [] OTHER -> <<"Invalid", "Invalid">>

AdequateLeaderFixedPipelineOriginSlotCarrier(phase) ==
  CASE phase = "Proposal" ->
         {<<"Local", localPhase>>:
            localPhase \in AdequateLeaderFixedLocalOriginProposalPhases}
           \cup {<<"Wire", "Proposal">>}
           \cup {<<"Chunk", chunk>>: chunk \in AsyncChunks}
    [] phase = "Prepare" ->
         {<<"Local", localPhase>>:
            localPhase \in AdequateLeaderFixedLocalOriginPreparePhases}
           \cup {<<"Wire", "PrepareVote">>}
    [] phase = "Commit" ->
         {<<"Local", localPhase>>:
            localPhase \in AdequateLeaderFixedLocalOriginCommitPhases}
           \cup {<<"Wire", "PrepareQC">>,
                 <<"Wire", "CommitVote">>}
    [] phase = "Decision" ->
         {<<"Local", localPhase>>:
            localPhase \in AdequateLeaderFixedLocalOriginDecisionPhases}
           \cup {<<"Wire", "CommitQC">>}
    [] OTHER -> {}

THEOREM AdequateLeaderFixedPipelineOriginSlotCarrierFitsConfiguredTail ==
  \A phase \in AdequateLeaderFixedPipelinePhases:
    ModelConfiguration
      => /\ IsFiniteSet(
               AdequateLeaderFixedPipelineOriginSlotCarrier(phase))
         /\ Cardinality(
              AdequateLeaderFixedPipelineOriginSlotCarrier(phase))
              <= AsyncChunkCount + 8
BY FS_Interval, FS_Image, FS_Union, FS_Subset,
   FS_CardinalityType, IsaT(300)
   DEF AdequateLeaderFixedPipelineOriginSlotCarrier,
       AdequateLeaderFixedLocalOriginProposalPhases,
       AdequateLeaderFixedLocalOriginPreparePhases,
       AdequateLeaderFixedLocalOriginCommitPhases,
       AdequateLeaderFixedLocalOriginDecisionPhases,
       AdequateLeaderFixedPipelinePhases,
       AsyncChunks, ModelConfiguration, AsyncConfiguration

\* Only the leader-directed copy of a broadcast vote is charged to signer
\* progress.  Proposal/Chunk and QC fanout copies are charged by recipient.
AdequateLeaderFixedOriginIsExactPipelineEpisode(
    origin, leaderContext, leader, leaderView, subject) ==
  LET item == AdequateLeaderFixedOriginRootItem(origin)
  IN /\ origin.context = leaderContext
     /\ origin.height = leaderContext.height
     /\ origin.leader = leader
     /\ origin.view = leaderView
     /\ origin.subject = subject
     /\ AdequateLeaderFixedOriginProtocolToken(origin)
          \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
     /\ IF item.kind \in {"PrepareVote", "CommitVote"}
        THEN item.payload.recipient = leader
        ELSE TRUE

(***************************************************************************
Source-frozen subject-replacement owners.

`PersistObservePrepare` may replace the proposal subject without changing the
fresh context or view.  The fixed-subject rank below therefore cannot start
until every exact replacement owner whose physical carrier was admitted
before the source cut has either retired or handed the same logical lifecycle
token to a causal child for the final subject.

The admitted owner identity is receiver-local proof data.  It contains the
complete frozen corridor identity, immutable causal origin and lifecycle
ordinal, plus the current all-ingress physical carrier ordinal.  Active
Ingress/Runtime wire and Reserved/Materialized producer views recover both
values from the same leader-wire lifecycle record, so a handoff does not
increment the logical owner count.

A restart-parked Dormant lifecycle retains only the immutable lifecycle token;
it owns no physical predecessor.  A real retry must reserve a fresh physical
ordinal before publication.  Consequently it may remain in the coalescing
universe, but its old lifecycle ordinal never places it before an already
admitted target.

An in-flight or sender-retained packet has no recipient ordinal.  It is
tracked separately below by its stable route-neutral identity and the
existing finite transport/non-descent episode.  Only its atomic local
acceptance may project that route into an admitted owner.

The source cut contains only active owners whose current physical carrier is
strictly before its frozen all-ingress high-watermark.  Terminal records and
strict slot high-watermarks are consulted solely as the serviced subtraction.
A Dormant record remains admitted as parked transport ownership but owns no
ingress selector barrier; a Terminal record cannot be selected as fresh work,
and an exact retry cannot recharge an already serviced identity.
***************************************************************************)

AdequateLeaderFixedSubjectReplacementOwnerIdentitySet ==
  [target: ValidatorIds,
   context: ContextRecords,
   leader: ValidatorIds,
   view: Views,
   subject: Subjects,
   phase: AsyncWorkKinds,
   node: ValidatorIds,
   origin: AsyncCandidateCausalOriginSet,
   ordinal: Nat \ {0},
   carrierOrdinal: Nat]

AdequateLeaderFixedSubjectReplacementOwnerIdentity(
    target, leaderContext, leader, leaderView,
    node, origin, ordinal, carrierOrdinal) ==
  [target |-> target,
   context |-> leaderContext,
   leader |-> leader,
   view |-> leaderView,
   subject |-> origin.subject,
   phase |-> origin.phase,
   node |-> node,
   origin |-> origin,
   ordinal |-> ordinal,
   carrierOrdinal |-> carrierOrdinal]

\* Only an authenticated PrepareQC delivery to the fresh self leader can run
\* the ObservePrepare WAL path which changes `AsyncProposalSubject(leader)`.
\* The certificate may be from any lower/equal view already admitted into the
\* synchronized leader's shared scheduler prefix.
AdequateLeaderFixedSubjectReplacementOrigin(
    origin, target, leaderContext, leader, leaderView) ==
  LET item == AdequateLeaderFixedOriginRootItem(origin)
  IN /\ target = leader
     /\ origin \in AsyncCandidateCausalOriginSet
     /\ origin.target = leader
     /\ origin.context = leaderContext
     /\ origin.height = leaderContext.height
     /\ origin.view \in 0..leaderView
     /\ origin.subject \in Subjects
     /\ origin.phase = "DeliverQC"
     /\ item.kind = "PrepareQC"
     /\ item.envelope.recipient = leader
     /\ item.envelope.qc.context = leaderContext
     /\ item.envelope.qc.height = leaderContext.height
     /\ item.envelope.qc.phase = "Prepare"
     /\ item.envelope.qc.view = origin.view
     /\ item.envelope.qc.subject = origin.subject

AdequateLeaderFixedPreAdmissionRouteIdentityCarrier ==
  {AsyncLeaderWireServiceIdentity(item):
     item \in AsyncNetworkItems}

AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentitySet ==
  [target: ValidatorIds,
   context: ContextRecords,
   leader: ValidatorIds,
   view: Views,
   subject: Subjects,
   phase: {"PrepareQC"},
   recipient: ValidatorIds,
   routeIdentity: AdequateLeaderFixedPreAdmissionRouteIdentityCarrier,
   origin: AsyncCandidateCausalOriginSet]

AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentity(
    item, target, leaderContext, leader, leaderView) ==
  [target |-> target,
   context |-> leaderContext,
   leader |-> leader,
   view |-> leaderView,
   subject |-> DeliverySubject(item),
   phase |-> "PrepareQC",
   recipient |-> item.envelope.recipient,
   routeIdentity |-> AsyncLeaderWireServiceIdentity(item),
   origin |->
     AsyncLeaderWireLifecycleCausalOriginAt(item, leaderContext)]

AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes(
    target, leaderContext, leader, leaderView) ==
  {AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentity(
     item, target, leaderContext, leader, leaderView):
     item \in
       {retainedItem \in asyncRetainedControl:
          /\ retainedItem.source
               \in AdequateLeaderFrozenResponsiveRoster(leaderContext)
          /\ AdequateLeaderFixedSubjectReplacementOrigin(
               AsyncLeaderWireLifecycleCausalOriginAt(
                 retainedItem, leaderContext),
               target, leaderContext, leader, leaderView)}}

AdequateLeaderFixedPreAdmissionSubjectReplacementRouteCapacity ==
  N * AsyncRetainedControlBudget

AdequateLeaderFixedActiveWireSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  {AdequateLeaderFixedSubjectReplacementOwnerIdentity(
     target, leaderContext, leader, leaderView,
     record.recipient, record.causalOrigin, record.schedulerOrdinal,
     record.physicalAdmissionOrdinal):
     record
       \in {activeRecord \in asyncLeaderWireLifecycles:
              /\ AsyncLeaderWireLifecycleActive(activeRecord)
              /\ AdequateLeaderFixedSubjectReplacementOrigin(
                   activeRecord.causalOrigin,
                   target, leaderContext, leader, leaderView)}}

AdequateLeaderFixedDormantWireSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  {AdequateLeaderFixedSubjectReplacementOwnerIdentity(
     target, leaderContext, leader, leaderView,
     record.recipient, record.causalOrigin, record.schedulerOrdinal,
     record.physicalAdmissionOrdinal):
     record
       \in {dormantRecord \in asyncLeaderWireLifecycles:
              /\ AsyncLeaderWireLifecycleDormant(dormantRecord)
              /\ AdequateLeaderFixedSubjectReplacementOrigin(
                   dormantRecord.causalOrigin,
                   target, leaderContext, leader, leaderView)}}

AdequateLeaderFixedWireSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  AdequateLeaderFixedActiveWireSubjectReplacementOwners(
    target, leaderContext, leader, leaderView)

AdequateLeaderFixedProducerSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  UNION
    {{AdequateLeaderFixedSubjectReplacementOwnerIdentity(
        target, leaderContext, leader, leaderView,
        record.node, record.causalOrigin, record.ordinal,
        lifecycle.physicalAdmissionOrdinal):
        lifecycle
          \in {wireLifecycle \in asyncLeaderWireLifecycles:
                 /\ wireLifecycle.recipient = record.node
                 /\ wireLifecycle.causalOrigin = record.causalOrigin
                 /\ wireLifecycle.schedulerOrdinal = record.ordinal
                 /\ AdequateLeaderFixedSubjectReplacementOrigin(
                      record.causalOrigin,
                      target, leaderContext, leader, leaderView)}}:
       record
         \in {producerRecord \in AsyncCandidateProducerContinuations:
                producerRecord.status \in {"Reserved", "Materialized"}}}

AdequateLeaderFixedLiveSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  AdequateLeaderFixedWireSubjectReplacementOwners(
    target, leaderContext, leader, leaderView)
    \cup
  AdequateLeaderFixedProducerSubjectReplacementOwners(
    target, leaderContext, leader, leaderView)

AdequateLeaderFixedPotentialSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  {owner
     \in AdequateLeaderFixedDormantWireSubjectReplacementOwners(
          target, leaderContext, leader, leaderView):
     \E record \in asyncLeaderWireLifecycles:
       /\ record.recipient = owner.node
       /\ record.causalOrigin = owner.origin
       /\ record.schedulerOrdinal = owner.ordinal
       /\ record.physicalAdmissionOrdinal = owner.carrierOrdinal
       /\ AsyncLeaderWireLifecycleDormant(record)
       /\ record
            \in AsyncLeaderWirePhysicalPredecessorRecordsBefore(
                 owner.node,
                 AsyncNextIngressPhysicalOrdinal(owner.node))}

\* "Admitted" is the finite lifecycle/coalescing universe, whereas "live"
\* above is the set which owns a concrete fair service action now.  Dormant
\* remains admitted so an exact retained retry cannot be misclassified as a
\* fresh post-target route, but it is absent from the physical predecessor
\* set and never selected as an active service owner.
AdequateLeaderFixedAdmittedSubjectReplacementOwners(
    target, leaderContext, leader, leaderView) ==
  AdequateLeaderFixedLiveSubjectReplacementOwners(
    target, leaderContext, leader, leaderView)
    \cup
  AdequateLeaderFixedDormantWireSubjectReplacementOwners(
    target, leaderContext, leader, leaderView)

AdequateLeaderFixedUnacceptedPreAdmissionSubjectReplacementRoutes(
    target, leaderContext, leader, leaderView) ==
  LET admitted ==
        AdequateLeaderFixedAdmittedSubjectReplacementOwners(
          target, leaderContext, leader, leaderView)
  IN {route
        \in AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes(
             target, leaderContext, leader, leaderView):
        ~\E owner \in admitted: owner.origin = route.origin}

THEOREM AdequateLeaderFixedDormantSubjectReplacementOwnsNoIngressAuthority ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views:
    \A owner
         \in AdequateLeaderFixedDormantWireSubjectReplacementOwners(
              target, leaderContext, leader, leaderView):
      \E record \in asyncLeaderWireLifecycles:
        /\ record.recipient = owner.node
        /\ record.causalOrigin = owner.origin
        /\ record.schedulerOrdinal = owner.ordinal
        /\ record.physicalAdmissionOrdinal = owner.carrierOrdinal
        /\ AsyncLeaderWireLifecycleDormant(record)
        /\ ~AsyncLeaderWireLifecycleActive(record)
        /\ ~AsyncLeaderWireLifecycleIngressProtected(record)
BY DormantLeaderWireOwnsNoIngressSchedulerBarrier, Isa
   DEF AdequateLeaderFixedDormantWireSubjectReplacementOwners

THEOREM AdequateLeaderFixedDormantSubjectReplacementOwnsNoPhysicalPredecessor ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views:
    AdequateLeaderFixedPotentialSubjectReplacementOwners(
      target, leaderContext, leader, leaderView)
      = {}
BY DormantLeaderWireOwnsNoPhysicalIngressPredecessor, Isa
   DEF AdequateLeaderFixedPotentialSubjectReplacementOwners

\* The configured bound is derived entirely from the retained transport/wire
\* slot tables and the existing producer-continuation capacity.  It is not a
\* bound over `Subjects` and creates no new environment or wire parameter.
AdequateLeaderFixedSubjectReplacementOwnerCapacity ==
  Cardinality(AsyncLeaderWireLifecycleSlotSet)
    + AsyncCandidateProducerContinuationCapacity

AdequateLeaderFixedSubjectReplacementOwnerConfiguredBound ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views:
    LET admitted ==
          AdequateLeaderFixedAdmittedSubjectReplacementOwners(
            target, leaderContext, leader, leaderView)
    IN /\ IsFiniteSet(admitted)
       /\ Cardinality(admitted)
            <= AdequateLeaderFixedSubjectReplacementOwnerCapacity

AdequateLeaderFixedSubjectReplacementOwnerConfiguredBoundProperty(
    specification) ==
  specification
    => []AdequateLeaderFixedSubjectReplacementOwnerConfiguredBound

THEOREM AdequateLeaderFixedSubjectReplacementOwnersFitConfiguredTables ==
  AsyncStrongTypeInvariant
    => AdequateLeaderFixedSubjectReplacementOwnerConfiguredBound
BY AsyncLeaderWireLifecycleSlotUniverseIsFinite,
   AsyncCandidateProducerContinuationsInjectIntoLifecycleStageOwners,
   FS_Image, FS_Union, FS_Subset, FS_CardinalityType, IsaT(900)
   DEF AdequateLeaderFixedSubjectReplacementOwnerConfiguredBound,
       AdequateLeaderFixedSubjectReplacementOwnerCapacity,
       AdequateLeaderFixedAdmittedSubjectReplacementOwners,
       AdequateLeaderFixedLiveSubjectReplacementOwners,
       AdequateLeaderFixedPotentialSubjectReplacementOwners,
       AdequateLeaderFixedWireSubjectReplacementOwners,
       AdequateLeaderFixedActiveWireSubjectReplacementOwners,
       AdequateLeaderFixedDormantWireSubjectReplacementOwners,
       AdequateLeaderFixedProducerSubjectReplacementOwners,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncIngressTypeInvariant,
       AsyncLeaderWireLifecycleTypeInvariant,
       AsyncControlServiceStateTypeInvariant

THEOREM AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementOwnerConfiguredBound ==
  \A initialContext:
    AdequateLeaderFixedSubjectReplacementOwnerConfiguredBoundProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AdequateLeaderFixedSubjectReplacementOwnersFitConfiguredTables,
   PTL
   DEF AdequateLeaderFixedSubjectReplacementOwnerConfiguredBoundProperty

\* A terminal wire lifecycle or terminal producer handoff is an exact
\* tombstone.  Pre-admission routes are deliberately absent: they may become
\* admitted only through the separate atomic-acceptance provider below.
AdequateLeaderFixedSubjectReplacementOwnerServiced(owner) ==
  /\ owner
       \in AdequateLeaderFixedSubjectReplacementOwnerIdentitySet
  /\ \/ \E record \in asyncLeaderWireLifecycles:
          /\ record.status \in {"VolatileTerminal", "Terminal"}
          /\ record.recipient = owner.node
          /\ record.causalOrigin = owner.origin
          /\ record.schedulerOrdinal = owner.ordinal
          /\ record.physicalAdmissionOrdinal = owner.carrierOrdinal
     \/ \E record \in AsyncCandidateProducerContinuations:
          /\ record.status = "Terminal"
          /\ record.node = owner.node
          /\ record.causalOrigin = owner.origin
          /\ record.ordinal = owner.ordinal

AdequateLeaderFixedSubjectReplacementOwnersBeforeOrdinal(
    target, leaderContext, leader, leaderView, ordinal) ==
  {owner
     \in AdequateLeaderFixedLiveSubjectReplacementOwners(
          target, leaderContext, leader, leaderView):
     /\ owner.ordinal < ordinal
     /\ owner.carrierOrdinal
          < AsyncNextIngressPhysicalOrdinal(leader)}

AdequateLeaderFixedPotentialSubjectReplacementOwnersBeforeOrdinal(
    target, leaderContext, leader, leaderView, ordinal) ==
  {owner
     \in AdequateLeaderFixedPotentialSubjectReplacementOwners(
          target, leaderContext, leader, leaderView):
     /\ owner.ordinal < ordinal
     /\ owner.carrierOrdinal
          < AsyncNextIngressPhysicalOrdinal(leader)}

AdequateLeaderFixedSubjectReplacementLastOwner(owners) ==
  CHOOSE owner \in owners:
    \A other \in owners: other.ordinal <= owner.ordinal

AdequateLeaderFixedSubjectReplacementCutSet ==
  [target: ValidatorIds,
   context: ContextRecords,
   leader: ValidatorIds,
   view: Views,
   sourceSubject: Subjects,
   sourceTargetOrdinal: Nat \ {0},
   physicalCutoffOrdinal: Nat \ {0},
   schedulerCeiling: Nat \ {0},
   owners:
     SUBSET AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
   potentialOwners:
     SUBSET AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
   targetOwner:
     AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
   predecessorOwners:
     SUBSET AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
   predecessorOrigins: SUBSET AsyncCandidateCausalOriginSet]

AdequateLeaderFixedSubjectReplacementCut(
    target, leaderContext, leader, leaderView,
    sourceSubject, sourceTargetOrdinal) ==
  LET owners ==
        AdequateLeaderFixedSubjectReplacementOwnersBeforeOrdinal(
          target, leaderContext, leader, leaderView,
          sourceTargetOrdinal)
      potentialOwners ==
        AdequateLeaderFixedPotentialSubjectReplacementOwnersBeforeOrdinal(
          target, leaderContext, leader, leaderView,
          sourceTargetOrdinal)
      targetOwner ==
        AdequateLeaderFixedSubjectReplacementLastOwner(owners)
  IN [target |-> target,
      context |-> leaderContext,
      leader |-> leader,
      view |-> leaderView,
      sourceSubject |-> sourceSubject,
      sourceTargetOrdinal |-> sourceTargetOrdinal,
      physicalCutoffOrdinal |->
        AsyncNextIngressPhysicalOrdinal(leader),
      schedulerCeiling |->
        AsyncNextCandidateLifecycleOrdinal(leader),
      owners |-> owners,
      potentialOwners |-> potentialOwners,
      targetOwner |-> targetOwner,
      predecessorOwners |-> owners \ {targetOwner},
      predecessorOrigins |->
        AsyncCausalEpisodeFrozenPredecessorOrigins(
          leader, targetOwner.ordinal)]

\* This equality is asserted only at the source state.  Every temporal
\* successor carries the resulting `cut` value as an immutable parameter.
\* `targetOwner` is the last source-admitted subject-changing owner ahead of
\* the original fixed-subject target.  Once its strictly older predecessors
\* retire, its own causal child is the fixed-subject target.  Every live owner
\* outside the active cut either has a later logical lifecycle ordinal or a
\* physical carrier at/after the frozen all-ingress cutoff.  Dormant lifecycle
\* tokens remain only in the separate coalescing universe.
AdequateLeaderFixedSubjectReplacementCutSource(
    target, leaderContext, leader, leaderView,
    sourceSubject, sourceTargetOrdinal, cut) ==
  LET live ==
        AdequateLeaderFixedLiveSubjectReplacementOwners(
          target, leaderContext, leader, leaderView)
      owners ==
        AdequateLeaderFixedSubjectReplacementOwnersBeforeOrdinal(
          target, leaderContext, leader, leaderView,
          sourceTargetOrdinal)
      potentialOwners ==
        AdequateLeaderFixedPotentialSubjectReplacementOwnersBeforeOrdinal(
          target, leaderContext, leader, leaderView,
          sourceTargetOrdinal)
  IN /\ cut =
          AdequateLeaderFixedSubjectReplacementCut(
            target, leaderContext, leader, leaderView,
            sourceSubject, sourceTargetOrdinal)
     /\ cut \in AdequateLeaderFixedSubjectReplacementCutSet
     /\ owners # {}
     /\ cut.owners = owners
     /\ cut.potentialOwners = potentialOwners
     /\ IsFiniteSet(cut.owners)
     /\ IsFiniteSet(cut.potentialOwners)
     /\ Cardinality(cut.owners \cup cut.potentialOwners)
          <= AdequateLeaderFixedSubjectReplacementOwnerCapacity
     /\ cut.targetOwner \in cut.owners
     /\ cut.predecessorOwners =
          cut.owners \ {cut.targetOwner}
     /\ \A owner \in cut.owners:
          owner.ordinal <= cut.targetOwner.ordinal
     /\ \A left, right \in cut.owners:
          left.ordinal = right.ordinal => left = right
     /\ cut.targetOwner.ordinal < cut.sourceTargetOrdinal
     /\ cut.physicalCutoffOrdinal =
          AsyncNextIngressPhysicalOrdinal(leader)
     /\ cut.sourceTargetOrdinal < cut.schedulerCeiling
     /\ cut.predecessorOrigins =
          AsyncCausalEpisodeFrozenPredecessorOrigins(
            leader, cut.targetOwner.ordinal)
     /\ \A owner
          \in live \ cut.owners:
          \/ cut.sourceTargetOrdinal < owner.ordinal
          \/ cut.physicalCutoffOrdinal <= owner.carrierOrdinal

AdequateLeaderFixedSubjectReplacementServicedOwners(cut) ==
  {owner \in cut.owners:
     AdequateLeaderFixedSubjectReplacementOwnerServiced(owner)}

AdequateLeaderFixedSubjectReplacementRemainingPredecessors(cut) ==
  cut.predecessorOwners
    \ AdequateLeaderFixedSubjectReplacementServicedOwners(cut)

AdequateLeaderFixedSubjectReplacementRemainingBudget(cut) ==
  Cardinality(
    AdequateLeaderFixedSubjectReplacementRemainingPredecessors(cut))

\* Dormant logical ordinals never enter the physical predecessor cut.  These
\* compatibility operators therefore denote the empty set and force
\* `knownPotential = {}`.  Keeping the names localizes the source-level change:
\* retained Dormant identities still coalesce retries, while any reactivated
\* carrier is ordered by its fresh physical ordinal in the ordinary live set.
AdequateLeaderFixedSubjectReplacementBlockingPotentialOwners(cut) ==
  {owner \in cut.potentialOwners:
     owner.ordinal < cut.targetOwner.ordinal}

AdequateLeaderFixedSubjectReplacementCurrentBlockingPotentialOwners(cut) ==
  AdequateLeaderFixedPotentialSubjectReplacementOwnersBeforeOrdinal(
    cut.target, cut.context, cut.leader, cut.view,
    cut.targetOwner.ordinal)

AdequateLeaderFixedSubjectReplacementPotentialKnownExact(
    cut, knownPotential) ==
  LET sourcePotential ==
        AdequateLeaderFixedSubjectReplacementBlockingPotentialOwners(cut)
      currentPotential ==
        AdequateLeaderFixedSubjectReplacementCurrentBlockingPotentialOwners(cut)
  IN /\ IsFiniteSet(sourcePotential)
     /\ currentPotential \subseteq sourcePotential
     /\ knownPotential \subseteq sourcePotential
     /\ sourcePotential \ currentPotential \subseteq knownPotential
     /\ knownPotential \cap currentPotential = {}

AdequateLeaderFixedSubjectReplacementPotentialDiscovered(
    cut, knownPotential) ==
  (AdequateLeaderFixedSubjectReplacementBlockingPotentialOwners(cut)
    \ AdequateLeaderFixedSubjectReplacementCurrentBlockingPotentialOwners(cut))
    \ knownPotential

\* The compatibility `knownPotential` value is forced empty, so predecessor
\* debt is exactly the source-active cut.  A Terminal identity is immediately
\* subtracted by the existing tombstone predicate.  Dormant identities never
\* enter this active set and therefore receive no invented fairness obligation.
AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners(
    cut, knownPotential) ==
  cut.predecessorOwners \cup knownPotential

AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners(
    cut, knownPotential) ==
  {owner
     \in AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners(
          cut, knownPotential):
     AdequateLeaderFixedSubjectReplacementOwnerServiced(owner)}

AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors(
    cut, knownPotential) ==
  AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners(
    cut, knownPotential)
    \ AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners(
        cut, knownPotential)

AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier ==
  Nat

AdequateLeaderFixedSubjectReplacementEpisodeRankOrdering ==
  OpToRel(<, Nat)

THEOREM AdequateLeaderFixedSubjectReplacementEpisodeRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderFixedSubjectReplacementEpisodeRankOrdering,
    AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier)
BY NatLessThanWellFounded
   DEF AdequateLeaderFixedSubjectReplacementEpisodeRankOrdering,
       AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier

AdequateLeaderFixedSubjectReplacementEpisodeDebtAtRank(
    cut, knownPotential, episodeRank) ==
  LET remaining ==
        AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors(
          cut, knownPotential)
  IN /\ AdequateLeaderFixedSubjectReplacementPotentialKnownExact(
           cut, knownPotential)
     /\ episodeRank
          \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier
     /\ episodeRank = Cardinality(remaining)

AdequateLeaderFixedSubjectReplacementInitialEpisodeRank(cut) ==
  AdequateLeaderFixedSubjectReplacementRemainingBudget(cut)

THEOREM AdequateLeaderFixedSubjectReplacementBudgetIsExactAndFinite ==
  \A cut \in AdequateLeaderFixedSubjectReplacementCutSet:
    IsFiniteSet(cut.owners)
      => /\ AdequateLeaderFixedSubjectReplacementRemainingBudget(cut)
              \in Nat
         /\ AdequateLeaderFixedSubjectReplacementRemainingBudget(cut)
              <= Cardinality(cut.owners)
BY FS_Subset, FS_CardinalityType, Isa
   DEF AdequateLeaderFixedSubjectReplacementRemainingBudget,
       AdequateLeaderFixedSubjectReplacementRemainingPredecessors,
       AdequateLeaderFixedSubjectReplacementServicedOwners

THEOREM AdequateLeaderFixedSubjectReplacementInitialEpisodeRankIsFinite ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     sourceSubject \in Subjects,
     sourceTargetOrdinal \in Nat \ {0},
     cut \in AdequateLeaderFixedSubjectReplacementCutSet:
    AdequateLeaderFixedSubjectReplacementCutSource(
      target, leaderContext, leader, leaderView,
      sourceSubject, sourceTargetOrdinal, cut)
      => AdequateLeaderFixedSubjectReplacementInitialEpisodeRank(cut)
           \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier
BY AdequateLeaderFixedSubjectReplacementBudgetIsExactAndFinite,
   AdequateLeaderFixedDormantSubjectReplacementOwnsNoPhysicalPredecessor,
   FS_Subset, FS_CardinalityType, Isa
   DEF AdequateLeaderFixedSubjectReplacementCutSource,
       AdequateLeaderFixedSubjectReplacementInitialEpisodeRank,
       AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier

AdequateLeaderFixedDiscoveredPipelineOriginPairs(
    leaderContext, leader, leaderView, subject) ==
  UNION
    {{<<node, origin>>:
        origin
          \in
            {pipelineOrigin \in
               AsyncCandidateLifecycleOrdinaryOriginsForNodeIn(
                 asyncControlServiceState, node):
               AdequateLeaderFixedOriginIsExactPipelineEpisode(
                 pipelineOrigin, leaderContext,
                 leader, leaderView, subject)}}:
       node \in AdequateLeaderFrozenResponsiveRoster(leaderContext)}

AdequateLeaderFixedLivePipelineOriginPairs(
    leaderContext, leader, leaderView, subject) ==
  UNION
    {{<<node, origin>>:
        origin
          \in
            {pipelineOrigin \in
               AsyncCandidateLifecycleActiveOriginsForNodeIn(
                 asyncControlServiceState, node):
               AdequateLeaderFixedOriginIsExactPipelineEpisode(
                 pipelineOrigin, leaderContext,
                 leader, leaderView, subject)}}:
       node \in AdequateLeaderFrozenResponsiveRoster(leaderContext)}

AdequateLeaderFixedLivePipelineOriginsForToken(
    token, leaderContext, leader, leaderView, subject) ==
  {pair \in
     AdequateLeaderFixedLivePipelineOriginPairs(
       leaderContext, leader, leaderView, subject):
     AdequateLeaderFixedOriginProtocolToken(pair[2]) = token}

AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
    token, leaderContext, leader, leaderView, subject) ==
  {pair \in
     AdequateLeaderFixedDiscoveredPipelineOriginPairs(
       leaderContext, leader, leaderView, subject):
     AdequateLeaderFixedOriginProtocolToken(pair[2]) = token}

AdequateLeaderFixedLivePipelineOriginSlotsForToken(
    token, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFixedOriginProtocolSlot(pair[2]):
     pair
       \in AdequateLeaderFixedLivePipelineOriginsForToken(
            token, leaderContext, leader, leaderView, subject)}

AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken(
    token, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFixedOriginProtocolSlot(pair[2]):
     pair
       \in AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
            token, leaderContext, leader, leaderView, subject)}

AdequateLeaderFixedLivePipelineOriginsForTokenAtNode(
    token, leaderContext, leader, leaderView, subject, node) ==
  {pair
     \in AdequateLeaderFixedLivePipelineOriginsForToken(
          token, leaderContext, leader, leaderView, subject):
     pair[1] = node}

AdequateLeaderFixedPipelineTokenNodeCutoff(
    token, leaderContext, leader, leaderView, subject,
    node, cutoffOrdinal) ==
  LET nodeOrigins ==
        AdequateLeaderFixedLivePipelineOriginsForTokenAtNode(
          token, leaderContext, leader, leaderView, subject, node)
  IN /\ nodeOrigins # {}
     /\ cutoffOrdinal \in Nat \ {0}
     /\ {pair[2]: pair \in nodeOrigins}
          \subseteq
            AsyncCausalEpisodeFrozenPredecessorOrigins(
              node, cutoffOrdinal)
     /\ \E pair \in nodeOrigins,
           record \in AsyncCandidateLifecycleAdmissions:
          /\ record.node = node
          /\ record.origin = pair[2]
          /\ record.ordinal = cutoffOrdinal

AdequateLeaderFixedAuthorityPipelineRemainingTokens(
    leaderContext, leader, leaderView, subject) ==
  {token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
     \/ ~AdequateLeaderFixedAuthorityPipelineTokenCompleted(
           token, leaderContext, leader, leaderView, subject)
     \/ AdequateLeaderFixedLivePipelineOriginsForToken(
          token, leaderContext, leader, leaderView, subject) # {}}

AdequateLeaderFixedAuthorityPipelineWindowsRemaining(
    leaderContext, leader, leaderView, subject) ==
  Cardinality(
    AdequateLeaderFixedAuthorityPipelineRemainingTokens(
      leaderContext, leader, leaderView, subject))

\* A source may claim one `4 * N` window only after this relation supplies:
\*
\*   * an injective/coalesced projection from every live raw root to its
\*     exact phase-local slot;
\*   * the configured `AsyncChunkCount + 8` slot bound; and
\*   * a maximal same-token lifecycle cutoff at every executing node, so the
\*     existing frozen causal cut contains every lower/equal root.
\*
\* Parent/wire/child handoff must carry, rather than recharge, that complete
\* multi-root cut.  This remains an explicit provider property.
AdequateLeaderFixedPipelineTokenOwnershipAndTailCarry ==
  \A leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
      LET charged ==
            AdequateLeaderFixedLivePipelineOriginsForToken(
              token, leaderContext, leader, leaderView, subject)
          chargedSlots ==
            AdequateLeaderFixedLivePipelineOriginSlotsForToken(
              token, leaderContext, leader, leaderView, subject)
          discovered ==
            AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
              token, leaderContext, leader, leaderView, subject)
          discoveredSlots ==
            AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken(
              token, leaderContext, leader, leaderView, subject)
      IN /\ IsFiniteSet(charged)
         /\ IsFiniteSet(discovered)
         /\ charged \subseteq discovered
         /\ chargedSlots
              \subseteq
                AdequateLeaderFixedPipelineOriginSlotCarrier(token[2])
         /\ discoveredSlots
              \subseteq
                AdequateLeaderFixedPipelineOriginSlotCarrier(token[2])
         /\ Cardinality(charged) = Cardinality(chargedSlots)
         /\ Cardinality(chargedSlots) <= AsyncChunkCount + 8
         /\ Cardinality(discoveredSlots) <= AsyncChunkCount + 8
         /\ (charged # {}
               => token
                    \in AdequateLeaderFixedAuthorityPipelineRemainingTokens(
                         leaderContext, leader, leaderView, subject))
         /\ \A node
              \in AdequateLeaderFrozenResponsiveRoster(leaderContext):
              LET nodeOrigins ==
                    AdequateLeaderFixedLivePipelineOriginsForTokenAtNode(
                      token, leaderContext, leader, leaderView, subject, node)
              IN nodeOrigins = {}
                   \/ \E cutoffOrdinal \in Nat \ {0}:
                        AdequateLeaderFixedPipelineTokenNodeCutoff(
                          token, leaderContext, leader, leaderView, subject,
                          node, cutoffOrdinal)

AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty(
    specification) ==
  specification
    => []AdequateLeaderFixedPipelineTokenOwnershipAndTailCarry

AdequateLeaderFixedPipelineOriginUnknownBudget(
    token, leaderContext, leader, leaderView, subject) ==
  Cardinality(
    AdequateLeaderFixedPipelineOriginSlotCarrier(token[2])
      \ AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken(
          token, leaderContext, leader, leaderView, subject))

THEOREM AdequateLeaderFixedPipelineOriginUnknownBudgetIsFinite ==
  \A token, leaderContext, leader, leaderView, subject:
    /\ ModelConfiguration
    /\ token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
    /\ AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken(
         token, leaderContext, leader, leaderView, subject)
         \subseteq
           AdequateLeaderFixedPipelineOriginSlotCarrier(token[2])
      => /\ AdequateLeaderFixedPipelineOriginUnknownBudget(
               token, leaderContext, leader, leaderView, subject) \in Nat
         /\ AdequateLeaderFixedPipelineOriginUnknownBudget(
              token, leaderContext, leader, leaderView, subject)
              <= AdequateLeaderFixedOriginSlotCapacity
BY AdequateLeaderFixedPipelineOriginSlotCarrierFitsConfiguredTail,
   FS_Subset, FS_CardinalityType, Isa
   DEF AdequateLeaderFixedPipelineOriginUnknownBudget,
       AdequateLeaderFixedOriginSlotCapacity

\* The discovered set is exact retained lifecycle/tombstone identity history,
\* not an existential ghost set.  It may grow while the frozen corridor is
\* live but never shrink.  The identity is the full `<<node, causalOrigin>>`
\* pair, not only its phase-local slot: equal-count A -> B replacement in one
\* slot is legitimate, while a later B -> A resurrection is not.  Lifecycle
\* Reclamation is an outer-token descent arm only after the semantic milestone
\* is complete and every charged live tail has drained.  A milestone alone is
\* not enough: `RemainingTokens` deliberately retains a completed token while
\* any exact origin is still live.  Once a token is absent, exact lifecycle
\* tombstones/coalescing must keep it absent for the rest of this frozen
\* corridor; monotone discovered history alone would not reject a fresh retry
\* admitted after drain.
AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
      LET discovered ==
          AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
            token, leaderContext, leader, leaderView, subject)
        active ==
          AdequateLeaderFixedLivePipelineOriginsForToken(
            token, leaderContext, leader, leaderView, subject)
        remaining ==
          AdequateLeaderFixedAuthorityPipelineRemainingTokens(
            leaderContext, leader, leaderView, subject)
    IN /\ (/\ AdequateLeaderFrozenTargetCorridor(
                  target, leaderContext, leader, leaderView)
             /\ [AsyncNext]_AsyncAllVars
             /\ token \notin remaining
             => \/ NodeHasDecision(target)'
                \/ ~(AdequateLeaderFrozenTargetCorridor(
                       target, leaderContext, leader, leaderView))'
                \/ token
                     \notin
                       (AdequateLeaderFixedAuthorityPipelineRemainingTokens(
                          leaderContext, leader, leaderView, subject))')
       /\ (/\ AdequateLeaderFrozenTargetCorridor(
                  target, leaderContext, leader, leaderView)
             /\ [AsyncNext]_AsyncAllVars
             /\ token \in remaining
             => \/ NodeHasDecision(target)'
                \/ ~(AdequateLeaderFrozenTargetCorridor(
                       target, leaderContext, leader, leaderView))'
                \/ token
                     \notin
                       (AdequateLeaderFixedAuthorityPipelineRemainingTokens(
                          leaderContext, leader, leaderView, subject))'
                \/ /\ discovered
                        \subseteq
                          (AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
                             token, leaderContext,
                             leader, leaderView, subject))'
                   /\ ((AdequateLeaderFixedLivePipelineOriginsForToken(
                          token, leaderContext, leader, leaderView, subject))'
                         \ active)
                        \subseteq
                          ((AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
                              token, leaderContext,
                              leader, leaderView, subject))'
                             \ discovered))

AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty(
    specification) ==
  specification
    => [][AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider]_AsyncAllVars

AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
    candidate, leaderContext, leader, leaderView, subject, semanticRank,
    occurrenceRank, occurrenceOwner) ==
  /\ occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier
  /\ occurrenceOwner
       \in AdequateLeaderFrozenCandidateOwnerUniverse(
            candidate.node, leaderContext, leader, leaderView, subject)
  /\ occurrenceRank[1] = semanticRank
  /\ occurrenceOwner =
       AdequateLeaderFrozenCandidateOwnerIdentity(
         candidate, semanticRank, candidate.node,
         leaderContext, leader, leaderView, subject)
  /\ AdequateLeaderTargetOccurrenceRankFrontier(
       candidate.node, leaderContext, leader, leaderView,
       subject, occurrenceRank)
  /\ AdequateLeaderTargetOccurrenceOwnerSelected(
       candidate.node, leaderContext, leader, leaderView,
       subject, occurrenceRank, occurrenceOwner)

AdequateLeaderFixedCandidateSemanticOccurrenceProjection(
    candidate, leaderContext, leader, leaderView, subject, semanticRank) ==
  \E occurrenceRank \in AdequateLeaderTargetOccurrenceRankCarrier,
     occurrenceOwner
       \in AdequateLeaderFrozenCandidateOwnerUniverse(
            candidate.node, leaderContext, leader, leaderView, subject):
    AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
      candidate, leaderContext, leader, leaderView, subject, semanticRank,
      occurrenceRank, occurrenceOwner)

AdequateLeaderFixedCandidateOwnsPipelineToken(
    candidate, token, leaderContext, leader, leaderView, subject,
    cutoffOrdinal, semanticRank) ==
  LET origin == candidate.causalOrigin
      chargedPair == <<candidate.node, origin>>
  IN /\ token
          \in AdequateLeaderFixedAuthorityPipelineRemainingTokens(
               leaderContext, leader, leaderView, subject)
     /\ token = AdequateLeaderFixedOriginProtocolToken(origin)
     /\ chargedPair
          \in AdequateLeaderFixedLivePipelineOriginsForToken(
               token, leaderContext, leader, leaderView, subject)
     /\ AdequateLeaderFixedPipelineTokenNodeCutoff(
          token, leaderContext, leader, leaderView, subject,
          candidate.node, cutoffOrdinal)
     /\ candidate.consumerContext = leaderContext
     /\ candidate.height = leaderContext.height
     /\ candidate.view = leaderView
     /\ candidate.subject = subject
     /\ ExactLeaderCandidateRank(candidate, semanticRank)
     /\ AdequateLeaderFixedCandidateSemanticOccurrenceProjection(
          candidate, leaderContext, leader, leaderView,
          subject, semanticRank)

AdequateLeaderFixedPerOriginSlotRankCarrier ==
  Nat \X Nat

AdequateLeaderFixedPerOriginSlotRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

AdequateLeaderFixedLiveOriginSlotRankCarrier ==
  Nat \X AdequateLeaderFixedPerOriginSlotRankCarrier

AdequateLeaderFixedLiveOriginSlotRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AdequateLeaderFixedPerOriginSlotRankOrdering,
    Nat,
    AdequateLeaderFixedPerOriginSlotRankCarrier)

AdequateLeaderFixedPipelineRankCarrier ==
  Nat \X AdequateLeaderFixedLiveOriginSlotRankCarrier

AdequateLeaderFixedPipelineRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AdequateLeaderFixedLiveOriginSlotRankOrdering,
    Nat,
    AdequateLeaderFixedLiveOriginSlotRankCarrier)

THEOREM AdequateLeaderFixedPipelineRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderFixedPipelineRankOrdering,
    AdequateLeaderFixedPipelineRankCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF AdequateLeaderFixedPipelineRankOrdering,
       AdequateLeaderFixedPipelineRankCarrier,
       AdequateLeaderFixedLiveOriginSlotRankOrdering,
       AdequateLeaderFixedLiveOriginSlotRankCarrier,
       AdequateLeaderFixedPerOriginSlotRankOrdering,
       AdequateLeaderFixedPerOriginSlotRankCarrier

AdequateLeaderFixedCandidateSelectedServiceDeadline(
    owner, packet, candidate) ==
  CASE owner.ownerKind = "Tick" ->
         IF CandidateInIoQueue(candidate)
         THEN asyncIoServiceDeadlines[candidate.node]
         ELSE asyncNodeServiceDeadlines[candidate.node]
    [] owner.ownerKind = "ServiceIo" ->
         asyncIoServiceDeadlines[owner.node]
    [] owner.ownerKind = "Retire" -> asyncNow
    [] owner.ownerKind = "Admit" ->
         packet.deadline
    [] OTHER -> asyncNodeServiceDeadlines[owner.node]

AdequateLeaderFixedCandidateSelectedServiceSlack(
    owner, packet, candidate) ==
  LET serviceDeadline ==
        AdequateLeaderFixedCandidateSelectedServiceDeadline(
          owner, packet, candidate)
  IN IF asyncNow < serviceDeadline
     THEN serviceDeadline - asyncNow
     ELSE 0

AdequateLeaderFixedPipelineRank(
    leaderContext, leader, leaderView, subject,
    token, candidate, cutoffOrdinal,
    semanticRank, owner, packet) ==
  LET windows ==
        AdequateLeaderFixedAuthorityPipelineWindowsRemaining(
          leaderContext, leader, leaderView, subject)
      liveSlotDebt ==
        Cardinality(
          AdequateLeaderFixedLivePipelineOriginSlotsForToken(
            token, leaderContext, leader, leaderView, subject))
      actionDebt ==
        AdequateLeaderFixedPerTokenCumulativeActionDebt(
          candidate, candidate.node, cutoffOrdinal, semanticRank)
      serviceSlack ==
        AdequateLeaderFixedCandidateSelectedServiceSlack(
          owner, packet, candidate)
  IN <<windows, <<liveSlotDebt, <<actionDebt, serviceSlack>>>>>>

\* Each undiscovered or currently live slot owns one complete cross-node
\* physical episode.  A newly discovered slot lowers `unknownBudget` before
\* it may reset `liveSlotDebt` or the selected per-slot action debt.
AdequateLeaderFixedPipelineClockPotential(
    windows, unknownBudget, liveSlotDebt, actionDebt, slack) ==
  LET originDebt == unknownBudget + liveSlotDebt
  IN IF windows = 0 \/ originDebt = 0 \/ actionDebt = 0
  THEN 0
  ELSE (windows - 1)
         * AdequateLeaderFixedProtocolTokenCharge
         * AsyncDeliveryBound
       + (originDebt - 1)
           * AdequateLeaderFixedPerOriginSlotEpisodeCharge
           * AsyncDeliveryBound
       + (actionDebt - 1) * AsyncDeliveryBound
       + slack

THEOREM AdequateLeaderFixedPipelineClockPotentialFitsConfiguredBudget ==
  \A windows \in 1..(4 * N),
     unknownBudget \in 0..AdequateLeaderFixedOriginSlotCapacity,
     liveSlotDebt \in 0..AdequateLeaderFixedOriginSlotCapacity,
     actionDebt \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
     slack \in 0..AsyncDeliveryBound:
    /\ ModelConfiguration
    /\ unknownBudget + liveSlotDebt
         \in 1..AdequateLeaderFixedOriginSlotCapacity
      => AdequateLeaderFixedPipelineClockPotential(
           windows, unknownBudget, liveSlotDebt, actionDebt, slack)
           <= AdequateLeaderFixedLeaderPipelineClockBudget
BY SMT
   DEF AdequateLeaderFixedPipelineClockPotential,
       AdequateLeaderFixedProtocolTokenCharge,
       AdequateLeaderFixedLeaderPipelineClockBudget,
       AdequateLeaderFixedLeaderPipelineActionBudget

AdequateLeaderFixedPipelineAbsoluteCeilingAtUnknownBudget(
    receipt, unknownBudget, rank) ==
  asyncNow
    + AdequateLeaderFixedPipelineClockPotential(
        rank[1], unknownBudget, rank[2][1],
        rank[2][2][1], rank[2][2][2])
    <= receipt.deadlineReceipt.armedAt
         + AdequateLeaderFixedLeaderPipelineClockBudget

AdequateLeaderFixedPipelineAbsoluteCeiling(
    receipt, token, leaderContext, leader, leaderView, rank) ==
  LET subject == receipt.subject
      unknownBudget ==
        AdequateLeaderFixedPipelineOriginUnknownBudget(
          token, leaderContext, leader, leaderView, subject)
  IN AdequateLeaderFixedPipelineAbsoluteCeilingAtUnknownBudget(
       receipt, unknownBudget, rank)

\* Admit is never a speculative candidate owner: it is selected
\* only through the pre-candidate transport arm below and only for the exact
\* selected overdue packet.  Producer-continuation actions similarly own the
\* gap after a parent departure, not an arbitrary scheduled candidate.
AdequateLeaderFixedScheduledOwnerIsExact(
    initialContext, owner, packet, candidate) ==
  /\ owner \in
       AdequateLeaderFixedSelectedServiceOwnerSet(initialContext)
  /\ owner.ownerKind \in {"Tick", "RunNode", "ServiceIo"}
  /\ CASE owner.ownerKind = "Tick" ->
            /\ owner.node = candidate.node
            /\ owner.source = AsyncUntrustedSource
       [] owner.ownerKind = "RunNode" ->
            /\ owner.node = candidate.node
            /\ owner.source = AsyncUntrustedSource
            /\ ~CandidateInIoQueue(candidate)
       [] owner.ownerKind = "ServiceIo" ->
            /\ owner.node = candidate.node
            /\ owner.source = AsyncUntrustedSource
            /\ CandidateInIoQueue(candidate)
       [] OTHER -> FALSE

AdequateLeaderFixedPipelineRankCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    token, candidate, cutoffOrdinal, semanticRank, owner, packet) ==
  LET rank ==
        AdequateLeaderFixedPipelineRank(
          leaderContext, leader, leaderView, receipt.subject,
          token, candidate, cutoffOrdinal,
          semanticRank, owner, packet)
      subject == receipt.subject
      liveSlots ==
        AdequateLeaderFixedLivePipelineOriginSlotsForToken(
          token, leaderContext, leader, leaderView, subject)
      discoveredSlots ==
        AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken(
          token, leaderContext, leader, leaderView, subject)
      unknownBudget ==
        AdequateLeaderFixedPipelineOriginUnknownBudget(
          token, leaderContext, leader, leaderView, subject)
      liveSlotDebt == Cardinality(liveSlots)
  IN /\ AdequateLeaderAuthorityDeadlineFixedSubjectWindowActive(
           target, leaderContext, leader, leaderView, receipt)
     /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
     /\ token
          \in AdequateLeaderFixedAuthorityPipelineRemainingTokens(
               leaderContext, leader, leaderView,
               receipt.subject)
     /\ liveSlots \subseteq discoveredSlots
     /\ discoveredSlots
          \subseteq
            AdequateLeaderFixedPipelineOriginSlotCarrier(token[2])
     /\ unknownBudget \in Nat
     /\ unknownBudget + liveSlotDebt
          \in 1..AdequateLeaderFixedOriginSlotCapacity
     /\ AdequateLeaderFixedCandidateOwnsPipelineToken(
          candidate, token, leaderContext, leader, leaderView, subject,
          cutoffOrdinal, semanticRank)
     /\ CandidateScheduled(candidate)
     /\ cutoffOrdinal = AsyncCandidateLifecycleOrdinal(candidate)
     /\ cutoffOrdinal \in Nat \ {0}
     /\ candidate
          \in AsyncCausalEpisodeCandidates(
               candidate.node, cutoffOrdinal)
     /\ AdequateLeaderFixedScheduledOwnerIsExact(
          initialContext, owner, packet, candidate)
     /\ AdequateLeaderFixedCandidateSelectedServiceSlack(
          owner, packet, candidate)
          \in 0..AsyncDeliveryBound
     /\ rank \in AdequateLeaderFixedPipelineRankCarrier
     /\ AdequateLeaderFixedPipelineAbsoluteCeiling(
          receipt, token, leaderContext, leader, leaderView, rank)

AdequateLeaderFixedCutTerminalForAuthority(
    target, leaderContext, leader, leaderView, receipt) ==
  \/ AdequateLeaderAuthorityDeadlineTargetDecision(target, receipt)
  \/ AdequateLeaderAuthorityDeadlineLeaderPipelineCorridorExit(
       target, leaderContext, leader, leaderView, receipt)
  \/ \E qc \in QcRecordSet:
       AdequateLeaderAuthorityDeadlineLeaderDecisionSource(
         target, leaderContext, leader, leaderView, receipt, qc)

AdequateLeaderFixedPipelineStrictRankGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank) ==
  \/ AdequateLeaderFixedCutTerminalForAuthority(
       target, leaderContext, leader, leaderView, receipt)
  \/ \E lowerRank \in
       SetLessThan(
         sourceRank,
         AdequateLeaderFixedPipelineRankOrdering,
         AdequateLeaderFixedPipelineRankCarrier):
       \E token
            \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
          candidate \in AsyncCandidateSet,
          cutoffOrdinal \in Nat,
          semanticRank \in (1..4) \X (0..9),
          owner
            \in AdequateLeaderFixedSelectedServiceOwnerSet(
                 initialContext),
          packet \in AsyncPacketSet:
         /\ AdequateLeaderFixedPipelineRankCell(
              initialContext, target, leaderContext,
              leader, leaderView, receipt,
              token, candidate, cutoffOrdinal, semanticRank,
              owner, packet)
         /\ AdequateLeaderFixedPipelineRank(
              leaderContext, leader, leaderView, receipt.subject,
              token, candidate, cutoffOrdinal,
              semanticRank, owner, packet)
              = lowerRank

AdequateLeaderFixedPipelineSameRankFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank) ==
  \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
     candidate \in AsyncCandidateSet,
     cutoffOrdinal \in Nat,
     semanticRank \in (1..4) \X (0..9),
     owner
       \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
     packet \in AsyncPacketSet:
    /\ AdequateLeaderFixedPipelineRankCell(
         initialContext, target, leaderContext,
         leader, leaderView, receipt,
         token, candidate, cutoffOrdinal, semanticRank,
         owner, packet)
    /\ AdequateLeaderFixedPipelineRank(
         leaderContext, leader, leaderView, receipt.subject,
         token, candidate, cutoffOrdinal,
         semanticRank, owner, packet)
         = sourceRank

\* Readiness is tied to the exact desired next rank/ceiling outcome.  Merely
\* naming a RunNode, I/O, or producer action is not sufficient.  At positive
\* slack the exact selected action is Tick; at zero slack a concrete due
\* owner must be enabled and consume the cell.
AdequateLeaderFixedScheduledOwnerReadyForRankCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    token, candidate, cutoffOrdinal, semanticRank,
    owner, packet, sourceRank) ==
  /\ AdequateLeaderFixedPipelineRankCell(
       initialContext, target, leaderContext,
       leader, leaderView, receipt,
       token, candidate, cutoffOrdinal,
       semanticRank, owner, packet)
  /\ AdequateLeaderFixedPipelineRank(
       leaderContext, leader, leaderView, receipt.subject,
       token, candidate, cutoffOrdinal,
       semanticRank, owner, packet)
       = sourceRank
  /\ ENABLED
       (AdequateLeaderFixedSelectedServiceOwnerAction(owner)
          /\ AdequateLeaderFixedPipelineStrictRankGoal(
               initialContext, target, leaderContext,
               leader, leaderView, receipt, sourceRank)')

AdequateLeaderFixedSelectedPipelineRankFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    token, candidate, cutoffOrdinal, semanticRank,
    owner, packet, sourceRank) ==
  /\ AdequateLeaderFixedScheduledOwnerReadyForRankCell(
       initialContext, target, leaderContext, leader, leaderView, receipt,
       token, candidate, cutoffOrdinal, semanticRank,
       owner, packet, sourceRank)
  /\ IF AdequateLeaderFixedCandidateSelectedServiceSlack(
          owner, packet, candidate) > 0
     THEN owner.ownerKind = "Tick"
     ELSE owner.ownerKind # "Tick"

\* The semantic three-way split is inherited from
\* `AdequateLeaderTargetNonDescentEpisodeAction`.  This deadline layer only
\* projects its two non-progress arms onto exact durable origin-slot history.
\* A newly active slot must also be newly discovered; otherwise it is a
\* forbidden resurrection of a retired logical request.
AdequateLeaderFixedPipelineOriginSlotsPreservedAction(
    token, leaderContext, leader, leaderView, subject) ==
  (AdequateLeaderFixedLivePipelineOriginSlotsForToken(
     token, leaderContext, leader, leaderView, subject))'
    =
  AdequateLeaderFixedLivePipelineOriginSlotsForToken(
    token, leaderContext, leader, leaderView, subject)

AdequateLeaderFixedPipelineOriginEqualCountReplacementAction(
    target, token, leaderContext, leader, leaderView,
    subject, semanticRank) ==
  LET beforeActive ==
        AdequateLeaderFixedLivePipelineOriginsForToken(
          token, leaderContext, leader, leaderView, subject)
      afterActive ==
        (AdequateLeaderFixedLivePipelineOriginsForToken(
           token, leaderContext, leader, leaderView, subject))'
      beforeDiscovered ==
        AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
          token, leaderContext, leader, leaderView, subject)
      afterDiscovered ==
        (AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
           token, leaderContext, leader, leaderView, subject))'
  IN /\ AdequateLeaderTargetEqualCountOwnerReplacementAction(
           target, leaderContext, leader, leaderView,
           subject, semanticRank)
     /\ afterDiscovered \ beforeDiscovered # {}
     /\ afterActive \ beforeActive
          \subseteq afterDiscovered \ beforeDiscovered

AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction(
    target, token, leaderContext, leader, leaderView,
    subject, semanticRank) ==
  LET beforeActive ==
        AdequateLeaderFixedLivePipelineOriginsForToken(
          token, leaderContext, leader, leaderView, subject)
      afterActive ==
        (AdequateLeaderFixedLivePipelineOriginsForToken(
           token, leaderContext, leader, leaderView, subject))'
      beforeDiscovered ==
        AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
          token, leaderContext, leader, leaderView, subject)
      afterDiscovered ==
        (AdequateLeaderFixedDiscoveredPipelineOriginsForToken(
           token, leaderContext, leader, leaderView, subject))'
  IN /\ AdequateLeaderTargetCountIncreasingReplenishmentAction(
           target, leaderContext, leader, leaderView,
           subject, semanticRank)
     /\ afterDiscovered \ beforeDiscovered # {}
     /\ afterActive \ beforeActive
          \subseteq afterDiscovered \ beforeDiscovered

AdequateLeaderFixedPipelineOriginNonDescentEpisodeAction(
    target, token, leaderContext, leader, leaderView,
    subject, semanticRank) ==
  /\ AdequateLeaderTargetNonDescentEpisodeAction(
       target, leaderContext, leader, leaderView,
       subject, semanticRank)
  /\ \/ AdequateLeaderFixedPipelineOriginEqualCountReplacementAction(
          target, token, leaderContext, leader, leaderView,
          subject, semanticRank)
     \/ AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction(
          target, token, leaderContext, leader, leaderView,
          subject, semanticRank)

\* Compatibility projection for the former Dormant-predecessor coordinate.
\* The split-ordinal transition makes this set empty: a Dormant identity owns
\* no physical carrier, and exact replay reserves a fresh carrier after the
\* source cutoff before it becomes active.  Retained lifecycle identities are
\* already charged by the finite semantic owner universe below.
AdequateLeaderFixedPipelineDormantPotentialDiscoveredIdentitySet(
    episodeTarget, sourceCutoffOrdinal,
    sourceDormantPotential, knownDormantPotential) ==
  {}

AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier ==
  Nat

AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering ==
  OpToRel(<, Nat)

THEOREM AdequateLeaderFixedPipelineOriginEpisodeBudgetOrderingIsWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering,
    AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier)
BY NatLessThanWellFounded
   DEF AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering,
       AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier

\* This fixed-token episode follows the selected lifecycle, not the globally
\* least semantic occurrence.  An unrelated lower occurrence may already be
\* live at another node/token; it neither ends nor replenishes this episode.
\* Only disappearance of the selected same-or-higher occurrence into the
\* exact producer/transport corridor changes the carrier arm.  The route arm
\* below still binds the immutable token and lifecycle cut.
AdequateLeaderFixedSelectedOccurrenceProducerResidual(
    episodeTarget, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank) ==
  /\ sourceOccurrenceRank
       \in AdequateLeaderTargetOccurrenceRankCarrier
  /\ ~AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
       episodeTarget, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetProducerTransportResidual(
       episodeTarget, leaderContext, leader, leaderView, subject)

AdequateLeaderFixedSelectedOccurrenceLifecycleActive(
    episodeTarget, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank) ==
  \/ AdequateLeaderTargetSameOrHigherOccurrenceFrontier(
       episodeTarget, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  \/ AdequateLeaderFixedSelectedOccurrenceProducerResidual(
       episodeTarget, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)

AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget(
    episodeTarget, leaderContext, leader, leaderView, subject,
    sourceOccurrenceRank, sourceCutoffOrdinal,
    sourceDormantPotential, knownDormantPotential,
    known, budget) ==
  /\ sourceCutoffOrdinal \in Nat \ {0}
  /\ sourceDormantPotential = {}
  /\ knownDormantPotential = {}
  /\ budget \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier
  /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
       episodeTarget, leaderContext, leader, leaderView, subject, known)
  /\ AdequateLeaderFixedSelectedOccurrenceLifecycleActive(
       episodeTarget, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank)
  /\ AdequateLeaderTargetLiveOwnerIdentitySet(
       episodeTarget, leaderContext, leader, leaderView, subject)
       \subseteq known
  /\ budget =
       AdequateLeaderTargetNonDescentEpisodeBudget(
         episodeTarget, leaderContext, leader, leaderView,
         subject, known)

AdequateLeaderFixedAnyPipelineTokenCarrier ==
  UNION
    {AdequateLeaderFixedPipelineTokenCarrier(tokenContext):
       tokenContext \in ContextRecords}

AdequateLeaderFixedPreCandidateRouteIdentityCarrier ==
  UNION
    {{<<"Local", routeContext, node, view, subject, token>>:
        node \in ValidatorIds,
        view \in Views,
        subject \in Subjects,
        token \in AdequateLeaderFixedPipelineTokenCarrier(routeContext)}:
       routeContext \in ContextRecords}
    \cup
  {AsyncLeaderWireServiceIdentity(item):
     item \in AsyncNetworkItems}
    \cup
  {record.identity:
     record \in AsyncCandidateProducerContinuationRecordSet}

AdequateLeaderFixedPreCandidateRouteTyped(route, leaderContext) ==
  /\ DOMAIN route =
       {"kind", "token", "identity", "node", "ordinal", "predecessors"}
  /\ route.kind \in {"Local", "Wire", "Producer"}
  /\ route.token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
  /\ route.identity
       \in AdequateLeaderFixedPreCandidateRouteIdentityCarrier
  /\ route.node \in ValidatorIds
  /\ route.ordinal \in Nat \ {0}
  /\ route.predecessors \in SUBSET AsyncCandidateCausalOriginSet

(***************************************************************************
Charged pre-candidate entry.

A fresh synchronized source does not imply `CandidateScheduled`.  Before the
first BeginProposal, and again across a producer/transport handoff, the exact
owner may be a producer-continuation reservation, an active leader wire, or
the serialized scheduler which will expose the candidate.  `entryDebt` is an
ordinary action-debt coordinate inside
`AdequateLeaderFixedProtocolTokenCharge`; it allocates no fifth token and no
extra clock window.

The coordinate is not an existential credit.  It is the deterministic
remaining pre-candidate route of the exact selected owner.  A positive-slack
Tick retains that owner's route and lowers only slack; at zero slack the
concrete reservation/admission/producer/runner transition must expose a
strictly smaller route or a scheduled candidate.  In particular, a proof may
not manufacture descent in the same state by choosing a smaller `entryDebt`.
***************************************************************************)

AdequateLeaderFixedEntryServiceDeadline(owner, packet, leader) ==
  CASE owner.ownerKind = "TickPacket" -> packet.deadline
    [] owner.ownerKind = "Tick" ->
         asyncNodeServiceDeadlines[owner.node]
    [] owner.ownerKind = "ServiceIo" ->
         asyncIoServiceDeadlines[owner.node]
    [] owner.ownerKind = "Retire" -> asyncNow
    [] owner.ownerKind = "Admit" -> packet.deadline
    [] OTHER -> asyncNodeServiceDeadlines[owner.node]

AdequateLeaderFixedEntryServiceSlack(owner, packet, leader) ==
  LET serviceDeadline ==
        AdequateLeaderFixedEntryServiceDeadline(owner, packet, leader)
  IN IF asyncNow < serviceDeadline
     THEN serviceDeadline - asyncNow
     ELSE 0

\* Exact state-derived remaining route actions.  Tick is an alias for the
\* same stage: reaching its deadline does not consume an action token, while
\* the concrete due action does.  Producer continuation status is durable
\* and distinguishes Reserved from Materialized/Terminal without a freely
\* chosen proof ordinal.
AdequateLeaderFixedPreCandidateRouteActionDebt(route, owner) ==
  CASE owner.ownerKind = "TickPacket" -> 3
    [] owner.ownerKind = "Tick" ->
         IF route.kind = "Producer"
         THEN 1
                + AsyncCandidateProducerContinuationStatusRank(
                    (AsyncCandidateProducerContinuationSelectedResolutionRecord(
                       route.node)).status)
         ELSE 2
    [] owner.ownerKind = "Admit" -> 3
    [] owner.ownerKind
         \in {"ResolveLocalProducer",
              "ServiceConditionalProducer",
              "ServiceVolatileProducer"} ->
         1
           + AsyncCandidateProducerContinuationStatusRank(
               (AsyncCandidateProducerContinuationSelectedResolutionRecord(
                  owner.node)).status)
    [] owner.ownerKind \in {"RunNode", "ServiceIo"} -> 2
    [] owner.ownerKind = "Retire" -> 1
    [] OTHER -> 1

\* A pre-deadline Tick and the zero-slack producer continuation are two fair
\* actions for one immutable logical pre-owner.  Both recover the same
\* continuation record and hence the same protocol token; Tick does not fall
\* back to the initial Proposal token during a later producer handoff.
AdequateLeaderFixedSelectedProducerContinuationOwnsToken(
    node, leaderContext, leader, leaderView, subject, token) ==
  LET record ==
        AsyncCandidateProducerContinuationSelectedResolutionRecord(node)
  IN /\ node
          \in AdequateLeaderFrozenResponsiveRoster(leaderContext)
     /\ \/ AsyncCandidateProducerContinuationSelectedSourceClass(
              node, "Local")
        \/ AsyncCandidateProducerContinuationSelectedSourceClass(
              node, "ConditionalTransport")
        \/ AsyncCandidateProducerContinuationSelectedSourceClass(
              node, "VolatileBody")
     /\ record.context = leaderContext
     /\ record.height = leaderContext.height
     /\ record.leader = leader
     /\ record.view = leaderView
     /\ record.subject = subject
     /\ token =
          AdequateLeaderFixedOriginProtocolToken(record.causalOrigin)

AdequateLeaderFixedPreCandidateOwnerIsExact(
    initialContext, target, leaderContext, leader, leaderView,
    subject, owner, packet) ==
  /\ owner \in
       AdequateLeaderFixedSelectedServiceOwnerSet(initialContext)
  /\ CASE owner.ownerKind = "Tick" ->
            /\ owner.node
                 \in AdequateLeaderFrozenResponsiveRoster(leaderContext)
            /\ owner.source = AsyncUntrustedSource
            /\ AdequateLeaderFrozenTargetCorridor(
                 target, leaderContext, leader, leaderView)
       [] owner.ownerKind = "TickPacket" ->
            /\ packet \in asyncTransport
            /\ packet.item.envelope.recipient = owner.node
            /\ packet.item.source = owner.source
            /\ AdequateLeaderTargetWireIdentity(
                 packet.item, target, leaderContext,
                 leader, leaderView, subject)
       [] owner.ownerKind = "Admit" ->
            /\ packet \in OverdueResponsivePackets
            /\ packet = ExactDecisionTargetNeutralSelectedOverduePacket
            /\ packet.item.envelope.recipient = owner.node
            /\ packet.item.source = owner.source
            /\ AdequateLeaderTargetWireIdentity(
                 packet.item, target, leaderContext,
                 leader, leaderView, subject)
            /\ ENABLED
                 PostGstAdmitHiddenPacket(owner.node, owner.source)
       [] owner.ownerKind = "ResolveLocalProducer" ->
            AsyncCandidateProducerContinuationSelectedSourceClass(
              owner.node, "Local")
       [] owner.ownerKind = "ServiceConditionalProducer" ->
            AsyncCandidateProducerContinuationSelectedSourceClass(
              owner.node, "ConditionalTransport")
       [] owner.ownerKind = "ServiceVolatileProducer" ->
            AsyncCandidateProducerContinuationSelectedSourceClass(
              owner.node, "VolatileBody")
       [] owner.ownerKind = "Retire" ->
            /\ owner.slot \in AsyncLeaderWireLifecycleSlotSet
            /\ \E record
                 \in AsyncLeaderWireLifecycleRecordsForSlot(owner.slot):
                 /\ AsyncLeaderWireLifecycleCanTerminal(record)
                 /\ record.recipient \in up
                 /\ AdequateLeaderTargetWireIdentity(
                      record.item, target, leaderContext,
                      leader, leaderView, subject)
       [] owner.ownerKind \in {"RunNode", "ServiceIo"} ->
            /\ owner.node
                 \in AdequateLeaderFrozenResponsiveRoster(leaderContext)
            /\ owner.source = AsyncUntrustedSource
       [] OTHER -> FALSE

AdequateLeaderFixedWireItemProtocolPhase(item) ==
  CASE item.kind \in {"Proposal", "Chunk"} -> "Proposal"
    [] item.kind = "PrepareVote" -> "Prepare"
    [] item.kind \in {"PrepareQC", "CommitVote"} -> "Commit"
    [] item.kind = "CommitQC" -> "Decision"
    [] OTHER -> "NoPipelinePhase"

AdequateLeaderFixedWireItemProtocolToken(item) ==
  <<IF item.kind \in {"PrepareVote", "CommitVote"}
      THEN item.source
      ELSE item.envelope.recipient,
    AdequateLeaderFixedWireItemProtocolPhase(item)>>

\* Owners whose physical action has no packet argument carry this one
\* constant proof-only value.  It is selected from the constant packet
\* carrier, so it cannot change as queues or deadlines change and cannot be
\* used to reselect a different route stage.
AdequateLeaderFixedProofOnlyPacket ==
  CHOOSE packet \in AsyncPacketSet: TRUE

\* Route identity is immutable across Tick, reservation, ingress, runner, and
\* durable producer-continuation stages.  The rank carries this value as a
\* parameter; the current fair owner is derived from state below.  Thus Tick
\* is only the waiting action for the same route, never an alternate proof
\* owner with a freely selectable debt.
AdequateLeaderFixedPreCandidateRoute(
    kind, token, identity, node, ordinal, predecessors) ==
  [kind |-> kind,
   token |-> token,
   identity |-> identity,
   node |-> node,
   ordinal |-> ordinal,
   predecessors |-> predecessors]

AdequateLeaderFixedLocalPreCandidateRoute(
    token, leaderContext, leader, leaderView, subject) ==
  LET ordinal == AsyncNextCandidateLifecycleOrdinal(leader)
      latent ==
        NoItemCandidate(
          "Normal", "AssembleBody", leader, leaderView, subject)
  IN AdequateLeaderFixedPreCandidateRoute(
       "Local", token,
       <<"Local", leaderContext, leader, leaderView, subject, token>>,
       leader, ordinal,
       AsyncCausalEpisodeFrozenPredecessorOrigins(leader, ordinal)
         \cup {latent.causalOrigin})

AdequateLeaderFixedWirePreCandidateRouteOrdinal(item) ==
  IF /\ AsyncLeaderWireLifecycleSlotOwned(item)
     /\ AsyncLeaderWireLifecycleIdentityMatches(
          item, AsyncLeaderWireLifecycleRecordForItem(item))
  THEN (AsyncLeaderWireLifecycleRecordForItem(item)).schedulerOrdinal
  ELSE AsyncNextCandidateLifecycleOrdinal(item.envelope.recipient)

AdequateLeaderFixedWirePreCandidateRoute(item, leaderContext) ==
  LET node == item.envelope.recipient
      ordinal == AdequateLeaderFixedWirePreCandidateRouteOrdinal(item)
      latent == AsyncLeaderWireRuntimeCandidate(item)
  IN AdequateLeaderFixedPreCandidateRoute(
       "Wire", AdequateLeaderFixedWireItemProtocolToken(item),
       AsyncLeaderWireServiceIdentity(item), node, ordinal,
       AsyncCausalEpisodeFrozenPredecessorOrigins(node, ordinal)
         \cup {latent.causalOrigin})

\* Exact retransmissions share the logical wire identity but can leave packet
\* records with different send/deadline times.  The oldest occurrence is the
\* unique clock owner for that route.  Later duplicates cannot replace it or
\* buy a later deadline; equal item/time triples are the same set value.
AdequateLeaderFixedWirePacketsForPreCandidateRoute(route) ==
  {packet \in asyncTransport:
     AsyncLeaderWireServiceIdentity(packet.item) = route.identity}

AdequateLeaderFixedWirePacketOccurrencePrecedes(left, right) ==
  \/ left.deadline < right.deadline
  \/ /\ left.deadline = right.deadline
     /\ left.sentAt <= right.sentAt

AdequateLeaderFixedSelectedWirePacketForPreCandidateRoute(route) ==
  LET packets ==
        AdequateLeaderFixedWirePacketsForPreCandidateRoute(route)
  IN CHOOSE packet \in packets:
       \A other \in packets:
         AdequateLeaderFixedWirePacketOccurrencePrecedes(packet, other)

AdequateLeaderFixedProducerPreCandidateRoute(record) ==
  AdequateLeaderFixedPreCandidateRoute(
    "Producer",
    AdequateLeaderFixedOriginProtocolToken(record.causalOrigin),
    record.identity, record.node, record.ordinal,
    AsyncCausalEpisodeFrozenPredecessorOrigins(
      record.node, record.ordinal)
      \cup {candidate.causalOrigin:
             candidate \in record.handoffCandidates})

AdequateLeaderFixedPreCandidateRouteOwnsCurrentCut(route) ==
  AsyncCausalEpisodeFrozenPredecessorOrigins(
    route.node, route.ordinal)
    \subseteq route.predecessors

AdequateLeaderFixedPreCandidateRouteMatchesLocal(
    route, token, leaderContext, leader, leaderView, subject) ==
  LET fresh ==
        AdequateLeaderFixedLocalPreCandidateRoute(
          token, leaderContext, leader, leaderView, subject)
  IN /\ route.kind = fresh.kind
     /\ route.token = fresh.token
     /\ route.identity = fresh.identity
     /\ route.node = fresh.node
     /\ route.ordinal = fresh.ordinal
     /\ AdequateLeaderFixedPreCandidateRouteOwnsCurrentCut(route)

AdequateLeaderFixedPreCandidateRouteMatchesWire(
    route, item, leaderContext) ==
  LET fresh ==
        AdequateLeaderFixedWirePreCandidateRoute(item, leaderContext)
  IN /\ route.kind = fresh.kind
     /\ route.token = fresh.token
     /\ route.identity = fresh.identity
     /\ route.node = fresh.node
     /\ route.ordinal = fresh.ordinal
     /\ AdequateLeaderFixedPreCandidateRouteOwnsCurrentCut(route)

AdequateLeaderFixedPreCandidateRouteMatchesProducer(route, record) ==
  LET fresh == AdequateLeaderFixedProducerPreCandidateRoute(record)
  IN /\ route.kind = fresh.kind
     /\ route.token = fresh.token
     /\ route.identity = fresh.identity
     /\ route.node = fresh.node
     /\ route.ordinal = fresh.ordinal
     /\ AdequateLeaderFixedPreCandidateRouteOwnsCurrentCut(route)

\* The route freezes the exact candidates which its final admission may
\* expose.  Credits for candidates already inside the frozen lifecycle cut
\* are counted by `AdequateLeaderFixedCutCumulativeActionDebt`; only the
\* not-yet-scheduled tail is reserved here.  Moving a candidate from this set
\* into the cut therefore preserves their sum instead of allocating a new
\* candidate episode.
AdequateLeaderFixedPreCandidateRouteLatentCandidates(
    route, leaderContext, leaderView, subject) ==
  CASE route.kind = "Local" ->
         {NoItemCandidate(
            "Normal", "AssembleBody", route.node, leaderView, subject)}
    [] route.kind = "Wire" ->
         {AsyncLeaderWireRuntimeCandidate(route.identity)}
    [] route.kind = "Producer" ->
         (AsyncCandidateProducerContinuationSelectedResolutionRecord(
            route.node)).handoffCandidates
    [] OTHER -> {}

AdequateLeaderFixedPreCandidateRouteExpectedSourceCut(
    route, leaderContext, leaderView, subject) ==
  AsyncCausalEpisodeFrozenPredecessorOrigins(
    route.node, route.ordinal)
    \cup
      {candidate.causalOrigin:
         candidate
           \in AdequateLeaderFixedPreCandidateRouteLatentCandidates(
                route, leaderContext, leaderView, subject)}

AdequateLeaderFixedPreCandidateRouteUnscheduledLatentCandidates(
    route, leaderContext, leaderView, subject) ==
  AdequateLeaderFixedPreCandidateRouteLatentCandidates(
    route, leaderContext, leaderView, subject)
    \ AsyncCausalEpisodeCandidates(route.node, route.ordinal)

AdequateLeaderFixedPreCandidateRouteLatentActionTokens(
    route, leaderContext, leaderView, subject) ==
  UNION
    {{<<candidate, actionToken>>:
        actionToken
          \in 1..AdequateLeaderFixedExactCandidateActionCredit(
                   candidate.class, candidate.kind)}:
       candidate
         \in AdequateLeaderFixedPreCandidateRouteUnscheduledLatentCandidates(
              route, leaderContext, leaderView, subject)}

AdequateLeaderFixedPreCandidateRouteLatentActionDebt(
    route, leaderContext, leaderView, subject) ==
  Cardinality(
    AdequateLeaderFixedPreCandidateRouteLatentActionTokens(
      route, leaderContext, leaderView, subject))

\* The configured physical-window ceiling is reserved before materialization.
\* A newly scheduled candidate consequently replaces this reserve with its
\* actual (no larger) physical rank.  The final +1 is the same slot handoff
\* unit used by the scheduled-cell rank; `routeActionDebt` makes the
\* materializing transition strict even when the actual physical rank reaches
\* its ceiling.
AdequateLeaderFixedConcretePreCandidateEntryDebt(
    route, leaderContext, leaderView, subject, owner) ==
  AdequateLeaderFixedCutCumulativeActionDebt(
    route.node, route.ordinal)
    + AdequateLeaderFixedPreCandidateRouteLatentActionDebt(
        route, leaderContext, leaderView, subject)
    + AdequateLeaderFixedCandidatePhysicalWindowBudget
    + 1
    + AdequateLeaderFixedPreCandidateRouteActionDebt(route, owner)

THEOREM AdequateLeaderFixedPreCandidateReservedTailFitsConfiguredCharge ==
  \A initialContext, leaderContext \in ContextRecords,
     leaderView \in Views, subject \in Subjects:
    \A route:
      \A owner
           \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
        /\ ModelConfiguration
        /\ AsyncStrongTypeInvariant
        /\ AsyncProgressOwnershipInvariant
        /\ AdequateLeaderFixedPreCandidateRouteTyped(route, leaderContext)
        /\ AdequateLeaderFixedCutCumulativeActionDebt(
             route.node, route.ordinal)
             + AdequateLeaderFixedPreCandidateRouteLatentActionDebt(
                 route, leaderContext, leaderView, subject)
             <= AsyncCandidateProducerActionEpisodeBudget
        /\ AdequateLeaderFixedPreCandidateRouteActionDebt(route, owner)
             \in 1..4
          => AdequateLeaderFixedConcretePreCandidateEntryDebt(
               route, leaderContext, leaderView, subject, owner)
               \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge
BY AdequateLeaderFixedCandidatePhysicalWindowFitsConfiguredBudget, SMT
   DEF AdequateLeaderFixedConcretePreCandidateEntryDebt,
       AdequateLeaderFixedPerOriginSlotEpisodeCharge,
       AdequateLeaderFixedCandidateCharge,
       AdequateLeaderFixedCandidatePhysicalWindowBudget,
       AdequateLeaderFixedRunnerPrefixCharge,
       AdequateLeaderFixedDeferredCursorCharge,
       AdequateLeaderFixedIoCarrierCharge,
       AdequateLeaderFixedIoSelectorCharge,
       ModelConfiguration, AsyncConfiguration

AdequateLeaderFixedCandidateContinuesPreCandidateRoute(
    candidate, route, leaderContext, leader, leaderView, subject) ==
  /\ AdequateLeaderFixedPreCandidateRouteTyped(route, leaderContext)
  /\ candidate.consumerContext = leaderContext
  /\ candidate.height = leaderContext.height
  /\ candidate.view = leaderView
  /\ candidate.subject = subject
  /\ AdequateLeaderFixedOriginProtocolToken(candidate.causalOrigin)
       = route.token
  /\ CASE route.kind = "Local" ->
            /\ AdequateLeaderFixedPreCandidateRouteMatchesLocal(
                 route, route.token,
                 leaderContext, leader, leaderView, subject)
            /\ AdequateLeaderFixedOriginRootItem(candidate.causalOrigin)
                 = NoAsyncItem
       [] route.kind = "Wire" ->
            LET root ==
                  AdequateLeaderFixedOriginRootItem(
                    candidate.causalOrigin)
            IN /\ root \in AsyncNetworkItems
               /\ candidate.node = route.node
               /\ AdequateLeaderFixedPreCandidateRouteMatchesWire(
                    route, root, leaderContext)
               /\ AsyncCandidateLifecycleOrdinal(candidate) = route.ordinal
       [] route.kind = "Producer" ->
            /\ candidate.node = route.node
            /\ candidate.node = route.identity.target
            /\ candidate.causalOrigin =
                 route.causalOrigin
            /\ AsyncCandidateLifecycleOrdinal(candidate) = route.ordinal
            /\ AdequateLeaderFixedPreCandidateRouteOwnsCurrentCut(route)
       [] OTHER -> FALSE

\* RunNode and ServiceIo may expose several queued records at their node, but
\* only the deterministic selected action may name this pre-candidate token.
\* The post-action handoff is tied to the same immutable token and exact
\* target/authority identity; an unrelated queued candidate cannot witness it.
AdequateLeaderFixedDeterministicHeadOrGateToken(
    initialContext, target, leaderContext, leader, leaderView, subject,
    token, owner) ==
  /\ owner.ownerKind \in {"RunNode", "ServiceIo"}
  /\ owner
       \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext)
  /\ ENABLED
       (AdequateLeaderFixedSelectedServiceOwnerAction(owner)
          /\ (\/ NodeHasDecision(target)'
              \/ ~(AdequateLeaderFrozenTargetCorridor(
                     target, leaderContext, leader, leaderView))'
              \/ \E candidate \in AsyncCandidateSet,
                    cutoffOrdinal \in Nat,
                    semanticRank \in (1..4) \X (0..9):
                   /\ (AdequateLeaderFixedCandidateOwnsPipelineToken(
                         candidate, token, leaderContext, leader, leaderView,
                         subject,
                         cutoffOrdinal, semanticRank))'
                   /\ ((CandidateScheduled(candidate)
                          \/ CandidateInFlight(candidate))')))

\* The pre-candidate rank may reserve only the token named by the concrete
\* scheduler, wire, or producer-continuation owner.  This prevents an
\* existential entry witness from borrowing a cheaper unrelated token.
AdequateLeaderFixedPreCandidateTokenIsExact(
    initialContext, target, leaderContext, leader, leaderView, subject,
    token, owner, packet) ==
  /\ token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
  /\ CASE owner.ownerKind = "Tick" ->
            \/ /\ owner.node = leader
                  /\ token = <<leader, "Proposal">>
               \/ AdequateLeaderFixedSelectedProducerContinuationOwnsToken(
                    owner.node, leaderContext, leader, leaderView,
                    subject, token)
       [] owner.ownerKind = "TickPacket" ->
            /\ AdequateLeaderTargetWireIdentity(
                 packet.item, target, leaderContext,
                 leader, leaderView, subject)
            /\ token =
                 AdequateLeaderFixedWireItemProtocolToken(packet.item)
       [] owner.ownerKind = "Admit" ->
            /\ AdequateLeaderTargetWireIdentity(
                 packet.item, target, leaderContext,
                 leader, leaderView, subject)
            /\ token =
                 AdequateLeaderFixedWireItemProtocolToken(packet.item)
       [] owner.ownerKind
            \in {"ResolveLocalProducer",
                 "ServiceConditionalProducer",
                 "ServiceVolatileProducer"} ->
            LET record ==
                  AsyncCandidateProducerContinuationSelectedResolutionRecord(
                    owner.node)
            IN /\ record.context = leaderContext
               /\ record.height = leaderContext.height
               /\ record.view = leaderView
               /\ record.subject = subject
               /\ token =
                    AdequateLeaderFixedOriginProtocolToken(
                      record.causalOrigin)
       [] owner.ownerKind = "Retire" ->
            \E record
              \in AsyncLeaderWireLifecycleRecordsForSlot(owner.slot):
              /\ AdequateLeaderTargetWireIdentity(
                   record.item, target, leaderContext,
                   leader, leaderView, subject)
              /\ token =
                   AdequateLeaderFixedWireItemProtocolToken(record.item)
       [] owner.ownerKind \in {"RunNode", "ServiceIo"} ->
            AdequateLeaderFixedDeterministicHeadOrGateToken(
              initialContext,
              target, leaderContext, leader, leaderView, subject,
              token, owner)
       [] OTHER -> FALSE

AdequateLeaderFixedPreCandidateRouteStageCandidate(
    initialContext, target, leaderContext, leader, leaderView, subject,
    route, token, owner, packet) ==
  /\ AdequateLeaderFixedPreCandidateRouteTyped(route, leaderContext)
  /\ route.token = token
  /\ AdequateLeaderFixedPreCandidateOwnerIsExact(
       initialContext, target, leaderContext, leader, leaderView,
       subject, owner, packet)
  /\ AdequateLeaderFixedPreCandidateTokenIsExact(
       initialContext, target, leaderContext, leader, leaderView,
       subject, token, owner, packet)
  /\ IF AdequateLeaderFixedEntryServiceSlack(
          owner, packet, leader) > 0
     THEN owner.ownerKind \in {"Tick", "TickPacket"}
     ELSE owner.ownerKind \notin {"Tick", "TickPacket"}
  /\ IF owner.ownerKind
          \in {"Tick", "ResolveLocalProducer",
               "ServiceConditionalProducer",
               "ServiceVolatileProducer",
               "RunNode", "ServiceIo", "Retire"}
     THEN packet = AdequateLeaderFixedProofOnlyPacket
     ELSE TRUE
  /\ CASE owner.ownerKind = "Tick" ->
            \/ /\ AdequateLeaderFixedPreCandidateRouteMatchesLocal(
                    route, token,
                    leaderContext, leader, leaderView, subject)
                  /\ owner.node = leader
               \/ LET record ==
                        AsyncCandidateProducerContinuationSelectedResolutionRecord(
                          owner.node)
                  IN /\ AdequateLeaderFixedSelectedProducerContinuationOwnsToken(
                           owner.node, leaderContext, leader, leaderView,
                           subject, token)
                     /\ AdequateLeaderFixedPreCandidateRouteMatchesProducer(
                          route, record)
       [] owner.ownerKind
            \in {"TickPacket", "Admit"} ->
            /\ AdequateLeaderFixedPreCandidateRouteMatchesWire(
                 route, packet.item, leaderContext)
            /\ IF owner.ownerKind = "TickPacket"
               THEN packet =
                      AdequateLeaderFixedSelectedWirePacketForPreCandidateRoute(
                        route)
               ELSE TRUE
       [] owner.ownerKind
            \in {"ResolveLocalProducer",
                 "ServiceConditionalProducer",
                 "ServiceVolatileProducer"} ->
            AdequateLeaderFixedPreCandidateRouteMatchesProducer(
              route,
              AsyncCandidateProducerContinuationSelectedResolutionRecord(
                owner.node))
       [] owner.ownerKind = "Retire" ->
            \E record
              \in AsyncLeaderWireLifecycleRecordsForSlot(owner.slot):
              /\ AdequateLeaderTargetWireIdentity(
                   record.item, target, leaderContext,
                   leader, leaderView, subject)
              /\ AdequateLeaderFixedPreCandidateRouteMatchesWire(
                   route, record.item, leaderContext)
       [] owner.ownerKind \in {"RunNode", "ServiceIo"} ->
            ENABLED
              (AdequateLeaderFixedSelectedServiceOwnerAction(owner)
                 /\ (\/ NodeHasDecision(target)'
                     \/ ~(AdequateLeaderFrozenTargetCorridor(
                            target, leaderContext,
                            leader, leaderView))'
                     \/ \E candidate \in AsyncCandidateSet:
                          (AdequateLeaderFixedCandidateContinuesPreCandidateRoute(
                             candidate, route, leaderContext,
                             leader, leaderView, subject))'
                            /\ ((CandidateScheduled(candidate)
                                   \/ CandidateInFlight(candidate))')))
       [] OTHER -> FALSE

AdequateLeaderFixedPreCandidateRouteStageSet(
    initialContext, target, leaderContext, leader, leaderView, subject,
    route, token) ==
  {stage
     \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext)
          \X AsyncPacketSet:
     AdequateLeaderFixedPreCandidateRouteStageCandidate(
       initialContext, target, leaderContext, leader, leaderView, subject,
       route, token, stage[1], stage[2])}

AdequateLeaderFixedPreCandidateRouteStageIsUnique(
    initialContext, target, leaderContext, leader, leaderView, subject,
    route, token, owner, packet) ==
  AdequateLeaderFixedPreCandidateRouteStageSet(
    initialContext, target, leaderContext, leader, leaderView, subject,
    route, token)
    = {<<owner, packet>>}

AdequateLeaderFixedPreCandidateReservedLiveSlotDebt(
    token, leaderContext, leader, leaderView, subject) ==
  LET liveSlotDebt ==
        Cardinality(
          AdequateLeaderFixedLivePipelineOriginSlotsForToken(
            token, leaderContext, leader, leaderView, subject))
  IN IF liveSlotDebt = 0 THEN 1 ELSE liveSlotDebt

AdequateLeaderFixedPreCandidateUnreservedUnknownBudget(
    token, leaderContext, leader, leaderView, subject) ==
  LET liveSlotDebt ==
        Cardinality(
          AdequateLeaderFixedLivePipelineOriginSlotsForToken(
            token, leaderContext, leader, leaderView, subject))
      unknownBudget ==
        AdequateLeaderFixedPipelineOriginUnknownBudget(
          token, leaderContext, leader, leaderView, subject)
  IN IF liveSlotDebt = 0
     THEN IF unknownBudget = 0 THEN 0 ELSE unknownBudget - 1
     ELSE unknownBudget

AdequateLeaderFixedPreCandidateEntryRank(
    token, leaderContext, leader, leaderView, subject,
    entryDebt, owner, packet) ==
  <<AdequateLeaderFixedAuthorityPipelineWindowsRemaining(
      leaderContext, leader, leaderView, subject),
    <<AdequateLeaderFixedPreCandidateReservedLiveSlotDebt(
        token, leaderContext, leader, leaderView, subject),
      <<entryDebt,
        AdequateLeaderFixedEntryServiceSlack(owner, packet, leader)>>>>>>

AdequateLeaderFixedPreCandidateEntryRankCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    route, token, entryDebt, owner, packet) ==
  LET subject == receipt.subject
      liveSlots ==
        AdequateLeaderFixedLivePipelineOriginSlotsForToken(
          token, leaderContext, leader, leaderView, subject)
      discoveredSlots ==
        AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken(
          token, leaderContext, leader, leaderView, subject)
      unknownBudget ==
        AdequateLeaderFixedPipelineOriginUnknownBudget(
          token, leaderContext, leader, leaderView, subject)
      unreservedUnknownBudget ==
        AdequateLeaderFixedPreCandidateUnreservedUnknownBudget(
          token, leaderContext, leader, leaderView, subject)
      reservedLiveSlotDebt ==
        AdequateLeaderFixedPreCandidateReservedLiveSlotDebt(
          token, leaderContext, leader, leaderView, subject)
      latentCandidates ==
        AdequateLeaderFixedPreCandidateRouteLatentCandidates(
          route, leaderContext, leaderView, subject)
      latentActionDebt ==
        AdequateLeaderFixedPreCandidateRouteLatentActionDebt(
          route, leaderContext, leaderView, subject)
      cutActionDebt ==
        AdequateLeaderFixedCutCumulativeActionDebt(
          route.node, route.ordinal)
      routeActionDebt ==
        AdequateLeaderFixedPreCandidateRouteActionDebt(route, owner)
      rank ==
        AdequateLeaderFixedPreCandidateEntryRank(
          token, leaderContext, leader, leaderView, subject,
          entryDebt, owner, packet)
  IN /\ AdequateLeaderAuthorityDeadlineFixedSubjectWindowActive(
           target, leaderContext, leader, leaderView, receipt)
     /\ AdequateLeaderFrozenTargetCorridor(
          target, leaderContext, leader, leaderView)
     /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
     /\ token
          \in AdequateLeaderFixedAuthorityPipelineRemainingTokens(
               leaderContext, leader, leaderView, subject)
     /\ AdequateLeaderFixedPreCandidateRouteTyped(route, leaderContext)
     /\ route.token = token
     /\ AdequateLeaderFixedPreCandidateRouteOwnsCurrentCut(route)
     /\ latentCandidates \subseteq AsyncCandidateSet
     /\ \A candidate \in latentCandidates:
          /\ candidate.node = route.node
          /\ candidate.consumerContext = leaderContext
          /\ candidate.height = leaderContext.height
          /\ candidate.view = leaderView
          /\ candidate.subject = subject
     /\ cutActionDebt + latentActionDebt
          <= AsyncCandidateProducerActionEpisodeBudget
     /\ routeActionDebt \in 1..4
     /\ AdequateLeaderFixedPreCandidateRouteStageIsUnique(
          initialContext, target, leaderContext, leader, leaderView, subject,
          route, token, owner, packet)
     /\ \/ owner.ownerKind
              \in {"Tick", "TickPacket",
                   "RunNode", "ServiceIo", "Retire"}
           \/ /\ AdequateLeaderTargetProductiveSubjectOpenFrontier(
                   target, leaderContext, leader, leaderView, subject)
              /\ AdequateLeaderTargetProducerResidual(
                   target, leaderContext, leader, leaderView, subject)
              /\ AdequateLeaderTargetConcreteProducerTransportOwner(
                   target, leaderContext, leader, leaderView, subject)
     /\ liveSlots \subseteq discoveredSlots
     /\ discoveredSlots
          \subseteq
            AdequateLeaderFixedPipelineOriginSlotCarrier(token[2])
     /\ unknownBudget \in Nat
     /\ (liveSlots = {} => unknownBudget > 0)
     /\ unreservedUnknownBudget + reservedLiveSlotDebt
          \in 1..AdequateLeaderFixedOriginSlotCapacity
     /\ entryDebt =
          AdequateLeaderFixedConcretePreCandidateEntryDebt(
            route, leaderContext, leaderView, subject, owner)
     /\ entryDebt \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge
     /\ AdequateLeaderFixedPreCandidateOwnerIsExact(
          initialContext, target, leaderContext, leader, leaderView,
          subject, owner, packet)
     /\ AdequateLeaderFixedPreCandidateTokenIsExact(
          initialContext, target, leaderContext, leader, leaderView,
          subject, token, owner, packet)
     /\ AdequateLeaderFixedEntryServiceSlack(owner, packet, leader)
          \in 0..AsyncDeliveryBound
     /\ rank \in AdequateLeaderFixedPipelineRankCarrier
     /\ AdequateLeaderFixedPipelineAbsoluteCeilingAtUnknownBudget(
          receipt, unreservedUnknownBudget, rank)

\* A serviced parent may leave an immutable producer continuation before its
\* child candidate is scheduled.  That gap is part of the same occurrence
\* episode, not a terminal and not a new physical window.  The exact
\* pre-candidate cell below retains the protocol token, continuation/wire
\* owner, lifecycle cut, ordinal debt, and absolute receipt ceiling at the
\* unchanged source rank.  A genuinely lower pre-candidate rank is already a
\* strict rank goal; a higher one is forbidden as physical replenishment.
AdequateLeaderFixedPipelineProducerHandoffFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    route, token, episodeTarget, sourceOccurrenceRank, sourceOccurrenceOwner,
    sourceCutoffOrdinal, sourceDormantPotential, knownDormantPotential,
    known, sourceRank, budget) ==
  /\ AdequateLeaderFixedPreCandidateRouteTyped(route, leaderContext)
  /\ token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
  /\ route.token = token
  /\ token[1] = episodeTarget
  /\ known
       \in SUBSET AdequateLeaderFrozenOwnerUniverse(
            episodeTarget, leaderContext, leader, leaderView,
            receipt.subject)
  /\ AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget(
       episodeTarget, leaderContext, leader, leaderView, receipt.subject,
       sourceOccurrenceRank, sourceCutoffOrdinal,
       sourceDormantPotential, knownDormantPotential,
       known, budget)
  /\ AdequateLeaderTargetOccurrenceOwnerCarried(
       episodeTarget, leaderContext, leader, leaderView,
       receipt.subject,
       sourceOccurrenceRank, known, sourceOccurrenceOwner)
  /\ AdequateLeaderFixedSelectedOccurrenceProducerResidual(
       episodeTarget, leaderContext, leader, leaderView,
       receipt.subject, sourceOccurrenceRank)
  /\ ~AdequateLeaderFixedPipelineStrictRankGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank)
  /\ \E entryDebt
          \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
        owner
          \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
        packet \in AsyncPacketSet:
       /\ AdequateLeaderFixedPreCandidateEntryRankCell(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            route, token, entryDebt, owner, packet)
       /\ AdequateLeaderFixedPreCandidateEntryRank(
            token, leaderContext, leader, leaderView, receipt.subject,
            entryDebt, owner, packet)
            = sourceRank

AdequateLeaderFixedCutPerActionProvider ==
  \A initialContext \in ContextRecords:
   \A target \in ValidatorIds:
    \A leaderContext \in ContextRecords:
     \A leader \in ValidatorIds:
      \A leaderView \in Views:
       \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
        \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
         \A candidate \in AsyncCandidateSet:
          \A cutoffOrdinal \in Nat:
           \A semanticRank \in (1..4) \X (0..9):
            \A owner
                 \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
             \A packet \in AsyncPacketSet:
              \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
       /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet, sourceRank)
       /\ [AsyncNext]_AsyncAllVars
       => \/ AdequateLeaderFixedPipelineStrictRankGoal(
               initialContext, target, leaderContext,
               leader, leaderView,
               receipt, sourceRank)'
          \/ /\ (AdequateLeaderFixedSelectedPipelineRankFrontier(
                    initialContext, target, leaderContext,
                    leader, leaderView, receipt,
                    token, candidate, cutoffOrdinal, semanticRank,
                    owner, packet, sourceRank))'
               /\ AdequateLeaderFixedPipelineOriginSlotsPreservedAction(
                    token, leaderContext, leader, leaderView,
                    receipt.subject)
               /\ ~<<AdequateLeaderFixedSelectedServiceOwnerAction(
                         owner)>>_AsyncAllVars
          \/ AdequateLeaderFixedPipelineOriginEqualCountReplacementAction(
               candidate.node, token, leaderContext, leader, leaderView,
               receipt.subject, semanticRank)
          \/ AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction(
               candidate.node, token, leaderContext, leader, leaderView,
               receipt.subject, semanticRank)
          \/ (\E route:
                \E occurrenceRank
                    \in AdequateLeaderTargetOccurrenceRankCarrier,
                  occurrenceOwner
                    \in AdequateLeaderFrozenCandidateOwnerUniverse(
                         candidate.node, leaderContext, leader, leaderView,
                         receipt.subject),
                  budget
                    \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
                  \E sourceDormantPotential, knownDormantPotential:
                    /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
                         candidate, leaderContext, leader, leaderView,
                         receipt.subject, semanticRank,
                         occurrenceRank, occurrenceOwner)
                    /\ sourceDormantPotential = {}
                    /\ knownDormantPotential = {}
                    /\ AdequateLeaderFixedPipelineProducerHandoffFrontier(
                         initialContext, target, leaderContext,
                         leader, leaderView, receipt,
                         route, token, candidate.node,
                         occurrenceRank, occurrenceOwner,
                         cutoffOrdinal,
                         sourceDormantPotential, knownDormantPotential,
                         AdequateLeaderTargetLiveOwnerIdentitySet(
                           candidate.node, leaderContext,
                           leader, leaderView, receipt.subject),
                         sourceRank, budget))'

AdequateLeaderFixedCutPerActionProviderProperty(specification) ==
  specification => [][AdequateLeaderFixedCutPerActionProvider]_AsyncAllVars

(***************************************************************************
Pinned finite/coalesced selected-lifecycle episode.

The natural budget is exactly the frozen semantic owner-identity universe
minus `known`.  Equal-count A -> B replacement and count-increasing
replenishment lower that complement.  Neither arm is called occurrence-rank
progress.  Dormant logical identities need no separate physical coordinate:
they own no carrier while parked, remain members of the finite semantic
universe, and can become live only after replay reserves a fresh physical
ordinal after the source cutoff.

A lower frontier must carry the immutable protocol token, episode target,
source occurrence rank, source owner identity, source lifecycle cutoff,
semantic known set, physical source rank, and receipt ceiling; none may be
re-chosen by a temporal existential.  The legacy Dormant-set parameters are
constrained to `{}` so any source consumer attempting to reintroduce
scheduler-ordinal priority is rejected.

The selected current candidate may change from A to B, but it must remain at
the same episode target and protocol token.  At the frozen occurrence rank its
physical rank must equal `sourceRank`.  A count-increasing discovery may expose
a bounded higher current occurrence/physical rank, but the immutable source
occurrence, source owner, source rank, and receipt ceiling remain parameters of
the episode.  The step property remains the explicit deadline-carry provider.
Replenishment is not its goal; only strict source-rank descent or a smaller
identity complement is.

An already-live lower semantic occurrence outside this selected token/cut is
deliberately ignored.  It is not a physical descent witness and it does not
prevent initialization of this episode.  The selected candidate remains on
the same-or-higher carrier until its exact action either produces a real
lower physical rank or transfers the same token/cut to the pre-candidate
route.  The outer occurrence-rank composition in
`SumeragiV2AdequateLeaderServiceClosureProofs` remains unchanged.
***************************************************************************)

AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible(
    sourceOccurrenceRank, currentOccurrenceRank, sourceRank, currentRank) ==
  /\ sourceOccurrenceRank[1] = currentOccurrenceRank[1]
  /\ sourceOccurrenceRank[2] <= currentOccurrenceRank[2]
  /\ IF sourceOccurrenceRank = currentOccurrenceRank
     THEN currentRank = sourceRank
     ELSE /\ sourceOccurrenceRank[2] < currentOccurrenceRank[2]
          /\ currentRank[1] = sourceRank[1]

AdequateLeaderFixedPipelineOriginEpisodeFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    token, episodeTarget, sourceOccurrenceRank, sourceOccurrenceOwner,
    sourceCutoffOrdinal, sourceDormantPotential, knownDormantPotential,
    known, sourceRank, budget) ==
  /\ token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
  /\ episodeTarget \in ValidatorIds
  /\ sourceOccurrenceRank
       \in AdequateLeaderTargetOccurrenceRankCarrier
  /\ sourceOccurrenceOwner
       \in AdequateLeaderFrozenCandidateOwnerUniverse(
            episodeTarget, leaderContext, leader, leaderView,
            receipt.subject)
  /\ known
       \in SUBSET AdequateLeaderFrozenOwnerUniverse(
            episodeTarget, leaderContext, leader, leaderView,
            receipt.subject)
  /\ AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget(
       episodeTarget, leaderContext, leader, leaderView, receipt.subject,
       sourceOccurrenceRank, sourceCutoffOrdinal,
       sourceDormantPotential, knownDormantPotential,
       known, budget)
  /\ AdequateLeaderTargetOccurrenceOwnerCarried(
       episodeTarget, leaderContext, leader, leaderView,
       receipt.subject,
       sourceOccurrenceRank, known, sourceOccurrenceOwner)
  /\ ~AdequateLeaderFixedPipelineStrictRankGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank)
  /\ \/ \E candidate \in AsyncCandidateSet,
             cutoffOrdinal \in Nat,
             currentSemanticRank \in (1..4) \X (0..9),
             currentOccurrenceRank
               \in AdequateLeaderTargetOccurrenceRankCarrier,
             currentOccurrenceOwner
               \in AdequateLeaderFrozenCandidateOwnerUniverse(
                    episodeTarget, leaderContext, leader, leaderView,
                    receipt.subject),
             currentRank \in AdequateLeaderFixedPipelineRankCarrier,
             owner
               \in AdequateLeaderFixedSelectedServiceOwnerSet(
                    initialContext),
             packet \in AsyncPacketSet:
            /\ candidate.node = episodeTarget
            /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
                 candidate, leaderContext, leader, leaderView,
                 receipt.subject,
                 currentSemanticRank,
                 currentOccurrenceRank, currentOccurrenceOwner)
            /\ AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible(
                 sourceOccurrenceRank, currentOccurrenceRank,
                 sourceRank, currentRank)
            /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt,
                 token, candidate, cutoffOrdinal, currentSemanticRank,
                 owner, packet, currentRank)
     \/ \E route:
          AdequateLeaderFixedPipelineProducerHandoffFrontier(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            route, token, episodeTarget,
            sourceOccurrenceRank, sourceOccurrenceOwner,
            sourceCutoffOrdinal,
            sourceDormantPotential, knownDormantPotential,
            known, sourceRank, budget)

AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    token, episodeTarget, sourceOccurrenceRank, sourceOccurrenceOwner,
    sourceCutoffOrdinal, sourceDormantPotential, knownDormantPotential,
    known, sourceRank, sourceBudget) ==
  \/ AdequateLeaderFixedPipelineStrictRankGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank)
  \/ \E discovered,
         known2
           \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                episodeTarget, leaderContext, leader, leaderView,
                receipt.subject):
       \E lowerBudget
            \in SetLessThan(
                 sourceBudget,
                 AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering,
                 AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier):
         /\ discovered =
              AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet(
                episodeTarget, leaderContext, leader, leaderView,
                receipt.subject, known)
         /\ discovered # {}
         /\ known2 = known \cup discovered
         /\ AdequateLeaderFixedPipelineOriginEpisodeFrontier(
              initialContext, target, leaderContext,
              leader, leaderView, receipt,
              token, episodeTarget,
              sourceOccurrenceRank, sourceOccurrenceOwner,
              sourceCutoffOrdinal,
              sourceDormantPotential, knownDormantPotential,
              known2, sourceRank, lowerBudget)

\* Exact one-step carrier for the selected candidate arm of a pinned
\* occurrence episode.  An unrelated action must retain every immutable
\* coordinate and the same selected owner.  Equal-count replacement and
\* count-increasing replenishment are handled only through the fresh
\* finite-universe identity exposed by the semantic non-descent action;
\* their post-state is a smaller complement budget, never a progress goal in
\* its own right.  The producer-handoff arm is discharged separately below,
\* after global exact-cell selection has exposed its concrete route owner.
AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProvider ==
  \A sourceDormantPotential, knownDormantPotential:
    \A initialContext \in ContextRecords:
     \A target \in ValidatorIds:
      \A leaderContext \in ContextRecords:
       \A leader \in ValidatorIds:
        \A leaderView \in Views:
         \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
          \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
           \A episodeTarget \in ValidatorIds:
            \A sourceOccurrenceRank
                 \in AdequateLeaderTargetOccurrenceRankCarrier:
             \A sourceOccurrenceOwner
                  \in AdequateLeaderFrozenCandidateOwnerUniverse(
                       episodeTarget, leaderContext, leader, leaderView,
                       receipt.subject):
              \A sourceCutoffOrdinal \in Nat \ {0}:
               \A known
                    \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                         episodeTarget, leaderContext, leader, leaderView,
                         receipt.subject):
                \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
                 \A budget
                      \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
                  \A candidate \in AsyncCandidateSet:
                   \A cutoffOrdinal \in Nat:
                    \A currentSemanticRank \in (1..4) \X (0..9):
                     \A currentOccurrenceRank
                          \in AdequateLeaderTargetOccurrenceRankCarrier:
                      \A currentOccurrenceOwner
                           \in AdequateLeaderFrozenCandidateOwnerUniverse(
                                episodeTarget, leaderContext, leader, leaderView,
                                receipt.subject):
                       \A currentRank \in AdequateLeaderFixedPipelineRankCarrier:
                        \A owner
                             \in AdequateLeaderFixedSelectedServiceOwnerSet(
                                  initialContext):
                         \A packet \in AsyncPacketSet:
      /\ candidate.node = episodeTarget
      /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
           candidate, leaderContext, leader, leaderView, receipt.subject,
           currentSemanticRank,
           currentOccurrenceRank, currentOccurrenceOwner)
      /\ AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible(
           sourceOccurrenceRank, currentOccurrenceRank,
           sourceRank, currentRank)
      /\ AdequateLeaderFixedPipelineOriginEpisodeFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           token, episodeTarget,
           sourceOccurrenceRank, sourceOccurrenceOwner,
           sourceCutoffOrdinal,
           sourceDormantPotential, knownDormantPotential,
           known, sourceRank, budget)
      /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           token, candidate, cutoffOrdinal, currentSemanticRank,
           owner, packet, currentRank)
      /\ [AsyncNext]_AsyncAllVars
      => \/ AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
              initialContext, target, leaderContext,
              leader, leaderView, receipt,
              token, episodeTarget,
              sourceOccurrenceRank, sourceOccurrenceOwner,
              sourceCutoffOrdinal,
              sourceDormantPotential, knownDormantPotential,
              known, sourceRank, budget)'
         \/ /\ (AdequateLeaderFixedPipelineOriginEpisodeFrontier(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt,
                   token, episodeTarget,
                   sourceOccurrenceRank, sourceOccurrenceOwner,
                   sourceCutoffOrdinal,
                   sourceDormantPotential, knownDormantPotential,
                   known, sourceRank, budget))'
            /\ (AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
                  candidate, leaderContext, leader, leaderView,
                  receipt.subject, currentSemanticRank,
                  currentOccurrenceRank, currentOccurrenceOwner))'
            /\ (AdequateLeaderFixedSelectedPipelineRankFrontier(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt,
                  token, candidate, cutoffOrdinal, currentSemanticRank,
                  owner, packet, currentRank))'
            /\ ~<<AdequateLeaderFixedSelectedServiceOwnerAction(
                     owner)>>_AsyncAllVars

AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty(
    specification) ==
  specification
    => [][AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProvider]_AsyncAllVars

AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty(
    specification) ==
  specification
    => \A sourceDormantPotential, knownDormantPotential:
         \A initialContext \in ContextRecords:
          \A target \in ValidatorIds:
           \A leaderContext \in ContextRecords:
            \A leader \in ValidatorIds:
             \A leaderView \in Views:
              \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
               \A token
                    \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
                \A episodeTarget \in ValidatorIds:
                 \A sourceOccurrenceRank
                      \in AdequateLeaderTargetOccurrenceRankCarrier:
                  \A sourceOccurrenceOwner
                       \in AdequateLeaderFrozenCandidateOwnerUniverse(
                            episodeTarget, leaderContext, leader, leaderView,
                            receipt.subject):
                   \A sourceCutoffOrdinal \in Nat \ {0}:
                    \A known
                         \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                              episodeTarget, leaderContext, leader, leaderView,
                              receipt.subject):
                     \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
                      \A budget
                           \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
           AdequateLeaderFixedPipelineOriginEpisodeFrontier(
             initialContext, target, leaderContext,
             leader, leaderView, receipt,
             token, episodeTarget,
             sourceOccurrenceRank, sourceOccurrenceOwner,
             sourceCutoffOrdinal,
             sourceDormantPotential, knownDormantPotential,
             known, sourceRank, budget)
             ~> AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt,
                  token, episodeTarget,
                  sourceOccurrenceRank, sourceOccurrenceOwner,
                  sourceCutoffOrdinal,
                  sourceDormantPotential, knownDormantPotential,
                  known, sourceRank, budget)

AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty(
    specification) ==
  specification
    => \A sourceDormantPotential, knownDormantPotential:
         \A initialContext \in ContextRecords:
          \A target \in ValidatorIds:
           \A leaderContext \in ContextRecords:
            \A leader \in ValidatorIds:
             \A leaderView \in Views:
              \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
               \A token
                    \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
                \A episodeTarget \in ValidatorIds:
                 \A sourceOccurrenceRank
                      \in AdequateLeaderTargetOccurrenceRankCarrier:
                  \A sourceOccurrenceOwner
                       \in AdequateLeaderFrozenCandidateOwnerUniverse(
                            episodeTarget, leaderContext, leader, leaderView,
                            receipt.subject):
                   \A sourceCutoffOrdinal \in Nat \ {0}:
                    \A known
                         \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                              episodeTarget, leaderContext, leader, leaderView,
                              receipt.subject):
                     \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
                      \A budget
                           \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
           AdequateLeaderFixedPipelineOriginEpisodeFrontier(
             initialContext, target, leaderContext,
             leader, leaderView, receipt,
             token, episodeTarget,
             sourceOccurrenceRank, sourceOccurrenceOwner,
             sourceCutoffOrdinal,
             sourceDormantPotential, knownDormantPotential,
             known, sourceRank, budget)
             ~> AdequateLeaderFixedPipelineStrictRankGoal(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt, sourceRank)

THEOREM AdequateLeaderFixedPipelineOriginEpisodeStepClosesNonDescentEpisode ==
  \A specification:
    AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty(
      specification)
      => AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty(
           specification)
BY AdequateLeaderFixedPipelineOriginEpisodeBudgetOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty,
       AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty,
       AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal,
       AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering,
       AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier

\* No state-level semantic shortcut is permitted here.  A lower occurrence
\* which merely coexists with the selected candidate need not have a lower
\* physical pipeline rank.  The episode therefore starts for the selected
\* lifecycle unconditionally; only the carried `CutPerAction` transition may
\* establish `AdequateLeaderFixedPipelineStrictRankGoal`.
THEOREM AdequateLeaderFixedSelectedFrontierStartsPinnedEpisodeOrStrictRank ==
  \A initialContext \in ContextRecords:
   \A target \in ValidatorIds:
    \A leaderContext \in ContextRecords:
     \A leader \in ValidatorIds:
      \A leaderView \in Views:
       \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
        \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
         \A candidate \in AsyncCandidateSet:
          \A cutoffOrdinal \in Nat:
           \A semanticRank \in (1..4) \X (0..9):
            \A owner
                 \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
             \A packet \in AsyncPacketSet:
              \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, receipt,
         token, candidate, cutoffOrdinal, semanticRank,
         owner, packet, sourceRank)
      => \/ AdequateLeaderFixedPipelineStrictRankGoal(
              initialContext, target, leaderContext,
              leader, leaderView, receipt, sourceRank)
         \/ \E occurrenceRank
                  \in AdequateLeaderTargetOccurrenceRankCarrier,
               occurrenceOwner
                 \in AdequateLeaderFrozenCandidateOwnerUniverse(
                      candidate.node, leaderContext, leader, leaderView,
                      receipt.subject),
               known
                 \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                      candidate.node, leaderContext, leader, leaderView,
                      receipt.subject),
               budget
                 \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
              \E sourceDormantPotential, knownDormantPotential:
                /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
                     candidate, leaderContext, leader, leaderView,
                     receipt.subject, semanticRank,
                     occurrenceRank, occurrenceOwner)
                /\ sourceDormantPotential = {}
                /\ knownDormantPotential = {}
                /\ known =
                     AdequateLeaderTargetLiveOwnerIdentitySet(
                       candidate.node, leaderContext, leader, leaderView,
                       receipt.subject)
                /\ AdequateLeaderFixedPipelineOriginEpisodeFrontier(
                     initialContext, target, leaderContext,
                     leader, leaderView, receipt,
                     token, candidate.node,
                     occurrenceRank, occurrenceOwner,
                     cutoffOrdinal,
                     sourceDormantPotential, knownDormantPotential,
                     known, sourceRank, budget)
BY AdequateLeaderTargetCurrentOwnersInitializeKnownEpisode,
   AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced,
   FS_Subset, FS_CardinalityType, IsaT(600)
   DEF AdequateLeaderFixedPipelineOriginEpisodeFrontier,
       AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget,
       AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier,
       AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible,
       AdequateLeaderFixedSelectedOccurrenceLifecycleActive,
       AdequateLeaderFixedSelectedOccurrenceProducerResidual

AdequateLeaderFixedSelectedOwnerServiceProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords:
         \A target \in ValidatorIds:
          \A leaderContext \in ContextRecords:
           \A leader \in ValidatorIds:
            \A leaderView \in Views:
             \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
              \A token:
               \A candidate \in AsyncCandidateSet:
                \A cutoffOrdinal \in Nat:
                 \A semanticRank \in (1..4) \X (0..9):
                  \A owner
                       \in AdequateLeaderFixedSelectedServiceOwnerSet(
                            initialContext):
                   \A packet \in AsyncPacketSet:
                    \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedSelectedPipelineRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           token, candidate, cutoffOrdinal, semanticRank,
           owner, packet, sourceRank)
           ~> AdequateLeaderFixedPipelineStrictRankGoal(
                initialContext, target, leaderContext,
                leader, leaderView,
                receipt, sourceRank)

THEOREM AdequateLeaderFixedOriginEpisodeClosureSuppliesSelectedOwnerService ==
  \A specification:
    /\ (specification => []AsyncStrongTypeInvariant)
    /\ AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty(
         specification)
      => AdequateLeaderFixedSelectedOwnerServiceProperty(specification)
BY AdequateLeaderFixedSelectedFrontierStartsPinnedEpisodeOrStrictRank, PTL
   DEF AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty,
       AdequateLeaderFixedSelectedOwnerServiceProperty


AdequateLeaderFixedFreshPreCandidateEntryRankCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    route, token, entryDebt, owner, packet) ==
  /\ AdequateLeaderFixedPreCandidateEntryRankCell(
       initialContext, target, leaderContext,
       leader, leaderView, receipt,
       route, token, entryDebt, owner, packet)
  /\ route.predecessors =
       AdequateLeaderFixedPreCandidateRouteExpectedSourceCut(
         route, leaderContext, leaderView, receipt.subject)

AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    route, sourceRank) ==
  \/ AdequateLeaderFixedCutTerminalForAuthority(
       target, leaderContext, leader, leaderView, receipt)
  \/ \E lowerRank \in
       SetLessThan(
         sourceRank,
         AdequateLeaderFixedPipelineRankOrdering,
         AdequateLeaderFixedPipelineRankCarrier):
       \/ \E scheduledToken
              \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
            candidate \in AsyncCandidateSet,
            cutoffOrdinal \in Nat,
            semanticRank \in (1..4) \X (0..9),
            scheduledOwner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(
                   initialContext),
            scheduledPacket \in AsyncPacketSet:
           /\ AdequateLeaderFixedCandidateContinuesPreCandidateRoute(
                candidate, route, leaderContext,
                leader, leaderView, receipt.subject)
           /\ AdequateLeaderFixedPipelineRankCell(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                scheduledToken, candidate, cutoffOrdinal, semanticRank,
                scheduledOwner, scheduledPacket)
           /\ AdequateLeaderFixedPipelineRank(
                leaderContext, leader, leaderView, receipt.subject,
                scheduledToken, candidate, cutoffOrdinal,
                semanticRank, scheduledOwner, scheduledPacket)
                = lowerRank
          \/ \E entryToken
               \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
             entryDebt
               \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
             entryOwner
               \in AdequateLeaderFixedSelectedServiceOwnerSet(
                    initialContext),
             entryPacket \in AsyncPacketSet:
            /\ AdequateLeaderFixedPreCandidateEntryRankCell(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt,
                 route, entryToken, entryDebt, entryOwner, entryPacket)
            /\ AdequateLeaderFixedPreCandidateEntryRank(
                 entryToken, leaderContext, leader, leaderView, receipt.subject,
                 entryDebt, entryOwner, entryPacket)
                 = lowerRank

AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    route, token, entryDebt, owner, packet, sourceRank) ==
  /\ AdequateLeaderFixedPreCandidateEntryRankCell(
       initialContext, target, leaderContext,
       leader, leaderView, receipt,
       route, token, entryDebt, owner, packet)
  /\ AdequateLeaderFixedPreCandidateEntryRank(
       token, leaderContext, leader, leaderView, receipt.subject,
       entryDebt, owner, packet)
       = sourceRank
  /\ ENABLED
       (AdequateLeaderFixedSelectedServiceOwnerAction(owner)
          /\ AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
               initialContext, target, leaderContext,
               leader, leaderView, receipt, route, sourceRank)')
  /\ IF AdequateLeaderFixedEntryServiceSlack(
          owner, packet, leader) > 0
     THEN owner.ownerKind \in {"Tick", "TickPacket"}
     ELSE owner.ownerKind \notin {"Tick", "TickPacket"}

AdequateLeaderFixedPreCandidateEntryServiceProperty(specification) ==
  specification
    => \A route:
       \A initialContext \in ContextRecords:
        \A target \in ValidatorIds:
         \A leaderContext \in ContextRecords:
          \A leader \in ValidatorIds:
           \A leaderView \in Views:
            \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
             \A token
                  \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
              \A entryDebt
                   \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge:
               \A owner
                    \in AdequateLeaderFixedSelectedServiceOwnerSet(
                         initialContext):
                \A packet \in AsyncPacketSet:
                 \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           route, token, entryDebt, owner, packet, sourceRank)
           ~> AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, route, sourceRank)

\* Until the exact selected pre-candidate owner executes, unrelated steps must
\* preserve the same token, entry debt, owner, packet, source rank, and receipt
\* ceiling.  This is the safety half needed to apply the owner's exact weak
\* fairness; selecting a different queued token is not a permitted successor.
AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider ==
  \A route:
  \A initialContext \in ContextRecords:
   \A target \in ValidatorIds:
    \A leaderContext \in ContextRecords:
     \A leader \in ValidatorIds:
      \A leaderView \in Views:
       \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
        \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
         \A entryDebt \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge:
          \A owner
               \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
           \A packet \in AsyncPacketSet:
            \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
    /\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, receipt,
         route, token, entryDebt, owner, packet, sourceRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
            initialContext, target, leaderContext,
            leader, leaderView, receipt, route, sourceRank)'
       \/ /\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
               initialContext, target, leaderContext,
               leader, leaderView, receipt,
               route, token, entryDebt, owner, packet, sourceRank)'
          /\ ~<<AdequateLeaderFixedSelectedServiceOwnerAction(
                   owner)>>_AsyncAllVars

AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty(
    specification) ==
  specification
    => [][AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider]_AsyncAllVars

\* These are the explicit clock-carry seams.  For Tick, `asyncNow'` rises
\* while the selected slack falls by the same amount.  For a due owner, the
\* next owner deadline may reset by at most `AsyncDeliveryBound`, but only
\* after action debt or the outer token coordinate strictly falls.  Because
\* both goals below contain `AdequateLeaderFixedPipelineAbsoluteCeiling`, a
\* reset cannot refresh `receipt.deadlineReceipt.armedAt` or borrow another clock window.
AdequateLeaderFixedSelectedCandidateActionCarriesAbsoluteCeiling ==
  \A sourceDormantPotential, knownDormantPotential:
    \A initialContext \in ContextRecords:
     \A target \in ValidatorIds:
      \A leaderContext \in ContextRecords:
       \A leader \in ValidatorIds:
        \A leaderView \in Views:
         \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
          \A token:
           \A candidate \in AsyncCandidateSet:
            \A cutoffOrdinal \in Nat:
             \A semanticRank \in (1..4) \X (0..9):
              \A owner
                   \in AdequateLeaderFixedSelectedServiceOwnerSet(
                        initialContext):
               \A packet \in AsyncPacketSet:
                \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
                 \A sourceOccurrenceRank
                      \in AdequateLeaderTargetOccurrenceRankCarrier:
                  \A sourceOccurrenceOwner
                       \in AdequateLeaderFrozenCandidateOwnerUniverse(
                            candidate.node, leaderContext, leader, leaderView,
                            receipt.subject):
                   \A sourceCutoffOrdinal \in Nat \ {0}:
                    \A sourceKnown
                         \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                              candidate.node, leaderContext, leader, leaderView,
                              receipt.subject):
                     \A currentOccurrenceRank
                          \in AdequateLeaderTargetOccurrenceRankCarrier:
                      \A currentOccurrenceOwner
                           \in AdequateLeaderFrozenCandidateOwnerUniverse(
                                candidate.node, leaderContext, leader, leaderView,
                                receipt.subject):
                       \A currentRank \in AdequateLeaderFixedPipelineRankCarrier:
                        \A sourceBudget
                             \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
      /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
           candidate, leaderContext, leader, leaderView,
           receipt.subject, semanticRank,
           currentOccurrenceRank, currentOccurrenceOwner)
      /\ AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible(
           sourceOccurrenceRank, currentOccurrenceRank,
           sourceRank, currentRank)
      /\ AdequateLeaderFixedPipelineOriginEpisodeFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           token, candidate.node, sourceOccurrenceRank,
           sourceOccurrenceOwner, sourceCutoffOrdinal,
           sourceDormantPotential, knownDormantPotential,
           sourceKnown, sourceRank, sourceBudget)
      /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           token, candidate, cutoffOrdinal, semanticRank,
           owner, packet, currentRank)
      /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
             owner)>>_AsyncAllVars
      => AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
           initialContext, target, leaderContext,
           leader, leaderView, receipt,
           token, candidate.node, sourceOccurrenceRank,
           sourceOccurrenceOwner, sourceCutoffOrdinal,
           sourceDormantPotential, knownDormantPotential,
           sourceKnown, sourceRank, sourceBudget)'

AdequateLeaderFixedSelectedEntryActionCarriesAbsoluteCeiling ==
  \A route:
  \A initialContext \in ContextRecords:
   \A target \in ValidatorIds:
    \A leaderContext \in ContextRecords:
     \A leader \in ValidatorIds:
      \A leaderView \in Views:
       \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
        \A token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
         \A entryDebt \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge:
          \A owner
               \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
           \A packet \in AsyncPacketSet:
            \A sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
    /\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, receipt,
         route, token, entryDebt, owner, packet, sourceRank)
    /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
           owner)>>_AsyncAllVars
    => AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, route, sourceRank)'

AdequateLeaderFixedSelectedActionClockCarryProviderProperty(
    specification) ==
  specification
    => /\ [][AdequateLeaderFixedSelectedCandidateActionCarriesAbsoluteCeiling]_AsyncAllVars
       /\ [][AdequateLeaderFixedSelectedEntryActionCarriesAbsoluteCeiling]_AsyncAllVars

THEOREM
    AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepFollowsProviders ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncStrongTypeInvariant'
  /\ AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider
  /\ AdequateLeaderFixedCutPerActionProvider
  /\ AdequateLeaderFixedSelectedCandidateActionCarriesAbsoluteCeiling
    => AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProvider
BY AdequateLeaderTargetNonDescentActionExposesFreshEpisodeIdentity,
   AdequateLeaderFreshTimeoutVoteReplenishmentConsumesProducerSlotAndOpensNonDescentEpisode,
   AdequateLeaderCarriedNonDescentResidualAdvancesKnownBudget,
   AdequateLeaderTargetNonDescentDiscoveryStrictlyConsumesBudget,
   AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant,
   AdequateLeaderLiveOwnersStayInsideFrozenUniverse,
   FS_Union, FS_Subset, FS_CardinalityType, IsaT(18000)
   DEF
     AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProvider,
     AdequateLeaderFixedPipelineOriginEpisodeFrontier,
     AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal,
     AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget,
     AdequateLeaderFixedSelectedOccurrenceLifecycleActive,
     AdequateLeaderFixedSelectedOccurrenceProducerResidual,
     AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier,
     AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering,
     AdequateLeaderFixedPipelineOriginNonDescentEpisodeAction,
     AdequateLeaderFixedPipelineOriginEqualCountReplacementAction,
     AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction,
     AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible,
     AdequateLeaderFixedCutPerActionProvider,
     AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider,
     AdequateLeaderTargetCarriedNonDescentEpisodeResidual,
     AdequateLeaderTargetCarriedNonDescentKnownAdvanceGoal,
     AdequateLeaderTargetNonDescentKnownAdvanceGoal,
     AdequateLeaderTargetNonDescentEpisodeResidual,
     AdequateLeaderTargetNonDescentEpisodeAtBudget,
     AdequateLeaderTargetNonDescentEpisodeFrontier,
     AdequateLeaderTargetNonDescentEpisodeBudget,
     AdequateLeaderTargetEpisodeKnownOwnerSet,
     SetLessThan, OpToRel

THEOREM AdequateLeaderFixedPreCandidateSelectionAndFairnessSupplyEntryService ==
  \A specification:
    /\ AdequateLeaderAsyncNextBehaviorProperty(specification)
    /\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification)
    /\ AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty(
         specification)
    /\ AdequateLeaderFixedSelectedActionClockCarryProviderProperty(
         specification)
      => AdequateLeaderFixedPreCandidateEntryServiceProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                AdequateLeaderAsyncNextBehaviorProperty(specification),
                AdequateLeaderFixedSelectedOwnerFairnessProperty(
                  specification),
                AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty(
                  specification),
                AdequateLeaderFixedSelectedActionClockCarryProviderProperty(
                  specification),
                specification
         PROVE \A route:
                 \A initialContext \in ContextRecords:
                  \A target \in ValidatorIds:
                   \A leaderContext \in ContextRecords:
                    \A leader \in ValidatorIds:
                     \A leaderView \in Views:
                      \A receipt
                           \in AdequateLeaderAuthorityDeadlineReceiptSet:
                       \A token
                            \in AdequateLeaderFixedPipelineTokenCarrier(
                                 leaderContext):
                        \A entryDebt
                             \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge:
                         \A owner
                              \in AdequateLeaderFixedSelectedServiceOwnerSet(
                                   initialContext):
                          \A packet \in AsyncPacketSet:
                           \A sourceRank
                                \in AdequateLeaderFixedPipelineRankCarrier:
                   AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                     initialContext, target, leaderContext,
                     leader, leaderView, receipt,
                     route, token, entryDebt, owner, packet, sourceRank)
                     ~> AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                          initialContext, target, leaderContext,
                          leader, leaderView, receipt, route, sourceRank)
    <2>1. ASSUME NEW route,
                  NEW initialContext \in ContextRecords,
                  NEW target \in ValidatorIds,
                  NEW leaderContext \in ContextRecords,
                  NEW leader \in ValidatorIds,
                  NEW leaderView \in Views,
                  NEW receipt
                    \in AdequateLeaderAuthorityDeadlineReceiptSet,
                  NEW token
                    \in AdequateLeaderFixedPipelineTokenCarrier(
                         leaderContext),
                  NEW entryDebt
                    \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
                  NEW owner
                    \in AdequateLeaderFixedSelectedServiceOwnerSet(
                         initialContext),
                  NEW packet \in AsyncPacketSet,
                  NEW sourceRank
                    \in AdequateLeaderFixedPipelineRankCarrier
           PROVE AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt,
                   route, token, entryDebt, owner, packet, sourceRank)
                   ~> AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt, route, sourceRank)
      <3>1. [](AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt,
                  route, token, entryDebt, owner, packet, sourceRank)
                /\ ~AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                     initialContext, target, leaderContext,
                     leader, leaderView, receipt, route, sourceRank)
               => ENABLED
                    <<AdequateLeaderFixedSelectedServiceOwnerAction(
                        owner)>>_AsyncAllVars)
        BY PTL, IsaT(900)
           DEF AdequateLeaderFixedSelectedPreCandidateEntryFrontier,
               AsyncAllVars
      <3>2. /\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt,
                   route, token, entryDebt, owner, packet, sourceRank)
             /\ ~AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt, route, sourceRank)
             /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
                      owner)>>_AsyncAllVars
            => AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt, route, sourceRank)'
        BY <1>1, PTL
           DEF AdequateLeaderFixedSelectedActionClockCarryProviderProperty,
               AdequateLeaderFixedSelectedEntryActionCarriesAbsoluteCeiling
      <3>3. /\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt,
                   route, token, entryDebt, owner, packet, sourceRank)
             /\ ~AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt, route, sourceRank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
                     initialContext, target, leaderContext,
                     leader, leaderView, receipt, route, sourceRank)'
               \/ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                    initialContext, target, leaderContext,
                    leader, leaderView, receipt,
                    route, token, entryDebt, owner, packet, sourceRank)'
        BY <1>1, PTL
           DEF AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty
      <3>4. WF_AsyncAllVars(
               AdequateLeaderFixedSelectedServiceOwnerAction(owner))
        BY <1>1, <2>1, PTL
           DEF AdequateLeaderFixedSelectedOwnerFairnessProperty
      <3>5. [][AsyncNext]_AsyncAllVars
        BY <1>1
           DEF AdequateLeaderAsyncNextBehaviorProperty
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1
     DEF AdequateLeaderFixedPreCandidateEntryServiceProperty

AdequateLeaderFixedSelectedPipelineServiceRankFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank) ==
  \/ \E token
         \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
       candidate \in AsyncCandidateSet,
       cutoffOrdinal \in Nat,
       semanticRank \in (1..4) \X (0..9),
       owner
         \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
       packet \in AsyncPacketSet:
       AdequateLeaderFixedSelectedPipelineRankFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, receipt,
         token, candidate, cutoffOrdinal, semanticRank,
         owner, packet, sourceRank)
  \/ \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
       \E route:
         \E entryDebt
              \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
            packet \in AsyncPacketSet:
           AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
             initialContext, target, leaderContext,
             leader, leaderView, receipt,
             route, token, entryDebt, owner, packet, sourceRank)

AdequateLeaderFixedPipelineServiceRankFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank) ==
  \/ \E token
         \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
       candidate \in AsyncCandidateSet,
       cutoffOrdinal \in Nat,
       semanticRank \in (1..4) \X (0..9),
       owner
         \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
       packet \in AsyncPacketSet:
       /\ AdequateLeaderFixedPipelineRankCell(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet)
       /\ AdequateLeaderFixedPipelineRank(
            leaderContext, leader, leaderView, receipt.subject,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet)
            = sourceRank
  \/ \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
       \E route:
         \E entryDebt
              \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
            packet \in AsyncPacketSet:
           /\ AdequateLeaderFixedPreCandidateEntryRankCell(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                route, token, entryDebt, owner, packet)
           /\ AdequateLeaderFixedPreCandidateEntryRank(
                token, leaderContext, leader, leaderView, receipt.subject,
                entryDebt, owner, packet)
                = sourceRank

\* The global fixed-clock blocker must stay attached to one immutable service
\* cell.  Merely finding some other selected candidate at the same numeric
\* rank is not progress for the source cell: two distinct candidates may have
\* equal rank coordinates.  These proof-only identities retain every stable
\* candidate coordinate, or the complete frozen pre-candidate route.  The
\* current Tick/action owner is deliberately excluded because it changes as
\* the same cell's absolute deadline is consumed.
AdequateLeaderFixedCandidatePipelineServiceCellIdentity(
    token, candidate, cutoffOrdinal, semanticRank) ==
  [kind |-> "Candidate",
   token |-> token,
   candidate |-> candidate,
   cutoffOrdinal |-> cutoffOrdinal,
   semanticRank |-> semanticRank]

AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity(route, token) ==
  [kind |-> "PreCandidate",
   token |-> token,
   route |-> route]

AdequateLeaderFixedPreCandidateRouteCarrier ==
  [kind: {"Local", "Wire", "Producer"},
   token: AdequateLeaderFixedAnyPipelineTokenCarrier,
   identity: AdequateLeaderFixedPreCandidateRouteIdentityCarrier,
   node: ValidatorIds,
   ordinal: Nat \ {0},
   predecessors: SUBSET AsyncCandidateCausalOriginSet]

AdequateLeaderFixedPipelineServiceCellIdentityCarrier ==
  {AdequateLeaderFixedCandidatePipelineServiceCellIdentity(
     token, candidate, cutoffOrdinal, semanticRank):
     token \in AdequateLeaderFixedAnyPipelineTokenCarrier,
     candidate \in AsyncCandidateSet,
     cutoffOrdinal \in Nat,
     semanticRank \in (1..4) \X (0..9)}
    \cup
  {AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity(route, token):
     route \in AdequateLeaderFixedPreCandidateRouteCarrier,
     token \in AdequateLeaderFixedAnyPipelineTokenCarrier}

AdequateLeaderFixedPipelineServiceRankFrontierForCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, cellIdentity) ==
  \/ \E token
         \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
       candidate \in AsyncCandidateSet,
       cutoffOrdinal \in Nat,
       semanticRank \in (1..4) \X (0..9),
       owner
         \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
       packet \in AsyncPacketSet:
       /\ cellIdentity =
            AdequateLeaderFixedCandidatePipelineServiceCellIdentity(
              token, candidate, cutoffOrdinal, semanticRank)
       /\ AdequateLeaderFixedPipelineRankCell(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet)
       /\ AdequateLeaderFixedPipelineRank(
            leaderContext, leader, leaderView, receipt.subject,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet)
            = sourceRank
  \/ \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
       \E route \in AdequateLeaderFixedPreCandidateRouteCarrier:
         \E entryDebt
              \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
            packet \in AsyncPacketSet:
           /\ cellIdentity =
                AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity(
                  route, token)
           /\ AdequateLeaderFixedPreCandidateEntryRankCell(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                route, token, entryDebt, owner, packet)
           /\ AdequateLeaderFixedPreCandidateEntryRank(
                token, leaderContext, leader, leaderView, receipt.subject,
                entryDebt, owner, packet)
                = sourceRank

AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, cellIdentity) ==
  \/ \E token
         \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
       candidate \in AsyncCandidateSet,
       cutoffOrdinal \in Nat,
       semanticRank \in (1..4) \X (0..9),
       owner
         \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
       packet \in AsyncPacketSet:
       /\ cellIdentity =
            AdequateLeaderFixedCandidatePipelineServiceCellIdentity(
              token, candidate, cutoffOrdinal, semanticRank)
       /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet, sourceRank)
  \/ \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
       \E route \in AdequateLeaderFixedPreCandidateRouteCarrier:
         \E entryDebt
              \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
            packet \in AsyncPacketSet:
           /\ cellIdentity =
                AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity(
                  route, token)
           /\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                route, token, entryDebt, owner, packet, sourceRank)

THEOREM AdequateLeaderFixedPipelineServiceRankFrontierHasExactCell ==
  \A initialContext, target, leaderContext, leader, leaderView, receipt,
     sourceRank:
    AdequateLeaderFixedPipelineServiceRankFrontier(
      initialContext, target, leaderContext,
      leader, leaderView, receipt, sourceRank)
      => \E cellIdentity
             \in AdequateLeaderFixedPipelineServiceCellIdentityCarrier:
           AdequateLeaderFixedPipelineServiceRankFrontierForCell(
             initialContext, target, leaderContext,
             leader, leaderView, receipt, sourceRank, cellIdentity)
BY Isa
   DEF AdequateLeaderFixedPipelineServiceRankFrontier,
       AdequateLeaderFixedPipelineServiceRankFrontierForCell,
       AdequateLeaderFixedPipelineServiceCellIdentityCarrier,
       AdequateLeaderFixedAnyPipelineTokenCarrier,
       AdequateLeaderFixedPreCandidateRouteCarrier,
       AdequateLeaderFixedPreCandidateRouteIdentityCarrier,
       AdequateLeaderFixedCandidatePipelineServiceCellIdentity,
       AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity,
       AdequateLeaderFixedPreCandidateEntryRankCell,
       AdequateLeaderFixedPreCandidateRouteTyped

THEOREM AdequateLeaderFixedSelectedExactCellProjectsToServiceFrontier ==
  \A initialContext, target, leaderContext, leader, leaderView, receipt,
     sourceRank, cellIdentity:
    AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell(
      initialContext, target, leaderContext,
      leader, leaderView, receipt, sourceRank, cellIdentity)
      => AdequateLeaderFixedSelectedPipelineServiceRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank)
BY Isa
   DEF AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell,
       AdequateLeaderFixedSelectedPipelineServiceRankFrontier

\* At the first state of an obligation the route stores equality with the
\* current admission cut.  Later frontiers retain that immutable set while
\* the live cut may shrink, so they use the subset form in the ordinary cell.
AdequateLeaderFixedFreshPipelineServiceRankFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank) ==
  \/ \E token
         \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
       candidate \in AsyncCandidateSet,
       cutoffOrdinal \in Nat,
       semanticRank \in (1..4) \X (0..9),
       owner
         \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
       packet \in AsyncPacketSet:
       /\ AdequateLeaderFixedPipelineRankCell(
            initialContext, target, leaderContext,
            leader, leaderView, receipt,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet)
       /\ AdequateLeaderFixedPipelineRank(
            leaderContext, leader, leaderView, receipt.subject,
            token, candidate, cutoffOrdinal, semanticRank,
            owner, packet)
            = sourceRank
  \/ \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
       \E route:
         \E entryDebt
              \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
            packet \in AsyncPacketSet:
           /\ AdequateLeaderFixedFreshPreCandidateEntryRankCell(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                route, token, entryDebt, owner, packet)
           /\ AdequateLeaderFixedPreCandidateEntryRank(
                token, leaderContext, leader, leaderView, receipt.subject,
                entryDebt, owner, packet)
                = sourceRank

\* Expose the immutable shared scheduler position of the first fixed-subject
\* target.  This is proof data only; the ordinary frontier remains unchanged.
AdequateLeaderFixedFreshPipelineServiceRankFrontierAtOrdinal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, targetOrdinal) ==
  /\ targetOrdinal \in Nat \ {0}
  /\ \/ \E token
           \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
         candidate \in AsyncCandidateSet,
         semanticRank \in (1..4) \X (0..9),
         owner
           \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
         packet \in AsyncPacketSet:
         /\ AdequateLeaderFixedPipelineRankCell(
              initialContext, target, leaderContext,
              leader, leaderView, receipt,
              token, candidate, targetOrdinal, semanticRank,
              owner, packet)
         /\ AdequateLeaderFixedPipelineRank(
              leaderContext, leader, leaderView, receipt.subject,
              token, candidate, targetOrdinal, semanticRank,
              owner, packet)
              = sourceRank
     \/ \E token
          \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
          \E route:
            \E entryDebt
                 \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
               owner
                 \in AdequateLeaderFixedSelectedServiceOwnerSet(
                      initialContext),
               packet \in AsyncPacketSet:
              /\ route.ordinal = targetOrdinal
              /\ AdequateLeaderFixedFreshPreCandidateEntryRankCell(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt,
                   route, token, entryDebt, owner, packet)
              /\ AdequateLeaderFixedPreCandidateEntryRank(
                   token, leaderContext, leader, leaderView,
                   receipt.subject, entryDebt, owner, packet)
                   = sourceRank

AdequateLeaderFixedPreCandidateRouteCarriesSubjectReplacementOrigin(
    route, leaderContext, origin) ==
  CASE route.kind = "Wire" ->
         AsyncLeaderWireLifecycleCausalOriginAt(
           route.identity, leaderContext) = origin
    [] route.kind = "Producer" ->
         route.causalOrigin = origin
    [] OTHER -> FALSE

AdequateLeaderFixedSubjectReplacementReceipt(sourceReceipt, subject) ==
  [deadlineReceipt |-> sourceReceipt.deadlineReceipt,
   subject |-> subject]

\* After every earlier owner in the frozen cut is serviced, the selected last
\* owner may enter the fixed kernel only through its exact lifecycle token and
\* source-admitted physical carrier (or a causal child which retains both).
\* A Dormant replay receives a later carrier and cannot join the frozen
\* predecessor subtraction.  Other owners remain logically after the anchor
\* or physically at/after the cut.  The receipt keeps the original `armedAt`
\* value, and the ordinary rank cell must still satisfy its absolute ceiling;
\* subject replacement therefore cannot reset the 4*N window.
AdequateLeaderFixedAnchoredSubjectPipelineServiceRankFrontier(
    initialContext, target, leaderContext, leader, leaderView,
    sourceReceipt, cut, knownPotential, sourceRank) ==
  LET anchor == cut.targetOwner
      receipt ==
        AdequateLeaderFixedSubjectReplacementReceipt(
          sourceReceipt, anchor.subject)
      live ==
        AdequateLeaderFixedLiveSubjectReplacementOwners(
          target, leaderContext, leader, leaderView)
      remaining ==
        AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors(
          cut, knownPotential)
  IN /\ cut \in AdequateLeaderFixedSubjectReplacementCutSet
     /\ receipt \in AdequateLeaderAuthorityDeadlineReceiptSet
     /\ cut.target = target
     /\ cut.context = leaderContext
     /\ cut.leader = leader
     /\ cut.view = leaderView
     /\ AdequateLeaderFixedSubjectReplacementPotentialKnownExact(
          cut, knownPotential)
     /\ remaining = {}
     /\ \A later
          \in live
                \ (AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners(
                     cut, knownPotential)
                    \cup {anchor}):
          \/ anchor.ordinal <= later.ordinal
          \/ cut.physicalCutoffOrdinal <= later.carrierOrdinal
     /\ AsyncCausalEpisodeFrozenPredecessorOrigins(
          leader, anchor.ordinal)
          \subseteq cut.predecessorOrigins
     /\ cut.physicalCutoffOrdinal
          <= AsyncNextIngressPhysicalOrdinal(leader)
     /\ \/ \E token
              \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext),
            candidate \in AsyncCandidateSet,
            semanticRank \in (1..4) \X (0..9),
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(
                   initialContext),
            packet \in AsyncPacketSet:
            /\ candidate.causalOrigin = anchor.origin
            /\ AdequateLeaderFixedPipelineRankCell(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt,
                 token, candidate, anchor.ordinal, semanticRank,
                 owner, packet)
            /\ AdequateLeaderFixedPipelineRank(
                 leaderContext, leader, leaderView, receipt.subject,
                 token, candidate, anchor.ordinal, semanticRank,
                 owner, packet)
                 = sourceRank
        \/ \E token
             \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
             \E route:
               \E entryDebt
                    \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
                  owner
                    \in AdequateLeaderFixedSelectedServiceOwnerSet(
                         initialContext),
                  packet \in AsyncPacketSet:
                 /\ route.ordinal = anchor.ordinal
                 /\ cut.predecessorOrigins
                      \subseteq route.predecessors
                 /\ AdequateLeaderFixedPreCandidateRouteCarriesSubjectReplacementOrigin(
                      route, leaderContext, anchor.origin)
                 /\ AdequateLeaderFixedPreCandidateEntryRankCell(
                      initialContext, target, leaderContext,
                      leader, leaderView, receipt,
                      route, token, entryDebt, owner, packet)
                 /\ AdequateLeaderFixedPreCandidateEntryRank(
                      token, leaderContext, leader, leaderView,
                      receipt.subject, entryDebt, owner, packet)
                      = sourceRank

AdequateLeaderFixedPipelineServiceRankDescentGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank) ==
  \/ AdequateLeaderFixedCutTerminalForAuthority(
       target, leaderContext, leader, leaderView, receipt)
  \/ \E lowerRank
       \in SetLessThan(
            sourceRank,
            AdequateLeaderFixedPipelineRankOrdering,
            AdequateLeaderFixedPipelineRankCarrier):
       AdequateLeaderFixedPipelineServiceRankFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, lowerRank)

THEOREM AdequateLeaderFixedStrictGoalsProjectToServiceRankDescent ==
  \A initialContext, target, leaderContext, leader, leaderView, receipt,
     sourceRank:
    (\/ AdequateLeaderFixedPipelineStrictRankGoal(
          initialContext, target, leaderContext,
          leader, leaderView, receipt, sourceRank)
     \/ \E route:
          AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
            initialContext, target, leaderContext,
            leader, leaderView, receipt, route, sourceRank))
      => AdequateLeaderFixedPipelineServiceRankDescentGoal(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank)
BY Isa
   DEF AdequateLeaderFixedPipelineStrictRankGoal,
       AdequateLeaderFixedPreCandidateEntryStrictRankGoal,
       AdequateLeaderFixedPipelineServiceRankDescentGoal,
       AdequateLeaderFixedPipelineServiceRankFrontier

AdequateLeaderFixedSelectedPipelineServiceRankDescentProperty(
    specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedSelectedPipelineServiceRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank)
           ~> AdequateLeaderFixedPipelineServiceRankDescentGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank)

THEOREM AdequateLeaderFixedSelectedServicesSupplyPipelineRankDescent ==
  \A specification:
    /\ AdequateLeaderFixedSelectedOwnerServiceProperty(specification)
    /\ AdequateLeaderFixedPreCandidateEntryServiceProperty(specification)
      => AdequateLeaderFixedSelectedPipelineServiceRankDescentProperty(
           specification)
BY AdequateLeaderFixedStrictGoalsProjectToServiceRankDescent, PTL
   DEF AdequateLeaderFixedSelectedOwnerServiceProperty,
       AdequateLeaderFixedPreCandidateEntryServiceProperty,
       AdequateLeaderFixedSelectedPipelineServiceRankFrontier,
       AdequateLeaderFixedSelectedPipelineServiceRankDescentProperty

(***************************************************************************
Frozen global fixed-clock blocker prefix.

A due action for another responsive node can disable Tick while the selected
pipeline cell is otherwise ready.  The immutable exact-Decision lifecycle
snapshot therefore freezes the due packet set, source-qualified ingress
journal, causal roots, materialized Candidate/Serve identities, and the past
logical scheduler and physical admission cuts.  It grants no future ordinal
interval.  A Dormant lifecycle without an already admitted physical carrier
is outside this prefix.  Its producer rank is
the actual remaining ingress journal followed by the proofless retained
handoff rank and the exact Candidate-occurrence/Serve-work/Serve-reach tail.
An unrelated later admission cannot enter the frozen predecessor set, and a
same-source retry is a journal stutter rather than progress.
***************************************************************************)

AdequateLeaderFixedGlobalLifecycleSnapshotCarrier ==
  [clock: Nat,
   packets: SUBSET AsyncPacketSet,
   producerRequests: SUBSET AsyncProducerIngressRequests,
   ingressEpisodes: SUBSET AsyncProducerIngressEpisodeSet,
   candidateRoots: SUBSET ExactDecisionTargetNeutralCausalRootSet,
   schedulerCuts: [Responsive -> Nat],
   physicalCuts: [Responsive -> Nat],
   predecessors:
     SUBSET
       (({"Packet"} \X AsyncPacketSet)
          \cup
        ({"Candidate"}
           \X ExactDecisionTargetNeutralCandidateOwnerIdentitySet)
          \cup
        ({"Serve"} \X ExactDecisionTargetNeutralServeOwnerIdentitySet)
          \cup
        ({"SchedulerCut"}
           \X [node: ValidatorIds, cutoffOrdinal: Nat])),
   candidateIdentities:
     SUBSET ExactDecisionTargetNeutralCandidateOwnerIdentitySet,
   serveIdentities:
     SUBSET ExactDecisionTargetNeutralServeOwnerIdentitySet]

AdequateLeaderFixedGlobalBlockerSnapshotCarrier ==
  [serviceCellIdentity:
     AdequateLeaderFixedPipelineServiceCellIdentityCarrier,
   fixedClock: AdequateLeaderFixedGlobalLifecycleSnapshotCarrier]

AdequateLeaderFixedConfiguredGlobalBlockerSnapshot(
    clockValue, serviceCellIdentity) ==
  [serviceCellIdentity |-> serviceCellIdentity,
   fixedClock |->
     ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)]

AdequateLeaderFixedGlobalBlockerSnapshotActive(snapshot, clockValue) ==
  /\ snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier
  /\ snapshot.serviceCellIdentity
       \in AdequateLeaderFixedPipelineServiceCellIdentityCarrier
  /\ snapshot.fixedClock.clock = clockValue
  /\ ExactDecisionTargetNeutralSnapshotActive(
       snapshot.fixedClock, clockValue)

AdequateLeaderFixedGlobalBlockerRankCarrier ==
  ExactDecisionTargetNeutralFixedClockCarrier

AdequateLeaderFixedGlobalBlockerRankOrdering ==
  ExactDecisionTargetNeutralFixedClockOrdering

AdequateLeaderFixedConcreteGlobalBlockerRank(snapshot, clockValue) ==
  ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
    snapshot.fixedClock, clockValue)

AdequateLeaderFixedGlobalBlockerProducerPrefix(blockerRank) ==
  ExactDecisionTargetNeutralProducerPrefix(blockerRank)

AdequateLeaderFixedGlobalProducerEpisodeRank(snapshot) ==
  ExactDecisionTargetNeutralProducerEpisodeRank(snapshot.fixedClock)

AdequateLeaderFixedGlobalProducerEpisodeRankCarrier ==
  ExactDecisionTargetNeutralProducerEpisodeCarrier

AdequateLeaderFixedGlobalProducerEpisodeRankOrdering ==
  ExactDecisionTargetNeutralProducerEpisodeOrdering

AdequateLeaderFixedGlobalProducerEpisodeBottom ==
  ExactDecisionTargetNeutralProducerEpisodeBottom

AdequateLeaderFixedGlobalBlockerSelectionGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, serviceCellIdentity) ==
  \/ AdequateLeaderFixedPipelineServiceRankDescentGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank)
  \/ AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank, serviceCellIdentity)

AdequateLeaderFixedGlobalBlockerPending(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue) ==
  /\ AdequateLeaderFixedPipelineServiceRankFrontier(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank)
  /\ ~AdequateLeaderFixedGlobalBlockerSelectionGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank,
       snapshot.serviceCellIdentity)
  /\ AdequateLeaderFixedPipelineServiceRankFrontierForCell(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank,
       snapshot.serviceCellIdentity)
  /\ clockValue \in Nat
  /\ asyncNow = clockValue
  /\ AdequateLeaderFixedGlobalBlockerSnapshotActive(snapshot, clockValue)

AdequateLeaderFixedGlobalBlockerAtRank(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank) ==
  /\ AdequateLeaderFixedGlobalBlockerPending(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank, snapshot, clockValue)
  /\ blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier
  /\ AdequateLeaderFixedConcreteGlobalBlockerRank(snapshot, clockValue)
       = blockerRank

AdequateLeaderFixedGlobalBlockerStrictRankGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank) ==
  \/ AdequateLeaderFixedGlobalBlockerSelectionGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank,
       snapshot.serviceCellIdentity)
  \/ \E lowerRank
       \in SetLessThan(
            blockerRank,
            AdequateLeaderFixedGlobalBlockerRankOrdering,
            AdequateLeaderFixedGlobalBlockerRankCarrier):
       AdequateLeaderFixedGlobalBlockerAtRank(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, sourceRank,
         snapshot, clockValue, lowerRank)

AdequateLeaderFixedGlobalProducerEpisodeAtRank(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank, episodeRank) ==
  LET currentRank ==
        AdequateLeaderFixedConcreteGlobalBlockerRank(snapshot, clockValue)
  IN /\ AdequateLeaderFixedGlobalBlockerPending(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank,
           snapshot, clockValue)
     /\ blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier
     /\ currentRank \in AdequateLeaderFixedGlobalBlockerRankCarrier
     /\ ~AdequateLeaderFixedGlobalBlockerStrictRankGoal(
          initialContext, target, leaderContext,
          leader, leaderView, receipt, sourceRank,
          snapshot, clockValue, blockerRank)
     /\ AdequateLeaderFixedGlobalBlockerProducerPrefix(currentRank)
          = AdequateLeaderFixedGlobalBlockerProducerPrefix(blockerRank)
     /\ episodeRank \in
          AdequateLeaderFixedGlobalProducerEpisodeRankCarrier
     /\ episodeRank =
          AdequateLeaderFixedGlobalProducerEpisodeRank(snapshot)

AdequateLeaderFixedGlobalProducerEpisodeOutcome(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank, episodeRank) ==
  \/ AdequateLeaderFixedGlobalBlockerStrictRankGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank,
       snapshot, clockValue, blockerRank)
  \/ \E lowerEpisodeRank
       \in SetLessThan(
            episodeRank,
            AdequateLeaderFixedGlobalProducerEpisodeRankOrdering,
            AdequateLeaderFixedGlobalProducerEpisodeRankCarrier):
       AdequateLeaderFixedGlobalProducerEpisodeAtRank(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, sourceRank,
         snapshot, clockValue, blockerRank, lowerEpisodeRank)

AdequateLeaderFixedGlobalBlockerOwnerReady(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank, episodeRank, owner) ==
  /\ owner \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext)
  /\ ENABLED
       (AdequateLeaderFixedSelectedServiceOwnerAction(owner)
          /\ AdequateLeaderFixedGlobalProducerEpisodeOutcome(
               initialContext, target, leaderContext,
               leader, leaderView, receipt, sourceRank,
               snapshot, clockValue, blockerRank, episodeRank)')

AdequateLeaderFixedSelectedGlobalBlockerOwner(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank, episodeRank) ==
  CHOOSE owner
    \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext):
      AdequateLeaderFixedGlobalBlockerOwnerReady(
        initialContext, target, leaderContext,
        leader, leaderView, receipt, sourceRank,
        snapshot, clockValue, blockerRank, episodeRank, owner)

AdequateLeaderFixedGlobalBlockerEntryProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
     serviceCellIdentity
       \in AdequateLeaderFixedPipelineServiceCellIdentityCarrier:
    /\ AdequateLeaderFixedPipelineServiceRankFrontierForCell(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, sourceRank, serviceCellIdentity)
    /\ ~AdequateLeaderFixedGlobalBlockerSelectionGoal(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, sourceRank, serviceCellIdentity)
    => LET clockValue == asyncNow
           snapshot ==
             AdequateLeaderFixedConfiguredGlobalBlockerSnapshot(
               clockValue, serviceCellIdentity)
           blockerRank ==
             AdequateLeaderFixedConcreteGlobalBlockerRank(
               snapshot, clockValue)
       IN AdequateLeaderFixedGlobalBlockerAtRank(
            initialContext, target, leaderContext,
            leader, leaderView, receipt, sourceRank,
            snapshot, clockValue, blockerRank)

AdequateLeaderFixedGlobalProducerEpisodeEntryProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
     snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
     clockValue \in Nat,
     blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier:
    AdequateLeaderFixedGlobalBlockerAtRank(
      initialContext, target, leaderContext,
      leader, leaderView, receipt, sourceRank,
      snapshot, clockValue, blockerRank)
      => \/ AdequateLeaderFixedGlobalBlockerStrictRankGoal(
               initialContext, target, leaderContext,
               leader, leaderView, receipt, sourceRank,
               snapshot, clockValue, blockerRank)
         \/ \E episodeRank
              \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier:
              AdequateLeaderFixedGlobalProducerEpisodeAtRank(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank,
                snapshot, clockValue, blockerRank, episodeRank)

AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
     snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
     clockValue \in Nat,
     blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier,
     episodeRank
       \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier:
    AdequateLeaderFixedGlobalProducerEpisodeAtRank(
      initialContext, target, leaderContext,
      leader, leaderView, receipt, sourceRank,
      snapshot, clockValue, blockerRank, episodeRank)
      => AdequateLeaderFixedGlobalBlockerOwnerReady(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank,
           snapshot, clockValue, blockerRank, episodeRank,
           AdequateLeaderFixedSelectedGlobalBlockerOwner(
             initialContext, target, leaderContext,
             leader, leaderView, receipt, sourceRank,
             snapshot, clockValue, blockerRank, episodeRank))

AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
     snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
     clockValue \in Nat,
     blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier,
     episodeRank
       \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier:
    LET owner ==
          AdequateLeaderFixedSelectedGlobalBlockerOwner(
            initialContext, target, leaderContext,
            leader, leaderView, receipt, sourceRank,
            snapshot, clockValue, blockerRank, episodeRank)
    IN /\ AdequateLeaderFixedGlobalProducerEpisodeAtRank(
             initialContext, target, leaderContext,
             leader, leaderView, receipt, sourceRank,
             snapshot, clockValue, blockerRank, episodeRank)
       /\ [AsyncNext]_AsyncAllVars
       => \/ AdequateLeaderFixedGlobalProducerEpisodeOutcome(
               initialContext, target, leaderContext,
               leader, leaderView, receipt, sourceRank,
               snapshot, clockValue, blockerRank, episodeRank)'
          \/ /\ (AdequateLeaderFixedGlobalProducerEpisodeAtRank(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt, sourceRank,
                   snapshot, clockValue, blockerRank, episodeRank))'
             /\ (AdequateLeaderFixedSelectedGlobalBlockerOwner(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt, sourceRank,
                   snapshot, clockValue, blockerRank, episodeRank))'
                  = owner
             /\ ~<<AdequateLeaderFixedSelectedServiceOwnerAction(
                       owner)>>_AsyncAllVars

\* A non-goal step may retain the exact producer rank or strictly lower it.
\* The frozen past scheduler cut, causal roots, and source journal cannot be
\* refreshed, so this is a true non-replenishment statement rather than a
\* bounded allowance for future Candidate or Serve ordinals.
AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
     snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
     clockValue \in Nat,
     blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier,
     episodeRank
       \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier:
    /\ AdequateLeaderFixedGlobalProducerEpisodeAtRank(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, sourceRank,
         snapshot, clockValue, blockerRank, episodeRank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~(AdequateLeaderFixedGlobalBlockerStrictRankGoal(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank,
           snapshot, clockValue, blockerRank))'
    => /\ (AdequateLeaderFixedGlobalBlockerSnapshotActive(
              snapshot, clockValue))'
       /\ AdequateLeaderFixedGlobalProducerEpisodeRank(snapshot)'
            \in
              {episodeRank}
                \cup
              SetLessThan(
                episodeRank,
                AdequateLeaderFixedGlobalProducerEpisodeRankOrdering,
                AdequateLeaderFixedGlobalProducerEpisodeRankCarrier)

\* The nested product bottom has no lower element.  Its selected exact fair
\* action must therefore reach selection or strict fixed-clock descent in the
\* same transition; there is no fabricated "last ordinal" token.
AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
     snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
     clockValue \in Nat,
     blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier:
    LET owner ==
          AdequateLeaderFixedSelectedGlobalBlockerOwner(
            initialContext, target, leaderContext,
            leader, leaderView, receipt, sourceRank,
            snapshot, clockValue, blockerRank,
            AdequateLeaderFixedGlobalProducerEpisodeBottom)
    IN /\ AdequateLeaderFixedGlobalProducerEpisodeAtRank(
            initialContext, target, leaderContext,
            leader, leaderView, receipt, sourceRank,
            snapshot, clockValue, blockerRank,
            AdequateLeaderFixedGlobalProducerEpisodeBottom)
       /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
              owner)>>_AsyncAllVars
       => (AdequateLeaderFixedGlobalBlockerStrictRankGoal(
             initialContext, target, leaderContext,
             leader, leaderView, receipt, sourceRank,
             snapshot, clockValue, blockerRank))'

AdequateLeaderFixedGlobalBlockerProviderProperty(specification) ==
  /\ (specification
        => [][(/\ AdequateLeaderFixedGlobalBlockerEntryProvider
               /\ AdequateLeaderFixedGlobalProducerEpisodeEntryProvider
               /\ AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider
               /\ AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider
               /\ AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider
               /\ AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider)]_AsyncAllVars)
  /\ AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty(
       specification)

AdequateLeaderFixedGlobalProducerEpisodeStepProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
          snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
          clockValue \in Nat,
          blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier,
          episodeRank
            \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier:
         AdequateLeaderFixedGlobalProducerEpisodeAtRank(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank,
           snapshot, clockValue, blockerRank, episodeRank)
           ~> AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank,
                snapshot, clockValue, blockerRank, episodeRank)

\* Freeze the selected blocker owner while weak fairness is applied.  The
\* selected-owner step provider preserves this equality on every non-owner
\* frame, so the temporal action below never depends on a state-varying
\* CHOOSE expression.
AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, snapshot, clockValue, blockerRank, episodeRank, owner) ==
  /\ AdequateLeaderFixedGlobalProducerEpisodeAtRank(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank,
       snapshot, clockValue, blockerRank, episodeRank)
  /\ AdequateLeaderFixedSelectedGlobalBlockerOwner(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank,
       snapshot, clockValue, blockerRank, episodeRank)
       = owner

THEOREM AdequateLeaderFixedGlobalBlockerProvidersSupplyProducerEpisodeStep ==
  \A specification:
    /\ AdequateLeaderAsyncNextBehaviorProperty(specification)
    /\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification)
    /\ AdequateLeaderFixedGlobalBlockerProviderProperty(specification)
      => AdequateLeaderFixedGlobalProducerEpisodeStepProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                AdequateLeaderAsyncNextBehaviorProperty(specification),
                AdequateLeaderFixedSelectedOwnerFairnessProperty(
                  specification),
                AdequateLeaderFixedGlobalBlockerProviderProperty(
                  specification),
                specification
         PROVE \A initialContext \in ContextRecords,
                   target \in ValidatorIds,
                   leaderContext \in ContextRecords,
                   leader \in ValidatorIds,
                   leaderView \in Views,
                   receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
                   sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
                   snapshot
                     \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
                   clockValue \in Nat,
                   blockerRank
                     \in AdequateLeaderFixedGlobalBlockerRankCarrier,
                   episodeRank
                     \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier:
                 AdequateLeaderFixedGlobalProducerEpisodeAtRank(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt, sourceRank,
                   snapshot, clockValue, blockerRank, episodeRank)
                   ~> AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt, sourceRank,
                        snapshot, clockValue, blockerRank, episodeRank)
    <2>1. ASSUME NEW initialContext \in ContextRecords,
                  NEW target \in ValidatorIds,
                  NEW leaderContext \in ContextRecords,
                  NEW leader \in ValidatorIds,
                  NEW leaderView \in Views,
                  NEW receipt
                    \in AdequateLeaderAuthorityDeadlineReceiptSet,
                  NEW sourceRank
                    \in AdequateLeaderFixedPipelineRankCarrier,
                  NEW snapshot
                    \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
                  NEW clockValue \in Nat,
                  NEW blockerRank
                    \in AdequateLeaderFixedGlobalBlockerRankCarrier,
                  NEW episodeRank
                    \in AdequateLeaderFixedGlobalProducerEpisodeRankCarrier
           PROVE AdequateLeaderFixedGlobalProducerEpisodeAtRank(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt, sourceRank,
                   snapshot, clockValue, blockerRank, episodeRank)
                   ~> AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt, sourceRank,
                        snapshot, clockValue, blockerRank, episodeRank)
      <3>1. \A owner
                  \in AdequateLeaderFixedSelectedServiceOwnerSet(
                       initialContext):
               AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt, sourceRank,
                 snapshot, clockValue, blockerRank, episodeRank, owner)
                 ~> AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                      initialContext, target, leaderContext,
                      leader, leaderView, receipt, sourceRank,
                      snapshot, clockValue, blockerRank, episodeRank)
        PROOF
          <4>1. ASSUME NEW owner
                        \in AdequateLeaderFixedSelectedServiceOwnerSet(
                             initialContext)
                 PROVE AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                         initialContext, target, leaderContext,
                         leader, leaderView, receipt, sourceRank,
                         snapshot, clockValue, blockerRank, episodeRank,
                         owner)
                         ~> AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                              initialContext, target, leaderContext,
                              leader, leaderView, receipt, sourceRank,
                              snapshot, clockValue, blockerRank, episodeRank)
            <5>1. [][AsyncNext]_AsyncAllVars
              BY <1>1
                 DEF AdequateLeaderAsyncNextBehaviorProperty
            <5>2. [](AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt, sourceRank,
                        snapshot, clockValue, blockerRank, episodeRank,
                        owner)
                      /\ ~AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                           initialContext, target, leaderContext,
                           leader, leaderView, receipt, sourceRank,
                           snapshot, clockValue, blockerRank, episodeRank)
                     => ENABLED
                          <<AdequateLeaderFixedSelectedServiceOwnerAction(
                              owner)>>_AsyncAllVars)
              BY <1>1, PTL, IsaT(900)
                 DEF AdequateLeaderFixedGlobalBlockerProviderProperty,
                     AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider,
                     AdequateLeaderFixedGlobalBlockerOwnerReady,
                     AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner,
                     AsyncAllVars
            <5>3. AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt, sourceRank,
                        snapshot, clockValue, blockerRank, episodeRank,
                        owner)
                      /\ ~AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                           initialContext, target, leaderContext,
                           leader, leaderView, receipt, sourceRank,
                           snapshot, clockValue, blockerRank, episodeRank)
                      /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
                               owner)>>_AsyncAllVars
                     => AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                          initialContext, target, leaderContext,
                          leader, leaderView, receipt, sourceRank,
                          snapshot, clockValue, blockerRank, episodeRank)'
              BY <1>1, <5>1, PTL, IsaT(900)
                 DEF AdequateLeaderFixedGlobalBlockerProviderProperty,
                     AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider,
                     AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner
            <5>4. AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt, sourceRank,
                        snapshot, clockValue, blockerRank, episodeRank,
                        owner)
                      /\ ~AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                           initialContext, target, leaderContext,
                           leader, leaderView, receipt, sourceRank,
                           snapshot, clockValue, blockerRank, episodeRank)
                      /\ [AsyncNext]_AsyncAllVars
                     => \/ AdequateLeaderFixedGlobalProducerEpisodeOutcome(
                              initialContext, target, leaderContext,
                              leader, leaderView, receipt, sourceRank,
                              snapshot, clockValue, blockerRank, episodeRank)'
                        \/ AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                             initialContext, target, leaderContext,
                             leader, leaderView, receipt, sourceRank,
                             snapshot, clockValue, blockerRank, episodeRank,
                             owner)'
              BY <1>1, PTL, IsaT(900)
                 DEF AdequateLeaderFixedGlobalBlockerProviderProperty,
                     AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider,
                     AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner
            <5>5. WF_AsyncAllVars(
                     AdequateLeaderFixedSelectedServiceOwnerAction(owner))
              BY <1>1, <4>1, PTL
                 DEF AdequateLeaderFixedSelectedOwnerFairnessProperty
            <5> QED BY <5>1, <5>2, <5>3, <5>4, <5>5, PTL
          <4> QED BY <4>1
      <3>2. [](AdequateLeaderFixedGlobalProducerEpisodeAtRank(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt, sourceRank,
                  snapshot, clockValue, blockerRank, episodeRank)
               => \E owner
                       \in AdequateLeaderFixedSelectedServiceOwnerSet(
                            initialContext):
                    AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner(
                      initialContext, target, leaderContext,
                      leader, leaderView, receipt, sourceRank,
                      snapshot, clockValue, blockerRank, episodeRank,
                      owner))
        BY <1>1, PTL, IsaT(900)
           DEF AdequateLeaderFixedGlobalBlockerProviderProperty,
               AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider,
               AdequateLeaderFixedGlobalBlockerOwnerReady,
               AdequateLeaderFixedGlobalProducerEpisodeAtRankForOwner
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1
     DEF AdequateLeaderFixedGlobalProducerEpisodeStepProperty

AdequateLeaderFixedGlobalBlockerRankStepProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
          snapshot \in AdequateLeaderFixedGlobalBlockerSnapshotCarrier,
          clockValue \in Nat,
          blockerRank \in AdequateLeaderFixedGlobalBlockerRankCarrier:
         AdequateLeaderFixedGlobalBlockerAtRank(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank,
           snapshot, clockValue, blockerRank)
           ~> AdequateLeaderFixedGlobalBlockerStrictRankGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank,
                snapshot, clockValue, blockerRank)

THEOREM AdequateLeaderFixedGlobalFiniteProducerEpisodeSuppliesRankStep ==
  \A specification:
    /\ AdequateLeaderFixedGlobalBlockerProviderProperty(specification)
    /\ AdequateLeaderFixedGlobalProducerEpisodeStepProperty(specification)
      => AdequateLeaderFixedGlobalBlockerRankStepProperty(specification)
BY ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded,
   ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank,
   WellFoundedLeadsTo, PTL
   DEF AdequateLeaderFixedGlobalBlockerProviderProperty,
       AdequateLeaderFixedGlobalProducerEpisodeEntryProvider,
       AdequateLeaderFixedGlobalProducerEpisodeStepProperty,
       AdequateLeaderFixedGlobalProducerEpisodeOutcome,
       AdequateLeaderFixedGlobalBlockerRankStepProperty

THEOREM AdequateLeaderFixedGlobalBlockerProvidersSupplyRankStep ==
  \A specification:
    /\ AdequateLeaderAsyncNextBehaviorProperty(specification)
    /\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification)
    /\ AdequateLeaderFixedGlobalBlockerProviderProperty(specification)
      => AdequateLeaderFixedGlobalBlockerRankStepProperty(specification)
BY AdequateLeaderFixedGlobalBlockerProvidersSupplyProducerEpisodeStep,
   AdequateLeaderFixedGlobalFiniteProducerEpisodeSuppliesRankStep

THEOREM AdequateLeaderFixedGlobalBlockerRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderFixedGlobalBlockerRankOrdering,
    AdequateLeaderFixedGlobalBlockerRankCarrier)
BY ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded
   DEF AdequateLeaderFixedGlobalBlockerRankOrdering,
       AdequateLeaderFixedGlobalBlockerRankCarrier

AdequateLeaderFixedGlobalBlockerSelectionClosureProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
          serviceCellIdentity
            \in AdequateLeaderFixedPipelineServiceCellIdentityCarrier:
         AdequateLeaderFixedPipelineServiceRankFrontierForCell(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank, serviceCellIdentity)
           ~> AdequateLeaderFixedGlobalBlockerSelectionGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank,
                serviceCellIdentity)

THEOREM AdequateLeaderFixedGlobalBlockerRankClosesOwnerSelection ==
  \A specification:
    /\ AdequateLeaderFixedGlobalBlockerProviderProperty(specification)
    /\ AdequateLeaderFixedGlobalBlockerRankStepProperty(specification)
      => AdequateLeaderFixedGlobalBlockerSelectionClosureProperty(
           specification)
BY AdequateLeaderFixedGlobalBlockerRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF AdequateLeaderFixedGlobalBlockerProviderProperty,
       AdequateLeaderFixedGlobalBlockerEntryProvider,
       AdequateLeaderFixedGlobalBlockerRankStepProperty,
       AdequateLeaderFixedGlobalBlockerSelectionClosureProperty,
       AdequateLeaderFixedGlobalBlockerSelectionGoal,
       AdequateLeaderFixedGlobalBlockerStrictRankGoal

\* A producer handoff initially exposes a raw pre-candidate rank cell, before
\* the global fixed-clock selector has chosen its concrete owner.  This
\* route-only rank keeps the original source rank as an immutable ceiling.
\* A lower route is therefore another finite rank obligation; a lower
\* candidate or the authority terminal is the original strict pipeline goal.
\* This auxiliary closure uses neither selected-candidate service nor the
\* aggregate pipeline closure, so it is below (and cannot circularly consume)
\* the non-descent episode proved from it.
AdequateLeaderFixedPreCandidateRawRouteRankFrontier(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, currentRank) ==
  /\ currentRank \in AdequateLeaderFixedPipelineRankCarrier
  /\ \/ currentRank = sourceRank
     \/ currentRank
          \in SetLessThan(
               sourceRank,
               AdequateLeaderFixedPipelineRankOrdering,
               AdequateLeaderFixedPipelineRankCarrier)
  /\ \E token
       \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
       \E route \in AdequateLeaderFixedPreCandidateRouteCarrier:
         \E entryDebt
              \in 1..AdequateLeaderFixedPerOriginSlotEpisodeCharge,
            owner
              \in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext),
            packet \in AsyncPacketSet:
           /\ AdequateLeaderFixedPreCandidateEntryRankCell(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                route, token, entryDebt, owner, packet)
           /\ AdequateLeaderFixedPreCandidateEntryRank(
                token, leaderContext, leader, leaderView, receipt.subject,
                entryDebt, owner, packet)
                = currentRank

AdequateLeaderFixedPreCandidateRawRouteRankDescentGoal(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    sourceRank, currentRank) ==
  \/ AdequateLeaderFixedPipelineStrictRankGoal(
       initialContext, target, leaderContext,
       leader, leaderView, receipt, sourceRank)
  \/ \E lowerRank
       \in SetLessThan(
            currentRank,
            AdequateLeaderFixedPipelineRankOrdering,
            AdequateLeaderFixedPipelineRankCarrier):
       AdequateLeaderFixedPreCandidateRawRouteRankFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, receipt, sourceRank, lowerRank)

AdequateLeaderFixedPreCandidateRawRouteRankStepProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank, currentRank
            \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedPreCandidateRawRouteRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank, currentRank)
           ~> AdequateLeaderFixedPreCandidateRawRouteRankDescentGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank, currentRank)

AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedPreCandidateRawRouteRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank, sourceRank)
           ~> AdequateLeaderFixedPipelineStrictRankGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank)

THEOREM
    AdequateLeaderFixedGlobalSelectionAndPreCandidateServiceSupplyRawRouteStep ==
  \A specification:
    /\ AdequateLeaderFixedGlobalBlockerSelectionClosureProperty(
         specification)
    /\ AdequateLeaderFixedPreCandidateEntryServiceProperty(specification)
      => AdequateLeaderFixedPreCandidateRawRouteRankStepProperty(
           specification)
BY AdequateLeaderFixedPipelineServiceRankFrontierHasExactCell,
   AdequateLeaderFixedSelectedExactCellProjectsToServiceFrontier,
   AdequateLeaderFixedStrictGoalsProjectToServiceRankDescent,
   PTL, SMT, Isa
   DEF AdequateLeaderFixedPreCandidateRawRouteRankStepProperty,
       AdequateLeaderFixedPreCandidateRawRouteRankFrontier,
       AdequateLeaderFixedPreCandidateRawRouteRankDescentGoal,
       AdequateLeaderFixedGlobalBlockerSelectionClosureProperty,
       AdequateLeaderFixedGlobalBlockerSelectionGoal,
       AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell,
       AdequateLeaderFixedSelectedPipelineServiceRankFrontier,
       AdequateLeaderFixedPreCandidateEntryServiceProperty,
       AdequateLeaderFixedPreCandidateEntryStrictRankGoal,
       AdequateLeaderFixedPipelineServiceRankDescentGoal,
       AdequateLeaderFixedPipelineServiceRankFrontier,
       AdequateLeaderFixedPipelineServiceRankFrontierForCell,
       AdequateLeaderFixedPipelineServiceCellIdentityCarrier,
       AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity,
       AdequateLeaderFixedPipelineStrictRankGoal,
       AdequateLeaderFixedPipelineRankOrdering,
       AdequateLeaderFixedLiveOriginSlotRankOrdering,
       AdequateLeaderFixedPerOriginSlotRankOrdering,
       LexPairOrdering, SetLessThan, OpToRel

THEOREM AdequateLeaderFixedPreCandidateRawRouteStepClosesRank ==
  \A specification:
    AdequateLeaderFixedPreCandidateRawRouteRankStepProperty(specification)
      => AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty(
           specification)
BY AdequateLeaderFixedPipelineRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF AdequateLeaderFixedPreCandidateRawRouteRankStepProperty,
       AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty,
       AdequateLeaderFixedPreCandidateRawRouteRankDescentGoal

THEOREM AdequateLeaderFixedPipelineProducerHandoffStartsRawRouteRank ==
  \A sourceDormantPotential, knownDormantPotential:
    \A initialContext, target, leaderContext, leader, leaderView, receipt,
       route, token, episodeTarget, sourceOccurrenceRank,
       sourceOccurrenceOwner, sourceCutoffOrdinal, known, sourceRank, budget:
      AdequateLeaderFixedPipelineProducerHandoffFrontier(
        initialContext, target, leaderContext,
        leader, leaderView, receipt,
        route, token, episodeTarget,
        sourceOccurrenceRank, sourceOccurrenceOwner,
        sourceCutoffOrdinal,
        sourceDormantPotential, knownDormantPotential,
        known, sourceRank, budget)
        => AdequateLeaderFixedPreCandidateRawRouteRankFrontier(
             initialContext, target, leaderContext,
             leader, leaderView, receipt, sourceRank, sourceRank)
BY Isa
   DEF AdequateLeaderFixedPipelineProducerHandoffFrontier,
       AdequateLeaderFixedPreCandidateRawRouteRankFrontier,
       AdequateLeaderFixedPreCandidateRouteCarrier,
       AdequateLeaderFixedAnyPipelineTokenCarrier

\* A candidate arm of the producer episode must retain every selected-cell
\* witness while the selected owner is waiting for its fair turn.  Freezing
\* these witnesses makes the fairness action constant; the producer-handoff
\* arm is closed independently by the raw-route rank below.
AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
    initialContext, target, leaderContext, leader, leaderView, receipt,
    token, episodeTarget, sourceOccurrenceRank, sourceOccurrenceOwner,
    sourceCutoffOrdinal, sourceDormantPotential, knownDormantPotential,
    known, sourceRank, budget,
    candidate, cutoffOrdinal, currentSemanticRank,
    currentOccurrenceRank, currentOccurrenceOwner,
    currentRank, owner, packet) ==
  /\ candidate.node = episodeTarget
  /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
       candidate, leaderContext, leader, leaderView, receipt.subject,
       currentSemanticRank, currentOccurrenceRank, currentOccurrenceOwner)
  /\ AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible(
       sourceOccurrenceRank, currentOccurrenceRank, sourceRank, currentRank)
  /\ AdequateLeaderFixedPipelineOriginEpisodeFrontier(
       initialContext, target, leaderContext,
       leader, leaderView, receipt,
       token, episodeTarget,
       sourceOccurrenceRank, sourceOccurrenceOwner,
       sourceCutoffOrdinal,
       sourceDormantPotential, knownDormantPotential,
       known, sourceRank, budget)
  /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
       initialContext, target, leaderContext,
       leader, leaderView, receipt,
       token, candidate, cutoffOrdinal, currentSemanticRank,
       owner, packet, currentRank)

THEOREM
    AdequateLeaderFixedCandidateFairnessAndRawRouteClosureSupplyEpisodeStep ==
  \A specification:
    /\ AdequateLeaderAsyncNextBehaviorProperty(specification)
    /\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification)
    /\ AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty(
         specification)
    /\ AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty(
         specification)
      => AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty(
           specification)
PROOF
  <1>1. ASSUME NEW specification,
                AdequateLeaderAsyncNextBehaviorProperty(specification),
                AdequateLeaderFixedSelectedOwnerFairnessProperty(
                  specification),
                AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty(
                  specification),
                AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty(
                  specification),
                specification
         PROVE \A sourceDormantPotential, knownDormantPotential:
                 \A initialContext \in ContextRecords:
                  \A target \in ValidatorIds:
                   \A leaderContext \in ContextRecords:
                    \A leader \in ValidatorIds:
                     \A leaderView \in Views:
                      \A receipt
                           \in AdequateLeaderAuthorityDeadlineReceiptSet:
                       \A token
                            \in AdequateLeaderFixedPipelineTokenCarrier(
                                 leaderContext):
                        \A episodeTarget \in ValidatorIds:
                         \A sourceOccurrenceRank
                              \in AdequateLeaderTargetOccurrenceRankCarrier:
                          \A sourceOccurrenceOwner
                               \in AdequateLeaderFrozenCandidateOwnerUniverse(
                                    episodeTarget, leaderContext,
                                    leader, leaderView, receipt.subject):
                           \A sourceCutoffOrdinal \in Nat \ {0}:
                            \A known
                                 \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                                      episodeTarget, leaderContext,
                                      leader, leaderView, receipt.subject):
                             \A sourceRank
                                  \in AdequateLeaderFixedPipelineRankCarrier:
                              \A budget
                                   \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier:
                   AdequateLeaderFixedPipelineOriginEpisodeFrontier(
                     initialContext, target, leaderContext,
                     leader, leaderView, receipt,
                     token, episodeTarget,
                     sourceOccurrenceRank, sourceOccurrenceOwner,
                     sourceCutoffOrdinal,
                     sourceDormantPotential, knownDormantPotential,
                     known, sourceRank, budget)
                     ~> AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                          initialContext, target, leaderContext,
                          leader, leaderView, receipt,
                          token, episodeTarget,
                          sourceOccurrenceRank, sourceOccurrenceOwner,
                          sourceCutoffOrdinal,
                          sourceDormantPotential, knownDormantPotential,
                          known, sourceRank, budget)
    <2>1. ASSUME NEW sourceDormantPotential,
                  NEW knownDormantPotential,
                  NEW initialContext \in ContextRecords,
                  NEW target \in ValidatorIds,
                  NEW leaderContext \in ContextRecords,
                  NEW leader \in ValidatorIds,
                  NEW leaderView \in Views,
                  NEW receipt
                    \in AdequateLeaderAuthorityDeadlineReceiptSet,
                  NEW token
                    \in AdequateLeaderFixedPipelineTokenCarrier(
                         leaderContext),
                  NEW episodeTarget \in ValidatorIds,
                  NEW sourceOccurrenceRank
                    \in AdequateLeaderTargetOccurrenceRankCarrier,
                  NEW sourceOccurrenceOwner
                    \in AdequateLeaderFrozenCandidateOwnerUniverse(
                         episodeTarget, leaderContext, leader, leaderView,
                         receipt.subject),
                  NEW sourceCutoffOrdinal \in Nat \ {0},
                  NEW known
                    \in SUBSET AdequateLeaderFrozenOwnerUniverse(
                         episodeTarget, leaderContext, leader, leaderView,
                         receipt.subject),
                  NEW sourceRank
                    \in AdequateLeaderFixedPipelineRankCarrier,
                  NEW budget
                    \in AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier
           PROVE AdequateLeaderFixedPipelineOriginEpisodeFrontier(
                   initialContext, target, leaderContext,
                   leader, leaderView, receipt,
                   token, episodeTarget,
                   sourceOccurrenceRank, sourceOccurrenceOwner,
                   sourceCutoffOrdinal,
                   sourceDormantPotential, knownDormantPotential,
                   known, sourceRank, budget)
                   ~> AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt,
                        token, episodeTarget,
                        sourceOccurrenceRank, sourceOccurrenceOwner,
                        sourceCutoffOrdinal,
                        sourceDormantPotential, knownDormantPotential,
                        known, sourceRank, budget)
      <3>1. \A candidate \in AsyncCandidateSet,
                    cutoffOrdinal \in Nat,
                    currentSemanticRank \in (1..4) \X (0..9),
                    currentOccurrenceRank
                      \in AdequateLeaderTargetOccurrenceRankCarrier,
                    currentOccurrenceOwner
                      \in AdequateLeaderFrozenCandidateOwnerUniverse(
                           episodeTarget, leaderContext, leader, leaderView,
                           receipt.subject),
                    currentRank
                      \in AdequateLeaderFixedPipelineRankCarrier,
                    owner
                      \in AdequateLeaderFixedSelectedServiceOwnerSet(
                           initialContext),
                    packet \in AsyncPacketSet:
               AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt,
                 token, episodeTarget,
                 sourceOccurrenceRank, sourceOccurrenceOwner,
                 sourceCutoffOrdinal,
                 sourceDormantPotential, knownDormantPotential,
                 known, sourceRank, budget,
                 candidate, cutoffOrdinal, currentSemanticRank,
                 currentOccurrenceRank, currentOccurrenceOwner,
                 currentRank, owner, packet)
                 ~> AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                      initialContext, target, leaderContext,
                      leader, leaderView, receipt,
                      token, episodeTarget,
                      sourceOccurrenceRank, sourceOccurrenceOwner,
                      sourceCutoffOrdinal,
                      sourceDormantPotential, knownDormantPotential,
                      known, sourceRank, budget)
        PROOF
          <4>1. ASSUME NEW candidate \in AsyncCandidateSet,
                        NEW cutoffOrdinal \in Nat,
                        NEW currentSemanticRank \in (1..4) \X (0..9),
                        NEW currentOccurrenceRank
                          \in AdequateLeaderTargetOccurrenceRankCarrier,
                        NEW currentOccurrenceOwner
                          \in AdequateLeaderFrozenCandidateOwnerUniverse(
                               episodeTarget, leaderContext, leader,
                               leaderView, receipt.subject),
                        NEW currentRank
                          \in AdequateLeaderFixedPipelineRankCarrier,
                        NEW owner
                          \in AdequateLeaderFixedSelectedServiceOwnerSet(
                               initialContext),
                        NEW packet \in AsyncPacketSet
                 PROVE AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                         initialContext, target, leaderContext,
                         leader, leaderView, receipt,
                         token, episodeTarget,
                         sourceOccurrenceRank, sourceOccurrenceOwner,
                         sourceCutoffOrdinal,
                         sourceDormantPotential, knownDormantPotential,
                         known, sourceRank, budget,
                         candidate, cutoffOrdinal, currentSemanticRank,
                         currentOccurrenceRank, currentOccurrenceOwner,
                         currentRank, owner, packet)
                         ~> AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                              initialContext, target, leaderContext,
                              leader, leaderView, receipt,
                              token, episodeTarget,
                              sourceOccurrenceRank, sourceOccurrenceOwner,
                              sourceCutoffOrdinal,
                              sourceDormantPotential, knownDormantPotential,
                              known, sourceRank, budget)
            <5>1. [][AsyncNext]_AsyncAllVars
              BY <1>1
                 DEF AdequateLeaderAsyncNextBehaviorProperty
            <5>2. [](AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt,
                        token, episodeTarget,
                        sourceOccurrenceRank, sourceOccurrenceOwner,
                        sourceCutoffOrdinal,
                        sourceDormantPotential, knownDormantPotential,
                        known, sourceRank, budget,
                        candidate, cutoffOrdinal, currentSemanticRank,
                        currentOccurrenceRank, currentOccurrenceOwner,
                        currentRank, owner, packet)
                      /\ ~AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                           initialContext, target, leaderContext,
                           leader, leaderView, receipt,
                           token, episodeTarget,
                           sourceOccurrenceRank, sourceOccurrenceOwner,
                           sourceCutoffOrdinal,
                           sourceDormantPotential, knownDormantPotential,
                           known, sourceRank, budget)
                     => ENABLED
                          <<AdequateLeaderFixedSelectedServiceOwnerAction(
                              owner)>>_AsyncAllVars)
              BY PTL, IsaT(1200)
                 DEF AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell,
                     AdequateLeaderFixedSelectedPipelineRankFrontier,
                     AdequateLeaderFixedScheduledOwnerReadyForRankCell,
                     AsyncAllVars
            <5>3. AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt,
                        token, episodeTarget,
                        sourceOccurrenceRank, sourceOccurrenceOwner,
                        sourceCutoffOrdinal,
                        sourceDormantPotential, knownDormantPotential,
                        known, sourceRank, budget,
                        candidate, cutoffOrdinal, currentSemanticRank,
                        currentOccurrenceRank, currentOccurrenceOwner,
                        currentRank, owner, packet)
                      /\ ~AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                           initialContext, target, leaderContext,
                           leader, leaderView, receipt,
                           token, episodeTarget,
                           sourceOccurrenceRank, sourceOccurrenceOwner,
                           sourceCutoffOrdinal,
                           sourceDormantPotential, knownDormantPotential,
                           known, sourceRank, budget)
                      /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
                               owner)>>_AsyncAllVars
                     => AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                          initialContext, target, leaderContext,
                          leader, leaderView, receipt,
                          token, episodeTarget,
                          sourceOccurrenceRank, sourceOccurrenceOwner,
                          sourceCutoffOrdinal,
                          sourceDormantPotential, knownDormantPotential,
                          known, sourceRank, budget)'
              BY <1>1, <5>1, PTL, IsaT(1200)
                 DEF AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty,
                     AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProvider,
                     AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell
            <5>4. AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                        initialContext, target, leaderContext,
                        leader, leaderView, receipt,
                        token, episodeTarget,
                        sourceOccurrenceRank, sourceOccurrenceOwner,
                        sourceCutoffOrdinal,
                        sourceDormantPotential, knownDormantPotential,
                        known, sourceRank, budget,
                        candidate, cutoffOrdinal, currentSemanticRank,
                        currentOccurrenceRank, currentOccurrenceOwner,
                        currentRank, owner, packet)
                      /\ ~AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                           initialContext, target, leaderContext,
                           leader, leaderView, receipt,
                           token, episodeTarget,
                           sourceOccurrenceRank, sourceOccurrenceOwner,
                           sourceCutoffOrdinal,
                           sourceDormantPotential, knownDormantPotential,
                           known, sourceRank, budget)
                      /\ [AsyncNext]_AsyncAllVars
                     => \/ AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                              initialContext, target, leaderContext,
                              leader, leaderView, receipt,
                              token, episodeTarget,
                              sourceOccurrenceRank, sourceOccurrenceOwner,
                              sourceCutoffOrdinal,
                              sourceDormantPotential, knownDormantPotential,
                              known, sourceRank, budget)'
                        \/ AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                             initialContext, target, leaderContext,
                             leader, leaderView, receipt,
                             token, episodeTarget,
                             sourceOccurrenceRank, sourceOccurrenceOwner,
                             sourceCutoffOrdinal,
                             sourceDormantPotential, knownDormantPotential,
                             known, sourceRank, budget,
                             candidate, cutoffOrdinal, currentSemanticRank,
                             currentOccurrenceRank, currentOccurrenceOwner,
                             currentRank, owner, packet)'
              BY <1>1, PTL, IsaT(1200)
                 DEF AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty,
                     AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProvider,
                     AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell
            <5>5. WF_AsyncAllVars(
                     AdequateLeaderFixedSelectedServiceOwnerAction(owner))
              BY <1>1, <4>1, PTL
                 DEF AdequateLeaderFixedSelectedOwnerFairnessProperty
            <5> QED BY <5>1, <5>2, <5>3, <5>4, <5>5, PTL
          <4> QED BY <4>1
      <3>2. \A route:
               AdequateLeaderFixedPipelineProducerHandoffFrontier(
                 initialContext, target, leaderContext,
                 leader, leaderView, receipt,
                 route, token, episodeTarget,
                 sourceOccurrenceRank, sourceOccurrenceOwner,
                 sourceCutoffOrdinal,
                 sourceDormantPotential, knownDormantPotential,
                 known, sourceRank, budget)
                 ~> AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal(
                      initialContext, target, leaderContext,
                      leader, leaderView, receipt,
                      token, episodeTarget,
                      sourceOccurrenceRank, sourceOccurrenceOwner,
                      sourceCutoffOrdinal,
                      sourceDormantPotential, knownDormantPotential,
                      known, sourceRank, budget)
        BY <1>1,
           AdequateLeaderFixedPipelineProducerHandoffStartsRawRouteRank,
           PTL
           DEF AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty,
               AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal
      <3>3. [](AdequateLeaderFixedPipelineOriginEpisodeFrontier(
                  initialContext, target, leaderContext,
                  leader, leaderView, receipt,
                  token, episodeTarget,
                  sourceOccurrenceRank, sourceOccurrenceOwner,
                  sourceCutoffOrdinal,
                  sourceDormantPotential, knownDormantPotential,
                  known, sourceRank, budget)
               => \/ \E candidate \in AsyncCandidateSet,
                         cutoffOrdinal \in Nat,
                         currentSemanticRank \in (1..4) \X (0..9),
                         currentOccurrenceRank
                           \in AdequateLeaderTargetOccurrenceRankCarrier,
                         currentOccurrenceOwner
                           \in AdequateLeaderFrozenCandidateOwnerUniverse(
                                episodeTarget, leaderContext, leader,
                                leaderView, receipt.subject),
                         currentRank
                           \in AdequateLeaderFixedPipelineRankCarrier,
                         owner
                           \in AdequateLeaderFixedSelectedServiceOwnerSet(
                                initialContext),
                         packet \in AsyncPacketSet:
                       AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell(
                         initialContext, target, leaderContext,
                         leader, leaderView, receipt,
                         token, episodeTarget,
                         sourceOccurrenceRank, sourceOccurrenceOwner,
                         sourceCutoffOrdinal,
                         sourceDormantPotential, knownDormantPotential,
                         known, sourceRank, budget,
                         candidate, cutoffOrdinal, currentSemanticRank,
                         currentOccurrenceRank, currentOccurrenceOwner,
                         currentRank, owner, packet)
                  \/ \E route:
                       AdequateLeaderFixedPipelineProducerHandoffFrontier(
                         initialContext, target, leaderContext,
                         leader, leaderView, receipt,
                         route, token, episodeTarget,
                         sourceOccurrenceRank, sourceOccurrenceOwner,
                         sourceCutoffOrdinal,
                         sourceDormantPotential, knownDormantPotential,
                         known, sourceRank, budget))
        BY PTL, IsaT(900)
           DEF AdequateLeaderFixedPipelineOriginEpisodeFrontier,
               AdequateLeaderFixedPipelineOriginEpisodeAtSelectedCell
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1
     DEF AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty

AdequateLeaderFixedPipelineServiceRankDescentProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedPipelineServiceRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank)
           ~> AdequateLeaderFixedPipelineServiceRankDescentGoal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt, sourceRank)

THEOREM AdequateLeaderFixedGlobalSelectionAndSelectedServiceSupplyPipelineRankDescent ==
  \A specification:
    /\ AdequateLeaderFixedGlobalBlockerSelectionClosureProperty(
         specification)
    /\ AdequateLeaderFixedSelectedPipelineServiceRankDescentProperty(
         specification)
      => AdequateLeaderFixedPipelineServiceRankDescentProperty(specification)
BY AdequateLeaderFixedPipelineServiceRankFrontierHasExactCell,
   AdequateLeaderFixedSelectedExactCellProjectsToServiceFrontier,
   PTL
   DEF AdequateLeaderFixedGlobalBlockerSelectionClosureProperty,
       AdequateLeaderFixedGlobalBlockerSelectionGoal,
       AdequateLeaderFixedSelectedPipelineServiceRankDescentProperty,
       AdequateLeaderFixedPipelineServiceRankDescentProperty

AdequateLeaderFixedPipelineServiceRankClosureProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
         AdequateLeaderFixedPipelineServiceRankFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, receipt, sourceRank)
           ~> AdequateLeaderFixedCutTerminalForAuthority(
                target, leaderContext, leader, leaderView, receipt)

THEOREM AdequateLeaderFixedPipelineRankDescentClosesService ==
  \A specification:
    AdequateLeaderFixedPipelineServiceRankDescentProperty(specification)
      => AdequateLeaderFixedPipelineServiceRankClosureProperty(specification)
BY AdequateLeaderFixedPipelineRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF AdequateLeaderFixedPipelineServiceRankDescentProperty,
       AdequateLeaderFixedPipelineServiceRankClosureProperty,
       AdequateLeaderFixedPipelineServiceRankDescentGoal

(***************************************************************************
The lower action lemmas required to discharge the provider are intentionally
listed at the proof boundary:

  * runner/due-mode selection:
      `HistoricalDiscoveryFixedClockBlockerCharacterization`,
      `CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner`,
      `DueNodeServiceEnablesConcreteGateProgress`, and
      `ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock`;
  * deferred and selector preservation:
      `AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut` and
      `ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock`;
  * I/O/ready service:
      `DueIoServiceEnablesConcreteLocalProgress`,
      `HistoricalDiscoveryServeFairActionLowersOccurrenceDebt`, and
      `ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets`;
  * producer/parent-child handoff:
      `AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut`,
      `AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt`,
      `ExactDecisionTargetNeutralSnapshotProducerEpisodeDoesNotReplenish`, and
      `AsyncCandidateProducerContinuationHandoffCandidatesThisStep`;
  * selected-action temporal occurrence:
      `ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness`,
      `ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame`,
      and the local selected-owner step providers below.

This is the same action shape as
`SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs`: non-Tick actions
freeze `asyncNow`; Tick lowers the selected service slack; a due node, I/O,
or packet owner disables Tick until its individually fair action consumes the
physical rank.  The historical theorem itself cannot discharge this leaf:
its source is a historical-recovery request and its terminal is a round
timeout, not this frozen leader/context/view/subject/receipt.

The remaining action proof is the exhaustive projection from those lower
classifications to `AdequateLeaderFixedCutPerActionProvider`, the global
blocker provider, and then `AdequateLeaderFixedSelectedOwnerServiceProperty`.
The immediate-entry, token-tail, and absolute-deadline carry seams named below
must also be discharged.  Static credit bounds are not substituted for any of
those projections.
***************************************************************************)

(***************************************************************************
Qualitative exact Decision-delivery retention.

This theorem is intentionally outside the quantitative bundle.  It is the
existing exact source/target carrier used by responsive dissemination; it
does not assign a clock charge to any of the physical delivery stages.
***************************************************************************)

AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProvider ==
  \A source, target \in AsyncCurrentResponsiveVoters,
     qc \in QcRecordSet:
    DecisionSourceAt(source, qc)
      => \/ NodeHasDecision(target)
         \/ /\ TimeoutDecisionKernelSource(source, target, qc)
            /\ CommitCertificateDelivery(source, target, qc)

AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProviderProperty(
    specification) ==
  specification
    => []AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProvider

THEOREM AsyncLiveProvidesAdequateLeaderFixedDecisionSourceTargetDeliveryRetention ==
  \A initialContext:
    AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveDecisionAuthorityRetainsExactTargetDelivery, PTL
   DEF AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProviderProperty,
       AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProvider

(***************************************************************************
Pre-admission subject-replacement transport.

Production retains an exact route/message owner before receiver admission but
does not assign that owner a recipient-global scheduler ordinal.  The frozen
`knownRoutes` parameter below is therefore a set of route-neutral identities,
not hypothetical future lifecycle positions.  A route leaves the finite
episode only when its exact local acceptance receives an ordinal strictly
after the already-admitted target, or when the exact control identity is
durably serviced/advanced.  Exact retransmission cannot enlarge
`knownRoutes`, and no existential known set can lower the budget in place.
***************************************************************************)

AdequateLeaderFixedPreAdmissionSubjectReplacementRouteResolved(
    route, target, leaderContext, leader, leaderView, targetOrdinal) ==
  /\ route
       \in AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentitySet
  /\ \/ \E owner
          \in AdequateLeaderFixedLiveSubjectReplacementOwners(
               target, leaderContext, leader, leaderView):
          /\ owner.origin = route.origin
          /\ targetOrdinal < owner.ordinal
     \/ AsyncControlServiceIdentityServicedOrAdvanced(
          AdequateLeaderFixedOriginRootItem(route.origin))

AdequateLeaderFixedPreAdmissionSubjectReplacementRemainingRoutes(
    knownRoutes, target, leaderContext, leader, leaderView,
    targetOrdinal) ==
  {route \in knownRoutes:
     ~AdequateLeaderFixedPreAdmissionSubjectReplacementRouteResolved(
        route, target, leaderContext, leader, leaderView, targetOrdinal)}

AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode(
    target, leaderContext, leader, leaderView,
    targetOrdinal, schedulerCeiling, knownRoutes, budget) ==
  LET remaining ==
        AdequateLeaderFixedPreAdmissionSubjectReplacementRemainingRoutes(
          knownRoutes, target, leaderContext, leader, leaderView,
          targetOrdinal)
  IN /\ knownRoutes
          \in SUBSET
               AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentitySet
     /\ IsFiniteSet(knownRoutes)
     /\ Cardinality(knownRoutes)
          <= AdequateLeaderFixedPreAdmissionSubjectReplacementRouteCapacity
     /\ targetOrdinal \in Nat \ {0}
     /\ schedulerCeiling \in Nat \ {0}
     /\ targetOrdinal < schedulerCeiling
     /\ schedulerCeiling
          <= AsyncNextCandidateLifecycleOrdinal(leader)
     /\ budget = Cardinality(remaining)
     /\ budget \in Nat
     /\ remaining
          \subseteq
            AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes(
              target, leaderContext, leader, leaderView)
     /\ \A owner
          \in AdequateLeaderFixedLiveSubjectReplacementOwners(
               target, leaderContext, leader, leaderView):
          owner.origin \in {route.origin: route \in knownRoutes}
            => targetOrdinal < owner.ordinal

AdequateLeaderFixedPreAdmissionSubjectReplacementBudgetDescentGoal(
    target, leaderContext, leader, leaderView,
    targetOrdinal, schedulerCeiling, knownRoutes, sourceBudget) ==
  \/ \A route \in knownRoutes:
       AdequateLeaderFixedPreAdmissionSubjectReplacementRouteResolved(
         route, target, leaderContext, leader, leaderView, targetOrdinal)
  \/ \E lowerBudget
       \in SetLessThan(sourceBudget, OpToRel(<, Nat), Nat):
       AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode(
         target, leaderContext, leader, leaderView,
         targetOrdinal, schedulerCeiling, knownRoutes, lowerBudget)

\* Retained retransmission is an ordinary source-owned fair action.  It emits
\* the exact immutable PrepareQC item and leaves the route-neutral identity
\* unchanged; the new packet's time fields are deliberately absent here.
\* This theorem does not grant the retained item a scheduler ordinal.  Only
\* the later atomic local admission action can mint or recover that ordinal.
THEOREM AdequateLeaderFixedRetainedSubjectReplacementRetryKeepsExactRoute ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     item \in asyncRetainedControl:
    LET route ==
          AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentity(
            item, target, leaderContext, leader, leaderView)
        packet == PacketForItem(item)
    IN /\ item.source
             \in AdequateLeaderFrozenResponsiveRoster(leaderContext)
       /\ AdequateLeaderFixedSubjectReplacementOrigin(
            AsyncLeaderWireLifecycleCausalOriginAt(item, leaderContext),
            target, leaderContext, leader, leaderView)
       /\ UNCHANGED vars
       /\ SendNodeRetransmissions(item.source)
       => /\ route
                \in
                  (AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes(
                     target, leaderContext, leader, leaderView))'
          /\ packet \in asyncTransport'
          /\ packet.item = item
          /\ packet.transportIdentity = route.routeIdentity
          /\ AsyncLeaderWireServiceIdentity(packet.item)
               = route.routeIdentity
BY PacketForItemExactRetryRetainsRouteIdentity, IsaT(600)
   DEF AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentity,
       AdequateLeaderFixedSubjectReplacementOrigin,
       SendNodeRetransmissions, RetryableItems,
       RetainedControlEmissionItems, SendableItems,
       PacketsForItems, PacketForItem,
       AsyncTransportRouteIdentity

AdequateLeaderFixedPreAdmissionSubjectReplacementStepProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          targetOrdinal, schedulerCeiling \in Nat \ {0},
          knownRoutes
            \in SUBSET
                 AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentitySet,
          budget \in Nat:
         AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode(
           target, leaderContext, leader, leaderView,
           targetOrdinal, schedulerCeiling, knownRoutes, budget)
           ~> AdequateLeaderFixedPreAdmissionSubjectReplacementBudgetDescentGoal(
                target, leaderContext, leader, leaderView,
                targetOrdinal, schedulerCeiling, knownRoutes, budget)

THEOREM AsyncLiveProvidesAdequateLeaderFixedPreAdmissionSubjectReplacementStep ==
  \A initialContext:
    AdequateLeaderFixedPreAdmissionSubjectReplacementStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AdequateLeaderFixedRetainedSubjectReplacementRetryKeepsExactRoute,
   ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness,
   ExactDecisionRequestIngressRankOrderingIsWellFounded,
   AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence,
   AsyncLiveProvidesExactLeaderCandidateSemanticHandoffs,
   AsyncLiveProvidesCandidateProducerContinuationFrozenPrefixClosure,
   StarvationFreedomObligation,
   AdmitHiddenLeaderWireIsAtomicLocalAcceptanceCut,
   AdmitFreshLeaderWireFreezesCurrentLocalSchedulerOrdinal,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier,
   AdmitDormantLeaderWireAppendsAfterExistingServeCarrier,
   PTL, IsaT(9000)
   DEF AdequateLeaderFixedPreAdmissionSubjectReplacementStepProperty,
       AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode,
       AdequateLeaderFixedPreAdmissionSubjectReplacementBudgetDescentGoal,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRemainingRoutes,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRouteResolved,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentity,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRouteCapacity,
       AdequateLeaderFixedLiveSubjectReplacementOwners,
       AdequateLeaderFixedWireSubjectReplacementOwners,
       AdequateLeaderFixedActiveWireSubjectReplacementOwners,
       AdequateLeaderFixedProducerSubjectReplacementOwners,
       AdequateLeaderFixedSubjectReplacementOwnerServiced,
       AdequateLeaderFixedSubjectReplacementOwnerIdentity,
       AdequateLeaderFixedSubjectReplacementOrigin,
       AdequateLeaderFixedOriginRootItem,
       AdequateLeaderWirePhysicalConvergenceProperty,
       ExactLeaderCandidateSemanticHandoffProperty,
       AsyncCandidateProducerContinuationFrozenPrefixClosureProperty,
       StarvationFreedomProperty,
       AsyncLiveSpecAt, AsyncFairnessAt

AdequateLeaderFixedPreAdmissionSubjectReplacementClosureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          targetOrdinal, schedulerCeiling \in Nat \ {0},
          knownRoutes
            \in SUBSET
                 AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentitySet,
          budget \in Nat:
         AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode(
           target, leaderContext, leader, leaderView,
           targetOrdinal, schedulerCeiling, knownRoutes, budget)
           ~> (\A route \in knownRoutes:
                AdequateLeaderFixedPreAdmissionSubjectReplacementRouteResolved(
                  route, target, leaderContext,
                  leader, leaderView, targetOrdinal))

THEOREM AdequateLeaderFixedPreAdmissionBudgetClosesWithoutOrdinalRecreation ==
  \A specification:
    AdequateLeaderFixedPreAdmissionSubjectReplacementStepProperty(
      specification)
      => AdequateLeaderFixedPreAdmissionSubjectReplacementClosureProperty(
           specification)
BY NatLessThanWellFounded, WellFoundedLeadsTo
   DEF AdequateLeaderFixedPreAdmissionSubjectReplacementStepProperty,
       AdequateLeaderFixedPreAdmissionSubjectReplacementClosureProperty,
       AdequateLeaderFixedPreAdmissionSubjectReplacementBudgetDescentGoal

(***************************************************************************
Finite source-frozen admitted subject-replacement episode.

The natural rank is exactly the unserviced active predecessor set captured
before the physical cutoff.  A parked Dormant identity receives no fair
service obligation and cannot enter this set by replay: atomic reactivation
reserves a fresh carrier after the frozen cutoff.  Its immutable lifecycle
identity remains available only to retry coalescing and the separate finite
semantic producer episode.
***************************************************************************)

AdequateLeaderFixedSubjectReplacementSelectedPredecessor(
    cut, knownPotential) ==
  LET remaining ==
        AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors(
          cut, knownPotential)
  IN CHOOSE owner \in remaining:
       \A other \in remaining: owner.ordinal <= other.ordinal

AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
    initialContext, target, leaderContext, leader, leaderView,
    sourceReceipt, cut, knownPotential, episodeRank) ==
  LET live ==
        AdequateLeaderFixedLiveSubjectReplacementOwners(
          target, leaderContext, leader, leaderView)
      serviced ==
        AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners(
          cut, knownPotential)
      remaining ==
        AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors(
          cut, knownPotential)
  IN /\ cut \in AdequateLeaderFixedSubjectReplacementCutSet
     /\ sourceReceipt \in AdequateLeaderAuthorityDeadlineReceiptSet
     /\ cut.target = target
     /\ cut.context = leaderContext
     /\ cut.leader = leader
     /\ cut.view = leaderView
     /\ cut.sourceSubject = sourceReceipt.subject
     /\ target = leader
     /\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive(
          target, leaderContext, leader, leaderView, sourceReceipt)
     /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
     /\ IsFiniteSet(cut.owners)
     /\ Cardinality(cut.owners)
          <= AdequateLeaderFixedSubjectReplacementOwnerCapacity
     /\ cut.targetOwner \in cut.owners
     /\ cut.predecessorOwners = cut.owners \ {cut.targetOwner}
     /\ AdequateLeaderFixedSubjectReplacementEpisodeDebtAtRank(
          cut, knownPotential, episodeRank)
     /\ remaining \subseteq live
     /\ cut.targetOwner \in live
     /\ serviced \cap live = {}
     /\ \A later
          \in live
                \ (AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners(
                     cut, knownPotential)
                    \cup {cut.targetOwner}):
          \/ cut.sourceTargetOrdinal < later.ordinal
          \/ cut.physicalCutoffOrdinal <= later.carrierOrdinal
     /\ AsyncCausalEpisodeFrozenPredecessorOrigins(
          leader, cut.targetOwner.ordinal)
          \subseteq cut.predecessorOrigins
     /\ cut.physicalCutoffOrdinal
          <= AsyncNextIngressPhysicalOrdinal(leader)
     /\ cut.schedulerCeiling
          <= AsyncNextCandidateLifecycleOrdinal(leader)
     /\ asyncNow
          <= sourceReceipt.deadlineReceipt.armedAt
               + AdequateLeaderFixedLeaderPipelineClockBudget

\* A fresh source can begin at any surviving protocol phase; Proposal may
\* already be complete and drained.  Entry therefore names the current raw
\* scheduled/pre-candidate cell for an exact remaining token (or the Decision
\* terminal) and never recreates `<<leader, "Proposal">>` at maximal debt.
\* Concrete owner selection is delayed until the separately ranked global
\* fixed-clock blocker episode has drained.
AdequateLeaderAuthorityDeadlineImmediateSourceEntryProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
    AdequateLeaderAuthorityDeadlineFreshSource(
      target, leaderContext, leader, leaderView, receipt)
      /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
      => \/ AdequateLeaderAuthorityDeadlineTargetDecision(target, receipt)
         \/ \E sourceRank
                \in AdequateLeaderFixedPipelineRankCarrier,
               targetOrdinal \in Nat \ {0}:
              LET schedulerCeiling ==
                    AsyncNextCandidateLifecycleOrdinal(leader)
                  preAdmissionRoutes ==
                    AdequateLeaderFixedUnacceptedPreAdmissionSubjectReplacementRoutes(
                      target, leaderContext, leader, leaderView)
                  replacementOwners ==
                    AdequateLeaderFixedSubjectReplacementOwnersBeforeOrdinal(
                      target, leaderContext, leader, leaderView,
                      targetOrdinal)
              IN /\ AdequateLeaderFixedFreshPipelineServiceRankFrontierAtOrdinal(
                       initialContext, target, leaderContext,
                       leader, leaderView, receipt,
                       sourceRank, targetOrdinal)
                 /\ targetOrdinal < schedulerCeiling
                 /\ IsFiniteSet(preAdmissionRoutes)
                 /\ Cardinality(preAdmissionRoutes)
                      <=
                        AdequateLeaderFixedPreAdmissionSubjectReplacementRouteCapacity
                 /\ \E transportBudget \in Nat:
                      AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode(
                        target, leaderContext, leader, leaderView,
                        targetOrdinal, schedulerCeiling,
                        preAdmissionRoutes, transportBudget)
                 /\ IF replacementOwners = {}
                    THEN TRUE
                    ELSE \E cut
                           \in AdequateLeaderFixedSubjectReplacementCutSet,
                             replacementRank
                               \in
                                 AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
                           /\ AdequateLeaderFixedSubjectReplacementCutSource(
                                target, leaderContext, leader, leaderView,
                                receipt.subject, targetOrdinal, cut)
                           /\ replacementRank =
                                AdequateLeaderFixedSubjectReplacementInitialEpisodeRank(
                                  cut)
                           /\ AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
                                initialContext, target, leaderContext,
                                leader, leaderView, receipt,
                                cut, {}, replacementRank)

AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty(
    specification) ==
  specification
    => []AdequateLeaderAuthorityDeadlineImmediateSourceEntryProvider

AdequateLeaderFixedSubjectReplacementTerminalGoal(
    initialContext, target, leaderContext, leader, leaderView,
    sourceReceipt, cut) ==
  \/ AdequateLeaderFixedCutTerminalForAuthority(
       target, leaderContext, leader, leaderView, sourceReceipt)
  \/ \E knownPotential
         \in SUBSET
              AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
       sourceRank \in AdequateLeaderFixedPipelineRankCarrier:
       AdequateLeaderFixedAnchoredSubjectPipelineServiceRankFrontier(
         initialContext, target, leaderContext,
         leader, leaderView, sourceReceipt, cut,
         knownPotential, sourceRank)

AdequateLeaderFixedSubjectReplacementBudgetDescentGoal(
    initialContext, target, leaderContext, leader, leaderView,
    sourceReceipt, cut, knownPotential, sourceRank) ==
  \/ AdequateLeaderFixedSubjectReplacementTerminalGoal(
       initialContext, target, leaderContext,
       leader, leaderView, sourceReceipt, cut)
  \/ \E discoveredPotential,
       knownPotential2
         \in SUBSET
              AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
       lowerRank
         \in SetLessThan(
              sourceRank,
              AdequateLeaderFixedSubjectReplacementEpisodeRankOrdering,
              AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier):
       /\ discoveredPotential =
            AdequateLeaderFixedSubjectReplacementPotentialDiscovered(
              cut, knownPotential)
       /\ knownPotential2 = knownPotential \cup discoveredPotential
       /\ AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
            initialContext, target, leaderContext,
            leader, leaderView, sourceReceipt, cut,
            knownPotential2, lowerRank)

\* The lower proof must carry the immutable logical and physical cut,
\* monotone serviced subtraction, same-origin child, shared scheduler
\* high-watermark, and the original absolute receipt ceiling.  A fresh
\* in-flight packet has only the pre-admission route identity above and cannot
\* appear in this ordinal set until its local FairV2Ingress acceptance.  A
\* Dormant replay accepted after the source has a carrier at/after
\* `physicalCutoffOrdinal` and therefore cannot recharge this natural rank.
AdequateLeaderFixedSubjectReplacementCutCarryProvider ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     sourceReceipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
     cut \in AdequateLeaderFixedSubjectReplacementCutSet,
     knownPotential
       \in SUBSET AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
     sourceRank
       \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
    LET serviced ==
          AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners(
            cut, knownPotential)
    IN /\ AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
             initialContext, target, leaderContext,
             leader, leaderView, sourceReceipt, cut,
             knownPotential, sourceRank)
       /\ [AsyncNext]_AsyncAllVars
       => \/ AdequateLeaderFixedCutTerminalForAuthority(
               target, leaderContext, leader, leaderView, sourceReceipt)'
          \/ (AdequateLeaderFixedSubjectReplacementBudgetDescentGoal(
                initialContext, target, leaderContext,
                leader, leaderView, sourceReceipt, cut,
                knownPotential, sourceRank))'
          \/ /\ serviced
                  \subseteq
                    (AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners(
                       cut, knownPotential))'
             /\ serviced
                  \cap
                    (AdequateLeaderFixedLiveSubjectReplacementOwners(
                       target, leaderContext, leader, leaderView))'
                    = {}
             /\ \A later
                  \in
                    (AdequateLeaderFixedLiveSubjectReplacementOwners(
                       target, leaderContext, leader, leaderView))'
                      \ (AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners(
                           cut, knownPotential)
                         \cup {cut.targetOwner}):
                  \/ cut.sourceTargetOrdinal < later.ordinal
                  \/ cut.physicalCutoffOrdinal <= later.carrierOrdinal
             /\ (AsyncCausalEpisodeFrozenPredecessorOrigins(
                    leader, cut.targetOwner.ordinal))'
                  \subseteq cut.predecessorOrigins
             /\ cut.schedulerCeiling
                  <= AsyncNextCandidateLifecycleOrdinal(leader)'
             /\ (AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
                    initialContext, target, leaderContext,
                    leader, leaderView, sourceReceipt, cut,
                    knownPotential, sourceRank))'

AdequateLeaderFixedSubjectReplacementCutCarryProviderProperty(
    specification) ==
  specification
    => [][AdequateLeaderFixedSubjectReplacementCutCarryProvider]_AsyncAllVars

\* Exhaustive AsyncNext projection for the immutable subject cut.  Dormant
\* admission retains the logical token but reserves a fresh physical carrier
\* after the frozen cutoff, so it cannot enter the predecessor rank.  Fresh
\* lifecycle, Candidate, Serve, Control, Completion, and priority allocations
\* likewise remain after that cutoff.  A terminal wire/producer record is
\* monotone service memory, not a replacement live owner.
THEOREM AdequateLeaderFixedSubjectReplacementCutFollowsAsyncStep ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    => AdequateLeaderFixedSubjectReplacementCutCarryProvider
BY PostGstStepCannotCreateDormantLeaderWirePotential,
   PostGstLeaderWireLifecycleRestartIsDisabled,
   AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   AdmitDormantLeaderWirePreservesLogicalPotentialPredecessors,
   DormantLeaderWireOwnsNoPhysicalIngressPredecessor,
   AdmitDormantLeaderWireAppendsAfterExistingServeCarrier,
   DormantLeaderWirePhysicalOrdinalExhaustionPublishesNothing,
   CoalescedDueLeaderWireLifecycleRetryPreservesFrozenOwner,
   AsyncFreshLeaderWireAdmissionProjectionFollowsRetainedOwners,
   AsyncLeaderWireAdmissionPrecedesSameStepCandidateAllocation,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   AsyncCandidateProducerContinuationTerminalRecordIsFixed,
   AsyncCandidateProducerContinuationReplacementRetiresOnlyTerminal,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   FS_Subset, FS_CardinalityType, SMT, IsaT(12000)
   DEF AdequateLeaderFixedSubjectReplacementCutCarryProvider,
       AdequateLeaderFixedSubjectReplacementEpisodeFrontier,
       AdequateLeaderFixedSubjectReplacementBudgetDescentGoal,
       AdequateLeaderFixedSubjectReplacementTerminalGoal,
       AdequateLeaderFixedSubjectReplacementEpisodeDebtAtRank,
       AdequateLeaderFixedSubjectReplacementPotentialKnownExact,
       AdequateLeaderFixedSubjectReplacementPotentialDiscovered,
       AdequateLeaderFixedSubjectReplacementBlockingPotentialOwners,
       AdequateLeaderFixedSubjectReplacementCurrentBlockingPotentialOwners,
       AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners,
       AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners,
       AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors,
       AdequateLeaderFixedSubjectReplacementOwnerServiced,
       AdequateLeaderFixedLiveSubjectReplacementOwners,
       AdequateLeaderFixedWireSubjectReplacementOwners,
       AdequateLeaderFixedActiveWireSubjectReplacementOwners,
       AdequateLeaderFixedPotentialSubjectReplacementOwners,
       AdequateLeaderFixedDormantWireSubjectReplacementOwners,
       AdequateLeaderFixedProducerSubjectReplacementOwners,
       AdequateLeaderFixedSubjectReplacementOwnerIdentity,
       AdequateLeaderFixedSubjectReplacementOrigin,
       AdequateLeaderAuthorityDeadlineFreshSelfWindowActive,
       AdequateLeaderFixedCutTerminalForAuthority,
       AdequateLeaderFrozenTargetCorridor,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleTransition,
       AsyncCandidateProducerContinuations,
       AsyncNext, AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementCutCarry ==
  \A initialContext:
    AdequateLeaderFixedSubjectReplacementCutCarryProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderFixedSubjectReplacementCutFollowsAsyncStep,
   PTL
   DEF AdequateLeaderFixedSubjectReplacementCutCarryProviderProperty

AdequateLeaderFixedSubjectReplacementSelectedOwnerServiceProperty(
    specification) ==
  specification
    => \A initialContext \in ContextRecords:
         \A target \in ValidatorIds:
           \A leaderContext \in ContextRecords:
             \A leader \in ValidatorIds:
               \A leaderView \in Views:
                 \A sourceReceipt
                       \in AdequateLeaderAuthorityDeadlineReceiptSet:
                   \A cut \in AdequateLeaderFixedSubjectReplacementCutSet:
                     \A knownPotential
                           \in SUBSET
                                AdequateLeaderFixedSubjectReplacementOwnerIdentitySet:
                       \A sourceRank
                             \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
                         LET selected ==
                               AdequateLeaderFixedSubjectReplacementSelectedPredecessor(
                                 cut, knownPotential)
                         IN /\ AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
                                  initialContext, target, leaderContext,
                                  leader, leaderView, sourceReceipt, cut,
                                  knownPotential, sourceRank)
                            /\ sourceRank > 0
                            /\ selected
                                 \in
                                   AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors(
                                     cut, knownPotential)
                              ~> AdequateLeaderFixedSubjectReplacementBudgetDescentGoal(
                                   initialContext, target, leaderContext,
                                   leader, leaderView, sourceReceipt, cut,
                                   knownPotential, sourceRank)

\* Membership in the live replacement set always exposes an existing physical
\* carrier with the same immutable causal origin and scheduler ordinal.  This
\* is a state decomposition only; it grants no fairness to a Dormant record.
THEOREM AdequateLeaderFixedLiveSubjectReplacementOwnerHasExactCarrier ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views:
    \A owner
          \in AdequateLeaderFixedLiveSubjectReplacementOwners(
               target, leaderContext, leader, leaderView):
      \/ \E wireRecord \in asyncLeaderWireLifecycles:
           /\ AsyncLeaderWireLifecycleActive(wireRecord)
           /\ wireRecord.recipient = owner.node
           /\ wireRecord.causalOrigin = owner.origin
           /\ wireRecord.schedulerOrdinal = owner.ordinal
           /\ wireRecord.physicalAdmissionOrdinal = owner.carrierOrdinal
           /\ AdequateLeaderFixedSubjectReplacementOrigin(
                wireRecord.causalOrigin,
                target, leaderContext, leader, leaderView)
       \/ \E producerRecord \in AsyncCandidateProducerContinuations:
           /\ producerRecord.status \in {"Reserved", "Materialized"}
           /\ producerRecord.node = owner.node
           /\ producerRecord.causalOrigin = owner.origin
           /\ producerRecord.ordinal = owner.ordinal
           /\ \E lifecycle \in asyncLeaderWireLifecycles:
                /\ lifecycle.recipient = owner.node
                /\ lifecycle.causalOrigin = owner.origin
                /\ lifecycle.schedulerOrdinal = owner.ordinal
                /\ lifecycle.physicalAdmissionOrdinal =
                     owner.carrierOrdinal
           /\ AdequateLeaderFixedSubjectReplacementOrigin(
                producerRecord.causalOrigin,
                target, leaderContext, leader, leaderView)
BY Isa
   DEF AdequateLeaderFixedLiveSubjectReplacementOwners,
       AdequateLeaderFixedWireSubjectReplacementOwners,
       AdequateLeaderFixedActiveWireSubjectReplacementOwners,
       AdequateLeaderFixedProducerSubjectReplacementOwners,
       AdequateLeaderFixedSubjectReplacementOwnerIdentity

THEOREM AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementSelectedOwnerService ==
  \A initialContext:
    AdequateLeaderFixedSubjectReplacementSelectedOwnerServiceProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementCutCarry,
   AdequateLeaderFixedLiveSubjectReplacementOwnerHasExactCarrier,
   AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence,
   AsyncLiveProvidesExactLeaderCandidateSemanticHandoffs,
   AsyncLiveProvidesCandidateProducerContinuationFrozenPrefixClosure,
   AsyncLiveProvidesAdequateLeaderTargetSelectedOwnerReservedContinuationStep,
   AsyncLiveProvidesAdequateLeaderTargetSelectedOwnerMaterializedContinuationStep,
   AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalOutcome,
   StarvationFreedomObligation,
   LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   AsyncCandidateProducerContinuationTerminalRecordIsFixed,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   PTL, IsaT(12000)
   DEF AdequateLeaderFixedSubjectReplacementSelectedOwnerServiceProperty,
       AdequateLeaderFixedSubjectReplacementSelectedPredecessor,
       AdequateLeaderFixedSubjectReplacementEpisodeFrontier,
       AdequateLeaderFixedSubjectReplacementBudgetDescentGoal,
       AdequateLeaderFixedSubjectReplacementTerminalGoal,
       AdequateLeaderFixedSubjectReplacementEpisodeDebtAtRank,
       AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners,
       AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners,
       AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors,
       AdequateLeaderFixedSubjectReplacementOwnerServiced,
       AdequateLeaderFixedSubjectReplacementOwnerIdentity,
       AdequateLeaderFixedLiveSubjectReplacementOwners,
       AdequateLeaderFixedWireSubjectReplacementOwners,
       AdequateLeaderFixedActiveWireSubjectReplacementOwners,
       AdequateLeaderFixedProducerSubjectReplacementOwners,
       AdequateLeaderFixedSubjectReplacementOrigin,
       AdequateLeaderFixedOriginRootItem,
       AdequateLeaderWirePhysicalConvergenceProperty,
       ExactLeaderCandidateSemanticHandoffProperty,
       AsyncCandidateProducerContinuationFrozenPrefixClosureProperty,
       AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerPhysicalOutcomeProperty,
       StarvationFreedomProperty,
       AsyncLiveSpecAt, AsyncFairnessAt

AdequateLeaderFixedSubjectReplacementTargetHandoffProperty(
    specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          sourceReceipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          cut \in AdequateLeaderFixedSubjectReplacementCutSet,
          knownPotential
            \in SUBSET
                 AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
          sourceRank
            \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
         /\ AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
              initialContext, target, leaderContext,
              leader, leaderView, sourceReceipt, cut,
              knownPotential, sourceRank)
         /\ sourceRank = 0
         /\ ~AdequateLeaderFixedSubjectReplacementOwnerServiced(
               cut.targetOwner)
           ~> AdequateLeaderFixedSubjectReplacementBudgetDescentGoal(
                initialContext, target, leaderContext,
                leader, leaderView, sourceReceipt, cut,
                knownPotential, sourceRank)

THEOREM AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementTargetHandoff ==
  \A initialContext:
    AdequateLeaderFixedSubjectReplacementTargetHandoffProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementCutCarry,
   AdequateLeaderFixedLiveSubjectReplacementOwnerHasExactCarrier,
   AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence,
   AsyncLiveProvidesExactLeaderCandidateSemanticHandoffs,
   AsyncLiveProvidesCandidateProducerContinuationFrozenPrefixClosure,
   AsyncLiveProvidesAdequateLeaderTargetSelectedOwnerReservedContinuationStep,
   AsyncLiveProvidesAdequateLeaderTargetSelectedOwnerMaterializedContinuationStep,
   AsyncLiveProvidesAdequateLeaderSelectedOwnerPhysicalOutcome,
   StarvationFreedomObligation,
   AdequateLeaderFixedPreCandidateReservedTailFitsConfiguredCharge,
   AdequateLeaderFixedCandidatePhysicalWindowFitsConfiguredBudget,
   AdequateLeaderFixedPipelineOriginSlotCarrierFitsConfiguredTail,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncLeaderWireAdmissionPrecedesSameStepCandidateAllocation,
   LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   AsyncCandidateProducerContinuationTerminalRecordIsFixed,
   PTL, IsaT(15000)
   DEF AdequateLeaderFixedSubjectReplacementTargetHandoffProperty,
       AdequateLeaderFixedSubjectReplacementEpisodeFrontier,
       AdequateLeaderFixedSubjectReplacementBudgetDescentGoal,
       AdequateLeaderFixedSubjectReplacementTerminalGoal,
       AdequateLeaderFixedSubjectReplacementEpisodeDebtAtRank,
       AdequateLeaderFixedSubjectReplacementEpisodePredecessorOwners,
       AdequateLeaderFixedSubjectReplacementEpisodeServicedOwners,
       AdequateLeaderFixedSubjectReplacementEpisodeRemainingPredecessors,
       AdequateLeaderFixedSubjectReplacementOwnerServiced,
       AdequateLeaderFixedLiveSubjectReplacementOwners,
       AdequateLeaderFixedWireSubjectReplacementOwners,
       AdequateLeaderFixedActiveWireSubjectReplacementOwners,
       AdequateLeaderFixedProducerSubjectReplacementOwners,
       AdequateLeaderFixedSubjectReplacementOrigin,
       AdequateLeaderFixedOriginRootItem,
       AdequateLeaderFixedAnchoredSubjectPipelineServiceRankFrontier,
       AdequateLeaderFixedSubjectReplacementReceipt,
       AdequateLeaderFixedPreCandidateRouteCarriesSubjectReplacementOrigin,
       AdequateLeaderFixedPreCandidateEntryRankCell,
       AdequateLeaderFixedPreCandidateEntryRank,
       AdequateLeaderFixedPreCandidateRouteStageCandidate,
       AdequateLeaderFixedPreCandidateRouteStageIsUnique,
       AdequateLeaderFixedWirePreCandidateRoute,
       AdequateLeaderFixedProducerPreCandidateRoute,
       AdequateLeaderFixedPipelineRankCell,
       AdequateLeaderFixedPipelineRank,
       AdequateLeaderWirePhysicalConvergenceProperty,
       ExactLeaderCandidateSemanticHandoffProperty,
       AsyncCandidateProducerContinuationFrozenPrefixClosureProperty,
       AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerPhysicalOutcomeProperty,
       StarvationFreedomProperty,
       AsyncLiveSpecAt, AsyncFairnessAt

\* Exact conjunction consumed by the fresh-self bundle.  Keeping this operator
\* equal to the five individual providers makes the aggregate a convenience,
\* not a stronger premise or a weakened release boundary.
AdequateLeaderFixedSubjectReplacementProviderProperties(specification) ==
  /\ AdequateLeaderFixedPreAdmissionSubjectReplacementStepProperty(
       specification)
  /\ AdequateLeaderFixedSubjectReplacementOwnerConfiguredBoundProperty(
       specification)
  /\ AdequateLeaderFixedSubjectReplacementCutCarryProviderProperty(
       specification)
  /\ AdequateLeaderFixedSubjectReplacementSelectedOwnerServiceProperty(
       specification)
  /\ AdequateLeaderFixedSubjectReplacementTargetHandoffProperty(
       specification)

THEOREM AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementProviders ==
  \A initialContext:
    AdequateLeaderFixedSubjectReplacementProviderProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderFixedPreAdmissionSubjectReplacementStep,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementOwnerConfiguredBound,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementCutCarry,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementSelectedOwnerService,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementTargetHandoff
   DEF AdequateLeaderFixedSubjectReplacementProviderProperties

AdequateLeaderFixedSubjectReplacementBudgetDescentProperty(
    specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          sourceReceipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          cut \in AdequateLeaderFixedSubjectReplacementCutSet,
          knownPotential
            \in SUBSET
                 AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
          sourceRank
            \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
         AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, sourceReceipt, cut,
           knownPotential, sourceRank)
           ~> AdequateLeaderFixedSubjectReplacementBudgetDescentGoal(
                initialContext, target, leaderContext,
                leader, leaderView, sourceReceipt, cut,
                knownPotential, sourceRank)

THEOREM AdequateLeaderFixedSubjectReplacementServicesSupplyBudgetDescent ==
  \A specification:
    /\ AdequateLeaderFixedSubjectReplacementSelectedOwnerServiceProperty(
         specification)
    /\ AdequateLeaderFixedSubjectReplacementTargetHandoffProperty(
         specification)
      => AdequateLeaderFixedSubjectReplacementBudgetDescentProperty(
           specification)
BY PTL, Isa
   DEF AdequateLeaderFixedSubjectReplacementSelectedOwnerServiceProperty,
       AdequateLeaderFixedSubjectReplacementTargetHandoffProperty,
       AdequateLeaderFixedSubjectReplacementBudgetDescentProperty,
       AdequateLeaderFixedSubjectReplacementBudgetDescentGoal

AdequateLeaderFixedSubjectReplacementClosureProperty(specification) ==
  specification
    => \A initialContext \in ContextRecords,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          sourceReceipt \in AdequateLeaderAuthorityDeadlineReceiptSet,
          cut \in AdequateLeaderFixedSubjectReplacementCutSet,
          knownPotential
            \in SUBSET
                 AdequateLeaderFixedSubjectReplacementOwnerIdentitySet,
          sourceRank
            \in AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
         AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
           initialContext, target, leaderContext,
           leader, leaderView, sourceReceipt, cut,
           knownPotential, sourceRank)
           ~> AdequateLeaderFixedSubjectReplacementTerminalGoal(
                initialContext, target, leaderContext,
                leader, leaderView, sourceReceipt, cut)

THEOREM AdequateLeaderFixedSubjectReplacementBudgetDescentClosesEpisode ==
  \A specification:
    AdequateLeaderFixedSubjectReplacementBudgetDescentProperty(
      specification)
      => AdequateLeaderFixedSubjectReplacementClosureProperty(specification)
BY AdequateLeaderFixedSubjectReplacementEpisodeRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF AdequateLeaderFixedSubjectReplacementBudgetDescentProperty,
       AdequateLeaderFixedSubjectReplacementClosureProperty,
       AdequateLeaderFixedSubjectReplacementBudgetDescentGoal,
       AdequateLeaderFixedSubjectReplacementEpisodeRankOrdering,
       AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier

(***************************************************************************
Fresh-self quantitative composition.
***************************************************************************)

AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryGoal(
    target, leaderContext, leader, leaderView, receipt) ==
  \/ AdequateLeaderFixedCutTerminalForAuthority(
       target, leaderContext, leader, leaderView, receipt)
  \/ \E initialContext \in ContextRecords,
        sourceRank \in AdequateLeaderFixedPipelineRankCarrier,
        targetOrdinal \in Nat \ {0}:
       LET replacementOwners ==
             AdequateLeaderFixedSubjectReplacementOwnersBeforeOrdinal(
               target, leaderContext, leader, leaderView,
               targetOrdinal)
       IN /\ AdequateLeaderFixedFreshPipelineServiceRankFrontierAtOrdinal(
                initialContext, target, leaderContext,
                leader, leaderView, receipt,
                sourceRank, targetOrdinal)
          /\ IF replacementOwners = {}
             THEN TRUE
             ELSE \E cut
                    \in AdequateLeaderFixedSubjectReplacementCutSet,
                      replacementRank
                        \in
                          AdequateLeaderFixedSubjectReplacementEpisodeRankCarrier:
                    /\ AdequateLeaderFixedSubjectReplacementCutSource(
                         target, leaderContext, leader, leaderView,
                         receipt.subject, targetOrdinal, cut)
                    /\ replacementRank =
                         AdequateLeaderFixedSubjectReplacementInitialEpisodeRank(
                           cut)
                    /\ AdequateLeaderFixedSubjectReplacementEpisodeFrontier(
                         initialContext, target, leaderContext,
                         leader, leaderView, receipt,
                         cut, {}, replacementRank)

AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
         AdequateLeaderAuthorityDeadlineFreshSource(
           target, leaderContext, leader, leaderView, receipt)
           ~> AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryGoal(
                target, leaderContext, leader, leaderView, receipt)

AdequateLeaderAuthorityDeadlineFreshSelfRankEntryProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
         AdequateLeaderAuthorityDeadlineFreshSource(
           target, leaderContext, leader, leaderView, receipt)
           ~> (AdequateLeaderFixedCutTerminalForAuthority(
                 target, leaderContext, leader, leaderView, receipt)
                \/ \E initialContext \in ContextRecords,
                      fixedReceipt
                        \in AdequateLeaderAuthorityDeadlineReceiptSet,
                      sourceRank
                        \in AdequateLeaderFixedPipelineRankCarrier:
                     /\ fixedReceipt.deadlineReceipt
                          = receipt.deadlineReceipt
                     /\ AdequateLeaderFixedPipelineServiceRankFrontier(
                          initialContext, target, leaderContext,
                          leader, leaderView, fixedReceipt, sourceRank))

THEOREM AdequateLeaderAuthorityDeadlineImmediateEntryStartsSubjectEpisode ==
  \A specification:
    AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty(
      specification)
      => AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryProperty(
           specification)
BY PTL, Isa
   DEF AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty,
       AdequateLeaderAuthorityDeadlineImmediateSourceEntryProvider,
       AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryProperty,
       AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryGoal,
       AdequateLeaderFixedPipelineServiceRankFrontier,
       AdequateLeaderFixedFreshPipelineServiceRankFrontierAtOrdinal,
       AdequateLeaderFixedPreCandidateEntryRank,
       AdequateLeaderFixedPreCandidateEntryRankCell

THEOREM AdequateLeaderAuthorityDeadlineSubjectEpisodeStartsFreshSelfRank ==
  \A specification:
    /\ AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryProperty(
         specification)
    /\ AdequateLeaderFixedSubjectReplacementClosureProperty(specification)
      => AdequateLeaderAuthorityDeadlineFreshSelfRankEntryProperty(
           specification)
BY PTL, Isa
   DEF AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryProperty,
       AdequateLeaderAuthorityDeadlineFreshSubjectReplacementEntryGoal,
       AdequateLeaderFixedSubjectReplacementClosureProperty,
       AdequateLeaderFixedSubjectReplacementTerminalGoal,
       AdequateLeaderFixedAnchoredSubjectPipelineServiceRankFrontier,
       AdequateLeaderFixedSubjectReplacementReceipt,
       AdequateLeaderAuthorityDeadlineFreshSelfRankEntryProperty,
       AdequateLeaderFixedFreshPipelineServiceRankFrontierAtOrdinal,
       AdequateLeaderFixedPipelineServiceRankFrontier

AdequateLeaderAuthorityDeadlineFreshSelfPipelineServiceProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
         AdequateLeaderAuthorityDeadlineFreshSource(
           target, leaderContext, leader, leaderView, receipt)
           ~> AdequateLeaderAuthorityDeadlineTargetDecision(
                target, receipt)

THEOREM AdequateLeaderAuthorityDeadlineFreshRankClosesSelfDecision ==
  \A specification:
    /\ ModelConfiguration
    /\ AdequateLeaderFixedConfiguredDeadlineCompatibility
    /\ AdequateLeaderAuthorityDeadlineFreshSelfRankEntryProperty(
         specification)
    /\ AdequateLeaderFixedPipelineServiceRankClosureProperty(specification)
    /\ AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty(
         specification)
      => AdequateLeaderAuthorityDeadlineFreshSelfPipelineServiceProperty(
           specification)
BY AdequateLeaderFreshReceiptOwnsCompatibleConfiguredCeiling,
   AdequateLeaderDecisionSourceIsItsNodeDecision,
   SMT, PTL, Isa
   DEF AdequateLeaderAuthorityDeadlineFreshSelfRankEntryProperty,
       AdequateLeaderFixedPipelineServiceRankClosureProperty,
       AdequateLeaderFixedCutTerminalForAuthority,
       AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty,
       AdequateLeaderAuthorityDeadlineFreshSelfPipelineServiceProperty,
       AdequateLeaderAuthorityDeadlineFreshSource,
       AdequateLeaderAuthorityDeadlineTargetDecision,
       AdequateLeaderAuthorityDeadlineLeaderDecisionSource,
       AdequateLeaderAuthorityDeadlineLeaderPipelineCorridorExit,
       AdequateLeaderAuthorityDeadlineStrictCorridorExit

(***************************************************************************
Reachable AsyncLive suppliers for the fresh-self safety/action seams.

These lemmas project the already-proved Async invariants and concrete
`AsyncNext` classifications.  They introduce no temporal fairness: the
selected-action predicates below are safety statements about an action which
has already occurred.  Fair occurrence remains confined to the separately
proved selected-owner and finite producer-prefix closures.
***************************************************************************)

\* Finite lifecycle admissions, their injective ordinary-slot allocation, and
\* the shared ordinal high-watermark determine both the exact protocol slot
\* projection and one maximal cutoff for every nonempty same-token node set.
THEOREM AdequateLeaderFixedLifecycleTablesSupplyPipelineTokenOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    => AdequateLeaderFixedPipelineTokenOwnershipAndTailCarry
BY AsyncCandidateLifecycleReviewedBucketsPartitionRecords,
   AsyncCandidateLifecycleActiveRecordsInjectIntoPhysicalOwners,
   AsyncCandidateLifecycleReviewedBucketsImplyPerNodeCapacity,
   AsyncCandidateLifecycleSlotInjectionBoundsGlobalOwners,
   AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin,
   AdequateLeaderFixedPipelineOriginSlotCarrierFitsConfiguredTail,
   FS_Image, FS_Subset, FS_CardinalityType, IsaT(7200)
   DEF AdequateLeaderFixedPipelineTokenOwnershipAndTailCarry,
       AdequateLeaderFixedLivePipelineOriginPairs,
       AdequateLeaderFixedDiscoveredPipelineOriginPairs,
       AdequateLeaderFixedLivePipelineOriginsForToken,
       AdequateLeaderFixedDiscoveredPipelineOriginsForToken,
       AdequateLeaderFixedLivePipelineOriginSlotsForToken,
       AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken,
       AdequateLeaderFixedLivePipelineOriginsForTokenAtNode,
       AdequateLeaderFixedPipelineTokenNodeCutoff,
       AdequateLeaderFixedAuthorityPipelineRemainingTokens,
       AdequateLeaderFixedOriginProtocolToken,
       AdequateLeaderFixedOriginProtocolSlot,
       AdequateLeaderFixedOriginProtocolPhase,
       AdequateLeaderFixedOriginProtocolPeer,
       AdequateLeaderFixedOriginIsExactPipelineEpisode,
       AdequateLeaderFixedPipelineOriginSlotCarrier,
       AsyncCandidateLifecycleActiveOriginsForNodeIn,
       AsyncCandidateLifecycleOrdinaryOriginsForNodeIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncCandidateLifecycleOrdinaryRecordBucketIn,
       AsyncCandidateLifecycleActiveSlots,
       AsyncCandidateLifecycleReviewedCapacityInvariantIn,
       AsyncCandidateLifecycleSlotInjectionInvariantIn,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncStrongTypeInvariant

THEOREM AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry ==
  \A initialContext:
    AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderFixedLifecycleTablesSupplyPipelineTokenOwnership,
   PTL
   DEF AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty

\* Within a frozen GST corridor, ordinary lifecycle history cannot compact a
\* current-view exact origin.  Candidate/Serve/Control terminal memory and
\* producer-continuation high-watermarks reject the only transitions which
\* could otherwise recreate a drained token.
THEOREM AdequateLeaderFixedPipelineOriginHistoryFollowsAsyncStep ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    => AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider
BY AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncCandidateSuccessfulServiceInstallsTombstone,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
   AsyncCandidateInternalBodyAvailableStageRetirementIsMonotoneAtGst,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalTombstonePersistsWithoutExit,
   AsyncCandidateSameHeightRestartPreservesTombstone,
   AsyncControlServiceExactRetryCoalesces,
   AsyncControlServiceServicedIdentityCannotResurrect,
   AsyncControlServiceTombstoneCannotReactivate,
   AsyncCandidateProducerContinuationExactRetryCoalesces,
   AsyncCandidateProducerContinuationHighWatermarkBlocksOldStage,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   AsyncCandidateProducerContinuationTerminalRecordIsFixed,
   PostGstLeaderWireLifecycleRestartIsDisabled,
   AdequateLeaderTerminalWireLifecycleCannotReactivate,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   FS_Subset, IsaT(9600)
   DEF AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider,
       AdequateLeaderFixedAuthorityPipelineRemainingTokens,
       AdequateLeaderFixedAuthorityPipelineTokenCompleted,
       AdequateLeaderFixedLivePipelineOriginPairs,
       AdequateLeaderFixedDiscoveredPipelineOriginPairs,
       AdequateLeaderFixedLivePipelineOriginsForToken,
       AdequateLeaderFixedDiscoveredPipelineOriginsForToken,
       AdequateLeaderFixedOriginIsExactPipelineEpisode,
       AdequateLeaderFixedOriginProtocolToken,
       AsyncCandidateLifecycleActiveOriginsForNodeIn,
       AsyncCandidateLifecycleOrdinaryOriginsForNodeIn,
       AsyncCandidateLifecycleRecordsForNodeIn,
       AsyncCandidateLifecycleOrdinaryRecordBucketIn,
       AsyncCandidateLifecycleActiveSlots,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AdequateLeaderFrozenTargetCorridor,
       AsyncNext, AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory ==
  \A initialContext:
    AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderFixedPipelineOriginHistoryFollowsAsyncStep,
   PTL
   DEF
     AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty

\* The source decomposition is immediate state evidence: the fresh
\* synchronized self corridor exposes its deterministic productive subject,
\* and the exact scheduler/wire/continuation lifecycle already owns a shared
\* ordinal strictly below the current high-watermark.  Finite pre-admission
\* routes and any earlier replacement owners are frozen at that same point.
THEOREM AdequateLeaderAuthorityDeadlineFreshSourceHasImmediateEntry ==
  \A initialContext \in ContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AdequateLeaderFixedPipelineTokenOwnershipAndTailCarry
    /\ AdequateLeaderAuthorityDeadlineFreshSource(
         target, leaderContext, leader, leaderView, receipt)
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
      => AdequateLeaderAuthorityDeadlineImmediateSourceEntryProvider
BY AdequateLeaderFreshSelfCorridorOpensOriginalTarget,
   AdequateLeaderScheduledProducerOriginUsesBoundedLifecycleToken,
   AdequateLeaderProducerOriginSourceNamesExactReceiptOrPhysicalTerminal,
   AdequateLeaderProducerOriginReceiptExposesExactReplacement,
   AdequateLeaderFixedPerTokenDebtFitsConfiguredCharge,
   AdequateLeaderFixedPreCandidateReservedTailFitsConfiguredCharge,
   AdequateLeaderFixedPipelineClockPotentialFitsConfiguredBudget,
   AdequateLeaderFixedSubjectReplacementOwnersFitConfiguredTables,
   AdequateLeaderFixedSubjectReplacementInitialEpisodeRankIsFinite,
   AsyncCandidateSchedulerCoverageExposesBoundedProducerOrigin,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncLeaderWireLifecycleSlotUniverseIsFinite,
   FS_Image, FS_Union, FS_Subset, FS_CardinalityType, IsaT(15000)
   DEF AdequateLeaderAuthorityDeadlineImmediateSourceEntryProvider,
       AdequateLeaderAuthorityDeadlineFreshSource,
       AdequateLeaderAuthorityDeadlineTargetDecision,
       AdequateLeaderFixedFreshPipelineServiceRankFrontierAtOrdinal,
       AdequateLeaderFixedFreshPreCandidateEntryRankCell,
       AdequateLeaderFixedPreCandidateEntryRankCell,
       AdequateLeaderFixedPreCandidateEntryRank,
       AdequateLeaderFixedPreCandidateRouteStageIsUnique,
       AdequateLeaderFixedPreCandidateRouteStageSet,
       AdequateLeaderFixedPreCandidateRouteStageCandidate,
       AdequateLeaderFixedPreCandidateRouteTyped,
       AdequateLeaderFixedPreCandidateRouteIdentityCarrier,
       AdequateLeaderFixedLocalPreCandidateRoute,
       AdequateLeaderFixedWirePreCandidateRoute,
       AdequateLeaderFixedProducerPreCandidateRoute,
       AdequateLeaderFixedPreAdmissionSubjectReplacementEpisode,
       AdequateLeaderFixedUnacceptedPreAdmissionSubjectReplacementRoutes,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes,
       AdequateLeaderFixedPreAdmissionSubjectReplacementRouteIdentity,
       AdequateLeaderFixedSubjectReplacementOwnersBeforeOrdinal,
       AdequateLeaderFixedSubjectReplacementCutSource,
       AdequateLeaderFixedSubjectReplacementEpisodeFrontier,
       AdequateLeaderFixedSubjectReplacementEpisodeDebtAtRank,
       AdequateLeaderFixedSubjectReplacementInitialEpisodeRank,
       AdequateLeaderTargetProductiveSubjectOpenFrontier,
       AdequateLeaderTargetOpenFrontier,
       AdequateLeaderTargetProtocolSubjectSource,
       AdequateLeaderTargetProducerTransportResidual,
       AdequateLeaderTargetProducerResidual,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncStrongTypeInvariant

THEOREM AsyncLiveProvidesAdequateLeaderAuthorityDeadlineImmediateSourceEntry ==
  \A initialContext:
    AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry,
   AdequateLeaderAuthorityDeadlineFreshSourceHasImmediateEntry,
   PTL
   DEF AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty

\* This is the exhaustive selected-candidate/pre-candidate action
\* classification.  Parent departure carries the immutable causal cut,
\* intermediate stages cannot recharge it, and the selected due action either
\* reaches the exact terminal/lower rank or enters one of the two explicitly
\* charged non-descent arms.  An unrelated step retains the exact selected
\* token, candidate, lifecycle ordinal, semantic rank, owner, packet, and
\* physical source rank; an existential same-rank B is not a carry witness.
\* Tick changes only the selected slack and every deadline reset remains below
\* the receipt's absolute ceiling.
THEOREM AdequateLeaderFixedSelectedActionsCarryPipelineRank ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
  /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    => /\ AdequateLeaderFixedCutPerActionProvider
       /\ AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider
       /\ AdequateLeaderFixedSelectedCandidateActionCarriesAbsoluteCeiling
       /\ AdequateLeaderFixedSelectedEntryActionCarriesAbsoluteCeiling
BY AdequateLeaderFixedPipelineOriginHistoryFollowsAsyncStep,
   AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut,
   AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt,
   AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut,
   AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget,
   AsyncCandidateProducerContinuationHandoffRetainsExactLifecycle,
   CandidateProducerContinuationFrozenOriginsCannotReplenish,
   AsyncCandidateProducerSourceTransitionInstallsExactContinuation,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   DueIoServiceEnablesConcreteLocalProgress,
   HistoricalDiscoveryServeFairActionLowersOccurrenceDebt,
   AdequateLeaderTargetEqualCountReplacementIntroducesAndRetires,
   AdequateLeaderTargetCountIncreaseIntroducesOwnerIdentity,
   AdequateLeaderFixedPipelineClockPotentialFitsConfiguredBudget,
   AdequateLeaderFixedPreCandidateReservedTailFitsConfiguredCharge,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   IsaT(18000)
   DEF AdequateLeaderFixedCutPerActionProvider,
       AdequateLeaderFixedPipelineStrictRankGoal,
       AdequateLeaderFixedPipelineOriginSlotsPreservedAction,
       AdequateLeaderFixedPipelineOriginEqualCountReplacementAction,
       AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction,
       AdequateLeaderFixedPipelineProducerHandoffFrontier,
       AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider,
       AdequateLeaderFixedSelectedPreCandidateEntryFrontier,
       AdequateLeaderFixedPreCandidateEntryStrictRankGoal,
       AdequateLeaderFixedSelectedCandidateActionCarriesAbsoluteCeiling,
       AdequateLeaderFixedSelectedEntryActionCarriesAbsoluteCeiling,
       AdequateLeaderFixedPipelineOriginEpisodeFrontier,
       AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal,
       AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible,
       AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates,
       AdequateLeaderFixedSelectedPipelineRankFrontier,
       AdequateLeaderFixedPipelineRankCell,
       AdequateLeaderFixedPipelineRank,
       AdequateLeaderFixedPipelineAbsoluteCeiling,
       AdequateLeaderFixedPipelineAbsoluteCeilingAtUnknownBudget,
       AdequateLeaderFixedCandidateSelectedServiceSlack,
       AdequateLeaderFixedEntryServiceSlack,
       AdequateLeaderFixedSelectedServiceOwnerAction,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncNext, AsyncAllVars

\* This is the one-step semantic boundary of the exact physical service
\* classification above.  It applies only when the selected immutable
\* occurrence actually reaches its retained completion/retirement record.
\* A transport retry, Tick-only slack step, or other same-owner frame does
\* not satisfy that postcondition; it remains in the already finite selected
\* owner/producer episode and is not relabelled as protocol progress.
\*
\* Keeping the proposal-subject corridor in the post-state separates a real
\* fixed-subject completion from the independently charged subject-switch
\* episode.  Within that corridor, exact owner retirement leaves only the
\* exhaustive semantic alternatives: Decision/lower occurrence, equal-count
\* replacement, or count-increasing replenishment.  The last two alternatives
\* are merely the entry actions for their finite discovery episode.
THEOREM AdequateLeaderFixedSelectedServiceHasExhaustiveOutcome ==
  \A initialContext \in ContextRecords:
    \A target \in ValidatorIds:
      \A leaderContext \in ContextRecords:
        \A leader \in ValidatorIds:
          \A leaderView \in Views:
            \A receipt \in AdequateLeaderAuthorityDeadlineReceiptSet:
              \A token
                    \in AdequateLeaderFixedPipelineTokenCarrier(
                         leaderContext):
                \A candidate \in AsyncCandidateSet:
                  \A cutoffOrdinal \in Nat:
                    \A semanticRank \in (1..4) \X (0..9):
                      \A serviceOwner
                            \in AdequateLeaderFixedSelectedServiceOwnerSet(
                                 initialContext):
                        \A packet \in AsyncPacketSet:
                          \A sourceRank
                                \in AdequateLeaderFixedPipelineRankCarrier:
                            \A occurrenceRank
                                  \in AdequateLeaderTargetOccurrenceRankCarrier:
                              \A occurrenceOwner
                                    \in AdequateLeaderFrozenCandidateOwnerUniverse(
                                         candidate.node, leaderContext,
                                         leader, leaderView,
                                         receipt.subject):
                                /\ AsyncStrongTypeInvariant
                                /\ AsyncStrongTypeInvariant'
                                /\ AsyncProgressOwnershipInvariant
                                /\ AsyncCandidateServiceTombstoneLifecycleInvariant
                                /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
                                /\ AsyncCandidateLifecycleSchedulerCoverageInvariant'
                                /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
                                /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                                /\ AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates(
                                     candidate, leaderContext, leader,
                                     leaderView, receipt.subject,
                                     semanticRank, occurrenceRank,
                                     occurrenceOwner)
                                /\ AdequateLeaderFixedSelectedPipelineRankFrontier(
                                     initialContext, target, leaderContext,
                                     leader, leaderView, receipt,
                                     token, candidate, cutoffOrdinal,
                                     semanticRank, serviceOwner, packet,
                                     sourceRank)
                                /\ AdequateLeaderTargetProtocolSubjectSource(
                                     candidate.node, leaderContext,
                                     leader, leaderView, receipt.subject)
                                /\ serviceOwner.ownerKind # "Tick"
                                /\ <<AdequateLeaderFixedSelectedServiceOwnerAction(
                                        serviceOwner)>>_AsyncAllVars
                                /\ [AsyncNext]_AsyncAllVars
                                /\ (AdequateLeaderTargetProtocolSubjectSource(
                                      candidate.node, leaderContext,
                                      leader, leaderView, receipt.subject))'
                                /\ (AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
                                      candidate.node, leaderContext, leader,
                                      leaderView, receipt.subject,
                                      occurrenceRank, occurrenceOwner))'
                                  => AdequateLeaderTargetServiceOutcomeAction(
                                       candidate.node, leaderContext, leader,
                                       leaderView, receipt.subject,
                                       occurrenceRank)
BY AdequateLeaderFixedSelectedActionsCarryPipelineRank,
   AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut,
   AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt,
   AsyncCandidateProducerContinuationHandoffRetainsExactLifecycle,
   CandidateProducerContinuationFrozenOriginsCannotReplenish,
   AsyncCandidateProducerSourceTransitionInstallsExactContinuation,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   AdequateLeaderNonDecisionDeclaredSuccessorStrictlyLowersStaticRank,
   AdequateLeaderLiveOwnersStayInsideFrozenUniverse,
   AsyncBracketNextPreservesStrongTypeInvariant,
   FS_Image, FS_CardinalityType, SMT, IsaT(24000)
   DEF AdequateLeaderFixedCutPerActionProvider,
       AdequateLeaderFixedPipelineStrictRankGoal,
       AdequateLeaderFixedPipelineOriginSlotsPreservedAction,
       AdequateLeaderFixedPipelineOriginEqualCountReplacementAction,
       AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction,
       AdequateLeaderFixedPipelineProducerHandoffFrontier,
       AdequateLeaderFixedSelectedOccurrenceProducerResidual,
       AdequateLeaderTargetSameOrHigherOccurrenceFrontier,
       AdequateLeaderTargetServiceOutcomeAction,
       AdequateLeaderTargetDecisionOrStrictlyLowerOccurrenceAction,
       AdequateLeaderTargetEqualCountOwnerReplacementAction,
       AdequateLeaderTargetCountIncreasingReplenishmentAction,
       AdequateLeaderTargetStrictOccurrenceDescentGoal,
       AdequateLeaderTargetZeroOwnerProducerCell,
       AdequateLeaderTargetOccurrenceRankFrontier,
       AdequateLeaderTargetOccurrenceOwnerRetirementClosed,
       AdequateLeaderTargetOccurrenceOwnerIdentitySet,
       AdequateLeaderTargetRankOwnerCount,
       AdequateLeaderTargetRankOwnerIdentitySet,
       AdequateLeaderTargetProducerTransportResidual,
       AdequateLeaderTargetProducerResidual,
       AdequateLeaderTargetProtocolSubjectSource,
       AdequateLeaderFixedCandidateSemanticOccurrenceCoordinates,
       AdequateLeaderFixedSelectedPipelineRankFrontier,
       AdequateLeaderFixedSelectedServiceOwnerAction,
       AsyncNext, AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderFixedCutPerAction ==
  \A initialContext:
    AdequateLeaderFixedCutPerActionProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AdequateLeaderFixedSelectedActionsCarryPipelineRank,
   PTL
   DEF AdequateLeaderFixedCutPerActionProviderProperty

THEOREM AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep ==
  \A initialContext:
    AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AdequateLeaderFixedSelectedActionsCarryPipelineRank,
   PTL
   DEF AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty

THEOREM AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry ==
  \A initialContext:
    AdequateLeaderFixedSelectedActionClockCarryProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AdequateLeaderFixedSelectedActionsCarryPipelineRank,
   PTL
   DEF AdequateLeaderFixedSelectedActionClockCarryProviderProperty

THEOREM
    AsyncLiveProvidesAdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStep ==
  \A initialContext:
    AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory,
   AsyncLiveProvidesAdequateLeaderFixedCutPerAction,
   AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry,
   AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepFollowsProviders,
   PTL
   DEF
     AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty,
     AdequateLeaderFixedSelectedActionClockCarryProviderProperty

(***************************************************************************
Concrete source-qualified retained ingress service.

The source below already contains a real transport packet whose exact
request/source episode is absent from the consumed journal.  Tick fairness
makes it due after GST.  Per-recipient/source Admit fairness consumes the
finite immutable prefix ahead of it; later traffic cannot move ahead of that
packet.  When the selected packet is finally admitted (including a policy
drop or an exact coalescing admission), the episode is first-distinct for its
request.  The leading journal coordinate therefore decreases enough to
dominate any change in the bounded exact Candidate-occurrence tail.  No
retransmission and no aggregate Decision theorem is used as progress.
***************************************************************************)

AdequateLeaderRetainedProducerPacketWitnessSet(
    request, target, leaderContext, leader, leaderView, subject) ==
  {packet \in asyncTransport:
     /\ AsyncProducerIngressRequest(packet.item) = request
     /\ packet.authenticatedSource \in AsyncIngressSources
     /\ AsyncProducerIngressEpisode(
          packet.item, packet.authenticatedSource)
          \notin asyncProducerConsumedEpisodes
     /\ AsyncProducerIngressRequest(packet.item)
          \in AdequateLeaderFrozenTargetProducerRequests(
               target, leaderContext, leader, leaderView, subject)}

AdequateLeaderRetainedProducerSelectedPacket(
    request, target, leaderContext, leader, leaderView, subject) ==
  CHOOSE packet \in
    AdequateLeaderRetainedProducerPacketWitnessSet(
      request, target, leaderContext, leader, leaderView, subject): TRUE

THEOREM AdequateLeaderRetainedProducerSourceHasActualPacketWitness ==
  \A request, node, cutoffOrdinal, sourceRank,
     target, leaderContext, leader, leaderView,
     subject, known, budget:
    AdequateLeaderRetainedProducerEpisodeAtRank(
      request, node, cutoffOrdinal, sourceRank,
      target, leaderContext, leader, leaderView,
      subject, known, budget)
      => LET packet ==
               AdequateLeaderRetainedProducerSelectedPacket(
                 request, target, leaderContext,
                 leader, leaderView, subject)
         IN /\ packet
                  \in AdequateLeaderRetainedProducerPacketWitnessSet(
                       request, target, leaderContext,
                       leader, leaderView, subject)
            /\ packet.item.envelope.recipient = node
BY Isa
   DEF AdequateLeaderRetainedProducerEpisodeAtRank,
       AdequateLeaderTargetProducerPacketEpisodesFor,
       AdequateLeaderTargetProducerPacketEpisodes,
       AdequateLeaderRetainedProducerPacketWitnessSet,
       AdequateLeaderRetainedProducerSelectedPacket,
       AdequateLeaderRetainedProducerRequestOwner,
       AsyncProducerIngressEpisode,
       AsyncProducerEpisode

THEOREM AdequateLeaderRetainedProducerExactAdmissionLowersCompositeRank ==
  \A request, node, cutoffOrdinal, sourceRank,
     target, leaderContext, leader, leaderView,
     subject, known, budget, packet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ AsyncProducerJournalClosed
    /\ AsyncNext
    /\ AdequateLeaderRetainedProducerEpisodeAtRank(
         request, node, cutoffOrdinal, sourceRank,
         target, leaderContext, leader, leaderView,
         subject, known, budget)
    /\ packet
         \in AdequateLeaderRetainedProducerPacketWitnessSet(
              request, target, leaderContext,
              leader, leaderView, subject)
    /\ packet =
         OldestDueSourcePacket(node, packet.authenticatedSource)
    /\ PostGstAdmitHiddenPacket(node, packet.authenticatedSource)
      => (AdequateLeaderRetainedProducerExactRankGoal(
            request, node, cutoffOrdinal, sourceRank, target))'
BY AsyncNextProjectsMonotoneProducerJournal,
   AsyncFiniteStrongTypeBoundsCandidateEpisodeTail,
   AsyncFiniteFirstDistinctTargetIngressDominatesCandidateEpisodeTail,
   IsaT(900)
   DEF AdequateLeaderRetainedProducerExactRankGoal,
       AdequateLeaderRetainedProducerEpisodeAtRank,
       AdequateLeaderRetainedProducerCompositeRank,
       AdequateLeaderRetainedProducerPacketWitnessSet,
       AsyncProducerFirstDistinctIngressEpisodeStepFor,
       AsyncProducerAdmittedIngressEpisodesFor,
       AsyncProducerAdmittedIngressEpisodes,
       AsyncProducerAdmittedIngressCoordinates,
       AsyncProducerIngressEpisode,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AsyncFiniteCandidateEpisodeTailTypeInvariant

AdequateLeaderRetainedProducerPacketClockDebt(packet) ==
  IF asyncNow < packet.deadline
  THEN packet.deadline - asyncNow
  ELSE 0

AdequateLeaderRetainedProducerPacketPrefixRank(
    snapshot, clockValue, packet) ==
  <<AdequateLeaderRetainedProducerPacketClockDebt(packet),
    ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot(
      snapshot, clockValue)>>

AdequateLeaderRetainedProducerPacketPrefixRankCarrier ==
  Nat \X ExactDecisionTargetNeutralFixedClockCarrier

AdequateLeaderRetainedProducerPacketPrefixRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ExactDecisionTargetNeutralFixedClockOrdering,
    Nat,
    ExactDecisionTargetNeutralFixedClockCarrier)

THEOREM AdequateLeaderRetainedProducerPacketPrefixRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderRetainedProducerPacketPrefixRankOrdering,
    AdequateLeaderRetainedProducerPacketPrefixRankCarrier)
BY NatLessThanWellFounded,
   ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded,
   WFLexPairOrdering
   DEF AdequateLeaderRetainedProducerPacketPrefixRankOrdering,
       AdequateLeaderRetainedProducerPacketPrefixRankCarrier

THEOREM AdequateLeaderRetainedProducerPacketPrefixRankIsInCarrier ==
  \A packet \in AsyncPacketSet,
     snapshot \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
     clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
    /\ asyncNow = clockValue
    => AdequateLeaderRetainedProducerPacketPrefixRank(
         snapshot, clockValue, packet)
         \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   ExactDecisionTargetNeutralPacketDependencyRankForSnapshotInCarrier,
   HistoricalDiscoveryIngressCounterRankInCarrier,
   HistoricalDiscoveryFixedClockRankShapeInCarrier,
   FS_CardinalityType, IsaT(300)
   DEF AdequateLeaderRetainedProducerPacketPrefixRank,
       AdequateLeaderRetainedProducerPacketClockDebt,
       AdequateLeaderRetainedProducerPacketPrefixRankCarrier,
       AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       ExactDecisionTargetNeutralConcreteBlockerStage,
       ExactDecisionTargetNeutralConcreteDependencyRankForSnapshot,
       ExactDecisionTargetNeutralSelectedOverduePacket,
       ExactDecisionTargetNeutralSelectedPacketDependencyRankForSnapshot,
       ExactDecisionTargetNeutralFixedClockCarrier,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       HistoricalDiscoveryBlockerStageCarrier

AdequateLeaderRetainedProducerPacketPrefixAtRank(
    initialContext,
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget,
    packet, snapshot, clockValue, prefixRank) ==
  /\ AdequateLeaderRetainedProducerEpisodeAtRank(
       request, node, cutoffOrdinal, sourceRank,
       target, leaderContext, leader, leaderView,
       subject, known, budget)
  /\ ~AdequateLeaderRetainedProducerExactRankGoal(
       request, node, cutoffOrdinal, sourceRank, target)
  /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
  /\ packet
       \in AdequateLeaderRetainedProducerPacketWitnessSet(
            request, target, leaderContext,
            leader, leaderView, subject)
  /\ packet.item.envelope.recipient = node
  /\ clockValue = asyncNow
  /\ snapshot \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier
  /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)
  /\ prefixRank
       \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier
  /\ prefixRank =
       AdequateLeaderRetainedProducerPacketPrefixRank(
         snapshot, clockValue, packet)

\* An inner fixed-clock descent retains the exact source-sealed snapshot,
\* including its physical admission cut.  Only strict outer clock-debt
\* descent may construct a new snapshot; a retry or owner replacement at the
\* same clock can never refresh the cut.
AdequateLeaderRetainedProducerPacketPrefixSnapshotCarry(
    snapshot, clockValue, prefixRank,
    snapshot2, clockValue2, lowerPrefixRank) ==
  \/ lowerPrefixRank[1] < prefixRank[1]
  \/ /\ lowerPrefixRank[1] = prefixRank[1]
     /\ snapshot2 = snapshot
     /\ clockValue2 = clockValue

THEOREM AdequateLeaderRetainedProducerSameClockCannotRefreshPhysicalCut ==
  \A snapshot, clockValue, prefixRank,
     snapshot2, clockValue2, lowerPrefixRank:
    /\ AdequateLeaderRetainedProducerPacketPrefixSnapshotCarry(
         snapshot, clockValue, prefixRank,
         snapshot2, clockValue2, lowerPrefixRank)
    /\ lowerPrefixRank[1] = prefixRank[1]
    => /\ snapshot2 = snapshot
       /\ clockValue2 = clockValue
       /\ snapshot2.physicalCuts = snapshot.physicalCuts
BY Isa
   DEF AdequateLeaderRetainedProducerPacketPrefixSnapshotCarry

THEOREM AdequateLeaderRetainedProducerConfiguredSnapshotCapturesPhysicalCut ==
  \A clockValue:
    (ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)).physicalCuts
      = ExactDecisionTargetNeutralCurrentPhysicalCuts
BY Isa DEF ExactDecisionTargetNeutralFixedClockSnapshot

THEOREM AdequateLeaderRetainedProducerAdmissionUsesCapturedPhysicalCut ==
  \A packet \in AsyncPacketSet, clockValue \in Nat:
    LET item == packet.item
        node == item.envelope.recipient
        source == packet.authenticatedSource
        snapshot ==
          ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)
    IN /\ asyncNow = clockValue
       /\ node \in Responsive
       /\ packet = OldestDueSourcePacket(node, source)
       /\ AdmitHiddenPacket(node, source)
       /\ item.kind \in AsyncLeaderWireKinds
       /\ AsyncLeaderWireLifecycleIdentityDerivable(item)
       => \E record \in asyncLeaderWireLifecycles':
            /\ record.identity =
                 AsyncLeaderWireLifecycleIdentityAt(item, context)
            /\ record.physicalAdmissionOrdinal
                 = snapshot.physicalCuts[node]
            /\ AsyncNextIngressPhysicalOrdinal(node)'
                 = snapshot.physicalCuts[node] + 1
BY AdmitHiddenLeaderWireIsAtomicLocalAcceptanceCut,
   AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal,
   AdequateLeaderRetainedProducerConfiguredSnapshotCapturesPhysicalCut,
   Isa
   DEF ExactDecisionTargetNeutralCurrentPhysicalCuts

AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
    initialContext,
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget,
    packet, snapshot, clockValue, prefixRank) ==
  \/ AdequateLeaderRetainedProducerRankOrKnownAdvanceGoal(
       request, node, cutoffOrdinal, sourceRank,
       target, leaderContext, leader, leaderView,
       subject, known, budget)
  \/ \E snapshot2 \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
       clockValue2 \in Nat,
       lowerPrefixRank
         \in SetLessThan(
              prefixRank,
              AdequateLeaderRetainedProducerPacketPrefixRankOrdering,
              AdequateLeaderRetainedProducerPacketPrefixRankCarrier):
       /\ AdequateLeaderRetainedProducerPacketPrefixSnapshotCarry(
            snapshot, clockValue, prefixRank,
            snapshot2, clockValue2, lowerPrefixRank)
       /\ AdequateLeaderRetainedProducerPacketPrefixAtRank(
            initialContext,
            request, node, cutoffOrdinal, sourceRank,
            target, leaderContext, leader, leaderView,
            subject, known, budget,
            packet, snapshot2, clockValue2, lowerPrefixRank)

AdequateLeaderRetainedProducerPacketOwnerReady(
    initialContext,
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget,
    packet, snapshot, clockValue, prefixRank, owner) ==
  /\ owner \in ExactDecisionTargetNeutralFairOwnerSet(initialContext)
  /\ ENABLED
       (ExactDecisionTargetNeutralFairAction(owner)
          /\ AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
               initialContext,
               request, node, cutoffOrdinal, sourceRank,
               target, leaderContext, leader, leaderView,
               subject, known, budget,
               packet, snapshot, clockValue, prefixRank)')

AdequateLeaderRetainedProducerSelectedPacketOwner(
    initialContext,
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget,
    packet, snapshot, clockValue, prefixRank) ==
  CHOOSE owner \in
    ExactDecisionTargetNeutralFairOwnerSet(initialContext):
      AdequateLeaderRetainedProducerPacketOwnerReady(
        initialContext,
        request, node, cutoffOrdinal, sourceRank,
        target, leaderContext, leader, leaderView,
        subject, known, budget,
        packet, snapshot, clockValue, prefixRank, owner)

AdequateLeaderRetainedProducerPacketPrefixEntryProvider(initialContext) ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds,
     cutoffOrdinal \in Nat,
     sourceRank \in Nat,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    \A known \in
         SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
           target, leaderContext, leader, leaderView, subject):
      \A budget \in Nat:
        /\ AdequateLeaderRetainedProducerEpisodeAtRank(
             request, node, cutoffOrdinal, sourceRank,
             target, leaderContext, leader, leaderView,
             subject, known, budget)
        /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
          => \/ AdequateLeaderRetainedProducerExactRankGoal(
                   request, node, cutoffOrdinal, sourceRank, target)
             \/ \E packet \in AsyncPacketSet,
                  snapshot \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
                  clockValue \in Nat,
                  prefixRank
                    \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier:
                  /\ snapshot =
                       ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)
                  /\ AdequateLeaderRetainedProducerPacketPrefixAtRank(
                       initialContext,
                       request, node, cutoffOrdinal, sourceRank,
                       target, leaderContext, leader, leaderView,
                       subject, known, budget,
                       packet, snapshot, clockValue, prefixRank)

AdequateLeaderRetainedProducerPacketConcreteOwnerProvider(initialContext) ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds,
     cutoffOrdinal \in Nat,
     sourceRank \in Nat,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    \A known \in
         SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
           target, leaderContext, leader, leaderView, subject):
      \A budget \in Nat,
         packet \in AsyncPacketSet,
         snapshot \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
         clockValue \in Nat,
         prefixRank \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier:
        AdequateLeaderRetainedProducerPacketPrefixAtRank(
          initialContext,
          request, node, cutoffOrdinal, sourceRank,
          target, leaderContext, leader, leaderView,
          subject, known, budget,
          packet, snapshot, clockValue, prefixRank)
          => AdequateLeaderRetainedProducerPacketOwnerReady(
               initialContext,
               request, node, cutoffOrdinal, sourceRank,
               target, leaderContext, leader, leaderView,
               subject, known, budget,
               packet, snapshot, clockValue, prefixRank,
               AdequateLeaderRetainedProducerSelectedPacketOwner(
                 initialContext,
                 request, node, cutoffOrdinal, sourceRank,
                 target, leaderContext, leader, leaderView,
                 subject, known, budget,
                 packet, snapshot, clockValue, prefixRank))

AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider(
    initialContext) ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds,
     cutoffOrdinal \in Nat,
     sourceRank \in Nat,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    \A known \in
         SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
           target, leaderContext, leader, leaderView, subject):
      \A budget \in Nat,
         packet \in AsyncPacketSet,
         snapshot \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
         clockValue \in Nat,
         prefixRank \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier:
        LET owner ==
              AdequateLeaderRetainedProducerSelectedPacketOwner(
                initialContext,
                request, node, cutoffOrdinal, sourceRank,
                target, leaderContext, leader, leaderView,
                subject, known, budget,
                packet, snapshot, clockValue, prefixRank)
        IN /\ AdequateLeaderRetainedProducerPacketPrefixAtRank(
                 initialContext,
                 request, node, cutoffOrdinal, sourceRank,
                 target, leaderContext, leader, leaderView,
                 subject, known, budget,
                 packet, snapshot, clockValue, prefixRank)
           /\ [AsyncNext]_AsyncAllVars
           => \/ (AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                    initialContext,
                    request, node, cutoffOrdinal, sourceRank,
                    target, leaderContext, leader, leaderView,
                    subject, known, budget,
                    packet, snapshot, clockValue, prefixRank))'
              \/ /\ (AdequateLeaderRetainedProducerPacketPrefixAtRank(
                       initialContext,
                       request, node, cutoffOrdinal, sourceRank,
                       target, leaderContext, leader, leaderView,
                       subject, known, budget,
                       packet, snapshot, clockValue, prefixRank))'
                 /\ (AdequateLeaderRetainedProducerSelectedPacketOwner(
                       initialContext,
                       request, node, cutoffOrdinal, sourceRank,
                       target, leaderContext, leader, leaderView,
                       subject, known, budget,
                       packet, snapshot, clockValue, prefixRank))'
                      = owner
                 /\ ~<<ExactDecisionTargetNeutralFairAction(owner)>>_AsyncAllVars

AdequateLeaderRetainedProducerPacketActionProviderProperty(
    specification, initialContext) ==
  /\ initialContext \in ContextRecords
  /\ (specification
        => [][(/\ AdequateLeaderRetainedProducerPacketPrefixEntryProvider(
                      initialContext)
               /\ AdequateLeaderRetainedProducerPacketConcreteOwnerProvider(
                    initialContext)
               /\ AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider(
                    initialContext))]_AsyncAllVars)

THEOREM AdequateLeaderRetainedProducerPacketFactsSupplyActionProviders ==
  \A initialContext \in ContextRecords:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ PostGstReplayQuarantineExcluded
      => /\ AdequateLeaderRetainedProducerPacketPrefixEntryProvider(
               initialContext)
         /\ AdequateLeaderRetainedProducerPacketConcreteOwnerProvider(
               initialContext)
         /\ AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider(
               initialContext)
BY AdequateLeaderRetainedProducerSourceHasActualPacketWitness,
   AdequateLeaderRetainedProducerExactAdmissionLowersCompositeRank,
   AdequateLeaderRetainedProducerPacketPrefixRankIsInCarrier,
   AdequateLeaderRetainedProducerAdmissionUsesCapturedPhysicalCut,
   AdequateLeaderRetainedProducerSameClockCannotRefreshPhysicalCut,
   ExactDecisionTargetNeutralSnapshotIsFinite,
   ExactDecisionTargetNeutralPacketDependencyRankForSnapshotInCarrier,
   ExactDecisionTargetNeutralActiveSnapshotConcreteRankIsInCarrier,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock,
   ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame,
   ExactDecisionTargetNeutralSnapshotProducerEpisodeDoesNotReplenish,
   HistoricalDiscoveryFixedClockBlockerCharacterization,
   HistoricalDiscoveryFixedClockLexStepStrictlyDescends,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryLowerServeInsertionReselectsLower,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryServeFairActionLowersOccurrenceDebt,
   CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   ExternalCandidateProducerContinuationSelectionIsReady,
   LocalContinuationReadyEnablesFairResolution,
   ConditionalTransportContinuationReadyEnablesFairService,
   VolatileBodyContinuationReadyEnablesFairService,
   CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends,
   AsyncTickEnabledHasConcreteSuccessor,
   OverdueResponsivePacketEnablesConcreteProgress,
   DueNodeServiceEnablesConcreteGateProgress,
   DueIoServiceEnablesConcreteLocalProgress,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   IsaT(18000)
   DEF AdequateLeaderRetainedProducerPacketPrefixEntryProvider,
       AdequateLeaderRetainedProducerPacketConcreteOwnerProvider,
       AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider,
       AdequateLeaderRetainedProducerPacketOwnerReady,
       AdequateLeaderRetainedProducerSelectedPacketOwner,
       AdequateLeaderRetainedProducerPacketPrefixAtRank,
       AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal,
       AdequateLeaderRetainedProducerPacketPrefixSnapshotCarry,
       AdequateLeaderRetainedProducerPacketPrefixRank,
       AdequateLeaderRetainedProducerPacketClockDebt,
       AdequateLeaderRetainedProducerPacketWitnessSet,
       AdequateLeaderRetainedProducerSelectedPacket,
       AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
       ExactDecisionTargetNeutralFairOwnerSet,
       ExactDecisionTargetNeutralFairAction,
       ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot,
       AsyncNext, AsyncAllVars

THEOREM AdequateLeaderRetainedProducerPacketFactsSupplyPrefixEntry ==
  \A initialContext \in ContextRecords:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ PostGstReplayQuarantineExcluded
      => AdequateLeaderRetainedProducerPacketPrefixEntryProvider(
           initialContext)
BY AdequateLeaderRetainedProducerPacketFactsSupplyActionProviders

THEOREM AdequateLeaderRetainedProducerPacketFactsSupplyConcreteOwner ==
  \A initialContext \in ContextRecords:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ PostGstReplayQuarantineExcluded
      => AdequateLeaderRetainedProducerPacketConcreteOwnerProvider(
           initialContext)
BY AdequateLeaderRetainedProducerPacketFactsSupplyActionProviders

THEOREM AdequateLeaderRetainedProducerPacketFactsSupplySelectedOwnerStep ==
  \A initialContext \in ContextRecords:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ PostGstReplayQuarantineExcluded
      => AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider(
           initialContext)
BY AdequateLeaderRetainedProducerPacketFactsSupplyActionProviders

THEOREM AsyncLiveProvidesAdequateLeaderRetainedProducerPacketActionProviders ==
  \A initialContext:
    AdequateLeaderRetainedProducerPacketActionProviderProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysExcludesPostGstReplayQuarantine,
   AdequateLeaderRetainedProducerPacketFactsSupplyActionProviders,
   PTL
   DEF AdequateLeaderRetainedProducerPacketActionProviderProperty

AdequateLeaderRetainedProducerExactOwnerFairnessProperty(
    specification, initialContext) ==
  /\ initialContext \in ContextRecords
  /\ (specification
        => \A owner \in
              ExactDecisionTargetNeutralFairOwnerSet(initialContext):
             WF_AsyncAllVars(ExactDecisionTargetNeutralFairAction(owner)))

THEOREM AsyncLiveProvidesAdequateLeaderRetainedProducerExactOwnerFairness ==
  \A initialContext:
    AdequateLeaderRetainedProducerExactOwnerFairnessProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
BY AsyncLiveSpecProjectsAsyncSpec,
   ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness, PTL
   DEF AdequateLeaderRetainedProducerExactOwnerFairnessProperty

AdequateLeaderRetainedProducerPacketPrefixRankStepProperty(
    specification, initialContext) ==
  /\ initialContext \in ContextRecords
  /\ (specification
    => \A request \in AsyncProducerIngressRequests,
          node \in ValidatorIds,
          cutoffOrdinal \in Nat,
          sourceRank \in Nat,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects:
         \A known \in
              SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
                target, leaderContext, leader, leaderView, subject):
           \A budget \in Nat,
              packet \in AsyncPacketSet,
              snapshot \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
              clockValue \in Nat,
              prefixRank
                \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier:
             AdequateLeaderRetainedProducerPacketPrefixAtRank(
               initialContext,
               request, node, cutoffOrdinal, sourceRank,
               target, leaderContext, leader, leaderView,
               subject, known, budget,
               packet, snapshot, clockValue, prefixRank)
               ~> AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                    initialContext,
                    request, node, cutoffOrdinal, sourceRank,
                    target, leaderContext, leader, leaderView,
                    subject, known, budget,
                    packet, snapshot, clockValue, prefixRank))

\* Freeze the exact packet owner selected in the current prefix cell.  The
\* packet step provider carries this equality across every non-owner frame,
\* so weak fairness is applied only to a constant owner value.
AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
    initialContext,
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget,
    packet, snapshot, clockValue, prefixRank, owner) ==
  /\ AdequateLeaderRetainedProducerPacketPrefixAtRank(
       initialContext,
       request, node, cutoffOrdinal, sourceRank,
       target, leaderContext, leader, leaderView,
       subject, known, budget,
       packet, snapshot, clockValue, prefixRank)
  /\ AdequateLeaderRetainedProducerSelectedPacketOwner(
       initialContext,
       request, node, cutoffOrdinal, sourceRank,
       target, leaderContext, leader, leaderView,
       subject, known, budget,
       packet, snapshot, clockValue, prefixRank)
       = owner

THEOREM AdequateLeaderRetainedProducerPacketProvidersSupplyRankStep ==
  \A specification, initialContext:
    /\ AdequateLeaderAsyncNextBehaviorProperty(specification)
    /\ AdequateLeaderRetainedProducerPacketActionProviderProperty(
         specification, initialContext)
    /\ AdequateLeaderRetainedProducerExactOwnerFairnessProperty(
         specification, initialContext)
      => AdequateLeaderRetainedProducerPacketPrefixRankStepProperty(
           specification, initialContext)
PROOF
  <1>1. ASSUME NEW specification,
                NEW initialContext,
                AdequateLeaderAsyncNextBehaviorProperty(specification),
                AdequateLeaderRetainedProducerPacketActionProviderProperty(
                  specification, initialContext),
                AdequateLeaderRetainedProducerExactOwnerFairnessProperty(
                  specification, initialContext)
         PROVE AdequateLeaderRetainedProducerPacketPrefixRankStepProperty(
                 specification, initialContext)
    <2>1. initialContext \in ContextRecords
      BY <1>1
         DEF AdequateLeaderRetainedProducerPacketActionProviderProperty
    <2>2. ASSUME specification
           PROVE \A request \in AsyncProducerIngressRequests,
                     node \in ValidatorIds,
                     cutoffOrdinal \in Nat,
                     sourceRank \in Nat,
                     target \in ValidatorIds,
                     leaderContext \in ContextRecords,
                     leader \in ValidatorIds,
                     leaderView \in Views,
                     subject \in Subjects:
                   \A known
                         \in SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
                              target, leaderContext, leader, leaderView,
                              subject):
                     \A budget \in Nat,
                        packet \in AsyncPacketSet,
                        snapshot
                          \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
                        clockValue \in Nat,
                        prefixRank
                          \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier:
                       AdequateLeaderRetainedProducerPacketPrefixAtRank(
                         initialContext,
                         request, node, cutoffOrdinal, sourceRank,
                         target, leaderContext, leader, leaderView,
                         subject, known, budget,
                         packet, snapshot, clockValue, prefixRank)
                         ~> AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                              initialContext,
                              request, node, cutoffOrdinal, sourceRank,
                              target, leaderContext, leader, leaderView,
                              subject, known, budget,
                              packet, snapshot, clockValue, prefixRank)
      <3>1. ASSUME NEW request \in AsyncProducerIngressRequests,
                    NEW node \in ValidatorIds,
                    NEW cutoffOrdinal \in Nat,
                    NEW sourceRank \in Nat,
                    NEW target \in ValidatorIds,
                    NEW leaderContext \in ContextRecords,
                    NEW leader \in ValidatorIds,
                    NEW leaderView \in Views,
                    NEW subject \in Subjects,
                    NEW known
                      \in SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
                           target, leaderContext, leader, leaderView,
                           subject),
                    NEW budget \in Nat,
                    NEW packet \in AsyncPacketSet,
                    NEW snapshot
                      \in AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
                    NEW clockValue \in Nat,
                    NEW prefixRank
                      \in AdequateLeaderRetainedProducerPacketPrefixRankCarrier
             PROVE AdequateLeaderRetainedProducerPacketPrefixAtRank(
                     initialContext,
                     request, node, cutoffOrdinal, sourceRank,
                     target, leaderContext, leader, leaderView,
                     subject, known, budget,
                     packet, snapshot, clockValue, prefixRank)
                     ~> AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                          initialContext,
                          request, node, cutoffOrdinal, sourceRank,
                          target, leaderContext, leader, leaderView,
                          subject, known, budget,
                          packet, snapshot, clockValue, prefixRank)
        <4>1. \A owner
                    \in ExactDecisionTargetNeutralFairOwnerSet(
                         initialContext):
                 AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                   initialContext,
                   request, node, cutoffOrdinal, sourceRank,
                   target, leaderContext, leader, leaderView,
                   subject, known, budget,
                   packet, snapshot, clockValue, prefixRank, owner)
                   ~> AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                        initialContext,
                        request, node, cutoffOrdinal, sourceRank,
                        target, leaderContext, leader, leaderView,
                        subject, known, budget,
                        packet, snapshot, clockValue, prefixRank)
          PROOF
            <5>1. ASSUME NEW owner
                          \in ExactDecisionTargetNeutralFairOwnerSet(
                               initialContext)
                   PROVE AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                           initialContext,
                           request, node, cutoffOrdinal, sourceRank,
                           target, leaderContext, leader, leaderView,
                           subject, known, budget,
                           packet, snapshot, clockValue, prefixRank, owner)
                           ~> AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                                initialContext,
                                request, node, cutoffOrdinal, sourceRank,
                                target, leaderContext, leader, leaderView,
                                subject, known, budget,
                                packet, snapshot, clockValue, prefixRank)
              <6>1. [][AsyncNext]_AsyncAllVars
                BY <1>1
                   DEF AdequateLeaderAsyncNextBehaviorProperty
              <6>2. [](AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                          initialContext,
                          request, node, cutoffOrdinal, sourceRank,
                          target, leaderContext, leader, leaderView,
                          subject, known, budget,
                          packet, snapshot, clockValue, prefixRank, owner)
                        /\ ~AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                             initialContext,
                             request, node, cutoffOrdinal, sourceRank,
                             target, leaderContext, leader, leaderView,
                             subject, known, budget,
                             packet, snapshot, clockValue, prefixRank)
                       => ENABLED
                            <<ExactDecisionTargetNeutralFairAction(
                                owner)>>_AsyncAllVars)
                BY <1>1, PTL, IsaT(900)
                   DEF AdequateLeaderRetainedProducerPacketActionProviderProperty,
                       AdequateLeaderRetainedProducerPacketConcreteOwnerProvider,
                       AdequateLeaderRetainedProducerPacketOwnerReady,
                       AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner,
                       AsyncAllVars
              <6>3. AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                          initialContext,
                          request, node, cutoffOrdinal, sourceRank,
                          target, leaderContext, leader, leaderView,
                          subject, known, budget,
                          packet, snapshot, clockValue, prefixRank, owner)
                        /\ ~AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                             initialContext,
                             request, node, cutoffOrdinal, sourceRank,
                             target, leaderContext, leader, leaderView,
                             subject, known, budget,
                             packet, snapshot, clockValue, prefixRank)
                        /\ <<ExactDecisionTargetNeutralFairAction(
                                 owner)>>_AsyncAllVars
                       => AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                            initialContext,
                            request, node, cutoffOrdinal, sourceRank,
                            target, leaderContext, leader, leaderView,
                            subject, known, budget,
                            packet, snapshot, clockValue, prefixRank)'
                BY <1>1, <6>1, PTL, IsaT(900)
                   DEF AdequateLeaderRetainedProducerPacketActionProviderProperty,
                       AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider,
                       AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner
              <6>4. AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                          initialContext,
                          request, node, cutoffOrdinal, sourceRank,
                          target, leaderContext, leader, leaderView,
                          subject, known, budget,
                          packet, snapshot, clockValue, prefixRank, owner)
                        /\ ~AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                             initialContext,
                             request, node, cutoffOrdinal, sourceRank,
                             target, leaderContext, leader, leaderView,
                             subject, known, budget,
                             packet, snapshot, clockValue, prefixRank)
                        /\ [AsyncNext]_AsyncAllVars
                       => \/ AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal(
                                initialContext,
                                request, node, cutoffOrdinal, sourceRank,
                                target, leaderContext, leader, leaderView,
                                subject, known, budget,
                                packet, snapshot, clockValue, prefixRank)'
                          \/ AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                               initialContext,
                               request, node, cutoffOrdinal, sourceRank,
                               target, leaderContext, leader, leaderView,
                               subject, known, budget,
                               packet, snapshot, clockValue, prefixRank,
                               owner)'
                BY <1>1, PTL, IsaT(900)
                   DEF AdequateLeaderRetainedProducerPacketActionProviderProperty,
                       AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider,
                       AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner
              <6>5. WF_AsyncAllVars(
                       ExactDecisionTargetNeutralFairAction(owner))
                BY <1>1, <5>1, PTL
                   DEF AdequateLeaderRetainedProducerExactOwnerFairnessProperty
              <6> QED BY <6>1, <6>2, <6>3, <6>4, <6>5, PTL
            <5> QED BY <5>1
        <4>2. [](AdequateLeaderRetainedProducerPacketPrefixAtRank(
                    initialContext,
                    request, node, cutoffOrdinal, sourceRank,
                    target, leaderContext, leader, leaderView,
                    subject, known, budget,
                    packet, snapshot, clockValue, prefixRank)
                 => \E owner
                         \in ExactDecisionTargetNeutralFairOwnerSet(
                              initialContext):
                      AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner(
                        initialContext,
                        request, node, cutoffOrdinal, sourceRank,
                        target, leaderContext, leader, leaderView,
                        subject, known, budget,
                        packet, snapshot, clockValue, prefixRank, owner))
          BY <1>1, PTL, IsaT(900)
             DEF AdequateLeaderRetainedProducerPacketActionProviderProperty,
                 AdequateLeaderRetainedProducerPacketConcreteOwnerProvider,
                 AdequateLeaderRetainedProducerPacketOwnerReady,
                 AdequateLeaderRetainedProducerPacketPrefixAtRankForOwner
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2
       DEF AdequateLeaderRetainedProducerPacketPrefixRankStepProperty
  <1> QED BY <1>1

AdequateLeaderRetainedProducerActualPacketStepProperty(specification) ==
  specification
    => \A request \in AsyncProducerIngressRequests,
          node \in ValidatorIds,
          cutoffOrdinal \in Nat,
          sourceRank \in Nat,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects:
         \A known \in
              SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
                target, leaderContext, leader, leaderView, subject):
           \A budget \in Nat:
             AdequateLeaderRetainedProducerEpisodeAtRank(
               request, node, cutoffOrdinal, sourceRank,
               target, leaderContext, leader, leaderView,
               subject, known, budget)
               ~> AdequateLeaderRetainedProducerRankOrKnownAdvanceGoal(
                    request, node, cutoffOrdinal, sourceRank,
                    target, leaderContext, leader, leaderView,
                    subject, known, budget)

THEOREM AdequateLeaderRetainedProducerPacketRankClosesNonDescentStep ==
  \A specification, initialContext:
    /\ (specification
          => [](AsyncCurrentResponsiveVoters
                  = AsyncVotersAt(initialContext)))
    /\ AdequateLeaderRetainedProducerPacketActionProviderProperty(
         specification, initialContext)
    /\ AdequateLeaderRetainedProducerPacketPrefixRankStepProperty(
         specification, initialContext)
      => AdequateLeaderRetainedProducerActualPacketStepProperty(
           specification)
BY AdequateLeaderRetainedProducerPacketPrefixRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF AdequateLeaderRetainedProducerPacketActionProviderProperty,
       AdequateLeaderRetainedProducerPacketPrefixEntryProvider,
       AdequateLeaderRetainedProducerPacketPrefixRankStepProperty,
       AdequateLeaderRetainedProducerActualPacketStepProperty,
       AdequateLeaderRetainedProducerPacketPrefixStrictRankGoal,
       AdequateLeaderRetainedProducerPacketPrefixSnapshotCarry

THEOREM AsyncLiveProvidesAdequateLeaderRetainedProducerNonDescentEpisodeStep ==
  \A initialContext:
    AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderRetainedProducerPacketActionProviders,
   AsyncLiveProvidesAdequateLeaderRetainedProducerExactOwnerFairness,
   AsyncLiveProvidesAdequateLeaderAsyncNextBehavior,
   AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AdequateLeaderRetainedProducerPacketProvidersSupplyRankStep,
   AdequateLeaderRetainedProducerPacketRankClosesNonDescentStep,
   PTL
   DEF AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty,
       AdequateLeaderRetainedProducerRankOrKnownAdvanceGoal,
       AdequateLeaderRetainedProducerActualPacketStepProperty,
       AsyncLiveSpecAt

\* Concrete global fixed-clock ownership.  This projection deliberately ends
\* at this pipeline's selected rank cell, not at an Exact-Decision residual.
\* The target-neutral lemmas below are used only for their concrete blocker
\* partition, immutable predecessor snapshot, retained-rank descent, and exact
\* fair-action classifications.  Candidate/pre-candidate target state is
\* carried by the local route/cut lemmas, so an unrelated exact-Decision exit
\* is never accepted as this provider's goal.
THEOREM AdequateLeaderFixedGlobalBlockerFactsSupplyProviders ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
  /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
  /\ PostGstReplayQuarantineExcluded
    => /\ AdequateLeaderFixedGlobalBlockerEntryProvider
       /\ AdequateLeaderFixedGlobalProducerEpisodeEntryProvider
       /\ AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider
       /\ AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider
       /\ AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider
       /\ AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider
BY HistoricalDiscoveryFixedClockBlockerCharacterization,
   ExactDecisionTargetNeutralSnapshotIsFinite,
   ExactDecisionTargetNeutralEpisodeRankIsInCarrier,
   ExactDecisionTargetNeutralActiveSnapshotConcreteRankIsInCarrier,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock,
   ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock,
   ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank,
   ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame,
   ExactDecisionTargetNeutralSnapshotProducerEpisodeDoesNotReplenish,
   ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank,
   CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   ExternalCandidateProducerContinuationSelectionIsReady,
   LocalContinuationReadyEnablesFairResolution,
   ConditionalTransportContinuationReadyEnablesFairService,
   VolatileBodyContinuationReadyEnablesFairService,
   CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends,
   AsyncTickEnabledHasConcreteSuccessor,
   OverdueResponsivePacketEnablesConcreteProgress,
   DueNodeServiceEnablesConcreteGateProgress,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   DueIoServiceEnablesConcreteLocalProgress,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryLowerServeInsertionReselectsLower,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryServeFairActionLowersOccurrenceDebt,
   AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut,
   AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt,
   AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut,
   AsyncCandidateProducerContinuationHandoffRetainsExactLifecycle,
   CandidateProducerContinuationFrozenOriginsCannotReplenish,
   AsyncCandidateProducerSourceTransitionInstallsExactContinuation,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   FS_Interval, FS_Image, FS_Union, FS_Product,
   FS_CardinalityType, IsaT(24000)
   DEF AdequateLeaderFixedGlobalBlockerEntryProvider,
       AdequateLeaderFixedGlobalProducerEpisodeEntryProvider,
       AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider,
       AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider,
       AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider,
       AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider,
       AdequateLeaderFixedConfiguredGlobalBlockerSnapshot,
       AdequateLeaderFixedGlobalLifecycleSnapshotCarrier,
       AdequateLeaderFixedGlobalBlockerSnapshotActive,
       AdequateLeaderFixedGlobalBlockerAtRank,
       AdequateLeaderFixedGlobalBlockerPending,
       AdequateLeaderFixedGlobalBlockerStrictRankGoal,
       AdequateLeaderFixedGlobalBlockerSelectionGoal,
       AdequateLeaderFixedGlobalProducerEpisodeAtRank,
       AdequateLeaderFixedGlobalProducerEpisodeOutcome,
       AdequateLeaderFixedGlobalBlockerOwnerReady,
       AdequateLeaderFixedSelectedGlobalBlockerOwner,
       AdequateLeaderFixedGlobalProducerEpisodeRank,
       AdequateLeaderFixedGlobalProducerEpisodeRankCarrier,
       AdequateLeaderFixedGlobalProducerEpisodeRankOrdering,
       AdequateLeaderFixedGlobalProducerEpisodeBottom,
       AdequateLeaderFixedConcreteGlobalBlockerRank,
       AdequateLeaderFixedGlobalBlockerProducerPrefix,
       AdequateLeaderFixedGlobalBlockerRankCarrier,
       AdequateLeaderFixedGlobalBlockerRankOrdering,
       AdequateLeaderFixedPipelineServiceRankFrontier,
       AdequateLeaderFixedSelectedPipelineServiceRankFrontier,
       AdequateLeaderFixedPipelineServiceRankFrontierForCell,
       AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell,
       AdequateLeaderFixedPipelineServiceCellIdentityCarrier,
       AdequateLeaderFixedCandidatePipelineServiceCellIdentity,
       AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity,
       AdequateLeaderFixedAnyPipelineTokenCarrier,
       AdequateLeaderFixedPreCandidateRouteCarrier,
       AdequateLeaderFixedPreCandidateRouteIdentityCarrier,
       AdequateLeaderFixedPipelineServiceRankDescentGoal,
       AdequateLeaderFixedSelectedServiceOwnerSet,
       AdequateLeaderFixedSelectedServiceOwnerAction,
       AdequateLeaderFixedPipelineRankCell,
       AdequateLeaderFixedPreCandidateEntryRankCell,
       AdequateLeaderFixedPreCandidateRouteTyped,
       AdequateLeaderFixedPipelineRank,
       AdequateLeaderFixedPreCandidateEntryRank,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncNext, AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders ==
  \A initialContext:
    AdequateLeaderFixedGlobalBlockerProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysExcludesPostGstReplayQuarantine,
   AsyncLiveProvidesAdequateLeaderRetainedProducerNonDescentEpisodeStep,
   AdequateLeaderFiniteRetainedProducerBudgetClosesNonDescentEpisode,
   AdequateLeaderFixedGlobalBlockerFactsSupplyProviders,
   PTL
   DEF AdequateLeaderFixedGlobalBlockerProviderProperty

\* The concrete AsyncLive supplier is now entirely below the aggregate
\* fresh-self bundle.  Candidate service uses its exact selected-owner weak
\* fairness; a producer handoff first closes the finite global selector and
\* then the route-only physical rank.  The semantic replacement/replenishment
\* arms can only shrink the frozen owner-universe complement.
THEOREM
    AsyncLiveProvidesAdequateLeaderFixedPipelineOriginNonDescentEpisodeStep ==
  \A initialContext:
    AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStep,
   AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness,
   AsyncLiveProvidesAdequateLeaderAsyncNextBehavior,
   AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders,
   AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep,
   AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry,
   AdequateLeaderFixedGlobalBlockerProvidersSupplyRankStep,
   AdequateLeaderFixedGlobalBlockerRankClosesOwnerSelection,
   AdequateLeaderFixedPreCandidateSelectionAndFairnessSupplyEntryService,
   AdequateLeaderFixedGlobalSelectionAndPreCandidateServiceSupplyRawRouteStep,
   AdequateLeaderFixedPreCandidateRawRouteStepClosesRank,
   AdequateLeaderFixedCandidateFairnessAndRawRouteClosureSupplyEpisodeStep

\* The roster-bound receipt makes every pre-deadline timeout exit impossible.
\* The frozen dual quorum excludes a same-or-higher TC, while bracket steps
\* preserve the immutable context/view/leader key, the real node deadlines,
\* and the receipt's source clock until Decision or its exact deadline.
THEOREM AdequateLeaderAuthorityDeadlineWindowFollowsAsyncStep ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    => AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider
BY DualQuorumIntersectionHasHonest,
   AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   AsyncFixedCorridorDeadlineAsyncNextClockIsMonotone,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncNextPreservesCandidateServiceTombstoneLifecycle,
   GstAsyncStepIsMonotone,
   PostGstAsyncBracketAdvancesEveryNodeView,
   ExecutePersistInstallAdvancesCertifiedView,
   IsaT(15000)
   DEF AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider,
       AdequateLeaderAuthorityDeadlineFreshSelfWindowActive,
       AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderCorridorAuthorityReceipt,
       AdequateLeaderCorridorAuthorityReceiptValid,
       AdequateLeaderResponsiveViewSynchronized,
       AdequateLeaderActiveTargetLeaderServiceWindow,
       AdequateLeaderActiveNodeServiceWindow,
       AdequateLeaderFrozenResponsiveRoster,
       FormedTimeoutCertificatesSound,
       CurrentIntentViewsBound,
       TimeoutVotesBindCertificate,
       TimeoutSignerSet,
       NodeTimedOut,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncNext, AsyncAllVars

THEOREM AsyncLiveProvidesAdequateLeaderAuthorityDeadlineNoPrematureExitStep ==
  \A initialContext:
    AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AdequateLeaderAuthorityDeadlineWindowFollowsAsyncStep,
   PTL
   DEF AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty

\* Exact conjunction proved in this slice.  The semantic-occurrence/physical
\* cut bridge remains deliberately separate below; this operator must not be
\* mistaken for the complete fresh-self quantitative bundle.
AdequateLeaderAuthorityDeadlineFreshSelfSafetyActionProviderProperties(
    specification) ==
  /\ AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty(
       specification)
  /\ AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty(
       specification)
  /\ AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty(
       specification)
  /\ AdequateLeaderFixedCutPerActionProviderProperty(specification)
  /\ AdequateLeaderFixedSelectedActionClockCarryProviderProperty(
       specification)
  /\ AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty(
       specification)
  /\ AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty(
       specification)

THEOREM AsyncLiveProvidesAdequateLeaderAuthorityDeadlineFreshSelfSafetyActions ==
  \A initialContext:
    AdequateLeaderAuthorityDeadlineFreshSelfSafetyActionProviderProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderAuthorityDeadlineImmediateSourceEntry,
   AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry,
   AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory,
   AsyncLiveProvidesAdequateLeaderFixedCutPerAction,
   AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry,
   AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep,
   AsyncLiveProvidesAdequateLeaderAuthorityDeadlineNoPrematureExitStep
   DEF
     AdequateLeaderAuthorityDeadlineFreshSelfSafetyActionProviderProperties

AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle(
    specification) ==
  /\ ModelConfiguration
  /\ AdequateLeaderFixedConfiguredPipelineBudgetCompatibility
  /\ AdequateLeaderFixedConfiguredDeadlineCompatibility
  /\ (specification => []AsyncStrongTypeInvariant)
  /\ AdequateLeaderAsyncNextBehaviorProperty(specification)
  /\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification)
  /\ AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty(
       specification)
  /\ AdequateLeaderFixedSubjectReplacementProviderProperties(
       specification)
  /\ AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty(
       specification)
  /\ AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty(
       specification)
  /\ AdequateLeaderFixedCutPerActionProviderProperty(specification)
  /\ AdequateLeaderFixedSelectedActionClockCarryProviderProperty(
       specification)
  /\ AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty(
       specification)
  /\ AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty(
       specification)
  /\ AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty(
       specification)
  /\ AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty(
       specification)
  /\ AdequateLeaderFixedGlobalBlockerProviderProperty(specification)
  /\ AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty(
       specification)
  /\ AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty(
       specification)

\* `ModelConfiguration` is obtained from the real Async initialization, not
\* postulated at this release boundary.  The two accounting equalities are
\* pure consequences of that configuration and contain no temporal credit.
THEOREM AsyncLiveSpecSuppliesAdequateLeaderConfiguredBudget ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => /\ ModelConfiguration
         /\ AdequateLeaderFixedConfiguredPipelineBudgetCompatibility
         /\ AdequateLeaderFixedConfiguredDeadlineCompatibility
BY AdequateLeaderFixedConfiguredBudgetCompatibilityIsDischarged, Isa
   DEF AsyncLiveSpecAt, AsyncSpecAt, AsyncInitAt, AsyncBaseInitAt, InitAt

THEOREM
    AsyncLiveProvidesAdequateLeaderAuthorityDeadlineDecisionRetention ==
  \A initialContext:
    AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecProvidesAdequateLeaderAuthorityDeadlineDecisionRetention,
   PTL
   DEF AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty

\* This is an implication from the actual live behavior because the bundle
\* deliberately contains the state-independent model configuration as a
\* conjunct.  Every provider property inside the bundle is itself stated over
\* the same AsyncLive behavior; no consumer theorem is imported here.
THEOREM
    AsyncLiveSpecSuppliesAdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecSuppliesAdequateLeaderConfiguredBudget,
   AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveProvidesAdequateLeaderAsyncNextBehavior,
   AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness,
   AsyncLiveProvidesAdequateLeaderAuthorityDeadlineImmediateSourceEntry,
   AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementProviders,
   AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry,
   AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory,
   AsyncLiveProvidesAdequateLeaderFixedCutPerAction,
   AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry,
   AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep,
   AsyncLiveProvidesAdequateLeaderFixedPipelineOriginNonDescentEpisodeStep,
   AsyncLiveProvidesAdequateLeaderRetainedProducerNonDescentEpisodeStep,
   AdequateLeaderFiniteRetainedProducerBudgetClosesNonDescentEpisode,
   AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders,
   AsyncLiveProvidesAdequateLeaderAuthorityDeadlineNoPrematureExitStep,
   AsyncLiveProvidesAdequateLeaderAuthorityDeadlineDecisionRetention,
   PTL
   DEF AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle

THEOREM AdequateLeaderAuthorityDeadlineFreshSelfBundleClosesPrimitiveRanks ==
  \A specification:
    AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle(
      specification)
      => /\ AdequateLeaderFixedSelectedOwnerServiceProperty(specification)
         /\ AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty(
              specification)
         /\ AdequateLeaderFixedPreCandidateEntryServiceProperty(specification)
         /\ AdequateLeaderFixedPreAdmissionSubjectReplacementClosureProperty(
              specification)
         /\ AdequateLeaderFixedSubjectReplacementBudgetDescentProperty(
              specification)
         /\ AdequateLeaderFixedSubjectReplacementClosureProperty(
              specification)
         /\ AdequateLeaderFixedGlobalBlockerRankStepProperty(specification)
         /\ AdequateLeaderFixedGlobalBlockerSelectionClosureProperty(
              specification)
         /\ AdequateLeaderFixedPipelineServiceRankDescentProperty(
              specification)
         /\ AdequateLeaderFixedPipelineServiceRankClosureProperty(
              specification)
         /\ AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty(
              specification)
BY AdequateLeaderFixedPipelineOriginEpisodeStepClosesNonDescentEpisode,
   AdequateLeaderFiniteRetainedProducerBudgetClosesNonDescentEpisode,
   AdequateLeaderFixedPreAdmissionBudgetClosesWithoutOrdinalRecreation,
   AdequateLeaderFixedSubjectReplacementServicesSupplyBudgetDescent,
   AdequateLeaderFixedSubjectReplacementBudgetDescentClosesEpisode,
   AdequateLeaderFixedOriginEpisodeClosureSuppliesSelectedOwnerService,
   AdequateLeaderFixedPreCandidateSelectionAndFairnessSupplyEntryService,
   AdequateLeaderFixedSelectedServicesSupplyPipelineRankDescent,
   AdequateLeaderFixedGlobalBlockerProvidersSupplyRankStep,
   AdequateLeaderFixedGlobalBlockerRankClosesOwnerSelection,
   AdequateLeaderFixedGlobalSelectionAndSelectedServiceSupplyPipelineRankDescent,
   AdequateLeaderFixedPipelineRankDescentClosesService,
   AdequateLeaderAuthorityDeadlineStepCarryPreventsPrematureExit
   DEF AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle,
       AdequateLeaderFixedSubjectReplacementProviderProperties

THEOREM AdequateLeaderAuthorityDeadlineFreshSelfBundleSuppliesPipelineService ==
  \A specification:
    AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle(
      specification)
      => AdequateLeaderAuthorityDeadlineFreshSelfPipelineServiceProperty(
           specification)
BY AdequateLeaderAuthorityDeadlineImmediateEntryStartsSubjectEpisode,
   AdequateLeaderAuthorityDeadlineFreshSelfBundleClosesPrimitiveRanks,
   AdequateLeaderAuthorityDeadlineSubjectEpisodeStartsFreshSelfRank,
   AdequateLeaderAuthorityDeadlineFreshRankClosesSelfDecision
   DEF AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle

THEOREM AdequateLeaderAuthorityDeadlineFreshSelfPipelineSuppliesFixedDeadlineService ==
  \A specification:
    AdequateLeaderAuthorityDeadlineFreshSelfPipelineServiceProperty(
      specification)
      => AdequateLeaderFixedCorridorDeadlineServiceProperty(specification)
BY PTL, Isa
   DEF AdequateLeaderAuthorityDeadlineFreshSelfPipelineServiceProperty,
       AdequateLeaderAuthorityDeadlineFreshSource,
       AdequateLeaderAuthorityDeadlineTargetDecision,
       AdequateLeaderFixedCorridorDeadlineServiceProperty,
       AdequateLeaderFixedCorridorDeadlineSource,
       AdequateLeaderFixedCorridorDecisionBeforeDeadline,
       AdequateLeaderAuthorityDeadlineReceipt,
       AsyncFixedCorridorDeadlineReceipt

THEOREM AdequateLeaderAuthorityDeadlineFreshSelfBundleSuppliesFixedDeadlineService ==
  \A specification:
    AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle(
      specification)
      => AdequateLeaderFixedCorridorDeadlineServiceProperty(specification)
BY AdequateLeaderAuthorityDeadlineFreshSelfBundleSuppliesPipelineService,
   AdequateLeaderAuthorityDeadlineFreshSelfPipelineSuppliesFixedDeadlineService

(***************************************************************************
Fresh-self Decision plus qualitative responsive dissemination.

This is the only downstream authority interface.  It contains no arbitrary
active-receipt acquisition, no stored-receipt reachability, and no
quantitative CommitQC stage claim.
***************************************************************************)

AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
    specification) ==
  /\ AdequateLeaderFixedCorridorDeadlineServiceProperty(specification)
  /\ StarvationFreedomProperty(specification)
  /\ AdequateLeaderResponsiveDecisionDisseminationProperty(specification)

THEOREM AsyncLiveFreshSelfBundleSuppliesFixedDeadlineAndResponsiveDissemination ==
  \A initialContext:
    AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderAuthorityDeadlineFreshSelfBundleSuppliesFixedDeadlineService,
   StarvationFreedomObligation,
   AsyncLiveProvidesResponsiveDecisionDissemination
   DEF AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty

\* Release-facing concrete consequence.  Under an actual AsyncLive behavior
\* the freshly constructed provider bundle above closes the quantitative
\* deadline; starvation and responsive Decision dissemination use their
\* already-proved exact scheduler owners.
THEOREM
    AsyncLiveSpecSuppliesAdequateLeaderFixedDeadlineAndResponsiveDissemination ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecSuppliesAdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle,
   AsyncLiveFreshSelfBundleSuppliesFixedDeadlineAndResponsiveDissemination

THEOREM AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence ==
  \A specification:
    /\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
    /\ AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
         specification)
      => AdequateLeaderLocalTargetDecisionConvergenceProperty(specification)
BY AdequateLeaderFixedDeadlineServiceClosesFreshSelfCorridor,
   PTL, Isa
   DEF AdequateLeaderLocalFreshSelfCorridorExposureProperty,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty,
       AdequateLeaderFreshSelfLeaderDecisionProperty,
       AdequateLeaderResponsiveDecisionDisseminationProperty,
       AdequateLeaderLocalTargetDecisionConvergenceProperty,
       AdequateLeaderLocalTargetDecisionSource,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch

=============================================================================
