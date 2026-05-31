---- MODULE SumeragiRbcDeliverAcceptanceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `evaluate_deliver_acceptance_with_policy(...)`.

The helper accepts an RBC DELIVER only after any required READY quorum is
present, required chunks are present unless the caller explicitly allows
missing chunks, and a present expected chunk root matches the computed root.
Deferral order matters: READY-quorum deferral must win over chunk deferral.
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
  "invalid_chunk_root",
  "absent_expected_root_accepts",
  "absent_computed_root_accepts",
  "ready_precedes_chunks",
  "zero_total_accepts"
}

DecisionValues == {
  "accept",
  "defer_ready_0_0",
  "defer_ready_0_1",
  "defer_ready_1_2",
  "defer_chunks_0_0",
  "defer_chunks_0_2",
  "defer_chunks_1_2",
  "invalid_root"
}

ReadyCount(c) ==
  CASE c = "defer_ready" -> 0
    [] c = "zero_required_ready_accepts" -> 0
    [] c = "ready_precedes_chunks" -> 0
    [] OTHER -> 1

RequiredReady(c) ==
  CASE c = "zero_required_ready_accepts" -> 0
    [] OTHER -> 1

ReceivedChunks(c) ==
  CASE c = "defer_chunks" -> 1
    [] c = "allow_missing_chunks_accepts" -> 1
    [] c = "ready_precedes_chunks" -> 0
    [] c = "zero_total_accepts" -> 0
    [] OTHER -> 1

TotalChunks(c) ==
  CASE c = "defer_chunks" -> 2
    [] c = "allow_missing_chunks_accepts" -> 2
    [] c = "ready_precedes_chunks" -> 2
    [] c = "zero_total_accepts" -> 0
    [] OTHER -> 1

AllowMissingChunks(c) ==
  c = "allow_missing_chunks_accepts"

ExpectedRootPresent(c) ==
  c # "absent_expected_root_accepts"

ComputedRootPresent(c) ==
  c # "absent_computed_root_accepts"

RootMatches(c) ==
  c # "invalid_chunk_root"

ReadyBlocks(c) ==
  RequiredReady(c) # 0 /\ ReadyCount(c) < RequiredReady(c)

ChunkBlocks(c) ==
  /\ ~AllowMissingChunks(c)
  /\ TotalChunks(c) # 0
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

SpecDecision(c) ==
  IF ReadyBlocks(c)
  THEN ReadyDecision(ReadyCount(c), RequiredReady(c))
  ELSE
    IF ChunkBlocks(c)
    THEN ChunkDecision(ReceivedChunks(c), TotalChunks(c))
    ELSE IF RootBlocks(c) THEN "invalid_root" ELSE "accept"

ActualReadyBlocks(c) ==
  CASE Bug = "ignore_ready_quorum" -> FALSE
    [] Bug = "require_ready_when_zero" -> ReadyCount(c) = 0
    [] OTHER -> ReadyBlocks(c)

ActualChunkBlocks(c) ==
  CASE Bug = "ignore_missing_chunks" -> FALSE
    [] Bug = "reject_allowed_missing_chunks" ->
         /\ TotalChunks(c) # 0
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

ActualDecision(c) ==
  IF Bug = "prefer_chunks_before_ready"
  THEN
    IF ActualChunkBlocks(c)
    THEN ActualChunkDecision(c)
    ELSE
      IF ActualReadyBlocks(c)
      THEN ActualReadyDecision(c)
      ELSE IF ActualRootBlocks(c) THEN "invalid_root" ELSE "accept"
  ELSE
    IF ActualReadyBlocks(c)
    THEN ActualReadyDecision(c)
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

MissingChunksDeferUnlessAllowed ==
  candidate = "defer_chunks" => decision = "defer_chunks_1_2"

AllowMissingChunksBypassesChunkDeferral ==
  candidate = "allow_missing_chunks_accepts" => decision = "accept"

ZeroTotalBypassesChunkDeferral ==
  candidate = "zero_total_accepts" => decision = "accept"

MismatchedChunkRootRejected ==
  candidate = "invalid_chunk_root" => decision = "invalid_root"

AbsentExpectedRootDoesNotReject ==
  candidate = "absent_expected_root_accepts" => decision = "accept"

AbsentComputedRootDoesNotReject ==
  candidate = "absent_computed_root_accepts" => decision = "accept"

AcceptRequiresAllGatesOpen ==
  decision = "accept" =>
    /\ ~ReadyBlocks(candidate)
    /\ ~ChunkBlocks(candidate)
    /\ ~RootBlocks(candidate)

Safety ==
  /\ DecisionMatchesSpec
  /\ ReadyQuorumDefersFirst
  /\ ZeroRequiredReadyDoesNotDefer
  /\ ReadyDeferralPrecedesChunkDeferral
  /\ MissingChunksDeferUnlessAllowed
  /\ AllowMissingChunksBypassesChunkDeferral
  /\ ZeroTotalBypassesChunkDeferral
  /\ MismatchedChunkRootRejected
  /\ AbsentExpectedRootDoesNotReject
  /\ AbsentComputedRootDoesNotReject
  /\ AcceptRequiresAllGatesOpen

====
