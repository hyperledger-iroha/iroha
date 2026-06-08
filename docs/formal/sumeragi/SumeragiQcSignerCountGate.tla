---- MODULE SumeragiQcSignerCountGate ----
EXTENDS Integers, Sequences

(***************************************************************************
A bounded abstract model for `qc_signer_count(...)`.

This helper reports the raw population count of the QC aggregate
`signers_bitmap`. Unlike bitmap admission, it does not know the roster length
and must count every set bit in every bitmap byte. Callers use that projection
for telemetry, block-sync comparison, and commit-path diagnostics, so malformed
padding bits are still counted rather than interpreted through topology state.
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
  "empty",
  "zero_byte",
  "low_bit",
  "high_bit",
  "full_byte",
  "two_sparse",
  "three_sparse",
  "padding_bits",
  "alternating_pair",
  "two_full_bytes",
  "three_zero_bytes",
  "mixed_three"
}

\* @type: Str => Seq(Int);
Bitmap(c) ==
  CASE c = "empty" -> <<>>
    [] c = "zero_byte" -> <<0>>
    [] c = "low_bit" -> <<1>>
    [] c = "high_bit" -> <<128>>
    [] c = "full_byte" -> <<255>>
    [] c = "two_sparse" -> <<3, 5>>
    [] c = "three_sparse" -> <<1, 2, 4>>
    [] c = "padding_bits" -> <<240>>
    [] c = "alternating_pair" -> <<170, 85>>
    [] c = "two_full_bytes" -> <<255, 255>>
    [] c = "three_zero_bytes" -> <<0, 0, 0>>
    [] c = "mixed_three" -> <<15, 0, 240>>
    [] OTHER -> <<>>

\* @type: Int => Int;
BytePopCount(byte) ==
  CASE byte = 0 -> 0
    [] byte = 1 -> 1
    [] byte = 2 -> 1
    [] byte = 3 -> 2
    [] byte = 4 -> 1
    [] byte = 5 -> 2
    [] byte = 15 -> 4
    [] byte = 85 -> 4
    [] byte = 128 -> 1
    [] byte = 170 -> 4
    [] byte = 240 -> 4
    [] byte = 255 -> 8
    [] OTHER -> 0

\* @type: Seq(Int) => Int;
BitCount(bitmap) ==
  (IF Len(bitmap) >= 1 THEN BytePopCount(bitmap[1]) ELSE 0) +
  (IF Len(bitmap) >= 2 THEN BytePopCount(bitmap[2]) ELSE 0) +
  (IF Len(bitmap) >= 3 THEN BytePopCount(bitmap[3]) ELSE 0)

\* @type: Str => Int;
SpecCount(c) ==
  BitCount(Bitmap(c))

\* @type: Str => Int;
ActualCount(c) ==
  CASE Bug = "empty_counts_one"
       /\ c = "empty" -> 1
    [] Bug = "zero_byte_counts_one"
       /\ c = "zero_byte" -> 1
    [] Bug = "low_bit_dropped"
       /\ c = "low_bit" -> 0
    [] Bug = "high_bit_ignored"
       /\ c = "high_bit" -> 0
    [] Bug = "full_byte_counts_one"
       /\ c = "full_byte" -> 1
    [] Bug = "counts_byte_values"
       /\ c = "two_sparse" -> 8
    [] Bug = "second_byte_ignored"
       /\ c = "two_sparse" -> BytePopCount(Bitmap(c)[1])
    [] Bug = "third_byte_ignored"
       /\ c = "three_sparse" ->
         BytePopCount(Bitmap(c)[1]) + BytePopCount(Bitmap(c)[2])
    [] Bug = "padding_bits_ignored"
       /\ c = "padding_bits" -> 0
    [] Bug = "alternating_pair_collapsed"
       /\ c = "alternating_pair" -> 2
    [] Bug = "two_full_bytes_saturates"
       /\ c = "two_full_bytes" -> 8
    [] Bug = "zero_bytes_add_length"
       /\ c = "three_zero_bytes" -> Len(Bitmap(c))
    [] Bug = "counts_nonzero_bytes"
       /\ c = "mixed_three" -> 2
    [] OTHER -> SpecCount(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "empty_counts_one",
       "zero_byte_counts_one",
       "low_bit_dropped",
       "high_bit_ignored",
       "full_byte_counts_one",
       "counts_byte_values",
       "second_byte_ignored",
       "third_byte_ignored",
       "padding_bits_ignored",
       "alternating_pair_collapsed",
       "two_full_bytes_saturates",
       "zero_bytes_add_length",
       "counts_nonzero_bytes"
     }
  /\ \A c \in Cases:
       /\ Len(Bitmap(c)) \in 0..3
       /\ SpecCount(c) \in 0..24
       /\ ActualCount(c) \in 0..255

