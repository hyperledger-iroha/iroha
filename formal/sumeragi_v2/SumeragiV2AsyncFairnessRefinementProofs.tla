---- MODULE SumeragiV2AsyncFairnessRefinementProofs ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

(***************************************************************************
Deductive refinement of the exact weak-fairness action inventory to the one
canonical asynchronous transition relation.  The proof is intentionally kept
out of the executable network module: TLC evaluates the small, fully framed
fair actions, while TLAPS checks their Core and outer-state projections here.
***************************************************************************)

THEOREM CoreStutterRefinesBracketNext ==
  UNCHANGED vars => [Next]_vars
BY PTL

THEOREM SetGstRefinesCoreNext ==
  SetGST => Next
BY DEF Next

THEOREM RestartRefinesCoreNext ==
  \A node \in ValidatorIds:
    Restart(node) => Next
BY DEF Next

THEOREM ResumeProposalRefinesCoreNext ==
  ModelConfiguration =>
    \A node, proposal:
      ResumeProposal(node, proposal) => Next
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE \A node, proposal:
                 ResumeProposal(node, proposal) => Next
    <2>1. ASSUME NEW node, NEW proposal,
                  ResumeProposal(node, proposal)
           PROVE Next
      <3>1. /\ node \in ValidatorIds
             /\ proposal \in proposalIntents
        BY <1>1, <2>1, Isa
           DEF ResumeProposal, ModelConfiguration, QuorumConfiguration
      <3>2. \E selectedNode \in ValidatorIds,
                  selectedProposal \in proposalIntents:
                  ResumeProposal(selectedNode, selectedProposal)
        BY <2>1, <3>1
      <3> QED BY <3>2 DEF Next
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ResumeVoteRefinesCoreNext ==
  ModelConfiguration =>
    \A node, vote:
      ResumeVote(node, vote) => Next
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE \A node, vote:
                 ResumeVote(node, vote) => Next
    <2>1. ASSUME NEW node, NEW vote,
                  ResumeVote(node, vote)
           PROVE Next
      <3>1. /\ node \in ValidatorIds
             /\ vote \in prepareIntents \cup commitIntents
        BY <1>1, <2>1, Isa
           DEF ResumeVote, VoteResumeAuthorized,
               ModelConfiguration, QuorumConfiguration
      <3>2. \E selectedNode \in ValidatorIds,
                  selectedVote \in prepareIntents \cup commitIntents:
                  ResumeVote(selectedNode, selectedVote)
        BY <2>1, <3>1
      <3> QED BY <3>2 DEF Next
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ResumeTimeoutRefinesCoreNext ==
  ModelConfiguration =>
    \A node, vote:
      ResumeTimeout(node, vote) => Next
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE \A node, vote:
                 ResumeTimeout(node, vote) => Next
    <2>1. ASSUME NEW node, NEW vote,
                  ResumeTimeout(node, vote)
           PROVE Next
      <3>1. /\ node \in ValidatorIds
             /\ vote \in timeoutIntents
        BY <1>1, <2>1, Isa
           DEF ResumeTimeout, ModelConfiguration, QuorumConfiguration
      <3>2. \E selectedNode \in ValidatorIds,
                  selectedVote \in timeoutIntents:
                  ResumeTimeout(selectedNode, selectedVote)
        BY <2>1, <3>1
      <3> QED BY <3>2 DEF Next
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RecoveryCoreReplayRefinesBracketNext ==
  ModelConfiguration =>
    \A node, candidate:
      RecoveryCoreReplay(node, candidate) => [Next]_vars
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE \A node, candidate:
                 RecoveryCoreReplay(node, candidate) => [Next]_vars
    <2>1. ASSUME NEW node, NEW candidate,
                  RecoveryCoreReplay(node, candidate)
           PROVE [Next]_vars
      <3>1. CASE candidate.kind = "SignProposal"
        <4>1. ResumeProposal(node, candidate.evidence)
          BY <2>1, <3>1 DEF RecoveryCoreReplay
        <4>2. Next
          BY <1>1, <4>1, ResumeProposalRefinesCoreNext
        <4> QED BY <4>2
      <3>2. CASE candidate.kind = "SignVote"
        <4>1. ResumeVote(node, candidate.evidence)
          BY <2>1, <3>2 DEF RecoveryCoreReplay
        <4>2. Next
          BY <1>1, <4>1, ResumeVoteRefinesCoreNext
        <4> QED BY <4>2
      <3>3. CASE candidate.kind = "SignTimeout"
        <4>1. ResumeTimeout(node, candidate.evidence)
          BY <2>1, <3>3 DEF RecoveryCoreReplay
        <4>2. Next
          BY <1>1, <4>1, ResumeTimeoutRefinesCoreNext
        <4> QED BY <4>2
      <3>4. CASE candidate.kind \notin
                    {"SignProposal", "SignVote", "SignTimeout"}
        BY <2>1, <3>4 DEF RecoveryCoreReplay
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM BodyRecordMembershipProjectsSubject ==
  \A node, contextValue, roundView, subject:
    BodyRecord(node, contextValue, roundView, subject) \in BodyRecordSet
      => subject \in Subjects
BY SMT DEF BodyRecord, BodyRecordSet

