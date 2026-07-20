---- MODULE SumeragiV2TimeoutSigningInvariant ----
EXTENDS SumeragiV2TimeoutDurability

(***************************************************************************
Inductive closure of timeout-sign request provenance.  Only four Core actions
can change signTimeouts: PersistTimeout and ResumeTimeout add exact requests,
CompleteTimeoutSignature removes one, and Crash filters a node's requests.
***************************************************************************)

TimeoutSigningMutationStep ==
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
  \/ \E node \in ValidatorIds: Crash(node)
  \/ \E node \in ValidatorIds, vote \in timeoutIntents:
       ResumeTimeout(node, vote)

StrongTimeoutDurabilityInvariant ==
  /\ StrongInductiveInvariant
  /\ TimeoutSigningProvenanceInvariant

THEOREM UnchangedTimeoutSigningPreservesProvenance ==
  (TimeoutSigningProvenanceInvariant /\ UNCHANGED signTimeouts)
    => TimeoutSigningProvenanceInvariant'
BY DEF TimeoutSigningProvenanceInvariant

THEOREM SubsetTimeoutSigningPreservesProvenance ==
  /\ TimeoutSigningProvenanceInvariant
  /\ signTimeouts' \subseteq signTimeouts
  => TimeoutSigningProvenanceInvariant'
BY Isa DEF TimeoutSigningProvenanceInvariant

THEOREM PersistTimeoutPreservesSigningProvenance ==
  \A request \in pendingTimeout:
    (/\ StrongInductiveInvariant
     /\ TimeoutSigningProvenanceInvariant
     /\ PersistTimeout(request))
      => TimeoutSigningProvenanceInvariant'
PROOF
  <1>1. ASSUME NEW request \in pendingTimeout,
                StrongInductiveInvariant,
                TimeoutSigningProvenanceInvariant,
                PersistTimeout(request)
         PROVE TimeoutSigningProvenanceInvariant'
    <2>1. request.vote.signer = request.node
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized
    <2>2. signTimeouts' =
             signTimeouts \cup {TimeoutSign(request.node, request.vote)}
      BY <1>1 DEF PersistTimeout
    <2> QED BY <1>1, <2>1, <2>2, Isa
         DEF TimeoutSigningProvenanceInvariant, TimeoutSign
  <1> QED BY <1>1

THEOREM CompleteTimeoutSignaturePreservesSigningProvenance ==
  \A request \in signTimeouts:
    (TimeoutSigningProvenanceInvariant
      /\ CompleteTimeoutSignature(request))
      => TimeoutSigningProvenanceInvariant'
PROOF
  <1>1. ASSUME NEW request \in signTimeouts,
                TimeoutSigningProvenanceInvariant,
                CompleteTimeoutSignature(request)
         PROVE TimeoutSigningProvenanceInvariant'
    <2>1. signTimeouts' \subseteq signTimeouts
      BY <1>1, Isa DEF CompleteTimeoutSignature
    <2> QED BY <1>1, <2>1,
         SubsetTimeoutSigningPreservesProvenance
  <1> QED BY <1>1

THEOREM CrashPreservesTimeoutSigningProvenance ==
  \A node \in ValidatorIds:
    (TimeoutSigningProvenanceInvariant /\ Crash(node))
      => TimeoutSigningProvenanceInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                TimeoutSigningProvenanceInvariant,
                Crash(node)
         PROVE TimeoutSigningProvenanceInvariant'
    <2>1. signTimeouts' \subseteq signTimeouts
      BY <1>1, Isa DEF Crash
    <2> QED BY <1>1, <2>1,
         SubsetTimeoutSigningPreservesProvenance
  <1> QED BY <1>1

THEOREM ResumeTimeoutPreservesSigningProvenance ==
  \A node \in ValidatorIds, vote \in timeoutIntents:
    (TimeoutSigningProvenanceInvariant /\ ResumeTimeout(node, vote))
      => TimeoutSigningProvenanceInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW vote \in timeoutIntents,
                TimeoutSigningProvenanceInvariant,
                ResumeTimeout(node, vote)
         PROVE TimeoutSigningProvenanceInvariant'
    <2>1. /\ vote.signer = node
           /\ signTimeouts' = signTimeouts \cup {TimeoutSign(node, vote)}
      BY <1>1 DEF ResumeTimeout
    <2> QED BY <1>1, <2>1, Isa
         DEF TimeoutSigningProvenanceInvariant, TimeoutSign
  <1> QED BY <1>1

THEOREM NextEitherMutatesTimeoutSigningOrLeavesIt ==
  Next => TimeoutSigningMutationStep \/ UNCHANGED signTimeouts
