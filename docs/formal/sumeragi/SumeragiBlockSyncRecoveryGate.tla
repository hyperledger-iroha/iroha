---- MODULE SumeragiBlockSyncRecoveryGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for BlockSyncUpdate/BlockCreated recovery admission.

This slice models the adapter-side gate formed by `handle_block_sync_update`
and the BlockCreated recovery modes. It focuses on the recovery decisions that
matter for restarted-peer catch-up and certified frontier repair:
stale-view updates are accepted only with a request or commit evidence,
payload-only recovery cannot steal authoritative frontier ownership, certified
commit-QC recovery can supersede stale same-height work and clear stale
inflight commit work, aborted placeholders revive only with commit-QC evidence,
sparse next-height payloads track missing commit-QC repair, and unvalidated
commit-QC sidecars do not promote lock/highest-QC state.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptStaleWithoutRequest,
  \* @type: Bool;
  BugDropRequestedStale,
  \* @type: Bool;
  BugAcceptFutureUnrequested,
  \* @type: Bool;
  BugReviveAbortedWithoutCommitQc,
  \* @type: Bool;
  BugKeepAbortedWithCommitQc,
  \* @type: Bool;
  BugSkipVoteBackedOwner,
  \* @type: Bool;
  BugStealOwnerWithPayloadOnly,
  \* @type: Bool;
  BugSkipCertifiedOwner,
  \* @type: Bool;
  BugActivateUncertifiedConflict,
  \* @type: Bool;
  BugDropCommitQcMarker,
  \* @type: Bool;
  BugSkipMissingCommitQcRequest,
  \* @type: Bool;
  BugKeepMissingRequest,
  \* @type: Bool;
  BugClearInflightForPayloadOnly,
  \* @type: Bool;
  BugKeepInflightForCertified,
  \* @type: Bool;
  BugPromoteUnvalidatedQc

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "requestedStalePayload",
  "staleNoRequest",
  "staleCommitVotes",
  "staleCommitQc",
  "abortedPayloadOnly",
  "abortedCommitQc",
  "sparseNextHeight",
  "unknownFrontierVoteOnly",
  "payloadOnlyStaleInflight",
  "certifiedStaleInflight",
  "sameHeightRawQuorumConflict",
  "sameHeightCertifiedConflict",
  "cachedCommitQcPayload",
  "unvalidatedCommitQc",
  "unrequestedFuture"
}

Stale(candidate) ==
  candidate \in {
    "requestedStalePayload",
    "staleNoRequest",
    "staleCommitVotes",
    "staleCommitQc"
  }

HasMissingRequest(candidate) ==
  candidate = "requestedStalePayload"

HasCommitVotes(candidate) ==
  candidate \in {"staleCommitVotes", "unknownFrontierVoteOnly"}

HasCommitQc(candidate) ==
  candidate \in {
    "staleCommitQc",
    "abortedCommitQc",
    "certifiedStaleInflight",
    "sameHeightCertifiedConflict",
    "cachedCommitQcPayload",
    "unvalidatedCommitQc"
  }

ValidatedCommitQc(candidate) ==
  HasCommitQc(candidate) /\ candidate # "unvalidatedCommitQc"

PayloadOnly(candidate) ==
  candidate \in {"abortedPayloadOnly", "payloadOnlyStaleInflight"}

AbortedPlaceholder(candidate) ==
  candidate \in {"abortedPayloadOnly", "abortedCommitQc"}

CertifiedRecovery(candidate) ==
  candidate \in {
    "certifiedStaleInflight",
    "sameHeightCertifiedConflict",
    "cachedCommitQcPayload"
  }

SpecAccepts(candidate) ==
  candidate \notin {"staleNoRequest", "unrequestedFuture"}

SpecDrops(candidate) ==
  ~SpecAccepts(candidate)

SpecRevivesAborted(candidate) ==
  candidate = "abortedCommitQc"

SpecActiveOwner(candidate) ==
  candidate \in {
    "staleCommitVotes",
    "staleCommitQc",
    "abortedCommitQc",
    "certifiedStaleInflight",
    "sameHeightCertifiedConflict",
    "cachedCommitQcPayload"
  }

SpecPassiveRetained(candidate) ==
  candidate \in {"payloadOnlyStaleInflight", "sameHeightRawQuorumConflict"}

SpecCommitQcMarked(candidate) ==
  candidate \in {
    "staleCommitQc",
    "abortedCommitQc",
    "certifiedStaleInflight",
    "sameHeightCertifiedConflict",
    "cachedCommitQcPayload"
  }

SpecMissingCommitQcRequest(candidate) ==
  candidate \in {"sparseNextHeight", "unknownFrontierVoteOnly"}

SpecClearsMissingRequest(candidate) ==
  candidate \in {"requestedStalePayload", "sameHeightCertifiedConflict"}

SpecClearsStaleInflight(candidate) ==
  candidate = "certifiedStaleInflight"

