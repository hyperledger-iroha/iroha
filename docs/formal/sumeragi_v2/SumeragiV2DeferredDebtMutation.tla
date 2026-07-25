---- MODULE SumeragiV2DeferredDebtMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded witness for deferred-drain debt creation.  A command rejected while
the serialized reducer is Busy is moved from the ordinary scheduler into a
deferred queue.  If that admission inherits a stale false debt bit, completing
the Busy owner leaves a nonempty queue that the runtime cannot select.

The repaired admission arms debt in the same transition that creates the
deferred owner.  Ordinary Busy completion then opens the serviceability fence,
and the fair deferred drain retires the owner.
***************************************************************************)

VARIABLES busy, deferredOwned, drainOwed, phase

vars == <<busy, deferredOwned, drainOwed, phase>>

Init ==
  /\ busy = TRUE
  /\ deferredOwned = FALSE
  /\ drainOwed = FALSE
  /\ phase = "Admit"

InheritedDebtDefer ==
  /\ phase = "Admit"
  /\ busy = TRUE
  /\ deferredOwned = FALSE
  /\ deferredOwned' = TRUE
  /\ UNCHANGED <<busy, drainOwed>>
  /\ phase' = "CompleteBusy"

ArmedDebtDefer ==
  /\ phase = "Admit"
  /\ busy = TRUE
  /\ deferredOwned = FALSE
  /\ deferredOwned' = TRUE
  /\ drainOwed' = TRUE
  /\ UNCHANGED busy
  /\ phase' = "CompleteBusy"

CompleteBusyOwner ==
  /\ phase = "CompleteBusy"
  /\ busy = TRUE
  /\ busy' = FALSE
  /\ UNCHANGED <<deferredOwned, drainOwed>>
  /\ phase' = "Drain"

DrainDeferredOwner ==
  /\ phase = "Drain"
  /\ busy = FALSE
  /\ deferredOwned = TRUE
  /\ drainOwed = TRUE
  /\ deferredOwned' = FALSE
  /\ UNCHANGED <<busy, drainOwed>>
  /\ phase' = "Idle"

InheritedDebtNext ==
  InheritedDebtDefer \/ CompleteBusyOwner \/ DrainDeferredOwner

ArmedDebtNext ==
  ArmedDebtDefer \/ CompleteBusyOwner \/ DrainDeferredOwner

InheritedDebtSpec ==
  /\ Init
  /\ [][InheritedDebtNext]_vars
  /\ WF_vars(InheritedDebtDefer)
  /\ WF_vars(CompleteBusyOwner)
  /\ WF_vars(DrainDeferredOwner)

ArmedDebtSpec ==
  /\ Init
  /\ [][ArmedDebtNext]_vars
  /\ WF_vars(ArmedDebtDefer)
  /\ WF_vars(CompleteBusyOwner)
  /\ WF_vars(DrainDeferredOwner)

DeferredDebtInvariant == deferredOwned = TRUE => drainOwed = TRUE

DeferredEventuallyServiced ==
  (deferredOwned = TRUE) ~> (deferredOwned = FALSE)

=============================================================================
