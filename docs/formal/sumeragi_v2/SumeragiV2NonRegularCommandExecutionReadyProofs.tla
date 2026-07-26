---- MODULE SumeragiV2NonRegularCommandExecutionReadyProofs ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

SignVoteWitness(command, request) ==
  [command |-> command, request |-> request]

FixedSignVoteReady(witness) ==
  /\ witness.command.kind = "SignVote"
  /\ witness.request \in signVotes
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.vote.view,
                    witness.request.vote.subject)
  /\ CompleteVoteSignatureReady(witness.request)
  /\ VoteOutbox(witness.request) \subseteq
       {item \in AsyncNetworkItems:
          item.kind \in AsyncControlKinds}

FixedSignVoteExecute(witness) ==
  /\ witness.command.kind = "SignVote"
  /\ witness.request \in signVotes
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.vote.view,
                    witness.request.vote.subject)
  /\ CompleteVoteSignature(witness.request)
  /\ PublishControlItems(VoteOutbox(witness.request))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM FixedSignVoteReadyIffEnabled ==
  \A witness:
    FixedSignVoteReady(witness)
      <=> ENABLED FixedSignVoteExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedSignVoteReady, FixedSignVoteExecute,
       CompleteVoteSignatureReady, CompleteVoteSignature,
       PublishControlItems, AsyncAuxVars, vars

THEOREM ExecuteSignVoteReadyImpliesEnabled ==
  \A command:
    ExecuteSignVoteReady(command) => ENABLED ExecuteSignVote(command)
PROOF
  <1>1. ASSUME NEW command, ExecuteSignVoteReady(command)
         PROVE ENABLED ExecuteSignVote(command)
    <2>1. PICK request \in signVotes:
             /\ CommandMatches(command, request.node, request.vote.view,
                               request.vote.subject)
             /\ CompleteVoteSignatureReady(request)
             /\ VoteOutbox(request) \subseteq
                  {item \in AsyncNetworkItems:
                     item.kind \in AsyncControlKinds}
      BY <1>1 DEF ExecuteSignVoteReady
    <2>2. PICK witness:
             witness = SignVoteWitness(command, request)
      BY Isa
    <2>3. FixedSignVoteReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF SignVoteWitness, FixedSignVoteReady,
             ExecuteSignVoteReady
    <2>4. ENABLED FixedSignVoteExecute(witness)
      BY <2>3, FixedSignVoteReadyIffEnabled
    <2>5. FixedSignVoteExecute(witness) \in BOOLEAN
      BY Isa DEF FixedSignVoteExecute
    <2>6. ExecuteSignVote(command) \in BOOLEAN
      BY Isa DEF ExecuteSignVote
    <2>7. FixedSignVoteExecute(witness) => ExecuteSignVote(command)
      BY <2>2, Isa
         DEF SignVoteWitness, FixedSignVoteExecute, ExecuteSignVote
    <2>8. ENABLED FixedSignVoteExecute(witness)
             => ENABLED ExecuteSignVote(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

SignVoteReadyProjection(command) ==
  /\ ExecuteSignVoteReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteSignVoteImpliesReadyProjection ==
  \A command:
    ExecuteSignVote(command) => SignVoteReadyProjection(command)
BY IsaT(300)
   DEF ExecuteSignVote, ExecuteSignVoteReady,
       CompleteVoteSignature, CompleteVoteSignatureReady,
       PublishControlItems, SignVoteReadyProjection

THEOREM SignVoteReadyProjectionIffReady ==
  \A command:
    ENABLED SignVoteReadyProjection(command)
      <=> ExecuteSignVoteReady(command)
BY ExpandENABLED, IsaT(300)
   DEF SignVoteReadyProjection, ExecuteSignVoteReady, vars

THEOREM ExecuteSignVoteEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteSignVote(command) => ExecuteSignVoteReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecuteSignVote(command)
         PROVE ExecuteSignVoteReady(command)
    <2>1. ExecuteSignVote(command) \in BOOLEAN
      BY Isa DEF ExecuteSignVote
    <2>2. SignVoteReadyProjection(command) \in BOOLEAN
      BY Isa DEF SignVoteReadyProjection
    <2>3. ExecuteSignVote(command)
             => SignVoteReadyProjection(command)
      BY ExecuteSignVoteImpliesReadyProjection
    <2>4. ENABLED ExecuteSignVote(command)
             => ENABLED SignVoteReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4, SignVoteReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecuteSignVoteReadyIffEnabledComposed ==
  \A command:
    ExecuteSignVoteReady(command) <=> ENABLED ExecuteSignVote(command)
BY ExecuteSignVoteReadyImpliesEnabled,
   ExecuteSignVoteEnabledImpliesReady

SignTimeoutWitness(command, request) ==
  [command |-> command, request |-> request]

FixedSignTimeoutReady(witness) ==
  /\ witness.command.kind = "SignTimeout"
  /\ witness.request \in signTimeouts
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.vote.view,
                    witness.request.vote.highSubject)
  /\ CompleteTimeoutSignatureReady(witness.request)
  /\ TimeoutOutbox(witness.request) \subseteq
       {item \in AsyncNetworkItems:
          item.kind \in AsyncControlKinds}

FixedSignTimeoutExecute(witness) ==
  /\ witness.command.kind = "SignTimeout"
  /\ witness.request \in signTimeouts
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.vote.view,
                    witness.request.vote.highSubject)
  /\ CompleteTimeoutSignature(witness.request)
  /\ PublishControlItems(TimeoutOutbox(witness.request))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM FixedSignTimeoutReadyIffEnabled ==
  \A witness:
    FixedSignTimeoutReady(witness)
      <=> ENABLED FixedSignTimeoutExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedSignTimeoutReady, FixedSignTimeoutExecute,
       CompleteTimeoutSignatureReady, CompleteTimeoutSignature,
       PublishControlItems, AsyncAuxVars, vars

