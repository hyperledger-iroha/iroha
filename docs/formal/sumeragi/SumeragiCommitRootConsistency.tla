---- MODULE SumeragiCommitRootConsistency ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for commit-QC execution-root consistency.

Commit votes carry parent/post state roots. QC aggregation must filter votes to
one execution-root group before quorum is evaluated. Permissioned mode selects
the largest same-root signer group with a deterministic low-root tie-break.
NPoS selects the heaviest same-root stake group with the same tie-break. Votes
from a different context are ignored, and QC validation rejects a signer whose
recorded vote roots do not match the QC roots.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxValidators,
  \* @type: Int;
  MaxStake,
  \* @type: Bool;
  BugMixRootSigners,
  \* @type: Bool;
  BugCountWrongContext,
  \* @type: Bool;
  BugTieHighRoot,
  \* @type: Bool;
  BugStakeIgnoresWeight,
  \* @type: Bool;
  BugUnderQuorumAccept,
  \* @type: Bool;
  BugValidateMismatchedRoots

VARIABLES
  \* @type: Str;
  mode,
  \* @type: Int;
  validators,
  \* @type: Int;
  totalStake,
  \* @type: Int;
  rootASigners,
  \* @type: Int;
  rootBSigners,
  \* @type: Int;
  wrongContextSigners,
  \* @type: Int;
  rootAStake,
  \* @type: Int;
  rootBStake,
  \* @type: Int;
  wrongContextStake,
  \* @type: Str;
  selectedRoot,
  \* @type: Int;
  selectedSigners,
  \* @type: Int;
  selectedStake,
  \* @type: Bool;
  accepted,
  \* @type: Str;
  validationVoteRoot,
  \* @type: Str;
  validationQcRoot,
  \* @type: Bool;
  validated

vars == <<
  mode,
  validators,
  totalStake,
  rootASigners,
  rootBSigners,
  wrongContextSigners,
  rootAStake,
  rootBStake,
  wrongContextStake,
  selectedRoot,
  selectedSigners,
  selectedStake,
  accepted,
  validationVoteRoot,
  validationQcRoot,
  validated
>>

Modes == {"Permissioned", "Npos"}
Roots == {"None", "A", "B"}
ConcreteRoots == {"A", "B"}

CommitQuorum(n) ==
  IF n <= 3 THEN n ELSE (n * 2) \div 3 + 1

StakeQuorum(stake, total) ==
  /\ total > 0
  /\ stake >= 0
  /\ stake <= total
  /\ stake * 3 > total * 2

SelectByCount(a, b, highTie) ==
  IF a = 0 /\ b = 0 THEN "None"
  ELSE IF a > b THEN "A"
  ELSE IF b > a THEN "B"
  ELSE IF highTie THEN "B" ELSE "A"

SelectByStake(a, b, highTie) ==
  IF a = 0 /\ b = 0 THEN "None"
  ELSE IF a > b THEN "A"
  ELSE IF b > a THEN "B"
  ELSE IF highTie THEN "B" ELSE "A"

SignersFor(root, a, b) ==
  CASE root = "A" -> a
    [] root = "B" -> b
    [] OTHER -> 0

StakeFor(root, a, b) ==
  CASE root = "A" -> a
    [] root = "B" -> b
    [] OTHER -> 0

SpecSelectedRoot(m, a, b, aStake, bStake) ==
  IF m = "Npos"
  THEN SelectByStake(aStake, bStake, FALSE)
  ELSE SelectByCount(a, b, FALSE)

PolicySelectedRoot(m, a, b, wrong, aStake, bStake, wrongStake) ==
  LET effectiveA == a + IF BugCountWrongContext THEN wrong ELSE 0
      effectiveAStake == aStake + IF BugCountWrongContext THEN wrongStake ELSE 0
  IN
    IF m = "Npos"
    THEN
      IF BugStakeIgnoresWeight
      THEN SelectByCount(effectiveA, b, BugTieHighRoot)
      ELSE SelectByStake(effectiveAStake, bStake, BugTieHighRoot)
    ELSE SelectByCount(effectiveA, b, BugTieHighRoot)

SpecAccepted(m, validators_, total, a, b, aStake, bStake) ==
  LET root == SpecSelectedRoot(m, a, b, aStake, bStake)
  IN
    IF m = "Npos"
    THEN StakeQuorum(StakeFor(root, aStake, bStake), total)
    ELSE SignersFor(root, a, b) >= CommitQuorum(validators_)

PolicyAccepted(m, validators_, total, a, b, wrong, aStake, bStake, wrongStake) ==
  LET effectiveA == a + IF BugCountWrongContext THEN wrong ELSE 0
      effectiveAStake == aStake + IF BugCountWrongContext THEN wrongStake ELSE 0
      root == PolicySelectedRoot(m, a, b, wrong, aStake, bStake, wrongStake)
      rootSigners == SignersFor(root, effectiveA, b)
      rootStake == StakeFor(root, effectiveAStake, bStake)
      quorumSigners == IF BugMixRootSigners THEN effectiveA + b ELSE rootSigners
      quorumStake == IF BugMixRootSigners THEN effectiveAStake + bStake ELSE rootStake
  IN
    IF m = "Npos"
    THEN
      IF BugUnderQuorumAccept
      THEN StakeQuorum(quorumStake + 1, total)
      ELSE StakeQuorum(quorumStake, total)
    ELSE
      IF BugUnderQuorumAccept
      THEN quorumSigners + 1 >= CommitQuorum(validators_)
      ELSE quorumSigners >= CommitQuorum(validators_)

