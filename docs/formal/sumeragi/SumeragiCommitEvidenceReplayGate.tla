---- MODULE SumeragiCommitEvidenceReplayGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for known-block commit-evidence replay.

This slice models the adapter-side gate formed by
`maybe_replay_known_block_commit_evidence(...)` and
`PendingBlock::should_replay_commit_evidence(...)`. Replay is allowed only for
an active pending block after the per-block cooldown, with remote targets and
with cached commit evidence to send. First evidence, vote-count progress,
commit-QC progress, view progress, and stalled positive evidence after cooldown
may replay. Inactive pending blocks, cooldown hits, zero-evidence states, and
local-only target sets must not emit network work. Vote evidence is sent as
`QcVote`, commit-QC evidence is sent as `CommitCert`, and neither path may fall
back to payload broadcasts or block-sync hydration.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugReplayInactive,
  \* @type: Bool;
  BugIgnoreCooldown,
  \* @type: Bool;
  BugReplayWithoutTargets,
  \* @type: Bool;
  BugSkipFirstEvidence,
  \* @type: Bool;
  BugSkipProgress,
  \* @type: Bool;
  BugSkipStalledRetry,
  \* @type: Bool;
  BugReplayNoEvidence,
  \* @type: Bool;
  BugVotesUsePayloadFallback,
  \* @type: Bool;
  BugCommitQcUsesVotes,
  \* @type: Bool;
  BugDropCommitQcReplay,
  \* @type: Bool;
  BugUseLocalTargets,
  \* @type: Bool;
  BugUseDuplicateTargets

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "missingPending",
  "wrongRound",
  "abortedPending",
  "cooldownVotes",
  "firstVotesRemote",
  "firstCommitQcRemote",
  "firstNoEvidenceRemote",
  "sameZeroNoProgress",
  "stalledVotesRemote",
  "stalledCommitQcRemote",
  "voteCountProgressRemote",
  "commitQcProgressRemote",
  "viewProgressRemote",
  "localOnlyVoteTargets",
  "duplicateVoteTargets",
  "localOnlyCommitQcTargets"
}

Inactive(candidate) ==
  candidate \in {"missingPending", "wrongRound", "abortedPending"}

CooldownActive(candidate) ==
  candidate = "cooldownVotes"

HasCommitVotes(candidate) ==
  candidate \in {
    "cooldownVotes",
    "firstVotesRemote",
    "stalledVotesRemote",
    "voteCountProgressRemote",
    "viewProgressRemote",
    "localOnlyVoteTargets",
    "duplicateVoteTargets"
  }

HasCommitQc(candidate) ==
  candidate \in {
    "firstCommitQcRemote",
    "stalledCommitQcRemote",
    "commitQcProgressRemote",
    "localOnlyCommitQcTargets"
  }

HasEvidence(candidate) ==
  HasCommitVotes(candidate) \/ HasCommitQc(candidate)

FirstEvidence(candidate) ==
  candidate \in {"firstVotesRemote", "firstCommitQcRemote"}

ProgressedEvidence(candidate) ==
  candidate \in {
    "voteCountProgressRemote",
    "commitQcProgressRemote",
    "viewProgressRemote"
  }

StalledPositiveEvidence(candidate) ==
  candidate \in {"stalledVotesRemote", "stalledCommitQcRemote"}

NoEvidence(candidate) ==
  candidate \in {"firstNoEvidenceRemote", "sameZeroNoProgress"}

LocalOnlyTargets(candidate) ==
  candidate \in {"localOnlyVoteTargets", "localOnlyCommitQcTargets"}

DuplicateTargets(candidate) ==
  candidate = "duplicateVoteTargets"

RemoteTargets(candidate) ==
  ~LocalOnlyTargets(candidate)

SpecReplays(candidate) ==
  /\ ~Inactive(candidate)
  /\ ~CooldownActive(candidate)
  /\ RemoteTargets(candidate)
  /\ HasEvidence(candidate)
  /\ FirstEvidence(candidate)
     \/ ProgressedEvidence(candidate)
     \/ StalledPositiveEvidence(candidate)
     \/ DuplicateTargets(candidate)

BugSkipsSpecReplay(candidate) ==
  \/ /\ FirstEvidence(candidate)
     /\ BugSkipFirstEvidence
  \/ /\ ProgressedEvidence(candidate)
     /\ BugSkipProgress
  \/ /\ StalledPositiveEvidence(candidate)
     /\ BugSkipStalledRetry
  \/ /\ HasCommitQc(candidate)
     /\ BugDropCommitQcReplay

BugAllowsUnsafeReplay(candidate) ==
  \/ /\ Inactive(candidate)
     /\ BugReplayInactive
  \/ /\ CooldownActive(candidate)
     /\ BugIgnoreCooldown
  \/ /\ LocalOnlyTargets(candidate)
     /\ BugReplayWithoutTargets
  \/ /\ NoEvidence(candidate)
     /\ BugReplayNoEvidence

ImplementationReplays(candidate) ==
  IF SpecReplays(candidate)
  THEN ~BugSkipsSpecReplay(candidate)
  ELSE BugAllowsUnsafeReplay(candidate)

SendsVoteEvidence(candidate) ==
  /\ ImplementationReplays(candidate)
  /\ \/ /\ HasCommitVotes(candidate)
        /\ ~BugVotesUsePayloadFallback
     \/ /\ HasCommitQc(candidate)
        /\ BugCommitQcUsesVotes

