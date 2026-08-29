---- MODULE SoraParliamentV1 ----
EXTENDS FiniteSets, Integers, Naturals, Sequences, TLC

(***************************************************************************
This is a finite safety model of the implemented first-release Parliament
attempt reducer. It deliberately abstracts hashes, signatures, zero-knowledge
proofs, and the exact set of Parliament bodies. Those primitives are admitted
as already verified inputs; this model checks only their lifecycle bindings.

The model is not a liveness proof, a cryptographic proof, or release evidence.
***************************************************************************)

CONSTANTS
    MaxHeight,
    SortitionPulseDelayBlocks,
    MaxSortitionRetries,
    MaxRandomnessRedraws,
    RegistrationBlocks,
    SurvivorBlocks,
    CommitmentBlocks,
    MaxCorpusEntries,
    ReleaseDelay,
    OpeningBlocks,
    FindingBlocks,
    EnactDelay,
    MaxRetries,
    MaxConcurrentReservations,
    ReservationIds,
    FirstConflictingReservation,
    SecondConflictingReservation,
    Bodies,
    SeatedAssignments,
    FirstAssignment,
    SecondAssignment,
    FindingRoots,
    TleSessions,
    AvailableReleaseSessions,
    ExpectedHead,
    CompetingHead,
    None

ASSUME /\ MaxHeight \in Nat \ {0}
       /\ SortitionPulseDelayBlocks \in Nat \ {0}
       /\ MaxSortitionRetries \in Nat
       /\ MaxRandomnessRedraws \in Nat \ {0}
       /\ RegistrationBlocks \in Nat \ {0}
       /\ SurvivorBlocks \in Nat \ {0}
       /\ CommitmentBlocks \in Nat \ {0}
       /\ MaxCorpusEntries \in Nat \ {0}
       /\ ReleaseDelay \in Nat \ {0}
       /\ OpeningBlocks \in Nat \ {0}
       /\ FindingBlocks \in Nat \ {0}
       /\ EnactDelay \in Nat \ {0}
       /\ MaxRetries \in Nat
       /\ Cardinality(ReservationIds) >= 3
       /\ MaxConcurrentReservations \in 2..Cardinality(ReservationIds)
       /\ FirstConflictingReservation \in ReservationIds
       /\ SecondConflictingReservation \in ReservationIds
       /\ FirstConflictingReservation # SecondConflictingReservation
       /\ Bodies # {}
       /\ SeatedAssignments # {}
       /\ SeatedAssignments = {FirstAssignment, SecondAssignment}
       /\ FirstAssignment # SecondAssignment
       /\ FindingRoots # {}
       /\ Cardinality(TleSessions) >= MaxRetries + 1
       /\ AvailableReleaseSessions \subseteq TleSessions
       /\ ExpectedHead # CompetingHead
       /\ None \notin TleSessions
       /\ None \notin SeatedAssignments
       /\ None \notin FindingRoots
       /\ None # ExpectedHead
       /\ None # CompetingHead

VARIABLES
    height,
    attemptStatus,
    governanceAttemptSequence,
    randomnessRedrawsBeforeAttempt,
    sortitionState,
    sortitionSequence,
    sortitionFailureKind,
    sortitionFailureHeight,
    supersededSortitionAttempts,
    requestHeight,
    sortitionPulseHeight,
    sortitionPulseKnown,
    sortitionPulseConsumed,
    sortitionCandidateCount,
    candidateSnapshotFrozen,
    rosterBodies,
    invitationCloseHeight,
    findingState,
    absentAssignments,
    absenceDeclaredBy,
    findingEndorsements,
    findingEndorsedBy,
    findingOpenedAtHeight,
    findingDeadlineHeight,
    findingFailureKind,
    findingFailureHeight,
    findingResultRoot,
    findingEndorsementRoot,
    findingEndorsingAssignments,
    findingEndorsementCount,
    ballotState,
    ballotSequence,
    currentTleSession,
    usedTleSessions,
    registeredAtHeight,
    registrationCloseHeight,
    survivorFreezeHeight,
    commitmentCloseHeight,
    releaseHeight,
    registrationClosedAt,
    survivorsFrozenAt,
    commitmentClosedAt,
    releasePulseKnown,
    openingHeight,
    failureHeight,
    ballotApproved,
    policyRequiresConfirmation,
    eligibleConfirmationCandidates,
    policyResultHeight,
    policyBindingCommitted,
    confirmationRequirementCommitted,
    confirmationRequestCommitted,
    confirmationRequestHeight,
    confirmationPulseHeight,
    certifiedAtHeight,
    enactAtHeight,
    certificateHead,
    certificateFindingRoot,
    certificateFindingEndorsementRoot,
    certificateFindingEndorsingAssignments,
    certificateFindingEndorsementCount,
    certificateFindingQuorum,
    observedHead,
    effectApplied,
    terminalHeight,
    plaintextPath,
    fallbackPath,
    timedOvnResourceReservations,
    rejectedReservationSnapshot,
    reservationAuditStep

vars == <<
    height,
    attemptStatus,
    governanceAttemptSequence,
    randomnessRedrawsBeforeAttempt,
    sortitionState,
    sortitionSequence,
    sortitionFailureKind,
    sortitionFailureHeight,
    supersededSortitionAttempts,
    requestHeight,
    sortitionPulseHeight,
    sortitionPulseKnown,
    sortitionPulseConsumed,
    sortitionCandidateCount,
    candidateSnapshotFrozen,
    rosterBodies,
    invitationCloseHeight,
    findingState,
    absentAssignments,
    absenceDeclaredBy,
    findingEndorsements,
    findingEndorsedBy,
    findingOpenedAtHeight,
    findingDeadlineHeight,
    findingFailureKind,
    findingFailureHeight,
    findingResultRoot,
    findingEndorsementRoot,
    findingEndorsingAssignments,
    findingEndorsementCount,
    ballotState,
    ballotSequence,
    currentTleSession,
    usedTleSessions,
    registeredAtHeight,
    registrationCloseHeight,
    survivorFreezeHeight,
    commitmentCloseHeight,
    releaseHeight,
    registrationClosedAt,
    survivorsFrozenAt,
    commitmentClosedAt,
    releasePulseKnown,
    openingHeight,
    failureHeight,
    ballotApproved,
    policyRequiresConfirmation,
    eligibleConfirmationCandidates,
    policyResultHeight,
    policyBindingCommitted,
    confirmationRequirementCommitted,
    confirmationRequestCommitted,
    confirmationRequestHeight,
    confirmationPulseHeight,
    certifiedAtHeight,
    enactAtHeight,
    certificateHead,
    certificateFindingRoot,
    certificateFindingEndorsementRoot,
    certificateFindingEndorsingAssignments,
    certificateFindingEndorsementCount,
    certificateFindingQuorum,
    observedHead,
    effectApplied,
    terminalHeight,
    plaintextPath,
    fallbackPath,
    timedOvnResourceReservations,
    rejectedReservationSnapshot,
    reservationAuditStep
>>

AttemptStates == {
    "Active", "Certified", "Rejected", "Enacted", "Superseded",
    "ExecutionFailed"
}
SortitionStates == {
    "None", "AwaitingPulse", "NoRoster", "Drawn", "RosterSealed"
}
SortitionFailureKinds == {
    None, "PulseUnavailable", "HiddenElectorateCapacityUnavailable"
}
FindingStates == {
    "None", "AwaitingReflection", "Collecting", "Approved", "NoResult"
}
FindingFailureKinds == {None, "QuorumUnreachable", "DeadlineExpired"}
BallotStates == {
    "None", "Registration", "SurvivorFreeze", "TimedCommitment",
    "AwaitingRelease", "Opening", "Approved", "Rejected", "NoResult"
}
OptionalReservationSet == (SUBSET ReservationIds) \cup {None}
ReservationConflicts == {
    <<FirstConflictingReservation, SecondConflictingReservation>>,
    <<SecondConflictingReservation, FirstConflictingReservation>>
}
NonConflictingReservation ==
    CHOOSE reservation \in
        ReservationIds \ {FirstConflictingReservation, SecondConflictingReservation}:
            TRUE
CertificateStates == {"Certified", "Enacted", "Superseded", "ExecutionFailed"}
OptionalHeight == (0..MaxHeight) \cup {None}

