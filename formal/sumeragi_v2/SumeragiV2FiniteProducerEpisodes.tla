---- MODULE SumeragiV2FiniteProducerEpisodes ----
EXTENDS Naturals, Sequences, FiniteSets, Functions, WellFoundedInduction

(***************************************************************************
Finite producer-episode kernel.

This module isolates the finite-production obligation needed by the
post-GST liveness argument.  A stable semantic request and an authenticated
network source form one immutable obligation identity.  Each identity has a
finite, typed universe of producer episodes.  The two state variables are
monotone journals:

  * `fpKnownObligations` records every request/source identity observed; and
  * `fpConsumedEpisodes` records every episode already discharged.

The kernel deliberately has no dependency on a production consensus module.
Production refinement must instantiate the opaque identity, stage, cursor,
and rank projections without weakening these journal semantics.  In
particular, the first rank component precharges every configured authenticated
source for a request before any source is observed; discovering an alternate
source therefore spends existing debt instead of creating new debt.
***************************************************************************)

CONSTANTS
  ProducerRequests,
  ProducerSources,
  ProducerSourceOrder,
  ProducerSourceCapacity,
  ProducerStages,
  ProducerCursors,
  ProducerEpisodeUniverse(_),
  ProducerInitialEpisodes(_),
  ProducerEpisodePredecessors(_),
  ProducerRankStates,
  ProducerInitialRankState,
  ProducerSchedulerRankCarrier,
  ProducerSchedulerRankOrdering,
  ProducerSchedulerRank(_, _),
  ProducerStageRankCarrier,
  ProducerStageRankOrdering,
  ProducerStageRank(_, _),
  ProducerCursorRankCarrier,
  ProducerCursorRankOrdering,
  ProducerCursorRank(_, _)

VARIABLES fpKnownObligations, fpConsumedEpisodes, fpRankState

ProducerEpisodeVars ==
  <<fpKnownObligations, fpConsumedEpisodes, fpRankState>>

(***************************************************************************
Stable identities and typed episode universes.
***************************************************************************)

ProducerObligation(request, source) ==
  [request |-> request, source |-> source]

ProducerObligationSet ==
  [request: ProducerRequests, source: ProducerSources]

ProducerEpisode(obligation, stage, cursor) ==
  [obligation |-> obligation, stage |-> stage, cursor |-> cursor]

ProducerEpisodeSet ==
  [obligation: ProducerObligationSet,
   stage: ProducerStages,
   cursor: ProducerCursors]

ProducerEpisodesOfType(obligation) ==
  {episode \in ProducerEpisodeSet:
     episode.obligation = obligation}

ProducerEpisodeSetPrefixClosed(obligation, episodes) ==
  /\ episodes \subseteq ProducerEpisodeUniverse(obligation)
  /\ \A episode \in episodes:
       ProducerEpisodePredecessors(episode) \subseteq episodes

(***************************************************************************
The request-global precharge is independent of the known-obligation journal.
It enumerates every authenticated source admitted by the configured geometry,
including sources that production has not observed yet.  The universes remain
source-typed, so their episodes cannot alias another source's charge.
***************************************************************************)

ProducerPrechargedObligationsFor(request) ==
  {ProducerObligation(request, source): source \in ProducerSources}

ProducerRequestPrechargedEpisodeUniverse(request) ==
  UNION {ProducerEpisodeUniverse(obligation):
           obligation \in ProducerPrechargedObligationsFor(request)}

(***************************************************************************
Configuration is explicit about authenticated source geometry.  The source
order is a duplicate-free enumeration of the entire source set, and the
reservation capacity is exactly that geometry.  A production specialization
therefore cannot silently reserve fewer attempts than the configured network
can authenticate for one semantic request.
***************************************************************************)

ProducerGeometryConfiguration ==
  /\ ProducerRequests # {}
  /\ ProducerSources # {}
  /\ IsFiniteSet(ProducerSources)
  /\ ProducerSourceOrder \in Seq(ProducerSources)
  /\ {ProducerSourceOrder[index]:
        index \in 1..Len(ProducerSourceOrder)} = ProducerSources
  /\ \A left, right \in 1..Len(ProducerSourceOrder):
       ProducerSourceOrder[left] = ProducerSourceOrder[right]
         => left = right
  /\ ProducerSourceCapacity = Len(ProducerSourceOrder)
  /\ ProducerSourceCapacity = Cardinality(ProducerSources)
  /\ ProducerSourceCapacity \in Nat \ {0}

