---- MODULE SumeragiV2AsyncInstallRunnerProofs ----
EXTENDS SumeragiV2AsyncRuntimeAdmissionTypeContinuationProofs

THEOREM CausalCandidateFromTypedCommand ==
  \A command, commandClass, kind:
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ commandClass \in AsyncCommandClasses
    /\ kind \in AsyncWorkKinds
    => /\ AsyncCandidateTyped(
             CausalCandidate(commandClass, kind, command))
       /\ CausalCandidate(commandClass, kind, command).node = command.node
PROOF
  <1>1. ASSUME NEW command, NEW commandClass, NEW kind,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command),
                commandClass \in AsyncCommandClasses,
                kind \in AsyncWorkKinds
         PROVE /\ AsyncCandidateTyped(
                      CausalCandidate(commandClass, kind, command))
               /\ CausalCandidate(commandClass, kind, command).node =
                    command.node
    <2>1. /\ context.height \in Heights
           /\ command.node \in ValidatorIds
           /\ command.view \in Views
           /\ command.subject \in SubjectOrNone
      BY <1>1 DEF AsyncTypeInvariant, TypeInvariant,
                   AsyncCandidateTyped
    <2>2. /\ DOMAIN CausalCandidate(commandClass, kind, command) =
                    AsyncCandidateDomain
           /\ CausalCandidate(commandClass, kind, command).class =
                commandClass
           /\ CausalCandidate(commandClass, kind, command).kind = kind
           /\ CausalCandidate(commandClass, kind, command).node =
                command.node
           /\ CausalCandidate(commandClass, kind, command).height =
                context.height
           /\ CausalCandidate(commandClass, kind, command).view =
                command.view
           /\ CausalCandidate(commandClass, kind, command).subject =
                command.subject
           /\ CausalCandidate(commandClass, kind, command).item =
                NoAsyncItem
           /\ CausalCandidate(commandClass, kind, command).consumerContext =
                command.consumerContext
           /\ CausalCandidate(commandClass, kind, command).consumerView =
                command.consumerView
           /\ CausalCandidate(commandClass, kind, command).consumerGeneration =
                command.consumerGeneration
           /\ CausalCandidate(commandClass, kind, command).evidence =
                command.evidence
           /\ CausalCandidate(commandClass, kind, command).bodyIdentity =
                command.bodyIdentity
           /\ CausalCandidate(commandClass, kind, command).manifestIdentity =
                command.manifestIdentity
           /\ CausalCandidate(commandClass, kind, command).commitmentIdentity =
                command.commitmentIdentity
      BY DEF CausalCandidate, AsyncCandidateFrom,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateSuccessorSemanticPhase,
             AsyncCandidateSuccessorProposalRound,
             AsyncCandidateWithIdentityAndOrigin,
             AsyncCandidateDomain
    <2> QED BY <1>1, <2>1, <2>2, SMT
         DEF AsyncCandidateTyped, AsyncEvidenceSet
  <1> QED BY <1>1
THEOREM NonInstallCommandSuccessorsTypedAndOwned ==
  \A command:
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind # "PersistInstallTC"
    => /\ AsyncQueueTyped(CommandSuccessors(command))
       /\ AsyncCausalQueueOwnership(
            command.node, CommandSuccessors(command))
BY CausalCandidateFromTypedCommand, SMTT(120)
   DEF CommandSuccessors, RetainedBodyRebindCandidate,
       PersistDecisionRecoverySuccessor, PersistDecisionRecoveryKind,
       PersistDecisionBody, PersistDecisionValidationHeld,
       PersistDecisionRequest, PersistDecisionRequests,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       AsyncQueueTyped, AsyncCausalQueueOwnership,
       AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds,
       SequenceSet
THEOREM ExecutedInstallSuccessorIsTypedAndOwned ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    => LET successor == InstallProposalSuccessor(command)
       IN /\ AsyncCandidateTyped(successor)
          /\ successor.node = command.node
PROOF
  <1>1. ASSUME NEW command,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command),
                command.kind = "PersistInstallTC",
                ExecuteCommand(command)
         PROVE LET successor == InstallProposalSuccessor(command)
               IN /\ AsyncCandidateTyped(successor)
                  /\ successor.node = command.node
    <2> DEFINE Successor == InstallProposalSuccessor(command)
    <2>1. /\ TypeInvariant
           /\ PendingCertificateWritesAuthorized
           /\ context.height \in Heights
           /\ command.node \in ValidatorIds
      BY <1>1
         DEF StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant, AsyncTypeInvariant,
             TypeInvariant, AsyncCandidateTyped
    <2>2. \E request \in pendingInstallTC:
             /\ command.node = request.node
             /\ command.view = request.tc.view
             /\ PersistInstallTC(request)
      BY <1>1, Isa
         DEF ExecuteCommand, ExecutePersistInstall
    <2>3. PICK request \in pendingInstallTC:
             /\ command.node = request.node
             /\ command.view = request.tc.view
             /\ PersistInstallTC(request)
      BY <2>2
    <2>4. command.view + 1 \in Views
      BY <2>1, <2>3 DEF PendingCertificateWritesAuthorized
    <2>5. InstallProposalSubject(command) \in SubjectOrNone
      BY <2>1, <2>3, SMTT(60), Isa
         DEF InstallProposalSubject, InstallRequests,
             AsyncProposalSubject, PendingCertificateWritesAuthorized,
             TCValid, AuthenticatedHighRef, HighRefValid,
             TcHighRank, TcHighSubject, HighestTimeoutVote,
             MaximalTimeoutVotes, EmptyTimeoutHigh,
             TypeInvariant, AsyncCandidateTyped
    <2>6. /\ context \in ContextRecords
           /\ InstallGenerationAfter(command) \in Generations
           /\ command.evidence \in AsyncEvidenceSet
      BY <1>1, <2>1, <2>3, SMT
         DEF PersistInstallTC, TypeInvariant, AsyncCandidateTyped,
             InstallGenerationAfter, InstallRequests, Generations
    <2>7. /\ DOMAIN Successor = AsyncCandidateDomain
           /\ Successor.class \in AsyncCommandClasses
           /\ Successor.kind \in AsyncWorkKinds
           /\ Successor.node = command.node
           /\ Successor.height = context.height
           /\ Successor.view = command.view + 1
           /\ Successor.subject = InstallProposalSubject(command)
           /\ Successor.item = NoAsyncItem
      BY Isa
         DEF Successor, InstallProposalSuccessor,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateWithIdentityAndOrigin,
             AsyncCandidateDomain,
             AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2> QED BY <2>1, <2>4, <2>5, <2>6, <2>7, SMT
         DEF Successor, InstallProposalSuccessor,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateSuccessorProposalRound,
             AsyncCandidateWithIdentityAndOrigin,
             AsyncCandidateTyped
  <1> QED BY <1>1
THEOREM ExecutedInstallProposalSuccessorMatchesPostState ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    => LET successor == InstallProposalSuccessor(command)
       IN /\ successor.consumerContext = context'
          /\ successor.consumerView = nodeView'[command.node]
          /\ successor.consumerGeneration = generation'[command.node]
          /\ successor.subject =
               IF highestRank'[command.node] = NoRank
               THEN AsyncHeartbeatSubject
               ELSE highestSubject'[command.node]
PROOF
  <1>1. ASSUME NEW command,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command),
                command.kind = "PersistInstallTC",
                ExecuteCommand(command)
         PROVE LET successor == InstallProposalSuccessor(command)
               IN /\ successor.consumerContext = context'
                  /\ successor.consumerView = nodeView'[command.node]
                  /\ successor.consumerGeneration =
                       generation'[command.node]
                  /\ successor.subject =
                       IF highestRank'[command.node] = NoRank
                       THEN AsyncHeartbeatSubject
                       ELSE highestSubject'[command.node]
    <2>1. PICK request \in pendingInstallTC:
             /\ command.node = request.node
             /\ command.view = request.tc.view
             /\ PersistInstallTC(request)
      BY <1>1, Isa DEF ExecuteCommand, ExecutePersistInstall
    <2>2. InstallRequests(command) = {request}
      BY <1>1, <2>1, Isa
         DEF StrongInductiveInvariant, Safety,
             OnePendingPersistencePerNode, RequestsUniqueByNode,
             AllPendingRequests, InstallRequests
    <2>3. /\ context' = context
           /\ nodeView'[command.node] = command.view + 1
           /\ generation'[command.node] =
                InstallGenerationAfter(command)
           /\ highestRank'[command.node] =
                IF TcHighRank(request.tc) > highestRank[command.node]
                THEN TcHighRank(request.tc)
                ELSE highestRank[command.node]
           /\ highestSubject'[command.node] =
                IF TcHighRank(request.tc) > highestRank[command.node]
                THEN TcHighSubject(request.tc)
                ELSE highestSubject[command.node]
      BY <2>1, <2>2, Isa
         DEF PersistInstallTC, InstallGenerationAfter, InstallRequests
    <2>4. InstallProposalSubject(command) =
             IF highestRank'[command.node] = NoRank
             THEN AsyncHeartbeatSubject
             ELSE highestSubject'[command.node]
      BY <1>1, <2>1, <2>2, <2>3, SMTT(30), Isa
         DEF InstallProposalSubject, AsyncProposalSubject
    <2> QED BY <2>3, <2>4, Isa
         DEF InstallProposalSuccessor,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateWithIdentityAndOrigin
  <1> QED BY <1>1
THEOREM ExecutedInstallCommitSignSuccessorIsTypedAndOwned ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    /\ InstallCommitSignRequests(command) # {}
    => /\ AsyncCandidateTyped(InstallCommitSignSuccessor(command))
       /\ InstallCommitSignSuccessor(command).node = command.node
BY SMTT(120), Isa
   DEF InstallCommitSignSuccessor, InstallCommitSignRequests,
       ActiveLockedCommitSignRequestsAfterInstall,
       ExactLockedCommitIntents, VoteSign, VoteSignSet,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       AsyncCandidateTyped, AsyncEvidenceSet,
       AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ExecuteCommand, ExecutePersistInstall

THEOREM ExecutedInstallLockedFetchSuccessorIsTypedAndOwned ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    /\ InstallResultingLockedPrepareQCs(command) # {}
    => /\ AsyncCandidateTyped(InstallLockedFetchSuccessor(command))
       /\ InstallLockedFetchSuccessor(command).node = command.node
BY SMTT(120), Isa
   DEF InstallLockedFetchSuccessor,
       InstallResultingLockedPrepareQCs, InstallRequests,
       HistoricalLockedPrepareRecoveryProvenance,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       AsyncCandidateTyped, AsyncEvidenceSet,
       AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ExecuteCommand, ExecutePersistInstall

(***************************************************************************
Every constructor emitted by a successful TC installation freezes the
post-install consumer epoch.  The constructor is evaluated in the pre-state,
so these equalities are the explicit bridge from `command.view + 1` and the
bounded generation increment to the reducer state produced by
`PersistInstallTC`.
***************************************************************************)
THEOREM ExecutedInstallLockedFetchSuccessorMatchesPostState ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    /\ InstallResultingLockedPrepareQCs(command) # {}
    => LET successor == InstallLockedFetchSuccessor(command)
       IN /\ successor.consumerContext = context'
          /\ successor.consumerView = nodeView'[command.node]
          /\ successor.consumerGeneration = generation'[command.node]
BY ExecutedInstallProposalSuccessorMatchesPostState, Isa
   DEF InstallLockedFetchSuccessor, InstallProposalSuccessor,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateWithIdentityAndOrigin

THEOREM ExecutedInstallCommitSignSuccessorMatchesPostState ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    /\ InstallCommitSignRequests(command) # {}
    => LET successor == InstallCommitSignSuccessor(command)
       IN /\ successor.consumerContext = context'
          /\ successor.consumerView = nodeView'[command.node]
          /\ successor.consumerGeneration = generation'[command.node]
BY ExecutedInstallProposalSuccessorMatchesPostState, Isa
   DEF InstallCommitSignSuccessor, InstallProposalSuccessor,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateWithIdentityAndOrigin

THEOREM ExecutedCommandSuccessorsTypedAndOwned ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ ExecuteCommand(command)
    => /\ AsyncQueueTyped(CommandSuccessors(command))
       /\ AsyncCausalQueueOwnership(
            command.node, CommandSuccessors(command))
PROOF
  <1>1. ASSUME NEW command,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command),
                ExecuteCommand(command)
         PROVE /\ AsyncQueueTyped(CommandSuccessors(command))
               /\ AsyncCausalQueueOwnership(
                    command.node, CommandSuccessors(command))
    <2>1. CASE command.kind # "PersistInstallTC"
      BY <1>1, <2>1, NonInstallCommandSuccessorsTypedAndOwned
    <2>2. CASE command.kind = "PersistInstallTC"
      <3> DEFINE FetchSuccessor == InstallLockedFetchSuccessor(command)
      <3> DEFINE ProposalSuccessor == InstallProposalSuccessor(command)
      <3> DEFINE SignSuccessor == InstallCommitSignSuccessor(command)
      <3>1. /\ AsyncCandidateTyped(ProposalSuccessor)
             /\ ProposalSuccessor.node = command.node
        BY <1>1, <2>2, ExecutedInstallSuccessorIsTypedAndOwned
           DEF ProposalSuccessor, InstallProposalSuccessor
      <3>2. CASE /\ InstallResultingLockedPrepareQCs(command) = {}
                   /\ InstallCommitSignRequests(command) = {}
        <4>1. CommandSuccessors(command) = <<ProposalSuccessor>>
          BY <2>2, <3>2
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors,
                 ProposalSuccessor
        <4>2. /\ AsyncQueueTyped(<<ProposalSuccessor>>)
               /\ AsyncCausalQueueOwnership(
                    command.node, <<ProposalSuccessor>>)
          BY <3>1, TypedCandidateFormsTypedSingleton,
             SingletonSequenceFacts, SMT
             DEF AsyncCausalQueueOwnership, SequenceSet
        <4> QED BY <4>1, <4>2
      <3>3. CASE /\ InstallResultingLockedPrepareQCs(command) = {}
                   /\ InstallCommitSignRequests(command) # {}
        <4>1. /\ AsyncCandidateTyped(SignSuccessor)
               /\ SignSuccessor.node = command.node
          BY <1>1, <2>2, <3>3,
             ExecutedInstallCommitSignSuccessorIsTypedAndOwned
             DEF SignSuccessor
        <4>2. CommandSuccessors(command) =
                 <<SignSuccessor, ProposalSuccessor>>
          BY <2>2, <3>3
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors,
                 SignSuccessor, ProposalSuccessor
        <4>3. /\ AsyncQueueTyped(
                        <<SignSuccessor, ProposalSuccessor>>)
               /\ AsyncCausalQueueOwnership(
                    command.node, <<SignSuccessor, ProposalSuccessor>>)
          BY <3>1, <4>1, Isa
             DEF AsyncQueueTyped, AsyncCausalQueueOwnership,
                 SequenceSet
        <4> QED BY <4>2, <4>3
      <3>4. CASE /\ InstallResultingLockedPrepareQCs(command) # {}
                   /\ InstallCommitSignRequests(command) = {}
        <4>1. /\ AsyncCandidateTyped(FetchSuccessor)
               /\ FetchSuccessor.node = command.node
          BY <1>1, <2>2, <3>4,
             ExecutedInstallLockedFetchSuccessorIsTypedAndOwned
             DEF FetchSuccessor
        <4>2. CommandSuccessors(command) =
                 <<FetchSuccessor, ProposalSuccessor>>
          BY <2>2, <3>4
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors,
                 FetchSuccessor, ProposalSuccessor
        <4>3. /\ AsyncQueueTyped(
                        <<FetchSuccessor, ProposalSuccessor>>)
               /\ AsyncCausalQueueOwnership(
                    command.node, <<FetchSuccessor, ProposalSuccessor>>)
          BY <3>1, <4>1, Isa
             DEF AsyncQueueTyped, AsyncCausalQueueOwnership,
                 SequenceSet
        <4> QED BY <4>2, <4>3
      <3>5. CASE /\ InstallResultingLockedPrepareQCs(command) # {}
                   /\ InstallCommitSignRequests(command) # {}
        <4>1. /\ AsyncCandidateTyped(FetchSuccessor)
               /\ FetchSuccessor.node = command.node
          BY <1>1, <2>2, <3>5,
             ExecutedInstallLockedFetchSuccessorIsTypedAndOwned
             DEF FetchSuccessor
        <4>2. /\ AsyncCandidateTyped(SignSuccessor)
               /\ SignSuccessor.node = command.node
          BY <1>1, <2>2, <3>5,
             ExecutedInstallCommitSignSuccessorIsTypedAndOwned
             DEF SignSuccessor
        <4>3. CommandSuccessors(command) =
                 <<FetchSuccessor, SignSuccessor, ProposalSuccessor>>
          BY <2>2, <3>5
             DEF CommandSuccessors, InstallCommandSuccessors,
                 InstallLockedFetchSuccessors,
                 InstallCommitSignSuccessors,
                 FetchSuccessor, SignSuccessor, ProposalSuccessor
        <4>4. /\ AsyncQueueTyped(
                        <<FetchSuccessor, SignSuccessor,
                          ProposalSuccessor>>)
               /\ AsyncCausalQueueOwnership(
                    command.node,
                    <<FetchSuccessor, SignSuccessor,
                      ProposalSuccessor>>)
          BY <3>1, <4>1, <4>2, Isa
             DEF AsyncQueueTyped, AsyncCausalQueueOwnership,
                 SequenceSet
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ExecutedFreshCommandSuccessorsTypedAndOwned ==
  \A command:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ ExecuteCommand(command)
    => /\ AsyncQueueTyped(FreshCommandSuccessors(command))
       /\ AsyncCausalQueueOwnership(
            command.node, FreshCommandSuccessors(command))
BY ExecutedCommandSuccessorsTypedAndOwned, Isa
   DEF FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       AsyncQueueTyped,
       AsyncCausalQueueOwnership, SequenceSet

THEOREM CommandSuccessorsHaveBoundedLength ==
  \A command:
    Len(CommandSuccessors(command)) \in 0..3
BY Isa
   DEF CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors

THEOREM CommandSuccessorInventoryIsClosed ==
  \A command:
    /\ (command.kind \notin CausalSuccessorParentKinds
          => CommandSuccessors(command) = <<>>)
    /\ (CommandSuccessors(command) # <<>>
          => command.kind \in CausalSuccessorParentKinds)
BY Isa
   DEF CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       CausalSuccessorParentKinds

THEOREM CommandSuccessorsHaveUniqueValues ==
  \A command:
    SequenceHasUniqueValues(CommandSuccessors(command))
BY Isa
   DEF CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       InstallLockedFetchSuccessor,
       InstallCommitSignSuccessor, InstallProposalSuccessor,
       PersistDecisionRecoverySuccessor, PersistDecisionRecoveryKind,
       PersistDecisionBody, PersistDecisionValidationHeld,
       PersistDecisionRequest,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       RetainedBodyRebindCandidate, CausalCandidate, NoItemCandidate,
       AsyncCandidate, SequenceHasUniqueValues, SequenceSet

THEOREM FreshCommandSuccessorsHaveUniqueValues ==
  \A command:
    SequenceHasUniqueValues(FreshCommandSuccessors(command))
BY CommandSuccessorsHaveUniqueValues, Isa
   DEF FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       SequenceHasUniqueValues, SequenceSet

THEOREM FreshCommandSuccessorsAreUnscheduled ==
  \A command:
    SequenceSet(FreshCommandSuccessors(command)) \cap
      (QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
        \cup TrackedWorkCandidates) = {}
BY Isa
   DEF FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       CandidateScheduled, SequenceSet

THEOREM TimeoutCausalSuccessorsTypedAndOwned ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant
      => /\ AsyncCandidateTyped(TimeoutCausalCommand(node))
         /\ AsyncQueueTyped(
              CommandSuccessors(TimeoutCausalCommand(node)))
         /\ AsyncCausalQueueOwnership(
              node, CommandSuccessors(TimeoutCausalCommand(node)))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant
         PROVE /\ AsyncCandidateTyped(TimeoutCausalCommand(node))
               /\ AsyncQueueTyped(
                    CommandSuccessors(TimeoutCausalCommand(node)))
               /\ AsyncCausalQueueOwnership(
                    node, CommandSuccessors(TimeoutCausalCommand(node)))
    <2>1. /\ TypeInvariant
           /\ context.height \in Heights
           /\ nodeView[node] \in Views
           /\ highestSubject[node] \in SubjectOrNone
      BY <1>1, CoreTypeImpliesCausalTypingFacts, SMT
         DEF AsyncTypeInvariant, AsyncCausalCoreTypingFacts
    <2>2. /\ AsyncCandidateTyped(TimeoutCausalCommand(node))
           /\ TimeoutCausalCommand(node).node = node
           /\ TimeoutCausalCommand(node).kind = "BeginTimeout"
      BY <2>1, Isa
         DEF TimeoutCausalCommand, NoItemCandidate, AsyncCandidate,
             AsyncCandidateTyped, AsyncCommandClasses,
             AsyncWorkKinds, AsyncReducerKinds
    <2>3. TimeoutCausalCommand(node).kind # "PersistInstallTC"
      BY <2>2
    <2>4. /\ AsyncQueueTyped(
                  CommandSuccessors(TimeoutCausalCommand(node)))
           /\ AsyncCausalQueueOwnership(
                TimeoutCausalCommand(node).node,
                CommandSuccessors(TimeoutCausalCommand(node)))
      BY <1>1, <2>2, <2>3,
         NonInstallCommandSuccessorsTypedAndOwned
    <2> QED BY <2>2, <2>4
  <1> QED BY <1>1

THEOREM FreshTimeoutCausalSuccessorsTypedAndOwned ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant
      => /\ AsyncQueueTyped(
              FreshCommandSuccessors(TimeoutCausalCommand(node)))
         /\ AsyncCausalQueueOwnership(
              node, FreshCommandSuccessors(TimeoutCausalCommand(node)))
BY TimeoutCausalSuccessorsTypedAndOwned, Isa
   DEF FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallCommitSignSuccessors,
       AsyncQueueTyped,
       AsyncCausalQueueOwnership, SequenceSet,
       TimeoutCausalCommand, NoItemCandidate, AsyncCandidate

THEOREM FunctionalConcatUpdateAtKey ==
  \A mapping, key, sequence:
    key \in DOMAIN mapping
      => [mapping EXCEPT ![key] = @ \o sequence][key]
           = mapping[key] \o sequence
BY Isa

THEOREM AppendOwnedCausalSuccessorsPreservesCausalType ==
  \A node \in ValidatorIds:
    \A successors:
      /\ AsyncCausalTypeInvariant
      /\ AsyncQueueTyped(successors)
      /\ AsyncCausalQueueOwnership(node, successors)
      /\ asyncCausalQueues' =
           [asyncCausalQueues EXCEPT ![node] = @ \o successors]
      => AsyncCausalTypeInvariant'
BY ConcatProperties, RangeConcatenation,
   FunctionalConcatUpdateAtKey, FunctionalUpdateAwayFromKey,
   SMTT(120)
   DEF AsyncCausalTypeInvariant, AsyncCausalQueueOwnership,
       AsyncQueueTyped, SequenceSet

THEOREM NextCommandClassCycleFacts ==
  \A commandClass \in AsyncCommandClasses:
    /\ NextCommandClass(commandClass) \in AsyncCommandClasses
    /\ NextCommandClass(NextCommandClass(commandClass))
         \in AsyncCommandClasses
    /\ {commandClass, NextCommandClass(commandClass),
         NextCommandClass(NextCommandClass(commandClass))}
         = AsyncCommandClasses
BY SMT DEF NextCommandClass, AsyncCommandClasses

THEOREM SelectedCommandClassFacts ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ NodeQueueNonempty(node)
    => /\ SelectedCommandClass(node) \in AsyncCommandClasses
       /\ CommandClassIndices(node, SelectedCommandClass(node)) # {}
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncRuntimeScalarTypeInvariant,
                NodeQueueNonempty(node)
         PROVE /\ SelectedCommandClass(node) \in AsyncCommandClasses
               /\ CommandClassIndices(
                    node, SelectedCommandClass(node)) # {}
    <2> DEFINE First == asyncNextCommandClass[node]
    <2> DEFINE Second == NextCommandClass(First)
    <2> DEFINE Third == NextCommandClass(Second)
    <2>1. /\ First \in AsyncCommandClasses
           /\ Second \in AsyncCommandClasses
           /\ Third \in AsyncCommandClasses
           /\ {First, Second, Third} = AsyncCommandClasses
      BY <1>1, NextCommandClassCycleFacts, SMT
         DEF AsyncRuntimeScalarTypeInvariant, First, Second, Third
    <2>2. /\ 1 \in 1..Len(asyncCommandQueues[node])
           /\ asyncCommandQueues[node][1].class \in AsyncCommandClasses
      BY <1>1, SMT
         DEF AsyncRuntimeScalarTypeInvariant, NodeQueueNonempty,
             AsyncQueueTyped, AsyncCandidateTyped
    <2>3. \/ CommandClassIndices(node, First) # {}
           \/ CommandClassIndices(node, Second) # {}
           \/ CommandClassIndices(node, Third) # {}
      BY <2>1, <2>2, SMT DEF CommandClassIndices
    <2> QED BY <2>1, <2>3, SMT
         DEF SelectedCommandClass, First, Second, Third
  <1> QED BY <1>1

NaturalIndexBelongs(indices, index) == index \in indices

THEOREM NonemptyNaturalSetHasLeast ==
  \A indices:
    /\ indices # {}
    /\ indices \subseteq Nat
    => \E least \in indices:
         \A other \in indices: least <= other
PROOF
  <1>1. ASSUME NEW indices,
                indices # {},
                indices \subseteq Nat
         PROVE \E least \in indices:
                 \A other \in indices: least <= other
    <2>1. PICK witness \in indices: TRUE
      BY <1>1, FS_EmptySet, Zenon
    <2>2. witness \in Nat
      BY <1>1, <2>1
    <2>3. NaturalIndexBelongs(indices, witness)
      BY <2>1 DEF NaturalIndexBelongs
    <2>4. \E least \in Nat:
             /\ NaturalIndexBelongs(indices, least)
             /\ \A prior \in 0..(least - 1):
                  ~NaturalIndexBelongs(indices, prior)
      BY <2>2, <2>3, SmallestNatural
    <2>5. PICK least \in Nat:
             /\ NaturalIndexBelongs(indices, least)
             /\ \A prior \in 0..(least - 1):
                  ~NaturalIndexBelongs(indices, prior)
      BY <2>4
    <2>6. \A other \in indices: least <= other
      BY <1>1, <2>5, SMT DEF NaturalIndexBelongs
    <2> QED BY <2>5, <2>6 DEF NaturalIndexBelongs
  <1> QED BY <1>1

THEOREM FirstCommandClassIndexIsMember ==
  \A node, commandClass:
    CommandClassIndices(node, commandClass) # {}
      => /\ FirstCommandClassIndex(node, commandClass)
                \in CommandClassIndices(node, commandClass)
         /\ \A other \in CommandClassIndices(node, commandClass):
              FirstCommandClassIndex(node, commandClass) <= other
PROOF
  <1>1. ASSUME NEW node, NEW commandClass,
                CommandClassIndices(node, commandClass) # {}
         PROVE /\ FirstCommandClassIndex(node, commandClass)
                       \in CommandClassIndices(node, commandClass)
               /\ \A other \in CommandClassIndices(node, commandClass):
                    FirstCommandClassIndex(node, commandClass) <= other
    <2>1. CommandClassIndices(node, commandClass) \subseteq Nat
      BY SMT DEF CommandClassIndices
    <2>2. \E least \in CommandClassIndices(node, commandClass):
             \A other \in CommandClassIndices(node, commandClass):
               least <= other
      BY <1>1, <2>1, NonemptyNaturalSetHasLeast
    <2> QED BY <2>2, Zenon DEF FirstCommandClassIndex
  <1> QED BY <1>1