THEOREM ExecuteSignTimeoutReadyImpliesEnabled ==
  \A command:
    ExecuteSignTimeoutReady(command)
      => ENABLED ExecuteSignTimeout(command)
PROOF
  <1>1. ASSUME NEW command, ExecuteSignTimeoutReady(command)
         PROVE ENABLED ExecuteSignTimeout(command)
    <2>1. PICK request \in signTimeouts:
             /\ CommandMatches(command, request.node, request.vote.view,
                               request.vote.highSubject)
             /\ CompleteTimeoutSignatureReady(request)
             /\ TimeoutOutbox(request) \subseteq
                  {item \in AsyncNetworkItems:
                     item.kind \in AsyncControlKinds}
      BY <1>1 DEF ExecuteSignTimeoutReady
    <2>2. PICK witness:
             witness = SignTimeoutWitness(command, request)
      BY Isa
    <2>3. FixedSignTimeoutReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF SignTimeoutWitness, FixedSignTimeoutReady,
             ExecuteSignTimeoutReady
    <2>4. ENABLED FixedSignTimeoutExecute(witness)
      BY <2>3, FixedSignTimeoutReadyIffEnabled
    <2>5. FixedSignTimeoutExecute(witness) \in BOOLEAN
      BY Isa DEF FixedSignTimeoutExecute
    <2>6. ExecuteSignTimeout(command) \in BOOLEAN
      BY Isa DEF ExecuteSignTimeout
    <2>7. FixedSignTimeoutExecute(witness)
             => ExecuteSignTimeout(command)
      BY <2>2, Isa
         DEF SignTimeoutWitness, FixedSignTimeoutExecute,
             ExecuteSignTimeout
    <2>8. ENABLED FixedSignTimeoutExecute(witness)
             => ENABLED ExecuteSignTimeout(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

SignTimeoutReadyProjection(command) ==
  /\ ExecuteSignTimeoutReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteSignTimeoutImpliesReadyProjection ==
  \A command:
    ExecuteSignTimeout(command)
      => SignTimeoutReadyProjection(command)
BY IsaT(300)
   DEF ExecuteSignTimeout, ExecuteSignTimeoutReady,
       CompleteTimeoutSignature, CompleteTimeoutSignatureReady,
       PublishControlItems, SignTimeoutReadyProjection

THEOREM SignTimeoutReadyProjectionIffReady ==
  \A command:
    ENABLED SignTimeoutReadyProjection(command)
      <=> ExecuteSignTimeoutReady(command)
BY ExpandENABLED, IsaT(300)
   DEF SignTimeoutReadyProjection, ExecuteSignTimeoutReady, vars

THEOREM ExecuteSignTimeoutEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteSignTimeout(command)
      => ExecuteSignTimeoutReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecuteSignTimeout(command)
         PROVE ExecuteSignTimeoutReady(command)
    <2>1. ExecuteSignTimeout(command) \in BOOLEAN
      BY Isa DEF ExecuteSignTimeout
    <2>2. SignTimeoutReadyProjection(command) \in BOOLEAN
      BY Isa DEF SignTimeoutReadyProjection
    <2>3. ExecuteSignTimeout(command)
             => SignTimeoutReadyProjection(command)
      BY ExecuteSignTimeoutImpliesReadyProjection
    <2>4. ENABLED ExecuteSignTimeout(command)
             => ENABLED SignTimeoutReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, SignTimeoutReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecuteSignTimeoutReadyIffEnabledComposed ==
  \A command:
    ExecuteSignTimeoutReady(command)
      <=> ENABLED ExecuteSignTimeout(command)
BY ExecuteSignTimeoutReadyImpliesEnabled,
   ExecuteSignTimeoutEnabledImpliesReady

SignProposalWitness(command, request) ==
  [command |-> command, request |-> request]

FixedSignProposalReady(witness) ==
  /\ witness.command.kind = "SignProposal"
  /\ witness.request \in signProposals
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.proposal.view,
                    witness.request.proposal.subject)
  /\ CompleteProposalSignatureReady(witness.request)
  /\ ProposalOutbox(witness.request) \subseteq
       {item \in AsyncNetworkItems:
          item.kind \in AsyncControlKinds}

FixedSignProposalExecute(witness) ==
  /\ witness.command.kind = "SignProposal"
  /\ witness.request \in signProposals
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.proposal.view,
                    witness.request.proposal.subject)
  /\ CompleteProposalSignature(witness.request)
  /\ PublishControlAndEphemeralItems(
       ProposalOutbox(witness.request),
       BroadcastChunkOutbox(
         witness.request.node, witness.request.proposal.view,
         witness.request.proposal.subject))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM FixedSignProposalReadyIffEnabled ==
  \A witness:
    FixedSignProposalReady(witness)
      <=> ENABLED FixedSignProposalExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedSignProposalReady, FixedSignProposalExecute,
       CompleteProposalSignatureReady, CompleteProposalSignature,
       PublishControlAndEphemeralItems, AsyncAuxVars, vars

THEOREM ExecuteSignProposalReadyImpliesEnabled ==
  \A command:
    ExecuteSignProposalReady(command)
      => ENABLED ExecuteSignProposal(command)