ProducerEpisodeStructureConfiguration ==
  /\ ProducerStages # {}
  /\ ProducerCursors # {}
  /\ IsFiniteSet(ProducerStages)
  /\ IsFiniteSet(ProducerCursors)
  /\ \A obligation \in ProducerObligationSet:
       /\ IsFiniteSet(ProducerEpisodeUniverse(obligation))
       /\ ProducerEpisodeUniverse(obligation) # {}
       /\ ProducerEpisodeUniverse(obligation)
            \subseteq ProducerEpisodesOfType(obligation)
       /\ ProducerInitialEpisodes(obligation) # {}
       /\ ProducerInitialEpisodes(obligation)
            \subseteq ProducerEpisodeUniverse(obligation)
       /\ ProducerEpisodeSetPrefixClosed(
            obligation, ProducerInitialEpisodes(obligation))
       /\ \A episode \in ProducerEpisodeUniverse(obligation):
            /\ ProducerEpisodePredecessors(episode)
                 \subseteq ProducerEpisodeUniverse(obligation)
            /\ episode \notin ProducerEpisodePredecessors(episode)

ProducerRankOrderingConfiguration ==
  /\ ProducerRankStates # {}
  /\ ProducerInitialRankState \in ProducerRankStates
  /\ ProducerSchedulerRankCarrier # {}
  /\ ProducerStageRankCarrier # {}
  /\ ProducerCursorRankCarrier # {}
  /\ ProducerSchedulerRankOrdering
       \subseteq
         ProducerSchedulerRankCarrier \X ProducerSchedulerRankCarrier
  /\ ProducerStageRankOrdering
       \subseteq ProducerStageRankCarrier \X ProducerStageRankCarrier
  /\ ProducerCursorRankOrdering
       \subseteq ProducerCursorRankCarrier \X ProducerCursorRankCarrier
  /\ IsWellFoundedOn(
       ProducerSchedulerRankOrdering, ProducerSchedulerRankCarrier)
  /\ IsWellFoundedOn(
       ProducerStageRankOrdering, ProducerStageRankCarrier)
  /\ IsWellFoundedOn(
       ProducerCursorRankOrdering, ProducerCursorRankCarrier)
  /\ \A rankState \in ProducerRankStates:
       \A obligation \in ProducerObligationSet:
         /\ ProducerSchedulerRank(rankState, obligation)
              \in ProducerSchedulerRankCarrier
         /\ ProducerStageRank(rankState, obligation)
              \in ProducerStageRankCarrier
         /\ ProducerCursorRank(rankState, obligation)
              \in ProducerCursorRankCarrier

ProducerConfiguration ==
  /\ ProducerGeometryConfiguration
  /\ ProducerEpisodeStructureConfiguration
  /\ ProducerRankOrderingConfiguration

(***************************************************************************
Journal projections and safety invariants.
***************************************************************************)

ProducerKnownSourcesFor(request) ==
  {source \in ProducerSources:
     ProducerObligation(request, source) \in fpKnownObligations}

ProducerConsumedEpisodesFor(obligation) ==
  fpConsumedEpisodes \cap ProducerEpisodeUniverse(obligation)

ProducerRemainingEpisodes(obligation) ==
  ProducerEpisodeUniverse(obligation)
    \ ProducerConsumedEpisodesFor(obligation)

ProducerRemainingEpisodeCount(obligation) ==
  Cardinality(ProducerRemainingEpisodes(obligation))

ProducerRequestConsumedEpisodes(request) ==
  fpConsumedEpisodes
    \cap ProducerRequestPrechargedEpisodeUniverse(request)

ProducerRequestRemainingEpisodes(request) ==
  ProducerRequestPrechargedEpisodeUniverse(request)
    \ ProducerRequestConsumedEpisodes(request)

ProducerRequestRemainingEpisodeCount(request) ==
  Cardinality(ProducerRequestRemainingEpisodes(request))

