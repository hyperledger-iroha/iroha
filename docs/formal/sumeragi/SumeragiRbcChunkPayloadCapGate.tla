---- MODULE SumeragiRbcChunkPayloadCapGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `rbc_chunk_payload_cap(...)`.

This slice pins the deterministic frame-cap arithmetic used before configuring
DA/RBC chunk sizes:
- if the encoded empty RBC chunk base frame already reaches the payload frame
  cap, the helper fails closed with a zero payload cap,
- otherwise it subtracts the base frame length and the fixed 64-byte nested
  Norito alignment headroom with saturating arithmetic,
- the exact boundary at base-plus-headroom still returns zero,
- any positive returned cap keeps `base_len + cap + headroom` within the
  payload frame cap, and
- the caller must use the consensus payload frame cap, not the larger control
  frame cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Headroom == 64

BaseOverFrame == "base_over_frame"
BaseEqualFrame == "base_equal_frame"
BelowHeadroomAfterBase == "below_headroom_after_base"
ExactHeadroomAfterBase == "exact_headroom_after_base"
OneByteAfterHeadroom == "one_byte_after_headroom"
LargePayload == "large_payload"
ZeroFrameZeroBase == "zero_frame_zero_base"
ZeroBaseAboveHeadroom == "zero_base_above_headroom"
PayloadSourceDiffers == "payload_source_differs"
BaseGrowthClamps == "base_growth_clamps"
BaseGrowthLeavesRoom == "base_growth_leaves_room"

Cases == {
  BaseOverFrame,
  BaseEqualFrame,
  BelowHeadroomAfterBase,
  ExactHeadroomAfterBase,
  OneByteAfterHeadroom,
  LargePayload,
  ZeroFrameZeroBase,
  ZeroBaseAboveHeadroom,
  PayloadSourceDiffers,
  BaseGrowthClamps,
  BaseGrowthLeavesRoom
}

BaseLen(c) ==
  CASE c = BaseOverFrame -> 120
    [] c = BaseEqualFrame -> 100
    [] c = BelowHeadroomAfterBase -> 100
    [] c = ExactHeadroomAfterBase -> 100
    [] c = OneByteAfterHeadroom -> 100
    [] c = LargePayload -> 120
    [] c = ZeroFrameZeroBase -> 0
    [] c = ZeroBaseAboveHeadroom -> 0
    [] c = PayloadSourceDiffers -> 80
    [] c = BaseGrowthClamps -> 180
    [] c = BaseGrowthLeavesRoom -> 100
    [] OTHER -> 0

PayloadFrameCap(c) ==
  CASE c = BaseOverFrame -> 100
    [] c = BaseEqualFrame -> 100
    [] c = BelowHeadroomAfterBase -> 150
    [] c = ExactHeadroomAfterBase -> 164
    [] c = OneByteAfterHeadroom -> 165
    [] c = LargePayload -> 512
    [] c = ZeroFrameZeroBase -> 0
    [] c = ZeroBaseAboveHeadroom -> 80
    [] c = PayloadSourceDiffers -> 160
    [] c = BaseGrowthClamps -> 200
    [] c = BaseGrowthLeavesRoom -> 200
    [] OTHER -> 0

\* A larger non-payload cap used only to catch caller/source mixups.
ControlFrameCap(c) ==
  CASE c = PayloadSourceDiffers -> 260
    [] OTHER -> PayloadFrameCap(c)

SaturatingSub(a, b) == IF a <= b THEN 0 ELSE a - b

SpecCap(c) ==
  IF BaseLen(c) >= PayloadFrameCap(c)
  THEN 0
  ELSE SaturatingSub(SaturatingSub(PayloadFrameCap(c), BaseLen(c)), Headroom)

SpecFit(c) ==
  SpecCap(c) = 0
    \/ BaseLen(c) + SpecCap(c) + Headroom <= PayloadFrameCap(c)