BugDropsAccepted(candidate) ==
  \/ /\ candidate = "requestedStalePayload"
     /\ BugDropRequestedStale
  \/ /\ candidate = "certifiedStaleInflight"
     /\ BugKeepInflightForCertified

BugAcceptsDropped(candidate) ==
  \/ /\ candidate = "staleNoRequest"
     /\ BugAcceptStaleWithoutRequest
  \/ /\ candidate = "unrequestedFuture"
     /\ BugAcceptFutureUnrequested

ImplementationAccepts(candidate) ==
  IF SpecAccepts(candidate)
  THEN ~BugDropsAccepted(candidate)
  ELSE BugAcceptsDropped(candidate)

ImplementationRevivesAborted(candidate) ==
  \/ /\ ImplementationAccepts(candidate)
     /\ SpecRevivesAborted(candidate)
     /\ ~BugKeepAbortedWithCommitQc
  \/ /\ ImplementationAccepts(candidate)
     /\ candidate = "abortedPayloadOnly"
     /\ BugReviveAbortedWithoutCommitQc

ImplementationActiveOwner(candidate) ==
  IF ImplementationAccepts(candidate)
  THEN
    \/ /\ SpecActiveOwner(candidate)
       /\ ~(
            \/ /\ candidate = "staleCommitVotes"
               /\ BugSkipVoteBackedOwner
            \/ /\ CertifiedRecovery(candidate)
               /\ BugSkipCertifiedOwner
          )
    \/ /\ candidate = "payloadOnlyStaleInflight"
       /\ BugStealOwnerWithPayloadOnly
    \/ /\ candidate = "sameHeightRawQuorumConflict"
       /\ BugActivateUncertifiedConflict
  ELSE FALSE

ImplementationPassiveRetained(candidate) ==
  /\ ImplementationAccepts(candidate)
  /\ \/ /\ SpecPassiveRetained(candidate)
        /\ ~ImplementationActiveOwner(candidate)
     \/ /\ candidate = "sameHeightCertifiedConflict"
        /\ BugSkipCertifiedOwner

ImplementationCommitQcMarked(candidate) ==
  IF SpecCommitQcMarked(candidate)
  THEN /\ ImplementationAccepts(candidate)
       /\ ~BugDropCommitQcMarker
  ELSE FALSE

ImplementationMissingCommitQcRequest(candidate) ==
  IF SpecMissingCommitQcRequest(candidate)
  THEN /\ ImplementationAccepts(candidate)
       /\ ~BugSkipMissingCommitQcRequest
  ELSE FALSE

ImplementationClearsMissingRequest(candidate) ==
  IF SpecClearsMissingRequest(candidate)
  THEN /\ ImplementationAccepts(candidate)
       /\ ~BugKeepMissingRequest
  ELSE FALSE

ImplementationClearsStaleInflight(candidate) ==
  IF SpecClearsStaleInflight(candidate)
  THEN /\ ImplementationAccepts(candidate)
       /\ ~BugKeepInflightForCertified
  ELSE /\ candidate = "payloadOnlyStaleInflight"
       /\ ImplementationAccepts(candidate)
       /\ BugClearInflightForPayloadOnly

ImplementationPromotesUnvalidatedQc(candidate) ==
  /\ candidate = "unvalidatedCommitQc"
  /\ ImplementationAccepts(candidate)
  /\ BugPromoteUnvalidatedQc

TypeInvariant ==
  /\ BugAcceptStaleWithoutRequest \in BOOLEAN
  /\ BugDropRequestedStale \in BOOLEAN
  /\ BugAcceptFutureUnrequested \in BOOLEAN
  /\ BugReviveAbortedWithoutCommitQc \in BOOLEAN
  /\ BugKeepAbortedWithCommitQc \in BOOLEAN
  /\ BugSkipVoteBackedOwner \in BOOLEAN
  /\ BugStealOwnerWithPayloadOnly \in BOOLEAN
  /\ BugSkipCertifiedOwner \in BOOLEAN
  /\ BugActivateUncertifiedConflict \in BOOLEAN
  /\ BugDropCommitQcMarker \in BOOLEAN
  /\ BugSkipMissingCommitQcRequest \in BOOLEAN
  /\ BugKeepMissingRequest \in BOOLEAN
  /\ BugClearInflightForPayloadOnly \in BOOLEAN
  /\ BugKeepInflightForCertified \in BOOLEAN
  /\ BugPromoteUnvalidatedQc \in BOOLEAN
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

AcceptedMatchesSpec ==
  \A candidate \in tried:
    ImplementationAccepts(candidate) <=> SpecAccepts(candidate)

DroppedMatchesSpec ==
  \A candidate \in tried:
    ~ImplementationAccepts(candidate) <=> SpecDrops(candidate)

StaleWithoutRequestDrops ==
  "staleNoRequest" \in tried =>
    ~ImplementationAccepts("staleNoRequest")

RequestedStalePayloadAcceptedAndClearsRequest ==
  "requestedStalePayload" \in tried =>
    /\ ImplementationAccepts("requestedStalePayload")
    /\ ImplementationClearsMissingRequest("requestedStalePayload")

