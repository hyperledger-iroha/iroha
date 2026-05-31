---- MODULE SumeragiPendingBlockActiveForTipGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pending block activity against the committed tip.

This slice captures `pending_block_is_active_for_tip(...)` together with the
local `pending_extends_tip(...)` predicate and the evidence disjunction in
`pending_block_has_consensus_evidence(...)`. A pending block is active only when
it is not consensus-inactive, its height is exactly the committed tip height
plus one, its parent hash equals the current tip hash, and at least one
consensus evidence source is present: authoritative payload, local commit vote,
observed commit QC, stored votes, or cached QC.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ActiveAuthoritativePayload == "active_authoritative_payload"
ActiveLocalCommitVote == "active_local_commit_vote"
ActiveCommitQcObserved == "active_commit_qc_observed"
ActiveStoredVotes == "active_stored_votes"
ActiveCachedQc == "active_cached_qc"
ActiveMultipleEvidence == "active_multiple_evidence"
InactiveWithEvidence == "inactive_with_evidence"
WrongHeightWithEvidence == "wrong_height_with_evidence"
ParentMismatchWithEvidence == "parent_mismatch_with_evidence"
MissingTipHashWithParent == "missing_tip_hash_with_parent"
TipHeightOverflowWithEvidence == "tip_height_overflow_with_evidence"
NoEvidence == "no_evidence"
NoEvidenceWithTip == "no_evidence_with_tip"
AbsentTipAndNoParent == "absent_tip_and_no_parent"

Cases == {
  ActiveAuthoritativePayload,
  ActiveLocalCommitVote,
  ActiveCommitQcObserved,
  ActiveStoredVotes,
  ActiveCachedQc,
  ActiveMultipleEvidence,
  InactiveWithEvidence,
  WrongHeightWithEvidence,
  ParentMismatchWithEvidence,
  MissingTipHashWithParent,
  TipHeightOverflowWithEvidence,
  NoEvidence,
  NoEvidenceWithTip,
  AbsentTipAndNoParent
}

ConsensusInactiveCases == {InactiveWithEvidence}

HeightMatchesCases == {
  ActiveAuthoritativePayload,
  ActiveLocalCommitVote,
  ActiveCommitQcObserved,
  ActiveStoredVotes,
  ActiveCachedQc,
  ActiveMultipleEvidence,
  InactiveWithEvidence,
  ParentMismatchWithEvidence,
  MissingTipHashWithParent,
  NoEvidence,
  NoEvidenceWithTip,
  AbsentTipAndNoParent
}

ParentMatchesCases == {
  ActiveAuthoritativePayload,
  ActiveLocalCommitVote,
  ActiveCommitQcObserved,
  ActiveStoredVotes,
  ActiveCachedQc,
  ActiveMultipleEvidence,
  InactiveWithEvidence,
  WrongHeightWithEvidence,
  NoEvidenceWithTip,
  AbsentTipAndNoParent
}

TipHeightConvertibleCases == Cases \ {TipHeightOverflowWithEvidence}

AuthoritativePayloadCases == {
  ActiveAuthoritativePayload,
  ActiveMultipleEvidence,
  InactiveWithEvidence,
  WrongHeightWithEvidence,
  ParentMismatchWithEvidence,
  MissingTipHashWithParent,
  TipHeightOverflowWithEvidence
}

LocalCommitVoteCases == {
  ActiveLocalCommitVote,
  ActiveMultipleEvidence
}

CommitQcObservedCases == {
  ActiveCommitQcObserved,
  ActiveMultipleEvidence
}

StoredVotesCases == {
  ActiveStoredVotes,
  ActiveMultipleEvidence
}

CachedQcCases == {
  ActiveCachedQc,
  ActiveMultipleEvidence
}

HasEvidence(c) ==
  c \in AuthoritativePayloadCases
    \/ c \in LocalCommitVoteCases
    \/ c \in CommitQcObservedCases
    \/ c \in StoredVotesCases
    \/ c \in CachedQcCases

ExtendsTip(c) ==
  c \in TipHeightConvertibleCases
    /\ c \in HeightMatchesCases
    /\ c \in ParentMatchesCases

SpecResult(c) ==
  c \notin ConsensusInactiveCases /\ ExtendsTip(c) /\ HasEvidence(c)

ReturnActive == 1
ReturnInactive == 2
CheckConsensusActive == 3
CheckHeight == 4
CheckParent == 5
CheckEvidence == 6
ConsensusInactiveRejected == 7
HeightMismatchRejected == 8
ParentMismatchRejected == 9
HeightOverflowRejected == 10
AuthoritativePayloadEvidence == 11
LocalCommitVoteEvidence == 12
CommitQcObservedEvidence == 13
StoredVotesEvidence == 14
CachedQcEvidence == 15
NoEvidenceRejected == 16

