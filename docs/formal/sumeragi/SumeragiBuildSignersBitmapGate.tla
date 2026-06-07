---- MODULE SumeragiBuildSignersBitmapGate ----
EXTENDS Integers, Sequences

(***************************************************************************
A bounded abstract model for `build_signers_bitmap(...)`.

This slice captures the QC signer-bitmap encoder from `main_loop.rs`. It
preserves the deterministic contract used when rebuilding and emitting QCs:
zero-length rosters produce an empty bitmap, non-empty rosters allocate exactly
`ceil(roster_len / 8)` bytes, signer indexes are interpreted as little-endian
bits within each byte, in-range signers are ORed into the bitmap, duplicate
observations collapse through the `BTreeSet` input, and out-of-roster/padding
indexes never set bits or extend the bitmap.
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
  "zero_roster_empty",
  "empty_signers_allocates_byte",
  "signer_zero_sets_low_bit",
  "single_middle_bit",
  "multiple_bits_or",
  "last_valid_bit_included",
  "index_at_len_ignored",
  "out_of_range_ignored",
  "mixed_range_filters",
  "byte_boundary_seven",
  "byte_boundary_eight",
  "second_byte_multiple",
  "roster_len_ceil_two_bytes",
  "padding_ignored_in_last_byte",
  "duplicates_collapsed",
  "full_roster_len_eight",
  "full_roster_len_nine"
}

RosterLen(c) ==
  CASE c = "zero_roster_empty" -> 0
    [] c = "empty_signers_allocates_byte" -> 6
    [] c = "signer_zero_sets_low_bit" -> 1
    [] c = "single_middle_bit" -> 6
    [] c = "multiple_bits_or" -> 6
    [] c = "last_valid_bit_included" -> 6
    [] c = "index_at_len_ignored" -> 6
    [] c = "out_of_range_ignored" -> 6
    [] c = "mixed_range_filters" -> 6
    [] c = "byte_boundary_seven" -> 8
    [] c = "byte_boundary_eight" -> 9
    [] c = "second_byte_multiple" -> 16
    [] c = "roster_len_ceil_two_bytes" -> 9
    [] c = "padding_ignored_in_last_byte" -> 9
    [] c = "duplicates_collapsed" -> 8
    [] c = "full_roster_len_eight" -> 8
    [] c = "full_roster_len_nine" -> 9
    [] OTHER -> 0

\* @type: Str => Set(Int);
Signers(c) ==
  CASE c = "zero_roster_empty" -> {0, 1}
    [] c = "empty_signers_allocates_byte" -> {}
    [] c = "signer_zero_sets_low_bit" -> {0}
    [] c = "single_middle_bit" -> {3}
    [] c = "multiple_bits_or" -> {0, 3, 5}
    [] c = "last_valid_bit_included" -> {5}
    [] c = "index_at_len_ignored" -> {6}
    [] c = "out_of_range_ignored" -> {9}
    [] c = "mixed_range_filters" -> {1, 5, 6, 9}
    [] c = "byte_boundary_seven" -> {7}
    [] c = "byte_boundary_eight" -> {8}
    [] c = "second_byte_multiple" -> {8, 10, 15}
    [] c = "roster_len_ceil_two_bytes" -> {}
    [] c = "padding_ignored_in_last_byte" -> {8, 9, 15}
    [] c = "duplicates_collapsed" -> {2, 5}
    [] c = "full_roster_len_eight" -> {0, 1, 2, 3, 4, 5, 6, 7}
    [] c = "full_roster_len_nine" -> {0, 1, 2, 3, 4, 5, 6, 7, 8}
    [] OTHER -> {}

BitmapLen(roster_len) ==
  IF roster_len = 0 THEN 0 ELSE ((roster_len - 1) \div 8) + 1

BitValue(c, idx, value) ==
  IF idx \in Signers(c) /\ idx < RosterLen(c) THEN value ELSE 0

ByteValue(c, base) ==
  BitValue(c, base + 0, 1) +
  BitValue(c, base + 1, 2) +
  BitValue(c, base + 2, 4) +
  BitValue(c, base + 3, 8) +
  BitValue(c, base + 4, 16) +
  BitValue(c, base + 5, 32) +
  BitValue(c, base + 6, 64) +
  BitValue(c, base + 7, 128)