PROOF
  <1>1. ASSUME NEW command, ExecuteSignProposalReady(command)
         PROVE ENABLED ExecuteSignProposal(command)
    <2>1. PICK request \in signProposals:
             /\ CommandMatches(command, request.node,
                               request.proposal.view,
                               request.proposal.subject)
             /\ CompleteProposalSignatureReady(request)
             /\ ProposalOutbox(request) \subseteq
                  {item \in AsyncNetworkItems:
                     item.kind \in AsyncControlKinds}
      BY <1>1 DEF ExecuteSignProposalReady
    <2>2. PICK witness:
             witness = SignProposalWitness(command, request)
      BY Isa
    <2>3. FixedSignProposalReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF SignProposalWitness, FixedSignProposalReady,
             ExecuteSignProposalReady
    <2>4. ENABLED FixedSignProposalExecute(witness)
      BY <2>3, FixedSignProposalReadyIffEnabled
    <2>5. FixedSignProposalExecute(witness) \in BOOLEAN
      BY Isa DEF FixedSignProposalExecute
    <2>6. ExecuteSignProposal(command) \in BOOLEAN
      BY Isa DEF ExecuteSignProposal
    <2>7. FixedSignProposalExecute(witness)
             => ExecuteSignProposal(command)
      BY <2>2, Isa
         DEF SignProposalWitness, FixedSignProposalExecute,
             ExecuteSignProposal
    <2>8. ENABLED FixedSignProposalExecute(witness)
             => ENABLED ExecuteSignProposal(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

SignProposalReadyProjection(command) ==
  /\ ExecuteSignProposalReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteSignProposalImpliesReadyProjection ==
  \A command:
    ExecuteSignProposal(command)
      => SignProposalReadyProjection(command)
BY IsaT(300)
   DEF ExecuteSignProposal, ExecuteSignProposalReady,
       CompleteProposalSignature, CompleteProposalSignatureReady,
       PublishControlAndEphemeralItems,
       SignProposalReadyProjection

THEOREM SignProposalReadyProjectionIffReady ==
  \A command:
    ENABLED SignProposalReadyProjection(command)
      <=> ExecuteSignProposalReady(command)
BY ExpandENABLED, IsaT(300)
   DEF SignProposalReadyProjection, ExecuteSignProposalReady, vars

THEOREM ExecuteSignProposalEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteSignProposal(command)
      => ExecuteSignProposalReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecuteSignProposal(command)
         PROVE ExecuteSignProposalReady(command)
    <2>1. ExecuteSignProposal(command) \in BOOLEAN
      BY Isa DEF ExecuteSignProposal
    <2>2. SignProposalReadyProjection(command) \in BOOLEAN
      BY Isa DEF SignProposalReadyProjection
    <2>3. ExecuteSignProposal(command)
             => SignProposalReadyProjection(command)
      BY ExecuteSignProposalImpliesReadyProjection
    <2>4. ENABLED ExecuteSignProposal(command)
             => ENABLED SignProposalReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, SignProposalReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecuteSignProposalReadyIffEnabledComposed ==
  \A command:
    ExecuteSignProposalReady(command)
      <=> ENABLED ExecuteSignProposal(command)
BY ExecuteSignProposalReadyImpliesEnabled,
   ExecuteSignProposalEnabledImpliesReady

PersistDecisionWitness(command, request) ==
  [command |-> command, request |-> request]

FixedPersistDecisionReady(witness) ==
  /\ witness.command.kind = "PersistDecision"
  /\ witness.request \in pendingDecision
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.qc.view,
                    witness.request.qc.subject)
  /\ PersistDecisionReady(witness.request)

FixedPersistDecisionExecute(witness) ==
  /\ witness.command.kind = "PersistDecision"
  /\ witness.request \in pendingDecision
  /\ CommandMatches(witness.command, witness.request.node,
                    witness.request.qc.view,
                    witness.request.qc.subject)
  /\ PersistDecision(witness.request)
  /\ PersistDecisionControl(
       witness.request.node,
       witness.request.qc,
       QcOutbox(witness.request.node, witness.request.qc),
       witness.request.rebroadcast)
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM FixedPersistDecisionReadyIffEnabled ==
  \A witness:
    FixedPersistDecisionReady(witness)
      <=> ENABLED FixedPersistDecisionExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedPersistDecisionReady, FixedPersistDecisionExecute,
       PersistDecisionReady, PersistDecision, PersistDecisionControl,
       AsyncAuxVars, vars

THEOREM ExecutePersistDecisionReadyImpliesEnabled ==
  \A command:
    ExecutePersistDecisionReady(command)
      => ENABLED ExecutePersistDecision(command)
