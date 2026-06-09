---- MODULE SumeragiSupersededFrontierPayloadRetentionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for
`should_retain_superseded_contiguous_frontier_payload(...)`.

This slice pins the helper that decides whether a superseded same-height
contiguous-frontier payload remains locally recoverable for exact body fetches:
- DA must be enabled,
- the incoming replacement must not already be materialized,
- invalid pending blocks and blocks already committed in Kura are never kept,
- the pending block must extend the current tip exactly via `pending_extends_tip`,
  including the valid "no parent and no tip hash" boundary, and
- retention is allowed only when some commit evidence exists: retired same-height
  state, a local commit vote, an observed Commit QC, stored commit votes, or a
  stored QC.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

RetiredSameHeight == "retired_same_height"
LocalCommitVote == "local_commit_vote"
CommitQcObserved == "commit_qc_observed"
CommitVotesObserved == "commit_votes_observed"
QcObserved == "qc_observed"
AbsentTipParentMatch == "absent_tip_parent_match"
NoCommitEvidence == "no_commit_evidence"
DaDisabled == "da_disabled"
IncomingMaterialized == "incoming_materialized"
InvalidPending == "invalid_pending"
CommittedInKura == "committed_in_kura"
WrongHeight == "wrong_height"
ParentMismatch == "parent_mismatch"
NoTipPendingParent == "no_tip_pending_parent"

Cases == {
  RetiredSameHeight,
  LocalCommitVote,
  CommitQcObserved,
  CommitVotesObserved,
  QcObserved,
  AbsentTipParentMatch,
  NoCommitEvidence,
  DaDisabled,
  IncomingMaterialized,
  InvalidPending,
  CommittedInKura,
  WrongHeight,
  ParentMismatch,
  NoTipPendingParent
}

DaEnabled(c) == c # DaDisabled

IncomingMaterializedBeforeSupersede(c) == c = IncomingMaterialized

ValidationInvalid(c) == c = InvalidPending

AlreadyCommitted(c) == c = CommittedInKura

ExtendsTip(c) == c \notin {WrongHeight, ParentMismatch, NoTipPendingParent}

RetiredEvidence(c) == c = RetiredSameHeight

LocalCommitVoteEvidence(c) ==
  c \in {
    LocalCommitVote,
    AbsentTipParentMatch,
    DaDisabled,
    IncomingMaterialized,
    InvalidPending,
    CommittedInKura,
    WrongHeight,
    ParentMismatch,
    NoTipPendingParent
  }

CommitQcObservedEvidence(c) == c = CommitQcObserved

CommitVotesEvidence(c) == c = CommitVotesObserved

QcEvidence(c) == c = QcObserved

CommitEvidence(c) ==
  RetiredEvidence(c)
    \/ LocalCommitVoteEvidence(c)
    \/ CommitQcObservedEvidence(c)
    \/ CommitVotesEvidence(c)
    \/ QcEvidence(c)

SpecRetain(c) ==
  DaEnabled(c)
    /\ ~IncomingMaterializedBeforeSupersede(c)
    /\ ~ValidationInvalid(c)
    /\ ~AlreadyCommitted(c)
    /\ ExtendsTip(c)
    /\ CommitEvidence(c)

ActualRetain(c) ==
  CASE Bug = "retain_da_disabled"
       /\ c = DaDisabled -> TRUE
    [] Bug = "retain_incoming_materialized"
       /\ c = IncomingMaterialized -> TRUE
    [] Bug = "retain_invalid_pending"
       /\ c = InvalidPending -> TRUE
    [] Bug = "retain_committed_block"
       /\ c = CommittedInKura -> TRUE
    [] Bug = "retain_wrong_height"
       /\ c = WrongHeight -> TRUE
    [] Bug = "retain_parent_mismatch"
       /\ c = ParentMismatch -> TRUE
    [] Bug = "retain_missing_tip_parent_mismatch"
       /\ c = NoTipPendingParent -> TRUE
    [] Bug = "reject_absent_tip_parent_match"
       /\ c = AbsentTipParentMatch -> FALSE
    [] Bug = "retain_without_commit_evidence"
       /\ c = NoCommitEvidence -> TRUE
    [] Bug = "ignore_retired_same_height"
       /\ c = RetiredSameHeight -> FALSE
    [] Bug = "ignore_local_commit_vote"
       /\ c \in {LocalCommitVote, AbsentTipParentMatch} -> FALSE
    [] Bug = "ignore_commit_qc_observed"
       /\ c = CommitQcObserved -> FALSE
    [] Bug = "ignore_commit_votes"
       /\ c = CommitVotesObserved -> FALSE
    [] Bug = "ignore_qc"
       /\ c = QcObserved -> FALSE
    [] Bug = "require_all_commit_evidence"
       /\ c \in {
            RetiredSameHeight,
            LocalCommitVote,
            CommitQcObserved,
            CommitVotesObserved,
            QcObserved,
            AbsentTipParentMatch
          } -> FALSE
    [] Bug = "require_qc_only"
       /\ c \in {
            RetiredSameHeight,
            LocalCommitVote,
            CommitQcObserved,
            CommitVotesObserved,
            AbsentTipParentMatch
          } -> FALSE
    [] OTHER -> SpecRetain(c)

