---- MODULE SumeragiV2CertifiedRequestCapacityMutation ----
EXTENDS Naturals

(***************************************************************************
Finite mutation model for the independent certified-request capacity seam.

The concrete prefix keeps worker capacity P, certified-request capacity Q,
and retained adapter debt T distinct:

  1. certified Fetch A owns one P slot and the sole Q slot;
  2. Fetch B is either genuinely new or already owns an ordinary, uncertified
     P slot when higher authority is emitted under Q pressure;
  3. production consumes the Q-blocked effect successfully without allocating
     a new P owner, Q owner, or T entry.  Exact reducer Missing/retry debt is
     the sole authority which remains reconstructible;
  4. unrelated retained T is present throughout, but cannot block the exact
     authenticated response for A.  Independent outer response count and
     full-envelope byte reserves admit that response without evicting ordinary
     outer owners;
  5. consuming response A atomically retires its exact P and Q owners; and
  6. periodic retransmission atomically installs new Fetch B into P and Q, or
     upgrades the pre-existing ordinary P owner without changing its identity.

PreserveMissingFetchDebt = FALSE reproduces lost retry authority.
FailOnCapacityBlockedFetch = TRUE independently reproduces fatal Q-pressure
handling.  ReserveTransportResponseCount = FALSE and
ReserveTransportResponseBytes = FALSE reproduce outer count and canonical-wire
byte saturation.  AllowTransportResponseUnderDebt = FALSE reproduces an inner
executor which incorrectly lets unrelated retained T block the response.

This compact finite mutation/refinement obligation covers the live-process
executor and outer-ingress seam.  It does not model Byzantine response
validation, crash/restart reconstruction, or deductive protocol liveness.
***************************************************************************)

CONSTANTS PreserveMissingFetchDebt,
          FailOnCapacityBlockedFetch,
          ReserveTransportResponseCount,
          ReserveTransportResponseBytes,
          AllowTransportResponseUnderDebt

ASSUME PreserveMissingFetchDebt \in BOOLEAN
ASSUME FailOnCapacityBlockedFetch \in BOOLEAN
ASSUME ReserveTransportResponseCount \in BOOLEAN
ASSUME ReserveTransportResponseBytes \in BOOLEAN
ASSUME AllowTransportResponseUnderDebt \in BOOLEAN

VARIABLES phase,
          preexistingOrdinaryWorkB,
          workA,
          workB,
          certifiedWorkB,
          requestOwner,
          missingFetchB,
          retainedFetchB,
          unrelatedRetainedT,
          higherAuthorityEmitted,
          outerGenericCountOwned,
          outerGenericBytesOwned,
          responseAAdmitted,
          responseAQueued,
          responseAConsumed,
          fatal,
          independentCapacityWitness

vars ==
  <<phase,
    preexistingOrdinaryWorkB,
    workA,
    workB,
    certifiedWorkB,
    requestOwner,
    missingFetchB,
    retainedFetchB,
    unrelatedRetainedT,
    higherAuthorityEmitted,
    outerGenericCountOwned,
    outerGenericBytesOwned,
    responseAAdmitted,
    responseAQueued,
    responseAConsumed,
    fatal,
    independentCapacityWitness>>

GeneralWorkCapacity == 2
CertifiedRequestCapacity == 1

NoRequest == "None"
FetchA == "FetchA"
FetchB == "FetchB"
RequestOwners == {NoRequest, FetchA, FetchB}

BoolCount(value) == IF value THEN 1 ELSE 0

PendingWorkCount == BoolCount(workA) + BoolCount(workB)

CertifiedRequestCount == IF requestOwner = NoRequest THEN 0 ELSE 1

TypeInvariant ==
  /\ phase \in 0..4
  /\ preexistingOrdinaryWorkB \in BOOLEAN
  /\ workA \in BOOLEAN
  /\ workB \in BOOLEAN
  /\ certifiedWorkB \in BOOLEAN
  /\ requestOwner \in RequestOwners
  /\ missingFetchB \in BOOLEAN
  /\ retainedFetchB \in BOOLEAN
  /\ unrelatedRetainedT \in BOOLEAN
  /\ higherAuthorityEmitted \in BOOLEAN
  /\ outerGenericCountOwned \in BOOLEAN
  /\ outerGenericBytesOwned \in BOOLEAN
  /\ responseAAdmitted \in BOOLEAN
  /\ responseAQueued \in BOOLEAN
  /\ responseAConsumed \in BOOLEAN
  /\ fatal \in BOOLEAN
  /\ independentCapacityWitness \in BOOLEAN

