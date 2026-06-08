---- MODULE SumeragiPacedRetransmitTargetsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `paced_retransmit_targets(...)`.

Pressure scoring and target-limit arithmetic are covered by
`SumeragiRetransmitBackpressureGate.tla`. This slice pins the deterministic
target selection contract after a limit has been chosen:
- zero limits and empty target lists fail closed,
- target lists already within the limit are returned in their original order
  without sorting or deduplication,
- over-limit target lists are sorted and deduplicated before checking whether
  they now fit,
- still-over-limit sorted targets rotate left by the deterministic
  height/view-derived offset modulo the deduplicated length, and
- the rotated list is truncated exactly to the requested limit.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ZeroLimit == "ZeroLimit"
EmptyTargets == "EmptyTargets"
UnderLimitPreservesOrder == "UnderLimitPreservesOrder"
EqualLimitPreservesDuplicates == "EqualLimitPreservesDuplicates"
OverLimitDedupFits == "OverLimitDedupFits"
OverLimitSortsBeforeTruncate == "OverLimitSortsBeforeTruncate"
RotateByOne == "RotateByOne"
RotateByLast == "RotateByLast"
OffsetModulo == "OffsetModulo"
LimitOne == "LimitOne"
HeightOffset == "HeightOffset"
ViewOffset == "ViewOffset"
DedupBeforeSortWouldDiffer == "DedupBeforeSortWouldDiffer"

Cases == {
  ZeroLimit,
  EmptyTargets,
  UnderLimitPreservesOrder,
  EqualLimitPreservesDuplicates,
  OverLimitDedupFits,
  OverLimitSortsBeforeTruncate,
  RotateByOne,
  RotateByLast,
  OffsetModulo,
  LimitOne,
  HeightOffset,
  ViewOffset,
  DedupBeforeSortWouldDiffer
}

FailClosedCases == {
  ZeroLimit,
  EmptyTargets
}

PreCapPreserveCases == {
  UnderLimitPreservesOrder,
  EqualLimitPreservesDuplicates
}

SortDedupCases == {
  OverLimitDedupFits,
  OverLimitSortsBeforeTruncate,
  DedupBeforeSortWouldDiffer
}

RotationOffsetCases == {
  RotateByOne,
  RotateByLast,
  OffsetModulo,
  HeightOffset,
  ViewOffset
}

LimitTruncationCases == {
  OverLimitSortsBeforeTruncate,
  LimitOne
}

Bugs == {
  "none",
  "zero_limit_selects",
  "empty_selects",
  "under_limit_sorts",
  "equal_limit_dedups",
  "over_limit_skips_dedup",
  "over_limit_preserves_input_order",
  "dedup_fits_truncates",
  "rotate_missing",
  "rotate_right",
  "height_offset_ignored",
  "view_offset_ignored",
  "offset_not_modded",
  "truncate_off_by_one",
  "limit_not_applied",
  "sort_descending",
  "dedup_before_sort_keeps_order",
  "empty_when_dedup_equals_limit"
}

\* Output tuple: <<selected_count, first, second, third>>. Zero means absent.
\* @type: <<Int, Int, Int, Int>>;
Empty == <<0, 0, 0, 0>>

TargetLimit(c) ==
  CASE c = ZeroLimit -> 0
    [] c = LimitOne -> 1
    [] c \in PreCapPreserveCases -> 3
    [] OTHER -> 2

\* @type: <<Int, Int, Int, Int>>;
OutSelectPeerOne == <<1, 1, 0, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutUnderLimitSorted == <<2, 1, 3, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutUnderLimitPreserved == <<2, 3, 1, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutEqualLimitPreserved == <<3, 2, 2, 1>>

\* @type: <<Int, Int, Int, Int>>;
OutSortedPair == <<2, 1, 2, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutDuplicatePair == <<2, 1, 1, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutRotatedOne == <<2, 2, 3, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutRotatedLast == <<2, 4, 1, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutOffsetThreeFour == <<2, 3, 4, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutLimitOnePeerThree == <<1, 3, 0, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutFullSortedTriple == <<3, 1, 2, 3>>

