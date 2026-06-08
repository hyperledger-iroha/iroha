---- MODULE SumeragiBlockCreatedFrontierWireGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `BlockCreated` frontier metadata helpers.

This slice pins the safety surface around inline frontier metadata carried by
`BlockCreated`: plain constructors must not invent metadata, enriched
constructors must preserve both the block and metadata, proposal/RBC INIT
metadata must be copied field-for-field, generic wire rebuilds must preserve
cached or authoritative metadata without fabricating it, local proposal rebuilds
may seed deterministic metadata from a live roster and must fall back to
authoritative metadata, and cached proposal rebroadcast must reject stale,
mismatched, or incomplete metadata before sending a frontier `BlockCreated`.
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
PlainBorrowedNewBlockKeepsFrontier == 1
PlainOwnedNewBlockKeepsFrontier == 2
PlainSignedBlockKeepsFrontier == 3
WithFrontierDropsBlock == 4
WithFrontierDropsFrontier == 5
FrontierDropsHighestQc == 6
FrontierDropsPayloadHash == 7
FrontierDropsProposer == 8
FrontierDropsEpoch == 9
FrontierDropsRosterHash == 10
FrontierDropsTotalChunks == 11
FrontierDropsChunkDigests == 12
FrontierDropsChunkRoot == 13
FrontierDropsLeaderSignature == 14
GenericWireDropsCachedMetadata == 15
GenericWireDropsAuthoritativeMetadata == 16
GenericWireFabricatesMetadata == 17
ProposalWireWithoutAuthoritativeCache == 18
ProposalWireAcceptsMismatchedCache == 19
ProposalWireSkipsPayloadHash == 20
ProposalWireSkipsRebuiltPayloadHash == 21
ProposalWireSkipsRebuiltEpoch == 22
LocalWireFailsToSeedAuthoritative == 23
LocalWireIgnoresAuthoritativeFallback == 24
LocalWireRequiresMatchingSignature == 25
LocalWireIgnoresLiveRosterFallback == 26
RebroadcastFilterIgnoresPayload == 27
RebroadcastFilterIgnoresProposer == 28
RebroadcastFilterIgnoresEpoch == 29
CachedProposalWrongLeaderAllowed == 30
HintWrongBlockAllowed == 31
RebroadcastWithoutLocalTopology == 32
RebroadcastWithoutMetadata == 33
RebroadcastProceedsWithoutFrontierWire == 34

Bugs == 0..34

NoFrontier == 0
Frontier == 1
Block == 10
OtherBlock == 11

ProposalHighestQc == 20
OtherHighestQc == 21
ProposalPayloadHash == 30
LocalPayloadHash == ProposalPayloadHash
RebuiltPayloadHash == ProposalPayloadHash
OtherPayloadHash == 31
BadRebuiltPayloadHash == 32
ProposalProposer == 40
OtherProposer == 41
ProposalEpoch == 50
RebuiltEpoch == ProposalEpoch
OtherEpoch == 51
BadRebuiltEpoch == 52
InitRosterHash == 60
OtherRosterHash == 61
InitTotalChunks == 70
OtherTotalChunks == 71
InitChunkDigests == 80
OtherChunkDigests == 81
InitChunkRoot == 90
OtherChunkRoot == 91
InitLeaderSignature == 100
ProposalLeaderSignature == 101
FirstBlockSignature == 102
NoSignature == 103

PlainBorrowedNewBlockFrontier ==
  IF Bug = PlainBorrowedNewBlockKeepsFrontier THEN Frontier ELSE NoFrontier

PlainOwnedNewBlockFrontier ==
  IF Bug = PlainOwnedNewBlockKeepsFrontier THEN Frontier ELSE NoFrontier

PlainSignedBlockFrontier ==
  IF Bug = PlainSignedBlockKeepsFrontier THEN Frontier ELSE NoFrontier

WithFrontierBlock ==
  IF Bug = WithFrontierDropsBlock THEN OtherBlock ELSE Block

WithFrontierFrontier ==
  IF Bug = WithFrontierDropsFrontier THEN NoFrontier ELSE Frontier

FrontierHighestQc ==
  IF Bug = FrontierDropsHighestQc THEN OtherHighestQc ELSE ProposalHighestQc

FrontierPayloadHash ==
  IF Bug = FrontierDropsPayloadHash THEN OtherPayloadHash ELSE ProposalPayloadHash

FrontierProposer ==
  IF Bug = FrontierDropsProposer THEN OtherProposer ELSE ProposalProposer

FrontierEpoch ==
  IF Bug = FrontierDropsEpoch THEN OtherEpoch ELSE ProposalEpoch

FrontierRosterHash ==
  IF Bug = FrontierDropsRosterHash THEN OtherRosterHash ELSE InitRosterHash

FrontierTotalChunks ==
  IF Bug = FrontierDropsTotalChunks THEN OtherTotalChunks ELSE InitTotalChunks

FrontierChunkDigests ==
  IF Bug = FrontierDropsChunkDigests THEN OtherChunkDigests ELSE InitChunkDigests

