---- MODULE SumeragiV2Core ----
EXTENDS SumeragiV2Availability, Sequences

(***************************************************************************
Production-aligned abstract reducer network for Sumeragi v2.

Each honest validator has its own persisted view, generation, lock, highest
PrepareQC, WAL intents, body state, pending persistence request, and pending
signature.  Network envelopes are addressed per recipient, so view divergence,
loss, duplication, reordering, old-view CommitQCs, and future-view TCs are all
representable.  Byzantine validators may emit arbitrary structurally valid
proposals, votes, and timeout votes under their own identity; honest
signatures are reachable only through the matching persisted intent.

The model abstracts signature verification, hashing, deterministic execution,
and fsync behind the same trusted adapter contracts as production.  The
Persistence actions are successful fsync acknowledgements, not write requests.
***************************************************************************)

CONSTANTS
  MaxHeight,
  MaxView,
  ViewDomain,
  MaxGeneration,
  EpochLength,
  LeaderStarts,
  LaneHashes,
  DaHashes,
  ChainIdValue,
  ProtocolVersionValue,
  ValidSubjects,
  Responsive

Heights == 0..MaxHeight
FiniteViews == 0..MaxView
Views == ViewDomain
Generations == 0..MaxGeneration
Phases == {"Prepare", "Commit"}
NoRank == -1
Ranks == {NoRank} \cup Views

CountRostersOneEpoch == << <<0, 1, 2, 3>> >>
CountRostersTwoEpochs == << <<0, 1, 2, 3>>, <<0, 1, 2, 3>> >>
CountPowersOneEpoch == << <<1, 1, 1, 1>> >>
CountPowersTwoEpochs == << <<1, 1, 1, 1>>, <<1, 1, 1, 1>> >>
StakePowersOneEpoch == << <<4, 3, 2, 1>> >>
StakePowersTwoEpochs == << <<4, 3, 2, 1>>, <<2, 4, 3, 1>> >>
StartsHeightZero == <<0>>
StartsHeightZeroOne == <<0, 1>>
StartsByzantineFirst == <<3>>
StartsByzantineFirstTwo == <<3, 0>>
LaneHashesOneHeight == <<101>>
LaneHashesTwoHeights == <<101, 102>>
DaHashesOneHeight == <<201>>
DaHashesTwoHeights == <<201, 202>>

ExpectedEpoch(blockHeight) == blockHeight \div EpochLength

LineagesAt(blockHeight) == [1..blockHeight -> Subjects]

ContextKey(blockHeight, lineage) ==
  LET contextEpoch == ExpectedEpoch(blockHeight)
  IN [chain |-> ChainIdValue,
      protocol |-> ProtocolVersionValue,
      height |-> blockHeight,
      epoch |-> contextEpoch,
      lineage |-> lineage,
      roster |-> RosterSequence(contextEpoch),
      powers |-> EpochPowers[contextEpoch + 1],
      laneHash |-> LaneHashes[blockHeight + 1],
      daHash |-> DaHashes[blockHeight + 1],
      leaderStart |-> LeaderStarts[blockHeight + 1]]

ParentContextKey(blockHeight, lineage) ==
  IF blockHeight = 0
  THEN [genesis |-> TRUE]
  ELSE ContextKey(blockHeight - 1, SubSeq(lineage, 1, blockHeight - 1))

(***************************************************************************
The production HeightContext identity binds the semantic parent finality
identity, not the incidental view or signer representation of a CommitQC.
The model records that projection explicitly.  The lineage embedded in
ContextKey is the abstract collision-resistant hash chain, so the projected
parent context key changes whenever the parent lineage changes.
***************************************************************************)
ParentFinalityIdentity(blockHeight, lineage) ==
  IF blockHeight = 0
  THEN [genesis |-> TRUE]
  ELSE [contextKey |-> ParentContextKey(blockHeight, lineage),
        height |-> blockHeight - 1,
        phase |-> "Commit",
        subject |-> lineage[blockHeight]]

CarriedParentCommit(parentContextKey, parentHeight, parentSubject,
                    roundView, signers) ==
  [contextKey |-> parentContextKey,
   height |-> parentHeight,
   phase |-> "Commit",
   subject |-> parentSubject,
   view |-> roundView,
   signers |-> signers]

SemanticParentFinality(qc) ==
  [contextKey |-> qc.contextKey,
   height |-> qc.height,
   phase |-> qc.phase,
   subject |-> qc.subject]

ContextRecord(blockHeight, lineage) ==
  LET contextEpoch == ExpectedEpoch(blockHeight)
  IN [chain |-> ChainIdValue,
      protocol |-> ProtocolVersionValue,
      height |-> blockHeight,
      epoch |-> contextEpoch,
      lineage |-> lineage,
      contextKey |-> ContextKey(blockHeight, lineage),
      parentContextKey |-> ParentContextKey(blockHeight, lineage),
      parentFinality |-> ParentFinalityIdentity(blockHeight, lineage),
      parent |-> IF blockHeight = 0
                  THEN NoSubject ELSE lineage[blockHeight],
      roster |-> RosterSequence(contextEpoch),
      powers |-> EpochPowers[contextEpoch + 1],
      laneHash |-> LaneHashes[blockHeight + 1],
      daHash |-> DaHashes[blockHeight + 1],
      leaderStart |-> LeaderStarts[blockHeight + 1]]

ContextRecords ==
  UNION {{ContextRecord(blockHeight, lineage):
            lineage \in LineagesAt(blockHeight)}:
           blockHeight \in Heights}

BodyRecordSet ==
  [node: ValidatorIds, context: ContextRecords, view: Views,
   subject: Subjects]

RetainedLockedBodyRecordSet ==
  [node: ValidatorIds, context: ContextRecords, subject: Subjects]

ValidationRecordSet ==
  [node: ValidatorIds, context: ContextRecords, view: Views,
   generation: Generations, subject: Subjects]

RetainedLockedBodiesSound(retainedLockedBodies, durableBodies) ==
  \A retained \in retainedLockedBodies:
    \E sourceView \in Views:
      BodyHeldBy(durableBodies, retained.node, retained.context,
                 sourceView, retained.subject)

Leader(context, roundView) ==
  LET roster == context.roster
      offset == (context.leaderStart + roundView) % Len(roster)
  IN roster[offset + 1]

Proposal(context, roundView, subject, proposer, justifyRank,
         justifySubject) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   subject |-> subject, proposer |-> proposer,
   justifyRank |-> justifyRank, justifySubject |-> justifySubject]

Vote(context, roundView, phase, subject, signer) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   phase |-> phase, subject |-> subject, signer |-> signer]

QC(context, roundView, phase, subject, signers) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   phase |-> phase, subject |-> subject, signers |-> signers]

TimeoutVote(context, roundView, signer, highRank, highSubject) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   signer |-> signer, highRank |-> highRank, highSubject |-> highSubject]

TC(context, roundView, votes) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   votes |-> votes]

ProposalRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   subject: Subjects, proposer: ValidatorIds,
   justifyRank: Ranks, justifySubject: SubjectOrNone]
VoteRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   phase: Phases, subject: Subjects, signer: ValidatorIds]
QcRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   phase: Phases, subject: Subjects, signers: SUBSET ValidatorIds]
TimeoutVoteRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   signer: ValidatorIds, highRank: Ranks, highSubject: SubjectOrNone]
TcRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   votes: SUBSET TimeoutVoteRecordSet]

TcWellTyped(tc) ==
  /\ tc \in TcRecordSet
  /\ DOMAIN tc = {"context", "height", "view", "votes"}
  /\ tc.context \in ContextRecords
  /\ tc.height \in Heights
  /\ tc.view \in Views
  /\ tc.votes \subseteq TimeoutVoteRecordSet

ProposalAt(node, proposal) == [node |-> node, proposal |-> proposal]
VoteAt(node, vote) == [node |-> node, vote |-> vote]
QcAt(node, qc) == [node |-> node, qc |-> qc]
TimeoutVoteAt(node, vote) == [node |-> node, vote |-> vote]
TcAt(node, tc) == [node |-> node, tc |-> tc]

ProposalEnvelope(recipient, proposal) ==
  [recipient |-> recipient, proposal |-> proposal]
VoteEnvelope(recipient, vote) == [recipient |-> recipient, vote |-> vote]
QcEnvelope(recipient, qc) == [recipient |-> recipient, qc |-> qc]
TimeoutEnvelope(recipient, vote) == [recipient |-> recipient, vote |-> vote]
TcEnvelope(recipient, tc) == [recipient |-> recipient, tc |-> tc]

ProposalWal(node, proposal) ==
  [node |-> node, kind |-> "Proposal", proposal |-> proposal]
PrepareWal(node, vote) ==
  [node |-> node, kind |-> "Prepare", vote |-> vote]
ObservePrepareWal(node, qc) ==
  [node |-> node, kind |-> "ObservePrepare", qc |-> qc]
LockCommitWal(node, qc, vote) ==
  [node |-> node, kind |-> "LockCommit", qc |-> qc, vote |-> vote]
TimeoutWal(node, vote) ==
  [node |-> node, kind |-> "Timeout", vote |-> vote]
InstallTcWal(node, tc, rebroadcast) ==
  [node |-> node, kind |-> "InstallTC", tc |-> tc,
   rebroadcast |-> rebroadcast]
