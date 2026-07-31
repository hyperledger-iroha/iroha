---- MODULE SumeragiV2TerminalIngressLifecycleProofs ----
EXTENDS Naturals, TLAPS

(***************************************************************************
Terminal ingress process-lifetime safety.

The production terminal runner enters this machine only after the exact live
or recovered terminal boundary has closed consensus ingress.  A successful
entry owns one non-copyable history-service lease.  While that lease exists,
the abstract admission actions represent immutable history requests; the
production refinement separately must establish authenticated current-version
classification.  Every exit revokes the lease, retires all remaining request
owners, and advances the same process-local ingress irreversibly to
TerminalRetired.

This is a suffix-safety model.  It deliberately makes no fairness claim that
the history-service loop eventually exits.  A restarted process owns a new
ingress instance and is covered by a fresh recovered-terminal entry; restart
is not a TerminalRetired-to-Closed transition in this machine.
***************************************************************************)

VARIABLES terminalIngressMode,
          terminalServiceOwner,
          terminalIngressOwners,
          terminalDetachedOwners,
          terminalSuccessfulAdmissions

terminalIngressVars ==
  <<terminalIngressMode,
    terminalServiceOwner,
    terminalIngressOwners,
    terminalDetachedOwners,
    terminalSuccessfulAdmissions>>

TerminalClosed == "Closed"
TerminalReadOnly == "TerminalReadOnly"
TerminalRetired == "TerminalRetired"

TerminalIngressModes ==
  {TerminalClosed, TerminalReadOnly, TerminalRetired}

TerminalAbsorbingModes == {TerminalReadOnly, TerminalRetired}

(***************************************************************************
The exact terminal boundary establishes Closed with no service owner.  The
successful Enter action nondeterministically records the number of already
authenticated history requests detached from the former consensus lanes;
all other pre-terminal owners are retired by that production transition.
***************************************************************************)
TerminalIngressLifecycleInit ==
  /\ terminalIngressMode = TerminalClosed
  /\ terminalServiceOwner = FALSE
  /\ terminalIngressOwners = 0
  /\ terminalDetachedOwners = 0
  /\ terminalSuccessfulAdmissions = 0

EnterTerminalReadOnly ==
  /\ terminalIngressMode = TerminalClosed
  /\ ~terminalServiceOwner
  /\ \E retainedHistory \in Nat:
       /\ terminalIngressMode' = TerminalReadOnly
       /\ terminalServiceOwner' = TRUE
       /\ terminalIngressOwners' = 0
       /\ terminalDetachedOwners' = retainedHistory
       /\ terminalSuccessfulAdmissions' =
            terminalSuccessfulAdmissions

(***************************************************************************
Enqueue and Coalesce are both successful caller-visible admissions.  Only an
enqueue creates another ingress owner.  Coalescing requires an existing exact
wire owner in the live terminal ingress; detached pre-entry requests are no
longer present in that deduplication index.
***************************************************************************)
AdmitTerminalHistoryEnqueue ==
  /\ terminalIngressMode = TerminalReadOnly
  /\ terminalServiceOwner
  /\ terminalIngressMode' = terminalIngressMode
  /\ terminalServiceOwner' = terminalServiceOwner
  /\ terminalIngressOwners' = terminalIngressOwners + 1
  /\ terminalDetachedOwners' = terminalDetachedOwners
  /\ terminalSuccessfulAdmissions' =
       terminalSuccessfulAdmissions + 1

AdmitTerminalHistoryCoalesce ==
  /\ terminalIngressMode = TerminalReadOnly
  /\ terminalServiceOwner
  /\ terminalIngressOwners > 0
  /\ terminalIngressMode' = terminalIngressMode
  /\ terminalServiceOwner' = terminalServiceOwner
  /\ terminalIngressOwners' = terminalIngressOwners
  /\ terminalDetachedOwners' = terminalDetachedOwners
  /\ terminalSuccessfulAdmissions' =
       terminalSuccessfulAdmissions + 1

