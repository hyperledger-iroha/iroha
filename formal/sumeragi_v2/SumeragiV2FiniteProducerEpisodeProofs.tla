---- MODULE SumeragiV2FiniteProducerEpisodeProofs ----
EXTENDS SumeragiV2FiniteProducerEpisodes, FiniteSetTheorems, TLAPS

(***************************************************************************
Deductive facts at the generic finite-episode boundary.

These theorems prove journal safety, prefix preservation, exact geometry, and
well-foundedness from the kernel's explicit assumptions.  They intentionally
do not assert temporal descent for a production transition system that has
not yet been connected to this module.
***************************************************************************)

THEOREM ProducerInitEstablishesTypeInvariant ==
  ProducerInit => ProducerTypeInvariant
PROOF
  <1>1. ASSUME ProducerInit
         PROVE ProducerTypeInvariant
    <2>1. /\ ProducerConfiguration
           /\ fpKnownObligations = {}
           /\ fpConsumedEpisodes = {}
           /\ fpRankState = ProducerInitialRankState
      BY <1>1 DEF ProducerInit
    <2>2. fpConsumedEpisodes
             \subseteq
               UNION {ProducerEpisodeUniverse(obligation):
                        obligation \in fpKnownObligations}
      BY <2>1, SMT
    <2> QED BY <2>1, <2>2
         DEF ProducerTypeInvariant, ProducerConfiguration,
             ProducerRankOrderingConfiguration
  <1> QED BY <1>1

THEOREM ProducerInitEstablishesGeometryInvariant ==
  ProducerInit => ProducerGeometryInvariant
PROOF
  <1>1. ASSUME ProducerInit
         PROVE ProducerGeometryInvariant
    <2>1. /\ fpKnownObligations = {}
           /\ ProducerSourceCapacity \in Nat \ {0}
      BY <1>1
         DEF ProducerInit, ProducerConfiguration,
             ProducerGeometryConfiguration
    <2>2. \A request \in ProducerRequests:
             ProducerKnownSourcesFor(request) = {}
      BY <2>1, SMT DEF ProducerKnownSourcesFor
    <2> QED BY <2>1, <2>2, FS_EmptySet, SMT
         DEF ProducerGeometryInvariant
  <1> QED BY <1>1

THEOREM ProducerInitEstablishesConsumedPrefixInvariant ==
  ProducerInit => ProducerConsumedPrefixInvariant
BY Isa
   DEF ProducerInit, ProducerConsumedPrefixInvariant

THEOREM ProducerInitEstablishesNoResurrectionInvariant ==
  ProducerInit => ProducerNoResurrectionInvariant
BY Isa
   DEF ProducerInit, ProducerNoResurrectionInvariant

THEOREM ProducerInitEstablishesRankProjectionTypeInvariant ==
  ProducerInit => ProducerRankProjectionTypeInvariant
BY Isa
   DEF ProducerInit, ProducerRankProjectionTypeInvariant,
       ProducerConfiguration, ProducerRankOrderingConfiguration

THEOREM ProducerInitEstablishesSafetyInvariant ==
  ProducerInit => ProducerSafetyInvariant
BY ProducerInitEstablishesTypeInvariant,
   ProducerInitEstablishesGeometryInvariant,
   ProducerInitEstablishesConsumedPrefixInvariant,
   ProducerInitEstablishesNoResurrectionInvariant,
   ProducerInitEstablishesRankProjectionTypeInvariant
   DEF ProducerSafetyInvariant

(***************************************************************************
Every kernel action extends both journals.  In particular, Transfer is an
exact journal stutter even when its opaque rank snapshot changes, and neither
Consume nor Complete can remove a previously consumed episode.
***************************************************************************)

THEOREM ObserveNewProducerSourcePreservesJournals ==
  \A request \in ProducerRequests, source \in ProducerSources,
     nextRankState \in ProducerRankStates:
    ObserveNewProducerSource(request, source, nextRankState)
      => ProducerJournalStepInvariant
BY Isa
   DEF ObserveNewProducerSource, ProducerJournalStepInvariant,
       ProducerKnownJournalMonotone, ProducerConsumedJournalMonotone,
       ProducerNoResurrectionStep

THEOREM TransferProducerObligationPreservesJournals ==
  \A obligation \in ProducerObligationSet,
     nextRankState \in ProducerRankStates:
    TransferProducerObligation(obligation, nextRankState)
      => ProducerJournalStepInvariant
BY Isa
   DEF TransferProducerObligation, ProducerJournalStepInvariant,
       ProducerKnownJournalMonotone, ProducerConsumedJournalMonotone,
       ProducerNoResurrectionStep

THEOREM ProducerProjectionBatchPreservesJournals ==
  \A freshObligations,
     transferredObligations,
     episodeChargeOrder,
     nextRankState:
    ProducerProjectionBatchAction(
      freshObligations,
      transferredObligations,
      episodeChargeOrder,
      nextRankState)
      => ProducerJournalStepInvariant
BY Isa
   DEF ProducerProjectionBatchAction, ProducerJournalStepInvariant,
       ProducerKnownJournalMonotone, ProducerConsumedJournalMonotone,
       ProducerNoResurrectionStep

THEOREM ProducerProjectionBatchChargesAreFreshAndUnique ==
  \A freshObligations,
     transferredObligations,
     episodeChargeOrder,
     nextRankState:
    ProducerProjectionBatchAction(
      freshObligations,
      transferredObligations,
      episodeChargeOrder,
      nextRankState)
      => /\ Len(episodeChargeOrder)
               = Cardinality(Range(episodeChargeOrder))
         /\ Range(episodeChargeOrder) \cap fpConsumedEpisodes = {}
         /\ \A obligation \in freshObligations:
              ProducerInitialEpisodes(obligation)
                \subseteq Range(episodeChargeOrder)
BY Isa DEF ProducerProjectionBatchAction

THEOREM ConsumeProducerEpisodesPreservesJournals ==
  \A obligation \in ProducerObligationSet:
    \A episodes \in SUBSET ProducerEpisodeUniverse(obligation),
       nextRankState \in ProducerRankStates:
      ConsumeProducerEpisodes(obligation, episodes, nextRankState)
        => ProducerJournalStepInvariant
BY Isa
   DEF ConsumeProducerEpisodes, ProducerJournalStepInvariant,
       ProducerKnownJournalMonotone, ProducerConsumedJournalMonotone,
       ProducerNoResurrectionStep

THEOREM CompleteProducerObligationPreservesJournals ==
  \A obligation \in ProducerObligationSet,
     nextRankState \in ProducerRankStates:
    CompleteProducerObligation(obligation, nextRankState)
      => ProducerJournalStepInvariant
BY Isa
   DEF CompleteProducerObligation, ProducerJournalStepInvariant,
       ProducerKnownJournalMonotone, ProducerConsumedJournalMonotone,
       ProducerNoResurrectionStep

