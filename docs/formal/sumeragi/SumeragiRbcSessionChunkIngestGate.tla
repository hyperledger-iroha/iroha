---- MODULE SumeragiRbcSessionChunkIngestGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC session construction and chunk admission.

This slice captures `RbcSession::new_with_layout(...)`,
`note_chunk_with_digest(...)`, and `drop_mismatched_chunks(...)`: invalid
session metadata fails closed, accepted chunks fill exactly one empty slot and
increment the received count, duplicate and rejected chunks do not mutate
stored data, missing expected digests invalidate the session, digest mismatches
stay non-mutating, and digest cleanup drops only mismatched chunks while
recomputing the received count.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NewOverCap == "new_over_cap"
NewLayoutMismatch == "new_layout_mismatch"
NewDigestCountMismatch == "new_digest_count_mismatch"
NewClean == "new_clean"
IngestOutOfBounds == "ingest_out_of_bounds"
IngestMissingExpectedDigest == "ingest_missing_expected_digest"
IngestDigestMismatch == "ingest_digest_mismatch"
IngestAcceptedEmptySlot == "ingest_accepted_empty_slot"
IngestDuplicateSlot == "ingest_duplicate_slot"
DropNoExpectedDigests == "drop_no_expected_digests"
DropExpectedLenMismatch == "drop_expected_len_mismatch"
DropSingleMismatch == "drop_single_mismatch"
DropAllMatch == "drop_all_match"
DropMixedMismatches == "drop_mixed_mismatches"

Cases == {
  NewOverCap,
  NewLayoutMismatch,
  NewDigestCountMismatch,
  NewClean,
  IngestOutOfBounds,
  IngestMissingExpectedDigest,
  IngestDigestMismatch,
  IngestAcceptedEmptySlot,
  IngestDuplicateSlot,
  DropNoExpectedDigests,
  DropExpectedLenMismatch,
  DropSingleMismatch,
  DropAllMatch,
  DropMixedMismatches
}

ConstructionErrorCases == {
  NewOverCap,
  NewLayoutMismatch,
  NewDigestCountMismatch
}

ConstructOk == 1
ConstructErrTooMany == 2
ConstructErrLayoutMismatch == 3
ConstructErrDigestCountMismatch == 4
SessionAllocated == 5
OutcomeOutOfBounds == 6
OutcomeExpectedDigestMissing == 7
OutcomeDigestMismatch == 8
OutcomeAccepted == 9
OutcomeDuplicate == 10
ChunkStored == 11
NoChunkStored == 12
ExistingChunkPreserved == 13
ExistingChunkOverwritten == 14
ReceivedIncremented == 15
ReceivedUnchanged == 16
ReceivedRecomputed == 17
InvalidSet == 18
InvalidPreserved == 19
NoMutation == 20
MatchingChunkPreserved == 21
MismatchedChunkDropped == 22
MatchingChunkDropped == 23
MismatchedChunkPreserved == 24
DropCountZero == 25
DropCountOne == 26
DropCountTwo == 27

ActionUniverse == 1..27

