---- MODULE SumeragiV2PersistInstallTimeoutRootRetirementMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite-state regression for atomic timeout-root retirement at the successful
PersistInstallTC boundary.

The installed certificate retires every same-node/context/height timeout
lifecycle whose immutable causal-root view is no later than the certificate
view.  Retirement covers Runtime FIFO, Busy-deferred, causal, deferred-
handoff, executor I/O, outstanding-work, and both ready-completion carriers.
The durable per-height high-watermark then coalesces exact retransmission, so
draining an old physical occurrence cannot recreate it.

The boundary is deliberately not the resulting node view.  A future-view
timeout root is the authenticated current-view root during a strict
same-round TC upgrade and must survive for replay in the new generation.
Likewise, a non-timeout proposal successor survives even when it retains the
installed TC's causal origin.  The mutation leaves the Busy-deferred carrier
unfiltered.  This bounded pair is regression evidence, not deductive proof of
AsyncNetwork.
***************************************************************************)

CONSTANT RetirementMode

RetirementModes == {"AtomicAllCarriers", "RetainDeferredCarrier"}

TimeoutLifecycleKinds ==
  {"BeginTimeout", "PersistTimeout", "SignTimeout",
   "DeliverTimeout", "FormTC", "DeliverTC",
   "BeginInstallTC", "PersistInstallTC"}

InstalledView == 1

Candidate(identity, kind, originView) ==
  [identity |-> identity,
   node |-> "Leader",
   consumerContext |-> "Ctx",
   height |-> 7,
   kind |-> kind,
   causalOrigin |->
     [node |-> "Leader", context |-> "Ctx", height |-> 7,
      view |-> originView, phase |-> "BeginTimeout"]]

OldTimeoutRoot == Candidate("OldTimeout", "PersistTimeout", 0)
EqualTimeoutRoot == Candidate("EqualTimeout", "DeliverTC", InstalledView)
FutureTimeoutRoot ==
  Candidate("FutureTimeout", "DeliverTimeout", InstalledView + 1)
PostInstallSuccessor ==
  Candidate("PostInstallSuccessor", "AssembleBody", InstalledView)

CandidateSet ==
  {OldTimeoutRoot, EqualTimeoutRoot,
   FutureTimeoutRoot, PostInstallSuccessor}

InitialOwners == CandidateSet
RetiredRoots == {OldTimeoutRoot, EqualTimeoutRoot}

TimeoutRootRetiredByInstall(candidate) ==
  /\ candidate.node = "Leader"
  /\ candidate.node = candidate.causalOrigin.node
  /\ candidate.consumerContext = "Ctx"
  /\ candidate.height = 7
  /\ candidate.causalOrigin.context = "Ctx"
  /\ candidate.causalOrigin.height = 7
  /\ candidate.kind \in TimeoutLifecycleKinds
  /\ candidate.causalOrigin.view \in 0..InstalledView

OwnersAfterInstall(owners) ==
  {candidate \in owners: ~TimeoutRootRetiredByInstall(candidate)}

VARIABLES stage, installed, timeoutRetiredThroughView,
          runtimeOwners, deferredOwners, causalOwners, handoffOwners,
          ioOwners, outstandingOwners, ioReadyOwners, localReadyOwners,
          lastTransition

vars ==
  <<stage, installed, timeoutRetiredThroughView,
    runtimeOwners, deferredOwners, causalOwners, handoffOwners,
    ioOwners, outstandingOwners, ioReadyOwners, localReadyOwners,
    lastTransition>>

PhysicalOwners ==
  runtimeOwners \cup deferredOwners \cup causalOwners \cup handoffOwners
    \cup ioOwners \cup outstandingOwners \cup ioReadyOwners
    \cup localReadyOwners

Init ==
  /\ RetirementMode \in RetirementModes
  /\ stage = "PersistInstallTC"
  /\ installed = FALSE
  /\ timeoutRetiredThroughView = 0
  /\ runtimeOwners = InitialOwners
  /\ deferredOwners = InitialOwners
  /\ causalOwners = InitialOwners
  /\ handoffOwners = InitialOwners
  /\ ioOwners = InitialOwners
  /\ outstandingOwners = InitialOwners
  /\ ioReadyOwners = InitialOwners
  /\ localReadyOwners = InitialOwners
  /\ lastTransition = "Init"

