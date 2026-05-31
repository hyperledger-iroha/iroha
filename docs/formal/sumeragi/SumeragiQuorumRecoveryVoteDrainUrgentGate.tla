---- MODULE SumeragiQuorumRecoveryVoteDrainUrgentGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `quorum_recovery_vote_drain_urgent()`.

The helper raises vote-worker urgency only when quorum timeout is non-zero and
some pending block is non-aborted, extends the committed tip, has either vote/QC
evidence or waiting vote work, and has reached the quorum timeout. Evidence
uses pending progress age; no-evidence queue backlog uses inserted-at age.
Scanning is existential: one later urgent pending block is sufficient even when
an earlier pending block is not urgent.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoPending == "no_pending"
ZeroQuorumLocalVoteAtTimeout == "zero_quorum_local_vote_at_timeout"
AbortedLocalVoteAtTimeout == "aborted_local_vote_at_timeout"
OffTipLocalVoteAtTimeout == "off_tip_local_vote_at_timeout"
LocalVoteAtTimeout == "local_vote_at_timeout"
CommitQcAtTimeout == "commit_qc_at_timeout"
StoredVotesAtTimeout == "stored_votes_at_timeout"
CachedQcAtTimeout == "cached_qc_at_timeout"
EvidenceUnderTimeout == "evidence_under_timeout"
EvidencePastTimeout == "evidence_past_timeout"
EvidenceProgressAtTimeoutInsertedUnder == "evidence_progress_at_timeout_inserted_under"
NoEvidenceNoVotesWaitingPastInserted ==
  "no_evidence_no_votes_waiting_past_inserted"
NoEvidenceVotesWaitingUnderTimeout == "no_evidence_votes_waiting_under_timeout"
NoEvidenceVotesWaitingAtTimeout == "no_evidence_votes_waiting_at_timeout"
NoEvidenceVotesWaitingPastTimeout == "no_evidence_votes_waiting_past_timeout"
NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder ==
  "no_evidence_votes_waiting_inserted_at_timeout_progress_under"
FirstNonUrgentSecondUrgent == "first_nonurgent_second_urgent"
AllNonUrgent == "all_nonurgent"

Cases == {
  NoPending,
  ZeroQuorumLocalVoteAtTimeout,
  AbortedLocalVoteAtTimeout,
  OffTipLocalVoteAtTimeout,
  LocalVoteAtTimeout,
  CommitQcAtTimeout,
  StoredVotesAtTimeout,
  CachedQcAtTimeout,
  EvidenceUnderTimeout,
  EvidencePastTimeout,
  EvidenceProgressAtTimeoutInsertedUnder,
  NoEvidenceNoVotesWaitingPastInserted,
  NoEvidenceVotesWaitingUnderTimeout,
  NoEvidenceVotesWaitingAtTimeout,
  NoEvidenceVotesWaitingPastTimeout,
  NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder,
  FirstNonUrgentSecondUrgent,
  AllNonUrgent
}

PrimaryPendingCases == Cases \ {NoPending}

QuorumNonzeroCases == Cases \ {ZeroQuorumLocalVoteAtTimeout}

AbortedCases == {AbortedLocalVoteAtTimeout}

TipExtendingCases == PrimaryPendingCases \ {OffTipLocalVoteAtTimeout}

LocalVoteCases == {
  ZeroQuorumLocalVoteAtTimeout,
  AbortedLocalVoteAtTimeout,
  OffTipLocalVoteAtTimeout,
  LocalVoteAtTimeout
}

CommitQcCases == {CommitQcAtTimeout}

StoredVotesCases == {StoredVotesAtTimeout}

CachedQcCases == {CachedQcAtTimeout}

GenericEvidenceCases == {
  EvidenceUnderTimeout,
  EvidencePastTimeout,
  EvidenceProgressAtTimeoutInsertedUnder
}

VoteEvidenceCases ==
  LocalVoteCases \cup CommitQcCases \cup StoredVotesCases
    \cup CachedQcCases \cup GenericEvidenceCases