(***************************************************************************
Rejected mutation/lane/history attempts do not transfer ownership.  This one
action represents Retry, Rejected, Obsolete, and FailStop outcomes; none is a
successful admission.
***************************************************************************)
RejectTerminalAdmission ==
  /\ terminalIngressMode \in TerminalAbsorbingModes
  /\ UNCHANGED terminalIngressVars

(***************************************************************************
The terminal runner consumes detached entry owners first, then requests which
arrived through TerminalReadOnly ingress.  Dequeue never creates ownership or
changes the caller-visible successful-admission count.
***************************************************************************)
DequeueTerminalHistory ==
  /\ terminalIngressMode = TerminalReadOnly
  /\ terminalServiceOwner
  /\ terminalDetachedOwners + terminalIngressOwners > 0
  /\ terminalIngressMode' = terminalIngressMode
  /\ terminalServiceOwner' = terminalServiceOwner
  /\ terminalDetachedOwners' =
       IF terminalDetachedOwners > 0
       THEN terminalDetachedOwners - 1
       ELSE terminalDetachedOwners
  /\ terminalIngressOwners' =
       IF terminalDetachedOwners > 0
       THEN terminalIngressOwners
       ELSE terminalIngressOwners - 1
  /\ terminalSuccessfulAdmissions' =
       terminalSuccessfulAdmissions

(***************************************************************************
Open, Close, Configure, and a second terminal entry are all exact no-ops once
the ingress has entered an absorbing terminal mode.
***************************************************************************)
TerminalControlNoOp ==
  /\ terminalIngressMode \in TerminalAbsorbingModes
  /\ UNCHANGED terminalIngressVars

(***************************************************************************
Every post-entry park exit performs this transition before the sole service
owner is destroyed.  Both detached and concurrently admitted owners are
retired in the same process-lifetime handoff.
***************************************************************************)
ExitTerminalHistoryService ==
  /\ terminalIngressMode = TerminalReadOnly
  /\ terminalServiceOwner
  /\ terminalIngressMode' = TerminalRetired
  /\ terminalServiceOwner' = FALSE
  /\ terminalIngressOwners' = 0
  /\ terminalDetachedOwners' = 0
  /\ terminalSuccessfulAdmissions' =
       terminalSuccessfulAdmissions

IdempotentTerminalRetire ==
  /\ terminalIngressMode = TerminalRetired
  /\ ~terminalServiceOwner
  /\ UNCHANGED terminalIngressVars

TerminalIngressStutter == UNCHANGED terminalIngressVars

TerminalIngressLifecycleNext ==
  \/ EnterTerminalReadOnly
  \/ AdmitTerminalHistoryEnqueue
  \/ AdmitTerminalHistoryCoalesce
  \/ RejectTerminalAdmission
  \/ DequeueTerminalHistory
  \/ TerminalControlNoOp
  \/ ExitTerminalHistoryService
  \/ IdempotentTerminalRetire
  \/ TerminalIngressStutter

TerminalIngressLifecycleSpec ==
  /\ TerminalIngressLifecycleInit
  /\ [][TerminalIngressLifecycleNext]_terminalIngressVars

(***************************************************************************
Inductive state invariant.  The service lease is exact rather than advisory:
it exists precisely in TerminalReadOnly.  Closed and Retired own no queued
history requests, and Retired is the unique post-owner state.
***************************************************************************)
TerminalIngressAbsorbencyInvariant ==
  /\ terminalIngressMode \in TerminalIngressModes
  /\ terminalServiceOwner \in BOOLEAN
  /\ terminalIngressOwners \in Nat
  /\ terminalDetachedOwners \in Nat
  /\ terminalSuccessfulAdmissions \in Nat
  /\ (terminalServiceOwner
        <=> terminalIngressMode = TerminalReadOnly)
  /\ (terminalIngressMode = TerminalClosed
        => /\ terminalIngressOwners = 0
           /\ terminalDetachedOwners = 0)
  /\ (terminalIngressMode = TerminalRetired
        => /\ ~terminalServiceOwner
           /\ terminalIngressOwners = 0
           /\ terminalDetachedOwners = 0)