\* @type: <<Int, Int, Int, Int>>;
OutDescendingPair == <<2, 3, 2, 0>>

\* @type: <<Int, Int, Int, Int>>;
OutDedupBeforeSortPair == <<2, 2, 1, 0>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecOutput(c) ==
  CASE c = ZeroLimit -> Empty
    [] c = EmptyTargets -> Empty
    [] c = UnderLimitPreservesOrder -> OutUnderLimitPreserved
    [] c = EqualLimitPreservesDuplicates -> OutEqualLimitPreserved
    [] c = OverLimitDedupFits -> OutSortedPair
    [] c = OverLimitSortsBeforeTruncate -> OutSortedPair
    [] c = RotateByOne -> OutRotatedOne
    [] c = RotateByLast -> OutRotatedLast
    [] c = OffsetModulo -> OutOffsetThreeFour
    [] c = LimitOne -> OutLimitOnePeerThree
    [] c = HeightOffset -> OutOffsetThreeFour
    [] c = ViewOffset -> OutRotatedOne
    [] c = DedupBeforeSortWouldDiffer -> OutSortedPair
    [] OTHER -> Empty

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualOutput(c) ==
  CASE Bug = "zero_limit_selects" /\ c = ZeroLimit ->
         OutSelectPeerOne
    [] Bug = "empty_selects" /\ c = EmptyTargets ->
         OutSelectPeerOne
    [] Bug = "under_limit_sorts" /\ c = UnderLimitPreservesOrder ->
         OutUnderLimitSorted
    [] Bug = "equal_limit_dedups" /\ c = EqualLimitPreservesDuplicates ->
         OutSortedPair
    [] Bug = "over_limit_skips_dedup" /\ c = OverLimitDedupFits ->
         OutDuplicatePair
    [] Bug = "over_limit_preserves_input_order" /\ c = OverLimitSortsBeforeTruncate ->
         OutUnderLimitPreserved
    [] Bug = "dedup_fits_truncates" /\ c = OverLimitDedupFits ->
         OutSelectPeerOne
    [] Bug = "rotate_missing" /\ c = RotateByOne ->
         OutSortedPair
    [] Bug = "rotate_right" /\ c = RotateByOne ->
         OutRotatedLast
    [] Bug = "height_offset_ignored" /\ c = HeightOffset ->
         OutSortedPair
    [] Bug = "view_offset_ignored" /\ c = ViewOffset ->
         OutSortedPair
    [] Bug = "offset_not_modded" /\ c = OffsetModulo ->
         Empty
    [] Bug = "truncate_off_by_one" /\ c = LimitOne ->
         OutOffsetThreeFour
    [] Bug = "limit_not_applied" /\ c = OverLimitSortsBeforeTruncate ->
         OutFullSortedTriple
    [] Bug = "sort_descending" /\ c = OverLimitSortsBeforeTruncate ->
         OutDescendingPair
    [] Bug = "dedup_before_sort_keeps_order" /\ c = DedupBeforeSortWouldDiffer ->
         OutDedupBeforeSortPair
    [] Bug = "empty_when_dedup_equals_limit" /\ c = OverLimitDedupFits ->
         Empty
    [] OTHER -> SpecOutput(c)

ActualCount(c) == ActualOutput(c)[1]

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs

SafetyFast ==
  \A c \in Cases: ActualOutput(c) = SpecOutput(c)

PacedRetransmitFailClosedExact ==
  \A c \in FailClosedCases:
    /\ ActualOutput(c) = Empty
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualCount(c) = 0

PacedRetransmitPreCapPreservationExact ==
  \A c \in PreCapPreserveCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualCount(c) <= TargetLimit(c)
    /\ IF c = UnderLimitPreservesOrder THEN
         ActualOutput(c) = OutUnderLimitPreserved
       ELSE TRUE
    /\ IF c = EqualLimitPreservesDuplicates THEN
         ActualOutput(c) = OutEqualLimitPreserved
       ELSE TRUE