SpecActions(c) ==
  CASE c = NewOverCap ->
      {ConstructErrTooMany, NoMutation}
    [] c = NewLayoutMismatch ->
      {ConstructErrLayoutMismatch, NoMutation}
    [] c = NewDigestCountMismatch ->
      {ConstructErrDigestCountMismatch, NoMutation}
    [] c = NewClean ->
      {ConstructOk, SessionAllocated, ReceivedUnchanged, InvalidPreserved}
    [] c = IngestOutOfBounds ->
      {OutcomeOutOfBounds, NoChunkStored, ReceivedUnchanged,
       InvalidPreserved}
    [] c = IngestMissingExpectedDigest ->
      {OutcomeExpectedDigestMissing, NoChunkStored, ReceivedUnchanged,
       InvalidSet}
    [] c = IngestDigestMismatch ->
      {OutcomeDigestMismatch, NoChunkStored, ReceivedUnchanged,
       InvalidPreserved}
    [] c = IngestAcceptedEmptySlot ->
      {OutcomeAccepted, ChunkStored, ReceivedIncremented, InvalidPreserved}
    [] c = IngestDuplicateSlot ->
      {OutcomeDuplicate, ExistingChunkPreserved, ReceivedUnchanged,
       InvalidPreserved}
    [] c = DropNoExpectedDigests ->
      {DropCountZero, NoMutation, ReceivedUnchanged, InvalidPreserved}
    [] c = DropExpectedLenMismatch ->
      {DropCountZero, InvalidSet, ReceivedUnchanged}
    [] c = DropSingleMismatch ->
      {MismatchedChunkDropped, MatchingChunkPreserved, DropCountOne,
       ReceivedRecomputed, InvalidPreserved}
    [] c = DropAllMatch ->
      {MatchingChunkPreserved, DropCountZero, ReceivedUnchanged,
       InvalidPreserved}
    [] c = DropMixedMismatches ->
      {MismatchedChunkDropped, MatchingChunkPreserved, DropCountTwo,
       ReceivedRecomputed, InvalidPreserved}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "over_cap_accepted" /\ c = NewOverCap ->
      (spec \ {ConstructErrTooMany, NoMutation}) \cup
        {ConstructOk, SessionAllocated}
    [] Bug = "layout_mismatch_accepted" /\ c = NewLayoutMismatch ->
      (spec \ {ConstructErrLayoutMismatch, NoMutation}) \cup
        {ConstructOk, SessionAllocated}
    [] Bug = "digest_count_mismatch_accepted" /\
       c = NewDigestCountMismatch ->
      (spec \ {ConstructErrDigestCountMismatch, NoMutation}) \cup
        {ConstructOk, SessionAllocated}
    [] Bug = "clean_init_invalid" /\ c = NewClean ->
      (spec \ {InvalidPreserved}) \cup {InvalidSet}
    [] Bug = "oob_accepts_chunk" /\ c = IngestOutOfBounds ->
      (spec \ {OutcomeOutOfBounds, NoChunkStored, ReceivedUnchanged}) \cup
        {OutcomeAccepted, ChunkStored, ReceivedIncremented}
    [] Bug = "missing_digest_not_invalid" /\
       c = IngestMissingExpectedDigest ->
      (spec \ {InvalidSet}) \cup {InvalidPreserved}
    [] Bug = "missing_digest_accepts" /\
       c = IngestMissingExpectedDigest ->
      (spec \ {OutcomeExpectedDigestMissing, NoChunkStored,
               ReceivedUnchanged, InvalidSet}) \cup
        {OutcomeAccepted, ChunkStored, ReceivedIncremented, InvalidPreserved}
    [] Bug = "digest_mismatch_accepts" /\ c = IngestDigestMismatch ->
      (spec \ {OutcomeDigestMismatch, NoChunkStored, ReceivedUnchanged}) \cup
        {OutcomeAccepted, ChunkStored, ReceivedIncremented}
    [] Bug = "digest_mismatch_invalidates" /\ c = IngestDigestMismatch ->
      (spec \ {InvalidPreserved}) \cup {InvalidSet}
    [] Bug = "accepted_skips_chunk" /\ c = IngestAcceptedEmptySlot ->
      (spec \ {ChunkStored}) \cup {NoChunkStored}
    [] Bug = "accepted_skips_increment" /\ c = IngestAcceptedEmptySlot ->
      (spec \ {ReceivedIncremented}) \cup {ReceivedUnchanged}
    [] Bug = "duplicate_overwrites" /\ c = IngestDuplicateSlot ->
      (spec \ {ExistingChunkPreserved}) \cup {ExistingChunkOverwritten}
    [] Bug = "duplicate_increments" /\ c = IngestDuplicateSlot ->
      (spec \ {ReceivedUnchanged}) \cup {ReceivedIncremented}
    [] Bug = "drop_without_digest_invalidates" /\
       c = DropNoExpectedDigests ->
      (spec \ {InvalidPreserved, NoMutation}) \cup {InvalidSet}
    [] Bug = "drop_digest_len_mismatch_not_invalid" /\
       c = DropExpectedLenMismatch ->
      (spec \ {InvalidSet}) \cup {InvalidPreserved}
    [] Bug = "drop_mismatch_keeps_chunk" /\ c = DropSingleMismatch ->
      (spec \ {MismatchedChunkDropped}) \cup {MismatchedChunkPreserved}
    [] Bug = "drop_mismatch_count_stale" /\
       c \in {DropSingleMismatch, DropMixedMismatches} ->
      (spec \ {ReceivedRecomputed}) \cup {ReceivedUnchanged}
    [] Bug = "drop_match_drops" /\ c = DropAllMatch ->
      (spec \ {MatchingChunkPreserved, DropCountZero}) \cup
        {MatchingChunkDropped, DropCountOne}
    [] Bug = "drop_multiple_counts_one" /\ c = DropMixedMismatches ->
      (spec \ {DropCountTwo}) \cup {DropCountOne}
    [] OTHER -> spec