ProducerObligationComplete(obligation) ==
  ProducerRemainingEpisodes(obligation) = {}

ProducerCompletedObligations ==
  {obligation \in fpKnownObligations:
     ProducerObligationComplete(obligation)}

ProducerTypeInvariant ==
  /\ ProducerConfiguration
  /\ fpKnownObligations \subseteq ProducerObligationSet
  /\ fpConsumedEpisodes
       \subseteq
         UNION {ProducerEpisodeUniverse(obligation):
                  obligation \in fpKnownObligations}
  /\ fpRankState \in ProducerRankStates

ProducerGeometryInvariant ==
  \A request \in ProducerRequests:
    Cardinality(ProducerKnownSourcesFor(request))
      <= ProducerSourceCapacity

ProducerConsumedPrefixInvariant ==
  \A obligation \in fpKnownObligations:
    ProducerEpisodeSetPrefixClosed(
      obligation, ProducerConsumedEpisodesFor(obligation))

(***************************************************************************
The state form exposes the exact consumed/remaining partition.  The step
form below is the non-resurrection property: once an episode enters the
consumed journal, no later kernel action can make it pending again.
***************************************************************************)

ProducerNoResurrectionInvariant ==
  \A obligation \in fpKnownObligations:
    /\ ProducerConsumedEpisodesFor(obligation)
         \cap ProducerRemainingEpisodes(obligation) = {}
    /\ ProducerConsumedEpisodesFor(obligation)
         \cup ProducerRemainingEpisodes(obligation)
           = ProducerEpisodeUniverse(obligation)

ProducerRankProjectionTypeInvariant ==
  \A obligation \in fpKnownObligations:
    /\ ProducerSchedulerRank(fpRankState, obligation)
         \in ProducerSchedulerRankCarrier
    /\ ProducerStageRank(fpRankState, obligation)
         \in ProducerStageRankCarrier
    /\ ProducerCursorRank(fpRankState, obligation)
         \in ProducerCursorRankCarrier

ProducerSafetyInvariant ==
  /\ ProducerTypeInvariant
  /\ ProducerGeometryInvariant
  /\ ProducerConsumedPrefixInvariant
  /\ ProducerNoResurrectionInvariant
  /\ ProducerRankProjectionTypeInvariant

ProducerKnownJournalMonotone ==
  fpKnownObligations \subseteq fpKnownObligations'

ProducerConsumedJournalMonotone ==
  fpConsumedEpisodes \subseteq fpConsumedEpisodes'

ProducerNoResurrectionStep ==
  \A episode \in fpConsumedEpisodes:
    episode \in fpConsumedEpisodes'

ProducerJournalStepInvariant ==
  /\ ProducerKnownJournalMonotone
  /\ ProducerConsumedJournalMonotone
  /\ ProducerNoResurrectionStep

(***************************************************************************
Kernel actions.

`ObserveNewProducerSource` creates exactly one geometry-bounded obligation.
Its nonempty initial prefix represents the source-observation episode(s).
Downstream production stages, including item zero, remain unconsumed.

`TransferProducerObligation` is the journal projection of a same-source
refresh: it transfers the existing obligation and cannot create or consume
another one.  The opaque rank snapshot may change so a production
specialization can project real scheduler/stage/cursor state.  The production
refinement must separately show that the matching source's message/chunk
cursor is preserved.
***************************************************************************)

ProducerInit ==
  /\ ProducerConfiguration
  /\ fpKnownObligations = {}
  /\ fpConsumedEpisodes = {}
  /\ fpRankState = ProducerInitialRankState

ObserveNewProducerSource(request, source, nextRankState) ==
  LET obligation == ProducerObligation(request, source)
  IN
    /\ request \in ProducerRequests
    /\ source \in ProducerSources
    /\ obligation \notin fpKnownObligations
    /\ Cardinality(ProducerKnownSourcesFor(request))
         < ProducerSourceCapacity
    /\ fpKnownObligations' = fpKnownObligations \cup {obligation}
    /\ fpConsumedEpisodes' =
         fpConsumedEpisodes \cup ProducerInitialEpisodes(obligation)
    /\ nextRankState \in ProducerRankStates
    /\ fpRankState' = nextRankState

