---- MODULE SumeragiV2ServeRestartTerminalDischargeMutation ----
EXTENDS FiniteSets, Naturals, Sequences, TLC

(***************************************************************************
Finite executable mutation kernel for V5 exact-Serve restart discharge.

Persisted ingress waiters and unsealed admissions are one finite startup
union.  Startup visits that union in immutable scheduler/lifecycle order,
persists each independently reconstructed typed terminal before advancing,
and exposes no producer while debt remains.  A crash between entries retains
the completed terminal and resumes the remaining suffix locally; requester
retransmission is never a recovery primitive.

A persisted terminal waiter is narrower still.  It must cite the exact
reconstructible Response tombstone.  Startup may replay that response or
atomically convert it to a durable-Decision negative, without a new signature
or ordinal.  Negative, orphaned, or output-mismatched terminal waiters are
corrupt and fail startup.  An exact negative retry is rejected before the
actor-global ordinal cut.  A positive terminal retransmission consumes one
fresh physical/shared scheduler carrier while retaining the completed logical
lifecycle; it cannot resurrect an admission.  PersistDecision also preserves
an already admitted pre-fence carrier until checked drain reclassifies it.
That checked drain can atomically terminalize an uncommitted Prepared owner;
an injected stage failure is an explicit bad terminal rather than a
requester-dependent retry requirement.

The kernel also isolates the coupled authority rules used by this repair:
certified requests fan out to the complete frozen roster, only frozen-QC
signers may produce a response, historical transport crosses the raw gate
without acquiring an active-height lifecycle, and future or same-height
foreign-context traffic is rejected.  This bounded model is mutation
evidence, not proof of the complete asynchronous transition relation.
***************************************************************************)

CONSTANTS
  DischargeCompleteUnion,
  UseCanonicalStartupOrder,
  ResumeCompleteUnionAfterCrash,
  ResumeCanonicalOrderAfterCrash,
  PersistTerminalBeforeAdvance,
  BlockProducerWhileStartupPending,
  RequireExactReplayBinding,
  RejectOrphanTerminalWaiter,
  RejectNegativeTerminalWaiter,
  RejectOwnerRequestMismatchWaiter,
  RejectAdmissionTerminalDuplicate,
  ConvertRestartResponseOnDecision,
  ConvertLiveResponseBeforeOrdinal,
  PreservePreFenceResponseUntilCheckedDrain,
  RejectNegativeRetryBeforeOrdinal,
  BlockTerminalResurrection,
  RequireCanonicalBodyAtStartup,
  PrunePredecessorFamily,
  TerminalizeReceiverClose,
  UseFullFrozenRosterFanout,
  RequireQcSignerResponseAuthority,
  EnforceRawContextGate,
  AvoidTerminalReplayResigning,
  SignOnlyResponseStartupTerminals,
  CompletePreparedCarrierDecisionDrain

BooleanConstants ==
  {DischargeCompleteUnion,
   UseCanonicalStartupOrder,
   ResumeCompleteUnionAfterCrash,
   ResumeCanonicalOrderAfterCrash,
   PersistTerminalBeforeAdvance,
   BlockProducerWhileStartupPending,
   RequireExactReplayBinding,
   RejectOrphanTerminalWaiter,
   RejectNegativeTerminalWaiter,
   RejectOwnerRequestMismatchWaiter,
   RejectAdmissionTerminalDuplicate,
   ConvertRestartResponseOnDecision,
   ConvertLiveResponseBeforeOrdinal,
   PreservePreFenceResponseUntilCheckedDrain,
   RejectNegativeRetryBeforeOrdinal,
   BlockTerminalResurrection,
   RequireCanonicalBodyAtStartup,
   PrunePredecessorFamily,
   TerminalizeReceiverClose,
   UseFullFrozenRosterFanout,
   RequireQcSignerResponseAuthority,
   EnforceRawContextGate,
   AvoidTerminalReplayResigning,
   SignOnlyResponseStartupTerminals,
   CompletePreparedCarrierDecisionDrain}

ASSUME BooleanConstants \subseteq BOOLEAN

NoSubject == "NoSubject"

Outcome(kind, decidedSubject) ==
  [kind |-> kind, decidedSubject |-> decidedSubject]

NoOutcome == Outcome("None", NoSubject)
ResponseOutcome == Outcome("Response", NoSubject)
InvalidCertificateOutcome == Outcome("InvalidCertificate", NoSubject)
LocalAuthorityAbsentOutcome ==
  Outcome("LocalRetentionAuthorityAbsent", NoSubject)
DecisionOutcome(subject) ==
  Outcome("SupersededByDurableDecision", subject)

Admission(
    identity, family, view, lifecycleOrdinal, schedulerOrdinal,
    certificateValid, localSigner, bodyState) ==
  [kind |-> "Admission",
   identity |-> identity,
   family |-> family,
   view |-> view,
   lifecycleOrdinal |-> lifecycleOrdinal,
   schedulerOrdinal |-> schedulerOrdinal,
   certificateValid |-> certificateValid,
   localSigner |-> localSigner,
   bodyState |-> bodyState]

OwnedWaiter(
    identity, ownerIdentity, lifecycleOrdinal, schedulerOrdinal,
    physicalOrdinal) ==
  [kind |-> "Waiter",
   identity |-> identity,
   ownerIdentity |-> ownerIdentity,
   lifecycleOrdinal |-> lifecycleOrdinal,
   schedulerOrdinal |-> schedulerOrdinal,
   physicalOrdinal |-> physicalOrdinal]

Waiter(identity, lifecycleOrdinal, schedulerOrdinal, physicalOrdinal) ==
  OwnedWaiter(
    identity, identity, lifecycleOrdinal, schedulerOrdinal,
    physicalOrdinal)

Tombstone(identity, family, view, lifecycleOrdinal, outcome, outputs) ==
  [kind |-> "Tombstone",
   identity |-> identity,
   family |-> family,
   view |-> view,
   lifecycleOrdinal |-> lifecycleOrdinal,
   outcome |-> outcome,
   outputs |-> outputs]

\* Each atom represents the complete frozen context/leader/view/subject/phase
\* request identity.  Separate family/view fields let the kernel mutate the
\* independently persisted ordering and replacement relations.
Identities ==
  {"A", "B", "C", "R", "N", "Body", "Old", "New", "Close"}

Families ==
  {"family-A", "family-B", "family-C", "family-R", "family-N",
   "family-Body", "family-rollover", "family-Close"}

