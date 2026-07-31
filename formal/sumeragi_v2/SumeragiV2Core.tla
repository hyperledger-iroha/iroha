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
Generations == IF ViewDomain = Nat THEN Nat ELSE 0..MaxGeneration
GenerationCanIncrement(value) ==
  ViewDomain = Nat \/ value < MaxGeneration
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

NoPrepareQC == [kind |-> "NoPrepareQC"]
NoTimeoutCertificate == [kind |-> "NoTimeoutCertificate"]

PrepareQcRank(highestPrepareQc) ==
  IF highestPrepareQc = NoPrepareQC
  THEN NoRank
  ELSE highestPrepareQc.view

PrepareQcSubject(highestPrepareQc) ==
  IF highestPrepareQc = NoPrepareQC
  THEN NoSubject
  ELSE highestPrepareQc.subject

Proposal(context, roundView, subject, proposer, timeoutCertificate,
         highestPrepareQc) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   subject |-> subject, proposer |-> proposer,
   timeoutCertificate |-> timeoutCertificate,
   highestPrepareQc |-> highestPrepareQc,
   justifyRank |-> PrepareQcRank(highestPrepareQc),
   justifySubject |->
     IF roundView = 0
     THEN context.parent
     ELSE PrepareQcSubject(highestPrepareQc)]

Vote(context, roundView, phase, subject, signer) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   phase |-> phase, subject |-> subject, signer |-> signer]

QC(context, roundView, phase, subject, signers) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   phase |-> phase, subject |-> subject, signers |-> signers]

TimeoutVote(context, roundView, signer, highestPrepareQc) ==
  [context |-> context, height |-> context.height, view |-> roundView,
   signer |-> signer, highestPrepareQc |-> highestPrepareQc,
   highRank |-> PrepareQcRank(highestPrepareQc),
   highSubject |-> PrepareQcSubject(highestPrepareQc)]

VoteRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   phase: Phases, subject: Subjects, signer: ValidatorIds]
QcRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   phase: Phases, subject: Subjects, signers: SUBSET ValidatorIds]

(***************************************************************************
Stable certificate identity and exact evidence shared by reducer recovery
owners.

Production `CertificateRef` consists of context, round height/view, phase, and
subject.  It remains useful for request deduplication, but it is not sufficient
for recovery ownership: a PrepareQC owner must retain the exact authenticated
QC value, including its signer evidence.
***************************************************************************)
CertificateRefOf(qc) ==
  [context |-> qc.context,
   height |-> qc.height,
   view |-> qc.view,
   phase |-> qc.phase,
   subject |-> qc.subject]

SameCertificateRef(left, right) ==
  CertificateRefOf(left) = CertificateRefOf(right)

SamePrepareRecoveryRef(left, right) ==
  /\ left \in QcRecordSet
  /\ right \in QcRecordSet
  /\ left.phase = "Prepare"
  /\ right.phase = "Prepare"
  /\ left = right

PrepareQcOptionSet == {NoPrepareQC} \cup QcRecordSet
TimeoutVoteRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   signer: ValidatorIds, highestPrepareQc: PrepareQcOptionSet,
   highRank: Ranks, highSubject: SubjectOrNone]
TcRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   votes: SUBSET TimeoutVoteRecordSet,
   highestPrepareQc: PrepareQcOptionSet]
TimeoutCertificateOptionSet == {NoTimeoutCertificate} \cup TcRecordSet
ProposalRecordSet ==
  [context: ContextRecords, height: Heights, view: Views,
   subject: Subjects, proposer: ValidatorIds,
   timeoutCertificate: TimeoutCertificateOptionSet,
   highestPrepareQc: PrepareQcOptionSet,
   justifyRank: Ranks, justifySubject: SubjectOrNone]

TcWellTyped(tc) ==
  /\ tc \in TcRecordSet
  /\ DOMAIN tc =
       {"context", "height", "view", "votes", "highestPrepareQc"}
  /\ tc.context \in ContextRecords
  /\ tc.height \in Heights
  /\ tc.view \in Views
  /\ tc.votes \subseteq TimeoutVoteRecordSet
  /\ tc.highestPrepareQc \in PrepareQcOptionSet

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
  lastInstalledTc,
  lockPrepareQc,
  highestPrepareQc,
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
    lastInstalledTc, lockPrepareQc, highestPrepareQc,
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
\* The one exception is the exact PrepareQC selected by an installed TC:
\* production persists that full QC as `durable.locked()`, while this Core
\* abstraction stores its rank/subject plus installed-TC provenance.  The
\* BeginLockCommit guard below narrows the global PrepareQC carrier back to
\* that exact abstract durable lock before authorizing historical Commit.
LockCommitQcValues == ReceivedQcValues \cup prepareQCs
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

