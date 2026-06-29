---- MODULE SumeragiNativeAmxAttestationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for native AMX proposer-side attestation gating.

This slice models the decision boundary formed by
`native_amx_receipt_for_plan(...)`, `NativeAmxSessionCache`, and
`aggregate_votes_to_qc(...)`. Native AMX proposal assembly may seal a receipt
only after every participant leg has both prepare and commit QCs. Missing
prepare quorum schedules prepare requests; missing commit quorum after prepare
quorum schedules commit requests. Invalid vote sets fail closed, vote
projection is deterministic in validator-set order, and the cache keeps retried
bodies and distinct participant legs separate.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSealNonNativePlan,
  \* @type: Bool;
  BugSealEmptyRoster,
  \* @type: Bool;
  BugSkipPrepareRequest,
  \* @type: Bool;
  BugSkipCommitRequest,
  \* @type: Bool;
  BugRequestCommitBeforePrepare,
  \* @type: Bool;
  BugRetryPrepareAfterQuorum,
  \* @type: Bool;
  BugSealWithPrepareOnly,
  \* @type: Bool;
  BugSealWithCommitOnly,
  \* @type: Bool;
  BugSealPartialMultiLeg,
  \* @type: Bool;
  BugAcceptDuplicatePrepare,
  \* @type: Bool;
  BugAcceptDuplicateCommit,
  \* @type: Bool;
  BugAcceptWrongPrepareBody,
  \* @type: Bool;
  BugAcceptWrongCommitBody,
  \* @type: Bool;
  BugAcceptOutsiderSigner,
  \* @type: Bool;
  BugUseArrivalOrderBitmap,
  \* @type: Bool;
  BugCollapseRetryBodies,
  \* @type: Bool;
  BugCollapseParticipantLegs

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

Candidates == {
  "nonNativePlan",
  "emptyRoster",
  "noPrepareVotes",
  "prepareBelowQuorum",
  "prepareQuorumNoCommitVotes",
  "prepareQuorumCommitBelowQuorum",
  "fullQuorumSingleLeg",
  "fullQuorumMultiLeg",
  "oneLegPendingMultiLeg",
  "commitWithoutPrepare",
  "duplicatePrepareSigner",
  "duplicateCommitSigner",
  "wrongPrepareBody",
  "wrongCommitBody",
  "outsiderPrepareSigner",
  "outsiderCommitSigner",
  "unsortedQuorumVotes",
  "retriedHeightSameSigner",
  "differentParticipantSameSigner"
}

InvalidVoteSet(candidate) ==
  candidate \in {
    "duplicatePrepareSigner",
    "duplicateCommitSigner",
    "wrongPrepareBody",
    "wrongCommitBody",
    "outsiderPrepareSigner",
    "outsiderCommitSigner"
  }

NativePlan(candidate) ==
  candidate # "nonNativePlan"

HasPrepareQuorum(candidate) ==
  candidate \in {
    "prepareQuorumNoCommitVotes",
    "prepareQuorumCommitBelowQuorum",
    "fullQuorumSingleLeg",
    "fullQuorumMultiLeg",
    "oneLegPendingMultiLeg",
    "duplicateCommitSigner",
    "wrongCommitBody",
    "outsiderCommitSigner",
    "unsortedQuorumVotes",
    "retriedHeightSameSigner",
    "differentParticipantSameSigner"
  }

HasCommitQuorum(candidate) ==
  candidate \in {
    "fullQuorumSingleLeg",
    "fullQuorumMultiLeg",
    "duplicatePrepareSigner",
    "duplicateCommitSigner",
    "wrongPrepareBody",
    "wrongCommitBody",
    "outsiderPrepareSigner",
    "outsiderCommitSigner",
    "unsortedQuorumVotes",
    "retriedHeightSameSigner",
    "differentParticipantSameSigner"
  }

SpecReceipt(candidate) ==
  candidate \in {
    "fullQuorumSingleLeg",
    "fullQuorumMultiLeg",
    "unsortedQuorumVotes",
    "retriedHeightSameSigner",
    "differentParticipantSameSigner"
  }

