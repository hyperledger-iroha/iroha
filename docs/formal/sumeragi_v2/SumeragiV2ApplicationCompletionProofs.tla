---- MODULE SumeragiV2ApplicationCompletionProofs ----
EXTENDS SumeragiV2ProgressWitnessFinalClosureProofs,
        SumeragiV2AsyncRankClosureProofs

(***************************************************************************
Application-completion reduction.

The imported proof layers already establish all of the source-preservation
facts needed after a durable Decision:

  * `FinalProgressWitnessClosureInvariantObligation` retains the exact
    Decision QC and exactly one recovery/service stage through every runner,
    non-runner, crash, restart, and replay arm;
  * `AsyncStrongTypeInvariant` excludes restart/replay authority after GST;
  * the exact handoff lemmas in
    `SumeragiV2DecisionWitnessPreservationProofs` prove the executable
    FetchBody -> certified request -> FetchCertifiedBody -> StoreBody ->
    ValidateBody -> Apply transitions; and
  * `SumeragiV2AsyncRankClosureProofs` proves protected-candidate starvation
    for `AsyncLiveSpecAt`.

Two temporal facts are not present in those imported layers.  There is no
theorem taking one exact active certified request through retransmission,
packet admission, an applied archive server, and the route-neutral
authenticated response into its exact FetchCertifiedBody owner.  There is
also no Decision-local Stage-2 theorem on `AsyncSpecAt`: the generic Stage-2
closure is intentionally stated only on `AsyncLiveSpecAt`, because an
unrelated exhausted InstallTC owner can violate the global
`AsyncInstallGenerationBudget`.

`ExactDecisionStageServiceProperty` below is therefore the smallest exact
missing temporal lemma.  It starts only after the already-proved source
invariant has supplied a current, exact, non-recovery Decision stage.  The
theorems in this module prove that this one lemma is sufficient to discharge
the existing `ApplicationCompletionProgressProperty` exactly.  The lemma is
kept as an operator, not asserted as a theorem or assumption, so this module
does not turn the remaining service-composition debt into an imported fact.
***************************************************************************)

ExactDecisionRecord(node, qc) ==
  /\ [node |-> node, qc |-> qc] \in decisions
  /\ qc.context = context
  /\ qc.phase = "Commit"

ExactDecisionServiceSource(node, qc) ==
  /\ gst
  /\ node \in AsyncCurrentResponsiveVoters
  /\ ExactDecisionRecord(node, qc)
  /\ DecisionRecoveryStageExact(node, qc)

(***************************************************************************
The GST recovery-phase invariant rules out both phases which can own durable
Decision recovery authority.  This is stronger than merely proving that the
node is not replay-quarantined: it eliminates the authority disjunct from the
exact source invariant itself.
***************************************************************************)

THEOREM GstExcludesDecisionRecoveryAuthority ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  => \A node, qc: ~DecisionRecoveryAuthority(node, qc)
BY Isa
   DEF AsyncStrongTypeInvariant, AsyncGstRecoveryPhaseInvariant,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority

THEOREM ExactDecisionSourceProjectsPostGstServiceStage ==
  \A node, qc:
    /\ AsyncStrongTypeInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ gst
    /\ node \in AsyncCurrentResponsiveVoters
    /\ ExactDecisionRecord(node, qc)
    => DecisionRecoveryStageExact(node, qc)
BY GstExcludesDecisionRecoveryAuthority, Isa
   DEF DecisionExactSourceRetentionInvariant,
       AsyncDecisionRecoveryStageExact, ExactDecisionRecord

THEOREM PostGstResponsiveDecisionHasExactServiceSource ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ gst
    /\ node \in AsyncCurrentResponsiveVoters
    /\ NodeHasDecision(node)
    => \E qc: ExactDecisionServiceSource(node, qc)
BY ExactDecisionSourceProjectsPostGstServiceStage, Isa
   DEF NodeHasDecision, ExactDecisionRecord,
       ExactDecisionServiceSource

(***************************************************************************
The generic Stage-2 budget is stronger than this pipeline needs.  A current
durable Decision excludes a pending InstallTC at the same node, so an exact
Decision owner can never be blocked by local generation exhaustion.  What is
missing is the temporal Stage-2 specialization which threads this local fact
through the existing Busy-owner rank; the model does not need or justify
assuming the budget for unrelated validators.
***************************************************************************)