\* An authenticated future PrepareQC is consumed as a reducer stutter.  It
\* cannot create local receipt/ownership until a separately authenticated TC
\* advances the recipient.  CommitQC remains view-unrestricted so decided
\* heights recover even when the recipient's local view lags.
QcDeliveryCreatesReceipt(node, qc) ==
  \/ qc.phase = "Commit"
  \/ /\ qc.phase = "Prepare"
     /\ qc.view <= nodeView[node]

PrepareQcOptionWireValid(highestPrepare) ==
  \/ highestPrepare = NoPrepareQC
  \/ /\ highestPrepare \in QcRecordSet
     /\ highestPrepare.phase = "Prepare"
     /\ QcWireValid(highestPrepare)

ExactPrepareQcMatchesRef(highestPrepare, highRank, highSubject) ==
  /\ PrepareQcOptionWireValid(highestPrepare)
  /\ PrepareQcRank(highestPrepare) = highRank
  /\ PrepareQcSubject(highestPrepare) = highSubject

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
  [highestPrepareQc |-> NoPrepareQC,
   highRank |-> NoRank, highSubject |-> NoSubject]

HighestTimeoutVote(votes) ==
  LET maxima == MaximalTimeoutVotes(votes)
  IN IF maxima = {}
     THEN EmptyTimeoutHigh
     ELSE CHOOSE candidate \in maxima: TRUE

TC(tcContext, roundView, votes) ==
  LET highest == HighestTimeoutVote(votes)
  IN [context |-> tcContext, height |-> tcContext.height, view |-> roundView,
      votes |-> votes, highestPrepareQc |-> highest.highestPrepareQc]

TCValid(tc) ==
  /\ tc \in TcRecordSet
  /\ DOMAIN tc =
       {"context", "height", "view", "votes", "highestPrepareQc"}
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
       /\ ExactPrepareQcMatchesRef(
            vote.highestPrepareQc, vote.highRank, vote.highSubject)
       /\ vote.highRank <= tc.view
  /\ TimeoutVotesDisjoint(tc.votes)
  /\ TimeoutHighsConflictFree(tc.votes)
  /\ DualQuorum(CurrentEpoch, TimeoutSignerSet(tc.votes))
  /\ tc.highestPrepareQc =
       (HighestTimeoutVote(tc.votes)).highestPrepareQc

TcHighRank(tc) == PrepareQcRank(tc.highestPrepareQc)
TcHighSubject(tc) == PrepareQcSubject(tc.highestPrepareQc)

(***************************************************************************
A second valid certificate for the timeout round which installed the current
view may expose a PrepareQC omitted by the first timeout quorum.  Production
admits that stale-by-one certificate only when its selected Prepare rank is
strictly above both durable Prepare frontiers.  The install rebinds the lock
and advances the asynchronous generation, but does not advance nodeView a
second time.
***************************************************************************)
StrictSameRoundTcUpgrade(node, tc) ==
  /\ tc.view + 1 = nodeView[node]
  /\ NodeInstalledTC(node, tc.view)
  /\ TcHighRank(tc) > highestRank[node]
  /\ TcHighRank(tc) > lockRank[node]
  /\ GenerationCanIncrement(generation[node])

InstalledTcAuthorizesCommitVote(commitVote) == FALSE

(***************************************************************************
An honest timeout unconditionally fences later Commit creation in that view.
Installed TCs may recover, fetch, and validate their exact selected PrepareQC,
but they do not authorize a new historical Commit.  This strict first-release
rule keeps the timeout-protection argument independent of later recovery
provenance and makes every durable Commit subject to the same timeout report.
***************************************************************************)
TimeoutVoteProtectsCommitSet(timeoutVote, commitSet) ==
  \A commitVote \in commitSet:
    (/\ timeoutVote.signer \in Honest
     /\ commitVote.signer = timeoutVote.signer
     /\ commitVote.context = timeoutVote.context
     /\ commitVote.phase = "Commit"
     /\ commitVote.view <= timeoutVote.view)
    => TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)

ResultingInstallLockRank(node, tc) ==
  IF TcHighRank(tc) > lockRank[node]
  THEN TcHighRank(tc)
  ELSE lockRank[node]

ResultingInstallLockSubject(node, tc) ==
  IF TcHighRank(tc) > lockRank[node]
  THEN TcHighSubject(tc)
  ELSE lockSubject[node]

ResultingInstallLockPrepareQc(node, tc) ==
  IF TcHighRank(tc) > lockRank[node]
  THEN tc.highestPrepareQc
  ELSE lockPrepareQc[node]

