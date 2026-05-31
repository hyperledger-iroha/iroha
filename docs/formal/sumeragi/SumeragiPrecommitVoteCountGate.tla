---- MODULE SumeragiPrecommitVoteCountGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for precommit vote counting.

This slice captures `precommit_vote_count(...)` and its
`qc_voting_signer_count(...)` delegate from `main_loop.rs`. It abstracts away
concrete bitmap bytes while preserving the deterministic contract: only
commit-phase QCs contribute precommit vote progress, non-commit phases return
zero even if their bitmap has set bits, empty bitmaps and zero-length rosters
return zero, set bits are counted only when their decoded signer index is less
than the supplied roster length, the last valid index is included while the
first out-of-range index is excluded, all bitmap bytes are scanned, bits rather
than bytes are counted, and accumulation uses saturating addition.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PreparePhaseZero == 1
NewViewPhaseZero == 2
CommitEmptyBitmapZero == 3
CommitZeroRosterZero == 4
CommitOneInRange == 5
CommitMultipleInRange == 6
CommitOutOfRangeIgnored == 7
CommitMixedRange == 8
CommitLastIndexIncluded == 9
CommitIndexAtLenExcluded == 10
CommitScansSecondByte == 11
CommitCountsBitsNotBytes == 12

Candidates == 1..12

CheckCommitPhase == 1
RejectNonCommitPhase == 2
CountBitmap == 3
CountSetBits == 4
CountBytes == 5
UseRosterLen == 6
CountInRangeOnly == 7
IgnoreOutOfRange == 8
CountOutOfRange == 9
IncludeBoundaryLast == 10
ExcludeBoundaryLen == 11
ScanAllBytes == 12
SaturatingAdd == 13
ReturnZero == 14
ReturnCount == 15

Actions == 1..15

CommitCountBase ==
  {CheckCommitPhase, CountBitmap, CountSetBits, UseRosterLen,
   CountInRangeOnly, ReturnCount}

SpecActions(candidate) ==
  CASE candidate = PreparePhaseZero ->
      {CheckCommitPhase, RejectNonCommitPhase, ReturnZero}
    [] candidate = NewViewPhaseZero ->
      {CheckCommitPhase, RejectNonCommitPhase, ReturnZero}
    [] candidate = CommitEmptyBitmapZero ->
      {CheckCommitPhase, CountBitmap, ReturnZero}
    [] candidate = CommitZeroRosterZero ->
      {CheckCommitPhase, CountBitmap, UseRosterLen, CountInRangeOnly,
       ReturnZero}
    [] candidate = CommitOneInRange ->
      CommitCountBase
    [] candidate = CommitMultipleInRange ->
      CommitCountBase \cup {SaturatingAdd}
    [] candidate = CommitOutOfRangeIgnored ->
      {CheckCommitPhase, CountBitmap, UseRosterLen, CountInRangeOnly,
       IgnoreOutOfRange, ReturnZero}
    [] candidate = CommitMixedRange ->
      CommitCountBase \cup {IgnoreOutOfRange}
    [] candidate = CommitLastIndexIncluded ->
      CommitCountBase \cup {IncludeBoundaryLast}
    [] candidate = CommitIndexAtLenExcluded ->
      {CheckCommitPhase, CountBitmap, UseRosterLen, CountInRangeOnly,
       ExcludeBoundaryLen, IgnoreOutOfRange, ReturnZero}
    [] candidate = CommitScansSecondByte ->
      CommitCountBase \cup {ScanAllBytes}
    [] candidate = CommitCountsBitsNotBytes ->
      CommitCountBase
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = PreparePhaseZero /\ Bug = "prepare_counts_bitmap" ->
      (spec \ {RejectNonCommitPhase, ReturnZero}) \cup
        {CountBitmap, ReturnCount}
    [] candidate = NewViewPhaseZero /\ Bug = "new_view_counts_bitmap" ->
      (spec \ {RejectNonCommitPhase, ReturnZero}) \cup
        {CountBitmap, ReturnCount}
    [] candidate = CommitEmptyBitmapZero /\ Bug = "empty_bitmap_returns_one" ->
      (spec \ {ReturnZero}) \cup {ReturnCount}
    [] candidate = CommitZeroRosterZero /\ Bug = "zero_roster_counts_bits" ->
      (spec \ {CountInRangeOnly, ReturnZero}) \cup {ReturnCount}
    [] candidate = CommitOneInRange /\ Bug = "commit_returns_zero" ->
      (spec \ {ReturnCount}) \cup {ReturnZero}
    [] candidate = CommitMultipleInRange /\ Bug = "drops_saturating_add" ->
      spec \ {SaturatingAdd}
    [] candidate = CommitOutOfRangeIgnored /\
          Bug = "out_of_range_counted" ->
      (spec \ {CountInRangeOnly, IgnoreOutOfRange, ReturnZero}) \cup
        {CountOutOfRange, ReturnCount}
    [] candidate = CommitMixedRange /\
          Bug = "mixed_range_counts_out_of_range" ->
      (spec \ {IgnoreOutOfRange}) \cup {CountOutOfRange}
    [] candidate = CommitLastIndexIncluded /\
          Bug = "last_valid_index_excluded" ->
      (spec \ {IncludeBoundaryLast, ReturnCount}) \cup {ReturnZero}
    [] candidate = CommitIndexAtLenExcluded /\
          Bug = "first_out_of_range_included" ->
      (spec \ {ExcludeBoundaryLen, IgnoreOutOfRange, ReturnZero}) \cup
        {CountOutOfRange, ReturnCount}
    [] candidate = CommitScansSecondByte /\ Bug = "second_byte_ignored" ->
      (spec \ {ScanAllBytes, ReturnCount}) \cup {ReturnZero}
    [] candidate = CommitCountsBitsNotBytes /\ Bug = "counts_bytes_not_bits" ->
      (spec \ {CountSetBits}) \cup {CountBytes}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "prepare_counts_bitmap",
       "new_view_counts_bitmap",
       "empty_bitmap_returns_one",
       "zero_roster_counts_bits",
       "commit_returns_zero",
       "drops_saturating_add",
       "out_of_range_counted",
       "mixed_range_counts_out_of_range",
       "last_valid_index_excluded",
       "first_out_of_range_included",
       "second_byte_ignored",
       "counts_bytes_not_bits"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