THEOREM TerminalIngressLifecycleInitEstablishesAbsorbencyInvariant ==
  TerminalIngressLifecycleInit => TerminalIngressAbsorbencyInvariant
BY SMTT(30)
   DEF TerminalIngressLifecycleInit,
       TerminalIngressAbsorbencyInvariant, TerminalIngressModes,
       TerminalClosed, TerminalReadOnly, TerminalRetired

THEOREM EnterTerminalReadOnlyPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ EnterTerminalReadOnly
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant, EnterTerminalReadOnly,
       TerminalIngressModes, TerminalClosed, TerminalReadOnly,
       TerminalRetired

THEOREM AdmitTerminalHistoryEnqueuePreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ AdmitTerminalHistoryEnqueue
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant,
       AdmitTerminalHistoryEnqueue, TerminalIngressModes,
       TerminalClosed, TerminalReadOnly, TerminalRetired

THEOREM AdmitTerminalHistoryCoalescePreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ AdmitTerminalHistoryCoalesce
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant,
       AdmitTerminalHistoryCoalesce, TerminalIngressModes,
       TerminalClosed, TerminalReadOnly, TerminalRetired

THEOREM RejectTerminalAdmissionPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ RejectTerminalAdmission
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant, RejectTerminalAdmission,
       terminalIngressVars

THEOREM DequeueTerminalHistoryPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ DequeueTerminalHistory
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant, DequeueTerminalHistory,
       TerminalIngressModes, TerminalClosed, TerminalReadOnly,
       TerminalRetired

THEOREM TerminalControlNoOpPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ TerminalControlNoOp
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant, TerminalControlNoOp,
       terminalIngressVars

THEOREM ExitTerminalHistoryServicePreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ ExitTerminalHistoryService
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant,
       ExitTerminalHistoryService, TerminalIngressModes,
       TerminalClosed, TerminalReadOnly, TerminalRetired

THEOREM IdempotentTerminalRetirePreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ IdempotentTerminalRetire
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant, IdempotentTerminalRetire,
       terminalIngressVars

THEOREM TerminalIngressStutterPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ TerminalIngressStutter
    => TerminalIngressAbsorbencyInvariant'
BY SMTT(30)
   DEF TerminalIngressAbsorbencyInvariant, TerminalIngressStutter,
       terminalIngressVars

THEOREM TerminalIngressLifecycleNextPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant /\ TerminalIngressLifecycleNext
    => TerminalIngressAbsorbencyInvariant'
PROOF
  <1>1. CASE EnterTerminalReadOnly
    BY <1>1, EnterTerminalReadOnlyPreservesAbsorbencyInvariant
  <1>2. CASE AdmitTerminalHistoryEnqueue
    BY <1>2, AdmitTerminalHistoryEnqueuePreservesAbsorbencyInvariant
  <1>3. CASE AdmitTerminalHistoryCoalesce
    BY <1>3, AdmitTerminalHistoryCoalescePreservesAbsorbencyInvariant
  <1>4. CASE RejectTerminalAdmission
    BY <1>4, RejectTerminalAdmissionPreservesAbsorbencyInvariant
  <1>5. CASE DequeueTerminalHistory
    BY <1>5, DequeueTerminalHistoryPreservesAbsorbencyInvariant
  <1>6. CASE TerminalControlNoOp
    BY <1>6, TerminalControlNoOpPreservesAbsorbencyInvariant
  <1>7. CASE ExitTerminalHistoryService
    BY <1>7, ExitTerminalHistoryServicePreservesAbsorbencyInvariant
  <1>8. CASE IdempotentTerminalRetire
    BY <1>8, IdempotentTerminalRetirePreservesAbsorbencyInvariant
  <1>9. CASE TerminalIngressStutter
    BY <1>9, TerminalIngressStutterPreservesAbsorbencyInvariant
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
       <1>8, <1>9 DEF TerminalIngressLifecycleNext

