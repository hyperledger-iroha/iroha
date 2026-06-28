---- MODULE SumeragiMissingBlockClearGate ----

(***************************************************************************
A bounded abstract model for `missing_block_clear_allowed(...)`.

The helper is intentionally small: payload-available clears are accepted only
after the block is known locally, while obsolete clears are accepted regardless
of local payload availability.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PayloadAvailable == 0
Obsolete == 1

\* @type: (Bool, Int) => Bool;
SpecAllowed(blockKnown, reason) ==
  IF reason = PayloadAvailable THEN blockKnown ELSE TRUE

\* @type: (Bool, Int) => Bool;
ActualAllowed(blockKnown, reason) ==
  IF Bug = 1 /\ reason = PayloadAvailable /\ ~blockKnown THEN TRUE
  ELSE IF Bug = 2 /\ reason = PayloadAvailable /\ blockKnown THEN FALSE
  ELSE IF Bug = 3 /\ reason = Obsolete /\ ~blockKnown THEN FALSE
  ELSE IF Bug = 4 /\ reason = Obsolete /\ blockKnown THEN FALSE
  ELSE IF Bug = 5
  THEN IF reason = PayloadAvailable THEN TRUE ELSE blockKnown
  ELSE IF Bug = 6 THEN blockKnown
  ELSE IF Bug = 7 THEN TRUE
  ELSE SpecAllowed(blockKnown, reason)

SpecPayloadKnown ==
  SpecAllowed(TRUE, PayloadAvailable)

ActualPayloadKnown ==
  ActualAllowed(TRUE, PayloadAvailable)

SpecPayloadMissing ==
  SpecAllowed(FALSE, PayloadAvailable)

ActualPayloadMissing ==
  ActualAllowed(FALSE, PayloadAvailable)

SpecObsoleteKnown ==
  SpecAllowed(TRUE, Obsolete)

ActualObsoleteKnown ==
  ActualAllowed(TRUE, Obsolete)

SpecObsoleteMissing ==
  SpecAllowed(FALSE, Obsolete)

ActualObsoleteMissing ==
  ActualAllowed(FALSE, Obsolete)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

\* @type: <<Bool, Bool, Bool, Bool>>;
SpecOutput ==
  <<SpecPayloadKnown, SpecPayloadMissing, SpecObsoleteKnown,
    SpecObsoleteMissing>>

\* @type: <<Bool, Bool, Bool, Bool>>;
ActualOutput ==
  <<ActualPayloadKnown, ActualPayloadMissing, ActualObsoleteKnown,
    ActualObsoleteMissing>>

MissingBlockClearMatchesSpec ==
  ActualOutput = SpecOutput

MissingBlockClearExactness ==
  MissingBlockClearMatchesSpec

MissingBlockClearCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MissingBlockClearExactness

SafetyFast ==
  MissingBlockClearExactness

BugPayloadAvailableWithoutLocalRejected ==
  ActualPayloadMissing = SpecPayloadMissing

BugPayloadAvailableWithLocalAllowed ==
  ActualPayloadKnown = SpecPayloadKnown

BugObsoleteWithoutLocalAllowed ==
  ActualObsoleteMissing = SpecObsoleteMissing

BugObsoleteWithLocalAllowed ==
  ActualObsoleteKnown = SpecObsoleteKnown

BugReasonsNotSwapped ==
  ActualPayloadMissing = SpecPayloadMissing

BugBothReasonsDoNotRequireLocal ==
  ActualObsoleteMissing = SpecObsoleteMissing

BugBothReasonsNotAlwaysAllowed ==
  ActualPayloadMissing = SpecPayloadMissing

====
