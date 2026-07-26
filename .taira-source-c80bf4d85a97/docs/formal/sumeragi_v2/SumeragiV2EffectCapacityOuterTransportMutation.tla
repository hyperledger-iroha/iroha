---- MODULE SumeragiV2EffectCapacityOuterTransportMutation ----
EXTENDS Naturals

(***************************************************************************
Finite mutation model for the shared outer-ingress transport-completion class.

Both a CertifiedBodyResponse and a PayloadChunk can be the only event able to
release retained executor debt. The production lane therefore classifies both
as one per-validator TransportCompletion class backed by an independent count
slot and full-envelope canonical-wire byte reserve. Reducer-producing generic
count and byte owners remain installed throughout admission and service.

Each initial state selects one completion kind. Disabling either classification
reproduces the saturated outer-ingress lasso for that kind while the other kind
continues to demonstrate that the two classifications are independent.
This is bounded mutation evidence, not deductive protocol liveness closure.
***************************************************************************)

CONSTANTS ClassifyCertifiedBodyResponse,
          ClassifyPayloadChunk

ASSUME ClassifyCertifiedBodyResponse \in BOOLEAN
ASSUME ClassifyPayloadChunk \in BOOLEAN

VARIABLES phase,
          completionKind,
          genericCountOwned,
          genericBytesOwned,
          transportCountReserveFree,
          transportByteReserveFree,
          completionQueued,
          blockingDebt

vars ==
  <<phase,
    completionKind,
    genericCountOwned,
    genericBytesOwned,
    transportCountReserveFree,
    transportByteReserveFree,
    completionQueued,
    blockingDebt>>

CertifiedBodyResponse == "CertifiedBodyResponse"
PayloadChunk == "PayloadChunk"
CompletionKinds == {CertifiedBodyResponse, PayloadChunk}

CompletionIsClassified ==
  CASE completionKind = CertifiedBodyResponse -> ClassifyCertifiedBodyResponse
    [] completionKind = PayloadChunk -> ClassifyPayloadChunk

TypeInvariant ==
  /\ phase \in 0..2
  /\ completionKind \in CompletionKinds
  /\ genericCountOwned \in BOOLEAN
  /\ genericBytesOwned \in BOOLEAN
  /\ transportCountReserveFree \in BOOLEAN
  /\ transportByteReserveFree \in BOOLEAN
  /\ completionQueued \in BOOLEAN
  /\ blockingDebt \in BOOLEAN

GenericSaturationIsPreserved ==
  /\ genericCountOwned
  /\ genericBytesOwned

QueuedCompletionOwnsBothReserves ==
  completionQueued =>
    /\ ~transportCountReserveFree
    /\ ~transportByteReserveFree

FreeReservesHaveNoQueuedCompletion ==
  (transportCountReserveFree \/ transportByteReserveFree) => ~completionQueued

ReleasedDebtRestoresBothReserves ==
  ~blockingDebt =>
    /\ phase = 2
    /\ ~completionQueued
    /\ transportCountReserveFree
    /\ transportByteReserveFree

Init ==
  /\ phase = 0
  /\ completionKind \in CompletionKinds
  /\ genericCountOwned = TRUE
  /\ genericBytesOwned = TRUE
  /\ transportCountReserveFree = TRUE
  /\ transportByteReserveFree = TRUE
  /\ completionQueued = FALSE
  /\ blockingDebt = TRUE

AdmitTransportCompletion ==
  /\ phase = 0
  /\ blockingDebt
  /\ genericCountOwned
  /\ genericBytesOwned
  /\ CompletionIsClassified
  /\ transportCountReserveFree
  /\ transportByteReserveFree
  /\ phase' = 1
  /\ transportCountReserveFree' = FALSE
  /\ transportByteReserveFree' = FALSE
  /\ completionQueued' = TRUE
  /\ UNCHANGED
       <<completionKind,
         genericCountOwned,
         genericBytesOwned,
         blockingDebt>>

ConsumeTransportCompletion ==
  /\ phase = 1
  /\ blockingDebt
  /\ completionQueued
  /\ phase' = 2
  /\ transportCountReserveFree' = TRUE
  /\ transportByteReserveFree' = TRUE
  /\ completionQueued' = FALSE
  /\ blockingDebt' = FALSE
  /\ UNCHANGED
       <<completionKind,
         genericCountOwned,
         genericBytesOwned>>

Next ==
  \/ AdmitTransportCompletion
  \/ ConsumeTransportCompletion

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AdmitTransportCompletion)
  /\ WF_vars(ConsumeTransportCompletion)

EveryTransportCompletionEventuallyReleasesDebt ==
  blockingDebt ~> ~blockingDebt

=============================================================================