THEOREM TerminalIngressLifecycleStepPreservesAbsorbencyInvariant ==
  TerminalIngressAbsorbencyInvariant
    /\ [TerminalIngressLifecycleNext]_terminalIngressVars
    => TerminalIngressAbsorbencyInvariant'
PROOF
  <1>1. CASE TerminalIngressLifecycleNext
    BY <1>1, TerminalIngressLifecycleNextPreservesAbsorbencyInvariant
  <1>2. CASE UNCHANGED terminalIngressVars
    BY <1>2, TerminalIngressStutterPreservesAbsorbencyInvariant
       DEF TerminalIngressStutter
  <1> QED BY <1>1, <1>2

THEOREM TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant ==
  TerminalIngressLifecycleSpec => []TerminalIngressAbsorbencyInvariant
PROOF
  <1>1. TerminalIngressLifecycleInit
           => TerminalIngressAbsorbencyInvariant
    BY TerminalIngressLifecycleInitEstablishesAbsorbencyInvariant
  <1>2. TerminalIngressAbsorbencyInvariant
           /\ [TerminalIngressLifecycleNext]_terminalIngressVars
           => TerminalIngressAbsorbencyInvariant'
    BY TerminalIngressLifecycleStepPreservesAbsorbencyInvariant
  <1> QED BY <1>1, <1>2, PTL DEF TerminalIngressLifecycleSpec

(***************************************************************************
Four action properties expose the process-lifetime contract directly.  They
remain separate from the state invariant so the production step projection
must account for terminal-mode preservation, permanent retirement, owner-exit
ordering, and caller-visible admission independently.
***************************************************************************)
TerminalModeAbsorbingStep ==
  terminalIngressMode \in TerminalAbsorbingModes
    => terminalIngressMode' \in TerminalAbsorbingModes

TerminalRetiredAbsorbingStep ==
  terminalIngressMode = TerminalRetired
    => terminalIngressMode' = TerminalRetired

EveryServiceOwnerExitRetiresStep ==
  terminalServiceOwner /\ ~terminalServiceOwner'
    => /\ terminalIngressMode = TerminalReadOnly
       /\ terminalIngressMode' = TerminalRetired
       /\ terminalIngressOwners' = 0
       /\ terminalDetachedOwners' = 0
       /\ terminalSuccessfulAdmissions' =
            terminalSuccessfulAdmissions

NoPostOwnerAdmissionStep ==
  ~terminalServiceOwner
    /\ terminalIngressMode \in TerminalAbsorbingModes
    => terminalSuccessfulAdmissions' = terminalSuccessfulAdmissions

TerminalIngressAbsorbencyStepProperties ==
  /\ TerminalModeAbsorbingStep
  /\ TerminalRetiredAbsorbingStep
  /\ EveryServiceOwnerExitRetiresStep
  /\ NoPostOwnerAdmissionStep

THEOREM TerminalIngressLifecycleNextEstablishesAbsorbencyStepProperties ==
  TerminalIngressAbsorbencyInvariant
    /\ TerminalIngressLifecycleNext
    => TerminalIngressAbsorbencyStepProperties
