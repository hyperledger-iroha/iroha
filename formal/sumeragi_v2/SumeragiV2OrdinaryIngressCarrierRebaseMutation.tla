---- MODULE SumeragiV2OrdinaryIngressCarrierRebaseMutation ----
EXTENDS Naturals, FiniteSets, TLC

(***************************************************************************
Finite executable mutation kernel for ordinary ingress lifecycle ownership.

The older aggregate carrier is accepted first but drains after the newer
carrier.  Physical acceptance reserves immutable actor-global ordinals.
Draining the older compatible carrier must atomically rebase the Busy owner
to the minimum retained ordinal.  A branch which mutates any normalized owner
identity is rejected instead of partially committing the rebase.
***************************************************************************)

CONSTANTS RebaseToMinimum, RejectIdentityMutation

ASSUME RebaseToMinimum \in BOOLEAN
ASSUME RejectIdentityMutation \in BOOLEAN

NoOwner == [kind |-> "None"]

Carrier(name, ordinal, ownerIdentity, status) ==
  [name |-> name, ordinal |-> ordinal,
   ownerIdentity |-> ownerIdentity, status |-> status]

Owner(ordinal, ownerIdentity) ==
  [kind |-> "BusyOwner", ordinal |-> ordinal,
   ownerIdentity |-> ownerIdentity]

VARIABLES phase, nextOrdinal, carriers, owner

vars == <<phase, nextOrdinal, carriers, owner>>

CarrierNamed(name) ==
  CHOOSE carrier \in carriers: carrier.name = name

CarrierOrdinals == {carrier.ordinal: carrier \in carriers}

MinimumCarrierOrdinal ==
  CHOOSE ordinal \in CarrierOrdinals:
    \A other \in CarrierOrdinals: ordinal <= other

ReplaceCarrierStatus(name, status) ==
  {IF carrier.name = name
   THEN [carrier EXCEPT !.status = status]
   ELSE carrier:
     carrier \in carriers}

Init ==
  /\ phase = "AcceptOlder"
  /\ nextOrdinal = 1
  /\ carriers = {}
  /\ owner = NoOwner

AcceptOlder ==
  /\ phase = "AcceptOlder"
  /\ carriers' =
       {Carrier("Older", nextOrdinal, "StableOwner", "Ingress")}
  /\ nextOrdinal' = nextOrdinal + 1
  /\ phase' = "AcceptNewer"
  /\ UNCHANGED owner

AcceptNewer ==
  /\ phase = "AcceptNewer"
  /\ carriers' =
       carriers
         \cup {Carrier(
                 "Newer", nextOrdinal, "StableOwner", "Ingress")}
  /\ nextOrdinal' = nextOrdinal + 1
  /\ phase' = "DrainNewer"
  /\ UNCHANGED owner

DrainNewer ==
  /\ phase = "DrainNewer"
  /\ carriers' = ReplaceCarrierStatus("Newer", "Deferred")
  /\ owner' = Owner((CarrierNamed("Newer")).ordinal, "StableOwner")
  /\ phase' = "DrainOlder"
  /\ UNCHANGED nextOrdinal

DrainOlderCompatible ==
  /\ phase = "DrainOlder"
  /\ carriers' = ReplaceCarrierStatus("Older", "Deferred")
  /\ owner' =
       IF RebaseToMinimum
       THEN Owner((CarrierNamed("Older")).ordinal, "StableOwner")
       ELSE owner
  /\ phase' = "Rebased"
  /\ UNCHANGED nextOrdinal

DrainOlderWithIdentityMutation ==
  /\ phase = "DrainOlder"
  /\ IF RejectIdentityMutation
     THEN /\ UNCHANGED <<carriers, owner>>
          /\ phase' = "RejectedMutation"
     ELSE /\ carriers' =
               ReplaceCarrierStatus("Older", "Deferred")
          /\ owner' =
               Owner((CarrierNamed("Older")).ordinal, "MutatedOwner")
          /\ phase' = "MutationCommitted"
  /\ UNCHANGED nextOrdinal

Next ==
  \/ AcceptOlder
  \/ AcceptNewer
  \/ DrainNewer
  \/ DrainOlderCompatible
  \/ DrainOlderWithIdentityMutation

Spec == Init /\ [][Next]_vars

AcceptedCarrierOrdinalsAreImmutableAndUnique ==
  /\ Cardinality(CarrierOrdinals) = Cardinality(carriers)
  /\ \A carrier \in carriers:
       /\ carrier.ordinal \in 1..(nextOrdinal - 1)
       /\ carrier.ownerIdentity
            \in {"StableOwner", "MutatedOwner"}

LaterAcceptedCarrierCannotOvertake ==
  /\ (phase \notin {"AcceptOlder", "AcceptNewer"}
        => (CarrierNamed("Older")).ordinal
             < (CarrierNamed("Newer")).ordinal)
  /\ nextOrdinal \in Nat \ {0}

BusyDeferredOwnerUsesMinimumCompatibleCarrier ==
  phase = "Rebased"
    => /\ owner.ownerIdentity = "StableOwner"
       /\ owner.ordinal = MinimumCarrierOrdinal

IdentityMutationCannotCommit ==
  phase # "MutationCommitted"

RejectedIdentityMutationPreservesCompleteOwnershipState ==
  phase = "RejectedMutation"
    => /\ carriers =
            {Carrier("Older", 1, "StableOwner", "Ingress"),
             Carrier("Newer", 2, "StableOwner", "Deferred")}
       /\ owner = Owner(2, "StableOwner")
       /\ nextOrdinal = 3

=============================================================================