Bugs == {
  "none",
  "retain_da_disabled",
  "retain_incoming_materialized",
  "retain_invalid_pending",
  "retain_committed_block",
  "retain_wrong_height",
  "retain_parent_mismatch",
  "retain_missing_tip_parent_mismatch",
  "reject_absent_tip_parent_match",
  "retain_without_commit_evidence",
  "ignore_retired_same_height",
  "ignore_local_commit_vote",
  "ignore_commit_qc_observed",
  "ignore_commit_votes",
  "ignore_qc",
  "require_all_commit_evidence",
  "require_qc_only"
}

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs
  /\ \A c \in Cases:
       /\ DaEnabled(c) \in BOOLEAN
       /\ IncomingMaterializedBeforeSupersede(c) \in BOOLEAN
       /\ ValidationInvalid(c) \in BOOLEAN
       /\ AlreadyCommitted(c) \in BOOLEAN
       /\ ExtendsTip(c) \in BOOLEAN
       /\ CommitEvidence(c) \in BOOLEAN
       /\ SpecRetain(c) \in BOOLEAN
       /\ ActualRetain(c) \in BOOLEAN

SupersededFrontierPayloadRetentionCoreSafety ==
  \A c \in Cases:
    ActualRetain(c) = SpecRetain(c)

NoBugInvariant == SupersededFrontierPayloadRetentionCoreSafety

AcceptedCommitEvidenceAnchors ==
  /\ ActualRetain(RetiredSameHeight)
  /\ ActualRetain(LocalCommitVote)
  /\ ActualRetain(CommitQcObserved)
  /\ ActualRetain(CommitVotesObserved)
  /\ ActualRetain(QcObserved)

BoundaryParentMatchAnchors ==
  /\ ExtendsTip(AbsentTipParentMatch)
  /\ CommitEvidence(AbsentTipParentMatch)
  /\ ActualRetain(AbsentTipParentMatch)
  /\ ~ExtendsTip(NoTipPendingParent)
  /\ ~ActualRetain(NoTipPendingParent)

AdmissionRejectionAnchors ==
  /\ ~ActualRetain(DaDisabled)
  /\ ~ActualRetain(IncomingMaterialized)
  /\ ~ActualRetain(InvalidPending)
  /\ ~ActualRetain(CommittedInKura)
  /\ ~ActualRetain(WrongHeight)
  /\ ~ActualRetain(ParentMismatch)
  /\ ~ActualRetain(NoTipPendingParent)

CommitEvidenceRejectionAnchors ==
  /\ ~CommitEvidence(NoCommitEvidence)
  /\ ~ActualRetain(NoCommitEvidence)

RetentionImpliesAllGuardsAnchors ==
  \A c \in Cases:
    ActualRetain(c)
      => /\ DaEnabled(c)
         /\ ~IncomingMaterializedBeforeSupersede(c)
         /\ ~ValidationInvalid(c)
         /\ ~AlreadyCommitted(c)
         /\ ExtendsTip(c)
         /\ CommitEvidence(c)

SafetyAnchors ==
  /\ NoBugInvariant
  /\ AcceptedCommitEvidenceAnchors
  /\ BoundaryParentMatchAnchors
  /\ AdmissionRejectionAnchors
  /\ CommitEvidenceRejectionAnchors
  /\ RetentionImpliesAllGuardsAnchors

SafetyFast == SupersededFrontierPayloadRetentionCoreSafety

BugRetainDaDisabled == NoBugInvariant
BugRetainIncomingMaterialized == NoBugInvariant
BugRetainInvalidPending == NoBugInvariant
BugRetainCommittedBlock == NoBugInvariant
BugRetainWrongHeight == NoBugInvariant
BugRetainParentMismatch == NoBugInvariant
BugRetainMissingTipParentMismatch == NoBugInvariant
BugRejectAbsentTipParentMatch == NoBugInvariant
BugRetainWithoutCommitEvidence == NoBugInvariant
BugIgnoreRetiredSameHeight == NoBugInvariant
BugIgnoreLocalCommitVote == NoBugInvariant
BugIgnoreCommitQcObserved == NoBugInvariant
BugIgnoreCommitVotes == NoBugInvariant
BugIgnoreQc == NoBugInvariant
BugRequireAllCommitEvidence == NoBugInvariant
BugRequireQcOnly == NoBugInvariant

====
