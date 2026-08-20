---- MODULE SumeragiV2DurableValidateLifecycleMutation ----
EXTENDS Naturals

(***************************************************************************
Bounded mutation model for the production-owned durable Validate lifecycle.

The model isolates five exact safety cuts:

  1. Ready ExecuteBody work reserves worker capacity, becomes Waiting, and
     accepts only a guarded completion for the same immutable row;
  2. a missing merge sidecar remains attached to that Waiting row and an exact
     sidecar wake returns the same ordinal to Ready without minting another;
  3. a rejected replacement Ready row is claimed only after its report output
     slot has been reserved, and consumes that slot exactly once;
  4. every admitted replay origin reopens with the same mandatory authority;
  5. an ambiguous error after LedgerV1 fsync crosses the fail-stop boundary
     instead of publishing volatile report state.

This is finite TLC mutation evidence. It neither proves the unbounded runtime
nor promotes a proof-ledger obligation.
***************************************************************************)

CONSTANTS EnabledScenarios,
          GuardExactCompletion,
          PreserveSidecarWaitingOrdinal,
          ReserveRejectedOutputBeforeClaim,
          RestoreReplayAuthorityAfterRestart,
          FailStopOnAmbiguousPostFsync

GuardedCompletionScenario == "GuardedCompletion"
SidecarWakeScenario == "SidecarWake"
RejectedOutputScenario == "RejectedOutput"
RestartReplayScenario == "RestartReplay"
AmbiguousPostFsyncScenario == "AmbiguousPostFsync"

Scenarios ==
  {GuardedCompletionScenario,
   SidecarWakeScenario,
   RejectedOutputScenario,
   RestartReplayScenario,
   AmbiguousPostFsyncScenario}

ReplayOrigins ==
  {"LiveWal",
   "LocalBody",
   "RemoteProposal",
   "InvalidBodyReport",
   "DirectSigned"}

ASSUME /\ EnabledScenarios \in (SUBSET Scenarios)
       /\ EnabledScenarios # {}
       /\ GuardExactCompletion \in BOOLEAN
       /\ PreserveSidecarWaitingOrdinal \in BOOLEAN
       /\ ReserveRejectedOutputBeforeClaim \in BOOLEAN
       /\ RestoreReplayAuthorityAfterRestart \in BOOLEAN
       /\ FailStopOnAmbiguousPostFsync \in BOOLEAN

ReadyExecute == "ReadyExecute"
WaitingValidate == "WaitingValidate"
GuardedCompletion == "GuardedCompletion"
WaitingSidecar == "WaitingSidecar"
ReadyResult == "ReadyResult"
ClaimedRejectedResult == "ClaimedRejectedResult"
LedgerFsynced == "LedgerFsynced"
ReportPublished == "ReportPublished"
FailStop == "FailStop"

Phases ==
  {ReadyExecute,
   WaitingValidate,
   GuardedCompletion,
   WaitingSidecar,
   ReadyResult,
   ClaimedRejectedResult,
   LedgerFsynced,
   ReportPublished,
   FailStop}

NoResult == "NoResult"
ValidatedResult == "ValidatedResult"
RejectedResult == "RejectedResult"
DeferredSidecarResult == "DeferredSidecarResult"
ResultKinds ==
  {NoResult, ValidatedResult, RejectedResult, DeferredSidecarResult}

ExactRow == "ValidateRow"
ForeignRow == "ForeignValidateRow"
NoRow == "NoRow"
Rows == {ExactRow, ForeignRow, NoRow}

InitialOrdinal == 1
InitialNextOrdinal == 2
OrdinalCeiling == 3

ExactSidecarRegistration ==
  [row |-> ExactRow,
   reference |-> "ExpectedMergeSidecar",
   owner |-> "ValidateOwner",
   generation |-> 7]

NoSidecarRegistration ==
  [row |-> NoRow,
   reference |-> "NoMergeSidecar",
   owner |-> "NoOwner",
   generation |-> 0]

SidecarRegistrationDomain ==
  {ExactSidecarRegistration, NoSidecarRegistration}

NoReplayAuthority ==
  [origin |-> "NoOrigin", row |-> NoRow, generation |-> 0]

ExactReplayAuthority(origin) ==
  [origin |-> origin, row |-> ExactRow, generation |-> 1]

ReplayAuthorityDomain ==
  {NoReplayAuthority}
    \cup {ExactReplayAuthority(origin): origin \in ReplayOrigins}

VARIABLES scenario, replayOrigin, state

vars == <<scenario, replayOrigin, state>>

