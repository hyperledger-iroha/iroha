---- MODULE SumeragiV2RegularCommandExecutionReadyProofs ----
EXTENDS SumeragiV2RegularCommandFramedReadyProofs, TLAPS

RegularPersistProposalExecute(command) ==
  /\ command.kind = "PersistProposal"
  /\ \E request \in pendingProposal:
       /\ CommandMatches(command, request.node, request.proposal.view,
                         request.proposal.subject)
       /\ PersistProposal(request)
  /\ UNCHANGED AsyncAuxVars

RegularPersistProposalReady(command) ==
  /\ command.kind = "PersistProposal"
  /\ \E request \in pendingProposal:
       /\ CommandMatches(command, request.node, request.proposal.view,
                         request.proposal.subject)
       /\ PersistProposalReady(request)

THEOREM RegularPersistProposalReadyImpliesEnabled ==
  \A command:
    RegularPersistProposalReady(command)
      => ENABLED RegularPersistProposalExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularPersistProposalReady(command)
         PROVE ENABLED RegularPersistProposalExecute(command)
    <2>1. PICK request \in pendingProposal:
             /\ CommandMatches(command, request.node,
                               request.proposal.view,
                               request.proposal.subject)
             /\ PersistProposalReady(request)
      BY <1>1 DEF RegularPersistProposalReady
    <2>2. ENABLED FramedPersistProposal(request)
      BY <2>1, PersistProposalReadyIffFramedEnabled
    <2>3. FramedPersistProposal(request) \in BOOLEAN
      BY Isa DEF FramedPersistProposal
    <2>4. RegularPersistProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistProposalExecute
    <2>5. FramedPersistProposal(request)
             => RegularPersistProposalExecute(command)
      BY <1>1, <2>1, Isa
         DEF FramedPersistProposal, RegularPersistProposalExecute,
             RegularPersistProposalReady
    <2>6. ENABLED FramedPersistProposal(request)
             => ENABLED RegularPersistProposalExecute(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

RegularFetchBodyExecute(command) ==
  /\ command.kind = "FetchBody"
  /\ ~CertifiedRecoveryFetchFrontier(command)
  /\ HeldChunksFor(command.node, command.view, command.subject) =
       AsyncChunks
  /\ ~BodyHeldBy(durableBodies, command.node, context,
                  command.view, command.subject)
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ FetchBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

RegularFetchBodyReady(command) ==
  /\ command.kind = "FetchBody"
  /\ ~CertifiedRecoveryFetchFrontier(command)
  /\ HeldChunksFor(command.node, command.view, command.subject) =
       AsyncChunks
  /\ ~BodyHeldBy(durableBodies, command.node, context,
                  command.view, command.subject)
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ FetchBodyReady(command.node, proposal)

THEOREM RegularFetchBodyReadyImpliesEnabled ==
  \A command:
    RegularFetchBodyReady(command)
      => ENABLED RegularFetchBodyExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularFetchBodyReady(command)
         PROVE ENABLED RegularFetchBodyExecute(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ FetchBodyReady(command.node, proposal)
      BY <1>1 DEF RegularFetchBodyReady
    <2>2. PICK witness:
             /\ witness =
                  RegularNodeProposalWitness(command.node, proposal)
             /\ FetchBodyReady(witness.node, witness.proposal)
      BY <2>1, Isa DEF RegularNodeProposalWitness
    <2>3. ENABLED FramedFetchBodyWitness(witness)
      BY <2>2, FetchBodyReadyIffBundledFramedEnabled
    <2>4. FramedFetchBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedFetchBodyWitness
    <2>5. RegularFetchBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularFetchBodyExecute
    <2>6. FramedFetchBodyWitness(witness)
             => RegularFetchBodyExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedFetchBodyWitness, RegularNodeProposalWitness,
             RegularFetchBodyExecute, RegularFetchBodyReady
    <2>7. ENABLED FramedFetchBodyWitness(witness)
             => ENABLED RegularFetchBodyExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularRebindRetainedBodyExecute(command) ==
  /\ command.kind = "RebindRetainedBody"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ RebindRetainedBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

RegularRebindRetainedBodyReady(command) ==
  /\ command.kind = "RebindRetainedBody"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ RebindRetainedBodyReady(command.node, proposal)

THEOREM RegularRebindRetainedBodyReadyImpliesEnabled ==
  \A command:
    RegularRebindRetainedBodyReady(command)
      => ENABLED RegularRebindRetainedBodyExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularRebindRetainedBodyReady(command)
         PROVE ENABLED RegularRebindRetainedBodyExecute(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBodyReady(command.node, proposal)
      BY <1>1 DEF RegularRebindRetainedBodyReady
    <2>2. PICK witness:
             /\ witness =
                  RegularNodeProposalWitness(command.node, proposal)
             /\ RebindRetainedBodyReady(witness.node,
                                        witness.proposal)
      BY <2>1, Isa DEF RegularNodeProposalWitness
    <2>3. ENABLED FramedRebindRetainedBodyWitness(witness)
      BY <2>2, RebindRetainedBodyReadyIffBundledFramedEnabled
    <2>4. FramedRebindRetainedBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedRebindRetainedBodyWitness
    <2>5. RegularRebindRetainedBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularRebindRetainedBodyExecute
    <2>6. FramedRebindRetainedBodyWitness(witness)
             => RegularRebindRetainedBodyExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedRebindRetainedBodyWitness,
             RegularNodeProposalWitness,
             RegularRebindRetainedBodyExecute,
             RegularRebindRetainedBodyReady
    <2>7. ENABLED FramedRebindRetainedBodyWitness(witness)
             => ENABLED RegularRebindRetainedBodyExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularValidateProposalExecute(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ ValidateBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

RegularValidateProposalReady(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ ValidateBodyReady(command.node, proposal)

THEOREM RegularValidateProposalReadyImpliesEnabled ==
  \A command:
    RegularValidateProposalReady(command)
      => ENABLED RegularValidateProposalExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularValidateProposalReady(command)
         PROVE ENABLED RegularValidateProposalExecute(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ValidateBodyReady(command.node, proposal)
      BY <1>1 DEF RegularValidateProposalReady
    <2>2. PICK witness:
             /\ witness =
                  RegularNodeProposalWitness(command.node, proposal)
             /\ ValidateBodyReady(witness.node, witness.proposal)
      BY <2>1, Isa DEF RegularNodeProposalWitness
    <2>3. ENABLED FramedValidateBodyWitness(witness)
      BY <2>2, ValidateBodyReadyIffBundledFramedEnabled
    <2>4. FramedValidateBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedValidateBodyWitness
    <2>5. RegularValidateProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularValidateProposalExecute
    <2>6. FramedValidateBodyWitness(witness)
             => RegularValidateProposalExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedValidateBodyWitness, RegularNodeProposalWitness,
             RegularValidateProposalExecute,
             RegularValidateProposalReady
    <2>7. ENABLED FramedValidateBodyWitness(witness)
             => ENABLED RegularValidateProposalExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularRejectProposalExecute(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ RejectBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

RegularRejectProposalReady(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ RejectBodyReady(command.node, proposal)

THEOREM RegularRejectProposalReadyImpliesEnabled ==
  \A command:
    RegularRejectProposalReady(command)
      => ENABLED RegularRejectProposalExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularRejectProposalReady(command)
         PROVE ENABLED RegularRejectProposalExecute(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RejectBodyReady(command.node, proposal)
      BY <1>1 DEF RegularRejectProposalReady
    <2>2. PICK witness:
             /\ witness =
                  RegularNodeProposalWitness(command.node, proposal)
             /\ RejectBodyReady(witness.node, witness.proposal)
      BY <2>1, Isa DEF RegularNodeProposalWitness
    <2>3. ENABLED FramedRejectBodyWitness(witness)
      BY <2>2, RejectBodyReadyIffBundledFramedEnabled
    <2>4. FramedRejectBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedRejectBodyWitness
    <2>5. RegularRejectProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularRejectProposalExecute
    <2>6. FramedRejectBodyWitness(witness)
             => RegularRejectProposalExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedRejectBodyWitness, RegularNodeProposalWitness,
             RegularRejectProposalExecute,
             RegularRejectProposalReady
    <2>7. ENABLED FramedRejectBodyWitness(witness)
             => ENABLED RegularRejectProposalExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularValidateDecisionExecute(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E qc \in DecisionQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ValidateDecidedBody(command.node, qc)
  /\ UNCHANGED AsyncAuxVars

RegularValidateDecisionReady(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E qc \in DecisionQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ValidateDecidedBodyReady(command.node, qc)

THEOREM RegularValidateDecisionReadyImpliesEnabled ==
  \A command:
    RegularValidateDecisionReady(command)
      => ENABLED RegularValidateDecisionExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularValidateDecisionReady(command)
         PROVE ENABLED RegularValidateDecisionExecute(command)
    <2>1. PICK qc \in DecisionQcValues:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ ValidateDecidedBodyReady(command.node, qc)
      BY <1>1 DEF RegularValidateDecisionReady
    <2>2. PICK witness:
             /\ witness = RegularNodeQcWitness(command.node, qc)
             /\ ValidateDecidedBodyReady(witness.node, witness.qc)
      BY <2>1, Isa DEF RegularNodeQcWitness
    <2>3. ENABLED FramedValidateDecidedBodyWitness(witness)
      BY <2>2, ValidateDecidedBodyReadyIffBundledFramedEnabled
    <2>4. FramedValidateDecidedBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedValidateDecidedBodyWitness
    <2>5. RegularValidateDecisionExecute(command) \in BOOLEAN
      BY Isa DEF RegularValidateDecisionExecute
    <2>6. FramedValidateDecidedBodyWitness(witness)
             => RegularValidateDecisionExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedValidateDecidedBodyWitness, RegularNodeQcWitness,
             RegularValidateDecisionExecute,
             RegularValidateDecisionReady
    <2>7. ENABLED FramedValidateDecidedBodyWitness(witness)
             => ENABLED RegularValidateDecisionExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularValidateLockExecute(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E qc \in prepareQCs:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ValidateLockedBody(command.node, qc)
  /\ UNCHANGED AsyncAuxVars

RegularValidateLockReady(command) ==
  /\ command.kind = "ValidateBody"
  /\ \E qc \in prepareQCs:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ ValidateLockedBodyReady(command.node, qc)

THEOREM RegularValidateLockReadyImpliesEnabled ==
  \A command:
    RegularValidateLockReady(command)
      => ENABLED RegularValidateLockExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularValidateLockReady(command)
         PROVE ENABLED RegularValidateLockExecute(command)
    <2>1. PICK qc \in prepareQCs:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ ValidateLockedBodyReady(command.node, qc)
      BY <1>1 DEF RegularValidateLockReady
    <2>2. PICK witness:
             /\ witness = RegularNodeQcWitness(command.node, qc)
             /\ ValidateLockedBodyReady(witness.node, witness.qc)
      BY <2>1, Isa DEF RegularNodeQcWitness
    <2>3. ENABLED FramedValidateLockedBodyWitness(witness)
      BY <2>2, ValidateLockedBodyReadyIffBundledFramedEnabled
    <2>4. FramedValidateLockedBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedValidateLockedBodyWitness
    <2>5. RegularValidateLockExecute(command) \in BOOLEAN
      BY Isa DEF RegularValidateLockExecute
    <2>6. FramedValidateLockedBodyWitness(witness)
             => RegularValidateLockExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedValidateLockedBodyWitness, RegularNodeQcWitness,
             RegularValidateLockExecute,
             RegularValidateLockReady
    <2>7. ENABLED FramedValidateLockedBodyWitness(witness)
             => ENABLED RegularValidateLockExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularBeginPrepareExecute(command) ==
  /\ command.kind = "BeginPrepare"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ BeginPrepare(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

RegularBeginPrepareReady(command) ==
  /\ command.kind = "BeginPrepare"
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ BeginPrepareReady(command.node, proposal)

THEOREM RegularBeginPrepareReadyImpliesEnabled ==
  \A command:
    RegularBeginPrepareReady(command)
      => ENABLED RegularBeginPrepareExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularBeginPrepareReady(command)
         PROVE ENABLED RegularBeginPrepareExecute(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ BeginPrepareReady(command.node, proposal)
      BY <1>1 DEF RegularBeginPrepareReady
    <2>2. PICK witness:
             /\ witness =
                  RegularNodeProposalWitness(command.node, proposal)
             /\ BeginPrepareReady(witness.node, witness.proposal)
      BY <2>1, Isa DEF RegularNodeProposalWitness
    <2>3. ENABLED FramedBeginPrepareWitness(witness)
      BY <2>2, BeginPrepareReadyIffBundledFramedEnabled
    <2>4. FramedBeginPrepareWitness(witness) \in BOOLEAN
      BY Isa DEF FramedBeginPrepareWitness
    <2>5. RegularBeginPrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginPrepareExecute
    <2>6. FramedBeginPrepareWitness(witness)
             => RegularBeginPrepareExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedBeginPrepareWitness, RegularNodeProposalWitness,
             RegularBeginPrepareExecute,
             RegularBeginPrepareReady
    <2>7. ENABLED FramedBeginPrepareWitness(witness)
             => ENABLED RegularBeginPrepareExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularPersistPrepareExecute(command) ==
  /\ command.kind = "PersistPrepare"
  /\ \E request \in pendingPrepare:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.subject)
       /\ PersistPrepare(request)
  /\ UNCHANGED AsyncAuxVars

RegularPersistPrepareReady(command) ==
  /\ command.kind = "PersistPrepare"
  /\ \E request \in pendingPrepare:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.subject)
       /\ PersistPrepareReady(request)

THEOREM RegularPersistPrepareReadyImpliesEnabled ==
  \A command:
    RegularPersistPrepareReady(command)
      => ENABLED RegularPersistPrepareExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularPersistPrepareReady(command)
         PROVE ENABLED RegularPersistPrepareExecute(command)
    <2>1. PICK request \in pendingPrepare:
             /\ CommandMatches(command, request.node, request.vote.view,
                               request.vote.subject)
             /\ PersistPrepareReady(request)
      BY <1>1 DEF RegularPersistPrepareReady
    <2>2. ENABLED FramedPersistPrepare(request)
      BY <2>1, PersistPrepareReadyIffFramedEnabled
    <2>3. FramedPersistPrepare(request) \in BOOLEAN
      BY Isa DEF FramedPersistPrepare
    <2>4. RegularPersistPrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistPrepareExecute
    <2>5. FramedPersistPrepare(request)
             => RegularPersistPrepareExecute(command)
      BY <1>1, <2>1, Isa
         DEF FramedPersistPrepare, RegularPersistPrepareExecute,
             RegularPersistPrepareReady
    <2>6. ENABLED FramedPersistPrepare(request)
             => ENABLED RegularPersistPrepareExecute(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

RegularBeginObservePrepareExecute(command) ==
  /\ command.kind = "BeginObservePrepare"
  /\ \E qc \in ReceivedQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ BeginObservePrepare(command.node, qc)
  /\ UNCHANGED AsyncAuxVars

RegularBeginObservePrepareReady(command) ==
  /\ command.kind = "BeginObservePrepare"
  /\ \E qc \in ReceivedQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ BeginObservePrepareReady(command.node, qc)

THEOREM RegularBeginObservePrepareReadyImpliesEnabled ==
  \A command:
    RegularBeginObservePrepareReady(command)
      => ENABLED RegularBeginObservePrepareExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularBeginObservePrepareReady(command)
         PROVE ENABLED RegularBeginObservePrepareExecute(command)
    <2>1. PICK qc \in ReceivedQcValues:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ BeginObservePrepareReady(command.node, qc)
      BY <1>1 DEF RegularBeginObservePrepareReady
    <2>2. PICK witness:
             /\ witness = RegularNodeQcWitness(command.node, qc)
             /\ BeginObservePrepareReady(witness.node, witness.qc)
      BY <2>1, Isa DEF RegularNodeQcWitness
    <2>3. ENABLED FramedBeginObservePrepareWitness(witness)
      BY <2>2, BeginObservePrepareReadyIffBundledFramedEnabled
    <2>4. FramedBeginObservePrepareWitness(witness) \in BOOLEAN
      BY Isa DEF FramedBeginObservePrepareWitness
    <2>5. RegularBeginObservePrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginObservePrepareExecute
    <2>6. FramedBeginObservePrepareWitness(witness)
             => RegularBeginObservePrepareExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedBeginObservePrepareWitness, RegularNodeQcWitness,
             RegularBeginObservePrepareExecute,
             RegularBeginObservePrepareReady
    <2>7. ENABLED FramedBeginObservePrepareWitness(witness)
             => ENABLED RegularBeginObservePrepareExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularPersistObservePrepareExecute(command) ==
  /\ command.kind = "PersistObservePrepare"
  /\ \E request \in pendingObservePrepare:
       /\ CommandMatches(command, request.node, request.qc.view,
                         request.qc.subject)
       /\ PersistObservePrepare(request)
  /\ UNCHANGED AsyncAuxVars

RegularPersistObservePrepareReady(command) ==
  /\ command.kind = "PersistObservePrepare"
  /\ \E request \in pendingObservePrepare:
       /\ CommandMatches(command, request.node, request.qc.view,
                         request.qc.subject)
       /\ PersistObservePrepareReady(request)

THEOREM RegularPersistObservePrepareReadyImpliesEnabled ==
  \A command:
    RegularPersistObservePrepareReady(command)
      => ENABLED RegularPersistObservePrepareExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularPersistObservePrepareReady(command)
         PROVE ENABLED RegularPersistObservePrepareExecute(command)
    <2>1. PICK request \in pendingObservePrepare:
             /\ CommandMatches(command, request.node, request.qc.view,
                               request.qc.subject)
             /\ PersistObservePrepareReady(request)
      BY <1>1 DEF RegularPersistObservePrepareReady
    <2>2. ENABLED FramedPersistObservePrepare(request)
      BY <2>1, PersistObservePrepareReadyIffFramedEnabled
    <2>3. FramedPersistObservePrepare(request) \in BOOLEAN
      BY Isa DEF FramedPersistObservePrepare
    <2>4. RegularPersistObservePrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistObservePrepareExecute
    <2>5. FramedPersistObservePrepare(request)
             => RegularPersistObservePrepareExecute(command)
      BY <1>1, <2>1, Isa
         DEF FramedPersistObservePrepare,
             RegularPersistObservePrepareExecute,
             RegularPersistObservePrepareReady
    <2>6. ENABLED FramedPersistObservePrepare(request)
             => ENABLED RegularPersistObservePrepareExecute(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

RegularBeginLockCommitExecute(command) ==
  /\ command.kind = "BeginLockCommit"
  /\ \E qc \in LockCommitQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ BeginLockCommit(command.node, qc)
  /\ UNCHANGED AsyncAuxVars

RegularBeginLockCommitReady(command) ==
  /\ command.kind = "BeginLockCommit"
  /\ \E qc \in LockCommitQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ BeginLockCommitReady(command.node, qc)

THEOREM RegularBeginLockCommitReadyImpliesEnabled ==
  \A command:
    RegularBeginLockCommitReady(command)
      => ENABLED RegularBeginLockCommitExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularBeginLockCommitReady(command)
         PROVE ENABLED RegularBeginLockCommitExecute(command)
    <2>1. PICK qc \in LockCommitQcValues:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ BeginLockCommitReady(command.node, qc)
      BY <1>1 DEF RegularBeginLockCommitReady
    <2>2. PICK witness:
             /\ witness = RegularNodeQcWitness(command.node, qc)
             /\ BeginLockCommitReady(witness.node, witness.qc)
      BY <2>1, Isa DEF RegularNodeQcWitness
    <2>3. ENABLED FramedBeginLockCommitWitness(witness)
      BY <2>2, BeginLockCommitReadyIffBundledFramedEnabled
    <2>4. FramedBeginLockCommitWitness(witness) \in BOOLEAN
      BY Isa DEF FramedBeginLockCommitWitness
    <2>5. RegularBeginLockCommitExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginLockCommitExecute
    <2>6. FramedBeginLockCommitWitness(witness)
             => RegularBeginLockCommitExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedBeginLockCommitWitness, RegularNodeQcWitness,
             RegularBeginLockCommitExecute,
             RegularBeginLockCommitReady
    <2>7. ENABLED FramedBeginLockCommitWitness(witness)
             => ENABLED RegularBeginLockCommitExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularPersistLockCommitExecute(command) ==
  /\ command.kind = "PersistLockCommit"
  /\ \E request \in pendingLockCommit:
       /\ CommandMatches(command, request.node, request.qc.view,
                         request.qc.subject)
       /\ PersistLockCommit(request)
  /\ UNCHANGED AsyncAuxVars

RegularPersistLockCommitReady(command) ==
  /\ command.kind = "PersistLockCommit"
  /\ \E request \in pendingLockCommit:
       /\ CommandMatches(command, request.node, request.qc.view,
                         request.qc.subject)
       /\ PersistLockCommitReady(request)

THEOREM RegularPersistLockCommitReadyImpliesEnabled ==
  \A command:
    RegularPersistLockCommitReady(command)
      => ENABLED RegularPersistLockCommitExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularPersistLockCommitReady(command)
         PROVE ENABLED RegularPersistLockCommitExecute(command)
    <2>1. PICK request \in pendingLockCommit:
             /\ CommandMatches(command, request.node, request.qc.view,
                               request.qc.subject)
             /\ PersistLockCommitReady(request)
      BY <1>1 DEF RegularPersistLockCommitReady
    <2>2. ENABLED FramedPersistLockCommit(request)
      BY <2>1, PersistLockCommitReadyIffFramedEnabled
    <2>3. FramedPersistLockCommit(request) \in BOOLEAN
      BY Isa DEF FramedPersistLockCommit
    <2>4. RegularPersistLockCommitExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistLockCommitExecute
    <2>5. FramedPersistLockCommit(request)
             => RegularPersistLockCommitExecute(command)
      BY <1>1, <2>1, Isa
         DEF FramedPersistLockCommit,
             RegularPersistLockCommitExecute,
             RegularPersistLockCommitReady
    <2>6. ENABLED FramedPersistLockCommit(request)
             => ENABLED RegularPersistLockCommitExecute(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

RegularBeginDecisionExecute(command) ==
  /\ command.kind = "BeginDecision"
  /\ \E qc \in ReceivedQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ BeginDecision(command.node, qc)
  /\ UNCHANGED AsyncAuxVars

RegularBeginDecisionReady(command) ==
  /\ command.kind = "BeginDecision"
  /\ \E qc \in ReceivedQcValues:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ BeginDecisionReady(command.node, qc)

THEOREM RegularBeginDecisionReadyImpliesEnabled ==
  \A command:
    RegularBeginDecisionReady(command)
      => ENABLED RegularBeginDecisionExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularBeginDecisionReady(command)
         PROVE ENABLED RegularBeginDecisionExecute(command)
    <2>1. PICK qc \in ReceivedQcValues:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ BeginDecisionReady(command.node, qc)
      BY <1>1 DEF RegularBeginDecisionReady
    <2>2. PICK witness:
             /\ witness = RegularNodeQcWitness(command.node, qc)
             /\ BeginDecisionReady(witness.node, witness.qc)
      BY <2>1, Isa DEF RegularNodeQcWitness
    <2>3. ENABLED FramedBeginDecisionWitness(witness)
      BY <2>2, BeginDecisionReadyIffBundledFramedEnabled
    <2>4. FramedBeginDecisionWitness(witness) \in BOOLEAN
      BY Isa DEF FramedBeginDecisionWitness
    <2>5. RegularBeginDecisionExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginDecisionExecute
    <2>6. FramedBeginDecisionWitness(witness)
             => RegularBeginDecisionExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedBeginDecisionWitness, RegularNodeQcWitness,
             RegularBeginDecisionExecute,
             RegularBeginDecisionReady
    <2>7. ENABLED FramedBeginDecisionWitness(witness)
             => ENABLED RegularBeginDecisionExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularPersistTimeoutExecute(command) ==
  /\ command.kind = "PersistTimeout"
  /\ \E request \in pendingTimeout:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.highSubject)
       /\ PersistTimeout(request)
  /\ UNCHANGED AsyncAuxVars

RegularPersistTimeoutReady(command) ==
  /\ command.kind = "PersistTimeout"
  /\ \E request \in pendingTimeout:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.highSubject)
       /\ PersistTimeoutReady(request)

THEOREM RegularPersistTimeoutReadyImpliesEnabled ==
  \A command:
    RegularPersistTimeoutReady(command)
      => ENABLED RegularPersistTimeoutExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularPersistTimeoutReady(command)
         PROVE ENABLED RegularPersistTimeoutExecute(command)
    <2>1. PICK request \in pendingTimeout:
             /\ CommandMatches(command, request.node, request.vote.view,
                               request.vote.highSubject)
             /\ PersistTimeoutReady(request)
      BY <1>1 DEF RegularPersistTimeoutReady
    <2>2. ENABLED FramedPersistTimeout(request)
      BY <2>1, PersistTimeoutReadyIffFramedEnabled
    <2>3. FramedPersistTimeout(request) \in BOOLEAN
      BY Isa DEF FramedPersistTimeout
    <2>4. RegularPersistTimeoutExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistTimeoutExecute
    <2>5. FramedPersistTimeout(request)
             => RegularPersistTimeoutExecute(command)
      BY <1>1, <2>1, Isa
         DEF FramedPersistTimeout, RegularPersistTimeoutExecute,
             RegularPersistTimeoutReady
    <2>6. ENABLED FramedPersistTimeout(request)
             => ENABLED RegularPersistTimeoutExecute(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

RegularBeginInstallTCExecute(command) ==
  /\ command.kind = "BeginInstallTC"
  /\ \E tc \in ReceivedTcValues:
       /\ command.node = command.node
       /\ command.view = tc.view
       /\ BeginInstallTC(command.node, tc)
  /\ UNCHANGED AsyncAuxVars

RegularBeginInstallTCReady(command) ==
  /\ command.kind = "BeginInstallTC"
  /\ \E tc \in ReceivedTcValues:
       /\ command.node = command.node
       /\ command.view = tc.view
       /\ BeginInstallTCReady(command.node, tc)

THEOREM RegularBeginInstallTCReadyImpliesEnabled ==
  \A command:
    RegularBeginInstallTCReady(command)
      => ENABLED RegularBeginInstallTCExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularBeginInstallTCReady(command)
         PROVE ENABLED RegularBeginInstallTCExecute(command)
    <2>1. PICK tc \in ReceivedTcValues:
             /\ command.node = command.node
             /\ command.view = tc.view
             /\ BeginInstallTCReady(command.node, tc)
      BY <1>1 DEF RegularBeginInstallTCReady
    <2>2. PICK witness:
             /\ witness = RegularNodeTcWitness(command.node, tc)
             /\ BeginInstallTCReady(witness.node, witness.tc)
      BY <2>1, Isa DEF RegularNodeTcWitness
    <2>3. ENABLED FramedBeginInstallTCWitness(witness)
      BY <2>2, BeginInstallTCReadyIffBundledFramedEnabled
    <2>4. FramedBeginInstallTCWitness(witness) \in BOOLEAN
      BY Isa DEF FramedBeginInstallTCWitness
    <2>5. RegularBeginInstallTCExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginInstallTCExecute
    <2>6. FramedBeginInstallTCWitness(witness)
             => RegularBeginInstallTCExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedBeginInstallTCWitness, RegularNodeTcWitness,
             RegularBeginInstallTCExecute,
             RegularBeginInstallTCReady
    <2>7. ENABLED FramedBeginInstallTCWitness(witness)
             => ENABLED RegularBeginInstallTCExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularFetchCertifiedBodyExecute(command) ==
  /\ command.kind = "FetchCertifiedBody"
  /\ command.item.kind = "CertifiedResponse"
  /\ command.item.envelope.recipient = command.node
  /\ command.item.envelope.view = command.view
  /\ command.item.envelope.subject = command.subject
  /\ \E qc \in DecisionQcValues \cup prepareQCs:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ CertifiedBodyRecoveryAuthority(command.node, qc)
       /\ command.item.source \in qc.signers
       /\ FetchCertifiedBody(command.node, qc)
  /\ UNCHANGED AsyncAuxVars

RegularFetchCertifiedBodyReady(command) ==
  /\ command.kind = "FetchCertifiedBody"
  /\ command.item.kind = "CertifiedResponse"
  /\ command.item.envelope.recipient = command.node
  /\ command.item.envelope.view = command.view
  /\ command.item.envelope.subject = command.subject
  /\ \E qc \in DecisionQcValues \cup prepareQCs:
       /\ CommandMatches(command, command.node, qc.view, qc.subject)
       /\ CertifiedBodyRecoveryAuthority(command.node, qc)
       /\ command.item.source \in qc.signers
       /\ FetchCertifiedBodyReady(command.node, qc)

THEOREM RegularFetchCertifiedBodyReadyImpliesEnabled ==
  \A command:
    RegularFetchCertifiedBodyReady(command)
      => ENABLED RegularFetchCertifiedBodyExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularFetchCertifiedBodyReady(command)
         PROVE ENABLED RegularFetchCertifiedBodyExecute(command)
    <2>1. PICK qc \in DecisionQcValues \cup prepareQCs:
             /\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\ CertifiedBodyRecoveryAuthority(command.node, qc)
             /\ command.item.source \in qc.signers
             /\ FetchCertifiedBodyReady(command.node, qc)
      BY <1>1 DEF RegularFetchCertifiedBodyReady
    <2>2. PICK witness:
             /\ witness = RegularNodeQcWitness(command.node, qc)
             /\ FetchCertifiedBodyReady(witness.node, witness.qc)
      BY <2>1, Isa DEF RegularNodeQcWitness
    <2>3. ENABLED FramedFetchCertifiedBodyWitness(witness)
      BY <2>2, FetchCertifiedBodyReadyIffBundledFramedEnabled
    <2>4. FramedFetchCertifiedBodyWitness(witness) \in BOOLEAN
      BY Isa DEF FramedFetchCertifiedBodyWitness
    <2>5. RegularFetchCertifiedBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularFetchCertifiedBodyExecute
    <2>6. FramedFetchCertifiedBodyWitness(witness)
             => RegularFetchCertifiedBodyExecute(command)
      BY <1>1, <2>1, <2>2, Isa
         DEF FramedFetchCertifiedBodyWitness, RegularNodeQcWitness,
             RegularFetchCertifiedBodyExecute,
             RegularFetchCertifiedBodyReady
    <2>7. ENABLED FramedFetchCertifiedBodyWitness(witness)
             => ENABLED RegularFetchCertifiedBodyExecute(command)
      BY <2>4, <2>5, <2>6, ENABLEDaxioms
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

RegularAssembleBodyExecute(command) ==
  /\ command.kind = "AssembleBody"
  /\ CommandMatches(command, command.node, nodeView[command.node],
                    command.subject)
  /\ AssembleLocalBody(command.node, command.subject)
  /\ UNCHANGED AsyncAuxVars

RegularAssembleBodyReady(command) ==
  /\ command.kind = "AssembleBody"
  /\ CommandMatches(command, command.node, nodeView[command.node],
                    command.subject)
  /\ AssembleLocalBodyReady(command.node, command.subject)

THEOREM RegularAssembleBodyReadyIffEnabled ==
  \A command:
    RegularAssembleBodyReady(command)
      <=> ENABLED RegularAssembleBodyExecute(command)
BY ExpandENABLED, IsaT(300)
   DEF RegularAssembleBodyReady, RegularAssembleBodyExecute,
       AssembleLocalBodyReady, AssembleLocalBody, AsyncAuxVars, vars

THEOREM RegularAssembleBodyExecuteImpliesReady ==
  \A command:
    RegularAssembleBodyExecute(command)
      => RegularAssembleBodyReady(command)
BY IsaT(300)
   DEF RegularAssembleBodyExecute, RegularAssembleBodyReady,
       AssembleLocalBodyReady, AssembleLocalBody

RegularBeginProposalExecute(command) ==
  /\ command.kind = "BeginProposal"
  /\ BeginLocalProposal(command.node, command.subject)
  /\ UNCHANGED AsyncAuxVars

RegularBeginProposalReady(command) ==
  /\ command.kind = "BeginProposal"
  /\ BeginLocalProposalReady(command.node, command.subject)

THEOREM RegularBeginProposalReadyIffEnabled ==
  \A command:
    RegularBeginProposalReady(command)
      <=> ENABLED RegularBeginProposalExecute(command)
BY ExpandENABLED, IsaT(300)
   DEF RegularBeginProposalReady, RegularBeginProposalExecute,
       BeginLocalProposalReady, BeginLocalProposal, AsyncAuxVars, vars

THEOREM RegularBeginProposalExecuteImpliesReady ==
  \A command:
    RegularBeginProposalExecute(command)
      => RegularBeginProposalReady(command)
BY Isa
   DEF RegularBeginProposalExecute, RegularBeginProposalReady,
       BeginLocalProposalReady, BeginLocalProposal

RegularStoreBodyExecute(command) ==
  /\ command.kind = "StoreBody"
  /\ StoreBody(command.node, command.view, command.subject)
  /\ UNCHANGED AsyncAuxVars

RegularStoreBodyReady(command) ==
  /\ command.kind = "StoreBody"
  /\ StoreBodyReady(command.node, command.view, command.subject)

THEOREM RegularStoreBodyReadyIffEnabled ==
  \A command:
    RegularStoreBodyReady(command)
      <=> ENABLED RegularStoreBodyExecute(command)
BY ExpandENABLED, IsaT(300)
   DEF RegularStoreBodyReady, RegularStoreBodyExecute,
       StoreBodyReady, StoreBody, AsyncAuxVars, vars

THEOREM RegularStoreBodyExecuteImpliesReady ==
  \A command:
    RegularStoreBodyExecute(command)
      => RegularStoreBodyReady(command)
BY Isa
   DEF RegularStoreBodyExecute, RegularStoreBodyReady,
       StoreBodyReady, StoreBody

RegularFormCommitQCExecute(command) ==
  /\ command.kind = "FormCommitQC"
  /\ FormCommitQC(command.node, command.view, command.subject)
  /\ UNCHANGED AsyncAuxVars

RegularFormCommitQCReady(command) ==
  /\ command.kind = "FormCommitQC"
  /\ FormCommitQCReady(command.node, command.view, command.subject)

THEOREM RegularFormCommitQCReadyIffEnabled ==
  \A command:
    RegularFormCommitQCReady(command)
      <=> ENABLED RegularFormCommitQCExecute(command)
BY ExpandENABLED, IsaT(300)
   DEF RegularFormCommitQCReady, RegularFormCommitQCExecute,
       FormCommitQCReady, FormCommitQC, AsyncAuxVars, vars

THEOREM RegularFormCommitQCExecuteImpliesReady ==
  \A command:
    RegularFormCommitQCExecute(command)
      => RegularFormCommitQCReady(command)
BY Isa
   DEF RegularFormCommitQCExecute, RegularFormCommitQCReady,
       FormCommitQCReady, FormCommitQC

RegularLeafReady(command) ==
  \/ RegularAssembleBodyReady(command)
  \/ RegularBeginProposalReady(command)
  \/ RegularPersistProposalReady(command)
  \/ RegularFetchBodyReady(command)
  \/ RegularRebindRetainedBodyReady(command)
  \/ RegularStoreBodyReady(command)
  \/ RegularValidateProposalReady(command)
  \/ RegularRejectProposalReady(command)
  \/ RegularValidateDecisionReady(command)
  \/ RegularValidateLockReady(command)
  \/ RegularBeginPrepareReady(command)
  \/ RegularPersistPrepareReady(command)
  \/ RegularBeginObservePrepareReady(command)
  \/ RegularPersistObservePrepareReady(command)
  \/ RegularBeginLockCommitReady(command)
  \/ RegularPersistLockCommitReady(command)
  \/ RegularFormCommitQCReady(command)
  \/ RegularBeginDecisionReady(command)
  \/ RegularPersistTimeoutReady(command)
  \/ RegularBeginInstallTCReady(command)
  \/ RegularFetchCertifiedBodyReady(command)

RegularLeafExecute(command) ==
  \/ RegularAssembleBodyExecute(command)
  \/ RegularBeginProposalExecute(command)
  \/ RegularPersistProposalExecute(command)
  \/ RegularFetchBodyExecute(command)
  \/ RegularRebindRetainedBodyExecute(command)
  \/ RegularStoreBodyExecute(command)
  \/ RegularValidateProposalExecute(command)
  \/ RegularRejectProposalExecute(command)
  \/ RegularValidateDecisionExecute(command)
  \/ RegularValidateLockExecute(command)
  \/ RegularBeginPrepareExecute(command)
  \/ RegularPersistPrepareExecute(command)
  \/ RegularBeginObservePrepareExecute(command)
  \/ RegularPersistObservePrepareExecute(command)
  \/ RegularBeginLockCommitExecute(command)
  \/ RegularPersistLockCommitExecute(command)
  \/ RegularFormCommitQCExecute(command)
  \/ RegularBeginDecisionExecute(command)
  \/ RegularPersistTimeoutExecute(command)
  \/ RegularBeginInstallTCExecute(command)
  \/ RegularFetchCertifiedBodyExecute(command)

RegularLocalSourceReady(command) ==
  \/ /\ command.kind = "AssembleBody"
     /\ CommandMatches(command, command.node, nodeView[command.node],
                       command.subject)
     /\ AssembleLocalBodyReady(command.node, command.subject)
  \/ /\ command.kind = "BeginProposal"
     /\ BeginLocalProposalReady(command.node, command.subject)
  \/ /\ command.kind = "PersistProposal"
     /\ \E request \in pendingProposal:
          /\ CommandMatches(command, request.node, request.proposal.view,
                            request.proposal.subject)
          /\ PersistProposalReady(request)

RegularLocalSourceCore(command) ==
  \/ /\ command.kind = "AssembleBody"
     /\ CommandMatches(command, command.node, nodeView[command.node],
                       command.subject)
     /\ AssembleLocalBody(command.node, command.subject)
  \/ /\ command.kind = "BeginProposal"
     /\ BeginLocalProposal(command.node, command.subject)
  \/ /\ command.kind = "PersistProposal"
     /\ \E request \in pendingProposal:
          /\ CommandMatches(command, request.node, request.proposal.view,
                            request.proposal.subject)
          /\ PersistProposal(request)

RegularLocalLeafReady(command) ==
  \/ RegularAssembleBodyReady(command)
  \/ RegularBeginProposalReady(command)
  \/ RegularPersistProposalReady(command)

RegularLocalLeafExecute(command) ==
  \/ RegularAssembleBodyExecute(command)
  \/ RegularBeginProposalExecute(command)
  \/ RegularPersistProposalExecute(command)

RegularBodySourceReady(command) ==
  \/ /\ command.kind = "FetchBody"
     /\ ~CertifiedRecoveryFetchFrontier(command)
     /\ HeldChunksFor(command.node, command.view, command.subject) =
          AsyncChunks
     /\ ~BodyHeldBy(durableBodies, command.node, context,
                     command.view, command.subject)
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ FetchBodyReady(command.node, proposal)
  \/ /\ command.kind = "RebindRetainedBody"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ RebindRetainedBodyReady(command.node, proposal)
  \/ /\ command.kind = "StoreBody"
     /\ StoreBodyReady(command.node, command.view, command.subject)

RegularBodySourceCore(command) ==
  \/ /\ command.kind = "FetchBody"
     /\ ~CertifiedRecoveryFetchFrontier(command)
     /\ HeldChunksFor(command.node, command.view, command.subject) =
          AsyncChunks
     /\ ~BodyHeldBy(durableBodies, command.node, context,
                     command.view, command.subject)
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ FetchBody(command.node, proposal)
  \/ /\ command.kind = "RebindRetainedBody"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ RebindRetainedBody(command.node, proposal)
  \/ /\ command.kind = "StoreBody"
     /\ StoreBody(command.node, command.view, command.subject)

RegularBodyLeafReady(command) ==
  \/ RegularFetchBodyReady(command)
  \/ RegularRebindRetainedBodyReady(command)
  \/ RegularStoreBodyReady(command)

RegularBodyLeafExecute(command) ==
  \/ RegularFetchBodyExecute(command)
  \/ RegularRebindRetainedBodyExecute(command)
  \/ RegularStoreBodyExecute(command)

RegularValidateSourceReady(command) ==
  /\ command.kind = "ValidateBody"
  /\ \/ \E proposal \in SeenProposalValues:
            /\ CommandMatches(command, command.node, proposal.view,
                              proposal.subject)
            /\ (ValidateBodyReady(command.node, proposal)
                  \/ RejectBodyReady(command.node, proposal))
     \/ \E qc \in DecisionQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ ValidateDecidedBodyReady(command.node, qc)
     \/ \E qc \in prepareQCs:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ ValidateLockedBodyReady(command.node, qc)

RegularValidateSourceCore(command) ==
  /\ command.kind = "ValidateBody"
  /\ \/ \E proposal \in SeenProposalValues:
            /\ CommandMatches(command, command.node, proposal.view,
                              proposal.subject)
            /\ (ValidateBody(command.node, proposal)
                  \/ RejectBody(command.node, proposal))
     \/ \E qc \in DecisionQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ ValidateDecidedBody(command.node, qc)
     \/ \E qc \in prepareQCs:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ ValidateLockedBody(command.node, qc)

RegularValidateLeafReady(command) ==
  \/ RegularValidateProposalReady(command)
  \/ RegularRejectProposalReady(command)
  \/ RegularValidateDecisionReady(command)
  \/ RegularValidateLockReady(command)

RegularValidateLeafExecute(command) ==
  \/ RegularValidateProposalExecute(command)
  \/ RegularRejectProposalExecute(command)
  \/ RegularValidateDecisionExecute(command)
  \/ RegularValidateLockExecute(command)

RegularPrepareSourceReady(command) ==
  \/ /\ command.kind = "BeginPrepare"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ BeginPrepareReady(command.node, proposal)
  \/ /\ command.kind = "PersistPrepare"
     /\ \E request \in pendingPrepare:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.subject)
          /\ PersistPrepareReady(request)
  \/ /\ command.kind = "BeginObservePrepare"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginObservePrepareReady(command.node, qc)
  \/ /\ command.kind = "PersistObservePrepare"
     /\ \E request \in pendingObservePrepare:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistObservePrepareReady(request)

RegularPrepareSourceCore(command) ==
  \/ /\ command.kind = "BeginPrepare"
     /\ \E proposal \in SeenProposalValues:
          /\ CommandMatches(command, command.node, proposal.view,
                            proposal.subject)
          /\ BeginPrepare(command.node, proposal)
  \/ /\ command.kind = "PersistPrepare"
     /\ \E request \in pendingPrepare:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.subject)
          /\ PersistPrepare(request)
  \/ /\ command.kind = "BeginObservePrepare"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginObservePrepare(command.node, qc)
  \/ /\ command.kind = "PersistObservePrepare"
     /\ \E request \in pendingObservePrepare:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistObservePrepare(request)

RegularPrepareLeafReady(command) ==
  \/ RegularBeginPrepareReady(command)
  \/ RegularPersistPrepareReady(command)
  \/ RegularBeginObservePrepareReady(command)
  \/ RegularPersistObservePrepareReady(command)

RegularPrepareLeafExecute(command) ==
  \/ RegularBeginPrepareExecute(command)
  \/ RegularPersistPrepareExecute(command)
  \/ RegularBeginObservePrepareExecute(command)
  \/ RegularPersistObservePrepareExecute(command)

RegularCommitSourceReady(command) ==
  \/ /\ command.kind = "BeginLockCommit"
     /\ \E qc \in LockCommitQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginLockCommitReady(command.node, qc)
  \/ /\ command.kind = "PersistLockCommit"
     /\ \E request \in pendingLockCommit:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistLockCommitReady(request)
  \/ /\ command.kind = "FormCommitQC"
     /\ FormCommitQCReady(command.node, command.view, command.subject)
  \/ /\ command.kind = "BeginDecision"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginDecisionReady(command.node, qc)

RegularCommitSourceCore(command) ==
  \/ /\ command.kind = "BeginLockCommit"
     /\ \E qc \in LockCommitQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginLockCommit(command.node, qc)
  \/ /\ command.kind = "PersistLockCommit"
     /\ \E request \in pendingLockCommit:
          /\ CommandMatches(command, request.node, request.qc.view,
                            request.qc.subject)
          /\ PersistLockCommit(request)
  \/ /\ command.kind = "FormCommitQC"
     /\ FormCommitQC(command.node, command.view, command.subject)
  \/ /\ command.kind = "BeginDecision"
     /\ \E qc \in ReceivedQcValues:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ BeginDecision(command.node, qc)

RegularCommitLeafReady(command) ==
  \/ RegularBeginLockCommitReady(command)
  \/ RegularPersistLockCommitReady(command)
  \/ RegularFormCommitQCReady(command)
  \/ RegularBeginDecisionReady(command)

RegularCommitLeafExecute(command) ==
  \/ RegularBeginLockCommitExecute(command)
  \/ RegularPersistLockCommitExecute(command)
  \/ RegularFormCommitQCExecute(command)
  \/ RegularBeginDecisionExecute(command)

RegularTimeoutSourceReady(command) ==
  \/ /\ command.kind = "PersistTimeout"
     /\ \E request \in pendingTimeout:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.highSubject)
          /\ PersistTimeoutReady(request)
  \/ /\ command.kind = "BeginInstallTC"
     /\ \E tc \in ReceivedTcValues:
          /\ command.node = command.node
          /\ command.view = tc.view
          /\ BeginInstallTCReady(command.node, tc)
  \/ /\ command.kind = "FetchCertifiedBody"
     /\ command.item.kind = "CertifiedResponse"
     /\ command.item.envelope.recipient = command.node
     /\ command.item.envelope.view = command.view
     /\ command.item.envelope.subject = command.subject
     /\ \E qc \in DecisionQcValues \cup prepareQCs:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ CertifiedBodyRecoveryAuthority(command.node, qc)
          /\ command.item.source \in qc.signers
          /\ FetchCertifiedBodyReady(command.node, qc)

RegularTimeoutSourceCore(command) ==
  \/ /\ command.kind = "PersistTimeout"
     /\ \E request \in pendingTimeout:
          /\ CommandMatches(command, request.node, request.vote.view,
                            request.vote.highSubject)
          /\ PersistTimeout(request)
  \/ /\ command.kind = "BeginInstallTC"
     /\ \E tc \in ReceivedTcValues:
          /\ command.node = command.node
          /\ command.view = tc.view
          /\ BeginInstallTC(command.node, tc)
  \/ /\ command.kind = "FetchCertifiedBody"
     /\ command.item.kind = "CertifiedResponse"
     /\ command.item.envelope.recipient = command.node
     /\ command.item.envelope.view = command.view
     /\ command.item.envelope.subject = command.subject
     /\ \E qc \in DecisionQcValues \cup prepareQCs:
          /\ CommandMatches(command, command.node, qc.view, qc.subject)
          /\ CertifiedBodyRecoveryAuthority(command.node, qc)
          /\ command.item.source \in qc.signers
          /\ FetchCertifiedBody(command.node, qc)

RegularTimeoutLeafReady(command) ==
  \/ RegularPersistTimeoutReady(command)
  \/ RegularBeginInstallTCReady(command)
  \/ RegularFetchCertifiedBodyReady(command)

RegularTimeoutLeafExecute(command) ==
  \/ RegularPersistTimeoutExecute(command)
  \/ RegularBeginInstallTCExecute(command)
  \/ RegularFetchCertifiedBodyExecute(command)

RegularGroupedSourceReady(command) ==
  \/ RegularLocalSourceReady(command)
  \/ RegularBodySourceReady(command)
  \/ RegularValidateSourceReady(command)
  \/ RegularPrepareSourceReady(command)
  \/ RegularCommitSourceReady(command)
  \/ RegularTimeoutSourceReady(command)

RegularGroupedSourceCore(command) ==
  \/ RegularLocalSourceCore(command)
  \/ RegularBodySourceCore(command)
  \/ RegularValidateSourceCore(command)
  \/ RegularPrepareSourceCore(command)
  \/ RegularCommitSourceCore(command)
  \/ RegularTimeoutSourceCore(command)

RegularLocalSourceExecute(command) ==
  /\ RegularLocalSourceCore(command)
  /\ UNCHANGED AsyncAuxVars

RegularBodySourceExecute(command) ==
  /\ RegularBodySourceCore(command)
  /\ UNCHANGED AsyncAuxVars

RegularValidateSourceExecute(command) ==
  /\ RegularValidateSourceCore(command)
  /\ UNCHANGED AsyncAuxVars

RegularPrepareSourceExecute(command) ==
  /\ RegularPrepareSourceCore(command)
  /\ UNCHANGED AsyncAuxVars

RegularCommitSourceExecute(command) ==
  /\ RegularCommitSourceCore(command)
  /\ UNCHANGED AsyncAuxVars

RegularTimeoutSourceExecute(command) ==
  /\ RegularTimeoutSourceCore(command)
  /\ UNCHANGED AsyncAuxVars

RegularGroupedSourceExecute(command) ==
  \/ RegularLocalSourceExecute(command)
  \/ RegularBodySourceExecute(command)
  \/ RegularValidateSourceExecute(command)
  \/ RegularPrepareSourceExecute(command)
  \/ RegularCommitSourceExecute(command)
  \/ RegularTimeoutSourceExecute(command)

RegularGroupedLeafReady(command) ==
  \/ RegularLocalLeafReady(command)
  \/ RegularBodyLeafReady(command)
  \/ RegularValidateLeafReady(command)
  \/ RegularPrepareLeafReady(command)
  \/ RegularCommitLeafReady(command)
  \/ RegularTimeoutLeafReady(command)

RegularGroupedLeafExecute(command) ==
  \/ RegularLocalLeafExecute(command)
  \/ RegularBodyLeafExecute(command)
  \/ RegularValidateLeafExecute(command)
  \/ RegularPrepareLeafExecute(command)
  \/ RegularCommitLeafExecute(command)
  \/ RegularTimeoutLeafExecute(command)

THEOREM RegularPersistProposalExecuteImpliesReady ==
  \A command:
    RegularPersistProposalExecute(command)
      => RegularPersistProposalReady(command)
BY Isa
   DEF RegularPersistProposalExecute, RegularPersistProposalReady,
       PersistProposalReady, PersistProposal

THEOREM RegularFetchBodyExecuteImpliesReady ==
  \A command:
    RegularFetchBodyExecute(command)
      => RegularFetchBodyReady(command)
BY Isa
   DEF RegularFetchBodyExecute, RegularFetchBodyReady,
       FetchBodyReady, FetchBody

THEOREM RegularRebindRetainedBodyExecuteImpliesReady ==
  \A command:
    RegularRebindRetainedBodyExecute(command)
      => RegularRebindRetainedBodyReady(command)
BY Isa
   DEF RegularRebindRetainedBodyExecute,
       RegularRebindRetainedBodyReady,
       RebindRetainedBodyReady, RebindRetainedBody

THEOREM RegularValidateProposalExecuteImpliesReady ==
  \A command:
    RegularValidateProposalExecute(command)
      => RegularValidateProposalReady(command)
BY Isa
   DEF RegularValidateProposalExecute, RegularValidateProposalReady,
       ValidateBodyReady, ValidateBody

THEOREM RegularRejectProposalExecuteImpliesReady ==
  \A command:
    RegularRejectProposalExecute(command)
      => RegularRejectProposalReady(command)
BY Isa
   DEF RegularRejectProposalExecute, RegularRejectProposalReady,
       RejectBodyReady, RejectBody

THEOREM RegularValidateDecisionExecuteImpliesReady ==
  \A command:
    RegularValidateDecisionExecute(command)
      => RegularValidateDecisionReady(command)
BY Isa
   DEF RegularValidateDecisionExecute, RegularValidateDecisionReady,
       ValidateDecidedBodyReady, ValidateDecidedBody

THEOREM RegularValidateLockExecuteImpliesReady ==
  \A command:
    RegularValidateLockExecute(command)
      => RegularValidateLockReady(command)
BY Isa
   DEF RegularValidateLockExecute, RegularValidateLockReady,
       ValidateLockedBodyReady, ValidateLockedBody

THEOREM RegularBeginPrepareExecuteImpliesReady ==
  \A command:
    RegularBeginPrepareExecute(command)
      => RegularBeginPrepareReady(command)
BY Isa
   DEF RegularBeginPrepareExecute, RegularBeginPrepareReady,
       BeginPrepareReady, BeginPrepare

THEOREM RegularPersistPrepareExecuteImpliesReady ==
  \A command:
    RegularPersistPrepareExecute(command)
      => RegularPersistPrepareReady(command)
BY Isa
   DEF RegularPersistPrepareExecute, RegularPersistPrepareReady,
       PersistPrepareReady, PersistPrepare

THEOREM RegularBeginObservePrepareExecuteImpliesReady ==
  \A command:
    RegularBeginObservePrepareExecute(command)
      => RegularBeginObservePrepareReady(command)
BY Isa
   DEF RegularBeginObservePrepareExecute,
       RegularBeginObservePrepareReady,
       BeginObservePrepareReady, BeginObservePrepare

THEOREM RegularPersistObservePrepareExecuteImpliesReady ==
  \A command:
    RegularPersistObservePrepareExecute(command)
      => RegularPersistObservePrepareReady(command)
BY Isa
   DEF RegularPersistObservePrepareExecute,
       RegularPersistObservePrepareReady,
       PersistObservePrepareReady, PersistObservePrepare

THEOREM RegularBeginLockCommitExecuteImpliesReady ==
  \A command:
    RegularBeginLockCommitExecute(command)
      => RegularBeginLockCommitReady(command)
BY Isa
   DEF RegularBeginLockCommitExecute, RegularBeginLockCommitReady,
       BeginLockCommitReady, BeginLockCommit

THEOREM RegularPersistLockCommitExecuteImpliesReady ==
  \A command:
    RegularPersistLockCommitExecute(command)
      => RegularPersistLockCommitReady(command)
BY Isa
   DEF RegularPersistLockCommitExecute,
       RegularPersistLockCommitReady,
       PersistLockCommitReady, PersistLockCommit

THEOREM RegularBeginDecisionExecuteImpliesReady ==
  \A command:
    RegularBeginDecisionExecute(command)
      => RegularBeginDecisionReady(command)
BY Isa
   DEF RegularBeginDecisionExecute, RegularBeginDecisionReady,
       BeginDecisionReady, BeginDecision

THEOREM RegularPersistTimeoutExecuteImpliesReady ==
  \A command:
    RegularPersistTimeoutExecute(command)
      => RegularPersistTimeoutReady(command)
BY Isa
   DEF RegularPersistTimeoutExecute, RegularPersistTimeoutReady,
       PersistTimeoutReady, PersistTimeout

THEOREM RegularBeginInstallTCExecuteImpliesReady ==
  \A command:
    RegularBeginInstallTCExecute(command)
      => RegularBeginInstallTCReady(command)
BY Isa
   DEF RegularBeginInstallTCExecute, RegularBeginInstallTCReady,
       BeginInstallTCReady, BeginInstallTC

THEOREM RegularFetchCertifiedBodyExecuteImpliesReady ==
  \A command:
    RegularFetchCertifiedBodyExecute(command)
      => RegularFetchCertifiedBodyReady(command)
BY Isa
   DEF RegularFetchCertifiedBodyExecute,
       RegularFetchCertifiedBodyReady,
       FetchCertifiedBodyReady, FetchCertifiedBody

THEOREM RegularCoreReadyDecomposesIntoLeaves ==
  \A command:
    RegularCoreCommandReady(command) <=> RegularLeafReady(command)
PROOF
  <1>1. ASSUME NEW command
         PROVE RegularCoreCommandReady(command)
                 <=> RegularLeafReady(command)
    <2>1. RegularCoreCommandReady(command)
             <=> RegularGroupedSourceReady(command)
      BY Isa
         DEF RegularCoreCommandReady, RegularGroupedSourceReady,
             RegularLocalSourceReady, RegularBodySourceReady,
             RegularValidateSourceReady, RegularPrepareSourceReady,
             RegularCommitSourceReady, RegularTimeoutSourceReady
    <2>2. RegularLocalSourceReady(command)
             <=> RegularLocalLeafReady(command)
      BY Isa
         DEF RegularLocalSourceReady, RegularLocalLeafReady,
             RegularAssembleBodyReady, RegularBeginProposalReady,
             RegularPersistProposalReady
    <2>3. RegularBodySourceReady(command)
             <=> RegularBodyLeafReady(command)
      BY Isa
         DEF RegularBodySourceReady, RegularBodyLeafReady,
             RegularFetchBodyReady, RegularRebindRetainedBodyReady,
             RegularStoreBodyReady
    <2>4. RegularValidateSourceReady(command)
             <=> RegularValidateLeafReady(command)
      BY Isa
         DEF RegularValidateSourceReady, RegularValidateLeafReady,
             RegularValidateProposalReady,
             RegularRejectProposalReady,
             RegularValidateDecisionReady, RegularValidateLockReady
    <2>5. RegularPrepareSourceReady(command)
             <=> RegularPrepareLeafReady(command)
      BY Isa
         DEF RegularPrepareSourceReady, RegularPrepareLeafReady,
             RegularBeginPrepareReady, RegularPersistPrepareReady,
             RegularBeginObservePrepareReady,
             RegularPersistObservePrepareReady
    <2>6. RegularCommitSourceReady(command)
             <=> RegularCommitLeafReady(command)
      BY Isa
         DEF RegularCommitSourceReady, RegularCommitLeafReady,
             RegularBeginLockCommitReady,
             RegularPersistLockCommitReady,
             RegularFormCommitQCReady, RegularBeginDecisionReady
    <2>7. RegularTimeoutSourceReady(command)
             <=> RegularTimeoutLeafReady(command)
      BY Isa
         DEF RegularTimeoutSourceReady, RegularTimeoutLeafReady,
             RegularPersistTimeoutReady,
             RegularBeginInstallTCReady,
             RegularFetchCertifiedBodyReady
    <2>8. RegularGroupedSourceReady(command)
             <=> RegularGroupedLeafReady(command)
      BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, Isa
         DEF RegularGroupedSourceReady, RegularGroupedLeafReady
    <2>9. RegularGroupedLeafReady(command)
             <=> RegularLeafReady(command)
      BY Isa
         DEF RegularGroupedLeafReady, RegularLocalLeafReady,
             RegularBodyLeafReady, RegularValidateLeafReady,
             RegularPrepareLeafReady, RegularCommitLeafReady,
             RegularTimeoutLeafReady, RegularLeafReady
    <2> QED BY <2>1, <2>8, <2>9
  <1> QED BY <1>1

THEOREM RegularExecutionDecomposesIntoLeaves ==
  \A command:
    ExecuteRegularCommand(command) <=> RegularLeafExecute(command)
PROOF
  <1>1. ASSUME NEW command
         PROVE ExecuteRegularCommand(command)
                 <=> RegularLeafExecute(command)
    <2>1. RegularCoreCommand(command)
             <=> RegularGroupedSourceCore(command)
      BY Isa
         DEF RegularCoreCommand, RegularGroupedSourceCore,
             RegularLocalSourceCore, RegularBodySourceCore,
             RegularValidateSourceCore, RegularPrepareSourceCore,
             RegularCommitSourceCore, RegularTimeoutSourceCore
    <2>2. RegularGroupedSourceExecute(command)
             <=> /\ RegularGroupedSourceCore(command)
                 /\ UNCHANGED AsyncAuxVars
      BY Isa
         DEF RegularGroupedSourceExecute,
             RegularLocalSourceExecute,
             RegularBodySourceExecute,
             RegularValidateSourceExecute,
             RegularPrepareSourceExecute,
             RegularCommitSourceExecute,
             RegularTimeoutSourceExecute,
             RegularGroupedSourceCore
    <2>3. ExecuteRegularCommand(command)
             <=> RegularGroupedSourceExecute(command)
      BY <2>1, <2>2, Isa DEF ExecuteRegularCommand
    <2>4. RegularLocalSourceExecute(command)
             <=> RegularLocalLeafExecute(command)
      BY Isa
         DEF RegularLocalSourceExecute, RegularLocalSourceCore,
             RegularLocalLeafExecute,
             RegularAssembleBodyExecute,
             RegularBeginProposalExecute,
             RegularPersistProposalExecute
    <2>5. RegularBodySourceExecute(command)
             <=> RegularBodyLeafExecute(command)
      BY Isa
         DEF RegularBodySourceExecute, RegularBodySourceCore,
             RegularBodyLeafExecute, RegularFetchBodyExecute,
             RegularRebindRetainedBodyExecute,
             RegularStoreBodyExecute
    <2>6. RegularValidateSourceExecute(command)
             <=> RegularValidateLeafExecute(command)
      BY Isa
         DEF RegularValidateSourceExecute,
             RegularValidateSourceCore,
             RegularValidateLeafExecute,
             RegularValidateProposalExecute,
             RegularRejectProposalExecute,
             RegularValidateDecisionExecute,
             RegularValidateLockExecute
    <2>7. RegularPrepareSourceExecute(command)
             <=> RegularPrepareLeafExecute(command)
      BY Isa
         DEF RegularPrepareSourceExecute, RegularPrepareSourceCore,
             RegularPrepareLeafExecute,
             RegularBeginPrepareExecute,
             RegularPersistPrepareExecute,
             RegularBeginObservePrepareExecute,
             RegularPersistObservePrepareExecute
    <2>8. RegularCommitSourceExecute(command)
             <=> RegularCommitLeafExecute(command)
      BY Isa
         DEF RegularCommitSourceExecute, RegularCommitSourceCore,
             RegularCommitLeafExecute,
             RegularBeginLockCommitExecute,
             RegularPersistLockCommitExecute,
             RegularFormCommitQCExecute,
             RegularBeginDecisionExecute
    <2>9. RegularTimeoutSourceExecute(command)
             <=> RegularTimeoutLeafExecute(command)
      BY Isa
         DEF RegularTimeoutSourceExecute, RegularTimeoutSourceCore,
             RegularTimeoutLeafExecute,
             RegularPersistTimeoutExecute,
             RegularBeginInstallTCExecute,
             RegularFetchCertifiedBodyExecute
    <2>10. RegularGroupedSourceExecute(command)
              <=> RegularGroupedLeafExecute(command)
      BY <2>4, <2>5, <2>6, <2>7, <2>8, <2>9, Isa
         DEF RegularGroupedSourceExecute, RegularGroupedLeafExecute
    <2>11. RegularGroupedLeafExecute(command)
              <=> RegularLeafExecute(command)
      BY Isa
         DEF RegularGroupedLeafExecute, RegularLocalLeafExecute,
             RegularBodyLeafExecute, RegularValidateLeafExecute,
             RegularPrepareLeafExecute, RegularCommitLeafExecute,
             RegularTimeoutLeafExecute, RegularLeafExecute
    <2> QED BY <2>3, <2>10, <2>11
  <1> QED BY <1>1

THEOREM RegularLeafEnabledSelectionsEnableAggregate ==
  \A command:
    /\ (ENABLED RegularAssembleBodyExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularBeginProposalExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularPersistProposalExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularFetchBodyExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularRebindRetainedBodyExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularStoreBodyExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularValidateProposalExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularRejectProposalExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularValidateDecisionExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularValidateLockExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularBeginPrepareExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularPersistPrepareExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularBeginObservePrepareExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularPersistObservePrepareExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularBeginLockCommitExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularPersistLockCommitExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularFormCommitQCExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularBeginDecisionExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularPersistTimeoutExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularBeginInstallTCExecute(command)
          => ENABLED RegularLeafExecute(command))
    /\ (ENABLED RegularFetchCertifiedBodyExecute(command)
          => ENABLED RegularLeafExecute(command))
PROOF
  <1>1. ASSUME NEW command
         PROVE
           /\ (ENABLED RegularAssembleBodyExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginProposalExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistProposalExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularFetchBodyExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularRebindRetainedBodyExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularStoreBodyExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularValidateProposalExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularRejectProposalExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularValidateDecisionExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularValidateLockExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginPrepareExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistPrepareExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginObservePrepareExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistObservePrepareExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginLockCommitExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistLockCommitExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularFormCommitQCExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginDecisionExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistTimeoutExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginInstallTCExecute(command)
                 => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularFetchCertifiedBodyExecute(command)
                 => ENABLED RegularLeafExecute(command))
    <2>1. RegularLeafExecute(command) \in BOOLEAN
      BY Isa DEF RegularLeafExecute
    <2>2. RegularAssembleBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularAssembleBodyExecute
    <2>3. RegularAssembleBodyExecute(command)
             => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>4. ENABLED RegularAssembleBodyExecute(command)
             => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2>5. RegularBeginProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginProposalExecute
    <2>6. RegularBeginProposalExecute(command)
             => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>7. ENABLED RegularBeginProposalExecute(command)
             => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>5, <2>6, ENABLEDaxioms
    <2>8. RegularPersistProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistProposalExecute
    <2>9. RegularPersistProposalExecute(command)
             => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>10. ENABLED RegularPersistProposalExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>8, <2>9, ENABLEDaxioms
    <2>11. RegularFetchBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularFetchBodyExecute
    <2>12. RegularFetchBodyExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>13. ENABLED RegularFetchBodyExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>11, <2>12, ENABLEDaxioms
    <2>14. RegularRebindRetainedBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularRebindRetainedBodyExecute
    <2>15. RegularRebindRetainedBodyExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>16. ENABLED RegularRebindRetainedBodyExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>14, <2>15, ENABLEDaxioms
    <2>17. RegularStoreBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularStoreBodyExecute
    <2>18. RegularStoreBodyExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>19. ENABLED RegularStoreBodyExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>17, <2>18, ENABLEDaxioms
    <2>20. RegularValidateProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularValidateProposalExecute
    <2>21. RegularValidateProposalExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>22. ENABLED RegularValidateProposalExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>20, <2>21, ENABLEDaxioms
    <2>23. RegularRejectProposalExecute(command) \in BOOLEAN
      BY Isa DEF RegularRejectProposalExecute
    <2>24. RegularRejectProposalExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>25. ENABLED RegularRejectProposalExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>23, <2>24, ENABLEDaxioms
    <2>26. RegularValidateDecisionExecute(command) \in BOOLEAN
      BY Isa DEF RegularValidateDecisionExecute
    <2>27. RegularValidateDecisionExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>28. ENABLED RegularValidateDecisionExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>26, <2>27, ENABLEDaxioms
    <2>29. RegularValidateLockExecute(command) \in BOOLEAN
      BY Isa DEF RegularValidateLockExecute
    <2>30. RegularValidateLockExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>31. ENABLED RegularValidateLockExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>29, <2>30, ENABLEDaxioms
    <2>32. RegularBeginPrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginPrepareExecute
    <2>33. RegularBeginPrepareExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>34. ENABLED RegularBeginPrepareExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>32, <2>33, ENABLEDaxioms
    <2>35. RegularPersistPrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistPrepareExecute
    <2>36. RegularPersistPrepareExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>37. ENABLED RegularPersistPrepareExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>35, <2>36, ENABLEDaxioms
    <2>38. RegularBeginObservePrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginObservePrepareExecute
    <2>39. RegularBeginObservePrepareExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>40. ENABLED RegularBeginObservePrepareExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>38, <2>39, ENABLEDaxioms
    <2>41. RegularPersistObservePrepareExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistObservePrepareExecute
    <2>42. RegularPersistObservePrepareExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>43. ENABLED RegularPersistObservePrepareExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>41, <2>42, ENABLEDaxioms
    <2>44. RegularBeginLockCommitExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginLockCommitExecute
    <2>45. RegularBeginLockCommitExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>46. ENABLED RegularBeginLockCommitExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>44, <2>45, ENABLEDaxioms
    <2>47. RegularPersistLockCommitExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistLockCommitExecute
    <2>48. RegularPersistLockCommitExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>49. ENABLED RegularPersistLockCommitExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>47, <2>48, ENABLEDaxioms
    <2>50. RegularFormCommitQCExecute(command) \in BOOLEAN
      BY Isa DEF RegularFormCommitQCExecute
    <2>51. RegularFormCommitQCExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>52. ENABLED RegularFormCommitQCExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>50, <2>51, ENABLEDaxioms
    <2>53. RegularBeginDecisionExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginDecisionExecute
    <2>54. RegularBeginDecisionExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>55. ENABLED RegularBeginDecisionExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>53, <2>54, ENABLEDaxioms
    <2>56. RegularPersistTimeoutExecute(command) \in BOOLEAN
      BY Isa DEF RegularPersistTimeoutExecute
    <2>57. RegularPersistTimeoutExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>58. ENABLED RegularPersistTimeoutExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>56, <2>57, ENABLEDaxioms
    <2>62. RegularBeginInstallTCExecute(command) \in BOOLEAN
      BY Isa DEF RegularBeginInstallTCExecute
    <2>63. RegularBeginInstallTCExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>64. ENABLED RegularBeginInstallTCExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>62, <2>63, ENABLEDaxioms
    <2>65. RegularFetchCertifiedBodyExecute(command) \in BOOLEAN
      BY Isa DEF RegularFetchCertifiedBodyExecute
    <2>66. RegularFetchCertifiedBodyExecute(command)
              => RegularLeafExecute(command)
      BY DEF RegularLeafExecute
    <2>67. ENABLED RegularFetchCertifiedBodyExecute(command)
              => ENABLED RegularLeafExecute(command)
      BY <2>1, <2>65, <2>66, ENABLEDaxioms
    <2> QED
      BY <2>4, <2>7, <2>10, <2>13, <2>16, <2>19,
         <2>22, <2>25, <2>28, <2>31, <2>34, <2>37,
         <2>40, <2>43, <2>46, <2>49, <2>52, <2>55,
         <2>58, <2>64, <2>67
  <1> QED BY <1>1

THEOREM RegularLeafReadyImpliesEnabled ==
  \A command:
    RegularLeafReady(command) => ENABLED RegularLeafExecute(command)
PROOF
  <1>1. ASSUME NEW command, RegularLeafReady(command)
         PROVE ENABLED RegularLeafExecute(command)
    <2>1. /\ (ENABLED RegularAssembleBodyExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginProposalExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistProposalExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularFetchBodyExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularRebindRetainedBodyExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularStoreBodyExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularValidateProposalExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularRejectProposalExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularValidateDecisionExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularValidateLockExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginPrepareExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistPrepareExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginObservePrepareExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistObservePrepareExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginLockCommitExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistLockCommitExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularFormCommitQCExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginDecisionExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularPersistTimeoutExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularBeginInstallTCExecute(command)
                   => ENABLED RegularLeafExecute(command))
           /\ (ENABLED RegularFetchCertifiedBodyExecute(command)
                   => ENABLED RegularLeafExecute(command))
      BY RegularLeafEnabledSelectionsEnableAggregate
    <2> QED
      BY <1>1, <2>1,
         RegularAssembleBodyReadyIffEnabled,
         RegularBeginProposalReadyIffEnabled,
         RegularPersistProposalReadyImpliesEnabled,
         RegularFetchBodyReadyImpliesEnabled,
         RegularRebindRetainedBodyReadyImpliesEnabled,
         RegularStoreBodyReadyIffEnabled,
         RegularValidateProposalReadyImpliesEnabled,
         RegularRejectProposalReadyImpliesEnabled,
         RegularValidateDecisionReadyImpliesEnabled,
         RegularValidateLockReadyImpliesEnabled,
         RegularBeginPrepareReadyImpliesEnabled,
         RegularPersistPrepareReadyImpliesEnabled,
         RegularBeginObservePrepareReadyImpliesEnabled,
         RegularPersistObservePrepareReadyImpliesEnabled,
         RegularBeginLockCommitReadyImpliesEnabled,
         RegularPersistLockCommitReadyImpliesEnabled,
         RegularFormCommitQCReadyIffEnabled,
         RegularBeginDecisionReadyImpliesEnabled,
         RegularPersistTimeoutReadyImpliesEnabled,
         RegularBeginInstallTCReadyImpliesEnabled,
         RegularFetchCertifiedBodyReadyImpliesEnabled,
         Isa DEF RegularLeafReady
  <1> QED BY <1>1

RegularLeafReadyProjection(command) ==
  /\ RegularLeafReady(command)
  /\ [TRUE]_vars

THEOREM RegularLeafExecuteImpliesReadyProjection ==
  \A command:
    RegularLeafExecute(command)
      => RegularLeafReadyProjection(command)
BY RegularAssembleBodyExecuteImpliesReady,
   RegularBeginProposalExecuteImpliesReady,
   RegularPersistProposalExecuteImpliesReady,
   RegularFetchBodyExecuteImpliesReady,
   RegularRebindRetainedBodyExecuteImpliesReady,
   RegularStoreBodyExecuteImpliesReady,
   RegularValidateProposalExecuteImpliesReady,
   RegularRejectProposalExecuteImpliesReady,
   RegularValidateDecisionExecuteImpliesReady,
   RegularValidateLockExecuteImpliesReady,
   RegularBeginPrepareExecuteImpliesReady,
   RegularPersistPrepareExecuteImpliesReady,
   RegularBeginObservePrepareExecuteImpliesReady,
   RegularPersistObservePrepareExecuteImpliesReady,
   RegularBeginLockCommitExecuteImpliesReady,
   RegularPersistLockCommitExecuteImpliesReady,
   RegularFormCommitQCExecuteImpliesReady,
   RegularBeginDecisionExecuteImpliesReady,
   RegularPersistTimeoutExecuteImpliesReady,
   RegularBeginInstallTCExecuteImpliesReady,
   RegularFetchCertifiedBodyExecuteImpliesReady,
   IsaT(300)
   DEF RegularLeafExecute, RegularLeafReady,
       RegularLeafReadyProjection

THEOREM RegularLeafReadyProjectionIffReady ==
  \A command:
    ENABLED RegularLeafReadyProjection(command)
      <=> RegularLeafReady(command)
BY ExpandENABLED, IsaT(300)
   DEF RegularLeafReadyProjection, RegularLeafReady, vars

THEOREM RegularLeafEnabledImpliesReady ==
  \A command:
    ENABLED RegularLeafExecute(command) => RegularLeafReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED RegularLeafExecute(command)
         PROVE RegularLeafReady(command)
    <2>1. RegularLeafExecute(command) \in BOOLEAN
      BY Isa DEF RegularLeafExecute
    <2>2. RegularLeafReadyProjection(command) \in BOOLEAN
      BY Isa DEF RegularLeafReadyProjection
    <2>3. RegularLeafExecute(command)
             => RegularLeafReadyProjection(command)
      BY RegularLeafExecuteImpliesReadyProjection
    <2>4. ENABLED RegularLeafExecute(command)
             => ENABLED RegularLeafReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, RegularLeafReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM RegularLeafReadyIffEnabled ==
  \A command:
    RegularLeafReady(command) <=> ENABLED RegularLeafExecute(command)
BY RegularLeafReadyImpliesEnabled, RegularLeafEnabledImpliesReady

THEOREM ExecuteRegularCommandReadyIffEnabledComposed ==
  \A command:
    ExecuteRegularCommandReady(command)
      <=> ENABLED ExecuteRegularCommand(command)
PROOF
  <1>1. ASSUME NEW command
         PROVE ExecuteRegularCommandReady(command)
                 <=> ENABLED ExecuteRegularCommand(command)
    <2>1. ExecuteRegularCommandReady(command)
             <=> RegularLeafReady(command)
      BY RegularCoreReadyDecomposesIntoLeaves
         DEF ExecuteRegularCommandReady
    <2>2. RegularLeafReady(command)
             <=> ENABLED RegularLeafExecute(command)
      BY RegularLeafReadyIffEnabled
    <2>3. ExecuteRegularCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteRegularCommand
    <2>4. RegularLeafExecute(command) \in BOOLEAN
      BY Isa DEF RegularLeafExecute
    <2>5. ExecuteRegularCommand(command)
             => RegularLeafExecute(command)
      BY RegularExecutionDecomposesIntoLeaves
    <2>6. RegularLeafExecute(command)
             => ExecuteRegularCommand(command)
      BY RegularExecutionDecomposesIntoLeaves
    <2>7. ENABLED ExecuteRegularCommand(command)
             => ENABLED RegularLeafExecute(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2>8. ENABLED RegularLeafExecute(command)
             => ENABLED ExecuteRegularCommand(command)
      BY <2>3, <2>4, <2>6, ENABLEDaxioms
    <2> QED BY <2>1, <2>2, <2>7, <2>8
  <1> QED BY <1>1

THEOREM ExecuteRegularCommandImpliesReady ==
  \A command:
    ExecuteRegularCommand(command)
      => ExecuteRegularCommandReady(command)
BY RegularExecutionDecomposesIntoLeaves,
   RegularLeafExecuteImpliesReadyProjection,
   RegularCoreReadyDecomposesIntoLeaves,
   Isa
   DEF RegularLeafReadyProjection, ExecuteRegularCommandReady

=============================================================================