ValidationPolicy(voteRoot, qcRoot) ==
  IF BugValidateMismatchedRoots THEN TRUE ELSE voteRoot = qcRoot

InputConstraints(validators_, total, a, b, wrong, aStake, bStake, wrongStake) ==
  /\ a + b + wrong <= validators_
  /\ aStake + bStake + wrongStake <= total
  /\ a = 0 => aStake = 0
  /\ b = 0 => bStake = 0
  /\ wrong = 0 => wrongStake = 0
  /\ a > 0 => aStake > 0
  /\ b > 0 => bStake > 0
  /\ wrong > 0 => wrongStake > 0

TypeInvariant ==
  /\ MaxValidators \in Nat
  /\ MaxValidators >= 4
  /\ MaxStake \in Nat
  /\ MaxStake >= 7
  /\ BugMixRootSigners \in BOOLEAN
  /\ BugCountWrongContext \in BOOLEAN
  /\ BugTieHighRoot \in BOOLEAN
  /\ BugStakeIgnoresWeight \in BOOLEAN
  /\ BugUnderQuorumAccept \in BOOLEAN
  /\ BugValidateMismatchedRoots \in BOOLEAN
  /\ mode \in Modes
  /\ validators \in 1..MaxValidators
  /\ totalStake \in 1..MaxStake
  /\ rootASigners \in 0..validators
  /\ rootBSigners \in 0..validators
  /\ wrongContextSigners \in 0..validators
  /\ rootAStake \in 0..totalStake
  /\ rootBStake \in 0..totalStake
  /\ wrongContextStake \in 0..totalStake
  /\ InputConstraints(
       validators,
       totalStake,
       rootASigners,
       rootBSigners,
       wrongContextSigners,
       rootAStake,
       rootBStake,
       wrongContextStake
     )
  /\ selectedRoot \in Roots
  /\ selectedSigners \in 0..validators
  /\ selectedStake \in 0..totalStake
  /\ accepted \in BOOLEAN
  /\ validationVoteRoot \in ConcreteRoots
  /\ validationQcRoot \in ConcreteRoots
  /\ validated \in BOOLEAN

Init ==
  /\ mode = "Permissioned"
  /\ validators = 1
  /\ totalStake = 1
  /\ rootASigners = 0
  /\ rootBSigners = 0
  /\ wrongContextSigners = 0
  /\ rootAStake = 0
  /\ rootBStake = 0
  /\ wrongContextStake = 0
  /\ selectedRoot = "None"
  /\ selectedSigners = 0
  /\ selectedStake = 0
  /\ accepted = FALSE
  /\ validationVoteRoot = "A"
  /\ validationQcRoot = "A"
  /\ validated = TRUE

Evaluate(m, validators_, total, a, b, wrong, aStake, bStake, wrongStake, voteRoot, qcRoot) ==
  LET root == PolicySelectedRoot(m, a, b, wrong, aStake, bStake, wrongStake)
      effectiveA == a + IF BugCountWrongContext THEN wrong ELSE 0
      effectiveAStake == aStake + IF BugCountWrongContext THEN wrongStake ELSE 0
  IN
    /\ InputConstraints(validators_, total, a, b, wrong, aStake, bStake, wrongStake)
    /\ mode' = m
    /\ validators' = validators_
    /\ totalStake' = total
    /\ rootASigners' = a
    /\ rootBSigners' = b
    /\ wrongContextSigners' = wrong
    /\ rootAStake' = aStake
    /\ rootBStake' = bStake
    /\ wrongContextStake' = wrongStake
    /\ selectedRoot' = root
    /\ selectedSigners' = SignersFor(root, effectiveA, b)
    /\ selectedStake' = StakeFor(root, effectiveAStake, bStake)
    /\ accepted' = PolicyAccepted(m, validators_, total, a, b, wrong, aStake, bStake, wrongStake)
    /\ validationVoteRoot' = voteRoot
    /\ validationQcRoot' = qcRoot
    /\ validated' = ValidationPolicy(voteRoot, qcRoot)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E m \in Modes:
       \E validators_ \in 1..MaxValidators:
         \E total \in 1..MaxStake:
           \E a \in 0..validators_:
             \E b \in 0..validators_:
               \E wrong \in 0..validators_:
                 \E aStake \in 0..total:
                   \E bStake \in 0..total:
                     \E wrongStake \in 0..total:
                       \E voteRoot \in ConcreteRoots:
                         \E qcRoot \in ConcreteRoots:
                           Evaluate(
                             m,
                             validators_,
                             total,
                             a,
                             b,
                             wrong,
                             aStake,
                             bStake,
                             wrongStake,
                             voteRoot,
                             qcRoot
                           )
  \/ Stable

