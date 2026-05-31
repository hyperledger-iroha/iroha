---- MODULE SumeragiEngineProposalLockGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure-engine proposal lock predicate.

This slice models `ConsensusEngine::proposal_satisfies_lock(...)`. Without a
locked QC, proposals are safe. With a locked QC, the locked subject remains
safe without extra evidence. Conflicting subjects must carry a strictly
greater QC under `qc_ref_cmp(...)`; absent, equal, or lower QCs cannot unlock
the conflict.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugRequireQcWhenUnlocked,
  \* @type: Bool;
  BugRejectLockedSubject,
  \* @type: Bool;
  BugIgnoreSubjectMatch,
  \* @type: Bool;
  BugAcceptConflictWithoutQc,
  \* @type: Bool;
  BugAcceptEqualQc,
  \* @type: Bool;
  BugUseNonStrictQcComparison,
  \* @type: Bool;
  BugAcceptLowerQc,
  \* @type: Bool;
  BugRejectHigherQc

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  accepted,
  \* @type: Set(Str);
  rejected

\* @type: <<Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, accepted, rejected>>

Candidates == {
  "unlocked_no_qc",
  "unlocked_with_qc",
  "locked_subject_no_qc",
  "locked_subject_lower_qc",
  "conflict_no_qc",
  "conflict_equal_qc",
  "conflict_lower_height_qc",
  "conflict_lower_view_qc",
  "conflict_higher_height_qc",
  "conflict_higher_view_qc",
  "conflict_higher_phase_qc",
  "conflict_higher_subject_qc"
}

HasLock(candidate) ==
  candidate \notin {"unlocked_no_qc", "unlocked_with_qc"}

SameSubject(candidate) ==
  candidate \in {"locked_subject_no_qc", "locked_subject_lower_qc"}

HasHighestQc(candidate) ==
  candidate \notin {"unlocked_no_qc", "locked_subject_no_qc", "conflict_no_qc"}

QcCmp(candidate) ==
  CASE candidate = "conflict_equal_qc" -> "eq"
    [] candidate \in {"conflict_lower_height_qc", "conflict_lower_view_qc", "locked_subject_lower_qc"} -> "lt"
    [] candidate \in {
      "conflict_higher_height_qc",
      "conflict_higher_view_qc",
      "conflict_higher_phase_qc",
      "conflict_higher_subject_qc"
    } -> "gt"
    [] OTHER -> "none"

SpecAccepts(candidate) ==
  \/ ~HasLock(candidate)
  \/ SameSubject(candidate)
  \/ QcCmp(candidate) = "gt"

ConflictAccepts(candidate) ==
  IF ~HasHighestQc(candidate)
  THEN BugAcceptConflictWithoutQc
  ELSE IF QcCmp(candidate) = "gt"
       THEN ~BugRejectHigherQc
       ELSE IF QcCmp(candidate) = "eq"
            THEN BugAcceptEqualQc \/ BugUseNonStrictQcComparison
            ELSE BugAcceptLowerQc

ImplementationAccepts(candidate) ==
  IF ~HasLock(candidate)
  THEN
    IF BugRequireQcWhenUnlocked
    THEN HasHighestQc(candidate)
    ELSE TRUE
  ELSE IF SameSubject(candidate) /\ ~BugIgnoreSubjectMatch
       THEN ~BugRejectLockedSubject
       ELSE ConflictAccepts(candidate)

TypeInvariant ==
  /\ BugRequireQcWhenUnlocked \in BOOLEAN
  /\ BugRejectLockedSubject \in BOOLEAN
  /\ BugIgnoreSubjectMatch \in BOOLEAN
  /\ BugAcceptConflictWithoutQc \in BOOLEAN
  /\ BugAcceptEqualQc \in BOOLEAN
  /\ BugUseNonStrictQcComparison \in BOOLEAN
  /\ BugAcceptLowerQc \in BOOLEAN
  /\ BugRejectHigherQc \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ accepted \subseteq Candidates
  /\ rejected \subseteq Candidates
  /\ accepted \cap rejected = {}
  /\ accepted \cup rejected = tried

Init ==
  /\ tried = {}
  /\ accepted = {}
  /\ rejected = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationAccepts(candidate)
     THEN
       /\ accepted' = accepted \cup {candidate}
       /\ rejected' = rejected
     ELSE
       /\ accepted' = accepted
       /\ rejected' = rejected \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AcceptedMatchesSpec ==
  accepted \subseteq {candidate \in Candidates : SpecAccepts(candidate)}

RejectedMatchesSpec ==
  rejected \subseteq {candidate \in Candidates : ~SpecAccepts(candidate)}

UnlockedProposalsAlwaysAccept ==
  \A candidate \in tried:
    ~HasLock(candidate) => candidate \in accepted

LockedSubjectAlwaysAccepts ==
  \A candidate \in tried:
    SameSubject(candidate) => candidate \in accepted

ConflictsWithoutQcNeverAccept ==
  "conflict_no_qc" \in tried =>
    "conflict_no_qc" \in rejected

EqualQcNeverUnlocks ==
  "conflict_equal_qc" \in tried =>
    "conflict_equal_qc" \in rejected

LowerQcNeverUnlocks ==
  \A candidate \in tried:
    candidate \in {"conflict_lower_height_qc", "conflict_lower_view_qc"} =>
      candidate \in rejected

StrictlyHigherQcUnlocks ==
  \A candidate \in tried:
    candidate \in {
      "conflict_higher_height_qc",
      "conflict_higher_view_qc",
      "conflict_higher_phase_qc",
      "conflict_higher_subject_qc"
    } =>
      candidate \in accepted

Safety ==
  /\ AcceptedMatchesSpec
  /\ RejectedMatchesSpec
  /\ UnlockedProposalsAlwaysAccept
  /\ LockedSubjectAlwaysAccepts
  /\ ConflictsWithoutQcNeverAccept
  /\ EqualQcNeverUnlocks
  /\ LowerQcNeverUnlocks
  /\ StrictlyHigherQcUnlocks

====
