---- MODULE SumeragiV2RestartTerminalDurabilityMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Finite mutation kernel for same-height restart durability.

A reducer service marker is process-local.  If it was the only evidence for
a Terminal producer continuation and Terminal leader-wire record, restart
must discard the continuation and reopen the exact wire identity with its
immutable ordinal.  A durable candidate-service tombstone is different: it
reconstructs the exact service identity, so both bounded Terminal records may
remain retired.  The mutation preserves every Terminal record regardless of
its evidence and therefore suppresses the volatile retransmission.
***************************************************************************)

CONSTANT RequireDurableTerminalPair

ASSUME RequireDurableTerminalPair \in BOOLEAN

WireIdentity == "context-0/height-0/view-1/subject-A/Proposal"
ServiceIdentity == "node-0/DeliverProposal/origin-A"
Ordinal == 1

WireStatuses == {"Pending", "Runtime", "Terminal"}

WireRecord(status) ==
  [identity |-> WireIdentity, ordinal |-> Ordinal, status |-> status]

ProducerTerminal ==
  [identity |-> ServiceIdentity, status |-> "Terminal"]

VARIABLES
  phase,
  wire,
  producerContinuations,
  transientMarker,
  durableServiceTombstone,
  packetOwned,
  candidateOwned

vars ==
  <<phase, wire, producerContinuations, transientMarker,
    durableServiceTombstone, packetOwned, candidateOwned>>

TypeInvariant ==
  /\ phase \in
       {"Fresh", "VolatileConsumed", "VolatileRestarted",
        "VolatileRetried", "DurableConsumed", "DurableRestarted"}
  /\ wire \in {WireRecord(status): status \in WireStatuses}
  /\ producerContinuations \subseteq {ProducerTerminal}
  /\ transientMarker \in BOOLEAN
  /\ durableServiceTombstone \in BOOLEAN
  /\ packetOwned \in BOOLEAN
  /\ candidateOwned \in BOOLEAN

VolatileTerminalReopensExactWireIdentity ==
  phase = "VolatileRestarted"
    => /\ producerContinuations = {}
       /\ wire = WireRecord("Pending")
       /\ wire.identity = WireIdentity
       /\ wire.ordinal = Ordinal
       /\ packetOwned
       /\ ~transientMarker
       /\ ~durableServiceTombstone

RetryAfterVolatileRestartUsesReservedOrdinal ==
  phase = "VolatileRetried"
    => /\ candidateOwned
       /\ wire.identity = WireIdentity
       /\ wire.ordinal = Ordinal

DurableTerminalPairSurvivesRestart ==
  phase = "DurableRestarted"
    => /\ producerContinuations = {ProducerTerminal}
       /\ wire = WireRecord("Terminal")
       /\ durableServiceTombstone
       /\ ~packetOwned
       /\ ~candidateOwned

Init ==
  /\ phase = "Fresh"
  /\ wire = WireRecord("Pending")
  /\ producerContinuations = {}
  /\ transientMarker = FALSE
  /\ durableServiceTombstone = FALSE
  /\ packetOwned = TRUE
  /\ candidateOwned = FALSE

ConsumeWithVolatileMarker ==
  /\ phase = "Fresh"
  /\ phase' = "VolatileConsumed"
  /\ wire' = WireRecord("Terminal")
  /\ producerContinuations' = {ProducerTerminal}
  /\ transientMarker' = TRUE
  /\ durableServiceTombstone' = FALSE
  /\ packetOwned' = FALSE
  /\ candidateOwned' = FALSE

RestartVolatileTerminal ==
  /\ phase = "VolatileConsumed"
  /\ phase' = "VolatileRestarted"
  /\ IF RequireDurableTerminalPair
     THEN /\ wire' = WireRecord("Pending")
          /\ producerContinuations' = {}
          /\ packetOwned' = TRUE
     ELSE /\ wire' = wire
          /\ producerContinuations' = producerContinuations
          /\ packetOwned' = FALSE
  /\ transientMarker' = FALSE
  /\ durableServiceTombstone' = FALSE
  /\ candidateOwned' = FALSE

RetryReopenedVolatileTerminal ==
  /\ phase = "VolatileRestarted"
  /\ phase' = "VolatileRetried"
  /\ candidateOwned' =
       (wire.status = "Pending" /\ packetOwned)
  /\ UNCHANGED
       <<wire, producerContinuations, transientMarker,
         durableServiceTombstone, packetOwned>>

ConsumeWithDurableTombstone ==
  /\ phase = "Fresh"
  /\ phase' = "DurableConsumed"
  /\ wire' = WireRecord("Terminal")
  /\ producerContinuations' = {ProducerTerminal}
  /\ transientMarker' = FALSE
  /\ durableServiceTombstone' = TRUE
  /\ packetOwned' = FALSE
  /\ candidateOwned' = FALSE

RestartDurableTerminal ==
  /\ phase = "DurableConsumed"
  /\ phase' = "DurableRestarted"
  /\ UNCHANGED
       <<wire, producerContinuations, durableServiceTombstone>>
  /\ transientMarker' = FALSE
  /\ packetOwned' = FALSE
  /\ candidateOwned' = FALSE

Next ==
  \/ ConsumeWithVolatileMarker
  \/ RestartVolatileTerminal
  \/ RetryReopenedVolatileTerminal
  \/ ConsumeWithDurableTombstone
  \/ RestartDurableTerminal

=============================================================================
