---- MODULE SumeragiV2AutonomousReservationCarrier ----
EXTENDS Naturals

(***************************************************************************
Bounded ownership model for one autonomous transaction reservation carried
unchanged from the ordinary queue through a control-only global anchor, lane
certification, durable full-candidate authorization, canonical application,
and reservation finalization.

The losing-owner path models the durable cross-store release protocol:

  1. retire the exact autonomous slot and move its Kura claim to
     ReleasePending;
  2. durably prepare the queue release barrier while the reservation remains
     excluded from ordinary selection;
  3. move the Kura claim to Released only after that barrier;
  4. durably complete the queue release and restore FIFO ownership.

The production refinement is source-bound separately to the queue reservation
and release-barrier APIs, Kura autonomous slot claims, full merge-candidate
signing authorization, `StateBlock::stage_certified_merge_entry`, and
`State::validate_merge_execution_batch`. This model is finite mutation
evidence, not a proof of that Rust trace mapping.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Str;
  ExactReservationIdentity,
  \* @type: Str;
  DriftedReservationIdentity,
  \* @type: Str;
  RecreatedReservationIdentity,
  \* @type: Str;
  IncarnationA,
  \* @type: Str;
  IncarnationB

ReservationModes ==
  {"Fixed", "CarrierIdentityDrift", "DuplicateApplication",
   "ReleaseAfterApplication", "ReleaseBeforeBarrier", "AbaRelease",
   "DigestOnlyAuthorization", "OrdinaryAnchorExecution"}

ReservationStages ==
  {"Queued", "Reserved", "Anchored", "Certified", "CandidateDurable",
   "CandidateAuthorized", "ReleasePending", "Released", "Applied",
   "Forgotten"}

ClaimStates == {"None", "Active", "ReleasePending", "Released", "Committed"}

ReservationIdentities ==
  {"None", ExactReservationIdentity, DriftedReservationIdentity,
   RecreatedReservationIdentity}

Incarnations == {"None", IncarnationA, IncarnationB}

ReservationConfiguration ==
  /\ Mode \in ReservationModes
  /\ ExactReservationIdentity # DriftedReservationIdentity
  /\ ExactReservationIdentity # RecreatedReservationIdentity
  /\ DriftedReservationIdentity # RecreatedReservationIdentity
  /\ IncarnationA # IncarnationB

BoolNat(value) == IF value THEN 1 ELSE 0

VARIABLES
  \* @type: Str;
  stage,
  \* @type: Str;
  reservationIdentity,
  \* @type: Str;
  carrierIdentity,
  \* @type: Str;
  incarnation,
  \* @type: Str;
  claimState,
  \* @type: Bool;
  queueOwns,
  \* @type: Bool;
  laneOwns,
  \* @type: Bool;
  mergeOwns,
  \* @type: Bool;
  releaseOwns,
  \* @type: Bool;
  committedOwner,
  \* @type: Int;
  executionCount,
  \* @type: Bool;
  controlOnlyAnchor,
  \* @type: Bool;
  candidateBodyDurable,
  \* @type: Bool;
  candidateAuthorized,
  \* @type: Bool;
  slotRetired,
  \* @type: Bool;
  releaseBarrier,
  \* @type: Bool;
  releaseCompletion,
  \* @type: Bool;
  released,
  \* @type: Bool;
  releaseAfterApply,
  \* @type: Bool;
  recreated,
  \* @type: Bool;
  staleRelease

vars ==
  <<stage, reservationIdentity, carrierIdentity, incarnation, claimState,
    queueOwns, laneOwns, mergeOwns, releaseOwns, committedOwner,
    executionCount, controlOnlyAnchor, candidateBodyDurable,
    candidateAuthorized, slotRetired, releaseBarrier, releaseCompletion,
    released, releaseAfterApply, recreated, staleRelease>>

Init ==
  /\ ReservationConfiguration
  /\ stage = "Queued"
  /\ reservationIdentity = "None"
  /\ carrierIdentity = "None"
  /\ incarnation = "None"
  /\ claimState = "None"
  /\ queueOwns = TRUE
  /\ laneOwns = FALSE
  /\ mergeOwns = FALSE
  /\ releaseOwns = FALSE
  /\ committedOwner = FALSE
  /\ executionCount = 0
  /\ controlOnlyAnchor = FALSE
  /\ candidateBodyDurable = FALSE
  /\ candidateAuthorized = FALSE
  /\ slotRetired = FALSE
  /\ releaseBarrier = FALSE
  /\ releaseCompletion = FALSE
  /\ released = FALSE
  /\ releaseAfterApply = FALSE
  /\ recreated = FALSE
  /\ staleRelease = FALSE

ReserveFifoTransaction ==
  /\ stage = "Queued"
  /\ queueOwns
  /\ ~released
  /\ ~recreated
  /\ executionCount = 0
  /\ stage' = "Reserved"
  /\ reservationIdentity' = ExactReservationIdentity
  /\ carrierIdentity' = ExactReservationIdentity
  /\ incarnation' = IncarnationA
  /\ claimState' = "Active"
  /\ queueOwns' = FALSE
  /\ laneOwns' = TRUE
  /\ mergeOwns' = FALSE
  /\ releaseOwns' = FALSE
  /\ committedOwner' = FALSE
  /\ controlOnlyAnchor' = FALSE
  /\ candidateBodyDurable' = FALSE
  /\ candidateAuthorized' = FALSE
  /\ slotRetired' = FALSE
  /\ releaseBarrier' = FALSE
  /\ releaseCompletion' = FALSE
  /\ released' = FALSE
  /\ releaseAfterApply' = FALSE
  /\ staleRelease' = FALSE
  /\ UNCHANGED <<executionCount, recreated>>

AnchorAutonomousControl ==
  /\ stage = "Reserved"
  /\ laneOwns
  /\ claimState = "Active"
  /\ stage' = "Anchored"
  /\ controlOnlyAnchor' = (Mode # "OrdinaryAnchorExecution")
  /\ executionCount' =
       IF Mode = "OrdinaryAnchorExecution"
       THEN executionCount + 1
       ELSE executionCount
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, candidateBodyDurable, candidateAuthorized,
                 slotRetired, releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease>>

CertifyAutonomousBundle ==
  /\ stage = "Anchored"
  /\ laneOwns
  /\ stage' = "Certified"
  /\ carrierIdentity' =
       IF Mode = "CarrierIdentityDrift"
       THEN DriftedReservationIdentity
       ELSE reservationIdentity
  /\ UNCHANGED <<reservationIdentity, incarnation, claimState, queueOwns,
                 laneOwns, mergeOwns, releaseOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease>>

PersistFullMergeCandidate ==
  /\ stage = "Certified"
  /\ laneOwns
  /\ stage' = "CandidateDurable"
  /\ candidateBodyDurable' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease>>

AuthorizeExactMergeCandidate ==
  /\ \/ /\ stage = "CandidateDurable"
        /\ candidateBodyDurable
     \/ /\ Mode = "DigestOnlyAuthorization"
        /\ stage = "Certified"
        /\ ~candidateBodyDurable
  /\ laneOwns
  /\ stage' = "CandidateAuthorized"
  /\ laneOwns' = FALSE
  /\ mergeOwns' = TRUE
  /\ candidateAuthorized' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, releaseOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 slotRetired, releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease>>

ApplyCanonicalCarrier ==
  /\ stage = "CandidateAuthorized"
  /\ mergeOwns
  /\ candidateAuthorized
  /\ carrierIdentity = reservationIdentity
  /\ executionCount' = executionCount + 1
  /\ claimState' = "Committed"
  /\ IF Mode = "DuplicateApplication"
     THEN /\ stage' = "CandidateAuthorized"
          /\ mergeOwns' = TRUE
          /\ committedOwner' = FALSE
     ELSE /\ stage' = "Applied"
          /\ mergeOwns' = FALSE
          /\ committedOwner' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, releaseOwns, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease>>

ForgetCommittedReservation ==
  /\ stage = "Applied"
  /\ committedOwner
  /\ executionCount = 1
  /\ stage' = "Forgotten"
  /\ claimState' = "None"
  /\ committedOwner' = FALSE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease>>

BeginLosingSlotRetirement ==
  /\ stage \in {"Reserved", "Anchored", "Certified", "CandidateDurable"}
  /\ laneOwns
  /\ claimState = "Active"
  /\ stage' = "ReleasePending"
  /\ claimState' = "ReleasePending"
  /\ laneOwns' = FALSE
  /\ releaseOwns' = TRUE
  /\ slotRetired' = TRUE
  /\ candidateBodyDurable' = FALSE
  /\ candidateAuthorized' = FALSE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, mergeOwns, committedOwner, executionCount,
                 controlOnlyAnchor, releaseBarrier, releaseCompletion,
                 released, releaseAfterApply, recreated, staleRelease>>

PrepareQueueReleaseBarrier ==
  /\ stage = "ReleasePending"
  /\ releaseOwns
  /\ claimState = "ReleasePending"
  /\ ~releaseBarrier
  /\ releaseBarrier' = TRUE
  /\ UNCHANGED <<stage, reservationIdentity, carrierIdentity, incarnation,
                 claimState, queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseCompletion, released, releaseAfterApply, recreated,
                 staleRelease>>

PublishReleasedClaim ==
  /\ stage = "ReleasePending"
  /\ releaseOwns
  /\ claimState = "ReleasePending"
  /\ (releaseBarrier \/ Mode = "ReleaseBeforeBarrier")
  /\ stage' = "Released"
  /\ claimState' = "Released"
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 queueOwns, laneOwns, mergeOwns, releaseOwns,
                 committedOwner, executionCount, controlOnlyAnchor,
                 candidateBodyDurable, candidateAuthorized, slotRetired,
                 releaseBarrier, releaseCompletion, released,
                 releaseAfterApply, recreated, staleRelease>>

CompleteQueueRelease ==
  /\ stage = "Released"
  /\ releaseOwns
  /\ claimState = "Released"
  /\ stage' = "Queued"
  /\ queueOwns' = TRUE
  /\ releaseOwns' = FALSE
  /\ releaseCompletion' = TRUE
  /\ released' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 claimState, laneOwns, mergeOwns, committedOwner,
                 executionCount, controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseAfterApply, recreated, staleRelease>>

ReserveRecreatedIncarnation ==
  /\ stage = "Queued"
  /\ queueOwns
  /\ released
  /\ releaseCompletion
  /\ ~recreated
  /\ executionCount = 0
  /\ stage' = "Reserved"
  /\ reservationIdentity' = RecreatedReservationIdentity
  /\ carrierIdentity' = RecreatedReservationIdentity
  /\ incarnation' = IncarnationB
  /\ claimState' = "Active"
  /\ queueOwns' = FALSE
  /\ laneOwns' = TRUE
  /\ mergeOwns' = FALSE
  /\ releaseOwns' = FALSE
  /\ committedOwner' = FALSE
  /\ controlOnlyAnchor' = FALSE
  /\ candidateBodyDurable' = FALSE
  /\ candidateAuthorized' = FALSE
  /\ slotRetired' = FALSE
  /\ released' = FALSE
  /\ releaseAfterApply' = FALSE
  /\ recreated' = TRUE
  /\ staleRelease' = FALSE
  /\ UNCHANGED <<executionCount, releaseBarrier, releaseCompletion>>

ReplayStaleReleaseMutation ==
  /\ Mode = "AbaRelease"
  /\ stage = "Reserved"
  /\ recreated
  /\ incarnation = IncarnationB
  /\ laneOwns
  /\ releaseBarrier
  /\ releaseCompletion
  /\ stage' = "Queued"
  /\ claimState' = "Released"
  /\ queueOwns' = TRUE
  /\ laneOwns' = FALSE
  /\ released' = TRUE
  /\ staleRelease' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 mergeOwns, releaseOwns, committedOwner, executionCount,
                 controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, releaseAfterApply, recreated>>

ReleaseCommittedMutation ==
  /\ Mode = "ReleaseAfterApplication"
  /\ stage = "Applied"
  /\ committedOwner
  /\ stage' = "Queued"
  /\ claimState' = "Released"
  /\ queueOwns' = TRUE
  /\ committedOwner' = FALSE
  /\ released' = TRUE
  /\ releaseAfterApply' = TRUE
  /\ UNCHANGED <<reservationIdentity, carrierIdentity, incarnation,
                 laneOwns, mergeOwns, releaseOwns, executionCount,
                 controlOnlyAnchor, candidateBodyDurable,
                 candidateAuthorized, slotRetired, releaseBarrier,
                 releaseCompletion, recreated, staleRelease>>

Next ==
  \/ ReserveFifoTransaction
  \/ AnchorAutonomousControl
  \/ CertifyAutonomousBundle
  \/ PersistFullMergeCandidate
  \/ AuthorizeExactMergeCandidate
  \/ ApplyCanonicalCarrier
  \/ ForgetCommittedReservation
  \/ BeginLosingSlotRetirement
  \/ PrepareQueueReleaseBarrier
  \/ PublishReleasedClaim
  \/ CompleteQueueRelease
  \/ ReserveRecreatedIncarnation
  \/ ReplayStaleReleaseMutation
  \/ ReleaseCommittedMutation

ReservationCarrierTypeInvariant ==
  /\ ReservationConfiguration
  /\ stage \in ReservationStages
  /\ reservationIdentity \in ReservationIdentities
  /\ carrierIdentity \in ReservationIdentities
  /\ incarnation \in Incarnations
  /\ claimState \in ClaimStates
  /\ queueOwns \in BOOLEAN
  /\ laneOwns \in BOOLEAN
  /\ mergeOwns \in BOOLEAN
  /\ releaseOwns \in BOOLEAN
  /\ committedOwner \in BOOLEAN
  /\ executionCount \in Nat
  /\ controlOnlyAnchor \in BOOLEAN
  /\ candidateBodyDurable \in BOOLEAN
  /\ candidateAuthorized \in BOOLEAN
  /\ slotRetired \in BOOLEAN
  /\ releaseBarrier \in BOOLEAN
  /\ releaseCompletion \in BOOLEAN
  /\ released \in BOOLEAN
  /\ releaseAfterApply \in BOOLEAN
  /\ recreated \in BOOLEAN
  /\ staleRelease \in BOOLEAN

SingleOwnershipInvariant ==
  BoolNat(queueOwns) + BoolNat(laneOwns) + BoolNat(mergeOwns)
    + BoolNat(releaseOwns) + BoolNat(committedOwner) <= 1

ExactCarrierIdentityInvariant ==
  stage \in
    {"Reserved", "Anchored", "Certified", "CandidateDurable",
     "CandidateAuthorized", "Applied", "Forgotten"} =>
    /\ reservationIdentity \in
         {ExactReservationIdentity, RecreatedReservationIdentity}
    /\ carrierIdentity = reservationIdentity

ControlOnlyAnchorInvariant ==
  stage \in
    {"Anchored", "Certified", "CandidateDurable", "CandidateAuthorized",
     "Applied", "Forgotten"} =>
    controlOnlyAnchor

CandidateAuthorizationInvariant ==
  candidateAuthorized =>
    /\ candidateBodyDurable
    /\ stage \in {"CandidateAuthorized", "Applied", "Forgotten"}

ReleaseOrderingInvariant ==
  claimState = "Released" /\ releaseOwns =>
    /\ slotRetired
    /\ releaseBarrier

QueueReleaseCompletionInvariant ==
  released /\ queueOwns /\ ~releaseAfterApply =>
    /\ releaseCompletion
    /\ claimState = "Released"

AtMostOnceApplicationInvariant == executionCount <= 1

NoReleaseAfterApplicationInvariant ==
  executionCount > 0 =>
    /\ ~queueOwns
    /\ ~released
    /\ ~releaseAfterApply

NoStaleIncarnationReleaseInvariant == ~staleRelease

ForgottenOnlyAfterApplicationInvariant ==
  stage = "Forgotten" => executionCount = 1

AutonomousReservationCarrierSafetyInvariant ==
  /\ ReservationCarrierTypeInvariant
  /\ SingleOwnershipInvariant
  /\ ExactCarrierIdentityInvariant
  /\ ControlOnlyAnchorInvariant
  /\ CandidateAuthorizationInvariant
  /\ ReleaseOrderingInvariant
  /\ QueueReleaseCompletionInvariant
  /\ AtMostOnceApplicationInvariant
  /\ NoReleaseAfterApplicationInvariant
  /\ NoStaleIncarnationReleaseInvariant
  /\ ForgottenOnlyAfterApplicationInvariant

ReservationCarrierSpec == Init /\ [][Next]_vars

AutonomousReservationCarrierProductionRefinementObligation ==
  ReservationCarrierSpec => []AutonomousReservationCarrierSafetyInvariant

====