PROOF
  <1>1. CASE EnterTerminalReadOnly
    BY <1>1, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           EnterTerminalReadOnly, TerminalAbsorbingModes,
           TerminalReadOnly, TerminalRetired
  <1>2. CASE AdmitTerminalHistoryEnqueue
    BY <1>2, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           AdmitTerminalHistoryEnqueue, TerminalAbsorbingModes,
           TerminalReadOnly, TerminalRetired
  <1>3. CASE AdmitTerminalHistoryCoalesce
    BY <1>3, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           AdmitTerminalHistoryCoalesce, TerminalAbsorbingModes,
           TerminalReadOnly, TerminalRetired
  <1>4. CASE RejectTerminalAdmission
    BY <1>4, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           RejectTerminalAdmission, TerminalAbsorbingModes,
           terminalIngressVars, TerminalReadOnly, TerminalRetired
  <1>5. CASE DequeueTerminalHistory
    BY <1>5, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           DequeueTerminalHistory, TerminalAbsorbingModes,
           TerminalReadOnly, TerminalRetired
  <1>6. CASE TerminalControlNoOp
    BY <1>6, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           TerminalControlNoOp, TerminalAbsorbingModes,
           terminalIngressVars, TerminalReadOnly, TerminalRetired
  <1>7. CASE ExitTerminalHistoryService
    BY <1>7, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           ExitTerminalHistoryService, TerminalAbsorbingModes,
           TerminalReadOnly, TerminalRetired
  <1>8. CASE IdempotentTerminalRetire
    BY <1>8, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           IdempotentTerminalRetire, TerminalAbsorbingModes,
           terminalIngressVars, TerminalReadOnly, TerminalRetired
  <1>9. CASE TerminalIngressStutter
    BY <1>9, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           TerminalIngressStutter, TerminalAbsorbingModes,
           terminalIngressVars, TerminalReadOnly, TerminalRetired
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
       <1>8, <1>9 DEF TerminalIngressLifecycleNext

THEOREM TerminalIngressLifecycleStepEstablishesAbsorbencyStepProperties ==
  TerminalIngressAbsorbencyInvariant
    /\ [TerminalIngressLifecycleNext]_terminalIngressVars
    => TerminalIngressAbsorbencyStepProperties
PROOF
  <1>1. CASE TerminalIngressLifecycleNext
    BY <1>1, TerminalIngressLifecycleNextEstablishesAbsorbencyStepProperties
  <1>2. CASE UNCHANGED terminalIngressVars
    BY <1>2, SMTT(30)
       DEF TerminalIngressAbsorbencyInvariant,
           TerminalIngressAbsorbencyStepProperties,
           TerminalModeAbsorbingStep, TerminalRetiredAbsorbingStep,
           EveryServiceOwnerExitRetiresStep, NoPostOwnerAdmissionStep,
           TerminalAbsorbingModes, terminalIngressVars,
           TerminalReadOnly, TerminalRetired
  <1> QED BY <1>1, <1>2

THEOREM TerminalIngressLifecycleSpecAlwaysTerminalModeAbsorbingStep ==
  TerminalIngressLifecycleSpec
    => [][TerminalModeAbsorbingStep]_terminalIngressVars
PROOF
  <1>1. TerminalIngressLifecycleSpec
           => []TerminalIngressAbsorbencyInvariant
    BY TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant
  <1>2. TerminalIngressAbsorbencyInvariant
           /\ [TerminalIngressLifecycleNext]_terminalIngressVars
           => [TerminalModeAbsorbingStep]_terminalIngressVars
    BY TerminalIngressLifecycleStepEstablishesAbsorbencyStepProperties
       DEF TerminalIngressAbsorbencyStepProperties
  <1> QED BY <1>1, <1>2, PTL DEF TerminalIngressLifecycleSpec

THEOREM TerminalIngressLifecycleSpecAlwaysTerminalRetiredAbsorbingStep ==
  TerminalIngressLifecycleSpec
    => [][TerminalRetiredAbsorbingStep]_terminalIngressVars
