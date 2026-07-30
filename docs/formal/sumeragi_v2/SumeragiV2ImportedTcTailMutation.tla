---- MODULE SumeragiV2ImportedTcTailMutation ----
EXTENDS Naturals

(***************************************************************************
Finite mutation for an authenticated future-round TC retained while a
same-round installed-TC upgrade advances only the reducer generation.

The target TC remains admissible at view 1 and must retain its exact
BeginInstallTC/PersistInstallTC owner.  The old strict consumer-generation
check discarded that owner even though neither height/context nor the target
view changed.  The repaired model makes only this authenticated imported tail
incarnation-neutral; unrelated local work remains generation-scoped.
***************************************************************************)

CONSTANT ContextStableImportedTail

ASSUME ContextStableImportedTail \in BOOLEAN

VARIABLES phase, currentView, currentGeneration,
          tcReceipt, installTailOwner, targetInstalled

vars ==
  <<phase, currentView, currentGeneration,
    tcReceipt, installTailOwner, targetInstalled>>

TypeInvariant ==
  /\ phase \in {"Fresh", "Admitted", "Upgraded", "Installed"}
  /\ currentView \in 1..2
  /\ currentGeneration \in 0..1
  /\ tcReceipt \in BOOLEAN
  /\ installTailOwner \in BOOLEAN
  /\ targetInstalled \in BOOLEAN

TcReceiptRetainsExactInstallTail ==
  /\ phase = "Upgraded"
  /\ tcReceipt
  /\ ~targetInstalled
  => installTailOwner

Init ==
  /\ phase = "Fresh"
  /\ currentView = 1
  /\ currentGeneration = 0
  /\ ~tcReceipt
  /\ ~installTailOwner
  /\ ~targetInstalled

AdmitTargetTimeoutCertificate ==
  /\ phase = "Fresh"
  /\ phase' = "Admitted"
  /\ tcReceipt'
  /\ installTailOwner'
  /\ ~targetInstalled'
  /\ UNCHANGED <<currentView, currentGeneration>>

InstallCompetingSameRoundUpgrade ==
  /\ phase = "Admitted"
  /\ phase' = "Upgraded"
  /\ currentView' = currentView
  /\ currentGeneration' = 1
  /\ tcReceipt' = tcReceipt
  /\ installTailOwner' =
       IF ContextStableImportedTail THEN installTailOwner ELSE FALSE
  /\ targetInstalled' = targetInstalled

ServiceImportedInstallTail ==
  /\ phase = "Upgraded"
  /\ installTailOwner
  /\ phase' = "Installed"
  /\ currentView' = 2
  /\ targetInstalled'
  /\ ~installTailOwner'
  /\ UNCHANGED <<currentGeneration, tcReceipt>>

Next ==
  \/ AdmitTargetTimeoutCertificate
  \/ InstallCompetingSameRoundUpgrade
  \/ ServiceImportedInstallTail

Spec ==
  Init
    /\ [][Next]_vars
    /\ WF_vars(ServiceImportedInstallTail)

TcReceiptEventuallyInstalls == tcReceipt ~> targetInstalled

=============================================================================
