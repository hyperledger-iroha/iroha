---- MODULE SumeragiKuraCommitRetryGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for the Sumeragi Kura durability and commit retry
gate.

This slice models the consensus-side contracts in
`kura_and_state_aligned_for_block`, `PendingBlock::mark_kura_persisted`,
`Actor::handle_kura_store_failure`, the Kura backoff/abort checks in the commit
pipeline, and the state-commit failure branches after a block is durable in
Kura. The model abstracts concrete blocks and hashes into representative
boundary cases while preserving the key safety obligations: Kura/state
alignment is fail-closed, retry backoff keeps the pending block without cleanup,
retry exhaustion removes the unsafe pending block and resets consensus anchors,
already durable blocks reset retry state before replay, already committed
blocks are dropped as duplicates, and state-commit failures only clean or
requeue the exact evidence required by the observed state head.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

AlignNoKura == 1
AlignMissingStateTip == 2
AlignLowerStateHeight == 3
AlignWrongStateHash == 4
AlignKuraStateTip == 5
KuraBackoffDefers == 6
KuraAbortedCleans == 7
AlreadyDurableMarksPending == 8
AlreadyCommittedSkips == 9
StoreFailureRetry == 10
StoreFailureExhausted == 11
StateHeightMismatchAligned == 12
StateHeightMismatchConflict == 13
StateCommitOtherFailure == 14
CommitMissingQcDefers == 15
CommitBeforeTipDefers == 16
AbortedWithoutQcDefers == 17
AbortedWithQcRevives == 18
RetiredWithoutQcDefers == 19
RetiredWithQcProceed == 20
MarkPersistedResetsRetry == 21
ResetQcWithFallback == 22
ResetQcWithoutFallback == 23

Candidates == 1..23

NoBug == 0
AcceptNoKuraAlignmentBug == 1
AcceptMissingTipAlignmentBug == 2
AcceptLowerHeightAlignmentBug == 3
AcceptWrongHashAlignmentBug == 4
RejectAlignedTipBug == 5
BackoffFinalizesBug == 6
AbortedKeepsPendingBug == 7
AbortedSkipsCleanupBug == 8
AlreadyDurableSkipsMarkBug == 9
AlreadyCommittedKeepsPendingBug == 10
StoreRetryDropsPendingBug == 11
StoreRetryCleansHashBug == 12
StoreExhaustedKeepsPendingBug == 13
StoreExhaustedSkipsCleanupBug == 14
StateAlignedKeepsPendingBug == 15
StateAlignedCleansBlockHashBug == 16
StateConflictKeepsPendingBug == 17
StateConflictSkipsViewChangeBug == 18
StateConflictSkipsRequeueBug == 19
StateOtherDropsPendingBug == 20
StateOtherForgetsKuraPersistedBug == 21
MissingQcFinalizesBug == 22
BeforeTipFinalizesBug == 23
AbortedWithoutQcFinalizesBug == 24
AbortedWithQcStaysAbortedBug == 25
RetiredWithoutQcFinalizesBug == 26
RetiredWithQcDefersBug == 27
MarkPersistedKeepsRetryBug == 28
ResetQcDropsFallbackBug == 29
ResetQcRetainsStaleBug == 30

Bugs == 0..30

AlignTrue == 1
AlignFalse == 2
KeepPending == 3
DropPending == 4
RetryScheduled == 5
RetryAttemptIncrement == 6
RetryReset == 7
KuraPersisted == 8
CleanBlockHash == 9
CleanRbc == 10
CleanParentQc == 11
CleanBlockQc == 12
ResetQcFallback == 13
TriggerViewChange == 14
RequeueTx == 15
FinalizeDeferred == 16
FinalizeProceed == 17
FinalizeSkipAlreadyCommitted == 18
CleanProposalHint == 19
CleanProposalCache == 20
ClearStaleQc == 21

Actions == 1..21