PersistInstallTC ==
  /\ stage = "PersistInstallTC"
  /\ stage' = "Installed"
  /\ installed' = TRUE
  /\ timeoutRetiredThroughView' = InstalledView
  /\ runtimeOwners' = OwnersAfterInstall(runtimeOwners)
  /\ deferredOwners' =
       IF RetirementMode = "AtomicAllCarriers"
       THEN OwnersAfterInstall(deferredOwners)
       ELSE deferredOwners
  /\ causalOwners' = OwnersAfterInstall(causalOwners)
  /\ handoffOwners' = OwnersAfterInstall(handoffOwners)
  /\ ioOwners' = OwnersAfterInstall(ioOwners)
  /\ outstandingOwners' = OwnersAfterInstall(outstandingOwners)
  /\ ioReadyOwners' = OwnersAfterInstall(ioReadyOwners)
  /\ localReadyOwners' = OwnersAfterInstall(localReadyOwners)
  /\ lastTransition' = "PersistInstallTC"

AdmissionCoalescedByInstall(candidate) ==
  /\ installed
  /\ candidate.kind \in TimeoutLifecycleKinds
  /\ candidate.causalOrigin.view <= timeoutRetiredThroughView

RetryRetiredExactRoots ==
  /\ stage = "Installed"
  /\ stage' = "Retried"
  /\ runtimeOwners' =
       runtimeOwners
         \cup {candidate \in RetiredRoots:
                 ~AdmissionCoalescedByInstall(candidate)}
  /\ lastTransition' = "RetryRetiredExactRoots"
  /\ UNCHANGED
       <<installed, timeoutRetiredThroughView,
         deferredOwners, causalOwners, handoffOwners, ioOwners,
         outstandingOwners, ioReadyOwners, localReadyOwners>>

SameHeightRestart ==
  /\ stage = "Retried"
  /\ stage' = "Restarted"
  /\ lastTransition' = "SameHeightRestart"
  /\ UNCHANGED
       <<installed, timeoutRetiredThroughView,
         runtimeOwners, deferredOwners, causalOwners, handoffOwners,
         ioOwners, outstandingOwners, ioReadyOwners, localReadyOwners>>

EnterFreshWindow ==
  /\ stage = "Restarted"
  /\ PhysicalOwners \cap RetiredRoots = {}
  /\ stage' = "Fresh"
  /\ lastTransition' = "EnterFreshWindow"
  /\ UNCHANGED
       <<installed, timeoutRetiredThroughView,
         runtimeOwners, deferredOwners, causalOwners, handoffOwners,
         ioOwners, outstandingOwners, ioReadyOwners, localReadyOwners>>

Next ==
  \/ PersistInstallTC
  \/ RetryRetiredExactRoots
  \/ SameHeightRestart
  \/ EnterFreshWindow

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(PersistInstallTC)
  /\ WF_vars(RetryRetiredExactRoots)
  /\ WF_vars(SameHeightRestart)
  /\ WF_vars(EnterFreshWindow)

TypeInvariant ==
  /\ RetirementMode \in RetirementModes
  /\ stage \in
       {"PersistInstallTC", "Installed", "Retried", "Restarted", "Fresh"}
  /\ installed \in BOOLEAN
  /\ timeoutRetiredThroughView \in Nat
  /\ runtimeOwners \subseteq CandidateSet
  /\ deferredOwners \subseteq CandidateSet
  /\ causalOwners \subseteq CandidateSet
  /\ handoffOwners \subseteq CandidateSet
  /\ ioOwners \subseteq CandidateSet
  /\ outstandingOwners \subseteq CandidateSet
  /\ ioReadyOwners \subseteq CandidateSet
  /\ localReadyOwners \subseteq CandidateSet
  /\ lastTransition \in
       {"Init", "PersistInstallTC", "RetryRetiredExactRoots",
        "SameHeightRestart", "EnterFreshWindow"}

InstallRetiresOlderOrEqualTimeoutRoots ==
  stage # "PersistInstallTC" => PhysicalOwners \cap RetiredRoots = {}

FutureAndPostInstallOwnersSurvive ==
  stage # "PersistInstallTC"
    => {FutureTimeoutRoot, PostInstallSuccessor} \subseteq
         runtimeOwners \cap deferredOwners \cap causalOwners
           \cap handoffOwners \cap ioOwners \cap outstandingOwners
           \cap ioReadyOwners \cap localReadyOwners

StrictSameRoundCurrentViewRootSurvives ==
  stage # "PersistInstallTC" => FutureTimeoutRoot \in PhysicalOwners

RetiredTimeoutRootCannotResurrect ==
  stage \in {"Retried", "Restarted", "Fresh"}
    => PhysicalOwners \cap RetiredRoots = {}

EqualViewRetryCoalescesBehindHighWatermark ==
  installed => AdmissionCoalescedByInstall(EqualTimeoutRoot)

SameHeightRestartPreservesRetirementHighWatermark ==
  stage \in {"Restarted", "Fresh"}
    => timeoutRetiredThroughView = InstalledView

PersistInstallEventuallyExposesFreshWindow ==
  (stage = "PersistInstallTC") ~> (stage = "Fresh")

====