ActionUniverse == 1..16

EvidenceActions(c) ==
  (IF c \in AuthoritativePayloadCases THEN {AuthoritativePayloadEvidence} ELSE {})
    \cup (IF c \in LocalCommitVoteCases THEN {LocalCommitVoteEvidence} ELSE {})
    \cup (IF c \in CommitQcObservedCases THEN {CommitQcObservedEvidence} ELSE {})
    \cup (IF c \in StoredVotesCases THEN {StoredVotesEvidence} ELSE {})
    \cup (IF c \in CachedQcCases THEN {CachedQcEvidence} ELSE {})
    \cup (IF ~HasEvidence(c) THEN {NoEvidenceRejected} ELSE {})

SpecActions(c) ==
  {CheckConsensusActive}
    \cup (IF SpecResult(c) THEN {ReturnActive} ELSE {ReturnInactive})
    \cup (IF c \in ConsensusInactiveCases
          THEN {ConsensusInactiveRejected}
          ELSE {CheckHeight})
    \cup (IF c \notin ConsensusInactiveCases
              /\ c \notin TipHeightConvertibleCases
          THEN {HeightOverflowRejected}
          ELSE {})
    \cup (IF c \notin ConsensusInactiveCases
              /\ c \in TipHeightConvertibleCases
              /\ c \notin HeightMatchesCases
          THEN {HeightMismatchRejected}
          ELSE {})
    \cup (IF c \notin ConsensusInactiveCases
              /\ c \in TipHeightConvertibleCases
              /\ c \in HeightMatchesCases
          THEN {CheckParent}
          ELSE {})
    \cup (IF c \notin ConsensusInactiveCases
              /\ c \in TipHeightConvertibleCases
              /\ c \in HeightMatchesCases
              /\ c \notin ParentMatchesCases
          THEN {ParentMismatchRejected}
          ELSE {})
    \cup (IF c \notin ConsensusInactiveCases /\ ExtendsTip(c)
          THEN {CheckEvidence} \cup EvidenceActions(c)
          ELSE {})

ImplementationResult(c) ==
  CASE Bug = "reject_authoritative_payload_evidence"
       /\ c = ActiveAuthoritativePayload ->
      FALSE
    [] Bug = "reject_local_commit_vote"
       /\ c = ActiveLocalCommitVote ->
      FALSE
    [] Bug = "reject_commit_qc_observed"
       /\ c = ActiveCommitQcObserved ->
      FALSE
    [] Bug = "reject_stored_votes"
       /\ c = ActiveStoredVotes ->
      FALSE
    [] Bug = "reject_cached_qc"
       /\ c = ActiveCachedQc ->
      FALSE
    [] Bug = "accept_inactive_pending"
       /\ c = InactiveWithEvidence ->
      TRUE
    [] Bug = "accept_wrong_height"
       /\ c = WrongHeightWithEvidence ->
      TRUE
    [] Bug = "accept_parent_mismatch"
       /\ c = ParentMismatchWithEvidence ->
      TRUE
    [] Bug = "accept_missing_tip_hash_parent"
       /\ c = MissingTipHashWithParent ->
      TRUE
    [] Bug = "accept_tip_height_overflow"
       /\ c = TipHeightOverflowWithEvidence ->
      TRUE
    [] Bug = "accept_no_evidence"
       /\ c \in {NoEvidence, NoEvidenceWithTip} ->
      TRUE
    [] Bug = "accept_absent_tip_and_no_parent"
       /\ c = AbsentTipAndNoParent ->
      TRUE
    [] Bug = "ignore_consensus_inactive"
       /\ c = InactiveWithEvidence ->
      TRUE
    [] Bug = "ignore_tip_extension"
       /\ c \in {WrongHeightWithEvidence, ParentMismatchWithEvidence,
                 MissingTipHashWithParent, TipHeightOverflowWithEvidence} ->
      TRUE
    [] Bug = "ignore_evidence_gate"
       /\ c \in {NoEvidence, NoEvidenceWithTip, AbsentTipAndNoParent} ->
      TRUE
    [] Bug = "require_multiple_evidence"
       /\ c \in {ActiveAuthoritativePayload, ActiveLocalCommitVote,
                 ActiveCommitQcObserved, ActiveStoredVotes, ActiveCachedQc} ->
      FALSE
    [] OTHER -> SpecResult(c)

ImplementationActions(c) ==
  (SpecActions(c) \ {ReturnActive, ReturnInactive})
    \cup (IF ImplementationResult(c) THEN {ReturnActive} ELSE {ReturnInactive})

