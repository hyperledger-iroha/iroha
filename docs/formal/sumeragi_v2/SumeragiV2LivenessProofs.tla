---- MODULE SumeragiV2LivenessProofs ----
EXTENDS SumeragiV2AsyncNetwork, SumeragiV2Proofs

(***************************************************************************
One-height liveness vocabulary and well-founded service measures.

This module contains no second consensus relation and no favourable network
step.  Every temporal property below is stated over the unbounded
`AsyncSpecAt(initialContext)`.  The asynchronous proof module discharges these
properties from the concrete FIFO, fair-ingress, IO-worker, retransmission,
and absolute-timeout actions.
***************************************************************************)

OneHeightDecisionLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => PostGstEventuallyAsyncDecisionAt(initialContext)

OneHeightApplicationLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => ResponsiveDecisionEventuallyAppliedAt(initialContext)

OneHeightCompletionLiveness(initialContext) ==
  AsyncSpecAt(initialContext)
    => (gst ~> AsyncAllResponsiveAppliedAt(initialContext))

CanonicalSuccessorContext(initialContext, subject) ==
  ContextRecord(initialContext.height + 1,
                Append(initialContext.lineage, subject))

CanonicalSuccessorAdmissible(initialContext, subject) ==
  /\ FrozenContextAdmissible(initialContext)
  /\ initialContext.height < MaxHeight
  /\ subject \in ValidSubjects
  /\ FrozenContextAdmissible(
       CanonicalSuccessorContext(initialContext, subject))

THEOREM IoAdmissionLimitsAreStrictlyReserved ==
  AsyncConfiguration
    => /\ AsyncIoAdmissionLimit("Serve")
             < AsyncIoAdmissionLimit("Consensus")
       /\ AsyncIoAdmissionLimit("Consensus")
             < AsyncIoAdmissionLimit("Control")
       /\ AsyncIoAdmissionLimit("Control") = AsyncIoCapacity
BY SMT DEF AsyncConfiguration, AsyncIoAdmissionLimit, AsyncIoCapacity

THEOREM RuntimeReachRankIsNatural ==
  AsyncTypeInvariant
    => \A node \in ValidatorIds: RuntimeReachRank(node) \in Nat
BY SMT DEF AsyncTypeInvariant, RuntimeReachRank

THEOREM RetransmissionBudgetCoversEveryClass ==
  AsyncConfiguration
    => /\ AsyncRetainedControlBudget \in Nat
       /\ AsyncRetainedProposalChunkBudget \in Nat
       /\ AsyncActiveCertifiedRequestBudget \in Nat
       /\ AsyncActiveCommitRequestBudget \in Nat
       /\ AsyncActiveRequestBudget
             = AsyncActiveCertifiedRequestBudget
                 + AsyncActiveCommitRequestBudget
       /\ AsyncRetransmitEmissionBudget
             = AsyncRetainedControlBudget
                 + AsyncRetainedProposalChunkBudget
                 + AsyncActiveRequestBudget
BY SMT DEF AsyncConfiguration, AsyncRetainedControlBudget,
           AsyncRetainedProposalChunkBudget,
           AsyncActiveCertifiedRequestBudget,
           AsyncActiveCommitRequestBudget, AsyncActiveRequestBudget,
           AsyncRetransmitEmissionBudget

THEOREM CanonicalSuccessorPreservesAdmissibility ==
  \A initialContext \in ContextRecords, subject \in ValidSubjects:
    (FrozenContextAdmissible(initialContext)
      /\ initialContext.height < MaxHeight)
      => FrozenContextAdmissible(
           CanonicalSuccessorContext(initialContext, subject))
BY SMT DEF FrozenContextAdmissible, CanonicalSuccessorContext,
           ContextRecords, LineagesAt, ContextRecord, Heights

=============================================================================