ZeroReturnCandidates ==
  {PreparePhaseZero, NewViewPhaseZero, CommitEmptyBitmapZero,
   CommitZeroRosterZero, CommitOutOfRangeIgnored, CommitIndexAtLenExcluded}

CountReturnCandidates ==
  {CommitOneInRange, CommitMultipleInRange, CommitMixedRange,
   CommitLastIndexIncluded, CommitScansSecondByte, CommitCountsBitsNotBytes}

SpecReturnClassification ==
  /\ \A c \in ZeroReturnCandidates:
       /\ ReturnZero \in SpecActions(c)
       /\ ReturnCount \notin SpecActions(c)
  /\ \A c \in CountReturnCandidates:
       /\ ReturnCount \in SpecActions(c)
       /\ ReturnZero \notin SpecActions(c)

SpecPhaseGateAnchors ==
  /\ RejectNonCommitPhase \in SpecActions(PreparePhaseZero)
  /\ RejectNonCommitPhase \in SpecActions(NewViewPhaseZero)
  /\ CountBitmap \notin SpecActions(PreparePhaseZero)
  /\ CountBitmap \notin SpecActions(NewViewPhaseZero)
  /\ \A c \in Candidates \ {PreparePhaseZero, NewViewPhaseZero}:
       /\ CheckCommitPhase \in SpecActions(c)
       /\ RejectNonCommitPhase \notin SpecActions(c)

SpecBitmapBoundaryAnchors ==
  /\ SaturatingAdd \in SpecActions(CommitMultipleInRange)
  /\ IgnoreOutOfRange \in SpecActions(CommitOutOfRangeIgnored)
  /\ IgnoreOutOfRange \in SpecActions(CommitMixedRange)
  /\ IncludeBoundaryLast \in SpecActions(CommitLastIndexIncluded)
  /\ ExcludeBoundaryLen \in SpecActions(CommitIndexAtLenExcluded)
  /\ ScanAllBytes \in SpecActions(CommitScansSecondByte)
  /\ CountSetBits \in SpecActions(CommitCountsBitsNotBytes)
  /\ CountBytes \notin SpecActions(CommitCountsBitsNotBytes)

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BugPrepareCountsBitmap ==
  ImplementationActions(PreparePhaseZero) = SpecActions(PreparePhaseZero)

BugNewViewCountsBitmap ==
  ImplementationActions(NewViewPhaseZero) = SpecActions(NewViewPhaseZero)

BugEmptyBitmapReturnsOne ==
  ImplementationActions(CommitEmptyBitmapZero) =
    SpecActions(CommitEmptyBitmapZero)

BugZeroRosterCountsBits ==
  ImplementationActions(CommitZeroRosterZero) =
    SpecActions(CommitZeroRosterZero)

BugCommitReturnsZero ==
  ImplementationActions(CommitOneInRange) = SpecActions(CommitOneInRange)

BugDropsSaturatingAdd ==
  ImplementationActions(CommitMultipleInRange) =
    SpecActions(CommitMultipleInRange)

BugOutOfRangeCounted ==
  ImplementationActions(CommitOutOfRangeIgnored) =
    SpecActions(CommitOutOfRangeIgnored)

BugMixedRangeCountsOutOfRange ==
  ImplementationActions(CommitMixedRange) = SpecActions(CommitMixedRange)

BugLastValidIndexExcluded ==
  ImplementationActions(CommitLastIndexIncluded) =
    SpecActions(CommitLastIndexIncluded)

BugFirstOutOfRangeIncluded ==
  ImplementationActions(CommitIndexAtLenExcluded) =
    SpecActions(CommitIndexAtLenExcluded)

BugSecondByteIgnored ==
  ImplementationActions(CommitScansSecondByte) =
    SpecActions(CommitScansSecondByte)

BugCountsBytesNotBits ==
  ImplementationActions(CommitCountsBitsNotBytes) =
    SpecActions(CommitCountsBitsNotBytes)

====
