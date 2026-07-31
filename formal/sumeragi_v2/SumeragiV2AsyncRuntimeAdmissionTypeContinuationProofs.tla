---- MODULE SumeragiV2AsyncRuntimeAdmissionTypeContinuationProofs ----
EXTENDS SumeragiV2AsyncRuntimeAdmissionTypeProofs

THEOREM IngressAdmissionRunnerPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ IngressDrainStep(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                IngressDrainStep(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE asyncRunnerBudget[node] > 0
                   /\ asyncIngressReady[node] # <<>>
                   /\ DrainableIngressIndices(node) # {}
      BY <1>1, <2>1, IngressDrainRunnerPreservesSchedulerType
    <2>2. CASE ~(asyncRunnerBudget[node] > 0
                    /\ asyncIngressReady[node] # <<>>
                    /\ DrainableIngressIndices(node) # {})
      BY <1>1, <2>2, IngressPhaseAdvancePreservesSchedulerType
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

=============================================================================
