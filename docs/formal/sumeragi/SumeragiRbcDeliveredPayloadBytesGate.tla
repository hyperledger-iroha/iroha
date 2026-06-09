---- MODULE SumeragiRbcDeliveredPayloadBytesGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for delivered RBC payload byte accounting.

This slice captures `RbcSession::delivered_payload_bytes(...)` and
`take_delivered_payload_bytes_for_telemetry_with_fallback(...)`: byte accounting
is gated on delivered and complete sessions, known payload layouts use the
layout size without summing chunks, legacy layouts sum stored chunks with
saturation, fallback bytes are used only for delivered unrecorded sessions whose
local bytes are unavailable, and successful telemetry extraction records the
session exactly once.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BytesNotDelivered == "bytes_not_delivered"
BytesIncomplete == "bytes_incomplete"
BytesKnownLayoutComplete == "bytes_known_layout_complete"
BytesUnknownLayoutComplete == "bytes_unknown_layout_complete"
BytesUnknownMissingSlot == "bytes_unknown_missing_slot"
BytesUnknownSaturates == "bytes_unknown_saturates"
TakeAlreadyRecorded == "take_already_recorded"
TakeNotDeliveredWithFallback == "take_not_delivered_with_fallback"
TakeIncompleteNoFallback == "take_incomplete_no_fallback"
TakeIncompleteWithFallback == "take_incomplete_with_fallback"
TakeCompleteComputed == "take_complete_computed"
TakeCompleteWithFallbackPrefersComputed ==
  "take_complete_with_fallback_prefers_computed"
TakeMissingSlotWithFallback == "take_missing_slot_with_fallback"
TakeDeliveredNoBytesNoFallback == "take_delivered_no_bytes_no_fallback"
TakeSaturatingComputed == "take_saturating_computed"

Cases == {
  BytesNotDelivered,
  BytesIncomplete,
  BytesKnownLayoutComplete,
  BytesUnknownLayoutComplete,
  BytesUnknownMissingSlot,
  BytesUnknownSaturates,
  TakeAlreadyRecorded,
  TakeNotDeliveredWithFallback,
  TakeIncompleteNoFallback,
  TakeIncompleteWithFallback,
  TakeCompleteComputed,
  TakeCompleteWithFallbackPrefersComputed,
  TakeMissingSlotWithFallback,
  TakeDeliveredNoBytesNoFallback,
  TakeSaturatingComputed
}

NoBytes == 1
BytesFromLayout == 2
BytesFromChunkSum == 3
BytesFromFallback == 4
BytesSaturated == 5
FallbackIgnored == 6
FallbackUsed == 7
DeliveredRequired == 8
CompleteRequired == 9
MissingSlotRejectsLocalBytes == 10
LayoutPreferred == 11
ChunksSummed == 12
ChunkSumSaturates == 13
TelemetryNone == 14
TelemetrySomeLayout == 15
TelemetrySomeSum == 16
TelemetrySomeFallback == 17
TelemetrySomeSaturated == 18
RecordedSet == 19
RecordedUnchanged == 20
RecordedAlreadyBlocks == 21

ActionUniverse == 1..21