PendingWorkWithinCapacity == PendingWorkCount <= GeneralWorkCapacity

CertifiedRequestsWithinCapacity ==
  CertifiedRequestCount <= CertifiedRequestCapacity

ExactRequestHasWorkOwner ==
  /\ (requestOwner = FetchA) => workA
  /\ (requestOwner = FetchB) => (workB /\ certifiedWorkB)

(***************************************************************************
The witness rules out conflating Q pressure with P exhaustion.  A new Fetch B
still has a free P slot; an ordinary Fetch B already owns its stable P slot and
needs only an authority upgrade in Q.
***************************************************************************)
IndependentRequestCapacityWitness ==
  higherAuthorityEmitted => independentCapacityWitness

(***************************************************************************
The Q-full effect has returned success during phases 1..3.  Its reducer
Missing debt is live, but no new P owner, Q owner, certified worker state, or
retained T entry has been partially installed.  A pre-existing ordinary Fetch
B remains byte-for-byte uncertified until the retry transition.
***************************************************************************)
CapacityBlockedFetchBRemainsMissing ==
  (phase \in 1..3) => missingFetchB

CapacityBlockedFetchBIsNeverRetained == ~retainedFetchB

CapacityBlockedFetchBHasNoPartialPQT ==
  (phase \in 1..3) =>
    /\ workB = preexistingOrdinaryWorkB
    /\ ~certifiedWorkB
    /\ requestOwner # FetchB
    /\ ~retainedFetchB

CertifiedRequestPressureIsNonfatal == higherAuthorityEmitted => ~fatal

UnrelatedRetainedDebtIsPreserved == unrelatedRetainedT

ResponseARetiresExactWorkAndRequest ==
  responseAConsumed =>
    /\ responseAAdmitted
    /\ ~responseAQueued
    /\ ~workA
    /\ requestOwner # FetchA

OuterGenericSaturationIsPreserved ==
  /\ outerGenericCountOwned
  /\ outerGenericBytesOwned

OuterResponseAdmissionUsesBothReserves ==
  responseAAdmitted =>
    /\ ReserveTransportResponseCount
    /\ ReserveTransportResponseBytes

QueuedResponseIsExactAdmittedOwner ==
  responseAQueued =>
    /\ responseAAdmitted
    /\ ~responseAConsumed

RetransmittedFetchBAtomicallyInstallsOrUpgrades ==
  (phase = 4) =>
    /\ ~workA
    /\ workB
    /\ certifiedWorkB
    /\ requestOwner = FetchB
    /\ CertifiedRequestCount = CertifiedRequestCapacity
    /\ PendingWorkCount = 1
    /\ ~missingFetchB
    /\ ~retainedFetchB
    /\ unrelatedRetainedT
    /\ responseAConsumed

Init ==
  /\ phase = 0
  /\ preexistingOrdinaryWorkB \in BOOLEAN
  /\ workA = TRUE
  /\ workB = preexistingOrdinaryWorkB
  /\ certifiedWorkB = FALSE
  /\ requestOwner = FetchA
  /\ missingFetchB = FALSE
  /\ retainedFetchB = FALSE
  /\ unrelatedRetainedT = TRUE
  /\ higherAuthorityEmitted = FALSE
  /\ outerGenericCountOwned = TRUE
  /\ outerGenericBytesOwned = TRUE
  /\ responseAAdmitted = FALSE
  /\ responseAQueued = FALSE
  /\ responseAConsumed = FALSE
  /\ fatal = FALSE
  /\ independentCapacityWitness = FALSE

(***************************************************************************
The effect is consumed even though Q is full.  The transition records only
reconstructible reducer Missing debt; P, Q, and T remain exactly unchanged.
***************************************************************************)
EmitHigherAuthorityFetchB ==
  /\ phase = 0
  /\ workA
  /\ workB = preexistingOrdinaryWorkB
  /\ ~certifiedWorkB
  /\ \/ /\ ~preexistingOrdinaryWorkB
         /\ PendingWorkCount < GeneralWorkCapacity
     \/ /\ preexistingOrdinaryWorkB
         /\ PendingWorkCount = GeneralWorkCapacity
  /\ requestOwner = FetchA
  /\ CertifiedRequestCount = CertifiedRequestCapacity
  /\ phase' = 1
  /\ missingFetchB' = PreserveMissingFetchDebt
  /\ retainedFetchB' = FALSE
  /\ higherAuthorityEmitted' = TRUE
  /\ fatal' = FailOnCapacityBlockedFetch
  /\ independentCapacityWitness' = TRUE
  /\ UNCHANGED
       <<preexistingOrdinaryWorkB,
         workA,
         workB,
         certifiedWorkB,
         requestOwner,
         unrelatedRetainedT,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         responseAQueued,
         responseAConsumed>>