TypeInvariant ==
  /\ scenario \in EnabledScenarios
  /\ replayOrigin \in ReplayOrigins
  /\ state.step \in 0..6
  /\ state.phase \in Phases
  /\ state.row \in Rows
  /\ state.rowOrdinal \in InitialOrdinal..OrdinalCeiling
  /\ state.nextOrdinal \in InitialNextOrdinal..OrdinalCeiling
  /\ state.workerCapacityReserved \in BOOLEAN
  /\ state.validateClaimed \in BOOLEAN
  /\ state.completionAccepted \in BOOLEAN
  /\ state.completionAddress \in Rows
  /\ state.guardedCompletionRetained \in BOOLEAN
  /\ state.resultKind \in ResultKinds
  /\ state.sidecarRegistrationDurable \in BOOLEAN
  /\ state.sidecarRegistration \in SidecarRegistrationDomain
  /\ state.sidecarWakeObserved \in BOOLEAN
  /\ state.sidecarWakeIdentity \in SidecarRegistrationDomain
  /\ state.replacementReadyPublished \in BOOLEAN
  /\ state.rejectionOutputReserved \in BOOLEAN
  /\ state.rejectionReservationPrecededClaim \in BOOLEAN
  /\ state.rejectionResultClaimed \in BOOLEAN
  /\ state.rejectionOutputCount \in 0..1
  /\ state.durableReplayAuthority \in ReplayAuthorityDomain
  /\ state.liveReplayAuthority \in ReplayAuthorityDomain
  /\ state.restartObserved \in BOOLEAN
  /\ state.ledgerFsynced \in BOOLEAN
  /\ state.reportPublished \in BOOLEAN
  /\ state.ambiguousPostFsyncObserved \in BOOLEAN
  /\ state.failStop \in BOOLEAN

ReadyWaitingGuardedLifecycle ==
  /\ (state.step = 0 =>
        /\ state.phase = ReadyExecute
        /\ ~state.workerCapacityReserved
        /\ ~state.validateClaimed)
  /\ (state.phase = WaitingValidate =>
        /\ state.row = ExactRow
        /\ state.rowOrdinal = InitialOrdinal
        /\ state.workerCapacityReserved = state.validateClaimed)
  /\ (state.phase = GuardedCompletion =>
        /\ state.completionAccepted
        /\ state.guardedCompletionRetained
        /\ ~state.workerCapacityReserved
        /\ ~state.validateClaimed)
  /\ (state.step > 0 =>
        /\ state.durableReplayAuthority = ExactReplayAuthority(replayOrigin)
        /\ state.row = ExactRow)

GuardedCompletionMatchesClaimedRow ==
  state.completionAccepted =>
    /\ state.completionAddress = ExactRow
    /\ state.row = ExactRow

ExactSidecarWakeReusesWaitingRow ==
  state.sidecarWakeObserved =>
    /\ state.sidecarWakeIdentity = ExactSidecarRegistration
    /\ state.row = ExactRow
    /\ state.rowOrdinal = InitialOrdinal
    /\ state.nextOrdinal = InitialNextOrdinal
    /\ state.phase = ReadyExecute

RejectedResultClaimHasReservedOutput ==
  state.rejectionResultClaimed =>
    state.rejectionReservationPrecededClaim

RejectedOutputConsumesExactlyOneReservation ==
  state.rejectionOutputCount = 1 =>
    /\ state.rejectionReservationPrecededClaim
    /\ ~state.rejectionOutputReserved

RestartReopensMandatoryReplayAuthority ==
  state.restartObserved =>
    /\ state.durableReplayAuthority = ExactReplayAuthority(replayOrigin)
    /\ state.liveReplayAuthority = ExactReplayAuthority(replayOrigin)
    /\ state.row = ExactRow
    /\ state.rowOrdinal = InitialOrdinal

AmbiguousPostFsyncRequiresFailStop ==
  state.ambiguousPostFsyncObserved =>
    /\ state.ledgerFsynced
    /\ state.failStop
    /\ state.phase = FailStop
    /\ ~state.reportPublished