PROOF
  <1>1. TerminalIngressLifecycleSpec
           => []TerminalIngressAbsorbencyInvariant
    BY TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant
  <1>2. TerminalIngressAbsorbencyInvariant
           /\ [TerminalIngressLifecycleNext]_terminalIngressVars
           => [TerminalRetiredAbsorbingStep]_terminalIngressVars
    BY TerminalIngressLifecycleStepEstablishesAbsorbencyStepProperties
       DEF TerminalIngressAbsorbencyStepProperties
  <1> QED BY <1>1, <1>2, PTL DEF TerminalIngressLifecycleSpec

THEOREM TerminalIngressLifecycleSpecAlwaysEveryServiceOwnerExitRetiresStep ==
  TerminalIngressLifecycleSpec
    => [][EveryServiceOwnerExitRetiresStep]_terminalIngressVars
PROOF
  <1>1. TerminalIngressLifecycleSpec
           => []TerminalIngressAbsorbencyInvariant
    BY TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant
  <1>2. TerminalIngressAbsorbencyInvariant
           /\ [TerminalIngressLifecycleNext]_terminalIngressVars
           => [EveryServiceOwnerExitRetiresStep]_terminalIngressVars
    BY TerminalIngressLifecycleStepEstablishesAbsorbencyStepProperties
       DEF TerminalIngressAbsorbencyStepProperties
  <1> QED BY <1>1, <1>2, PTL DEF TerminalIngressLifecycleSpec

THEOREM TerminalIngressLifecycleSpecAlwaysNoPostOwnerAdmissionStep ==
  TerminalIngressLifecycleSpec
    => [][NoPostOwnerAdmissionStep]_terminalIngressVars
PROOF
  <1>1. TerminalIngressLifecycleSpec
           => []TerminalIngressAbsorbencyInvariant
    BY TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant
  <1>2. TerminalIngressAbsorbencyInvariant
           /\ [TerminalIngressLifecycleNext]_terminalIngressVars
           => [NoPostOwnerAdmissionStep]_terminalIngressVars
    BY TerminalIngressLifecycleStepEstablishesAbsorbencyStepProperties
       DEF TerminalIngressAbsorbencyStepProperties
  <1> QED BY <1>1, <1>2, PTL DEF TerminalIngressLifecycleSpec

TerminalIngressProcessLifetimeAbsorbencyProperty ==
  /\ []TerminalIngressAbsorbencyInvariant
  /\ [][TerminalModeAbsorbingStep]_terminalIngressVars
  /\ [][TerminalRetiredAbsorbingStep]_terminalIngressVars
  /\ [][EveryServiceOwnerExitRetiresStep]_terminalIngressVars
  /\ [][NoPostOwnerAdmissionStep]_terminalIngressVars

THEOREM TerminalIngressProcessLifetimeAbsorbencyObligation ==
  TerminalIngressLifecycleSpec
    => TerminalIngressProcessLifetimeAbsorbencyProperty
PROOF
  <1>1. TerminalIngressLifecycleSpec
           => []TerminalIngressAbsorbencyInvariant
    BY TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant
  <1>2. TerminalIngressLifecycleSpec
           => [][TerminalModeAbsorbingStep]_terminalIngressVars
    BY TerminalIngressLifecycleSpecAlwaysTerminalModeAbsorbingStep
  <1>3. TerminalIngressLifecycleSpec
           => [][TerminalRetiredAbsorbingStep]_terminalIngressVars
    BY TerminalIngressLifecycleSpecAlwaysTerminalRetiredAbsorbingStep
  <1>4. TerminalIngressLifecycleSpec
           => [][EveryServiceOwnerExitRetiresStep]_terminalIngressVars
    BY TerminalIngressLifecycleSpecAlwaysEveryServiceOwnerExitRetiresStep
  <1>5. TerminalIngressLifecycleSpec
           => [][NoPostOwnerAdmissionStep]_terminalIngressVars
    BY TerminalIngressLifecycleSpecAlwaysNoPostOwnerAdmissionStep
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5
       DEF TerminalIngressProcessLifetimeAbsorbencyProperty

=============================================================================