ExactOutputs(identity) ==
  CASE identity = "A" -> {"wire-A"}
    [] identity = "B" -> {"wire-B"}
    [] identity = "C" -> {"wire-C"}
    [] identity = "R" -> {"wire-R"}
    [] identity = "N" -> {"wire-N"}
    [] identity = "Body" -> {"wire-Body"}
    [] identity = "Old" -> {"wire-Old"}
    [] identity = "New" -> {"wire-New"}
    [] OTHER -> {"wire-Close"}

AllOutputs ==
  UNION {ExactOutputs(identity): identity \in Identities}
    \cup {"wire-mismatch"}

ReconstructedOutcome(admission) ==
  IF ~admission.certificateValid
  THEN InvalidCertificateOutcome
  ELSE IF ~admission.localSigner
       THEN LocalAuthorityAbsentOutcome
       ELSE ResponseOutcome

TerminalForAdmission(admission) ==
  LET outcome == ReconstructedOutcome(admission)
  IN Tombstone(
       admission.identity, admission.family, admission.view,
       admission.lifecycleOrdinal, outcome,
       IF outcome = ResponseOutcome
       THEN ExactOutputs(admission.identity)
       ELSE {})

StartupTerminalSignatureCost(terminal) ==
  IF terminal.outcome = ResponseOutcome
  THEN 1
  ELSE IF SignOnlyResponseStartupTerminals THEN 0 ELSE 1

