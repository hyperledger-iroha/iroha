---- MODULE SumeragiBlockMessageWireGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `BlockMessageWire`.

This slice pins the cached full-frame representation used by consensus network
payloads. Cached bytes must remain a self-describing Norito-framed
`BlockMessage`; mutation must drop the cache; serialization must use cached
bytes only when they are present; and decode-from-slice must validate the
header, compute the exact framed prefix including alignment padding, leave
trailing envelope bytes unconsumed, decode the embedded `BlockMessage`, and
cache exactly the consumed frame.
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
NewKeepsCache == 1
WithEncodedDropsCache == 2
WithEncodedOwnedDropsCache == 3
CachedSerializeReencodes == 4
UncachedSerializeUsesEmptyCache == 5
MakeMutKeepsCache == 6
IntoMessageUsesCache == 7
AcceptBadMagic == 8
AcceptBadMajor == 9
AcceptBadMinor == 10
AcceptBadSchema == 11
AcceptCompressed == 12
AcceptMissingLen == 13
AcceptLengthOverflow == 14
ConsumedOmitsPadding == 15
ConsumesTrailingBytes == 16
DecodeWrongMessage == 17
DecodeDropsCache == 18
TryDeserializeDropsCache == 19
CachedPayloadNotSelfDescribing == 20

Bugs == 0..20

NoCache == 0
WrappedMessage == 1
DecodedMessage == 2
WrongMessage == 3
CachedFrame == 10
EncodedWrappedFrame == 11

HeaderSize == 31
Padding == 1
PayloadLen == 96
TrailingLen == 7
GoodPrefixLen == HeaderSize + Padding + PayloadLen
InputLen == GoodPrefixLen + TrailingLen

NewCache ==
  IF Bug = NewKeepsCache THEN CachedFrame ELSE NoCache

WithEncodedCache ==
  IF Bug = WithEncodedDropsCache THEN NoCache ELSE CachedFrame

WithEncodedOwnedCache ==
  IF Bug = WithEncodedOwnedDropsCache THEN NoCache ELSE CachedFrame

SerializeCached ==
  IF Bug = CachedSerializeReencodes THEN EncodedWrappedFrame ELSE CachedFrame

SerializeUncached ==
  IF Bug = UncachedSerializeUsesEmptyCache THEN NoCache ELSE EncodedWrappedFrame

MakeMutCacheAfter ==
  IF Bug = MakeMutKeepsCache THEN CachedFrame ELSE NoCache

IntoMessageOutput ==
  IF Bug = IntoMessageUsesCache THEN DecodedMessage ELSE WrappedMessage

AcceptsFrame(magicOk, majorOk, minorOk, schemaOk, compressionOk, hasLen, lenFits, totalFits, payloadAvailable) ==
  /\ IF Bug = AcceptBadMagic THEN TRUE ELSE magicOk
  /\ IF Bug = AcceptBadMajor THEN TRUE ELSE majorOk
  /\ IF Bug = AcceptBadMinor THEN TRUE ELSE minorOk
  /\ IF Bug = AcceptBadSchema THEN TRUE ELSE schemaOk
  /\ IF Bug = AcceptCompressed THEN TRUE ELSE compressionOk
  /\ IF Bug = AcceptMissingLen THEN TRUE ELSE hasLen
  /\ IF Bug = AcceptLengthOverflow THEN TRUE ELSE lenFits
  /\ IF Bug = AcceptLengthOverflow THEN TRUE ELSE totalFits
  /\ IF Bug = AcceptLengthOverflow THEN TRUE ELSE payloadAvailable

ConsumedPrefixLen ==
  IF Bug = ConsumedOmitsPadding THEN HeaderSize + PayloadLen
  ELSE IF Bug = ConsumesTrailingBytes THEN InputLen
  ELSE GoodPrefixLen

DecodeFromSliceMessage ==
  IF Bug = DecodeWrongMessage THEN WrongMessage ELSE DecodedMessage

DecodeFromSliceCache ==
  IF Bug = DecodeDropsCache THEN NoCache ELSE CachedFrame

