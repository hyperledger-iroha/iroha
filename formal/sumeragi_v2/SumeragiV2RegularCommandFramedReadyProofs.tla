---- MODULE SumeragiV2RegularCommandFramedReadyProofs ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

FramedPersistProposal(request) ==
  /\ PersistProposal(request)
  /\ UNCHANGED AsyncAuxVars

THEOREM PersistProposalReadyIffFramedEnabled ==
  \A request:
    PersistProposalReady(request) <=> ENABLED FramedPersistProposal(request)
BY ExpandENABLED, IsaT(300)
   DEF PersistProposalReady, FramedPersistProposal,
       PersistProposal, AsyncAuxVars, vars

FramedFetchBody(node, proposal) ==
  /\ FetchBody(node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM FetchBodyReadyIffFramedEnabled ==
  \A node, proposal:
    FetchBodyReady(node, proposal) <=> ENABLED FramedFetchBody(node, proposal)
BY ExpandENABLED, IsaT(300)
   DEF FetchBodyReady, FramedFetchBody,
       FetchBody, AsyncAuxVars, vars

RegularNodeProposalWitness(node, proposal) ==
  [node |-> node, proposal |-> proposal]

FramedFetchBodyWitness(witness) ==
  /\ FetchBody(witness.node, witness.proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM FetchBodyReadyIffBundledFramedEnabled ==
  \A witness:
    FetchBodyReady(witness.node, witness.proposal)
      <=> ENABLED FramedFetchBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF FetchBodyReady, FramedFetchBodyWitness,
       FetchBody, AsyncAuxVars, vars

FramedRebindRetainedBody(node, proposal) ==
  /\ RebindRetainedBody(node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RebindRetainedBodyReadyIffFramedEnabled ==
  \A node, proposal:
    RebindRetainedBodyReady(node, proposal)
      <=> ENABLED FramedRebindRetainedBody(node, proposal)
BY ExpandENABLED, IsaT(300)
   DEF RebindRetainedBodyReady, FramedRebindRetainedBody,
       RebindRetainedBody, AsyncAuxVars, vars

FramedRebindRetainedBodyWitness(witness) ==
  /\ RebindRetainedBody(witness.node, witness.proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RebindRetainedBodyReadyIffBundledFramedEnabled ==
  \A witness:
    RebindRetainedBodyReady(witness.node, witness.proposal)
      <=> ENABLED FramedRebindRetainedBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF RebindRetainedBodyReady, FramedRebindRetainedBodyWitness,
       RebindRetainedBody, AsyncAuxVars, vars

FramedValidateBody(node, proposal) ==
  /\ ValidateBody(node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM ValidateBodyReadyIffFramedEnabled ==
  \A node, proposal:
    ValidateBodyReady(node, proposal)
      <=> ENABLED FramedValidateBody(node, proposal)
BY ExpandENABLED, IsaT(300)
   DEF ValidateBodyReady, FramedValidateBody,
       ValidateBody, AsyncAuxVars, vars

FramedValidateBodyWitness(witness) ==
  /\ ValidateBody(witness.node, witness.proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM ValidateBodyReadyIffBundledFramedEnabled ==
  \A witness:
    ValidateBodyReady(witness.node, witness.proposal)
      <=> ENABLED FramedValidateBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF ValidateBodyReady, FramedValidateBodyWitness,
       ValidateBody, AsyncAuxVars, vars

FramedRejectBody(node, proposal) ==
  /\ RejectBody(node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RejectBodyReadyIffFramedEnabled ==
  \A node, proposal:
    RejectBodyReady(node, proposal)
      <=> ENABLED FramedRejectBody(node, proposal)
BY ExpandENABLED, IsaT(300)
   DEF RejectBodyReady, FramedRejectBody,
       RejectBody, AsyncAuxVars, vars

FramedRejectBodyWitness(witness) ==
  /\ RejectBody(witness.node, witness.proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RejectBodyReadyIffBundledFramedEnabled ==
  \A witness:
    RejectBodyReady(witness.node, witness.proposal)
      <=> ENABLED FramedRejectBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF RejectBodyReady, FramedRejectBodyWitness,
       RejectBody, AsyncAuxVars, vars

RegularNodeQcWitness(node, qc) ==
  [node |-> node, qc |-> qc]

FramedValidateDecidedBody(node, qc) ==
  /\ ValidateDecidedBody(node, qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM ValidateDecidedBodyReadyIffFramedEnabled ==
  \A node, qc:
    ValidateDecidedBodyReady(node, qc)
      <=> ENABLED FramedValidateDecidedBody(node, qc)
BY ExpandENABLED, IsaT(300)
   DEF ValidateDecidedBodyReady, FramedValidateDecidedBody,
       ValidateDecidedBody, AsyncAuxVars, vars

FramedValidateDecidedBodyWitness(witness) ==
  /\ ValidateDecidedBody(witness.node, witness.qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM ValidateDecidedBodyReadyIffBundledFramedEnabled ==
  \A witness:
    ValidateDecidedBodyReady(witness.node, witness.qc)
      <=> ENABLED FramedValidateDecidedBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF ValidateDecidedBodyReady, FramedValidateDecidedBodyWitness,
       ValidateDecidedBody, AsyncAuxVars, vars

FramedValidateLockedBody(node, qc) ==
  /\ ValidateLockedBody(node, qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM ValidateLockedBodyReadyIffFramedEnabled ==
  \A node, qc:
    ValidateLockedBodyReady(node, qc)
      <=> ENABLED FramedValidateLockedBody(node, qc)
BY ExpandENABLED, IsaT(300)
   DEF ValidateLockedBodyReady, FramedValidateLockedBody,
       ValidateLockedBody, AsyncAuxVars, vars

FramedValidateLockedBodyWitness(witness) ==
  /\ ValidateLockedBody(witness.node, witness.qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM ValidateLockedBodyReadyIffBundledFramedEnabled ==
  \A witness:
    ValidateLockedBodyReady(witness.node, witness.qc)
      <=> ENABLED FramedValidateLockedBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF ValidateLockedBodyReady, FramedValidateLockedBodyWitness,
       ValidateLockedBody, AsyncAuxVars, vars

FramedBeginPrepare(node, proposal) ==
  /\ BeginPrepare(node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginPrepareReadyIffFramedEnabled ==
  \A node, proposal:
    BeginPrepareReady(node, proposal)
      <=> ENABLED FramedBeginPrepare(node, proposal)
BY ExpandENABLED, IsaT(300)
   DEF BeginPrepareReady, FramedBeginPrepare,
       BeginPrepare, AsyncAuxVars, vars

FramedBeginPrepareWitness(witness) ==
  /\ BeginPrepare(witness.node, witness.proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginPrepareReadyIffBundledFramedEnabled ==
  \A witness:
    BeginPrepareReady(witness.node, witness.proposal)
      <=> ENABLED FramedBeginPrepareWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF BeginPrepareReady, FramedBeginPrepareWitness,
       BeginPrepare, AsyncAuxVars, vars

FramedPersistPrepare(request) ==
  /\ PersistPrepare(request)
  /\ UNCHANGED AsyncAuxVars

THEOREM PersistPrepareReadyIffFramedEnabled ==
  \A request:
    PersistPrepareReady(request) <=> ENABLED FramedPersistPrepare(request)
BY ExpandENABLED, IsaT(300)
   DEF PersistPrepareReady, FramedPersistPrepare,
       PersistPrepare, AsyncAuxVars, vars

FramedBeginObservePrepare(node, qc) ==
  /\ BeginObservePrepare(node, qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginObservePrepareReadyIffFramedEnabled ==
  \A node, qc:
    BeginObservePrepareReady(node, qc)
      <=> ENABLED FramedBeginObservePrepare(node, qc)
BY ExpandENABLED, IsaT(300)
   DEF BeginObservePrepareReady, FramedBeginObservePrepare,
       BeginObservePrepare, AsyncAuxVars, vars

FramedBeginObservePrepareWitness(witness) ==
  /\ BeginObservePrepare(witness.node, witness.qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginObservePrepareReadyIffBundledFramedEnabled ==
  \A witness:
    BeginObservePrepareReady(witness.node, witness.qc)
      <=> ENABLED FramedBeginObservePrepareWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF BeginObservePrepareReady, FramedBeginObservePrepareWitness,
       BeginObservePrepare, AsyncAuxVars, vars

FramedPersistObservePrepare(request) ==
  /\ PersistObservePrepare(request)
  /\ UNCHANGED AsyncAuxVars

THEOREM PersistObservePrepareReadyIffFramedEnabled ==
  \A request:
    PersistObservePrepareReady(request)
      <=> ENABLED FramedPersistObservePrepare(request)
BY ExpandENABLED, IsaT(300)
   DEF PersistObservePrepareReady, FramedPersistObservePrepare,
       PersistObservePrepare, AsyncAuxVars, vars

FramedBeginLockCommit(node, qc) ==
  /\ BeginLockCommit(node, qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginLockCommitReadyIffFramedEnabled ==
  \A node, qc:
    BeginLockCommitReady(node, qc)
      <=> ENABLED FramedBeginLockCommit(node, qc)
BY ExpandENABLED, IsaT(300)
   DEF BeginLockCommitReady, FramedBeginLockCommit,
       BeginLockCommit, AsyncAuxVars, vars

FramedBeginLockCommitWitness(witness) ==
  /\ BeginLockCommit(witness.node, witness.qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginLockCommitReadyIffBundledFramedEnabled ==
  \A witness:
    BeginLockCommitReady(witness.node, witness.qc)
      <=> ENABLED FramedBeginLockCommitWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF BeginLockCommitReady, FramedBeginLockCommitWitness,
       BeginLockCommit, AsyncAuxVars, vars

FramedPersistLockCommit(request) ==
  /\ PersistLockCommit(request)
  /\ UNCHANGED AsyncAuxVars

THEOREM PersistLockCommitReadyIffFramedEnabled ==
  \A request:
    PersistLockCommitReady(request)
      <=> ENABLED FramedPersistLockCommit(request)
BY ExpandENABLED, IsaT(300)
   DEF PersistLockCommitReady, FramedPersistLockCommit,
       PersistLockCommit, AsyncAuxVars, vars

FramedBeginDecision(node, qc) ==
  /\ BeginDecision(node, qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginDecisionReadyIffFramedEnabled ==
  \A node, qc:
    BeginDecisionReady(node, qc)
      <=> ENABLED FramedBeginDecision(node, qc)
BY ExpandENABLED, IsaT(300)
   DEF BeginDecisionReady, FramedBeginDecision,
       BeginDecision, AsyncAuxVars, vars

FramedBeginDecisionWitness(witness) ==
  /\ BeginDecision(witness.node, witness.qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginDecisionReadyIffBundledFramedEnabled ==
  \A witness:
    BeginDecisionReady(witness.node, witness.qc)
      <=> ENABLED FramedBeginDecisionWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF BeginDecisionReady, FramedBeginDecisionWitness,
       BeginDecision, AsyncAuxVars, vars

FramedPersistTimeout(request) ==
  /\ PersistTimeout(request)
  /\ UNCHANGED AsyncAuxVars

THEOREM PersistTimeoutReadyIffFramedEnabled ==
  \A request:
    PersistTimeoutReady(request) <=> ENABLED FramedPersistTimeout(request)
BY ExpandENABLED, IsaT(300)
   DEF PersistTimeoutReady, FramedPersistTimeout,
       PersistTimeout, AsyncAuxVars, vars

FramedBeginInstallTC(node, tc) ==
  /\ BeginInstallTC(node, tc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginInstallTCReadyIffFramedEnabled ==
  \A node, tc:
    BeginInstallTCReady(node, tc)
      <=> ENABLED FramedBeginInstallTC(node, tc)
BY ExpandENABLED, IsaT(300)
   DEF BeginInstallTCReady, FramedBeginInstallTC,
       BeginInstallTC, AsyncAuxVars, vars

RegularNodeTcWitness(node, tc) ==
  [node |-> node, tc |-> tc]

FramedBeginInstallTCWitness(witness) ==
  /\ BeginInstallTC(witness.node, witness.tc)
  /\ UNCHANGED AsyncAuxVars

THEOREM BeginInstallTCReadyIffBundledFramedEnabled ==
  \A witness:
    BeginInstallTCReady(witness.node, witness.tc)
      <=> ENABLED FramedBeginInstallTCWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF BeginInstallTCReady, FramedBeginInstallTCWitness,
       BeginInstallTC, AsyncAuxVars, vars

FramedFetchCertifiedBody(node, qc) ==
  /\ FetchCertifiedBody(node, qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM FetchCertifiedBodyReadyIffFramedEnabled ==
  \A node, qc:
    FetchCertifiedBodyReady(node, qc)
      <=> ENABLED FramedFetchCertifiedBody(node, qc)
BY ExpandENABLED, IsaT(300)
   DEF FetchCertifiedBodyReady, FramedFetchCertifiedBody,
       FetchCertifiedBody, AsyncAuxVars, vars

FramedFetchCertifiedBodyWitness(witness) ==
  /\ FetchCertifiedBody(witness.node, witness.qc)
  /\ UNCHANGED AsyncAuxVars

THEOREM FetchCertifiedBodyReadyIffBundledFramedEnabled ==
  \A witness:
    FetchCertifiedBodyReady(witness.node, witness.qc)
      <=> ENABLED FramedFetchCertifiedBodyWitness(witness)
BY ExpandENABLED, IsaT(300)
   DEF FetchCertifiedBodyReady, FramedFetchCertifiedBodyWitness,
       FetchCertifiedBody, AsyncAuxVars, vars

=============================================================================