\* @type: Str => Seq(Int);
SpecBitmap(c) ==
  CASE BitmapLen(RosterLen(c)) = 0 -> <<>>
    [] BitmapLen(RosterLen(c)) = 1 -> <<ByteValue(c, 0)>>
    [] BitmapLen(RosterLen(c)) = 2 -> <<ByteValue(c, 0), ByteValue(c, 8)>>
    [] OTHER -> <<>>

\* @type: Str => Seq(Int);
ActualBitmap(c) ==
  CASE c = "zero_roster_empty" /\ Bug = "zero_roster_allocates_byte" ->
      <<0>>
    [] c = "empty_signers_allocates_byte" /\ Bug = "empty_signers_returns_empty" ->
      <<>>
    [] c = "roster_len_ceil_two_bytes" /\ Bug = "length_uses_floor_div" ->
      <<0>>
    [] c = "signer_zero_sets_low_bit" /\ Bug = "signer_zero_dropped" ->
      <<0>>
    [] c = "single_middle_bit" /\ Bug = "middle_bit_shift_wrong" ->
      <<4>>
    [] c = "multiple_bits_or" /\ Bug = "multiple_bits_overwrite" ->
      <<32>>
    [] c = "last_valid_bit_included" /\ Bug = "last_valid_index_excluded" ->
      <<0>>
    [] c = "index_at_len_ignored" /\ Bug = "first_out_of_range_included" ->
      <<64>>
    [] c = "out_of_range_ignored" /\ Bug = "out_of_range_extends_bitmap" ->
      <<0, 2>>
    [] c = "mixed_range_filters" /\ Bug = "mixed_range_counts_padding" ->
      <<98, 2>>
    [] c = "byte_boundary_seven" /\ Bug = "byte_seven_big_endian" ->
      <<1>>
    [] c = "byte_boundary_eight" /\ Bug = "byte_eight_stays_first_byte" ->
      <<1, 0>>
    [] c = "second_byte_multiple" /\ Bug = "second_byte_ignored" ->
      <<0, 0>>
    [] c = "padding_ignored_in_last_byte" /\ Bug = "padding_in_last_byte_counted" ->
      <<0, 131>>
    [] c = "duplicates_collapsed" /\ Bug = "duplicate_observation_toggles_bit" ->
      <<32>>
    [] c = "full_roster_len_eight" /\ Bug = "full_roster_drops_first" ->
      <<254>>
    [] c = "full_roster_len_nine" /\ Bug = "full_roster_drops_second_byte" ->
      <<255, 0>>
    [] OTHER ->
      SpecBitmap(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "zero_roster_allocates_byte",
       "empty_signers_returns_empty",
       "length_uses_floor_div",
       "signer_zero_dropped",
       "middle_bit_shift_wrong",
       "multiple_bits_overwrite",
       "last_valid_index_excluded",
       "first_out_of_range_included",
       "out_of_range_extends_bitmap",
       "mixed_range_counts_padding",
       "byte_seven_big_endian",
       "byte_eight_stays_first_byte",
       "second_byte_ignored",
       "padding_in_last_byte_counted",
       "duplicate_observation_toggles_bit",
       "full_roster_drops_first",
       "full_roster_drops_second_byte"
     }
  /\ \A c \in Cases:
       /\ RosterLen(c) \in 0..16
       /\ Signers(c) \subseteq 0..31
       /\ Len(SpecBitmap(c)) \in 0..2
       /\ Len(ActualBitmap(c)) \in 0..2
       /\ \A i \in 1..Len(SpecBitmap(c)):
            SpecBitmap(c)[i] \in 0..255
       /\ \A i \in 1..Len(ActualBitmap(c)):
            ActualBitmap(c)[i] \in 0..255

Safety ==
  \A c \in Cases:
    ActualBitmap(c) = SpecBitmap(c)

BitmapLengthExact ==
  \A c \in Cases:
    Len(ActualBitmap(c)) = BitmapLen(RosterLen(c))

