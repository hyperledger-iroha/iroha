---- MODULE SumeragiV2IndexedServiceActivationMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite regression for indexed successor service activation.

Every pre-created Async instance has the canonical standalone initializer:
all validators are active and both local service deadlines are armed.  The
first independent successor join must atomically burn a restriction tombstone,
retain only the joined owner, and zero both deadline carriers for every
inactive validator.  Otherwise an unjoined validator can remain a timed owner
whose service action is unavailable in the indexed product and permanently
block Tick.  Once every validator has joined, the tombstone also prevents the
restriction episode from being recreated.

The four configurations paired with this module check two distinct defects:

  * the fixed first join reaches a second Tick, while the mutation which leaves
    an unjoined clock owner admits a fair stuttering lasso at time one; and
  * the fixed all-joined state rejects restriction re-entry, while omitting the
    tombstone guard violates exact activation/joined-membership coherence.
***************************************************************************)

ValidatorIds == {"A", "B"}
ActivationBound == 1
MaxMutationTime == 2

VARIABLES
  mutationNow,
  joinedNodes,
  activeNodes,
  restricted,
  nodeServiceDeadline,
  ioServiceDeadline,
  lastTransition

MutationVars ==
  <<mutationNow,
    joinedNodes,
    activeNodes,
    restricted,
    nodeServiceDeadline,
    ioServiceDeadline,
    lastTransition>>

MutationTransitionNames ==
  {"Initial", "FixedFirstJoin", "BugFirstJoin", "Tick", "ServiceA",
   "BugReenterRestriction", "FixedReentryRejected"}

MutationTypeInvariant ==
  /\ mutationNow \in 0..MaxMutationTime
  /\ joinedNodes \subseteq ValidatorIds
  /\ activeNodes \subseteq ValidatorIds
  /\ restricted \in BOOLEAN
  /\ nodeServiceDeadline \in [ValidatorIds -> 0..3]
  /\ ioServiceDeadline \in [ValidatorIds -> 0..3]
  /\ lastTransition \in MutationTransitionNames