UnrequestedFutureDrops ==
  "unrequestedFuture" \in tried =>
    ~ImplementationAccepts("unrequestedFuture")

AbortedPayloadOnlyDoesNotRevive ==
  "abortedPayloadOnly" \in tried =>
    /\ ImplementationAccepts("abortedPayloadOnly")
    /\ ~ImplementationRevivesAborted("abortedPayloadOnly")
    /\ ~ImplementationActiveOwner("abortedPayloadOnly")

AbortedCommitQcRevivesAndKeepsQc ==
  "abortedCommitQc" \in tried =>
    /\ ImplementationAccepts("abortedCommitQc")
    /\ ImplementationRevivesAborted("abortedCommitQc")
    /\ ImplementationActiveOwner("abortedCommitQc")
    /\ ImplementationCommitQcMarked("abortedCommitQc")

VoteBackedStaleRecoveryIsAuthoritative ==
  "staleCommitVotes" \in tried =>
    /\ ImplementationAccepts("staleCommitVotes")
    /\ ImplementationActiveOwner("staleCommitVotes")

CertifiedRecoveryIsAuthoritative ==
  \A candidate \in tried:
    CertifiedRecovery(candidate) =>
      /\ ImplementationAccepts(candidate)
      /\ ImplementationActiveOwner(candidate)

PayloadOnlyDoesNotStealOwner ==
  "payloadOnlyStaleInflight" \in tried =>
    /\ ImplementationAccepts("payloadOnlyStaleInflight")
    /\ ~ImplementationActiveOwner("payloadOnlyStaleInflight")
    /\ ImplementationPassiveRetained("payloadOnlyStaleInflight")

UncertifiedSameHeightConflictStaysPassive ==
  "sameHeightRawQuorumConflict" \in tried =>
    /\ ImplementationAccepts("sameHeightRawQuorumConflict")
    /\ ~ImplementationActiveOwner("sameHeightRawQuorumConflict")
    /\ ImplementationPassiveRetained("sameHeightRawQuorumConflict")

CommitQcEvidenceIsRetained ==
  \A candidate \in tried:
    SpecCommitQcMarked(candidate) => ImplementationCommitQcMarked(candidate)

SparsePayloadTracksMissingCommitQc ==
  "sparseNextHeight" \in tried =>
    /\ ImplementationAccepts("sparseNextHeight")
    /\ ImplementationMissingCommitQcRequest("sparseNextHeight")

UnknownVoteOnlyTracksMissingCommitQc ==
  "unknownFrontierVoteOnly" \in tried =>
    /\ ImplementationAccepts("unknownFrontierVoteOnly")
    /\ ImplementationMissingCommitQcRequest("unknownFrontierVoteOnly")
    /\ ~ImplementationActiveOwner("unknownFrontierVoteOnly")

CertifiedRecoveryClearsStaleInflight ==
  "certifiedStaleInflight" \in tried =>
    ImplementationClearsStaleInflight("certifiedStaleInflight")

PayloadOnlyDoesNotClearStaleInflight ==
  "payloadOnlyStaleInflight" \in tried =>
    ~ImplementationClearsStaleInflight("payloadOnlyStaleInflight")

UnvalidatedCommitQcDoesNotPromote ==
  "unvalidatedCommitQc" \in tried =>
    ~ImplementationPromotesUnvalidatedQc("unvalidatedCommitQc")

BlockSyncRecoveryAdmissionExact ==
  /\ AcceptedMatchesSpec
  /\ DroppedMatchesSpec
  /\ StaleWithoutRequestDrops
  /\ RequestedStalePayloadAcceptedAndClearsRequest
  /\ UnrequestedFutureDrops

BlockSyncRecoveryAbortedExact ==
  /\ AbortedPayloadOnlyDoesNotRevive
  /\ AbortedCommitQcRevivesAndKeepsQc

BlockSyncRecoveryOwnerExact ==
  /\ VoteBackedStaleRecoveryIsAuthoritative
  /\ CertifiedRecoveryIsAuthoritative
  /\ PayloadOnlyDoesNotStealOwner
  /\ UncertifiedSameHeightConflictStaysPassive

BlockSyncRecoveryCommitQcRepairExact ==
  /\ CommitQcEvidenceIsRetained
  /\ SparsePayloadTracksMissingCommitQc
  /\ UnknownVoteOnlyTracksMissingCommitQc

BlockSyncRecoveryInflightAndValidationExact ==
  /\ CertifiedRecoveryClearsStaleInflight
  /\ PayloadOnlyDoesNotClearStaleInflight
  /\ UnvalidatedCommitQcDoesNotPromote

BlockSyncRecoveryExactness ==
  /\ BlockSyncRecoveryAdmissionExact
  /\ BlockSyncRecoveryAbortedExact
  /\ BlockSyncRecoveryOwnerExact
  /\ BlockSyncRecoveryCommitQcRepairExact
  /\ BlockSyncRecoveryInflightAndValidationExact

SafetyFast ==
  BlockSyncRecoveryExactness

====