RemoveIdentity(records, identity) ==
  {record \in records: record.identity # identity}

AdmissionPrecedesOrEquals(left, right) ==
  \/ left.schedulerOrdinal < right.schedulerOrdinal
  \/ /\ left.schedulerOrdinal = right.schedulerOrdinal
     /\ left.lifecycleOrdinal <= right.lifecycleOrdinal

CanonicalAdmission(records) ==
  CHOOSE candidate \in records:
    \A other \in records: AdmissionPrecedesOrEquals(candidate, other)

SelectedAdmission(records, useCanonical) ==
  IF useCanonical \/ Cardinality(records) = 1
  THEN CanonicalAdmission(records)
  ELSE CHOOSE candidate \in records:
         candidate # CanonicalAdmission(records)

AdmissionA ==
  Admission("A", "family-A", 1, 1, 2, TRUE, TRUE, "Valid")
AdmissionB ==
  Admission("B", "family-B", 1, 2, 1, FALSE, TRUE, "Valid")
AdmissionC ==
  Admission("C", "family-C", 1, 3, 3, TRUE, FALSE, "Valid")
WaiterA == Waiter("A", 1, 2, 7)
WaiterB == Waiter("B", 2, 1, 4)
UnionAdmissions == {AdmissionA, AdmissionB, AdmissionC}
UnionWaiters == {WaiterA, WaiterB}
UnionTerminals ==
  {TerminalForAdmission(admission): admission \in UnionAdmissions}
CanonicalUnionOrder ==
  LET first == CanonicalAdmission(UnionAdmissions)
      afterFirst == RemoveIdentity(UnionAdmissions, first.identity)
      second == CanonicalAdmission(afterFirst)
      afterSecond == RemoveIdentity(afterFirst, second.identity)
      third == CanonicalAdmission(afterSecond)
  IN <<first.identity, second.identity, third.identity>>

ResponseTombstoneR ==
  Tombstone(
    "R", "family-R", 4, 4, ResponseOutcome, ExactOutputs("R"))
MismatchedResponseTombstoneR ==
  Tombstone(
    "R", "family-R", 4, 4, ResponseOutcome, {"wire-mismatch"})
DecisionTombstoneR ==
  Tombstone(
    "R", "family-R", 4, 4, DecisionOutcome("Decision-B"), {})
NegativeTombstoneN ==
  Tombstone(
    "N", "family-N", 3, 9, InvalidCertificateOutcome, {})
TerminalWaiterR == Waiter("R", 4, 4, 6)
NegativeWaiterN == Waiter("N", 9, 9, 8)
NegativeRetryWaiterN == Waiter("N", 9, 10, 10)
LiveDecisionRetryWaiter == Waiter("R", 4, 10, 10)
OwnerRequestMismatchWaiterR ==
  OwnedWaiter("R", "A", 4, 4, 6)
ResponseTombstoneA == TerminalForAdmission(AdmissionA)
PreFencePreparedAdmissionR ==
  Admission(
    "R", "family-R", 4, 4, 10, TRUE, TRUE, "Valid")

MissingBodyAdmission ==
  Admission(
    "Body", "family-Body", 6, 5, 5, TRUE, TRUE, "Missing")
CorruptBodyAdmission ==
  Admission(
    "Body", "family-Body", 6, 5, 5, TRUE, TRUE, "Corrupt")
BodyWaiter == Waiter("Body", 5, 5, 7)

OldFamilyTombstone ==
  Tombstone(
    "Old", "family-rollover", 1, 6,
    ResponseOutcome, ExactOutputs("Old"))
NewFamilyAdmission ==
  Admission(
    "New", "family-rollover", 2, 7, 7, TRUE, TRUE, "Valid")
NewFamilyWaiter == Waiter("New", 7, 7, 5)
NewFamilyTombstone == TerminalForAdmission(NewFamilyAdmission)

CloseAdmission ==
  Admission(
    "Close", "family-Close", 1, 8, 8, TRUE, TRUE, "Valid")
CloseWaiter == Waiter("Close", 8, 8, 9)
CloseTombstone == TerminalForAdmission(CloseAdmission)

FrozenRoster == {"P0", "P1", "P2", "P3"}
Requester == "P0"
FrozenQcSigners == {"P0", "P2", "P3"}
ResponsiveDualQuorumSignerIntersection == {"P2", "P3"}
FullRemoteFrozenRoster == FrozenRoster \ {Requester}
NoSource == "NoSource"

Scenarios ==
  {"Union", "InterruptedUnion",
   "TerminalReplay", "TerminalDecision",
   "LiveDecisionRetry", "LiveDecisionPreFenceCarrier",
   "LiveDecisionPreFencePreparedCarrier",
   "TerminalMismatchCorrupt", "TerminalOrphanCorrupt",
   "TerminalNegativeCorrupt", "TerminalOwnerMismatchCorrupt",
   "AdmissionTerminalDuplicateCorrupt", "NegativeRetry",
   "TerminalResurrection", "MissingBody", "CorruptBody",
   "FamilyAdvance", "ReceiverClose",
   "SignerResponse", "NonSignerResponse",
   "RawActive", "RawHistorical", "RawFuture", "RawForeignSameHeight"}

\* `scenario` is an immutable bounded-fixture input for exogenous facts such
\* as the durable Decision or raw context class.  In particular, corrupt
\* terminal-waiter acceptance is not selected by this label: it is computed
\* from the persisted waiter/tombstone identity, owner, lifecycle, outcome,
\* and exact output binding below.

Phases ==
  {"ChooseScenario", "Pending", "InterruptedAfterFirst",
   "InterruptedCrashed", "ResumedAfterSecond",
   "ResumedAfterSecondCrashed", "DecisionPersisted",
   "Complete", "StartupRejected",
   "PolicyRejected", "NegativeReadmitted",
   "TerminalReplayComplete", "Resurrected"}

VARIABLES
  phase,
  scenario,
  admissions,
  waiters,
  tombstones,
  dischargeOrder,
  producerRuns,
  nextLifecycleOrdinal,
  nextSchedulerOrdinal,
  nextPhysicalOrdinal,
  signatureCount,
  emittedOutputs,
  fanout,
  responseSource,
  responseOutcome,
  transportPassed,
  lifecycleAdmitted

vars ==
  <<phase, scenario, admissions, waiters, tombstones, dischargeOrder,
    producerRuns, nextLifecycleOrdinal, nextSchedulerOrdinal,
    nextPhysicalOrdinal, signatureCount, emittedOutputs, fanout,
    responseSource, responseOutcome, transportPassed, lifecycleAdmitted>>

TypeInvariant ==
  /\ phase \in Phases
  /\ scenario \in Scenarios \cup {"None"}
  /\ IsFiniteSet(admissions)
  /\ IsFiniteSet(waiters)
  /\ IsFiniteSet(tombstones)
  /\ \A admission \in admissions:
       /\ admission.kind = "Admission"
       /\ admission.identity \in Identities
       /\ admission.family \in Families
       /\ admission.view \in Nat
       /\ admission.lifecycleOrdinal \in Nat \ {0}
       /\ admission.schedulerOrdinal \in Nat \ {0}
       /\ admission.lifecycleOrdinal < nextLifecycleOrdinal
       /\ admission.schedulerOrdinal < nextSchedulerOrdinal
       /\ admission.certificateValid \in BOOLEAN
       /\ admission.localSigner \in BOOLEAN
       /\ admission.bodyState \in {"Valid", "Missing", "Corrupt"}
  /\ \A waiter \in waiters:
       /\ waiter.kind = "Waiter"
       /\ waiter.identity \in Identities
       /\ waiter.ownerIdentity \in Identities
       /\ waiter.lifecycleOrdinal \in Nat \ {0}
       /\ waiter.schedulerOrdinal \in Nat \ {0}
       /\ waiter.physicalOrdinal \in Nat \ {0}
       /\ waiter.lifecycleOrdinal < nextLifecycleOrdinal
       /\ waiter.schedulerOrdinal < nextSchedulerOrdinal
       /\ waiter.physicalOrdinal < nextPhysicalOrdinal
  /\ \A tombstone \in tombstones:
       /\ tombstone.kind = "Tombstone"
       /\ tombstone.identity \in Identities
       /\ tombstone.family \in Families
       /\ tombstone.view \in Nat
       /\ tombstone.lifecycleOrdinal \in Nat \ {0}
       /\ tombstone.lifecycleOrdinal < nextLifecycleOrdinal
       /\ tombstone.outcome \in
            {ResponseOutcome, InvalidCertificateOutcome,
             LocalAuthorityAbsentOutcome, DecisionOutcome("Decision-B")}
       /\ tombstone.outputs \subseteq AllOutputs
  /\ dischargeOrder \in Seq(Identities)
  /\ producerRuns \in Nat
  /\ nextLifecycleOrdinal \in Nat \ {0}
  /\ nextSchedulerOrdinal \in Nat \ {0}
  /\ nextPhysicalOrdinal \in Nat \ {0}
  /\ signatureCount \in Nat
  /\ emittedOutputs \subseteq AllOutputs
  /\ fanout \subseteq FrozenRoster
  /\ responseSource \in FrozenRoster \cup {NoSource}
  /\ responseOutcome \in
       {NoOutcome, ResponseOutcome, LocalAuthorityAbsentOutcome}
  /\ transportPassed \in BOOLEAN
  /\ lifecycleAdmitted \in BOOLEAN

Init ==
  /\ phase = "ChooseScenario"
  /\ scenario = "None"
  /\ admissions = {}
  /\ waiters = {}
  /\ tombstones = {}
  /\ dischargeOrder = <<>>
  /\ producerRuns = 0
  /\ nextLifecycleOrdinal = 10
  /\ nextSchedulerOrdinal = 10
  /\ nextPhysicalOrdinal = 10
  /\ signatureCount = 0
  /\ emittedOutputs = {}
  /\ fanout = {}
  /\ responseSource = NoSource
  /\ responseOutcome = NoOutcome
  /\ ~transportPassed
  /\ ~lifecycleAdmitted

BeginStateAtCuts(
    nextScenario, nextAdmissions, nextWaiters, nextTombstones,
    nextResponseSource, nextSchedulerCut, nextPhysicalCut) ==
  /\ phase = "ChooseScenario"
  /\ phase' = "Pending"
  /\ scenario' = nextScenario
  /\ admissions' = nextAdmissions
  /\ waiters' = nextWaiters
  /\ tombstones' = nextTombstones
  /\ dischargeOrder' = <<>>
  /\ producerRuns' = 0
  /\ nextLifecycleOrdinal' = 10
  /\ nextSchedulerOrdinal' = nextSchedulerCut
  /\ nextPhysicalOrdinal' = nextPhysicalCut
  /\ signatureCount' = 0
  /\ emittedOutputs' = {}
  /\ fanout' = {}
  /\ responseSource' = nextResponseSource
  /\ responseOutcome' = NoOutcome
  /\ ~transportPassed'
  /\ ~lifecycleAdmitted'

BeginState(
    nextScenario, nextAdmissions, nextWaiters, nextTombstones,
    nextResponseSource) ==
  BeginStateAtCuts(
    nextScenario, nextAdmissions, nextWaiters, nextTombstones,
    nextResponseSource, 10, 10)

BeginUnion ==
  BeginState("Union", UnionAdmissions, UnionWaiters, {}, NoSource)

BeginInterruptedUnion ==
  BeginState(
    "InterruptedUnion", UnionAdmissions, UnionWaiters, {}, NoSource)

BeginTerminalReplay ==
  BeginState(
    "TerminalReplay", {}, {TerminalWaiterR},
    {ResponseTombstoneR}, NoSource)

BeginTerminalDecision ==
  BeginState(
    "TerminalDecision", {}, {TerminalWaiterR},
    {ResponseTombstoneR}, NoSource)

BeginLiveDecisionRetry ==
  BeginState(
    "LiveDecisionRetry", {}, {}, {ResponseTombstoneR}, NoSource)

BeginLiveDecisionPreFenceCarrier ==
  BeginStateAtCuts(
    "LiveDecisionPreFenceCarrier", {},
    {LiveDecisionRetryWaiter}, {ResponseTombstoneR}, NoSource, 11, 11)

BeginLiveDecisionPreFencePreparedCarrier ==
  BeginStateAtCuts(
    "LiveDecisionPreFencePreparedCarrier",
    {PreFencePreparedAdmissionR},
    {LiveDecisionRetryWaiter}, {}, NoSource, 11, 11)

BeginTerminalMismatchCorrupt ==
  BeginState(
    "TerminalMismatchCorrupt", {}, {TerminalWaiterR},
    {MismatchedResponseTombstoneR}, NoSource)

BeginTerminalOrphanCorrupt ==
  BeginState(
    "TerminalOrphanCorrupt", {}, {TerminalWaiterR}, {}, NoSource)

BeginTerminalNegativeCorrupt ==
  BeginState(
    "TerminalNegativeCorrupt", {}, {NegativeWaiterN},
    {NegativeTombstoneN}, NoSource)

BeginTerminalOwnerMismatchCorrupt ==
  BeginState(
    "TerminalOwnerMismatchCorrupt", {},
    {OwnerRequestMismatchWaiterR}, {ResponseTombstoneR}, NoSource)

BeginAdmissionTerminalDuplicateCorrupt ==
  BeginState(
    "AdmissionTerminalDuplicateCorrupt",
    {AdmissionA}, {WaiterA}, {ResponseTombstoneA}, NoSource)

BeginNegativeRetry ==
  BeginState(
    "NegativeRetry", {}, {}, {NegativeTombstoneN}, NoSource)

BeginTerminalResurrection ==
  BeginState(
    "TerminalResurrection", {}, {}, {ResponseTombstoneR}, NoSource)

BeginMissingBody ==
  BeginState(
    "MissingBody", {MissingBodyAdmission}, {BodyWaiter}, {}, NoSource)

BeginCorruptBody ==
  BeginState(
    "CorruptBody", {CorruptBodyAdmission}, {BodyWaiter}, {}, NoSource)

BeginFamilyAdvance ==
  BeginState(
    "FamilyAdvance", {NewFamilyAdmission}, {NewFamilyWaiter},
    {OldFamilyTombstone}, NoSource)

BeginReceiverClose ==
  BeginState(
    "ReceiverClose", {CloseAdmission}, {CloseWaiter}, {}, NoSource)

BeginSignerResponse ==
  BeginState("SignerResponse", {}, {}, {}, "P2")

BeginNonSignerResponse ==
  BeginState("NonSignerResponse", {}, {}, {}, "P1")

BeginRawActive ==
  BeginState("RawActive", {}, {}, {}, NoSource)

BeginRawHistorical ==
  BeginState("RawHistorical", {}, {}, {}, NoSource)

BeginRawFuture ==
  BeginState("RawFuture", {}, {}, {}, NoSource)

BeginRawForeignSameHeight ==
  BeginState("RawForeignSameHeight", {}, {}, {}, NoSource)

DischargeSelectedStartupOwner(selected, nextPhase, dropSuffix) ==
  LET terminal == TerminalForAdmission(selected)
      remainingAdmissions ==
        RemoveIdentity(admissions, selected.identity)
      remainingWaiters ==
        RemoveIdentity(waiters, selected.identity)
  IN
  /\ selected \in admissions
  /\ phase' = nextPhase
  /\ admissions' = IF dropSuffix THEN {} ELSE remainingAdmissions
  /\ waiters' = IF dropSuffix THEN {} ELSE remainingWaiters
  /\ tombstones' =
       IF PersistTerminalBeforeAdvance
       THEN tombstones \cup {terminal}
       ELSE tombstones
  /\ dischargeOrder' = Append(dischargeOrder, selected.identity)
  /\ emittedOutputs' =
       IF PersistTerminalBeforeAdvance
       THEN emittedOutputs \cup terminal.outputs
       ELSE emittedOutputs
  /\ signatureCount' =
       IF PersistTerminalBeforeAdvance
       THEN signatureCount + StartupTerminalSignatureCost(terminal)
       ELSE signatureCount
  /\ UNCHANGED
       <<scenario, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

DischargeNextUnionOwner ==
  /\ scenario = "Union"
  /\ phase = "Pending"
  /\ admissions # {}
  /\ LET selected ==
           SelectedAdmission(admissions, UseCanonicalStartupOrder)
         remaining ==
           RemoveIdentity(admissions, selected.identity)
         dropSuffix == ~DischargeCompleteUnion
         nextPhase ==
           IF dropSuffix \/ remaining = {}
           THEN "Complete"
           ELSE "Pending"
     IN DischargeSelectedStartupOwner(selected, nextPhase, dropSuffix)

DischargeFirstInterruptedEntry ==
  /\ scenario = "InterruptedUnion"
  /\ phase = "Pending"
  /\ admissions # {}
  /\ LET selected ==
           SelectedAdmission(admissions, UseCanonicalStartupOrder)
     IN DischargeSelectedStartupOwner(
          selected, "InterruptedAfterFirst", FALSE)

CrashDuringInterruptedDischarge ==
  /\ scenario = "InterruptedUnion"
  /\ \/ /\ phase = "InterruptedAfterFirst"
        /\ phase' = "InterruptedCrashed"
     \/ /\ phase = "ResumedAfterSecond"
        /\ phase' = "ResumedAfterSecondCrashed"
  /\ UNCHANGED
       <<scenario, admissions, waiters, tombstones, dischargeOrder,
         producerRuns, nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, emittedOutputs, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

ResumeSecondInterruptedEntry ==
  /\ scenario = "InterruptedUnion"
  /\ phase = "InterruptedCrashed"
  /\ admissions # {}
  /\ LET selected ==
           SelectedAdmission(admissions, ResumeCanonicalOrderAfterCrash)
         dropSuffix == ~ResumeCompleteUnionAfterCrash
         nextPhase ==
           IF dropSuffix
           THEN "Complete"
           ELSE "ResumedAfterSecond"
     IN DischargeSelectedStartupOwner(selected, nextPhase, dropSuffix)

DischargeFinalInterruptedEntry ==
  /\ scenario = "InterruptedUnion"
  /\ phase \in {"ResumedAfterSecond", "ResumedAfterSecondCrashed"}
  /\ Cardinality(admissions) = 1
  /\ LET selected == CanonicalAdmission(admissions)
     IN DischargeSelectedStartupOwner(selected, "Complete", FALSE)

ProducerDuringStartupDischarge ==
  /\ scenario \in {"Union", "InterruptedUnion"}
  /\ phase \in
       {"Pending", "InterruptedAfterFirst", "InterruptedCrashed",
        "ResumedAfterSecond", "ResumedAfterSecondCrashed"}
  /\ ~BlockProducerWhileStartupPending
  /\ producerRuns' = producerRuns + 1
  /\ UNCHANGED
       <<phase, scenario, admissions, waiters, tombstones,
         dischargeOrder, nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, emittedOutputs, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

WaiterHasTerminal(waiter) ==
  \E terminal \in tombstones:
    /\ terminal.identity = waiter.identity
    /\ terminal.lifecycleOrdinal = waiter.lifecycleOrdinal

TerminalForWaiter(waiter) ==
  CHOOSE terminal \in tombstones:
    /\ terminal.identity = waiter.identity
    /\ terminal.lifecycleOrdinal = waiter.lifecycleOrdinal

ExactReplayBinding(waiter, terminal) ==
  /\ terminal.identity = waiter.identity
  /\ terminal.lifecycleOrdinal = waiter.lifecycleOrdinal
  /\ \/ terminal.outcome # ResponseOutcome
     \/ terminal.outputs = ExactOutputs(waiter.identity)

TerminalWaiterAccepted ==
  /\ Cardinality(waiters) = 1
  /\ LET waiter == CHOOSE candidate \in waiters: TRUE
         hasTerminal == WaiterHasTerminal(waiter)
     IN
     /\ \/ hasTerminal
        \/ ~RejectOrphanTerminalWaiter
     /\ \/ waiter.ownerIdentity = waiter.identity
        \/ ~RejectOwnerRequestMismatchWaiter
     /\ IF hasTerminal
        THEN LET terminal == TerminalForWaiter(waiter)
             IN /\ \/ terminal.outcome = ResponseOutcome
                      \/ ~RejectNegativeTerminalWaiter
                /\ \/ ExactReplayBinding(waiter, terminal)
                   \/ ~RequireExactReplayBinding
        ELSE TRUE

TombstonesAfterTerminalWaiter ==
  IF scenario = "TerminalDecision"
  THEN IF ConvertRestartResponseOnDecision
       THEN {DecisionTombstoneR}
       ELSE {ResponseTombstoneR}
  ELSE tombstones

OutputsAfterTerminalWaiter ==
  IF scenario = "TerminalReplay"
  THEN ExactOutputs("R")
  ELSE {}

HandleTerminalWaiter ==
  /\ scenario \in
       {"TerminalReplay", "TerminalDecision",
        "TerminalMismatchCorrupt", "TerminalOrphanCorrupt",
        "TerminalNegativeCorrupt", "TerminalOwnerMismatchCorrupt"}
  /\ phase = "Pending"
  /\ phase' =
       IF TerminalWaiterAccepted
       THEN "Complete"
       ELSE "StartupRejected"
  /\ IF TerminalWaiterAccepted
     THEN /\ waiters' = {}
          /\ tombstones' = TombstonesAfterTerminalWaiter
          /\ emittedOutputs' = OutputsAfterTerminalWaiter
     ELSE /\ UNCHANGED <<waiters, tombstones, emittedOutputs>>
  /\ signatureCount' =
       IF /\ TerminalWaiterAccepted
          /\ scenario \in {"TerminalReplay", "TerminalDecision"}
          /\ ~AvoidTerminalReplayResigning
       THEN signatureCount + 1
       ELSE signatureCount
  /\ UNCHANGED
       <<scenario, admissions, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

(***************************************************************************
The runtime publication path uses the same durable-Decision conversion before
the exact retry can acquire a fresh physical/scheduler position.  This keeps
the live gate aligned with startup revalidation instead of deferring the
conversion to a newly admitted runner occurrence.
***************************************************************************)
HandleLiveDecisionRetry ==
  /\ scenario = "LiveDecisionRetry"
  /\ phase = "Pending"
  /\ phase' = "Complete"
  /\ IF ConvertLiveResponseBeforeOrdinal
     THEN /\ tombstones' = {DecisionTombstoneR}
          /\ waiters' = {}
          /\ UNCHANGED
               <<nextSchedulerOrdinal, nextPhysicalOrdinal>>
     ELSE /\ tombstones' = {ResponseTombstoneR}
          /\ waiters' = {LiveDecisionRetryWaiter}
          /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
          /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
  /\ emittedOutputs' = {}
  /\ signatureCount' =
       IF AvoidTerminalReplayResigning
       THEN signatureCount
       ELSE signatureCount + 1
  /\ UNCHANGED
       <<scenario, admissions, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

PublishDecisionWithPreFenceCarrier ==
  /\ scenario = "LiveDecisionPreFenceCarrier"
  /\ phase = "Pending"
  /\ phase' = "DecisionPersisted"
  /\ waiters' = {LiveDecisionRetryWaiter}
  /\ tombstones' =
       IF PreservePreFenceResponseUntilCheckedDrain
       THEN {ResponseTombstoneR}
       ELSE {DecisionTombstoneR}
  /\ emittedOutputs' = {}
  /\ signatureCount' =
       IF AvoidTerminalReplayResigning
       THEN signatureCount
       ELSE signatureCount + 1
  /\ UNCHANGED
       <<scenario, admissions, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

DrainPreFenceCarrierAfterDecision ==
  /\ scenario = "LiveDecisionPreFenceCarrier"
  /\ phase = "DecisionPersisted"
  /\ ResponseTombstoneR \in tombstones
  /\ phase' = "Complete"
  /\ waiters' = {}
  /\ tombstones' = {DecisionTombstoneR}
  /\ emittedOutputs' = {}
  /\ UNCHANGED
       <<scenario, admissions, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

PublishDecisionWithPreparedCarrier ==
  /\ scenario = "LiveDecisionPreFencePreparedCarrier"
  /\ phase = "Pending"
  /\ phase' = "DecisionPersisted"
  /\ emittedOutputs' = {}
  /\ UNCHANGED
       <<scenario, admissions, waiters, tombstones, dischargeOrder,
         producerRuns, nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

DrainPreparedCarrierAfterDecision ==
  /\ scenario = "LiveDecisionPreFencePreparedCarrier"
  /\ phase = "DecisionPersisted"
  /\ IF CompletePreparedCarrierDecisionDrain
     THEN /\ phase' = "Complete"
          /\ admissions' = {}
          /\ waiters' = {}
          /\ tombstones' = {DecisionTombstoneR}
     ELSE /\ phase' = "PolicyRejected"
          /\ UNCHANGED <<admissions, waiters, tombstones>>
  /\ emittedOutputs' = {}
  /\ UNCHANGED
       <<scenario, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

HandleAdmissionTerminalDuplicate ==
  /\ scenario = "AdmissionTerminalDuplicateCorrupt"
  /\ phase = "Pending"
  /\ IF RejectAdmissionTerminalDuplicate
     THEN /\ phase' = "StartupRejected"
          /\ UNCHANGED
               <<admissions, waiters, tombstones, emittedOutputs>>
     ELSE /\ phase' = "Complete"
          /\ admissions' = {}
          /\ waiters' = {}
          /\ tombstones' = {ResponseTombstoneA}
          /\ emittedOutputs' = ExactOutputs("A")
  /\ UNCHANGED
       <<scenario, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

TryNegativeRetry ==
  /\ scenario = "NegativeRetry"
  /\ phase = "Pending"
  /\ IF RejectNegativeRetryBeforeOrdinal
     THEN /\ phase' = "PolicyRejected"
          /\ UNCHANGED
               <<waiters, nextSchedulerOrdinal, nextPhysicalOrdinal>>
     ELSE /\ phase' = "NegativeReadmitted"
          \* The mutant acquires the current fresh physical/shared carrier
          \* while retaining the already-terminal logical lifecycle ordinal.
          /\ waiters' = {NegativeRetryWaiterN}
          /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
          /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
  /\ UNCHANGED
       <<scenario, admissions, tombstones, dischargeOrder,
         producerRuns, nextLifecycleOrdinal, signatureCount,
         emittedOutputs, fanout, responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

ResurrectedAdmissionR ==
  Admission(
    "R", "family-R", 4, 10, 10, TRUE, TRUE, "Valid")
ResurrectedWaiterR == Waiter("R", 10, 10, 10)

TryTerminalRetry ==
  /\ scenario = "TerminalResurrection"
  /\ phase = "Pending"
  /\ IF BlockTerminalResurrection
     THEN /\ phase' = "TerminalReplayComplete"
          /\ admissions' = {}
          /\ waiters' = {}
          /\ emittedOutputs' = ExactOutputs("R")
          /\ UNCHANGED nextLifecycleOrdinal
          /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
          /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
     ELSE /\ phase' = "Resurrected"
          /\ admissions' = {ResurrectedAdmissionR}
          /\ waiters' = {ResurrectedWaiterR}
          /\ emittedOutputs' = {}
          /\ nextLifecycleOrdinal' = nextLifecycleOrdinal + 1
          /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
          /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
  /\ UNCHANGED
       <<scenario, tombstones, dischargeOrder, producerRuns,
         signatureCount, fanout, responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

BodyFailureAdmission ==
  IF scenario = "MissingBody"
  THEN MissingBodyAdmission
  ELSE CorruptBodyAdmission

RestartWithUnavailableCanonicalBody ==
  /\ scenario \in {"MissingBody", "CorruptBody"}
  /\ phase = "Pending"
  /\ IF RequireCanonicalBodyAtStartup
     THEN /\ phase' = "StartupRejected"
          /\ UNCHANGED
               <<admissions, waiters, tombstones, emittedOutputs>>
     ELSE /\ phase' = "Complete"
          /\ admissions' = {}
          /\ waiters' = {}
          /\ tombstones' =
               {Tombstone(
                  "Body", "family-Body", 6, 5,
                  LocalAuthorityAbsentOutcome, {})}
          /\ emittedOutputs' = {}
  /\ UNCHANGED
       <<scenario, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

RestartFamilyAdvance ==
  /\ scenario = "FamilyAdvance"
  /\ phase = "Pending"
  /\ phase' = "Complete"
  /\ admissions' = {}
  /\ waiters' = {}
  /\ tombstones' =
       IF PrunePredecessorFamily
       THEN {NewFamilyTombstone}
       ELSE {OldFamilyTombstone, NewFamilyTombstone}
  /\ emittedOutputs' = ExactOutputs("New")
  /\ dischargeOrder' = <<"New">>
  /\ signatureCount' = signatureCount + 1
  /\ UNCHANGED
       <<scenario, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

CloseServeReceiver ==
  /\ scenario = "ReceiverClose"
  /\ phase = "Pending"
  /\ phase' = "Complete"
  /\ IF TerminalizeReceiverClose
     THEN /\ admissions' = {}
          /\ waiters' = {}
          /\ tombstones' = {CloseTombstone}
          /\ emittedOutputs' = ExactOutputs("Close")
          /\ signatureCount' = signatureCount + 1
     ELSE /\ UNCHANGED <<admissions, waiters, tombstones>>
          /\ emittedOutputs' = {}
          /\ UNCHANGED signatureCount
  /\ UNCHANGED
       <<scenario, dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, fanout,
         responseSource, responseOutcome,
         transportPassed, lifecycleAdmitted>>

ClassifyCertifiedResponseAuthority ==
  /\ scenario \in {"SignerResponse", "NonSignerResponse"}
  /\ phase = "Pending"
  /\ phase' = "Complete"
  /\ fanout' =
       IF UseFullFrozenRosterFanout
       THEN FullRemoteFrozenRoster
       ELSE FrozenQcSigners \ {Requester}
  /\ responseOutcome' =
       IF \/ ~RequireQcSignerResponseAuthority
          \/ responseSource \in FrozenQcSigners
       THEN ResponseOutcome
       ELSE LocalAuthorityAbsentOutcome
  /\ emittedOutputs' =
       IF \/ ~RequireQcSignerResponseAuthority
          \/ responseSource \in FrozenQcSigners
       THEN {"wire-R"}
       ELSE {}
  /\ transportPassed'
  /\ ~lifecycleAdmitted'
  /\ UNCHANGED
       <<scenario, admissions, waiters, tombstones,
         dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount>>

ApplyCertifiedRawContextGate ==
  /\ scenario \in
       {"RawActive", "RawHistorical",
        "RawFuture", "RawForeignSameHeight"}
  /\ phase = "Pending"
  /\ phase' = "Complete"
  /\ IF EnforceRawContextGate
     THEN /\ transportPassed' =
               scenario \in {"RawActive", "RawHistorical"}
          /\ lifecycleAdmitted' = (scenario = "RawActive")
     ELSE /\ transportPassed'
          /\ lifecycleAdmitted'
  /\ UNCHANGED
       <<scenario, admissions, waiters, tombstones,
         dischargeOrder, producerRuns,
         nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, signatureCount, emittedOutputs, fanout,
         responseSource, responseOutcome>>

Next ==
  \/ BeginUnion
  \/ BeginInterruptedUnion
  \/ BeginTerminalReplay
  \/ BeginTerminalDecision
  \/ BeginLiveDecisionRetry
  \/ BeginLiveDecisionPreFenceCarrier
  \/ BeginLiveDecisionPreFencePreparedCarrier
  \/ BeginTerminalMismatchCorrupt
  \/ BeginTerminalOrphanCorrupt
  \/ BeginTerminalNegativeCorrupt
  \/ BeginTerminalOwnerMismatchCorrupt
  \/ BeginAdmissionTerminalDuplicateCorrupt
  \/ BeginNegativeRetry
  \/ BeginTerminalResurrection
  \/ BeginMissingBody
  \/ BeginCorruptBody
  \/ BeginFamilyAdvance
  \/ BeginReceiverClose
  \/ BeginSignerResponse
  \/ BeginNonSignerResponse
  \/ BeginRawActive
  \/ BeginRawHistorical
  \/ BeginRawFuture
  \/ BeginRawForeignSameHeight
  \/ DischargeNextUnionOwner
  \/ DischargeFirstInterruptedEntry
  \/ CrashDuringInterruptedDischarge
  \/ ResumeSecondInterruptedEntry
  \/ DischargeFinalInterruptedEntry
  \/ ProducerDuringStartupDischarge
  \/ HandleTerminalWaiter
  \/ HandleLiveDecisionRetry
  \/ PublishDecisionWithPreFenceCarrier
  \/ DrainPreFenceCarrierAfterDecision
  \/ PublishDecisionWithPreparedCarrier
  \/ DrainPreparedCarrierAfterDecision
  \/ HandleAdmissionTerminalDuplicate
  \/ TryNegativeRetry
  \/ TryTerminalRetry
  \/ RestartWithUnavailableCanonicalBody
  \/ RestartFamilyAdvance
  \/ CloseServeReceiver
  \/ ClassifyCertifiedResponseAuthority
  \/ ApplyCertifiedRawContextGate

Spec == Init /\ [][Next]_vars

RestartUnionDischargesEveryAdmission ==
  /\ scenario = "Union"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = UnionTerminals
       /\ emittedOutputs = ExactOutputs("A")
       /\ signatureCount = 1

RestartUnionUsesCanonicalOrder ==
  /\ scenario = "Union"
  /\ phase = "Complete"
    => dischargeOrder = CanonicalUnionOrder

InterruptedDischargePersistsBeforeAdvance ==
  /\ ((/\ scenario = "InterruptedUnion"
       /\ phase \in {"InterruptedAfterFirst", "InterruptedCrashed"})
        => /\ tombstones = {TerminalForAdmission(AdmissionB)}
           /\ admissions = {AdmissionA, AdmissionC}
           /\ waiters = {WaiterA}
           /\ dischargeOrder = <<"B">>
           /\ emittedOutputs = {}
           /\ signatureCount = 0)
  /\ ((/\ scenario = "InterruptedUnion"
       /\ phase \in
            {"ResumedAfterSecond", "ResumedAfterSecondCrashed"})
        => /\ tombstones =
                {TerminalForAdmission(AdmissionB),
                 TerminalForAdmission(AdmissionA)}
           /\ admissions = {AdmissionC}
           /\ waiters = {}
           /\ dischargeOrder = <<"B", "A">>
           /\ emittedOutputs = ExactOutputs("A")
           /\ signatureCount = 1)

InterruptedRestartResumesWithoutReopening ==
  /\ scenario = "InterruptedUnion"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = UnionTerminals
       /\ dischargeOrder = CanonicalUnionOrder
       /\ emittedOutputs = ExactOutputs("A")
       /\ signatureCount = 1

CrashResumeDischargesEveryRemainingAdmission ==
  /\ scenario = "InterruptedUnion"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = UnionTerminals
       /\ emittedOutputs = ExactOutputs("A")
       /\ signatureCount = 1

CrashResumeUsesCanonicalRemainingOrder ==
  /\ scenario = "InterruptedUnion"
  /\ phase = "Complete"
    => dischargeOrder = CanonicalUnionOrder

ProducerHiddenUntilStartupDischarged ==
  /\ scenario \in {"Union", "InterruptedUnion"}
  /\ phase \in
       {"Pending", "InterruptedAfterFirst", "InterruptedCrashed",
        "ResumedAfterSecond", "ResumedAfterSecondCrashed"}
    => producerRuns = 0

TerminalReplayIsExactAndOrdinalStable ==
  /\ scenario = "TerminalReplay"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = {ResponseTombstoneR}
       /\ emittedOutputs = ExactOutputs("R")
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

RestartDecisionSupersessionConvertsResponseAtomically ==
  /\ scenario = "TerminalDecision"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = {DecisionTombstoneR}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

LiveDecisionSupersessionConvertsResponseBeforeOrdinal ==
  /\ scenario = "LiveDecisionRetry"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = {DecisionTombstoneR}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

PreFenceCarrierDefersDecisionRewriteUntilCheckedDrain ==
  /\ scenario = "LiveDecisionPreFenceCarrier"
  /\ phase \in {"DecisionPersisted", "Complete"}
    => /\ IF phase = "DecisionPersisted"
          THEN /\ tombstones = {ResponseTombstoneR}
               /\ waiters = {LiveDecisionRetryWaiter}
          ELSE /\ tombstones = {DecisionTombstoneR}
               /\ waiters = {}
       /\ admissions = {}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 11
       /\ nextPhysicalOrdinal = 11
       /\ signatureCount = 0

PreparedCarrierDecisionDrainIsAtomicAndOrdinalStable ==
  /\ scenario = "LiveDecisionPreFencePreparedCarrier"
  /\ phase \in {"DecisionPersisted", "Complete", "PolicyRejected"}
    => /\ phase # "PolicyRejected"
       /\ IF phase = "DecisionPersisted"
          THEN /\ admissions = {PreFencePreparedAdmissionR}
               /\ waiters = {LiveDecisionRetryWaiter}
               /\ tombstones = {}
          ELSE /\ admissions = {}
               /\ waiters = {}
               /\ tombstones = {DecisionTombstoneR}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 11
       /\ nextPhysicalOrdinal = 11
       /\ signatureCount = 0

InitialCorruptWaiters(nextScenario) ==
  IF nextScenario = "TerminalNegativeCorrupt"
  THEN {NegativeWaiterN}
  ELSE IF nextScenario = "TerminalOwnerMismatchCorrupt"
       THEN {OwnerRequestMismatchWaiterR}
  ELSE {TerminalWaiterR}

InitialCorruptTombstones(nextScenario) ==
  CASE nextScenario = "TerminalNegativeCorrupt" -> {NegativeTombstoneN}
    [] nextScenario = "TerminalMismatchCorrupt" ->
         {MismatchedResponseTombstoneR}
    [] nextScenario = "TerminalOwnerMismatchCorrupt" ->
         {ResponseTombstoneR}
    [] OTHER -> {}

CorruptTerminalWaiterFailStops ==
  /\ scenario \in
       {"TerminalMismatchCorrupt",
        "TerminalOrphanCorrupt", "TerminalNegativeCorrupt",
        "TerminalOwnerMismatchCorrupt"}
  /\ phase \in {"Complete", "StartupRejected"}
    => /\ phase = "StartupRejected"
       /\ admissions = {}
       /\ waiters = InitialCorruptWaiters(scenario)
       /\ tombstones = InitialCorruptTombstones(scenario)
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

OwnerRequestMismatchWaiterFailStops ==
  /\ scenario = "TerminalOwnerMismatchCorrupt"
  /\ phase \in {"Complete", "StartupRejected"}
    => /\ phase = "StartupRejected"
       /\ admissions = {}
       /\ waiters = {OwnerRequestMismatchWaiterR}
       /\ tombstones = {ResponseTombstoneR}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

DuplicateAdmissionTerminalFailsStartupAndPreservesState ==
  /\ scenario = "AdmissionTerminalDuplicateCorrupt"
  /\ phase \in {"Complete", "StartupRejected"}
    => /\ phase = "StartupRejected"
       /\ admissions = {AdmissionA}
       /\ waiters = {WaiterA}
       /\ tombstones = {ResponseTombstoneA}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

NegativeRetryConsumesNoFreshOrdinal ==
  /\ scenario = "NegativeRetry"
  /\ phase \in {"PolicyRejected", "NegativeReadmitted"}
    => /\ phase = "PolicyRejected"
       /\ waiters = {}
       /\ tombstones = {NegativeTombstoneN}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

TerminalResponseRetryUsesFreshCarrierWithoutLifecycleResurrection ==
  /\ scenario = "TerminalResurrection"
  /\ phase \in {"TerminalReplayComplete", "Resurrected"}
    => /\ phase = "TerminalReplayComplete"
       /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = {ResponseTombstoneR}
       /\ emittedOutputs = ExactOutputs("R")
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 11
       /\ nextPhysicalOrdinal = 11
       /\ signatureCount = 0

InitialBodyAdmission(nextScenario) ==
  IF nextScenario = "MissingBody"
  THEN MissingBodyAdmission
  ELSE CorruptBodyAdmission

MissingOrCorruptBodyFailStopsAndPreservesState ==
  /\ scenario \in {"MissingBody", "CorruptBody"}
  /\ phase \in {"Complete", "StartupRejected"}
    => /\ phase = "StartupRejected"
       /\ admissions = {InitialBodyAdmission(scenario)}
       /\ waiters = {BodyWaiter}
       /\ tombstones = {}
       /\ emittedOutputs = {}
       /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal = 10
       /\ nextPhysicalOrdinal = 10
       /\ signatureCount = 0

SuccessorTerminalPrunesPredecessorFamily ==
  /\ scenario = "FamilyAdvance"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = {NewFamilyTombstone}
       /\ dischargeOrder = <<"New">>
       /\ signatureCount = 1

ReceiverClosePublishesTypedTerminalWithoutDebt ==
  /\ scenario = "ReceiverClose"
  /\ phase = "Complete"
    => /\ admissions = {}
       /\ waiters = {}
       /\ tombstones = {CloseTombstone}
       /\ emittedOutputs = ExactOutputs("Close")
       /\ signatureCount = 1

CertifiedRequestFansOutToFullFrozenRoster ==
  /\ scenario \in {"SignerResponse", "NonSignerResponse"}
  /\ phase = "Complete"
    => fanout = FullRemoteFrozenRoster

OnlyFrozenQcSignersCanRespond ==
  /\ ((/\ scenario = "SignerResponse"
       /\ phase = "Complete")
        => /\ responseSource \in
                ResponsiveDualQuorumSignerIntersection
           /\ responseSource \in FrozenQcSigners
           /\ responseOutcome = ResponseOutcome
           /\ emittedOutputs = {"wire-R"})
  /\ ((/\ scenario = "NonSignerResponse"
       /\ phase = "Complete")
        => /\ responseSource \notin FrozenQcSigners
           /\ responseOutcome = LocalAuthorityAbsentOutcome
           /\ emittedOutputs = {})

RawContextGateSeparatesLifecycleAuthority ==
  /\ ((/\ scenario = "RawActive"
       /\ phase = "Complete")
        => /\ transportPassed
           /\ lifecycleAdmitted)
  /\ ((/\ scenario = "RawHistorical"
       /\ phase = "Complete")
        => /\ transportPassed
           /\ ~lifecycleAdmitted)
  /\ ((/\ scenario \in {"RawFuture", "RawForeignSameHeight"}
       /\ phase = "Complete")
        => /\ ~transportPassed
           /\ ~lifecycleAdmitted)

TerminalReplayAndDecisionConversionDoNotResignOrMintOrdinal ==
  /\ scenario \in
       {"TerminalReplay", "TerminalDecision",
        "LiveDecisionRetry", "LiveDecisionPreFenceCarrier",
        "LiveDecisionPreFencePreparedCarrier"}
  /\ phase # "ChooseScenario"
    => /\ nextLifecycleOrdinal = 10
       /\ nextSchedulerOrdinal =
            IF scenario \in
                 {"LiveDecisionPreFenceCarrier",
                  "LiveDecisionPreFencePreparedCarrier"}
            THEN 11
            ELSE 10
       /\ nextPhysicalOrdinal =
            IF scenario \in
                 {"LiveDecisionPreFenceCarrier",
                  "LiveDecisionPreFencePreparedCarrier"}
            THEN 11
            ELSE 10
       /\ signatureCount = 0

UnsealedRestartResponsesSignExactlyOnce ==
  /\ scenario \in
       {"Union", "InterruptedUnion", "FamilyAdvance", "ReceiverClose"}
  /\ phase = "Complete"
    => CASE scenario = "Union" -> signatureCount = 1
         [] scenario = "InterruptedUnion" -> signatureCount = 1
         [] scenario = "FamilyAdvance" -> signatureCount = 1
         [] OTHER -> signatureCount = 1

=============================================================================
