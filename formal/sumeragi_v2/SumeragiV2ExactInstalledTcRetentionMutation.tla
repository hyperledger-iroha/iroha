---- MODULE SumeragiV2ExactInstalledTcRetentionMutation ----
EXTENDS Naturals

(***************************************************************************
Compact mutation kernel for exact retained TimeoutCertificate authority.

Both certificates occupy the same source/control-class view but carry
different authenticated evidence.  The repaired install removes the old
class owner before remembering the newly durable TC.  The retired generic
view-only replacement leaves the old evidence retained while advancing the
durable last-installed record.  Its broad view frontier therefore still
holds; only the exact-authority invariant identifies the defect.
***************************************************************************)

CONSTANT Mode

OldTc == [view |-> 3, evidence |-> "old-quorum"]
InstalledTc == [view |-> 3, evidence |-> "upgraded-quorum"]

VARIABLES lastInstalledTc, retainedTc, phase

vars == <<lastInstalledTc, retainedTc, phase>>

Init ==
  /\ lastInstalledTc = OldTc
  /\ retainedTc = OldTc
  /\ phase = "PendingInstall"

RemoveClassThenRememberExact ==
  /\ phase = "PendingInstall"
  /\ lastInstalledTc' = InstalledTc
  /\ retainedTc' = InstalledTc
  /\ phase' = "Installed"

ViewOnlyRememberSameView ==
  /\ phase = "PendingInstall"
  /\ lastInstalledTc' = InstalledTc
  /\ retainedTc' =
       IF InstalledTc.view > retainedTc.view
            \/ InstalledTc = retainedTc
       THEN InstalledTc
       ELSE retainedTc
  /\ phase' = "Installed"

SelectedInstall ==
  IF Mode = "RemoveThenRemember"
  THEN RemoveClassThenRememberExact
  ELSE ViewOnlyRememberSameView

Next == SelectedInstall

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SelectedInstall)

TypeInvariant ==
  /\ Mode \in {"RemoveThenRemember", "ViewOnlyRemember"}
  /\ lastInstalledTc \in {OldTc, InstalledTc}
  /\ retainedTc \in {OldTc, InstalledTc}
  /\ phase \in {"PendingInstall", "Installed"}

RetainedViewFrontier ==
  retainedTc.view = lastInstalledTc.view

ExactInstalledTcAuthority ==
  phase = "Installed" => retainedTc = lastInstalledTc

InstallCompletes ==
  <> (phase = "Installed")

=============================================================================
