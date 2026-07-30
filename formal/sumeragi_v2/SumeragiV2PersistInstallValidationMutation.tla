---- MODULE SumeragiV2PersistInstallValidationMutation ----
EXTENDS Naturals

(***************************************************************************
Compact mutation kernel for the volatile-validation generation boundary.

The repaired PersistInstallTC projection clears every validation receipt
owned by the installing reducer when it advances that reducer's generation.
The retired behavior advances the generation but retains those receipts,
leaving an orphan which can no longer correspond to production `body_work`.
The other reducer's receipt demonstrates that the repair is node-local.
***************************************************************************)

CONSTANT Mode

Nodes == {"installing", "other"}
Generations == 0..1
Subjects == {"block"}

ValidationRecord(node, generationValue) ==
  [node |-> node, generation |-> generationValue, subject |-> "block"]

ValidationRecordSet ==
  [node: Nodes, generation: Generations, subject: Subjects]

InstallingValidation == ValidationRecord("installing", 0)
OtherValidation == ValidationRecord("other", 0)

VARIABLES generation, validatedBodies, pendingInstall

vars == <<generation, validatedBodies, pendingInstall>>

Init ==
  /\ generation = [node \in Nodes |-> 0]
  /\ validatedBodies = {InstallingValidation, OtherValidation}
  /\ pendingInstall = TRUE

RepairedPersistInstall ==
  /\ pendingInstall
  /\ generation' = [generation EXCEPT !["installing"] = 1]
  /\ validatedBodies' =
       {validation \in validatedBodies:
          validation.node # "installing"}
  /\ pendingInstall' = FALSE

RetainStaleValidationPersistInstall ==
  /\ pendingInstall
  /\ generation' = [generation EXCEPT !["installing"] = 1]
  /\ validatedBodies' = validatedBodies
  /\ pendingInstall' = FALSE

SelectedPersistInstall ==
  IF Mode = "Repaired"
  THEN RepairedPersistInstall
  ELSE RetainStaleValidationPersistInstall

Next == SelectedPersistInstall

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SelectedPersistInstall)

TypeInvariant ==
  /\ Mode \in {"Repaired", "RetainStaleValidation"}
  /\ generation \in [Nodes -> Generations]
  /\ validatedBodies \subseteq ValidationRecordSet
  /\ pendingInstall \in BOOLEAN

NoOrphanedValidationReceipt ==
  \A validation \in validatedBodies:
    validation.generation = generation[validation.node]

InstallingValidationClearedAfterInstall ==
  ~pendingInstall =>
    ~\E validation \in validatedBodies:
       validation.node = "installing"

OtherNodeValidationIsPreserved ==
  OtherValidation \in validatedBodies

InstallCompletes == <>~pendingInstall

=============================================================================