SelectedRootMatchesSpec ==
  selectedRoot =
    SpecSelectedRoot(mode, rootASigners, rootBSigners, rootAStake, rootBStake)

SelectedEvidenceMatchesSpecRoot ==
  /\ selectedSigners =
     SignersFor(
       SpecSelectedRoot(mode, rootASigners, rootBSigners, rootAStake, rootBStake),
       rootASigners,
       rootBSigners
     )
  /\ selectedStake =
     StakeFor(
       SpecSelectedRoot(mode, rootASigners, rootBSigners, rootAStake, rootBStake),
       rootAStake,
       rootBStake
     )

AcceptedMatchesSpec ==
  accepted =
    SpecAccepted(
      mode,
      validators,
      totalStake,
      rootASigners,
      rootBSigners,
      rootAStake,
      rootBStake
    )

MixedRootsCannotSatisfyPermissionedQuorum ==
  /\ mode = "Permissioned"
  /\ rootASigners < CommitQuorum(validators)
  /\ rootBSigners < CommitQuorum(validators)
  /\ rootASigners + rootBSigners >= CommitQuorum(validators)
  => ~accepted

MixedRootsCannotSatisfyStakeQuorum ==
  /\ mode = "Npos"
  /\ ~StakeQuorum(rootAStake, totalStake)
  /\ ~StakeQuorum(rootBStake, totalStake)
  /\ StakeQuorum(rootAStake + rootBStake, totalStake)
  => ~accepted

WrongContextCannotSatisfyRootQuorum ==
  /\ mode = "Permissioned"
  /\ rootBSigners = 0
  /\ rootASigners < CommitQuorum(validators)
  /\ rootASigners + wrongContextSigners >= CommitQuorum(validators)
  => ~accepted

WrongContextCannotSatisfyStakeQuorum ==
  /\ mode = "Npos"
  /\ rootBStake = 0
  /\ ~StakeQuorum(rootAStake, totalStake)
  /\ StakeQuorum(rootAStake + wrongContextStake, totalStake)
  => ~accepted

ValidationRootMismatchRejected ==
  validationVoteRoot # validationQcRoot => ~validated

ValidatedMatchesSpec ==
  validated = (validationVoteRoot = validationQcRoot)

CommitRootSelectionExact ==
  /\ SelectedRootMatchesSpec
  /\ SelectedEvidenceMatchesSpecRoot

CommitRootQuorumExact ==
  /\ AcceptedMatchesSpec
  /\ MixedRootsCannotSatisfyPermissionedQuorum
  /\ MixedRootsCannotSatisfyStakeQuorum
  /\ WrongContextCannotSatisfyRootQuorum
  /\ WrongContextCannotSatisfyStakeQuorum

CommitRootValidationExact ==
  /\ ValidationRootMismatchRejected
  /\ ValidatedMatchesSpec

CommitRootConsistencyExactness ==
  /\ CommitRootSelectionExact
  /\ CommitRootQuorumExact
  /\ CommitRootValidationExact

CommitRootConsistencyCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CommitRootConsistencyExactness

TlcWitnessState ==
  \/ Init
  \/ /\ validators = 4
     /\ totalStake = 7
     /\ validationVoteRoot \in ConcreteRoots
     /\ validationQcRoot \in ConcreteRoots
     /\ \/ /\ mode = "Permissioned"
           /\ rootASigners = 2
           /\ rootBSigners = 1
           /\ wrongContextSigners = 0
           /\ rootAStake = 2
           /\ rootBStake = 1
           /\ wrongContextStake = 0
        \/ /\ mode = "Permissioned"
           /\ rootASigners = 2
           /\ rootBSigners = 0
           /\ wrongContextSigners = 1
           /\ rootAStake = 2
           /\ rootBStake = 0
           /\ wrongContextStake = 1
        \/ /\ mode = "Permissioned"
           /\ rootASigners = 1
           /\ rootBSigners = 1
           /\ wrongContextSigners = 0
           /\ rootAStake = 1
           /\ rootBStake = 1
           /\ wrongContextStake = 0
        \/ /\ mode = "Permissioned"
           /\ rootASigners = 2
           /\ rootBSigners = 0
           /\ wrongContextSigners = 0
           /\ rootAStake = 2
           /\ rootBStake = 0
           /\ wrongContextStake = 0
        \/ /\ mode = "Npos"
           /\ rootASigners = 1
           /\ rootBSigners = 1
           /\ wrongContextSigners = 0
           /\ rootAStake = 2
           /\ rootBStake = 5
           /\ wrongContextStake = 0
        \/ /\ mode = "Npos"
           /\ rootASigners = 1
           /\ rootBSigners = 1
           /\ wrongContextSigners = 0
           /\ rootAStake = 3
           /\ rootBStake = 3
           /\ wrongContextStake = 0

====