BoolToNat(predicate) == IF predicate THEN 1 ELSE 0

InitialSortitionRedrawCost == BoolToNat(governanceAttemptSequence > 0)

SortitionRandomnessRedrawsUsed ==
    IF sortitionState = "None"
    THEN 0
    ELSE sortitionSequence + InitialSortitionRedrawCost

BallotRandomnessRedrawsUsed ==
    IF ballotState = "None" THEN 0 ELSE ballotSequence

ProposalRandomnessRedrawsUsed ==
    randomnessRedrawsBeforeAttempt +
        SortitionRandomnessRedrawsUsed +
        BallotRandomnessRedrawsUsed +
        BoolToNat(confirmationRequestCommitted)

PublicFindingQuorum ==
    (2 * Cardinality(SeatedAssignments) + 2) \div 3

AssignmentOrder == <<FirstAssignment, SecondAssignment>>

CanonicalEndorserSequence(endorsements, root) ==
    SelectSeq(AssignmentOrder,
        LAMBDA assignment: endorsements[assignment] = root)

FindingCount(endorsements, root) ==
    Len(CanonicalEndorserSequence(endorsements, root))

RecordedFindingAssignments(endorsements) ==
    {assignment \in SeatedAssignments : endorsements[assignment] # None}

FindingQuorumUnreachable(absent, endorsements) ==
    LET eligible == Cardinality(SeatedAssignments \ absent)
        remaining == eligible - Cardinality(RecordedFindingAssignments(endorsements))
    IN
    \A root \in FindingRoots:
        FindingCount(endorsements, root) + remaining < PublicFindingQuorum

FindingEvidenceRoot(endorsements, root) ==
    <<root, CanonicalEndorserSequence(endorsements, root)>>

FindingEvidenceRoots ==
    {FindingEvidenceRoot(endorsements, root) :
        endorsements \in [SeatedAssignments -> FindingRoots \cup {None}],
        root \in FindingRoots}

Init ==
    /\ height = 0
    /\ attemptStatus = "Active"
    /\ \/ /\ governanceAttemptSequence = 0
           /\ randomnessRedrawsBeforeAttempt = 0
       \/ /\ governanceAttemptSequence = 1
           /\ randomnessRedrawsBeforeAttempt \in
                 0..(MaxRandomnessRedraws - 1)
    /\ sortitionState = "None"
    /\ sortitionSequence = 0
    /\ sortitionFailureKind = None
    /\ sortitionFailureHeight = None
    /\ supersededSortitionAttempts = 0
    /\ requestHeight = None
    /\ sortitionPulseHeight = None
    /\ sortitionPulseKnown = FALSE
    /\ sortitionPulseConsumed = FALSE
    /\ sortitionCandidateCount = None
    /\ candidateSnapshotFrozen = FALSE
    /\ rosterBodies = {}
    /\ invitationCloseHeight = None
    /\ findingState = "None"
    /\ absentAssignments = {}
    /\ absenceDeclaredBy = [assignment \in SeatedAssignments |-> None]
    /\ findingEndorsements = [assignment \in SeatedAssignments |-> None]
    /\ findingEndorsedBy = [assignment \in SeatedAssignments |-> None]
    /\ findingOpenedAtHeight = None
    /\ findingDeadlineHeight = None
    /\ findingFailureKind = None
    /\ findingFailureHeight = None
    /\ findingResultRoot = None
    /\ findingEndorsementRoot = None
    /\ findingEndorsingAssignments = <<>>
    /\ findingEndorsementCount = 0
    /\ ballotState = "None"
    /\ ballotSequence = 0
    /\ currentTleSession = None
    /\ usedTleSessions = {}
    /\ registeredAtHeight = None
    /\ registrationCloseHeight = None
    /\ survivorFreezeHeight = None
    /\ commitmentCloseHeight = None
    /\ releaseHeight = None
    /\ registrationClosedAt = None
    /\ survivorsFrozenAt = None
    /\ commitmentClosedAt = None
    /\ releasePulseKnown = FALSE
    /\ openingHeight = None
    /\ failureHeight = None
    /\ ballotApproved = FALSE
    /\ policyRequiresConfirmation = FALSE
    /\ eligibleConfirmationCandidates = None
    /\ policyResultHeight = None
    /\ policyBindingCommitted = FALSE
    /\ confirmationRequirementCommitted = FALSE
    /\ confirmationRequestCommitted = FALSE
    /\ confirmationRequestHeight = None
    /\ confirmationPulseHeight = None
    /\ certifiedAtHeight = None
    /\ enactAtHeight = None
    /\ certificateHead = None
    /\ certificateFindingRoot = None
    /\ certificateFindingEndorsementRoot = None
    /\ certificateFindingEndorsingAssignments = <<>>
    /\ certificateFindingEndorsementCount = 0
    /\ certificateFindingQuorum = 0
    /\ observedHead = ExpectedHead
    /\ effectApplied = FALSE
    /\ terminalHeight = None
    /\ plaintextPath = FALSE
    /\ fallbackPath = FALSE
    /\ timedOvnResourceReservations = {}
    /\ rejectedReservationSnapshot = None
    /\ reservationAuditStep = 0

FindingLifecycleFrame ==
    UNCHANGED <<
        findingState, absentAssignments, absenceDeclaredBy,
        findingEndorsements, findingEndorsedBy, findingResultRoot,
        findingOpenedAtHeight, findingDeadlineHeight, findingFailureKind,
        findingFailureHeight,
        findingEndorsementRoot, findingEndorsingAssignments,
        findingEndorsementCount
    >>

FindingCertificateFrame ==
    UNCHANGED <<
        governanceAttemptSequence, randomnessRedrawsBeforeAttempt,
        certificateFindingRoot, certificateFindingEndorsementRoot,
        certificateFindingEndorsingAssignments,
        certificateFindingEndorsementCount, certificateFindingQuorum
    >>

PolicyConfirmationFrame ==
    UNCHANGED <<
        policyRequiresConfirmation, eligibleConfirmationCandidates,
        policyResultHeight, policyBindingCommitted,
        confirmationRequirementCommitted, confirmationRequestCommitted,
        confirmationRequestHeight, confirmationPulseHeight
    >>

FindingFrame ==
    FindingLifecycleFrame /\ FindingCertificateFrame /\ PolicyConfirmationFrame

ReservationFrame ==
    UNCHANGED <<
        timedOvnResourceReservations,
        rejectedReservationSnapshot,
        reservationAuditStep
    >>

CoreFrame ==
    UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved,
        policyRequiresConfirmation, eligibleConfirmationCandidates,
        policyResultHeight, policyBindingCommitted,
        confirmationRequirementCommitted, confirmationRequestCommitted,
        confirmationRequestHeight, confirmationPulseHeight, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
    >>

CoreFrameExceptAttemptStatus ==
    UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved,
        policyRequiresConfirmation, eligibleConfirmationCandidates,
        policyResultHeight, policyBindingCommitted,
        confirmationRequirementCommitted, confirmationRequestCommitted,
        confirmationRequestHeight, confirmationPulseHeight, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
    >>

Tick ==
    /\ height < MaxHeight
    /\ ~(attemptStatus = "Certified" /\ height = enactAtHeight)
    /\ height' = height + 1
    /\ UNCHANGED <<
        attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight, sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, failureHeight, ballotApproved,
        certifiedAtHeight, enactAtHeight, certificateHead, observedHead,
        effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

CommitInitialSortitionBatch ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "None"
    /\ ProposalRandomnessRedrawsUsed + InitialSortitionRedrawCost <=
          MaxRandomnessRedraws
    /\ height + SortitionPulseDelayBlocks <= MaxHeight
    /\ sortitionState' = "AwaitingPulse"
    /\ requestHeight' = height
    /\ sortitionPulseHeight' = height + SortitionPulseDelayBlocks
    /\ sortitionPulseConsumed' = FALSE
    /\ sortitionCandidateCount' = 2
    /\ candidateSnapshotFrozen' = TRUE
    /\ UNCHANGED <<
        height, attemptStatus, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts,
        sortitionPulseKnown, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, failureHeight, ballotApproved,
        certifiedAtHeight, enactAtHeight, certificateHead, observedHead,
        effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

RecordInitialHiddenSortitionCapacityFailure(candidateCount) ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "None"
    /\ ProposalRandomnessRedrawsUsed + InitialSortitionRedrawCost <=
          MaxRandomnessRedraws
    /\ candidateCount \in 0..1
    /\ height + SortitionPulseDelayBlocks <= MaxHeight
    /\ sortitionState' = "NoRoster"
    /\ sortitionFailureKind' = "HiddenElectorateCapacityUnavailable"
    /\ sortitionFailureHeight' = height
    /\ requestHeight' = height
    /\ sortitionPulseHeight' = height + SortitionPulseDelayBlocks
    /\ sortitionPulseKnown' = FALSE
    /\ sortitionPulseConsumed' = FALSE
    /\ sortitionCandidateCount' = candidateCount
    /\ candidateSnapshotFrozen' = TRUE
    /\ attemptStatus' =
          IF (MaxSortitionRetries = 0 \/
                ProposalRandomnessRedrawsUsed + InitialSortitionRedrawCost =
                    MaxRandomnessRedraws)
          THEN "Rejected"
          ELSE attemptStatus
    /\ UNCHANGED <<
        height, sortitionSequence, supersededSortitionAttempts, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, failureHeight, ballotApproved,
        certifiedAtHeight, enactAtHeight, certificateHead, observedHead,
        effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

RevealSortitionPulse ==
    /\ sortitionState = "AwaitingPulse"
    /\ ~sortitionPulseKnown
    /\ height >= sortitionPulseHeight
    /\ sortitionPulseKnown' = TRUE
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, failureHeight, ballotApproved,
        certifiedAtHeight, enactAtHeight, certificateHead, observedHead,
        effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

ConsumeInitialSortitionBatch ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "AwaitingPulse"
    /\ sortitionPulseKnown
    /\ sortitionState' = "Drawn"
    /\ sortitionPulseConsumed' = TRUE
    /\ rosterBodies' = Bodies
    /\ UNCHANGED <<
        height, attemptStatus, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight,
        sortitionPulseKnown, sortitionCandidateCount, candidateSnapshotFrozen,
        invitationCloseHeight,
        ballotState, ballotSequence, currentTleSession, usedTleSessions,
        registeredAtHeight, registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

FailSortitionPulseUnavailable ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "AwaitingPulse"
    /\ ~sortitionPulseKnown
    /\ height > sortitionPulseHeight
    /\ sortitionState' = "NoRoster"
    /\ sortitionFailureKind' = "PulseUnavailable"
    /\ sortitionFailureHeight' = height
    /\ attemptStatus' =
          IF (sortitionSequence = MaxSortitionRetries \/
                ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws)
          THEN "Rejected"
          ELSE attemptStatus
    /\ UNCHANGED <<
        height, sortitionSequence, supersededSortitionAttempts,
        requestHeight, sortitionPulseHeight, sortitionPulseKnown,
        sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies, invitationCloseHeight,
        ballotState, ballotSequence, currentTleSession, usedTleSessions,
        registeredAtHeight, registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

SortitionRetryHeightEligible ==
    IF sortitionFailureKind = "HiddenElectorateCapacityUnavailable"
    THEN height > sortitionFailureHeight
    ELSE height >= sortitionFailureHeight

RetryInitialSortitionBatch ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "NoRoster"
    /\ sortitionFailureKind \in {
          "PulseUnavailable", "HiddenElectorateCapacityUnavailable"
       }
    /\ sortitionFailureHeight # None
    /\ sortitionSequence < MaxSortitionRetries
    /\ ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws
    /\ SortitionRetryHeightEligible
    /\ height + SortitionPulseDelayBlocks <= MaxHeight
    /\ sortitionState' = "AwaitingPulse"
    /\ sortitionSequence' = sortitionSequence + 1
    /\ supersededSortitionAttempts' = supersededSortitionAttempts + 1
    /\ requestHeight' = height
    /\ sortitionPulseHeight' = height + SortitionPulseDelayBlocks
    /\ sortitionPulseKnown' = FALSE
    /\ sortitionPulseConsumed' = FALSE
    /\ sortitionCandidateCount' = 2
    /\ sortitionFailureKind' = None
    /\ sortitionFailureHeight' = None
    /\ candidateSnapshotFrozen' = TRUE
    /\ UNCHANGED <<
        height, attemptStatus, rosterBodies, invitationCloseHeight,
        ballotState, ballotSequence, currentTleSession, usedTleSessions,
        registeredAtHeight, registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

RecordRetryHiddenSortitionCapacityFailure(candidateCount) ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "NoRoster"
    /\ sortitionFailureKind \in {
          "PulseUnavailable", "HiddenElectorateCapacityUnavailable"
       }
    /\ sortitionFailureHeight # None
    /\ sortitionSequence < MaxSortitionRetries
    /\ ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws
    /\ SortitionRetryHeightEligible
    /\ candidateCount \in 0..1
    /\ height + SortitionPulseDelayBlocks <= MaxHeight
    /\ sortitionSequence' = sortitionSequence + 1
    /\ supersededSortitionAttempts' = supersededSortitionAttempts + 1
    /\ requestHeight' = height
    /\ sortitionPulseHeight' = height + SortitionPulseDelayBlocks
    /\ sortitionPulseKnown' = FALSE
    /\ sortitionPulseConsumed' = FALSE
    /\ sortitionCandidateCount' = candidateCount
    /\ candidateSnapshotFrozen' = TRUE
    /\ sortitionFailureKind' = "HiddenElectorateCapacityUnavailable"
    /\ sortitionFailureHeight' = height
    /\ attemptStatus' =
          IF (sortitionSequence + 1 = MaxSortitionRetries \/
                ProposalRandomnessRedrawsUsed + 1 = MaxRandomnessRedraws)
          THEN "Rejected"
          ELSE attemptStatus
    /\ UNCHANGED <<
        height, sortitionState, rosterBodies, invitationCloseHeight,
        ballotState, ballotSequence, currentTleSession, usedTleSessions,
        registeredAtHeight, registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

SealInvitationRosters ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "Drawn"
    /\ findingState = "None"
    /\ height < MaxHeight
    /\ sortitionState' = "RosterSealed"
    /\ invitationCloseHeight' = height + 1
    /\ findingState' = "AwaitingReflection"
    /\ UNCHANGED <<
        height, attemptStatus, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        ballotState, ballotSequence, currentTleSession, usedTleSessions,
        registeredAtHeight, registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        terminalHeight, plaintextPath, fallbackPath
        >>
    /\ UNCHANGED <<
        absentAssignments, absenceDeclaredBy, findingEndorsements,
        findingEndorsedBy, findingOpenedAtHeight, findingDeadlineHeight,
        findingFailureKind, findingFailureHeight, findingResultRoot,
        findingEndorsementRoot,
        findingEndorsingAssignments, findingEndorsementCount
        >>
    /\ FindingCertificateFrame
    /\ PolicyConfirmationFrame

EnterPublicFindingReflection ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "RosterSealed"
    /\ findingState = "AwaitingReflection"
    /\ height > invitationCloseHeight
    /\ height + FindingBlocks <= MaxHeight
    /\ findingState' = "Collecting"
    /\ findingOpenedAtHeight' = height
    /\ findingDeadlineHeight' = height + FindingBlocks
    /\ UNCHANGED <<
        absentAssignments, absenceDeclaredBy, findingEndorsements,
        findingEndorsedBy, findingFailureKind, findingFailureHeight,
        findingResultRoot, findingEndorsementRoot,
        findingEndorsingAssignments, findingEndorsementCount
        >>
    /\ CoreFrame
    /\ FindingCertificateFrame

RecordSelfAbsence(assignment) ==
    LET updatedAbsent == absentAssignments \cup {assignment}
        unreachable == FindingQuorumUnreachable(
            updatedAbsent, findingEndorsements)
    IN
    /\ attemptStatus = "Active"
    /\ sortitionState = "RosterSealed"
    /\ findingState \in {"AwaitingReflection", "Collecting"}
    /\ IF findingState = "AwaitingReflection"
          THEN TRUE
          ELSE height <= findingDeadlineHeight
    /\ assignment \in SeatedAssignments
    /\ assignment \notin absentAssignments
    /\ absenceDeclaredBy[assignment] = None
    /\ \A seated \in SeatedAssignments:
          findingEndorsements[seated] = None
    /\ absentAssignments' = updatedAbsent
    /\ absenceDeclaredBy' =
          [absenceDeclaredBy EXCEPT ![assignment] = assignment]
    /\ findingState' = IF unreachable THEN "NoResult" ELSE findingState
    /\ attemptStatus' = IF unreachable THEN "Rejected" ELSE attemptStatus
    /\ findingFailureKind' =
          IF unreachable THEN "QuorumUnreachable" ELSE findingFailureKind
    /\ findingFailureHeight' =
          IF unreachable THEN height ELSE findingFailureHeight
    /\ UNCHANGED <<
        findingEndorsements, findingEndorsedBy, findingOpenedAtHeight,
        findingDeadlineHeight, findingResultRoot, findingEndorsementRoot,
        findingEndorsingAssignments, findingEndorsementCount
        >>
    /\ CoreFrameExceptAttemptStatus
    /\ FindingCertificateFrame

EndorsePublicFinding(assignment, root) ==
    LET updated == [findingEndorsements EXCEPT ![assignment] = root]
        endorsers == CanonicalEndorserSequence(updated, root)
        count == Len(endorsers)
        unreachable == FindingQuorumUnreachable(absentAssignments, updated)
    IN
    /\ attemptStatus = "Active"
    /\ sortitionState = "RosterSealed"
    /\ findingState = "Collecting"
    /\ height > invitationCloseHeight
    /\ height <= findingDeadlineHeight
    /\ assignment \in SeatedAssignments \ absentAssignments
    /\ root \in FindingRoots
    /\ findingEndorsements[assignment] = None
    /\ findingEndorsedBy[assignment] = None
    /\ findingEndorsements' = updated
    /\ findingEndorsedBy' =
          [findingEndorsedBy EXCEPT ![assignment] = assignment]
    /\ IF count >= PublicFindingQuorum
          THEN /\ attemptStatus' = attemptStatus
               /\ findingState' = "Approved"
               /\ UNCHANGED <<findingFailureKind, findingFailureHeight>>
               /\ findingResultRoot' = root
               /\ findingEndorsementRoot' = <<root, endorsers>>
               /\ findingEndorsingAssignments' = endorsers
               /\ findingEndorsementCount' = count
          ELSE IF unreachable
            THEN /\ attemptStatus' = "Rejected"
                 /\ findingState' = "NoResult"
                 /\ findingFailureKind' = "QuorumUnreachable"
                 /\ findingFailureHeight' = height
                 /\ UNCHANGED <<
                     findingResultRoot, findingEndorsementRoot,
                     findingEndorsingAssignments, findingEndorsementCount
                     >>
            ELSE /\ attemptStatus' = attemptStatus
                 /\ UNCHANGED <<findingFailureKind, findingFailureHeight>>
                 /\ UNCHANGED <<
                     findingState, findingResultRoot, findingEndorsementRoot,
                     findingEndorsingAssignments, findingEndorsementCount
                     >>
    /\ UNCHANGED <<
        absentAssignments, absenceDeclaredBy, findingOpenedAtHeight,
        findingDeadlineHeight
        >>
    /\ CoreFrameExceptAttemptStatus
    /\ FindingCertificateFrame

FailPublicFindingNoResult ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "RosterSealed"
    /\ findingState = "Collecting"
    /\ findingDeadlineHeight # None
    /\ height > findingDeadlineHeight
    /\ findingState' = "NoResult"
    /\ findingFailureKind' = "DeadlineExpired"
    /\ findingFailureHeight' = height
    /\ attemptStatus' = "Rejected"
    /\ UNCHANGED <<
        absentAssignments, absenceDeclaredBy, findingEndorsements,
        findingEndorsedBy, findingOpenedAtHeight, findingDeadlineHeight,
        findingResultRoot, findingEndorsementRoot,
        findingEndorsingAssignments, findingEndorsementCount
        >>
    /\ CoreFrameExceptAttemptStatus
    /\ FindingCertificateFrame

RegisterPrivateBallot ==
    /\ attemptStatus = "Active"
    /\ sortitionState = "RosterSealed"
    /\ findingState = "Approved"
    /\ height > invitationCloseHeight
    /\ ballotState \in {"None", "NoResult"}
    /\ ballotState = "None" \/ ballotSequence < MaxRetries
    /\ ballotState = "None" \/
          ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws
    /\ IF ballotState = "NoResult"
          THEN /\ failureHeight # None
               /\ height >= failureHeight
          ELSE TRUE
    /\ \E tle \in TleSessions \ usedTleSessions:
        /\ ballotState' = "Registration"
        /\ ballotSequence' = IF ballotState = "None" THEN 0 ELSE ballotSequence + 1
        /\ currentTleSession' = tle
        /\ usedTleSessions' = usedTleSessions \cup {tle}
        /\ registeredAtHeight' = height
        /\ registrationCloseHeight' = height + RegistrationBlocks
        /\ survivorFreezeHeight' = height + RegistrationBlocks + SurvivorBlocks
        /\ commitmentCloseHeight' =
              height + RegistrationBlocks + SurvivorBlocks + CommitmentBlocks
        /\ releaseHeight' =
              height + RegistrationBlocks + SurvivorBlocks +
              CommitmentBlocks + ReleaseDelay
        /\ releaseHeight' + OpeningBlocks <= MaxHeight
        /\ registrationClosedAt' = None
        /\ survivorsFrozenAt' = None
        /\ commitmentClosedAt' = None
        /\ releasePulseKnown' = FALSE
        /\ openingHeight' = None
        /\ failureHeight' = None
        /\ ballotApproved' = FALSE
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, certifiedAtHeight, enactAtHeight,
        certificateHead, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingFrame

CloseRegistrationAtBoundary ==
    /\ attemptStatus = "Active"
    /\ ballotState = "Registration"
    /\ height = registrationCloseHeight
    /\ ballotState' = "SurvivorFreeze"
    /\ registrationClosedAt' = height
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, survivorsFrozenAt,
        commitmentClosedAt, releasePulseKnown, openingHeight, failureHeight,
        ballotApproved, certifiedAtHeight, enactAtHeight, certificateHead,
        observedHead, effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

FreezeSurvivorsAtBoundary ==
    /\ attemptStatus = "Active"
    /\ ballotState = "SurvivorFreeze"
    /\ registrationClosedAt = registrationCloseHeight
    /\ height = survivorFreezeHeight
    /\ ballotState' = "TimedCommitment"
    /\ survivorsFrozenAt' = height
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        commitmentClosedAt, releasePulseKnown, openingHeight, failureHeight,
        ballotApproved, certifiedAtHeight, enactAtHeight, certificateHead,
        observedHead, effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

\* Refinement note: concrete contiguous 1..32-record CorpusOpen appends occur
\* across (survivorFreezeHeight, commitmentCloseHeight]. Intermediate prefixes
\* stutter at this abstraction because their raw count is not modeled. This
\* action represents only the append reaching the exact frozen survivor count;
\* scripts/formal/check_sora_parliament_source_contract.py separately pins the
\* concrete prefix, chunk, capacity, and terminalization guards.
FreezeCommitmentInWindow ==
    /\ attemptStatus = "Active"
    /\ ballotState = "TimedCommitment"
    /\ survivorsFrozenAt = survivorFreezeHeight
    /\ height > survivorFreezeHeight
    /\ height <= commitmentCloseHeight
    /\ ballotState' = "AwaitingRelease"
    /\ commitmentClosedAt' = height
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, releasePulseKnown, openingHeight, failureHeight,
        ballotApproved, certifiedAtHeight, enactAtHeight, certificateHead,
        observedHead, effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

ObserveCommittedReleasePulse ==
    /\ attemptStatus = "Active"
    /\ ballotState = "AwaitingRelease"
    /\ height >= releaseHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ currentTleSession \in AvailableReleaseSessions
    /\ releasePulseKnown' = TRUE
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, openingHeight, failureHeight,
        ballotApproved, certifiedAtHeight, enactAtHeight, certificateHead,
        observedHead, effectApplied, terminalHeight, plaintextPath, fallbackPath
        >>
    /\ FindingFrame

BeginAggregateOpening ==
    /\ attemptStatus = "Active"
    /\ ballotState = "AwaitingRelease"
    /\ releasePulseKnown
    /\ height >= releaseHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ ballotState' = "Opening"
    /\ openingHeight' = height
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        failureHeight, ballotApproved, certifiedAtHeight, enactAtHeight,
        certificateHead, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingFrame

FailPrivateBallotNoResult ==
    /\ attemptStatus = "Active"
    /\ \/ /\ ballotState = "Registration"
           /\ height > registrationCloseHeight
       \/ /\ ballotState = "SurvivorFreeze"
           /\ height > survivorFreezeHeight
       \/ /\ ballotState = "TimedCommitment"
           /\ height > commitmentCloseHeight
       \/ /\ ballotState = "AwaitingRelease"
           /\ ~releasePulseKnown
           /\ height > releaseHeight
           /\ height <= releaseHeight + OpeningBlocks
           /\ currentTleSession \notin AvailableReleaseSessions
       \/ /\ ballotState \in {"AwaitingRelease", "Opening"}
           /\ height > releaseHeight + OpeningBlocks
    /\ ballotState' = "NoResult"
    /\ failureHeight' = height
    /\ attemptStatus' =
          IF (ballotSequence = MaxRetries \/
                ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws)
          THEN "Rejected"
          ELSE attemptStatus
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, ballotApproved, certifiedAtHeight, enactAtHeight,
        certificateHead, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingFrame

\* This branch represents a wide Policy result that needs no Confirmation Jury.
FinalizeAggregateApprovedAndCertify ==
    /\ attemptStatus = "Active"
    /\ findingState = "Approved"
    /\ ballotState = "Opening"
    /\ height >= openingHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ height + EnactDelay <= MaxHeight
    /\ ballotState' = "Approved"
    /\ ballotApproved' = TRUE
    /\ policyRequiresConfirmation' = FALSE
    /\ eligibleConfirmationCandidates' = None
    /\ policyResultHeight' = height
    /\ policyBindingCommitted' = TRUE
    /\ confirmationRequirementCommitted' = FALSE
    /\ confirmationRequestCommitted' = FALSE
    /\ confirmationRequestHeight' = None
    /\ confirmationPulseHeight' = None
    /\ attemptStatus' = "Certified"
    /\ certifiedAtHeight' = height
    /\ enactAtHeight' = height + EnactDelay
    /\ certificateHead' = ExpectedHead
    /\ certificateFindingRoot' = findingResultRoot
    /\ certificateFindingEndorsementRoot' = findingEndorsementRoot
    /\ certificateFindingEndorsingAssignments' = findingEndorsingAssignments
    /\ certificateFindingEndorsementCount' = findingEndorsementCount
    /\ certificateFindingQuorum' = PublicFindingQuorum
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingLifecycleFrame

FinalizeNarrowPolicyCapacityNoResult(eligibleCount) ==
    /\ attemptStatus = "Active"
    /\ findingState = "Approved"
    /\ ballotState = "Opening"
    /\ height >= openingHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ eligibleCount \in 0..1
    /\ ballotState' = "NoResult"
    /\ ballotApproved' = FALSE
    /\ failureHeight' = height
    /\ attemptStatus' = "Rejected"
    /\ policyRequiresConfirmation' = TRUE
    /\ eligibleConfirmationCandidates' = eligibleCount
    /\ policyResultHeight' = height
    /\ policyBindingCommitted' = FALSE
    /\ confirmationRequirementCommitted' = FALSE
    /\ confirmationRequestCommitted' = FALSE
    /\ confirmationRequestHeight' = None
    /\ confirmationPulseHeight' = None
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, certifiedAtHeight, enactAtHeight,
        certificateHead, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingLifecycleFrame
    /\ FindingCertificateFrame

\* Typed concrete classification:
\* ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted.
FinalizeNarrowPolicyRandomnessRedrawBudgetExhausted ==
    /\ attemptStatus = "Active"
    /\ findingState = "Approved"
    /\ ballotState = "Opening"
    /\ height >= openingHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws
    /\ ballotState' = "NoResult"
    /\ ballotApproved' = FALSE
    /\ failureHeight' = height
    /\ attemptStatus' = "Rejected"
    /\ policyRequiresConfirmation' = TRUE
    /\ eligibleConfirmationCandidates' = 2
    /\ policyResultHeight' = height
    /\ policyBindingCommitted' = FALSE
    /\ confirmationRequirementCommitted' = FALSE
    /\ confirmationRequestCommitted' = FALSE
    /\ confirmationRequestHeight' = None
    /\ confirmationPulseHeight' = None
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, certifiedAtHeight, enactAtHeight,
        certificateHead, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingLifecycleFrame
    /\ FindingCertificateFrame

FinalizeNarrowPolicyAndRegisterConfirmationRequest ==
    /\ attemptStatus = "Active"
    /\ findingState = "Approved"
    /\ ballotState = "Opening"
    /\ height >= openingHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ height + SortitionPulseDelayBlocks <= MaxHeight
    /\ ProposalRandomnessRedrawsUsed < MaxRandomnessRedraws
    /\ ballotState' = "Approved"
    /\ ballotApproved' = TRUE
    /\ policyRequiresConfirmation' = TRUE
    /\ eligibleConfirmationCandidates' = 2
    /\ policyResultHeight' = height
    /\ policyBindingCommitted' = TRUE
    /\ confirmationRequirementCommitted' = TRUE
    /\ confirmationRequestCommitted' = TRUE
    /\ confirmationRequestHeight' = height
    /\ confirmationPulseHeight' = height + SortitionPulseDelayBlocks
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight, sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies, invitationCloseHeight,
        ballotSequence, currentTleSession, usedTleSessions,
        registeredAtHeight, registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, certifiedAtHeight, enactAtHeight,
        certificateHead, observedHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingLifecycleFrame
    /\ FindingCertificateFrame

FinalizeAggregateRejected ==
    /\ attemptStatus = "Active"
    /\ ballotState = "Opening"
    /\ height >= openingHeight
    /\ height <= releaseHeight + OpeningBlocks
    /\ ballotState' = "Rejected"
    /\ ballotApproved' = FALSE
    /\ attemptStatus' = "Rejected"
    /\ terminalHeight' = height
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotSequence, currentTleSession,
        usedTleSessions, registeredAtHeight, registrationCloseHeight,
        survivorFreezeHeight, commitmentCloseHeight, releaseHeight,
        registrationClosedAt, survivorsFrozenAt, commitmentClosedAt,
        releasePulseKnown, openingHeight, failureHeight, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        plaintextPath, fallbackPath
        >>
    /\ FindingFrame

ChangeGovernedHead ==
    /\ attemptStatus \in {"Active", "Certified"}
    /\ observedHead = ExpectedHead
    /\ observedHead' = CompetingHead
    /\ UNCHANGED <<
        height, attemptStatus, sortitionState, sortitionSequence,
        sortitionFailureKind, sortitionFailureHeight,
        supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight, sortitionPulseKnown, sortitionPulseConsumed,
        sortitionCandidateCount, candidateSnapshotFrozen,
        rosterBodies, invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, effectApplied, terminalHeight,
        plaintextPath, fallbackPath
        >>
    /\ FindingFrame

EnactAtExactHeight ==
    /\ attemptStatus = "Certified"
    /\ height = enactAtHeight
    /\ observedHead = certificateHead
    /\ attemptStatus' = "Enacted"
    /\ effectApplied' = TRUE
    /\ terminalHeight' = height
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, plaintextPath,
        fallbackPath
        >>
    /\ FindingFrame

RecordInternalExecutionFailureAtExactHeight ==
    /\ attemptStatus = "Certified"
    /\ height = enactAtHeight
    /\ observedHead = certificateHead
    /\ attemptStatus' = "ExecutionFailed"
    /\ effectApplied' = FALSE
    /\ terminalHeight' = height
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, plaintextPath,
        fallbackPath
        >>
    /\ FindingFrame

SupersedeAtExactHeight ==
    /\ attemptStatus = "Certified"
    /\ height = enactAtHeight
    /\ observedHead # certificateHead
    /\ attemptStatus' = "Superseded"
    /\ terminalHeight' = height
    /\ UNCHANGED <<
        height, sortitionState, sortitionSequence, sortitionFailureKind,
        sortitionFailureHeight, supersededSortitionAttempts, requestHeight,
        sortitionPulseHeight,
        sortitionPulseKnown, sortitionPulseConsumed, sortitionCandidateCount,
        candidateSnapshotFrozen, rosterBodies,
        invitationCloseHeight, ballotState, ballotSequence,
        currentTleSession, usedTleSessions, registeredAtHeight,
        registrationCloseHeight, survivorFreezeHeight,
        commitmentCloseHeight, releaseHeight, registrationClosedAt,
        survivorsFrozenAt, commitmentClosedAt, releasePulseKnown,
        openingHeight, failureHeight, ballotApproved, certifiedAtHeight,
        enactAtHeight, certificateHead, observedHead, effectApplied,
        plaintextPath, fallbackPath
        >>
    /\ FindingFrame

ReservationConflictsWithActive(candidate) ==
    \E incumbent \in timedOvnResourceReservations:
        <<candidate, incumbent>> \in ReservationConflicts

ReservationAdmissionAllowed(candidate) ==
    /\ candidate \notin timedOvnResourceReservations
    /\ Cardinality(timedOvnResourceReservations) < MaxConcurrentReservations
    /\ ~ReservationConflictsWithActive(candidate)

AdmitTimedOvnResourceReservation(candidate) ==
    /\ candidate \in ReservationIds
    /\ ReservationAdmissionAllowed(candidate)
    /\ timedOvnResourceReservations' =
          timedOvnResourceReservations \cup {candidate}
    /\ rejectedReservationSnapshot' = None
    /\ CoreFrame
    /\ FindingFrame

RejectTimedOvnResourceReservation(candidate) ==
    /\ candidate \in ReservationIds
    /\ ~ReservationAdmissionAllowed(candidate)
    /\ timedOvnResourceReservations' = timedOvnResourceReservations
    /\ rejectedReservationSnapshot' = timedOvnResourceReservations
    /\ CoreFrame
    /\ FindingFrame

ReleaseTimedOvnResourceReservation(candidate) ==
    /\ candidate \in timedOvnResourceReservations
    /\ timedOvnResourceReservations' =
          timedOvnResourceReservations \ {candidate}
    /\ rejectedReservationSnapshot' = None
    /\ CoreFrame
    /\ FindingFrame

\* An exact replay of an already committed request or session is transport,
\* not a fresh adversarial draw. It therefore stutters over the complete
\* proposal state, including the cumulative randomness-redraw expression.
ReplayCommittedTransportIdempotently ==
    /\ attemptStatus = "Active"
    /\ \/ sortitionState # "None"
       \/ ballotState # "None"
       \/ confirmationRequestCommitted
    /\ UNCHANGED vars

ReducerNext ==
    \/ Tick
    \/ ReplayCommittedTransportIdempotently
    \/ CommitInitialSortitionBatch
    \/ \E candidateCount \in 0..1:
          RecordInitialHiddenSortitionCapacityFailure(candidateCount)
    \/ RevealSortitionPulse
    \/ FailSortitionPulseUnavailable
    \/ RetryInitialSortitionBatch
    \/ \E candidateCount \in 0..1:
          RecordRetryHiddenSortitionCapacityFailure(candidateCount)
    \/ ConsumeInitialSortitionBatch
    \/ SealInvitationRosters
    \/ EnterPublicFindingReflection
    \/ \E assignment \in SeatedAssignments: RecordSelfAbsence(assignment)
    \/ \E assignment \in SeatedAssignments, root \in FindingRoots:
          EndorsePublicFinding(assignment, root)
    \/ FailPublicFindingNoResult
    \/ RegisterPrivateBallot
    \/ CloseRegistrationAtBoundary
    \/ FreezeSurvivorsAtBoundary
    \/ FreezeCommitmentInWindow
    \/ ObserveCommittedReleasePulse
    \/ BeginAggregateOpening
    \/ FailPrivateBallotNoResult
    \/ FinalizeAggregateApprovedAndCertify
    \/ \E eligibleCount \in 0..1:
          FinalizeNarrowPolicyCapacityNoResult(eligibleCount)
    \/ FinalizeNarrowPolicyRandomnessRedrawBudgetExhausted
    \/ FinalizeNarrowPolicyAndRegisterConfirmationRequest
    \/ FinalizeAggregateRejected
    \/ ChangeGovernedHead
    \/ EnactAtExactHeight
    \/ RecordInternalExecutionFailureAtExactHeight
    \/ SupersedeAtExactHeight

ReservationNext ==
    \/ /\ reservationAuditStep = 0
       /\ AdmitTimedOvnResourceReservation(FirstConflictingReservation)
       /\ reservationAuditStep' = 1
    \/ /\ reservationAuditStep = 1
       /\ RejectTimedOvnResourceReservation(SecondConflictingReservation)
       /\ reservationAuditStep' = 2
    \/ /\ reservationAuditStep = 2
       /\ AdmitTimedOvnResourceReservation(NonConflictingReservation)
       /\ reservationAuditStep' = 3
    \/ /\ reservationAuditStep = 3
       /\ RejectTimedOvnResourceReservation(SecondConflictingReservation)
       /\ reservationAuditStep' = 4
    \/ /\ reservationAuditStep = 4
       /\ ReleaseTimedOvnResourceReservation(FirstConflictingReservation)
       /\ reservationAuditStep' = 5
    \/ /\ reservationAuditStep = 5
       /\ AdmitTimedOvnResourceReservation(SecondConflictingReservation)
       /\ reservationAuditStep' = 6
    \/ /\ reservationAuditStep = 6
       /\ ReleaseTimedOvnResourceReservation(NonConflictingReservation)
       /\ reservationAuditStep' = 7
    \/ /\ reservationAuditStep = 7
       /\ ReleaseTimedOvnResourceReservation(SecondConflictingReservation)
       /\ reservationAuditStep' = 8

Next ==
    \/ /\ reservationAuditStep = 8
       /\ ReducerNext
       /\ ReservationFrame
    \/ ReservationNext

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ height \in 0..MaxHeight
    /\ attemptStatus \in AttemptStates
    /\ governanceAttemptSequence \in 0..1
    /\ randomnessRedrawsBeforeAttempt \in 0..MaxRandomnessRedraws
    /\ sortitionState \in SortitionStates
    /\ sortitionSequence \in 0..MaxSortitionRetries
    /\ sortitionFailureKind \in SortitionFailureKinds
    /\ sortitionFailureHeight \in OptionalHeight
    /\ supersededSortitionAttempts \in 0..MaxSortitionRetries
    /\ requestHeight \in OptionalHeight
    /\ sortitionPulseHeight \in OptionalHeight
    /\ sortitionPulseKnown \in BOOLEAN
    /\ sortitionPulseConsumed \in BOOLEAN
    /\ sortitionCandidateCount \in (0..2) \cup {None}
    /\ candidateSnapshotFrozen \in BOOLEAN
    /\ rosterBodies \subseteq Bodies
    /\ invitationCloseHeight \in OptionalHeight
    /\ findingState \in FindingStates
    /\ absentAssignments \subseteq SeatedAssignments
    /\ absenceDeclaredBy \in
          [SeatedAssignments -> SeatedAssignments \cup {None}]
    /\ findingEndorsements \in
          [SeatedAssignments -> FindingRoots \cup {None}]
    /\ findingEndorsedBy \in
          [SeatedAssignments -> SeatedAssignments \cup {None}]
    /\ findingOpenedAtHeight \in OptionalHeight
    /\ findingDeadlineHeight \in OptionalHeight
    /\ findingFailureKind \in FindingFailureKinds
    /\ findingFailureHeight \in OptionalHeight
    /\ findingResultRoot \in FindingRoots \cup {None}
    /\ findingEndorsementRoot \in FindingEvidenceRoots \cup {None}
    /\ findingEndorsingAssignments \in Seq(SeatedAssignments)
    /\ findingEndorsementCount \in 0..Cardinality(SeatedAssignments)
    /\ ballotState \in BallotStates
    /\ ballotSequence \in 0..MaxRetries
    /\ currentTleSession \in TleSessions \cup {None}
    /\ usedTleSessions \subseteq TleSessions
    /\ registeredAtHeight \in OptionalHeight
    /\ registrationCloseHeight \in OptionalHeight
    /\ survivorFreezeHeight \in OptionalHeight
    /\ commitmentCloseHeight \in OptionalHeight
    /\ releaseHeight \in OptionalHeight
    /\ registrationClosedAt \in OptionalHeight
    /\ survivorsFrozenAt \in OptionalHeight
    /\ commitmentClosedAt \in OptionalHeight
    /\ releasePulseKnown \in BOOLEAN
    /\ openingHeight \in OptionalHeight
    /\ failureHeight \in OptionalHeight
    /\ ballotApproved \in BOOLEAN
    /\ policyRequiresConfirmation \in BOOLEAN
    /\ eligibleConfirmationCandidates \in (0..2) \cup {None}
    /\ policyResultHeight \in OptionalHeight
    /\ policyBindingCommitted \in BOOLEAN
    /\ confirmationRequirementCommitted \in BOOLEAN
    /\ confirmationRequestCommitted \in BOOLEAN
    /\ confirmationRequestHeight \in OptionalHeight
    /\ confirmationPulseHeight \in OptionalHeight
    /\ certifiedAtHeight \in OptionalHeight
    /\ enactAtHeight \in OptionalHeight
    /\ certificateHead \in {ExpectedHead, CompetingHead, None}
    /\ certificateFindingRoot \in FindingRoots \cup {None}
    /\ certificateFindingEndorsementRoot \in FindingEvidenceRoots \cup {None}
    /\ certificateFindingEndorsingAssignments \in Seq(SeatedAssignments)
    /\ certificateFindingEndorsementCount \in
          0..Cardinality(SeatedAssignments)
    /\ certificateFindingQuorum \in 0..Cardinality(SeatedAssignments)
    /\ observedHead \in {ExpectedHead, CompetingHead}
    /\ effectApplied \in BOOLEAN
    /\ terminalHeight \in OptionalHeight
    /\ plaintextPath \in BOOLEAN
    /\ fallbackPath \in BOOLEAN
    /\ timedOvnResourceReservations \subseteq ReservationIds
    /\ rejectedReservationSnapshot \in OptionalReservationSet
    /\ reservationAuditStep \in 0..8

ProposalWideRandomnessRedrawBudget ==
    /\ ProposalRandomnessRedrawsUsed \in 0..MaxRandomnessRedraws
    /\ governanceAttemptSequence = 0 =>
          randomnessRedrawsBeforeAttempt = 0
    /\ governanceAttemptSequence > 0 =>
          randomnessRedrawsBeforeAttempt < MaxRandomnessRedraws
    /\ (sortitionState = "NoRoster" /\
          ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws) =>
          attemptStatus = "Rejected"
    /\ (ballotState = "NoResult" /\
          ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws) =>
          attemptStatus = "Rejected"

FuturePulseSortition ==
    /\ (sortitionState = "None")
       \/ /\ candidateSnapshotFrozen
          /\ requestHeight # None
          /\ sortitionPulseHeight # None
          /\ sortitionPulseHeight =
                requestHeight + SortitionPulseDelayBlocks
    /\ sortitionPulseKnown => height >= sortitionPulseHeight

ObjectiveBoundedSortitionRetries ==
    /\ supersededSortitionAttempts = sortitionSequence
    /\ sortitionState = "None" =>
          /\ sortitionSequence = 0
          /\ sortitionFailureKind = None
          /\ sortitionFailureHeight = None
          /\ ~sortitionPulseKnown
          /\ ~sortitionPulseConsumed
          /\ sortitionCandidateCount = None
    /\ sortitionState = "AwaitingPulse" =>
          /\ attemptStatus = "Active"
          /\ sortitionFailureKind = None
          /\ sortitionFailureHeight = None
          /\ ~sortitionPulseConsumed
          /\ sortitionCandidateCount = 2
    /\ sortitionState = "NoRoster" =>
          /\ sortitionFailureKind \in {
                "PulseUnavailable", "HiddenElectorateCapacityUnavailable"
             }
          /\ sortitionFailureHeight # None
          /\ ~sortitionPulseKnown
          /\ ~sortitionPulseConsumed
          /\ IF sortitionFailureKind = "PulseUnavailable"
                THEN /\ sortitionFailureHeight > sortitionPulseHeight
                     /\ sortitionCandidateCount = 2
                ELSE /\ sortitionFailureHeight = requestHeight
                     /\ sortitionCandidateCount \in 0..1
          /\ attemptStatus =
                IF (sortitionSequence = MaxSortitionRetries \/
                      ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws)
                THEN "Rejected"
                ELSE "Active"
    /\ sortitionState \in {"Drawn", "RosterSealed"} =>
          /\ sortitionPulseKnown
          /\ sortitionPulseConsumed
          /\ sortitionCandidateCount = 2
          /\ sortitionFailureKind = None
          /\ sortitionFailureHeight = None

HiddenElectorateCapacityConsumesNoPulse ==
    /\ sortitionFailureKind = "HiddenElectorateCapacityUnavailable" =>
          /\ sortitionState = "NoRoster"
          /\ sortitionCandidateCount \in 0..1
          /\ sortitionFailureHeight = requestHeight
          /\ ~sortitionPulseKnown
          /\ ~sortitionPulseConsumed
    /\ sortitionCandidateCount \in 0..1 =>
          sortitionFailureKind = "HiddenElectorateCapacityUnavailable"

TimedOvnReservationSafety ==
    /\ Cardinality(timedOvnResourceReservations) <=
          MaxConcurrentReservations
    /\ \A left, right \in timedOvnResourceReservations:
          left = right \/
              <<left, right>> \notin ReservationConflicts

RejectedReservationDoesNotLeak ==
    rejectedReservationSnapshot = None \/
        timedOvnResourceReservations = rejectedReservationSnapshot

TimedOvnReservationAuditShape ==
    CASE reservationAuditStep = 0 ->
             timedOvnResourceReservations = {}
      [] reservationAuditStep \in {1, 2} ->
             timedOvnResourceReservations = {FirstConflictingReservation}
      [] reservationAuditStep \in {3, 4} ->
             timedOvnResourceReservations = {
                 FirstConflictingReservation,
                 NonConflictingReservation
             }
      [] reservationAuditStep = 5 ->
             timedOvnResourceReservations = {NonConflictingReservation}
      [] reservationAuditStep = 6 ->
             timedOvnResourceReservations = {
                 NonConflictingReservation,
                 SecondConflictingReservation
             }
      [] reservationAuditStep = 7 ->
             timedOvnResourceReservations = {SecondConflictingReservation}
      [] OTHER -> timedOvnResourceReservations = {}

SimultaneousInitialDraw == rosterBodies = {} \/ rosterBodies = Bodies

AuthorityBoundImmutableMemberRecords ==
    /\ \A assignment \in SeatedAssignments:
          /\ absenceDeclaredBy[assignment] \in {None, assignment}
          /\ (assignment \in absentAssignments) =
                (absenceDeclaredBy[assignment] = assignment)
          /\ findingEndorsedBy[assignment] \in {None, assignment}
          /\ (findingEndorsements[assignment] = None) =
                (findingEndorsedBy[assignment] = None)
          /\ findingEndorsements[assignment] # None =>
                assignment \notin absentAssignments

PublicFindingQuorumBinding ==
    /\ findingState # "None" => sortitionState = "RosterSealed"
    /\ findingState = "None" =>
          /\ absentAssignments = {}
          /\ \A assignment \in SeatedAssignments:
                /\ absenceDeclaredBy[assignment] = None
                /\ findingEndorsements[assignment] = None
                /\ findingEndorsedBy[assignment] = None
          /\ findingResultRoot = None
          /\ findingOpenedAtHeight = None
          /\ findingDeadlineHeight = None
          /\ findingFailureKind = None
          /\ findingFailureHeight = None
          /\ findingEndorsementRoot = None
          /\ findingEndorsingAssignments = <<>>
          /\ findingEndorsementCount = 0
    /\ findingState = "AwaitingReflection" =>
          /\ \A assignment \in SeatedAssignments:
                /\ findingEndorsements[assignment] = None
                /\ findingEndorsedBy[assignment] = None
          /\ findingOpenedAtHeight = None
          /\ findingDeadlineHeight = None
          /\ findingFailureKind = None
          /\ findingFailureHeight = None
          /\ findingResultRoot = None
          /\ findingEndorsementRoot = None
          /\ findingEndorsingAssignments = <<>>
          /\ findingEndorsementCount = 0
    /\ findingState = "Collecting" =>
          /\ \A root \in FindingRoots:
                FindingCount(findingEndorsements, root) < PublicFindingQuorum
          /\ ~FindingQuorumUnreachable(
                absentAssignments, findingEndorsements)
          /\ findingOpenedAtHeight # None
          /\ findingDeadlineHeight = findingOpenedAtHeight + FindingBlocks
          /\ findingFailureKind = None
          /\ findingFailureHeight = None
          /\ findingResultRoot = None
          /\ findingEndorsementRoot = None
          /\ findingEndorsingAssignments = <<>>
          /\ findingEndorsementCount = 0
    /\ findingState = "Approved" =>
          /\ findingOpenedAtHeight # None
          /\ findingDeadlineHeight = findingOpenedAtHeight + FindingBlocks
          /\ findingFailureKind = None
          /\ findingFailureHeight = None
          /\ findingResultRoot \in FindingRoots
          /\ findingEndorsementCount =
                FindingCount(findingEndorsements, findingResultRoot)
          /\ findingEndorsementCount = PublicFindingQuorum
          /\ findingEndorsingAssignments =
                CanonicalEndorserSequence(
                    findingEndorsements, findingResultRoot)
          /\ Len(findingEndorsingAssignments) = findingEndorsementCount
          /\ findingEndorsementRoot =
                FindingEvidenceRoot(findingEndorsements, findingResultRoot)
    /\ findingState = "NoResult" =>
          /\ attemptStatus = "Rejected"
          /\ findingFailureHeight # None
          /\ IF findingFailureKind = "QuorumUnreachable"
                THEN FindingQuorumUnreachable(
                    absentAssignments, findingEndorsements)
                ELSE /\ findingFailureKind = "DeadlineExpired"
                     /\ findingOpenedAtHeight # None
                     /\ findingDeadlineHeight =
                           findingOpenedAtHeight + FindingBlocks
                     /\ findingFailureHeight > findingDeadlineHeight
          /\ findingResultRoot = None
          /\ findingEndorsementRoot = None
          /\ findingEndorsingAssignments = <<>>
          /\ findingEndorsementCount = 0
    /\ ballotState # "None" => findingState = "Approved"

ExactBallotSchedule ==
    ballotState = "None" \/
        /\ registeredAtHeight # None
        /\ registrationCloseHeight = registeredAtHeight + RegistrationBlocks
        /\ survivorFreezeHeight = registrationCloseHeight + SurvivorBlocks
        /\ commitmentCloseHeight = survivorFreezeHeight + CommitmentBlocks
        /\ releaseHeight = commitmentCloseHeight + ReleaseDelay
        /\ releaseHeight + OpeningBlocks <= MaxHeight

PhaseCapacity ==
    /\ RegistrationBlocks >= MaxCorpusEntries + 1
    /\ SurvivorBlocks >= MaxCorpusEntries
    /\ CommitmentBlocks * 32 >= MaxCorpusEntries
    /\ MaxCorpusEntries >= Cardinality(SeatedAssignments)

ExactPhaseBoundaries ==
    /\ registrationClosedAt = None
       \/ registrationClosedAt = registrationCloseHeight
    /\ survivorsFrozenAt = None
       \/ survivorsFrozenAt = survivorFreezeHeight
    /\ commitmentClosedAt = None
       \/ /\ commitmentClosedAt > survivorFreezeHeight
          /\ commitmentClosedAt <= commitmentCloseHeight

ExactPublicFindingDeadline ==
    /\ findingOpenedAtHeight = None
       \/ findingDeadlineHeight = findingOpenedAtHeight + FindingBlocks
    /\ (findingFailureKind = None) = (findingFailureHeight = None)

ObjectiveReleaseAvailability ==
    releasePulseKnown => currentTleSession \in AvailableReleaseSessions

BoundedOpeningWindow ==
    openingHeight = None \/
        /\ openingHeight >= releaseHeight
        /\ openingHeight <= releaseHeight + OpeningBlocks

FreshRetrySessions ==
    ballotState = "None" \/
        /\ currentTleSession \in usedTleSessions
        /\ Cardinality(usedTleSessions) = ballotSequence + 1
        /\ ballotSequence <= MaxRetries
        /\ (ballotState # "NoResult" \/
              /\ failureHeight # None
              /\ registeredAtHeight <= failureHeight)

AtomicPolicyConfirmationCapacity ==
    /\ confirmationRequirementCommitted = confirmationRequestCommitted
    /\ policyRequiresConfirmation =>
          /\ eligibleConfirmationCandidates \in 0..2
          /\ policyResultHeight # None
          /\ IF ~confirmationRequestCommitted
                THEN /\ (eligibleConfirmationCandidates \in 0..1 \/
                          ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws)
                     /\ attemptStatus = "Rejected"
                     /\ ballotState = "NoResult"
                     /\ ~ballotApproved
                     /\ failureHeight = policyResultHeight
                     /\ ~policyBindingCommitted
                     /\ ~confirmationRequirementCommitted
                     /\ ~confirmationRequestCommitted
                     /\ confirmationRequestHeight = None
                     /\ confirmationPulseHeight = None
                ELSE /\ eligibleConfirmationCandidates = 2
                     /\ ProposalRandomnessRedrawsUsed <= MaxRandomnessRedraws
                     /\ attemptStatus = "Active"
                     /\ ballotState = "Approved"
                     /\ ballotApproved
                     /\ policyBindingCommitted
                     /\ confirmationRequirementCommitted
                     /\ confirmationRequestCommitted
                     /\ confirmationRequestHeight = policyResultHeight
                     /\ confirmationPulseHeight =
                           policyResultHeight + SortitionPulseDelayBlocks
    /\ ~policyRequiresConfirmation =>
          /\ eligibleConfirmationCandidates = None
          /\ ~confirmationRequirementCommitted
          /\ ~confirmationRequestCommitted
          /\ confirmationRequestHeight = None
          /\ confirmationPulseHeight = None

NoPlaintextOrFallback == ~plaintextPath /\ ~fallbackPath

CertificateBindsApprovedResult ==
    /\ attemptStatus \in CertificateStates =>
          /\ ballotState = "Approved"
          /\ ballotApproved
          /\ findingState = "Approved"
          /\ certifiedAtHeight # None
          /\ enactAtHeight = certifiedAtHeight + EnactDelay
          /\ certificateHead = ExpectedHead
          /\ certificateFindingRoot = findingResultRoot
          /\ certificateFindingEndorsementRoot = findingEndorsementRoot
          /\ certificateFindingEndorsingAssignments =
                findingEndorsingAssignments
          /\ certificateFindingEndorsementCount = findingEndorsementCount
          /\ certificateFindingQuorum = PublicFindingQuorum
    /\ attemptStatus \notin CertificateStates =>
          /\ certificateFindingRoot = None
          /\ certificateFindingEndorsementRoot = None
          /\ certificateFindingEndorsingAssignments = <<>>
          /\ certificateFindingEndorsementCount = 0
          /\ certificateFindingQuorum = 0

ExactHeightCasEnactment ==
    /\ attemptStatus = "Enacted" =>
        /\ effectApplied
        /\ observedHead = certificateHead
        /\ terminalHeight = enactAtHeight
    /\ attemptStatus = "Superseded" =>
        /\ ~effectApplied
        /\ observedHead # certificateHead
        /\ terminalHeight = enactAtHeight
    /\ attemptStatus = "ExecutionFailed" =>
        /\ ~effectApplied
        /\ observedHead = certificateHead
        /\ terminalHeight = enactAtHeight

CertifiedCannotPassDueHeight ==
    attemptStatus # "Certified" \/ height <= enactAtHeight

NoResultTerminalization ==
    /\ ballotState = "NoResult" =>
          attemptStatus =
              IF (policyRequiresConfirmation /\
                    eligibleConfirmationCandidates \in 0..1) \/
                    ballotSequence = MaxRetries \/
                    ProposalRandomnessRedrawsUsed = MaxRandomnessRedraws
              THEN "Rejected"
              ELSE "Active"
    /\ findingState = "NoResult" => attemptStatus = "Rejected"

=============================================================================
