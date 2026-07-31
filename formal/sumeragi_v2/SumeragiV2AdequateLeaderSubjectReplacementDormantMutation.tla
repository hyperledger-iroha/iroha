---- MODULE SumeragiV2AdequateLeaderSubjectReplacementDormantMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Bounded mutation witness for split lifecycle and physical ingress ordinals.

The retained logical leader-wire token keeps lifecycle ordinal 1 across a
dormant replay.  A Serve carrier has already been admitted at physical
ordinal 2.  The repaired transition reserves physical ordinal 3 before it
publishes the replay carrier, so the retained lifecycle identity cannot move
ahead of already-admitted Serve work.

The mutation incorrectly reuses lifecycle ordinal 1 as the physical carrier
position.  Admission then crosses the frozen physical cut and violates
`DormantReplayCannotCrossPhysicalCut` immediately.  Retry transport still
coalesces into one logical lifecycle in both models; the distinguishing fact
is exclusively ownership of a fresh physical ingress position.
***************************************************************************)

CONSTANT ReserveFreshPhysicalOrdinal

ASSUME ReserveFreshPhysicalOrdinal \in BOOLEAN

RetainedLifecycleOrdinal == 1
AnchorPhysicalOrdinal == 2

VARIABLES
  phase,
  packetPresent,
  dormant,
  physicalHighWatermark,
  replayPhysicalOrdinal,
  replayCarrierActive,
  anchorServiced

vars ==
  <<phase, packetPresent, dormant, physicalHighWatermark,
    replayPhysicalOrdinal, replayCarrierActive, anchorServiced>>

TypeInvariant ==
  /\ phase
       \in {"Dormant", "RetryReady", "Reactivated",
            "ReplayServiced", "AnchorServiced"}
  /\ packetPresent \in BOOLEAN
  /\ dormant \in BOOLEAN
  /\ physicalHighWatermark \in Nat
  /\ replayPhysicalOrdinal \in Nat
  /\ replayCarrierActive \in BOOLEAN
  /\ anchorServiced \in BOOLEAN

NoCarrierPublishedBeforeAtomicAdmission ==
  phase \in {"Dormant", "RetryReady"}
    => /\ replayPhysicalOrdinal = 0
       /\ ~replayCarrierActive

DormantReplayCannotCrossPhysicalCut ==
  replayCarrierActive
    => AnchorPhysicalOrdinal < replayPhysicalOrdinal

FreshReservationAdvancesPhysicalHighWatermark ==
  phase \in {"Reactivated", "ReplayServiced", "AnchorServiced"}
    => physicalHighWatermark
         >= replayPhysicalOrdinal

AnchorIsPhysicalHead ==
  ~replayCarrierActive
    \/ AnchorPhysicalOrdinal < replayPhysicalOrdinal

Init ==
  /\ phase = "Dormant"
  /\ ~packetPresent
  /\ dormant
  /\ physicalHighWatermark = AnchorPhysicalOrdinal
  /\ replayPhysicalOrdinal = 0
  /\ ~replayCarrierActive
  /\ ~anchorServiced

EmitExactRetry ==
  /\ phase = "Dormant"
  /\ dormant
  /\ ~packetPresent
  /\ phase' = "RetryReady"
  /\ packetPresent'
  /\ UNCHANGED
       <<dormant, physicalHighWatermark, replayPhysicalOrdinal,
         replayCarrierActive, anchorServiced>>

AdmitExactDormantRetry ==
  /\ phase = "RetryReady"
  /\ dormant
  /\ packetPresent
  /\ phase' = "Reactivated"
  /\ ~packetPresent'
  /\ ~dormant'
  /\ replayPhysicalOrdinal' =
       IF ReserveFreshPhysicalOrdinal
       THEN physicalHighWatermark + 1
       ELSE RetainedLifecycleOrdinal
  /\ physicalHighWatermark' =
       IF ReserveFreshPhysicalOrdinal
       THEN physicalHighWatermark + 1
       ELSE physicalHighWatermark
  /\ replayCarrierActive'
  /\ UNCHANGED anchorServiced

ServiceReplayAheadOfAnchor ==
  /\ phase = "Reactivated"
  /\ replayCarrierActive
  /\ replayPhysicalOrdinal < AnchorPhysicalOrdinal
  /\ phase' = "ReplayServiced"
  /\ ~replayCarrierActive'
  /\ UNCHANGED
       <<packetPresent, dormant, physicalHighWatermark,
         replayPhysicalOrdinal, anchorServiced>>

ServiceAnchor ==
  /\ phase \in {"Reactivated", "ReplayServiced"}
  /\ ~anchorServiced
  /\ AnchorIsPhysicalHead
  /\ phase' = "AnchorServiced"
  /\ anchorServiced'
  /\ UNCHANGED
       <<packetPresent, dormant, physicalHighWatermark,
         replayPhysicalOrdinal, replayCarrierActive>>

Next ==
  \/ EmitExactRetry
  \/ AdmitExactDormantRetry
  \/ ServiceReplayAheadOfAnchor
  \/ ServiceAnchor

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(EmitExactRetry)
  /\ WF_vars(AdmitExactDormantRetry)
  /\ WF_vars(ServiceReplayAheadOfAnchor)
  /\ WF_vars(ServiceAnchor)

AnchorEventuallyServiced ==
  <>anchorServiced

=============================================================================
