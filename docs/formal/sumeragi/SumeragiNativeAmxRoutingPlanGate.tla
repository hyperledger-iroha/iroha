---- MODULE SumeragiNativeAmxRoutingPlanGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for native AMX routing-plan canonicalization and block
execution-context projection.

This slice models the boundary formed by `RoutingPlan::native_amx(...)`,
`RoutingPlan::digest(...)`, `execution_context_for_routing_plan(...)`,
`execution_context_legs_for_routing_plan(...)`, and the block
execution-context recheck against a freshly resolved routing plan. Native AMX
plans must be accepted for multi-dataspace or universal-coordinator targets,
canonicalize participant legs by `(dataspace_id, lane_id)`, force coordinator
and participant roles, bind the coordinator plus participant set into the
native plan digest, and project durable execution contexts in coordinator-first
order. Single-route plans must remain single-route contexts and cannot carry
native AMX receipts.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

SingleTarget == 1
OneParticipantNoCoordinator == 2
MultiParticipantNative == 3
UniversalCoordinatorNative == 4
UnorderedInputCanonical == 5
DuplicateInputDedup == 6
SameDataspaceDifferentLaneKept == 7
CoordinatorRoleForced == 8
ParticipantRolesForced == 9
NativeLegsCoordinatorFirst == 10
SingleLegsOneCoordinator == 11
SingleDigestDomain == 12
NativeDigestDomain == 13
DigestBindsCoordinator == 14
DigestBindsParticipants == 15
DigestIgnoresInputOrder == 16
ContextCoordinatorRoute == 17
ContextPlanDigest == 18
ContextPlanLegs == 19
RejectWrongCoordinatorContext == 20
RejectWrongDigestContext == 21
RejectWrongLegsContext == 22
NativeRequiresReceipt == 23
SingleRejectsReceipt == 24
UnknownParticipantLaneRejected == 25
CanonicalLowestLane == 26

Candidates == 1..26

NoBug == 0
CollapseNativeToSingleBug == 1
PromoteSingleToNativeBug == 2
DropNativePlanBug == 3
AcceptUnknownLaneBug == 4
UseNonUniversalCoordinatorBug == 5
ReorderParticipantsBug == 6
KeepDuplicateParticipantBug == 7
DedupByDataspaceOnlyBug == 8
WrongCoordinatorRoleBug == 9
WrongParticipantRoleBug == 10
ProjectParticipantFirstBug == 11
ProjectSingleAsParticipantBug == 12
SingleDigestUsesNativeDomainBug == 13
NativeDigestUsesSingleDomainBug == 14
DigestSkipsCoordinatorBug == 15
DigestSkipsParticipantBug == 16
DigestUsesInputOrderBug == 17
ContextUsesParticipantRouteBug == 18
ContextWrongDigestBug == 19
ContextDropsParticipantLegsBug == 20
ValidateAcceptWrongCoordinatorBug == 21
ValidateAcceptWrongDigestBug == 22
ValidateAcceptWrongLegsBug == 23
AllowMissingNativeReceiptBug == 24
AllowSingleReceiptBug == 25
UseHighestLaneBug == 26

Bugs == 0..26

