---- MODULE SumeragiV2Inductive ----
EXTENDS SumeragiV2Reconfiguration, SumeragiV2SafetyDefinitions

(***************************************************************************
Inductive strengthening for the executable reducer relation.

The small Safety predicate in SumeragiV2Core states the release properties,
but a transition proof also needs the provenance that the reducer carries
between asynchronous boundaries.  The predicates below record exactly those
facts: pending WAL writes retain their admission guards; honest network input
comes from an acknowledged intent; certificates are backed by such input;
and TC ingress comes only from a locally formed, valid certificate.  None of
these clauses assumes a certificate or vote that the production actions do
not construct.
***************************************************************************)

VoteIntentFor(vote) ==
  IF vote.phase = "Prepare" THEN vote \in prepareIntents
  ELSE IF vote.phase = "Commit" THEN vote \in commitIntents
  ELSE FALSE

PrepareCarriesHigherSafeQc(vote) ==
  \A commitVote \in commitIntents:
    (/\ vote.signer \in Honest
     /\ commitVote.signer = vote.signer
     /\ commitVote.context = vote.context
     /\ commitVote.phase = "Commit"
     /\ commitVote.view < vote.view
     /\ commitVote.subject # vote.subject)
    => \E qc \in prepareQCs:
         /\ qc.context = vote.context
         /\ qc.phase = "Prepare"
         /\ commitVote.view < qc.view
         /\ qc.view < vote.view
         /\ qc.subject = vote.subject

PrepareLineageSound ==
  \A vote \in prepareIntents:
    vote.signer \in Honest => PrepareCarriesHigherSafeQc(vote)

LocksCoverOwnCommits ==
  \A vote \in commitIntents:
    (vote.signer \in Honest /\ vote.context = context)
      => /\ lockRank[vote.signer] >= vote.view
         /\ (lockRank[vote.signer] = vote.view
               => lockSubject[vote.signer] = vote.subject)

CurrentIntentViewsBound ==
  /\ \A vote \in prepareIntents:
       (vote.signer \in Honest /\ vote.context = context)
         => vote.view <= nodeView[vote.signer]
  /\ \A vote \in commitIntents:
       (vote.signer \in Honest /\ vote.context = context)
         => vote.view <= nodeView[vote.signer]

PendingVoteWritesAuthorized ==
  /\ \A request \in pendingPrepare:
       /\ request.node \in Honest
       /\ request.vote.phase = "Prepare"
       /\ request.vote.signer = request.node
       /\ request.vote.context = context
       /\ request.vote.view = nodeView[request.node]
       /\ request.vote.subject \in ValidSubjects
       /\ BodyHeldBy(durableBodies, request.node,
                     request.vote.context, request.vote.subject)
       /\ CanAppendVote(prepareIntents, request.vote)
       /\ PrepareCarriesHigherSafeQc(request.vote)
  /\ \A request \in pendingLockCommit:
       /\ request.node \in Honest
       /\ request.vote.phase = "Commit"
       /\ request.vote.signer = request.node
       /\ request.vote.context = request.qc.context
       /\ request.vote.view = request.qc.view
       /\ request.vote.subject = request.qc.subject
       /\ request.qc.phase = "Prepare"
       /\ request.vote.view = nodeView[request.node]
       /\ request.vote.subject \in ValidSubjects
       /\ BodyHeldBy(durableBodies, request.node,
                     request.vote.context, request.vote.subject)
       /\ request.qc.view >= lockRank[request.node]
       /\ (request.qc.view = lockRank[request.node]
             => request.qc.subject = lockSubject[request.node])
       /\ CanAppendVote(commitIntents, request.vote)
  /\ \A request \in pendingTimeout:
       /\ request.node \in Honest
       /\ request.vote.signer = request.node
       /\ CanAppendTimeout(timeoutIntents, request.vote)
       /\ TimeoutVoteProtectsCommitSet(request.vote, commitIntents)

IntentPhasesCorrect ==
  /\ \A vote \in prepareIntents: vote.phase = "Prepare"
  /\ \A vote \in commitIntents: vote.phase = "Commit"

PendingCertificateWritesAuthorized ==
  /\ \A request \in pendingObservePrepare:
       /\ request.qc \in prepareQCs
       /\ request.qc.view > highestRank[request.node]
  /\ \A request \in pendingInstallTC:
       /\ request.tc \in formedTCs
       /\ request.tc.votes # {}
       /\ request.tc.view >= nodeView[request.node]
  /\ \A request \in pendingDecision:
       /\ request.qc \in commitQCs
       /\ request.qc.phase = "Commit"

HonestVoteTransportBacked ==
  /\ \A envelope \in voteNetwork:
       envelope.vote.signer \in Honest => VoteIntentFor(envelope.vote)
  /\ \A received \in receivedVotes:
       received.vote.signer \in Honest => VoteIntentFor(received.vote)

QcTransportBacked ==
  /\ \A envelope \in qcNetwork:
       envelope.qc \in prepareQCs \cup commitQCs
  /\ \A received \in receivedQCs:
       received.qc \in prepareQCs \cup commitQCs

HonestTimeoutTransportBacked ==
  /\ \A envelope \in timeoutNetwork:
       envelope.vote.signer \in Honest
         => envelope.vote \in timeoutIntents
  /\ \A received \in receivedTimeoutVotes:
       received.vote.signer \in Honest
         => received.vote \in timeoutIntents

TcTransportBacked ==
  /\ \A envelope \in tcNetwork: envelope.tc \in formedTCs
  /\ \A received \in receivedTCs: received.tc \in formedTCs
  /\ \A installed \in installedTCs: installed.tc \in formedTCs

HistoricalQcValid(qc) ==
  /\ qc.context \in ContextRecords
  /\ qc.height = qc.context.height
  /\ qc.context.epoch \in Epochs
  /\ qc.view \in Views
  /\ qc.phase \in Phases
  /\ qc.subject \in ValidSubjects
  /\ DualQuorum(qc.context.epoch, qc.signers)

CertificatesBackedByIntents ==
  /\ \A qc \in prepareQCs:
       /\ HistoricalQcValid(qc)
       /\ CertificateBackedBy(qc.context.epoch, qc, prepareIntents)
  /\ \A qc \in commitQCs:
       /\ HistoricalQcValid(qc)
       /\ CertificateBackedBy(qc.context.epoch, qc, commitIntents)

HonestDurableIntentsSound ==
  /\ HonestIntentSound(prepareIntents, durableBodies, ValidSubjects)
  /\ HonestIntentSound(commitIntents, durableBodies, ValidSubjects)

FormedTimeoutCertificatesSound ==
  \A tc \in formedTCs:
    /\ tc.context \in ContextRecords
    /\ tc.height = tc.context.height
    /\ tc.context.epoch \in Epochs
    /\ tc.view \in Views
    /\ tc.votes # {}
    /\ TimeoutVotesDisjoint(tc.votes)
    /\ TimeoutHighsConflictFree(tc.votes)
    /\ TimeoutVotesBindCertificate(tc)
    /\ DualQuorum(tc.context.epoch, TimeoutSignerSet(tc.votes))
    /\ \A vote \in tc.votes:
         /\ vote.signer \in VotingRoster(tc.context.epoch)
         /\ vote.highRank \in Ranks
         /\ vote.signer \in Honest => vote \in timeoutIntents
    /\ TCMaximumProtectsReports(tc)

DurableTimeoutsProtectCommits ==
  TimeoutIntentProtectsCommits(timeoutIntents, commitIntents)

HighestAndLockAreCertified ==
  \A node \in ValidatorIds:
    /\ (highestRank[node] = NoRank
          => highestSubject[node] = NoSubject)
    /\ (highestRank[node] # NoRank
          => \E qc \in prepareQCs:
               /\ qc.context = context
               /\ qc.view = highestRank[node]
               /\ qc.subject = highestSubject[node])
    /\ (lockRank[node] = NoRank => lockSubject[node] = NoSubject)
    /\ (lockRank[node] # NoRank
          => \E qc \in prepareQCs:
               /\ qc.context = context
               /\ qc.view = lockRank[node]
               /\ qc.subject = lockSubject[node])

ReducerProvenanceInvariant ==
  /\ HonestVoteUnique(prepareIntents)
  /\ HonestVoteUnique(commitIntents)
  /\ HonestTimeoutUnique(timeoutIntents)
  /\ IntentPhasesCorrect
  /\ PendingVoteWritesAuthorized
  /\ PendingCertificateWritesAuthorized
  /\ HonestVoteTransportBacked
  /\ QcTransportBacked
  /\ HonestTimeoutTransportBacked
  /\ TcTransportBacked
  /\ CertificatesBackedByIntents
  /\ HonestDurableIntentsSound
  /\ FormedTimeoutCertificatesSound
  /\ DurableTimeoutsProtectCommits
  /\ HighestAndLockAreCertified

LineageInvariant ==
  /\ PrepareLineageSound
  /\ LocksCoverOwnCommits
  /\ CurrentIntentViewsBound

StrongInductiveInvariant ==
  /\ Safety
  /\ ContextIdentityBindsFrozenEpoch
  /\ OldContextCertificateRejected
  /\ ContextParentWasApplied
  /\ ReducerProvenanceInvariant

ProofRelevantVars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    durableBodies, receivedVotes, receivedQCs, receivedTimeoutVotes,
    receivedTCs, proposalIntents, prepareIntents, commitIntents,
    timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork, decisions, applied>>

ProofRelevantWithoutDurableVars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    receivedVotes, receivedQCs, receivedTimeoutVotes, receivedTCs,
    proposalIntents, prepareIntents, commitIntents, timeoutIntents,
    prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork, decisions, applied>>

ProofRelevantWithoutPendingProposalVars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    durableBodies, receivedVotes, receivedQCs, receivedTimeoutVotes,
    receivedTCs, proposalIntents, prepareIntents, commitIntents,
    timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingPrepare, pendingObservePrepare, pendingLockCommit,
    pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork, decisions, applied>>

=============================================================================
