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

The repaired Begin is disabled in that state. An Applied startup failure is
also disabled until the visible status is Running; failure may not atomically
rewrite a Queued owner into recovered state. The two configurations below
separate the red buggy witness from the green fixed/lifecycle corridor.
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
  CASE activationStatus = "Queued" -> 10
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
  SuccessorActivationPipelineDistance

MutationTypeInvariant ==
  /\ activationStatus \in {"Queued", "Running"}
  /\ predecessorOwnership \in {"Published", "Absent"}
  /\ activationPrerequisites
       \subseteq SuccessorActivationRequiredPrerequisites
  /\ activationTokens \subseteq {AppliedSuccessorActivationToken}
  /\ activationFailurePresent \in BOOLEAN
  /\ activationFailureHistoryPresent \in BOOLEAN
  /\ lastTransition
       \in {"Initial", "BuggyBegin", "FixedBegin", "AppliedFailure"}
  /\ previousRank \in 0..10

(***************************************************************************
This is the rank-bearing projection of SuccessorActivationProtocolInvariant:
pending shape-valid owners must not enter the pipeline CASE fallback, and
currently latched failure owns a visible Running state until restart. Keeping
OTHER at zero is intentional: malformed states fail
closed instead of receiving an artificial progress rank.
***************************************************************************)
SuccessorActivationProtocolInvariantProjection ==
  /\ MutationTypeInvariant
  /\ ExactDurableParentApplicationWitness
  /\ (activationFailurePresent => activationStatus = "Running")
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
  /\ previousRank = 10

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

MutationLatchAppliedSuccessorStartupFailure ==
  /\ activationStatus = "Running"
  /\ predecessorOwnership = "Published"
  /\ ~activationFailurePresent
  /\ activationPrerequisites' = {}
  /\ activationTokens' = {}
  /\ activationFailurePresent' = TRUE
  /\ activationFailureHistoryPresent' = TRUE
  /\ lastTransition' = "AppliedFailure"
  /\ previousRank' = SuccessorActivationRank
  /\ UNCHANGED <<activationStatus, predecessorOwnership>>

StaleBuggyBeginIsEnabled ==
  StaleAppliedTokenState => ENABLED BuggyBeginSuccessorActivation

StaleFixedBeginIsDisabled ==
  StaleAppliedTokenState => ~ENABLED FixedBeginSuccessorActivation

StaleAppliedFailureIsDisabled ==
  StaleAppliedTokenState
    => ~ENABLED MutationLatchAppliedSuccessorStartupFailure

BuggyBeginViolationWitness ==
  lastTransition = "BuggyBegin"
    => ~SuccessorActivationProtocolInvariantProjection

AppliedFailurePreservesRunningWitness ==
  lastTransition = "AppliedFailure"
    => activationStatus = "Running"

BugMutationNext == BuggyBeginSuccessorActivation

BugMutationSpec ==
  StaleAppliedTokenInit /\ [][BugMutationNext]_MutationVars

FixedMutationNext ==
  \/ FixedBeginSuccessorActivation
  \/ MutationLatchAppliedSuccessorStartupFailure

FixedMutationSpec ==
  StaleAppliedTokenInit /\ [][FixedMutationNext]_MutationVars

=============================================================================