(***************************************************************************
Reducer-producing count and byte owners saturate their ordinary outer ingress
partitions.  The authenticated response crosses that boundary only through a
dedicated response count slot and full canonical-envelope byte reserve.
Unrelated retained T is framed and no generic owner is evicted.
***************************************************************************)
AdmitOuterTransportResponseA ==
  /\ phase = 1
  /\ ~fatal
  /\ missingFetchB
  /\ ~retainedFetchB
  /\ unrelatedRetainedT
  /\ workA
  /\ requestOwner = FetchA
  /\ outerGenericCountOwned
  /\ outerGenericBytesOwned
  /\ ReserveTransportResponseCount
  /\ ReserveTransportResponseBytes
  /\ ~responseAAdmitted
  /\ ~responseAQueued
  /\ phase' = 2
  /\ responseAAdmitted' = TRUE
  /\ responseAQueued' = TRUE
  /\ UNCHANGED
       <<preexistingOrdinaryWorkB,
         workA,
         workB,
         certifiedWorkB,
         requestOwner,
         missingFetchB,
         retainedFetchB,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAConsumed,
         fatal,
         independentCapacityWitness>>

(***************************************************************************
The admitted response is transport-only.  It crosses unrelated retained T and
atomically releases the exact P and Q resources owned by A.
***************************************************************************)
ConsumeTransportOnlyResponseA ==
  /\ phase = 2
  /\ ~fatal
  /\ missingFetchB
  /\ ~retainedFetchB
  /\ unrelatedRetainedT
  /\ AllowTransportResponseUnderDebt
  /\ responseAAdmitted
  /\ responseAQueued
  /\ workA
  /\ requestOwner = FetchA
  /\ phase' = 3
  /\ workA' = FALSE
  /\ requestOwner' = NoRequest
  /\ responseAQueued' = FALSE
  /\ responseAConsumed' = TRUE
  /\ UNCHANGED
       <<preexistingOrdinaryWorkB,
         workB,
         certifiedWorkB,
         missingFetchB,
         retainedFetchB,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         fatal,
         independentCapacityWitness>>

(***************************************************************************
The periodic reducer source reconstructs Fetch B after A has released Q.
New work acquires P and Q atomically.  An existing ordinary worker retains its
P identity while its certified bit and Q owner appear in the same transition.
No retained Fetch is drained.
***************************************************************************)
RetransmitMissingFetchB ==
  /\ phase = 3
  /\ ~fatal
  /\ missingFetchB
  /\ ~retainedFetchB
  /\ unrelatedRetainedT
  /\ responseAConsumed
  /\ \/ /\ ~workB
         /\ PendingWorkCount < GeneralWorkCapacity
     \/ /\ workB
         /\ preexistingOrdinaryWorkB
         /\ ~certifiedWorkB
  /\ CertifiedRequestCount < CertifiedRequestCapacity
  /\ phase' = 4
  /\ workB' = TRUE
  /\ certifiedWorkB' = TRUE
  /\ requestOwner' = FetchB
  /\ missingFetchB' = FALSE
  /\ UNCHANGED
       <<preexistingOrdinaryWorkB,
         workA,
         retainedFetchB,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         responseAQueued,
         responseAConsumed,
         fatal,
         independentCapacityWitness>>

Next ==
  \/ EmitHigherAuthorityFetchB
  \/ AdmitOuterTransportResponseA
  \/ ConsumeTransportOnlyResponseA
  \/ RetransmitMissingFetchB

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(EmitHigherAuthorityFetchB)
  /\ WF_vars(AdmitOuterTransportResponseA)
  /\ WF_vars(ConsumeTransportOnlyResponseA)
  /\ WF_vars(RetransmitMissingFetchB)

OuterResponseEventuallyAdmitted ==
  (phase = 1 /\ missingFetchB) ~> responseAAdmitted

TransportResponseEventuallyReleasesA ==
  (phase = 1 /\ missingFetchB) ~> responseAConsumed

MissingFetchBEventuallyOwnsRequest ==
  (phase = 1 /\ missingFetchB)
    ~> (phase = 4 /\ requestOwner = FetchB /\ certifiedWorkB)

RepairedCertifiedRequestProgress ==
  /\ OuterResponseEventuallyAdmitted
  /\ TransportResponseEventuallyReleasesA
  /\ MissingFetchBEventuallyOwnsRequest

=============================================================================
