---- MODULE SumeragiNewViewHighestQcVotesGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for actor-side NEW_VIEW highest-QC vote selection.

This slice pins `select_new_view_highest_qc_from_votes(...)`,
`new_view_highest_qc_rank(...)`, and `new_view_highest_qc_signer_groups(...)`:
only accepted votes from the requested signer set, height, view, epoch, and
NEW_VIEW phase may contribute a highest-QC candidate; candidates must be
Prepare or Commit QCs; equal QC references are grouped by exact reference; and
selection ranks candidates by height, view, phase rank, then block hash.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoPhase == 0
NewViewPhase == 1
CommitPhase == 2
PreparePhase == 3
OtherPhase == 4

TargetHeight == 10
TargetView == 2
TargetEpoch == 1

\* @type: (Int, Int, Int, Int, Int) => <<Int, Int, Int, Int, Int>>;
Qc(phase, height, view, epoch, hash) ==
  <<phase, height, view, epoch, hash>>

NoQc == Qc(NoPhase, 0, 0, 0, 0)
QPrepareBase == Qc(PreparePhase, 4, 1, TargetEpoch, 30)
QPrepareSame == Qc(PreparePhase, 5, 2, TargetEpoch, 40)
QCommitSame == Qc(CommitPhase, 5, 2, TargetEpoch, 40)
QLowerCommitView == Qc(CommitPhase, 5, 1, TargetEpoch, 60)
QHigherPrepareView == Qc(PreparePhase, 5, 2, TargetEpoch, 20)
QLowerCommitHeight == Qc(CommitPhase, 5, 9, TargetEpoch, 90)
QHigherPrepareHeight == Qc(PreparePhase, 6, 0, TargetEpoch, 10)
QLowHash == Qc(CommitPhase, 7, 1, TargetEpoch, 10)
QHighHash == Qc(CommitPhase, 7, 1, TargetEpoch, 90)
QInvalidNewView == Qc(NewViewPhase, 4, 1, TargetEpoch, 50)
QDuplicate == Qc(CommitPhase, 8, 1, TargetEpoch, 55)
QGroupA == Qc(CommitPhase, 3, 1, TargetEpoch, 55)
QGroupB == Qc(CommitPhase, 3, 2, TargetEpoch, 55)

QcUniverse == {
  NoQc,
  QPrepareBase,
  QPrepareSame,
  QCommitSame,
  QLowerCommitView,
  QHigherPrepareView,
  QLowerCommitHeight,
  QHigherPrepareHeight,
  QLowHash,
  QHighHash,
  QInvalidNewView,
  QDuplicate,
  QGroupA,
  QGroupB
}

Cases == {
  "no_signers",
  "missing_vote",
  "valid_prepare",
  "commit_beats_prepare",
  "higher_view_beats_lower_commit",
  "higher_height_beats_lower_commit",
  "hash_tiebreak",
  "wrong_vote_phase",
  "wrong_vote_height",
  "wrong_vote_view",
  "wrong_vote_epoch",
  "missing_highest",
  "invalid_candidate_phase",
  "group_duplicate_exact",
  "group_distinct_reference",
  "signers_filter"
}

\* @type: (Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>) => <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>>;
Vote(signer, phase, height, view, epoch, highestQc) ==
  <<signer, phase, height, view, epoch, highestQc>>

\* @type: (Int, <<Int, Int, Int, Int, Int>>) => <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>>;
GoodVote(signer, highestQc) ==
  Vote(signer, NewViewPhase, TargetHeight, TargetView, TargetEpoch, highestQc)

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => Int;
VoteSigner(v) == v[1]

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => Int;
VotePhase(v) == v[2]

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => Int;
VoteHeight(v) == v[3]

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => Int;
VoteView(v) == v[4]

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => Int;
VoteEpoch(v) == v[5]

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => <<Int, Int, Int, Int, Int>>;
VoteHighestQc(v) == v[6]