SpecPrepareRequest(candidate) ==
  candidate \in {"noPrepareVotes", "prepareBelowQuorum", "commitWithoutPrepare"}

SpecCommitRequest(candidate) ==
  candidate \in {"prepareQuorumNoCommitVotes", "prepareQuorumCommitBelowQuorum"}

SpecPending(candidate) ==
  SpecPrepareRequest(candidate)
    \/ SpecCommitRequest(candidate)
    \/ candidate = "oneLegPendingMultiLeg"

SpecPrepareQc(candidate) ==
  SpecReceipt(candidate)

SpecCommitQc(candidate) ==
  SpecReceipt(candidate)

SpecDeterministicBitmap(candidate) ==
  candidate = "unsortedQuorumVotes"

SpecRetryBodiesSeparate(candidate) ==
  candidate = "retriedHeightSameSigner"

SpecParticipantLegsSeparate(candidate) ==
  candidate = "differentParticipantSameSigner"

ImplementationPrepareRequest(candidate) ==
  IF SpecPrepareRequest(candidate)
  THEN ~BugSkipPrepareRequest
  ELSE FALSE

ImplementationCommitRequest(candidate) ==
  IF SpecCommitRequest(candidate)
  THEN ~BugSkipCommitRequest
  ELSE /\ candidate = "commitWithoutPrepare"
       /\ BugRequestCommitBeforePrepare

ImplementationPrepareRetriedAfterQuorum(candidate) ==
  /\ HasPrepareQuorum(candidate)
  /\ BugRetryPrepareAfterQuorum

ImplementationPrepareQc(candidate) ==
  \/ SpecPrepareQc(candidate)
  \/ /\ candidate \in {"prepareQuorumNoCommitVotes", "prepareQuorumCommitBelowQuorum"}
     /\ BugSealWithPrepareOnly
  \/ /\ candidate = "oneLegPendingMultiLeg"
     /\ BugSealPartialMultiLeg
  \/ /\ candidate \in {"duplicatePrepareSigner", "duplicateCommitSigner"}
     /\ (BugAcceptDuplicatePrepare \/ BugAcceptDuplicateCommit)
  \/ /\ candidate \in {"wrongPrepareBody", "wrongCommitBody"}
     /\ (BugAcceptWrongPrepareBody \/ BugAcceptWrongCommitBody)
  \/ /\ candidate \in {"outsiderPrepareSigner", "outsiderCommitSigner"}
     /\ BugAcceptOutsiderSigner

ImplementationCommitQc(candidate) ==
  \/ SpecCommitQc(candidate)
  \/ /\ candidate = "commitWithoutPrepare"
     /\ BugSealWithCommitOnly
  \/ /\ candidate \in {"prepareQuorumNoCommitVotes", "prepareQuorumCommitBelowQuorum"}
     /\ BugSealWithPrepareOnly
  \/ /\ candidate = "oneLegPendingMultiLeg"
     /\ BugSealPartialMultiLeg
  \/ /\ candidate \in {"duplicatePrepareSigner", "duplicateCommitSigner"}
     /\ (BugAcceptDuplicatePrepare \/ BugAcceptDuplicateCommit)
  \/ /\ candidate \in {"wrongPrepareBody", "wrongCommitBody"}
     /\ (BugAcceptWrongPrepareBody \/ BugAcceptWrongCommitBody)
  \/ /\ candidate \in {"outsiderPrepareSigner", "outsiderCommitSigner"}
     /\ BugAcceptOutsiderSigner

ImplementationReceipt(candidate) ==
  \/ SpecReceipt(candidate)
  \/ /\ candidate = "nonNativePlan"
     /\ BugSealNonNativePlan
  \/ /\ candidate = "emptyRoster"
     /\ BugSealEmptyRoster
  \/ /\ candidate \in {"prepareQuorumNoCommitVotes", "prepareQuorumCommitBelowQuorum"}
     /\ BugSealWithPrepareOnly
  \/ /\ candidate = "commitWithoutPrepare"
     /\ BugSealWithCommitOnly
  \/ /\ candidate = "oneLegPendingMultiLeg"
     /\ BugSealPartialMultiLeg
  \/ /\ candidate \in {"duplicatePrepareSigner", "duplicateCommitSigner"}
     /\ (BugAcceptDuplicatePrepare \/ BugAcceptDuplicateCommit)
  \/ /\ candidate \in {"wrongPrepareBody", "wrongCommitBody"}
     /\ (BugAcceptWrongPrepareBody \/ BugAcceptWrongCommitBody)
  \/ /\ candidate \in {"outsiderPrepareSigner", "outsiderCommitSigner"}
     /\ BugAcceptOutsiderSigner

