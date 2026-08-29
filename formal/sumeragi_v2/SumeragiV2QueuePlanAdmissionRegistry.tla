---- MODULE SumeragiV2QueuePlanAdmissionRegistry ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Bounded safety model for the global QueuePlan admission registry.

One transaction-entrypoint key may be journaled and availability-certified
under competing routing generations.  An exact f+1 certificate becomes a
public 202 only after that certificate is durable on the ingress node.  That
acceptance does not claim that the binding is already present in canonical
WSV.  A later proposal-native Sumeragi carrier applies the globally ordered
compare-and-set.  The transaction is never ordinary global-body work: the
exact selected binding can execute only through autonomous lane ownership and
the certified merge corridor.

The model also keeps the admission tombstone after cancellation.  Cancellation
stops execution but cannot make the key absent again, because absence would
reopen an ABA race for a delayed certificate.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Str;
  BindingA,
  \* @type: Str;
  BindingB,
  \* @type: Str;
  BindingRecreated

AdmissionModes ==
  {"Fixed", "SplitRoutePublicAcceptance", "ExecutionBeforeGlobalCas",
   "ConflictingCas", "RestartAba", "LocalExpiryClearsTombstone",
   "DeferredBypass", "CancellationBypass",
   "GuardDropDeletesDurableOwner", "DuplicateExecution"}

Bindings == {BindingA, BindingB, BindingRecreated}
OptionalBinding == Bindings \union {"None"}
ExecutionRoles == {"None", "Autonomous", "Ordinary"}

AdmissionConfiguration ==
  /\ Mode \in AdmissionModes
  /\ BindingA # BindingB
  /\ BindingA # BindingRecreated
  /\ BindingB # BindingRecreated

VARIABLES
  \* @type: Set(Str);
  durableClaims,
  \* @type: Set(Str);
  certificates,
  \* @type: Set(Str);
  locallyDurableCertificates,
  \* @type: Set(Str);
  canonicalBindings,
  \* @type: Set(Str);
  publicAccepted,
  \* @type: Set(Str);
  eligible,
  \* @type: Str;
  executedBinding,
  \* @type: Str;
  executionRole,
  \* @type: Int;
  executionCount,
  \* @type: Bool;
  registryTombstone,
  \* @type: Bool;
  everBound,
  \* @type: Bool;
  cancelled,
  \* @type: Bool;
  restarted,
  \* @type: Bool;
  recreated,
  \* @type: Str;
  activeBinding

vars ==
  <<durableClaims, certificates, locallyDurableCertificates,
    canonicalBindings, publicAccepted, eligible, executedBinding,
    executionRole, executionCount, registryTombstone, everBound, cancelled,
    restarted, recreated, activeBinding>>

Init ==
  /\ AdmissionConfiguration
  /\ durableClaims = {}
  /\ certificates = {}
  /\ locallyDurableCertificates = {}
  /\ canonicalBindings = {}
  /\ publicAccepted = {}
  /\ eligible = {}
  /\ executedBinding = "None"
  /\ executionRole = "None"
  /\ executionCount = 0
  /\ registryTombstone = FALSE
  /\ everBound = FALSE
  /\ cancelled = FALSE
  /\ restarted = FALSE
  /\ recreated = FALSE
  /\ activeBinding = BindingA

JournalClaim(binding) ==
  /\ binding \in Bindings
  /\ durableClaims' = durableClaims \union {binding}
  /\ UNCHANGED <<certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, eligible, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

CertifyAvailability(binding) ==
  /\ binding \in durableClaims
  /\ certificates' = certificates \union {binding}
  /\ UNCHANGED <<durableClaims, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, eligible, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

PersistCertificateLocally(binding) ==
  /\ binding \in certificates
  /\ locallyDurableCertificates' =
       locallyDurableCertificates \union {binding}
  /\ UNCHANGED <<durableClaims, certificates, canonicalBindings,
                 publicAccepted, eligible, executedBinding, executionRole,
                 executionCount, registryTombstone, everBound, cancelled,
                 restarted, recreated, activeBinding>>

ApplyProposalNativeBinding(binding) ==
  /\ binding \in certificates
  /\ \/ binding = activeBinding
     \/ Mode = "ConflictingCas"
  /\ ~cancelled
  /\ \/ canonicalBindings = {}
     \/ binding \in canonicalBindings
     \/ Mode = "ConflictingCas"
  /\ canonicalBindings' = canonicalBindings \union {binding}
  /\ registryTombstone' = TRUE
  /\ everBound' = TRUE
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 publicAccepted, eligible, executedBinding, executionCount,
                 executionRole, cancelled, restarted, recreated, activeBinding>>