VotesWaitingCases == {
  NoEvidenceVotesWaitingUnderTimeout,
  NoEvidenceVotesWaitingAtTimeout,
  NoEvidenceVotesWaitingPastTimeout,
  NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder
}

ProgressAgeAtLeastCases ==
  {LocalVoteAtTimeout, CommitQcAtTimeout, StoredVotesAtTimeout,
   CachedQcAtTimeout, EvidencePastTimeout,
   EvidenceProgressAtTimeoutInsertedUnder}

InsertedAgeAtLeastCases == {
  NoEvidenceNoVotesWaitingPastInserted,
  NoEvidenceVotesWaitingAtTimeout,
  NoEvidenceVotesWaitingPastTimeout,
  NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder
}

PrimaryUrgent(c) ==
  /\ c \in PrimaryPendingCases
  /\ c \in QuorumNonzeroCases
  /\ c \notin AbortedCases
  /\ c \in TipExtendingCases
  /\ ((c \in VoteEvidenceCases /\ c \in ProgressAgeAtLeastCases)
      \/ (c \notin VoteEvidenceCases
          /\ c \in VotesWaitingCases
          /\ c \in InsertedAgeAtLeastCases))

SpecUrgent(c) ==
  PrimaryUrgent(c) \/ c = FirstNonUrgentSecondUrgent

ReturnUrgent == 1
ReturnNotUrgent == 2
CheckQuorumTimeout == 3
RejectZeroQuorum == 4
CheckPrimaryPending == 5
RejectNoPending == 6
CheckAborted == 7
RejectAborted == 8
CheckTipExtension == 9
RejectOffTip == 10
CheckVoteEvidence == 11
EvidencePath == 12
CheckVotesWaiting == 13
RejectNoEvidenceNoVotesWaiting == 14
UseProgressAge == 15
UseInsertedAge == 16
CheckAge == 17
RejectUnderTimeout == 18
AcceptAgeDue == 19
ScanSecondPending == 20
SecondPendingUrgent == 21

ActionUniverse == 1..21

AgeActions(c) ==
  {CheckAge}
    \cup (IF PrimaryUrgent(c) THEN {AcceptAgeDue} ELSE {RejectUnderTimeout})

PrimaryDecisionActions(c) ==
  {CheckPrimaryPending}
    \cup (IF c = NoPending THEN {RejectNoPending} ELSE {})
    \cup (IF c \in PrimaryPendingCases THEN {CheckAborted} ELSE {})
    \cup (IF c \in AbortedCases THEN {RejectAborted} ELSE {})
    \cup (IF c \in PrimaryPendingCases /\ c \notin AbortedCases
          THEN {CheckTipExtension}
          ELSE {})
    \cup (IF c = OffTipLocalVoteAtTimeout THEN {RejectOffTip} ELSE {})
    \cup (IF c \in PrimaryPendingCases /\ c \notin AbortedCases
              /\ c \in TipExtendingCases
          THEN {CheckVoteEvidence}
          ELSE {})
    \cup (IF c \in VoteEvidenceCases /\ c \notin AbortedCases
              /\ c \in TipExtendingCases
          THEN {EvidencePath} \cup {UseProgressAge} \cup AgeActions(c)
          ELSE {})
    \cup (IF c \in PrimaryPendingCases /\ c \notin VoteEvidenceCases
              /\ c \notin AbortedCases /\ c \in TipExtendingCases
          THEN {CheckVotesWaiting}
          ELSE {})
    \cup (IF c \in {NoEvidenceNoVotesWaitingPastInserted, AllNonUrgent}
          THEN {RejectNoEvidenceNoVotesWaiting}
          ELSE {})
    \cup (IF c \in VotesWaitingCases
          THEN {UseInsertedAge} \cup AgeActions(c)
          ELSE {})
    \cup (IF c = FirstNonUrgentSecondUrgent
          THEN {RejectNoEvidenceNoVotesWaiting, ScanSecondPending,
                SecondPendingUrgent}
          ELSE {})