ImplementationPending(candidate) ==
  /\ SpecPending(candidate)
  /\ ~ImplementationReceipt(candidate)

ImplementationDeterministicBitmap(candidate) ==
  /\ SpecDeterministicBitmap(candidate)
  /\ ~BugUseArrivalOrderBitmap

ImplementationRetryBodiesSeparate(candidate) ==
  /\ SpecRetryBodiesSeparate(candidate)
  /\ ~BugCollapseRetryBodies

ImplementationParticipantLegsSeparate(candidate) ==
  /\ SpecParticipantLegsSeparate(candidate)
  /\ ~BugCollapseParticipantLegs

TypeInvariant ==
  /\ BugSealNonNativePlan \in BOOLEAN
  /\ BugSealEmptyRoster \in BOOLEAN
  /\ BugSkipPrepareRequest \in BOOLEAN
  /\ BugSkipCommitRequest \in BOOLEAN
  /\ BugRequestCommitBeforePrepare \in BOOLEAN
  /\ BugRetryPrepareAfterQuorum \in BOOLEAN
  /\ BugSealWithPrepareOnly \in BOOLEAN
  /\ BugSealWithCommitOnly \in BOOLEAN
  /\ BugSealPartialMultiLeg \in BOOLEAN
  /\ BugAcceptDuplicatePrepare \in BOOLEAN
  /\ BugAcceptDuplicateCommit \in BOOLEAN
  /\ BugAcceptWrongPrepareBody \in BOOLEAN
  /\ BugAcceptWrongCommitBody \in BOOLEAN
  /\ BugAcceptOutsiderSigner \in BOOLEAN
  /\ BugUseArrivalOrderBitmap \in BOOLEAN
  /\ BugCollapseRetryBodies \in BOOLEAN
  /\ BugCollapseParticipantLegs \in BOOLEAN
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

ReceiptsMatchSpec ==
  \A candidate \in tried:
    ImplementationReceipt(candidate) <=> SpecReceipt(candidate)

PrepareRequestsMatchSpec ==
  \A candidate \in tried:
    ImplementationPrepareRequest(candidate) <=> SpecPrepareRequest(candidate)

CommitRequestsMatchSpec ==
  \A candidate \in tried:
    ImplementationCommitRequest(candidate) <=> SpecCommitRequest(candidate)

ReceiptRequiresBothQcs ==
  \A candidate \in tried:
    ImplementationReceipt(candidate) =>
      /\ ImplementationPrepareQc(candidate)
      /\ ImplementationCommitQc(candidate)

NoCommitRequestBeforePrepareQuorum ==
  \A candidate \in tried:
    ~HasPrepareQuorum(candidate) => ~ImplementationCommitRequest(candidate)

NoPrepareRetryAfterPrepareQuorum ==
  \A candidate \in tried:
    ~ImplementationPrepareRetriedAfterQuorum(candidate)

PrepareRequestBeforePrepareQuorum ==
  \A candidate \in tried:
    SpecPrepareRequest(candidate) => ImplementationPrepareRequest(candidate)

CommitRequestAfterPrepareBeforeCommit ==
  \A candidate \in tried:
    SpecCommitRequest(candidate) => ImplementationCommitRequest(candidate)

NoPartialMultiLegReceipt ==
  "oneLegPendingMultiLeg" \in tried =>
    /\ ImplementationPending("oneLegPendingMultiLeg")
    /\ ~ImplementationReceipt("oneLegPendingMultiLeg")