TransferProducerObligation(obligation, nextRankState) ==
  /\ obligation \in ProducerObligationSet
  /\ obligation \in fpKnownObligations
  /\ fpKnownObligations' = fpKnownObligations
  /\ fpConsumedEpisodes' = fpConsumedEpisodes
  /\ nextRankState \in ProducerRankStates
  /\ fpRankState' = nextRankState

(***************************************************************************
One production transition may expose several new source obligations while it
also advances several already-known obligations.  This projection action is
the exact atomic journal relation for that case:

  * `freshObligations` contains every newly exposed request/source member;
  * `transferredObligations` records every same-source ownership transfer;
  * `episodeChargeOrder` is duplicate-free, contains every fresh observation
    prefix, and contains no episode already present in the consumed journal.

The action is intentionally not wired into `ProducerNext` yet.  A production
specialization must first prove the open batch-projection premise in the
temporal companion for its complete real action inventory.  This keeps the
kernel useful without claiming an unreviewed production integration.
***************************************************************************)

ProducerProjectionBatchAction(
    freshObligations,
    transferredObligations,
    episodeChargeOrder,
    nextRankState) ==
  LET projectedObligations ==
        fpKnownObligations \cup freshObligations
      episodeCharges == Range(episodeChargeOrder)
  IN
    /\ freshObligations
         \subseteq ProducerObligationSet \ fpKnownObligations
    /\ transferredObligations \subseteq fpKnownObligations
    /\ episodeChargeOrder \in Seq(ProducerEpisodeSet)
    /\ Len(episodeChargeOrder) = Cardinality(episodeCharges)
    /\ \/ freshObligations # {}
       \/ transferredObligations # {}
       \/ episodeCharges # {}
    /\ \A request \in ProducerRequests:
         Cardinality(
           {source \in ProducerSources:
              ProducerObligation(request, source)
                \in projectedObligations})
           <= ProducerSourceCapacity
    /\ episodeCharges \cap fpConsumedEpisodes = {}
    /\ episodeCharges
         \subseteq
           UNION {ProducerEpisodeUniverse(obligation):
                    obligation \in projectedObligations}
    /\ \A obligation \in freshObligations:
         ProducerInitialEpisodes(obligation) \subseteq episodeCharges
    /\ \A obligation \in projectedObligations:
         ProducerEpisodeSetPrefixClosed(
           obligation,
           (fpConsumedEpisodes \cup episodeCharges)
             \cap ProducerEpisodeUniverse(obligation))
    /\ fpKnownObligations' = projectedObligations
    /\ fpConsumedEpisodes' = fpConsumedEpisodes \cup episodeCharges
    /\ nextRankState \in ProducerRankStates
    /\ fpRankState' = nextRankState

ConsumeProducerEpisodes(obligation, episodes, nextRankState) ==
  /\ obligation \in fpKnownObligations
  /\ episodes # {}
  /\ episodes \subseteq ProducerRemainingEpisodes(obligation)
  /\ ProducerEpisodeSetPrefixClosed(
       obligation,
       ProducerConsumedEpisodesFor(obligation) \cup episodes)
  /\ fpKnownObligations' = fpKnownObligations
  /\ fpConsumedEpisodes' = fpConsumedEpisodes \cup episodes
  /\ nextRankState \in ProducerRankStates
  /\ fpRankState' = nextRankState

CompleteProducerObligation(obligation, nextRankState) ==
  /\ obligation \in fpKnownObligations
  /\ ~ProducerObligationComplete(obligation)
  /\ fpKnownObligations' = fpKnownObligations
  /\ fpConsumedEpisodes' =
       fpConsumedEpisodes \cup ProducerEpisodeUniverse(obligation)
  /\ nextRankState \in ProducerRankStates
  /\ fpRankState' = nextRankState

ProducerNext ==
  \/ \E request \in ProducerRequests, source \in ProducerSources,
       nextRankState \in ProducerRankStates:
       ObserveNewProducerSource(request, source, nextRankState)
  \/ \E obligation \in fpKnownObligations,
       nextRankState \in ProducerRankStates:
       TransferProducerObligation(obligation, nextRankState)
  \/ \E obligation \in fpKnownObligations:
       \E episodes \in SUBSET ProducerEpisodeUniverse(obligation),
          nextRankState \in ProducerRankStates:
         ConsumeProducerEpisodes(obligation, episodes, nextRankState)
  \/ \E obligation \in fpKnownObligations,
       nextRankState \in ProducerRankStates:
       CompleteProducerObligation(obligation, nextRankState)