THEOREM ExactDecisionSourceExcludesLocalInstallExhaustion ==
  \A node, qc:
    /\ DecisionTimeoutFrontierInvariant
    /\ ExactDecisionRecord(node, qc)
    => ~InstallGenerationExhausted(node)
BY DurableDecisionNodeCannotOwnPendingInstall, Isa
   DEF ExactDecisionRecord, InstallGenerationExhausted

THEOREM ExactDecisionExecutableOwnerIsResponsiveProtected ==
  \A node, qc, candidate:
    /\ node \in AsyncCurrentResponsiveVoters
    /\ DecisionExecutableStageOwner(node, qc, candidate)
    => ResponsiveProtectedCandidateOwned(candidate)
BY Isa
   DEF DecisionExecutableStageOwner, DecisionPipelineCandidate,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate

(***************************************************************************
This is the exact remaining lemma, stated at the narrowest preserved source.
It does not quantify over an abstract coordinate-compatible candidate and it
does not treat a route, relay, archive signature owner, or cited QC signer as
interchangeable.  Its source is the exact durable Decision QC plus the exact
route-neutral service stage retained by the final witness invariant.
***************************************************************************)

ExactDecisionStageServiceProperty(specification) ==
  specification
    => \A node, qc:
         ExactDecisionServiceSource(node, qc)
           ~> NodeHasApplication(node)

THEOREM AsyncSpecAlwaysSuppliesExactDecisionServiceSource ==
  \A initialContext, node:
    /\ AsyncSpecAt(initialContext)
    /\ node \in AsyncCurrentResponsiveVoters
    => []((gst /\ NodeHasDecision(node))
            => \E qc: ExactDecisionServiceSource(node, qc))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node,
                AsyncSpecAt(initialContext),
                node \in AsyncCurrentResponsiveVoters
         PROVE []((gst /\ NodeHasDecision(node))
                    => \E qc:
                         ExactDecisionServiceSource(node, qc))
    <2>1. [](AsyncCurrentResponsiveVoters
               = AsyncVotersAt(initialContext))
      BY <1>1, AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>2. [] (node \in AsyncCurrentResponsiveVoters)
      BY <1>1, <2>1, PTL
    <2>3. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>4. []FinalProgressWitnessClosureInvariant
      BY <1>1, FinalProgressWitnessClosureInvariantObligation
    <2>5. [](gst /\ NodeHasDecision(node)
               => \E qc: ExactDecisionServiceSource(node, qc))
      BY <2>2, <2>3, <2>4,
         PostGstResponsiveDecisionHasExactServiceSource, PTL
         DEF FinalProgressWitnessClosureInvariant,
             FinalWitnessSourceRetentionInvariant
    <2> QED BY <2>5
  <1> QED BY <1>1

(***************************************************************************
Deductive discharge of the release-facing property from the one missing
service lemma.  The existential lift is sound because the exact Decision
source is retained until application, while the fixed-context theorem keeps
the initially quantified responsive voter in the same voter domain.
***************************************************************************)

THEOREM ExactDecisionStageServiceDischargesApplicationCompletion ==
  \A initialContext:
    ExactDecisionStageServiceProperty(AsyncSpecAt(initialContext))
      => ApplicationCompletionProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ExactDecisionStageServiceProperty(
                  AsyncSpecAt(initialContext))
         PROVE ApplicationCompletionProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node \in AsyncCurrentResponsiveVoters:
                   (gst /\ NodeHasDecision(node))
                     ~> NodeHasApplication(node)
      <3>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters
             PROVE (gst /\ NodeHasDecision(node))
                     ~> NodeHasApplication(node)
        <4>1. []((gst /\ NodeHasDecision(node))
                   => \E qc:
                        ExactDecisionServiceSource(node, qc))
          BY <2>1, <3>1,
             AsyncSpecAlwaysSuppliesExactDecisionServiceSource
        <4>2. \A qc:
                 ExactDecisionServiceSource(node, qc)
                   ~> NodeHasApplication(node)
          BY <1>1, <2>1
             DEF ExactDecisionStageServiceProperty
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
         DEF ApplicationCompletionProgressProperty
  <1> QED BY <1>1

THEOREM ApplicationCompletionProgressReduction ==
  (\A initialContext:
     ExactDecisionStageServiceProperty(AsyncSpecAt(initialContext)))
    => \A initialContext:
         ApplicationCompletionProgressProperty(
           AsyncSpecAt(initialContext))
BY ExactDecisionStageServiceDischargesApplicationCompletion

=============================================================================