Init ==
  /\ scenario \in EnabledScenarios
  /\ replayOrigin \in ReplayOrigins
  /\ state =
       [step |-> 0,
        phase |-> ReadyExecute,
        row |-> ExactRow,
        rowOrdinal |-> InitialOrdinal,
        nextOrdinal |-> InitialNextOrdinal,
        workerCapacityReserved |-> FALSE,
        validateClaimed |-> FALSE,
        completionAccepted |-> FALSE,
        completionAddress |-> NoRow,
        guardedCompletionRetained |-> FALSE,
        resultKind |-> NoResult,
        sidecarRegistrationDurable |-> FALSE,
        sidecarRegistration |-> NoSidecarRegistration,
        sidecarWakeObserved |-> FALSE,
        sidecarWakeIdentity |-> NoSidecarRegistration,
        replacementReadyPublished |-> FALSE,
        rejectionOutputReserved |-> FALSE,
        rejectionReservationPrecededClaim |-> FALSE,
        rejectionResultClaimed |-> FALSE,
        rejectionOutputCount |-> 0,
        durableReplayAuthority |-> NoReplayAuthority,
        liveReplayAuthority |-> NoReplayAuthority,
        restartObserved |-> FALSE,
        ledgerFsynced |-> FALSE,
        reportPublished |-> FALSE,
        ambiguousPostFsyncObserved |-> FALSE,
        failStop |-> FALSE]

BeginValidate ==
  /\ state.step = 0
  /\ state.phase = ReadyExecute
  /\ state' =
       [state EXCEPT
          !.step = 1,
          !.phase = WaitingValidate,
          !.workerCapacityReserved = TRUE,
          !.validateClaimed = TRUE,
          !.durableReplayAuthority = ExactReplayAuthority(replayOrigin),
          !.liveReplayAuthority = ExactReplayAuthority(replayOrigin)]
  /\ UNCHANGED <<scenario, replayOrigin>>

WorkerReturnsGuardedCompletion ==
  /\ scenario # RestartReplayScenario
  /\ state.step = 1
  /\ state.phase = WaitingValidate
  /\ state.workerCapacityReserved
  /\ state.validateClaimed
  /\ state' =
       [state EXCEPT
          !.step = 2,
          !.phase = GuardedCompletion,
          !.workerCapacityReserved = FALSE,
          !.validateClaimed = FALSE,
          !.completionAccepted = TRUE,
          !.completionAddress =
             IF scenario = GuardedCompletionScenario /\ ~GuardExactCompletion
             THEN ForeignRow
             ELSE ExactRow,
          !.guardedCompletionRetained = TRUE,
          !.resultKind =
             CASE scenario = SidecarWakeScenario -> DeferredSidecarResult
               [] scenario \in
                    {RejectedOutputScenario, AmbiguousPostFsyncScenario} ->
                    RejectedResult
               [] OTHER -> ValidatedResult]
  /\ UNCHANGED <<scenario, replayOrigin>>