THEOREM ProducerNextPreservesJournals ==
  ProducerNext => ProducerJournalStepInvariant
PROOF
  <1>1. ASSUME ProducerNext
         PROVE ProducerJournalStepInvariant
    <2>1. CASE \E request \in ProducerRequests,
                     source \in ProducerSources,
                     nextRankState \in ProducerRankStates:
                  ObserveNewProducerSource(
                    request, source, nextRankState)
      <3>1. PICK request \in ProducerRequests,
                    source \in ProducerSources,
                    nextRankState \in ProducerRankStates:
               ObserveNewProducerSource(
                 request, source, nextRankState)
        BY <2>1
      <3> QED BY <3>1,
           ObserveNewProducerSourcePreservesJournals
    <2>2. CASE \E obligation \in fpKnownObligations,
                     nextRankState \in ProducerRankStates:
                  TransferProducerObligation(
                    obligation, nextRankState)
      <3>1. PICK obligation \in fpKnownObligations,
                    nextRankState \in ProducerRankStates:
               TransferProducerObligation(
                 obligation, nextRankState)
        BY <2>2
      <3>2. obligation \in ProducerObligationSet
        BY <3>1 DEF TransferProducerObligation
      <3> QED BY <3>1, <3>2,
           TransferProducerObligationPreservesJournals
    <2>3. CASE \E obligation \in fpKnownObligations:
                  \E episodes
                       \in SUBSET ProducerEpisodeUniverse(obligation),
                     nextRankState \in ProducerRankStates:
                    ConsumeProducerEpisodes(
                      obligation, episodes, nextRankState)
      <3>1. PICK obligation \in fpKnownObligations:
               \E episodes
                    \in SUBSET ProducerEpisodeUniverse(obligation),
                  nextRankState \in ProducerRankStates:
                 ConsumeProducerEpisodes(
                   obligation, episodes, nextRankState)
        BY <2>3
      <3>2. PICK episodes
                      \in SUBSET ProducerEpisodeUniverse(obligation),
                    nextRankState \in ProducerRankStates:
               ConsumeProducerEpisodes(
                 obligation, episodes, nextRankState)
        BY <3>1
      <3> QED BY <3>2, Isa
           DEF ConsumeProducerEpisodes,
               ProducerJournalStepInvariant,
               ProducerKnownJournalMonotone,
               ProducerConsumedJournalMonotone,
               ProducerNoResurrectionStep
    <2>4. CASE \E obligation \in fpKnownObligations,
                     nextRankState \in ProducerRankStates:
                  CompleteProducerObligation(
                    obligation, nextRankState)
      <3>1. PICK obligation \in fpKnownObligations,
                    nextRankState \in ProducerRankStates:
               CompleteProducerObligation(
                 obligation, nextRankState)
        BY <2>4
      <3> QED BY <3>1, Isa
           DEF CompleteProducerObligation,
               ProducerJournalStepInvariant,
               ProducerKnownJournalMonotone,
               ProducerConsumedJournalMonotone,
               ProducerNoResurrectionStep
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ProducerNext
  <1> QED BY <1>1

THEOREM ProducerConsumedEpisodeCannotReturnToRemaining ==
  \A obligation \in fpKnownObligations:
    \A episode \in ProducerConsumedEpisodesFor(obligation):
      ProducerJournalStepInvariant
        => episode \notin ProducerRemainingEpisodes(obligation)'
BY Isa
   DEF ProducerJournalStepInvariant, ProducerConsumedJournalMonotone,
       ProducerNoResurrectionStep, ProducerConsumedEpisodesFor,
       ProducerRemainingEpisodes

(***************************************************************************
Action-specific identity and completion facts.
***************************************************************************)

THEOREM ObserveNewProducerSourceAddsExactStableObligation ==
  \A request \in ProducerRequests, source \in ProducerSources,
     nextRankState \in ProducerRankStates:
    ObserveNewProducerSource(request, source, nextRankState)
      => fpKnownObligations'
           = fpKnownObligations
               \cup {ProducerObligation(request, source)}
BY Isa DEF ObserveNewProducerSource

THEOREM TransferProducerObligationDoesNotRecreateIdentity ==
  \A obligation \in ProducerObligationSet,
     nextRankState \in ProducerRankStates:
    TransferProducerObligation(obligation, nextRankState)
      => /\ fpKnownObligations' = fpKnownObligations
         /\ fpConsumedEpisodes' = fpConsumedEpisodes
BY Isa
   DEF TransferProducerObligation

THEOREM CompleteProducerObligationConsumesItsUniverse ==
  \A obligation \in fpKnownObligations,
     nextRankState \in ProducerRankStates:
      /\ ProducerTypeInvariant
      /\ CompleteProducerObligation(obligation, nextRankState)
      => /\ ProducerConsumedEpisodesFor(obligation)'
             = ProducerEpisodeUniverse(obligation)
         /\ ProducerObligationComplete(obligation)'
BY FS_Subset, Isa
   DEF ProducerTypeInvariant, CompleteProducerObligation,
       ProducerConsumedEpisodesFor, ProducerRemainingEpisodes,
       ProducerObligationComplete

(***************************************************************************
The consumed/remaining partition is definitional once an obligation is
known.  Keeping it as a proved lemma makes the exact non-resurrection
interpretation available to production refinements without a hidden
temporal assumption.
***************************************************************************)

THEOREM ProducerKnownObligationHasExactEpisodePartition ==
  \A obligation \in fpKnownObligations:
    /\ ProducerConsumedEpisodesFor(obligation)
         \cap ProducerRemainingEpisodes(obligation) = {}
    /\ ProducerConsumedEpisodesFor(obligation)
         \cup ProducerRemainingEpisodes(obligation)
           = ProducerEpisodeUniverse(obligation)
BY Isa
   DEF ProducerConsumedEpisodesFor, ProducerRemainingEpisodes

THEOREM ProducerTypeImpliesNoResurrectionInvariant ==
  ProducerTypeInvariant => ProducerNoResurrectionInvariant
BY ProducerKnownObligationHasExactEpisodePartition
   DEF ProducerNoResurrectionInvariant

(***************************************************************************
Safety preservation.  Typed obligation records make distinct per-source
universes disjoint, so adding or consuming one obligation cannot disturb the
prefix of another.  The New guard plus the exact source enumeration preserves
the configured geometry bound.
***************************************************************************)

THEOREM ObserveNewProducerSourcePreservesSafetyInvariant ==
  \A request \in ProducerRequests, source \in ProducerSources,
     nextRankState \in ProducerRankStates:
    /\ ProducerSafetyInvariant
    /\ ObserveNewProducerSource(request, source, nextRankState)
    => ProducerSafetyInvariant'
