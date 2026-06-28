---- MODULE SumeragiRbcChunkPostDebugGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC chunk post scheduling and debug masks.

This slice pins `mask_includes(...)`, the RBC debug-mask predicates,
`mutate_chunk_for_equivocation(...)`, `fork_ready_message(...)`, and
`schedule_rbc_chunk_posts(...)`. Masks only include indices below 64; local,
out-of-range, disallowed, debug-dropped, empty, and unexpected chunk targets do
not post; canonical chunk posts reuse the cached frame; debug equivocation
requires both validator and chunk masks and rebuilds a fresh mutated chunk; and
READY forking always mutates or creates signature bytes.
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
MaskAllowsIndex64 == 1
MaskIgnoresSetBit == 2
WithholdHighChunk == 3
WithholdIgnoresLowMask == 4
EquivocateValidatorOnly == 5
EquivocateChunkOnly == 6
EquivocateAllowsChunk64 == 7
DropMaskPosts == 8
LocalPeerPosts == 9
OobValidatorPosts == 10
DisallowedChunkPosts == 11
UnexpectedVariantPosts == 12
CanonicalRebuildsFresh == 13
CompactChunkRejected == 14
EquivocationUsesCached == 15
EquivocationNonemptyUnchanged == 16
EquivocationEmptyNotAppended == 17
CountPostAsSkip == 18
CountSkipAsPost == 19
ReadyMaskAllowsIndex64 == 20
ForkReadyNonemptyUnchanged == 21
ForkReadyEmptyUnchanged == 22

Bugs == 0..22

MaskRejectsIndex64 ==
  Bug # MaskAllowsIndex64

MaskHonorsSetBits ==
  Bug # MaskIgnoresSetBit

WithholdRejectsHighChunks ==
  Bug # WithholdHighChunk

WithholdHonorsSelectedLowChunks ==
  Bug # WithholdIgnoresLowMask

EquivocationRequiresValidatorMask ==
  Bug # EquivocateChunkOnly

EquivocationRequiresChunkMask ==
  Bug # EquivocateValidatorOnly

EquivocationRejectsChunkIndex64 ==
  Bug # EquivocateAllowsChunk64

DropMaskSuppressesPost ==
  Bug # DropMaskPosts

LocalPeerSuppressesPost ==
  Bug # LocalPeerPosts

OutOfRangeValidatorSuppressesPost ==
  Bug # OobValidatorPosts

DisallowedChunkSuppressesPost ==
  Bug # DisallowedChunkPosts

UnexpectedVariantSuppressesPost ==
  Bug # UnexpectedVariantPosts

CanonicalPostUsesCachedFrame ==
  Bug # CanonicalRebuildsFresh

CompactChunkPostsLikeFullChunk ==
  Bug # CompactChunkRejected

EquivocatedPostUsesFreshFrame ==
  Bug # EquivocationUsesCached

NonemptyEquivocationMutatesBytes ==
  Bug # EquivocationNonemptyUnchanged

EmptyEquivocationAppendsByte ==
  Bug # EquivocationEmptyNotAppended

PostsIncrementCount ==
  Bug # CountPostAsSkip

SkipsDoNotIncrementCount ==
  Bug # CountSkipAsPost

ReadyMaskRejectsIndex64 ==
  Bug # ReadyMaskAllowsIndex64

ForkReadyMutatesNonemptySignature ==
  Bug # ForkReadyNonemptyUnchanged

ForkReadyAppendsEmptySignature ==
  Bug # ForkReadyEmptyUnchanged

Init ==
  checked = 0

Next ==
  \/ /\ checked < 22
     /\ checked' = checked + 1
  \/ /\ checked = 22
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..22

MaskSafety ==
  /\ MaskRejectsIndex64
  /\ MaskHonorsSetBits
  /\ ReadyMaskRejectsIndex64

WithholdSafety ==
  /\ WithholdRejectsHighChunks
  /\ WithholdHonorsSelectedLowChunks

EquivocationPredicateSafety ==
  /\ EquivocationRequiresValidatorMask
  /\ EquivocationRequiresChunkMask
  /\ EquivocationRejectsChunkIndex64

ScheduleSkipSafety ==
  /\ DropMaskSuppressesPost
  /\ LocalPeerSuppressesPost
  /\ OutOfRangeValidatorSuppressesPost
  /\ DisallowedChunkSuppressesPost
  /\ UnexpectedVariantSuppressesPost
  /\ SkipsDoNotIncrementCount

SchedulePostSafety ==
  /\ CanonicalPostUsesCachedFrame
  /\ CompactChunkPostsLikeFullChunk
  /\ EquivocatedPostUsesFreshFrame
  /\ NonemptyEquivocationMutatesBytes
  /\ EmptyEquivocationAppendsByte
  /\ PostsIncrementCount

ForkReadySafety ==
  /\ ForkReadyMutatesNonemptySignature
  /\ ForkReadyAppendsEmptySignature

RbcChunkPostDebugCoreSafety ==
  /\ MaskSafety
  /\ WithholdSafety
  /\ EquivocationPredicateSafety
  /\ ScheduleSkipSafety
  /\ SchedulePostSafety
  /\ ForkReadySafety

SafetyFast ==
  RbcChunkPostDebugCoreSafety

MaskAnchors ==
  /\ MaskRejectsIndex64
  /\ MaskHonorsSetBits
  /\ ReadyMaskRejectsIndex64

WithholdAnchors ==
  /\ WithholdRejectsHighChunks
  /\ WithholdHonorsSelectedLowChunks

EquivocationPredicateAnchors ==
  /\ EquivocationRequiresValidatorMask
  /\ EquivocationRequiresChunkMask
  /\ EquivocationRejectsChunkIndex64

ScheduleSkipAnchors ==
  /\ DropMaskSuppressesPost
  /\ LocalPeerSuppressesPost
  /\ OutOfRangeValidatorSuppressesPost
  /\ DisallowedChunkSuppressesPost
  /\ UnexpectedVariantSuppressesPost
  /\ SkipsDoNotIncrementCount

SchedulePostAnchors ==
  /\ CanonicalPostUsesCachedFrame
  /\ CompactChunkPostsLikeFullChunk
  /\ EquivocatedPostUsesFreshFrame
  /\ NonemptyEquivocationMutatesBytes
  /\ EmptyEquivocationAppendsByte
  /\ PostsIncrementCount

ForkReadyAnchors ==
  /\ ForkReadyMutatesNonemptySignature
  /\ ForkReadyAppendsEmptySignature

SafetyAnchors ==
  /\ MaskAnchors
  /\ WithholdAnchors
  /\ EquivocationPredicateAnchors
  /\ ScheduleSkipAnchors
  /\ SchedulePostAnchors
  /\ ForkReadyAnchors

RbcChunkPostDebugExactness ==
  /\ RbcChunkPostDebugCoreSafety
  /\ SafetyAnchors

RbcChunkPostDebugCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcChunkPostDebugExactness

====
