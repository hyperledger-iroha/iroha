---- MODULE SumeragiV2ChainEpochRefinementShard05 ----
EXTENDS SumeragiV2ChainEpochRefinementShard04

THEOREM IndexedFixedCorridorDeadlineProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncFixedCorridorDeadlineReceipts
           = IndexedFixedCorridorDeadlines(initialContext)
BY DEF IndexedAsync!AsyncFixedCorridorDeadlineReceipts,
       IndexedFixedCorridorDeadlines

THEOREM IndexedServeProducerEpisodeDueProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncServeProducerEpisodeDebt
           = IndexedServeProducerEpisodeDue(initialContext)
BY DEF IndexedAsync!AsyncServeProducerEpisodeDebt,
       IndexedServeProducerEpisodeDue

(***************************************************************************
The producer journal is part of the authoritative transition state.  These
equalities prevent indexed contexts from aliasing a hidden global journal and
pin the known-obligation, consumed-episode, and origin-history order used by
the finite producer ranks.
***************************************************************************)
THEOREM IndexedThreeFieldProducerProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncProducerVars =
              indexedAsyncState[initialContext][5]
         /\ indexedAsyncState[initialContext][5] =
              <<IndexedProducer(initialContext, 1),
                IndexedProducer(initialContext, 2),
                IndexedProducer(initialContext, 3)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncProducerVars, IndexedProducer

THEOREM VerificationThreeFieldProducerProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => /\ VerificationAsyncProof!AsyncProducerVars =
             indexedAsyncState[VerificationContext][5]
       /\ indexedAsyncState[VerificationContext][5] =
            <<VerificationProducer(1), VerificationProducer(2),
              VerificationProducer(3)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncProducerVars,
           VerificationProducer, IndexedProducer

THEOREM VerificationInstanceVariablesAreExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => VerificationAsyncProof!AsyncAllVars =
       IndexedAsyncStateAt(VerificationContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       VerificationAsyncProof!AsyncAllVars,
       VerificationAsyncProof!AsyncSchedulerVars,
       VerificationAsyncProof!AsyncRecoveryVars,
       VerificationAsyncProof!AsyncProducerVars,
       VerificationAsyncProof!vars,
       VerificationCore, VerificationScheduler, VerificationRecovery,
       VerificationProducer,
       VerificationFixedCorridorDeadlines,
       VerificationServeProducerEpisodeDue,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines,
       IndexedServeProducerEpisodeDue

(***************************************************************************
The seven Serve lifecycle fields are pinned separately from the aggregate
scheduler tuple.  This prevents an arity-correct WITH clause from silently
dropping the retained-attempt field at index 17, shifting every later owner,
and thereby erasing immutable admission, tombstone, or retry-coalescing state
from the indexed liveness product.
***************************************************************************)
THEOREM IndexedSevenFieldServeLifecycleProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncServeLifecycleVars =
              <<IndexedScheduler(initialContext, 11),
                IndexedScheduler(initialContext, 14),
                IndexedScheduler(initialContext, 15),
                IndexedScheduler(initialContext, 16),
                IndexedScheduler(initialContext, 17)>>
         /\ IndexedAsync(initialContext)!AsyncServeIngressAdmissionVars =
              <<IndexedScheduler(initialContext, 12),
                IndexedScheduler(initialContext, 13)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncServeLifecycleVars,
           IndexedAsync!AsyncServeIngressAdmissionVars,
           IndexedScheduler

THEOREM VerificationSevenFieldServeLifecycleProjectionIsExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => /\ VerificationAsyncProof!AsyncServeLifecycleVars =
           <<VerificationScheduler(11), VerificationScheduler(14),
             VerificationScheduler(15), VerificationScheduler(16),
             VerificationScheduler(17)>>
     /\ VerificationAsyncProof!AsyncServeIngressAdmissionVars =
           <<VerificationScheduler(12), VerificationScheduler(13)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncServeLifecycleVars,
           VerificationAsyncProof!AsyncServeIngressAdmissionVars,
           VerificationScheduler, IndexedScheduler

(***************************************************************************
The appended service-activation record is pinned independently.  This keeps
all reviewed scheduler indices 1..45 stable while preventing an arity-correct
instance from dropping or aliasing the irreversible restriction tombstone.
***************************************************************************)
THEOREM IndexedServiceActivationProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
       IndexedAsync(initialContext)!AsyncSchedulerVars[46]
           = IndexedScheduler(initialContext, 46)
BY DEF IndexedAsync!AsyncSchedulerVars, IndexedScheduler

THEOREM VerificationServiceActivationProjectionIsExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => VerificationAsyncProof!AsyncSchedulerVars[46]
       = VerificationScheduler(46)
BY DEF VerificationAsyncProof!AsyncSchedulerVars,
       VerificationScheduler, IndexedScheduler

THEOREM IndexedLeaderWireLifecycleProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncSchedulerVars[42]
           = IndexedScheduler(initialContext, 42)
BY DEF IndexedAsync!AsyncSchedulerVars, IndexedScheduler

THEOREM VerificationLeaderWireLifecycleProjectionIsExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => VerificationAsyncProof!AsyncSchedulerVars[42]
       = VerificationScheduler(42)
BY DEF VerificationAsyncProof!AsyncSchedulerVars,
       VerificationScheduler, IndexedScheduler

(***************************************************************************
The recovery projection is extensional, not merely length-compatible.  These
facts pin the five production fields at the chain-composition boundary and
prevent a future WITH-clause edit from silently dropping, duplicating, or
reordering recovery phase, owner, generation, replay-queue state, or the
historical-lock restart authority.
***************************************************************************)
THEOREM IndexedFiveFieldRecoveryProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncRecoveryVars =
              indexedAsyncState[initialContext][4]
         /\ indexedAsyncState[initialContext][4] =
              <<IndexedRecovery(initialContext, 1),
                IndexedRecovery(initialContext, 2),
                IndexedRecovery(initialContext, 3),
                IndexedRecovery(initialContext, 4),
                IndexedRecovery(initialContext, 5)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncRecoveryVars, IndexedRecovery

THEOREM VerificationFiveFieldRecoveryProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => /\ VerificationAsyncProof!AsyncRecoveryVars =
             indexedAsyncState[VerificationContext][4]
       /\ indexedAsyncState[VerificationContext][4] =
            <<VerificationRecovery(1), VerificationRecovery(2),
              VerificationRecovery(3), VerificationRecovery(4),
              VerificationRecovery(5)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncRecoveryVars,
           VerificationRecovery, IndexedRecovery

THEOREM IndexedHistoricalRecoveryTargetProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         \A node \in ValidatorIds:
           IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
             <=> node \in IndexedScheduler(initialContext, 44)
BY DEF IndexedAsync!HistoricalRecoveryTarget

THEOREM VerificationHistoricalRecoveryTargetProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => \A node \in ValidatorIds:
         VerificationAsyncProof!HistoricalRecoveryTarget(node)
           <=> node \in VerificationScheduler(44)
BY DEF VerificationAsyncProof!HistoricalRecoveryTarget

THEOREM IndexedInitProjectsEveryAsyncInit ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainInit =>
      IndexedAsync(initialContext)!AsyncInitAt(initialContext)
BY DEF IndexedChainInit

(***************************************************************************
This fact is derived from the exact InitAt payload, rather than from the
ChainEpoch equality in IndexedChainInit. Non-genesis instances retain their
synthetic parent receipt internally, but its context/height is strictly below
the frozen instance and hence absent from both projected current-receipt sets.
***************************************************************************)
=============================================================================
