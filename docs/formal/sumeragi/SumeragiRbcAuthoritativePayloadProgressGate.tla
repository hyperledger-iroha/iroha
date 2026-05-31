---- MODULE SumeragiRbcAuthoritativePayloadProgressGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC authoritative payload progress knowledge.

This slice captures `rbc_session_has_authoritative_payload_for_progress(...)`
and its complete-chunk helper. A session must first have valid metadata for the
progress slot: it cannot be invalid, it must advertise a payload hash, it must
carry a block header and leader signature, and that header must match the
session key hash, height, and view. Once metadata matches, complete RBC chunks
are authoritative when their observed chunk root is accepted by the helper. If
chunks are not complete or fail the root check, the predicate falls back to the
actor-local authoritative payload lookup and accepts only matching height, view,
and payload hash.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InvalidSession == "invalid_session"
MissingPayloadHash == "missing_payload_hash"
MissingHeader == "missing_header"
MissingLeaderSignature == "missing_leader_signature"
HeaderHashMismatch == "header_hash_mismatch"
HeaderHeightMismatch == "header_height_mismatch"
HeaderViewMismatch == "header_view_mismatch"
CompleteZeroExpectedRoot == "complete_zero_expected_root"
CompleteAllExpectedRootMatch == "complete_all_expected_root_match"
CompleteAllNoExpectedObservedRoot == "complete_all_no_expected_observed_root"
ZeroMissingExpectedRoot == "zero_missing_expected_root"
IncompleteChunks == "incomplete_chunks"
RootMismatch == "root_mismatch"
MissingObservedRoot == "missing_observed_root"
LocalPayloadMatch == "local_payload_match"
LocalPayloadWrongHeight == "local_payload_wrong_height"
LocalPayloadWrongView == "local_payload_wrong_view"
LocalPayloadWrongHash == "local_payload_wrong_hash"
LocalPayloadAbsent == "local_payload_absent"

Cases == {
  InvalidSession,
  MissingPayloadHash,
  MissingHeader,
  MissingLeaderSignature,
  HeaderHashMismatch,
  HeaderHeightMismatch,
  HeaderViewMismatch,
  CompleteZeroExpectedRoot,
  CompleteAllExpectedRootMatch,
  CompleteAllNoExpectedObservedRoot,
  ZeroMissingExpectedRoot,
  IncompleteChunks,
  RootMismatch,
  MissingObservedRoot,
  LocalPayloadMatch,
  LocalPayloadWrongHeight,
  LocalPayloadWrongView,
  LocalPayloadWrongHash,
  LocalPayloadAbsent
}

MetadataInvalidCases == {
  InvalidSession,
  MissingPayloadHash,
  MissingHeader,
  MissingLeaderSignature,
  HeaderHashMismatch,
  HeaderHeightMismatch,
  HeaderViewMismatch
}

MetadataOk(c) ==
  c \notin MetadataInvalidCases

CompletePayloadCases == {
  CompleteZeroExpectedRoot,
  CompleteAllExpectedRootMatch,
  CompleteAllNoExpectedObservedRoot
}

ChunkRejectedCases == {
  ZeroMissingExpectedRoot,
  IncompleteChunks,
  RootMismatch,
  MissingObservedRoot,
  LocalPayloadMatch,
  LocalPayloadWrongHeight,
  LocalPayloadWrongView,
  LocalPayloadWrongHash,
  LocalPayloadAbsent
}

LocalFallbackCases == ChunkRejectedCases

LocalFallbackMatches(c) ==
  c = LocalPayloadMatch

SpecResult(c) ==
  MetadataOk(c) /\ (c \in CompletePayloadCases \/ LocalFallbackMatches(c))

CheckMetadata == 1
ReturnTrue == 2
ReturnFalse == 3
RejectInvalidSession == 4
RejectMissingPayloadHash == 5
RejectMissingHeader == 6
RejectMissingLeaderSignature == 7
RejectHeaderHashMismatch == 8
RejectHeaderHeightMismatch == 9
RejectHeaderViewMismatch == 10
CheckCompleteChunks == 11
AcceptZeroChunkExpectedRoot == 12
AcceptExpectedRootMatch == 13
AcceptNoExpectedObservedRoot == 14
RejectZeroMissingExpectedRoot == 15
RejectIncompleteChunks == 16
RejectRootMismatch == 17
RejectMissingObservedRoot == 18
RejectNoCompleteChunkPayload == 19
CheckLocalPayload == 20
AcceptLocalPayload == 21
RejectLocalPayloadHeight == 22
RejectLocalPayloadView == 23
RejectLocalPayloadHash == 24
RejectLocalPayloadAbsent == 25

ActionUniverse == 1..25

MetadataRejectAction(c) ==
  CASE c = InvalidSession -> {RejectInvalidSession}
    [] c = MissingPayloadHash -> {RejectMissingPayloadHash}
    [] c = MissingHeader -> {RejectMissingHeader}
    [] c = MissingLeaderSignature -> {RejectMissingLeaderSignature}
    [] c = HeaderHashMismatch -> {RejectHeaderHashMismatch}
    [] c = HeaderHeightMismatch -> {RejectHeaderHeightMismatch}
    [] c = HeaderViewMismatch -> {RejectHeaderViewMismatch}
    [] OTHER -> {}