ActualCap(c) ==
  CASE Bug = "base_overflow_returns_positive"
       /\ c = BaseOverFrame -> 1
    [] Bug = "base_equal_returns_one"
       /\ c = BaseEqualFrame -> 1
    [] Bug = "below_headroom_underflows"
       /\ c = BelowHeadroomAfterBase ->
      Headroom - SaturatingSub(PayloadFrameCap(c), BaseLen(c))
    [] Bug = "exact_headroom_allows_one"
       /\ c = ExactHeadroomAfterBase -> 1
    [] Bug = "omits_headroom"
       /\ c \in {OneByteAfterHeadroom, LargePayload, BaseGrowthLeavesRoom} ->
      SaturatingSub(PayloadFrameCap(c), BaseLen(c))
    [] Bug = "double_subtracts_headroom"
       /\ c = LargePayload ->
      SaturatingSub(SaturatingSub(PayloadFrameCap(c), BaseLen(c)),
                    Headroom + Headroom)
    [] Bug = "omits_base_len"
       /\ c = BaseGrowthLeavesRoom ->
      SaturatingSub(PayloadFrameCap(c), Headroom)
    [] Bug = "uses_control_frame_cap"
       /\ c = PayloadSourceDiffers ->
      SaturatingSub(SaturatingSub(ControlFrameCap(c), BaseLen(c)), Headroom)
    [] Bug = "off_by_one_positive_cap"
       /\ c = OneByteAfterHeadroom -> 2
    [] Bug = "large_cap_not_maximal"
       /\ c = LargePayload -> SpecCap(c) - 1
    [] Bug = "zero_frame_allows_payload"
       /\ c = ZeroFrameZeroBase -> 1
    [] Bug = "zero_base_omits_headroom"
       /\ c = ZeroBaseAboveHeadroom -> PayloadFrameCap(c)
    [] OTHER -> SpecCap(c)

ActualFit(c) ==
  ActualCap(c) = 0
    \/ BaseLen(c) + ActualCap(c) + Headroom <= PayloadFrameCap(c)

Bugs == {
  "none",
  "base_overflow_returns_positive",
  "base_equal_returns_one",
  "below_headroom_underflows",
  "exact_headroom_allows_one",
  "omits_headroom",
  "double_subtracts_headroom",
  "omits_base_len",
  "uses_control_frame_cap",
  "off_by_one_positive_cap",
  "large_cap_not_maximal",
  "zero_frame_allows_payload",
  "zero_base_omits_headroom"
}

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs
  /\ \A c \in Cases:
       /\ BaseLen(c) \in 0..512
       /\ PayloadFrameCap(c) \in 0..512
       /\ ControlFrameCap(c) \in 0..512
       /\ SpecCap(c) \in 0..512
       /\ ActualCap(c) \in 0..512
       /\ SpecFit(c)

RbcChunkPayloadCapCoreSafety ==
  \A c \in Cases:
    /\ ActualCap(c) = SpecCap(c)
    /\ ActualFit(c)

RbcChunkPayloadCapExactness == RbcChunkPayloadCapCoreSafety

RbcChunkPayloadCapCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcChunkPayloadCapExactness

NoBugInvariant == RbcChunkPayloadCapExactness

SafetyFast == RbcChunkPayloadCapExactness

BugBaseOverflowReturnsPositive == NoBugInvariant
BugBaseEqualReturnsOne == NoBugInvariant
BugBelowHeadroomUnderflows == NoBugInvariant
BugExactHeadroomAllowsOne == NoBugInvariant
BugOmitsHeadroom == NoBugInvariant
BugDoubleSubtractsHeadroom == NoBugInvariant
BugOmitsBaseLen == NoBugInvariant
BugUsesControlFrameCap == NoBugInvariant
BugOffByOnePositiveCap == NoBugInvariant
BugLargeCapNotMaximal == NoBugInvariant
BugZeroFrameAllowsPayload == NoBugInvariant
BugZeroBaseOmitsHeadroom == NoBugInvariant

====