SpecActions(c) ==
  {CheckQuorumTimeout}
    \cup (IF SpecUrgent(c) THEN {ReturnUrgent} ELSE {ReturnNotUrgent})
    \cup (IF c \notin QuorumNonzeroCases
          THEN {RejectZeroQuorum}
          ELSE PrimaryDecisionActions(c))

ImplementationUrgent(c) ==
  CASE Bug = "accept_zero_quorum"
       /\ c = ZeroQuorumLocalVoteAtTimeout ->
      TRUE
    [] Bug = "accept_aborted"
       /\ c = AbortedLocalVoteAtTimeout ->
      TRUE
    [] Bug = "accept_off_tip"
       /\ c = OffTipLocalVoteAtTimeout ->
      TRUE
    [] Bug = "reject_local_vote"
       /\ c = LocalVoteAtTimeout ->
      FALSE
    [] Bug = "reject_commit_qc"
       /\ c = CommitQcAtTimeout ->
      FALSE
    [] Bug = "reject_stored_votes"
       /\ c = StoredVotesAtTimeout ->
      FALSE
    [] Bug = "reject_cached_qc"
       /\ c = CachedQcAtTimeout ->
      FALSE
    [] Bug = "require_queue_for_evidence"
       /\ c = LocalVoteAtTimeout ->
      FALSE
    [] Bug = "accept_no_evidence_without_waiting"
       /\ c = NoEvidenceNoVotesWaitingPastInserted ->
      TRUE
    [] Bug = "reject_no_evidence_with_waiting"
       /\ c = NoEvidenceVotesWaitingAtTimeout ->
      FALSE
    [] Bug = "use_progress_age_for_no_evidence_queue"
       /\ c = NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder ->
      FALSE
    [] Bug = "use_inserted_age_for_evidence"
       /\ c = EvidenceProgressAtTimeoutInsertedUnder ->
      FALSE
    [] Bug = "reject_timeout_boundary"
       /\ c \in {LocalVoteAtTimeout, NoEvidenceVotesWaitingAtTimeout} ->
      FALSE
    [] Bug = "accept_under_timeout"
       /\ c \in {EvidenceUnderTimeout, NoEvidenceVotesWaitingUnderTimeout} ->
      TRUE
    [] Bug = "stop_after_first_pending"
       /\ c = FirstNonUrgentSecondUrgent ->
      FALSE
    [] Bug = "accept_all_nonurgent"
       /\ c = AllNonUrgent ->
      TRUE
    [] Bug = "accept_no_pending"
       /\ c = NoPending ->
      TRUE
    [] OTHER -> SpecUrgent(c)

ImplementationActions(c) ==
  (SpecActions(c) \ {ReturnUrgent, ReturnNotUrgent})
    \cup (IF ImplementationUrgent(c)
          THEN {ReturnUrgent}
          ELSE {ReturnNotUrgent})