FrontierChunkRoot ==
  IF Bug = FrontierDropsChunkRoot THEN OtherChunkRoot ELSE InitChunkRoot

FrontierLeaderSignature ==
  IF Bug = FrontierDropsLeaderSignature THEN ProposalLeaderSignature ELSE InitLeaderSignature

GenericWireCachedFrontier ==
  IF Bug = GenericWireDropsCachedMetadata THEN NoFrontier ELSE Frontier

GenericWireAuthoritativeFrontier ==
  IF Bug = GenericWireDropsAuthoritativeMetadata THEN NoFrontier ELSE Frontier

GenericWireNoMetadataFrontier ==
  IF Bug = GenericWireFabricatesMetadata THEN Frontier ELSE NoFrontier

ProposalWireBeforeAuthoritativeCache ==
  IF Bug = ProposalWireWithoutAuthoritativeCache THEN Frontier ELSE NoFrontier

ProposalWireMatchingAuthoritativeCache ==
  Frontier

ProposalWireMismatchedAuthoritativeCache ==
  IF Bug = ProposalWireAcceptsMismatchedCache THEN Frontier ELSE NoFrontier

ProposalWirePayloadMismatch ==
  IF Bug = ProposalWireSkipsPayloadHash THEN Frontier ELSE NoFrontier

ProposalWireRebuiltPayloadMismatch ==
  IF Bug = ProposalWireSkipsRebuiltPayloadHash THEN Frontier ELSE NoFrontier

ProposalWireRebuiltEpochMismatch ==
  IF Bug = ProposalWireSkipsRebuiltEpoch THEN Frontier ELSE NoFrontier

LocalWireSeedsAuthoritativeFrontier ==
  IF Bug = LocalWireFailsToSeedAuthoritative THEN NoFrontier ELSE Frontier

LocalWireAuthoritativeFallbackFrontier ==
  IF Bug = LocalWireIgnoresAuthoritativeFallback THEN NoFrontier ELSE Frontier

LocalWireMissingProposerSignature ==
  IF Bug = LocalWireRequiresMatchingSignature THEN NoSignature ELSE FirstBlockSignature

LocalWireLiveRosterFallbackFrontier ==
  IF Bug = LocalWireIgnoresLiveRosterFallback THEN NoFrontier ELSE Frontier

LocallyAuthoritativeFilter(payloadOk, proposerOk, epochOk) ==
  /\ IF Bug = RebroadcastFilterIgnoresPayload THEN TRUE ELSE payloadOk
  /\ IF Bug = RebroadcastFilterIgnoresProposer THEN TRUE ELSE proposerOk
  /\ IF Bug = RebroadcastFilterIgnoresEpoch THEN TRUE ELSE epochOk

CachedProposalWrongLeaderRebroadcast ==
  IF Bug = CachedProposalWrongLeaderAllowed THEN TRUE ELSE FALSE

HintWrongBlockRebroadcast ==
  IF Bug = HintWrongBlockAllowed THEN TRUE ELSE FALSE

MissingLocalTopologyRebroadcast ==
  IF Bug = RebroadcastWithoutLocalTopology THEN TRUE ELSE FALSE

NoMetadataRebroadcast ==
  IF Bug = RebroadcastWithoutMetadata THEN TRUE ELSE FALSE

MissingFrontierWireRebroadcast ==
  IF Bug = RebroadcastProceedsWithoutFrontierWire THEN TRUE ELSE FALSE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 34
     /\ checked' = checked + 1
  \/ /\ checked = 34
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..34
  /\ NoFrontier # Frontier
  /\ Block # OtherBlock
  /\ ProposalPayloadHash = LocalPayloadHash
  /\ RebuiltPayloadHash = ProposalPayloadHash
  /\ RebuiltEpoch = ProposalEpoch

ConstructorSafety ==
  /\ PlainBorrowedNewBlockFrontier = NoFrontier
  /\ PlainOwnedNewBlockFrontier = NoFrontier
  /\ PlainSignedBlockFrontier = NoFrontier
  /\ WithFrontierBlock = Block
  /\ WithFrontierFrontier = Frontier

FrontierInfoCopySafety ==
  /\ FrontierHighestQc = ProposalHighestQc
  /\ FrontierPayloadHash = ProposalPayloadHash
  /\ FrontierProposer = ProposalProposer
  /\ FrontierEpoch = ProposalEpoch
  /\ FrontierRosterHash = InitRosterHash
  /\ FrontierTotalChunks = InitTotalChunks
  /\ FrontierChunkDigests = InitChunkDigests
  /\ FrontierChunkRoot = InitChunkRoot
  /\ FrontierLeaderSignature = InitLeaderSignature

WireRebuildSafety ==
  /\ GenericWireCachedFrontier = Frontier
  /\ GenericWireAuthoritativeFrontier = Frontier
  /\ GenericWireNoMetadataFrontier = NoFrontier
  /\ ProposalWireBeforeAuthoritativeCache = NoFrontier
  /\ ProposalWireMatchingAuthoritativeCache = Frontier
  /\ ProposalWireMismatchedAuthoritativeCache = NoFrontier
  /\ ProposalWirePayloadMismatch = NoFrontier
  /\ ProposalWireRebuiltPayloadMismatch = NoFrontier
  /\ ProposalWireRebuiltEpochMismatch = NoFrontier
  /\ LocalWireSeedsAuthoritativeFrontier = Frontier
  /\ LocalWireAuthoritativeFallbackFrontier = Frontier
  /\ LocalWireMissingProposerSignature = FirstBlockSignature
  /\ LocalWireLiveRosterFallbackFrontier = Frontier