ProducerSpec ==
  ProducerInit /\ [][ProducerNext]_ProducerEpisodeVars

ProducerAllKnownObligationsComplete ==
  \A obligation \in fpKnownObligations:
    ProducerObligationComplete(obligation)

(***************************************************************************
Four-component request/source lexicographic rank.

`ProducerRankComponents` is the readable four-tuple:

  <<request-global precharged remaining episodes,
    scheduler rank, stage rank, source cursor rank>>

`ProducerRank` is the equivalent right-nested pair required by the standard
`WFLexPairOrdering` constructor.  Thus the first component dominates every
tail component; scheduler dominates stage and cursor; and stage dominates
cursor.
***************************************************************************)

ProducerStageCursorRankCarrier ==
  ProducerStageRankCarrier \X ProducerCursorRankCarrier

ProducerStageCursorRankOrdering ==
  LexPairOrdering(
    ProducerStageRankOrdering,
    ProducerCursorRankOrdering,
    ProducerStageRankCarrier,
    ProducerCursorRankCarrier)

ProducerSchedulerStageCursorRankCarrier ==
  ProducerSchedulerRankCarrier \X ProducerStageCursorRankCarrier

ProducerSchedulerStageCursorRankOrdering ==
  LexPairOrdering(
    ProducerSchedulerRankOrdering,
    ProducerStageCursorRankOrdering,
    ProducerSchedulerRankCarrier,
    ProducerStageCursorRankCarrier)

ProducerRankCarrier ==
  Nat \X ProducerSchedulerStageCursorRankCarrier

ProducerRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    ProducerSchedulerStageCursorRankOrdering,
    Nat,
    ProducerSchedulerStageCursorRankCarrier)

ProducerRequestSourceRankComponents(request, source) ==
  LET obligation == ProducerObligation(request, source)
  IN
    <<ProducerRequestRemainingEpisodeCount(request),
      ProducerSchedulerRank(fpRankState, obligation),
      ProducerStageRank(fpRankState, obligation),
      ProducerCursorRank(fpRankState, obligation)>>

ProducerRequestSourceRank(request, source) ==
  LET obligation == ProducerObligation(request, source)
  IN
    <<ProducerRequestRemainingEpisodeCount(request),
      <<ProducerSchedulerRank(fpRankState, obligation),
        <<ProducerStageRank(fpRankState, obligation),
          ProducerCursorRank(fpRankState, obligation)>>
      >>
    >>

ProducerRankComponents(obligation) ==
  ProducerRequestSourceRankComponents(
    obligation.request, obligation.source)

ProducerRank(obligation) ==
  ProducerRequestSourceRank(obligation.request, obligation.source)

ProducerRankStrictlyDecreases(obligation) ==
  <<ProducerRank(obligation)', ProducerRank(obligation)>>
    \in ProducerRankOrdering

(***************************************************************************
TODO(production refinement): instantiate semantic request identity,
authenticated source identity, and the exact configured source geometry;
map every ingress/lane/runner/worker/sidecar/daemon producer transition to
New, Transfer, Consume, Complete, the exact batch projection, or a kernel
stutter; and prove that the request-global precharged episode universe is a
finite upper bound over all such source/stage producer episodes.

TODO(descent integration): instantiate `ProducerRankStates` with the exact
production snapshot needed by the scheduler, stage, and per-source cursor
projections, then prove that a stable incomplete obligation under the reviewed
fairness and local-termination assumptions either completes or takes a
transition whose four-component rank strictly decreases.  This generic kernel
proves the carrier is well-founded; it does not claim that unconnected
production transitions descend it.

TODO(source isolation): prove that a same-source refresh refines Transfer
without resetting its cursor, an alternate source refines New with downstream
item zero still pending, and cancellation/Complete changes only the matching
obligation.  Only after those projections are source-bound may this kernel be
composed with the height-progress theorem.
***************************************************************************)

=============================================================================
