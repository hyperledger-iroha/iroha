---- MODULE SumeragiRbcDeliverAcceptanceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `evaluate_deliver_acceptance_with_policy(...)`.

The helper accepts an RBC DELIVER only after any required READY quorum is
present, the chunk shape is possible, required chunks are present unless the
caller explicitly allows missing chunks, and a present expected chunk root
matches the computed root. Deferral order matters: READY-quorum deferral must
win over malformed chunk-shape rejection and chunk deferral.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  decision

\* @type: <<Str, Str>>;
vars == <<candidate, decision>>

Cases == {
  "accept_full",
  "defer_ready",
  "zero_required_ready_accepts",
  "defer_chunks",
  "allow_missing_chunks_accepts",
  "allow_missing_overcount_invalid",
  "invalid_chunk_root",
  "absent_expected_root_accepts",
  "absent_computed_root_accepts",
  "ready_precedes_chunks",
  "ready_precedes_invalid_shape",
  "zero_total_invalid",
  "overcount_invalid"
}

DecisionValues == {
  "accept",
  "defer_ready_0_0",
  "defer_ready_0_1",
  "defer_ready_1_2",
  "defer_chunks_0_0",
  "defer_chunks_0_2",
  "defer_chunks_1_2",
  "invalid_shape_0_0",
  "invalid_shape_2_1",
  "invalid_root"
}

ReadyCount(c) ==
  CASE c = "defer_ready" -> 0
    [] c = "zero_required_ready_accepts" -> 0
    [] c = "ready_precedes_chunks" -> 0
    [] c = "ready_precedes_invalid_shape" -> 0
    [] OTHER -> 1

RequiredReady(c) ==
  CASE c = "zero_required_ready_accepts" -> 0
    [] OTHER -> 1

ReceivedChunks(c) ==
  CASE c = "defer_chunks" -> 1
    [] c = "allow_missing_chunks_accepts" -> 1
    [] c = "allow_missing_overcount_invalid" -> 2
    [] c = "ready_precedes_chunks" -> 0
    [] c = "ready_precedes_invalid_shape" -> 2
    [] c = "zero_total_invalid" -> 0
    [] c = "overcount_invalid" -> 2
    [] OTHER -> 1

TotalChunks(c) ==
  CASE c = "defer_chunks" -> 2
    [] c = "allow_missing_chunks_accepts" -> 2
    [] c = "allow_missing_overcount_invalid" -> 1
    [] c = "ready_precedes_chunks" -> 2
    [] c = "ready_precedes_invalid_shape" -> 1
    [] c = "zero_total_invalid" -> 0
    [] c = "overcount_invalid" -> 1
    [] OTHER -> 1

AllowMissingChunks(c) ==
  c \in {"allow_missing_chunks_accepts", "allow_missing_overcount_invalid"}

ExpectedRootPresent(c) ==
  c # "absent_expected_root_accepts"

ComputedRootPresent(c) ==
  c # "absent_computed_root_accepts"

RootMatches(c) ==
  c # "invalid_chunk_root"

ReadyBlocks(c) ==
  RequiredReady(c) # 0 /\ ReadyCount(c) < RequiredReady(c)

InvalidShape(c) ==
  \/ TotalChunks(c) = 0
  \/ ReceivedChunks(c) > TotalChunks(c)

ChunkBlocks(c) ==
  /\ ~InvalidShape(c)
  /\ ~AllowMissingChunks(c)
  /\ ReceivedChunks(c) < TotalChunks(c)

RootBlocks(c) ==
  /\ ExpectedRootPresent(c)
  /\ ComputedRootPresent(c)
  /\ ~RootMatches(c)

ReadyDecision(count, required) ==
  CASE count = 0 /\ required = 0 -> "defer_ready_0_0"
    [] count = 0 /\ required = 1 -> "defer_ready_0_1"
    [] OTHER -> "defer_ready_1_2"

ChunkDecision(received, total) ==
  CASE received = 0 /\ total = 0 -> "defer_chunks_0_0"
    [] received = 0 /\ total = 2 -> "defer_chunks_0_2"
    [] OTHER -> "defer_chunks_1_2"

ShapeDecision(received, total) ==
  CASE received = 0 /\ total = 0 -> "invalid_shape_0_0"
    [] OTHER -> "invalid_shape_2_1"

SpecDecision(c) ==
  IF ReadyBlocks(c)
  THEN ReadyDecision(ReadyCount(c), RequiredReady(c))
  ELSE
    IF InvalidShape(c)
    THEN ShapeDecision(ReceivedChunks(c), TotalChunks(c))
    ELSE
      IF ChunkBlocks(c)
      THEN ChunkDecision(ReceivedChunks(c), TotalChunks(c))
      ELSE IF RootBlocks(c) THEN "invalid_root" ELSE "accept"

ActualReadyBlocks(c) ==
  CASE Bug = "ignore_ready_quorum" -> FALSE
    [] Bug = "require_ready_when_zero" -> ReadyCount(c) = 0
    [] OTHER -> ReadyBlocks(c)

ActualShapeBlocks(c) ==
  CASE Bug = "accept_overcounted_chunks" ->
         TotalChunks(c) = 0
    [] Bug = "require_chunks_for_zero_total" ->
         /\ TotalChunks(c) # 0
         /\ ReceivedChunks(c) > TotalChunks(c)
    [] OTHER -> InvalidShape(c)