SpecActions(candidate) ==
  CASE candidate = AlignNoKura -> {AlignFalse}
    [] candidate = AlignMissingStateTip -> {AlignFalse}
    [] candidate = AlignLowerStateHeight -> {AlignFalse}
    [] candidate = AlignWrongStateHash -> {AlignFalse}
    [] candidate = AlignKuraStateTip -> {AlignTrue}
    [] candidate = KuraBackoffDefers -> {KeepPending, FinalizeDeferred}
    [] candidate = KuraAbortedCleans ->
      {DropPending, CleanRbc, CleanBlockQc, ResetQcFallback,
       TriggerViewChange, FinalizeDeferred}
    [] candidate = AlreadyDurableMarksPending ->
      {KeepPending, KuraPersisted, RetryReset, FinalizeProceed}
    [] candidate = AlreadyCommittedSkips ->
      {DropPending, FinalizeSkipAlreadyCommitted, CleanRbc, CleanParentQc}
    [] candidate = StoreFailureRetry ->
      {KeepPending, RetryScheduled, RetryAttemptIncrement, FinalizeDeferred}
    [] candidate = StoreFailureExhausted ->
      {DropPending, CleanBlockHash, CleanRbc, CleanBlockQc, ResetQcFallback,
       TriggerViewChange, RequeueTx, FinalizeDeferred}
    [] candidate = StateHeightMismatchAligned ->
      {DropPending, CleanRbc, CleanParentQc, FinalizeSkipAlreadyCommitted}
    [] candidate = StateHeightMismatchConflict ->
      {DropPending, CleanBlockHash, CleanRbc, CleanBlockQc, CleanProposalHint,
       CleanProposalCache, TriggerViewChange, RequeueTx, FinalizeDeferred}
    [] candidate = StateCommitOtherFailure ->
      {KeepPending, KuraPersisted, RetryReset, FinalizeDeferred}
    [] candidate = CommitMissingQcDefers -> {KeepPending, FinalizeDeferred}
    [] candidate = CommitBeforeTipDefers -> {KeepPending, FinalizeDeferred}
    [] candidate = AbortedWithoutQcDefers -> {KeepPending, FinalizeDeferred}
    [] candidate = AbortedWithQcRevives -> {KeepPending, FinalizeProceed}
    [] candidate = RetiredWithoutQcDefers -> {KeepPending, FinalizeDeferred}
    [] candidate = RetiredWithQcProceed -> {KeepPending, FinalizeProceed}
    [] candidate = MarkPersistedResetsRetry -> {KuraPersisted, RetryReset}
    [] candidate = ResetQcWithFallback -> {ResetQcFallback}
    [] candidate = ResetQcWithoutFallback -> {ResetQcFallback}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = AlignNoKura /\ Bug = AcceptNoKuraAlignmentBug ->
      (spec \ {AlignFalse}) \cup {AlignTrue}
    [] candidate = AlignMissingStateTip /\ Bug = AcceptMissingTipAlignmentBug ->
      (spec \ {AlignFalse}) \cup {AlignTrue}
    [] candidate = AlignLowerStateHeight /\ Bug = AcceptLowerHeightAlignmentBug ->
      (spec \ {AlignFalse}) \cup {AlignTrue}
    [] candidate = AlignWrongStateHash /\ Bug = AcceptWrongHashAlignmentBug ->
      (spec \ {AlignFalse}) \cup {AlignTrue}
    [] candidate = AlignKuraStateTip /\ Bug = RejectAlignedTipBug ->
      (spec \ {AlignTrue}) \cup {AlignFalse}
    [] candidate = KuraBackoffDefers /\ Bug = BackoffFinalizesBug ->
      (spec \ {FinalizeDeferred}) \cup {FinalizeProceed}
    [] candidate = KuraAbortedCleans /\ Bug = AbortedKeepsPendingBug ->
      (spec \ {DropPending}) \cup {KeepPending}
    [] candidate = KuraAbortedCleans /\ Bug = AbortedSkipsCleanupBug ->
      spec \ {CleanRbc, CleanBlockQc, ResetQcFallback, TriggerViewChange}
    [] candidate = AlreadyDurableMarksPending /\
          Bug = AlreadyDurableSkipsMarkBug ->
      spec \ {KuraPersisted, RetryReset}
    [] candidate = AlreadyCommittedSkips /\
          Bug = AlreadyCommittedKeepsPendingBug ->
      (spec \ {DropPending}) \cup {KeepPending}
    [] candidate = StoreFailureRetry /\ Bug = StoreRetryDropsPendingBug ->
      (spec \ {KeepPending}) \cup {DropPending}
    [] candidate = StoreFailureRetry /\ Bug = StoreRetryCleansHashBug ->
      spec \cup {CleanBlockHash}
    [] candidate = StoreFailureExhausted /\
          Bug = StoreExhaustedKeepsPendingBug ->
      (spec \ {DropPending}) \cup {KeepPending}
    [] candidate = StoreFailureExhausted /\
          Bug = StoreExhaustedSkipsCleanupBug ->
      spec \ {CleanBlockHash, CleanRbc, CleanBlockQc, ResetQcFallback,
              TriggerViewChange, RequeueTx}
    [] candidate = StateHeightMismatchAligned /\
          Bug = StateAlignedKeepsPendingBug ->
      (spec \ {DropPending}) \cup {KeepPending}
    [] candidate = StateHeightMismatchAligned /\
          Bug = StateAlignedCleansBlockHashBug ->
      spec \cup {CleanBlockHash}
    [] candidate = StateHeightMismatchConflict /\
          Bug = StateConflictKeepsPendingBug ->
      (spec \ {DropPending}) \cup {KeepPending}
    [] candidate = StateHeightMismatchConflict /\
          Bug = StateConflictSkipsViewChangeBug ->
      spec \ {TriggerViewChange}
    [] candidate = StateHeightMismatchConflict /\
          Bug = StateConflictSkipsRequeueBug ->
      spec \ {RequeueTx}
    [] candidate = StateCommitOtherFailure /\
          Bug = StateOtherDropsPendingBug ->
      (spec \ {KeepPending}) \cup {DropPending}
    [] candidate = StateCommitOtherFailure /\
          Bug = StateOtherForgetsKuraPersistedBug ->
      spec \ {KuraPersisted, RetryReset}
    [] candidate = CommitMissingQcDefers /\ Bug = MissingQcFinalizesBug ->
      (spec \ {FinalizeDeferred}) \cup {FinalizeProceed}
    [] candidate = CommitBeforeTipDefers /\ Bug = BeforeTipFinalizesBug ->
      (spec \ {FinalizeDeferred}) \cup {FinalizeProceed}
    [] candidate = AbortedWithoutQcDefers /\
          Bug = AbortedWithoutQcFinalizesBug ->
      (spec \ {FinalizeDeferred}) \cup {FinalizeProceed}
    [] candidate = AbortedWithQcRevives /\ Bug = AbortedWithQcStaysAbortedBug ->
      (spec \ {FinalizeProceed}) \cup {FinalizeDeferred}
    [] candidate = RetiredWithoutQcDefers /\
          Bug = RetiredWithoutQcFinalizesBug ->
      (spec \ {FinalizeDeferred}) \cup {FinalizeProceed}
    [] candidate = RetiredWithQcProceed /\ Bug = RetiredWithQcDefersBug ->
      (spec \ {FinalizeProceed}) \cup {FinalizeDeferred}
    [] candidate = MarkPersistedResetsRetry /\ Bug = MarkPersistedKeepsRetryBug ->
      spec \ {RetryReset}
    [] candidate = ResetQcWithFallback /\ Bug = ResetQcDropsFallbackBug ->
      {}
    [] candidate = ResetQcWithoutFallback /\ Bug = ResetQcRetainsStaleBug ->
      {}
    [] OTHER -> spec

Init ==
  tried = {}

Next ==
  \E candidate \in Candidates \ tried:
    tried' = tried \cup {candidate}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried: ImplementationActions(candidate) \subseteq Actions

Safety ==
  \A candidate \in tried:
    ImplementationActions(candidate) = SpecActions(candidate)

====