CountWithinBitmapWidth ==
  \A c \in Cases: SpecCount(c) <= Len(Bitmap(c)) * 8

RawCountAnchors ==
  /\ SpecCount("empty") = 0
  /\ SpecCount("zero_byte") = 0
  /\ SpecCount("low_bit") = 1
  /\ SpecCount("high_bit") = 1
  /\ SpecCount("full_byte") = 8
  /\ SpecCount("two_sparse") = 4
  /\ SpecCount("three_sparse") = 3
  /\ SpecCount("padding_bits") = 4
  /\ SpecCount("alternating_pair") = 8
  /\ SpecCount("two_full_bytes") = 16
  /\ SpecCount("three_zero_bytes") = 0
  /\ SpecCount("mixed_three") = 8

SafetyFast ==
  \A c \in Cases: ActualCount(c) = SpecCount(c)

EmptyAndZeroByteCountExact ==
  /\ ActualCount("empty") = SpecCount("empty")
  /\ ActualCount("zero_byte") = SpecCount("zero_byte")
  /\ ActualCount("three_zero_bytes") = SpecCount("three_zero_bytes")

SingleBitCountExact ==
  /\ ActualCount("low_bit") = SpecCount("low_bit")
  /\ ActualCount("high_bit") = SpecCount("high_bit")

FullByteAndPaddingCountExact ==
  /\ ActualCount("full_byte") = SpecCount("full_byte")
  /\ ActualCount("padding_bits") = SpecCount("padding_bits")

MultiByteCountExact ==
  /\ ActualCount("two_sparse") = SpecCount("two_sparse")
  /\ ActualCount("three_sparse") = SpecCount("three_sparse")
  /\ ActualCount("alternating_pair") = SpecCount("alternating_pair")
  /\ ActualCount("two_full_bytes") = SpecCount("two_full_bytes")
  /\ ActualCount("mixed_three") = SpecCount("mixed_three")

QcSignerRawCountExactness ==
  /\ CountWithinBitmapWidth
  /\ RawCountAnchors
  /\ SafetyFast
  /\ EmptyAndZeroByteCountExact
  /\ SingleBitCountExact
  /\ FullByteAndPaddingCountExact
  /\ MultiByteCountExact

BugEmptyCountsOne ==
  ActualCount("empty") = SpecCount("empty")

BugZeroByteCountsOne ==
  ActualCount("zero_byte") = SpecCount("zero_byte")

BugLowBitDropped ==
  ActualCount("low_bit") = SpecCount("low_bit")

BugHighBitIgnored ==
  ActualCount("high_bit") = SpecCount("high_bit")

BugFullByteCountsOne ==
  ActualCount("full_byte") = SpecCount("full_byte")

BugCountsByteValues ==
  ActualCount("two_sparse") = SpecCount("two_sparse")

BugSecondByteIgnored ==
  ActualCount("two_sparse") = SpecCount("two_sparse")

BugThirdByteIgnored ==
  ActualCount("three_sparse") = SpecCount("three_sparse")

BugPaddingBitsIgnored ==
  ActualCount("padding_bits") = SpecCount("padding_bits")

BugAlternatingPairCollapsed ==
  ActualCount("alternating_pair") = SpecCount("alternating_pair")

BugTwoFullBytesSaturates ==
  ActualCount("two_full_bytes") = SpecCount("two_full_bytes")

BugZeroBytesAddLength ==
  ActualCount("three_zero_bytes") = SpecCount("three_zero_bytes")

BugCountsNonzeroBytes ==
  ActualCount("mixed_three") = SpecCount("mixed_three")

====