SpecActions(c) ==
  CASE c = BytesNotDelivered ->
      {NoBytes, DeliveredRequired}
    [] c = BytesIncomplete ->
      {NoBytes, CompleteRequired}
    [] c = BytesKnownLayoutComplete ->
      {BytesFromLayout, LayoutPreferred}
    [] c = BytesUnknownLayoutComplete ->
      {BytesFromChunkSum, ChunksSummed}
    [] c = BytesUnknownMissingSlot ->
      {NoBytes, MissingSlotRejectsLocalBytes}
    [] c = BytesUnknownSaturates ->
      {BytesFromChunkSum, ChunksSummed, BytesSaturated, ChunkSumSaturates}
    [] c = TakeAlreadyRecorded ->
      {TelemetryNone, RecordedAlreadyBlocks, RecordedUnchanged}
    [] c = TakeNotDeliveredWithFallback ->
      {TelemetryNone, DeliveredRequired, FallbackIgnored, RecordedUnchanged}
    [] c = TakeIncompleteNoFallback ->
      {TelemetryNone, CompleteRequired, RecordedUnchanged}
    [] c = TakeIncompleteWithFallback ->
      {TelemetrySomeFallback, BytesFromFallback, FallbackUsed, RecordedSet}
    [] c = TakeCompleteComputed ->
      {TelemetrySomeSum, BytesFromChunkSum, ChunksSummed, RecordedSet}
    [] c = TakeCompleteWithFallbackPrefersComputed ->
      {TelemetrySomeSum, BytesFromChunkSum, ChunksSummed, FallbackIgnored,
       RecordedSet}
    [] c = TakeMissingSlotWithFallback ->
      {TelemetrySomeFallback, MissingSlotRejectsLocalBytes, BytesFromFallback,
       FallbackUsed, RecordedSet}
    [] c = TakeDeliveredNoBytesNoFallback ->
      {TelemetryNone, MissingSlotRejectsLocalBytes, RecordedUnchanged}
    [] c = TakeSaturatingComputed ->
      {TelemetrySomeSaturated, BytesFromChunkSum, ChunksSummed,
       BytesSaturated, ChunkSumSaturates, RecordedSet}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "bytes_not_delivered_reports" /\ c = BytesNotDelivered ->
      (spec \ {NoBytes, DeliveredRequired}) \cup {BytesFromChunkSum}
    [] Bug = "bytes_incomplete_reports" /\ c = BytesIncomplete ->
      (spec \ {NoBytes, CompleteRequired}) \cup {BytesFromChunkSum}
    [] Bug = "known_layout_sums_chunks" /\ c = BytesKnownLayoutComplete ->
      (spec \ {BytesFromLayout, LayoutPreferred}) \cup
        {BytesFromChunkSum, ChunksSummed}
    [] Bug = "unknown_complete_uses_layout" /\
       c = BytesUnknownLayoutComplete ->
      (spec \ {BytesFromChunkSum, ChunksSummed}) \cup {BytesFromLayout}
    [] Bug = "missing_slot_counts_zero" /\ c = BytesUnknownMissingSlot ->
      (spec \ {NoBytes, MissingSlotRejectsLocalBytes}) \cup
        {BytesFromChunkSum}
    [] Bug = "chunk_sum_not_saturating" /\ c = BytesUnknownSaturates ->
      spec \ {BytesSaturated, ChunkSumSaturates}
    [] Bug = "take_recorded_reports_again" /\ c = TakeAlreadyRecorded ->
      (spec \ {TelemetryNone, RecordedAlreadyBlocks, RecordedUnchanged}) \cup
        {TelemetrySomeSum, RecordedSet}
    [] Bug = "take_not_delivered_uses_fallback" /\
       c = TakeNotDeliveredWithFallback ->
      (spec \ {TelemetryNone, DeliveredRequired, FallbackIgnored,
               RecordedUnchanged}) \cup
        {TelemetrySomeFallback, BytesFromFallback, FallbackUsed, RecordedSet}
    [] Bug = "take_none_sets_recorded" /\
       c \in {TakeIncompleteNoFallback, TakeDeliveredNoBytesNoFallback} ->
      (spec \ {RecordedUnchanged}) \cup {RecordedSet}
    [] Bug = "fallback_ignored_for_incomplete" /\
       c = TakeIncompleteWithFallback ->
      (spec \ {TelemetrySomeFallback, BytesFromFallback, FallbackUsed,
               RecordedSet}) \cup
        {TelemetryNone, FallbackIgnored, RecordedUnchanged}
    [] Bug = "fallback_does_not_set_recorded" /\
       c \in {TakeIncompleteWithFallback, TakeMissingSlotWithFallback} ->
      (spec \ {RecordedSet}) \cup {RecordedUnchanged}
    [] Bug = "computed_uses_fallback" /\
       c = TakeCompleteWithFallbackPrefersComputed ->
      (spec \ {TelemetrySomeSum, BytesFromChunkSum, ChunksSummed,
               FallbackIgnored}) \cup
        {TelemetrySomeFallback, BytesFromFallback, FallbackUsed}
    [] Bug = "computed_does_not_set_recorded" /\
       c \in {TakeCompleteComputed, TakeCompleteWithFallbackPrefersComputed,
              TakeSaturatingComputed} ->
      (spec \ {RecordedSet}) \cup {RecordedUnchanged}
    [] Bug = "missing_slot_ignores_fallback" /\
       c = TakeMissingSlotWithFallback ->
      (spec \ {TelemetrySomeFallback, BytesFromFallback, FallbackUsed,
               RecordedSet}) \cup
        {TelemetryNone, FallbackIgnored, RecordedUnchanged}
    [] Bug = "no_fallback_reports_zero" /\ c = TakeDeliveredNoBytesNoFallback ->
      (spec \ {TelemetryNone, RecordedUnchanged}) \cup
        {TelemetrySomeSum, BytesFromChunkSum, RecordedSet}
    [] Bug = "take_saturation_lost" /\ c = TakeSaturatingComputed ->
      (spec \ {TelemetrySomeSaturated, BytesSaturated, ChunkSumSaturates}) \cup
        {TelemetrySomeSum}
    [] OTHER -> spec