ReturnPublicAccepted(binding) ==
  /\ binding \in locallyDurableCertificates
  /\ binding = activeBinding
  /\ \/ publicAccepted = {}
     \/ publicAccepted = {binding}
  /\ publicAccepted' = publicAccepted \union {binding}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, eligible, executedBinding, executionRole,
                 executionCount, registryTombstone, everBound, cancelled,
                 restarted, recreated, activeBinding>>

ActivateExactQueueClaim(binding) ==
  /\ canonicalBindings = {binding}
  /\ binding = activeBinding
  /\ ~cancelled
  /\ executionCount = 0
  /\ eligible' = eligible \union {binding}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

ExecuteEligible(binding) ==
  /\ binding \in eligible
  /\ executionCount = 0
  /\ \/ /\ canonicalBindings = {binding}
        /\ binding = activeBinding
        /\ ~cancelled
     \/ Mode \in {"ExecutionBeforeGlobalCas", "CancellationBypass"}
  /\ executedBinding' = binding
  /\ executionRole' =
       IF Mode = "ExecutionBeforeGlobalCas"
       THEN "Ordinary"
       ELSE "Autonomous"
  /\ executionCount' = executionCount + 1
  /\ eligible' = eligible \ {binding}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, registryTombstone,
                 everBound, cancelled, restarted, recreated, activeBinding>>

Restart ==
  /\ ~restarted
  /\ restarted' = TRUE
  /\ eligible' = {}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, recreated, activeBinding>>

ReconcileExactAfterRestart(binding) ==
  /\ restarted
  /\ binding \in durableClaims
  /\ canonicalBindings = {binding}
  /\ binding = activeBinding
  /\ ~cancelled
  /\ executionCount = 0
  /\ eligible' = eligible \union {binding}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

RecreateLaneIncarnation ==
  /\ ~recreated
  /\ recreated' = TRUE
  /\ activeBinding' = BindingRecreated
  /\ eligible' = {}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted>>

CancelCanonicalBinding ==
  /\ canonicalBindings # {}
  /\ executionCount = 0
  /\ ~cancelled
  /\ cancelled' = TRUE
  /\ eligible' = {}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 restarted, recreated, activeBinding>>

\* Negative controls.
AcceptDeferredWithoutCertificateMutation ==
  /\ Mode = "DeferredBypass"
  /\ BindingA \notin publicAccepted
  /\ publicAccepted' = publicAccepted \union {BindingA}
  /\ eligible' = eligible \union {BindingA}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, executedBinding, executionRole,
                 executionCount, registryTombstone, everBound, cancelled,
                 restarted, recreated, activeBinding>>

ActivateBeforeGlobalCasMutation ==
  /\ Mode = "ExecutionBeforeGlobalCas"
  /\ BindingA \in certificates
  /\ canonicalBindings = {}
  /\ publicAccepted' = publicAccepted \union {BindingA}
  /\ eligible' = eligible \union {BindingA}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, executedBinding, executionRole,
                 executionCount, registryTombstone, everBound, cancelled,
                 restarted, recreated, activeBinding>>

AcceptCompetingRouteMutation ==
  /\ Mode = "SplitRoutePublicAcceptance"
  /\ BindingA \in locallyDurableCertificates
  /\ BindingB \in locallyDurableCertificates
  /\ publicAccepted' = publicAccepted \union {BindingA, BindingB}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, eligible, executedBinding, executionRole,
                 executionCount, registryTombstone, everBound, cancelled,
                 restarted, recreated, activeBinding>>

ReplayStaleIncarnationMutation ==
  /\ Mode = "RestartAba"
  /\ restarted
  /\ recreated
  /\ BindingA \in certificates
  /\ eligible' = eligible \union {BindingA}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

ClearTombstoneOnLocalExpiryMutation ==
  /\ Mode = "LocalExpiryClearsTombstone"
  /\ everBound
  /\ registryTombstone
  /\ canonicalBindings' = {}
  /\ registryTombstone' = FALSE
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 publicAccepted, eligible, executedBinding, executionRole,
                 executionCount, everBound, cancelled, restarted, recreated,
                 activeBinding>>

ActivateCancelledMutation ==
  /\ Mode = "CancellationBypass"
  /\ cancelled
  /\ canonicalBindings = {BindingA}
  /\ eligible' = eligible \union {BindingA}
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

GuardDropDeletesDurableOwnerMutation ==
  /\ Mode = "GuardDropDeletesDurableOwner"
  /\ BindingA \in durableClaims
  /\ BindingA \in certificates
  /\ canonicalBindings = {BindingA}
  /\ durableClaims' = durableClaims \ {BindingA}
  /\ UNCHANGED <<certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, eligible, executedBinding,
                 executionRole, executionCount, registryTombstone, everBound,
                 cancelled, restarted, recreated, activeBinding>>