ZeroRosterAndAllocationExact ==
  /\ ActualBitmap("zero_roster_empty") = <<>>
  /\ ActualBitmap("empty_signers_allocates_byte") = <<0>>
  /\ ActualBitmap("roster_len_ceil_two_bytes") = <<0, 0>>

SingleSignerBitExact ==
  /\ ActualBitmap("signer_zero_sets_low_bit") = <<1>>
  /\ ActualBitmap("single_middle_bit") = <<8>>
  /\ ActualBitmap("last_valid_bit_included") = <<32>>
  /\ ActualBitmap("byte_boundary_seven") = <<128>>
  /\ ActualBitmap("byte_boundary_eight") = <<0, 1>>

MultiSignerOrExact ==
  /\ ActualBitmap("multiple_bits_or") = <<41>>
  /\ ActualBitmap("second_byte_multiple") = <<0, 133>>
  /\ ActualBitmap("full_roster_len_eight") = <<255>>
  /\ ActualBitmap("full_roster_len_nine") = <<255, 1>>

OutOfRangeAndPaddingFiltered ==
  /\ ActualBitmap("index_at_len_ignored") = <<0>>
  /\ ActualBitmap("out_of_range_ignored") = <<0>>
  /\ ActualBitmap("mixed_range_filters") = <<34>>
  /\ ActualBitmap("padding_ignored_in_last_byte") = <<0, 1>>

DuplicateSignerCollapsedExact ==
  ActualBitmap("duplicates_collapsed") = <<36>>

BuildSignersBitmapExactness ==
  /\ Safety
  /\ BitmapLengthExact
  /\ ZeroRosterAndAllocationExact
  /\ SingleSignerBitExact
  /\ MultiSignerOrExact
  /\ OutOfRangeAndPaddingFiltered
  /\ DuplicateSignerCollapsedExact

BugZeroRosterAllocatesByte ==
  ActualBitmap("zero_roster_empty") = SpecBitmap("zero_roster_empty")

BugEmptySignersReturnsEmpty ==
  ActualBitmap("empty_signers_allocates_byte") =
    SpecBitmap("empty_signers_allocates_byte")

BugLengthUsesFloorDiv ==
  ActualBitmap("roster_len_ceil_two_bytes") =
    SpecBitmap("roster_len_ceil_two_bytes")

BugSignerZeroDropped ==
  ActualBitmap("signer_zero_sets_low_bit") =
    SpecBitmap("signer_zero_sets_low_bit")

BugMiddleBitShiftWrong ==
  ActualBitmap("single_middle_bit") = SpecBitmap("single_middle_bit")

BugMultipleBitsOverwrite ==
  ActualBitmap("multiple_bits_or") = SpecBitmap("multiple_bits_or")

BugLastValidIndexExcluded ==
  ActualBitmap("last_valid_bit_included") =
    SpecBitmap("last_valid_bit_included")

BugFirstOutOfRangeIncluded ==
  ActualBitmap("index_at_len_ignored") = SpecBitmap("index_at_len_ignored")

BugOutOfRangeExtendsBitmap ==
  ActualBitmap("out_of_range_ignored") = SpecBitmap("out_of_range_ignored")

BugMixedRangeCountsPadding ==
  ActualBitmap("mixed_range_filters") = SpecBitmap("mixed_range_filters")

BugByteSevenBigEndian ==
  ActualBitmap("byte_boundary_seven") = SpecBitmap("byte_boundary_seven")

BugByteEightStaysFirstByte ==
  ActualBitmap("byte_boundary_eight") = SpecBitmap("byte_boundary_eight")

BugSecondByteIgnored ==
  ActualBitmap("second_byte_multiple") = SpecBitmap("second_byte_multiple")

BugPaddingInLastByteCounted ==
  ActualBitmap("padding_ignored_in_last_byte") =
    SpecBitmap("padding_ignored_in_last_byte")

BugDuplicateObservationTogglesBit ==
  ActualBitmap("duplicates_collapsed") = SpecBitmap("duplicates_collapsed")

BugFullRosterDropsFirst ==
  ActualBitmap("full_roster_len_eight") = SpecBitmap("full_roster_len_eight")

BugFullRosterDropsSecondByte ==
  ActualBitmap("full_roster_len_nine") = SpecBitmap("full_roster_len_nine")

====