CrashAndReopenWaiting ==
  /\ scenario = RestartReplayScenario
  /\ state.step = 1
  /\ state.phase = WaitingValidate
  /\ state' =
       [state EXCEPT
          !.step = 2,
          !.workerCapacityReserved = FALSE,
          !.validateClaimed = FALSE,
          !.liveReplayAuthority =
             IF RestoreReplayAuthorityAfterRestart
             THEN state.durableReplayAuthority
             ELSE NoReplayAuthority,
          !.restartObserved = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

ResumeRestartedValidate ==
  /\ scenario = RestartReplayScenario
  /\ state.step = 2
  /\ state.phase = WaitingValidate
  /\ state' =
       [state EXCEPT
          !.step = 3,
          !.workerCapacityReserved = TRUE,
          !.validateClaimed = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

WorkerReturnsAfterRestart ==
  /\ scenario = RestartReplayScenario
  /\ state.step = 3
  /\ state.phase = WaitingValidate
  /\ state.liveReplayAuthority = ExactReplayAuthority(replayOrigin)
  /\ state' =
       [state EXCEPT
          !.step = 4,
          !.phase = GuardedCompletion,
          !.workerCapacityReserved = FALSE,
          !.validateClaimed = FALSE,
          !.completionAccepted = TRUE,
          !.completionAddress = ExactRow,
          !.guardedCompletionRetained = TRUE,
          !.resultKind = ValidatedResult]
  /\ UNCHANGED <<scenario, replayOrigin>>

PublishValidatedReplacementReady ==
  /\ \/ /\ scenario = GuardedCompletionScenario
         /\ state.step = 2
      \/ /\ scenario = RestartReplayScenario
         /\ state.step = 4
  /\ state.phase = GuardedCompletion
  /\ state.completionAccepted
  /\ state.resultKind = ValidatedResult
  /\ state' =
       [state EXCEPT
          !.step = @ + 1,
          !.phase = ReadyResult,
          !.guardedCompletionRetained = FALSE,
          !.replacementReadyPublished = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

RegisterExactSidecarWait ==
  /\ scenario = SidecarWakeScenario
  /\ state.step = 2
  /\ state.phase = GuardedCompletion
  /\ state.resultKind = DeferredSidecarResult
  /\ state' =
       [state EXCEPT
          !.step = 3,
          !.phase = WaitingSidecar,
          !.sidecarRegistrationDurable = TRUE,
          !.sidecarRegistration = ExactSidecarRegistration]
  /\ UNCHANGED <<scenario, replayOrigin>>

DeliverExactSidecar ==
  /\ scenario = SidecarWakeScenario
  /\ state.step = 3
  /\ state.phase = WaitingSidecar
  /\ state.sidecarRegistrationDurable
  /\ state.sidecarRegistration = ExactSidecarRegistration
  /\ state' =
       [state EXCEPT
          !.step = 4,
          !.phase = ReadyExecute,
          !.rowOrdinal =
             IF PreserveSidecarWaitingOrdinal
             THEN state.rowOrdinal
             ELSE state.nextOrdinal,
          !.nextOrdinal =
             IF PreserveSidecarWaitingOrdinal
             THEN state.nextOrdinal
             ELSE state.nextOrdinal + 1,
          !.guardedCompletionRetained = FALSE,
          !.sidecarRegistrationDurable = FALSE,
          !.sidecarRegistration = NoSidecarRegistration,
          !.sidecarWakeObserved = TRUE,
          !.sidecarWakeIdentity = ExactSidecarRegistration]
  /\ UNCHANGED <<scenario, replayOrigin>>

PublishRejectedReplacementReady ==
  /\ scenario \in {RejectedOutputScenario, AmbiguousPostFsyncScenario}
  /\ state.step = 2
  /\ state.phase = GuardedCompletion
  /\ state.resultKind = RejectedResult
  /\ state' =
       [state EXCEPT
          !.step = 3,
          !.phase = ReadyResult,
          !.guardedCompletionRetained = FALSE,
          !.replacementReadyPublished = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

ClaimRejectedResult ==
  /\ scenario \in {RejectedOutputScenario, AmbiguousPostFsyncScenario}
  /\ state.step = 3
  /\ state.phase = ReadyResult
  /\ state.resultKind = RejectedResult
  /\ state' =
       [state EXCEPT
          !.step = 4,
          !.phase = ClaimedRejectedResult,
          !.rejectionOutputReserved = ReserveRejectedOutputBeforeClaim,
          !.rejectionReservationPrecededClaim =
             ReserveRejectedOutputBeforeClaim,
          !.rejectionResultClaimed = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

PublishRejectedReport ==
  /\ scenario = RejectedOutputScenario
  /\ state.step = 4
  /\ state.phase = ClaimedRejectedResult
  /\ state' =
       [state EXCEPT
          !.step = 5,
          !.phase = ReportPublished,
          !.rejectionOutputReserved = FALSE,
          !.rejectionOutputCount = 1,
          !.ledgerFsynced = TRUE,
          !.reportPublished = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

FsyncRejectedReport ==
  /\ scenario = AmbiguousPostFsyncScenario
  /\ state.step = 4
  /\ state.phase = ClaimedRejectedResult
  /\ state' =
       [state EXCEPT
          !.step = 5,
          !.phase = LedgerFsynced,
          !.rejectionOutputReserved = FALSE,
          !.rejectionOutputCount = 1,
          !.ledgerFsynced = TRUE]
  /\ UNCHANGED <<scenario, replayOrigin>>

ObserveAmbiguousPostFsync ==
  /\ scenario = AmbiguousPostFsyncScenario
  /\ state.step = 5
  /\ state.phase = LedgerFsynced
  /\ state' =
       [state EXCEPT
          !.step = 6,
          !.phase =
             IF FailStopOnAmbiguousPostFsync THEN FailStop ELSE ReportPublished,
          !.reportPublished = ~FailStopOnAmbiguousPostFsync,
          !.ambiguousPostFsyncObserved = TRUE,
          !.failStop = FailStopOnAmbiguousPostFsync]
  /\ UNCHANGED <<scenario, replayOrigin>>

Next ==
  \/ BeginValidate
  \/ WorkerReturnsGuardedCompletion
  \/ CrashAndReopenWaiting
  \/ ResumeRestartedValidate
  \/ WorkerReturnsAfterRestart
  \/ PublishValidatedReplacementReady
  \/ RegisterExactSidecarWait
  \/ DeliverExactSidecar
  \/ PublishRejectedReplacementReady
  \/ ClaimRejectedResult
  \/ PublishRejectedReport
  \/ FsyncRejectedReport
  \/ ObserveAmbiguousPostFsync

=============================================================================
