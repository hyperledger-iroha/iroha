---- MODULE SumeragiInvalidQcShapeGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `check_invalid_commit_qc_shape(...)`.

This helper is intentionally only a shape-to-evidence constructor. It emits
`InvalidQc` evidence when a QC has an empty signer bitmap or when both view and
height are zero. It must not treat height zero alone or view zero alone as
invalid, must clone the supplied certificate into the evidence payload, and
must attach the fixed diagnostic reason used by the Rust helper.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoKind == 0
InvalidQc == 1
DoubleCommit == 2
NoPayload == 0
NoReason == 0
InvalidReason == 1

EmptyBitmapNonzero == 1
ZeroSentinelNonempty == 2
BothEmptyAndZero == 3
EmptyBitmapHeightZeroViewNonzero == 4
HeightZeroAloneNonempty == 5
ViewZeroAloneNonempty == 6
ValidNonemptyNonzero == 7

SpecEmits(c) ==
  c \in {
    EmptyBitmapNonzero,
    ZeroSentinelNonempty,
    BothEmptyAndZero,
    EmptyBitmapHeightZeroViewNonzero
  }

ActualEmits(c) ==
  CASE Bug = "empty_bitmap_not_detected"
       /\ c = EmptyBitmapNonzero -> FALSE
    [] Bug = "zero_sentinel_not_detected"
       /\ c = ZeroSentinelNonempty -> FALSE
    [] Bug = "requires_both_conditions"
       /\ c = EmptyBitmapNonzero -> FALSE
    [] Bug = "height_zero_alone_detected"
       /\ c = HeightZeroAloneNonempty -> TRUE
    [] Bug = "view_zero_alone_detected"
       /\ c = ViewZeroAloneNonempty -> TRUE
    [] Bug = "valid_nonempty_detected"
       /\ c = ValidNonemptyNonzero -> TRUE
    [] Bug = "empty_bitmap_height_zero_not_detected"
       /\ c = EmptyBitmapHeightZeroViewNonzero -> FALSE
    [] OTHER -> SpecEmits(c)

SpecKind(c) ==
  IF SpecEmits(c) THEN InvalidQc ELSE NoKind

ActualKind(c) ==
  CASE Bug = "wrong_kind"
       /\ c = EmptyBitmapNonzero -> DoubleCommit
    [] OTHER -> SpecKind(c)

SpecPayload(c) ==
  CASE c = EmptyBitmapNonzero -> 1
    [] c = ZeroSentinelNonempty -> 2
    [] c = BothEmptyAndZero -> 3
    [] c = EmptyBitmapHeightZeroViewNonzero -> 4
    [] OTHER -> NoPayload

ActualPayload(c) ==
  CASE Bug = "drops_certificate_height"
       /\ c = EmptyBitmapNonzero -> 5
    [] Bug = "drops_certificate_view"
       /\ c = EmptyBitmapNonzero -> 6
    [] Bug = "drops_certificate_bitmap"
       /\ c = EmptyBitmapNonzero -> 7
    [] Bug = "drops_certificate_subject"
       /\ c = EmptyBitmapNonzero -> 8
    [] Bug = "payload_for_nonemitting_case"
       /\ c = ValidNonemptyNonzero -> 9
    [] OTHER -> SpecPayload(c)

SpecReason(c) ==
  IF SpecEmits(c) THEN InvalidReason ELSE NoReason

ActualReason(c) ==
  CASE Bug = "wrong_reason"
       /\ c = EmptyBitmapNonzero -> 2
    [] Bug = "reason_omitted"
       /\ c = EmptyBitmapNonzero -> NoReason
    [] Bug = "reason_for_nonemitting_case"
       /\ c = ValidNonemptyNonzero -> InvalidReason
    [] OTHER -> SpecReason(c)

SpecEvidence(c) ==
  <<SpecEmits(c), SpecKind(c), SpecPayload(c), SpecReason(c)>>

ActualEvidence(c) ==
  <<ActualEmits(c), ActualKind(c), ActualPayload(c), ActualReason(c)>>

Matches(c) ==
  ActualEvidence(c) = SpecEvidence(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_bitmap_not_detected",
       "zero_sentinel_not_detected",
       "requires_both_conditions",
       "height_zero_alone_detected",
       "view_zero_alone_detected",
       "valid_nonempty_detected",
       "empty_bitmap_height_zero_not_detected",
       "wrong_kind",
       "drops_certificate_height",
       "drops_certificate_view",
       "drops_certificate_bitmap",
       "drops_certificate_subject",
       "payload_for_nonemitting_case",
       "wrong_reason",
       "reason_omitted",
       "reason_for_nonemitting_case"
     }
  /\ checked = 0

InvalidQcShapeMatchesSpec ==
  /\ Matches(EmptyBitmapNonzero)
  /\ Matches(ZeroSentinelNonempty)
  /\ Matches(BothEmptyAndZero)
  /\ Matches(EmptyBitmapHeightZeroViewNonzero)
  /\ Matches(HeightZeroAloneNonempty)
  /\ Matches(ViewZeroAloneNonempty)
  /\ Matches(ValidNonemptyNonzero)

InvalidQcShapeExactness ==
  InvalidQcShapeMatchesSpec

InvalidQcShapeCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ InvalidQcShapeExactness

SafetyFast ==
  InvalidQcShapeExactness

BugEmptyBitmapNotDetected ==
  Matches(EmptyBitmapNonzero)

BugZeroSentinelNotDetected ==
  Matches(ZeroSentinelNonempty)

BugRequiresBothConditions ==
  Matches(EmptyBitmapNonzero)

BugHeightZeroAloneDetected ==
  Matches(HeightZeroAloneNonempty)

BugViewZeroAloneDetected ==
  Matches(ViewZeroAloneNonempty)

BugValidNonemptyDetected ==
  Matches(ValidNonemptyNonzero)

BugEmptyBitmapHeightZeroNotDetected ==
  Matches(EmptyBitmapHeightZeroViewNonzero)

BugWrongKind ==
  Matches(EmptyBitmapNonzero)

BugDropsCertificateHeight ==
  Matches(EmptyBitmapNonzero)

BugDropsCertificateView ==
  Matches(EmptyBitmapNonzero)

BugDropsCertificateBitmap ==
  Matches(EmptyBitmapNonzero)

BugDropsCertificateSubject ==
  Matches(EmptyBitmapNonzero)

BugPayloadForNonemittingCase ==
  Matches(ValidNonemptyNonzero)

BugWrongReason ==
  Matches(EmptyBitmapNonzero)

BugReasonOmitted ==
  Matches(EmptyBitmapNonzero)

BugReasonForNonemittingCase ==
  Matches(ValidNonemptyNonzero)

====
