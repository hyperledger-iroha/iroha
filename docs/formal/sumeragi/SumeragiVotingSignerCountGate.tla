---- MODULE SumeragiVotingSignerCountGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for `voting_signer_count(...)`.

This slice captures the set-based vote support helper from `main_loop.rs`.
It preserves the deterministic contract used by partial NEW_VIEW convergence
and quorum-support checks: the helper counts unique signer indexes only, empty
signer sets and zero-length voting rosters return zero, signer index `0` and
the last valid index count, the first index equal to the voting length and all
larger padding/observer indexes are ignored, and higher in-range indexes remain
eligible for voting support.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "empty_set_zero",
  "zero_roster_zero",
  "signer_zero_counts",
  "single_in_range",
  "multiple_in_range",
  "last_index_included",
  "index_at_len_excluded",
  "out_of_range_ignored",
  "mixed_range",
  "duplicates_collapsed",
  "full_roster_counts_all",
  "higher_in_range_counts",
  "padding_ignored",
  "sparse_high_ignored"
}

VotingLen(c) ==
  CASE c = "empty_set_zero" -> 4
    [] c = "zero_roster_zero" -> 0
    [] c = "signer_zero_counts" -> 3
    [] c = "single_in_range" -> 3
    [] c = "multiple_in_range" -> 5
    [] c = "last_index_included" -> 4
    [] c = "index_at_len_excluded" -> 4
    [] c = "out_of_range_ignored" -> 4
    [] c = "mixed_range" -> 4
    [] c = "duplicates_collapsed" -> 5
    [] c = "full_roster_counts_all" -> 4
    [] c = "higher_in_range_counts" -> 9
    [] c = "padding_ignored" -> 4
    [] c = "sparse_high_ignored" -> 8
    [] OTHER -> 0

UniqueSigners(c) ==
  CASE c = "empty_set_zero" -> {}
    [] c = "zero_roster_zero" -> {0, 1}
    [] c = "signer_zero_counts" -> {0}
    [] c = "single_in_range" -> {1}
    [] c = "multiple_in_range" -> {0, 2, 4}
    [] c = "last_index_included" -> {3}
    [] c = "index_at_len_excluded" -> {4}
    [] c = "out_of_range_ignored" -> {7}
    [] c = "mixed_range" -> {0, 3, 4, 10}
    [] c = "duplicates_collapsed" -> {1, 3}
    [] c = "full_roster_counts_all" -> {0, 1, 2, 3}
    [] c = "higher_in_range_counts" -> {8}
    [] c = "padding_ignored" -> {0, 1, 2, 3, 4, 5}
    [] c = "sparse_high_ignored" -> {2, 8, 31}
    [] OTHER -> {}

OccurrenceCount(c) ==
  CASE c = "duplicates_collapsed" -> 3
    [] OTHER -> Cardinality(UniqueSigners(c))

SpecInRangeSigners(c) ==
  {signer \in UniqueSigners(c): signer < VotingLen(c)}

SpecCount(c) ==
  Cardinality(SpecInRangeSigners(c))

AllUniqueCount(c) ==
  Cardinality(UniqueSigners(c))

DropSignerZeroCount(c) ==
  Cardinality(SpecInRangeSigners(c) \ {0})

DropLastValidCount(c) ==
  Cardinality({signer \in UniqueSigners(c): signer + 1 < VotingLen(c)})

IncludeIndexAtLenCount(c) ==
  Cardinality({signer \in UniqueSigners(c): signer <= VotingLen(c)})

DropHigherIndexesCount(c) ==
  Cardinality({signer \in SpecInRangeSigners(c): signer < 8})

ActualCount(c) ==
  CASE c = "empty_set_zero" /\ Bug = "empty_set_returns_one" ->
      1
    [] c = "zero_roster_zero" /\ Bug = "zero_roster_counts_signers" ->
      AllUniqueCount(c)
    [] c = "signer_zero_counts" /\ Bug = "signer_zero_dropped" ->
      DropSignerZeroCount(c)
    [] c = "multiple_in_range" /\ Bug = "drops_first_signer" ->
      DropSignerZeroCount(c)
    [] c = "last_index_included" /\ Bug = "last_valid_index_excluded" ->
      DropLastValidCount(c)
    [] c = "index_at_len_excluded" /\ Bug = "first_out_of_range_included" ->
      IncludeIndexAtLenCount(c)
    [] c = "out_of_range_ignored" /\ Bug = "out_of_range_counted" ->
      AllUniqueCount(c)
    [] c = "mixed_range" /\ Bug = "mixed_range_counts_out_of_range" ->
      AllUniqueCount(c)
    [] c = "duplicates_collapsed" /\ Bug = "duplicates_counted" ->
      OccurrenceCount(c)
    [] c = "full_roster_counts_all" /\ Bug = "full_roster_drops_last" ->
      DropLastValidCount(c)
    [] c = "higher_in_range_counts" /\ Bug = "higher_in_range_ignored" ->
      DropHigherIndexesCount(c)
    [] c = "padding_ignored" /\ Bug = "padding_counted" ->
      AllUniqueCount(c)
    [] c = "sparse_high_ignored" /\ Bug = "sparse_high_counted" ->
      AllUniqueCount(c)
    [] OTHER ->
      SpecCount(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "empty_set_returns_one",
       "zero_roster_counts_signers",
       "signer_zero_dropped",
       "drops_first_signer",
       "last_valid_index_excluded",
       "first_out_of_range_included",
       "out_of_range_counted",
       "mixed_range_counts_out_of_range",
       "duplicates_counted",
       "full_roster_drops_last",
       "higher_in_range_ignored",
       "padding_counted",
       "sparse_high_counted"
     }
  /\ \A c \in Cases:
       /\ VotingLen(c) \in 0..9
       /\ UniqueSigners(c) \subseteq 0..31
       /\ SpecCount(c) \in 0..6
       /\ ActualCount(c) \in 0..6