CompleteChunkAction(c) ==
  CASE c = CompleteZeroExpectedRoot -> {AcceptZeroChunkExpectedRoot}
    [] c = CompleteAllExpectedRootMatch -> {AcceptExpectedRootMatch}
    [] c = CompleteAllNoExpectedObservedRoot -> {AcceptNoExpectedObservedRoot}
    [] c = ZeroMissingExpectedRoot -> {RejectZeroMissingExpectedRoot}
    [] c = IncompleteChunks -> {RejectIncompleteChunks}
    [] c = RootMismatch -> {RejectRootMismatch}
    [] c = MissingObservedRoot -> {RejectMissingObservedRoot}
    [] c \in {LocalPayloadMatch, LocalPayloadWrongHeight,
              LocalPayloadWrongView, LocalPayloadWrongHash,
              LocalPayloadAbsent} -> {RejectNoCompleteChunkPayload}
    [] OTHER -> {}

LocalPayloadAction(c) ==
  CASE c = LocalPayloadMatch -> {AcceptLocalPayload}
    [] c = LocalPayloadWrongHeight -> {RejectLocalPayloadHeight}
    [] c = LocalPayloadWrongView -> {RejectLocalPayloadView}
    [] c = LocalPayloadWrongHash -> {RejectLocalPayloadHash}
    [] c = LocalPayloadAbsent -> {RejectLocalPayloadAbsent}
    [] c \in {ZeroMissingExpectedRoot, IncompleteChunks, RootMismatch,
              MissingObservedRoot} -> {RejectLocalPayloadAbsent}
    [] OTHER -> {}

SpecActions(c) ==
  {CheckMetadata}
    \cup (IF SpecResult(c) THEN {ReturnTrue} ELSE {ReturnFalse})
    \cup (IF ~MetadataOk(c) THEN MetadataRejectAction(c) ELSE {})
    \cup (IF MetadataOk(c) THEN {CheckCompleteChunks} ELSE {})
    \cup (IF MetadataOk(c) THEN CompleteChunkAction(c) ELSE {})
    \cup (IF MetadataOk(c) /\ c \in LocalFallbackCases
          THEN {CheckLocalPayload} \cup LocalPayloadAction(c)
          ELSE {})

ImplementationResult(c) ==
  CASE Bug = "accept_invalid_session"
       /\ c = InvalidSession ->
      TRUE
    [] Bug = "accept_missing_payload_hash"
       /\ c = MissingPayloadHash ->
      TRUE
    [] Bug = "accept_missing_header"
       /\ c = MissingHeader ->
      TRUE
    [] Bug = "accept_missing_leader_signature"
       /\ c = MissingLeaderSignature ->
      TRUE
    [] Bug = "accept_header_hash_mismatch"
       /\ c = HeaderHashMismatch ->
      TRUE
    [] Bug = "accept_header_height_mismatch"
       /\ c = HeaderHeightMismatch ->
      TRUE
    [] Bug = "accept_header_view_mismatch"
       /\ c = HeaderViewMismatch ->
      TRUE
    [] Bug = "reject_zero_chunk_expected_root"
       /\ c = CompleteZeroExpectedRoot ->
      FALSE
    [] Bug = "reject_expected_root_match"
       /\ c = CompleteAllExpectedRootMatch ->
      FALSE
    [] Bug = "reject_no_expected_observed_root"
       /\ c = CompleteAllNoExpectedObservedRoot ->
      FALSE
    [] Bug = "accept_zero_missing_expected_root"
       /\ c = ZeroMissingExpectedRoot ->
      TRUE
    [] Bug = "accept_incomplete_chunks"
       /\ c = IncompleteChunks ->
      TRUE
    [] Bug = "accept_root_mismatch"
       /\ c = RootMismatch ->
      TRUE
    [] Bug = "accept_missing_observed_root"
       /\ c = MissingObservedRoot ->
      TRUE
    [] Bug = "skip_local_fallback"
       /\ c = LocalPayloadMatch ->
      FALSE
    [] Bug = "accept_local_wrong_height"
       /\ c = LocalPayloadWrongHeight ->
      TRUE
    [] Bug = "accept_local_wrong_view"
       /\ c = LocalPayloadWrongView ->
      TRUE
    [] Bug = "accept_local_wrong_hash"
       /\ c = LocalPayloadWrongHash ->
      TRUE
    [] Bug = "accept_local_absent"
       /\ c = LocalPayloadAbsent ->
      TRUE
    [] OTHER -> SpecResult(c)

WithReturn(actions, result) ==
  (actions \ {ReturnTrue, ReturnFalse})
    \cup (IF result THEN {ReturnTrue} ELSE {ReturnFalse})

ImplementationActions(c) ==
  WithReturn(SpecActions(c), ImplementationResult(c))