TryDeserializeCache ==
  IF Bug = TryDeserializeDropsCache THEN NoCache ELSE CachedFrame

CachedPayloadDecodedMessage ==
  IF Bug = CachedPayloadNotSelfDescribing THEN WrongMessage ELSE DecodedMessage

Init ==
  checked = 0

Next ==
  \/ /\ checked < 20
     /\ checked' = checked + 1
  \/ /\ checked = 20
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..20
  /\ HeaderSize \in Nat
  /\ Padding \in Nat
  /\ PayloadLen \in Nat
  /\ TrailingLen \in Nat
  /\ GoodPrefixLen < InputLen

CacheConstructionSafety ==
  /\ NewCache = NoCache
  /\ WithEncodedCache = CachedFrame
  /\ WithEncodedOwnedCache = CachedFrame

SerializationSafety ==
  /\ SerializeCached = CachedFrame
  /\ SerializeUncached = EncodedWrappedFrame
  /\ MakeMutCacheAfter = NoCache
  /\ IntoMessageOutput = WrappedMessage

FrameRejectionSafety ==
  /\ ~AcceptsFrame(FALSE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, FALSE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, FALSE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, TRUE, FALSE, TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, TRUE, TRUE, FALSE, TRUE, TRUE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, TRUE, TRUE, TRUE, FALSE, TRUE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, FALSE, TRUE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, FALSE, TRUE)
  /\ ~AcceptsFrame(TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, FALSE)

FramePrefixSafety ==
  /\ AcceptsFrame(TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ConsumedPrefixLen = GoodPrefixLen
  /\ ConsumedPrefixLen < InputLen

DecodeSafety ==
  /\ DecodeFromSliceMessage = DecodedMessage
  /\ DecodeFromSliceCache = CachedFrame
  /\ TryDeserializeCache = CachedFrame
  /\ CachedPayloadDecodedMessage = DecodedMessage

CacheConstructionAnchors ==
  /\ CacheConstructionSafety
  /\ NewCache = NoCache
  /\ WithEncodedCache = CachedFrame
  /\ WithEncodedOwnedCache = CachedFrame

SerializationAnchors ==
  /\ SerializationSafety
  /\ SerializeCached = CachedFrame
  /\ SerializeUncached = EncodedWrappedFrame
  /\ MakeMutCacheAfter = NoCache
  /\ IntoMessageOutput = WrappedMessage

FrameValidationAnchors ==
  /\ FrameRejectionSafety
  /\ FramePrefixSafety
  /\ AcceptsFrame(TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE, TRUE)
  /\ ConsumedPrefixLen = GoodPrefixLen
  /\ ConsumedPrefixLen < InputLen

DecodeAnchors ==
  /\ DecodeSafety
  /\ DecodeFromSliceMessage = DecodedMessage
  /\ DecodeFromSliceCache = CachedFrame
  /\ TryDeserializeCache = CachedFrame
  /\ CachedPayloadDecodedMessage = DecodedMessage

BlockMessageWireSafetyAnchors ==
  /\ CacheConstructionAnchors
  /\ SerializationAnchors
  /\ FrameValidationAnchors
  /\ DecodeAnchors

SafetyFast ==
  /\ CacheConstructionSafety
  /\ SerializationSafety
  /\ FrameRejectionSafety
  /\ FramePrefixSafety
  /\ DecodeSafety

WireCacheConstructionExactness ==
  /\ CacheConstructionSafety
  /\ CacheConstructionAnchors

WireSerializationExactness ==
  /\ SerializationSafety
  /\ SerializationAnchors

WireFrameValidationExactness ==
  /\ FrameRejectionSafety
  /\ FramePrefixSafety
  /\ FrameValidationAnchors

WireDecodeExactness ==
  /\ DecodeSafety
  /\ DecodeAnchors

BlockMessageWireExactness ==
  /\ SafetyFast
  /\ WireCacheConstructionExactness
  /\ WireSerializationExactness
  /\ WireFrameValidationExactness
  /\ WireDecodeExactness
  /\ BlockMessageWireSafetyAnchors

Safety == BlockMessageWireExactness

====