\* @type: <<Int, Int, Int, Int, Int>> => Int;
QcPhase(q) == q[1]

\* @type: <<Int, Int, Int, Int, Int>> => Int;
QcHeight(q) == q[2]

\* @type: <<Int, Int, Int, Int, Int>> => Int;
QcView(q) == q[3]

\* @type: <<Int, Int, Int, Int, Int>> => Int;
QcHash(q) == q[5]

\* @type: Str => Set(Int);
Signers(c) ==
  CASE c = "no_signers" -> {}
    [] c = "missing_vote" -> {1}
    [] c = "commit_beats_prepare" -> {1, 2}
    [] c = "higher_view_beats_lower_commit" -> {1, 2}
    [] c = "higher_height_beats_lower_commit" -> {1, 2}
    [] c = "hash_tiebreak" -> {1, 2}
    [] c = "group_duplicate_exact" -> {1, 2}
    [] c = "group_distinct_reference" -> {1, 2}
    [] c = "signers_filter" -> {1}
    [] OTHER -> {1}

\* @type: Str => Set(<<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>>);
Votes(c) ==
  CASE c = "no_signers" -> {GoodVote(1, QPrepareBase)}
    [] c = "missing_vote" -> {}
    [] c = "valid_prepare" -> {GoodVote(1, QPrepareBase)}
    [] c = "commit_beats_prepare" ->
       {GoodVote(1, QPrepareSame), GoodVote(2, QCommitSame)}
    [] c = "higher_view_beats_lower_commit" ->
       {GoodVote(1, QLowerCommitView), GoodVote(2, QHigherPrepareView)}
    [] c = "higher_height_beats_lower_commit" ->
       {GoodVote(1, QLowerCommitHeight), GoodVote(2, QHigherPrepareHeight)}
    [] c = "hash_tiebreak" ->
       {GoodVote(1, QLowHash), GoodVote(2, QHighHash)}
    [] c = "wrong_vote_phase" ->
       {Vote(1, CommitPhase, TargetHeight, TargetView, TargetEpoch, QPrepareBase)}
    [] c = "wrong_vote_height" ->
       {Vote(1, NewViewPhase, TargetHeight + 1, TargetView, TargetEpoch, QPrepareBase)}
    [] c = "wrong_vote_view" ->
       {Vote(1, NewViewPhase, TargetHeight, TargetView + 1, TargetEpoch, QPrepareBase)}
    [] c = "wrong_vote_epoch" ->
       {Vote(1, NewViewPhase, TargetHeight, TargetView, TargetEpoch + 1, QPrepareBase)}
    [] c = "missing_highest" ->
       {GoodVote(1, NoQc)}
    [] c = "invalid_candidate_phase" ->
       {GoodVote(1, QInvalidNewView)}
    [] c = "group_duplicate_exact" ->
       {GoodVote(1, QDuplicate), GoodVote(2, QDuplicate)}
    [] c = "group_distinct_reference" ->
       {GoodVote(1, QGroupA), GoodVote(2, QGroupB)}
    [] c = "signers_filter" ->
       {GoodVote(1, QPrepareBase), GoodVote(2, QHighHash)}
    [] OTHER -> {}

\* @type: Int => Int;
PhaseRank(phase) ==
  CASE phase = PreparePhase -> 0
    [] phase = CommitPhase -> 1
    [] phase = NewViewPhase -> 2
    [] OTHER -> -1

\* @type: <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>> => Bool;
VoteSlotMatches(v) ==
  /\ VotePhase(v) = NewViewPhase
  /\ VoteHeight(v) = TargetHeight
  /\ VoteView(v) = TargetView
  /\ VoteEpoch(v) = TargetEpoch

\* @type: <<Int, Int, Int, Int, Int>> => Bool;
CandidatePresent(q) == q # NoQc

\* @type: <<Int, Int, Int, Int, Int>> => Bool;
CandidatePhaseValid(q) ==
  QcPhase(q) = CommitPhase \/ QcPhase(q) = PreparePhase

