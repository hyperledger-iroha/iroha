---- MODULE SumeragiQuorumPolicy ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi quorum-policy arithmetic.

The permissioned policy accepts only a strict supermajority count:
floor(2 * validators / 3) + 1, with zero validators rejected and signer counts
above the active validator count rejected.

The NPoS policy accepts only signed stake that strictly exceeds two thirds of
total stake. Missing/negative stake, zero/negative total stake, signed stake
above total stake, exact two-thirds stake, and arithmetic overflow all fail
closed.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxValidators,
  \* @type: Int;
  MaxStake,
  \* @type: Bool;
  BugCountAllowsUnderThreshold,
  \* @type: Bool;
  BugCountAllowsOverValidatorCount,
  \* @type: Bool;
  BugStakeAllowsExactTwoThirds,
  \* @type: Bool;
  BugStakeAllowsOverTotal,
  \* @type: Bool;
  BugStakeAllowsInvalidInput,
  \* @type: Bool;
  BugStakeIgnoresOverflow

VARIABLES
  \* @type: Int;
  validators,
  \* @type: Int;
  signedCount,
  \* @type: Int;
  totalStake,
  \* @type: Int;
  signedStake,
  \* @type: Bool;
  stakeOverflow,
  \* @type: Bool;
  countAccepted,
  \* @type: Bool;
  stakeAccepted

vars == <<
  validators,
  signedCount,
  totalStake,
  signedStake,
  stakeOverflow,
  countAccepted,
  stakeAccepted
>>

CountValues == 0..(MaxValidators + 1)
StakeValues == -1..(MaxStake + 1)

PermissionedThreshold(n) ==
  (n * 2) \div 3 + 1

CountSpecSatisfied ==
  /\ validators > 0
  /\ signedCount <= validators
  /\ signedCount >= PermissionedThreshold(validators)

CountPolicySatisfied ==
  IF BugCountAllowsUnderThreshold
  THEN
    /\ validators > 0
    /\ signedCount <= validators
    /\ signedCount >= PermissionedThreshold(validators) - 1
  ELSE IF BugCountAllowsOverValidatorCount
  THEN
    /\ validators > 0
    /\ signedCount >= PermissionedThreshold(validators)
  ELSE
    CountSpecSatisfied

StakeSpecSatisfied ==
  /\ ~stakeOverflow
  /\ totalStake > 0
  /\ signedStake >= 0
  /\ signedStake <= totalStake
  /\ signedStake * 3 > totalStake * 2

StakePolicySatisfied ==
  IF BugStakeAllowsInvalidInput
  THEN
    \/ StakeSpecSatisfied
    \/ signedStake < 0
    \/ totalStake <= 0
  ELSE IF BugStakeAllowsOverTotal
  THEN
    /\ ~stakeOverflow
    /\ totalStake > 0
    /\ signedStake >= 0
    /\ signedStake * 3 > totalStake * 2
  ELSE IF BugStakeAllowsExactTwoThirds
  THEN
    /\ ~stakeOverflow
    /\ totalStake > 0
    /\ signedStake >= 0
    /\ signedStake <= totalStake
    /\ signedStake * 3 >= totalStake * 2
  ELSE IF BugStakeIgnoresOverflow
  THEN
    /\ totalStake > 0
    /\ signedStake >= 0
    /\ signedStake <= totalStake
    /\ signedStake * 3 > totalStake * 2
  ELSE
    StakeSpecSatisfied

TypeInvariant ==
  /\ MaxValidators \in Nat
  /\ MaxValidators >= 4
  /\ MaxStake \in Nat
  /\ MaxStake >= 6
  /\ BugCountAllowsUnderThreshold \in BOOLEAN
  /\ BugCountAllowsOverValidatorCount \in BOOLEAN
  /\ BugStakeAllowsExactTwoThirds \in BOOLEAN
  /\ BugStakeAllowsOverTotal \in BOOLEAN
  /\ BugStakeAllowsInvalidInput \in BOOLEAN
  /\ BugStakeIgnoresOverflow \in BOOLEAN
  /\ validators \in 0..MaxValidators
  /\ signedCount \in CountValues
  /\ totalStake \in StakeValues
  /\ signedStake \in StakeValues
  /\ stakeOverflow \in BOOLEAN
  /\ countAccepted \in BOOLEAN
  /\ stakeAccepted \in BOOLEAN

Init ==
  /\ validators = 0
  /\ signedCount = 0
  /\ totalStake = 0
  /\ signedStake = -1
  /\ stakeOverflow = FALSE
  /\ countAccepted = FALSE
  /\ stakeAccepted = FALSE

Evaluate(v, c, total, signed, overflow) ==
  /\ validators' = v
  /\ signedCount' = c
  /\ totalStake' = total
  /\ signedStake' = signed
  /\ stakeOverflow' = overflow
  /\ countAccepted' = CountPolicySatisfied'
  /\ stakeAccepted' = StakePolicySatisfied'

Stable ==
  UNCHANGED vars

Next ==
  \/ \E v \in 0..MaxValidators:
       \E c \in CountValues:
         \E total \in StakeValues:
           \E signed \in StakeValues:
             \E overflow \in BOOLEAN:
               Evaluate(v, c, total, signed, overflow)
  \/ Stable

CountMatchesStrictSupermajority ==
  countAccepted = CountSpecSatisfied

CountRejectsOverValidatorCount ==
  signedCount > validators => ~countAccepted

StakeMatchesStrictSupermajority ==
  stakeAccepted = StakeSpecSatisfied

ExactTwoThirdsStakeRejected ==
  /\ totalStake > 0
  /\ signedStake * 3 = totalStake * 2
  => ~stakeAccepted

StakeRejectsInvalidInputs ==
  (totalStake <= 0 \/ signedStake < 0 \/ stakeOverflow) => ~stakeAccepted

StakeRejectsOverTotal ==
  signedStake > totalStake => ~stakeAccepted

====