Bugs == {
  "none",
  "accept_zero_quorum",
  "accept_aborted",
  "accept_off_tip",
  "reject_local_vote",
  "reject_commit_qc",
  "reject_stored_votes",
  "reject_cached_qc",
  "require_queue_for_evidence",
  "accept_no_evidence_without_waiting",
  "reject_no_evidence_with_waiting",
  "use_progress_age_for_no_evidence_queue",
  "use_inserted_age_for_evidence",
  "reject_timeout_boundary",
  "accept_under_timeout",
  "stop_after_first_pending",
  "accept_all_nonurgent",
  "accept_no_pending"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecUrgent(c) \in BOOLEAN
       /\ ImplementationUrgent(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationUrgent(c) = SpecUrgent(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

QuorumTimeoutRequired ==
  /\ ImplementationUrgent(ZeroQuorumLocalVoteAtTimeout) = FALSE
  /\ RejectZeroQuorum
       \in ImplementationActions(ZeroQuorumLocalVoteAtTimeout)
  /\ ~(CheckPrimaryPending
       \in ImplementationActions(ZeroQuorumLocalVoteAtTimeout))

PendingMustBeLiveAndTipExtending ==
  /\ ImplementationUrgent(NoPending) = FALSE
  /\ RejectNoPending \in ImplementationActions(NoPending)
  /\ ImplementationUrgent(AbortedLocalVoteAtTimeout) = FALSE
  /\ RejectAborted \in ImplementationActions(AbortedLocalVoteAtTimeout)
  /\ ~(CheckTipExtension
       \in ImplementationActions(AbortedLocalVoteAtTimeout))
  /\ ImplementationUrgent(OffTipLocalVoteAtTimeout) = FALSE
  /\ RejectOffTip \in ImplementationActions(OffTipLocalVoteAtTimeout)
  /\ ~(CheckVoteEvidence
       \in ImplementationActions(OffTipLocalVoteAtTimeout))

AnyVoteEvidenceCanDriveUrgency ==
  /\ ImplementationUrgent(LocalVoteAtTimeout) = TRUE
  /\ ImplementationUrgent(CommitQcAtTimeout) = TRUE
  /\ ImplementationUrgent(StoredVotesAtTimeout) = TRUE
  /\ ImplementationUrgent(CachedQcAtTimeout) = TRUE
  /\ EvidencePath \in ImplementationActions(LocalVoteAtTimeout)
  /\ EvidencePath \in ImplementationActions(CommitQcAtTimeout)
  /\ EvidencePath \in ImplementationActions(StoredVotesAtTimeout)
  /\ EvidencePath \in ImplementationActions(CachedQcAtTimeout)
  /\ ~(CheckVotesWaiting \in ImplementationActions(LocalVoteAtTimeout))

NoEvidenceRequiresWaitingVotes ==
  /\ ImplementationUrgent(NoEvidenceNoVotesWaitingPastInserted) = FALSE
  /\ RejectNoEvidenceNoVotesWaiting
       \in ImplementationActions(NoEvidenceNoVotesWaitingPastInserted)
  /\ ImplementationUrgent(NoEvidenceVotesWaitingAtTimeout) = TRUE
  /\ CheckVotesWaiting
       \in ImplementationActions(NoEvidenceVotesWaitingAtTimeout)
  /\ UseInsertedAge
       \in ImplementationActions(NoEvidenceVotesWaitingAtTimeout)

AgeSourceAndBoundaryMatchRust ==
  /\ ImplementationUrgent(EvidenceUnderTimeout) = FALSE
  /\ RejectUnderTimeout \in ImplementationActions(EvidenceUnderTimeout)
  /\ ImplementationUrgent(EvidencePastTimeout) = TRUE
  /\ ImplementationUrgent(EvidenceProgressAtTimeoutInsertedUnder) = TRUE
  /\ UseProgressAge
       \in ImplementationActions(EvidenceProgressAtTimeoutInsertedUnder)
  /\ ImplementationUrgent(NoEvidenceVotesWaitingUnderTimeout) = FALSE
  /\ ImplementationUrgent(NoEvidenceVotesWaitingAtTimeout) = TRUE
  /\ ImplementationUrgent(NoEvidenceVotesWaitingPastTimeout) = TRUE
  /\ ImplementationUrgent(
       NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder) = TRUE
  /\ UseInsertedAge
       \in ImplementationActions(
            NoEvidenceVotesWaitingInsertedAtTimeoutProgressUnder)

PendingScanIsExistential ==
  /\ ImplementationUrgent(FirstNonUrgentSecondUrgent) = TRUE
  /\ ScanSecondPending \in ImplementationActions(FirstNonUrgentSecondUrgent)
  /\ SecondPendingUrgent \in ImplementationActions(FirstNonUrgentSecondUrgent)
  /\ ImplementationUrgent(AllNonUrgent) = FALSE

NoBugInvariant ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ QuorumTimeoutRequired
  /\ PendingMustBeLiveAndTipExtending
  /\ AnyVoteEvidenceCanDriveUrgency
  /\ NoEvidenceRequiresWaitingVotes
  /\ AgeSourceAndBoundaryMatchRust
  /\ PendingScanIsExistential

SafetyFast == NoBugInvariant

====
