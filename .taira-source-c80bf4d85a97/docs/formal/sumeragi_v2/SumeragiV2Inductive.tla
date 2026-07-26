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
  /\ \A vote \in timeoutIntents:
       (vote.signer \in Honest /\ vote.context = context)
         => vote.view <= nodeView[vote.signer]

CommitIntentsPreparedBy(commits, certificates) ==
  \A vote \in commits:
    vote.signer \in Honest
      => \E qc \in certificates:
           /\ qc.context = vote.context
           /\ qc.view = vote.view
           /\ qc.phase = "Prepare"
           /\ qc.subject = vote.subject

HonestCommitIntentPrepared ==
  CommitIntentsPreparedBy(commitIntents, prepareQCs)
  /\ \A vote \in commitIntents:
       (vote.signer \in Honest /\ vote.context = context)
         => vote.view <= nodeView[vote.signer]

DurableIntentsDoNotAnticipateHeight ==
  /\ \A vote \in prepareIntents:
       vote.context.height <= height
  /\ \A vote \in commitIntents:
       vote.context.height <= height
  /\ \A vote \in timeoutIntents:
       vote.context.height <= height

PendingVoteWritesAuthorized ==
  /\ \A request \in pendingPrepare:
       /\ request.node \in Honest
       /\ request.vote.phase = "Prepare"
       /\ request.vote.signer = request.node
       /\ request.vote.context = context
       /\ request.vote.view = nodeView[request.node]
       /\ request.vote.subject \in ValidSubjects
       /\ BodyHeldBy(durableBodies, request.node,
                     request.vote.context, request.vote.view,
                     request.vote.subject)
       /\ CanAppendVote(prepareIntents, request.vote)
       /\ PrepareCarriesHigherSafeQc(request.vote)
  /\ \A request \in pendingLockCommit:
       /\ request.node \in Honest
       \* Local WAL payloads retain the complete constructor identity.  The
       \* broad vote carrier also admits malformed wire records whose
       \* redundant `height` can disagree with `context.height`; those must
       \* never become durable LockCommit intents.
       /\ request.vote =
            Vote(context, request.qc.view, "Commit",
                 request.qc.subject, request.node)
       /\ request.vote.phase = "Commit"
       /\ request.vote.signer = request.node
       /\ request.vote.context = context
       /\ request.vote.context = request.qc.context
       /\ request.vote.view = request.qc.view
       /\ request.vote.subject = request.qc.subject
       /\ request.qc.phase = "Prepare"
       /\ request.qc \in prepareQCs
       /\ CurrentOpenPrepareForCommit(request.node, request.qc)
       /\ request.vote.subject \in ValidSubjects
       /\ BodyHeldBy(durableBodies, request.node,
                     request.vote.context, request.vote.view,
                     request.vote.subject)
       /\ request.qc.view >= lockRank[request.node]
       /\ (request.qc.view = lockRank[request.node]
             => request.qc.subject = lockSubject[request.node])
       /\ CanAppendVote(commitIntents, request.vote)
  /\ \A request \in pendingTimeout:
       /\ request.node \in Honest
       /\ request.vote.signer = request.node
       /\ request.vote.context = context
       /\ request.vote.view = nodeView[request.node]
       /\ CanAppendTimeout(timeoutIntents, request.vote)
       /\ TimeoutVoteProtectsCommitSet(request.vote, commitIntents)

IntentPhasesCorrect ==
  /\ \A vote \in prepareIntents: vote.phase = "Prepare"
  /\ \A vote \in commitIntents: vote.phase = "Commit"

CertificatePhasesCorrect ==
  /\ \A qc \in prepareQCs: qc.phase = "Prepare"
  /\ \A qc \in commitQCs: qc.phase = "Commit"

