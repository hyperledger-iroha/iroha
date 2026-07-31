---- MODULE SumeragiV2PersistInstallTimeoutTagMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite-state regression for the PersistInstallTC clock-reset transaction.

A timeout may expire while an older PersistInstallTC candidate is waiting for
the serialized runner.  DirectTimeoutStep then retains TimeoutElapsed because
the timeout lifecycle cannot be admitted ahead of that older candidate.  A
successful install moves the node to the certified next view and resets its
deadline and emission flag.  The retained timeout tag belongs to the old
view, so that same transaction must clear it atomically.  Otherwise the next
DeferredTimeoutStep immediately consumes the stale tag in the new view and
the newly adequate service window is never exposed.

The repaired mode clears the tag in the install transaction.  The mutation
keeps it.  This bounded pair checks the exact action boundary and its liveness
consequence; it is regression evidence, not deductive proof of AsyncNetwork.
***************************************************************************)

CONSTANT InstallTagMode

InstallTagModes == {"ClearOnInstall", "RetainOnInstall"}

AdequateTimeout == 8
ServiceBudget == 4

VARIABLES stage, view, now, deadline, timeoutElapsed,
          timeoutEmitted, freshWindowEntered, deferredTimeoutStarted,
          lastTransition

vars ==
  <<stage, view, now, deadline, timeoutElapsed,
    timeoutEmitted, freshWindowEntered, deferredTimeoutStarted,
    lastTransition>>

Init ==
  /\ InstallTagMode \in InstallTagModes
  /\ stage = "PersistInstallTC"
  /\ view = 0
  /\ now = 4
  /\ deadline = 4
  /\ timeoutElapsed
  /\ ~timeoutEmitted
  /\ ~freshWindowEntered
  /\ ~deferredTimeoutStarted
  /\ lastTransition = "Init"

PersistInstallTC ==
  /\ stage = "PersistInstallTC"
  /\ stage' = "Installed"
  /\ view' = view + 1
  /\ deadline' = now + AdequateTimeout
  /\ timeoutElapsed' = (InstallTagMode = "RetainOnInstall")
  /\ timeoutEmitted' = FALSE
  /\ lastTransition' = "PersistInstallTC"
  /\ UNCHANGED <<now, freshWindowEntered, deferredTimeoutStarted>>

EnterFreshWindow ==
  /\ stage = "Installed"
  /\ ~timeoutElapsed
  /\ ~timeoutEmitted
  /\ now + ServiceBudget < deadline
  /\ stage' = "Fresh"
  /\ freshWindowEntered' = TRUE
  /\ lastTransition' = "EnterFreshWindow"
  /\ UNCHANGED
       <<view, now, deadline, timeoutElapsed,
         timeoutEmitted, deferredTimeoutStarted>>

DeferredTimeoutStep ==
  /\ stage = "Installed"
  /\ timeoutElapsed
  /\ ~timeoutEmitted
  /\ stage' = "TimeoutStarted"
  /\ timeoutElapsed' = FALSE
  /\ timeoutEmitted' = TRUE
  /\ deferredTimeoutStarted' = TRUE
  /\ lastTransition' = "DeferredTimeoutStep"
  /\ UNCHANGED <<view, now, deadline, freshWindowEntered>>

Next ==
  \/ PersistInstallTC
  \/ EnterFreshWindow
  \/ DeferredTimeoutStep

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(PersistInstallTC)
  /\ WF_vars(EnterFreshWindow)
  /\ WF_vars(DeferredTimeoutStep)

TypeInvariant ==
  /\ InstallTagMode \in InstallTagModes
  /\ stage \in
       {"PersistInstallTC", "Installed", "Fresh", "TimeoutStarted"}
  /\ view \in Nat
  /\ now \in Nat
  /\ deadline \in Nat
  /\ timeoutElapsed \in BOOLEAN
  /\ timeoutEmitted \in BOOLEAN
  /\ freshWindowEntered \in BOOLEAN
  /\ deferredTimeoutStarted \in BOOLEAN
  /\ lastTransition \in
       {"Init", "PersistInstallTC", "EnterFreshWindow",
        "DeferredTimeoutStep"}

SuccessfulInstallClearsOldViewTimeoutTag ==
  stage = "Installed" => ~timeoutElapsed

FreshWindowCannotConsumeStaleTimeoutTag ==
  freshWindowEntered => ~deferredTimeoutStarted

PersistInstallEventuallyExposesFreshWindow ==
  (stage = "PersistInstallTC") ~> freshWindowEntered

====