THEOREM RegularCoreCommandRefinesCoreNext ==
  TypeInvariant =>
    \A command:
      /\ AsyncCandidateTyped(command)
      /\ RegularCoreCommand(command)
      => Next
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A command:
                 /\ AsyncCandidateTyped(command)
                 /\ RegularCoreCommand(command)
                 => Next
    <2>1. ASSUME NEW command,
                  AsyncCandidateTyped(command),
                  RegularCoreCommand(command)
           PROVE Next
      <3>1. CASE command.kind = "AssembleBody"
                  /\ CommandMatches(
                       command, command.node, nodeView[command.node],
                       command.subject)
                  /\ AssembleLocalBody(command.node, command.subject)
        <4>1. /\ command.node \in ValidatorIds
               /\ command.subject \in Subjects
          BY <1>1, <2>1, <3>1, Isa
             DEF TypeInvariant, ModelConfiguration,
                 AssembleLocalBody
        <4>2. \E node \in ValidatorIds, subject \in Subjects:
                 AssembleLocalBody(node, subject)
          BY <3>1, <4>1
        <4> QED BY <4>2 DEF Next
      <3>2. CASE command.kind = "BeginProposal"
                  /\ BeginLocalProposal(command.node, command.subject)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. command.subject \in Subjects
          BY <3>2, Isa
             DEF BeginLocalProposal, ProposalWireValidFor,
                 LocalProposalFor, Proposal
        <4>3. \E node \in ValidatorIds, subject \in Subjects:
                 BeginLocalProposal(node, subject)
          BY <3>2, <4>1, <4>2
        <4> QED BY <4>3 DEF Next
      <3>3. CASE command.kind = "PersistProposal"
                  /\ \E request \in pendingProposal:
                       /\ CommandMatches(
                            command, request.node, request.proposal.view,
                            request.proposal.subject)
                       /\ PersistProposal(request)
        BY <3>3 DEF Next
      <3>4. CASE command.kind = "FetchBody"
                  /\ ~CertifiedRecoveryFetchFrontier(command)
                  /\ HeldChunksFor(command.node, command.view,
                                     command.subject) = AsyncChunks
                  /\ ~BodyHeldBy(
                        durableBodies, command.node, context,
                        command.view, command.subject)
                  /\ \E proposal \in SeenProposalValues:
                       /\ CommandMatches(
                            command, command.node, proposal.view,
                            proposal.subject)
                       /\ FetchBody(command.node, proposal)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. \E node \in ValidatorIds,
                    proposal \in SeenProposalValues:
                 FetchBody(node, proposal)
          BY <3>4, <4>1
        <4> QED BY <4>2 DEF Next
      <3>5. CASE command.kind = "RebindRetainedBody"
                  /\ \E proposal \in SeenProposalValues:
                       /\ CommandMatches(
                            command, command.node, proposal.view,
                            proposal.subject)
                       /\ RebindRetainedBody(command.node, proposal)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. \E node \in ValidatorIds,
                    proposal \in SeenProposalValues:
                 RebindRetainedBody(node, proposal)
          BY <3>5, <4>1
        <4> QED BY <4>2 DEF Next
      <3>6. CASE command.kind = "StoreBody"
                  /\ StoreBody(
                       command.node, command.view, command.subject)
        <4>1. /\ command.node \in ValidatorIds
               /\ command.view \in Views
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. BodyRecord(command.node, context, command.view,
                         command.subject) \in BodyRecordSet
          BY <1>1, <3>6, Isa DEF TypeInvariant, StoreBody
        <4>3. command.subject \in Subjects
          BY <4>2, BodyRecordMembershipProjectsSubject
        <4>4. \E node \in ValidatorIds, roundView \in Views,
                    subject \in Subjects:
                 StoreBody(node, roundView, subject)
          BY <3>6, <4>1, <4>3
        <4> QED BY <4>4 DEF Next
      <3>7. CASE command.kind = "ValidateBody"
                  /\ (\/ \E proposal \in SeenProposalValues:
                            /\ CommandMatches(
                                 command, command.node, proposal.view,
                                 proposal.subject)
                            /\ (ValidateBody(command.node, proposal)
                                  \/ RejectBody(command.node, proposal))
                      \/ \E qc \in DecisionQcValues:
                            /\ CommandMatches(
                                 command, command.node, qc.view,
                                 qc.subject)
                            /\ ValidateDecidedBody(command.node, qc)
                      \/ \E qc \in prepareQCs:
                            /\ CommandMatches(
                                 command, command.node, qc.view,
                                 qc.subject)
                            /\ ValidateLockedBody(command.node, qc))
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>7, <4>1 DEF Next
      <3>8. CASE command.kind = "BeginPrepare"
                  /\ \E proposal \in SeenProposalValues:
                       /\ CommandMatches(
                            command, command.node, proposal.view,
                            proposal.subject)
                       /\ BeginPrepare(command.node, proposal)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>8, <4>1 DEF Next
      <3>9. CASE command.kind = "PersistPrepare"
                  /\ \E request \in pendingPrepare:
                       /\ CommandMatches(
                            command, request.node, request.vote.view,
                            request.vote.subject)
                       /\ PersistPrepare(request)
        BY <3>9 DEF Next
      <3>10. CASE command.kind = "BeginObservePrepare"
                   /\ \E qc \in ReceivedQcValues:
                        /\ CommandMatches(
                             command, command.node, qc.view, qc.subject)
                        /\ BeginObservePrepare(command.node, qc)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>10, <4>1 DEF Next
      <3>11. CASE command.kind = "PersistObservePrepare"
                   /\ \E request \in pendingObservePrepare:
                        /\ CommandMatches(
                             command, request.node, request.qc.view,
                             request.qc.subject)
                        /\ PersistObservePrepare(request)
        BY <3>11 DEF Next
      <3>12. CASE command.kind = "BeginLockCommit"
                   /\ \E qc \in LockCommitQcValues:
                        /\ CommandMatches(
                             command, command.node, qc.view, qc.subject)
                        /\ BeginLockCommit(command.node, qc)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>12, <4>1 DEF Next
      <3>13. CASE command.kind = "PersistLockCommit"
                   /\ \E request \in pendingLockCommit:
                        /\ CommandMatches(
                             command, request.node, request.qc.view,
                             request.qc.subject)
                        /\ PersistLockCommit(request)
        BY <3>13 DEF Next
      <3>14. CASE command.kind = "FormCommitQC"
                   /\ FormCommitQC(
                        command.node, command.view, command.subject)
        <4>1. /\ command.node \in ValidatorIds
               /\ command.view \in Views
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. command.subject \in Subjects
          BY <3>14, Isa DEF FormCommitQC, QcRecordSet, QC
        <4>3. \E node \in ValidatorIds, roundView \in Views,
                    subject \in Subjects:
                 FormCommitQC(node, roundView, subject)
          BY <3>14, <4>1, <4>2
        <4> QED BY <4>3 DEF Next
      <3>15. CASE command.kind = "BeginDecision"
                   /\ \E qc \in ReceivedQcValues:
                        /\ CommandMatches(
                             command, command.node, qc.view, qc.subject)
                        /\ BeginDecision(command.node, qc)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>15, <4>1 DEF Next
      <3>16. CASE command.kind = "PersistTimeout"
                   /\ \E request \in pendingTimeout:
                        /\ CommandMatches(
                             command, request.node, request.vote.view,
                             request.vote.highSubject)
                        /\ PersistTimeout(request)
        BY <3>16 DEF Next
      <3>17. CASE command.kind = "FormTC"
                   /\ FormTC(command.node, command.view)
        <4>1. /\ command.node \in ValidatorIds
               /\ command.view \in Views
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>17, <4>1 DEF Next
      <3>18. CASE command.kind = "BeginInstallTC"
                   /\ \E tc \in ReceivedTcValues:
                        /\ command.node = command.node
                        /\ command.view = tc.view
                        /\ BeginInstallTC(command.node, tc)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4> QED BY <3>18, <4>1 DEF Next
      <3>19. CASE command.kind = "FetchCertifiedBody"
                   /\ command.item.kind = "CertifiedResponse"
                   /\ command.item.envelope.recipient = command.node
                   /\ command.item.envelope.view = command.view
                   /\ command.item.envelope.subject = command.subject
                   /\ CertifiedResponseCapabilityAuthorized(command.item)
                   /\ AcceptCertifiedResponseCapability(
                        command.node, command.view, command.subject)
        <4>1. /\ command.node \in ValidatorIds
               /\ command.view \in Views
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. BodyRecord(
                 command.node, context, command.view, command.subject)
                 \in BodyRecordSet
          BY <3>19, Isa
             DEF AcceptCertifiedResponseCapability,
                 InstallCertifiedBodyEffect
        <4>3. command.subject \in Subjects
          BY <4>2, BodyRecordMembershipProjectsSubject
        <4>4. \E node \in ValidatorIds, roundView \in Views,
                    subject \in Subjects:
                 AcceptCertifiedResponseCapability(
                   node, roundView, subject)
          BY <3>19, <4>1, <4>3
        <4> QED BY <4>4 DEF Next
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                   <3>7, <3>8, <3>9, <3>10, <3>11, <3>12, <3>13,
                   <3>14, <3>15, <3>16, <3>17, <3>18, <3>19
           DEF RegularCoreCommand
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM FormPrepareQcRefinesCoreNext ==
  \A node \in ValidatorIds, roundView \in Views:
    \A subject:
      FormPrepareQC(node, roundView, subject) => Next
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW roundView \in Views,
                NEW subject,
                FormPrepareQC(node, roundView, subject)
         PROVE Next
    <2>1. subject \in Subjects
      BY <1>1, Isa DEF FormPrepareQC, QcRecordSet, QC
    <2>2. \E selectedNode \in ValidatorIds,
                selectedView \in Views,
                selectedSubject \in Subjects:
             FormPrepareQC(selectedNode, selectedView, selectedSubject)
      BY <1>1, <2>1
    <2> QED BY <2>2 DEF Next
  <1> QED BY <1>1

THEOREM ExecuteCoreDeliveryRefinesCoreNext ==
  \A command:
    ExecuteCoreDelivery(command) => Next
PROOF
  <1>1. ASSUME NEW command, ExecuteCoreDelivery(command)
         PROVE Next
    <2>1. CASE /\ command.kind = "DeliverProposal"
                /\ command.item.kind = "Proposal"
                /\ DeliverProposal(command.item.envelope)
      <3>1. command.item.envelope \in proposalNetwork
        BY <2>1 DEF DeliverProposal
      <3>2. \E envelope \in proposalNetwork:
               DeliverProposal(envelope)
        BY <2>1, <3>1
      <3> QED BY <3>2 DEF Next
    <2>2. CASE /\ command.kind = "DeliverVote"
                /\ command.item.kind
                     \in {"PrepareVote", "CommitVote"}
                /\ DeliverVote(command.item.envelope)
      <3>1. command.item.envelope \in voteNetwork
        BY <2>2 DEF DeliverVote
      <3>2. \E envelope \in voteNetwork: DeliverVote(envelope)
        BY <2>2, <3>1
      <3> QED BY <3>2 DEF Next
    <2>3. CASE /\ command.kind = "DeliverQC"
                /\ command.item.kind \in {"PrepareQC", "CommitQC"}
                /\ DeliverQC(command.item.envelope)
      <3>1. command.item.envelope \in qcNetwork
        BY <2>3 DEF DeliverQC
      <3>2. \E envelope \in qcNetwork: DeliverQC(envelope)
        BY <2>3, <3>1
      <3> QED BY <3>2 DEF Next
    <2>4. CASE /\ command.kind = "DeliverTimeout"
                /\ command.item.kind = "TimeoutVote"
                /\ DeliverTimeout(command.item.envelope)
      <3>1. command.item.envelope \in timeoutNetwork
        BY <2>4 DEF DeliverTimeout
      <3>2. \E envelope \in timeoutNetwork:
               DeliverTimeout(envelope)
        BY <2>4, <3>1
      <3> QED BY <3>2 DEF Next
    <2>5. CASE /\ command.kind = "DeliverTC"
                /\ command.item.kind = "TimeoutCertificate"
                /\ DeliverTC(command.item.envelope)
      <3>1. command.item.envelope \in tcNetwork
        BY <2>5 DEF DeliverTC
      <3>2. \E envelope \in tcNetwork: DeliverTC(envelope)
        BY <2>5, <3>1
      <3> QED BY <3>2 DEF Next
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF ExecuteCoreDelivery
  <1> QED BY <1>1