PendingCertificateWritesAuthorized ==
  /\ \A request \in pendingObservePrepare:
       /\ request.qc \in prepareQCs
       /\ request.qc.context = context
       /\ request.qc.view > highestRank[request.node]
  /\ \A request \in pendingInstallTC:
       /\ request.tc \in formedTCs
       /\ request.tc.context = context
       /\ TCValid(request.tc)
       /\ request.tc.votes # {}
       /\ request.tc.view + 1 \in Views
       /\ request.tc.view + 1 >= nodeView[request.node]
  /\ \A request \in pendingDecision:
       /\ request.qc \in commitQCs
       /\ request.qc.context = context
       /\ request.qc.phase = "Commit"
       /\ request.qc.height = height

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

ReceivedPrepareQcViewAdmissible ==
  \A received \in receivedQCs:
    received.qc.phase = "Prepare"
      => received.qc.view <= nodeView[received.node]

HonestTimeoutTransportBacked ==
  /\ \A envelope \in timeoutNetwork:
       envelope.vote.signer \in Honest
         => envelope.vote \in timeoutIntents
  /\ \A received \in receivedTimeoutVotes:
       received.vote.signer \in Honest
         => received.vote \in timeoutIntents

TcTransportBacked ==
  /\ \A envelope \in tcNetwork:
       /\ envelope.tc \in formedTCs
       /\ TCValid(envelope.tc)
  /\ \A received \in receivedTCs:
       /\ received.tc \in formedTCs
       /\ TCValid(received.tc)
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
    /\ IsFiniteSet(tc.votes)
    /\ tc.votes # {}
    /\ TimeoutVotesDisjoint(tc.votes)
    /\ TimeoutHighsConflictFree(tc.votes)
    /\ TimeoutVotesBindCertificate(tc)
    /\ DualQuorum(tc.context.epoch, TimeoutSignerSet(tc.votes))
    /\ \A vote \in tc.votes:
         /\ vote.signer \in VotingRoster(tc.context.epoch)
         /\ vote.highRank \in Ranks
         /\ vote.highRank <= tc.view
         /\ vote.signer \in Honest => vote \in timeoutIntents
    /\ TCMaximumProtectsReports(tc)

TimeoutCertificateSelectorsSound ==
  \A tc \in formedTCs:
    HighestTimeoutVote(tc.votes) \in tc.votes

DurableTimeoutsProtectCommits ==
  TimeoutIntentProtectsCommits(timeoutIntents, commitIntents)

(***************************************************************************
Every pending LockAndCommit is for the durable current round, and every
timeout/Commit pair is protected directly. Installed TC state cannot authorize
new historical Commit creation.
***************************************************************************)
SameRoundLockAndCommitAuthorizationInvariant ==
  /\ \A request \in pendingLockCommit:
       /\ request.vote.view = nodeView[request.node]
       /\ CurrentOpenPrepareForCommit(request.node, request.qc)
  /\ \A timeoutVote \in timeoutIntents, commitVote \in commitIntents:
       (/\ timeoutVote.signer \in Honest
        /\ commitVote.signer = timeoutVote.signer
        /\ commitVote.context = timeoutVote.context
        /\ commitVote.phase = "Commit"
        /\ commitVote.view <= timeoutVote.view)
         => TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)

\* Compatibility aliases retain parser stability for the older proof module;
\* both now denote the exact same-round invariant above.
HistoricalLockedCommitAuthorizationInvariant ==
  SameRoundLockAndCommitAuthorizationInvariant

HistoricalTcLockedCommitAuthorizationInvariant ==
  SameRoundLockAndCommitAuthorizationInvariant

(***************************************************************************
This authorization invariant is a derived consequence of
PendingVoteWritesAuthorized and DurableTimeoutsProtectCommits.  It is kept as
the named release obligation below, but is deliberately not duplicated as an
independent reducer conjunct: doing so would add the same proof obligation to
every action-preservation branch without strengthening the invariant.
***************************************************************************)