PROOF
  <1>1. ASSUME NEW request \in ProducerRequests,
                NEW source \in ProducerSources,
                NEW nextRankState \in ProducerRankStates,
                ProducerSafetyInvariant,
                ObserveNewProducerSource(
                  request, source, nextRankState)
         PROVE ProducerSafetyInvariant'
    <2> DEFINE NewObligation ==
           ProducerObligation(request, source)
    <2>1. ProducerTypeInvariant'
      <3>1. ProducerConfiguration
        BY <1>1
           DEF ProducerSafetyInvariant, ProducerTypeInvariant
      <3>2. NewObligation \in ProducerObligationSet
        BY <1>1, Isa
           DEF NewObligation, ProducerObligation,
               ProducerObligationSet
      <3>3. fpKnownObligations'
               \subseteq ProducerObligationSet
        BY <1>1, <3>2, SMT
           DEF NewObligation, ObserveNewProducerSource,
               ProducerSafetyInvariant, ProducerTypeInvariant
      <3>4. ProducerInitialEpisodes(NewObligation)
               \subseteq ProducerEpisodeUniverse(NewObligation)
        BY <1>1, <3>1, <3>2
           DEF ProducerConfiguration,
               ProducerEpisodeStructureConfiguration
      <3>5. fpConsumedEpisodes'
               \subseteq
                 UNION {ProducerEpisodeUniverse(obligation):
                          obligation \in fpKnownObligations'}
        BY <1>1, <3>3, <3>4, Isa
           DEF NewObligation, ObserveNewProducerSource,
               ProducerSafetyInvariant, ProducerTypeInvariant
      <3>6. fpRankState' \in ProducerRankStates
        BY <1>1 DEF ObserveNewProducerSource
      <3> QED BY <3>1, <3>3, <3>5, <3>6
           DEF ProducerTypeInvariant
    <2>2. ProducerGeometryInvariant'
      <3>1. ASSUME NEW checkedRequest \in ProducerRequests
             PROVE Cardinality(
                     ProducerKnownSourcesFor(checkedRequest)')
                     <= ProducerSourceCapacity
        <4>1. IsFiniteSet(
                 ProducerKnownSourcesFor(checkedRequest))
          BY <1>1, <3>1, FS_Subset
             DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                 ProducerConfiguration,
                 ProducerGeometryConfiguration,
                 ProducerKnownSourcesFor
        <4>2. CASE checkedRequest = request
          <5>1. source
                   \notin ProducerKnownSourcesFor(checkedRequest)
            BY <1>1, <4>2, SMT
               DEF NewObligation, ProducerKnownSourcesFor,
                   ObserveNewProducerSource
          <5>2. ProducerKnownSourcesFor(checkedRequest)'
                   = ProducerKnownSourcesFor(checkedRequest)
                       \cup {source}
            BY <1>1, <3>1, <4>2, Isa
               DEF NewObligation, ProducerKnownSourcesFor,
                   ProducerObligation,
                   ObserveNewProducerSource
          <5>3. Cardinality(
                   ProducerKnownSourcesFor(checkedRequest)')
                   = Cardinality(
                       ProducerKnownSourcesFor(checkedRequest)) + 1
            BY <4>1, <5>1, <5>2, FS_AddElement, SMT
          <5>4. Cardinality(
                   ProducerKnownSourcesFor(checkedRequest))
                   < ProducerSourceCapacity
            BY <1>1, <4>2
               DEF ProducerKnownSourcesFor,
                   ObserveNewProducerSource
          <5>5. /\ Cardinality(
                       ProducerKnownSourcesFor(checkedRequest))
                       \in Nat
                 /\ ProducerSourceCapacity \in Nat
            BY <1>1, <4>1, FS_CardinalityType
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerGeometryConfiguration
          <5> QED BY <5>3, <5>4, <5>5, SMT
        <4>3. CASE checkedRequest # request
          <5>1. ProducerKnownSourcesFor(checkedRequest)'
                   = ProducerKnownSourcesFor(checkedRequest)
            BY <1>1, <3>1, <4>3, Isa
               DEF NewObligation, ProducerKnownSourcesFor,
                   ProducerObligation,
                   ObserveNewProducerSource
          <5>2. Cardinality(
                   ProducerKnownSourcesFor(checkedRequest))
                   <= ProducerSourceCapacity
            BY <1>1, <3>1
               DEF ProducerSafetyInvariant,
                   ProducerGeometryInvariant
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1 DEF ProducerGeometryInvariant
    <2>3. ProducerConsumedPrefixInvariant'
      <3>1. NewObligation \in ProducerObligationSet
        BY <1>1, Isa
           DEF NewObligation, ProducerObligation,
               ProducerObligationSet
      <3>2. \A oldObligation \in fpKnownObligations:
               ProducerEpisodeUniverse(oldObligation)
                 \cap ProducerEpisodeUniverse(NewObligation)
                   = {}
        <4>1. ASSUME NEW oldObligation \in fpKnownObligations
               PROVE ProducerEpisodeUniverse(oldObligation)
                       \cap ProducerEpisodeUniverse(NewObligation)
                         = {}
          <5>1. /\ oldObligation \in ProducerObligationSet
                 /\ oldObligation # NewObligation
            BY <1>1, <3>1, <4>1, SMT
               DEF NewObligation, ProducerSafetyInvariant,
                   ProducerTypeInvariant,
                   ObserveNewProducerSource
          <5>2. /\ ProducerEpisodeUniverse(oldObligation)
                       \subseteq
                         ProducerEpisodesOfType(oldObligation)
                 /\ ProducerEpisodeUniverse(NewObligation)
                       \subseteq
                         ProducerEpisodesOfType(NewObligation)
            BY <1>1, <3>1, <5>1
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerEpisodeStructureConfiguration
          <5>3. ASSUME NEW episode
                          \in ProducerEpisodeUniverse(oldObligation)
                              \cap
                                ProducerEpisodeUniverse(NewObligation)
                 PROVE FALSE
            <6>1. /\ episode \in
                         ProducerEpisodeUniverse(oldObligation)
                   /\ episode \in
                         ProducerEpisodeUniverse(NewObligation)
              BY <5>3
            <6>2. /\ episode.obligation = oldObligation
                   /\ episode.obligation = NewObligation
              BY <5>2, <6>1 DEF ProducerEpisodesOfType
            <6> QED BY <5>1, <6>2
          <5> QED BY <5>3
        <4> QED BY <4>1
      <3>3. fpConsumedEpisodes
               \cap ProducerEpisodeUniverse(NewObligation)
                 = {}
        <4>1. ASSUME NEW episode
                        \in fpConsumedEpisodes
                            \cap
                              ProducerEpisodeUniverse(NewObligation)
               PROVE FALSE
          <5>1. /\ episode \in fpConsumedEpisodes
                 /\ episode \in
                       ProducerEpisodeUniverse(NewObligation)
            BY <4>1
          <5>2. PICK oldObligation \in fpKnownObligations:
                   episode
                     \in ProducerEpisodeUniverse(oldObligation)
            BY <1>1, <5>1, Isa
               DEF ProducerSafetyInvariant, ProducerTypeInvariant
          <5>3. ProducerEpisodeUniverse(oldObligation)
                   \cap ProducerEpisodeUniverse(NewObligation)
                     = {}
            BY <3>2, <5>2
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3>4. ProducerEpisodeSetPrefixClosed(
               NewObligation,
               ProducerInitialEpisodes(NewObligation))
        BY <1>1, <3>1
           DEF ProducerSafetyInvariant, ProducerTypeInvariant,
               ProducerConfiguration,
               ProducerEpisodeStructureConfiguration
      <3>5. ASSUME NEW knownObligation
                       \in fpKnownObligations'
             PROVE ProducerEpisodeSetPrefixClosed(
                     knownObligation,
                     ProducerConsumedEpisodesFor(knownObligation)')
        <4>1. CASE knownObligation = NewObligation
          <5>1. ProducerInitialEpisodes(NewObligation)
                   \subseteq ProducerEpisodeUniverse(NewObligation)
            BY <1>1, <3>1
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerEpisodeStructureConfiguration
          <5>2. ProducerConsumedEpisodesFor(knownObligation)'
                   = ProducerInitialEpisodes(NewObligation)
            BY <1>1, <3>3, <4>1, <5>1, Isa
               DEF NewObligation, ProducerConsumedEpisodesFor,
                   ObserveNewProducerSource
          <5> QED BY <3>4, <4>1, <5>2
        <4>2. CASE knownObligation # NewObligation
          <5>1. knownObligation \in fpKnownObligations
            BY <1>1, <3>5, <4>2, SMT
               DEF NewObligation, ObserveNewProducerSource
          <5>2. ProducerEpisodeUniverse(knownObligation)
                   \cap ProducerEpisodeUniverse(NewObligation)
                     = {}
            BY <3>2, <5>1
          <5>3. ProducerInitialEpisodes(NewObligation)
                   \subseteq ProducerEpisodeUniverse(NewObligation)
            BY <1>1, <3>1
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerEpisodeStructureConfiguration
          <5>4. ProducerInitialEpisodes(NewObligation)
                   \cap ProducerEpisodeUniverse(knownObligation)
                     = {}
            <6>1. ASSUME NEW episode
                            \in ProducerInitialEpisodes(NewObligation)
                                \cap
                                  ProducerEpisodeUniverse(
                                    knownObligation)
                   PROVE FALSE
              <7>1. /\ episode
                           \in ProducerInitialEpisodes(NewObligation)
                     /\ episode
                           \in ProducerEpisodeUniverse(
                                knownObligation)
                BY <6>1
              <7>2. episode
                       \in ProducerEpisodeUniverse(NewObligation)
                BY <5>3, <7>1
              <7>3. episode
                       \in ProducerEpisodeUniverse(knownObligation)
                           \cap
                             ProducerEpisodeUniverse(NewObligation)
                BY <7>1, <7>2
              <7> QED BY <5>2, <7>3
            <6> QED BY <6>1
          <5>5. ProducerConsumedEpisodesFor(knownObligation)'
                   = ProducerConsumedEpisodesFor(knownObligation)
            BY <1>1, <5>4, Isa
               DEF NewObligation, ProducerConsumedEpisodesFor,
                   ObserveNewProducerSource
          <5>6. ProducerEpisodeSetPrefixClosed(
                   knownObligation,
                   ProducerConsumedEpisodesFor(knownObligation))
            BY <1>1, <5>1
               DEF ProducerSafetyInvariant,
                   ProducerConsumedPrefixInvariant
          <5> QED BY <5>5, <5>6
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>5
           DEF ProducerConsumedPrefixInvariant
    <2>4. ProducerNoResurrectionInvariant'
      BY <1>1, Isa
         DEF ProducerNoResurrectionInvariant,
             ProducerConsumedEpisodesFor,
             ProducerRemainingEpisodes,
             ObserveNewProducerSource
    <2>5. ProducerRankProjectionTypeInvariant'
      <3>1. ASSUME NEW knownObligation \in fpKnownObligations'
             PROVE /\ ProducerSchedulerRank(
                          fpRankState', knownObligation)
                        \in ProducerSchedulerRankCarrier
                   /\ ProducerStageRank(fpRankState', knownObligation)
                        \in ProducerStageRankCarrier
                   /\ ProducerCursorRank(fpRankState', knownObligation)
                        \in ProducerCursorRankCarrier
        <4>1. /\ knownObligation \in ProducerObligationSet
               /\ fpRankState' = nextRankState
          BY <1>1, <2>1, <3>1
             DEF ProducerTypeInvariant, ObserveNewProducerSource
        <4>2. /\ ProducerSchedulerRank(
                       nextRankState, knownObligation)
                     \in ProducerSchedulerRankCarrier
               /\ ProducerStageRank(nextRankState, knownObligation)
                    \in ProducerStageRankCarrier
               /\ ProducerCursorRank(nextRankState, knownObligation)
                    \in ProducerCursorRankCarrier
          BY <1>1, <4>1
             DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                 ProducerConfiguration,
                 ProducerRankOrderingConfiguration
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
           DEF ProducerRankProjectionTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
         DEF ProducerSafetyInvariant
  <1> QED BY <1>1

THEOREM TransferProducerObligationPreservesSafetyInvariant ==
  \A obligation \in ProducerObligationSet,
     nextRankState \in ProducerRankStates:
    /\ ProducerSafetyInvariant
    /\ TransferProducerObligation(obligation, nextRankState)
    => ProducerSafetyInvariant'
PROOF
  <1>1. ASSUME NEW obligation \in ProducerObligationSet,
                NEW nextRankState \in ProducerRankStates,
                ProducerSafetyInvariant,
                TransferProducerObligation(
                  obligation, nextRankState)
         PROVE ProducerSafetyInvariant'
    <2>1. ProducerTypeInvariant'
      BY <1>1, Isa
         DEF ProducerSafetyInvariant, ProducerTypeInvariant,
             ProducerConfiguration,
             ProducerRankOrderingConfiguration,
             TransferProducerObligation
    <2>2. ProducerGeometryInvariant'
      BY <1>1, Isa
         DEF ProducerSafetyInvariant, ProducerGeometryInvariant,
             ProducerKnownSourcesFor, TransferProducerObligation
    <2>3. ProducerConsumedPrefixInvariant'
      BY <1>1, Isa
         DEF ProducerSafetyInvariant,
             ProducerConsumedPrefixInvariant,
             ProducerConsumedEpisodesFor,
             ProducerEpisodeSetPrefixClosed,
             TransferProducerObligation
    <2>4. ProducerNoResurrectionInvariant'
      BY <1>1, Isa
         DEF ProducerSafetyInvariant,
             ProducerNoResurrectionInvariant,
             ProducerConsumedEpisodesFor,
             ProducerRemainingEpisodes,
             TransferProducerObligation
    <2>5. ProducerRankProjectionTypeInvariant'
      <3>1. ASSUME NEW knownObligation \in fpKnownObligations'
             PROVE /\ ProducerSchedulerRank(
                          fpRankState', knownObligation)
                        \in ProducerSchedulerRankCarrier
                   /\ ProducerStageRank(fpRankState', knownObligation)
                        \in ProducerStageRankCarrier
                   /\ ProducerCursorRank(fpRankState', knownObligation)
                        \in ProducerCursorRankCarrier
        <4>1. /\ knownObligation \in fpKnownObligations
               /\ fpRankState' = nextRankState
          BY <1>1, <3>1 DEF TransferProducerObligation
        <4>2. knownObligation \in ProducerObligationSet
          BY <1>1, <4>1, SMT
             DEF ProducerSafetyInvariant, ProducerTypeInvariant
        <4>3. /\ ProducerSchedulerRank(
                       nextRankState, knownObligation)
                     \in ProducerSchedulerRankCarrier
               /\ ProducerStageRank(nextRankState, knownObligation)
                    \in ProducerStageRankCarrier
               /\ ProducerCursorRank(nextRankState, knownObligation)
                    \in ProducerCursorRankCarrier
          BY <1>1, <4>2
             DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                 ProducerConfiguration,
                 ProducerRankOrderingConfiguration
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1
           DEF ProducerRankProjectionTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
         DEF ProducerSafetyInvariant
  <1> QED BY <1>1

THEOREM ConsumeProducerEpisodesPreservesSafetyInvariant ==
  \A obligation \in ProducerObligationSet:
    \A episodes \in SUBSET ProducerEpisodeUniverse(obligation),
       nextRankState \in ProducerRankStates:
      /\ ProducerSafetyInvariant
      /\ ConsumeProducerEpisodes(obligation, episodes, nextRankState)
      => ProducerSafetyInvariant'
PROOF
  <1>1. ASSUME NEW obligation \in ProducerObligationSet,
                NEW episodes
                      \in SUBSET ProducerEpisodeUniverse(obligation),
                NEW nextRankState \in ProducerRankStates,
                ProducerSafetyInvariant,
                ConsumeProducerEpisodes(
                  obligation, episodes, nextRankState)
         PROVE ProducerSafetyInvariant'
    <2>1. ProducerTypeInvariant'
      <3>1. ProducerConfiguration
        BY <1>1
           DEF ProducerSafetyInvariant, ProducerTypeInvariant
      <3>2. /\ fpKnownObligations' = fpKnownObligations
             /\ obligation \in fpKnownObligations
             /\ episodes
                  \subseteq ProducerEpisodeUniverse(obligation)
        BY <1>1
           DEF ConsumeProducerEpisodes, ProducerRemainingEpisodes
      <3>3. fpKnownObligations'
               \subseteq ProducerObligationSet
        BY <1>1, <3>2
           DEF ProducerSafetyInvariant, ProducerTypeInvariant
      <3>4. ProducerEpisodeUniverse(obligation)
               \subseteq
                 UNION {ProducerEpisodeUniverse(knownObligation):
                          knownObligation \in fpKnownObligations'}
        BY <3>2, Isa
      <3>5. fpConsumedEpisodes'
               \subseteq
                 UNION {ProducerEpisodeUniverse(knownObligation):
                          knownObligation \in fpKnownObligations'}
        BY <1>1, <3>2, <3>4, Isa
           DEF ConsumeProducerEpisodes, ProducerSafetyInvariant,
               ProducerTypeInvariant
      <3>6. fpRankState' \in ProducerRankStates
        BY <1>1 DEF ConsumeProducerEpisodes
      <3> QED BY <3>1, <3>3, <3>5, <3>6
           DEF ProducerTypeInvariant
    <2>2. ProducerGeometryInvariant'
      BY <1>1, Isa
         DEF ProducerSafetyInvariant, ProducerGeometryInvariant,
             ProducerKnownSourcesFor, ConsumeProducerEpisodes
    <2>3. ProducerConsumedPrefixInvariant'
      <3>1. \A knownObligation \in fpKnownObligations:
               knownObligation # obligation
                 => ProducerEpisodeUniverse(knownObligation)
                      \cap ProducerEpisodeUniverse(obligation)
                        = {}
        <4>1. ASSUME NEW knownObligation
                           \in fpKnownObligations,
                        knownObligation # obligation
               PROVE ProducerEpisodeUniverse(knownObligation)
                       \cap ProducerEpisodeUniverse(obligation)
                         = {}
          <5>1. knownObligation \in ProducerObligationSet
            BY <1>1, <4>1, SMT
               DEF ProducerSafetyInvariant, ProducerTypeInvariant
          <5>2. /\ ProducerEpisodeUniverse(knownObligation)
                       \subseteq
                         ProducerEpisodesOfType(knownObligation)
                 /\ ProducerEpisodeUniverse(obligation)
                       \subseteq ProducerEpisodesOfType(obligation)
            BY <1>1, <5>1
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerEpisodeStructureConfiguration
          <5>3. ASSUME NEW episode
                          \in ProducerEpisodeUniverse(knownObligation)
                              \cap
                                ProducerEpisodeUniverse(obligation)
                 PROVE FALSE
            <6>1. /\ episode
                         \in ProducerEpisodeUniverse(knownObligation)
                   /\ episode \in ProducerEpisodeUniverse(obligation)
              BY <5>3
            <6>2. /\ episode.obligation = knownObligation
                   /\ episode.obligation = obligation
              BY <5>2, <6>1 DEF ProducerEpisodesOfType
            <6> QED BY <4>1, <6>2
          <5> QED BY <5>3
        <4> QED BY <4>1
      <3>2. ASSUME NEW knownObligation
                       \in fpKnownObligations'
             PROVE ProducerEpisodeSetPrefixClosed(
                     knownObligation,
                     ProducerConsumedEpisodesFor(knownObligation)')
        <4>1. knownObligation \in fpKnownObligations
          BY <1>1, <3>2 DEF ConsumeProducerEpisodes
        <4>2. CASE knownObligation = obligation
          <5>1. ProducerConsumedEpisodesFor(knownObligation)'
                   = ProducerConsumedEpisodesFor(obligation)
                       \cup episodes
            BY <1>1, <4>2, Isa
               DEF ProducerConsumedEpisodesFor,
                   ConsumeProducerEpisodes,
                   ProducerRemainingEpisodes
          <5>2. ProducerEpisodeSetPrefixClosed(
                   obligation,
                   ProducerConsumedEpisodesFor(obligation)
                     \cup episodes)
            BY <1>1 DEF ConsumeProducerEpisodes
          <5> QED BY <4>2, <5>1, <5>2
        <4>3. CASE knownObligation # obligation
          <5>1. ProducerEpisodeUniverse(knownObligation)
                   \cap ProducerEpisodeUniverse(obligation)
                     = {}
            BY <3>1, <4>1, <4>3
          <5>2. episodes
                   \cap ProducerEpisodeUniverse(knownObligation)
                     = {}
            <6>1. ASSUME NEW episode
                            \in episodes
                                \cap
                                  ProducerEpisodeUniverse(
                                    knownObligation)
                   PROVE FALSE
              <7>1. /\ episode \in episodes
                     /\ episode
                           \in ProducerEpisodeUniverse(
                                knownObligation)
                BY <6>1
              <7>2. episode \in ProducerEpisodeUniverse(obligation)
                BY <1>1, <7>1
              <7>3. episode
                       \in ProducerEpisodeUniverse(knownObligation)
                           \cap ProducerEpisodeUniverse(obligation)
                BY <7>1, <7>2
              <7> QED BY <5>1, <7>3
            <6> QED BY <6>1
          <5>3. ProducerConsumedEpisodesFor(knownObligation)'
                   = ProducerConsumedEpisodesFor(knownObligation)
            BY <1>1, <5>2, Isa
               DEF ProducerConsumedEpisodesFor,
                   ConsumeProducerEpisodes
          <5>4. ProducerEpisodeSetPrefixClosed(
                   knownObligation,
                   ProducerConsumedEpisodesFor(knownObligation))
            BY <1>1, <4>1
               DEF ProducerSafetyInvariant,
                   ProducerConsumedPrefixInvariant
          <5> QED BY <5>3, <5>4
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>2
           DEF ProducerConsumedPrefixInvariant
    <2>4. ProducerNoResurrectionInvariant'
      BY <1>1, Isa
         DEF ProducerNoResurrectionInvariant,
             ProducerConsumedEpisodesFor,
             ProducerRemainingEpisodes,
             ConsumeProducerEpisodes
    <2>5. ProducerRankProjectionTypeInvariant'
      <3>1. ASSUME NEW knownObligation
                       \in fpKnownObligations'
             PROVE /\ ProducerSchedulerRank(
                          fpRankState', knownObligation)
                        \in ProducerSchedulerRankCarrier
                   /\ ProducerStageRank(fpRankState', knownObligation)
                        \in ProducerStageRankCarrier
                   /\ ProducerCursorRank(fpRankState', knownObligation)
                        \in ProducerCursorRankCarrier
        <4>1. /\ knownObligation \in fpKnownObligations
               /\ fpRankState' = nextRankState
          BY <1>1, <3>1 DEF ConsumeProducerEpisodes
        <4>2. knownObligation \in ProducerObligationSet
          BY <1>1, <4>1, SMT
             DEF ProducerSafetyInvariant, ProducerTypeInvariant
        <4>3. /\ ProducerSchedulerRank(
                       nextRankState, knownObligation)
                     \in ProducerSchedulerRankCarrier
               /\ ProducerStageRank(nextRankState, knownObligation)
                    \in ProducerStageRankCarrier
               /\ ProducerCursorRank(nextRankState, knownObligation)
                    \in ProducerCursorRankCarrier
          BY <1>1, <4>2
             DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                 ProducerConfiguration,
                 ProducerRankOrderingConfiguration
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1
           DEF ProducerRankProjectionTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
         DEF ProducerSafetyInvariant
  <1> QED BY <1>1

THEOREM CompleteProducerObligationPreservesSafetyInvariant ==
  \A obligation \in ProducerObligationSet,
     nextRankState \in ProducerRankStates:
    /\ ProducerSafetyInvariant
    /\ CompleteProducerObligation(obligation, nextRankState)
    => ProducerSafetyInvariant'
PROOF
  <1>1. ASSUME NEW obligation \in ProducerObligationSet,
                NEW nextRankState \in ProducerRankStates,
                ProducerSafetyInvariant,
                CompleteProducerObligation(
                  obligation, nextRankState)
         PROVE ProducerSafetyInvariant'
    <2>1. ProducerTypeInvariant'
      <3>1. /\ ProducerConfiguration
             /\ fpKnownObligations' = fpKnownObligations
             /\ obligation \in fpKnownObligations
        BY <1>1
           DEF ProducerSafetyInvariant, ProducerTypeInvariant,
               CompleteProducerObligation
      <3>2. fpKnownObligations'
               \subseteq ProducerObligationSet
        BY <1>1, <3>1
           DEF ProducerSafetyInvariant, ProducerTypeInvariant
      <3>3. ProducerEpisodeUniverse(obligation)
               \subseteq
                 UNION {ProducerEpisodeUniverse(knownObligation):
                          knownObligation \in fpKnownObligations'}
        BY <3>1, Isa
      <3>4. fpConsumedEpisodes'
               \subseteq
                 UNION {ProducerEpisodeUniverse(knownObligation):
                          knownObligation \in fpKnownObligations'}
        BY <1>1, <3>1, <3>3, Isa
           DEF CompleteProducerObligation, ProducerSafetyInvariant,
               ProducerTypeInvariant
      <3>5. fpRankState' \in ProducerRankStates
        BY <1>1 DEF CompleteProducerObligation
      <3> QED BY <3>1, <3>2, <3>4, <3>5
           DEF ProducerTypeInvariant
    <2>2. ProducerGeometryInvariant'
      BY <1>1, Isa
         DEF ProducerSafetyInvariant, ProducerGeometryInvariant,
             ProducerKnownSourcesFor, CompleteProducerObligation
    <2>3. ProducerConsumedPrefixInvariant'
      <3>1. \A knownObligation \in fpKnownObligations:
               knownObligation # obligation
                 => ProducerEpisodeUniverse(knownObligation)
                      \cap ProducerEpisodeUniverse(obligation)
                        = {}
        <4>1. ASSUME NEW knownObligation
                           \in fpKnownObligations,
                        knownObligation # obligation
               PROVE ProducerEpisodeUniverse(knownObligation)
                       \cap ProducerEpisodeUniverse(obligation)
                         = {}
          <5>1. knownObligation \in ProducerObligationSet
            BY <1>1, <4>1, SMT
               DEF ProducerSafetyInvariant, ProducerTypeInvariant
          <5>2. /\ ProducerEpisodeUniverse(knownObligation)
                       \subseteq
                         ProducerEpisodesOfType(knownObligation)
                 /\ ProducerEpisodeUniverse(obligation)
                       \subseteq ProducerEpisodesOfType(obligation)
            BY <1>1, <5>1
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerEpisodeStructureConfiguration
          <5>3. ASSUME NEW episode
                          \in ProducerEpisodeUniverse(knownObligation)
                              \cap
                                ProducerEpisodeUniverse(obligation)
                 PROVE FALSE
            <6>1. /\ episode
                         \in ProducerEpisodeUniverse(knownObligation)
                   /\ episode \in ProducerEpisodeUniverse(obligation)
              BY <5>3
            <6>2. /\ episode.obligation = knownObligation
                   /\ episode.obligation = obligation
              BY <5>2, <6>1 DEF ProducerEpisodesOfType
            <6> QED BY <4>1, <6>2
          <5> QED BY <5>3
        <4> QED BY <4>1
      <3>2. ASSUME NEW knownObligation
                       \in fpKnownObligations'
             PROVE ProducerEpisodeSetPrefixClosed(
                     knownObligation,
                     ProducerConsumedEpisodesFor(knownObligation)')
        <4>1. knownObligation \in fpKnownObligations
          BY <1>1, <3>2 DEF CompleteProducerObligation
        <4>2. CASE knownObligation = obligation
          <5>1. ProducerConsumedEpisodesFor(knownObligation)'
                   = ProducerEpisodeUniverse(obligation)
            BY <1>1, <4>2, Isa
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConsumedEpisodesFor,
                   CompleteProducerObligation
          <5>2. ProducerEpisodeSetPrefixClosed(
                   obligation, ProducerEpisodeUniverse(obligation))
            BY <1>1
               DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                   ProducerConfiguration,
                   ProducerEpisodeStructureConfiguration,
                   ProducerEpisodeSetPrefixClosed
          <5> QED BY <4>2, <5>1, <5>2
        <4>3. CASE knownObligation # obligation
          <5>1. ProducerEpisodeUniverse(knownObligation)
                   \cap ProducerEpisodeUniverse(obligation)
                     = {}
            BY <3>1, <4>1, <4>3
          <5>2. ProducerConsumedEpisodesFor(knownObligation)'
                   = ProducerConsumedEpisodesFor(knownObligation)
            BY <1>1, <5>1, Isa
               DEF ProducerConsumedEpisodesFor,
                   CompleteProducerObligation
          <5>3. ProducerEpisodeSetPrefixClosed(
                   knownObligation,
                   ProducerConsumedEpisodesFor(knownObligation))
            BY <1>1, <4>1
               DEF ProducerSafetyInvariant,
                   ProducerConsumedPrefixInvariant
          <5> QED BY <5>2, <5>3
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>2
           DEF ProducerConsumedPrefixInvariant
    <2>4. ProducerNoResurrectionInvariant'
      BY <1>1, Isa
         DEF ProducerNoResurrectionInvariant,
             ProducerConsumedEpisodesFor,
             ProducerRemainingEpisodes,
             CompleteProducerObligation
    <2>5. ProducerRankProjectionTypeInvariant'
      <3>1. ASSUME NEW knownObligation
                       \in fpKnownObligations'
             PROVE /\ ProducerSchedulerRank(
                          fpRankState', knownObligation)
                        \in ProducerSchedulerRankCarrier
                   /\ ProducerStageRank(fpRankState', knownObligation)
                        \in ProducerStageRankCarrier
                   /\ ProducerCursorRank(fpRankState', knownObligation)
                        \in ProducerCursorRankCarrier
        <4>1. /\ knownObligation \in fpKnownObligations
               /\ fpRankState' = nextRankState
          BY <1>1, <3>1 DEF CompleteProducerObligation
        <4>2. knownObligation \in ProducerObligationSet
          BY <1>1, <4>1, SMT
             DEF ProducerSafetyInvariant, ProducerTypeInvariant
        <4>3. /\ ProducerSchedulerRank(
                       nextRankState, knownObligation)
                     \in ProducerSchedulerRankCarrier
               /\ ProducerStageRank(nextRankState, knownObligation)
                    \in ProducerStageRankCarrier
               /\ ProducerCursorRank(nextRankState, knownObligation)
                    \in ProducerCursorRankCarrier
          BY <1>1, <4>2
             DEF ProducerSafetyInvariant, ProducerTypeInvariant,
                 ProducerConfiguration,
                 ProducerRankOrderingConfiguration
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1
           DEF ProducerRankProjectionTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
         DEF ProducerSafetyInvariant
  <1> QED BY <1>1

THEOREM ProducerNextPreservesSafetyInvariant ==
  /\ ProducerSafetyInvariant
  /\ ProducerNext
  => ProducerSafetyInvariant'
PROOF
  <1>1. ASSUME ProducerSafetyInvariant,
                ProducerNext
         PROVE ProducerSafetyInvariant'
    <2>1. CASE \E request \in ProducerRequests,
                     source \in ProducerSources,
                     nextRankState \in ProducerRankStates:
                  ObserveNewProducerSource(
                    request, source, nextRankState)
      <3>1. PICK request \in ProducerRequests,
                    source \in ProducerSources,
                    nextRankState \in ProducerRankStates:
               ObserveNewProducerSource(
                 request, source, nextRankState)
        BY <2>1
      <3> QED BY <1>1, <3>1,
           ObserveNewProducerSourcePreservesSafetyInvariant
    <2>2. CASE \E obligation \in fpKnownObligations,
                     nextRankState \in ProducerRankStates:
                  TransferProducerObligation(
                    obligation, nextRankState)
      <3>1. PICK obligation \in fpKnownObligations,
                    nextRankState \in ProducerRankStates:
               TransferProducerObligation(
                 obligation, nextRankState)
        BY <2>2
      <3>2. obligation \in ProducerObligationSet
        BY <3>1 DEF TransferProducerObligation
      <3> QED BY <1>1, <3>1, <3>2,
           TransferProducerObligationPreservesSafetyInvariant
    <2>3. CASE \E obligation \in fpKnownObligations:
                  \E episodes
                       \in SUBSET ProducerEpisodeUniverse(obligation),
                     nextRankState \in ProducerRankStates:
                    ConsumeProducerEpisodes(
                      obligation, episodes, nextRankState)
      <3>1. PICK obligation \in fpKnownObligations:
               \E episodes
                    \in SUBSET ProducerEpisodeUniverse(obligation),
                  nextRankState \in ProducerRankStates:
                 ConsumeProducerEpisodes(
                   obligation, episodes, nextRankState)
        BY <2>3
      <3>2. PICK episodes
                      \in SUBSET ProducerEpisodeUniverse(obligation),
                    nextRankState \in ProducerRankStates:
               ConsumeProducerEpisodes(
                 obligation, episodes, nextRankState)
        BY <3>1
      <3>3. obligation \in ProducerObligationSet
        BY <1>1, <3>1, SMT
           DEF ProducerSafetyInvariant, ProducerTypeInvariant
      <3> QED BY <1>1, <3>2, <3>3,
           ConsumeProducerEpisodesPreservesSafetyInvariant
    <2>4. CASE \E obligation \in fpKnownObligations,
                     nextRankState \in ProducerRankStates:
                  CompleteProducerObligation(
                    obligation, nextRankState)
      <3>1. PICK obligation \in fpKnownObligations,
                    nextRankState \in ProducerRankStates:
               CompleteProducerObligation(
                 obligation, nextRankState)
        BY <2>4
      <3>2. obligation \in ProducerObligationSet
        BY <1>1, <3>1, SMT
           DEF ProducerSafetyInvariant, ProducerTypeInvariant
      <3> QED BY <1>1, <3>1, <3>2,
           CompleteProducerObligationPreservesSafetyInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ProducerNext
  <1> QED BY <1>1

THEOREM ProducerSpecPreservesSafetyInvariant ==
  ProducerSpec => []ProducerSafetyInvariant
PROOF
  <1>1. ProducerInit => ProducerSafetyInvariant
    BY ProducerInitEstablishesSafetyInvariant
  <1>2. /\ ProducerSafetyInvariant
         /\ UNCHANGED ProducerEpisodeVars
        => ProducerSafetyInvariant'
    BY Isa
       DEF ProducerEpisodeVars, ProducerSafetyInvariant,
           ProducerTypeInvariant, ProducerGeometryInvariant,
           ProducerConsumedPrefixInvariant,
           ProducerNoResurrectionInvariant,
           ProducerRankProjectionTypeInvariant,
           ProducerKnownSourcesFor, ProducerConsumedEpisodesFor,
           ProducerRemainingEpisodes
  <1>3. /\ ProducerSafetyInvariant
         /\ [ProducerNext]_ProducerEpisodeVars
        => ProducerSafetyInvariant'
    BY <1>2, ProducerNextPreservesSafetyInvariant, Isa
  <1> QED BY <1>1, <1>3, PTL DEF ProducerSpec

(***************************************************************************
Finite remaining debt and the exact four-component well-founded carrier.
***************************************************************************)

THEOREM ProducerRequestPrechargedEpisodeUniverseIsFinite ==
  \A request \in ProducerRequests:
    ProducerConfiguration
      => IsFiniteSet(
           ProducerRequestPrechargedEpisodeUniverse(request))
BY FS_Image, FS_Union, IsaT(600)
   DEF ProducerRequestPrechargedEpisodeUniverse,
       ProducerPrechargedObligationsFor,
       ProducerObligation, ProducerConfiguration,
       ProducerGeometryConfiguration,
       ProducerEpisodeStructureConfiguration,
       ProducerObligationSet

THEOREM ProducerRequestRemainingEpisodeCountIsNatural ==
  \A request \in ProducerRequests:
    ProducerConfiguration
      => ProducerRequestRemainingEpisodeCount(request) \in Nat
BY ProducerRequestPrechargedEpisodeUniverseIsFinite,
   FS_Subset, FS_CardinalityType, Isa
   DEF ProducerRequestRemainingEpisodeCount,
       ProducerRequestRemainingEpisodes,
       ProducerRequestConsumedEpisodes

THEOREM ProducerRemainingEpisodeCountIsNatural ==
  \A obligation \in fpKnownObligations:
    ProducerTypeInvariant
      => ProducerRemainingEpisodeCount(obligation) \in Nat
PROOF
  <1>1. ASSUME NEW obligation \in fpKnownObligations,
                ProducerTypeInvariant
         PROVE ProducerRemainingEpisodeCount(obligation) \in Nat
    <2>1. obligation \in ProducerObligationSet
      BY <1>1, SMT DEF ProducerTypeInvariant
    <2>2. IsFiniteSet(ProducerEpisodeUniverse(obligation))
      BY <1>1, <2>1
         DEF ProducerTypeInvariant, ProducerConfiguration,
             ProducerEpisodeStructureConfiguration
    <2>3. ProducerRemainingEpisodes(obligation)
             \in SUBSET ProducerEpisodeUniverse(obligation)
      BY Isa
         DEF ProducerRemainingEpisodes,
             ProducerConsumedEpisodesFor
    <2>4. IsFiniteSet(ProducerRemainingEpisodes(obligation))
      BY <2>2, <2>3, FS_Subset
    <2> QED BY <2>4, FS_CardinalityType
         DEF ProducerRemainingEpisodeCount
  <1> QED BY <1>1

THEOREM ProducerStageCursorRankOrderingIsWellFounded ==
  ProducerRankOrderingConfiguration
    => IsWellFoundedOn(
         ProducerStageCursorRankOrdering,
         ProducerStageCursorRankCarrier)
BY WFLexPairOrdering
   DEF ProducerRankOrderingConfiguration,
       ProducerStageCursorRankOrdering,
       ProducerStageCursorRankCarrier

THEOREM ProducerSchedulerStageCursorRankOrderingIsWellFounded ==
  ProducerRankOrderingConfiguration
    => IsWellFoundedOn(
         ProducerSchedulerStageCursorRankOrdering,
         ProducerSchedulerStageCursorRankCarrier)
BY ProducerStageCursorRankOrderingIsWellFounded,
   WFLexPairOrdering
   DEF ProducerRankOrderingConfiguration,
       ProducerSchedulerStageCursorRankOrdering,
       ProducerSchedulerStageCursorRankCarrier

THEOREM ProducerRankOrderingIsWellFounded ==
  ProducerRankOrderingConfiguration
    => IsWellFoundedOn(ProducerRankOrdering, ProducerRankCarrier)
BY NatLessThanWellFounded,
   ProducerSchedulerStageCursorRankOrderingIsWellFounded,
   WFLexPairOrdering
   DEF ProducerRankOrdering, ProducerRankCarrier

THEOREM ProducerRequestSourceRankInCarrier ==
  \A request \in ProducerRequests, source \in ProducerSources:
    /\ ProducerSafetyInvariant
    /\ ProducerObligation(request, source) \in fpKnownObligations
    => ProducerRequestSourceRank(request, source)
         \in ProducerRankCarrier
BY ProducerRequestRemainingEpisodeCountIsNatural, Isa
   DEF ProducerSafetyInvariant, ProducerTypeInvariant,
       ProducerRankProjectionTypeInvariant,
       ProducerRequestSourceRank, ProducerRankCarrier,
       ProducerSchedulerStageCursorRankCarrier,
       ProducerStageCursorRankCarrier

THEOREM ProducerRankInCarrier ==
  \A obligation \in fpKnownObligations:
    ProducerSafetyInvariant
      => ProducerRank(obligation) \in ProducerRankCarrier
BY ProducerRequestSourceRankInCarrier, Isa
   DEF ProducerSafetyInvariant, ProducerTypeInvariant,
       ProducerRank, ProducerObligation, ProducerObligationSet

(***************************************************************************
TODO(production descent proof): after the production state machine supplies
the state-sensitive scheduler/stage/cursor projections, prove an exact action
classification through `ProducerProjectionBatchAction` and a strict-rank
lemma for every enabled producer step.  The batch facts above establish only
journal monotonicity and one-time charging; they do not establish that any
production action refines the batch relation.  Until that refinement exists,
no temporal theorem in this module claims that fair scheduling consumes all
remaining episodes.
***************************************************************************)

=============================================================================