DecisionWal(node, qc, rebroadcast) ==
  [node |-> node, kind |-> "Decision", qc |-> qc,
   rebroadcast |-> rebroadcast]

ProposalSign(node, proposal) ==
  [node |-> node, kind |-> "Proposal", proposal |-> proposal]
VoteSign(node, vote) == [node |-> node, kind |-> "Vote", vote |-> vote]
TimeoutSign(node, vote) ==
  [node |-> node, kind |-> "Timeout", vote |-> vote]

ProposalWalSet == [node: ValidatorIds, kind: {"Proposal"},
                   proposal: ProposalRecordSet]
PrepareWalSet == [node: ValidatorIds, kind: {"Prepare"}, vote: VoteRecordSet]
ObservePrepareWalSet == [node: ValidatorIds, kind: {"ObservePrepare"},
                         qc: QcRecordSet]
LockCommitWalSet == [node: ValidatorIds, kind: {"LockCommit"},
                     qc: QcRecordSet, vote: VoteRecordSet]
TimeoutWalSet == [node: ValidatorIds, kind: {"Timeout"},
                  vote: TimeoutVoteRecordSet]
InstallTcWalSet == [node: ValidatorIds, kind: {"InstallTC"},
                    tc: TcRecordSet, rebroadcast: BOOLEAN]
DecisionWalSet == [node: ValidatorIds, kind: {"Decision"},
                   qc: QcRecordSet, rebroadcast: BOOLEAN]

ProposalSignSet == [node: ValidatorIds, kind: {"Proposal"},
                    proposal: ProposalRecordSet]
VoteSignSet == [node: ValidatorIds, kind: {"Vote"}, vote: VoteRecordSet]
TimeoutSignSet == [node: ValidatorIds, kind: {"Timeout"},
                   vote: TimeoutVoteRecordSet]

ProposalEnvelopeSet == [recipient: ValidatorIds, proposal: ProposalRecordSet]
VoteEnvelopeSet == [recipient: ValidatorIds, vote: VoteRecordSet]
QcEnvelopeSet == [recipient: ValidatorIds, qc: QcRecordSet]
TimeoutEnvelopeSet == [recipient: ValidatorIds, vote: TimeoutVoteRecordSet]
TcEnvelopeSet == [recipient: ValidatorIds, tc: TcRecordSet]

VARIABLES
  height,
  context,
  contextHistory,
  nodeView,
  generation,
  up,
  gst,
  availableBodies,
  durableBodies,
  retainedLockedBodies,
  validatedBodies,
  invalidBodies,
  seenProposals,
  receivedVotes,
  receivedQCs,
  receivedTimeoutVotes,
  receivedTCs,
  proposalIntents,
  prepareIntents,
  commitIntents,
  timeoutIntents,
  prepareQCs,
  commitQCs,
  formedTCs,
  installedTCs,
  lockRank,
  lockSubject,
  highestRank,
  highestSubject,
  pendingProposal,
  pendingPrepare,
  pendingObservePrepare,
  pendingLockCommit,
  pendingTimeout,
  pendingInstallTC,
  pendingDecision,
  signProposals,
  signVotes,
  signTimeouts,
  proposalNetwork,
  voteNetwork,
  qcNetwork,
  timeoutNetwork,
  tcNetwork,
  decisions,
  applied

vars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    availableBodies, durableBodies, retainedLockedBodies,
    validatedBodies, invalidBodies,
    seenProposals, receivedVotes, receivedQCs, receivedTimeoutVotes,
    receivedTCs, proposalIntents, prepareIntents, commitIntents,
    timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, proposalNetwork, voteNetwork,
    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

CurrentEpoch == context.epoch
CurrentVoters == VotingRoster(CurrentEpoch)

BroadcastProposals(proposal) ==
  {ProposalEnvelope(recipient, proposal): recipient \in CurrentVoters}
BroadcastVotes(vote) ==
  {VoteEnvelope(recipient, vote):
     recipient \in CurrentVoters \ {vote.signer}}
BroadcastQCs(qc) ==
  {QcEnvelope(recipient, qc): recipient \in CurrentVoters}
BroadcastTimeouts(vote) ==
  {TimeoutEnvelope(recipient, vote): recipient \in CurrentVoters}
BroadcastTCs(tc) ==
  {TcEnvelope(recipient, tc): recipient \in CurrentVoters}

PendingNodes ==
  {request.node: request \in pendingProposal}
    \cup {request.node: request \in pendingPrepare}
    \cup {request.node: request \in pendingObservePrepare}
    \cup {request.node: request \in pendingLockCommit}
    \cup {request.node: request \in pendingTimeout}
    \cup {request.node: request \in pendingInstallTC}
    \cup {request.node: request \in pendingDecision}

AllPendingRequests ==
  pendingProposal \cup pendingPrepare \cup pendingObservePrepare
    \cup pendingLockCommit \cup pendingTimeout \cup pendingInstallTC
    \cup pendingDecision

RequestNodeSet(requests) == {request.node: request \in requests}

RequestsUniqueByNode(requests) ==
  \A left, right \in requests:
    left.node = right.node => left = right

SigningNodes ==
  {request.node: request \in signProposals}
    \cup {request.node: request \in signVotes}
    \cup {request.node: request \in signTimeouts}

(***************************************************************************
State-derived action domains.  Quantifying over the values already present in
authenticated ingress or durable state is behaviorally equivalent to ranging
over the full record universe and then testing membership in an action guard.
It also prevents TLC from materializing the powerset in TcRecordSet.
***************************************************************************)
SeenProposalValues == {entry.proposal: entry \in seenProposals}
ReceivedQcValues == {entry.qc: entry \in receivedQCs}
\* A certificate's global authenticity ghost is not local reducer knowledge.
\* The node which forms a PrepareQC installs its own receipt below; every
\* other node must receive the certificate through authenticated ingress.
LockCommitQcValues == ReceivedQcValues
ReceivedTcValues == {entry.tc: entry \in receivedTCs}
DecisionQcValues == {decision.qc: decision \in decisions}

NodeIdle(node) == node \notin PendingNodes \cup SigningNodes

NodeTimedOut(node, roundView) ==
  \E vote \in timeoutIntents:
    /\ vote.signer = node
    /\ vote.context = context
    /\ vote.view = roundView

NodeInstalledTC(node, roundView) ==
  \E entry \in installedTCs:
    /\ entry.node = node
    /\ entry.tc.context = context
    /\ entry.tc.view = roundView

NoDecisionForNode(node) ==
  ~\E decision \in decisions:
    /\ decision.node = node
    /\ decision.qc.context = context

HighRefValid(highRank, highSubject) ==
  \/ /\ highRank = NoRank
     /\ highSubject = NoSubject
  \/ /\ highRank \in Views
     /\ highSubject \in Subjects
     /\ \E qc \in prepareQCs:
          /\ qc.context = context
         /\ qc.view = highRank
         /\ qc.subject = highSubject

\* The production wire carries the complete optional PrepareQC and verifies
\* its signatures against the frozen roster.  The compact model retains only
\* rank/subject and represents successful wire authentication by the
\* equivalent ghost certificate fact.
AuthenticatedHighRef(highRank, highSubject) ==
  HighRefValid(highRank, highSubject)

\* Checks performed from the QC wire object and frozen local height context.
QcWireValid(qc) ==
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.view \in Views
  /\ qc.phase \in Phases
  /\ qc.subject \in Subjects
  /\ DualQuorum(CurrentEpoch, qc.signers)

\* Semantic external validity is derived from honest durable vote provenance;
\* it is deliberately not an ingress or certificate-formation guard.
QcValid(qc) ==
  /\ QcWireValid(qc)
  /\ qc.subject \in ValidSubjects

VoteBacksCertificate(vote, qc, signer) ==
  /\ vote.context = qc.context
  /\ vote.view = qc.view
  /\ vote.phase = qc.phase
  /\ vote.subject = qc.subject
  /\ vote.signer = signer

CertificateHonestIntentBacked(qc, intents) ==
  \A signer \in qc.signers \cap Honest:
    \E vote \in intents: VoteBacksCertificate(vote, qc, signer)

TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote) ==
  /\ timeoutVote.highRank >= commitVote.view
  /\ (timeoutVote.highRank = commitVote.view
        => timeoutVote.highSubject = commitVote.subject)

TimeoutSignerSet(votes) == {vote.signer: vote \in votes}

TimeoutVotesDisjoint(votes) ==
  Cardinality(TimeoutSignerSet(votes)) = Cardinality(votes)