HighestAndLockAreCertified ==
  \A node \in ValidatorIds:
    /\ PrepareQcRank(highestPrepareQc[node]) = highestRank[node]
    /\ PrepareQcSubject(highestPrepareQc[node]) = highestSubject[node]
    /\ PrepareQcRank(lockPrepareQc[node]) = lockRank[node]
    /\ PrepareQcSubject(lockPrepareQc[node]) = lockSubject[node]
    /\ (highestPrepareQc[node] = NoPrepareQC
          <=> highestRank[node] = NoRank)
    /\ (highestPrepareQc[node] # NoPrepareQC
          => /\ highestPrepareQc[node] \in prepareQCs
             /\ highestPrepareQc[node].context = context
             /\ highestPrepareQc[node].phase = "Prepare")
    /\ (lockPrepareQc[node] = NoPrepareQC
          <=> lockRank[node] = NoRank)
    /\ (lockPrepareQc[node] # NoPrepareQC
          => /\ lockPrepareQc[node] \in prepareQCs
             /\ lockPrepareQc[node].context = context
             /\ lockPrepareQc[node].phase = "Prepare")

(***************************************************************************
Every non-empty durable lock has one of the two reducer origins which can
reconstruct it after a view change.  PersistLockCommit records the matching
local Commit intent atomically with the lock.  PersistInstallTC records the TC
which selected an advancing lock.  A later no-high TC may retain either source
without replacing it.  This reverse direction is intentionally separate from
LocksCoverOwnCommits (which states Commit => lock) and from
HighestAndLockAreCertified (which states only that a matching abstract QC
exists).
***************************************************************************)
DurableLockRecoveryProvenanceInvariant ==
  \A node \in ValidatorIds:
    \/ lockRank[node] = NoRank
    \/ ExactLockedCommitIntents(
         node, lockRank[node], lockSubject[node]) # {}
    \/ \E installed \in installedTCs:
         /\ installed.node = node
         /\ installed.tc.context = context
         /\ installed.tc.highestPrepareQc = lockPrepareQc[node]

ReducerProvenanceInvariant ==
  /\ HonestVoteUnique(prepareIntents)
  /\ HonestVoteUnique(commitIntents)
  /\ HonestTimeoutUnique(timeoutIntents)
  /\ IntentPhasesCorrect
  /\ PendingVoteWritesAuthorized
  /\ PendingCertificateWritesAuthorized
  /\ HonestVoteTransportBacked
  /\ QcTransportBacked
  /\ ReceivedPrepareQcViewAdmissible
  /\ HonestTimeoutTransportBacked
  /\ TcTransportBacked
  /\ CertificatesBackedByIntents
  /\ HonestDurableIntentsSound
  /\ FormedTimeoutCertificatesSound
  /\ DurableTimeoutsProtectCommits
  /\ HighestAndLockAreCertified
  /\ DurableLockRecoveryProvenanceInvariant

ReducerProvenanceWithoutVoteTransport ==
  /\ HonestVoteUnique(prepareIntents)
  /\ HonestVoteUnique(commitIntents)
  /\ HonestTimeoutUnique(timeoutIntents)
  /\ IntentPhasesCorrect
  /\ PendingVoteWritesAuthorized
  /\ PendingCertificateWritesAuthorized
  /\ QcTransportBacked
  /\ ReceivedPrepareQcViewAdmissible
  /\ HonestTimeoutTransportBacked
  /\ TcTransportBacked
  /\ CertificatesBackedByIntents
  /\ HonestDurableIntentsSound
  /\ FormedTimeoutCertificatesSound
  /\ DurableTimeoutsProtectCommits
  /\ HighestAndLockAreCertified
  /\ DurableLockRecoveryProvenanceInvariant