ActualChunkBlocks(c) ==
  CASE Bug = "ignore_missing_chunks" -> FALSE
    [] Bug = "reject_allowed_missing_chunks" ->
         /\ ~InvalidShape(c)
         /\ ReceivedChunks(c) < TotalChunks(c)
    [] Bug = "require_chunks_for_zero_total" ->
         \/ TotalChunks(c) = 0
         \/ ChunkBlocks(c)
    [] OTHER -> ChunkBlocks(c)

ActualRootBlocks(c) ==
  CASE Bug = "ignore_chunk_root_mismatch" -> FALSE
    [] Bug = "reject_absent_root" ->
         \/ RootBlocks(c)
         \/ ~ExpectedRootPresent(c)
         \/ ~ComputedRootPresent(c)
    [] OTHER -> RootBlocks(c)

ActualReadyDecision(c) ==
  IF Bug = "wrong_defer_ready_count"
  THEN "defer_ready_1_2"
  ELSE ReadyDecision(ReadyCount(c), RequiredReady(c))

ActualChunkDecision(c) ==
  IF Bug = "wrong_defer_chunk_count"
  THEN "defer_chunks_0_2"
  ELSE ChunkDecision(ReceivedChunks(c), TotalChunks(c))

ActualShapeDecision(c) ==
  ShapeDecision(ReceivedChunks(c), TotalChunks(c))

ActualDecision(c) ==
  IF Bug = "prefer_chunks_before_ready"
  THEN
    IF ActualChunkBlocks(c)
    THEN ActualChunkDecision(c)
    ELSE
      IF ActualReadyBlocks(c)
      THEN ActualReadyDecision(c)
      ELSE
        IF ActualShapeBlocks(c)
        THEN ActualShapeDecision(c)
        ELSE IF ActualRootBlocks(c) THEN "invalid_root" ELSE "accept"
  ELSE
    IF ActualReadyBlocks(c)
    THEN ActualReadyDecision(c)
    ELSE
      IF ActualShapeBlocks(c)
      THEN ActualShapeDecision(c)
      ELSE
        IF ActualChunkBlocks(c)
        THEN ActualChunkDecision(c)
        ELSE IF ActualRootBlocks(c) THEN "invalid_root" ELSE "accept"

TypeInvariant ==
  /\ Bug \in {
       "none",
       "ignore_ready_quorum",
       "require_ready_when_zero",
       "prefer_chunks_before_ready",
       "ignore_missing_chunks",
       "reject_allowed_missing_chunks",
       "require_chunks_for_zero_total",
       "accept_overcounted_chunks",
       "ignore_chunk_root_mismatch",
       "reject_absent_root",
       "wrong_defer_ready_count",
       "wrong_defer_chunk_count"
     }
  /\ candidate \in Cases
  /\ decision \in DecisionValues

Init ==
  /\ candidate \in Cases
  /\ decision = ActualDecision(candidate)

Next ==
  UNCHANGED vars

DecisionMatchesSpec ==
  decision = SpecDecision(candidate)

ReadyQuorumDefersFirst ==
  ReadyBlocks(candidate) => decision = ReadyDecision(ReadyCount(candidate), RequiredReady(candidate))

ZeroRequiredReadyDoesNotDefer ==
  candidate = "zero_required_ready_accepts" => decision = "accept"

ReadyDeferralPrecedesChunkDeferral ==
  candidate = "ready_precedes_chunks" => decision = "defer_ready_0_1"

ReadyDeferralPrecedesInvalidShape ==
  candidate = "ready_precedes_invalid_shape" => decision = "defer_ready_0_1"

MissingChunksDeferUnlessAllowed ==
  candidate = "defer_chunks" => decision = "defer_chunks_1_2"

AllowMissingChunksBypassesChunkDeferral ==
  candidate = "allow_missing_chunks_accepts" => decision = "accept"

InvalidChunkShapeRejected ==
  /\ candidate = "zero_total_invalid" => decision = "invalid_shape_0_0"
  /\ candidate \in {"overcount_invalid", "allow_missing_overcount_invalid"} =>
       decision = "invalid_shape_2_1"

MismatchedChunkRootRejected ==
  candidate = "invalid_chunk_root" => decision = "invalid_root"

AbsentExpectedRootDoesNotReject ==
  candidate = "absent_expected_root_accepts" => decision = "accept"

AbsentComputedRootDoesNotReject ==
  candidate = "absent_computed_root_accepts" => decision = "accept"

AcceptRequiresAllGatesOpen ==
  decision = "accept" =>
    /\ ~ReadyBlocks(candidate)
    /\ ~InvalidShape(candidate)
    /\ ~ChunkBlocks(candidate)
    /\ ~RootBlocks(candidate)

Safety ==
  /\ DecisionMatchesSpec
  /\ ReadyQuorumDefersFirst
  /\ ZeroRequiredReadyDoesNotDefer
  /\ ReadyDeferralPrecedesChunkDeferral
  /\ ReadyDeferralPrecedesInvalidShape
  /\ MissingChunksDeferUnlessAllowed
  /\ AllowMissingChunksBypassesChunkDeferral
  /\ InvalidChunkShapeRejected
  /\ MismatchedChunkRootRejected
  /\ AbsentExpectedRootDoesNotReject
  /\ AbsentComputedRootDoesNotReject
  /\ AcceptRequiresAllGatesOpen

====