RebroadcastSafety ==
  /\ LocallyAuthoritativeFilter(TRUE, TRUE, TRUE)
  /\ ~LocallyAuthoritativeFilter(FALSE, TRUE, TRUE)
  /\ ~LocallyAuthoritativeFilter(TRUE, FALSE, TRUE)
  /\ ~LocallyAuthoritativeFilter(TRUE, TRUE, FALSE)
  /\ ~CachedProposalWrongLeaderRebroadcast
  /\ ~HintWrongBlockRebroadcast
  /\ ~MissingLocalTopologyRebroadcast
  /\ ~NoMetadataRebroadcast
  /\ ~MissingFrontierWireRebroadcast

SafetyFast ==
  /\ ConstructorSafety
  /\ FrontierInfoCopySafety
  /\ WireRebuildSafety
  /\ RebroadcastSafety

ConstructorAnchors ==
  /\ ConstructorSafety
  /\ PlainBorrowedNewBlockFrontier = NoFrontier
  /\ PlainOwnedNewBlockFrontier = NoFrontier
  /\ PlainSignedBlockFrontier = NoFrontier
  /\ WithFrontierBlock = Block
  /\ WithFrontierFrontier = Frontier

FrontierInfoCopyAnchors ==
  /\ FrontierInfoCopySafety
  /\ FrontierHighestQc = ProposalHighestQc
  /\ FrontierPayloadHash = ProposalPayloadHash
  /\ FrontierProposer = ProposalProposer
  /\ FrontierEpoch = ProposalEpoch
  /\ FrontierRosterHash = InitRosterHash
  /\ FrontierTotalChunks = InitTotalChunks
  /\ FrontierChunkDigests = InitChunkDigests
  /\ FrontierChunkRoot = InitChunkRoot
  /\ FrontierLeaderSignature = InitLeaderSignature

WireRebuildAnchors ==
  /\ WireRebuildSafety
  /\ GenericWireCachedFrontier = Frontier
  /\ GenericWireAuthoritativeFrontier = Frontier
  /\ GenericWireNoMetadataFrontier = NoFrontier
  /\ ProposalWireBeforeAuthoritativeCache = NoFrontier
  /\ ProposalWireMatchingAuthoritativeCache = Frontier
  /\ ProposalWireMismatchedAuthoritativeCache = NoFrontier
  /\ ProposalWirePayloadMismatch = NoFrontier
  /\ ProposalWireRebuiltPayloadMismatch = NoFrontier
  /\ ProposalWireRebuiltEpochMismatch = NoFrontier
  /\ LocalWireSeedsAuthoritativeFrontier = Frontier
  /\ LocalWireAuthoritativeFallbackFrontier = Frontier
  /\ LocalWireMissingProposerSignature = FirstBlockSignature
  /\ LocalWireLiveRosterFallbackFrontier = Frontier

RebroadcastAnchors ==
  /\ RebroadcastSafety
  /\ LocallyAuthoritativeFilter(TRUE, TRUE, TRUE)
  /\ ~LocallyAuthoritativeFilter(FALSE, TRUE, TRUE)
  /\ ~LocallyAuthoritativeFilter(TRUE, FALSE, TRUE)
  /\ ~LocallyAuthoritativeFilter(TRUE, TRUE, FALSE)
  /\ ~CachedProposalWrongLeaderRebroadcast
  /\ ~HintWrongBlockRebroadcast
  /\ ~MissingLocalTopologyRebroadcast
  /\ ~NoMetadataRebroadcast
  /\ ~MissingFrontierWireRebroadcast

BlockCreatedFrontierWireSafetyAnchors ==
  /\ ConstructorAnchors
  /\ FrontierInfoCopyAnchors
  /\ WireRebuildAnchors
  /\ RebroadcastAnchors

BlockCreatedConstructorExactness ==
  /\ ConstructorSafety
  /\ ConstructorAnchors

BlockCreatedFrontierInfoCopyExactness ==
  /\ FrontierInfoCopySafety
  /\ FrontierInfoCopyAnchors

BlockCreatedWireRebuildExactness ==
  /\ WireRebuildSafety
  /\ WireRebuildAnchors

BlockCreatedRebroadcastExactness ==
  /\ RebroadcastSafety
  /\ RebroadcastAnchors

BlockCreatedFrontierWireExactness ==
  /\ SafetyFast
  /\ BlockCreatedConstructorExactness
  /\ BlockCreatedFrontierInfoCopyExactness
  /\ BlockCreatedWireRebuildExactness
  /\ BlockCreatedRebroadcastExactness
  /\ BlockCreatedFrontierWireSafetyAnchors

Safety == BlockCreatedFrontierWireExactness

====
