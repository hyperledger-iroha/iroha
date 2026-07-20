---- MODULE SumeragiV2SuccessorStaleTokenMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Depth-one adversarial projection for the successor-start boundary.

The initial state is shape-valid and has a durable parent witness, but it
carries two stale consumer artifacts: a non-empty prerequisite set and the
exact Applied token that must only be bound after Begin.  The deliberately
buggy Begin omits both guards.  Its single transition exposes Running, makes
the stale token credential-ready, and reaches the pipeline CASE fallback at
distance zero, violating the projected protocol invariant at depth one.

The repaired Begin is disabled in that state.  FailClosed remains enabled,
removes the stale artifacts, records durable failure history, and strictly
decreases the rank from 19 to 9.  The two configurations below deliberately
separate the red buggy witness from the green fixed/fail-closed corridor.
***************************************************************************)

VARIABLES
  activationStatus,
  predecessorOwnership,
  activationPrerequisites,
  activationTokens,
  activationFailurePresent,
  activationFailureHistoryPresent,
  lastTransition,
  previousRank

MutationVars ==
  <<activationStatus,
    predecessorOwnership,
    activationPrerequisites,
    activationTokens,
    activationFailurePresent,
    activationFailureHistoryPresent,
    lastTransition,
    previousRank>>

SuccessorActivationRequiredPrerequisites ==
  {"DeferredStatus", "AdapterReady", "RuntimeReady", "ServicesReady",
   "StartupApplied", "ClocksArmed", "IngressOpen"}

SuccessorActivationAdapterPrerequisites == {"DeferredStatus"}

SuccessorActivationRuntimePrerequisites ==
  {"DeferredStatus", "AdapterReady"}

SuccessorActivationServicePrerequisites ==
  {"DeferredStatus", "AdapterReady", "RuntimeReady"}

SuccessorActivationStartupPrerequisites ==
  {"DeferredStatus", "AdapterReady", "RuntimeReady", "ServicesReady"}

SuccessorActivationClockPrerequisites ==
  {"DeferredStatus", "AdapterReady", "RuntimeReady", "ServicesReady",
   "StartupApplied", "ClocksArmed"}

AppliedSuccessorActivationToken ==
  [kind |-> "Applied",
   parentContext |-> "Parent",
   node |-> "Node",
   successorContext |-> "Successor"]

ExactDurableParentApplicationWitness == TRUE

SuccessorActivationCredentialReady ==
  /\ activationStatus = "Running"
  /\ predecessorOwnership = "Published"
  /\ AppliedSuccessorActivationToken \in activationTokens
  /\ ~activationFailurePresent

SuccessorActivationPipelineDistance ==
  CASE activationStatus = "Queued"
         -> IF activationFailureHistoryPresent THEN 9 ELSE 10
  [] /\ activationStatus = "Running"
     /\ ~SuccessorActivationCredentialReady
         -> 9
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites = {}
         -> 8
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites = SuccessorActivationAdapterPrerequisites
         -> 7
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites = SuccessorActivationRuntimePrerequisites
         -> 6
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites = SuccessorActivationServicePrerequisites
         -> 5
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites = SuccessorActivationStartupPrerequisites
         -> 4
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites = SuccessorActivationClockPrerequisites
         -> 3
  [] /\ SuccessorActivationCredentialReady
     /\ activationPrerequisites =
          SuccessorActivationRequiredPrerequisites
         -> 1
  [] OTHER -> 0

SuccessorActivationRank ==
  IF ~activationFailureHistoryPresent
  THEN 9 + SuccessorActivationPipelineDistance
  ELSE SuccessorActivationPipelineDistance

MutationTypeInvariant ==
  /\ activationStatus \in {"Queued", "Running"}
  /\ predecessorOwnership \in {"Published", "Absent"}
  /\ activationPrerequisites
       \subseteq SuccessorActivationRequiredPrerequisites
  /\ activationTokens \subseteq {AppliedSuccessorActivationToken}
  /\ activationFailurePresent \in BOOLEAN
  /\ activationFailureHistoryPresent \in BOOLEAN
  /\ lastTransition \in {"Initial", "BuggyBegin", "FixedBegin", "FailClosed"}
  /\ previousRank \in 0..19