\* @type: (Str, <<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>>) => Bool;
SpecAcceptsVote(c, v) ==
  /\ VoteSigner(v) \in Signers(c)
  /\ VoteSlotMatches(v)
  /\ CandidatePresent(VoteHighestQc(v))
  /\ CandidatePhaseValid(VoteHighestQc(v))

\* @type: Str => Set(<<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>>);
SpecMatchingVotes(c) == {v \in Votes(c): SpecAcceptsVote(c, v)}

\* @type: Str => Set(<<Int, Int, Int, Int, Int>>);
SpecGroups(c) == {VoteHighestQc(v): v \in SpecMatchingVotes(c)}

\* @type: (Str, <<Int, Int, Int, Int, Int>>) => Int;
SpecGroupSize(c, q) ==
  Cardinality({v \in SpecMatchingVotes(c): VoteHighestQc(v) = q})

\* @type: Str => Int;
SpecGroupCount(c) == Cardinality(SpecGroups(c))

\* @type: (<<Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int>>) => Bool;
RankHigher(left, right) ==
  \/ QcHeight(left) > QcHeight(right)
  \/ /\ QcHeight(left) = QcHeight(right)
     /\ QcView(left) > QcView(right)
  \/ /\ QcHeight(left) = QcHeight(right)
     /\ QcView(left) = QcView(right)
     /\ PhaseRank(QcPhase(left)) > PhaseRank(QcPhase(right))
  \/ /\ QcHeight(left) = QcHeight(right)
     /\ QcView(left) = QcView(right)
     /\ PhaseRank(QcPhase(left)) = PhaseRank(QcPhase(right))
     /\ QcHash(left) > QcHash(right)

\* @type: Set(<<Int, Int, Int, Int, Int>>) => <<Int, Int, Int, Int, Int>>;
Best(groups) ==
  IF groups = {} THEN
    NoQc
  ELSE
    CHOOSE q \in groups:
      \A other \in groups: ~RankHigher(other, q)

\* @type: Str => <<Int, Int, Int, Int, Int>>;
SpecSelected(c) == Best(SpecGroups(c))

\* @type: Str => Set(<<Int, Int, Int, Int, Int, <<Int, Int, Int, Int, Int>>>>);
ActualMatchingVotes(c) ==
  CASE Bug = "include_non_signer"
       /\ c = "signers_filter" ->
       {v \in Votes(c):
          /\ VoteSlotMatches(v)
          /\ CandidatePresent(VoteHighestQc(v))
          /\ CandidatePhaseValid(VoteHighestQc(v))}
    [] Bug = "accept_wrong_vote_phase"
       /\ c = "wrong_vote_phase" -> Votes(c)
    [] Bug = "accept_wrong_vote_height"
       /\ c = "wrong_vote_height" -> Votes(c)
    [] Bug = "accept_wrong_vote_view"
       /\ c = "wrong_vote_view" -> Votes(c)
    [] Bug = "accept_wrong_vote_epoch"
       /\ c = "wrong_vote_epoch" -> Votes(c)
    [] Bug = "accept_missing_highest"
       /\ c = "missing_highest" -> Votes(c)
    [] Bug = "accept_invalid_candidate_phase"
       /\ c = "invalid_candidate_phase" -> Votes(c)
    [] OTHER -> SpecMatchingVotes(c)

\* @type: Str => Set(<<Int, Int, Int, Int, Int>>);
ActualGroups(c) ==
  CASE Bug = "merge_distinct_references"
       /\ c = "group_distinct_reference" -> {QGroupA}
    [] OTHER -> {VoteHighestQc(v): v \in ActualMatchingVotes(c)}