TimeoutHighsConflictFree(votes) ==
  \A left, right \in votes:
    (left.highRank = right.highRank /\ left.highRank # NoRank)
      => left.highSubject = right.highSubject

MaximalTimeoutVotes(votes) ==
  {candidate \in votes:
    \A other \in votes: candidate.highRank >= other.highRank}

EmptyTimeoutHigh ==
  [highRank |-> NoRank, highSubject |-> NoSubject]

HighestTimeoutVote(votes) ==
  LET maxima == MaximalTimeoutVotes(votes)
  IN IF maxima = {}
     THEN EmptyTimeoutHigh
     ELSE CHOOSE candidate \in maxima: TRUE

TCValid(tc) ==
  /\ tc \in TcRecordSet
  /\ DOMAIN tc = {"context", "height", "view", "votes"}
  /\ tc.context = context
  /\ tc.height = height
  /\ tc.view \in Views
  /\ IsFiniteSet(tc.votes)
  /\ tc.votes # {}
  /\ \A vote \in tc.votes:
       /\ vote \in TimeoutVoteRecordSet
       /\ vote.context = context
       /\ vote.height = height
       /\ vote.view = tc.view
       /\ vote.signer \in CurrentVoters
       /\ AuthenticatedHighRef(vote.highRank, vote.highSubject)
       /\ vote.highRank <= tc.view
  /\ TimeoutVotesDisjoint(tc.votes)
  /\ TimeoutHighsConflictFree(tc.votes)
  /\ DualQuorum(CurrentEpoch, TimeoutSignerSet(tc.votes))

TcHighRank(tc) == HighestTimeoutVote(tc.votes).highRank
TcHighSubject(tc) == HighestTimeoutVote(tc.votes).highSubject

InstalledTcAuthorizesCommitVote(commitVote) ==
  \E installed \in installedTCs:
    /\ installed.node = commitVote.signer
    /\ installed.tc.context = commitVote.context
    /\ installed.tc.view >= commitVote.view
    /\ TcHighRank(installed.tc) = commitVote.view
    /\ TcHighSubject(installed.tc) = commitVote.subject

(***************************************************************************
An honest timeout ordinarily fences later Commit creation in that view.  The
one production exception is a Commit for the exact PrepareQC selected by a
durably installed TC: installation first promotes that QC to the node's lock,
and local body validation may then persist the matching historical intent only
when no higher conflicting-subject local Prepare intent or known PrepareQC
exists.  Higher same-subject reproposals are non-conflicting.
The installed-TC record is retained as durable provenance after the lock later
advances, so old timeout/Commit compatibility remains state-checkable.
***************************************************************************)
TimeoutVoteProtectsCommitSet(timeoutVote, commitSet) ==
  \A commitVote \in commitSet:
    (/\ timeoutVote.signer \in Honest
     /\ commitVote.signer = timeoutVote.signer
     /\ commitVote.context = timeoutVote.context
     /\ commitVote.phase = "Commit"
     /\ commitVote.view <= timeoutVote.view)
    => \/ TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)
       \/ InstalledTcAuthorizesCommitVote(commitVote)

ResultingInstallLockRank(node, tc) ==
  IF TcHighRank(tc) > lockRank[node]
  THEN TcHighRank(tc)
  ELSE lockRank[node]

ResultingInstallLockSubject(node, tc) ==
  IF TcHighRank(tc) > lockRank[node]
  THEN TcHighSubject(tc)
  ELSE lockSubject[node]

ExactLockedCommitIntents(node, roundView, subject) ==
  {vote \in commitIntents:
    /\ vote.signer = node
    /\ vote.context = context
    /\ vote.phase = "Commit"
    /\ vote.view = roundView
    /\ vote.subject = subject}

InstalledTcSelectsPrepareFor(node, qc) ==
  \E installed \in installedTCs:
    /\ installed.node = node
    /\ installed.tc.context = qc.context
    /\ installed.tc.view >= qc.view
    /\ TcHighRank(installed.tc) = qc.view
    /\ TcHighSubject(installed.tc) = qc.subject

CurrentOpenPrepareForCommit(node, qc) ==
  /\ QcAt(node, qc) \in receivedQCs
  /\ qc.view = nodeView[node]
  /\ ~NodeTimedOut(node, qc.view)