SendsCommitCert(candidate) ==
  /\ ImplementationReplays(candidate)
  /\ HasCommitQc(candidate)
  /\ ~BugCommitQcUsesVotes
  /\ ~BugDropCommitQcReplay

UsesPayloadFallback(candidate) ==
  /\ ImplementationReplays(candidate)
  /\ \/ /\ HasCommitVotes(candidate)
        /\ BugVotesUsePayloadFallback
     \/ /\ HasCommitQc(candidate)
        /\ BugVotesUsePayloadFallback

TargetsAreRemoteOnly(candidate) ==
  /\ ImplementationReplays(candidate)
  /\ ~BugUseLocalTargets

TargetsAreDeduped(candidate) ==
  /\ ImplementationReplays(candidate)
  /\ DuplicateTargets(candidate)
  /\ ~BugUseDuplicateTargets

TypeInvariant ==
  /\ BugReplayInactive \in BOOLEAN
  /\ BugIgnoreCooldown \in BOOLEAN
  /\ BugReplayWithoutTargets \in BOOLEAN
  /\ BugSkipFirstEvidence \in BOOLEAN
  /\ BugSkipProgress \in BOOLEAN
  /\ BugSkipStalledRetry \in BOOLEAN
  /\ BugReplayNoEvidence \in BOOLEAN
  /\ BugVotesUsePayloadFallback \in BOOLEAN
  /\ BugCommitQcUsesVotes \in BOOLEAN
  /\ BugDropCommitQcReplay \in BOOLEAN
  /\ BugUseLocalTargets \in BOOLEAN
  /\ BugUseDuplicateTargets \in BOOLEAN
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

ReplayMatchesSpec ==
  \A candidate \in tried:
    ImplementationReplays(candidate) <=> SpecReplays(candidate)

InactivePendingNeverReplays ==
  \A candidate \in tried:
    Inactive(candidate) => ~ImplementationReplays(candidate)

CooldownSuppressesReplay ==
  "cooldownVotes" \in tried =>
    ~ImplementationReplays("cooldownVotes")

NoEvidenceNeverReplays ==
  \A candidate \in tried:
    NoEvidence(candidate) => ~ImplementationReplays(candidate)

RemoteTargetsRequired ==
  \A candidate \in tried:
    ImplementationReplays(candidate) => RemoteTargets(candidate)

FirstEvidenceReplays ==
  \A candidate \in tried:
    FirstEvidence(candidate) => ImplementationReplays(candidate)

ProgressReplays ==
  \A candidate \in tried:
    ProgressedEvidence(candidate) => ImplementationReplays(candidate)

StalledPositiveEvidenceRetries ==
  \A candidate \in tried:
    StalledPositiveEvidence(candidate) => ImplementationReplays(candidate)

VoteEvidenceUsesVoteReplay ==
  \A candidate \in tried:
    /\ ImplementationReplays(candidate)
    /\ HasCommitVotes(candidate)
    /\ ~HasCommitQc(candidate)
    =>
      /\ SendsVoteEvidence(candidate)
      /\ ~SendsCommitCert(candidate)
      /\ ~UsesPayloadFallback(candidate)

CommitQcUsesCommitCertReplay ==
  \A candidate \in tried:
    /\ ImplementationReplays(candidate)
    /\ HasCommitQc(candidate)
    =>
      /\ SendsCommitCert(candidate)
      /\ ~SendsVoteEvidence(candidate)
      /\ ~UsesPayloadFallback(candidate)

PayloadFallbackNeverUsed ==
  \A candidate \in tried:
    ~UsesPayloadFallback(candidate)

ReplayTargetsExcludeLocal ==
  \A candidate \in tried:
    ImplementationReplays(candidate) => TargetsAreRemoteOnly(candidate)

DuplicateExplicitTargetsAreDeduped ==
  "duplicateVoteTargets" \in tried =>
    /\ ImplementationReplays("duplicateVoteTargets")
    /\ TargetsAreDeduped("duplicateVoteTargets")

CommitEvidenceReplayAdmissionExact ==
  /\ ReplayMatchesSpec
  /\ InactivePendingNeverReplays
  /\ CooldownSuppressesReplay
  /\ NoEvidenceNeverReplays
  /\ RemoteTargetsRequired

CommitEvidenceReplayProgressExact ==
  /\ FirstEvidenceReplays
  /\ ProgressReplays
  /\ StalledPositiveEvidenceRetries

CommitEvidenceReplayKindExact ==
  /\ VoteEvidenceUsesVoteReplay
  /\ CommitQcUsesCommitCertReplay
  /\ PayloadFallbackNeverUsed

CommitEvidenceReplayTargetExact ==
  /\ ReplayTargetsExcludeLocal
  /\ DuplicateExplicitTargetsAreDeduped

CommitEvidenceReplayExactness ==
  /\ CommitEvidenceReplayAdmissionExact
  /\ CommitEvidenceReplayProgressExact
  /\ CommitEvidenceReplayKindExact
  /\ CommitEvidenceReplayTargetExact

CommitEvidenceReplayCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CommitEvidenceReplayExactness

SafetyFast ==
  CommitEvidenceReplayExactness

====