Bugs == {
  "none",
  "accept_invalid_session",
  "accept_missing_payload_hash",
  "accept_missing_header",
  "accept_missing_leader_signature",
  "accept_header_hash_mismatch",
  "accept_header_height_mismatch",
  "accept_header_view_mismatch",
  "reject_zero_chunk_expected_root",
  "reject_expected_root_match",
  "reject_no_expected_observed_root",
  "accept_zero_missing_expected_root",
  "accept_incomplete_chunks",
  "accept_root_mismatch",
  "accept_missing_observed_root",
  "skip_local_fallback",
  "accept_local_wrong_height",
  "accept_local_wrong_view",
  "accept_local_wrong_hash",
  "accept_local_absent"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

MetadataGateRejectsIncompleteSessions ==
  /\ \A c \in MetadataInvalidCases:
       /\ ImplementationResult(c) = FALSE
       /\ MetadataRejectAction(c) \subseteq ImplementationActions(c)
       /\ ~(CheckCompleteChunks \in ImplementationActions(c))
       /\ ~(CheckLocalPayload \in ImplementationActions(c))

CompleteChunkPayloadsAreAuthoritative ==
  /\ ImplementationResult(CompleteZeroExpectedRoot) = TRUE
  /\ AcceptZeroChunkExpectedRoot
       \in ImplementationActions(CompleteZeroExpectedRoot)
  /\ ~(CheckLocalPayload \in ImplementationActions(CompleteZeroExpectedRoot))
  /\ ImplementationResult(CompleteAllExpectedRootMatch) = TRUE
  /\ AcceptExpectedRootMatch
       \in ImplementationActions(CompleteAllExpectedRootMatch)
  /\ ~(CheckLocalPayload \in ImplementationActions(CompleteAllExpectedRootMatch))
  /\ ImplementationResult(CompleteAllNoExpectedObservedRoot) = TRUE
  /\ AcceptNoExpectedObservedRoot
       \in ImplementationActions(CompleteAllNoExpectedObservedRoot)
  /\ ~(CheckLocalPayload
       \in ImplementationActions(CompleteAllNoExpectedObservedRoot))

ChunkFailuresNeedLocalFallback ==
  /\ ImplementationResult(ZeroMissingExpectedRoot) = FALSE
  /\ RejectZeroMissingExpectedRoot
       \in ImplementationActions(ZeroMissingExpectedRoot)
  /\ CheckLocalPayload \in ImplementationActions(ZeroMissingExpectedRoot)
  /\ ImplementationResult(IncompleteChunks) = FALSE
  /\ RejectIncompleteChunks \in ImplementationActions(IncompleteChunks)
  /\ CheckLocalPayload \in ImplementationActions(IncompleteChunks)
  /\ ImplementationResult(RootMismatch) = FALSE
  /\ RejectRootMismatch \in ImplementationActions(RootMismatch)
  /\ CheckLocalPayload \in ImplementationActions(RootMismatch)
  /\ ImplementationResult(MissingObservedRoot) = FALSE
  /\ RejectMissingObservedRoot \in ImplementationActions(MissingObservedRoot)
  /\ CheckLocalPayload \in ImplementationActions(MissingObservedRoot)

LocalFallbackRequiresExactSlotAndPayloadHash ==
  /\ ImplementationResult(LocalPayloadMatch) = TRUE
  /\ AcceptLocalPayload \in ImplementationActions(LocalPayloadMatch)
  /\ ImplementationResult(LocalPayloadWrongHeight) = FALSE
  /\ RejectLocalPayloadHeight
       \in ImplementationActions(LocalPayloadWrongHeight)
  /\ ImplementationResult(LocalPayloadWrongView) = FALSE
  /\ RejectLocalPayloadView \in ImplementationActions(LocalPayloadWrongView)
  /\ ImplementationResult(LocalPayloadWrongHash) = FALSE
  /\ RejectLocalPayloadHash \in ImplementationActions(LocalPayloadWrongHash)
  /\ ImplementationResult(LocalPayloadAbsent) = FALSE
  /\ RejectLocalPayloadAbsent \in ImplementationActions(LocalPayloadAbsent)

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckMetadata \in ImplementationActions(c)
  /\ \A c \in MetadataInvalidCases:
       /\ ~(CheckCompleteChunks \in ImplementationActions(c))
       /\ ~(CheckLocalPayload \in ImplementationActions(c))
  /\ \A c \in Cases \ MetadataInvalidCases:
       CheckCompleteChunks \in ImplementationActions(c)
  /\ \A c \in CompletePayloadCases:
       ~(CheckLocalPayload \in ImplementationActions(c))
  /\ \A c \in LocalFallbackCases:
       CheckLocalPayload \in ImplementationActions(c)

NoBugInvariant ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ MetadataGateRejectsIncompleteSessions
  /\ CompleteChunkPayloadsAreAuthoritative
  /\ ChunkFailuresNeedLocalFallback
  /\ LocalFallbackRequiresExactSlotAndPayloadHash
  /\ LookupShapeMatchesShortCircuit

SafetyFast == NoBugInvariant

====