THEOREM ExecuteCommandRefinesCoreBracketNext ==
  TypeInvariant =>
    \A command:
      /\ AsyncCandidateTyped(command)
      /\ ExecuteCommand(command)
      => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A command:
                 /\ AsyncCandidateTyped(command)
                 /\ ExecuteCommand(command)
                 => [Next]_vars
    <2>1. ASSUME NEW command,
                  AsyncCandidateTyped(command),
                  ExecuteCommand(command)
           PROVE [Next]_vars
      <3>1. CASE ExecuteRegularCommand(command)
        <4>1. RegularCoreCommand(command)
          BY <3>1 DEF ExecuteRegularCommand
        <4>2. Next
          BY <1>1, <2>1, <4>1,
             RegularCoreCommandRefinesCoreNext
        <4> QED BY <4>2
      <3>2. CASE ExecuteDecisionFetch(command)
        <4>1. UNCHANGED vars
          BY <3>2, Isa DEF ExecuteDecisionFetch
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>3. CASE ExecuteSignProposal(command)
        <4>1. \E request \in signProposals:
                 CompleteProposalSignature(request)
          BY <3>3 DEF ExecuteSignProposal
        <4>2. Next BY <4>1 DEF Next
        <4> QED BY <4>2
      <3>4. CASE ExecuteSignVote(command)
        <4>1. \E request \in signVotes:
                 CompleteVoteSignature(request)
          BY <3>4 DEF ExecuteSignVote
        <4>2. Next BY <4>1 DEF Next
        <4> QED BY <4>2
      <3>5. CASE ExecuteFormPrepareQC(command)
        <4>1. /\ command.node \in ValidatorIds
               /\ command.view \in Views
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. FormPrepareQC(
                 command.node, command.view, command.subject)
          BY <3>5 DEF ExecuteFormPrepareQC
        <4>3. Next
          BY <4>1, <4>2, FormPrepareQcRefinesCoreNext
        <4> QED BY <4>3
      <3>6. CASE ExecuteSignTimeout(command)
        <4>1. \E request \in signTimeouts:
                 CompleteTimeoutSignature(request)
          BY <3>6 DEF ExecuteSignTimeout
        <4>2. Next BY <4>1 DEF Next
        <4> QED BY <4>2
      <3>7. CASE ExecutePersistInstall(command)
        <4>1. \E request \in pendingInstallTC:
                 PersistInstallTC(request)
          BY <3>7 DEF ExecutePersistInstall
        <4>2. Next BY <4>1 DEF Next
        <4> QED BY <4>2
      <3>8. CASE ExecutePersistDecision(command)
        <4>1. \E request \in pendingDecision:
                 PersistDecision(request)
          BY <3>8 DEF ExecutePersistDecision
        <4>2. Next BY <4>1 DEF Next
        <4> QED BY <4>2
      <3>9. CASE ExecuteRequestCertifiedBody(command)
        <4>1. UNCHANGED vars
          BY <3>9 DEF ExecuteRequestCertifiedBody
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>10. CASE ExecuteApply(command)
        <4>1. command.node \in ValidatorIds
          BY <2>1 DEF AsyncCandidateTyped
        <4>2. \E qc \in DecisionQcValues:
                  ApplyDecision(command.node, qc)
          BY <3>10 DEF ExecuteApply
        <4>3. Next BY <4>1, <4>2 DEF Next
        <4> QED BY <4>3
      <3>11. CASE ExecuteCoreDelivery(command)
        <4>1. Next
          BY <3>11, ExecuteCoreDeliveryRefinesCoreNext
        <4> QED BY <4>1
      <3>12. CASE ExecuteChunkDelivery(command)
        <4>1. UNCHANGED vars
          BY <3>12 DEF ExecuteChunkDelivery
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>13. CASE ExecuteRejectAuthenticatedJunk(command)
        <4>1. UNCHANGED vars
          BY <3>13 DEF ExecuteRejectAuthenticatedJunk
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                   <3>7, <3>8, <3>9, <3>10, <3>11, <3>12, <3>13
           DEF ExecuteCommand
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM BeginTimeoutRefinesCoreNext ==
  \A node \in ValidatorIds:
    BeginTimeout(node) => Next
BY DEF Next