(***************************************************************************
This is the rank-bearing projection of SuccessorActivationProtocolInvariant:
pending shape-valid owners must not enter the pipeline CASE fallback, and
durable failure history owns a Queued/Running state with Absent predecessor
ownership.  Keeping OTHER at zero is intentional: malformed states fail
closed instead of receiving an artificial progress rank.
***************************************************************************)
SuccessorActivationProtocolInvariantProjection ==
  /\ MutationTypeInvariant
  /\ ExactDurableParentApplicationWitness
  /\ (activationFailureHistoryPresent
         => predecessorOwnership = "Absent")
  /\ SuccessorActivationPipelineDistance \in 1..10

StaleAppliedTokenState ==
  /\ activationStatus = "Queued"
  /\ predecessorOwnership = "Published"
  /\ activationPrerequisites = {"IngressOpen"}
  /\ activationTokens = {AppliedSuccessorActivationToken}
  /\ activationFailurePresent = FALSE
  /\ activationFailureHistoryPresent = FALSE

StaleAppliedTokenInit ==
  /\ StaleAppliedTokenState
  /\ lastTransition = "Initial"
  /\ previousRank = 19

(***************************************************************************
Mutation only: this is the pre-repair Begin relation.  It intentionally omits
both exact empty-prerequisite and exact Applied-token-absence guards.
***************************************************************************)
BuggyBeginSuccessorActivation ==
  /\ activationStatus = "Queued"
  /\ predecessorOwnership = "Published"
  /\ ExactDurableParentApplicationWitness
  /\ activationStatus' = "Running"
  /\ lastTransition' = "BuggyBegin"
  /\ previousRank' = SuccessorActivationRank
  /\ UNCHANGED <<predecessorOwnership,
                  activationPrerequisites,
                  activationTokens,
                  activationFailurePresent,
                  activationFailureHistoryPresent>>

FixedBeginSuccessorActivation ==
  /\ activationStatus = "Queued"
  /\ predecessorOwnership = "Published"
  /\ ExactDurableParentApplicationWitness
  /\ activationPrerequisites = {}
  /\ AppliedSuccessorActivationToken \notin activationTokens
  /\ activationStatus' = "Running"
  /\ lastTransition' = "FixedBegin"
  /\ previousRank' = SuccessorActivationRank
  /\ UNCHANGED <<predecessorOwnership,
                  activationPrerequisites,
                  activationTokens,
                  activationFailurePresent,
                  activationFailureHistoryPresent>>

MutationFailClosedSuccessorStartup ==
  /\ activationStatus \in {"Queued", "Running"}
  /\ predecessorOwnership = "Published"
  /\ ~activationFailureHistoryPresent
  /\ activationStatus' = "Queued"
  /\ predecessorOwnership' = "Absent"
  /\ activationPrerequisites' = {}
  /\ activationTokens' = {}
  /\ activationFailurePresent' = TRUE
  /\ activationFailureHistoryPresent' = TRUE
  /\ lastTransition' = "FailClosed"
  /\ previousRank' = SuccessorActivationRank

StaleBuggyBeginIsEnabled ==
  StaleAppliedTokenState => ENABLED BuggyBeginSuccessorActivation

StaleFixedBeginIsDisabled ==
  StaleAppliedTokenState => ~ENABLED FixedBeginSuccessorActivation

StaleFailClosedIsEnabled ==
  StaleAppliedTokenState => ENABLED MutationFailClosedSuccessorStartup

BuggyBeginViolationWitness ==
  lastTransition = "BuggyBegin"
    => ~SuccessorActivationProtocolInvariantProjection

FailClosedStrictlyDecreasesRankWitness ==
  lastTransition = "FailClosed"
    => SuccessorActivationRank < previousRank

BugMutationNext == BuggyBeginSuccessorActivation

BugMutationSpec ==
  StaleAppliedTokenInit /\ [][BugMutationNext]_MutationVars

FixedMutationNext ==
  \/ FixedBeginSuccessorActivation
  \/ MutationFailClosedSuccessorStartup

FixedMutationSpec ==
  StaleAppliedTokenInit /\ [][FixedMutationNext]_MutationVars

=============================================================================
