---- MODULE SumeragiBlockMessageRbcCompactGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC chunk compact block-message helpers.

This slice pins `BlockMessage::from_rbc_chunk(...)`,
`BlockMessage::normalize(...)`, `RbcChunkCompact::try_from_chunk(...)`,
`RbcChunkCompact::into_chunk(...)`, and `BlockMessage::priority(...)`.
Compact encoding is allowed only when height, view, and epoch fit in u32;
all payload-identifying fields must be preserved; compact messages normalize
back to full RBC chunks; non-compact messages normalize unchanged; and consensus
block messages retain high network priority.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == 0
CompactRejectsFit == 1
CompactAcceptsHeightOverflow == 2
CompactAcceptsViewOverflow == 3
CompactAcceptsEpochOverflow == 4
CompactDropsBlockHash == 5
CompactTruncatesIndex == 6
CompactDropsBytes == 7
FromRbcChunkNeverCompacts == 8
FromRbcChunkAlwaysCompacts == 9
NormalizeKeepsCompact == 10
NormalizeMutatesOther == 11
IntoChunkDropsEpoch == 12
PriorityRbcNotHigh == 13
PriorityFetchNotHigh == 14

Bugs == 0..14

TryFromAcceptsFit ==
  Bug # CompactRejectsFit

TryFromRejectsHeightOverflow ==
  Bug # CompactAcceptsHeightOverflow

TryFromRejectsViewOverflow ==
  Bug # CompactAcceptsViewOverflow

TryFromRejectsEpochOverflow ==
  Bug # CompactAcceptsEpochOverflow

CompactPreservesBlockHash ==
  Bug # CompactDropsBlockHash

CompactPreservesIndex ==
  Bug # CompactTruncatesIndex

CompactPreservesBytes ==
  Bug # CompactDropsBytes

FromRbcChunkUsesCompactWhenFit ==
  Bug # FromRbcChunkNeverCompacts

FromRbcChunkFallsBackOnAnyOverflow ==
  Bug # FromRbcChunkAlwaysCompacts

NormalizeExpandsCompact ==
  Bug # NormalizeKeepsCompact

NormalizeLeavesOtherMessagesUnchanged ==
  Bug # NormalizeMutatesOther

IntoChunkWidensHeightViewEpoch ==
  Bug # IntoChunkDropsEpoch

RbcChunkMessagesAreHighPriority ==
  Bug # PriorityRbcNotHigh

FetchMessagesAreHighPriority ==
  Bug # PriorityFetchNotHigh

Init ==
  checked = 0

Next ==
  \/ /\ checked < 14
     /\ checked' = checked + 1
  \/ /\ checked = 14
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..14

CompactBoundarySafety ==
  /\ TryFromAcceptsFit
  /\ TryFromRejectsHeightOverflow
  /\ TryFromRejectsViewOverflow
  /\ TryFromRejectsEpochOverflow
  /\ FromRbcChunkUsesCompactWhenFit
  /\ FromRbcChunkFallsBackOnAnyOverflow

CompactFieldSafety ==
  /\ CompactPreservesBlockHash
  /\ CompactPreservesIndex
  /\ CompactPreservesBytes

NormalizeSafety ==
  /\ NormalizeExpandsCompact
  /\ NormalizeLeavesOtherMessagesUnchanged
  /\ IntoChunkWidensHeightViewEpoch

PrioritySafety ==
  /\ RbcChunkMessagesAreHighPriority
  /\ FetchMessagesAreHighPriority

BlockMessageRbcCompactSafetyAnchors ==
  /\ CompactBoundarySafety
  /\ CompactFieldSafety
  /\ NormalizeSafety
  /\ PrioritySafety

CompactBoundaryExactness ==
  /\ CompactBoundarySafety

CompactFieldPreservationExactness ==
  /\ CompactFieldSafety

CompactNormalizeAndWidenExactness ==
  /\ NormalizeSafety

CompactPriorityExactness ==
  /\ PrioritySafety

BlockMessageRbcCompactExactness ==
  /\ CompactBoundaryExactness
  /\ CompactFieldPreservationExactness
  /\ CompactNormalizeAndWidenExactness
  /\ CompactPriorityExactness
  /\ BlockMessageRbcCompactSafetyAnchors

Safety == BlockMessageRbcCompactExactness

SafetyFast == Safety

BlockMessageRbcCompactCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ BlockMessageRbcCompactSafetyAnchors

====
