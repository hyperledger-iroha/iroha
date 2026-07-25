---- MODULE SumeragiV2AsyncRankClosureProofs ----
EXTENDS SumeragiV2Stage2BusyRankScratch,
        SumeragiV2Stage3CursorKernelScratch,
        SumeragiV2Stage6CapacityScratch

(***************************************************************************
Acyclic closure of the independently proved protected-service rank leaves.

The Stage-2, Stage-3, and Stage-6 modules extend only the proof-bearing
asynchronous base.  Consequently their strict corridors cannot import either
of the aggregate results below, or any of the still-open temporal claims in
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
    <2>1. ProtectedStage3RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage3RankProgressFromFairSchedulerObligation
    <2>2. ProtectedStage4RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage4RankProgressFromFairScheduler
    <2>3. ProtectedStage5RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage5RankProgressFromFairFifo
    <2>4. ProtectedStage6RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage6RankProgressFromFairCausalAdmissionObligation
    <2>5. ProtectedPostDeferredRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>1, <2>2, <2>3, <2>4,
         ProtectedPostDeferredRanksComposeFromLeavesObligation
    <2>6. ProtectedStage2RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>5, ProtectedStage2RankProgressWithExactHandoffObligation,
         PTL
    <2>7. ProtectedServeRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedServeRankProgressFromFairFifo
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>6, <2>7,
         ProtectedServiceRanksProgressLeafCompositionObligation
  <1> QED BY <1>1

THEOREM AsyncRankClosureStarvationFreedomObligation ==
  \A initialContext:
    StarvationFreedomProperty(AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE StarvationFreedomProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec
    <2>2. ProtectedServiceRanksProgressProperty(
             AsyncSpecAt(initialContext))
      BY AsyncRankClosureProtectedServiceRankProgressObligation
    <2>3. StarvationFreedomProperty(AsyncSpecAt(initialContext))
      BY <2>1, <2>2,
         ProtectedServiceRankProgressImpliesStarvation
    <2> QED BY <2>3, PTL DEF StarvationFreedomProperty
  <1> QED BY <1>1

=============================================================================