PacedRetransmitSortDedupExact ==
  \A c \in SortDedupCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualOutput(c) = OutSortedPair
    /\ ActualCount(c) <= TargetLimit(c)

PacedRetransmitRotationOffsetExact ==
  \A c \in RotationOffsetCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualCount(c) <= TargetLimit(c)
    /\ ActualOutput(c) # Empty
    /\ IF c \in {RotateByOne, ViewOffset} THEN
         ActualOutput(c) = OutRotatedOne
       ELSE TRUE
    /\ IF c = RotateByLast THEN ActualOutput(c) = OutRotatedLast ELSE TRUE
    /\ IF c \in {OffsetModulo, HeightOffset} THEN
         ActualOutput(c) = OutOffsetThreeFour
       ELSE TRUE

PacedRetransmitLimitTruncationExact ==
  \A c \in LimitTruncationCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualCount(c) = TargetLimit(c)
    /\ IF c = LimitOne THEN ActualOutput(c) = OutLimitOnePeerThree ELSE TRUE
    /\ IF c = OverLimitSortsBeforeTruncate THEN
         ActualOutput(c) = OutSortedPair
       ELSE TRUE

PacedRetransmitTargetSelectionExactness ==
  /\ SafetyFast
  /\ PacedRetransmitFailClosedExact
  /\ PacedRetransmitPreCapPreservationExact
  /\ PacedRetransmitSortDedupExact
  /\ PacedRetransmitRotationOffsetExact
  /\ PacedRetransmitLimitTruncationExact

BugZeroLimitSelects ==
  Bug # "zero_limit_selects" \/ OutSelectPeerOne = Empty

BugEmptySelects ==
  Bug # "empty_selects" \/ OutSelectPeerOne = Empty

BugUnderLimitSorts ==
  Bug # "under_limit_sorts" \/ OutUnderLimitSorted = OutUnderLimitPreserved

BugEqualLimitDedups ==
  Bug # "equal_limit_dedups" \/ OutSortedPair = OutEqualLimitPreserved

BugOverLimitSkipsDedup ==
  Bug # "over_limit_skips_dedup" \/ OutDuplicatePair = OutSortedPair

BugOverLimitPreservesInputOrder ==
  Bug # "over_limit_preserves_input_order" \/ OutUnderLimitPreserved = OutSortedPair

BugDedupFitsTruncates ==
  Bug # "dedup_fits_truncates" \/ OutSelectPeerOne = OutSortedPair

BugRotateMissing ==
  Bug # "rotate_missing" \/ OutSortedPair = OutRotatedOne

BugRotateRight ==
  Bug # "rotate_right" \/ OutRotatedLast = OutRotatedOne

BugHeightOffsetIgnored ==
  Bug # "height_offset_ignored" \/ OutSortedPair = OutOffsetThreeFour

BugViewOffsetIgnored ==
  Bug # "view_offset_ignored" \/ OutSortedPair = OutRotatedOne

BugOffsetNotModded ==
  Bug # "offset_not_modded" \/ Empty = OutOffsetThreeFour

BugTruncateOffByOne ==
  Bug # "truncate_off_by_one" \/ OutOffsetThreeFour = OutLimitOnePeerThree

BugLimitNotApplied ==
  Bug # "limit_not_applied" \/ OutFullSortedTriple = OutSortedPair

BugSortDescending ==
  Bug # "sort_descending" \/ OutDescendingPair = OutSortedPair

BugDedupBeforeSortKeepsOrder ==
  Bug # "dedup_before_sort_keeps_order" \/ OutDedupBeforeSortPair = OutSortedPair

BugEmptyWhenDedupEqualsLimit ==
  Bug # "empty_when_dedup_equals_limit" \/ Empty = OutSortedPair

====