SpecCountBounds ==
  \A c \in Cases:
    /\ SpecCount(c) <= VotingLen(c)
    /\ SpecCount(c) <= Cardinality(UniqueSigners(c))

VoteSupportAnchors ==
  /\ SpecCount("empty_set_zero") = 0
  /\ SpecCount("zero_roster_zero") = 0
  /\ SpecCount("signer_zero_counts") = 1
  /\ SpecCount("single_in_range") = 1
  /\ SpecCount("multiple_in_range") = 3
  /\ SpecCount("last_index_included") = 1
  /\ SpecCount("index_at_len_excluded") = 0
  /\ SpecCount("out_of_range_ignored") = 0
  /\ SpecCount("mixed_range") = 2
  /\ SpecCount("duplicates_collapsed") = 2
  /\ SpecCount("full_roster_counts_all") = 4
  /\ SpecCount("higher_in_range_counts") = 1
  /\ SpecCount("padding_ignored") = 4
  /\ SpecCount("sparse_high_ignored") = 1

Safety ==
  \A c \in Cases:
    ActualCount(c) = SpecCount(c)

EmptyAndZeroRosterSupportExact ==
  /\ ActualCount("empty_set_zero") = 0
  /\ ActualCount("zero_roster_zero") = 0

InRangeSupportExact ==
  /\ ActualCount("signer_zero_counts") = 1
  /\ ActualCount("single_in_range") = 1
  /\ ActualCount("multiple_in_range") = 3
  /\ ActualCount("last_index_included") = 1
  /\ ActualCount("full_roster_counts_all") = 4
  /\ ActualCount("higher_in_range_counts") = 1

OutOfRangeSupportFiltered ==
  /\ ActualCount("index_at_len_excluded") = 0
  /\ ActualCount("out_of_range_ignored") = 0
  /\ ActualCount("mixed_range") = 2
  /\ ActualCount("padding_ignored") = 4
  /\ ActualCount("sparse_high_ignored") = 1

DuplicateSupportCollapsed ==
  /\ ActualCount("duplicates_collapsed") = 2
  /\ ActualCount("duplicates_collapsed") < OccurrenceCount("duplicates_collapsed")

VotingSignerSupportCountExactness ==
  /\ Safety
  /\ SpecCountBounds
  /\ VoteSupportAnchors
  /\ EmptyAndZeroRosterSupportExact
  /\ InRangeSupportExact
  /\ OutOfRangeSupportFiltered
  /\ DuplicateSupportCollapsed

VotingSignerSupportCountCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VotingSignerSupportCountExactness

BugEmptySetReturnsOne ==
  ActualCount("empty_set_zero") = SpecCount("empty_set_zero")

BugZeroRosterCountsSigners ==
  ActualCount("zero_roster_zero") = SpecCount("zero_roster_zero")

BugSignerZeroDropped ==
  ActualCount("signer_zero_counts") = SpecCount("signer_zero_counts")

BugDropsFirstSigner ==
  ActualCount("multiple_in_range") = SpecCount("multiple_in_range")

BugLastValidIndexExcluded ==
  ActualCount("last_index_included") = SpecCount("last_index_included")

BugFirstOutOfRangeIncluded ==
  ActualCount("index_at_len_excluded") = SpecCount("index_at_len_excluded")

BugOutOfRangeCounted ==
  ActualCount("out_of_range_ignored") = SpecCount("out_of_range_ignored")

BugMixedRangeCountsOutOfRange ==
  ActualCount("mixed_range") = SpecCount("mixed_range")

BugDuplicatesCounted ==
  ActualCount("duplicates_collapsed") = SpecCount("duplicates_collapsed")

BugFullRosterDropsLast ==
  ActualCount("full_roster_counts_all") = SpecCount("full_roster_counts_all")

BugHigherInRangeIgnored ==
  ActualCount("higher_in_range_counts") = SpecCount("higher_in_range_counts")

BugPaddingCounted ==
  ActualCount("padding_ignored") = SpecCount("padding_ignored")

BugSparseHighCounted ==
  ActualCount("sparse_high_ignored") = SpecCount("sparse_high_ignored")

====