\* @type: (Str, <<Int, Int, Int, Int, Int>>) => Int;
ActualGroupSize(c, q) ==
  CASE Bug = "duplicate_exact_not_grouped"
       /\ c = "group_duplicate_exact"
       /\ q = QDuplicate -> 1
    [] Bug = "merge_distinct_references"
       /\ c = "group_distinct_reference"
       /\ q = QGroupA -> 2
    [] Bug = "merge_distinct_references"
       /\ c = "group_distinct_reference"
       /\ q = QGroupB -> 0
    [] OTHER -> Cardinality({v \in ActualMatchingVotes(c): VoteHighestQc(v) = q})

\* @type: Str => Int;
ActualGroupCount(c) == Cardinality(ActualGroups(c))

\* @type: Str => <<Int, Int, Int, Int, Int>>;
ActualSelected(c) ==
  CASE Bug = "rank_prepare_over_commit"
       /\ c = "commit_beats_prepare" -> QPrepareSame
    [] Bug = "rank_phase_before_view"
       /\ c = "higher_view_beats_lower_commit" -> QLowerCommitView
    [] Bug = "rank_phase_before_height"
       /\ c = "higher_height_beats_lower_commit" -> QLowerCommitHeight
    [] Bug = "rank_low_hash"
       /\ c = "hash_tiebreak" -> QLowHash
    [] OTHER -> Best(ActualGroups(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "include_non_signer",
       "accept_wrong_vote_phase",
       "accept_wrong_vote_height",
       "accept_wrong_vote_view",
       "accept_wrong_vote_epoch",
       "accept_missing_highest",
       "accept_invalid_candidate_phase",
       "rank_prepare_over_commit",
       "rank_phase_before_view",
       "rank_phase_before_height",
       "rank_low_hash",
       "merge_distinct_references",
       "duplicate_exact_not_grouped"
     }
  /\ checked = 0

NewViewHighestQcVotesMatchesSpec ==
  /\ \A c \in Cases:
       ActualGroups(c) = SpecGroups(c)
  /\ \A c \in Cases:
       ActualGroupCount(c) = SpecGroupCount(c)
  /\ \A c \in Cases:
       ActualSelected(c) = SpecSelected(c)
  /\ \A c \in Cases:
       \A q \in QcUniverse:
         ActualGroupSize(c, q) = SpecGroupSize(c, q)

SafetyFast ==
  NewViewHighestQcVotesMatchesSpec

BugIncludeNonSigner ==
  ActualGroups("signers_filter") = SpecGroups("signers_filter")

BugAcceptWrongVotePhase ==
  ActualGroups("wrong_vote_phase") = SpecGroups("wrong_vote_phase")

BugAcceptWrongVoteHeight ==
  ActualGroups("wrong_vote_height") = SpecGroups("wrong_vote_height")

BugAcceptWrongVoteView ==
  ActualGroups("wrong_vote_view") = SpecGroups("wrong_vote_view")

BugAcceptWrongVoteEpoch ==
  ActualGroups("wrong_vote_epoch") = SpecGroups("wrong_vote_epoch")

BugAcceptMissingHighest ==
  ActualGroupCount("missing_highest") = SpecGroupCount("missing_highest")

BugAcceptInvalidCandidatePhase ==
  ActualGroups("invalid_candidate_phase") = SpecGroups("invalid_candidate_phase")

BugRankPrepareOverCommit ==
  ActualSelected("commit_beats_prepare") = SpecSelected("commit_beats_prepare")

BugRankPhaseBeforeView ==
  ActualSelected("higher_view_beats_lower_commit") =
    SpecSelected("higher_view_beats_lower_commit")

BugRankPhaseBeforeHeight ==
  ActualSelected("higher_height_beats_lower_commit") =
    SpecSelected("higher_height_beats_lower_commit")

BugRankLowHash ==
  ActualSelected("hash_tiebreak") = SpecSelected("hash_tiebreak")

BugMergeDistinctReferences ==
  ActualGroupCount("group_distinct_reference") = SpecGroupCount("group_distinct_reference")

BugDuplicateExactNotGrouped ==
  ActualGroupSize("group_duplicate_exact", QDuplicate) =
    SpecGroupSize("group_duplicate_exact", QDuplicate)

====