THEOREM DirectTimeoutStepRefinesCoreBracketNext ==
  \A node \in ValidatorIds:
    DirectTimeoutStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, DirectTimeoutStep(node)
         PROVE [Next]_vars
    <2>1. CASE BeginTimeoutEnabled(node)
      <3>1. BeginTimeout(node)
        BY <1>1, <2>1 DEF DirectTimeoutStep
      <3>2. Next BY <1>1, <3>1, BeginTimeoutRefinesCoreNext
      <3> QED BY <3>2
    <2>2. CASE ~BeginTimeoutEnabled(node)
      <3>1. UNCHANGED vars
        BY <1>1, <2>2 DEF DirectTimeoutStep
      <3> QED BY <3>1, CoreStutterRefinesBracketNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DeferredTimeoutStepRefinesCoreBracketNext ==
  \A node \in ValidatorIds:
    DeferredTimeoutStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, DeferredTimeoutStep(node)
         PROVE [Next]_vars
    <2>1. CASE BeginTimeoutEnabled(node)
      <3>1. BeginTimeout(node)
        BY <1>1, <2>1 DEF DeferredTimeoutStep
      <3>2. Next BY <1>1, <3>1, BeginTimeoutRefinesCoreNext
      <3> QED BY <3>2
    <2>2. CASE ~BeginTimeoutEnabled(node)
      <3>1. UNCHANGED vars
        BY <1>1, <2>2 DEF DeferredTimeoutStep
      <3> QED BY <3>1, CoreStutterRefinesBracketNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DeferredTagStepRefinesCoreBracketNext ==
  \A node \in ValidatorIds:
    DeferredTagStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, DeferredTagStep(node)
         PROVE [Next]_vars
    <2>1. CASE DeferredTimeoutExecutable(node)
      <3>1. DeferredTimeoutStep(node)
        BY <1>1, <2>1 DEF DeferredTagStep
      <3> QED BY <1>1, <3>1,
                   DeferredTimeoutStepRefinesCoreBracketNext
    <2>2. CASE ~DeferredTimeoutExecutable(node)
      <3>1. UNCHANGED vars
        BY <1>1, <2>2 DEF DeferredTagStep, DeferredRetransmitStep
      <3> QED BY <3>1, CoreStutterRefinesBracketNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM FifoRuntimeStepRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      FifoRuntimeStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 FifoRuntimeStep(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds, FifoRuntimeStep(node)
           PROVE [Next]_vars
      <3>1. CASE CommandDispatchable(NextNodeCommand(node))
        <4>1. /\ AsyncCandidateTyped(NextNodeCommand(node))
               /\ ExecuteCommand(NextNodeCommand(node))
          BY <2>1, <3>1 DEF FifoRuntimeStep, CommandDispatchable
        <4> QED BY <1>1, <4>1,
                     ExecuteCommandRefinesCoreBracketNext
      <3>2. CASE /\ ~CommandDispatchable(NextNodeCommand(node))
                   /\ ~NodeIdle(node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>2 DEF FifoRuntimeStep, DeferCommand
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>3. CASE /\ ~CommandDispatchable(NextNodeCommand(node))
                   /\ NodeIdle(node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>3 DEF FifoRuntimeStep, DiscardCommand
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM DeferredDrainStepRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      DeferredDrainStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 DeferredDrainStep(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds,
                  DeferredDrainStep(node)
           PROVE [Next]_vars
      <3>1. CASE ~DeferredQueueNonempty(node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>1 DEF DeferredDrainStep
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>2. CASE /\ DeferredQueueNonempty(node)
                   /\ CommandDispatchable(NextDeferredCommand(node))
        <4>1. /\ AsyncCandidateTyped(NextDeferredCommand(node))
               /\ ExecuteCommand(NextDeferredCommand(node))
          BY <2>1, <3>2 DEF DeferredDrainStep, CommandDispatchable
        <4> QED BY <1>1, <4>1,
                     ExecuteCommandRefinesCoreBracketNext
      <3>3. CASE /\ DeferredQueueNonempty(node)
                   /\ ~CommandDispatchable(NextDeferredCommand(node))
                   /\ ~NodeIdle(node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>3 DEF DeferredDrainStep
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>4. CASE /\ DeferredQueueNonempty(node)
                   /\ ~CommandDispatchable(NextDeferredCommand(node))
                   /\ NodeIdle(node)
        <4>1. UNCHANGED vars
          BY <2>1, <3>4 DEF DeferredDrainStep, DiscardCommand
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RuntimeStepRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      RuntimeStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 RuntimeStep(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds, RuntimeStep(node)
           PROVE [Next]_vars
      <3>1. CASE DeferredDrainStep(node)
        BY <1>1, <2>1, <3>1,
           DeferredDrainStepRefinesCoreBracketNext
      <3>2. CASE DeferredTagStep(node)
        BY <2>1, <3>2, DeferredTagStepRefinesCoreBracketNext
      <3>3. CASE DirectTimeoutStep(node)
        BY <2>1, <3>3, DirectTimeoutStepRefinesCoreBracketNext
      <3>4. CASE FifoRuntimeStep(node)
        BY <1>1, <2>1, <3>4,
           FifoRuntimeStepRefinesCoreBracketNext
      <3>5. CASE DirectRetransmitStep(node)
        <4>1. UNCHANGED vars
          BY <3>5 DEF DirectRetransmitStep
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>6. CASE IdleRuntimeStep(node)
        <4>1. UNCHANGED vars
          BY <3>6 DEF IdleRuntimeStep
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6
           DEF RuntimeStep
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM LocalAdmissionStepRefinesCoreBracketNext ==
  \A node:
    LocalAdmissionStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node, LocalAdmissionStep(node)
         PROVE [Next]_vars
    <2>1. UNCHANGED vars
      BY <1>1, Isa
         DEF LocalAdmissionStep, AdmitProducerCompletion,
             AdmitCausalHead
    <2> QED BY <2>1, CoreStutterRefinesBracketNext
  <1> QED BY <1>1

SelectedFairIngressItem(node) ==
  SelectedIngressItemAt(node, FirstDrainableIngressIndex(node))

THEOREM DrainFairIngressSelectedCoreChoice ==
  \A node:
    DrainFairIngressSelected(node)
      => \/ ImportAuthenticatedCommitCertificate(
               SelectedFairIngressItem(node).envelope)
         \/ UNCHANGED vars
BY Isa
   DEF DrainFairIngressSelected, SelectedFairIngressItem

THEOREM ImportCommitCertificateRefinesCoreNext ==
  \A envelope:
    ImportAuthenticatedCommitCertificate(envelope) => Next
BY DEF ImportAuthenticatedCommitCertificate, Next

THEOREM DrainFairIngressSelectedRefinesCoreBracketNext ==
  \A node:
    DrainFairIngressSelected(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node, DrainFairIngressSelected(node)
         PROVE [Next]_vars
    <2>1. \/ ImportAuthenticatedCommitCertificate(
                 SelectedFairIngressItem(node).envelope)
           \/ UNCHANGED vars
      BY <1>1, DrainFairIngressSelectedCoreChoice
    <2>2. CASE ImportAuthenticatedCommitCertificate(
                  SelectedFairIngressItem(node).envelope)
      <3>1. Next BY <2>2, ImportCommitCertificateRefinesCoreNext
      <3> QED BY <3>1
    <2>3. CASE UNCHANGED vars
      BY <2>3, CoreStutterRefinesBracketNext
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM IngressDrainStepRefinesCoreBracketNext ==
  \A node:
    IngressDrainStep(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node, IngressDrainStep(node)
         PROVE [Next]_vars
    <2>1. CASE /\ asyncRunnerBudget[node] > 0
                /\ asyncIngressReady[node] # <<>>
                /\ DrainableIngressIndices(node) # {}
      <3>1. DrainFairIngressSelected(node)
        BY <1>1, <2>1 DEF IngressDrainStep
      <3> QED BY <3>1,
                   DrainFairIngressSelectedRefinesCoreBracketNext
    <2>2. CASE ~(/\ asyncRunnerBudget[node] > 0
                  /\ asyncIngressReady[node] # <<>>
                  /\ DrainableIngressIndices(node) # {})
      <3>1. UNCHANGED vars
        BY <1>1, <2>2 DEF IngressDrainStep
      <3> QED BY <3>1, CoreStutterRefinesBracketNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM SerializedRuntimeStepRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      (SerializedRuntimeStep(node)
        \/ SerializedRuntimePrecedesServeIngressStep(node))
        => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 (SerializedRuntimeStep(node)
                   \/ SerializedRuntimePrecedesServeIngressStep(node))
                   => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds,
                  SerializedRuntimeStep(node)
                    \/ SerializedRuntimePrecedesServeIngressStep(node)
           PROVE [Next]_vars
      <3>1. CASE SerializedRuntimeStep(node)
        <4>1. RuntimeStep(node)
          BY <3>1 DEF SerializedRuntimeStep
        <4> QED BY <1>1, <2>1, <4>1,
                     RuntimeStepRefinesCoreBracketNext
      <3>2. CASE SerializedRuntimePrecedesServeIngressStep(node)
        <4>1. RuntimeStep(node)
          BY <3>2 DEF SerializedRuntimePrecedesServeIngressStep
        <4> QED BY <1>1, <2>1, <4>1,
                     RuntimeStepRefinesCoreBracketNext
      <3> QED BY <2>1, <3>1, <3>2
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM SerializedLocalPredecessorRefinesCoreBracketNext ==
  \A node:
    SerializedLocalPrecedesServeIngressStep(node) => [Next]_vars
BY CoreStutterRefinesBracketNext, Isa
   DEF SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead

THEOREM ReplayRunNodeContinuationRefinesCoreBracketNext ==
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
          BY <3>1
             DEF AsyncCandidateProducerContinuationExactLocalReplayStep
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>2. CASE
                AsyncCandidateProducerContinuationReplayTargetOnlyTurn(
                  node)
        <4>1. UNCHANGED vars
          BY <3>2
             DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>3. CASE
                AsyncCandidateProducerContinuationExactRuntimeReplayStep(
                  node)
        <4>1. RuntimeStep(node)
          BY <3>3, Isa
             DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
                 RuntimeStep
        <4> QED BY <1>1, <2>1, <4>1,
             RuntimeStepRefinesCoreBracketNext
      <3> QED BY <2>1, <3>1, <3>2, <3>3
           DEF ReplayRunNodeCandidateProducerContinuation
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RunNodeWorkRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      RunNodeWork(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 RunNodeWork(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds, RunNodeWork(node)
           PROVE [Next]_vars
      <3>0. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
        BY <3>0, CoreStutterRefinesBracketNext, Isa
           DEF ResolveRunNodeCandidateProducerContinuation, vars
      <3>0p. CASE
               ReplayRunNodeCandidateProducerContinuation(node)
        BY <1>1, <2>1, <3>0p,
           ReplayRunNodeContinuationRefinesCoreBracketNext
      <3>1. CASE LocalAdmissionStep(node)
        BY <3>1, LocalAdmissionStepRefinesCoreBracketNext
      <3>2. CASE IngressDrainStep(node)
        BY <3>2, IngressDrainStepRefinesCoreBracketNext
      <3>3. CASE SerializedRuntimeStep(node)
                    \/ SerializedRuntimePrecedesServeIngressStep(node)
        BY <1>1, <2>1, <3>3,
           SerializedRuntimeStepRefinesCoreBracketNext
      <3>4. CASE AsyncServeIngressTargetOnlyTurn(node)
        <4>1. UNCHANGED vars
          BY <3>4 DEF AsyncServeIngressTargetOnlyTurn
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>5. CASE SerializedLocalPrecedesServeIngressStep(node)
        BY <3>5, SerializedLocalPredecessorRefinesCoreBracketNext
      <3> QED BY <2>1, <3>0, <3>0p, <3>1, <3>2, <3>3, <3>4,
                   <3>5
           DEF RunNodeWork
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RunNodeRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      RunNode(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 RunNode(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds, RunNode(node)
           PROVE [Next]_vars
      <3>1. RunNodeWork(node) BY <2>1 DEF RunNode
      <3> QED BY <1>1, <2>1, <3>1,
                   RunNodeWorkRefinesCoreBracketNext
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RunHistoricalRecoveryNodeRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node \in ValidatorIds:
      RunHistoricalRecoveryNode(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node \in ValidatorIds:
                 RunHistoricalRecoveryNode(node) => [Next]_vars
    <2>1. ASSUME NEW node \in ValidatorIds,
                  RunHistoricalRecoveryNode(node)
           PROVE [Next]_vars
      <3>1. RunNodeWork(node)
        BY <2>1 DEF RunHistoricalRecoveryNode
      <3> QED BY <1>1, <2>1, <3>1,
                   RunNodeWorkRefinesCoreBracketNext
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RunHistoricalServerRefinesCoreBracketNext ==
  \A node:
    RunHistoricalServer(node) => [Next]_vars
PROOF
  <1>1. ASSUME NEW node, RunHistoricalServer(node)
         PROVE [Next]_vars
    <2>1. UNCHANGED vars
      BY <1>1, Isa
         DEF RunHistoricalServer, DrainHistoricalIngressSelected,
             HistoricalIdleStep
    <2> QED BY <2>1, CoreStutterRefinesBracketNext
  <1> QED BY <1>1

THEOREM RunNodeAnyRefinesCoreBracketNext ==
  TypeInvariant =>
    \A node:
      RunNode(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant
         PROVE \A node: RunNode(node) => [Next]_vars
    <2>1. ASSUME NEW node, RunNode(node)
           PROVE [Next]_vars
      <3>1. node \in ValidatorIds
        BY <1>1, <2>1, Isa
           DEF RunNode, AsyncCurrentResponsiveVoters, TypeInvariant,
               ModelConfiguration, QuorumConfiguration
      <3> QED BY <1>1, <2>1, <3>1,
                   RunNodeRefinesCoreBracketNext
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RunHistoricalRecoveryNodeAnyRefinesCoreBracketNext ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant
  => \A node:
       RunHistoricalRecoveryNode(node) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant, AsyncSchedulerTypeInvariant
         PROVE \A node:
                 RunHistoricalRecoveryNode(node) => [Next]_vars
    <2>1. ASSUME NEW node, RunHistoricalRecoveryNode(node)
           PROVE [Next]_vars
      <3>1. node \in ValidatorIds
        BY <1>1, <2>1, Isa
           DEF RunHistoricalRecoveryNode, HistoricalRecoveryTarget,
               AsyncSchedulerTypeInvariant,
               AsyncHistoricalRecoveryTypeInvariant,
               TypeInvariant, ModelConfiguration,
               QuorumConfiguration
      <3> QED BY <1>1, <2>1, <3>1,
                   RunHistoricalRecoveryNodeRefinesCoreBracketNext
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM PreGstResponsiveRestartRefinesCoreBracketNext ==
  PreGstResponsiveRestart => [Next]_vars
PROOF
  <1>1. ASSUME PreGstResponsiveRestart
         PROVE [Next]_vars
    <2>1. /\ asyncRecoveryNode \in ValidatorIds
           /\ Restart(asyncRecoveryNode)
      BY <1>1 DEF PreGstResponsiveRestart
    <2>2. Next BY <2>1, RestartRefinesCoreNext
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM PreGstResponsiveReplayRefinesCoreBracketNext ==
  ModelConfiguration =>
    (PreGstResponsiveReplay => [Next]_vars)
PROOF
  <1>1. ASSUME ModelConfiguration, PreGstResponsiveReplay
         PROVE [Next]_vars
    <2>1. CASE Len(RestartSignatureReplay(asyncRecoveryNode)) > 0
      <3>1. RecoveryCoreReplay(
               asyncRecoveryNode,
               Head(RestartSignatureReplay(asyncRecoveryNode)))
        BY <1>1, <2>1 DEF PreGstResponsiveReplay
      <3> QED BY <1>1, <3>1,
                   RecoveryCoreReplayRefinesBracketNext
    <2>2. CASE ~(Len(RestartSignatureReplay(asyncRecoveryNode)) > 0)
      <3>1. UNCHANGED vars
        BY <1>1, <2>2 DEF PreGstResponsiveReplay
      <3> QED BY <3>1, CoreStutterRefinesBracketNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DriveResponsiveReplayHeadRefinesCoreBracketNext ==
  ModelConfiguration =>
    (DriveResponsiveReplayHead => [Next]_vars)
PROOF
  <1>1. ASSUME ModelConfiguration, DriveResponsiveReplayHead
         PROVE [Next]_vars
    <2>1. RecoveryCoreReplay(
             asyncRecoveryNode, Head(asyncRecoveryReplayQueue))
      BY <1>1 DEF DriveResponsiveReplayHead
    <2> QED BY <1>1, <2>1,
                 RecoveryCoreReplayRefinesBracketNext
  <1> QED BY <1>1

THEOREM AsyncFairActionsRefineCoreBracketNext ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant
  => \A initialContext \in ContextRecords:
       AsyncFairActionAt(initialContext) => [Next]_vars
PROOF
  <1>1. ASSUME TypeInvariant, AsyncSchedulerTypeInvariant
         PROVE \A initialContext \in ContextRecords:
                 AsyncFairActionAt(initialContext) => [Next]_vars
    <2>1. ASSUME NEW initialContext \in ContextRecords,
                  AsyncFairActionAt(initialContext)
           PROVE [Next]_vars
      <3>1. CASE AsyncSetGST
        <4>1. SetGST BY <3>1 DEF AsyncSetGST
        <4>2. Next BY <4>1, SetGstRefinesCoreNext
        <4> QED BY <4>2
      <3>2. CASE PreGstResponsiveRestart
        BY <3>2, PreGstResponsiveRestartRefinesCoreBracketNext
      <3>3. CASE PreGstResponsiveReplay
        <4>1. ModelConfiguration BY <1>1 DEF TypeInvariant
        <4> QED BY <3>3, <4>1,
                     PreGstResponsiveReplayRefinesCoreBracketNext
      <3>4. CASE ResponsiveReplayRunNode
        <4>1. RunNode(asyncRecoveryNode)
          BY <3>4 DEF ResponsiveReplayRunNode
        <4> QED BY <1>1, <4>1,
                     RunNodeAnyRefinesCoreBracketNext
      <3>5. CASE ResponsiveReplayServiceIoWorker
        <4>1. UNCHANGED vars
          BY <3>5, Isa
             DEF ResponsiveReplayServiceIoWorker,
                 ServiceIoWorker, ServiceIoWorkerWork
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>6. CASE DriveResponsiveReplayHead
        <4>1. ModelConfiguration BY <1>1 DEF TypeInvariant
        <4> QED BY <3>6, <4>1,
                     DriveResponsiveReplayHeadRefinesCoreBracketNext
      <3>7. CASE FinishResponsiveReplay
        <4>1. UNCHANGED vars
          BY <3>7 DEF FinishResponsiveReplay
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>8. CASE AsyncTick
        <4>1. UNCHANGED vars
          BY <3>8 DEF AsyncTick, AsyncNonClockVars
        <4> QED BY <4>1, CoreStutterRefinesBracketNext
      <3>9. CASE \E node \in AsyncVotersAt(initialContext):
                    PostGstRunNode(node)
        <4>1. \A node:
                 PostGstRunNode(node) => [Next]_vars
          <5>1. ASSUME NEW node, PostGstRunNode(node)
                 PROVE [Next]_vars
            <6>1. RunNode(node) BY <5>1 DEF PostGstRunNode
            <6> QED BY <1>1, <6>1,
                         RunNodeAnyRefinesCoreBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>9, <4>1
      <3>10. CASE \E node \in Responsive:
                     PostGstOpenHistoricalRecovery(node)
        <4>1. \A node:
                 PostGstOpenHistoricalRecovery(node) => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstOpenHistoricalRecovery(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1 DEF PostGstOpenHistoricalRecovery,
                           OpenHistoricalRecovery
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>10, <4>1
      <3>11. CASE \E node \in Responsive:
                     PostGstRunHistoricalRecoveryNode(node)
        <4>1. \A node:
                 PostGstRunHistoricalRecoveryNode(node) => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstRunHistoricalRecoveryNode(node)
                 PROVE [Next]_vars
            <6>1. RunHistoricalRecoveryNode(node)
              BY <5>1 DEF PostGstRunHistoricalRecoveryNode
            <6> QED BY <1>1, <6>1,
                         RunHistoricalRecoveryNodeAnyRefinesCoreBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>11, <4>1
      <3>12. CASE \E node \in Responsive:
                     PostGstRunHistoricalServer(node)
        <4>1. \A node:
                 PostGstRunHistoricalServer(node) => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstRunHistoricalServer(node)
                 PROVE [Next]_vars
            <6>1. RunHistoricalServer(node)
              BY <5>1 DEF PostGstRunHistoricalServer
            <6>2. node \in AsyncResponsiveAppliedArchiveServers
              BY <6>1 DEF RunHistoricalServer
            <6> QED BY <6>1, <6>2,
                         RunHistoricalServerRefinesCoreBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>12, <4>1
      <3>13. CASE \E node \in AsyncVotersAt(initialContext):
                     PostGstCommitCertificateDiscovery(node)
        <4>1. \A node:
                 PostGstCommitCertificateDiscovery(node) => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstCommitCertificateDiscovery(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1 DEF PostGstCommitCertificateDiscovery,
                           DirectCommitCertificateDiscoveryStep,
                           CommitCertificateDiscoveryStepWork
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>13, <4>1
      <3>14. CASE \E node \in Responsive:
                     PostGstHistoricalCommitCertificateDiscovery(node)
        <4>1. \A node:
                 PostGstHistoricalCommitCertificateDiscovery(node)
                   => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstHistoricalCommitCertificateDiscovery(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1
                 DEF PostGstHistoricalCommitCertificateDiscovery,
                     DirectHistoricalCommitCertificateDiscoveryStep,
                     CommitCertificateDiscoveryStepWork
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>14, <4>1
      <3>15. CASE \E node \in Responsive:
                     PostGstServiceIoWorker(node)
        <4>1. \A node:
                 PostGstServiceIoWorker(node) => [Next]_vars
          <5>1. ASSUME NEW node, PostGstServiceIoWorker(node)
                 PROVE [Next]_vars
            <6>1. ServiceIoWorker(node)
              BY <5>1 DEF PostGstServiceIoWorker
            <6>2. node \in AsyncArchiveIoServiceNodes
              BY <6>1 DEF ServiceIoWorker
            <6>3. UNCHANGED vars
              BY <5>1, Isa DEF PostGstServiceIoWorker,
                                  ServiceIoWorker, ServiceIoWorkerWork
            <6> QED BY <6>2, <6>3,
                         CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>15, <4>1
      <3>16. CASE \E node \in Responsive:
                     PostGstServiceHistoricalRecoveryIoWorker(node)
        <4>1. \A node:
                 PostGstServiceHistoricalRecoveryIoWorker(node)
                   => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstServiceHistoricalRecoveryIoWorker(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1, Isa
                 DEF PostGstServiceHistoricalRecoveryIoWorker,
                     ServiceHistoricalRecoveryIoWorker,
                     ServiceIoWorkerWork
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>16, <4>1
      <3>17. CASE \/ (\E node \in AsyncVotersAt(initialContext):
                         PostGstResolveLocalCandidateProducerContinuation(node))
                    \/ (\E node \in AsyncVotersAt(initialContext):
                         PostGstServiceConditionalTransportProducerContinuation(
                           node))
                    \/ (\E node \in AsyncVotersAt(initialContext):
                         PostGstServiceVolatileBodyProducerContinuation(node))
                    \/ (\E slot \in AsyncLeaderWireLifecycleSlotSet:
                         PostGstRetireLeaderWireLifecycleSlot(slot))
        <4>1. \A node:
                 PostGstResolveLocalCandidateProducerContinuation(node)
                   => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstResolveLocalCandidateProducerContinuation(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1
                 DEF PostGstResolveLocalCandidateProducerContinuation,
                     ResolveLocalCandidateProducerContinuation,
                     ResolveCandidateProducerContinuation
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4>2. \A node:
                 PostGstServiceConditionalTransportProducerContinuation(node)
                   => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstServiceConditionalTransportProducerContinuation(
                        node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1
                 DEF PostGstServiceConditionalTransportProducerContinuation,
                     ServiceConditionalTransportProducerContinuation,
                     ResolveCandidateProducerContinuation
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4>3. \A node:
                 PostGstServiceVolatileBodyProducerContinuation(node)
                   => [Next]_vars
          <5>1. ASSUME NEW node,
                      PostGstServiceVolatileBodyProducerContinuation(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1
                 DEF PostGstServiceVolatileBodyProducerContinuation,
                     ServiceVolatileBodyProducerContinuation,
                     ResolveCandidateProducerContinuation
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4>4. \A slot:
                 PostGstRetireLeaderWireLifecycleSlot(slot)
                   => [Next]_vars
          <5>1. ASSUME NEW slot,
                      PostGstRetireLeaderWireLifecycleSlot(slot)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1
                 DEF PostGstRetireLeaderWireLifecycleSlot,
                     RetireLeaderWireLifecycleSlot
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>17, <4>1, <4>2, <4>3, <4>4
      <3>18. CASE \E recipient \in Responsive,
                         source \in AsyncIngressSources:
                     PostGstAdmitHiddenPacket(recipient, source)
        <4>1. \A recipient \in Responsive,
                     source \in AsyncIngressSources:
                 PostGstAdmitHiddenPacket(recipient, source)
                   => [Next]_vars
          <5>1. ASSUME NEW recipient \in Responsive,
                      NEW source \in AsyncIngressSources,
                      PostGstAdmitHiddenPacket(recipient, source)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1, Isa
                 DEF PostGstAdmitHiddenPacket, AdmitIngressPacket,
                     AdmitHiddenPacket, CoalesceHiddenPacket
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>18, <4>1
      <3>19. CASE \E recipient \in ValidatorIds,
                         source \in AsyncIngressSources:
                     PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source)
        <4>1. \A recipient, source:
                 PostGstAdmitHistoricalRecoveryPacket(recipient, source)
                   => [Next]_vars
          <5>1. ASSUME NEW recipient, NEW source,
                      PostGstAdmitHistoricalRecoveryPacket(
                        recipient, source)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1, Isa
                 DEF PostGstAdmitHistoricalRecoveryPacket,
                     AdmitIngressPacket, AdmitHiddenPacket,
                     CoalesceHiddenPacket
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>19, <4>1
      <3>20. CASE \E node \in Responsive:
                     AsyncActivateServiceNode(node)
        <4>1. \A node:
                 AsyncActivateServiceNode(node) => [Next]_vars
          <5>1. ASSUME NEW node, AsyncActivateServiceNode(node)
                 PROVE [Next]_vars
            <6>1. UNCHANGED vars
              BY <5>1, Isa
                 DEF AsyncActivateServiceNode,
                     AsyncServiceActivationFrameVars
            <6> QED BY <6>1, CoreStutterRefinesBracketNext
          <5> QED BY <5>1
        <4> QED BY <3>20, <4>1
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                   <3>7, <3>8, <3>9, <3>10, <3>11, <3>12, <3>13,
                   <3>14, <3>15, <3>16, <3>17, <3>18, <3>19,
                   <3>20
           DEF AsyncFairActionAt
    <2> QED BY <2>1
  <1> QED BY <1>1

AsyncOuterStep ==
  \/ AsyncNonCrashStep
  \/ (\E node \in ValidatorIds: AsyncActivateServiceNode(node))
  \/ (\E node \in ValidatorIds: PreGstCrash(node))
  \/ (\E node \in ValidatorIds: PreGstResponsiveCrash(node))
  \/ PreGstResponsiveRestart
  \/ PreGstResponsiveReplay

AsyncOuterFrameSatisfied ==
  /\ AsyncOuterStep
  /\ UNCHANGED <<height, context>>

THEOREM RunnerCategorySuppliesOuterFrame ==
  /\ AsyncRunnerStep
  /\ AsyncNonCrashOuterFrame
  => AsyncOuterFrameSatisfied
BY Isa
   DEF AsyncOuterFrameSatisfied, AsyncOuterStep,
       AsyncNonCrashStep, AsyncNonCrashOuterFrame,
       AsyncCoreOuterFrame

THEOREM NonRunnerCategorySuppliesOuterFrame ==
  /\ AsyncNonRunnerStep
  /\ AsyncNonRunnerOuterFrame
  => AsyncOuterFrameSatisfied
BY Isa
   DEF AsyncOuterFrameSatisfied, AsyncOuterStep,
       AsyncNonCrashStep, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame

THEOREM RecoveryCategorySuppliesOuterFrame ==
  /\ (DriveResponsiveReplayHead \/ FinishResponsiveReplay)
  /\ AsyncRecoveryOuterFrame
  => AsyncOuterFrameSatisfied
BY Isa
   DEF AsyncOuterFrameSatisfied, AsyncOuterStep,
       AsyncNonCrashStep, AsyncRecoveryOuterFrame,
       AsyncCoreOuterFrame

THEOREM AsyncFairActionsSupplyOuterFrame ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant
  => \A initialContext \in ContextRecords:
       AsyncFairActionAt(initialContext) => AsyncOuterFrameSatisfied
PROOF
  <1>1. ASSUME TypeInvariant, AsyncSchedulerTypeInvariant
         PROVE \A initialContext \in ContextRecords:
                 AsyncFairActionAt(initialContext)
                   => AsyncOuterFrameSatisfied
    <2>1. ASSUME NEW initialContext \in ContextRecords,
                  AsyncFairActionAt(initialContext)
           PROVE AsyncOuterFrameSatisfied
      <3>1. CASE AsyncSetGST
        <4>1. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>1, Isa
             DEF AsyncNonRunnerStep, AsyncSetGST,
                 AsyncNonRunnerOuterFrame
        <4> QED BY <4>1, NonRunnerCategorySuppliesOuterFrame
      <3>2. CASE PreGstResponsiveRestart
        <4>1. AsyncCoreOuterFrame
          BY <3>2 DEF PreGstResponsiveRestart
        <4> QED BY <3>2, <4>1
                     DEF AsyncOuterFrameSatisfied, AsyncOuterStep,
                         AsyncCoreOuterFrame
      <3>3. CASE PreGstResponsiveReplay
        <4>1. AsyncCoreOuterFrame
          BY <3>3 DEF PreGstResponsiveReplay
        <4> QED BY <3>3, <4>1
                     DEF AsyncOuterFrameSatisfied, AsyncOuterStep,
                         AsyncCoreOuterFrame
      <3>4. CASE ResponsiveReplayRunNode
        <4>1. /\ AsyncRunnerStep
               /\ AsyncNonCrashOuterFrame
          BY <3>4, Isa
             DEF ResponsiveReplayRunNode, AsyncRunnerStep, RunNode
        <4> QED BY <4>1, RunnerCategorySuppliesOuterFrame
      <3>5. CASE ResponsiveReplayServiceIoWorker
        <4>1. /\ ServiceIoWorker(asyncRecoveryNode)
               /\ AsyncNonRunnerOuterFrame
          BY <3>5 DEF ResponsiveReplayServiceIoWorker
        <4>2. AsyncNonRunnerStep
          BY <4>1
             DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame,
                 ServiceIoWorker
        <4>3. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <4>1, <4>2
        <4> QED BY <4>3, NonRunnerCategorySuppliesOuterFrame
      <3>6. CASE DriveResponsiveReplayHead
        <4>1. AsyncRecoveryOuterFrame
          BY <3>6 DEF DriveResponsiveReplayHead
        <4> QED BY <3>6, <4>1,
                     RecoveryCategorySuppliesOuterFrame
      <3>7. CASE FinishResponsiveReplay
        <4>1. AsyncRecoveryOuterFrame
          BY <3>7 DEF FinishResponsiveReplay
        <4> QED BY <3>7, <4>1,
                     RecoveryCategorySuppliesOuterFrame
      <3>8. CASE AsyncTick
        <4>1. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>8, Isa
             DEF AsyncNonRunnerStep, AsyncTick,
                 AsyncNonRunnerOuterFrame
        <4> QED BY <4>1, NonRunnerCategorySuppliesOuterFrame
      <3>9. CASE \E node \in AsyncVotersAt(initialContext):
                    PostGstRunNode(node)
        <4>1. \A node:
                 PostGstRunNode(node)
                   => /\ AsyncRunnerStep
                      /\ AsyncNonCrashOuterFrame
          <5>1. ASSUME NEW node, PostGstRunNode(node)
                 PROVE /\ AsyncRunnerStep
                        /\ AsyncNonCrashOuterFrame
            BY <5>1, Isa
               DEF PostGstRunNode, AsyncRunnerStep, RunNode
          <5> QED BY <5>1
        <4>2. /\ AsyncRunnerStep
               /\ AsyncNonCrashOuterFrame
          BY <3>9, <4>1
        <4> QED BY <4>2, RunnerCategorySuppliesOuterFrame
      <3>10. CASE \E node \in Responsive:
                     PostGstOpenHistoricalRecovery(node)
        <4>1. \A node \in Responsive:
                 PostGstOpenHistoricalRecovery(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node \in Responsive,
                      PostGstOpenHistoricalRecovery(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. node \in ValidatorIds
              BY <1>1, <5>1, Isa
                 DEF TypeInvariant, ModelConfiguration,
                     QuorumConfiguration
            <6>2. /\ OpenHistoricalRecovery(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1 DEF PostGstOpenHistoricalRecovery
            <6>3. AsyncNonRunnerStep
              BY <6>1, <6>2
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>2, <6>3
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>10, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>11. CASE \E node \in Responsive:
                     PostGstRunHistoricalRecoveryNode(node)
        <4>1. \A node:
                 PostGstRunHistoricalRecoveryNode(node)
                   => /\ AsyncRunnerStep
                      /\ AsyncNonCrashOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstRunHistoricalRecoveryNode(node)
                 PROVE /\ AsyncRunnerStep
                        /\ AsyncNonCrashOuterFrame
            BY <5>1, Isa
               DEF PostGstRunHistoricalRecoveryNode,
                   AsyncRunnerStep, RunHistoricalRecoveryNode,
                   HistoricalRecoveryTarget
          <5> QED BY <5>1
        <4>2. /\ AsyncRunnerStep
               /\ AsyncNonCrashOuterFrame
          BY <3>11, <4>1
        <4> QED BY <4>2, RunnerCategorySuppliesOuterFrame
      <3>12. CASE \E node \in Responsive:
                     PostGstRunHistoricalServer(node)
        <4>1. \A node:
                 PostGstRunHistoricalServer(node)
                   => /\ AsyncRunnerStep
                      /\ AsyncNonCrashOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstRunHistoricalServer(node)
                 PROVE /\ AsyncRunnerStep
                        /\ AsyncNonCrashOuterFrame
            <6>1. /\ RunHistoricalServer(node)
                   /\ AsyncNonCrashOuterFrame
              BY <5>1 DEF PostGstRunHistoricalServer
            <6>2. node \in AsyncResponsiveAppliedArchiveServers
              BY <6>1 DEF RunHistoricalServer
            <6>3. AsyncRunnerStep
              BY <6>1, <6>2 DEF AsyncRunnerStep
            <6> QED BY <6>1, <6>3
          <5> QED BY <5>1
        <4>2. /\ AsyncRunnerStep
               /\ AsyncNonCrashOuterFrame
          BY <3>12, <4>1
        <4> QED BY <4>2, RunnerCategorySuppliesOuterFrame
      <3>13. CASE \E node \in AsyncVotersAt(initialContext):
                     PostGstCommitCertificateDiscovery(node)
        <4>1. \A node:
                 PostGstCommitCertificateDiscovery(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstCommitCertificateDiscovery(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ DirectCommitCertificateDiscoveryStep(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1 DEF PostGstCommitCertificateDiscovery
            <6>2. node \in AsyncCurrentResponsiveVoters
              BY <6>1
                 DEF DirectCommitCertificateDiscoveryStep,
                     CommitCertificateDiscoveryDue
            <6>3. AsyncNonRunnerStep
              BY <6>1, <6>2
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>3
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>13, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>14. CASE \E node \in Responsive:
                     PostGstHistoricalCommitCertificateDiscovery(node)
        <4>1. \A node:
                 PostGstHistoricalCommitCertificateDiscovery(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstHistoricalCommitCertificateDiscovery(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ DirectHistoricalCommitCertificateDiscoveryStep(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1
                 DEF PostGstHistoricalCommitCertificateDiscovery
            <6>2. node \in asyncHistoricalRecoveryTargets
              BY <6>1
                 DEF DirectHistoricalCommitCertificateDiscoveryStep,
                     HistoricalCommitCertificateDiscoveryDue,
                     HistoricalRecoveryTarget
            <6>3. AsyncNonRunnerStep
              BY <6>1, <6>2
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>3
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>14, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>15. CASE \E node \in Responsive:
                     PostGstServiceIoWorker(node)
        <4>1. \A node:
                 PostGstServiceIoWorker(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node, PostGstServiceIoWorker(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ ServiceIoWorker(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1 DEF PostGstServiceIoWorker
            <6>2. node \in AsyncArchiveIoServiceNodes
              BY <6>1 DEF ServiceIoWorker
            <6>3. AsyncNonRunnerStep
              BY <6>1, <6>2
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>3
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>15, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>16. CASE \E node \in Responsive:
                     PostGstServiceHistoricalRecoveryIoWorker(node)
        <4>1. \A node:
                 PostGstServiceHistoricalRecoveryIoWorker(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstServiceHistoricalRecoveryIoWorker(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ ServiceHistoricalRecoveryIoWorker(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1
                 DEF PostGstServiceHistoricalRecoveryIoWorker
            <6>2. node \in asyncHistoricalRecoveryTargets
              BY <6>1 DEF ServiceHistoricalRecoveryIoWorker,
                            HistoricalRecoveryTarget
            <6>3. AsyncNonRunnerStep
              BY <6>1, <6>2
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>3
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>16, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>17. CASE \/ (\E node \in AsyncVotersAt(initialContext):
                         PostGstResolveLocalCandidateProducerContinuation(node))
                    \/ (\E node \in AsyncVotersAt(initialContext):
                         PostGstServiceConditionalTransportProducerContinuation(
                           node))
                    \/ (\E node \in AsyncVotersAt(initialContext):
                         PostGstServiceVolatileBodyProducerContinuation(node))
                    \/ (\E slot \in AsyncLeaderWireLifecycleSlotSet:
                         PostGstRetireLeaderWireLifecycleSlot(slot))
        <4>1. \A node:
                 PostGstResolveLocalCandidateProducerContinuation(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstResolveLocalCandidateProducerContinuation(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ node \in AsyncCurrentResponsiveVoters
                   /\ ResolveCandidateProducerContinuation(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1
                 DEF PostGstResolveLocalCandidateProducerContinuation,
                     ResolveLocalCandidateProducerContinuation,
                     ResolveCandidateProducerContinuation
            <6>2. AsyncNonRunnerStep
              BY <6>1
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4>2. \A node:
                 PostGstServiceConditionalTransportProducerContinuation(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstServiceConditionalTransportProducerContinuation(
                        node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ node \in AsyncCurrentResponsiveVoters
                   /\ ResolveCandidateProducerContinuation(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1
                 DEF PostGstServiceConditionalTransportProducerContinuation,
                     ServiceConditionalTransportProducerContinuation,
                     ResolveCandidateProducerContinuation
            <6>2. AsyncNonRunnerStep
              BY <6>1
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4>3. \A node:
                 PostGstServiceVolatileBodyProducerContinuation(node)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW node,
                      PostGstServiceVolatileBodyProducerContinuation(node)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ node \in AsyncCurrentResponsiveVoters
                   /\ ResolveCandidateProducerContinuation(node)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1
                 DEF PostGstServiceVolatileBodyProducerContinuation,
                     ServiceVolatileBodyProducerContinuation,
                     ResolveCandidateProducerContinuation
            <6>2. AsyncNonRunnerStep
              BY <6>1
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4>4. \A slot \in AsyncLeaderWireLifecycleSlotSet:
                 PostGstRetireLeaderWireLifecycleSlot(slot)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW slot \in AsyncLeaderWireLifecycleSlotSet,
                      PostGstRetireLeaderWireLifecycleSlot(slot)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ RetireLeaderWireLifecycleSlot(slot)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1 DEF PostGstRetireLeaderWireLifecycleSlot
            <6>2. AsyncNetworkStep
              BY <5>1, <6>1 DEF AsyncNetworkStep
            <6>3. AsyncNonRunnerStep
              BY <6>1, <6>2
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>1, <6>3
          <5> QED BY <5>1
        <4>5. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>17, <4>1, <4>2, <4>3, <4>4
        <4> QED BY <4>5, NonRunnerCategorySuppliesOuterFrame
      <3>18. CASE \E recipient \in Responsive,
                         source \in AsyncIngressSources:
                     PostGstAdmitHiddenPacket(recipient, source)
        <4>1. \A recipient \in Responsive,
                     source \in AsyncIngressSources:
                 PostGstAdmitHiddenPacket(recipient, source)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW recipient \in Responsive,
                      NEW source \in AsyncIngressSources,
                      PostGstAdmitHiddenPacket(recipient, source)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. /\ recipient \in ValidatorIds
                   /\ source \in AsyncIngressSources
              BY <1>1, <5>1, Isa
                 DEF AsyncIngressSources,
                     TypeInvariant, ModelConfiguration,
                     QuorumConfiguration
            <6>2. /\ AdmitIngressPacket(recipient, source)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1 DEF PostGstAdmitHiddenPacket
            <6>3. AsyncNetworkStep
              BY <6>1, <6>2 DEF AsyncNetworkStep
            <6>4. AsyncNonRunnerStep
              BY <6>2, <6>3
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>2, <6>4
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>18, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>19. CASE \E recipient \in ValidatorIds,
                         source \in AsyncIngressSources:
                     PostGstAdmitHistoricalRecoveryPacket(
                       recipient, source)
        <4>1. \A recipient \in ValidatorIds,
                     source \in AsyncIngressSources:
                 PostGstAdmitHistoricalRecoveryPacket(recipient, source)
                   => /\ AsyncNonRunnerStep
                      /\ AsyncNonRunnerOuterFrame
          <5>1. ASSUME NEW recipient \in ValidatorIds,
                      NEW source \in AsyncIngressSources,
                      PostGstAdmitHistoricalRecoveryPacket(
                        recipient, source)
                 PROVE /\ AsyncNonRunnerStep
                        /\ AsyncNonRunnerOuterFrame
            <6>1. source \in AsyncIngressSources
              BY <5>1
            <6>2. /\ AdmitIngressPacket(recipient, source)
                   /\ AsyncNonRunnerOuterFrame
              BY <5>1
                 DEF PostGstAdmitHistoricalRecoveryPacket
            <6>3. AsyncNetworkStep
              BY <5>1, <6>1, <6>2 DEF AsyncNetworkStep
            <6>4. AsyncNonRunnerStep
              BY <6>2, <6>3
                 DEF AsyncNonRunnerStep, AsyncNonRunnerOuterFrame
            <6> QED BY <6>2, <6>4
          <5> QED BY <5>1
        <4>2. /\ AsyncNonRunnerStep
               /\ AsyncNonRunnerOuterFrame
          BY <3>19, <4>1
        <4> QED BY <4>2, NonRunnerCategorySuppliesOuterFrame
      <3>20. CASE \E node \in Responsive:
                     AsyncActivateServiceNode(node)
        <4>1. \A node \in Responsive:
                 AsyncActivateServiceNode(node)
                   => AsyncOuterFrameSatisfied
          <5>1. ASSUME NEW node \in Responsive,
                      AsyncActivateServiceNode(node)
                 PROVE AsyncOuterFrameSatisfied
            <6>1. node \in ValidatorIds
              BY <1>1, <5>1, Isa
                 DEF TypeInvariant, ModelConfiguration,
                     QuorumConfiguration
            <6>2. AsyncOuterStep
              BY <5>1, <6>1 DEF AsyncOuterStep
            <6>3. UNCHANGED <<height, context>>
              BY <5>1, Isa
                 DEF AsyncActivateServiceNode,
                     AsyncServiceActivationFrameVars, vars
            <6> QED BY <6>2, <6>3 DEF AsyncOuterFrameSatisfied
          <5> QED BY <5>1
        <4> QED BY <3>20, <4>1
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                   <3>7, <3>8, <3>9, <3>10, <3>11, <3>12, <3>13,
                   <3>14, <3>15, <3>16, <3>17, <3>18, <3>19,
                   <3>20
           DEF AsyncFairActionAt
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncFairActionsRefineAsyncNextObligation ==
  /\ TypeInvariant
  /\ AsyncSchedulerTypeInvariant
  => \A initialContext \in ContextRecords:
       AsyncFairActionAt(initialContext) => AsyncNext
PROOF
  <1>1. ASSUME TypeInvariant, AsyncSchedulerTypeInvariant
         PROVE \A initialContext \in ContextRecords:
                 AsyncFairActionAt(initialContext) => AsyncNext
    <2>1. ASSUME NEW initialContext \in ContextRecords,
                  AsyncFairActionAt(initialContext)
           PROVE AsyncNext
      <3>1. [Next]_vars
        BY <1>1, <2>1, AsyncFairActionsRefineCoreBracketNext
      <3>2. AsyncOuterFrameSatisfied
        BY <1>1, <2>1, AsyncFairActionsSupplyOuterFrame
      <3> QED BY <3>1, <3>2
                   DEF AsyncNext, AsyncOuterFrameSatisfied,
                       AsyncOuterStep
    <2> QED BY <2>1
  <1> QED BY <1>1

=============================================================================
