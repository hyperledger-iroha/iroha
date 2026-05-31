---- MODULE SumeragiRbcPayloadChunkingGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `chunk_count(...)` and `chunk_payload_bytes(...)`.

The helper pair converts payload bytes into plain RBC chunks. The critical
contract is small but consensus-facing: chunk size zero is clamped to one byte,
empty payloads are represented as one empty chunk, non-empty payloads use
ceiling division, chunk vectors have the same length as the computed count,
and non-empty chunks cover the payload without trailing empty chunks.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  chunk_count_value,
  \* @type: Int;
  chunks_len,
  \* @type: Int;
  first_chunk_len,
  \* @type: Int;
  last_chunk_len,
  \* @type: Int;
  full_chunk_count,
  \* @type: Bool;
  has_empty_chunk

\* @type: <<Str, Int, Int, Int, Int, Int, Bool>>;
vars ==
  <<candidate, chunk_count_value, chunks_len, first_chunk_len,
    last_chunk_len, full_chunk_count, has_empty_chunk>>

Cases == {
  "empty_payload",
  "one_byte_size_four",
  "exact_boundary",
  "two_chunks_exact",
  "over_boundary",
  "multi_chunk",
  "zero_chunk_size",
  "large_chunk_size",
  "unit_chunk_size"
}

CountValues == 0..32

PayloadLen(c) ==
  CASE c = "empty_payload" -> 0
    [] c = "one_byte_size_four" -> 1
    [] c = "exact_boundary" -> 4
    [] c = "two_chunks_exact" -> 8
    [] c = "over_boundary" -> 5
    [] c = "multi_chunk" -> 9
    [] c = "zero_chunk_size" -> 3
    [] c = "large_chunk_size" -> 3
    [] c = "unit_chunk_size" -> 3

ConfiguredChunkSize(c) ==
  CASE c = "zero_chunk_size" -> 0
    [] c = "large_chunk_size" -> 10
    [] c = "unit_chunk_size" -> 1
    [] OTHER -> 4

EffectiveChunkSize(c) ==
  IF ConfiguredChunkSize(c) = 0 THEN 1 ELSE ConfiguredChunkSize(c)

Min(a, b) ==
  IF a <= b THEN a ELSE b

SpecChunkCount(c) ==
  LET len == PayloadLen(c) IN
  LET effective == EffectiveChunkSize(c) IN
    IF len = 0
    THEN 1
    ELSE (len + effective - 1) \div effective

SpecChunksLen(c) ==
  SpecChunkCount(c)

SpecFirstChunkLen(c) ==
  IF PayloadLen(c) = 0
  THEN 0
  ELSE Min(PayloadLen(c), EffectiveChunkSize(c))

SpecLastChunkLen(c) ==
  IF PayloadLen(c) = 0
  THEN 0
  ELSE PayloadLen(c) - ((SpecChunkCount(c) - 1) * EffectiveChunkSize(c))

SpecFullChunkCount(c) ==
  IF PayloadLen(c) = 0
  THEN 0
  ELSE SpecChunkCount(c) - 1

SpecHasEmptyChunk(c) ==
  PayloadLen(c) = 0

ActualChunkCount(c) ==
  CASE Bug = "empty_count_zero" /\ c = "empty_payload" -> 0
    [] Bug = "zero_size_not_clamped" /\ c = "zero_chunk_size" -> 1
    [] Bug = "floor_division" /\ c \in {"over_boundary", "multi_chunk"} ->
         PayloadLen(c) \div EffectiveChunkSize(c)
    [] Bug = "exact_boundary_adds_empty_chunk" /\
       c \in {"exact_boundary", "two_chunks_exact"} -> SpecChunkCount(c) + 1
    [] Bug = "large_chunk_splits_unit" /\ c = "large_chunk_size" -> PayloadLen(c)
    [] OTHER -> SpecChunkCount(c)

ActualChunksLen(c) ==
  CASE Bug = "empty_no_chunk" /\ c = "empty_payload" -> 0
    [] Bug = "chunks_len_mismatch" /\ c # "empty_payload" -> SpecChunkCount(c) + 1
    [] Bug = "exact_boundary_adds_empty_chunk" /\
       c \in {"exact_boundary", "two_chunks_exact"} -> SpecChunkCount(c) + 1
    [] Bug = "zero_size_not_clamped" /\ c = "zero_chunk_size" -> 1
    [] Bug = "large_chunk_splits_unit" /\ c = "large_chunk_size" -> PayloadLen(c)
    [] OTHER -> SpecChunksLen(c)

ActualFirstChunkLen(c) ==
  CASE Bug = "nonempty_first_chunk_empty" /\ PayloadLen(c) # 0 -> 0
    [] Bug = "zero_size_not_clamped" /\ c = "zero_chunk_size" -> PayloadLen(c)
    [] Bug = "large_chunk_splits_unit" /\ c = "large_chunk_size" -> 1
    [] OTHER -> SpecFirstChunkLen(c)

ActualLastChunkLen(c) ==
  CASE Bug = "exact_boundary_adds_empty_chunk" /\
       c \in {"exact_boundary", "two_chunks_exact"} -> 0
    [] Bug = "last_chunk_uses_full_size" /\ c \in {"over_boundary", "multi_chunk"} ->
         EffectiveChunkSize(c)
    [] Bug = "zero_size_not_clamped" /\ c = "zero_chunk_size" -> PayloadLen(c)
    [] Bug = "large_chunk_splits_unit" /\ c = "large_chunk_size" -> 1
    [] OTHER -> SpecLastChunkLen(c)