BY IsaT(120)
   DEF Next, TimeoutSigningMutationStep,
       SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, RejectBody, ValidateDecidedBody, ValidateLockedBody,
       BeginPrepare, PersistPrepare, CompleteVoteSignature,
       ByzantineBroadcastVote, DeliverVote, FormPrepareQC, DeliverQC,
       BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout,
       ByzantineBroadcastTimeout, DeliverTimeout, FormTC, DeliverTC,
       BeginInstallTC, PersistInstallTC, FetchCertifiedBody,
       ApplyDecision, Restart, ResumeProposal, ResumeVote, DropProposal

THEOREM TimeoutSigningMutationPreservesProvenance ==
  (/\ StrongInductiveInvariant
   /\ TimeoutSigningProvenanceInvariant
   /\ TimeoutSigningMutationStep)
    => TimeoutSigningProvenanceInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
                TimeoutSigningProvenanceInvariant,
                TimeoutSigningMutationStep
         PROVE TimeoutSigningProvenanceInvariant'
    <2>1. CASE \E request \in pendingTimeout: PersistTimeout(request)
      BY <1>1, <2>1, PersistTimeoutPreservesSigningProvenance
    <2>2. CASE \E request \in signTimeouts:
                   CompleteTimeoutSignature(request)
      BY <1>1, <2>2,
         CompleteTimeoutSignaturePreservesSigningProvenance
    <2>3. CASE \E node \in ValidatorIds: Crash(node)
      BY <1>1, <2>3, CrashPreservesTimeoutSigningProvenance
    <2>4. CASE \E node \in ValidatorIds, vote \in timeoutIntents:
                   ResumeTimeout(node, vote)
      BY <1>1, <2>4, ResumeTimeoutPreservesSigningProvenance
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF TimeoutSigningMutationStep
  <1> QED BY <1>1

THEOREM NextPreservesTimeoutSigningProvenance ==
  (/\ StrongInductiveInvariant
   /\ TimeoutSigningProvenanceInvariant
   /\ Next)
    => TimeoutSigningProvenanceInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
                TimeoutSigningProvenanceInvariant,
                Next
         PROVE TimeoutSigningProvenanceInvariant'
    <2>1. TimeoutSigningMutationStep \/ UNCHANGED signTimeouts
      BY <1>1, NextEitherMutatesTimeoutSigningOrLeavesIt
    <2>2. CASE TimeoutSigningMutationStep
      BY <1>1, <2>2, TimeoutSigningMutationPreservesProvenance
    <2>3. CASE UNCHANGED signTimeouts
      BY <1>1, <2>3, UnchangedTimeoutSigningPreservesProvenance
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM InitAtEstablishesTimeoutSigningProvenance ==
  \A initialContext:
    InitAt(initialContext) => TimeoutSigningProvenanceInvariant
BY DEF InitAt, TimeoutSigningProvenanceInvariant

THEOREM InitAtEstablishesStrongTimeoutDurabilityInvariant ==
  \A initialContext:
    InitAt(initialContext) => StrongTimeoutDurabilityInvariant
BY InitAtEstablishesStrongInductiveInvariant,
   InitAtEstablishesTimeoutSigningProvenance
   DEF StrongTimeoutDurabilityInvariant

THEOREM NextPreservesStrongTimeoutDurabilityInvariant ==
  (StrongTimeoutDurabilityInvariant /\ Next)
    => StrongTimeoutDurabilityInvariant'
BY NextPreservesStrongInductiveInvariant,
   NextPreservesTimeoutSigningProvenance
   DEF StrongTimeoutDurabilityInvariant

THEOREM StutterPreservesStrongTimeoutDurabilityInvariant ==
  (StrongTimeoutDurabilityInvariant /\ UNCHANGED vars)
    => StrongTimeoutDurabilityInvariant'
PROOF
  <1>1. ASSUME StrongTimeoutDurabilityInvariant,
                UNCHANGED vars
         PROVE StrongTimeoutDurabilityInvariant'
    <2>1. StrongInductiveInvariant
      BY <1>1 DEF StrongTimeoutDurabilityInvariant
    <2>2. [Next]_vars
      BY <1>1
    <2>3. StrongInductiveInvariant'
      BY <2>1, <2>2, CoreStrongInductiveActionPreservation
    <2>4. TimeoutSigningProvenanceInvariant
      BY <1>1 DEF StrongTimeoutDurabilityInvariant
    <2>5. TimeoutSigningProvenanceInvariant'
      BY <1>1, <2>4, UnchangedTimeoutSigningPreservesProvenance
         DEF vars
    <2> QED BY <2>3, <2>5 DEF StrongTimeoutDurabilityInvariant
  <1> QED BY <1>1

THEOREM CoreActionPreservesStrongTimeoutDurabilityInvariant ==
  (StrongTimeoutDurabilityInvariant /\ [Next]_vars)
    => StrongTimeoutDurabilityInvariant'
BY NextPreservesStrongTimeoutDurabilityInvariant,
   StutterPreservesStrongTimeoutDurabilityInvariant
   DEF StrongTimeoutDurabilityInvariant

=============================================================================