ReducerProvenanceWithoutTimeoutTransport ==
  /\ HonestVoteUnique(prepareIntents)
  /\ HonestVoteUnique(commitIntents)
  /\ HonestTimeoutUnique(timeoutIntents)
  /\ IntentPhasesCorrect
  /\ PendingVoteWritesAuthorized
  /\ PendingCertificateWritesAuthorized
  /\ HonestVoteTransportBacked
  /\ QcTransportBacked
  /\ ReceivedPrepareQcViewAdmissible
  /\ TcTransportBacked
  /\ CertificatesBackedByIntents
  /\ HonestDurableIntentsSound
  /\ FormedTimeoutCertificatesSound
  /\ DurableTimeoutsProtectCommits
  /\ HighestAndLockAreCertified
  /\ DurableLockRecoveryProvenanceInvariant

LineageInvariant ==
  /\ PrepareLineageSound
  /\ LocksCoverOwnCommits
  /\ CurrentIntentViewsBound
  /\ HonestCommitIntentPrepared
  /\ CertificatePhasesCorrect
  /\ DurableIntentsDoNotAnticipateHeight

StrongInductiveInvariant ==
  /\ Safety
  /\ ContextIdentityBindsFrozenEpoch
  /\ OldContextCertificateRejected
  /\ ContextParentWasApplied
  /\ ReducerProvenanceInvariant
  /\ LineageInvariant

ProofRelevantVars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    durableBodies, retainedLockedBodies,
    receivedVotes, receivedQCs, receivedTimeoutVotes,
    receivedTCs, proposalIntents, prepareIntents, commitIntents,
    timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
    lastInstalledTc, lockPrepareQc, highestPrepareQc,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork, decisions, applied>>

ProofRelevantWithoutDurableVars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    retainedLockedBodies, receivedVotes, receivedQCs,
    receivedTimeoutVotes, receivedTCs,
    proposalIntents, prepareIntents, commitIntents, timeoutIntents,
    prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingProposal, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork, decisions, applied>>

ProofRelevantWithoutPendingProposalVars ==
  <<height, context, contextHistory, nodeView, generation, up, gst,
    durableBodies, retainedLockedBodies,
    receivedVotes, receivedQCs, receivedTimeoutVotes,
    receivedTCs, proposalIntents, prepareIntents, commitIntents,
    timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingPrepare, pendingObservePrepare, pendingLockCommit,
    pendingTimeout, pendingInstallTC, pendingDecision,
    signProposals, signVotes, signTimeouts, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork, decisions, applied>>

LineageVars ==
  <<height, context, nodeView,
    prepareIntents, commitIntents, timeoutIntents,
    prepareQCs, commitQCs, lockRank, lockSubject>>

ProvenanceVars ==
  <<height, context, nodeView, durableBodies, retainedLockedBodies,
    receivedVotes, receivedQCs,
    receivedTimeoutVotes, receivedTCs, prepareIntents, commitIntents,
    timeoutIntents, prepareQCs, commitQCs, formedTCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject, pendingPrepare,
    pendingObservePrepare, pendingLockCommit, pendingTimeout,
    pendingInstallTC, pendingDecision, voteNetwork, qcNetwork,
    timeoutNetwork, tcNetwork>>

ProvenanceWithoutVoteTransportVars ==
  <<height, context, nodeView, durableBodies, retainedLockedBodies,
    receivedQCs,
    receivedTimeoutVotes,
    receivedTCs, prepareIntents, commitIntents, timeoutIntents, prepareQCs,
    commitQCs, formedTCs, installedTCs, lockRank, lockSubject, highestRank,
    highestSubject, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    qcNetwork, timeoutNetwork, tcNetwork>>

ProvenanceWithoutTimeoutTransportVars ==
  <<height, context, nodeView, durableBodies, retainedLockedBodies,
    receivedVotes, receivedQCs,
    receivedTCs, prepareIntents, commitIntents, timeoutIntents, prepareQCs,
    commitQCs, formedTCs, installedTCs, lockRank, lockSubject, highestRank,
    highestSubject, pendingPrepare, pendingObservePrepare,
    pendingLockCommit, pendingTimeout, pendingInstallTC, pendingDecision,
    voteNetwork, qcNetwork, tcNetwork>>

=============================================================================
