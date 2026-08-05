---- MODULE SumeragiV2ExactResponseClaimLifecycleMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Finite mutation for the exact certified-response waiter lifecycle.

The production waiter family is the signed request hash.  The first valid
authenticated occurrence receives one recipient-local admission ordinal.
An exact retry coalesces into that occurrence, a different responder for the
same family cannot acquire a second charge, and consumption retires the whole
family before a delayed retry can arrive.

Same-height recovery is deliberately different from consumption: it retains
the durable outstanding request and the ordinal high-watermark, retires the
volatile live response occurrence, and lets the family reopen with a fresh
ordinal.  The four Boolean constants isolate those four repair edges.
***************************************************************************)

CONSTANTS
  CoalesceExactDuplicate,
  EnforceSingleFamilyCharge,
  RetireConsumedFamily,
  ReopenDurableFamilyAfterRestart

ASSUME
  /\ CoalesceExactDuplicate \in BOOLEAN
  /\ EnforceSingleFamilyCharge \in BOOLEAN
  /\ RetireConsumedFamily \in BOOLEAN
  /\ ReopenDurableFamilyAfterRestart \in BOOLEAN

ClaimIdentities == {"response-a", "response-a-duplicate", "response-b",
                    "response-after-restart"}

VARIABLES
  phase,
  activeRequest,
  durableRequest,
  claimIdentities,
  claimOrdinals,
  nextOrdinal,
  familyConsumed

vars ==
  <<phase, activeRequest, durableRequest, claimIdentities,
    claimOrdinals, nextOrdinal, familyConsumed>>

TypeInvariant ==
  /\ phase \in
       {"Awaiting", "Claimed", "Duplicated", "Contended",
        "Consumed", "LateRetried", "Restarted", "Reopened"}
  /\ activeRequest \in BOOLEAN
  /\ durableRequest \in BOOLEAN
  /\ claimIdentities \subseteq ClaimIdentities
  /\ claimOrdinals \subseteq 1..2
  /\ nextOrdinal \in 1..3
  /\ familyConsumed \in BOOLEAN

ClaimUsesOutstandingRequestCharge ==
  claimIdentities # {} => activeRequest

OneLogicalChargePerWaiterFamily ==
  Cardinality(claimIdentities) <= 1

ExactDuplicateKeepsFirstOrdinal ==
  phase = "Duplicated"
    => /\ claimIdentities = {"response-a"}
       /\ claimOrdinals = {1}
       /\ nextOrdinal = 2

CompetingResponderCannotReplaceOrDoubleCharge ==
  phase = "Contended"
    => /\ claimIdentities = {"response-a"}
       /\ claimOrdinals = {1}
       /\ nextOrdinal = 2

ConsumedFamilyCannotResurrect ==
  phase \in {"Consumed", "LateRetried"}
    => /\ familyConsumed
       /\ ~activeRequest
       /\ claimIdentities = {}
       /\ claimOrdinals = {}

SameHeightRestartReopensDurableFamily ==
  phase = "Restarted"
    => /\ activeRequest
       /\ durableRequest
       /\ ~familyConsumed
       /\ claimIdentities = {}
       /\ claimOrdinals = {}
       /\ nextOrdinal = 2

ReopenedFamilyUsesFreshOrdinal ==
  phase = "Reopened"
    => /\ activeRequest
       /\ claimIdentities = {"response-after-restart"}
       /\ claimOrdinals = {2}
       /\ nextOrdinal = 3

Init ==
  /\ phase = "Awaiting"
  /\ activeRequest = TRUE
  /\ durableRequest = TRUE
  /\ claimIdentities = {}
  /\ claimOrdinals = {}
  /\ nextOrdinal = 1
  /\ familyConsumed = FALSE

AdmitFirstResponse ==
  /\ phase = "Awaiting"
  /\ phase' = "Claimed"
  /\ claimIdentities' = {"response-a"}
  /\ claimOrdinals' = {1}
  /\ nextOrdinal' = 2
  /\ UNCHANGED <<activeRequest, durableRequest, familyConsumed>>

ReceiveExactDuplicate ==
  /\ phase = "Claimed"
  /\ phase' = "Duplicated"
  /\ claimIdentities' =
       IF CoalesceExactDuplicate
       THEN claimIdentities
       ELSE claimIdentities \cup {"response-a-duplicate"}
  /\ claimOrdinals' =
       IF CoalesceExactDuplicate
       THEN claimOrdinals
       ELSE claimOrdinals \cup {nextOrdinal}
  /\ nextOrdinal' =
       IF CoalesceExactDuplicate THEN nextOrdinal ELSE nextOrdinal + 1
  /\ UNCHANGED <<activeRequest, durableRequest, familyConsumed>>

ReceiveCompetingResponder ==
  /\ phase \in {"Claimed", "Duplicated"}
  /\ phase' = "Contended"
  /\ claimIdentities' =
       IF EnforceSingleFamilyCharge
       THEN claimIdentities
       ELSE claimIdentities \cup {"response-b"}
  /\ claimOrdinals' =
       IF EnforceSingleFamilyCharge
       THEN claimOrdinals
       ELSE claimOrdinals \cup {nextOrdinal}
  /\ nextOrdinal' =
       IF EnforceSingleFamilyCharge THEN nextOrdinal ELSE nextOrdinal + 1
  /\ UNCHANGED <<activeRequest, durableRequest, familyConsumed>>

ConsumeResponse ==
  /\ phase \in {"Claimed", "Duplicated", "Contended"}
  /\ phase' = "Consumed"
  /\ activeRequest' = FALSE
  /\ familyConsumed' = TRUE
  /\ claimIdentities' =
       IF RetireConsumedFamily THEN {} ELSE claimIdentities
  /\ claimOrdinals' =
       IF RetireConsumedFamily THEN {} ELSE claimOrdinals
  /\ UNCHANGED <<durableRequest, nextOrdinal>>

DelayedRetryAfterConsumption ==
  /\ phase = "Consumed"
  /\ phase' = "LateRetried"
  /\ IF RetireConsumedFamily
     THEN /\ claimIdentities' = {}
          /\ claimOrdinals' = {}
     ELSE /\ claimIdentities' = {"response-b"}
          /\ claimOrdinals' = {nextOrdinal}
  /\ UNCHANGED
       <<activeRequest, durableRequest, nextOrdinal, familyConsumed>>

RestartSameHeight ==
  /\ phase = "Claimed"
  /\ phase' = "Restarted"
  /\ activeRequest' = TRUE
  /\ durableRequest' = TRUE
  /\ familyConsumed' = FALSE
  /\ claimIdentities' =
       IF ReopenDurableFamilyAfterRestart THEN {} ELSE claimIdentities
  /\ claimOrdinals' =
       IF ReopenDurableFamilyAfterRestart THEN {} ELSE claimOrdinals
  /\ UNCHANGED nextOrdinal

RetransmitAfterRestart ==
  /\ phase = "Restarted"
  /\ claimIdentities = {}
  /\ phase' = "Reopened"
  /\ claimIdentities' = {"response-after-restart"}
  /\ claimOrdinals' = {nextOrdinal}
  /\ nextOrdinal' = nextOrdinal + 1
  /\ UNCHANGED
       <<activeRequest, durableRequest, familyConsumed>>

Next ==
  \/ AdmitFirstResponse
  \/ ReceiveExactDuplicate
  \/ ReceiveCompetingResponder
  \/ ConsumeResponse
  \/ DelayedRetryAfterConsumption
  \/ RestartSameHeight
  \/ RetransmitAfterRestart

=============================================================================