BugCollapseNativeToSingle == Bug = CollapseNativeToSingleBug
BugPromoteSingleToNative == Bug = PromoteSingleToNativeBug
BugDropNativePlan == Bug = DropNativePlanBug
BugAcceptUnknownLane == Bug = AcceptUnknownLaneBug
BugUseNonUniversalCoordinator == Bug = UseNonUniversalCoordinatorBug
BugReorderParticipants == Bug = ReorderParticipantsBug
BugKeepDuplicateParticipant == Bug = KeepDuplicateParticipantBug
BugDedupByDataspaceOnly == Bug = DedupByDataspaceOnlyBug
BugWrongCoordinatorRole == Bug = WrongCoordinatorRoleBug
BugWrongParticipantRole == Bug = WrongParticipantRoleBug
BugProjectParticipantFirst == Bug = ProjectParticipantFirstBug
BugProjectSingleAsParticipant == Bug = ProjectSingleAsParticipantBug
BugSingleDigestUsesNativeDomain == Bug = SingleDigestUsesNativeDomainBug
BugNativeDigestUsesSingleDomain == Bug = NativeDigestUsesSingleDomainBug
BugDigestSkipsCoordinator == Bug = DigestSkipsCoordinatorBug
BugDigestSkipsParticipant == Bug = DigestSkipsParticipantBug
BugDigestUsesInputOrder == Bug = DigestUsesInputOrderBug
BugContextUsesParticipantRoute == Bug = ContextUsesParticipantRouteBug
BugContextWrongDigest == Bug = ContextWrongDigestBug
BugContextDropsParticipantLegs == Bug = ContextDropsParticipantLegsBug
BugValidateAcceptWrongCoordinator == Bug = ValidateAcceptWrongCoordinatorBug
BugValidateAcceptWrongDigest == Bug = ValidateAcceptWrongDigestBug
BugValidateAcceptWrongLegs == Bug = ValidateAcceptWrongLegsBug
BugAllowMissingNativeReceipt == Bug = AllowMissingNativeReceiptBug
BugAllowSingleReceipt == Bug = AllowSingleReceiptBug
BugUseHighestLane == Bug = UseHighestLaneBug

SpecAccepted(candidate) ==
  candidate # UnknownParticipantLaneRejected

SpecNative(candidate) ==
  candidate \in {
    MultiParticipantNative,
    UniversalCoordinatorNative,
    UnorderedInputCanonical,
    DuplicateInputDedup,
    SameDataspaceDifferentLaneKept,
    CoordinatorRoleForced,
    ParticipantRolesForced,
    NativeLegsCoordinatorFirst,
    NativeDigestDomain,
    DigestBindsCoordinator,
    DigestBindsParticipants,
    DigestIgnoresInputOrder,
    ContextCoordinatorRoute,
    ContextPlanDigest,
    ContextPlanLegs,
    RejectWrongCoordinatorContext,
    RejectWrongDigestContext,
    RejectWrongLegsContext,
    NativeRequiresReceipt,
    CanonicalLowestLane
  }

SpecSingle(candidate) ==
  candidate \in {
    SingleTarget,
    OneParticipantNoCoordinator,
    SingleLegsOneCoordinator,
    SingleDigestDomain,
    SingleRejectsReceipt
  }

ImplementationAccepted(candidate) ==
  IF SpecAccepted(candidate)
  THEN ~(SpecNative(candidate) /\ BugDropNativePlan)
  ELSE BugAcceptUnknownLane

ImplementationNative(candidate) ==
  /\ ImplementationAccepted(candidate)
  /\ IF SpecNative(candidate)
     THEN ~BugCollapseNativeToSingle
     ELSE BugPromoteSingleToNative

