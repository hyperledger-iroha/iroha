---- MODULE SumeragiPendingFastUnblockGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `pending_fast_unblock_due(...)`.

The helper permits the fast-unblock path only when the configured fast timeout
is non-zero, no local commit vote or observed commit QC is present, no stored
votes or cached QC are available for the pending block, and the pending
progress age is at least the timeout. Every evidence source short-circuits the
decision before later lookups or the age comparison.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DuePastTimeout == "due_past_timeout"
DueAtTimeoutBoundary == "due_at_timeout_boundary"
UnderTimeout == "under_timeout"
ZeroTimeoutPastAge == "zero_timeout_past_age"
LocalCommitVotePastAge == "local_commit_vote_past_age"
CommitQcObservedPastAge == "commit_qc_observed_past_age"
StoredVotesPastAge == "stored_votes_past_age"
CachedQcPastAge == "cached_qc_past_age"
MultipleEvidencePastAge == "multiple_evidence_past_age"

Cases == {
  DuePastTimeout,
  DueAtTimeoutBoundary,
  UnderTimeout,
  ZeroTimeoutPastAge,
  LocalCommitVotePastAge,
  CommitQcObservedPastAge,
  StoredVotesPastAge,
  CachedQcPastAge,
  MultipleEvidencePastAge
}

TimeoutNonzeroCases == Cases \ {ZeroTimeoutPastAge}

AgeAtLeastCases == {
  DuePastTimeout,
  DueAtTimeoutBoundary,
  ZeroTimeoutPastAge,
  LocalCommitVotePastAge,
  CommitQcObservedPastAge,
  StoredVotesPastAge,
  CachedQcPastAge,
  MultipleEvidencePastAge
}

AgePastTimeoutCases == AgeAtLeastCases \ {DueAtTimeoutBoundary}

LocalCommitVoteCases == {
  LocalCommitVotePastAge,
  MultipleEvidencePastAge
}

CommitQcObservedCases == {
  CommitQcObservedPastAge
}

LocalEvidenceCases == LocalCommitVoteCases \cup CommitQcObservedCases

StoredVotesCases == {
  StoredVotesPastAge,
  MultipleEvidencePastAge
}

CachedQcCases == {
  CachedQcPastAge,
  MultipleEvidencePastAge
}

NoEvidenceBeforeAgeCases ==
  Cases \ (LocalEvidenceCases \cup StoredVotesCases \cup CachedQcCases)

SpecResult(c) ==
  c \in TimeoutNonzeroCases
    /\ c \notin LocalEvidenceCases
    /\ c \notin StoredVotesCases
    /\ c \notin CachedQcCases
    /\ c \in AgeAtLeastCases

ReturnDue == 1
ReturnNotDue == 2
CheckTimeout == 3
RejectZeroTimeout == 4
CheckLocalEvidence == 5
RejectLocalCommitVote == 6
RejectCommitQcObserved == 7
CheckStoredVotes == 8
RejectStoredVotes == 9
CheckCachedQc == 10
RejectCachedQc == 11
CheckAge == 12
AcceptPastTimeout == 13
AcceptBoundary == 14
RejectUnderTimeout == 15

ActionUniverse == 1..15

NoLocalEvidence(c) ==
  c \notin LocalEvidenceCases

NoStoredVotes(c) ==
  c \notin StoredVotesCases

NoCachedQc(c) ==
  c \notin CachedQcCases

AgeDecisionActions(c) ==
  {CheckAge}
    \cup (IF c = DueAtTimeoutBoundary THEN {AcceptBoundary} ELSE {})
    \cup (IF c \in AgePastTimeoutCases
          THEN {AcceptPastTimeout}
          ELSE {})
    \cup (IF c \notin AgeAtLeastCases
          THEN {RejectUnderTimeout}
          ELSE {})