Bugs == {
  "none",
  "bytes_not_delivered_reports",
  "bytes_incomplete_reports",
  "known_layout_sums_chunks",
  "unknown_complete_uses_layout",
  "missing_slot_counts_zero",
  "chunk_sum_not_saturating",
  "take_recorded_reports_again",
  "take_not_delivered_uses_fallback",
  "take_none_sets_recorded",
  "fallback_ignored_for_incomplete",
  "fallback_does_not_set_recorded",
  "computed_uses_fallback",
  "computed_does_not_set_recorded",
  "missing_slot_ignores_fallback",
  "no_fallback_reports_zero",
  "take_saturation_lost"
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

DeliveredPayloadBytesAreGatedAndExact ==
  /\ NoBytes \in ImplementationActions(BytesNotDelivered)
  /\ DeliveredRequired \in ImplementationActions(BytesNotDelivered)
  /\ NoBytes \in ImplementationActions(BytesIncomplete)
  /\ CompleteRequired \in ImplementationActions(BytesIncomplete)
  /\ BytesFromLayout \in ImplementationActions(BytesKnownLayoutComplete)
  /\ LayoutPreferred \in ImplementationActions(BytesKnownLayoutComplete)
  /\ BytesFromChunkSum \in ImplementationActions(BytesUnknownLayoutComplete)
  /\ ChunksSummed \in ImplementationActions(BytesUnknownLayoutComplete)
  /\ NoBytes \in ImplementationActions(BytesUnknownMissingSlot)
  /\ MissingSlotRejectsLocalBytes \in
       ImplementationActions(BytesUnknownMissingSlot)
  /\ BytesSaturated \in ImplementationActions(BytesUnknownSaturates)
  /\ ChunkSumSaturates \in ImplementationActions(BytesUnknownSaturates)

TelemetryExtractionRecordsOnlySuccessfulReports ==
  /\ TelemetryNone \in ImplementationActions(TakeAlreadyRecorded)
  /\ RecordedAlreadyBlocks \in ImplementationActions(TakeAlreadyRecorded)
  /\ RecordedUnchanged \in ImplementationActions(TakeAlreadyRecorded)
  /\ TelemetryNone \in ImplementationActions(TakeNotDeliveredWithFallback)
  /\ FallbackIgnored \in ImplementationActions(TakeNotDeliveredWithFallback)
  /\ RecordedUnchanged \in
       ImplementationActions(TakeNotDeliveredWithFallback)
  /\ TelemetryNone \in ImplementationActions(TakeIncompleteNoFallback)
  /\ RecordedUnchanged \in ImplementationActions(TakeIncompleteNoFallback)
  /\ TelemetryNone \in ImplementationActions(TakeDeliveredNoBytesNoFallback)
  /\ RecordedUnchanged \in
       ImplementationActions(TakeDeliveredNoBytesNoFallback)

FallbackAndComputedBytesFollowPriority ==
  /\ TelemetrySomeFallback \in
       ImplementationActions(TakeIncompleteWithFallback)
  /\ FallbackUsed \in ImplementationActions(TakeIncompleteWithFallback)
  /\ RecordedSet \in ImplementationActions(TakeIncompleteWithFallback)
  /\ TelemetrySomeSum \in ImplementationActions(TakeCompleteComputed)
  /\ BytesFromChunkSum \in ImplementationActions(TakeCompleteComputed)
  /\ RecordedSet \in ImplementationActions(TakeCompleteComputed)
  /\ TelemetrySomeSum \in
       ImplementationActions(TakeCompleteWithFallbackPrefersComputed)
  /\ FallbackIgnored \in
       ImplementationActions(TakeCompleteWithFallbackPrefersComputed)
  /\ RecordedSet \in
       ImplementationActions(TakeCompleteWithFallbackPrefersComputed)
  /\ TelemetrySomeFallback \in
       ImplementationActions(TakeMissingSlotWithFallback)
  /\ FallbackUsed \in ImplementationActions(TakeMissingSlotWithFallback)
  /\ RecordedSet \in ImplementationActions(TakeMissingSlotWithFallback)
  /\ TelemetrySomeSaturated \in ImplementationActions(TakeSaturatingComputed)
  /\ BytesSaturated \in ImplementationActions(TakeSaturatingComputed)
  /\ RecordedSet \in ImplementationActions(TakeSaturatingComputed)

RbcDeliveredPayloadBytesCoreSafety ==
  /\ ActionsMatchSpec
  /\ DeliveredPayloadBytesAreGatedAndExact
  /\ TelemetryExtractionRecordsOnlySuccessfulReports
  /\ FallbackAndComputedBytesFollowPriority

SafetyFast ==
  RbcDeliveredPayloadBytesCoreSafety

====
