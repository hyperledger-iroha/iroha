---- MODULE SumeragiV2Core ----
EXTENDS SumeragiV2Availability, Sequences

(***************************************************************************
Production-aligned abstract reducer network for Sumeragi v2.

Each honest validator has its own persisted view, generation, lock, highest
PrepareQC, WAL intents, body state, pending persistence request, and pending
signature.  Network envelopes are addressed per recipient, so view divergence,
loss, duplication, reordering, old-view CommitQCs, and future-view TCs are all
representable.  Byzantine validators may emit arbitrary structurally valid
votes and timeout votes under their own identity; honest signatures are
reachable only through the matching persisted intent.

The model abstracts signature verification, hashing, deterministic execution,
and fsync behind the same trusted adapter contracts as production.  The
Persistence actions are successful fsync acknowledgements, not write requests.
***************************************************************************)

CONSTANTS
  MaxHeight,
  MaxView,
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
Views == 0..MaxView
Generations == 0..MaxGeneration
Phases == {"Prepare", "Commit"}
NoRank == -1
Ranks == NoRank..MaxView

CountRostersOneEpoch == << <<0, 1, 2, 3>> >>
CountRostersTwoEpochs == << <<0, 1, 2, 3>>, <<0, 1, 2, 3>> >>
CountPowersOneEpoch == << <<1, 1, 1, 1>> >>
CountPowersTwoEpochs == << <<1, 1, 1, 1>>, <<1, 1, 1, 1>> >>
StakePowersOneEpoch == << <<4, 3, 2, 1>> >>
StakePowersTwoEpochs == << <<4, 3, 2, 1>>, <<2, 4, 3, 1>> >>
StartsHeightZero == <<0>>
StartsHeightZeroOne == <<0, 1>>
StartsByzantineFirst == <<3>>
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
    availableBodies, durableBodies, validatedBodies, invalidBodies,
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
  {VoteEnvelope(recipient, vote): recipient \in CurrentVoters}
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

HighRefValid(highRank, highSubject) ==
  \/ /\ highRank = NoRank
     /\ highSubject = NoSubject
  \/ /\ highRank \in Views
     /\ highSubject \in Subjects
     /\ \E qc \in prepareQCs:
          /\ qc.context = context
          /\ qc.view = highRank
          /\ qc.subject = highSubject

QcValid(qc) ==
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.view \in Views
  /\ qc.phase \in Phases
  /\ qc.subject \in ValidSubjects
  /\ DualQuorum(CurrentEpoch, qc.signers)

VoteBacksCertificate(vote, qc, signer) ==
  /\ vote.context = qc.context
  /\ vote.view = qc.view
  /\ vote.phase = qc.phase
  /\ vote.subject = qc.subject
  /\ vote.signer = signer

CertificateHonestIntentBacked(qc, intents) ==
  \A signer \in qc.signers \cap Honest:
    \E vote \in intents: VoteBacksCertificate(vote, qc, signer)

TimeoutVoteProtectsCommitSet(timeoutVote, commitSet) ==
  \A commitVote \in commitSet:
    (/\ timeoutVote.signer \in Honest
     /\ commitVote.signer = timeoutVote.signer
     /\ commitVote.context = timeoutVote.context
     /\ commitVote.phase = "Commit"
     /\ commitVote.view <= timeoutVote.view)
    => /\ timeoutVote.highRank >= commitVote.view
       /\ (timeoutVote.highRank = commitVote.view
             => timeoutVote.highSubject = commitVote.subject)

TimeoutSignerSet(votes) == {vote.signer: vote \in votes}

TimeoutVotesDisjoint(votes) ==
  Cardinality(TimeoutSignerSet(votes)) = Cardinality(votes)

