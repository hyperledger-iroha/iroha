---- MODULE SumeragiV2ImportedCertificateTailMutation ----
EXTENDS Naturals

(***************************************************************************
Finite mutation for an authenticated CommitQC retained behind a TC install.

The immutable CommitQC is admitted for one height/context while the reducer
is Busy.  Installing the older TC advances the local view and generation
before the CommitQC gets its runner turn.  Production retags that exact
authenticated occurrence without changing its certificate/subject/lifecycle
identity.  The repaired model represents the same boundary by making the
imported BeginDecision tail height/context-stable.

The mutation reinstates the old strict consumer-incarnation check.  It drops
the sole admitted Decision owner after the TC transition while retaining the
CommitQC receipt, reproducing the replenishment-free lasso found in the full
model.
***************************************************************************)

CONSTANT ContextStableImportedTail

ASSUME ContextStableImportedTail \in BOOLEAN

VARIABLES phase, currentView, currentGeneration,
          commitReceipt, beginDecisionOwner, decision

vars ==
  <<phase, currentView, currentGeneration,
    commitReceipt, beginDecisionOwner, decision>>

TypeInvariant ==
  /\ phase \in {"Fresh", "Admitted", "TcInstalled", "Decided"}
  /\ currentView \in 0..1
  /\ currentGeneration \in 0..1
  /\ commitReceipt \in BOOLEAN
  /\ beginDecisionOwner \in BOOLEAN
  /\ decision \in BOOLEAN

ReceiptRetainsExactDecisionTail ==
  /\ phase = "TcInstalled"
  /\ commitReceipt
  /\ ~decision
  => beginDecisionOwner

Init ==
  /\ phase = "Fresh"
  /\ currentView = 0
  /\ currentGeneration = 0
  /\ ~commitReceipt
  /\ ~beginDecisionOwner
  /\ ~decision

AdmitCommitCertificate ==
  /\ phase = "Fresh"
  /\ phase' = "Admitted"
  /\ commitReceipt'
  /\ beginDecisionOwner'
  /\ ~decision'
  /\ UNCHANGED <<currentView, currentGeneration>>

InstallTimeoutCertificate ==
  /\ phase = "Admitted"
  /\ phase' = "TcInstalled"
  /\ currentView' = 1
  /\ currentGeneration' = 1
  /\ commitReceipt' = commitReceipt
  /\ beginDecisionOwner' =
       IF ContextStableImportedTail THEN beginDecisionOwner ELSE FALSE
  /\ decision' = decision

ServiceImportedDecisionTail ==
  /\ phase = "TcInstalled"
  /\ beginDecisionOwner
  /\ phase' = "Decided"
  /\ decision'
  /\ ~beginDecisionOwner'
  /\ UNCHANGED <<currentView, currentGeneration, commitReceipt>>

Next ==
  \/ AdmitCommitCertificate
  \/ InstallTimeoutCertificate
  \/ ServiceImportedDecisionTail

Spec ==
  Init
    /\ [][Next]_vars
    /\ WF_vars(ServiceImportedDecisionTail)

ReceiptEventuallyDecides == commitReceipt ~> decision

=============================================================================