ResultingInstallHighestPrepareQc(node, tc) ==
  IF TcHighRank(tc) > highestRank[node]
  THEN tc.highestPrepareQc
  ELSE highestPrepareQc[node]

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
    /\ installed.tc.highestPrepareQc = qc

CurrentOpenPrepareForCommit(node, qc) ==
  /\ QcAt(node, qc) \in receivedQCs
  /\ qc.view = nodeView[node]
  /\ ~NodeTimedOut(node, qc.view)

NoHigherPrepareOriginKnown(node, qc) ==
  /\ ~\E vote \in prepareIntents:
       /\ vote.signer = node
       /\ vote.context = qc.context
       /\ vote.phase = "Prepare"
       /\ vote.view > qc.view
  /\ ~(highestRank[node] > qc.view)

NoHigherConflictingPrepareKnown(node, qc) ==
  /\ ~\E vote \in prepareIntents:
       /\ vote.signer = node
       /\ vote.context = qc.context
       /\ vote.phase = "Prepare"
       /\ vote.view > qc.view
       /\ vote.subject # qc.subject
  /\ ~(highestRank[node] > qc.view
        /\ highestSubject[node] # qc.subject)

HistoricalLockedPrepareRecoveryProvenance(node, qc) ==
  \/ InstalledTcSelectsPrepareFor(node, qc)
  \/ ExactLockedCommitIntents(node, qc.view, qc.subject) # {}

HistoricalLockedPrepareSource(node, qc) ==
  /\ qc \in prepareQCs
  /\ qc.context = context
  /\ qc.phase = "Prepare"
  /\ qc.view < nodeView[node]
  /\ lockPrepareQc[node] = qc
  /\ qc.view = lockRank[node]
  /\ qc.subject = lockSubject[node]
  /\ HistoricalLockedPrepareRecoveryProvenance(node, qc)
  /\ NoDecisionForNode(node)

HistoricalLockedPrepareForCommit(node, qc) == FALSE

\* Compatibility aliases for proof modules whose theorem names predate the
\* source-neutral recovery vocabulary. Recovery remains available, while the
\* fresh historical-Commit authorization predicate is unconditionally false.
HistoricalTcLockedPrepareSource(node, qc) ==
  HistoricalLockedPrepareSource(node, qc)

HistoricalTcLockedPrepareForCommit(node, qc) ==
  HistoricalLockedPrepareForCommit(node, qc)

\* Current proof modules use the source-neutral name; retain the older
\* historical spelling only as a compatibility alias.
LockedPrepareRecoverySource(node, qc) ==
  HistoricalLockedPrepareSource(node, qc)

(***************************************************************************
The certified-body wire protocol accepts either the local durable Commit
Decision or the current durable locked PrepareQC. A locked Prepare has one of
two durable origins: an installed TC selected it, or an earlier LockAndCommit
already recorded its exact local Commit intent before a later no-high TC
carried the lock forward. Both origins authorize Fetch/Validate; only an
already durable same-round Commit intent authorizes old-round retransmission.
Neither origin authorizes a fresh historical BeginLockCommit: recovery fetches
and validates the body for unchanged later reproposal, then terminates without
creating a late Commit.

The timeout certificate, durable lock, and recovery candidates retain the
same complete PrepareQC value.  Rank and subject remain scheduling/safety
projections only; they never reconstruct or substitute certificate identity.
***************************************************************************)

DecisionCertifiedBodyRecoveryAuthority(node, qc) ==
  /\ [node |-> node, qc |-> qc] \in decisions
  /\ qc.context = context
  /\ qc.phase = "Commit"

CertifiedBodyRecoveryAuthority(node, qc) ==
  \/ DecisionCertifiedBodyRecoveryAuthority(node, qc)
  \/ HistoricalLockedPrepareSource(node, qc)

(***************************************************************************
TC acknowledgement clears the installing node's volatile vote pool. If the
resulting lock still has the node's exact durable same-round Commit intent,
production may queue that intent for re-signing without changing its round. A
newly promoted lock without such an intent is recovered for later unchanged
reproposal and never enters a historical LockAndCommit path.
***************************************************************************)
ActiveLockedCommitSignRequestsAfterInstall(node, tc) ==
  {VoteSign(node, vote):
    vote \in ExactLockedCommitIntents(
      node, ResultingInstallLockRank(node, tc),
      ResultingInstallLockSubject(node, tc))}

ProposalJustified(node, proposal) ==
  \/ /\ proposal.view = 0
     /\ proposal.timeoutCertificate = NoTimeoutCertificate
     /\ proposal.highestPrepareQc = NoPrepareQC
     /\ proposal.justifyRank = NoRank
     /\ proposal.justifySubject = context.parent
  \/ /\ proposal.view > 0
     /\ lastInstalledTc[node] # NoTimeoutCertificate
     /\ [node |-> node, tc |-> lastInstalledTc[node]] \in installedTCs
     /\ lastInstalledTc[node].context = context
     /\ lastInstalledTc[node].view + 1 = proposal.view
     /\ TCValid(lastInstalledTc[node])
     /\ proposal.timeoutCertificate = lastInstalledTc[node]
     /\ proposal.highestPrepareQc =
          lastInstalledTc[node].highestPrepareQc
     /\ proposal.justifyRank =
          PrepareQcRank(proposal.highestPrepareQc)
     /\ proposal.justifySubject =
          PrepareQcSubject(proposal.highestPrepareQc)
     /\ proposal.justifyRank < proposal.view

SafeToPrepare(node, proposal) ==
  \/ lockRank[node] = NoRank
  \/ proposal.subject = lockSubject[node]
  \/ /\ proposal.highestPrepareQc # NoPrepareQC
     /\ proposal.highestPrepareQc.view > lockRank[node]
     /\ proposal.highestPrepareQc.subject = proposal.subject

\* Wire/local-state checks do not decide external validity from a subject hash.
ProposalWireValidFor(node, proposal) ==
  /\ proposal \in ProposalRecordSet
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
  /\ lockPrepareQc[node] # NoPrepareQC
  /\ lockPrepareQc[node].context = context
  /\ lockPrepareQc[node].phase = "Prepare"
  /\ lockPrepareQc[node].view = roundView
  /\ lockPrepareQc[node].subject = subject
  /\ lockRank[node] = roundView
  /\ lockSubject[node] = subject
  /\ lockPrepareQc[node] \in prepareQCs

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

TimeoutVotesIn(receipts, node, roundView) ==
  {received.vote:
    received \in {entry \in receipts:
      /\ entry.node = node
      /\ entry.vote.context = context
      /\ entry.vote.view = roundView}}

TimeoutVotesAt(node, roundView) ==
  TimeoutVotesIn(receivedTimeoutVotes, node, roundView)

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

TimeoutReceiptAdmitted(node, vote) ==
  /\ NoDecisionForNode(node)
  /\ vote.view = nodeView[node]
  /\ ~TimeoutVoteSlotOccupied(node, vote)

TimeoutReceiptsAfter(node, vote) ==
  IF TimeoutReceiptAdmitted(node, vote)
  THEN receivedTimeoutVotes \cup {TimeoutVoteAt(node, vote)}
  ELSE receivedTimeoutVotes

TimeoutCertificateAfterReceipt(node, vote) ==
  TC(context, vote.view,
     TimeoutVotesIn(TimeoutReceiptsAfter(node, vote), node, vote.view))

TimeoutInstallRequestAfterReceipt(node, vote) ==
  InstallTcWal(node, TimeoutCertificateAfterReceipt(node, vote), TRUE)

TimeoutReceiptFormsTC(node, vote) ==
  /\ TimeoutReceiptAdmitted(node, vote)
  /\ vote.view + 1 \in Views
  /\ TCValid(TimeoutCertificateAfterReceipt(node, vote))

TimeoutDeliveryGuard(envelope) ==
  /\ envelope \in TimeoutEnvelopeSet
  /\ envelope \in timeoutNetwork
  /\ envelope.recipient \in up
  /\ NodeIdle(envelope.recipient)
  /\ envelope.vote.context = context
  /\ envelope.vote.height = height
  /\ envelope.vote.signer \in CurrentVoters
  /\ ExactPrepareQcMatchesRef(
       envelope.vote.highestPrepareQc,
       envelope.vote.highRank, envelope.vote.highSubject)
  /\ envelope.vote.highRank <= envelope.vote.view

TimeoutReceiptSurvivesInstall(received, node, tc) ==
  \/ received.node # node
  \/ /\ StrictSameRoundTcUpgrade(node, tc)
     /\ received.vote.context = context
     /\ received.vote.height = height
     /\ received.vote.view = nodeView[node]

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
       /\ ExactPrepareQcMatchesRef(
            received.vote.highestPrepareQc,
            received.vote.highRank, received.vote.highSubject)
       /\ received.vote.highRank <= received.vote.view

ModelConfiguration ==
  /\ QuorumConfiguration
  /\ MaxHeight \in Nat
  /\ ViewDomain \subseteq Nat
  /\ 0 \in ViewDomain
  /\ \A roundView \in ViewDomain: 0..roundView \subseteq ViewDomain
  /\ MaxGeneration \in Nat
  \* Finite TLC instances use the same representable width for every view-
  \* local generation episode.  The temporal proof uses mathematical Nat for
  \* both domains; this is a type correspondence, not a liveness budget.
  /\ \/ ViewDomain = Nat
     \/ ViewDomain \subseteq 0..MaxGeneration
  /\ EpochLength \in Nat \ {0}
  /\ MaxEpoch >= ExpectedEpoch(MaxHeight)
  /\ Len(LeaderStarts) = MaxHeight + 1
  /\ Len(LaneHashes) = MaxHeight + 1
  /\ Len(DaHashes) = MaxHeight + 1
  /\ \A index \in 1..Len(LeaderStarts):
       LeaderStarts[index] \in 0..(N - 1)
  /\ ProtocolVersionValue = 4
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
  /\ lastInstalledTc =
       [node \in ValidatorIds |-> NoTimeoutCertificate]
  /\ lockPrepareQc = [node \in ValidatorIds |-> NoPrepareQC]
  /\ highestPrepareQc = [node \in ValidatorIds |-> NoPrepareQC]
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
                 commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

LocalProposalJustification(node) ==
  LET roundView == nodeView[node]
  IN IF roundView = 0
     THEN [timeoutCertificate |-> NoTimeoutCertificate,
           highestPrepareQc |-> NoPrepareQC]
     ELSE LET tc == lastInstalledTc[node]
          IN [timeoutCertificate |-> tc,
              highestPrepareQc |-> tc.highestPrepareQc]

LocalProposalFor(node, subject) ==
  LET justification == LocalProposalJustification(node)
  IN Proposal(context, nodeView[node], subject, node,
              justification.timeoutCertificate,
              justification.highestPrepareQc)

\* A local leader may choose fresh work only when its justification has no
\* Prepare high certificate.  Once the installed TC selects a high subject,
\* production promotes that certificate into the durable lock and the runner
\* rebinds the exact retained body.  Key this guard to the justification itself
\* so even a locally-unlocked abstraction cannot choose an unrelated subject.
LocalProposalReproposesJustifiedHigh(proposal) ==
  \/ proposal.justifyRank = NoRank
  \/ proposal.subject = proposal.justifySubject

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
     /\ LocalProposalReproposesJustifiedHigh(proposal)
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                 commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                 highestRank, highestSubject, pendingProposal, pendingPrepare,
                 pendingObservePrepare, pendingLockCommit, pendingTimeout,
                 pendingInstallTC, pendingDecision, signVotes, signTimeouts,
                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                 decisions, applied>>

ByzantineBroadcastProposal(signer, roundView, subject,
                           timeoutCertificate, highestPrepare) ==
  LET proposal == Proposal(context, roundView, subject, signer,
                           timeoutCertificate, highestPrepare)
  IN /\ signer \in Byzantine(CurrentEpoch) \cap up
     /\ signer = Leader(context, roundView)
     /\ roundView \in Views
     /\ subject \in Subjects
     /\ timeoutCertificate \in TimeoutCertificateOptionSet
     /\ highestPrepare \in PrepareQcOptionSet
     /\ proposalNetwork' = proposalNetwork \cup BroadcastProposals(proposal)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

(***************************************************************************
TC-installed locked-body recovery has certificate authority but need not have
a leader Proposal receipt.  Validation therefore mirrors Decision recovery:
it records only the exact current-generation validation marker.  The ordinary
ValidateBody causal successor subsequently attempts BeginLockCommit, whose
HistoricalLockedPrepareForCommit guard applies the higher-origin fence.
***************************************************************************)

ValidateLockedBody(node, qc) ==
  LET validation == ValidationRecord(node, context, qc.view,
                                      generation[node], qc.subject)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ HistoricalLockedPrepareSource(node, qc)
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                 commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                 commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
     /\ receivedQCs' =
          IF QcDeliveryCreatesReceipt(envelope.recipient, envelope.qc)
          THEN receivedQCs \cup {received}
          ELSE receivedQCs
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingLockCommit, pendingTimeout,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

PersistObservePrepare(request) ==
  /\ request \in pendingObservePrepare
  /\ highestPrepareQc' =
       [highestPrepareQc EXCEPT ![request.node] = request.qc]
  /\ highestRank' = [highestRank EXCEPT ![request.node] = request.qc.view]
  /\ highestSubject' =
       [highestSubject EXCEPT ![request.node] = request.qc.subject]
  /\ pendingObservePrepare' = pendingObservePrepare \ {request}
  /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                 up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lastInstalledTc,
                 lockPrepareQc, lockRank, lockSubject,
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
     /\ CurrentOpenPrepareForCommit(node, qc)
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
     /\ lockPrepareQc' =
          [lockPrepareQc EXCEPT ![request.node] = request.qc]
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
     /\ highestPrepareQc' =
          [highestPrepareQc EXCEPT ![request.node] =
             IF request.qc.view > highestRank[request.node]
             THEN request.qc ELSE @]
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
                    formedTCs, installedTCs, lastInstalledTc,
                    pendingProposal, pendingPrepare,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, signProposals, signVotes,
                    signTimeouts, proposalNetwork, voteNetwork, timeoutNetwork,
                 tcNetwork, applied>>

LocalTimeoutVoteFor(node) ==
  TimeoutVote(context, nodeView[node], node, highestPrepareQc[node])

TimeoutRequestFor(node) == TimeoutWal(node, LocalTimeoutVoteFor(node))

BeginTimeoutReady(node) ==
  LET roundView == nodeView[node]
      request == TimeoutRequestFor(node)
  IN /\ node \in Honest \cap up \cap CurrentVoters
     /\ NodeIdle(node)
     /\ NoDecisionForNode(node)
     /\ ~NodeTimedOut(node, roundView)
     /\ request \in TimeoutWalSet

BeginTimeout(node) ==
  LET request == TimeoutRequestFor(node)
  IN /\ BeginTimeoutReady(node)
     /\ pendingTimeout' = pendingTimeout \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingInstallTC, pendingDecision, signProposals,
                    signVotes, proposalNetwork, voteNetwork, qcNetwork,
                    timeoutNetwork, tcNetwork, decisions, applied>>

LocalTimeoutCompletionGuard(request) ==
  LET node == request.node
      vote == request.vote
  IN /\ request \in signTimeouts
     /\ node \in Honest \cap up \cap CurrentVoters
     /\ node \notin PendingNodes
     /\ NoDecisionForNode(node)
     /\ vote = LocalTimeoutVoteFor(node)
     /\ vote.context = context
     /\ vote.height = height
     /\ vote.view = nodeView[node]
     /\ vote.signer = node
     /\ vote.highestPrepareQc = highestPrepareQc[node]
     /\ vote.highRank = highestRank[node]
     /\ vote.highSubject = highestSubject[node]
     /\ vote \in timeoutIntents
     /\ ExactPrepareQcMatchesRef(
          vote.highestPrepareQc, vote.highRank, vote.highSubject)
     /\ vote.highRank <= vote.view

CompleteTimeoutSignature(request) ==
  LET node == request.node
      vote == request.vote
      nextReceipts == TimeoutReceiptsAfter(node, vote)
      tc == TimeoutCertificateAfterReceipt(node, vote)
      installRequest == TimeoutInstallRequestAfterReceipt(node, vote)
      formsTC == TimeoutReceiptFormsTC(node, vote)
  IN /\ LocalTimeoutCompletionGuard(request)
     /\ signTimeouts' = signTimeouts \ {request}
     /\ timeoutNetwork' = timeoutNetwork \cup BroadcastTimeouts(vote)
     /\ receivedTimeoutVotes' = nextReceipts
     /\ formedTCs' =
          IF formsTC
          THEN formedTCs \cup {tc}
          ELSE formedTCs
     /\ pendingInstallTC' =
          IF formsTC
          THEN pendingInstallTC \cup {installRequest}
          ELSE pendingInstallTC
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, validatedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs, receivedTCs,
                    proposalIntents, prepareIntents, commitIntents,
                    timeoutIntents, prepareQCs, commitQCs, installedTCs,
                    lastInstalledTc, lockPrepareQc, highestPrepareQc,
                    lockRank, lockSubject, highestRank, highestSubject,
                    pendingProposal, pendingPrepare, pendingObservePrepare,
                    pendingLockCommit, pendingTimeout, pendingDecision,
                    signProposals, signVotes, proposalNetwork, voteNetwork,
                    qcNetwork, tcNetwork, decisions, applied>>

ByzantineBroadcastTimeout(signer, roundView, highestPrepare) ==
  LET vote == TimeoutVote(context, roundView, signer, highestPrepare)
  IN /\ signer \in Byzantine(CurrentEpoch) \cap up
     /\ roundView \in Views
     /\ PrepareQcOptionWireValid(highestPrepare)
     /\ PrepareQcRank(highestPrepare) <= roundView
     /\ timeoutNetwork' = timeoutNetwork \cup BroadcastTimeouts(vote)
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
  LET node == envelope.recipient
      vote == envelope.vote
      nextReceipts == TimeoutReceiptsAfter(node, vote)
      tc == TimeoutCertificateAfterReceipt(node, vote)
      installRequest == TimeoutInstallRequestAfterReceipt(node, vote)
      formsTC == TimeoutReceiptFormsTC(node, vote)
  IN /\ TimeoutDeliveryGuard(envelope)
     /\ timeoutNetwork' = timeoutNetwork \ {envelope}
     /\ receivedTimeoutVotes' = nextReceipts
     /\ formedTCs' =
          IF formsTC
          THEN formedTCs \cup {tc}
          ELSE formedTCs
     /\ pendingInstallTC' =
          IF formsTC
          THEN pendingInstallTC \cup {installRequest}
          ELSE pendingInstallTC
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies,
                    retainedLockedBodies, validatedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs, receivedTCs,
                    proposalIntents, prepareIntents, commitIntents,
                    timeoutIntents, prepareQCs, commitQCs, installedTCs,
                    lastInstalledTc, lockPrepareQc, highestPrepareQc,
                    lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, tcNetwork, decisions, applied>>

\* Proof-layer compatibility tombstone for the retired standalone TC reducer
\* action.  Receipt turns form a TC atomically in CompleteTimeoutSignature or
\* DeliverTimeout; this predicate is always disabled and is not part of Next.
FormTC(node, roundView) == FALSE

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
                    formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, decisions, applied>>

BeginInstallTC(node, tc) ==
  LET request == InstallTcWal(node, tc, FALSE)
  IN /\ TcAt(node, tc) \in receivedTCs
     /\ tc.view + 1 \in Views
     /\ \/ tc.view >= nodeView[node]
        \/ StrictSameRoundTcUpgrade(node, tc)
     /\ NodeIdle(node)
     /\ NoDecisionForNode(node)
     /\ pendingInstallTC' = pendingInstallTC \cup {request}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                    invalidBodies, seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
      sameRoundUpgrade == StrictSameRoundTcUpgrade(node, tc)
  IN /\ request \in pendingInstallTC
     /\ \/ tc.view >= nodeView[node]
        \/ sameRoundUpgrade
     /\ (sameRoundUpgrade => GenerationCanIncrement(generation[node]))
     /\ nodeView' =
          [nodeView EXCEPT ![node] =
             IF sameRoundUpgrade THEN @ ELSE tc.view + 1]
     /\ generation' =
          [generation EXCEPT ![node] =
             IF sameRoundUpgrade THEN @ + 1 ELSE 0]
     /\ lastInstalledTc' = [lastInstalledTc EXCEPT ![node] = tc]
     /\ highestPrepareQc' =
          [highestPrepareQc EXCEPT ![node] =
             IF advancesHigh THEN tc.highestPrepareQc ELSE @]
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
     /\ lockPrepareQc' =
          [lockPrepareQc EXCEPT ![node] =
             IF advancesLock THEN tc.highestPrepareQc ELSE @]
     /\ prepareQCs' =
          IF tc.highestPrepareQc = NoPrepareQC
          THEN prepareQCs
          ELSE prepareQCs \cup {tc.highestPrepareQc}
     /\ installedTCs' = installedTCs \cup {installed}
     /\ pendingInstallTC' = pendingInstallTC \ {request}
     /\ receivedVotes' =
          {received \in receivedVotes: received.node # node}
     /\ receivedTimeoutVotes' =
          {received \in receivedTimeoutVotes:
            TimeoutReceiptSurvivesInstall(received, node, tc)}
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
                    receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents,
                    commitQCs, formedTCs, pendingProposal, pendingPrepare,
                    pendingObservePrepare, pendingLockCommit, pendingTimeout,
                    pendingDecision, signProposals, signTimeouts,
                    proposalNetwork, voteNetwork, qcNetwork, timeoutNetwork,
                    decisions, applied>>

InstallCertifiedBodyEffectReady(node, roundView, subject) ==
  LET body == BodyRecord(node, context, roundView, subject)
  IN /\ node \in ValidatorIds
     /\ roundView \in Views
     /\ subject \in Subjects
     /\ ~BodyHeldBy(durableBodies, node, context, roundView, subject)
     /\ body \in BodyRecordSet

InstallCertifiedBodyEffect(node, roundView, subject) ==
  LET body == BodyRecord(node, context, roundView, subject)
  IN /\ InstallCertifiedBodyEffectReady(node, roundView, subject)
     /\ availableBodies' = availableBodies \cup {body}
     /\ UNCHANGED <<height, context, contextHistory, nodeView, generation,
                    up, gst, durableBodies, retainedLockedBodies,
                    validatedBodies, invalidBodies,
                    seenProposals, receivedVotes, receivedQCs,
                    receivedTimeoutVotes, receivedTCs, proposalIntents,
                    prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signVotes, signTimeouts, proposalNetwork,
                    voteNetwork, qcNetwork, timeoutNetwork, tcNetwork,
                    decisions, applied>>

FetchCertifiedBody(node, qc) ==
  /\ CertifiedBodyRecoveryAuthority(node, qc)
  /\ InstallCertifiedBodyEffect(node, qc.view, qc.subject)

\* A serialized response command carries a frozen authenticated capability.
\* Mutable request/lock authority was checked when ingress created that token;
\* reducer acceptance only installs the exact materialized body identity.
AcceptCertifiedResponseCapability(node, roundView, subject) ==
  InstallCertifiedBodyEffect(node, roundView, subject)

ApplyDecision(node, qc) ==
  LET application == [node |-> node, qc |-> qc]
  \* Applying retires the node's historical-recovery clock ownership, so the
  \* durable Decision must be the exact current-context Commit authority.
  \* Command evidence remains causal provenance and is intentionally separate.
  IN /\ DecisionCertifiedBodyRecoveryAuthority(node, qc)
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                 installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject, highestRank,
                 highestSubject, proposalNetwork, voteNetwork, qcNetwork,
                 timeoutNetwork, tcNetwork, decisions, applied>>

Restart(node) ==
  /\ node \in ValidatorIds \ up
  /\ GenerationCanIncrement(generation[node])
  /\ up' = up \cup {node}
  \* Restart is a same-view process replacement.  Advancing the executor
  \* generation rejects delayed pre-crash callbacks even when their semantic
  \* identity is retained durably for exact replay.
  /\ generation' = [generation EXCEPT ![node] = @ + 1]
  /\ UNCHANGED <<height, context, contextHistory, nodeView, gst,
                 availableBodies, durableBodies, retainedLockedBodies, validatedBodies,
                 invalidBodies, seenProposals, receivedVotes, receivedQCs,
                 receivedTimeoutVotes, receivedTCs, proposalIntents,
                 prepareIntents, commitIntents, timeoutIntents, prepareQCs,
                 commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
                    highestRank, highestSubject, pendingProposal,
                    pendingPrepare, pendingObservePrepare, pendingLockCommit,
                    pendingTimeout, pendingInstallTC, pendingDecision,
                    signProposals, signTimeouts, proposalNetwork, voteNetwork,
                    qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>

ResumeTimeout(node, vote) ==
  LET request == TimeoutSign(node, vote)
  IN /\ node \in up \cap Honest
     /\ NodeIdle(node)
     \* A durable Decision is terminal for timeout signing.  Production
     \* recovery emits the decided-body frontier instead of replaying any
     \* older timeout intent, so standalone Core replay keeps the same guard.
     /\ NoDecisionForNode(node)
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
                    commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
                 commitQCs, formedTCs, installedTCs, lastInstalledTc, lockPrepareQc, highestPrepareQc, lockRank, lockSubject,
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
       subject \in Subjects,
       timeoutCertificate \in TimeoutCertificateOptionSet,
       highestPrepare \in PrepareQcOptionSet:
       ByzantineBroadcastProposal(signer, roundView, subject,
                                  timeoutCertificate, highestPrepare)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal) \/ RebindRetainedBody(node, proposal)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       StoreBody(node, roundView, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal) \/ RejectBody(node, proposal)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ValidateDecidedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in prepareQCs:
       ValidateLockedBody(node, qc)
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
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highestPrepare \in PrepareQcOptionSet:
       ByzantineBroadcastTimeout(signer, roundView, highestPrepare)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)
  \/ \E request \in pendingInstallTC: PersistInstallTC(request)
  \/ \E node \in ValidatorIds,
       qc \in DecisionQcValues \cup prepareQCs:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects:
       AcceptCertifiedResponseCapability(node, roundView, subject)
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
  /\ lastInstalledTc
       \in [ValidatorIds -> TimeoutCertificateOptionSet]
  /\ lockPrepareQc \in [ValidatorIds -> PrepareQcOptionSet]
  /\ highestPrepareQc \in [ValidatorIds -> PrepareQcOptionSet]
  /\ lockRank \in [ValidatorIds -> Ranks]
  /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
  /\ highestRank \in [ValidatorIds -> Ranks]
  /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
  /\ \A node \in ValidatorIds:
       /\ PrepareQcRank(lockPrepareQc[node]) = lockRank[node]
       /\ PrepareQcSubject(lockPrepareQc[node]) = lockSubject[node]
       /\ PrepareQcRank(highestPrepareQc[node]) = highestRank[node]
       /\ PrepareQcSubject(highestPrepareQc[node]) = highestSubject[node]
       /\ (lockPrepareQc[node] # NoPrepareQC
             => lockPrepareQc[node] \in prepareQCs)
       /\ (highestPrepareQc[node] # NoPrepareQC
             => highestPrepareQc[node] \in prepareQCs)
       /\ (lastInstalledTc[node] # NoTimeoutCertificate
             => [node |-> node, tc |-> lastInstalledTc[node]]
                  \in installedTCs)
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
      => /\ right.highestPrepareQc = left.highestPrepareQc
         /\ right.highRank = left.highRank
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