ActualFullChunkCount(c) ==
  CASE Bug = "full_chunk_count_off_by_one" /\ PayloadLen(c) # 0 -> SpecChunkCount(c)
    [] Bug = "zero_size_not_clamped" /\ c = "zero_chunk_size" -> 0
    [] Bug = "large_chunk_splits_unit" /\ c = "large_chunk_size" -> PayloadLen(c) - 1
    [] OTHER -> SpecFullChunkCount(c)

ActualHasEmptyChunk(c) ==
  CASE Bug = "empty_no_chunk" /\ c = "empty_payload" -> FALSE
    [] Bug = "exact_boundary_adds_empty_chunk" /\
       c \in {"exact_boundary", "two_chunks_exact"} -> TRUE
    [] Bug = "nonempty_first_chunk_empty" /\ PayloadLen(c) # 0 -> TRUE
    [] OTHER -> SpecHasEmptyChunk(c)

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_count_zero",
       "empty_no_chunk",
       "zero_size_not_clamped",
       "floor_division",
       "exact_boundary_adds_empty_chunk",
       "large_chunk_splits_unit",
       "last_chunk_uses_full_size",
       "chunks_len_mismatch",
       "nonempty_first_chunk_empty",
       "full_chunk_count_off_by_one"
     }
  /\ candidate \in Cases
  /\ chunk_count_value \in CountValues
  /\ chunks_len \in CountValues
  /\ first_chunk_len \in CountValues
  /\ last_chunk_len \in CountValues
  /\ full_chunk_count \in CountValues
  /\ has_empty_chunk \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ chunk_count_value = ActualChunkCount(candidate)
  /\ chunks_len = ActualChunksLen(candidate)
  /\ first_chunk_len = ActualFirstChunkLen(candidate)
  /\ last_chunk_len = ActualLastChunkLen(candidate)
  /\ full_chunk_count = ActualFullChunkCount(candidate)
  /\ has_empty_chunk = ActualHasEmptyChunk(candidate)

Next ==
  UNCHANGED vars

ChunkCountMatchesSpec ==
  chunk_count_value = SpecChunkCount(candidate)

ChunkVectorLenMatchesCount ==
  chunks_len = chunk_count_value

ChunkLengthsMatchSpec ==
  /\ first_chunk_len = SpecFirstChunkLen(candidate)
  /\ last_chunk_len = SpecLastChunkLen(candidate)
  /\ full_chunk_count = SpecFullChunkCount(candidate)
  /\ has_empty_chunk = SpecHasEmptyChunk(candidate)

EmptyPayloadSingleEmptyChunk ==
  candidate = "empty_payload" =>
    /\ chunk_count_value = 1
    /\ chunks_len = 1
    /\ first_chunk_len = 0
    /\ last_chunk_len = 0
    /\ full_chunk_count = 0
    /\ has_empty_chunk

ZeroChunkSizeClampedToOne ==
  candidate = "zero_chunk_size" =>
    /\ EffectiveChunkSize(candidate) = 1
    /\ chunk_count_value = PayloadLen(candidate)
    /\ chunks_len = PayloadLen(candidate)
    /\ first_chunk_len = 1
    /\ last_chunk_len = 1

CeilDivisionForNonEmptyPayloads ==
  PayloadLen(candidate) # 0 =>
    chunk_count_value =
      (PayloadLen(candidate) + EffectiveChunkSize(candidate) - 1)
        \div EffectiveChunkSize(candidate)

ExactBoundaryHasNoTrailingEmptyChunk ==
  candidate \in {"exact_boundary", "two_chunks_exact"} =>
    /\ last_chunk_len = EffectiveChunkSize(candidate)
    /\ ~has_empty_chunk

LargeChunkSizeKeepsSingleChunk ==
  candidate = "large_chunk_size" =>
    /\ chunk_count_value = 1
    /\ chunks_len = 1
    /\ first_chunk_len = PayloadLen(candidate)
    /\ last_chunk_len = PayloadLen(candidate)

NonEmptyChunksAreNonEmpty ==
  PayloadLen(candidate) # 0 =>
    /\ first_chunk_len # 0
    /\ last_chunk_len # 0
    /\ ~has_empty_chunk

LastChunkWithinEffectiveSize ==
  PayloadLen(candidate) # 0 =>
    /\ last_chunk_len >= 1
    /\ last_chunk_len <= EffectiveChunkSize(candidate)

FullChunksBeforeLast ==
  PayloadLen(candidate) # 0 =>
    full_chunk_count = chunk_count_value - 1

PayloadCoverageMatchesLength ==
  PayloadLen(candidate) # 0 =>
    full_chunk_count * EffectiveChunkSize(candidate) + last_chunk_len =
      PayloadLen(candidate)

Safety ==
  /\ ChunkCountMatchesSpec
  /\ ChunkVectorLenMatchesCount
  /\ ChunkLengthsMatchSpec
  /\ EmptyPayloadSingleEmptyChunk
  /\ ZeroChunkSizeClampedToOne
  /\ CeilDivisionForNonEmptyPayloads
  /\ ExactBoundaryHasNoTrailingEmptyChunk
  /\ LargeChunkSizeKeepsSingleChunk
  /\ NonEmptyChunksAreNonEmpty
  /\ LastChunkWithinEffectiveSize
  /\ FullChunksBeforeLast
  /\ PayloadCoverageMatchesLength

====