Bugs == {
  "none",
  "over_cap_accepted",
  "layout_mismatch_accepted",
  "digest_count_mismatch_accepted",
  "clean_init_invalid",
  "oob_accepts_chunk",
  "missing_digest_not_invalid",
  "missing_digest_accepts",
  "digest_mismatch_accepts",
  "digest_mismatch_invalidates",
  "accepted_skips_chunk",
  "accepted_skips_increment",
  "duplicate_overwrites",
  "duplicate_increments",
  "drop_without_digest_invalidates",
  "drop_digest_len_mismatch_not_invalid",
  "drop_mismatch_keeps_chunk",
  "drop_mismatch_count_stale",
  "drop_match_drops",
  "drop_multiple_counts_one"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ConstructionRejectsInvalidMetadata ==
  /\ \A c \in ConstructionErrorCases:
       ConstructOk \notin ImplementationActions(c)
  /\ ConstructErrTooMany \in ImplementationActions(NewOverCap)
  /\ ConstructErrLayoutMismatch \in ImplementationActions(NewLayoutMismatch)
  /\ ConstructErrDigestCountMismatch \in
       ImplementationActions(NewDigestCountMismatch)
  /\ ConstructOk \in ImplementationActions(NewClean)
  /\ SessionAllocated \in ImplementationActions(NewClean)
  /\ InvalidPreserved \in ImplementationActions(NewClean)

RejectedChunksDoNotMutate ==
  /\ OutcomeOutOfBounds \in ImplementationActions(IngestOutOfBounds)
  /\ NoChunkStored \in ImplementationActions(IngestOutOfBounds)
  /\ ReceivedUnchanged \in ImplementationActions(IngestOutOfBounds)
  /\ InvalidPreserved \in ImplementationActions(IngestOutOfBounds)
  /\ OutcomeExpectedDigestMissing \in
       ImplementationActions(IngestMissingExpectedDigest)
  /\ NoChunkStored \in ImplementationActions(IngestMissingExpectedDigest)
  /\ InvalidSet \in ImplementationActions(IngestMissingExpectedDigest)
  /\ OutcomeDigestMismatch \in ImplementationActions(IngestDigestMismatch)
  /\ NoChunkStored \in ImplementationActions(IngestDigestMismatch)
  /\ ReceivedUnchanged \in ImplementationActions(IngestDigestMismatch)
  /\ InvalidPreserved \in ImplementationActions(IngestDigestMismatch)

AcceptedAndDuplicateChunksAccountExactly ==
  /\ OutcomeAccepted \in ImplementationActions(IngestAcceptedEmptySlot)
  /\ ChunkStored \in ImplementationActions(IngestAcceptedEmptySlot)
  /\ ReceivedIncremented \in
       ImplementationActions(IngestAcceptedEmptySlot)
  /\ OutcomeDuplicate \in ImplementationActions(IngestDuplicateSlot)
  /\ ExistingChunkPreserved \in ImplementationActions(IngestDuplicateSlot)
  /\ ExistingChunkOverwritten \notin ImplementationActions(IngestDuplicateSlot)
  /\ ReceivedUnchanged \in ImplementationActions(IngestDuplicateSlot)

DigestCleanupMatchesExpectedDigests ==
  /\ DropCountZero \in ImplementationActions(DropNoExpectedDigests)
  /\ NoMutation \in ImplementationActions(DropNoExpectedDigests)
  /\ InvalidPreserved \in ImplementationActions(DropNoExpectedDigests)
  /\ DropCountZero \in ImplementationActions(DropExpectedLenMismatch)
  /\ InvalidSet \in ImplementationActions(DropExpectedLenMismatch)
  /\ MismatchedChunkDropped \in ImplementationActions(DropSingleMismatch)
  /\ MatchingChunkPreserved \in ImplementationActions(DropSingleMismatch)
  /\ DropCountOne \in ImplementationActions(DropSingleMismatch)
  /\ ReceivedRecomputed \in ImplementationActions(DropSingleMismatch)
  /\ MatchingChunkPreserved \in ImplementationActions(DropAllMatch)
  /\ MatchingChunkDropped \notin ImplementationActions(DropAllMatch)
  /\ DropCountZero \in ImplementationActions(DropAllMatch)
  /\ MismatchedChunkDropped \in ImplementationActions(DropMixedMismatches)
  /\ MatchingChunkPreserved \in ImplementationActions(DropMixedMismatches)
  /\ DropCountTwo \in ImplementationActions(DropMixedMismatches)
  /\ ReceivedRecomputed \in ImplementationActions(DropMixedMismatches)

RbcSessionChunkIngestCoreSafety ==
  /\ ActionsMatchSpec
  /\ ConstructionRejectsInvalidMetadata
  /\ RejectedChunksDoNotMutate
  /\ AcceptedAndDuplicateChunksAccountExactly
  /\ DigestCleanupMatchesExpectedDigests

RbcSessionChunkIngestExactness ==
  /\ ActionsMatchSpec
  /\ ConstructionRejectsInvalidMetadata
  /\ RejectedChunksDoNotMutate
  /\ AcceptedAndDuplicateChunksAccountExactly
  /\ DigestCleanupMatchesExpectedDigests

RbcSessionChunkIngestCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcSessionChunkIngestExactness

SafetyFast ==
  RbcSessionChunkIngestExactness

====