ExecuteDuplicateMutation ==
  /\ Mode = "DuplicateExecution"
  /\ executedBinding # "None"
  /\ canonicalBindings = {executedBinding}
  /\ ~cancelled
  /\ executedBinding' = executedBinding
  /\ executionCount' = executionCount + 1
  /\ UNCHANGED <<durableClaims, certificates, locallyDurableCertificates,
                 canonicalBindings, publicAccepted, eligible,
                 executionRole, registryTombstone, everBound, cancelled,
                 restarted, recreated, activeBinding>>

Next ==
  \/ \E binding \in Bindings: JournalClaim(binding)
  \/ \E binding \in Bindings: CertifyAvailability(binding)
  \/ \E binding \in Bindings: PersistCertificateLocally(binding)
  \/ \E binding \in Bindings: ApplyProposalNativeBinding(binding)
  \/ \E binding \in Bindings: ReturnPublicAccepted(binding)
  \/ \E binding \in Bindings: ActivateExactQueueClaim(binding)
  \/ \E binding \in Bindings: ExecuteEligible(binding)
  \/ Restart
  \/ \E binding \in Bindings: ReconcileExactAfterRestart(binding)
  \/ RecreateLaneIncarnation
  \/ CancelCanonicalBinding
  \/ AcceptDeferredWithoutCertificateMutation
  \/ ActivateBeforeGlobalCasMutation
  \/ AcceptCompetingRouteMutation
  \/ ReplayStaleIncarnationMutation
  \/ ClearTombstoneOnLocalExpiryMutation
  \/ ActivateCancelledMutation
  \/ GuardDropDeletesDurableOwnerMutation
  \/ ExecuteDuplicateMutation

QueuePlanAdmissionTypeInvariant ==
  /\ AdmissionConfiguration
  /\ durableClaims \subseteq Bindings
  /\ certificates \subseteq Bindings
  /\ locallyDurableCertificates \subseteq Bindings
  /\ canonicalBindings \subseteq Bindings
  /\ publicAccepted \subseteq Bindings
  /\ eligible \subseteq Bindings
  /\ executedBinding \in OptionalBinding
  /\ executionRole \in ExecutionRoles
  /\ executionCount \in Nat
  /\ registryTombstone \in BOOLEAN
  /\ everBound \in BOOLEAN
  /\ cancelled \in BOOLEAN
  /\ restarted \in BOOLEAN
  /\ recreated \in BOOLEAN
  /\ activeBinding \in {BindingA, BindingRecreated}

MLAdmissionCasUnique ==
  Cardinality(canonicalBindings) <= 1

MLCertificateDurable ==
  /\ certificates \subseteq durableClaims
  /\ locallyDurableCertificates \subseteq certificates
  /\ canonicalBindings \subseteq certificates

MLPublic202Exact ==
  /\ publicAccepted \subseteq locallyDurableCertificates
  /\ Cardinality(publicAccepted) <= 1

MLExecutionRequiresExactBinding ==
  executedBinding # "None" =>
    canonicalBindings = {executedBinding}

MLQueuePlanExecutionAutonomousOnly ==
  executedBinding # "None" => executionRole = "Autonomous"

MLQueueEligibilityExact ==
  /\ eligible \subseteq canonicalBindings
  /\ eligible \subseteq {activeBinding}
  /\ (cancelled => eligible = {})

MLAdmissionAtMostOnceExecution ==
  executionCount <= 1

MLImmutableAdmissionTombstone ==
  /\ registryTombstone = (canonicalBindings # {})
  /\ (everBound => registryTombstone)

MLCancellationStopsExecution ==
  cancelled => executionCount = 0

QueuePlanAdmissionRegistrySafetyInvariant ==
  /\ QueuePlanAdmissionTypeInvariant
  /\ MLAdmissionCasUnique
  /\ MLCertificateDurable
  /\ MLPublic202Exact
  /\ MLExecutionRequiresExactBinding
  /\ MLQueuePlanExecutionAutonomousOnly
  /\ MLQueueEligibilityExact
  /\ MLAdmissionAtMostOnceExecution
  /\ MLImmutableAdmissionTombstone
  /\ MLCancellationStopsExecution

\* The executable source-binding ledger maps this obligation to the shared
\* QueuePlan certificate verifier, ingress-local durable persistence,
\* proposal-native CAS staging, candidate/provider FIFO fences, locked-body and
\* common block rejection, startup reconciliation, and immutable marker helpers.
QueuePlanAdmissionRegistryProductionRefinementObligation ==
  QueuePlanAdmissionRegistrySafetyInvariant

QueuePlanAdmissionRegistrySpec == Init /\ [][Next]_vars

====
