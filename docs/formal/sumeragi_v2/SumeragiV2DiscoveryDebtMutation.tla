---- MODULE SumeragiV2DiscoveryDebtMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Minimal scheduler mutation for recurring commit-certificate discovery.  The
old runtime lets a response retire the active request just before every fair
RunNode occurrence, re-enabling discovery inside that same fair action ahead
of an already queued command.  The repaired model makes discovery a separate
auxiliary prefix, so taking it cannot satisfy weak fairness of RunNode.
***************************************************************************)

VARIABLES queueOwned, requestActive, fifoOwed

vars == <<queueOwned, requestActive, fifoOwed>>

Init ==
  /\ queueOwned = TRUE
  /\ requestActive = FALSE
  /\ fifoOwed = FALSE

AdmitResponse ==
  /\ requestActive
  /\ requestActive' = FALSE
  /\ UNCHANGED <<queueOwned, fifoOwed>>

OldRunNode ==
  IF ~requestActive
  THEN /\ requestActive' = TRUE
       /\ UNCHANGED <<queueOwned, fifoOwed>>
  ELSE /\ queueOwned' = FALSE
       /\ UNCHANGED <<requestActive, fifoOwed>>

FixedDiscoveryPrefix ==
  /\ ~requestActive
  /\ requestActive' = TRUE
  /\ UNCHANGED <<queueOwned, fifoOwed>>

FixedRunNode ==
  /\ queueOwned
  /\ queueOwned' = FALSE
  /\ UNCHANGED <<requestActive, fifoOwed>>

OldNext == OldRunNode \/ AdmitResponse

FixedNext == FixedRunNode \/ FixedDiscoveryPrefix \/ AdmitResponse

OldSpec ==
  /\ Init
  /\ [][OldNext]_vars
  /\ WF_vars(OldRunNode)
  /\ WF_vars(AdmitResponse)

FixedSpec ==
  /\ Init
  /\ [][FixedNext]_vars
  /\ WF_vars(FixedRunNode)
  /\ WF_vars(FixedDiscoveryPrefix)
  /\ WF_vars(AdmitResponse)

QueueEventuallyServiced == queueOwned ~> ~queueOwned

=============================================================================