TimeoutHighsConflictFree(votes) ==
  \A left, right \in votes:
    (left.highRank = right.highRank /\ left.highRank # NoRank)
      => left.highSubject = right.highSubject

HighestTimeoutVote(votes) ==
  CHOOSE candidate \in votes:
    \A other \in votes: candidate.highRank >= other.highRank

TCValid(tc) ==
  /\ tc.context = context
  /\ tc.height = height
  /\ tc.view \in Views
  /\ tc.votes # {}
  /\ \A vote \in tc.votes:
       /\ vote.context = context
       /\ vote.height = height
       /\ vote.view = tc.view
       /\ vote.signer \in CurrentVoters
       /\ HighRefValid(vote.highRank, vote.highSubject)
       /\ vote.highRank <= tc.view
  /\ TimeoutVotesDisjoint(tc.votes)
  /\ TimeoutHighsConflictFree(tc.votes)
  /\ DualQuorum(CurrentEpoch, TimeoutSignerSet(tc.votes))

TcHighRank(tc) == HighestTimeoutVote(tc.votes).highRank
TcHighSubject(tc) == HighestTimeoutVote(tc.votes).highSubject

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
          /\ HighRefValid(proposal.justifyRank,
                          proposal.justifySubject)
          /\ proposal.justifyRank < proposal.view

SafeToPrepare(node, proposal) ==
  \/ lockRank[node] = NoRank
  \/ lockSubject[node] = proposal.subject
  \/ /\ proposal.justifyRank > lockRank[node]
     /\ proposal.justifySubject = proposal.subject

ProposalValidFor(node, proposal) ==
  /\ proposal.context = context
  /\ proposal.height = height
  /\ proposal.view = nodeView[node]
  /\ proposal.proposer = Leader(context, proposal.view)
  /\ proposal.subject \in ValidSubjects
  /\ ProposalJustified(node, proposal)
  /\ SafeToPrepare(node, proposal)

VoteSignersAt(node, roundView, phase, subject) ==
  {received.vote.signer:
    received \in {entry \in receivedVotes:
      /\ entry.node = node
      /\ entry.vote.context = context
      /\ entry.vote.view = roundView
      /\ entry.vote.phase = phase
      /\ entry.vote.subject = subject}}

TimeoutVotesAt(node, roundView) ==
  {received.vote:
    received \in {entry \in receivedTimeoutVotes:
      /\ entry.node = node
      /\ entry.vote.context = context
      /\ entry.vote.view = roundView}}

ModelConfiguration ==
  /\ QuorumConfiguration
  /\ MaxHeight \in Nat
  /\ MaxView \in Nat
  /\ MaxGeneration \in Nat
  /\ EpochLength \in Nat \ {0}
  /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
  /\ Len(LeaderStarts) = MaxHeight + 1
  /\ Len(LaneHashes) = MaxHeight + 1
  /\ Len(DaHashes) = MaxHeight + 1
  /\ \A index \in 1..Len(LeaderStarts):
       LeaderStarts[index] \in 0..(N - 1)
  /\ ProtocolVersionValue = 2
  /\ ValidSubjects \subseteq Subjects
  /\ ValidSubjects # {}
  /\ Responsive \subseteq Honest
  /\ \A epoch \in Epochs:
       DualQuorum(epoch, Responsive \cap VotingRoster(epoch))

Init ==
  /\ ModelConfiguration
  /\ height = 0
  /\ context = ContextRecord(0, <<>>)
  /\ contextHistory = {context}
  /\ nodeView = [node \in ValidatorIds |-> 0]
  /\ generation = [node \in ValidatorIds |-> 0]
  /\ up = ValidatorIds
  /\ gst = FALSE
  /\ availableBodies = {}
  /\ durableBodies = {}
  /\ validatedBodies = {}
  /\ invalidBodies = {}
  /\ seenProposals = {}
  /\ receivedVotes = {}
  /\ receivedQCs = {}
  /\ receivedTimeoutVotes = {}
  /\ receivedTCs = {}
  /\ proposalIntents = {}
  /\ prepareIntents = {}
  /\ commitIntents = {}
  /\ timeoutIntents = {}
  /\ prepareQCs = {}
  /\ commitQCs = {}
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
  /\ decisions = {}
  /\ applied = {}

SetGST ==
  /\ ~gst
  /\ Responsive \subseteq up
  /\ gst' = TRUE
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation, up,
                 availableBodies, durableBodies, validatedBodies,
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

AssembleLocalBody(node, subject) ==
  LET body == BodyRecord(node, context, subject)
      validation == ValidationRecord(node, context, nodeView[node],
                                      generation[node], subject)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ node = Leader(context, nodeView[node])
     /\ subject \in ValidSubjects
     /\ ~BodyHeldBy(durableBodies, node, context, subject)
     /\ durableBodies' = durableBodies \cup {body}
     /\ validatedBodies' = validatedBodies \cup {validation}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, invalidBodies, seenProposals,
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
     /\ BodyHeldBy(durableBodies, node, context, subject)
     /\ BodyValidatedBy(validatedBodies, node, context, roundView,
                        generation[node], subject)
     /\ ProposalValidFor(node, proposal)
     /\ ~\E prior \in proposalIntents:
           /\ prior.proposer = node
           /\ prior.context = context
           /\ prior.view = roundView
     /\ proposal \notin proposalIntents
     /\ request \in ProposalWalSet
     /\ request \notin pendingProposal
     /\ pendingProposal' = pendingProposal \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  /\ request.proposal \in proposalIntents
  /\ signProposals' = signProposals \ {request}
  /\ proposalNetwork' =
       proposalNetwork \cup BroadcastProposals(request.proposal)
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal, pendingPrepare,
                 pendingObservePrepare, pendingLockCommit, pendingTimeout,
                 pendingInstallTC, pendingDecision, signVotes, signTimeouts,
                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                 decisions, applied>>

DeliverProposal(envelope) ==
  LET seen == ProposalAt(envelope.recipient, envelope.proposal)
  IN /\ envelope \in proposalNetwork
     /\ envelope.recipient \in up
     /\ ProposalValidFor(envelope.recipient, envelope.proposal)
     /\ proposalNetwork' = proposalNetwork \ {envelope}
     /\ seenProposals' = seenProposals \cup {seen}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  LET body == BodyRecord(node, context, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ body \notin availableBodies \cup durableBodies
     /\ availableBodies' = availableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, durableBodies, validatedBodies, invalidBodies,
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

StoreBody(node, subject) ==
  LET body == BodyRecord(node, context, subject)
  IN /\ body \in availableBodies
     /\ availableBodies' = availableBodies \ {body}
     /\ durableBodies' = durableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, validatedBodies, invalidBodies, seenProposals,
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
     /\ BodyHeldBy(durableBodies, node, context, proposal.subject)
     /\ proposal.subject \in ValidSubjects
     /\ validation \notin validatedBodies
     /\ validatedBodies' = validatedBodies \cup {validation}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, invalidBodies,
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
  LET body == BodyRecord(node, context, proposal.subject)
  IN /\ ProposalAt(node, proposal) \in seenProposals
     /\ BodyHeldBy(durableBodies, node, context, proposal.subject)
     /\ proposal.subject \notin ValidSubjects
     /\ invalidBodies' = invalidBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ ProposalValidFor(node, proposal)
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
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  /\ (request.vote \in prepareIntents \/ request.vote \in commitIntents)
  /\ signVotes' = signVotes \ {request}
  /\ voteNetwork' = voteNetwork \cup BroadcastVotes(request.vote)
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
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
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ voteNetwork' = voteNetwork \ {envelope}
     /\ receivedVotes' = receivedVotes \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

FormPrepareQC(node, roundView, subject) ==
  LET signers == VoteSignersAt(node, roundView, "Prepare", subject)
      qc == QC(context, roundView, "Prepare", subject, signers)
  IN /\ node \in up
     /\ QcValid(qc)
     /\ qc \in QcRecordSet
     /\ CertificateHonestIntentBacked(qc, prepareIntents)
     /\ qc \notin prepareQCs
     /\ prepareQCs' = prepareQCs \cup {qc}
     /\ qcNetwork' = qcNetwork \cup BroadcastQCs(qc)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, commitQCs,
                    formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

DeliverQC(envelope) ==
  LET received == QcAt(envelope.recipient, envelope.qc)
  IN /\ envelope \in qcNetwork
     /\ envelope.recipient \in up
     /\ QcValid(envelope.qc)
     /\ qcNetwork' = qcNetwork \ {envelope}
     /\ receivedQCs' = receivedQCs \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ qc.phase = "Prepare"
     /\ qc.view <= nodeView[node]
     /\ qc.view > highestRank[node]
     /\ NodeIdle(node)
     /\ pendingObservePrepare' = pendingObservePrepare \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
                 up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ QcAt(node, qc) \in receivedQCs
     /\ qc.phase = "Prepare"
     /\ qc.view = nodeView[node]
     /\ ~NodeTimedOut(node, qc.view)
     /\ BodyHeldBy(durableBodies, node, context, qc.subject)
     /\ BodyValidatedBy(validatedBodies, node, context, qc.view,
                        generation[node], qc.subject)
     /\ NodeIdle(node)
     /\ qc.view >= lockRank[node]
     /\ (qc.view = lockRank[node] => qc.subject = lockSubject[node])
     /\ vote \notin commitIntents
     /\ pendingLockCommit' = pendingLockCommit \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  IN /\ request \in pendingLockCommit
     /\ request.vote \notin commitIntents
     /\ commitIntents' = commitIntents \cup {request.vote}
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
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
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
     /\ QcValid(qc)
     /\ qc \in QcRecordSet
     /\ CertificateHonestIntentBacked(qc, commitIntents)
     /\ qc \notin commitQCs
     /\ NodeIdle(node)
     /\ commitQCs' = commitQCs \cup {qc}
     /\ pendingDecision' = pendingDecision \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  IN /\ QcAt(node, qc) \in receivedQCs
     /\ qc.phase = "Commit"
     /\ NodeIdle(node)
     /\ ~\E decision \in decisions:
           /\ decision.node = node
           /\ decision.qc.context = context
     /\ pendingDecision' = pendingDecision \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ ~NodeTimedOut(node, roundView)
     /\ HighRefValid(vote.highRank, vote.highSubject)
     /\ TimeoutVoteProtectsCommitSet(vote, commitIntents)
     /\ request \in TimeoutWalSet
     /\ pendingTimeout' = pendingTimeout \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  /\ request.vote \in timeoutIntents
  /\ signTimeouts' = signTimeouts \ {request}
  /\ timeoutNetwork' =
       timeoutNetwork \cup BroadcastTimeouts(request.vote)
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ HighRefValid(highRank, highSubject)
     /\ highRank <= roundView
     /\ timeoutNetwork' = timeoutNetwork \cup BroadcastTimeouts(vote)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, tcNetwork, decisions, applied>>

DeliverTimeout(envelope) ==
  LET received == TimeoutVoteAt(envelope.recipient, envelope.vote)
  IN /\ envelope \in timeoutNetwork
     /\ envelope.recipient \in up
     /\ envelope.vote.context = context
     /\ envelope.vote.signer \in CurrentVoters
     /\ HighRefValid(envelope.vote.highRank, envelope.vote.highSubject)
     /\ envelope.vote.highRank <= envelope.vote.view
     /\ timeoutNetwork' = timeoutNetwork \ {envelope}
     /\ receivedTimeoutVotes' = receivedTimeoutVotes \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ roundView < MaxView
     /\ roundView >= nodeView[node]
     /\ TCValid(tc)
     /\ tc \notin formedTCs
     /\ formedTCs' = formedTCs \cup {tc}
     /\ pendingInstallTC' = pendingInstallTC \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ receivedTCs' = receivedTCs \cup {received}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ tc.view < MaxView
     /\ tc.view >= nodeView[node]
     /\ NodeIdle(node)
     /\ pendingInstallTC' = pendingInstallTC \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ tcNetwork' =
          IF request.rebroadcast
          THEN tcNetwork \cup BroadcastTCs(tc)
          ELSE tcNetwork
     /\ UNCHANGED <<height, context, contextHistory, up, gst,
                    availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, pendingProposal, pendingPrepare,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingDecision, signProposals, signVotes, signTimeouts,
                    proposalNetwork, voteNetwork, qcNetwork, timeoutNetwork,
                    decisions, applied>>

FetchCertifiedBody(node, qc) ==
  LET body == BodyRecord(node, context, qc.subject)
  IN /\ \E decision \in decisions:
           /\ decision.node = node
           /\ decision.qc = qc
     /\ qc.phase = "Commit"
     /\ ~BodyHeldBy(durableBodies, node, context, qc.subject)
     /\ CertifiedBodyAvailable(CurrentEpoch, qc.signers, durableBodies,
                               context, qc.subject)
     /\ availableBodies' = availableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, durableBodies, validatedBodies, invalidBodies,
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
     /\ BodyHeldBy(durableBodies, node, context, qc.subject)
     /\ \E validation \in validatedBodies:
           /\ validation.node = node
           /\ validation.context = context
           /\ validation.subject = qc.subject
     /\ application \notin applied
     /\ applied' = applied \cup {application}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
  /\ ~(gst /\ node \in Responsive)
  /\ up' = up \ {node}
  /\ validatedBodies' =
       {validation \in validatedBodies: validation.node # node}
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
                 availableBodies, durableBodies, invalidBodies, seenProposals,
                 receivedVotes, receivedQCs, receivedTimeoutVotes, receivedTCs,
                 proposalIntents, prepareIntents, commitIntents,
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
                 availableBodies, durableBodies, validatedBodies,
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
     /\ proposal.context = context
     /\ proposal.view = nodeView[node]
     /\ ~NodeTimedOut(node, proposal.view)
     /\ signProposals' = signProposals \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

ResumeVote(node, vote) ==
  LET request == VoteSign(node, vote)
  IN /\ node \in up \cap Honest
     /\ NodeIdle(node)
     /\ vote.context = context
     /\ vote.view = nodeView[node]
     /\ ~NodeTimedOut(node, vote.view)
     /\ (vote \in prepareIntents \/ vote \in commitIntents)
     /\ signVotes' = signVotes \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
     /\ vote.context = context
     /\ vote.view = nodeView[node]
     /\ signTimeouts' = signTimeouts \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, validatedBodies,
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
                 gst, availableBodies, durableBodies, validatedBodies,
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
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal)
  \/ \E node \in ValidatorIds, subject \in Subjects: StoreBody(node, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal) \/ RejectBody(node, proposal)
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
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare: PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
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

(***************************************************************************
Trusted-contract liveness corridor used only by liveness.cfg.  After GST the
protocol assumptions rule out message loss, crashes of responsive validators,
and a timeout beating a responsive leader's bounded successful round.  The
relation below applies exactly those assumptions: it removes Byzantine noise,
loss, crash/replay actions, and permits a timeout only while the expected
leader for that validator's view is not responsive.  Every retained action is
the same production action used by Next.
***************************************************************************)

ReliableBeginTimeout(node) ==
  /\ Leader(context, nodeView[node]) \notin Responsive
  /\ BeginTimeout(node)

HeartbeatSubject == CHOOSE subject \in ValidSubjects: TRUE

HonestProposalSubject(node) ==
  IF lockRank[node] = NoRank THEN HeartbeatSubject ELSE lockSubject[node]

ReliableAssembleLocalBody(node) ==
  AssembleLocalBody(node, HonestProposalSubject(node))

ReliableBeginLocalProposal(node) ==
  BeginLocalProposal(node, HonestProposalSubject(node))

ReliableNext ==
  \/ SetGST
  \/ \E node \in ValidatorIds: ReliableAssembleLocalBody(node)
  \/ \E node \in ValidatorIds: ReliableBeginLocalProposal(node)
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal)
  \/ \E node \in ValidatorIds, subject \in Subjects: StoreBody(node, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormPrepareQC(node, roundView, subject)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare: PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginLockCommit(node, qc)
  \/ \E request \in pendingLockCommit: PersistLockCommit(request)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormCommitQC(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginDecision(node, qc)
  \/ \E request \in pendingDecision: PersistDecision(request)
  \/ \E node \in ValidatorIds: ReliableBeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
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
  /\ \A request \in pendingInstallTC:
       /\ request.node \in ValidatorIds
       /\ request.kind = "InstallTC"
       /\ TcWellTyped(request.tc)
       /\ request.rebroadcast \in BOOLEAN
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
  \A left, right \in decisions:
    left.qc.context = right.qc.context => left.qc.subject = right.qc.subject

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

\* Static successor for the two quorum-only counterexample configurations.
QuorumCheckNext == UNCHANGED vars

GenesisDecisionExists ==
  \E decision \in decisions: decision.qc.context.height = 0

PostGstEventuallyGenesisDecision == gst ~> GenesisDecisionExists

=============================================================================
