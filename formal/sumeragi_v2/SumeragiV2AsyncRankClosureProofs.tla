---- MODULE SumeragiV2AsyncRankClosureProofs ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs

(***************************************************************************
Acyclic closure of the independently proved protected-service rank leaves.

The Stage-2, Stage-3, and Stage-6 leaves come directly from the proof-bearing
asynchronous base. Consequently their strict corridors cannot import either of
the aggregate results below, or any of the still-open temporal claims in
`SumeragiV2AsyncTemporalClosureProofs`.
***************************************************************************)

THEOREM AsyncRankClosureProtectedServiceRankProgressObligation ==
  \A initialContext:
    ProtectedServiceRanksProgressProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ProtectedServiceRanksProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ProtectedServiceFiniteRunnerEpisodeClosureProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure
    <2>2. ProtectedStage3RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>1, ProtectedStage3RankProgressFromFairSchedulerObligation
         DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty
    <2>3. ProtectedStage4RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>1, ProtectedStage4RankProgressFromFairScheduler
         DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty,
             Stage6FiniteRunnerEpisodeClosureProperty
    <2>4. ProtectedStage5RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage5RankProgressFromFairFifo
    <2>5. ProtectedStage6RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>1, ProtectedStage6RankProgressFromFairCausalAdmissionObligation
         DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty
    <2>6. ProtectedPostDeferredRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>2, <2>3, <2>4, <2>5,
         ProtectedPostDeferredRanksComposeFromLeavesObligation
    <2>7. ProtectedStage2RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>6, ProtectedStage2RankProgressWithExactHandoffObligation,
         PTL
    <2>8. ProtectedServeRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedServeRankProgressFromFairFifo
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>7, <2>8,
         ProtectedServiceRanksProgressLeafCompositionObligation
  <1> QED BY <1>1

THEOREM AsyncRankClosureStarvationFreedomObligation ==
  \A initialContext:
    StarvationFreedomProperty(AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE StarvationFreedomProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ProtectedServiceRanksProgressProperty(
             AsyncSpecAt(initialContext))
      BY AsyncRankClosureProtectedServiceRankProgressObligation
    <2>2. AsyncLiveSpecAt(initialContext)
             => AsyncSpecAt(initialContext)
      BY AsyncLiveSpecProjectsAsyncSpec
    <2> QED BY <2>1, <2>2,
         ProtectedServiceRankProgressImpliesStarvation, PTL
         DEF StarvationFreedomProperty,
             ProtectedServeStarvationProperty
  <1> QED BY <1>1

=============================================================================