SpecActions(c) ==
  {CheckTimeout}
    \cup (IF SpecResult(c) THEN {ReturnDue} ELSE {ReturnNotDue})
    \cup (IF c \notin TimeoutNonzeroCases
          THEN {RejectZeroTimeout}
          ELSE {CheckLocalEvidence})
    \cup (IF c \in TimeoutNonzeroCases /\ c \in LocalCommitVoteCases
          THEN {RejectLocalCommitVote}
          ELSE {})
    \cup (IF c \in TimeoutNonzeroCases
              /\ c \notin LocalCommitVoteCases
              /\ c \in CommitQcObservedCases
          THEN {RejectCommitQcObserved}
          ELSE {})
    \cup (IF c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
          THEN {CheckStoredVotes}
          ELSE {})
    \cup (IF c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
              /\ c \in StoredVotesCases
          THEN {RejectStoredVotes}
          ELSE {})
    \cup (IF c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
              /\ NoStoredVotes(c)
          THEN {CheckCachedQc}
          ELSE {})
    \cup (IF c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
              /\ NoStoredVotes(c) /\ c \in CachedQcCases
          THEN {RejectCachedQc}
          ELSE {})
    \cup (IF c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
              /\ NoStoredVotes(c) /\ NoCachedQc(c)
          THEN AgeDecisionActions(c)
          ELSE {})

ImplementationResult(c) ==
  CASE Bug = "reject_due_age"
       /\ c = DuePastTimeout ->
      FALSE
    [] Bug = "reject_boundary_age"
       /\ c = DueAtTimeoutBoundary ->
      FALSE
    [] Bug = "accept_zero_timeout"
       /\ c = ZeroTimeoutPastAge ->
      TRUE
    [] Bug = "accept_local_commit_vote"
       /\ c = LocalCommitVotePastAge ->
      TRUE
    [] Bug = "accept_commit_qc_observed"
       /\ c = CommitQcObservedPastAge ->
      TRUE
    [] Bug = "accept_stored_votes"
       /\ c = StoredVotesPastAge ->
      TRUE
    [] Bug = "accept_cached_qc"
       /\ c = CachedQcPastAge ->
      TRUE
    [] Bug = "accept_under_timeout"
       /\ c = UnderTimeout ->
      TRUE
    [] Bug = "skip_local_evidence_guard"
       /\ c \in LocalEvidenceCases ->
      TRUE
    [] Bug = "skip_stored_votes_guard"
       /\ c = StoredVotesPastAge ->
      TRUE
    [] Bug = "skip_cached_qc_guard"
       /\ c = CachedQcPastAge ->
      TRUE
    [] Bug = "accept_any_evidence"
       /\ c = MultipleEvidencePastAge ->
      TRUE
    [] OTHER -> SpecResult(c)

ImplementationActions(c) ==
  (SpecActions(c) \ {ReturnDue, ReturnNotDue})
    \cup (IF ImplementationResult(c) THEN {ReturnDue} ELSE {ReturnNotDue})