NoHigherConflictingPrepareKnown(node, qc) ==
  /\ ~\E vote \in prepareIntents:
       /\ vote.signer = node
       /\ vote.context = qc.context
       /\ vote.phase = "Prepare"
       /\ vote.view > qc.view
       /\ vote.subject # qc.subject
  /\ ~(highestRank[node] > qc.view
        /\ highestSubject[node] # qc.subject)

HistoricalTcLockedPrepareForCommit(node, qc) ==
  /\ qc \in prepareQCs
  /\ qc.view < nodeView[node]
  /\ qc.view = lockRank[node]
  /\ qc.subject = lockSubject[node]
  /\ InstalledTcSelectsPrepareFor(node, qc)
  /\ NoHigherConflictingPrepareKnown(node, qc)

(***************************************************************************
TC acknowledgement clears the installing node's volatile vote pool.  If the
resulting lock still has the node's exact durable Commit intent, production
queues that intent for re-signing.  A newly promoted lock without such an
intent does not sign immediately: its exact body must first become durable and
validate, after which HistoricalTcLockedPrepareForCommit authorizes the normal
persistence-before-sign pipeline for that one historical round and subject.
***************************************************************************)
ActiveLockedCommitSignRequestsAfterInstall(node, tc) ==
  {VoteSign(node, vote):
    vote \in ExactLockedCommitIntents(
      node, ResultingInstallLockRank(node, tc),
      ResultingInstallLockSubject(node, tc))}

ProposalJustified(node, proposal) ==
  \/ /\ proposal.view = 0
     /\ proposal.justifyRank = NoRank
     /\ proposal.justifySubject = context.parent
  \/ /\ proposal.view > 0
     /\ \E installed \in installedTCs:
          /\ installed.node = node
          /\ installed.tc.context = context
          /\ installed.tc.view + 1 = proposal.view
          /\ proposal.justifyRank = TcHighRank(installed.tc)
          /\ proposal.justifySubject = TcHighSubject(installed.tc)
          /\ AuthenticatedHighRef(proposal.justifyRank,
                                  proposal.justifySubject)
          /\ proposal.justifyRank < proposal.view

SafeToPrepare(node, proposal) ==
  \/ lockRank[node] = NoRank
  \/ lockSubject[node] = proposal.subject
  \/ /\ proposal.justifyRank > lockRank[node]
     /\ proposal.justifySubject = proposal.subject

\* Wire/local-state checks do not decide external validity from a subject hash.
ProposalWireValidFor(node, proposal) ==
  /\ proposal.context = context
  /\ proposal.height = height
  /\ proposal.view = nodeView[node]
  /\ proposal.proposer = Leader(context, proposal.view)
  /\ proposal.subject \in Subjects
  /\ ProposalJustified(node, proposal)
  /\ SafeToPrepare(node, proposal)

ProposalValidFor(node, proposal) ==
  /\ ProposalWireValidFor(node, proposal)
  /\ proposal.subject \in ValidSubjects

VoteSignersAt(node, roundView, phase, subject) ==
  {received.vote.signer:
    received \in {entry \in receivedVotes:
      /\ entry.node = node
      /\ entry.vote.context = context
      /\ entry.vote.view = roundView
      /\ entry.vote.phase = phase
      /\ entry.vote.subject = subject}}

(***************************************************************************
Vote admission across a view installation.

The volatile receipt pool is cleared for the node that installs a TC.  Every
Commit vote, including one from the current view, is admitted only for the
exact durable lock backed by the authenticated PrepareQC ghost fact that
established it.  Prepare votes remain current-view only.  This is the formal
counterpart of retaining signed CommitVote control while rebuilding volatile
pools without allowing an unlocked current-view Commit to create a third pool.
***************************************************************************)

LockedPrepareRound(node, roundView, subject) ==
  /\ lockRank[node] = roundView
  /\ lockSubject[node] = subject
  /\ \E qc \in prepareQCs:
       /\ qc.context = context
       /\ qc.view = roundView
       /\ qc.phase = "Prepare"
       /\ qc.subject = subject

VoteRoundAdmissible(node, vote) ==
  \/ /\ vote.phase = "Prepare"
     /\ vote.view = nodeView[node]
  \/ /\ vote.phase = "Commit"
     /\ LockedPrepareRound(node, vote.view, vote.subject)

CommitRoundAdmissible(node, roundView, subject) ==
  LockedPrepareRound(node, roundView, subject)

(***************************************************************************
After a durable LockCommit acknowledgement, production retires every
superseded historical vote pool for that node.  It retains the current-view
pool(s), which can still contribute to current progress, plus the one exact
historical Commit pool selected by the newly durable lock.  Other validators'
volatile pools are independent and remain untouched.
***************************************************************************)
VoteReceiptSurvivesLockCommit(received, node, roundView, subject) ==
  \/ received.node # node
  \/ received.vote.view = nodeView[node]
  \/ /\ received.vote.phase = "Commit"
     /\ received.vote.view = roundView
     /\ received.vote.subject = subject

TimeoutVotesAt(node, roundView) ==
  {received.vote:
    received \in {entry \in receivedTimeoutVotes:
      /\ entry.node = node
      /\ entry.vote.context = context
      /\ entry.vote.view = roundView}}

(***************************************************************************
The production reducer's timeout pool is keyed by round and signer.  The
first authenticated vote occupies that slot; a later conflicting vote is
reported as equivocation evidence but cannot replace or join the TC pool.
***************************************************************************)
SameTimeoutVoteSlot(left, right) ==
  /\ left.node = right.node
  /\ left.vote.context = right.vote.context
  /\ left.vote.view = right.vote.view
  /\ left.vote.signer = right.vote.signer

TimeoutVoteSlotOccupied(node, vote) ==
  \E received \in receivedTimeoutVotes:
    SameTimeoutVoteSlot(received, TimeoutVoteAt(node, vote))

ReceivedTimeoutVoteSlotsUnique ==
  \A left, right \in receivedTimeoutVotes:
    SameTimeoutVoteSlot(left, right) => left = right

ReceivedTimeoutVotePoolInvariant ==
  /\ IsFiniteSet(receivedTimeoutVotes)
  /\ ReceivedTimeoutVoteSlotsUnique
  /\ \A received \in receivedTimeoutVotes:
       /\ received.node \in ValidatorIds
       /\ received.vote \in TimeoutVoteRecordSet
       /\ received.vote.context = context
       /\ received.vote.height = height
       /\ received.vote.signer \in CurrentVoters
       /\ AuthenticatedHighRef(received.vote.highRank,
                               received.vote.highSubject)
       /\ received.vote.highRank <= received.vote.view

ModelConfiguration ==
  /\ QuorumConfiguration
  /\ MaxHeight \in Nat
  /\ ViewDomain \subseteq Nat
  /\ 0 \in ViewDomain
  /\ \A roundView \in ViewDomain: 0..roundView \subseteq ViewDomain
  /\ MaxGeneration \in Nat
  /\ EpochLength \in Nat \ {0}
  /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
  /\ Len(LeaderStarts) = MaxHeight + 1
  /\ Len(LaneHashes) = MaxHeight + 1
  /\ Len(DaHashes) = MaxHeight + 1
  /\ \A index \in 1..Len(LeaderStarts):
       LeaderStarts[index] \in 0..(N - 1)
  /\ ProtocolVersionValue = 3
  /\ ValidSubjects \subseteq Subjects
  /\ ValidSubjects # {}
  /\ Responsive \subseteq Honest
  /\ \A epoch \in Epochs:
       DualQuorum(epoch, Responsive \cap VotingRoster(epoch))

BootstrapParentContext(initialContext) ==
  ContextRecord(initialContext.height - 1,
                [index \in 1..(initialContext.height - 1) |->
                   initialContext.lineage[index]])

BootstrapParentSigners(initialContext) ==
  Responsive
    \cap VotingRoster(BootstrapParentContext(initialContext).epoch)

BootstrapParentPrepareQC(initialContext) ==
  QC(BootstrapParentContext(initialContext), 0, "Prepare",
     initialContext.parent, BootstrapParentSigners(initialContext))

BootstrapParentCommitQC(initialContext) ==
  QC(BootstrapParentContext(initialContext), 0, "Commit",
     initialContext.parent, BootstrapParentSigners(initialContext))

BootstrapParentPrepareIntents(initialContext) ==
  {Vote(BootstrapParentContext(initialContext), 0, "Prepare",
        initialContext.parent, signer):
     signer \in BootstrapParentSigners(initialContext)}

BootstrapParentCommitIntents(initialContext) ==
  {Vote(BootstrapParentContext(initialContext), 0, "Commit",
        initialContext.parent, signer):
     signer \in BootstrapParentSigners(initialContext)}

BootstrapParentDecisionNode(initialContext) ==
  CHOOSE node \in
    Responsive \cap BootstrapParentSigners(initialContext): TRUE

BootstrapParentDecision(initialContext) ==
  [node |-> BootstrapParentDecisionNode(initialContext),
   qc |-> BootstrapParentCommitQC(initialContext)]

BootstrapParentBodies(initialContext) ==
  {BodyRecord(signer, BootstrapParentContext(initialContext),
              0, initialContext.parent):
     signer \in BootstrapParentSigners(initialContext) \cap Honest}

FrozenContextAdmissible(initialContext) ==
  /\ initialContext \in ContextRecords
  /\ \A index \in DOMAIN initialContext.lineage:
       initialContext.lineage[index] \in ValidSubjects

(***************************************************************************
Initialize one arbitrary frozen height context.  A non-genesis context carries
the minimal exact durable parent evidence that an `AdvanceContext` successor
retains: dual-quorum Prepare/Commit intents and certificates, durable bodies
for honest signers, and one matching decision/application receipt.  Current-
context reducer state is otherwise fresh.  Genesis remains the concrete TLC
entry point through `Init` below.
***************************************************************************)
InitAt(initialContext) ==
  /\ ModelConfiguration
  /\ FrozenContextAdmissible(initialContext)
  /\ height = initialContext.height
  /\ context = initialContext
  /\ contextHistory = {context}
  /\ nodeView = [node \in ValidatorIds |-> 0]
  /\ generation = [node \in ValidatorIds |-> 0]
  /\ up = ValidatorIds
  /\ gst = FALSE
  /\ availableBodies = {}
  /\ durableBodies =
       IF initialContext.height = 0
       THEN {} ELSE BootstrapParentBodies(initialContext)
  /\ retainedLockedBodies = {}
  /\ validatedBodies = {}
  /\ invalidBodies = {}
  /\ seenProposals = {}
  /\ receivedVotes = {}
  /\ receivedQCs = {}
  /\ receivedTimeoutVotes = {}
  /\ receivedTCs = {}
  /\ proposalIntents = {}
  /\ prepareIntents =
       IF initialContext.height = 0
       THEN {} ELSE BootstrapParentPrepareIntents(initialContext)
  /\ commitIntents =
       IF initialContext.height = 0
       THEN {} ELSE BootstrapParentCommitIntents(initialContext)
  /\ timeoutIntents = {}
  /\ prepareQCs =
       IF initialContext.height = 0
       THEN {} ELSE {BootstrapParentPrepareQC(initialContext)}
  /\ commitQCs =
       IF initialContext.height = 0
       THEN {} ELSE {BootstrapParentCommitQC(initialContext)}
  /\ formedTCs = {}
  /\ installedTCs = {}
  /\ lockRank = [node \in ValidatorIds |-> NoRank]
  /\ lockSubject = [node \in ValidatorIds |-> NoSubject]
  /\ highestRank = [node \in ValidatorIds |-> NoRank]
  /\ highestSubject = [node \in ValidatorIds |-> NoSubject]
  /\ pendingProposal = {}
  /\ pendingPrepare = {}
  /\ pendingObservePrepare = {}
  /\ pendingLockCommit = {}
  /\ pendingTimeout = {}
  /\ pendingInstallTC = {}
  /\ pendingDecision = {}
  /\ signProposals = {}
  /\ signVotes = {}
  /\ signTimeouts = {}
  /\ proposalNetwork = {}
  /\ voteNetwork = {}
  /\ qcNetwork = {}
  /\ timeoutNetwork = {}
  /\ tcNetwork = {}
  /\ decisions =
       IF initialContext.height = 0
       THEN {} ELSE {BootstrapParentDecision(initialContext)}
  /\ applied =
       IF initialContext.height = 0
       THEN {} ELSE {BootstrapParentDecision(initialContext)}

Init == InitAt(ContextRecord(0, <<>>))

SetGST ==
  /\ ~gst
  /\ Responsive \subseteq up
  /\ gst' = TRUE
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation, up,
                 availableBodies, durableBodies, retainedLockedBodies,
                 validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal,
                 pendingPrepare, pendingObservePrepare, pendingLockCommit,
                 pendingTimeout, pendingInstallTC, pendingDecision,
                 signProposals, signVotes, signTimeouts, proposalNetwork,
                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                 decisions, applied>>

(***************************************************************************
`LocalProposalReady` is the trusted completion of the full production body
manifest and execution-commitment checks.  Core abstracts that identity to
context/view/subject; the adapter contract makes both the full manifest and
execution commitment single-valued for that abstract key before this action.
***************************************************************************)
ExactDecidedLocalBody(node, roundView, subject) ==
  \E decision \in decisions:
    /\ decision.node = node
    /\ decision.qc.context = context
    /\ decision.qc.phase = "Commit"
    /\ decision.qc.view = roundView
    /\ decision.qc.subject = subject

LocalBodyNotSupersededByDecision(node, roundView, subject) ==
  \A decision \in decisions:
    (decision.node = node
       /\ decision.qc.context = context
       /\ decision.qc.phase = "Commit")
      => (decision.qc.view = roundView
            /\ decision.qc.subject = subject)

AssembleLocalBody(node, subject) ==
  LET roundView == nodeView[node]
      body == BodyRecord(node, context, roundView, subject)
      validation == ValidationRecord(node, context, roundView,
                                      generation[node], subject)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ node = Leader(context, nodeView[node])
     /\ subject \in ValidSubjects
     /\ LocalBodyNotSupersededByDecision(node, roundView, subject)
     /\ body \in BodyRecordSet
     /\ validation \in ValidationRecordSet
     /\ ~BodyHeldBy(durableBodies, node, context, roundView, subject)
     /\ durableBodies' = durableBodies \cup {body}
     /\ validatedBodies' = validatedBodies \cup {validation}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, retainedLockedBodies,
                    invalidBodies, seenProposals,
                    receivedVotes, receivedQCs, receivedTimeoutVotes,
                    receivedTCs, proposalIntents, prepareIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

LocalProposalJustification(node) ==
  LET roundView == nodeView[node]
  IN IF roundView = 0
     THEN [rank |-> NoRank, subject |-> context.parent]
     ELSE LET tc == CHOOSE installed \in installedTCs:
                      /\ installed.node = node
                      /\ installed.tc.context = context
                      /\ installed.tc.view + 1 = roundView
          IN [rank |-> TcHighRank(tc.tc), subject |-> TcHighSubject(tc.tc)]

LocalProposalFor(node, subject) ==
  LET justification == LocalProposalJustification(node)
  IN Proposal(context, nodeView[node], subject, node,
              justification.rank, justification.subject)

BeginLocalProposal(node, subject) ==
  LET roundView == nodeView[node]
      proposal == LocalProposalFor(node, subject)
      request == ProposalWal(node, proposal)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ node = Leader(context, roundView)
     /\ NodeIdle(node)
     /\ (roundView = 0 \/ NodeInstalledTC(node, roundView - 1))
     /\ BodyHeldBy(durableBodies, node, context, roundView, subject)
     /\ BodyValidatedBy(validatedBodies, node, context, roundView,
                        generation[node], subject)
     /\ ProposalWireValidFor(node, proposal)
     /\ ~\E prior \in proposalIntents:
           /\ prior.proposer = node
           /\ prior.context = context
           /\ prior.view = roundView
     /\ proposal \notin proposalIntents
     /\ request \in ProposalWalSet
     /\ request \notin pendingProposal
     /\ pendingProposal' = pendingProposal \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingPrepare,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

PersistProposal(request) ==
  LET signRequest == ProposalSign(request.node, request.proposal)
  IN /\ request \in pendingProposal
     /\ request.proposal \notin proposalIntents
     /\ proposalIntents' = proposalIntents \cup {request.proposal}
     /\ pendingProposal' = pendingProposal \ {request}
     /\ signProposals' = signProposals \cup {signRequest}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, prepareIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingPrepare,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingInstallTC, pendingDecision, signVotes, signTimeouts,
                    proposalNetwork, voteNetwork, qcNetwork, timeoutNetwork,
                    tcNetwork, decisions, applied>>

CompleteProposalSignature(request) ==
  /\ request \in signProposals
  /\ request.proposal.proposer = request.node
  /\ request.proposal \in proposalIntents
  /\ signProposals' = signProposals \ {request}
  /\ proposalNetwork' =
       proposalNetwork \cup BroadcastProposals(request.proposal)
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal, pendingPrepare,
                 pendingObservePrepare, pendingLockCommit, pendingTimeout,
                 pendingInstallTC, pendingDecision, signVotes, signTimeouts,
                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                 decisions, applied>>

ByzantineBroadcastProposal(signer, roundView, subject,
                           justifyRank, justifySubject) ==
  LET proposal == Proposal(context, roundView, subject, signer,
                           justifyRank, justifySubject)
  IN /\ signer \in Byzantine(CurrentEpoch) \cap up
     /\ signer = Leader(context, roundView)
     /\ roundView \in Views
     /\ subject \in Subjects
     /\ justifyRank \in Ranks
     /\ justifySubject \in SubjectOrNone
     /\ proposalNetwork' = proposalNetwork \cup BroadcastProposals(proposal)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

DeliverProposal(envelope) ==
  LET seen == ProposalAt(envelope.recipient, envelope.proposal)
  IN /\ envelope \in proposalNetwork
     /\ envelope.recipient \in up
     /\ ProposalWireValidFor(envelope.recipient, envelope.proposal)
     /\ proposalNetwork' = proposalNetwork \ {envelope}
     /\ seenProposals' = seenProposals \cup {seen}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

FetchBody(node, proposal) ==
  LET body == BodyRecord(node, context, proposal.view, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     \* `availableBodies` is the current-round adapter staging boundary.
     \* Ordinary fetch stages only the proposal's exact view.  Cross-view
     \* locked-byte reuse is authorized exclusively by RebindRetainedBody.
     /\ body \notin availableBodies
     /\ body \in BodyRecordSet
     /\ availableBodies' = availableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, durableBodies, retainedLockedBodies,
                    validatedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

RebindRetainedBody(node, proposal) ==
  LET body == BodyRecord(node, context, proposal.view, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ lockRank[node] # NoRank
     /\ lockSubject[node] = proposal.subject
     /\ RetainedLockedBodyHeldBy(retainedLockedBodies, node, context,
                                  proposal.subject)
     /\ body \notin availableBodies
     /\ body \in BodyRecordSet
     /\ availableBodies' = availableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, durableBodies, retainedLockedBodies,
                    validatedBodies, invalidBodies, seenProposals,
                    receivedVotes, receivedQCs, receivedTimeoutVotes,
                    receivedTCs, proposalIntents, prepareIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

StoreBody(node, roundView, subject) ==
  LET body == BodyRecord(node, context, roundView, subject)
  IN /\ body \in availableBodies
     /\ availableBodies' = availableBodies \ {body}
     /\ durableBodies' = durableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals,
                    receivedVotes, receivedQCs, receivedTimeoutVotes,
                    receivedTCs, proposalIntents, prepareIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

ValidateBody(node, proposal) ==
  LET validation == ValidationRecord(node, context, proposal.view,
                                      generation[node], proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ BodyHeldBy(durableBodies, node, context, proposal.view,
                    proposal.subject)
     /\ proposal.subject \in ValidSubjects
     /\ validation \notin validatedBodies
     /\ validation \in ValidationRecordSet
     /\ validatedBodies' = validatedBodies \cup {validation}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

(***************************************************************************
Certificate-first recovery deliberately has no leader proposal authority.
The authenticated CommitQC identifies the decided round and subject, while
the certified response supplies bytes from which the production adapter
rederives the canonical manifest before durable storage.  Deterministic body
validation therefore needs the exact durable decision, not a fabricated
`seenProposals` entry.  This action records only local validation evidence;
proposal-gated Prepare voting remains guarded by `BeginPrepare`.
***************************************************************************)
ValidateDecidedBody(node, qc) ==
  LET validation == ValidationRecord(node, context, qc.view,
                                      generation[node], qc.subject)
      decision == [node |-> node, qc |-> qc]
  IN /\ decision \in decisions
     /\ qc.phase = "Commit"
     /\ qc.context = context
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ qc.subject \in ValidSubjects
     /\ validation \notin validatedBodies
     /\ validation \in ValidationRecordSet
     /\ validatedBodies' = validatedBodies \cup {validation}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

RejectBody(node, proposal) ==
  LET body == BodyRecord(node, context, proposal.view, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ BodyHeldBy(durableBodies, node, context, proposal.view,
                    proposal.subject)
     /\ proposal.subject \notin ValidSubjects
     /\ body \in BodyRecordSet
     /\ invalidBodies' = invalidBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, validatedBodies,
                    seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

PrepareVoteFor(node, proposal) ==
  Vote(context, proposal.view, "Prepare", proposal.subject, node)

PrepareRequestFor(node, proposal) ==
  PrepareWal(node, PrepareVoteFor(node, proposal))

BeginPrepare(node, proposal) ==
  LET vote == PrepareVoteFor(node, proposal)
      request == PrepareRequestFor(node, proposal)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ NodeIdle(node)
     /\ ProposalAt(node, proposal) \in seenProposals
     /\ ProposalWireValidFor(node, proposal)
     /\ PrepareSignerAvailability(durableBodies, validatedBodies, context,
                                  proposal.view, generation,
                                  proposal.subject, node)
     /\ lockRank[node] < proposal.view
     /\ ~NodeTimedOut(node, proposal.view)
     /\ ~\E prior \in prepareIntents:
           /\ prior.signer = node
           /\ prior.context = context
           /\ prior.view = proposal.view
     /\ request \in PrepareWalSet
     /\ pendingPrepare' = pendingPrepare \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

PersistPrepare(request) ==
  LET signRequest == VoteSign(request.node, request.vote)
  IN /\ request \in pendingPrepare
     /\ request.vote \notin prepareIntents
     /\ prepareIntents' = prepareIntents \cup {request.vote}
     /\ pendingPrepare' = pendingPrepare \ {request}
     /\ signVotes' = signVotes \cup {signRequest}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingInstallTC, pendingDecision, signProposals,
                    signTimeouts, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

CompleteVoteSignature(request) ==
  /\ request \in signVotes
  /\ request.vote.signer = request.node
  /\ (request.vote \in prepareIntents \/ request.vote \in commitIntents)
  /\ VoteRoundAdmissible(request.node, request.vote)
  /\ signVotes' = signVotes \ {request}
  /\ receivedVotes' =
       receivedVotes \cup {VoteAt(request.node, request.vote)}
  /\ voteNetwork' = voteNetwork \cup BroadcastVotes(request.vote)
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal, pendingPrepare,
                 pendingObservePrepare, pendingLockCommit, pendingTimeout,
                 pendingInstallTC, pendingDecision, signProposals,
                 signTimeouts, proposalNetwork, qcNetwork, timeoutNetwork,
                 tcNetwork, decisions, applied>>

ByzantineBroadcastVote(signer, roundView, phase, subject) ==
  LET vote == Vote(context, roundView, phase, subject, signer)
  IN /\ signer \in Byzantine(CurrentEpoch) \cap up
     /\ roundView \in Views
     /\ phase \in Phases
     /\ subject \in Subjects
     /\ voteNetwork' = voteNetwork \cup BroadcastVotes(vote)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

DeliverVote(envelope) ==
  LET received == VoteAt(envelope.recipient, envelope.vote)
  IN /\ envelope \in voteNetwork
     /\ envelope.recipient \in up
     /\ envelope.vote.context = context
     /\ envelope.vote.signer \in CurrentVoters
     /\ VoteRoundAdmissible(envelope.recipient, envelope.vote)
     \* `voteNetwork` is immutable authenticated delivery history.  The
     \* asynchronous transport owns packet loss and retransmission; consuming
     \* this fact here made a retained CommitVote impossible to redeliver
     \* after PersistInstallTC cleared the volatile receipt pool.
     /\ received \notin receivedVotes
     /\ receivedVotes' = receivedVotes \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

FormPrepareQC(node, roundView, subject) ==
  LET signers == VoteSignersAt(node, roundView, "Prepare", subject)
      qc == QC(context, roundView, "Prepare", subject, signers)
      received == QcAt(node, qc)
  IN /\ node \in up
     /\ roundView = nodeView[node]
     /\ QcWireValid(qc)
     /\ qc \in QcRecordSet
     /\ prepareQCs' = prepareQCs \cup {qc}
     /\ receivedQCs' = receivedQCs \cup {received}
     /\ qcNetwork' = qcNetwork \cup BroadcastQCs(qc)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

(***************************************************************************
The asynchronous adapter may recover an already authenticated durable Commit
certificate and place one addressed copy back on the Core transport.  The
certificate must belong to the exact frozen context and the recipient must be
both responsive and currently up.  Keeping the envelope insertion idempotent
models fair retransmission without minting a new certificate or changing any
reducer-local state.
***************************************************************************)
ImportAuthenticatedCommitCertificate(envelope) ==
  /\ envelope \in QcEnvelopeSet
  /\ envelope.recipient \in Responsive \cap up
  /\ envelope.qc \in commitQCs
  /\ envelope.qc.context = context
  /\ envelope.qc.phase = "Commit"
  /\ QcWireValid(envelope.qc)
  /\ envelope \notin qcNetwork
  /\ qcNetwork' = qcNetwork \cup {envelope}
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies,
                 retainedLockedBodies, validatedBodies, invalidBodies,
                 seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal,
                 pendingPrepare, pendingObservePrepare, pendingLockCommit,
                 pendingTimeout, pendingInstallTC, pendingDecision,
                 signProposals, signVotes, signTimeouts, proposalNetwork,
                 voteNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

DeliverQC(envelope) ==
  LET received == QcAt(envelope.recipient, envelope.qc)
  IN /\ envelope \in qcNetwork
     /\ envelope.recipient \in up
     /\ QcWireValid(envelope.qc)
     /\ qcNetwork' = qcNetwork \ {envelope}
     /\ receivedQCs' = receivedQCs \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

BeginObservePrepare(node, qc) ==
  LET request == ObservePrepareWal(node, qc)
  IN /\ QcAt(node, qc) \in receivedQCs
     /\ qc.context = context
     /\ qc.phase = "Prepare"
     /\ qc.view <= nodeView[node]
     /\ qc.view > highestRank[node]
     /\ NodeIdle(node)
     /\ pendingObservePrepare' = pendingObservePrepare \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingLockCommit, pendingTimeout,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

PersistObservePrepare(request) ==
  /\ request \in pendingObservePrepare
  /\ highestRank' = [highestRank EXCEPT ![request.node] = request.qc.view]
  /\ highestSubject' =
       [highestSubject EXCEPT ![request.node] = request.qc.subject]
  /\ pendingObservePrepare' = pendingObservePrepare \ {request}
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 pendingProposal, pendingPrepare, pendingLockCommit,
                 pendingTimeout, pendingInstallTC, pendingDecision,
                 signProposals, signVotes, signTimeouts, proposalNetwork,
                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                 decisions, applied>>

BeginLockCommit(node, qc) ==
  LET vote == Vote(context, qc.view, "Commit", qc.subject, node)
      request == LockCommitWal(node, qc, vote)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ qc.context = context
     /\ qc.phase = "Prepare"
     /\ \/ CurrentOpenPrepareForCommit(node, qc)
        \/ HistoricalTcLockedPrepareForCommit(node, qc)
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ BodyValidatedBy(validatedBodies, node, context, qc.view,
                        generation[node], qc.subject)
     /\ NodeIdle(node)
     /\ qc.view >= lockRank[node]
     /\ (qc.view = lockRank[node] => qc.subject = lockSubject[node])
     /\ vote \notin commitIntents
     /\ pendingLockCommit' = pendingLockCommit \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingTimeout,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

PersistLockCommit(request) ==
  LET signRequest == VoteSign(request.node, request.vote)
      retained == RetainedLockedBodyRecord(
                    request.node, request.qc.context, request.qc.subject)
  IN /\ request \in pendingLockCommit
     /\ request.vote \notin commitIntents
     /\ BodyHeldBy(durableBodies, request.node, request.qc.context,
                    request.qc.view, request.qc.subject)
     /\ retained \in RetainedLockedBodyRecordSet
     /\ commitIntents' = commitIntents \cup {request.vote}
     /\ retainedLockedBodies' = retainedLockedBodies \cup {retained}
     /\ lockRank' = [lockRank EXCEPT ![request.node] = request.qc.view]
     /\ lockSubject' =
          [lockSubject EXCEPT ![request.node] = request.qc.subject]
     /\ highestRank' =
          [highestRank EXCEPT ![request.node] =
             IF request.qc.view > @ THEN request.qc.view ELSE @]
     /\ highestSubject' =
          [highestSubject EXCEPT ![request.node] =
             IF request.qc.view > highestRank[request.node]
             THEN request.qc.subject ELSE @]
     /\ pendingLockCommit' = pendingLockCommit \ {request}
     /\ signVotes' = signVotes \cup {signRequest}
     /\ receivedVotes' =
          {received \in receivedVotes:
             VoteReceiptSurvivesLockCommit(
               received, request.node, request.qc.view,
               request.qc.subject)}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, pendingProposal, pendingPrepare,
                    pendingObservePrepare, pendingTimeout, pendingInstallTC,
                    pendingDecision, signProposals, signTimeouts,
                    proposalNetwork, voteNetwork, qcNetwork, timeoutNetwork,
                    tcNetwork, decisions, applied>>

FormCommitQC(node, roundView, subject) ==
  LET signers == VoteSignersAt(node, roundView, "Commit", subject)
      qc == QC(context, roundView, "Commit", subject, signers)
      request == DecisionWal(node, qc, TRUE)
  IN /\ node \in up
     /\ CommitRoundAdmissible(node, roundView, subject)
     /\ QcWireValid(qc)
     /\ qc \in QcRecordSet
     /\ NodeIdle(node)
     /\ ~\E decision \in decisions:
           /\ decision.node = node
           /\ decision.qc.context = context
     /\ commitQCs' = commitQCs \cup {qc}
     /\ pendingDecision' = pendingDecision \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, signProposals, signVotes,
                    signTimeouts, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

BeginDecision(node, qc) ==
  LET request == DecisionWal(node, qc, FALSE)
  IN /\ node \in ValidatorIds
     /\ QcAt(node, qc) \in receivedQCs
     /\ qc.context = context
     /\ qc.phase = "Commit"
     /\ NodeIdle(node)
     /\ ~\E decision \in decisions:
           /\ decision.node = node
           /\ decision.qc.context = context
     /\ pendingDecision' = pendingDecision \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, signProposals, signVotes,
                    signTimeouts, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

PersistDecision(request) ==
  LET decision == [node |-> request.node, qc |-> request.qc]
  IN /\ request \in pendingDecision
     /\ decisions' = decisions \cup {decision}
     /\ pendingDecision' = pendingDecision \ {request}
     /\ qcNetwork' =
          IF request.rebroadcast
          THEN qcNetwork \cup BroadcastQCs(request.qc)
          ELSE qcNetwork
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, signProposals, signVotes,
                    signTimeouts, proposalNetwork, voteNetwork, timeoutNetwork,
                 tcNetwork, applied>>

LocalTimeoutVoteFor(node) ==
  TimeoutVote(context, nodeView[node], node,
              highestRank[node], highestSubject[node])

TimeoutRequestFor(node) == TimeoutWal(node, LocalTimeoutVoteFor(node))

BeginTimeout(node) ==
  LET roundView == nodeView[node]
      vote == LocalTimeoutVoteFor(node)
      request == TimeoutRequestFor(node)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ NodeIdle(node)
     /\ NoDecisionForNode(node)
     /\ ~NodeTimedOut(node, roundView)
     /\ request \in TimeoutWalSet
     /\ pendingTimeout' = pendingTimeout \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

PersistTimeout(request) ==
  LET signRequest == TimeoutSign(request.node, request.vote)
  IN /\ request \in pendingTimeout
     /\ request.vote \notin timeoutIntents
     /\ timeoutIntents' = timeoutIntents \cup {request.vote}
     /\ pendingTimeout' = pendingTimeout \ {request}
     /\ signTimeouts' = signTimeouts \cup {signRequest}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

CompleteTimeoutSignature(request) ==
  /\ request \in signTimeouts
  /\ request.vote.signer = request.node
  /\ request.vote \in timeoutIntents
  /\ signTimeouts' = signTimeouts \ {request}
  /\ timeoutNetwork' =
       timeoutNetwork \cup BroadcastTimeouts(request.vote)
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal, pendingPrepare,
                 pendingObservePrepare, pendingLockCommit, pendingTimeout,
                 pendingInstallTC, pendingDecision, signProposals, signVotes,
                 proposalNetwork, voteNetwork, qcNetwork, tcNetwork,
                 decisions, applied>>

ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject) ==
  LET vote == TimeoutVote(context, roundView, signer, highRank, highSubject)
  IN /\ signer \in Byzantine(CurrentEpoch) \cap up
     /\ roundView \in Views
     /\ AuthenticatedHighRef(highRank, highSubject)
     /\ highRank <= roundView
     /\ timeoutNetwork' = timeoutNetwork \cup BroadcastTimeouts(vote)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, tcNetwork, decisions, applied>>

\* Timeout delivery is the reducer ingress boundary.  Require the complete
\* wire schema here: checking selected fields (including vote.view) cannot
\* exclude records with a non-canonical DOMAIN.  The remaining guards perform
\* current-context, roster, authenticated-high-reference, and rank admission.
DeliverTimeout(envelope) ==
  LET received == TimeoutVoteAt(envelope.recipient, envelope.vote)
  IN /\ envelope \in TimeoutEnvelopeSet
     /\ envelope \in timeoutNetwork
     /\ envelope.recipient \in up
     /\ envelope.vote.context = context
     /\ envelope.vote.height = height
     /\ envelope.vote.signer \in CurrentVoters
     /\ AuthenticatedHighRef(envelope.vote.highRank,
                             envelope.vote.highSubject)
     /\ envelope.vote.highRank <= envelope.vote.view
     /\ timeoutNetwork' = timeoutNetwork \ {envelope}
     /\ receivedTimeoutVotes' =
          IF ~NoDecisionForNode(envelope.recipient)
             \/ TimeoutVoteSlotOccupied(envelope.recipient, envelope.vote)
          THEN receivedTimeoutVotes
          ELSE receivedTimeoutVotes \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTCs, proposalIntents, prepareIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, tcNetwork, decisions, applied>>

FormTC(node, roundView) ==
  LET votes == TimeoutVotesAt(node, roundView)
      tc == TC(context, roundView, votes)
      request == InstallTcWal(node, tc, TRUE)
  IN /\ node \in up
     /\ NodeIdle(node)
     /\ NoDecisionForNode(node)
     /\ roundView + 1 \in Views
     /\ roundView >= nodeView[node]
     /\ TCValid(tc)
     /\ formedTCs' = formedTCs \cup {tc}
     /\ pendingInstallTC' = pendingInstallTC \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingDecision, signProposals, signVotes,
                    signTimeouts, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

DeliverTC(envelope) ==
  LET received == TcAt(envelope.recipient, envelope.tc)
  IN /\ envelope \in tcNetwork
     /\ envelope.recipient \in up
     /\ TCValid(envelope.tc)
     /\ tcNetwork' = tcNetwork \ {envelope}
     /\ receivedTCs' =
          IF NoDecisionForNode(envelope.recipient)
          THEN receivedTCs \cup {received}
          ELSE receivedTCs
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, proposalIntents, prepareIntents,
                    commitIntents, timeoutIntents, prepareQCs, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, decisions, applied>>

BeginInstallTC(node, tc) ==
  LET request == InstallTcWal(node, tc, FALSE)
  IN /\ TcAt(node, tc) \in receivedTCs
     /\ tc.view + 1 \in Views
     /\ tc.view >= nodeView[node]
     /\ NodeIdle(node)
     /\ NoDecisionForNode(node)
     /\ pendingInstallTC' = pendingInstallTC \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingDecision, signProposals, signVotes,
                    signTimeouts, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

PersistInstallTC(request) ==
  LET node == request.node
      tc == request.tc
      selectedRank == TcHighRank(tc)
      selectedSubject == TcHighSubject(tc)
      installed == [node |-> node, tc |-> tc]
      advancesHigh == selectedRank > highestRank[node]
      advancesLock == selectedRank > lockRank[node]
  IN /\ request \in pendingInstallTC
     /\ tc.view >= nodeView[node]
     /\ nodeView' = [nodeView EXCEPT ![node] = tc.view + 1]
     /\ generation' =
          [generation EXCEPT ![node] = IF @ < MaxGeneration THEN @ + 1 ELSE @]
     /\ highestRank' =
          [highestRank EXCEPT ![node] = IF advancesHigh THEN selectedRank ELSE @]
     /\ highestSubject' =
          [highestSubject EXCEPT ![node] =
             IF advancesHigh THEN selectedSubject ELSE @]
     /\ lockRank' =
          [lockRank EXCEPT ![node] = IF advancesLock THEN selectedRank ELSE @]
     /\ lockSubject' =
          [lockSubject EXCEPT ![node] =
             IF advancesLock THEN selectedSubject ELSE @]
     /\ installedTCs' = installedTCs \cup {installed}
     /\ pendingInstallTC' = pendingInstallTC \ {request}
     /\ receivedVotes' =
          {received \in receivedVotes: received.node # node}
     /\ signVotes' =
          signVotes
            \cup ActiveLockedCommitSignRequestsAfterInstall(node, tc)
     /\ tcNetwork' =
          IF request.rebroadcast
          THEN tcNetwork \cup BroadcastTCs(tc)
          ELSE tcNetwork
     /\ UNCHANGED <<height, context, contextHistory, up, gst,
                    availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, pendingProposal, pendingPrepare,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingDecision, signProposals, signTimeouts,
                    proposalNetwork, voteNetwork, qcNetwork, timeoutNetwork,
                    decisions, applied>>

FetchCertifiedBody(node, qc) ==
  LET body == BodyRecord(node, context, qc.view, qc.subject)
  IN /\ \E decision \in decisions:
           /\ decision.node = node
           /\ decision.qc = qc
     /\ qc.phase = "Commit"
     /\ qc.context = context
     /\ ~BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ body \in BodyRecordSet
     /\ availableBodies' = availableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, durableBodies, retainedLockedBodies,
                    validatedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

ApplyDecision(node, qc) ==
  LET application == [node |-> node, qc |-> qc]
  IN /\ [node |-> node, qc |-> qc] \in decisions
     /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ \E validation \in validatedBodies:
           /\ validation.node = node
           /\ validation.context = context
           /\ validation.view = qc.view
           /\ validation.subject = qc.subject
     /\ application \notin applied
     /\ applied' = applied \cup {application}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions>>

Crash(node) ==
  /\ node \in up
  /\ up' = up \ {node}
  \* Adapter staging, retained aliases, validation receipts, ingress
  \* knowledge, and quorum accumulators are process-local.  Exact bodies and
  \* immutable WAL/network ghosts below survive the process boundary.
  /\ availableBodies' =
       {body \in availableBodies: body.node # node}
  /\ retainedLockedBodies' =
       {body \in retainedLockedBodies: body.node # node}
  /\ validatedBodies' =
       {validation \in validatedBodies: validation.node # node}
  /\ invalidBodies' = {body \in invalidBodies: body.node # node}
  /\ seenProposals' = {entry \in seenProposals: entry.node # node}
  /\ receivedVotes' = {entry \in receivedVotes: entry.node # node}
  /\ receivedQCs' = {entry \in receivedQCs: entry.node # node}
  /\ receivedTimeoutVotes' =
       {entry \in receivedTimeoutVotes: entry.node # node}
  /\ receivedTCs' = {entry \in receivedTCs: entry.node # node}
  /\ pendingProposal' = {request \in pendingProposal: request.node # node}
  /\ pendingPrepare' = {request \in pendingPrepare: request.node # node}
  /\ pendingObservePrepare' =
       {request \in pendingObservePrepare: request.node # node}
  /\ pendingLockCommit' =
       {request \in pendingLockCommit: request.node # node}
  /\ pendingTimeout' = {request \in pendingTimeout: request.node # node}
  /\ pendingInstallTC' =
       {request \in pendingInstallTC: request.node # node}
  /\ pendingDecision' = {request \in pendingDecision: request.node # node}
  /\ signProposals' = {request \in signProposals: request.node # node}
  /\ signVotes' = {request \in signVotes: request.node # node}
  /\ signTimeouts' = {request \in signTimeouts: request.node # node}
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation, gst,
                 durableBodies, proposalIntents, prepareIntents, commitIntents,
                 timeoutIntents, prepareQCs, commitQCs, formedTCs,
                 installedTCs, lockRank, lockSubject, highestRank,
                 highestSubject, proposalNetwork, voteNetwork, qcNetwork,
                 timeoutNetwork, tcNetwork, decisions, applied>>

Restart(node) ==
  /\ node \in ValidatorIds \ up
  /\ generation[node] < MaxGeneration
  /\ up' = up \cup {node}
  /\ generation' = [generation EXCEPT ![node] = @ + 1]
  /\ UNCHANGED <<height, context, contextHistory, nodeView, gst,
                 availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal,
                 pendingPrepare, pendingObservePrepare, pendingLockCommit,
                 pendingTimeout, pendingInstallTC, pendingDecision,
                 signProposals, signVotes, signTimeouts, proposalNetwork,
                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                 decisions, applied>>

ResumeProposal(node, proposal) ==
  LET request == ProposalSign(node, proposal)
  IN /\ node \in up \cap Honest
     /\ NodeIdle(node)
     /\ proposal \in proposalIntents
     /\ proposal.proposer = node
     /\ proposal.context = context
     /\ proposal.view = nodeView[node]
     /\ ~NodeTimedOut(node, proposal.view)
     /\ signProposals' = signProposals \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

(***************************************************************************
Replay may reconstruct an exact Commit intent after the node timed out or
installed a later TC.  That durable vote remains signable only while it is the
active Prepare lock's exact round and subject.  Prepare replay stays confined
to the open current view; neither branch can create a new durable intent.
***************************************************************************)
VoteResumeAuthorized(node, vote) ==
  \/ /\ vote.phase = "Prepare"
     /\ vote \in prepareIntents
     /\ vote.view = nodeView[node]
     /\ ~NodeTimedOut(node, vote.view)
  \/ /\ vote.phase = "Commit"
     /\ vote \in commitIntents
     /\ vote.view <= nodeView[node]
     /\ LockedPrepareRound(node, vote.view, vote.subject)

ResumeVote(node, vote) ==
  LET request == VoteSign(node, vote)
  IN /\ node \in up \cap Honest
     /\ NodeIdle(node)
     /\ vote.signer = node
     /\ vote.context = context
     /\ VoteResumeAuthorized(node, vote)
     /\ signVotes' = signVotes \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

ResumeTimeout(node, vote) ==
  LET request == TimeoutSign(node, vote)
  IN /\ node \in up \cap Honest
     /\ NodeIdle(node)
     /\ vote \in timeoutIntents
     /\ vote.signer = node
     /\ vote.context = context
     /\ vote.view = nodeView[node]
     /\ signTimeouts' = signTimeouts \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

DropProposal(envelope) ==
  /\ envelope \in proposalNetwork
  /\ ~gst
  /\ proposalNetwork' = proposalNetwork \ {envelope}
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation, up,
                 gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal, pendingPrepare,
                 pendingObservePrepare, pendingLockCommit, pendingTimeout,
                 pendingInstallTC, pendingDecision, signProposals, signVotes,
                 signTimeouts, voteNetwork, qcNetwork, timeoutNetwork,
                 tcNetwork, decisions, applied>>

Next ==
  \/ SetGST
  \/ \E node \in ValidatorIds, subject \in Subjects:
       AssembleLocalBody(node, subject)
  \/ \E node \in ValidatorIds, subject \in Subjects:
       BeginLocalProposal(node, subject)
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       subject \in Subjects, justifyRank \in Ranks,
       justifySubject \in SubjectOrNone:
       ByzantineBroadcastProposal(signer, roundView, subject,
                                  justifyRank, justifySubject)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal) \/ RebindRetainedBody(node, proposal)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       StoreBody(node, roundView, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal) \/ RejectBody(node, proposal)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ValidateDecidedBody(node, qc)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       ByzantineBroadcastVote(signer, roundView, phase, subject)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormPrepareQC(node, roundView, subject)
  \/ \E envelope \in QcEnvelopeSet:
       ImportAuthenticatedCommitCertificate(envelope)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare: PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
       BeginLockCommit(node, qc)
  \/ \E request \in pendingLockCommit: PersistLockCommit(request)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormCommitQC(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginDecision(node, qc)
  \/ \E request \in pendingDecision: PersistDecision(request)
  \/ \E node \in ValidatorIds: BeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views, highRank \in Ranks,
       highSubject \in SubjectOrNone:
       ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views: FormTC(node, roundView)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)
  \/ \E request \in pendingInstallTC: PersistInstallTC(request)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ApplyDecision(node, qc)
  \/ \E node \in ValidatorIds: Crash(node) \/ Restart(node)
  \/ \E node \in ValidatorIds, proposal \in proposalIntents:
       ResumeProposal(node, proposal)
  \/ \E node \in ValidatorIds, vote \in prepareIntents \cup commitIntents:
       ResumeVote(node, vote)
  \/ \E node \in ValidatorIds, vote \in timeoutIntents:
       ResumeTimeout(node, vote)
  \/ \E envelope \in proposalNetwork: DropProposal(envelope)

TypeInvariant ==
  /\ ModelConfiguration
  /\ height \in Heights
  /\ context \in ContextRecords
  /\ context.height = height
  /\ contextHistory \subseteq ContextRecords
  /\ context \in contextHistory
  /\ nodeView \in [ValidatorIds -> Views]
  /\ generation \in [ValidatorIds -> Generations]
  /\ up \subseteq ValidatorIds
  /\ gst \in BOOLEAN
  /\ availableBodies \subseteq BodyRecordSet
  /\ durableBodies \subseteq BodyRecordSet
  /\ retainedLockedBodies \subseteq RetainedLockedBodyRecordSet
  /\ validatedBodies \subseteq ValidationRecordSet
  /\ invalidBodies \subseteq BodyRecordSet
  /\ ValidatedBodiesSound(validatedBodies, ValidSubjects)
  /\ RetainedLockedBodiesSound(retainedLockedBodies, durableBodies)
  /\ proposalIntents \subseteq ProposalRecordSet
  /\ prepareIntents \subseteq VoteRecordSet
  /\ commitIntents \subseteq VoteRecordSet
  /\ timeoutIntents \subseteq TimeoutVoteRecordSet
  /\ prepareQCs \subseteq QcRecordSet
  /\ commitQCs \subseteq QcRecordSet
  /\ \A tc \in formedTCs: TcWellTyped(tc)
  /\ \A entry \in receivedTCs:
       /\ entry.node \in ValidatorIds
       /\ TcWellTyped(entry.tc)
  /\ \A entry \in installedTCs:
       /\ entry.node \in ValidatorIds
       /\ TcWellTyped(entry.tc)
  /\ lockRank \in [ValidatorIds -> Ranks]
  /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
  /\ highestRank \in [ValidatorIds -> Ranks]
  /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
  /\ pendingProposal \subseteq ProposalWalSet
  /\ pendingPrepare \subseteq PrepareWalSet
  /\ pendingObservePrepare \subseteq ObservePrepareWalSet
  /\ pendingLockCommit \subseteq LockCommitWalSet
  /\ pendingTimeout \subseteq TimeoutWalSet
  /\ pendingInstallTC \subseteq InstallTcWalSet
  /\ pendingDecision \subseteq DecisionWalSet
  /\ signProposals \subseteq ProposalSignSet
  /\ signVotes \subseteq VoteSignSet
  /\ signTimeouts \subseteq TimeoutSignSet

OnePendingPersistencePerNode ==
  RequestsUniqueByNode(AllPendingRequests)

PrepareSigningRequiresIntent ==
  \A request \in signVotes:
    request.vote.phase = "Prepare" => request.vote \in prepareIntents

CommitSigningRequiresIntent ==
  \A request \in signVotes:
    request.vote.phase = "Commit" => request.vote \in commitIntents

TimeoutSigningRequiresIntent ==
  \A request \in signTimeouts: request.vote \in timeoutIntents

ProposalSigningRequiresIntent ==
  \A request \in signProposals: request.proposal \in proposalIntents

HonestPrepareUniqueness ==
  \A left, right \in prepareIntents:
    (left.signer \in Honest /\ right.signer = left.signer
     /\ right.context = left.context /\ right.view = left.view)
      => right.subject = left.subject

HonestCommitUniqueness ==
  \A left, right \in commitIntents:
    (left.signer \in Honest /\ right.signer = left.signer
     /\ right.context = left.context /\ right.view = left.view)
      => right.subject = left.subject

HonestTimeoutUniqueness ==
  \A left, right \in timeoutIntents:
    (left.signer \in Honest /\ right.signer = left.signer
     /\ right.context = left.context /\ right.view = left.view)
      => /\ right.highRank = left.highRank
         /\ right.highSubject = left.highSubject

LockBelowHighest ==
  \A node \in ValidatorIds: lockRank[node] <= highestRank[node]

DecisionAgreement ==
  /\ \A decision \in decisions: decision.qc \in commitQCs
  /\ \A left, right \in decisions:
       left.qc.context = right.qc.context
         => left.qc.subject = right.qc.subject

OldViewCommitQCAccepted ==
  \A request \in pendingDecision:
    request.qc.phase = "Commit" /\ request.qc.view <= nodeView[request.node]

AppliedRequiresDecision ==
  applied \subseteq decisions

Safety ==
  /\ TypeInvariant
  /\ OnePendingPersistencePerNode
  /\ ProposalSigningRequiresIntent
  /\ PrepareSigningRequiresIntent
  /\ CommitSigningRequiresIntent
  /\ TimeoutSigningRequiresIntent
  /\ HonestPrepareUniqueness
  /\ HonestCommitUniqueness
  /\ HonestTimeoutUniqueness
  /\ LockBelowHighest
  /\ DecisionAgreement
  /\ AppliedRequiresDecision

CoreSpec == Init /\ [][Next]_vars

CoreSpecAt(initialContext) == InitAt(initialContext) /\ [][Next]_vars

\* Static successor for the two quorum-only counterexample configurations.
QuorumCheckNext == UNCHANGED vars

GenesisDecisionExists ==
  \E decision \in decisions: decision.qc.context.height = 0

PostGstEventuallyGenesisDecision == gst ~> GenesisDecisionExists

=============================================================================