InvalidVoteSetsFailClosed ==
  \A candidate \in tried:
    InvalidVoteSet(candidate) =>
      /\ ~ImplementationReceipt(candidate)
      /\ ~ImplementationPrepareQc(candidate)
      /\ ~ImplementationCommitQc(candidate)

NonNativePlanDoesNotEmitReceiptOrRequests ==
  "nonNativePlan" \in tried =>
    /\ ~ImplementationReceipt("nonNativePlan")
    /\ ~ImplementationPrepareRequest("nonNativePlan")
    /\ ~ImplementationCommitRequest("nonNativePlan")

EmptyRosterFailsClosed ==
  "emptyRoster" \in tried =>
    /\ ~ImplementationReceipt("emptyRoster")
    /\ ~ImplementationPrepareRequest("emptyRoster")
    /\ ~ImplementationCommitRequest("emptyRoster")

UnsortedVotesUseValidatorOrder ==
  "unsortedQuorumVotes" \in tried =>
    /\ ImplementationReceipt("unsortedQuorumVotes")
    /\ ImplementationDeterministicBitmap("unsortedQuorumVotes")

RetriedBodiesRemainSeparate ==
  "retriedHeightSameSigner" \in tried =>
    /\ ImplementationReceipt("retriedHeightSameSigner")
    /\ ImplementationRetryBodiesSeparate("retriedHeightSameSigner")

ParticipantLegsRemainSeparate ==
  "differentParticipantSameSigner" \in tried =>
    /\ ImplementationReceipt("differentParticipantSameSigner")
    /\ ImplementationParticipantLegsSeparate("differentParticipantSameSigner")

NativeAmxAdmissionCases == {
  "nonNativePlan", "emptyRoster"
}

NativeAmxRequestCases == {
  "noPrepareVotes", "prepareBelowQuorum", "prepareQuorumNoCommitVotes",
  "prepareQuorumCommitBelowQuorum", "commitWithoutPrepare"
}

NativeAmxReceiptCases == {
  "fullQuorumSingleLeg", "fullQuorumMultiLeg", "oneLegPendingMultiLeg"
}

NativeAmxInvalidVoteCases == {
  "duplicatePrepareSigner", "duplicateCommitSigner", "wrongPrepareBody",
  "wrongCommitBody", "outsiderPrepareSigner", "outsiderCommitSigner"
}

NativeAmxDeterminismCacheCases == {
  "unsortedQuorumVotes", "retriedHeightSameSigner",
  "differentParticipantSameSigner"
}

NativeAmxAttestationGroupedCases ==
  NativeAmxAdmissionCases \cup NativeAmxRequestCases \cup
  NativeAmxReceiptCases \cup NativeAmxInvalidVoteCases \cup
  NativeAmxDeterminismCacheCases

NativeAmxAttestationCaseGroupsComplete ==
  NativeAmxAttestationGroupedCases = Candidates

NativeAmxRequestExact ==
  /\ PrepareRequestsMatchSpec
  /\ CommitRequestsMatchSpec
  /\ NoCommitRequestBeforePrepareQuorum
  /\ NoPrepareRetryAfterPrepareQuorum
  /\ PrepareRequestBeforePrepareQuorum
  /\ CommitRequestAfterPrepareBeforeCommit

NativeAmxReceiptExact ==
  /\ ReceiptsMatchSpec
  /\ ReceiptRequiresBothQcs
  /\ NoPartialMultiLegReceipt

NativeAmxFailClosedExact ==
  /\ InvalidVoteSetsFailClosed
  /\ NonNativePlanDoesNotEmitReceiptOrRequests
  /\ EmptyRosterFailsClosed

NativeAmxDeterministicCacheExact ==
  /\ UnsortedVotesUseValidatorOrder
  /\ RetriedBodiesRemainSeparate
  /\ ParticipantLegsRemainSeparate

NativeAmxAttestationExactness ==
  /\ NativeAmxAttestationCaseGroupsComplete
  /\ NativeAmxRequestExact
  /\ NativeAmxReceiptExact
  /\ NativeAmxFailClosedExact
  /\ NativeAmxDeterministicCacheExact

NativeAmxAttestationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NativeAmxAttestationExactness

====