Bugs == {
  "none",
  "reject_authoritative_payload_evidence",
  "reject_local_commit_vote",
  "reject_commit_qc_observed",
  "reject_stored_votes",
  "reject_cached_qc",
  "accept_inactive_pending",
  "accept_wrong_height",
  "accept_parent_mismatch",
  "accept_missing_tip_hash_parent",
  "accept_tip_height_overflow",
  "accept_no_evidence",
  "accept_absent_tip_and_no_parent",
  "ignore_consensus_inactive",
  "ignore_tip_extension",
  "ignore_evidence_gate",
  "require_multiple_evidence"
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

EachEvidenceSourceCanKeepPendingActive ==
  /\ ImplementationResult(ActiveAuthoritativePayload) = TRUE
  /\ AuthoritativePayloadEvidence
       \in ImplementationActions(ActiveAuthoritativePayload)
  /\ ImplementationResult(ActiveLocalCommitVote) = TRUE
  /\ LocalCommitVoteEvidence \in ImplementationActions(ActiveLocalCommitVote)
  /\ ImplementationResult(ActiveCommitQcObserved) = TRUE
  /\ CommitQcObservedEvidence
       \in ImplementationActions(ActiveCommitQcObserved)
  /\ ImplementationResult(ActiveStoredVotes) = TRUE
  /\ StoredVotesEvidence \in ImplementationActions(ActiveStoredVotes)
  /\ ImplementationResult(ActiveCachedQc) = TRUE
  /\ CachedQcEvidence \in ImplementationActions(ActiveCachedQc)
  /\ ImplementationResult(ActiveMultipleEvidence) = TRUE

InactivePendingNeverActive ==
  /\ ImplementationResult(InactiveWithEvidence) = FALSE
  /\ ConsensusInactiveRejected \in ImplementationActions(InactiveWithEvidence)
  /\ ~(CheckHeight \in ImplementationActions(InactiveWithEvidence))
  /\ ~(CheckEvidence \in ImplementationActions(InactiveWithEvidence))

TipExtensionIsRequired ==
  /\ ImplementationResult(WrongHeightWithEvidence) = FALSE
  /\ HeightMismatchRejected \in ImplementationActions(WrongHeightWithEvidence)
  /\ ImplementationResult(ParentMismatchWithEvidence) = FALSE
  /\ ParentMismatchRejected
       \in ImplementationActions(ParentMismatchWithEvidence)
  /\ ImplementationResult(MissingTipHashWithParent) = FALSE
  /\ ParentMismatchRejected \in ImplementationActions(MissingTipHashWithParent)
  /\ ImplementationResult(TipHeightOverflowWithEvidence) = FALSE
  /\ HeightOverflowRejected
       \in ImplementationActions(TipHeightOverflowWithEvidence)
  /\ ImplementationResult(AbsentTipAndNoParent) = FALSE

ConsensusEvidenceIsRequired ==
  /\ ImplementationResult(NoEvidence) = FALSE
  /\ ImplementationResult(NoEvidenceWithTip) = FALSE
  /\ ImplementationResult(AbsentTipAndNoParent) = FALSE
  /\ NoEvidenceRejected \in ImplementationActions(NoEvidenceWithTip)
  /\ NoEvidenceRejected \in ImplementationActions(AbsentTipAndNoParent)

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckConsensusActive \in ImplementationActions(c)
  /\ ~(CheckHeight \in ImplementationActions(InactiveWithEvidence))
  /\ \A c \in Cases \ ConsensusInactiveCases:
       CheckHeight \in ImplementationActions(c)
  /\ \A c \in {WrongHeightWithEvidence, TipHeightOverflowWithEvidence}:
       ~(CheckParent \in ImplementationActions(c))
  /\ \A c \in Cases:
       (c \notin ConsensusInactiveCases /\ c \in TipHeightConvertibleCases
          /\ c \in HeightMatchesCases) =>
         CheckParent \in ImplementationActions(c)
  /\ \A c \in Cases:
       (~ExtendsTip(c) \/ c \in ConsensusInactiveCases) =>
         ~(CheckEvidence \in ImplementationActions(c))
  /\ \A c \in Cases:
       (c \notin ConsensusInactiveCases /\ ExtendsTip(c)) =>
         CheckEvidence \in ImplementationActions(c)

NoBugInvariant ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ EachEvidenceSourceCanKeepPendingActive
  /\ InactivePendingNeverActive
  /\ TipExtensionIsRequired
  /\ ConsensusEvidenceIsRequired
  /\ LookupShapeMatchesShortCircuit

SafetyFast == NoBugInvariant

====