THEOREM NextNodeCommandIndexFacts ==
  \A node \in ValidatorIds:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ NodeQueueNonempty(node)
    => LET index == NextNodeCommandIndex(node)
       IN /\ index \in 1..Len(asyncCommandQueues[node])
          /\ asyncCommandQueues[node][index].class
               = SelectedCommandClass(node)
          /\ \A other \in CommandClassIndices(
                           node, SelectedCommandClass(node)):
               index <= other
          /\ NextNodeCommand(node) = asyncCommandQueues[node][index]
          /\ AsyncCandidateTyped(NextNodeCommand(node))
          /\ NextNodeCommand(node).node = node
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncRuntimeScalarTypeInvariant,
                NodeQueueNonempty(node)
         PROVE LET index == NextNodeCommandIndex(node)
               IN /\ index \in 1..Len(asyncCommandQueues[node])
                  /\ asyncCommandQueues[node][index].class
                       = SelectedCommandClass(node)
                  /\ \A other \in CommandClassIndices(
                                   node, SelectedCommandClass(node)):
                       index <= other
                  /\ NextNodeCommand(node)
                       = asyncCommandQueues[node][index]
                  /\ AsyncCandidateTyped(NextNodeCommand(node))
                  /\ NextNodeCommand(node).node = node
    <2> DEFINE Class == SelectedCommandClass(node)
    <2> DEFINE Index == NextNodeCommandIndex(node)
    <2>1. /\ Class \in AsyncCommandClasses
           /\ CommandClassIndices(node, Class) # {}
      BY <1>1, SelectedCommandClassFacts DEF Class
    <2>2. /\ Index \in CommandClassIndices(node, Class)
           /\ \A other \in CommandClassIndices(node, Class):
                Index <= other
      BY <2>1, FirstCommandClassIndexIsMember
         DEF Index, NextNodeCommandIndex
    <2>3. /\ Index \in 1..Len(asyncCommandQueues[node])
           /\ asyncCommandQueues[node][Index].class = Class
      BY <2>2 DEF CommandClassIndices
    <2>4. /\ AsyncCandidateTyped(asyncCommandQueues[node][Index])
           /\ asyncCommandQueues[node][Index].node = node
      BY <1>1, <2>3, SMT
         DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
             AsyncCommandQueueOwnership, SequenceSet
    <2> QED BY <2>2, <2>3, <2>4
         DEF Index, Class, NextNodeCommandIndex, NextNodeCommand
  <1> QED BY <1>1

THEOREM SequenceWithoutIndexFacts ==
  \A sequence, index:
    /\ sequence \in Seq(Range(sequence))
    /\ index \in 1..Len(sequence)
    => LET result == SequenceWithoutIndex(sequence, index)
       IN /\ result \in Seq(Range(sequence))
          /\ Len(result) = Len(sequence) - 1
          /\ DOMAIN result = 1..Len(result)
          /\ \A resultIndex \in 1..Len(result):
               result[resultIndex] =
                 IF resultIndex < index
                 THEN sequence[resultIndex]
                 ELSE sequence[resultIndex + 1]
          /\ Range(result) \subseteq Range(sequence)
PROOF
  <1>1. ASSUME NEW sequence, NEW index,
                sequence \in Seq(Range(sequence)),
                index \in 1..Len(sequence)
         PROVE LET result == SequenceWithoutIndex(sequence, index)
               IN /\ result \in Seq(Range(sequence))
                  /\ Len(result) = Len(sequence) - 1
                  /\ DOMAIN result = 1..Len(result)
                  /\ \A resultIndex \in 1..Len(result):
                       result[resultIndex] =
                         IF resultIndex < index
                         THEN sequence[resultIndex]
                         ELSE sequence[resultIndex + 1]
                  /\ Range(result) \subseteq Range(sequence)
    <2> DEFINE Prefix == SubSeq(sequence, 1, index - 1)
    <2> DEFINE Suffix == SubSeq(sequence, index + 1, Len(sequence))
    <2> DEFINE Result == Prefix \o Suffix
    <2>1. /\ Prefix \in Seq(Range(sequence))
           /\ Len(Prefix) = index - 1
           /\ \A prefixIndex \in 1..Len(Prefix):
                Prefix[prefixIndex] = sequence[prefixIndex]
      BY <1>1, SubSeqProperties, SMT DEF Prefix
    <2>2. /\ Suffix \in Seq(Range(sequence))
           /\ Len(Suffix) = Len(sequence) - index
           /\ \A suffixIndex \in 1..Len(Suffix):
                Suffix[suffixIndex] = sequence[index + suffixIndex]
      BY <1>1, SubSeqProperties, SMT DEF Suffix
    <2>3. /\ Result \in Seq(Range(sequence))
           /\ Len(Result) = Len(sequence) - 1
           /\ DOMAIN Result = 1..Len(Result)
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < index
                  THEN sequence[resultIndex]
                  ELSE sequence[resultIndex + 1]
      BY <1>1, <2>1, <2>2, ConcatProperties, SMT DEF Result
    <2>4. Range(Result) \subseteq Range(sequence)
      BY <2>3, RangeEquality, SMT
    <2> QED BY <2>3, <2>4
         DEF Result, SequenceWithoutIndex, Prefix, Suffix
  <1> QED BY <1>1

THEOREM TypedOwnedSequenceWithoutIndexFacts ==
  \A node, sequence, index:
    /\ AsyncQueueTyped(sequence)
    /\ AsyncCommandQueueOwnership(node, sequence)
    /\ index \in 1..Len(sequence)
    => /\ AsyncQueueTyped(SequenceWithoutIndex(sequence, index))
       /\ AsyncCommandQueueOwnership(
            node, SequenceWithoutIndex(sequence, index))
       /\ Len(SequenceWithoutIndex(sequence, index))
            = Len(sequence) - 1
PROOF
  <1>1. ASSUME NEW node, NEW sequence, NEW index,
                AsyncQueueTyped(sequence),
                AsyncCommandQueueOwnership(node, sequence),
                index \in 1..Len(sequence)
         PROVE /\ AsyncQueueTyped(
                      SequenceWithoutIndex(sequence, index))
               /\ AsyncCommandQueueOwnership(
                    node, SequenceWithoutIndex(sequence, index))
               /\ Len(SequenceWithoutIndex(sequence, index))
                    = Len(sequence) - 1
    <2> DEFINE Result == SequenceWithoutIndex(sequence, index)
    <2>1. /\ sequence \in Seq(Range(sequence))
           /\ Result \in Seq(Range(sequence))
           /\ Len(Result) = Len(sequence) - 1
           /\ DOMAIN Result = 1..Len(Result)
           /\ Range(Result) \subseteq Range(sequence)
      BY <1>1, SequenceWithoutIndexFacts DEF AsyncQueueTyped, Result
    <2>2. /\ SequenceSet(sequence) = Range(sequence)
           /\ SequenceSet(Result) = Range(Result)
      BY <2>1, RangeEquality DEF SequenceSet
    <2>3. SequenceSet(Result) \subseteq SequenceSet(sequence)
      BY <2>1, <2>2
    <2>4. \A candidate \in SequenceSet(Result):
             AsyncCandidateTyped(candidate)
      <3>1. ASSUME NEW candidate \in SequenceSet(Result)
             PROVE AsyncCandidateTyped(candidate)
        <4>1. candidate \in SequenceSet(sequence)
          BY <2>3, <3>1
        <4> QED BY <1>1, <4>1 DEF AsyncQueueTyped
      <3> QED BY <3>1
    <2>5. \A candidate \in SequenceSet(Result): candidate.node = node
      <3>1. ASSUME NEW candidate \in SequenceSet(Result)
             PROVE candidate.node = node
        <4>1. candidate \in SequenceSet(sequence)
          BY <2>3, <3>1
        <4> QED BY <1>1, <4>1 DEF AsyncCommandQueueOwnership
      <3> QED BY <3>1
    <2>6. Result \in Seq(Range(Result))
      BY <2>1, SeqOfRange
    <2> QED BY <2>1, <2>4, <2>5, <2>6
         DEF Result, AsyncQueueTyped, AsyncCommandQueueOwnership,
             SequenceSet
  <1> QED BY <1>1

(***************************************************************************
The shared ingress corridor has two serialized-runtime surfaces.  The ordinary
surface is enabled only without a physical ingress barrier.  The predecessor
interleave uses the same Runtime body for one strictly older lifecycle while
freezing every Serve/I/O owner.  Leader-wire and ordinary ingress owners are
selected by the enclosing shared physical barrier.  This union is an
action-shape helper only: it does not classify the interleave as protocol
progress and carries no fairness assumption.
***************************************************************************)
SerializedRunnerRuntimeStep(node) ==
  \/ SerializedRuntimeStep(node)
  \/ SerializedRuntimePrecedesServeIngressStep(node)
  \/ AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)

THEOREM SerializedRuntimePrecedesServeIngressExactFrame ==
  \A node \in ValidatorIds:
    SerializedRuntimePrecedesServeIngressStep(node)
      => /\ asyncRunnerPhase[node] = "Runtime"
         /\ asyncRunnerPhase'[node] = "Local"
         /\ RuntimeStep(node)
         /\ asyncIoQueues' = asyncIoQueues
         /\ asyncNextServeAdmissionOrdinal' =
              asyncNextServeAdmissionOrdinal
         /\ asyncNextServeIngressOrdinal' =
              asyncNextServeIngressOrdinal
         /\ asyncServeIngressAdmissions' =
              asyncServeIngressAdmissions
         /\ AsyncServeIngressLifecycleOwnerIdentities(node)' =
              AsyncServeIngressLifecycleOwnerIdentities(node)
         /\ AsyncIngressSchedulerBarrierActive(node)
         /\ (AsyncServeIngressLifecycleOwnerIdentities(node) # {}
               => AsyncServeEarliestIngressSchedulerOrdinal(node)' =
                    AsyncServeEarliestIngressSchedulerOrdinal(node))
         /\ asyncServeAdmissions' = asyncServeAdmissions
         /\ asyncServeReservations' = asyncServeReservations
         /\ asyncServeTombstones' = asyncServeTombstones
BY Isa
   DEF SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncServeEarliestIngressSchedulerOrdinal,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncIoVars, AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars

THEOREM SerializedRuntimePrecedesServeIngressRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      SerializedRuntimePrecedesServeIngressStep(node) => [Next]_vars
BY RuntimeStepRefinesCoreBracketNext
   DEF SerializedRuntimePrecedesServeIngressStep

THEOREM SerializedRunnerRuntimeRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      SerializedRunnerRuntimeStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 SerializedRunnerRuntimeStep(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds,
                  SerializedRunnerRuntimeStep(node)
           PROVE [Next]_vars
      <3>1. CASE SerializedRuntimeStep(node)
        BY <1>1, <2>1, <3>1,
           SerializedRuntimeStepRefinesCoreBracketNext
      <3>2. CASE SerializedRuntimePrecedesServeIngressStep(node)
        BY <1>1, <2>1, <3>2,
           SerializedRuntimePrecedesServeIngressRefinesCoreBracketNext
      <3>3. CASE
                AsyncCandidateProducerContinuationExactRuntimeReplayStep(
                  node)
        <4>1. CASE /\ DeferredWorkOwnsRuntimeTurn(node)
                      /\ DeferredDrainStep(node)
          BY <1>1, <2>1, <3>3, <4>1,
             DeferredDrainStepRefinesCoreBracketNext
        <4>2. CASE /\ ~DeferredWorkOwnsRuntimeTurn(node)
                      /\ FifoRuntimeStep(node)
          BY <1>1, <2>1, <3>3, <4>2,
             FifoRuntimeStepRefinesCoreBracketNext
        <4> QED BY <3>3, <4>1, <4>2
             DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep
      <3> QED BY <2>1, <3>1, <3>2, <3>3
           DEF SerializedRunnerRuntimeStep
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM SerializedLocalPrecedesServeIngressRefinesCoreBracketNext ==
  \A node:
    SerializedLocalPrecedesServeIngressStep(node) => [Next]_vars
BY CoreStutterRefinesBracketNext, Isa
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead

THEOREM AsyncServeIngressTargetOnlyRefinesCoreBracketNext ==
  \A node:
    AsyncServeIngressTargetOnlyTurn(node) => [Next]_vars
BY CoreStutterRefinesBracketNext, Isa
   DEF AsyncServeIngressTargetOnlyTurn

THEOREM CandidateProducerContinuationReplayRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      ReplayRunNodeCandidateProducerContinuation(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 ReplayRunNodeCandidateProducerContinuation(node)
                   => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds,
                  ReplayRunNodeCandidateProducerContinuation(node)
           PROVE [Next]_vars
      <3>1. CASE
                AsyncCandidateProducerContinuationExactLocalReplayStep(
                  node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>1, Isa
             DEF ReplayRunNodeCandidateProducerContinuation,
                 AsyncCandidateProducerContinuationExactLocalReplayStep,
                 EnqueueCandidate
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>2. CASE
                AsyncCandidateProducerContinuationReplayTargetOnlyTurn(
                  node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>2
             DEF ReplayRunNodeCandidateProducerContinuation,
                 AsyncCandidateProducerContinuationReplayTargetOnlyTurn
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>3. CASE
                AsyncCandidateProducerContinuationExactRuntimeReplayStep(
                  node)
        <4>1. SerializedRunnerRuntimeStep(node)
          BY <3>3 DEF SerializedRunnerRuntimeStep
        <4> QED BY <1>1, <2>1, <4>1,
             SerializedRunnerRuntimeRefinesCoreBracketNext
      <3> QED BY <2>1, <3>1, <3>2, <3>3
           DEF ReplayRunNodeCandidateProducerContinuation
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
`AsyncIoVars` freezes the complete logical/physical Serve ticket.  Candidate,
timeout, and periodic retransmit lifecycle bookkeeping is updated by the
outer control-service transition, however, and shares the same scheduler
ordinal source.  Monotone high-watermark allocation is therefore part of the
type frame rather than an implicit I/O stutter premise.
***************************************************************************)
THEOREM FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType ==
  /\ AsyncSharedSchedulerOrdinalInjectionInvariant
  /\ AsyncServeOrdinalInvariant
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncControlServiceSlotTransition
  /\ UNCHANGED <<AsyncServeLifecycleVars,
                  AsyncServeIngressAdmissionVars>>
  => /\ AsyncSharedSchedulerOrdinalInjectionInvariant'
     /\ AsyncServeOrdinalInvariant'
BY AsyncSharedSchedulerHighWatermarkIsMonotone, IsaT(900)
   DEF AsyncSharedSchedulerOrdinalInjectionInvariant,
       AsyncServeOrdinalInvariant,
       AsyncControlServiceSlotTransition,
       AsyncNextCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleAdmissions,
       AsyncTimeoutLifecycleOwned, AsyncTimeoutLifecycleOrdinal,
       AsyncRetransmitLifecycleOwned,
       AsyncRetransmitLifecycleOrdinal,
       AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars

THEOREM SerializedRuntimePreservesScalarType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncRuntimeScalarTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                RunNodeWork(node),
                SerializedRunnerRuntimeStep(node)
         PROVE AsyncRuntimeScalarTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. /\ AsyncRuntimeScalarTypeInvariant
           /\ DOMAIN asyncCommandQueues = ValidatorIds
           /\ \A other \in ValidatorIds:
                /\ AsyncQueueTyped(asyncCommandQueues[other])
                /\ AsyncCommandQueueOwnership(
                     other, asyncCommandQueues[other])
           /\ asyncNextCommandClass
                \in [ValidatorIds -> AsyncCommandClasses]
           /\ asyncFifoOwed \in [ValidatorIds -> BOOLEAN]
           /\ asyncTimeoutEmitted \in [ValidatorIds -> BOOLEAN]
           /\ asyncRunnerPhase \in
                [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget \in
                [ValidatorIds ->
                  0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>3. /\ "Local" \in {"Local", "Ingress", "Runtime"}
           /\ AsyncQueueCapacity \in
                0..(AsyncQueueCapacity + AsyncIngressCapacity)
      BY <2>2, SMT DEF AsyncRuntimeScalarTypeInvariant,
                         AsyncConfiguration
    <2>4. /\ asyncRunnerPhase' \in
                [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget' \in
                [ValidatorIds ->
                  0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1, <2>1, <2>2, <2>3,
         FunctionalUpdatePreservesType
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncCandidateProducerContinuationExactRuntimeReplayStep
    <2>5. \/ /\ asyncCommandQueues' = asyncCommandQueues
                 /\ asyncNextCommandClass' = asyncNextCommandClass
           \/ /\ NodeQueueNonempty(node)
                 /\ asyncCommandQueues' =
                      [asyncCommandQueues EXCEPT
                         ![node] = SequenceWithoutIndex(
                           @, NextNodeCommandIndex(node))]
                 /\ asyncNextCommandClass' =
                      [asyncNextCommandClass EXCEPT
                         ![node] = NextCommandClass(
                           SelectedCommandClass(node))]
      BY <1>1, Isa
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             RuntimeStep, DeferredDrainStep,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DirectTimeoutStep,
             DirectRetransmitStep, FifoRuntimeStep, IdleRuntimeStep,
             RemoveNextNodeCommand, DeferCommand, DiscardCommand,
             AsyncDeferredVars, vars
    <2>6. /\ DOMAIN asyncCommandQueues' = ValidatorIds
           /\ \A other \in ValidatorIds:
                /\ AsyncQueueTyped(asyncCommandQueues'[other])
                /\ AsyncCommandQueueOwnership(
                     other, asyncCommandQueues'[other])
      <3>1. CASE /\ asyncCommandQueues' = asyncCommandQueues
                    /\ asyncNextCommandClass' = asyncNextCommandClass
        BY <2>2, <3>1
      <3>2. CASE /\ NodeQueueNonempty(node)
                    /\ asyncCommandQueues' =
                         [asyncCommandQueues EXCEPT
                            ![node] = SequenceWithoutIndex(
                              @, NextNodeCommandIndex(node))]
                    /\ asyncNextCommandClass' =
                         [asyncNextCommandClass EXCEPT
                            ![node] = NextCommandClass(
                              SelectedCommandClass(node))]
        <4>1. /\ NextNodeCommandIndex(node)
                     \in 1..Len(asyncCommandQueues[node])
               /\ AsyncQueueTyped(
                    SequenceWithoutIndex(
                      asyncCommandQueues[node],
                      NextNodeCommandIndex(node)))
               /\ AsyncCommandQueueOwnership(
                    node,
                    SequenceWithoutIndex(
                      asyncCommandQueues[node],
                      NextNodeCommandIndex(node)))
          BY <2>1, <2>2, <3>2, NextNodeCommandIndexFacts,
             TypedOwnedSequenceWithoutIndexFacts
        <4>2. \A other \in ValidatorIds:
                 /\ AsyncQueueTyped(asyncCommandQueues'[other])
                 /\ AsyncCommandQueueOwnership(
                      other, asyncCommandQueues'[other])
          <5>1. ASSUME NEW other \in ValidatorIds
                 PROVE /\ AsyncQueueTyped(asyncCommandQueues'[other])
                       /\ AsyncCommandQueueOwnership(
                            other, asyncCommandQueues'[other])
            <6>1. CASE other = node
              BY <3>2, <4>1, <6>1, Isa
            <6>2. CASE other # node
              BY <2>2, <3>2, <5>1, <6>2,
                 FunctionalUpdateAwayFromKey
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <2>2, <3>2, <4>2, Isa
      <3> QED BY <2>5, <3>1, <3>2
    <2>7. asyncNextCommandClass'
              \in [ValidatorIds -> AsyncCommandClasses]
      <3>1. CASE /\ asyncCommandQueues' = asyncCommandQueues
                    /\ asyncNextCommandClass' = asyncNextCommandClass
        BY <2>2, <3>1
      <3>2. CASE /\ NodeQueueNonempty(node)
                    /\ asyncCommandQueues' =
                         [asyncCommandQueues EXCEPT
                            ![node] = SequenceWithoutIndex(
                              @, NextNodeCommandIndex(node))]
                    /\ asyncNextCommandClass' =
                         [asyncNextCommandClass EXCEPT
                            ![node] = NextCommandClass(
                              SelectedCommandClass(node))]
        <4>1. /\ SelectedCommandClass(node) \in AsyncCommandClasses
               /\ NextCommandClass(SelectedCommandClass(node))
                    \in AsyncCommandClasses
          BY <2>1, <2>2, <3>2, SelectedCommandClassFacts,
             NextCommandClassCycleFacts
        <4> QED BY <2>2, <3>2, <4>1,
             FunctionalUpdatePreservesType
      <3> QED BY <2>5, <3>1, <3>2
    <2>8. /\ asyncFifoOwed' \in [ValidatorIds -> BOOLEAN]
           /\ asyncTimeoutEmitted' \in [ValidatorIds -> BOOLEAN]
      BY <1>1, <2>1, <2>2,
         FunctionalUpdatePreservesType, SMTT(120)
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             RuntimeStep, DeferredDrainStep,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DirectTimeoutStep,
             DirectRetransmitStep, FifoRuntimeStep, IdleRuntimeStep,
             NodeQueueNonempty, AsyncDeferredVars, vars
    <2>9. /\ asyncNow' \in Nat
           /\ asyncCausalAdmissionOwed'
                \in [ValidatorIds -> BOOLEAN]
           /\ asyncNextLocalSource'
                \in [ValidatorIds -> AsyncLocalSources]
      BY <1>1, <2>2, Isa
         DEF RunNodeWork, SerializedRunnerRuntimeStep,
             SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             AsyncLocalAdmissionVars,
             AsyncRuntimeScalarTypeInvariant
    <2> QED BY <2>2, <2>4, <2>6, <2>7, <2>8, <2>9
         DEF AsyncRuntimeScalarTypeInvariant
  <1> QED BY <1>1

THEOREM NonemptyTypedQueueHeadIsTyped ==
  \A queue:
    AsyncQueueTyped(queue) /\ Len(queue) > 0
      => AsyncCandidateTyped(Head(queue))
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0
         PROVE AsyncCandidateTyped(Head(queue))
    <2>1. /\ 1 \in 1..Len(queue)
           /\ AsyncCandidateTyped(queue[1])
      BY <1>1, SMT DEF AsyncQueueTyped
    <2>2. Head(queue) = queue[1]
      BY <1>1, NonemptySequenceHeadIsFirst DEF AsyncQueueTyped
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM NonemptyOwnedTypedQueueHeadFacts ==
  \A node, queue:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCommandQueueOwnership(node, queue)
    /\ Len(queue) > 0
    => /\ AsyncCandidateTyped(Head(queue))
       /\ Head(queue).node = node
BY NonemptyTypedQueueHeadIsTyped, NonemptySequenceHeadIsFirst, SMT
   DEF AsyncQueueTyped, AsyncCommandQueueOwnership, SequenceSet

THEOREM SelectedDeferredQueueFacts ==
  \A node \in ValidatorIds:
    AsyncDeferredTypeInvariant
      => /\ SelectedDeferredClass(node) \in AsyncCommandClasses
         /\ (DeferredQueueNonempty(node)
               => /\ DeferredClassNonempty(
                        node, SelectedDeferredClass(node))
                  /\ AsyncQueueTyped(
                       DeferredClassQueue(
                         node, SelectedDeferredClass(node)))
                  /\ AsyncCommandQueueOwnership(
                       node,
                       DeferredClassQueue(
                         node, SelectedDeferredClass(node))))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncDeferredTypeInvariant
         PROVE /\ SelectedDeferredClass(node)
                      \in AsyncCommandClasses
               /\ (DeferredQueueNonempty(node)
                     => /\ DeferredClassNonempty(
                              node, SelectedDeferredClass(node))
                        /\ AsyncQueueTyped(
                             DeferredClassQueue(
                               node, SelectedDeferredClass(node)))
                        /\ AsyncCommandQueueOwnership(
                             node,
                             DeferredClassQueue(
                               node, SelectedDeferredClass(node))))
    <2>1. /\ asyncNextDeferredClass[node]
                  \in AsyncCommandClasses
           /\ AsyncCompletionSequenceTyped(
                asyncDeferredCompletionQueues[node])
           /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
           /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncDeferredCompletionQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncDeferredProgressQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncDeferredNormalQueues[node])
      BY <1>1, FunctionValueHasCodomain
         DEF AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant,
             AsyncDeferredContentTypeInvariant
    <2>2. /\ SelectedDeferredClass(node) \in AsyncCommandClasses
           /\ (DeferredQueueNonempty(node)
                 => DeferredClassNonempty(
                      node, SelectedDeferredClass(node)))
      BY <2>1, SMTT(30)
         DEF DeferredQueueNonempty, DeferredClassQueue,
             DeferredClassNonempty, SelectedDeferredClass,
             NextCommandClass, AsyncCommandClasses
    <2>3. CASE SelectedDeferredClass(node) = "Completion"
      BY <2>1, <2>2, <2>3, Isa
         DEF DeferredClassQueue, AsyncCompletionSequenceTyped,
             AsyncQueueTyped
    <2>4. CASE SelectedDeferredClass(node) = "Progress"
      BY <2>1, <2>2, <2>4 DEF DeferredClassQueue
    <2>5. CASE SelectedDeferredClass(node) = "Normal"
      BY <2>1, <2>2, <2>5 DEF DeferredClassQueue
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF AsyncCommandClasses
  <1> QED BY <1>1

THEOREM AdvanceNextDeferredClassPreservesCursorType ==
  \A node \in ValidatorIds:
    /\ AsyncDeferredTypeInvariant
    /\ AdvanceNextDeferredClass(node)
    => asyncNextDeferredClass'
         \in [ValidatorIds -> AsyncCommandClasses]
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncDeferredTypeInvariant,
                AdvanceNextDeferredClass(node)
         PROVE asyncNextDeferredClass'
                 \in [ValidatorIds -> AsyncCommandClasses]
    <2>1. /\ asyncNextDeferredClass
                  \in [ValidatorIds -> AsyncCommandClasses]
           /\ SelectedDeferredClass(node) \in AsyncCommandClasses
      BY <1>1, SelectedDeferredQueueFacts
         DEF AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant
    <2>2. NextCommandClass(SelectedDeferredClass(node))
             \in AsyncCommandClasses
      BY <2>1, SMT
         DEF AsyncCommandClasses, NextCommandClass
    <2> QED BY <1>1, <2>1, <2>2,
         FunctionalUpdatePreservesType
         DEF AdvanceNextDeferredClass
  <1> QED BY <1>1


THEOREM RuntimeSelectedCommandsAreTyped ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant
      => /\ (NodeQueueNonempty(node)
                => /\ AsyncCandidateTyped(NextNodeCommand(node))
                   /\ NextNodeCommand(node).node = node)
         /\ (DeferredQueueNonempty(node)
                => /\ AsyncCandidateTyped(NextDeferredCommand(node))
                   /\ NextDeferredCommand(node).node = node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant
         PROVE /\ (NodeQueueNonempty(node)
                       => /\ AsyncCandidateTyped(NextNodeCommand(node))
                          /\ NextNodeCommand(node).node = node)
               /\ (DeferredQueueNonempty(node)
                       => /\ AsyncCandidateTyped(NextDeferredCommand(node))
                          /\ NextDeferredCommand(node).node = node)
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncCommandQueues[node])
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>2. NodeQueueNonempty(node)
             => /\ AsyncCandidateTyped(NextNodeCommand(node))
                /\ NextNodeCommand(node).node = node
      BY <1>1, <2>1, NextNodeCommandIndexFacts
    <2>3. /\ AsyncCompletionSequenceTyped(
                    asyncDeferredCompletionQueues[node])
           /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
           /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncDeferredCompletionQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncDeferredProgressQueues[node])
           /\ AsyncCommandQueueOwnership(
                node, asyncDeferredNormalQueues[node])
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncDeferredTypeInvariant,
             AsyncDeferredContentTypeInvariant
    <2>4. DeferredQueueNonempty(node)
             => /\ AsyncCandidateTyped(NextDeferredCommand(node))
                /\ NextDeferredCommand(node).node = node
      BY <1>1, <2>3, SelectedDeferredQueueFacts,
         NonemptyOwnedTypedQueueHeadFacts
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             DeferredClassNonempty, NextDeferredCommand
    <2> QED BY <2>2, <2>4
  <1> QED BY <1>1

THEOREM DeferredProgressAfterPreservesOwnedType ==
  \A node \in ValidatorIds:
    \A command:
      /\ AsyncDeferredContentTypeInvariant
      /\ AsyncCandidateTyped(command)
      /\ command.node = node
      /\ command.class = "Progress"
      => LET updated == DeferredProgressAfter(node, command)
         IN /\ AsyncQueueTyped(updated)
            /\ AsyncCommandQueueOwnership(node, updated)
            /\ \A candidate \in SequenceSet(updated):
                 candidate.class = "Progress"
            /\ Len(updated) <= AsyncDeferredProgressCapacity
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW command,
                AsyncDeferredContentTypeInvariant,
                AsyncCandidateTyped(command),
                command.node = node,
                command.class = "Progress"
         PROVE LET updated == DeferredProgressAfter(node, command)
               IN /\ AsyncQueueTyped(updated)
                  /\ AsyncCommandQueueOwnership(node, updated)
                  /\ \A candidate \in SequenceSet(updated):
                       candidate.class = "Progress"
                  /\ Len(updated) <= AsyncDeferredProgressCapacity
    <2> DEFINE Queue == asyncDeferredProgressQueues[node]
    <2>1. /\ AsyncQueueTyped(Queue)
           /\ AsyncCommandQueueOwnership(node, Queue)
           /\ \A candidate \in SequenceSet(Queue):
                candidate.class = "Progress"
           /\ Len(Queue) <= AsyncDeferredProgressCapacity
      BY <1>1 DEF AsyncDeferredContentTypeInvariant, Queue
    <2>2. CASE command \in SequenceSet(Queue)
      BY <2>1, <2>2 DEF DeferredProgressAfter, Queue
    <2>3. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) # {}
      BY <2>1, <2>3 DEF DeferredProgressAfter, Queue
    <2>4. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) < AsyncDeferredProgressCapacity
      <3>1. /\ AsyncQueueTyped(Append(Queue, command))
             /\ AsyncCommandQueueOwnership(
                  node, Append(Queue, command))
        BY <1>1, <2>1, TypedCandidateAppendPreservesQueueType,
           AppendOwnedCandidatePreservesCommandQueueOwnership
      <3>2. \A candidate \in SequenceSet(Append(Queue, command)):
               candidate.class = "Progress"
        BY <1>1, <2>1, SequenceSetAfterAppend, SMT
           DEF AsyncQueueTyped
      <3>3. Len(Append(Queue, command)) <=
               AsyncDeferredProgressCapacity
        BY <2>1, <2>4, AppendSequenceFacts, SMT
           DEF AsyncQueueTyped
      <3>4. DeferredProgressAfter(node, command) = Append(Queue, command)
        BY <2>4 DEF DeferredProgressAfter, Queue
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2>5. CASE /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) >= AsyncDeferredProgressCapacity
      BY <2>1, <2>5 DEF DeferredProgressAfter, Queue
    <2>6. \/ command \in SequenceSet(Queue)
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) # {}
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) < AsyncDeferredProgressCapacity
           \/ /\ command \notin SequenceSet(Queue)
                 /\ SameProtectedProgressSlotIndices(node, command) = {}
                 /\ Len(Queue) >= AsyncDeferredProgressCapacity
      BY SMT
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM OwnedClassQueueTailPreservesFacts ==
  \A node, queue, commandClass:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCommandQueueOwnership(node, queue)
    /\ \A candidate \in SequenceSet(queue):
         candidate.class = commandClass
    /\ Len(queue) > 0
    => /\ AsyncQueueTyped(Tail(queue))
       /\ AsyncCommandQueueOwnership(node, Tail(queue))
       /\ \A candidate \in SequenceSet(Tail(queue)):
            candidate.class = commandClass
       /\ Len(Tail(queue)) <= Len(queue)
BY TypedQueueTailFacts,
   OwnedTypedQueueTailPreservesCommandQueueOwnership, SMT

THEOREM TypedCompletionTailPreservesSequenceType ==
  \A queue:
    /\ AsyncCompletionSequenceTyped(queue)
    /\ Len(queue) > 0
    => AsyncCompletionSequenceTyped(Tail(queue))
PROOF
  <1>1. ASSUME NEW queue,
                AsyncCompletionSequenceTyped(queue),
                Len(queue) > 0
         PROVE AsyncCompletionSequenceTyped(Tail(queue))
    <2>1. /\ queue \in Seq(Range(queue))
           /\ queue # <<>>
      BY <1>1, EmptySeq, SMT DEF AsyncCompletionSequenceTyped
    <2>2. /\ Tail(queue) \in Seq(Range(queue))
           /\ Range(Tail(queue)) \subseteq Range(queue)
      BY <2>1, HeadTailProperties
    <2>3. /\ Tail(queue) \in Seq(Range(Tail(queue)))
           /\ DOMAIN Tail(queue) = 1..Len(Tail(queue))
      BY <2>2, SeqOfRange, LenProperties
    <2>4. \A index \in 1..Len(Tail(queue)):
             /\ AsyncCandidateTyped(Tail(queue)[index])
             /\ Tail(queue)[index].class = "Completion"
      <3>1. ASSUME NEW index \in 1..Len(Tail(queue))
             PROVE /\ AsyncCandidateTyped(Tail(queue)[index])
                   /\ Tail(queue)[index].class = "Completion"
        <4>1. Tail(queue)[index] \in Range(Tail(queue))
          BY <2>3, <3>1, RangeEquality
        <4>2. Tail(queue)[index] \in Range(queue)
          BY <2>2, <4>1
        <4>3. PICK original \in 1..Len(queue):
                 Tail(queue)[index] = queue[original]
          BY <1>1, <4>2, RangeEquality
             DEF AsyncCompletionSequenceTyped
        <4> QED BY <1>1, <4>3 DEF AsyncCompletionSequenceTyped
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4 DEF AsyncCompletionSequenceTyped
  <1> QED BY <1>1


THEOREM DeferTypedOwnedCommandPreservesDeferredContentType ==
  \A command:
    /\ AsyncDeferredContentTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ DeferCommand(command)
    => AsyncDeferredContentTypeInvariant'
BY DeferredProgressAfterPreservesOwnedType,
   TypedCandidateAppendPreservesQueueType,
   TypedCompletionAppendPreservesSequenceType,
   AppendOwnedCandidatePreservesCommandQueueOwnership,
   SequenceSetAfterAppend, AppendSequenceFacts,
   FunctionalUpdateAwayFromKey, FunctionalAppendUpdateAtKey,
   SMTT(120)
   DEF DeferCommand, AsyncDeferredContentTypeInvariant,
       AsyncCompletionSequenceTyped, AsyncQueueTyped,
       AsyncCommandQueueOwnership, SequenceSet

THEOREM DeferredCapacityWeakOrderTransitive ==
  \A low, middle, high \in Nat:
    low <= middle /\ middle <= high => low <= high
BY SMT

THEOREM TypedDeferredQueueLengthIsNatural ==
  \A queue:
    AsyncQueueTyped(queue) => Len(queue) \in Nat
BY LenProperties DEF AsyncQueueTyped

THEOREM ConfiguredDeferredCapacitiesAreNatural ==
  AsyncConfiguration
    => /\ AsyncDeferredProgressCapacity \in Nat
       /\ AsyncDeferredNormalCapacity \in Nat
BY DEF AsyncConfiguration

THEOREM RemoveNextDeferredCommandPreservesDeferredContentType ==
  \A node \in ValidatorIds:
    /\ AsyncConfiguration
    /\ AsyncDeferredTypeInvariant
    /\ DeferredQueueNonempty(node)
    /\ RemoveNextDeferredCommand(node)
    => AsyncDeferredContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncConfiguration,
                AsyncDeferredTypeInvariant,
                DeferredQueueNonempty(node),
                RemoveNextDeferredCommand(node)
         PROVE AsyncDeferredContentTypeInvariant'
    <2>1. /\ AsyncDeferredContentTypeInvariant
           /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
           /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
           /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
      BY <1>1 DEF AsyncDeferredTypeInvariant,
                    AsyncDeferredTopologyTypeInvariant
    <2>2. /\ SelectedDeferredClass(node) \in AsyncCommandClasses
           /\ DeferredClassNonempty(
                node, SelectedDeferredClass(node))
      BY <1>1, SelectedDeferredQueueFacts
    <2>3. ASSUME NEW other \in ValidatorIds
           PROVE /\ AsyncCompletionSequenceTyped(
                        asyncDeferredCompletionQueues'[other])
                 /\ AsyncQueueTyped(
                      asyncDeferredProgressQueues'[other])
                 /\ AsyncQueueTyped(asyncDeferredNormalQueues'[other])
                 /\ AsyncCommandQueueOwnership(
                      other, asyncDeferredCompletionQueues'[other])
                 /\ AsyncCommandQueueOwnership(
                      other, asyncDeferredProgressQueues'[other])
                 /\ AsyncCommandQueueOwnership(
                      other, asyncDeferredNormalQueues'[other])
                 /\ (\A progressCandidate \in SequenceSet(
                         asyncDeferredProgressQueues'[other]):
                       progressCandidate.class = "Progress")
                 /\ (\A normalCandidate \in SequenceSet(
                         asyncDeferredNormalQueues'[other]):
                       normalCandidate.class = "Normal")
                 /\ Len(asyncDeferredProgressQueues'[other]) <=
                      AsyncDeferredProgressCapacity
                 /\ Len(asyncDeferredNormalQueues'[other]) <=
                      AsyncDeferredNormalCapacity
      <3>1. /\ AsyncCompletionSequenceTyped(
                     asyncDeferredCompletionQueues[other])
             /\ AsyncQueueTyped(asyncDeferredProgressQueues[other])
             /\ AsyncQueueTyped(asyncDeferredNormalQueues[other])
             /\ AsyncCommandQueueOwnership(
                  other, asyncDeferredCompletionQueues[other])
             /\ AsyncCommandQueueOwnership(
                  other, asyncDeferredProgressQueues[other])
             /\ AsyncCommandQueueOwnership(
                  other, asyncDeferredNormalQueues[other])
             /\ (\A progressCandidate \in SequenceSet(
                     asyncDeferredProgressQueues[other]):
                   progressCandidate.class = "Progress")
             /\ (\A normalCandidate \in SequenceSet(
                     asyncDeferredNormalQueues[other]):
                   normalCandidate.class = "Normal")
             /\ Len(asyncDeferredProgressQueues[other]) <=
                  AsyncDeferredProgressCapacity
             /\ Len(asyncDeferredNormalQueues[other]) <=
                  AsyncDeferredNormalCapacity
        BY <2>1, <2>3 DEF AsyncDeferredContentTypeInvariant
      <3>2. CASE other = node
        <4>1. CASE SelectedDeferredClass(node) = "Completion"
          <5>1. /\ asyncDeferredCompletionQueues'[node] =
                        Tail(asyncDeferredCompletionQueues[node])
                 /\ asyncDeferredProgressQueues' =
                        asyncDeferredProgressQueues
                 /\ asyncDeferredNormalQueues' =
                        asyncDeferredNormalQueues
            BY <1>1, <2>1, <4>1, FunctionalTailUpdateAtKey, Isa
               DEF RemoveNextDeferredCommand,
                   AdvanceNextDeferredClass,
                   AsyncDeferredTopologyTypeInvariant
          <5>2. AsyncCompletionSequenceTyped(
                   Tail(asyncDeferredCompletionQueues[node]))
            BY <2>2, <3>1, <3>2, <4>1,
               TypedCompletionTailPreservesSequenceType
               DEF DeferredClassNonempty, DeferredClassQueue
          <5>3. AsyncCommandQueueOwnership(
                   node, Tail(asyncDeferredCompletionQueues[node]))
            BY <2>2, <3>1, <3>2, <4>1,
               OwnedTypedQueueTailPreservesCommandQueueOwnership
               DEF DeferredClassNonempty, DeferredClassQueue,
                   AsyncCompletionSequenceTyped, AsyncQueueTyped
          <5> QED BY <3>1, <3>2, <5>1, <5>2, <5>3
        <4>2. CASE SelectedDeferredClass(node) = "Progress"
          <5>1. /\ asyncDeferredCompletionQueues' =
                        asyncDeferredCompletionQueues
                 /\ asyncDeferredProgressQueues'[node] =
                        Tail(asyncDeferredProgressQueues[node])
                 /\ asyncDeferredNormalQueues' =
                        asyncDeferredNormalQueues
            BY <1>1, <2>1, <4>2, FunctionalTailUpdateAtKey, Isa
               DEF RemoveNextDeferredCommand,
                   AdvanceNextDeferredClass,
                   AsyncDeferredTopologyTypeInvariant
          <5>2. /\ AsyncQueueTyped(
                        Tail(asyncDeferredProgressQueues[node]))
                 /\ AsyncCommandQueueOwnership(
                      node, Tail(asyncDeferredProgressQueues[node]))
                 /\ (\A candidate \in SequenceSet(
                         Tail(asyncDeferredProgressQueues[node])):
                       candidate.class = "Progress")
                 /\ Len(Tail(asyncDeferredProgressQueues[node])) <=
                      Len(asyncDeferredProgressQueues[node])
            BY <2>2, <3>1, <3>2, <4>2,
               OwnedClassQueueTailPreservesFacts
               DEF DeferredClassNonempty, DeferredClassQueue
          <5>3. Len(asyncDeferredProgressQueues[node]) <=
                   AsyncDeferredProgressCapacity
            BY <3>1, <3>2, Isa
          <5>4. Len(Tail(asyncDeferredProgressQueues[node])) \in Nat
            BY <5>2, TypedDeferredQueueLengthIsNatural
          <5>5. Len(asyncDeferredProgressQueues[node]) \in Nat
            BY <3>1, <3>2, TypedDeferredQueueLengthIsNatural
          <5>6. AsyncDeferredProgressCapacity \in Nat
            BY <1>1, ConfiguredDeferredCapacitiesAreNatural
          <5>7. Len(Tail(asyncDeferredProgressQueues[node])) <=
                   AsyncDeferredProgressCapacity
            BY <5>2, <5>3, <5>4, <5>5, <5>6,
               DeferredCapacityWeakOrderTransitive
          <5> QED BY <3>1, <3>2, <5>1, <5>2, <5>7, Isa
        <4>3. CASE SelectedDeferredClass(node) = "Normal"
          <5>1. /\ asyncDeferredCompletionQueues' =
                        asyncDeferredCompletionQueues
                 /\ asyncDeferredProgressQueues' =
                        asyncDeferredProgressQueues
                 /\ asyncDeferredNormalQueues'[node] =
                        Tail(asyncDeferredNormalQueues[node])
            BY <1>1, <2>1, <4>3, FunctionalTailUpdateAtKey, Isa
               DEF RemoveNextDeferredCommand,
                   AdvanceNextDeferredClass,
                   AsyncDeferredTopologyTypeInvariant
          <5>2. /\ AsyncQueueTyped(
                        Tail(asyncDeferredNormalQueues[node]))
                 /\ AsyncCommandQueueOwnership(
                      node, Tail(asyncDeferredNormalQueues[node]))
                 /\ (\A candidate \in SequenceSet(
                         Tail(asyncDeferredNormalQueues[node])):
                       candidate.class = "Normal")
                 /\ Len(Tail(asyncDeferredNormalQueues[node])) <=
                      Len(asyncDeferredNormalQueues[node])
            BY <2>2, <3>1, <3>2, <4>3,
               OwnedClassQueueTailPreservesFacts
               DEF DeferredClassNonempty, DeferredClassQueue
          <5>3. Len(asyncDeferredNormalQueues[node]) <=
                   AsyncDeferredNormalCapacity
            BY <3>1, <3>2, Isa
          <5>4. Len(Tail(asyncDeferredNormalQueues[node])) \in Nat
            BY <5>2, TypedDeferredQueueLengthIsNatural
          <5>5. Len(asyncDeferredNormalQueues[node]) \in Nat
            BY <3>1, <3>2, TypedDeferredQueueLengthIsNatural
          <5>6. AsyncDeferredNormalCapacity \in Nat
            BY <1>1, ConfiguredDeferredCapacitiesAreNatural
          <5>7. Len(Tail(asyncDeferredNormalQueues[node])) <=
                   AsyncDeferredNormalCapacity
            BY <5>2, <5>3, <5>4, <5>5, <5>6,
               DeferredCapacityWeakOrderTransitive
          <5> QED BY <3>1, <3>2, <5>1, <5>2, <5>7, Isa
        <4> QED BY <2>2, <4>1, <4>2, <4>3
             DEF AsyncCommandClasses
      <3>3. CASE other # node
        <4>1. /\ asyncDeferredCompletionQueues'[other] =
                      asyncDeferredCompletionQueues[other]
               /\ asyncDeferredProgressQueues'[other] =
                      asyncDeferredProgressQueues[other]
               /\ asyncDeferredNormalQueues'[other] =
                      asyncDeferredNormalQueues[other]
          BY <1>1, <2>1, <2>3, <3>3,
             FunctionalTailUpdateAwayFromKey, Isa
             DEF RemoveNextDeferredCommand,
                 AdvanceNextDeferredClass,
                 AsyncDeferredTopologyTypeInvariant,
                 AsyncCommandClasses
        <4> QED BY <3>1, <4>1
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>3 DEF AsyncDeferredContentTypeInvariant
  <1> QED BY <1>1

THEOREM TypedQueueHeadTailIndexFacts ==
  \A queue:
    /\ AsyncQueueTyped(queue)
    /\ Len(queue) > 0
    => /\ Head(queue) = queue[1]
       /\ Len(Tail(queue)) = Len(queue) - 1
       /\ \A index \in 1..Len(Tail(queue)):
            Tail(queue)[index] = queue[index + 1]
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0
         PROVE /\ Head(queue) = queue[1]
               /\ Len(Tail(queue)) = Len(queue) - 1
               /\ \A index \in 1..Len(Tail(queue)):
                    Tail(queue)[index] = queue[index + 1]
    <2>1. /\ queue \in Seq(Range(queue))
           /\ queue # <<>>
      BY <1>1, EmptySeq, SMT DEF AsyncQueueTyped
    <2> QED BY <2>1, HeadTailProperties,
                 NonemptySequenceHeadIsFirst
  <1> QED BY <1>1

TailCompletionIndexShift(queue) ==
  [index \in AsyncCompletionIndices(queue) \ {1} |-> index - 1]

THEOREM TailCompletionIndexShiftIsBijection ==
  \A queue:
    /\ AsyncQueueTyped(queue)
    /\ Len(queue) > 0
    => TailCompletionIndexShift(queue)
         \in Bijection(
              AsyncCompletionIndices(queue) \ {1},
              AsyncCompletionIndices(Tail(queue)))
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0
         PROVE TailCompletionIndexShift(queue)
                 \in Bijection(
                      AsyncCompletionIndices(queue) \ {1},
                      AsyncCompletionIndices(Tail(queue)))
    <2> DEFINE Old == AsyncCompletionIndices(queue) \ {1}
    <2> DEFINE New == AsyncCompletionIndices(Tail(queue))
    <2>1. /\ Len(queue) \in Nat
           /\ Len(Tail(queue)) = Len(queue) - 1
           /\ \A index \in 1..Len(Tail(queue)):
                Tail(queue)[index] = queue[index + 1]
      BY <1>1, TypedQueueHeadTailIndexFacts, LenProperties
         DEF AsyncQueueTyped
    <2>2. /\ Old \subseteq 2..Len(queue)
           /\ New \subseteq 1..Len(Tail(queue))
      BY <1>1, <2>1, SMT DEF AsyncCompletionIndices, Old, New
    <2>3. \A index \in Old: index - 1 \in New
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncCompletionIndices, Old, New
    <2>4. \A index \in New: index + 1 \in Old
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncCompletionIndices, Old, New
    <2>5. TailCompletionIndexShift(queue) \in [Old -> New]
      BY <2>3, Isa DEF TailCompletionIndexShift, Old
    <2>6. TailCompletionIndexShift(queue) \in Injection(Old, New)
      BY <2>2, <2>5, SMT
         DEF Injection, TailCompletionIndexShift, Old
    <2>7. TailCompletionIndexShift(queue) \in Surjection(Old, New)
      BY <2>4, <2>5, SMT
         DEF Surjection, TailCompletionIndexShift, Old
    <2> QED BY <2>6, <2>7 DEF Bijection
  <1> QED BY <1>1

THEOREM CompletionCountAfterTypedQueueTail ==
  \A queue:
    /\ AsyncQueueTyped(queue)
    /\ Len(queue) > 0
    => Cardinality(AsyncCompletionIndices(Tail(queue))) =
         IF Head(queue).class = "Completion"
         THEN Cardinality(AsyncCompletionIndices(queue)) - 1
         ELSE Cardinality(AsyncCompletionIndices(queue))
PROOF
  <1>1. ASSUME NEW queue,
                AsyncQueueTyped(queue),
                Len(queue) > 0
         PROVE Cardinality(AsyncCompletionIndices(Tail(queue))) =
                 IF Head(queue).class = "Completion"
                 THEN Cardinality(AsyncCompletionIndices(queue)) - 1
                 ELSE Cardinality(AsyncCompletionIndices(queue))
    <2> DEFINE Old == AsyncCompletionIndices(queue)
    <2> DEFINE New == AsyncCompletionIndices(Tail(queue))
    <2>1. /\ IsFiniteSet(Old)
           /\ (1 \in Old <=> Head(queue).class = "Completion")
      BY <1>1, TypedQueueHeadTailIndexFacts, FS_Interval,
         FS_Subset, SMT
         DEF AsyncCompletionIndices, Old
    <2>2. /\ IsFiniteSet(Old \ {1})
           /\ Cardinality(Old \ {1}) =
                IF 1 \in Old
                THEN Cardinality(Old) - 1
                ELSE Cardinality(Old)
      BY <2>1, FS_RemoveElement
    <2>3. ExistsBijection(Old \ {1}, New)
      BY <1>1, TailCompletionIndexShiftIsBijection
         DEF ExistsBijection, Old, New
    <2>4. Cardinality(New) = Cardinality(Old \ {1})
      BY <2>2, <2>3, FS_Bijection
    <2> QED BY <2>1, <2>2, <2>4 DEF Old, New
  <1> QED BY <1>1

RemovalCompletionIndexShift(queue, removed) ==
  [oldIndex \in AsyncCompletionIndices(queue) \ {removed} |->
     IF oldIndex < removed THEN oldIndex ELSE oldIndex - 1]

THEOREM RemovalCompletionIndexShiftIsBijection ==
  \A queue, removed:
    /\ AsyncQueueTyped(queue)
    /\ removed \in 1..Len(queue)
    => RemovalCompletionIndexShift(queue, removed)
         \in Bijection(
              AsyncCompletionIndices(queue) \ {removed},
              AsyncCompletionIndices(
                SequenceWithoutIndex(queue, removed)))
PROOF
  <1>1. ASSUME NEW queue, NEW removed,
                AsyncQueueTyped(queue),
                removed \in 1..Len(queue)
         PROVE RemovalCompletionIndexShift(queue, removed)
                 \in Bijection(
                      AsyncCompletionIndices(queue) \ {removed},
                      AsyncCompletionIndices(
                        SequenceWithoutIndex(queue, removed)))
    <2> DEFINE Old == AsyncCompletionIndices(queue) \ {removed}
    <2> DEFINE Result == SequenceWithoutIndex(queue, removed)
    <2> DEFINE New == AsyncCompletionIndices(Result)
    <2>1. /\ Result \in Seq(Range(queue))
           /\ Len(Result) = Len(queue) - 1
           /\ \A resultIndex \in 1..Len(Result):
                Result[resultIndex] =
                  IF resultIndex < removed
                  THEN queue[resultIndex]
                  ELSE queue[resultIndex + 1]
      BY <1>1, SequenceWithoutIndexFacts DEF AsyncQueueTyped, Result
    <2>2. /\ Old \subseteq 1..Len(queue)
           /\ New \subseteq 1..Len(Result)
      BY <1>1, <2>1, SMT DEF AsyncCompletionIndices, Old, New
    <2>3. \A oldIndex \in Old:
             IF oldIndex < removed
             THEN oldIndex \in New
             ELSE oldIndex - 1 \in New
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncCompletionIndices, Old, New
    <2>4. \A newIndex \in New:
             IF newIndex < removed
             THEN newIndex \in Old
             ELSE newIndex + 1 \in Old
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncCompletionIndices, Old, New
    <2>5. RemovalCompletionIndexShift(queue, removed) \in [Old -> New]
      BY <2>3, Isa DEF RemovalCompletionIndexShift, Old
    <2>6. RemovalCompletionIndexShift(queue, removed)
             \in Injection(Old, New)
      BY <2>2, <2>5, SMT
         DEF Injection, RemovalCompletionIndexShift, Old
    <2>7. RemovalCompletionIndexShift(queue, removed)
             \in Surjection(Old, New)
      BY <2>4, <2>5, SMT
         DEF Surjection, RemovalCompletionIndexShift, Old
    <2> QED BY <2>6, <2>7 DEF Bijection
  <1> QED BY <1>1

THEOREM CompletionCountAfterTypedQueueRemoval ==
  \A queue, removed:
    /\ AsyncQueueTyped(queue)
    /\ removed \in 1..Len(queue)
    => Cardinality(
         AsyncCompletionIndices(SequenceWithoutIndex(queue, removed))) =
         IF queue[removed].class = "Completion"
         THEN Cardinality(AsyncCompletionIndices(queue)) - 1
         ELSE Cardinality(AsyncCompletionIndices(queue))
PROOF
  <1>1. ASSUME NEW queue, NEW removed,
                AsyncQueueTyped(queue),
                removed \in 1..Len(queue)
         PROVE Cardinality(
                 AsyncCompletionIndices(
                   SequenceWithoutIndex(queue, removed))) =
                 IF queue[removed].class = "Completion"
                 THEN Cardinality(AsyncCompletionIndices(queue)) - 1
                 ELSE Cardinality(AsyncCompletionIndices(queue))
    <2> DEFINE All == AsyncCompletionIndices(queue)
    <2> DEFINE Remaining == All \ {removed}
    <2> DEFINE New == AsyncCompletionIndices(
                         SequenceWithoutIndex(queue, removed))
    <2>1. /\ IsFiniteSet(All)
           /\ (removed \in All
                 <=> queue[removed].class = "Completion")
      BY <1>1, FS_Interval, FS_Subset, SMT
         DEF AsyncCompletionIndices, All
    <2>2. /\ IsFiniteSet(Remaining)
           /\ Cardinality(Remaining) =
                IF removed \in All
                THEN Cardinality(All) - 1
                ELSE Cardinality(All)
      BY <2>1, FS_RemoveElement DEF Remaining
    <2>3. ExistsBijection(Remaining, New)
      BY <1>1, RemovalCompletionIndexShiftIsBijection
         DEF ExistsBijection, Remaining, New, All
    <2>4. Cardinality(New) = Cardinality(Remaining)
      BY <2>2, <2>3, FS_Bijection
    <2> QED BY <2>1, <2>2, <2>4 DEF All, Remaining, New
  <1> QED BY <1>1

THEOREM FifoRuntimePreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ SerializedRunnerRuntimeStep(node)
    /\ FifoRuntimeStep(node)
    => AsyncIoTypeInvariant'
BY RuntimeSelectedCommandsAreTyped, NextNodeCommandIndexFacts,
   FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
   CompletionCountAfterTypedQueueRemoval,
   TypedOwnedSequenceWithoutIndexFacts, SequenceSetAfterAppend,
   CompletionAppendCountIncreasesByOne,
   FunctionalUpdateAwayFromKey,
   FunctionalAppendUpdateAtKey,
   FS_Interval, FS_Subset, FS_CardinalityType,
   SMTT(120)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant, AsyncIoCapacityTypeInvariant,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       FifoRuntimeStep, NodeQueueNonempty,
       NextNodeCommand, NextNodeCommandIndex,
       RemoveNextNodeCommand, SequenceWithoutIndex,
       DeferCommand, DiscardCommand,
       AsyncIoVars, AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncCompletionLoad, AsyncOutstandingWorkCount,
       QueuedCompletionCount, QueuedCompletionIndices,
       AsyncCompletionIndices, DeferredCompletionCount,
       AsyncDeferredContentTypeInvariant,
       AsyncCompletionSequenceTyped, SequenceSet, vars

THEOREM DeferredDrainRuntimePreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ SerializedRunnerRuntimeStep(node)
    /\ DeferredDrainStep(node)
    => AsyncIoTypeInvariant'
BY RuntimeSelectedCommandsAreTyped, TypedQueueTailFacts,
   FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
   FunctionalTailUpdateAtKey, FunctionalUpdateAwayFromKey,
   FS_Interval, FS_Subset, FS_CardinalityType,
   SMTT(120)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant, AsyncIoCapacityTypeInvariant,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       DeferredDrainStep,
       DeferredQueueNonempty, NextDeferredCommand,
       RemoveNextDeferredCommand, DiscardCommand,
       AsyncIoVars, AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncCompletionLoad, AsyncOutstandingWorkCount,
       QueuedCompletionCount, QueuedCompletionIndices,
       DeferredCompletionCount, AsyncDeferredContentTypeInvariant,
       AsyncCompletionSequenceTyped, SequenceSet, vars

THEOREM NonQueueRuntimeBranchPreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncIoTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ SerializedRunnerRuntimeStep(node)
    /\ (DeferredTagStep(node)
          \/ DirectTimeoutStep(node)
          \/ DirectRetransmitStep(node)
          \/ IdleRuntimeStep(node))
    => AsyncIoTypeInvariant'
BY SchedulerIoStutterPreservesIoType,
   FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
   Isa
   DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       DeferredTagStep,
       DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, IdleRuntimeStep,
       AsyncIoVars, AsyncDeferredVars, DeferCommand,
       DiscardCommand, vars

THEOREM SerializedRuntimePreservesIoType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncIoTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                AsyncControlServiceSlotTransition,
                SerializedRunnerRuntimeStep(node)
         PROVE AsyncIoTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. CASE DeferredDrainStep(node)
      BY <1>1, <2>1, <2>2, DeferredDrainRuntimePreservesIoType
    <2>3. CASE FifoRuntimeStep(node)
      BY <1>1, <2>1, <2>3, FifoRuntimePreservesIoType
    <2>4. CASE DeferredTagStep(node)
                   \/ DirectTimeoutStep(node)
                   \/ DirectRetransmitStep(node)
                   \/ IdleRuntimeStep(node)
      BY <1>1, <2>1, <2>4, NonQueueRuntimeBranchPreservesIoType
    <2> QED BY <1>1, <2>2, <2>3, <2>4
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep
  <1> QED BY <1>1

THEOREM SerializedRuntimeCausalFrame ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => \/ LeaveCausalQueues
       \/ AppendCausalSuccessors(TimeoutCausalCommand(node))
       \/ \E command:
            /\ AsyncCandidateTyped(command)
            /\ ExecuteCommand(command)
            /\ AppendCausalSuccessors(command)
BY RuntimeSelectedCommandsAreTyped, SMTT(120)
   DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, FifoRuntimeStep,
       IdleRuntimeStep, NodeQueueNonempty, DeferredQueueNonempty,
       DeferCommand, DiscardCommand, vars

THEOREM SerializedRuntimePreservesCausalType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncCausalTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                SerializedRunnerRuntimeStep(node)
         PROVE AsyncCausalTypeInvariant'
    <2>1. /\ node \in ValidatorIds
           /\ AsyncCausalTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant
    <2>2. \/ LeaveCausalQueues
           \/ AppendCausalSuccessors(TimeoutCausalCommand(node))
           \/ \E command:
                /\ AsyncCandidateTyped(command)
                /\ ExecuteCommand(command)
                /\ AppendCausalSuccessors(command)
      BY <1>1, <2>1, SerializedRuntimeCausalFrame
    <2>3. CASE LeaveCausalQueues
      BY <2>1, <2>3, AsyncCausalTypeStutter DEF LeaveCausalQueues
    <2>4. CASE AppendCausalSuccessors(TimeoutCausalCommand(node))
      <3>1. /\ AsyncQueueTyped(
                       FreshCommandSuccessors(TimeoutCausalCommand(node)))
             /\ AsyncCausalQueueOwnership(
                  node, FreshCommandSuccessors(TimeoutCausalCommand(node)))
        BY <1>1, <2>1, FreshTimeoutCausalSuccessorsTypedAndOwned
      <3>2. asyncCausalQueues' =
               [asyncCausalQueues EXCEPT
                  ![node] = @ \o
                    FreshCommandSuccessors(TimeoutCausalCommand(node))]
        BY <2>4 DEF AppendCausalSuccessors,
                      TimeoutCausalCommand, NoItemCandidate,
                      AsyncCandidate
      <3> QED BY <2>1, <3>1, <3>2,
           AppendOwnedCausalSuccessorsPreservesCausalType
    <2>5. CASE \E command:
                    /\ AsyncCandidateTyped(command)
                    /\ ExecuteCommand(command)
                    /\ AppendCausalSuccessors(command)
      <3>1. PICK command:
                    /\ AsyncCandidateTyped(command)
                    /\ ExecuteCommand(command)
                    /\ AppendCausalSuccessors(command)
        BY <2>5
      <3>2. /\ AsyncQueueTyped(FreshCommandSuccessors(command))
             /\ AsyncCausalQueueOwnership(
                  command.node, FreshCommandSuccessors(command))
        BY <1>1, <3>1, ExecutedFreshCommandSuccessorsTypedAndOwned
      <3>3. /\ command.node \in ValidatorIds
             /\ asyncCausalQueues' =
                  [asyncCausalQueues EXCEPT
                     ![command.node] = @ \o FreshCommandSuccessors(command)]
        BY <3>1 DEF AsyncCandidateTyped, AppendCausalSuccessors
      <3> QED BY <2>1, <3>2, <3>3,
           AppendOwnedCausalSuccessorsPreservesCausalType
    <2> QED BY <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM ExecuteCommandLeavesIngress ==
  \A command:
    ExecuteCommand(command)
      => UNCHANGED <<asyncIngressLanes, asyncIngressReady>>
BY Isa
   DEF ExecuteCommand, ExecuteRegularCommand, ExecuteSignProposal,
       ExecuteSignVote, ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AsyncAuxVars, vars

THEOREM ExecuteCommandOnlyRetiresCertifiedResponseClaim ==
  \A command:
    ExecuteCommand(command)
      => /\ asyncCertifiedResponseClaim'
               \subseteq asyncCertifiedResponseClaim
         /\ UNCHANGED asyncIngressLanes
BY SMTT(90), Isa
   DEF ExecuteCommand, ExecuteRegularCommand,
       RetireCompletedBodyCertifiedResponseAuthority,
       RetireNodeCertifiedResponseAuthority,
       FilterCertifiedResponseAuthority,
       CertifiedResponseClaimForRequests,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       PublishControlItems, PublishEphemeralItems,
       PublishControlAndEphemeralItems,
       PublishCertifiedRequests,
       PersistInstalledControlAfterInstall, PersistDecisionControl,
       AsyncAuxVars

THEOREM ExecuteCommandPreservesClaimIngressOwnership ==
  \A command:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ ExecuteCommand(command)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY ExecuteCommandOnlyRetiresCertifiedResponseClaim,
   CertifiedResponseClaimIngressOwnershipIsDownwardClosed

THEOREM SerializedRuntimeLeavesIngress ==
  \A node \in ValidatorIds:
    SerializedRunnerRuntimeStep(node)
      => UNCHANGED <<asyncIngressLanes, asyncIngressReady>>
BY ExecuteCommandLeavesIngress, Isa
   DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, FifoRuntimeStep,
       IdleRuntimeStep, DeferCommand, DiscardCommand, AsyncAuxVars, vars

THEOREM SerializedRuntimeOnlyRetiresCertifiedResponseClaim ==
  \A node \in ValidatorIds:
    SerializedRunnerRuntimeStep(node)
      => asyncCertifiedResponseClaim'
           \subseteq asyncCertifiedResponseClaim
BY ExecuteCommandOnlyRetiresCertifiedResponseClaim,
   SMTT(120), Isa
   DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, FifoRuntimeStep,
       IdleRuntimeStep, DeferCommand, DiscardCommand,
       SendNodeRetransmissions, NoSendItem, AsyncAuxVars, vars

THEOREM SerializedRuntimePreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY SerializedRuntimeLeavesIngress,
   SerializedRuntimeOnlyRetiresCertifiedResponseClaim,
   CertifiedResponseClaimIngressOwnershipIsDownwardClosed

THEOREM SerializedRuntimePreservesIngressType ==
  \A node \in ValidatorIds:
    /\ AsyncIngressTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncIngressTypeInvariant'
BY SerializedRuntimeLeavesIngress,
   AsyncIngressTopologyTypeStutter,
   AsyncIngressCapacityTypeStutter,
   AsyncIngressContentTypeStutter, Isa
   DEF AsyncIngressTypeInvariant, AsyncIngressTopologyTypeVars,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncIoVars, AsyncServeLifecycleVars,
       AsyncServeIngressAdmissionVars

THEOREM SerializedRuntimePreservesTransportClockType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncTransportClockTypeInvariant'
BY FunctionalUpdatePreservesType, RunnerServiceFramePreservesClockType,
   SMTT(120)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, AsyncTransportClockTypeInvariant,
       RunNodeWork, RunnerServiceFrame,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep,
       DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, FifoRuntimeStep,
       IdleRuntimeStep, ExecuteCommand, ExecuteRegularCommand,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, DeferCommand, DiscardCommand,
       AsyncAuxVars, AsyncDeferredVars, AsyncCompletionTags,
       AsyncConfiguration, vars

THEOREM SerializedRuntimePreservesDeferredType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncDeferredTypeInvariant'
BY RuntimeSelectedCommandsAreTyped,
   DeferTypedOwnedCommandPreservesDeferredContentType,
   RemoveNextDeferredCommandPreservesDeferredContentType,
   TypedCompletionTailPreservesSequenceType,
   OwnedClassQueueTailPreservesFacts,
   AsyncDeferredContentTypeStutter,
   FunctionalUpdatePreservesType, FunctionalUpdateAwayFromKey,
   AdvanceNextDeferredClassPreservesCursorType,
   SMTT(120)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredTopologyTypeInvariant,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep,
       DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, FifoRuntimeStep,
       IdleRuntimeStep, NodeQueueNonempty, DeferredQueueNonempty,
       NextNodeCommand, NextDeferredCommand, DeferredClassQueue,
       SelectedDeferredClass, AdvanceNextDeferredClass,
       NextCommandClass, DeferCommand,
       DeferredHandoffQueueHead, DeferredHandoffCandidate,
       DeferredHandoffActive, DeferredHandoffMatches,
       InstallDeferredHandoff, RetainDeferredHandoffs,
       ClearDeferredHandoff, AsyncDeferredHandoffSet,
       AsyncDeferredHandoff, NoAsyncDeferredHandoff,
       AsyncCandidateTyped, AsyncCandidateSet, AsyncCandidateDomain,
       DiscardCommand, AsyncDeferredVars, vars

RetainableControlBatch(items, voters) ==
  /\ IsFiniteSet(items)
  /\ \A item \in items:
       /\ AsyncItemTyped(item)
       /\ item.kind \in AsyncControlKinds
  /\ IF items = {}
     THEN TRUE
     ELSE LET fresh == CHOOSE item \in items: TRUE
          IN /\ \A item \in items:
                   /\ item.source = fresh.source
                   /\ ControlClass(item) = ControlClass(fresh)
                   /\ ControlView(item) = ControlView(fresh)
             /\ Cardinality(items) <= Cardinality(voters)
             /\ {recipientItem.envelope.recipient:
                   recipientItem \in items} =
                  ControlRecipients(
                    fresh.source, ControlClass(fresh), voters)

THEOREM ProposalControlEnvelopeIsTyped ==
  \A envelope \in ProposalEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem(
      "Proposal", envelope.proposal.proposer, envelope))
PROOF
  <1>1. ASSUME NEW envelope \in ProposalEnvelopeSet
         PROVE AsyncItemTyped(AsyncNetworkItem(
                 "Proposal", envelope.proposal.proposer, envelope))
    <2>1. /\ envelope.proposal.proposer \in ValidatorIds
           /\ envelope.recipient \in ValidatorIds
      BY <1>1 DEF ProposalEnvelopeSet, ProposalRecordSet
    <2> QED BY <1>1, <2>1, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources
  <1> QED BY <1>1

THEOREM VoteControlEnvelopeIsTyped ==
  \A envelope \in VoteEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem(
      IF envelope.vote.phase = "Prepare"
      THEN "PrepareVote" ELSE "CommitVote",
      envelope.vote.signer, envelope))
PROOF
  <1>1. ASSUME NEW envelope \in VoteEnvelopeSet
         PROVE AsyncItemTyped(AsyncNetworkItem(
                 IF envelope.vote.phase = "Prepare"
                 THEN "PrepareVote" ELSE "CommitVote",
                 envelope.vote.signer, envelope))
    <2>1. /\ envelope.vote.signer \in ValidatorIds
           /\ envelope.vote.phase \in Phases
           /\ envelope.recipient \in ValidatorIds
      BY <1>1 DEF VoteEnvelopeSet, VoteRecordSet
    <2>2. CASE envelope.vote.phase = "Prepare"
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources
    <2>3. CASE envelope.vote.phase # "Prepare"
      BY <1>1, <2>1, <2>3, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources, Phases
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM QcControlEnvelopeIsTyped ==
  \A source \in ValidatorIds, envelope \in QcEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem(
      IF envelope.qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
      source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in QcEnvelopeSet
         PROVE AsyncItemTyped(AsyncNetworkItem(
                 IF envelope.qc.phase = "Prepare"
                 THEN "PrepareQC" ELSE "CommitQC",
                 source, envelope))
    <2>1. /\ envelope.qc.phase \in Phases
           /\ envelope.recipient \in ValidatorIds
      BY <1>1 DEF QcEnvelopeSet, QcRecordSet
    <2>2. CASE envelope.qc.phase = "Prepare"
      BY <1>1, <2>1, <2>2, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources
    <2>3. CASE envelope.qc.phase # "Prepare"
      BY <1>1, <2>1, <2>3, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources, Phases
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM TimeoutControlEnvelopeIsTyped ==
  \A envelope \in TimeoutEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem(
      "TimeoutVote", envelope.vote.signer, envelope))
PROOF
  <1>1. ASSUME NEW envelope \in TimeoutEnvelopeSet
         PROVE AsyncItemTyped(AsyncNetworkItem(
                 "TimeoutVote", envelope.vote.signer, envelope))
    <2>1. /\ envelope.vote.signer \in ValidatorIds
           /\ envelope.recipient \in ValidatorIds
      BY <1>1 DEF TimeoutEnvelopeSet, TimeoutVoteRecordSet
    <2> QED BY <1>1, <2>1, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources
  <1> QED BY <1>1

THEOREM TcControlEnvelopeIsTyped ==
  \A source \in ValidatorIds, envelope \in TcEnvelopeSet:
    AsyncItemTyped(AsyncNetworkItem(
      "TimeoutCertificate", source, envelope))
PROOF
  <1>1. ASSUME NEW source \in ValidatorIds,
                NEW envelope \in TcEnvelopeSet
         PROVE AsyncItemTyped(AsyncNetworkItem(
                 "TimeoutCertificate", source, envelope))
    <2>1. envelope.recipient \in ValidatorIds
      BY <1>1 DEF TcEnvelopeSet
    <2> QED BY <1>1, <2>1, SMT
         DEF AsyncItemTyped, AsyncNetworkItem, AsyncNetworkKinds,
             AsyncIngressSources
  <1> QED BY <1>1

THEOREM UniformControlBatchIsRetainable ==
  \A items, voters:
    /\ IsFiniteSet(items)
    /\ \A item \in items:
         /\ AsyncItemTyped(item)
         /\ item.kind \in AsyncControlKinds
    /\ Cardinality(items) <= Cardinality(voters)
    /\ \A item \in items:
         {recipientItem.envelope.recipient:
            recipientItem \in items} =
           ControlRecipients(item.source, ControlClass(item), voters)
    /\ \A left, right \in items:
         /\ left.source = right.source
         /\ ControlClass(left) = ControlClass(right)
         /\ ControlView(left) = ControlView(right)
    => RetainableControlBatch(items, voters)
PROOF
  <1>1. ASSUME NEW items, NEW voters,
                IsFiniteSet(items),
                \A item \in items:
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in AsyncControlKinds,
                Cardinality(items) <= Cardinality(voters),
                \A item \in items:
                  {recipientItem.envelope.recipient:
                     recipientItem \in items} =
                    ControlRecipients(
                      item.source, ControlClass(item), voters),
                \A left, right \in items:
                  /\ left.source = right.source
                  /\ ControlClass(left) = ControlClass(right)
                  /\ ControlView(left) = ControlView(right)
         PROVE RetainableControlBatch(items, voters)
    <2>1. CASE items = {}
      BY <1>1, <2>1 DEF RetainableControlBatch
    <2>2. CASE items # {}
      <3> DEFINE Fresh == CHOOSE item \in items: TRUE
      <3>1. Fresh \in items
        BY <2>2, FS_EmptySet, Zenon DEF Fresh
      <3>2. \A item \in items:
               /\ item.source = Fresh.source
               /\ ControlClass(item) = ControlClass(Fresh)
               /\ ControlView(item) = ControlView(Fresh)
        BY <1>1, <3>1
      <3> QED BY <1>1, <2>2, <3>1, <3>2
           DEF RetainableControlBatch, Fresh
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1, Zenon

THEOREM ProposalOutboxIsRetainable ==
  \A request \in ProposalSignSet:
    (/\ AsyncTypeInvariant
     /\ request.proposal.proposer = request.node)
      => RetainableControlBatch(ProposalOutbox(request), CurrentVoters)
PROOF
  <1>1. ASSUME NEW request \in ProposalSignSet,
                /\ AsyncTypeInvariant
                /\ request.proposal.proposer = request.node
         PROVE RetainableControlBatch(
                 ProposalOutbox(request), CurrentVoters)
    <2> DEFINE Items == ProposalOutbox(request)
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
         DEF AsyncTypeInvariant
    <2>2. /\ IsFiniteSet(Items)
           /\ Cardinality(Items) <= Cardinality(CurrentVoters)
      BY <2>1, FS_Image
         DEF Items, ProposalOutbox, AsyncNetworkItem, ProposalEnvelope
    <2>3. \A item \in Items:
             /\ AsyncItemTyped(item)
             /\ item.kind \in AsyncControlKinds
      <3>1. ASSUME NEW item \in Items
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind \in AsyncControlKinds
        <4>1. PICK recipient \in CurrentVoters:
                 item = AsyncNetworkItem(
                   "Proposal", request.node,
                   ProposalEnvelope(recipient, request.proposal))
          BY <3>1 DEF Items, ProposalOutbox
        <4>2. /\ recipient \in ValidatorIds
               /\ request.proposal \in ProposalRecordSet
          BY <1>1, <2>1, <4>1 DEF ProposalSignSet
        <4>3. ProposalEnvelope(recipient, request.proposal)
                  \in ProposalEnvelopeSet
          BY <4>2 DEF ProposalEnvelope, ProposalEnvelopeSet
        <4>4. item = AsyncNetworkItem(
                 "Proposal", request.proposal.proposer,
                 ProposalEnvelope(recipient, request.proposal))
          BY <1>1, <4>1
        <4>5. AsyncItemTyped(AsyncNetworkItem(
                 "Proposal",
                 ProposalEnvelope(recipient, request.proposal).proposal.proposer,
                 ProposalEnvelope(recipient, request.proposal)))
          BY <4>3, ProposalControlEnvelopeIsTyped
        <4>6. AsyncItemTyped(AsyncNetworkItem(
                 "Proposal", request.proposal.proposer,
                 ProposalEnvelope(recipient, request.proposal)))
          BY <4>5 DEF ProposalEnvelope
        <4>7. AsyncItemTyped(item)
          BY <4>4, <4>6
        <4> QED BY <4>1, <4>7 DEF AsyncControlKinds, AsyncNetworkItem
      <3> QED BY <3>1
    <2>4. \A left, right \in Items:
             /\ left.source = right.source
             /\ ControlClass(left) = ControlClass(right)
             /\ ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW left \in Items, NEW right \in Items
             PROVE /\ left.source = right.source
                   /\ ControlClass(left) = ControlClass(right)
                   /\ ControlView(left) = ControlView(right)
        <4>1. PICK leftRecipient \in CurrentVoters:
                 left = AsyncNetworkItem(
                   "Proposal", request.node,
                   ProposalEnvelope(leftRecipient, request.proposal))
          BY <3>1 DEF Items, ProposalOutbox
        <4>2. PICK rightRecipient \in CurrentVoters:
                 right = AsyncNetworkItem(
                   "Proposal", request.node,
                   ProposalEnvelope(rightRecipient, request.proposal))
          BY <3>1 DEF Items, ProposalOutbox
        <4> QED BY <4>1, <4>2
             DEF AsyncNetworkItem, ProposalEnvelope,
                 ControlClass, ControlView
      <3> QED BY <3>1
    <2>5. {item.envelope.recipient: item \in Items} = CurrentVoters
      BY Isa
         DEF Items, ProposalOutbox, AsyncNetworkItem, ProposalEnvelope
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
                 UniformControlBatchIsRetainable
         DEF Items, ControlRecipients, ControlClass
  <1> QED BY <1>1

THEOREM VoteOutboxIsRetainable ==
  \A request \in VoteSignSet:
    (/\ AsyncTypeInvariant
     /\ request.vote.signer = request.node)
      => RetainableControlBatch(VoteOutbox(request), CurrentVoters)
PROOF
  <1>1. ASSUME NEW request \in VoteSignSet,
                /\ AsyncTypeInvariant
                /\ request.vote.signer = request.node
         PROVE RetainableControlBatch(VoteOutbox(request), CurrentVoters)
    <2> DEFINE Items == VoteOutbox(request)
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
         DEF AsyncTypeInvariant
    <2>2. /\ IsFiniteSet(Items)
           /\ Cardinality(Items) <= Cardinality(CurrentVoters)
      BY <2>1, FS_Image
         DEF Items, VoteOutbox, AsyncNetworkItem, VoteEnvelope
    <2>3. \A item \in Items:
             /\ AsyncItemTyped(item)
             /\ item.kind \in AsyncControlKinds
      <3>1. ASSUME NEW item \in Items
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind \in AsyncControlKinds
        <4>1. PICK recipient \in CurrentVoters:
                 item = AsyncNetworkItem(
                   IF request.vote.phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   request.node, VoteEnvelope(recipient, request.vote))
          BY <3>1 DEF Items, VoteOutbox
        <4>2. /\ recipient \in ValidatorIds
               /\ request.vote \in VoteRecordSet
          BY <1>1, <2>1, <4>1 DEF VoteSignSet
        <4>3. VoteEnvelope(recipient, request.vote) \in VoteEnvelopeSet
          BY <4>2 DEF VoteEnvelope, VoteEnvelopeSet
        <4>4. item = AsyncNetworkItem(
                 IF request.vote.phase = "Prepare"
                 THEN "PrepareVote" ELSE "CommitVote",
                 request.vote.signer,
                 VoteEnvelope(recipient, request.vote))
          BY <1>1, <4>1
        <4>5. AsyncItemTyped(AsyncNetworkItem(
                 IF VoteEnvelope(recipient, request.vote).vote.phase = "Prepare"
                 THEN "PrepareVote" ELSE "CommitVote",
                 VoteEnvelope(recipient, request.vote).vote.signer,
                 VoteEnvelope(recipient, request.vote)))
          BY <4>3, VoteControlEnvelopeIsTyped
        <4>6. AsyncItemTyped(AsyncNetworkItem(
                 IF request.vote.phase = "Prepare"
                 THEN "PrepareVote" ELSE "CommitVote",
                 request.vote.signer,
                 VoteEnvelope(recipient, request.vote)))
          BY <4>5 DEF VoteEnvelope
        <4>7. AsyncItemTyped(item)
          BY <4>4, <4>6
        <4> QED BY <1>1, <4>1, <4>7
             DEF AsyncControlKinds, AsyncNetworkItem,
                 VoteSignSet, VoteRecordSet, Phases
      <3> QED BY <3>1
    <2>4. \A left, right \in Items:
             /\ left.source = right.source
             /\ ControlClass(left) = ControlClass(right)
             /\ ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW left \in Items, NEW right \in Items
             PROVE /\ left.source = right.source
                   /\ ControlClass(left) = ControlClass(right)
                   /\ ControlView(left) = ControlView(right)
        <4>1. PICK leftRecipient \in CurrentVoters:
                 left = AsyncNetworkItem(
                   IF request.vote.phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   request.node, VoteEnvelope(leftRecipient, request.vote))
          BY <3>1 DEF Items, VoteOutbox
        <4>2. PICK rightRecipient \in CurrentVoters:
                 right = AsyncNetworkItem(
                   IF request.vote.phase = "Prepare"
                   THEN "PrepareVote" ELSE "CommitVote",
                   request.node, VoteEnvelope(rightRecipient, request.vote))
          BY <3>1 DEF Items, VoteOutbox
        <4> QED BY <4>1, <4>2
             DEF AsyncNetworkItem, VoteEnvelope, ControlClass, ControlView
      <3> QED BY <3>1
    <2>5. {item.envelope.recipient: item \in Items} =
             CurrentVoters \ {request.node}
      BY Isa DEF Items, VoteOutbox, AsyncNetworkItem, VoteEnvelope
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
                 UniformControlBatchIsRetainable
         DEF Items, ControlRecipients, ControlClass
  <1> QED BY <1>1

THEOREM QcOutboxIsRetainable ==
  \A node \in ValidatorIds, qc \in QcRecordSet:
    AsyncTypeInvariant
      => RetainableControlBatch(QcOutbox(node, qc), CurrentVoters)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW qc \in QcRecordSet,
                AsyncTypeInvariant
         PROVE RetainableControlBatch(QcOutbox(node, qc), CurrentVoters)
    <2> DEFINE Items == QcOutbox(node, qc)
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
         DEF AsyncTypeInvariant
    <2>2. /\ IsFiniteSet(Items)
           /\ Cardinality(Items) <= Cardinality(CurrentVoters)
      BY <2>1, FS_Image
         DEF Items, QcOutbox, AsyncNetworkItem, QcEnvelope
    <2>3. \A item \in Items:
             /\ AsyncItemTyped(item)
             /\ item.kind \in AsyncControlKinds
      <3>1. ASSUME NEW item \in Items
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind \in AsyncControlKinds
        <4>1. PICK recipient \in CurrentVoters:
                 item = AsyncNetworkItem(
                   IF qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
                   node, QcEnvelope(recipient, qc))
          BY <3>1 DEF Items, QcOutbox
        <4>2. QcEnvelope(recipient, qc) \in QcEnvelopeSet
          BY <1>1, <2>1, <4>1 DEF QcEnvelope, QcEnvelopeSet
        <4>3. AsyncItemTyped(AsyncNetworkItem(
                 IF QcEnvelope(recipient, qc).qc.phase = "Prepare"
                 THEN "PrepareQC" ELSE "CommitQC",
                 node, QcEnvelope(recipient, qc)))
          BY <1>1, <4>2, QcControlEnvelopeIsTyped
        <4>4. AsyncItemTyped(AsyncNetworkItem(
                 IF qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
                 node, QcEnvelope(recipient, qc)))
          BY <4>3 DEF QcEnvelope
        <4>5. AsyncItemTyped(item)
          BY <4>1, <4>4
        <4> QED BY <1>1, <4>1, <4>5
             DEF AsyncControlKinds, AsyncNetworkItem,
                 QcRecordSet, Phases
      <3> QED BY <3>1
    <2>4. \A left, right \in Items:
             /\ left.source = right.source
             /\ ControlClass(left) = ControlClass(right)
             /\ ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW left \in Items, NEW right \in Items
             PROVE /\ left.source = right.source
                   /\ ControlClass(left) = ControlClass(right)
                   /\ ControlView(left) = ControlView(right)
        <4>1. PICK leftRecipient \in CurrentVoters:
                 left = AsyncNetworkItem(
                   IF qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
                   node, QcEnvelope(leftRecipient, qc))
          BY <3>1 DEF Items, QcOutbox
        <4>2. PICK rightRecipient \in CurrentVoters:
                 right = AsyncNetworkItem(
                   IF qc.phase = "Prepare" THEN "PrepareQC" ELSE "CommitQC",
                   node, QcEnvelope(rightRecipient, qc))
          BY <3>1 DEF Items, QcOutbox
        <4> QED BY <4>1, <4>2
             DEF AsyncNetworkItem, QcEnvelope, ControlClass, ControlView
      <3> QED BY <3>1
    <2>5. {item.envelope.recipient: item \in Items} = CurrentVoters
      BY Isa DEF Items, QcOutbox, AsyncNetworkItem, QcEnvelope
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
                 UniformControlBatchIsRetainable
         DEF Items, ControlRecipients, ControlClass
  <1> QED BY <1>1

THEOREM TimeoutOutboxIsRetainable ==
  \A request \in TimeoutSignSet:
    (/\ AsyncTypeInvariant
     /\ request.vote.signer = request.node)
      => RetainableControlBatch(TimeoutOutbox(request), CurrentVoters)
PROOF
  <1>1. ASSUME NEW request \in TimeoutSignSet,
                /\ AsyncTypeInvariant
                /\ request.vote.signer = request.node
         PROVE RetainableControlBatch(
                 TimeoutOutbox(request), CurrentVoters)
    <2> DEFINE Items == TimeoutOutbox(request)
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
         DEF AsyncTypeInvariant
    <2>2. /\ IsFiniteSet(Items)
           /\ Cardinality(Items) <= Cardinality(CurrentVoters)
      BY <2>1, FS_Image
         DEF Items, TimeoutOutbox, AsyncNetworkItem, TimeoutEnvelope
    <2>3. \A item \in Items:
             /\ AsyncItemTyped(item)
             /\ item.kind \in AsyncControlKinds
      <3>1. ASSUME NEW item \in Items
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind \in AsyncControlKinds
        <4>1. PICK recipient \in CurrentVoters:
                 item = AsyncNetworkItem(
                   "TimeoutVote", request.node,
                   TimeoutEnvelope(recipient, request.vote))
          BY <3>1 DEF Items, TimeoutOutbox
        <4>2. /\ recipient \in ValidatorIds
               /\ request.vote \in TimeoutVoteRecordSet
          BY <1>1, <2>1, <4>1 DEF TimeoutSignSet
        <4>3. TimeoutEnvelope(recipient, request.vote)
                  \in TimeoutEnvelopeSet
          BY <4>2 DEF TimeoutEnvelope, TimeoutEnvelopeSet
        <4>4. item = AsyncNetworkItem(
                 "TimeoutVote", request.vote.signer,
                 TimeoutEnvelope(recipient, request.vote))
          BY <1>1, <4>1
        <4>5. AsyncItemTyped(AsyncNetworkItem(
                 "TimeoutVote",
                 TimeoutEnvelope(recipient, request.vote).vote.signer,
                 TimeoutEnvelope(recipient, request.vote)))
          BY <4>3, TimeoutControlEnvelopeIsTyped
        <4>6. AsyncItemTyped(AsyncNetworkItem(
                 "TimeoutVote", request.vote.signer,
                 TimeoutEnvelope(recipient, request.vote)))
          BY <4>5 DEF TimeoutEnvelope
        <4>7. AsyncItemTyped(item)
          BY <4>4, <4>6
        <4> QED BY <4>1, <4>7 DEF AsyncControlKinds, AsyncNetworkItem
      <3> QED BY <3>1
    <2>4. \A left, right \in Items:
             /\ left.source = right.source
             /\ ControlClass(left) = ControlClass(right)
             /\ ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW left \in Items, NEW right \in Items
             PROVE /\ left.source = right.source
                   /\ ControlClass(left) = ControlClass(right)
                   /\ ControlView(left) = ControlView(right)
        <4>1. PICK leftRecipient \in CurrentVoters:
                 left = AsyncNetworkItem(
                   "TimeoutVote", request.node,
                   TimeoutEnvelope(leftRecipient, request.vote))
          BY <3>1 DEF Items, TimeoutOutbox
        <4>2. PICK rightRecipient \in CurrentVoters:
                 right = AsyncNetworkItem(
                   "TimeoutVote", request.node,
                   TimeoutEnvelope(rightRecipient, request.vote))
          BY <3>1 DEF Items, TimeoutOutbox
        <4> QED BY <4>1, <4>2
             DEF AsyncNetworkItem, TimeoutEnvelope,
                 ControlClass, ControlView
      <3> QED BY <3>1
    <2>5. {item.envelope.recipient: item \in Items} = CurrentVoters
      BY Isa DEF Items, TimeoutOutbox, AsyncNetworkItem, TimeoutEnvelope
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
                 UniformControlBatchIsRetainable
         DEF Items, ControlRecipients, ControlClass
  <1> QED BY <1>1

THEOREM TcOutboxIsRetainable ==
  \A node \in ValidatorIds, tc \in TcRecordSet:
    AsyncTypeInvariant
      => RetainableControlBatch(TcOutbox(node, tc), CurrentVoters)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW tc \in TcRecordSet,
                AsyncTypeInvariant
         PROVE RetainableControlBatch(TcOutbox(node, tc), CurrentVoters)
    <2> DEFINE Items == TcOutbox(node, tc)
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
         DEF AsyncTypeInvariant
    <2>2. /\ IsFiniteSet(Items)
           /\ Cardinality(Items) <= Cardinality(CurrentVoters)
      BY <2>1, FS_Image
         DEF Items, TcOutbox, AsyncNetworkItem, TcEnvelope
    <2>3. \A item \in Items:
             /\ AsyncItemTyped(item)
             /\ item.kind \in AsyncControlKinds
      <3>1. ASSUME NEW item \in Items
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind \in AsyncControlKinds
        <4>1. PICK recipient \in CurrentVoters:
                 item = AsyncNetworkItem(
                   "TimeoutCertificate", node,
                   TcEnvelope(recipient, tc))
          BY <3>1 DEF Items, TcOutbox
        <4>2. TcEnvelope(recipient, tc) \in TcEnvelopeSet
          BY <1>1, <2>1, <4>1 DEF TcEnvelope, TcEnvelopeSet
        <4>3. AsyncItemTyped(AsyncNetworkItem(
                 "TimeoutCertificate", node, TcEnvelope(recipient, tc)))
          BY <1>1, <4>2, TcControlEnvelopeIsTyped
        <4>4. AsyncItemTyped(item)
          BY <4>1, <4>3
        <4> QED BY <4>1, <4>4 DEF AsyncControlKinds, AsyncNetworkItem
      <3> QED BY <3>1
    <2>4. \A left, right \in Items:
             /\ left.source = right.source
             /\ ControlClass(left) = ControlClass(right)
             /\ ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW left \in Items, NEW right \in Items
             PROVE /\ left.source = right.source
                   /\ ControlClass(left) = ControlClass(right)
                   /\ ControlView(left) = ControlView(right)
        <4>1. PICK leftRecipient \in CurrentVoters:
                 left = AsyncNetworkItem(
                   "TimeoutCertificate", node,
                   TcEnvelope(leftRecipient, tc))
          BY <3>1 DEF Items, TcOutbox
        <4>2. PICK rightRecipient \in CurrentVoters:
                 right = AsyncNetworkItem(
                   "TimeoutCertificate", node,
                   TcEnvelope(rightRecipient, tc))
          BY <3>1 DEF Items, TcOutbox
        <4> QED BY <4>1, <4>2
             DEF AsyncNetworkItem, TcEnvelope, ControlClass, ControlView
      <3> QED BY <3>1
    <2>5. {item.envelope.recipient: item \in Items} = CurrentVoters
      BY Isa DEF Items, TcOutbox, AsyncNetworkItem, TcEnvelope
    <2> QED BY <2>2, <2>3, <2>4, <2>5,
                 UniformControlBatchIsRetainable
         DEF Items, ControlRecipients, ControlClass
  <1> QED BY <1>1

THEOREM RememberedControlPreservesRetainedType ==
  \A retained, items, voters:
    /\ AsyncRetainedControlType(retained, voters)
    /\ RetainableControlBatch(items, voters)
    => AsyncRetainedControlType(
         RememberedControl(retained, items), voters)
PROOF
  <1>1. ASSUME NEW retained, NEW items, NEW voters,
                AsyncRetainedControlType(retained, voters),
                RetainableControlBatch(items, voters)
         PROVE AsyncRetainedControlType(
                 RememberedControl(retained, items), voters)
    <2>1. CASE items = {}
      BY <1>1, <2>1 DEF RememberedControl
    <2>2. CASE items # {}
      <3> DEFINE Fresh == CHOOSE item \in items: TRUE
      <3> DEFINE Existing ==
             RetainedClassItems(
               retained, Fresh.source, ControlClass(Fresh))
      <3>1. /\ Fresh \in items
             /\ \A item \in items:
                  /\ item.source = Fresh.source
                  /\ ControlClass(item) = ControlClass(Fresh)
                  /\ ControlView(item) = ControlView(Fresh)
             /\ Cardinality(items) <= Cardinality(voters)
             /\ {recipientItem.envelope.recipient:
                   recipientItem \in items} =
                  ControlRecipients(
                    Fresh.source, ControlClass(Fresh), voters)
        BY <1>1, <2>2, FS_EmptySet, Zenon
           DEF RetainableControlBatch, Fresh
      <3>2. CASE ~(Existing = {}
                     \/ ControlView(Fresh) >
                          ControlView(CHOOSE item \in Existing: TRUE)
                     \/ items = Existing)
        BY <1>1, <2>2, <3>2
           DEF RememberedControl, Fresh, Existing
      <3>3. CASE Existing = {}
                    \/ ControlView(Fresh) >
                         ControlView(CHOOSE item \in Existing: TRUE)
                    \/ items = Existing
        <4> DEFINE Updated == (retained \ Existing) \cup items
        <4>1. RememberedControl(retained, items) = Updated
          BY <2>2, <3>3 DEF RememberedControl, Fresh, Existing, Updated
        <4>2. /\ IsFiniteSet(Updated)
               /\ \A item \in Updated:
                    /\ AsyncItemTyped(item)
                    /\ item.kind \in AsyncControlKinds
          BY <1>1, FS_Difference, FS_Union
             DEF AsyncRetainedControlType, RetainableControlBatch,
                 Updated
        <4>3. \A source \in ValidatorIds,
                      controlClass \in AsyncControlKinds:
                 LET retainedClass ==
                       RetainedClassItems(Updated, source, controlClass)
                 IN \/ retainedClass = {}
                    \/ /\ Cardinality(retainedClass) <=
                             Cardinality(voters)
                       /\ {item.envelope.recipient:
                             item \in retainedClass} =
                            ControlRecipients(
                              source, controlClass, voters)
                       /\ \A left, right \in retainedClass:
                            ControlView(left) = ControlView(right)
          <5>1. ASSUME NEW source \in ValidatorIds,
                        NEW controlClass \in AsyncControlKinds
                 PROVE LET retainedClass ==
                             RetainedClassItems(
                               Updated, source, controlClass)
                       IN \/ retainedClass = {}
                          \/ /\ Cardinality(retainedClass) <=
                                   Cardinality(voters)
                             /\ {item.envelope.recipient:
                                   item \in retainedClass} =
                                  ControlRecipients(
                                    source, controlClass, voters)
                             /\ \A left, right \in retainedClass:
                                  ControlView(left) = ControlView(right)
            <6>1. CASE /\ source = Fresh.source
                         /\ controlClass = ControlClass(Fresh)
              <7>1. RetainedClassItems(
                       Updated, source, controlClass) = items
                BY <3>1, <5>1, <6>1, Isa
                   DEF RetainedClassItems, Existing, Updated
              <7> QED BY <1>1, <2>2, <3>1, <7>1
                   DEF RetainableControlBatch
            <6>2. CASE source # Fresh.source
                         \/ controlClass # ControlClass(Fresh)
              <7>1. RetainedClassItems(
                       Updated, source, controlClass) =
                     RetainedClassItems(retained, source, controlClass)
                BY <3>1, <5>1, <6>2, Isa
                   DEF RetainedClassItems, Existing, Updated
              <7> QED BY <1>1, <5>1, <7>1
                   DEF AsyncRetainedControlType
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <4>1, <4>2, <4>3
             DEF AsyncRetainedControlType
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM InstalledControlPreservesRetainedType ==
  \A retained, node, items, voters:
    /\ AsyncRetainedControlType(
         RememberedControl(retained, items), voters)
    => AsyncRetainedControlType(
         InstalledControl(retained, node, items), voters)
PROOF
  <1>1. ASSUME NEW retained, NEW node, NEW items, NEW voters,
                AsyncRetainedControlType(
                  RememberedControl(retained, items), voters)
         PROVE AsyncRetainedControlType(
                 InstalledControl(retained, node, items), voters)
    <2> DEFINE Remembered == RememberedControl(retained, items)
    <2> DEFINE Installed == InstalledControl(retained, node, items)
    <2>1. Installed =
             {item \in Remembered:
                item.source # node
                  \/ ControlClass(item)
                       \in AsyncInstallRetainedControlKinds}
      BY DEF InstalledControl, Remembered, Installed
    <2>2. /\ Installed \subseteq Remembered
           /\ IsFiniteSet(Installed)
           /\ \A item \in Installed:
                /\ AsyncItemTyped(item)
                /\ item.kind \in AsyncControlKinds
      BY <1>1, <2>1, FS_Subset
         DEF AsyncRetainedControlType, Remembered
    <2>3. \A source \in ValidatorIds,
                  controlClass \in AsyncControlKinds:
             LET installedClass ==
                   RetainedClassItems(Installed, source, controlClass)
             IN \/ installedClass = {}
                \/ /\ Cardinality(installedClass) <=
                         Cardinality(voters)
                   /\ {item.envelope.recipient:
                         item \in installedClass} =
                        ControlRecipients(source, controlClass, voters)
                   /\ \A left, right \in installedClass:
                        ControlView(left) = ControlView(right)
      <3>1. ASSUME NEW source \in ValidatorIds,
                    NEW controlClass \in AsyncControlKinds
             PROVE LET installedClass ==
                         RetainedClassItems(
                           Installed, source, controlClass)
                   IN \/ installedClass = {}
                      \/ /\ Cardinality(installedClass) <=
                               Cardinality(voters)
                         /\ {item.envelope.recipient:
                               item \in installedClass} =
                              ControlRecipients(
                                source, controlClass, voters)
                         /\ \A left, right \in installedClass:
                              ControlView(left) = ControlView(right)
        <4>1. CASE source = node
                     /\ controlClass
                          \notin AsyncInstallRetainedControlKinds
          BY <2>1, <4>1, Isa DEF RetainedClassItems, Installed
        <4>2. CASE source # node
                     \/ controlClass
                          \in AsyncInstallRetainedControlKinds
          <5>1. RetainedClassItems(Installed, source, controlClass) =
                   RetainedClassItems(Remembered, source, controlClass)
            BY <2>1, <3>1, <4>2, Isa
               DEF RetainedClassItems, Installed
          <5> QED BY <1>1, <3>1, <5>1
               DEF AsyncRetainedControlType, Remembered
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3
         DEF AsyncRetainedControlType, Installed
  <1> QED BY <1>1

THEOREM TypedRetentionOnlyPreservesTransportContentType ==
  /\ AsyncTransportContentTypeInvariant
  /\ AsyncRetainedControlType(asyncRetainedControl', CurrentVoters')
  /\ asyncActiveRequests' \subseteq asyncActiveRequests
  /\ AsyncCertifiedResponseClaimInvariant'
  /\ UNCHANGED <<context, asyncSentItems,
                  asyncTransport, asyncHeldChunks>>
  => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME AsyncTransportContentTypeInvariant,
              AsyncRetainedControlType(
                asyncRetainedControl', CurrentVoters'),
              asyncActiveRequests' \subseteq asyncActiveRequests,
              AsyncCertifiedResponseClaimInvariant',
              UNCHANGED <<context, asyncSentItems,
                          asyncTransport, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncSentItemsType(asyncSentItems)
           /\ AsyncActiveRequestsType(
                asyncActiveRequests, asyncSentItems)
           /\ AsyncCertifiedResponseClaimInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1, AsyncTransportHistoryTypeDecomposition
         DEF AsyncTransportContentTypeInvariant
    <2>2. AsyncSentItemsType(asyncSentItems')
      BY <1>1, <2>1 DEF AsyncSentItemsType
    <2>3. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      <3>1. IsFiniteSet(asyncActiveRequests')
        BY <1>1, <2>1, FS_Subset DEF AsyncActiveRequestsType
      <3>2. /\ asyncActiveRequests' \subseteq asyncSentItems'
             /\ \A item \in asyncActiveRequests':
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in {"CertifiedRequest",
                                      "CommitCertificateRequest"}
        BY <1>1, <2>1 DEF AsyncActiveRequestsType
      <3>3. AsyncCertifiedRequestLogicalIndexConsistent(
               asyncActiveRequests')
        BY <1>1, <2>1,
           CertifiedRequestLogicalIndexConsistencyIsDownwardClosed
           DEF AsyncActiveRequestsType
      <3> QED BY <3>1, <3>2, <3>3 DEF AsyncActiveRequestsType
    <2>4. /\ AsyncPacketContentTypeInvariant'
           /\ AsyncHeldChunksTypeInvariant'
      BY <1>1, <2>1
         DEF AsyncPacketContentTypeInvariant,
             AsyncHeldChunksTypeInvariant
    <2>5. AsyncTransportHistoryTypeInvariant'
      BY <1>1, <2>2, <2>3
         DEF AsyncTransportHistoryTypeInvariant,
             AsyncSentItemsType, AsyncRetainedControlType,
             AsyncActiveRequestsType
    <2> QED BY <2>4, <2>5
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM PublishTypedItemsWithTypedRetentionPreservesTransportContentType ==
  \A items:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ IsFiniteSet(items)
    /\ \A item \in items: AsyncItemTyped(item)
    /\ AsyncRetainedControlType(asyncRetainedControl', CurrentVoters')
    /\ asyncSentItems' = asyncSentItems \cup items
    /\ asyncActiveRequests' \subseteq asyncActiveRequests
    /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
    /\ AsyncCertifiedResponseClaimInvariant'
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                IsFiniteSet(items),
                \A item \in items: AsyncItemTyped(item),
                AsyncRetainedControlType(
                  asyncRetainedControl', CurrentVoters'),
                asyncSentItems' = asyncSentItems \cup items,
                asyncActiveRequests' \subseteq asyncActiveRequests,
                asyncTransport' =
                  asyncTransport \cup PacketsForItems(items),
                AsyncCertifiedResponseClaimInvariant',
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncSentItemsType(asyncSentItems)
           /\ AsyncActiveRequestsType(
                asyncActiveRequests, asyncSentItems)
           /\ AsyncCertifiedResponseClaimInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1, AsyncTransportHistoryTypeDecomposition
         DEF AsyncTransportContentTypeInvariant
    <2>2. AsyncSentItemsType(asyncSentItems')
      BY <1>1, <2>1, FS_Union DEF AsyncSentItemsType
    <2>3. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      <3>1. IsFiniteSet(asyncActiveRequests')
        BY <1>1, <2>1, FS_Subset DEF AsyncActiveRequestsType
      <3>2. /\ asyncActiveRequests' \subseteq asyncSentItems'
             /\ \A item \in asyncActiveRequests':
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in {"CertifiedRequest",
                                      "CommitCertificateRequest"}
        BY <1>1, <2>1 DEF AsyncActiveRequestsType
      <3>3. AsyncCertifiedRequestLogicalIndexConsistent(
               asyncActiveRequests')
        BY <1>1, <2>1,
           CertifiedRequestLogicalIndexConsistencyIsDownwardClosed
           DEF AsyncActiveRequestsType
      <3> QED BY <3>1, <3>2, <3>3 DEF AsyncActiveRequestsType
    <2>4. AsyncTransportHistoryTypeInvariant'
      BY <1>1, <2>2, <2>3
         DEF AsyncTransportHistoryTypeInvariant,
             AsyncSentItemsType, AsyncRetainedControlType,
             AsyncActiveRequestsType
    <2>5. /\ IsFiniteSet(PacketsForItems(items))
           /\ \A packet \in PacketsForItems(items):
                AsyncPacketTyped(packet)
      BY <1>1, PacketsForItemsAreFiniteAndTyped
         DEF AsyncRuntimeScalarTypeInvariant
    <2>6. AsyncPacketContentTypeInvariant'
      BY <1>1, <2>1, <2>5, FS_Union
         DEF AsyncPacketContentTypeInvariant
    <2>7. AsyncHeldChunksTypeInvariant'
      BY <1>1, <2>1 DEF AsyncHeldChunksTypeInvariant
    <2> QED BY <2>4, <2>6, <2>7
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM RememberRetainableControlPreservesTransportContentType ==
  \A items:
    /\ AsyncTransportContentTypeInvariant
    /\ RetainableControlBatch(items, CurrentVoters)
    /\ asyncRetainedControl' =
         RememberedControl(asyncRetainedControl, items)
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncSentItems, asyncActiveRequests,
           asyncCertifiedResponseClaim, asyncTransport, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
BY RememberedControlPreservesRetainedType,
   TypedRetentionOnlyPreservesTransportContentType,
   AppendSentHistoryPreservesCertifiedResponseClaimInvariant, Isa
   DEF AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncRetainedControlType, AsyncCertifiedRequestsIn,
       CurrentVoters, CurrentEpoch

THEOREM RememberRetainableControlOptionallyPublishes ==
  \A items, broadcast:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ RetainableControlBatch(items, CurrentVoters)
    /\ broadcast \in BOOLEAN
    /\ asyncRetainedControl' =
         RememberedControl(asyncRetainedControl, items)
    /\ asyncSentItems' =
         IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
    /\ asyncActiveRequests' = asyncActiveRequests
    /\ asyncTransport' =
         IF broadcast
         THEN asyncTransport \cup PacketsForItems(items)
         ELSE asyncTransport
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncCertifiedResponseClaim, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items, NEW broadcast,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                RetainableControlBatch(items, CurrentVoters),
                broadcast \in BOOLEAN,
                asyncRetainedControl' =
                  RememberedControl(asyncRetainedControl, items),
                asyncSentItems' =
                  IF broadcast
                  THEN asyncSentItems \cup items ELSE asyncSentItems,
                asyncActiveRequests' = asyncActiveRequests,
                asyncTransport' =
                  IF broadcast
                  THEN asyncTransport \cup PacketsForItems(items)
                  ELSE asyncTransport,
                UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncCertifiedResponseClaim, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, RememberedControlPreservesRetainedType, Isa
         DEF AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncRetainedControlType, CurrentVoters, CurrentEpoch
    <2>2. CASE broadcast = TRUE
      BY <1>1, <2>1, <2>2,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant,
         PublishTypedItemsWithTypedRetentionPreservesTransportContentType
         DEF RetainableControlBatch, AsyncCertifiedRequestsIn
    <2>3. CASE broadcast = FALSE
      BY <1>1, <2>1, <2>3,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant,
         TypedRetentionOnlyPreservesTransportContentType
         DEF AsyncCertifiedRequestsIn
    <2> QED BY <1>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM InstallRetainableControlOptionallyPublishes ==
  \A node, items, broadcast:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ RetainableControlBatch(items, CurrentVoters)
    /\ broadcast \in BOOLEAN
    /\ asyncRetainedControl' =
         InstalledControl(asyncRetainedControl, node, items)
    /\ asyncSentItems' =
         IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
    /\ asyncActiveRequests' \subseteq asyncActiveRequests
    /\ asyncTransport' =
         IF broadcast
         THEN asyncTransport \cup PacketsForItems(items)
         ELSE asyncTransport
    /\ AsyncCertifiedResponseClaimInvariant'
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node, NEW items, NEW broadcast,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                RetainableControlBatch(items, CurrentVoters),
                broadcast \in BOOLEAN,
                asyncRetainedControl' =
                  InstalledControl(asyncRetainedControl, node, items),
                asyncSentItems' =
                  IF broadcast
                  THEN asyncSentItems \cup items ELSE asyncSentItems,
                asyncActiveRequests' \subseteq asyncActiveRequests,
                asyncTransport' =
                  IF broadcast
                  THEN asyncTransport \cup PacketsForItems(items)
                  ELSE asyncTransport,
                AsyncCertifiedResponseClaimInvariant',
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncRetainedControlType(
             RememberedControl(asyncRetainedControl, items), CurrentVoters)
      BY <1>1, RememberedControlPreservesRetainedType
         DEF AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncRetainedControlType
    <2>2. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, <2>1, InstalledControlPreservesRetainedType, Isa
         DEF CurrentVoters, CurrentEpoch
    <2>3. CASE broadcast = TRUE
      BY <1>1, <2>2, <2>3,
         PublishTypedItemsWithTypedRetentionPreservesTransportContentType
         DEF RetainableControlBatch
    <2>4. CASE broadcast = FALSE
      BY <1>1, <2>2, <2>4,
         TypedRetentionOnlyPreservesTransportContentType
    <2> QED BY <1>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM RememberRetainableControlWithFilteredRequestsOptionallyPublishes ==
  \A items, broadcast:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ RetainableControlBatch(items, CurrentVoters)
    /\ broadcast \in BOOLEAN
    /\ asyncRetainedControl' =
         RememberedControl(asyncRetainedControl, items)
    /\ asyncSentItems' =
         IF broadcast THEN asyncSentItems \cup items ELSE asyncSentItems
    /\ asyncActiveRequests' \subseteq asyncActiveRequests
    /\ asyncTransport' =
         IF broadcast
         THEN asyncTransport \cup PacketsForItems(items)
         ELSE asyncTransport
    /\ AsyncCertifiedResponseClaimInvariant'
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items, NEW broadcast,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                RetainableControlBatch(items, CurrentVoters),
                broadcast \in BOOLEAN,
                asyncRetainedControl' =
                  RememberedControl(asyncRetainedControl, items),
                asyncSentItems' =
                  IF broadcast
                  THEN asyncSentItems \cup items ELSE asyncSentItems,
                asyncActiveRequests' \subseteq asyncActiveRequests,
                asyncTransport' =
                  IF broadcast
                  THEN asyncTransport \cup PacketsForItems(items)
                  ELSE asyncTransport,
                AsyncCertifiedResponseClaimInvariant',
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, RememberedControlPreservesRetainedType, Isa
         DEF AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncRetainedControlType, CurrentVoters, CurrentEpoch
    <2>2. CASE broadcast = TRUE
      BY <1>1, <2>1, <2>2,
         PublishTypedItemsWithTypedRetentionPreservesTransportContentType
         DEF RetainableControlBatch
    <2>3. CASE broadcast = FALSE
      BY <1>1, <2>1, <2>3,
         TypedRetentionOnlyPreservesTransportContentType
    <2> QED BY <1>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM PublishRetainableControlAndEphemeralPreservesTransportContentType ==
  \A controlItems, ephemeralItems:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ RetainableControlBatch(controlItems, CurrentVoters)
    /\ IsFiniteSet(ephemeralItems)
    /\ \A item \in ephemeralItems: AsyncItemTyped(item)
    /\ PublishControlAndEphemeralItems(controlItems, ephemeralItems)
    /\ AsyncCertifiedResponseClaimInvariant'
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW controlItems, NEW ephemeralItems,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                RetainableControlBatch(controlItems, CurrentVoters),
                IsFiniteSet(ephemeralItems),
                \A item \in ephemeralItems: AsyncItemTyped(item),
                PublishControlAndEphemeralItems(
                  controlItems, ephemeralItems),
                AsyncCertifiedResponseClaimInvariant',
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2> DEFINE Items == controlItems \cup ephemeralItems
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items: AsyncItemTyped(item)
      BY <1>1, FS_Union DEF RetainableControlBatch, Items
    <2>2. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, RememberedControlPreservesRetainedType, Isa
         DEF PublishControlAndEphemeralItems,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncRetainedControlType, CurrentVoters, CurrentEpoch
    <2>3. /\ asyncSentItems' = asyncSentItems \cup Items
           /\ asyncActiveRequests' = asyncActiveRequests
           /\ asyncTransport' =
                asyncTransport \cup PacketsForItems(Items)
      BY <1>1 DEF PublishControlAndEphemeralItems, Items
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         PublishTypedItemsWithTypedRetentionPreservesTransportContentType
  <1> QED BY <1>1

THEOREM PublishRetainableControlPreservesTransportContentType ==
  \A items:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ RetainableControlBatch(items, CurrentVoters)
    /\ PublishControlItems(items)
    /\ AsyncCertifiedResponseClaimInvariant'
    /\ UNCHANGED <<context, asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                RetainableControlBatch(items, CurrentVoters),
                PublishControlItems(items),
                AsyncCertifiedResponseClaimInvariant',
                UNCHANGED <<context, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncSentItemsType(asyncSentItems)
           /\ AsyncRetainedControlType(
                asyncRetainedControl, CurrentVoters)
           /\ AsyncActiveRequestsType(
                asyncActiveRequests, asyncSentItems)
           /\ AsyncCertifiedResponseClaimInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1, AsyncTransportHistoryTypeDecomposition
         DEF AsyncTransportContentTypeInvariant
    <2>2. /\ IsFiniteSet(items)
           /\ \A item \in items: AsyncItemTyped(item)
      BY <1>1 DEF RetainableControlBatch
    <2>3. /\ asyncRetainedControl' =
                RememberedControl(asyncRetainedControl, items)
           /\ asyncSentItems' = asyncSentItems \cup items
           /\ asyncActiveRequests' = asyncActiveRequests
           /\ asyncTransport' =
                asyncTransport \cup PacketsForItems(items)
           /\ CurrentVoters' = CurrentVoters
           /\ asyncHeldChunks' = asyncHeldChunks
      BY <1>1, Isa
         DEF PublishControlItems, CurrentVoters, CurrentEpoch
    <2>4. AsyncSentItemsType(asyncSentItems')
      BY <2>1, <2>2, <2>3, FS_Union
         DEF AsyncSentItemsType
    <2>5. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, <2>1, <2>3,
         RememberedControlPreservesRetainedType
    <2>6. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      BY <2>1, <2>3, Isa DEF AsyncActiveRequestsType
    <2>7. AsyncTransportHistoryTypeInvariant'
      BY <1>1, <2>4, <2>5, <2>6
         DEF AsyncTransportHistoryTypeInvariant,
             AsyncSentItemsType, AsyncRetainedControlType,
             AsyncActiveRequestsType
    <2>8. /\ IsFiniteSet(PacketsForItems(items))
           /\ \A packet \in PacketsForItems(items):
                AsyncPacketTyped(packet)
      BY <1>1, <2>2, PacketsForItemsAreFiniteAndTyped
         DEF AsyncRuntimeScalarTypeInvariant
    <2>9. AsyncPacketContentTypeInvariant'
      BY <2>1, <2>3, <2>8, FS_Union
         DEF AsyncPacketContentTypeInvariant
    <2>10. AsyncHeldChunksTypeInvariant'
      BY <2>1, <2>3 DEF AsyncHeldChunksTypeInvariant
    <2> QED BY <2>7, <2>9, <2>10
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM PublishTrackedRequestsPreservesTransportContentType ==
  \A items:
    /\ AsyncRuntimeScalarTypeInvariant
    /\ AsyncTransportContentTypeInvariant
    /\ IsFiniteSet(items)
    /\ \A item \in items:
         /\ AsyncItemTyped(item)
         /\ item.kind \in {"CertifiedRequest",
                            "CommitCertificateRequest"}
    /\ AsyncCertifiedRequestLogicalIndexConsistent(items)
    /\ AsyncCertifiedRequestSetsCompatible(asyncActiveRequests, items)
    /\ asyncActiveRequests' = asyncActiveRequests \cup items
    /\ asyncSentItems' = asyncSentItems \cup items
    /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
    /\ UNCHANGED
         <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
           asyncCertifiedResponseClaim, asyncRetainedControl,
           asyncHeldChunks>>
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW items,
                AsyncRuntimeScalarTypeInvariant,
                AsyncTransportContentTypeInvariant,
                IsFiniteSet(items),
                \A item \in items:
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in {"CertifiedRequest",
                                     "CommitCertificateRequest"},
                AsyncCertifiedRequestLogicalIndexConsistent(items),
                AsyncCertifiedRequestSetsCompatible(
                  asyncActiveRequests, items),
                asyncActiveRequests' = asyncActiveRequests \cup items,
                asyncSentItems' = asyncSentItems \cup items,
                asyncTransport' =
                  asyncTransport \cup PacketsForItems(items),
                UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncCertifiedResponseClaim, asyncRetainedControl,
                    asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ AsyncSentItemsType(asyncSentItems)
           /\ AsyncRetainedControlType(
                asyncRetainedControl, CurrentVoters)
           /\ AsyncActiveRequestsType(
                asyncActiveRequests, asyncSentItems)
           /\ AsyncCertifiedResponseClaimInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1, AsyncTransportHistoryTypeDecomposition
         DEF AsyncTransportContentTypeInvariant
    <2>2. /\ CurrentVoters' = CurrentVoters
           /\ AsyncSentItemsType(asyncSentItems')
      BY <1>1, <2>1, FS_Union, Isa
         DEF CurrentVoters, CurrentEpoch, AsyncSentItemsType
    <2>3. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, <2>1, <2>2
    <2>4. AsyncActiveRequestsType(
             asyncActiveRequests', asyncSentItems')
      <3>1. IsFiniteSet(asyncActiveRequests')
        BY <1>1, <2>1, FS_Union DEF AsyncActiveRequestsType
      <3>2. /\ asyncActiveRequests' \subseteq asyncSentItems'
             /\ \A item \in asyncActiveRequests':
                  /\ AsyncItemTyped(item)
                  /\ item.kind \in {"CertifiedRequest",
                                      "CommitCertificateRequest"}
        BY <1>1, <2>1 DEF AsyncActiveRequestsType
      <3>3. AsyncCertifiedRequestLogicalIndexConsistent(
               asyncActiveRequests')
        BY <1>1, <2>1,
           CompatibleCertifiedRequestUnionIsLogicallyConsistent
           DEF AsyncActiveRequestsType
      <3> QED BY <3>1, <3>2, <3>3 DEF AsyncActiveRequestsType
    <2>4a. AsyncCertifiedResponseClaimInvariant'
      BY <1>1, <2>1,
         ExtendActiveRequestsPreservesCertifiedResponseClaimInvariant
    <2>5. AsyncTransportHistoryTypeInvariant'
      BY <2>2, <2>3, <2>4, <2>4a
         DEF AsyncTransportHistoryTypeInvariant,
             AsyncSentItemsType, AsyncRetainedControlType,
             AsyncActiveRequestsType
    <2>6. /\ IsFiniteSet(PacketsForItems(items))
           /\ \A packet \in PacketsForItems(items):
                AsyncPacketTyped(packet)
      BY <1>1, PacketsForItemsAreFiniteAndTyped
         DEF AsyncRuntimeScalarTypeInvariant
    <2>7. AsyncPacketContentTypeInvariant'
      BY <1>1, <2>1, <2>6, FS_Union
         DEF AsyncPacketContentTypeInvariant
    <2>8. AsyncHeldChunksTypeInvariant'
      BY <1>1, <2>1 DEF AsyncHeldChunksTypeInvariant
    <2> QED BY <2>5, <2>7, <2>8
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM RuntimeValidatorIdsAreFinite ==
  TypeInvariant => IsFiniteSet(ValidatorIds)
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE IsFiniteSet(ValidatorIds)
    <2>1. /\ 0 \in Int
           /\ N - 1 \in Int
      BY <1>1, SMT
         DEF TypeInvariant, ModelConfiguration, QuorumConfiguration
    <2> QED BY <2>1, FS_Interval DEF ValidatorIds
  <1> QED BY <1>1

THEOREM QcSignerDifferenceIsFinite ==
  \A node, qc:
    TypeInvariant /\ qc \in QcRecordSet
      => IsFiniteSet(qc.signers \ {node})
PROOF
  <1>1. ASSUME NEW node, NEW qc,
                TypeInvariant,
                qc \in QcRecordSet
         PROVE IsFiniteSet(qc.signers \ {node})
    <2>1. IsFiniteSet(ValidatorIds)
      BY <1>1, RuntimeValidatorIdsAreFinite
    <2>2. qc.signers \ {node} \subseteq ValidatorIds
      BY <1>1 DEF QcRecordSet
    <2> QED BY <2>1, <2>2, FS_Subset
  <1> QED BY <1>1

THEOREM AsyncBodyEnvelopeConstructorTyped ==
  \A recipient \in ValidatorIds, blockHeight \in Heights,
     roundView \in Views, subject \in Subjects,
     chunk \in 0..AsyncChunkCount,
     nonce \in 0..(AsyncIngressCapacity - 1):
    AsyncBodyEnvelopeTyped(
      AsyncBodyEnvelope(recipient, blockHeight, roundView,
                        subject, chunk, nonce))
BY Isa DEF AsyncBodyEnvelopeTyped, AsyncBodyEnvelope

THEOREM AsyncBodyNetworkItemConstructorTyped ==
  \A kind \in {"Chunk", "CommitCertificateRequest"},
     source \in ValidatorIds:
    \A envelope:
      /\ AsyncBodyEnvelopeTyped(envelope)
      /\ (kind = "CommitCertificateRequest" =>
            AsyncCommitCertificateRequestEnvelopeTyped(envelope))
        => AsyncItemTyped(AsyncNetworkItem(kind, source, envelope))
PROOF
  <1>1. ASSUME NEW kind \in
                  {"Chunk", "CommitCertificateRequest"},
                NEW source \in ValidatorIds,
                NEW envelope,
                /\ AsyncBodyEnvelopeTyped(envelope)
                /\ (kind = "CommitCertificateRequest" =>
                      AsyncCommitCertificateRequestEnvelopeTyped(envelope))
         PROVE AsyncItemTyped(
                 AsyncNetworkItem(kind, source, envelope))
    <2>1. /\ DOMAIN AsyncNetworkItem(kind, source, envelope) =
                  {"kind", "source", "envelope"}
           /\ AsyncNetworkItem(kind, source, envelope).kind = kind
           /\ AsyncNetworkItem(kind, source, envelope).source = source
           /\ AsyncNetworkItem(kind, source, envelope).envelope = envelope
      BY DEF AsyncNetworkItem
    <2>2. kind \in AsyncNetworkKinds
      BY <1>1, Isa DEF AsyncNetworkKinds
    <2>3. /\ source \in AsyncIngressSources
           /\ envelope.recipient \in ValidatorIds
      BY <1>1, Isa
         DEF AsyncIngressSources, AsyncBodyEnvelopeTyped
    <2>4. CASE kind = "Chunk"
      <3> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, SMT
           DEF AsyncItemTyped
    <2>5. CASE kind = "CommitCertificateRequest"
      <3> QED BY <1>1, <2>1, <2>2, <2>3, <2>5, SMT
           DEF AsyncItemTyped, AsyncReplyRequestItemTyped
    <2> QED BY <1>1, <2>4, <2>5, Isa
  <1> QED BY <1>1

THEOREM AsyncChunkReceiptConstructorTyped ==
  \A node \in ValidatorIds, roundView \in Views,
     subject \in Subjects, chunk \in AsyncChunks:
    AsyncChunkReceipt(node, roundView, subject, chunk)
      \in AsyncChunkReceiptSet
BY Isa DEF AsyncChunkReceipt, AsyncChunkReceiptSet

THEOREM ChunkOutboxIsFiniteAndTyped ==
  \A recipient, source, roundView, subject:
    (/\ AsyncTypeInvariant
     /\ recipient \in ValidatorIds
     /\ source \in ValidatorIds
     /\ roundView \in Views
     /\ subject \in Subjects)
    => /\ IsFiniteSet(
             ChunkOutbox(recipient, source, roundView, subject))
       /\ \A item \in
              ChunkOutbox(recipient, source, roundView, subject):
            AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW recipient, NEW source, NEW roundView, NEW subject,
                /\ AsyncTypeInvariant
                /\ recipient \in ValidatorIds
                /\ source \in ValidatorIds
                /\ roundView \in Views
                /\ subject \in Subjects
         PROVE /\ IsFiniteSet(
                      ChunkOutbox(
                        recipient, source, roundView, subject))
               /\ \A item \in
                      ChunkOutbox(
                        recipient, source, roundView, subject):
                    AsyncItemTyped(item)
    <2>1. /\ IsFiniteSet(AsyncChunks)
           /\ context.height \in Heights
           /\ AsyncChunkCount \in Nat \ {0}
           /\ AsyncIngressCapacity \in Nat \ {0}
      BY <1>1, FS_Interval, SMT
         DEF AsyncTypeInvariant, TypeInvariant, AsyncConfiguration,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncChunks
    <2>2. IsFiniteSet(
             ChunkOutbox(recipient, source, roundView, subject))
      BY <2>1, FS_Image, Isa DEF ChunkOutbox
    <2>3. \A item \in
                    ChunkOutbox(recipient, source, roundView, subject):
             AsyncItemTyped(item)
      <3>1. ASSUME NEW item \in
                         ChunkOutbox(
                           recipient, source, roundView, subject)
             PROVE AsyncItemTyped(item)
        <4>1. PICK chunk \in AsyncChunks:
                 item = AsyncNetworkItem(
                   "Chunk", source,
                   AsyncBodyEnvelope(recipient, context.height,
                                     roundView, subject, chunk, 0))
          BY <3>1 DEF ChunkOutbox
        <4>2. /\ chunk \in 0..AsyncChunkCount
               /\ 0 \in 0..(AsyncIngressCapacity - 1)
          BY <2>1, <4>1, SMT DEF AsyncChunks
        <4>3. AsyncBodyEnvelopeTyped(
                 AsyncBodyEnvelope(recipient, context.height,
                                   roundView, subject, chunk, 0))
          BY <1>1, <2>1, <4>2,
             AsyncBodyEnvelopeConstructorTyped
        <4>4. AsyncItemTyped(
                 AsyncNetworkItem(
                   "Chunk", source,
                   AsyncBodyEnvelope(recipient, context.height,
                                     roundView, subject, chunk, 0)))
          BY <1>1, <4>3,
             AsyncBodyNetworkItemConstructorTyped
        <4> QED BY <4>1, <4>4
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM BroadcastChunkOutboxIsFiniteAndTyped ==
  \A source, roundView, subject:
    (/\ AsyncTypeInvariant
     /\ source \in ValidatorIds
     /\ roundView \in Views
     /\ subject \in Subjects)
    => /\ IsFiniteSet(
             BroadcastChunkOutbox(source, roundView, subject))
       /\ \A item \in BroadcastChunkOutbox(source, roundView, subject):
            AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW source, NEW roundView, NEW subject,
                /\ AsyncTypeInvariant
                /\ source \in ValidatorIds
                /\ roundView \in Views
                /\ subject \in Subjects
         PROVE /\ IsFiniteSet(
                      BroadcastChunkOutbox(source, roundView, subject))
               /\ \A item \in
                      BroadcastChunkOutbox(source, roundView, subject):
                    AsyncItemTyped(item)
    <2> DEFINE ChunkBatches ==
          {ChunkOutbox(recipient, source, roundView, subject):
             recipient \in CurrentVoters}
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators
         DEF AsyncTypeInvariant
    <2>2. IsFiniteSet(ChunkBatches)
      BY <2>1, FS_Image, Isa DEF ChunkBatches
    <2>3. \A batch \in ChunkBatches: IsFiniteSet(batch)
      <3>1. ASSUME NEW batch \in ChunkBatches
             PROVE IsFiniteSet(batch)
        <4>1. PICK recipient \in CurrentVoters:
                 batch = ChunkOutbox(
                   recipient, source, roundView, subject)
          BY <3>1 DEF ChunkBatches
        <4>2. recipient \in ValidatorIds
          BY <2>1, <4>1
        <4> QED BY <1>1, <4>1, <4>2,
                     ChunkOutboxIsFiniteAndTyped
      <3> QED BY <3>1
    <2>4. IsFiniteSet(
             BroadcastChunkOutbox(source, roundView, subject))
      BY <2>2, <2>3, FS_UNION
         DEF BroadcastChunkOutbox, ChunkBatches
    <2>5. \A item \in BroadcastChunkOutbox(source, roundView, subject):
             AsyncItemTyped(item)
      <3>1. ASSUME NEW item \in
                         BroadcastChunkOutbox(
                           source, roundView, subject)
             PROVE AsyncItemTyped(item)
        <4>1. PICK recipient \in CurrentVoters:
                 item \in ChunkOutbox(
                   recipient, source, roundView, subject)
          BY <3>1 DEF BroadcastChunkOutbox
        <4>2. recipient \in ValidatorIds
          BY <2>1, <4>1
        <4> QED BY <1>1, <4>1, <4>2,
                     ChunkOutboxIsFiniteAndTyped
      <3> QED BY <3>1
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM CertifiedRequestOutboxIsFiniteAndTyped ==
  \A node \in ValidatorIds, qc \in QcRecordSet:
    (/\ AsyncTypeInvariant
     /\ qc.context = context)
    => /\ IsFiniteSet(CertifiedRequestOutbox(node, qc))
       /\ \A item \in CertifiedRequestOutbox(node, qc):
            /\ AsyncItemTyped(item)
            /\ item.kind = "CertifiedRequest"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW qc \in QcRecordSet,
                /\ AsyncTypeInvariant
                /\ qc.context = context
         PROVE /\ IsFiniteSet(CertifiedRequestOutbox(node, qc))
               /\ \A item \in CertifiedRequestOutbox(node, qc):
                    /\ AsyncItemTyped(item)
                    /\ item.kind = "CertifiedRequest"
    <2>1. /\ IsFiniteSet(ValidatorIds)
           /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
           /\ qc.signers \subseteq ValidatorIds
           /\ Responsive \subseteq ValidatorIds
           /\ AsyncConfiguration
      BY <1>1, RuntimeValidatorIdsAreFinite,
         RuntimeCurrentVotersAreFiniteValidators,
         ResponsiveAreValidators, SMT
         DEF AsyncTypeInvariant, TypeInvariant, QcRecordSet,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant
    <2>2. /\ CertifiedArchiveRoutes(node, qc) \subseteq ValidatorIds
           /\ IsFiniteSet(CertifiedArchiveRoutes(node, qc))
      BY <2>1, FS_Subset, Isa
         DEF CertifiedArchiveRoutes, AsyncResponsiveArchiveServers,
             AsyncArchiveServerIds
    <2>3. IsFiniteSet(CertifiedRequestOutbox(node, qc))
      BY <2>2, FS_Image, Isa DEF CertifiedRequestOutbox
    <2>4. \A item \in CertifiedRequestOutbox(node, qc):
             /\ AsyncItemTyped(item)
             /\ item.kind = "CertifiedRequest"
      <3>1. ASSUME NEW item \in CertifiedRequestOutbox(node, qc)
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind = "CertifiedRequest"
        <4>1. PICK recipient \in CertifiedArchiveRoutes(node, qc):
                 item = AsyncNetworkItem(
                   "CertifiedRequest", node,
                   AsyncCertifiedRequestEnvelope(
                     recipient, node, qc, 0))
          BY <3>1 DEF CertifiedRequestOutbox
        <4>2. recipient \in ValidatorIds
          BY <2>2, <4>1
        <4>3. AsyncItemTyped(item)
          BY <1>1, <2>1, <4>1, <4>2, SMTT(60)
             DEF AsyncItemTyped, AsyncReplyRequestItemTyped,
                 AsyncCertifiedRequestEnvelope, AsyncNetworkItem,
                 AsyncNetworkKinds, AsyncIngressSources,
                 AsyncConfiguration
        <4>4. item.kind = "CertifiedRequest"
          BY <4>1 DEF AsyncNetworkItem
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM CommitCertificateRequestOutboxIsFiniteAndTyped ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant
      => /\ IsFiniteSet(CommitCertificateRequestOutbox(node))
         /\ \A item \in CommitCertificateRequestOutbox(node):
              /\ AsyncItemTyped(item)
              /\ item.kind = "CommitCertificateRequest"
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant
         PROVE /\ IsFiniteSet(CommitCertificateRequestOutbox(node))
               /\ \A item \in CommitCertificateRequestOutbox(node):
                    /\ AsyncItemTyped(item)
                    /\ item.kind = "CommitCertificateRequest"
    <2>1. /\ IsFiniteSet(CurrentVoters)
           /\ CurrentVoters \subseteq ValidatorIds
           /\ IsFiniteSet(CurrentVoters \ {node})
           /\ context.height \in Heights
           /\ nodeView[node] \in Views
           /\ AsyncHeartbeatSubject \in Subjects
           /\ AsyncChunkCount \in Nat \ {0}
           /\ AsyncIngressCapacity \in Nat \ {0}
      BY <1>1, RuntimeCurrentVotersAreFiniteValidators,
         FS_Difference, SMT
         DEF AsyncTypeInvariant, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncConfiguration,
             ModelConfiguration, AsyncHeartbeatSubject, Subjects
    <2>2. IsFiniteSet(CommitCertificateRequestOutbox(node))
      BY <2>1, FS_Image, Isa DEF CommitCertificateRequestOutbox
    <2>3. \A item \in CommitCertificateRequestOutbox(node):
             /\ AsyncItemTyped(item)
             /\ item.kind = "CommitCertificateRequest"
      <3>1. ASSUME NEW item \in CommitCertificateRequestOutbox(node)
             PROVE /\ AsyncItemTyped(item)
                   /\ item.kind = "CommitCertificateRequest"
        <4>1. PICK server \in CurrentVoters \ {node}:
                 item = AsyncNetworkItem(
                   "CommitCertificateRequest", node,
                   AsyncBodyEnvelope(server, context.height,
                                     nodeView[node],
                                     AsyncHeartbeatSubject,
                                     NoAsyncChunk, 0))
          BY <3>1 DEF CommitCertificateRequestOutbox
        <4>2. server \in ValidatorIds
          BY <2>1, <4>1, Isa
        <4>3. NoAsyncChunk \in 0..AsyncChunkCount
          BY <2>1, SMT DEF NoAsyncChunk
        <4>4. 0 \in 0..(AsyncIngressCapacity - 1)
          BY <2>1, SMT
        <4>5. AsyncBodyEnvelopeTyped(
                 AsyncBodyEnvelope(server, context.height,
                                   nodeView[node], AsyncHeartbeatSubject,
                                   NoAsyncChunk, 0))
          BY <1>1, <2>1, <4>2, <4>3, <4>4,
             AsyncBodyEnvelopeConstructorTyped
        <4>6. AsyncCommitCertificateRequestEnvelopeTyped(
                 AsyncBodyEnvelope(server, context.height,
                                   nodeView[node], AsyncHeartbeatSubject,
                                   NoAsyncChunk, 0))
          BY <4>5
             DEF AsyncCommitCertificateRequestEnvelopeTyped,
                 AsyncBodyEnvelope
        <4>7. AsyncItemTyped(
                 AsyncNetworkItem(
                   "CommitCertificateRequest", node,
                   AsyncBodyEnvelope(server, context.height,
                                     nodeView[node], AsyncHeartbeatSubject,
                                     NoAsyncChunk, 0)))
          BY <1>1, <4>5, <4>6,
             AsyncBodyNetworkItemConstructorTyped
        <4>8. item.kind = "CommitCertificateRequest"
          BY <4>1 DEF AsyncNetworkItem
        <4> QED BY <4>1, <4>7, <4>8
      <3> QED BY <3>1
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM RetainedProposalChunksAreFiniteAndTyped ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant
      => /\ IsFiniteSet(RetainedProposalChunks(node))
         /\ \A item \in RetainedProposalChunks(node):
              AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant
         PROVE /\ IsFiniteSet(RetainedProposalChunks(node))
               /\ \A item \in RetainedProposalChunks(node):
                    AsyncItemTyped(item)
    <2> DEFINE Proposals ==
          {retained \in asyncRetainedControl:
             /\ retained.source = node
             /\ retained.kind = "Proposal"}
    <2> DEFINE ChunkBatches ==
          {BroadcastChunkOutbox(
             node, item.envelope.proposal.view,
             item.envelope.proposal.subject): item \in Proposals}
    <2>1. /\ IsFiniteSet(asyncRetainedControl)
           /\ \A item \in asyncRetainedControl: AsyncItemTyped(item)
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant
    <2>2. /\ Proposals \subseteq asyncRetainedControl
           /\ IsFiniteSet(Proposals)
      BY <2>1, FS_Subset DEF Proposals
    <2>3. \A proposalItem \in Proposals:
             /\ proposalItem.envelope.proposal.view \in Views
             /\ proposalItem.envelope.proposal.subject \in Subjects
      <3>1. ASSUME NEW proposalItem \in Proposals
             PROVE /\ proposalItem.envelope.proposal.view \in Views
                   /\ proposalItem.envelope.proposal.subject \in Subjects
        <4>1. /\ proposalItem \in asyncRetainedControl
               /\ AsyncItemTyped(proposalItem)
               /\ proposalItem.kind = "Proposal"
          BY <2>1, <3>1 DEF Proposals
        <4> QED BY <4>1
             DEF AsyncItemTyped, ProposalEnvelopeSet,
                 ProposalRecordSet
      <3> QED BY <3>1
    <2>4. IsFiniteSet(ChunkBatches)
      BY <2>2, FS_Image, Isa DEF ChunkBatches
    <2>5. \A batch \in ChunkBatches: IsFiniteSet(batch)
      <3>1. ASSUME NEW batch \in ChunkBatches
             PROVE IsFiniteSet(batch)
        <4>1. PICK proposalItem \in Proposals:
                 batch = BroadcastChunkOutbox(
                   node, proposalItem.envelope.proposal.view,
                   proposalItem.envelope.proposal.subject)
          BY <3>1 DEF ChunkBatches
        <4> QED BY <1>1, <2>3, <4>1,
                     BroadcastChunkOutboxIsFiniteAndTyped
      <3> QED BY <3>1
    <2>6. IsFiniteSet(RetainedProposalChunks(node))
      BY <2>4, <2>5, FS_UNION
         DEF RetainedProposalChunks, Proposals, ChunkBatches
    <2>7. \A item \in RetainedProposalChunks(node):
             AsyncItemTyped(item)
      <3>1. ASSUME NEW item \in RetainedProposalChunks(node)
             PROVE AsyncItemTyped(item)
        <4>1. PICK proposalItem \in Proposals:
                 item \in BroadcastChunkOutbox(
                   node, proposalItem.envelope.proposal.view,
                   proposalItem.envelope.proposal.subject)
          BY <3>1 DEF RetainedProposalChunks, Proposals
        <4> QED BY <1>1, <2>3, <4>1,
                     BroadcastChunkOutboxIsFiniteAndTyped
      <3> QED BY <3>1
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM RetryableItemsAreFiniteAndTyped ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant
      => /\ IsFiniteSet(RetryableItems(node))
         /\ \A item \in RetryableItems(node): AsyncItemTyped(item)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant
         PROVE /\ IsFiniteSet(RetryableItems(node))
               /\ \A item \in RetryableItems(node): AsyncItemTyped(item)
    <2>1. /\ IsFiniteSet(asyncRetainedControl)
           /\ (\A item \in asyncRetainedControl: AsyncItemTyped(item))
           /\ IsFiniteSet(asyncActiveRequests)
           /\ (\A item \in asyncActiveRequests: AsyncItemTyped(item))
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant
    <2>2. /\ SendableItems(node) \subseteq asyncRetainedControl
           /\ ActiveRequestItems(node) \subseteq asyncActiveRequests
           /\ IsFiniteSet(SendableItems(node))
           /\ IsFiniteSet(ActiveRequestItems(node))
      BY <2>1, FS_Subset, Isa DEF SendableItems, ActiveRequestItems
    <2>3. /\ IsFiniteSet(RetainedProposalChunks(node))
           /\ \A item \in RetainedProposalChunks(node):
                AsyncItemTyped(item)
      BY <1>1, RetainedProposalChunksAreFiniteAndTyped
    <2>4. /\ IsFiniteSet(RetainedControlEmissionItems(node))
           /\ \A item \in RetainedControlEmissionItems(node):
                AsyncItemTyped(item)
      BY <2>1, <2>2, <2>3, FS_Union
         DEF RetainedControlEmissionItems
    <2> QED BY <2>1, <2>2, <2>4, FS_Union
         DEF RetryableItems
  <1> QED BY <1>1

THEOREM SendNodeRetransmissionsPreservesTransportContentType ==
  \A node \in ValidatorIds:
    (/\ AsyncTypeInvariant
     /\ SendNodeRetransmissions(node)
     /\ UNCHANGED
          <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
            asyncHeldChunks>>)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                /\ AsyncTypeInvariant
                /\ SendNodeRetransmissions(node)
                /\ UNCHANGED
                     <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                       asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ IsFiniteSet(RetryableItems(node))
           /\ \A item \in RetryableItems(node): AsyncItemTyped(item)
      BY <1>1, RetryableItemsAreFiniteAndTyped
    <2>2. PublishEphemeralItems(RetryableItems(node))
      BY <1>1 DEF SendNodeRetransmissions, PublishEphemeralItems
    <2>3. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
                 PublishEphemeralItemsPreservesTransportContentType
  <1> QED BY <1>1

THEOREM AsyncTypeProvidesTransportContentInputs ==
  AsyncTypeInvariant
    => /\ AsyncRuntimeScalarTypeInvariant
       /\ AsyncTransportContentTypeInvariant
BY DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant

THEOREM CoreNextLeavesContext ==
  Next => UNCHANGED context
PROOF
  <1>1. ASSUME Next
         PROVE UNCHANGED context
    <2>1. \/ LockStableNext
           \/ (\E request \in pendingLockCommit:
                 PersistLockCommit(request))
           \/ (\E request \in pendingInstallTC:
                 PersistInstallTC(request))
      BY <1>1, NextLockFootprintClassification
    <2>2. CASE LockStableNext
      <3>1. UnchangedContextAndLocks
        BY <2>2, LockStableNextLeavesContextAndLocks
      <3> QED BY <3>1 DEF UnchangedContextAndLocks
    <2>3. CASE \E request \in pendingLockCommit:
                    PersistLockCommit(request)
      <3>1. PICK request \in pendingLockCommit:
               PersistLockCommit(request)
        BY <2>3
      <3> QED BY <3>1 DEF PersistLockCommit
    <2>4. CASE \E request \in pendingInstallTC:
                    PersistInstallTC(request)
      <3>1. PICK request \in pendingInstallTC:
               PersistInstallTC(request)
        BY <2>4
      <3> QED BY <3>1 DEF PersistInstallTC
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM RegularCoreCommandLeavesContext ==
  \A command:
    RegularCoreCommand(command) => UNCHANGED context
BY IsaM("blast")
   DEF RegularCoreCommand,
       AssembleLocalBody, BeginLocalProposal, PersistProposal,
       FetchBody, RebindRetainedBody, StoreBody, ValidateBody, RejectBody,
       ValidateDecidedBody, ValidateLockedBody, BeginPrepare,
       PersistPrepare,
       BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit, FormCommitQC,
       BeginDecision, PersistTimeout, FormTC, BeginInstallTC,
       AcceptCertifiedResponseCapability, InstallCertifiedBodyEffect

THEOREM ExecuteApplyLeavesContext ==
  \A command:
    ExecuteApply(command) => UNCHANGED context
BY Isa DEF ExecuteApply, ApplyDecision, vars

THEOREM ExecuteCoreDeliveryLeavesContext ==
  \A command:
    ExecuteCoreDelivery(command) => UNCHANGED context
BY IsaM("blast")
   DEF ExecuteCoreDelivery, DeliverProposal, DeliverVote, DeliverQC,
       DeliverTimeout, DeliverTC

THEOREM FilterCertifiedResponseAuthorityPreservesContent ==
  /\ AsyncTransportContentTypeInvariant
  /\ asyncActiveRequests' \subseteq asyncActiveRequests
  /\ asyncCertifiedResponseClaim' =
       CertifiedResponseClaimForRequests(asyncActiveRequests')
  /\ UNCHANGED <<context, asyncSentItems, asyncRetainedControl,
                  asyncTransport, asyncHeldChunks>>
  => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME AsyncTransportContentTypeInvariant,
                asyncActiveRequests' \subseteq asyncActiveRequests,
                asyncCertifiedResponseClaim' =
                  CertifiedResponseClaimForRequests(
                    asyncActiveRequests'),
                UNCHANGED
                  <<context, asyncSentItems, asyncRetainedControl,
                    asyncTransport, asyncHeldChunks>>
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncCertifiedResponseClaimInvariant
      BY <1>1
         DEF AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant
    <2>2. /\ asyncSentItems' = asyncSentItems \cup {}
           /\ IsFiniteSet({})
           /\ \A item \in {}: AsyncItemTyped(item)
      BY <1>1, FS_EmptySet
    <2>3. AsyncCertifiedResponseClaimInvariant'
      BY <1>1, <2>1, <2>2,
         FilterActiveRequestsAndClaimPreservesInvariant
    <2>4. AsyncRetainedControlType(
             asyncRetainedControl', CurrentVoters')
      BY <1>1, Isa
         DEF AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncRetainedControlType, CurrentVoters, CurrentEpoch
    <2> QED BY <1>1, <2>3, <2>4,
         TypedRetentionOnlyPreservesTransportContentType
  <1> QED BY <1>1

THEOREM ExecuteRegularCommandPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteRegularCommand(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecuteRegularCommand(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant
    <2>2. CASE command.kind \in
                 {"FetchBody", "RebindRetainedBody"}
      <3>1. /\ asyncActiveRequests' \subseteq asyncActiveRequests
             /\ asyncCertifiedResponseClaim' =
                  CertifiedResponseClaimForRequests(
                    asyncActiveRequests')
             /\ UNCHANGED
                  <<context, asyncSentItems, asyncRetainedControl,
                    asyncTransport, asyncHeldChunks>>
        BY <1>1, <2>2, RegularCoreCommandLeavesContext, Isa
           DEF ExecuteRegularCommand,
               RetireCompletedBodyCertifiedResponseAuthority,
               FilterCertifiedResponseAuthority, vars
      <3> QED BY <2>1, <3>1,
           FilterCertifiedResponseAuthorityPreservesContent
    <2>3. CASE command.kind \notin
                 {"FetchBody", "RebindRetainedBody"}
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>3, RegularCoreCommandLeavesContext, Isa
           DEF ExecuteRegularCommand, AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <2>1, <3>1,
           AsyncTransportContentTypeStutter
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ExecuteApplyPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteApply(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecuteApply(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant
    <2>2. /\ asyncActiveRequests' \subseteq asyncActiveRequests
           /\ asyncCertifiedResponseClaim' =
                CertifiedResponseClaimForRequests(
                  asyncActiveRequests')
           /\ UNCHANGED
                <<context, asyncSentItems, asyncRetainedControl,
                  asyncTransport, asyncHeldChunks>>
      BY <1>1, ExecuteApplyLeavesContext, Isa
         DEF ExecuteApply, RetireNodeCertifiedResponseAuthority,
             ActiveRequestsWithoutNode,
             FilterCertifiedResponseAuthority, vars
    <2> QED BY <2>1, <2>2,
         FilterCertifiedResponseAuthorityPreservesContent
  <1> QED BY <1>1

THEOREM ExecuteRejectAuthenticatedJunkPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteRejectAuthenticatedJunk(command))
      => AsyncTransportContentTypeInvariant'
BY AsyncTransportContentTypeStutter,
   Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       ExecuteRejectAuthenticatedJunk, AsyncAuxVars, vars

THEOREM ExecuteSignProposalPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteSignProposal(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecuteSignProposal(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. PICK request \in signProposals:
             /\ CompleteProposalSignature(request)
             /\ PublishControlAndEphemeralItems(
                  ProposalOutbox(request),
                  BroadcastChunkOutbox(
                    request.node, request.proposal.view,
                    request.proposal.subject))
      BY <1>1 DEF ExecuteSignProposal
    <2>2. /\ request \in ProposalSignSet
           /\ request.proposal.proposer = request.node
           /\ request.node \in ValidatorIds
           /\ request.proposal.view \in Views
           /\ request.proposal.subject \in Subjects
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, TypeInvariant,
             CompleteProposalSignature, ProposalSignSet,
             ProposalRecordSet
    <2>3. RetainableControlBatch(
             ProposalOutbox(request), CurrentVoters)
      BY <1>1, <2>2, ProposalOutboxIsRetainable
    <2>4. /\ IsFiniteSet(
                  BroadcastChunkOutbox(
                    request.node, request.proposal.view,
                    request.proposal.subject))
           /\ \A item \in BroadcastChunkOutbox(
                         request.node, request.proposal.view,
                         request.proposal.subject):
                AsyncItemTyped(item)
      BY <1>1, <2>2, BroadcastChunkOutboxIsFiniteAndTyped
    <2>5. UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF ExecuteSignProposal, CompleteProposalSignature, vars
    <2>6. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2>6a. AsyncCertifiedResponseClaimInvariant'
      BY <1>1, <2>1, <2>6,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant,
         Isa
         DEF PublishControlAndEphemeralItems,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncCertifiedRequestsIn
    <2> QED BY <2>1, <2>3, <2>4, <2>5, <2>6, <2>6a,
         PublishRetainableControlAndEphemeralPreservesTransportContentType
  <1> QED BY <1>1

THEOREM ExecuteSignVotePreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteSignVote(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecuteSignVote(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. PICK request \in signVotes:
             /\ CompleteVoteSignature(request)
             /\ PublishControlItems(VoteOutbox(request))
      BY <1>1 DEF ExecuteSignVote
    <2>2. /\ request \in VoteSignSet
           /\ request.vote.signer = request.node
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, TypeInvariant,
             CompleteVoteSignature
    <2>3. RetainableControlBatch(
             VoteOutbox(request), CurrentVoters)
      BY <1>1, <2>2, VoteOutboxIsRetainable
    <2>4. UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF ExecuteSignVote, CompleteVoteSignature, vars
    <2>5. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2>5a. AsyncCertifiedResponseClaimInvariant'
      BY <1>1, <2>1, <2>5,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant,
         Isa
         DEF PublishControlItems,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncCertifiedRequestsIn
    <2> QED BY <2>1, <2>3, <2>4, <2>5, <2>5a,
         PublishRetainableControlPreservesTransportContentType
  <1> QED BY <1>1

THEOREM ExecuteFormPrepareQcPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteFormPrepareQC(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecuteFormPrepareQC(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2> DEFINE Signers ==
          VoteSignersAt(command.node, command.view, "Prepare",
                        command.subject)
    <2> DEFINE Certificate ==
          QC(context, command.view, "Prepare", command.subject, Signers)
    <2> DEFINE Items == QcOutbox(command.node, Certificate)
    <2>1. /\ FormPrepareQC(
                  command.node, command.view, command.subject)
           /\ PublishControlItems(Items)
      BY <1>1
         DEF ExecuteFormPrepareQC, Signers, Certificate, Items
    <2>2. /\ command.node \in ValidatorIds
           /\ Certificate \in QcRecordSet
           /\ UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF AsyncTypeInvariant, TypeInvariant, FormPrepareQC,
             Signers, Certificate, ExecuteFormPrepareQC, vars
    <2>3. RetainableControlBatch(Items, CurrentVoters)
      BY <1>1, <2>2, QcOutboxIsRetainable DEF Items
    <2>4. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2>4a. AsyncCertifiedResponseClaimInvariant'
      BY <1>1, <2>1, <2>4,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant,
         Isa
         DEF PublishControlItems,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncCertifiedRequestsIn
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>4a,
         PublishRetainableControlPreservesTransportContentType
  <1> QED BY <1>1

THEOREM ExecuteSignTimeoutPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecuteSignTimeout(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecuteSignTimeout(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. PICK request \in signTimeouts:
             /\ CompleteTimeoutSignature(request)
             /\ PublishControlItems(TimeoutOutbox(request))
      BY <1>1 DEF ExecuteSignTimeout
    <2>2. /\ request \in TimeoutSignSet
           /\ request.vote.signer = request.node
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, TypeInvariant,
             CompleteTimeoutSignature
    <2>3. RetainableControlBatch(
             TimeoutOutbox(request), CurrentVoters)
      BY <1>1, <2>2, TimeoutOutboxIsRetainable
    <2>4. UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF ExecuteSignTimeout, CompleteTimeoutSignature, vars
    <2>5. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2>5a. AsyncCertifiedResponseClaimInvariant'
      BY <1>1, <2>1, <2>5,
         AppendSentHistoryPreservesCertifiedResponseClaimInvariant,
         Isa
         DEF PublishControlItems,
             AsyncTransportContentTypeInvariant,
             AsyncTransportHistoryTypeInvariant,
             AsyncCertifiedRequestsIn
    <2> QED BY <2>1, <2>3, <2>4, <2>5, <2>5a,
         PublishRetainableControlPreservesTransportContentType
  <1> QED BY <1>1

THEOREM ExecutePersistInstallPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecutePersistInstall(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecutePersistInstall(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. PICK request \in pendingInstallTC:
             /\ PersistInstallTC(request)
             /\ PersistInstalledControlAfterInstall(
                  request.node, request.tc,
                  TcOutbox(request.node, request.tc),
                  request.rebroadcast)
      BY <1>1 DEF ExecutePersistInstall
    <2>2. /\ request \in InstallTcWalSet
           /\ request.node \in ValidatorIds
           /\ request.tc \in TcRecordSet
           /\ request.rebroadcast \in BOOLEAN
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, TypeInvariant, InstallTcWalSet
    <2>3. RetainableControlBatch(
             TcOutbox(request.node, request.tc), CurrentVoters)
      BY <1>1, <2>2, TcOutboxIsRetainable
    <2>4. UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF ExecutePersistInstall, PersistInstallTC, vars
    <2>5. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2>5a. AsyncCertifiedResponseClaimInvariant'
      <3>1. CASE request.rebroadcast = TRUE
        BY <1>1, <2>1, <2>5, <3>1,
           FilterActiveRequestsAndClaimPreservesInvariant, Isa
           DEF PersistInstalledControlAfterInstall,
               AsyncTransportContentTypeInvariant,
               AsyncTransportHistoryTypeInvariant
      <3>2. CASE request.rebroadcast = FALSE
        BY <1>1, <2>1, <2>5, <3>2,
           FilterActiveRequestsAndClaimPreservesInvariant, Isa
           DEF PersistInstalledControlAfterInstall,
               AsyncTransportContentTypeInvariant,
               AsyncTransportHistoryTypeInvariant
      <3> QED BY <2>2, <3>1, <3>2, SMT
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>5a,
         InstallRetainableControlOptionallyPublishes
         DEF PersistInstalledControlAfterInstall
  <1> QED BY <1>1

THEOREM ExecutePersistDecisionPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ ExecutePersistDecision(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ ExecutePersistDecision(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. PICK request \in pendingDecision:
             /\ PersistDecision(request)
             /\ PersistDecisionControl(
                  request.node, request.qc,
                  QcOutbox(request.node, request.qc),
                  request.rebroadcast)
      BY <1>1 DEF ExecutePersistDecision
    <2>2. /\ request \in DecisionWalSet
           /\ request.node \in ValidatorIds
           /\ request.qc \in QcRecordSet
           /\ request.rebroadcast \in BOOLEAN
      BY <1>1, <2>1
         DEF AsyncTypeInvariant, TypeInvariant, DecisionWalSet
    <2>3. RetainableControlBatch(
             QcOutbox(request.node, request.qc), CurrentVoters)
      BY <1>1, <2>2, QcOutboxIsRetainable
    <2>4. UNCHANGED <<context, asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF ExecutePersistDecision, PersistDecision, vars
    <2>5. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2>5a. AsyncCertifiedResponseClaimInvariant'
      <3>1. CASE request.rebroadcast = TRUE
        BY <1>1, <2>1, <2>5, <3>1,
           FilterActiveRequestsAndClaimPreservesInvariant, Isa
           DEF PersistDecisionControl,
               AsyncTransportContentTypeInvariant,
               AsyncTransportHistoryTypeInvariant
      <3>2. CASE request.rebroadcast = FALSE
        BY <1>1, <2>1, <2>5, <3>2,
           FilterActiveRequestsAndClaimPreservesInvariant, Isa
           DEF PersistDecisionControl,
               AsyncTransportContentTypeInvariant,
               AsyncTransportHistoryTypeInvariant
      <3> QED BY <2>2, <3>1, <3>2, SMT
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>5a,
         RememberRetainableControlWithFilteredRequestsOptionallyPublishes
         DEF PersistDecisionControl
  <1> QED BY <1>1

THEOREM ExecuteRequestCertifiedBodyPreservesTransportContentType ==
  \A command:
    (/\ StrongInductiveInvariant
     /\ AsyncTypeInvariant
     /\ AsyncCandidateTyped(command)
     /\ ExecuteRequestCertifiedBody(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ StrongInductiveInvariant
                /\ AsyncTypeInvariant
                /\ AsyncCandidateTyped(command)
                /\ ExecuteRequestCertifiedBody(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. PICK qc \in DecisionQcValues \cup prepareQCs:
             /\ CommandMatches(command, command.node,
                                 qc.view, qc.subject)
             /\ command.evidence = qc
             /\ CertifiedBodyRecoveryAuthority(command.node, qc)
             /\ PublishCertifiedRequests(
                  CertifiedRequestOutbox(command.node, qc))
      BY <1>1 DEF ExecuteRequestCertifiedBody
    <2>2. /\ command.node \in ValidatorIds
           /\ qc \in QcRecordSet
      BY <1>1, <2>1, SMT
         DEF AsyncCandidateTyped, StrongInductiveInvariant, Safety,
             TypeInvariant, DecisionQcValues
    <2>3. /\ IsFiniteSet(
                  CertifiedRequestOutbox(command.node, qc))
           /\ \A item \in CertifiedRequestOutbox(command.node, qc):
                /\ AsyncItemTyped(item)
                /\ item.kind = "CertifiedRequest"
      BY <1>1, <2>1, <2>2,
         CertifiedRequestOutboxIsFiniteAndTyped
    <2>4. /\ UNCHANGED context
           /\ UNCHANGED
                <<AsyncCertifiedResponseClaimCoreAuthorityVars,
                  asyncCertifiedResponseClaim, asyncRetainedControl,
                  asyncHeldChunks>>
      BY <1>1, <2>1, Isa
         DEF ExecuteRequestCertifiedBody, PublishCertifiedRequests,
             AsyncCertifiedResponseClaimCoreAuthorityVars, vars
    <2>5. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
    <2> QED BY <2>1, <2>3, <2>4, <2>5,
         PublishTrackedRequestsPreservesTransportContentType
         DEF PublishCertifiedRequests
  <1> QED BY <1>1

THEOREM ExecuteDecisionFetchPreservesTransportContentType ==
  \A command:
    (/\ StrongInductiveInvariant
     /\ AsyncTypeInvariant
     /\ AsyncCandidateTyped(command)
     /\ ExecuteDecisionFetch(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ StrongInductiveInvariant
                /\ AsyncTypeInvariant
                /\ AsyncCandidateTyped(command)
                /\ ExecuteDecisionFetch(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. /\ command.node \in ValidatorIds
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
      BY <1>1, AsyncTypeProvidesTransportContentInputs
         DEF AsyncCandidateTyped
    <2>2. CASE BodyHeldBy(durableBodies, command.node, context,
                           command.view, command.subject)
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>2, Isa
           DEF ExecuteDecisionFetch, AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <2>1, <3>1,
                   AsyncTransportContentTypeStutter
    <2>3. CASE ~BodyHeldBy(durableBodies, command.node, context,
                            command.view, command.subject)
      <3>1. PICK qc \in DecisionQcValues \cup prepareQCs:
               /\ CommandMatches(command, command.node,
                                   qc.view, qc.subject)
               /\ command.evidence = qc
               /\ CertifiedBodyRecoveryAuthority(command.node, qc)
               /\ PublishCertifiedRequests(
                    CertifiedRequestOutbox(command.node, qc))
        BY <1>1, <2>3 DEF ExecuteDecisionFetch
      <3>2. qc \in QcRecordSet
        BY <1>1, <3>1, SMT
           DEF StrongInductiveInvariant, Safety, TypeInvariant,
               DecisionQcValues
      <3>3. /\ IsFiniteSet(
                    CertifiedRequestOutbox(command.node, qc))
             /\ \A item \in CertifiedRequestOutbox(command.node, qc):
                  /\ AsyncItemTyped(item)
                  /\ item.kind = "CertifiedRequest"
        BY <1>1, <2>1, <3>1, <3>2,
           CertifiedRequestOutboxIsFiniteAndTyped
      <3>4. /\ UNCHANGED context
             /\ UNCHANGED
                  <<AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncCertifiedResponseClaim, asyncRetainedControl,
                    asyncHeldChunks>>
        BY <1>1, <2>3, <3>1, Isa
           DEF ExecuteDecisionFetch, PublishCertifiedRequests,
               AsyncCertifiedResponseClaimCoreAuthorityVars, vars
      <3> QED BY <2>1, <3>1, <3>3, <3>4,
                   PublishTrackedRequestsPreservesTransportContentType
                   DEF PublishCertifiedRequests
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ExecuteCoreDeliveryPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ AsyncCandidateTyped(command)
     /\ ExecuteCoreDelivery(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ AsyncCandidateTyped(command)
                /\ ExecuteCoreDelivery(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. AsyncTransportContentTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant
    <2>2. AsyncTransportHistoryTypeInvariant
      BY <2>1 DEF AsyncTransportContentTypeInvariant
    <2>3. AsyncSentItemsType(asyncSentItems)
      BY <2>2, AsyncTransportHistoryTypeDecomposition
    <2>4. /\ command.item \in asyncSentItems
           /\ AsyncItemTyped(command.item)
           /\ command.node \in ValidatorIds
      BY <1>1, <2>3, Isa
         DEF AsyncSentItemsType, AsyncCandidateTyped,
             ExecuteCoreDelivery
    <2>5. UNCHANGED context
      BY <1>1, ExecuteCoreDeliveryLeavesContext
    <2>6. CASE command.item.kind = "PrepareQC"
      <3>1. /\ command.item.envelope.qc \in QcRecordSet
             /\ RetainableControlBatch(
                  QcOutbox(command.node, command.item.envelope.qc),
                  CurrentVoters)
        BY <1>1, <2>4, <2>6, QcOutboxIsRetainable
           DEF AsyncItemTyped, QcEnvelopeSet
      <3>2. /\ asyncRetainedControl' =
                    RememberedControl(
                      asyncRetainedControl,
                      QcOutbox(
                        command.node, command.item.envelope.qc))
             /\ UNCHANGED
                  <<context,
                    AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncSentItems, asyncActiveRequests,
                    asyncCertifiedResponseClaim, asyncTransport,
                    asyncHeldChunks>>
        BY <1>1, <2>5, <2>6, Isa
           DEF ExecuteCoreDelivery,
               AsyncCertifiedResponseClaimCoreAuthorityVars, vars
      <3> QED BY <2>1, <3>1, <3>2,
           RememberRetainableControlPreservesTransportContentType
    <2>7. CASE command.item.kind # "PrepareQC"
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>5, <2>7, Isa
           DEF ExecuteCoreDelivery,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <2>1, <3>1, AsyncTransportContentTypeStutter
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM ExecuteChunkDeliveryPreservesTransportContentType ==
  \A command:
    (/\ AsyncTypeInvariant
     /\ AsyncCandidateTyped(command)
     /\ ExecuteChunkDelivery(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ AsyncTypeInvariant
                /\ AsyncCandidateTyped(command)
                /\ ExecuteChunkDelivery(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2> DEFINE Receipt ==
          AsyncChunkReceipt(
            command.node, command.item.envelope.view,
            command.item.envelope.subject,
            command.item.envelope.chunk)
    <2>1. /\ AsyncTransportHistoryTypeInvariant
           /\ AsyncPacketContentTypeInvariant
           /\ AsyncHeldChunksTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportContentTypeInvariant
    <2>2. /\ command.item \in asyncSentItems
           /\ AsyncItemTyped(command.item)
      BY <1>1, <2>1, Isa
         DEF AsyncTransportHistoryTypeInvariant, AsyncSentItemsType,
             ExecuteChunkDelivery
    <2>3. command.node \in ValidatorIds
      BY <1>1 DEF AsyncCandidateTyped
    <2>4. command.item.kind = "Chunk"
      BY <1>1 DEF ExecuteChunkDelivery
    <2>5. AsyncBodyEnvelopeTyped(command.item.envelope)
      BY <2>2, <2>4, SMT DEF AsyncItemTyped
    <2>6. /\ command.item.envelope.view \in Views
           /\ command.item.envelope.subject \in Subjects
      BY <2>5 DEF AsyncBodyEnvelopeTyped
    <2>7. command.item.envelope.chunk \in AsyncChunks
      BY <1>1 DEF ExecuteChunkDelivery
    <2>8. Receipt \in AsyncChunkReceiptSet
      BY <2>3, <2>6, <2>7,
         AsyncChunkReceiptConstructorTyped DEF Receipt
    <2>9. /\ UNCHANGED AsyncTransportHistoryTypeVars
           /\ UNCHANGED asyncTransport
           /\ asyncHeldChunks' = asyncHeldChunks \cup {Receipt}
      BY <1>1, Isa
         DEF ExecuteChunkDelivery,
             AsyncTransportHistoryTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars, Receipt, vars
    <2>10. AsyncTransportHistoryTypeInvariant'
      BY <2>1, <2>9, AsyncTransportHistoryTypeStutter
    <2>11. AsyncPacketContentTypeInvariant'
      BY <2>1, <2>9 DEF AsyncPacketContentTypeInvariant
    <2>12. AsyncHeldChunksTypeInvariant'
      BY <2>1, <2>8, <2>9, Isa
         DEF AsyncHeldChunksTypeInvariant
    <2> QED BY <2>10, <2>11, <2>12
         DEF AsyncTransportContentTypeInvariant
  <1> QED BY <1>1

THEOREM ExecuteCommandPreservesTransportContentType ==
  \A command:
    (/\ StrongInductiveInvariant
     /\ AsyncTypeInvariant
     /\ AsyncCandidateTyped(command)
     /\ ExecuteCommand(command))
      => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW command,
                /\ StrongInductiveInvariant
                /\ AsyncTypeInvariant
                /\ AsyncCandidateTyped(command)
                /\ ExecuteCommand(command)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. CASE ExecuteRegularCommand(command)
      BY <1>1, <2>1,
         ExecuteRegularCommandPreservesTransportContentType
    <2>2. CASE ExecuteDecisionFetch(command)
      BY <1>1, <2>2,
         ExecuteDecisionFetchPreservesTransportContentType
    <2>3. CASE ExecuteApply(command)
      BY <1>1, <2>3,
         ExecuteApplyPreservesTransportContentType
    <2>4. CASE ExecuteRejectAuthenticatedJunk(command)
      BY <1>1, <2>4,
         ExecuteRejectAuthenticatedJunkPreservesTransportContentType
    <2>5. CASE ExecuteSignProposal(command)
      BY <1>1, <2>5,
         ExecuteSignProposalPreservesTransportContentType
    <2>6. CASE ExecuteSignVote(command)
      BY <1>1, <2>6,
         ExecuteSignVotePreservesTransportContentType
    <2>7. CASE ExecuteFormPrepareQC(command)
      BY <1>1, <2>7,
         ExecuteFormPrepareQcPreservesTransportContentType
    <2>8. CASE ExecuteSignTimeout(command)
      BY <1>1, <2>8,
         ExecuteSignTimeoutPreservesTransportContentType
    <2>9. CASE ExecutePersistInstall(command)
      BY <1>1, <2>9,
         ExecutePersistInstallPreservesTransportContentType
    <2>10. CASE ExecutePersistDecision(command)
      BY <1>1, <2>10,
         ExecutePersistDecisionPreservesTransportContentType
    <2>11. CASE ExecuteRequestCertifiedBody(command)
      BY <1>1, <2>11,
         ExecuteRequestCertifiedBodyPreservesTransportContentType
    <2>12. CASE ExecuteCoreDelivery(command)
      BY <1>1, <2>12,
         ExecuteCoreDeliveryPreservesTransportContentType
    <2>13. CASE ExecuteChunkDelivery(command)
      BY <1>1, <2>13,
         ExecuteChunkDeliveryPreservesTransportContentType
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
                <2>6, <2>7, <2>8, <2>9, <2>10, <2>11,
                <2>12, <2>13
         DEF ExecuteCommand
  <1> QED BY <1>1

THEOREM DirectCommitDiscoveryPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ CommitCertificateDiscoveryStepWork(node)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                CommitCertificateDiscoveryStepWork(node)
         PROVE AsyncTransportContentTypeInvariant'
    <2> DEFINE Items == CommitCertificateRequestOutbox(node)
    <2>1. /\ IsFiniteSet(Items)
           /\ \A item \in Items:
                /\ AsyncItemTyped(item)
                /\ item.kind = "CommitCertificateRequest"
      BY <1>1, CommitCertificateRequestOutboxIsFiniteAndTyped
         DEF Items
    <2>2. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncTransportContentTypeInvariant
           /\ asyncActiveRequests' = asyncActiveRequests \cup Items
           /\ asyncSentItems' = asyncSentItems \cup Items
           /\ asyncTransport' = asyncTransport \cup PacketsForItems(Items)
           /\ UNCHANGED
                <<context,
                  AsyncCertifiedResponseClaimCoreAuthorityVars,
                  asyncCertifiedResponseClaim, asyncRetainedControl,
                  asyncHeldChunks>>
      BY <1>1, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncTransportTypeInvariant,
             CommitCertificateDiscoveryStepWork,
             PublishCommitCertificateRequests, Items, vars,
             AsyncDeferredVars,
             AsyncCertifiedResponseClaimCoreAuthorityVars
    <2>3. /\ AsyncCertifiedRequestLogicalIndexConsistent(Items)
           /\ AsyncCertifiedRequestSetsCompatible(
                asyncActiveRequests, Items)
      BY <2>1, SMT
         DEF AsyncCertifiedRequestLogicalIndexConsistent,
             AsyncCertifiedRequestSetsCompatible,
             AsyncCertifiedRequestsIn
    <2> QED BY <2>1, <2>2, <2>3,
                 PublishTrackedRequestsPreservesTransportContentType
  <1> QED BY <1>1

THEOREM DirectTimeoutPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DirectTimeoutStep(node)
    => AsyncTransportContentTypeInvariant'
BY AsyncTransportContentTypeStutter, Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, DirectTimeoutStep,
       BeginTimeoutEnabled, BeginTimeout, AppendCausalSuccessors,
       LeaveCausalQueues, AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars, vars

THEOREM DeferredTimeoutPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DeferredTimeoutStep(node)
    => AsyncTransportContentTypeInvariant'
BY AsyncTransportContentTypeStutter, Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, DeferredTimeoutStep,
       BeginTimeoutEnabled, BeginTimeout, AppendCausalSuccessors,
       LeaveCausalQueues, AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars, vars

THEOREM DirectRetransmitPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DirectRetransmitStep(node)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                DirectRetransmitStep(node)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. CASE NodeIdle(node) /\ RetryableItems(node) # {}
      <3>1. /\ SendNodeRetransmissions(node)
             /\ UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncHeldChunks>>
        BY <1>1, <2>1, Isa
           DEF DirectRetransmitStep,
               AsyncCertifiedResponseClaimCoreAuthorityVars, vars
      <3> QED BY <1>1, <3>1,
                   SendNodeRetransmissionsPreservesTransportContentType
    <2>2. CASE ~(NodeIdle(node) /\ RetryableItems(node) # {})
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>2, Isa
           DEF DirectRetransmitStep, NoSendItem,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <1>1, <3>1, AsyncTransportContentTypeStutter
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DeferredRetransmitPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DeferredRetransmitStep(node)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                DeferredRetransmitStep(node)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. CASE RetryableItems(node) # {}
      <3>1. /\ SendNodeRetransmissions(node)
             /\ UNCHANGED
                  <<context, AsyncCertifiedResponseClaimCoreAuthorityVars,
                    asyncHeldChunks>>
        BY <1>1, <2>1, Isa
           DEF DeferredRetransmitStep,
               AsyncCertifiedResponseClaimCoreAuthorityVars, vars
      <3> QED BY <1>1, <3>1,
                   SendNodeRetransmissionsPreservesTransportContentType
    <2>2. CASE RetryableItems(node) = {}
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>2, Isa
           DEF DeferredRetransmitStep, NoSendItem,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <1>1, <3>1, AsyncTransportContentTypeStutter
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DeferredTagPreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ DeferredTagStep(node)
    => AsyncTransportContentTypeInvariant'
BY DeferredTimeoutPreservesTransportContentType,
   DeferredRetransmitPreservesTransportContentType, Isa
   DEF DeferredTagStep

THEOREM IdleRuntimePreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ IdleRuntimeStep(node)
    => AsyncTransportContentTypeInvariant'
BY AsyncTransportContentTypeStutter, Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant, IdleRuntimeStep,
       LeaveCausalQueues, AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars, vars,
       AsyncDeferredVars

THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext

(***************************************************************************
GST remains a genuine weak-fairness consequence across finitely many
responsive-process generations.  Restart reconstructs an optional exact
locked-body Fetch prefix together with the current signature owner.  The normal
serialized runner drains that ordered replay, and only then may recovery install
the next durable signature from the retained tail.  Strict restart-generation
increase in the finite model makes the number of pre-GST rearm/crash cycles
finite, after which SetGST is continuously enabled.  This does not assume the
diagnostic pending-install generation predicate.
***************************************************************************)

AsyncRecoveryEligibleReady ==
  /\ ~gst
  /\ asyncRecoveryPhase = "Eligible"
  /\ Responsive \subseteq up

AsyncRecoveryRequiredPending ==
  /\ ~gst
  /\ asyncRecoveryPhase = "RestartRequired"

AsyncRecoveryReplayPending ==
  /\ ~gst
  /\ asyncRecoveryPhase = "ReplayRequired"

AsyncRecoveryReplayingPending ==
  /\ ~gst
  /\ asyncRecoveryPhase = "Replaying"

AsyncRecoveryRecoveredReady ==
  /\ ~gst
  /\ asyncRecoveryPhase = "Recovered"
  /\ Responsive \subseteq up

(***************************************************************************
An unconsumed locked Commit replay is carried by the exact remaining FIFO
candidate.  Once that candidate is installed into Core, the ordinary source
witnesses take over.  This carrier deliberately excludes recovery authority:
FinishResponsiveReplay removes that authority, so authority cannot justify
its own Finish guard.
***************************************************************************)
ReplayTailCommitReadyInvariant ==
  asyncRecoveryPhase = "Replaying" =>
    \A vote \in RestartLockedCommitIntents(asyncRecoveryNode):
      \/ ReplayCommitIntentReady(asyncRecoveryNode, vote)
      \/ ReplayLockedCommitCandidate(asyncRecoveryNode, vote)
           \in SequenceSet(asyncRecoveryReplayQueue)

ReplayCommitCarrierFrame ==
  /\ asyncRecoveryNode' = asyncRecoveryNode
  /\ (RestartLockedCommitIntents(asyncRecoveryNode))' =
       RestartLockedCommitIntents(asyncRecoveryNode)
  /\ \A vote \in RestartLockedCommitIntents(asyncRecoveryNode):
       /\ (ReplayCommitIntentReady(asyncRecoveryNode, vote)
             => (ReplayCommitIntentReady(asyncRecoveryNode, vote))')
       /\ (ReplayLockedCommitCandidate(asyncRecoveryNode, vote))' =
            ReplayLockedCommitCandidate(asyncRecoveryNode, vote)

THEOREM InitAtSetsAllValidatorsUp ==
  \A initialContext:
    InitAt(initialContext) => up = ValidatorIds
BY DEF InitAt

THEOREM AsyncInitStartsRecoveryEligibleReady ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncRecoveryEligibleReady
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncRecoveryEligibleReady
    <2>1. ModelConfiguration
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. Responsive \subseteq ValidatorIds
      BY <2>1, ModelResponsiveValidators
    <2>3. up = ValidatorIds
      BY <1>1, InitAtSetsAllValidatorsUp
         DEF AsyncInitAt, AsyncBaseInitAt
    <2> QED BY <1>1, <2>2, <2>3
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
             AsyncRecoveryInit, AsyncRecoveryEligibleReady
  <1> QED BY <1>1

THEOREM AsyncSetGstEnabledWhileReady ==
  (AsyncRecoveryEligibleReady \/ AsyncRecoveryRecoveredReady)
    => ENABLED <<AsyncSetGST>>_AsyncAllVars
BY ExpandENABLED, SMTT(30)
   DEF AsyncSetGST, SetGST, AsyncRecoveryEligibleReady,
       AsyncRecoveryRecoveredReady,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM AsyncSetGstEstablishesGst ==
  <<AsyncSetGST>>_AsyncAllVars => gst'
BY SMT DEF AsyncSetGST, SetGST

THEOREM FifoRuntimePreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ FifoRuntimeStep(node)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                FifoRuntimeStep(node)
         PROVE AsyncTransportContentTypeInvariant'
    <2> DEFINE Command == NextNodeCommand(node)
    <2>1. /\ NodeQueueNonempty(node)
           /\ AsyncCandidateTyped(Command)
      BY <1>1, RuntimeSelectedCommandsAreTyped DEF FifoRuntimeStep,
                                                       Command
    <2>2. CASE CommandDispatchable(Command)
      <3>1. ExecuteCommand(Command)
        BY <1>1, <2>2 DEF FifoRuntimeStep, Command
      <3> QED BY <1>1, <2>1, <3>1,
                   ExecuteCommandPreservesTransportContentType
    <2>3. CASE ~CommandDispatchable(Command)
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>3, Isa
           DEF FifoRuntimeStep, DeferCommand, DiscardCommand,
               Command, AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <1>1, <3>1, AsyncTransportContentTypeStutter
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM DeferredDrainRuntimePreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ DeferredDrainStep(node)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                DeferredDrainStep(node)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. CASE ~DeferredQueueNonempty(node)
      <3>1. UNCHANGED AsyncTransportContentTypeVars
        BY <1>1, <2>1, Isa
           DEF DeferredDrainStep, DeferredWorkServiceable,
               AsyncTransportContentTypeVars,
               AsyncCertifiedResponseClaimAuthorityVars, vars
      <3> QED BY <1>1, <3>1, AsyncTransportContentTypeStutter
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant
    <2>2. CASE DeferredQueueNonempty(node)
      <3> DEFINE Command == NextDeferredCommand(node)
      <3>1. AsyncCandidateTyped(Command)
        BY <1>1, <2>2, RuntimeSelectedCommandsAreTyped DEF Command
      <3>2. CASE DeferredHandoffAllowsExecution(node, Command)
        <4>1. ExecuteCommand(Command)
          BY <1>1, <2>2, <3>2 DEF DeferredDrainStep, Command
        <4> QED BY <1>1, <3>1, <4>1,
                     ExecuteCommandPreservesTransportContentType
      <3>3. CASE ~DeferredHandoffAllowsExecution(node, Command)
        <4>1. UNCHANGED AsyncTransportContentTypeVars
          BY <1>1, <2>2, <3>3, Isa
             DEF DeferredDrainStep, DiscardCommand,
                 Command, AsyncTransportContentTypeVars,
                 AsyncCertifiedResponseClaimAuthorityVars, vars
        <4> QED BY <1>1, <4>1, AsyncTransportContentTypeStutter
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncTransportTypeInvariant
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM SerializedRuntimePreservesTransportContentType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncTransportContentTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                SerializedRunnerRuntimeStep(node)
         PROVE AsyncTransportContentTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. CASE DeferredDrainStep(node)
      BY <1>1, <2>1, <2>2,
         DeferredDrainRuntimePreservesTransportContentType
    <2>3. CASE DeferredTagStep(node)
      BY <1>1, <2>1, <2>3,
         DeferredTagPreservesTransportContentType
    <2>4. CASE DirectTimeoutStep(node)
      BY <1>1, <2>1, <2>4,
         DirectTimeoutPreservesTransportContentType
    <2>5. CASE FifoRuntimeStep(node)
      BY <1>1, <2>1, <2>5,
         FifoRuntimePreservesTransportContentType
    <2>6. CASE DirectRetransmitStep(node)
      BY <1>1, <2>1, <2>6,
         DirectRetransmitPreservesTransportContentType
    <2>7. CASE IdleRuntimeStep(node)
      BY <1>1, <2>1, <2>7,
         IdleRuntimePreservesTransportContentType
    <2> QED BY <1>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep
  <1> QED BY <1>1

THEOREM SerializedRunnerRuntimePreservesHistoricalRecoveryType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncHistoricalRecoveryTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                SerializedRunnerRuntimeStep(node)
         PROVE AsyncHistoricalRecoveryTypeInvariant'
    <2>1. CASE DeferredDrainStep(node)
      BY <1>1, <2>1, DeferredDrainPreservesHistoricalRecoveryType
    <2>2. CASE FifoRuntimeStep(node)
      BY <1>1, <2>2, FifoRuntimePreservesHistoricalRecoveryType
    <2>3. CASE DeferredTagStep(node)
                   \/ DirectTimeoutStep(node)
                   \/ DirectRetransmitStep(node)
                   \/ IdleRuntimeStep(node)
      BY <1>1, <2>3,
         NonCommandRuntimeLeafPreservesHistoricalRecoveryType
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep
  <1> QED BY <1>1

THEOREM SerializedRuntimePreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ RunNodeWork(node)
    /\ SerializedRunnerRuntimeStep(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                AsyncControlServiceSlotTransition,
                RunNodeWork(node),
                SerializedRunnerRuntimeStep(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1
    <2>2. AsyncRuntimeScalarTypeInvariant'
      BY <1>1, SerializedRuntimePreservesScalarType
    <2>3. AsyncCausalTypeInvariant'
      BY <1>1, SerializedRuntimePreservesCausalType
    <2>4. AsyncIoTypeInvariant'
      BY <1>1, SerializedRuntimePreservesIoType
    <2>5. AsyncDeferredTypeInvariant'
      BY <1>1, SerializedRuntimePreservesDeferredType
    <2>6. AsyncTransportClockTypeInvariant'
      BY <1>1, SerializedRuntimePreservesTransportClockType
    <2>7. AsyncTransportContentTypeInvariant'
      BY <1>1, SerializedRuntimePreservesTransportContentType
    <2>8. AsyncIngressTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant
    <2>9. AsyncIngressTypeInvariant'
      BY <1>1, <2>1, <2>8,
         SerializedRuntimePreservesIngressType
    <2>10. AsyncHistoricalRecoveryTypeInvariant'
      BY <1>1, SerializedRunnerRuntimePreservesHistoricalRecoveryType
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, <2>9, <2>10
         DEF AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncTransportTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncServeIngressTargetOnlyPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ RunNodeWork(node)
    /\ AsyncServeIngressTargetOnlyTurn(node)
    => AsyncSchedulerTypeInvariant'
BY FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
   AsyncCausalTypeStutter,
   AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
   AsyncIoCapacityTypeStutter,
   AsyncDeferredTopologyTypeStutter, AsyncDeferredContentTypeStutter,
   RunnerServiceFramePreservesClockType,
   AsyncTransportContentTypeStutter,
   AsyncIngressTopologyTypeStutter,
   AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter,
   HistoricalRecoveryFramePreservesType,
   FunctionalUpdatePreservesType, IsaT(300)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
       AsyncServeIngressTargetOnlyTurn,
       RunNodeWork, RunnerServiceFrame,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
       AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
       AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       AsyncIngressTopologyTypeVars,
       AsyncHistoricalRecoveryFrameVars,
       AsyncConfiguration, vars

THEOREM CandidateProducerContinuationReplayTargetOnlyPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ RunNodeWork(node)
    /\ AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
    => AsyncSchedulerTypeInvariant'
BY FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
   AsyncCausalTypeStutter,
   AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
   AsyncIoCapacityTypeStutter,
   AsyncDeferredTopologyTypeStutter, AsyncDeferredContentTypeStutter,
   RunnerServiceFramePreservesClockType,
   AsyncTransportContentTypeStutter,
   AsyncIngressTopologyTypeStutter,
   AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter,
   HistoricalRecoveryFramePreservesType,
   FunctionalUpdatePreservesType, IsaT(300)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       RunNodeWork, RunnerServiceFrame,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
       AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
       AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       AsyncIngressTopologyTypeVars,
       AsyncHistoricalRecoveryFrameVars,
       AsyncConfiguration, vars

THEOREM CandidateProducerContinuationExactLocalReplayPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ RunNodeWork(node)
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
    => AsyncSchedulerTypeInvariant'
BY FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
   EnqueueCandidatePreservesIoType,
   TypedCandidateAppendPreservesQueueType,
   AsyncCausalTypeStutter,
   AsyncDeferredTopologyTypeStutter, AsyncDeferredContentTypeStutter,
   RunnerServiceFramePreservesClockType,
   AsyncTransportContentTypeStutter,
   AsyncIngressTopologyTypeStutter,
   AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter,
   HistoricalRecoveryFramePreservesType,
   FunctionalAppendUpdateAtKey, FunctionalUpdatePreservesType,
   FunctionalUpdateAwayFromKey, IsaT(900)
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
       AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationRecord,
       EnqueueCandidate, RunNodeWork, RunnerServiceFrame,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
       AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
       AsyncTransportContentTypeVars,
       AsyncCertifiedResponseClaimAuthorityVars,
       AsyncIngressTopologyTypeVars,
       AsyncHistoricalRecoveryFrameVars,
       AsyncConfiguration, vars

THEOREM CandidateProducerContinuationExactRuntimeReplayPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ RunNodeWork(node)
    /\ AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                AsyncControlServiceSlotTransition,
                RunNodeWork(node),
                AsyncCandidateProducerContinuationExactRuntimeReplayStep(
                  node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. SerializedRunnerRuntimeStep(node)
      BY <1>1 DEF SerializedRunnerRuntimeStep
    <2> QED BY <1>1, <2>1,
         SerializedRuntimePreservesSchedulerType
  <1> QED BY <1>1

THEOREM CandidateProducerContinuationReplayPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ RunNodeWork(node)
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                AsyncControlServiceSlotTransition,
                RunNodeWork(node),
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1,
         CandidateProducerContinuationExactLocalReplayPreservesSchedulerType
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <1>1, <2>2,
         CandidateProducerContinuationReplayTargetOnlyPreservesSchedulerType
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      BY <1>1, <2>3,
         CandidateProducerContinuationExactRuntimeReplayPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM RunNodeWorkConcreteActionCaseSplit ==
  \A node:
    RunNodeWork(node)
      => \/ ResolveRunNodeCandidateProducerContinuation(node)
         \/ ReplayRunNodeCandidateProducerContinuation(node)
         \/ LocalAdmissionStep(node)
         \/ IngressDrainStep(node)
         \/ SerializedRunnerRuntimeStep(node)
         \/ SerializedLocalPrecedesServeIngressStep(node)
         \/ AsyncServeIngressTargetOnlyTurn(node)
BY Isa
   DEF RunNodeWork, SerializedRunnerRuntimeStep

THEOREM InstallRunnerRunNodeWorkRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      RunNodeWork(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 RunNodeWork(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds, RunNodeWork(node)
           PROVE [Next]_vars
      <3>1r. CASE
                ResolveRunNodeCandidateProducerContinuation(node)
        BY <3>1r, Isa
           DEF ResolveRunNodeCandidateProducerContinuation, vars
      <3>1p. CASE
                ReplayRunNodeCandidateProducerContinuation(node)
        BY <1>1, <2>1, <3>1p,
           CandidateProducerContinuationReplayRefinesCoreBracketNext
      <3>1. CASE LocalAdmissionStep(node)
        BY <3>1, LocalAdmissionStepRefinesCoreBracketNext
      <3>2. CASE IngressDrainStep(node)
        BY <3>2, IngressDrainStepRefinesCoreBracketNext
      <3>3. CASE SerializedRunnerRuntimeStep(node)
        BY <1>1, <2>1, <3>3,
           SerializedRunnerRuntimeRefinesCoreBracketNext
      <3>4. CASE AsyncServeIngressTargetOnlyTurn(node)
        BY <3>4, AsyncServeIngressTargetOnlyRefinesCoreBracketNext
      <3>5. CASE SerializedLocalPrecedesServeIngressStep(node)
        BY <3>5,
           SerializedLocalPrecedesServeIngressRefinesCoreBracketNext
      <3> QED BY <2>1, <3>1r, <3>1p, <3>1, <3>2, <3>3, <3>4,
                    <3>5,
           RunNodeWorkConcreteActionCaseSplit
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RunNodeWorkPreservesSchedulerType ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ RunNodeWork(node)
    => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                AsyncControlServiceSlotTransition,
                RunNodeWork(node)
         PROVE AsyncSchedulerTypeInvariant'
    <2>1r. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>1r,
         FrozenServeStateAndSharedSchedulerTransitionPreservesServeOrdinalType,
         AsyncCausalTypeStutter,
         AsyncIoTopologyTypeStutter, AsyncIoContentTypeStutter,
         AsyncIoCapacityTypeStutter,
         AsyncDeferredTopologyTypeStutter, AsyncDeferredContentTypeStutter,
         RunnerServiceFramePreservesClockType,
         AsyncTransportContentTypeStutter,
         AsyncIngressTopologyTypeStutter,
         AsyncIngressCapacityTypeStutter, AsyncIngressContentTypeStutter,
         HistoricalRecoveryFramePreservesType,
         FunctionalUpdatePreservesType, IsaT(300)
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncTransportTypeInvariant, AsyncIngressTypeInvariant,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             RunNodeWork, RunnerServiceFrame,
             AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
             AsyncIoTopologyTypeVars, AsyncIoContentTypeVars,
             AsyncIoCapacityTypeVars, AsyncDeferredTopologyTypeVars,
             AsyncTransportContentTypeVars,
             AsyncCertifiedResponseClaimAuthorityVars,
             AsyncIngressTopologyTypeVars,
             AsyncHistoricalRecoveryFrameVars,
             AsyncConfiguration, vars
    <2>1p. CASE
              ReplayRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>1p,
         CandidateProducerContinuationReplayPreservesSchedulerType
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1, LocalAdmissionRunnerPreservesSchedulerType
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2, IngressAdmissionRunnerPreservesSchedulerType
    <2>3. CASE SerializedRunnerRuntimeStep(node)
      BY <1>1, <2>3, SerializedRuntimePreservesSchedulerType
    <2>4. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <1>1, <2>4,
         AsyncServeIngressTargetOnlyPreservesSchedulerType
    <2>5. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <1>1, <2>5,
         SerializedLocalPrecedesServeIngressPreservesSchedulerType
    <2> QED BY <1>1, <2>1r, <2>1p, <2>1, <2>2, <2>3, <2>4,
                  <2>5,
         RunNodeWorkConcreteActionCaseSplit
  <1> QED BY <1>1

THEOREM LocalAdmissionStepPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ LocalAdmissionStep(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY CertifiedResponseClaimIngressOwnershipStutter, Isa
   DEF LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       LeaveCausalQueues, EnqueueCandidate,
       AsyncDeferredVars, AsyncIoVars, AsyncLocalAdmissionVars,
       AsyncAuxVars, vars

THEOREM IngressDrainStepPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ IngressDrainStep(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                IngressDrainStep(node)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. CASE /\ asyncRunnerBudget[node] > 0
                 /\ asyncIngressReady[node] # <<>>
                 /\ DrainableIngressIndices(node) # {}
      <3>1. DrainFairIngressSelected(node)
        BY <1>1, <2>1 DEF IngressDrainStep
      <3> QED BY <1>1, <3>1,
           DrainFairIngressSelectedPreservesClaimIngressOwnership
    <2>2. CASE ~(asyncRunnerBudget[node] > 0
                   /\ asyncIngressReady[node] # <<>>
                   /\ DrainableIngressIndices(node) # {})
      BY <1>1, <2>2,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF IngressDrainStep, AsyncDeferredVars,
             AsyncLocalAdmissionVars, vars
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RunHistoricalServerPreservesClaimIngressOwnership ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ RunHistoricalServer(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in AsyncResponsiveAppliedArchiveServers,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                RunHistoricalServer(node)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncResponsiveAppliedArchiveServersAreValidators
    <2>2. CASE HistoricalDrainableIngressIndices(node) # {}
      <3>1. DrainHistoricalIngressSelected(node)
        BY <1>1, <2>2 DEF RunHistoricalServer
      <3> QED BY <1>1, <2>1, <3>1,
           DrainHistoricalIngressSelectedPreservesClaimIngressOwnership
    <2>3. CASE HistoricalDrainableIngressIndices(node) = {}
      BY <1>1, <2>3,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF RunHistoricalServer, HistoricalIdleStep
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AsyncServeIngressTargetOnlyPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ AsyncServeIngressTargetOnlyTurn(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY CertifiedResponseClaimIngressOwnershipStutter, Isa
   DEF AsyncServeIngressTargetOnlyTurn

THEOREM SerializedLocalPrecedesServeIngressPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY CertifiedResponseClaimIngressOwnershipStutter, Isa
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
       EnqueueCandidate, AsyncIoVars, AsyncDeferredVars, vars

THEOREM CandidateProducerContinuationExactLocalReplayPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
BY CertifiedResponseClaimIngressOwnershipStutter, Isa
   DEF AsyncCandidateProducerContinuationExactLocalReplayStep,
       EnqueueCandidate,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       vars

THEOREM CandidateProducerContinuationReplayPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      BY <1>1, <2>1,
         CandidateProducerContinuationExactLocalReplayPreservesClaimIngressOwnership
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      BY <1>1, <2>2,
         CertifiedResponseClaimIngressOwnershipStutter
         DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      <3>1. SerializedRunnerRuntimeStep(node)
        BY <2>3 DEF SerializedRunnerRuntimeStep
      <3> QED BY <1>1, <3>1,
           SerializedRuntimePreservesClaimIngressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM RunNodeWorkPreservesClaimIngressOwnership ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
    /\ RunNodeWork(node)
    => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncCertifiedResponseClaimIngressOwnershipInvariant,
                RunNodeWork(node)
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1r. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>1r,
         CertifiedResponseClaimIngressOwnershipStutter, Isa
         DEF ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService
    <2>1p. CASE
              ReplayRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>1p,
         CandidateProducerContinuationReplayPreservesClaimIngressOwnership
    <2>1. CASE LocalAdmissionStep(node)
      BY <1>1, <2>1,
         LocalAdmissionStepPreservesClaimIngressOwnership
    <2>2. CASE IngressDrainStep(node)
      BY <1>1, <2>2,
         IngressDrainStepPreservesClaimIngressOwnership
    <2>3. CASE SerializedRunnerRuntimeStep(node)
      BY <1>1, <2>3,
         SerializedRuntimePreservesClaimIngressOwnership
    <2>4. CASE AsyncServeIngressTargetOnlyTurn(node)
      BY <1>1, <2>4,
         AsyncServeIngressTargetOnlyPreservesClaimIngressOwnership
    <2>5. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <1>1, <2>5,
         SerializedLocalPrecedesServeIngressPreservesClaimIngressOwnership
    <2> QED BY <1>1, <2>1r, <2>1p, <2>1, <2>2, <2>3, <2>4,
                  <2>5,
         RunNodeWorkConcreteActionCaseSplit
  <1> QED BY <1>1

THEOREM AsyncRunnerStepPreservesClaimIngressOwnership ==
  /\ AsyncTypeInvariant
  /\ AsyncCertifiedResponseClaimIngressOwnershipInvariant
  /\ AsyncRunnerStep
  => AsyncCertifiedResponseClaimIngressOwnershipInvariant'
PROOF
  <1>1. ASSUME AsyncTypeInvariant,
              AsyncCertifiedResponseClaimIngressOwnershipInvariant,
              AsyncRunnerStep
         PROVE AsyncCertifiedResponseClaimIngressOwnershipInvariant'
    <2>1. CASE \E node \in AsyncCurrentResponsiveVoters:
                    RunNode(node)
      <3>1. PICK node \in AsyncCurrentResponsiveVoters:
               RunNode(node)
        BY <2>1
      <3>2. node \in ValidatorIds
        BY <1>1, <3>1
           DEF AsyncTypeInvariant, TypeInvariant,
               AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
               ModelConfiguration, QuorumConfiguration, ValidatorIds
      <3> QED BY <1>1, <3>1, <3>2,
           RunNodeWorkPreservesClaimIngressOwnership
           DEF RunNode
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(node)
      <3>1. PICK node \in asyncHistoricalRecoveryTargets:
               RunHistoricalRecoveryNode(node)
        BY <2>2
      <3>2. node \in ValidatorIds
        BY <1>1, <3>1, HistoricalRecoveryTargetsAreValidators
      <3> QED BY <1>1, <3>1, <3>2,
           RunNodeWorkPreservesClaimIngressOwnership
           DEF RunHistoricalRecoveryNode
    <2>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                    RunHistoricalServer(node)
      <3>1. PICK node \in AsyncResponsiveAppliedArchiveServers:
               RunHistoricalServer(node)
        BY <2>3
      <3> QED BY <1>1, <3>1,
           RunHistoricalServerPreservesClaimIngressOwnership
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

=============================================================================