Bugs == {
  "none",
  "reject_due_age",
  "reject_boundary_age",
  "accept_zero_timeout",
  "accept_local_commit_vote",
  "accept_commit_qc_observed",
  "accept_stored_votes",
  "accept_cached_qc",
  "accept_under_timeout",
  "skip_local_evidence_guard",
  "skip_stored_votes_guard",
  "skip_cached_qc_guard",
  "accept_any_evidence"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

DueAgeAllowsFastUnblock ==
  /\ ImplementationResult(DuePastTimeout) = TRUE
  /\ AcceptPastTimeout \in ImplementationActions(DuePastTimeout)
  /\ ReturnDue \in ImplementationActions(DuePastTimeout)

TimeoutBoundaryIsInclusive ==
  /\ ImplementationResult(DueAtTimeoutBoundary) = TRUE
  /\ AcceptBoundary \in ImplementationActions(DueAtTimeoutBoundary)
  /\ ReturnDue \in ImplementationActions(DueAtTimeoutBoundary)

ZeroTimeoutDisablesFastUnblock ==
  /\ ImplementationResult(ZeroTimeoutPastAge) = FALSE
  /\ RejectZeroTimeout \in ImplementationActions(ZeroTimeoutPastAge)
  /\ ~(CheckLocalEvidence \in ImplementationActions(ZeroTimeoutPastAge))
  /\ ~(CheckStoredVotes \in ImplementationActions(ZeroTimeoutPastAge))
  /\ ~(CheckCachedQc \in ImplementationActions(ZeroTimeoutPastAge))
  /\ ~(CheckAge \in ImplementationActions(ZeroTimeoutPastAge))

LocalEvidenceShortCircuits ==
  /\ ImplementationResult(LocalCommitVotePastAge) = FALSE
  /\ RejectLocalCommitVote \in ImplementationActions(LocalCommitVotePastAge)
  /\ ~(CheckStoredVotes \in ImplementationActions(LocalCommitVotePastAge))
  /\ ~(CheckCachedQc \in ImplementationActions(LocalCommitVotePastAge))
  /\ ~(CheckAge \in ImplementationActions(LocalCommitVotePastAge))
  /\ ImplementationResult(CommitQcObservedPastAge) = FALSE
  /\ RejectCommitQcObserved
       \in ImplementationActions(CommitQcObservedPastAge)
  /\ ~(CheckStoredVotes \in ImplementationActions(CommitQcObservedPastAge))
  /\ ~(CheckCachedQc \in ImplementationActions(CommitQcObservedPastAge))
  /\ ~(CheckAge \in ImplementationActions(CommitQcObservedPastAge))

ConsensusEvidenceShortCircuits ==
  /\ ImplementationResult(StoredVotesPastAge) = FALSE
  /\ CheckStoredVotes \in ImplementationActions(StoredVotesPastAge)
  /\ RejectStoredVotes \in ImplementationActions(StoredVotesPastAge)
  /\ ~(CheckCachedQc \in ImplementationActions(StoredVotesPastAge))
  /\ ~(CheckAge \in ImplementationActions(StoredVotesPastAge))
  /\ ImplementationResult(CachedQcPastAge) = FALSE
  /\ CheckCachedQc \in ImplementationActions(CachedQcPastAge)
  /\ RejectCachedQc \in ImplementationActions(CachedQcPastAge)
  /\ ~(CheckAge \in ImplementationActions(CachedQcPastAge))
  /\ ImplementationResult(MultipleEvidencePastAge) = FALSE
  /\ RejectLocalCommitVote
       \in ImplementationActions(MultipleEvidencePastAge)
  /\ ~(CheckStoredVotes \in ImplementationActions(MultipleEvidencePastAge))
  /\ ~(CheckCachedQc \in ImplementationActions(MultipleEvidencePastAge))
  /\ ~(CheckAge \in ImplementationActions(MultipleEvidencePastAge))

AgeGateRequiresTimeout ==
  /\ ImplementationResult(UnderTimeout) = FALSE
  /\ CheckAge \in ImplementationActions(UnderTimeout)
  /\ RejectUnderTimeout \in ImplementationActions(UnderTimeout)
  /\ ImplementationResult(DuePastTimeout) = TRUE
  /\ ImplementationResult(DueAtTimeoutBoundary) = TRUE

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckTimeout \in ImplementationActions(c)
  /\ \A c \in TimeoutNonzeroCases:
       CheckLocalEvidence \in ImplementationActions(c)
  /\ \A c \in Cases \ TimeoutNonzeroCases:
       ~(CheckLocalEvidence \in ImplementationActions(c))
  /\ \A c \in Cases:
       (c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)) =>
         CheckStoredVotes \in ImplementationActions(c)
  /\ \A c \in Cases:
       (c \notin TimeoutNonzeroCases \/ ~NoLocalEvidence(c)) =>
         ~(CheckStoredVotes \in ImplementationActions(c))
  /\ \A c \in Cases:
       (c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
          /\ NoStoredVotes(c)) =>
         CheckCachedQc \in ImplementationActions(c)
  /\ \A c \in Cases:
       (c \in TimeoutNonzeroCases /\ NoLocalEvidence(c)
          /\ NoStoredVotes(c) /\ NoCachedQc(c)) =>
         CheckAge \in ImplementationActions(c)
  /\ \A c \in Cases:
       (c \notin TimeoutNonzeroCases \/ ~NoLocalEvidence(c)
          \/ ~NoStoredVotes(c) \/ ~NoCachedQc(c)) =>
         ~(CheckAge \in ImplementationActions(c))

PendingFastUnblockCoreSafety ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ DueAgeAllowsFastUnblock
  /\ TimeoutBoundaryIsInclusive
  /\ ZeroTimeoutDisablesFastUnblock
  /\ LocalEvidenceShortCircuits
  /\ ConsensusEvidenceShortCircuits
  /\ AgeGateRequiresTimeout
  /\ LookupShapeMatchesShortCircuit

NoBugInvariant == PendingFastUnblockCoreSafety

SafetyFast == PendingFastUnblockCoreSafety

====