PROOF
  <1>1. ASSUME NEW command, ExecutePersistDecisionReady(command)
         PROVE ENABLED ExecutePersistDecision(command)
    <2>1. PICK request \in pendingDecision:
             /\ CommandMatches(command, request.node, request.qc.view,
                               request.qc.subject)
             /\ PersistDecisionReady(request)
      BY <1>1 DEF ExecutePersistDecisionReady
    <2>2. PICK witness:
             witness = PersistDecisionWitness(command, request)
      BY Isa
    <2>3. FixedPersistDecisionReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF PersistDecisionWitness, FixedPersistDecisionReady,
             ExecutePersistDecisionReady
    <2>4. ENABLED FixedPersistDecisionExecute(witness)
      BY <2>3, FixedPersistDecisionReadyIffEnabled
    <2>5. FixedPersistDecisionExecute(witness) \in BOOLEAN
      BY Isa DEF FixedPersistDecisionExecute
    <2>6. ExecutePersistDecision(command) \in BOOLEAN
      BY Isa DEF ExecutePersistDecision
    <2>7. FixedPersistDecisionExecute(witness)
             => ExecutePersistDecision(command)
      BY <2>2, Isa
         DEF PersistDecisionWitness, FixedPersistDecisionExecute,
             ExecutePersistDecision
    <2>8. ENABLED FixedPersistDecisionExecute(witness)
             => ENABLED ExecutePersistDecision(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

PersistDecisionReadyProjection(command) ==
  /\ ExecutePersistDecisionReady(command)
  /\ [TRUE]_vars

THEOREM ExecutePersistDecisionImpliesReadyProjection ==
  \A command:
    ExecutePersistDecision(command)
      => PersistDecisionReadyProjection(command)
BY IsaT(300)
   DEF ExecutePersistDecision, ExecutePersistDecisionReady,
       PersistDecision, PersistDecisionReady,
       PersistDecisionControl, PersistDecisionReadyProjection

THEOREM PersistDecisionReadyProjectionIffReady ==
  \A command:
    ENABLED PersistDecisionReadyProjection(command)
      <=> ExecutePersistDecisionReady(command)
BY ExpandENABLED, IsaT(300)
   DEF PersistDecisionReadyProjection,
       ExecutePersistDecisionReady, vars

THEOREM ExecutePersistDecisionEnabledImpliesReady ==
  \A command:
    ENABLED ExecutePersistDecision(command)
      => ExecutePersistDecisionReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecutePersistDecision(command)
         PROVE ExecutePersistDecisionReady(command)
    <2>1. ExecutePersistDecision(command) \in BOOLEAN
      BY Isa DEF ExecutePersistDecision
    <2>2. PersistDecisionReadyProjection(command) \in BOOLEAN
      BY Isa DEF PersistDecisionReadyProjection
    <2>3. ExecutePersistDecision(command)
             => PersistDecisionReadyProjection(command)
      BY ExecutePersistDecisionImpliesReadyProjection
    <2>4. ENABLED ExecutePersistDecision(command)
             => ENABLED PersistDecisionReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, PersistDecisionReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecutePersistDecisionReadyIffEnabledComposed ==
  \A command:
    ExecutePersistDecisionReady(command)
      <=> ENABLED ExecutePersistDecision(command)
BY ExecutePersistDecisionReadyImpliesEnabled,
   ExecutePersistDecisionEnabledImpliesReady

ApplyWitness(command, qc) ==
  [command |-> command, qc |-> qc]

FixedApplyReady(witness) ==
  /\ witness.command.kind = "Apply"
  /\ witness.qc \in DecisionQcValues
  /\ CommandMatches(witness.command, witness.command.node,
                    witness.qc.view, witness.qc.subject)
  /\ ApplyDecisionReady(witness.command.node, witness.qc)

FixedApplyExecute(witness) ==
  /\ witness.command.kind = "Apply"
  /\ witness.qc \in DecisionQcValues
  /\ CommandMatches(witness.command, witness.command.node,
                    witness.qc.view, witness.qc.subject)
  /\ ApplyDecision(witness.command.node, witness.qc)
  /\ asyncHistoricalRecoveryTargets' =
       asyncHistoricalRecoveryTargets \ {witness.command.node}
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines, asyncSentItems,
                 asyncRetainedControl, asyncActiveRequests,
                 asyncTransport, asyncIngressLanes, asyncIngressReady,
                 asyncHeldChunks>>

\* Both sides of the readiness/execution equivalence expose the same durable
\* current-context Commit authority.  Causal command evidence is not reused as
\* the Decision certificate.
THEOREM ExecuteApplyUsesCurrentCommitAuthority ==
  \A command:
    ExecuteApply(command)
      => \E qc \in DecisionQcValues:
           /\ CommandMatches(command, command.node,
                             qc.view, qc.subject)
           /\ DecisionCertifiedBodyRecoveryAuthority(command.node, qc)
           /\ ApplyDecision(command.node, qc)
BY Isa
   DEF ExecuteApply, ApplyDecision

THEOREM ExecuteApplyReadyUsesCurrentCommitAuthority ==
  \A command:
    ExecuteApplyReady(command)
      => \E qc \in DecisionQcValues:
           /\ CommandMatches(command, command.node,
                             qc.view, qc.subject)
           /\ DecisionCertifiedBodyRecoveryAuthority(command.node, qc)
           /\ ApplyDecisionReady(command.node, qc)
BY Isa
   DEF ExecuteApplyReady, ApplyDecisionReady

THEOREM FixedApplyReadyIffEnabled ==
  \A witness:
    FixedApplyReady(witness) <=> ENABLED FixedApplyExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedApplyReady, FixedApplyExecute,
       ApplyDecisionReady, ApplyDecision, AsyncAuxVars, vars

THEOREM ExecuteApplyReadyImpliesEnabled ==
  \A command:
    ExecuteApplyReady(command) => ENABLED ExecuteApply(command)
PROOF
  <1>1. ASSUME NEW command, ExecuteApplyReady(command)
         PROVE ENABLED ExecuteApply(command)
    <2>1. PICK qc \in DecisionQcValues:
             /\ CommandMatches(command, command.node,
                               qc.view, qc.subject)
             /\ ApplyDecisionReady(command.node, qc)
      BY <1>1 DEF ExecuteApplyReady
    <2>2. PICK witness:
             witness = ApplyWitness(command, qc)
      BY Isa
    <2>3. FixedApplyReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF ApplyWitness, FixedApplyReady, ExecuteApplyReady
    <2>4. ENABLED FixedApplyExecute(witness)
      BY <2>3, FixedApplyReadyIffEnabled
    <2>5. FixedApplyExecute(witness) \in BOOLEAN
      BY Isa DEF FixedApplyExecute
    <2>6. ExecuteApply(command) \in BOOLEAN
      BY Isa DEF ExecuteApply
    <2>7. FixedApplyExecute(witness) => ExecuteApply(command)
      BY <2>2, Isa
         DEF ApplyWitness, FixedApplyExecute, ExecuteApply
    <2>8. ENABLED FixedApplyExecute(witness)
             => ENABLED ExecuteApply(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

ApplyReadyProjection(command) ==
  /\ ExecuteApplyReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteApplyImpliesReadyProjection ==
  \A command:
    ExecuteApply(command) => ApplyReadyProjection(command)
BY IsaT(300)
   DEF ExecuteApply, ExecuteApplyReady,
       ApplyDecision, ApplyDecisionReady, ApplyReadyProjection

THEOREM ApplyReadyProjectionIffReady ==
  \A command:
    ENABLED ApplyReadyProjection(command)
      <=> ExecuteApplyReady(command)
BY ExpandENABLED, IsaT(300)
   DEF ApplyReadyProjection, ExecuteApplyReady, vars

THEOREM ExecuteApplyEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteApply(command) => ExecuteApplyReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecuteApply(command)
         PROVE ExecuteApplyReady(command)
    <2>1. ExecuteApply(command) \in BOOLEAN
      BY Isa DEF ExecuteApply
    <2>2. ApplyReadyProjection(command) \in BOOLEAN
      BY Isa DEF ApplyReadyProjection
    <2>3. ExecuteApply(command) => ApplyReadyProjection(command)
      BY ExecuteApplyImpliesReadyProjection
    <2>4. ENABLED ExecuteApply(command)
             => ENABLED ApplyReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <1>1, <2>4, ApplyReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecuteApplyReadyIffEnabledComposed ==
  \A command:
    ExecuteApplyReady(command) <=> ENABLED ExecuteApply(command)
BY ExecuteApplyReadyImpliesEnabled,
   ExecuteApplyEnabledImpliesReady

PersistInstallWitness(command, request) ==
  [command |-> command, request |-> request]

FixedPersistInstallReady(witness) ==
  /\ witness.command.kind = "PersistInstallTC"
  /\ witness.request \in pendingInstallTC
  /\ witness.command.node = witness.request.node
  /\ witness.command.view = witness.request.tc.view
  /\ PersistInstallTCReady(witness.request)

FixedPersistInstallExecute(witness) ==
  /\ witness.command.kind = "PersistInstallTC"
  /\ witness.request \in pendingInstallTC
  /\ witness.command.node = witness.request.node
  /\ witness.command.view = witness.request.tc.view
  /\ PersistInstallTC(witness.request)
  /\ PersistInstalledControlAfterInstall(
       witness.request.node, witness.request.tc,
       TcOutbox(witness.request.node, witness.request.tc),
       witness.request.rebroadcast)
  /\ asyncNodeDeadlines' =
       [asyncNodeDeadlines EXCEPT
          ![witness.command.node] =
            asyncNow + AsyncViewTimeout(witness.command.view + 1)]
  /\ asyncRetransmitDeadlines' =
       [asyncRetransmitDeadlines EXCEPT
          ![witness.command.node] = asyncNow + AsyncRetransmitPeriod]
  /\ UNCHANGED <<asyncOutstandingTags,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM FixedPersistInstallReadyIffEnabled ==
  \A witness:
    FixedPersistInstallReady(witness)
      <=> ENABLED FixedPersistInstallExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedPersistInstallReady, FixedPersistInstallExecute,
       PersistInstallTCReady, PersistInstallTC,
       PersistInstalledControlAfterInstall, AsyncAuxVars, vars

THEOREM ExecutePersistInstallReadyImpliesEnabled ==
  \A command:
    ExecutePersistInstallReady(command)
      => ENABLED ExecutePersistInstall(command)
PROOF
  <1>1. ASSUME NEW command, ExecutePersistInstallReady(command)
         PROVE ENABLED ExecutePersistInstall(command)
    <2>1. PICK request \in pendingInstallTC:
             /\ command.node = request.node
             /\ command.view = request.tc.view
             /\ PersistInstallTCReady(request)
      BY <1>1 DEF ExecutePersistInstallReady
    <2>2. PICK witness:
             witness = PersistInstallWitness(command, request)
      BY Isa
    <2>3. FixedPersistInstallReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF PersistInstallWitness, FixedPersistInstallReady,
             ExecutePersistInstallReady
    <2>4. ENABLED FixedPersistInstallExecute(witness)
      BY <2>3, FixedPersistInstallReadyIffEnabled
    <2>5. FixedPersistInstallExecute(witness) \in BOOLEAN
      BY Isa DEF FixedPersistInstallExecute
    <2>6. ExecutePersistInstall(command) \in BOOLEAN
      BY Isa DEF ExecutePersistInstall
    <2>7. FixedPersistInstallExecute(witness)
             => ExecutePersistInstall(command)
      BY <2>2, Isa
         DEF PersistInstallWitness, FixedPersistInstallExecute,
             ExecutePersistInstall
    <2>8. ENABLED FixedPersistInstallExecute(witness)
             => ENABLED ExecutePersistInstall(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

PersistInstallReadyProjection(command) ==
  /\ ExecutePersistInstallReady(command)
  /\ [TRUE]_vars

THEOREM ExecutePersistInstallImpliesReadyProjection ==
  \A command:
    ExecutePersistInstall(command)
      => PersistInstallReadyProjection(command)
BY IsaT(300)
   DEF ExecutePersistInstall, ExecutePersistInstallReady,
       PersistInstallTC, PersistInstallTCReady,
       PersistInstalledControlAfterInstall,
       PersistInstallReadyProjection

THEOREM PersistInstallReadyProjectionIffReady ==
  \A command:
    ENABLED PersistInstallReadyProjection(command)
      <=> ExecutePersistInstallReady(command)
BY ExpandENABLED, IsaT(300)
   DEF PersistInstallReadyProjection,
       ExecutePersistInstallReady, vars

THEOREM ExecutePersistInstallEnabledImpliesReady ==
  \A command:
    ENABLED ExecutePersistInstall(command)
      => ExecutePersistInstallReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecutePersistInstall(command)
         PROVE ExecutePersistInstallReady(command)
    <2>1. ExecutePersistInstall(command) \in BOOLEAN
      BY Isa DEF ExecutePersistInstall
    <2>2. PersistInstallReadyProjection(command) \in BOOLEAN
      BY Isa DEF PersistInstallReadyProjection
    <2>3. ExecutePersistInstall(command)
             => PersistInstallReadyProjection(command)
      BY ExecutePersistInstallImpliesReadyProjection
    <2>4. ENABLED ExecutePersistInstall(command)
             => ENABLED PersistInstallReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, PersistInstallReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecutePersistInstallReadyIffEnabledComposed ==
  \A command:
    ExecutePersistInstallReady(command)
      <=> ENABLED ExecutePersistInstall(command)
BY ExecutePersistInstallReadyImpliesEnabled,
   ExecutePersistInstallEnabledImpliesReady

RequestCertifiedBodyWitness(command, qc) ==
  [command |-> command, qc |-> qc]

FixedRequestCertifiedBodyReady(witness) ==
  /\ witness.command.kind = "RequestCertifiedBody"
  /\ ~BodyHeldBy(durableBodies, witness.command.node, context,
                  witness.command.view, witness.command.subject)
  /\ witness.qc \in DecisionQcValues \cup prepareQCs
  /\ CommandMatches(witness.command, witness.command.node,
                    witness.qc.view, witness.qc.subject)
  /\ witness.command.evidence = witness.qc
  /\ CertifiedBodyRecoveryAuthority(witness.command.node, witness.qc)
  /\ \A item \in
       CertifiedRequestOutbox(witness.command.node, witness.qc):
       item.kind = "CertifiedRequest"

FixedRequestCertifiedBodyExecute(witness) ==
  /\ witness.command.kind = "RequestCertifiedBody"
  /\ ~BodyHeldBy(durableBodies, witness.command.node, context,
                  witness.command.view, witness.command.subject)
  /\ witness.qc \in DecisionQcValues \cup prepareQCs
  /\ CommandMatches(witness.command, witness.command.node,
                    witness.qc.view, witness.qc.subject)
  /\ witness.command.evidence = witness.qc
  /\ CertifiedBodyRecoveryAuthority(witness.command.node, witness.qc)
  /\ UNCHANGED vars
  /\ PublishCertifiedRequests(
       CertifiedRequestOutbox(witness.command.node, witness.qc))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                 asyncRetransmitDeadlines,
                 asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                 asyncHistoricalRecoveryTargets>>

THEOREM FixedRequestCertifiedBodyReadyIffEnabled ==
  \A witness:
    FixedRequestCertifiedBodyReady(witness)
      <=> ENABLED FixedRequestCertifiedBodyExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedRequestCertifiedBodyReady,
       FixedRequestCertifiedBodyExecute,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       AsyncAuxVars, vars

THEOREM ExecuteRequestCertifiedBodyReadyImpliesEnabled ==
  \A command:
    ExecuteRequestCertifiedBodyReady(command)
      => ENABLED ExecuteRequestCertifiedBody(command)
PROOF
  <1>1. ASSUME NEW command,
                ExecuteRequestCertifiedBodyReady(command)
         PROVE ENABLED ExecuteRequestCertifiedBody(command)
    <2>1. PICK qc \in DecisionQcValues \cup prepareQCs:
             /\ CommandMatches(command, command.node,
                               qc.view, qc.subject)
             /\ command.evidence = qc
             /\ CertifiedBodyRecoveryAuthority(command.node, qc)
             /\ \A item \in CertifiedRequestOutbox(command.node, qc):
                    item.kind = "CertifiedRequest"
      BY <1>1 DEF ExecuteRequestCertifiedBodyReady
    <2>2. PICK witness:
             witness = RequestCertifiedBodyWitness(command, qc)
      BY Isa
    <2>3. FixedRequestCertifiedBodyReady(witness)
      BY <1>1, <2>1, <2>2, Isa
         DEF RequestCertifiedBodyWitness,
             FixedRequestCertifiedBodyReady,
             ExecuteRequestCertifiedBodyReady
    <2>4. ENABLED FixedRequestCertifiedBodyExecute(witness)
      BY <2>3, FixedRequestCertifiedBodyReadyIffEnabled
    <2>5. FixedRequestCertifiedBodyExecute(witness) \in BOOLEAN
      BY Isa DEF FixedRequestCertifiedBodyExecute
    <2>6. ExecuteRequestCertifiedBody(command) \in BOOLEAN
      BY Isa DEF ExecuteRequestCertifiedBody
    <2>7. FixedRequestCertifiedBodyExecute(witness)
             => ExecuteRequestCertifiedBody(command)
      BY <2>2, Isa
         DEF RequestCertifiedBodyWitness,
             FixedRequestCertifiedBodyExecute,
             ExecuteRequestCertifiedBody
    <2>8. ENABLED FixedRequestCertifiedBodyExecute(witness)
             => ENABLED ExecuteRequestCertifiedBody(command)
      BY <2>5, <2>6, <2>7, ENABLEDaxioms
    <2> QED BY <2>4, <2>8
  <1> QED BY <1>1

RequestCertifiedBodyReadyProjection(command) ==
  /\ ExecuteRequestCertifiedBodyReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteRequestCertifiedBodyImpliesReadyProjection ==
  \A command:
    ExecuteRequestCertifiedBody(command)
      => RequestCertifiedBodyReadyProjection(command)
BY IsaT(300)
   DEF ExecuteRequestCertifiedBody,
       ExecuteRequestCertifiedBodyReady,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       RequestCertifiedBodyReadyProjection

THEOREM RequestCertifiedBodyReadyProjectionIffReady ==
  \A command:
    ENABLED RequestCertifiedBodyReadyProjection(command)
      <=> ExecuteRequestCertifiedBodyReady(command)
BY ExpandENABLED, IsaT(300)
   DEF RequestCertifiedBodyReadyProjection,
       ExecuteRequestCertifiedBodyReady, vars

THEOREM ExecuteRequestCertifiedBodyEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteRequestCertifiedBody(command)
      => ExecuteRequestCertifiedBodyReady(command)
PROOF
  <1>1. ASSUME NEW command,
                ENABLED ExecuteRequestCertifiedBody(command)
         PROVE ExecuteRequestCertifiedBodyReady(command)
    <2>1. ExecuteRequestCertifiedBody(command) \in BOOLEAN
      BY Isa DEF ExecuteRequestCertifiedBody
    <2>2. RequestCertifiedBodyReadyProjection(command) \in BOOLEAN
      BY Isa DEF RequestCertifiedBodyReadyProjection
    <2>3. ExecuteRequestCertifiedBody(command)
             => RequestCertifiedBodyReadyProjection(command)
      BY ExecuteRequestCertifiedBodyImpliesReadyProjection
    <2>4. ENABLED ExecuteRequestCertifiedBody(command)
             => ENABLED RequestCertifiedBodyReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4,
         RequestCertifiedBodyReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecuteRequestCertifiedBodyReadyIffEnabledComposed ==
  \A command:
    ExecuteRequestCertifiedBodyReady(command)
      <=> ENABLED ExecuteRequestCertifiedBody(command)
BY ExecuteRequestCertifiedBodyReadyImpliesEnabled,
   ExecuteRequestCertifiedBodyEnabledImpliesReady

DecisionFetchHeldReady(command) ==
  /\ CertifiedRecoveryFetchFrontier(command)
  /\ BodyHeldBy(durableBodies, command.node, context,
                 command.view, command.subject)

DecisionFetchHeldExecute(command) ==
  /\ CertifiedRecoveryFetchFrontier(command)
  /\ BodyHeldBy(durableBodies, command.node, context,
                 command.view, command.subject)
  /\ UNCHANGED vars
  /\ UNCHANGED <<asyncSentItems, asyncRetainedControl,
                  asyncActiveRequests, asyncTransport>>
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                  asyncRetransmitDeadlines,
                  asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                  asyncHistoricalRecoveryTargets>>

THEOREM DecisionFetchHeldReadyIffEnabled ==
  \A command:
    DecisionFetchHeldReady(command)
      <=> ENABLED DecisionFetchHeldExecute(command)
BY ExpandENABLED, IsaT(300)
   DEF DecisionFetchHeldReady, DecisionFetchHeldExecute, vars

DecisionFetchMissingWitness(command, qc) ==
  [command |-> command, qc |-> qc]

FixedDecisionFetchMissingReady(witness) ==
  /\ CertifiedRecoveryFetchFrontier(witness.command)
  /\ ~BodyHeldBy(durableBodies, witness.command.node, context,
                  witness.command.view, witness.command.subject)
  /\ witness.qc \in DecisionQcValues \cup prepareQCs
  /\ CommandMatches(witness.command, witness.command.node,
                    witness.qc.view, witness.qc.subject)
  /\ witness.command.evidence = witness.qc
  /\ CertifiedBodyRecoveryAuthority(witness.command.node, witness.qc)
  /\ \A item \in
       CertifiedRequestOutbox(witness.command.node, witness.qc):
       item.kind = "CertifiedRequest"

FixedDecisionFetchMissingExecute(witness) ==
  /\ CertifiedRecoveryFetchFrontier(witness.command)
  /\ ~BodyHeldBy(durableBodies, witness.command.node, context,
                  witness.command.view, witness.command.subject)
  /\ witness.qc \in DecisionQcValues \cup prepareQCs
  /\ CommandMatches(witness.command, witness.command.node,
                    witness.qc.view, witness.qc.subject)
  /\ witness.command.evidence = witness.qc
  /\ CertifiedBodyRecoveryAuthority(witness.command.node, witness.qc)
  /\ UNCHANGED vars
  /\ PublishCertifiedRequests(
       CertifiedRequestOutbox(witness.command.node, witness.qc))
  /\ UNCHANGED <<asyncOutstandingTags, asyncNodeDeadlines,
                  asyncRetransmitDeadlines,
                  asyncIngressLanes, asyncIngressReady, asyncHeldChunks,
                  asyncHistoricalRecoveryTargets>>

THEOREM FixedDecisionFetchMissingReadyIffEnabled ==
  \A witness:
    FixedDecisionFetchMissingReady(witness)
      <=> ENABLED FixedDecisionFetchMissingExecute(witness)
BY ExpandENABLED, IsaT(300)
   DEF FixedDecisionFetchMissingReady,
       FixedDecisionFetchMissingExecute,
       PublishCertifiedRequests, CertifiedRequestOutbox, vars

THEOREM ExecuteDecisionFetchReadyImpliesEnabled ==
  \A command:
    ExecuteDecisionFetchReady(command)
      => ENABLED ExecuteDecisionFetch(command)
PROOF
  <1>1. ASSUME NEW command, ExecuteDecisionFetchReady(command)
         PROVE ENABLED ExecuteDecisionFetch(command)
    <2>1. CASE BodyHeldBy(durableBodies, command.node, context,
                           command.view, command.subject)
      <3>1. DecisionFetchHeldReady(command)
        BY <1>1, <2>1
           DEF ExecuteDecisionFetchReady, DecisionFetchHeldReady
      <3>2. ENABLED DecisionFetchHeldExecute(command)
        BY <3>1, DecisionFetchHeldReadyIffEnabled
      <3>3. DecisionFetchHeldExecute(command) \in BOOLEAN
        BY Isa DEF DecisionFetchHeldExecute
      <3>4. ExecuteDecisionFetch(command) \in BOOLEAN
        BY Isa DEF ExecuteDecisionFetch
      <3>5. DecisionFetchHeldExecute(command)
               => ExecuteDecisionFetch(command)
        BY <2>1, Isa
           DEF DecisionFetchHeldExecute, ExecuteDecisionFetch
      <3>6. ENABLED DecisionFetchHeldExecute(command)
               => ENABLED ExecuteDecisionFetch(command)
        BY <3>3, <3>4, <3>5, ENABLEDaxioms
      <3> QED BY <3>2, <3>6
    <2>2. CASE ~BodyHeldBy(durableBodies, command.node, context,
                            command.view, command.subject)
      <3>1. PICK qc \in DecisionQcValues \cup prepareQCs:
               /\ CommandMatches(command, command.node,
                                 qc.view, qc.subject)
               /\ command.evidence = qc
               /\ CertifiedBodyRecoveryAuthority(command.node, qc)
        BY <1>1, Isa
           DEF ExecuteDecisionFetchReady,
               CertifiedRecoveryFetchFrontier,
               DecisionFetchFrontier, LockedPrepareFetchFrontier,
               CertifiedBodyRecoveryAuthority,
               DecisionCertifiedBodyRecoveryAuthority,
               HistoricalLockedPrepareSource, CommandMatches
      <3>2. PICK witness:
               witness = DecisionFetchMissingWitness(command, qc)
        BY Isa
      <3>3. FixedDecisionFetchMissingReady(witness)
        BY <1>1, <2>2, <3>1, <3>2, Isa
           DEF DecisionFetchMissingWitness,
               FixedDecisionFetchMissingReady,
               ExecuteDecisionFetchReady, CertifiedRequestOutbox,
               AsyncNetworkItem
      <3>4. ENABLED FixedDecisionFetchMissingExecute(witness)
        BY <3>3, FixedDecisionFetchMissingReadyIffEnabled
      <3>5. FixedDecisionFetchMissingExecute(witness) \in BOOLEAN
        BY Isa DEF FixedDecisionFetchMissingExecute
      <3>6. ExecuteDecisionFetch(command) \in BOOLEAN
        BY Isa DEF ExecuteDecisionFetch
      <3>7. FixedDecisionFetchMissingExecute(witness)
               => ExecuteDecisionFetch(command)
        BY <2>2, <3>2, Isa
           DEF DecisionFetchMissingWitness,
               FixedDecisionFetchMissingExecute,
               ExecuteDecisionFetch
      <3>8. ENABLED FixedDecisionFetchMissingExecute(witness)
               => ENABLED ExecuteDecisionFetch(command)
        BY <3>5, <3>6, <3>7, ENABLEDaxioms
      <3> QED BY <3>4, <3>8
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

DecisionFetchReadyProjection(command) ==
  /\ ExecuteDecisionFetchReady(command)
  /\ [TRUE]_vars

THEOREM ExecuteDecisionFetchImpliesReadyProjection ==
  \A command:
    ExecuteDecisionFetch(command)
      => DecisionFetchReadyProjection(command)
BY Isa
   DEF ExecuteDecisionFetch, ExecuteDecisionFetchReady,
       DecisionFetchReadyProjection

THEOREM DecisionFetchReadyProjectionIffReady ==
  \A command:
    ENABLED DecisionFetchReadyProjection(command)
      <=> ExecuteDecisionFetchReady(command)
BY ExpandENABLED, IsaT(300)
   DEF DecisionFetchReadyProjection,
       ExecuteDecisionFetchReady, vars

THEOREM ExecuteDecisionFetchEnabledImpliesReady ==
  \A command:
    ENABLED ExecuteDecisionFetch(command)
      => ExecuteDecisionFetchReady(command)
PROOF
  <1>1. ASSUME NEW command, ENABLED ExecuteDecisionFetch(command)
         PROVE ExecuteDecisionFetchReady(command)
    <2>1. ExecuteDecisionFetch(command) \in BOOLEAN
      BY Isa DEF ExecuteDecisionFetch
    <2>2. DecisionFetchReadyProjection(command) \in BOOLEAN
      BY Isa DEF DecisionFetchReadyProjection
    <2>3. ExecuteDecisionFetch(command)
             => DecisionFetchReadyProjection(command)
      BY ExecuteDecisionFetchImpliesReadyProjection
    <2>4. ENABLED ExecuteDecisionFetch(command)
             => ENABLED DecisionFetchReadyProjection(command)
      BY <2>1, <2>2, <2>3, ENABLEDaxioms
    <2> QED
      BY <1>1, <2>4, DecisionFetchReadyProjectionIffReady
  <1> QED BY <1>1

THEOREM ExecuteDecisionFetchReadyIffEnabledComposed ==
  \A command:
    ExecuteDecisionFetchReady(command)
      <=> ENABLED ExecuteDecisionFetch(command)
BY ExecuteDecisionFetchReadyImpliesEnabled,
   ExecuteDecisionFetchEnabledImpliesReady

THEOREM ExecuteDecisionFetchImpliesReady ==
  \A command:
    ExecuteDecisionFetch(command) => ExecuteDecisionFetchReady(command)
BY ExecuteDecisionFetchImpliesReadyProjection
   DEF DecisionFetchReadyProjection

THEOREM ExecuteSignProposalImpliesReady ==
  \A command:
    ExecuteSignProposal(command) => ExecuteSignProposalReady(command)
BY ExecuteSignProposalImpliesReadyProjection
   DEF SignProposalReadyProjection

THEOREM ExecuteSignVoteImpliesReady ==
  \A command:
    ExecuteSignVote(command) => ExecuteSignVoteReady(command)
BY ExecuteSignVoteImpliesReadyProjection
   DEF SignVoteReadyProjection

THEOREM ExecuteSignTimeoutImpliesReady ==
  \A command:
    ExecuteSignTimeout(command) => ExecuteSignTimeoutReady(command)
BY ExecuteSignTimeoutImpliesReadyProjection
   DEF SignTimeoutReadyProjection

THEOREM ExecutePersistInstallImpliesReady ==
  \A command:
    ExecutePersistInstall(command)
      => ExecutePersistInstallReady(command)
BY ExecutePersistInstallImpliesReadyProjection
   DEF PersistInstallReadyProjection

THEOREM ExecutePersistDecisionImpliesReady ==
  \A command:
    ExecutePersistDecision(command)
      => ExecutePersistDecisionReady(command)
BY ExecutePersistDecisionImpliesReadyProjection
   DEF PersistDecisionReadyProjection

THEOREM ExecuteRequestCertifiedBodyImpliesReady ==
  \A command:
    ExecuteRequestCertifiedBody(command)
      => ExecuteRequestCertifiedBodyReady(command)
BY ExecuteRequestCertifiedBodyImpliesReadyProjection
   DEF RequestCertifiedBodyReadyProjection

THEOREM ExecuteApplyImpliesReady ==
  \A command:
    ExecuteApply(command) => ExecuteApplyReady(command)
BY ExecuteApplyImpliesReadyProjection
   DEF ApplyReadyProjection

=============================================================================