ActivationDeadlinePairInvariant ==
  \A node \in ValidatorIds:
    /\ (node \in activeNodes <=> nodeServiceDeadline[node] # 0)
    /\ (node \in activeNodes <=> ioServiceDeadline[node] # 0)

ActivationMembershipCoherence ==
  IF restricted
  THEN /\ joinedNodes # {}
       /\ activeNodes = joinedNodes
  ELSE /\ joinedNodes = {}
       /\ activeNodes = ValidatorIds

ActivationCoherence ==
  /\ MutationTypeInvariant
  /\ ActivationDeadlinePairInvariant
  /\ ActivationMembershipCoherence

CanonicalFreshSuccessorInit ==
  /\ mutationNow = 0
  /\ joinedNodes = {}
  /\ activeNodes = ValidatorIds
  /\ restricted = FALSE
  /\ nodeServiceDeadline =
       [node \in ValidatorIds |-> ActivationBound]
  /\ ioServiceDeadline =
       [node \in ValidatorIds |-> ActivationBound]
  /\ lastTransition = "Initial"

(***************************************************************************
Production repair: publish the first join and restrict the scheduler in one
transition.  The unjoined owner receives zero in both deadline carriers.
***************************************************************************)
FixedFirstJoin ==
  /\ lastTransition = "Initial"
  /\ joinedNodes = {}
  /\ ~restricted
  /\ activeNodes = ValidatorIds
  /\ joinedNodes' = {"A"}
  /\ activeNodes' = {"A"}
  /\ restricted' = TRUE
  /\ nodeServiceDeadline' =
       [node \in ValidatorIds |->
          IF node = "A" THEN ActivationBound ELSE 0]
  /\ ioServiceDeadline' =
       [node \in ValidatorIds |->
          IF node = "A" THEN ActivationBound ELSE 0]
  /\ lastTransition' = "FixedFirstJoin"
  /\ UNCHANGED mutationNow

(***************************************************************************
Mutation: joined membership advances while the canonical all-active service
state is left intact.  Validator B cannot run in this context, yet its due
deadline remains in Tick's blocker set.
***************************************************************************)
BugFirstJoinLeavesUnjoinedClockOwner ==
  /\ lastTransition = "Initial"
  /\ joinedNodes = {}
  /\ ~restricted
  /\ activeNodes = ValidatorIds
  /\ joinedNodes' = {"A"}
  /\ lastTransition' = "BugFirstJoin"
  /\ UNCHANGED <<mutationNow, activeNodes, restricted,
                  nodeServiceDeadline, ioServiceDeadline>>

MutationTickEnabled ==
  /\ mutationNow < MaxMutationTime
  /\ \A node \in activeNodes:
       /\ nodeServiceDeadline[node] > mutationNow
       /\ ioServiceDeadline[node] > mutationNow

MutationTick ==
  /\ lastTransition # "Initial"
  /\ MutationTickEnabled
  /\ mutationNow' = mutationNow + 1
  /\ lastTransition' = "Tick"
  /\ UNCHANGED <<joinedNodes, activeNodes, restricted,
                  nodeServiceDeadline, ioServiceDeadline>>

ServiceJoinedA ==
  /\ "A" \in joinedNodes
  /\ "A" \in activeNodes
  /\ nodeServiceDeadline["A"] <= mutationNow
  /\ ioServiceDeadline["A"] <= mutationNow
  /\ nodeServiceDeadline' =
       [nodeServiceDeadline EXCEPT
          !["A"] = mutationNow + ActivationBound]
  /\ ioServiceDeadline' =
       [ioServiceDeadline EXCEPT
          !["A"] = mutationNow + ActivationBound]
  /\ lastTransition' = "ServiceA"
  /\ UNCHANGED <<mutationNow, joinedNodes, activeNodes, restricted>>

FixedActivationNext ==
  \/ FixedFirstJoin
  \/ MutationTick
  \/ ServiceJoinedA

BugActivationNext ==
  \/ BugFirstJoinLeavesUnjoinedClockOwner
  \/ MutationTick
  \/ ServiceJoinedA

FixedActivationSpec ==
  /\ CanonicalFreshSuccessorInit
  /\ [][FixedActivationNext]_MutationVars
  /\ WF_MutationVars(FixedFirstJoin)
  /\ WF_MutationVars(MutationTick)
  /\ WF_MutationVars(ServiceJoinedA)

BugActivationSpec ==
  /\ CanonicalFreshSuccessorInit
  /\ [][BugActivationNext]_MutationVars
  /\ WF_MutationVars(BugFirstJoinLeavesUnjoinedClockOwner)
  /\ WF_MutationVars(MutationTick)
  /\ WF_MutationVars(ServiceJoinedA)

EventuallySecondTick == <>(mutationNow = MaxMutationTime)

FixedFirstJoinDisablesUnjoinedClockOwner ==
  lastTransition = "FixedFirstJoin"
    => /\ "B" \notin activeNodes
       /\ nodeServiceDeadline["B"] = 0
       /\ ioServiceDeadline["B"] = 0

BugFirstJoinRetainsUnjoinedClockOwner ==
  lastTransition = "BugFirstJoin"
    => /\ "B" \in activeNodes
       /\ "B" \notin joinedNodes

(***************************************************************************
Restriction-tombstone re-entry pair.  This starts after both validators have
joined and been rearmed.  The repaired entry action is disabled because the
irreversible tombstone is already set.  The mutation ignores it and recreates
the singleton restriction, breaking active/joined equality at depth one.
***************************************************************************)

AllJoinedRestrictedInit ==
  /\ mutationNow = 0
  /\ joinedNodes = ValidatorIds
  /\ activeNodes = ValidatorIds
  /\ restricted = TRUE
  /\ nodeServiceDeadline =
       [node \in ValidatorIds |-> ActivationBound]
  /\ ioServiceDeadline =
       [node \in ValidatorIds |-> ActivationBound]
  /\ lastTransition = "Initial"

FixedEnterIndexedRestriction ==
  /\ lastTransition = "Initial"
  /\ ~restricted
  /\ activeNodes = ValidatorIds
  /\ activeNodes' = {"A"}
  /\ restricted' = TRUE
  /\ nodeServiceDeadline' =
       [node \in ValidatorIds |->
          IF node = "A" THEN ActivationBound ELSE 0]
  /\ ioServiceDeadline' =
       [node \in ValidatorIds |->
          IF node = "A" THEN ActivationBound ELSE 0]
  /\ lastTransition' = "FixedFirstJoin"
  /\ UNCHANGED <<mutationNow, joinedNodes>>

BugReenterIndexedRestriction ==
  /\ lastTransition = "Initial"
  /\ activeNodes = ValidatorIds
  /\ activeNodes' = {"A"}
  /\ restricted' = TRUE
  /\ nodeServiceDeadline' =
       [node \in ValidatorIds |->
          IF node = "A" THEN ActivationBound ELSE 0]
  /\ ioServiceDeadline' =
       [node \in ValidatorIds |->
          IF node = "A" THEN ActivationBound ELSE 0]
  /\ lastTransition' = "BugReenterRestriction"
  /\ UNCHANGED <<mutationNow, joinedNodes>>

FixedObserveReentryRejected ==
  /\ lastTransition = "Initial"
  /\ ~ENABLED FixedEnterIndexedRestriction
  /\ lastTransition' = "FixedReentryRejected"
  /\ UNCHANGED <<mutationNow, joinedNodes, activeNodes, restricted,
                  nodeServiceDeadline, ioServiceDeadline>>

ReentryFixedNext == FixedObserveReentryRejected
ReentryBugNext == BugReenterIndexedRestriction

ReentryFixedSpec ==
  /\ AllJoinedRestrictedInit
  /\ [][ReentryFixedNext]_MutationVars
  /\ WF_MutationVars(FixedObserveReentryRejected)

ReentryBugSpec ==
  /\ AllJoinedRestrictedInit
  /\ [][ReentryBugNext]_MutationVars
  /\ WF_MutationVars(BugReenterIndexedRestriction)

ReentryFixedActionIsDisabled ==
  lastTransition = "Initial"
    => ~ENABLED FixedEnterIndexedRestriction

ReentryBugActionIsEnabled ==
  lastTransition = "Initial"
    => ENABLED BugReenterIndexedRestriction

FixedReentryRejectionPreservesAllJoinedActivation ==
  lastTransition = "FixedReentryRejected"
    => /\ joinedNodes = ValidatorIds
       /\ activeNodes = ValidatorIds
       /\ restricted

=============================================================================