ImplementationUsesUniversalCoordinator(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugUseNonUniversalCoordinator

ImplementationParticipantsSorted(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugReorderParticipants

ImplementationParticipantsDeduped(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugKeepDuplicateParticipant

ImplementationSameDataspaceDifferentLaneKept(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugDedupByDataspaceOnly

ImplementationCoordinatorRoleForced(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugWrongCoordinatorRole

ImplementationParticipantRolesForced(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugWrongParticipantRole

ImplementationNativeLegsCoordinatorFirst(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugProjectParticipantFirst

ImplementationSingleLegsOneCoordinator(candidate) ==
  /\ ImplementationAccepted(candidate)
  /\ ~ImplementationNative(candidate)
  /\ ~BugProjectSingleAsParticipant

ImplementationSingleDigestDomain(candidate) ==
  /\ ImplementationAccepted(candidate)
  /\ ~ImplementationNative(candidate)
  /\ ~BugSingleDigestUsesNativeDomain

ImplementationNativeDigestDomain(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugNativeDigestUsesSingleDomain

ImplementationDigestBindsCoordinator(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugDigestSkipsCoordinator

ImplementationDigestBindsParticipants(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugDigestSkipsParticipant

ImplementationDigestIgnoresInputOrder(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugDigestUsesInputOrder

ImplementationContextCoordinatorRoute(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugContextUsesParticipantRoute

ImplementationContextPlanDigest(candidate) ==
  /\ ImplementationAccepted(candidate)
  /\ ~BugContextWrongDigest

ImplementationContextPlanLegs(candidate) ==
  /\ ImplementationAccepted(candidate)
  /\ ~BugContextDropsParticipantLegs

ImplementationRejectsWrongCoordinatorContext(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugValidateAcceptWrongCoordinator

ImplementationRejectsWrongDigestContext(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugValidateAcceptWrongDigest

ImplementationRejectsWrongLegsContext(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugValidateAcceptWrongLegs

ImplementationNativeRequiresReceipt(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugAllowMissingNativeReceipt

ImplementationSingleRejectsReceipt(candidate) ==
  /\ ImplementationAccepted(candidate)
  /\ ~ImplementationNative(candidate)
  /\ ~BugAllowSingleReceipt

ImplementationCanonicalLowestLane(candidate) ==
  /\ ImplementationNative(candidate)
  /\ ~BugUseHighestLane

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AdmissionMatchesSpec ==
  \A candidate \in tried:
    ImplementationAccepted(candidate) <=> SpecAccepted(candidate)

PlanKindMatchesSpec ==
  \A candidate \in tried:
    ImplementationAccepted(candidate) =>
      (ImplementationNative(candidate) <=> SpecNative(candidate))

SingleTargetsStaySingle ==
  \A candidate \in tried:
    SpecSingle(candidate) =>
      /\ ImplementationAccepted(candidate)
      /\ ~ImplementationNative(candidate)

NativeTargetsStayNative ==
  \A candidate \in tried:
    SpecNative(candidate) =>
      /\ ImplementationAccepted(candidate)
      /\ ImplementationNative(candidate)

UniversalCoordinatorRoutePreserved ==
  UniversalCoordinatorNative \in tried =>
    ImplementationUsesUniversalCoordinator(UniversalCoordinatorNative)

ParticipantsAreCanonical ==
  /\ UnorderedInputCanonical \in tried =>
       /\ ImplementationParticipantsSorted(UnorderedInputCanonical)
       /\ ImplementationDigestIgnoresInputOrder(UnorderedInputCanonical)
  /\ DuplicateInputDedup \in tried =>
       ImplementationParticipantsDeduped(DuplicateInputDedup)
  /\ SameDataspaceDifferentLaneKept \in tried =>
       ImplementationSameDataspaceDifferentLaneKept(SameDataspaceDifferentLaneKept)

RolesAreForced ==
  /\ CoordinatorRoleForced \in tried =>
       ImplementationCoordinatorRoleForced(CoordinatorRoleForced)
  /\ ParticipantRolesForced \in tried =>
       ImplementationParticipantRolesForced(ParticipantRolesForced)

LegProjectionMatchesPlan ==
  /\ NativeLegsCoordinatorFirst \in tried =>
       ImplementationNativeLegsCoordinatorFirst(NativeLegsCoordinatorFirst)
  /\ SingleLegsOneCoordinator \in tried =>
       ImplementationSingleLegsOneCoordinator(SingleLegsOneCoordinator)

DigestMatchesPlan ==
  /\ SingleDigestDomain \in tried =>
       ImplementationSingleDigestDomain(SingleDigestDomain)
  /\ NativeDigestDomain \in tried =>
       ImplementationNativeDigestDomain(NativeDigestDomain)
  /\ DigestBindsCoordinator \in tried =>
       ImplementationDigestBindsCoordinator(DigestBindsCoordinator)
  /\ DigestBindsParticipants \in tried =>
       ImplementationDigestBindsParticipants(DigestBindsParticipants)
  /\ DigestIgnoresInputOrder \in tried =>
       ImplementationDigestIgnoresInputOrder(DigestIgnoresInputOrder)

ExecutionContextProjectionMatchesPlan ==
  /\ ContextCoordinatorRoute \in tried =>
       ImplementationContextCoordinatorRoute(ContextCoordinatorRoute)
  /\ ContextPlanDigest \in tried =>
       ImplementationContextPlanDigest(ContextPlanDigest)
  /\ ContextPlanLegs \in tried =>
       ImplementationContextPlanLegs(ContextPlanLegs)

BlockRecheckRejectsMutatedContext ==
  /\ RejectWrongCoordinatorContext \in tried =>
       ImplementationRejectsWrongCoordinatorContext(RejectWrongCoordinatorContext)
  /\ RejectWrongDigestContext \in tried =>
       ImplementationRejectsWrongDigestContext(RejectWrongDigestContext)
  /\ RejectWrongLegsContext \in tried =>
       ImplementationRejectsWrongLegsContext(RejectWrongLegsContext)

ReceiptPresenceMatchesPlanKind ==
  /\ NativeRequiresReceipt \in tried =>
       ImplementationNativeRequiresReceipt(NativeRequiresReceipt)
  /\ SingleRejectsReceipt \in tried =>
       ImplementationSingleRejectsReceipt(SingleRejectsReceipt)

RoutingResolutionFailsClosed ==
  /\ UnknownParticipantLaneRejected \in tried =>
       ~ImplementationAccepted(UnknownParticipantLaneRejected)
  /\ CanonicalLowestLane \in tried =>
       ImplementationCanonicalLowestLane(CanonicalLowestLane)

RoutingAdmissionCases == {
  SingleTarget,
  OneParticipantNoCoordinator,
  MultiParticipantNative,
  UniversalCoordinatorNative,
  UnknownParticipantLaneRejected
}

RoutingParticipantCanonicalCases == {
  UnorderedInputCanonical,
  DuplicateInputDedup,
  SameDataspaceDifferentLaneKept,
  CanonicalLowestLane
}

RoutingRoleProjectionCases == {
  CoordinatorRoleForced,
  ParticipantRolesForced,
  NativeLegsCoordinatorFirst,
  SingleLegsOneCoordinator
}

RoutingDigestCases == {
  SingleDigestDomain,
  NativeDigestDomain,
  DigestBindsCoordinator,
  DigestBindsParticipants,
  DigestIgnoresInputOrder
}

RoutingExecutionContextCases == {
  ContextCoordinatorRoute,
  ContextPlanDigest,
  ContextPlanLegs,
  RejectWrongCoordinatorContext,
  RejectWrongDigestContext,
  RejectWrongLegsContext
}

RoutingReceiptCases == {
  NativeRequiresReceipt,
  SingleRejectsReceipt
}

NativeAmxRoutingPlanGroupedCases ==
  RoutingAdmissionCases \cup
  RoutingParticipantCanonicalCases \cup
  RoutingRoleProjectionCases \cup
  RoutingDigestCases \cup
  RoutingExecutionContextCases \cup
  RoutingReceiptCases

NativeAmxRoutingPlanCaseGroupsComplete ==
  NativeAmxRoutingPlanGroupedCases = Candidates

NativeAmxRoutingAdmissionExact ==
  /\ AdmissionMatchesSpec
  /\ PlanKindMatchesSpec
  /\ SingleTargetsStaySingle
  /\ NativeTargetsStayNative
  /\ UniversalCoordinatorRoutePreserved

NativeAmxRoutingParticipantCanonicalExact ==
  /\ ParticipantsAreCanonical

NativeAmxRoutingRoleProjectionExact ==
  /\ RolesAreForced
  /\ LegProjectionMatchesPlan

NativeAmxRoutingDigestExact ==
  /\ DigestMatchesPlan

NativeAmxRoutingExecutionContextExact ==
  /\ ExecutionContextProjectionMatchesPlan
  /\ BlockRecheckRejectsMutatedContext

NativeAmxRoutingReceiptExact ==
  /\ ReceiptPresenceMatchesPlanKind

NativeAmxRoutingResolutionExact ==
  /\ RoutingResolutionFailsClosed

NativeAmxRoutingPlanExactness ==
  /\ NativeAmxRoutingPlanCaseGroupsComplete
  /\ NativeAmxRoutingAdmissionExact
  /\ NativeAmxRoutingParticipantCanonicalExact
  /\ NativeAmxRoutingRoleProjectionExact
  /\ NativeAmxRoutingDigestExact
  /\ NativeAmxRoutingExecutionContextExact
  /\ NativeAmxRoutingReceiptExact
  /\ NativeAmxRoutingResolutionExact

Safety ==
  NativeAmxRoutingPlanExactness

====
