---- MODULE SumeragiEnginePrepareVoteCacheGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for prepare-QC commit-vote cache/output fields.

This slice models the accepted branch in
`ConsensusEngine::on_prepare_qc(...)`. A safe current-context Prepare QC with
no existing commit vote for the round must insert exactly
`commit_votes[certificate.round] = certificate.subject` and emit exactly one
`SignVote { phase: Commit, round: certificate.round, subject:
certificate.subject, highest_qc: None }`.

Prepare QCs rejected by the shared certificate prefilter, by the
pending-finality phase, or by existing same-round commit-vote cache entries
must not emit another commit vote. Replay and conflicting same-round Prepare
QCs must preserve the existing cached subject.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipCacheInsert,
  \* @type: Bool;
  BugCacheWrongRound,
  \* @type: Bool;
  BugCacheWrongSubject,
  \* @type: Bool;
  BugSkipCommitVoteOutput,
  \* @type: Bool;
  BugOutputWrongPhase,
  \* @type: Bool;
  BugOutputWrongRound,
  \* @type: Bool;
  BugOutputWrongSubject,
  \* @type: Bool;
  BugOutputCarriesHighestQc,
  \* @type: Bool;
  BugCacheOnRejected,
  \* @type: Bool;
  BugOutputOnRejected,
  \* @type: Bool;
  BugOverwriteConflict,
  \* @type: Bool;
  BugOutputReplayConflict,
  \* @type: Bool;
  BugClearExistingOnReplayConflict

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_prepare",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "pending_finality",
  "replay_same_prepare",
  "conflicting_prepare"
}

RejectedCases == {
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "pending_finality"
}

ReplayConflictCases == {"replay_same_prepare", "conflicting_prepare"}

Values == {
  "none",
  "round_current",
  "round_other",
  "subject_a",
  "subject_b",
  "phase_commit",
  "phase_prepare",
  "highest_none",
  "highest_some"
}

CertificateRound(candidate) ==
  IF candidate \in {"wrong_height", "wrong_epoch", "wrong_validator_set", "stale_view"}
  THEN "round_other"
  ELSE "round_current"

CertificateSubject(candidate) ==
  IF candidate = "conflicting_prepare"
  THEN "subject_b"
  ELSE "subject_a"

InitialCacheKey(candidate) ==
  IF candidate \in ReplayConflictCases THEN "round_current" ELSE "none"

InitialCacheSubject(candidate) ==
  IF candidate \in ReplayConflictCases THEN "subject_a" ELSE "none"

SafeAccepted(candidate) ==
  candidate = "safe_prepare"

SpecCacheKey(candidate) ==
  IF SafeAccepted(candidate)
  THEN "round_current"
  ELSE InitialCacheKey(candidate)

SpecCacheSubject(candidate) ==
  IF SafeAccepted(candidate)
  THEN "subject_a"
  ELSE InitialCacheSubject(candidate)

SpecOutputPhase(candidate) ==
  IF SafeAccepted(candidate) THEN "phase_commit" ELSE "none"

SpecOutputRound(candidate) ==
  IF SafeAccepted(candidate) THEN "round_current" ELSE "none"

SpecOutputSubject(candidate) ==
  IF SafeAccepted(candidate) THEN "subject_a" ELSE "none"

SpecOutputHighest(candidate) ==
  IF SafeAccepted(candidate) THEN "highest_none" ELSE "none"

WrongSubject(subject) ==
  IF subject = "subject_a" THEN "subject_b" ELSE "subject_a"

ImplementationCacheRejected(candidate) ==
  /\ candidate \in RejectedCases
  /\ BugCacheOnRejected

ImplementationOutputRejected(candidate) ==
  /\ candidate \in RejectedCases
  /\ BugOutputOnRejected

ImplementationCacheKey(candidate) ==
  IF SafeAccepted(candidate)
  THEN
    IF BugSkipCacheInsert
    THEN InitialCacheKey(candidate)
    ELSE IF BugCacheWrongRound
         THEN "round_other"
         ELSE CertificateRound(candidate)
  ELSE IF candidate = "replay_same_prepare"
       THEN
         IF BugClearExistingOnReplayConflict
         THEN "none"
         ELSE InitialCacheKey(candidate)
       ELSE IF candidate = "conflicting_prepare"
            THEN
              IF BugClearExistingOnReplayConflict
              THEN "none"
              ELSE IF BugOverwriteConflict
                   THEN CertificateRound(candidate)
                   ELSE InitialCacheKey(candidate)
            ELSE IF ImplementationCacheRejected(candidate)
                 THEN CertificateRound(candidate)
                 ELSE InitialCacheKey(candidate)

ImplementationCacheSubject(candidate) ==
  IF SafeAccepted(candidate)
  THEN
    IF BugSkipCacheInsert
    THEN InitialCacheSubject(candidate)
    ELSE IF BugCacheWrongSubject
         THEN WrongSubject(CertificateSubject(candidate))
         ELSE CertificateSubject(candidate)
  ELSE IF candidate = "replay_same_prepare"
       THEN
         IF BugClearExistingOnReplayConflict
         THEN "none"
         ELSE InitialCacheSubject(candidate)
       ELSE IF candidate = "conflicting_prepare"
            THEN
              IF BugClearExistingOnReplayConflict
              THEN "none"
              ELSE IF BugOverwriteConflict
                   THEN CertificateSubject(candidate)
                   ELSE InitialCacheSubject(candidate)
            ELSE IF ImplementationCacheRejected(candidate)
                 THEN CertificateSubject(candidate)
                 ELSE InitialCacheSubject(candidate)

ImplementationOutputPhase(candidate) ==
  IF SafeAccepted(candidate)
  THEN
    IF BugSkipCommitVoteOutput
    THEN "none"
    ELSE IF BugOutputWrongPhase
         THEN "phase_prepare"
         ELSE "phase_commit"
  ELSE IF ImplementationOutputRejected(candidate) \/ (candidate \in ReplayConflictCases /\ BugOutputReplayConflict)
       THEN "phase_commit"
       ELSE "none"

ImplementationOutputRound(candidate) ==
  IF SafeAccepted(candidate)
  THEN
    IF BugSkipCommitVoteOutput
    THEN "none"
    ELSE IF BugOutputWrongRound
         THEN "round_other"
         ELSE CertificateRound(candidate)
  ELSE IF ImplementationOutputRejected(candidate) \/ (candidate \in ReplayConflictCases /\ BugOutputReplayConflict)
       THEN CertificateRound(candidate)
       ELSE "none"

ImplementationOutputSubject(candidate) ==
  IF SafeAccepted(candidate)
  THEN
    IF BugSkipCommitVoteOutput
    THEN "none"
    ELSE IF BugOutputWrongSubject
         THEN WrongSubject(CertificateSubject(candidate))
         ELSE CertificateSubject(candidate)
  ELSE IF ImplementationOutputRejected(candidate) \/ (candidate \in ReplayConflictCases /\ BugOutputReplayConflict)
       THEN CertificateSubject(candidate)
       ELSE "none"

ImplementationOutputHighest(candidate) ==
  IF SafeAccepted(candidate)
  THEN
    IF BugSkipCommitVoteOutput
    THEN "none"
    ELSE IF BugOutputCarriesHighestQc
         THEN "highest_some"
         ELSE "highest_none"
  ELSE IF ImplementationOutputRejected(candidate) \/ (candidate \in ReplayConflictCases /\ BugOutputReplayConflict)
       THEN "highest_none"
       ELSE "none"

TypeInvariant ==
  /\ BugSkipCacheInsert \in BOOLEAN
  /\ BugCacheWrongRound \in BOOLEAN
  /\ BugCacheWrongSubject \in BOOLEAN
  /\ BugSkipCommitVoteOutput \in BOOLEAN
  /\ BugOutputWrongPhase \in BOOLEAN
  /\ BugOutputWrongRound \in BOOLEAN
  /\ BugOutputWrongSubject \in BOOLEAN
  /\ BugOutputCarriesHighestQc \in BOOLEAN
  /\ BugCacheOnRejected \in BOOLEAN
  /\ BugOutputOnRejected \in BOOLEAN
  /\ BugOverwriteConflict \in BOOLEAN
  /\ BugOutputReplayConflict \in BOOLEAN
  /\ BugClearExistingOnReplayConflict \in BOOLEAN
  /\ tried \subseteq Cases

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

CacheKeyMatchesSpec ==
  \A candidate \in tried:
    ImplementationCacheKey(candidate) = SpecCacheKey(candidate)

CacheSubjectMatchesSpec ==
  \A candidate \in tried:
    ImplementationCacheSubject(candidate) = SpecCacheSubject(candidate)

OutputPhaseMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputPhase(candidate) = SpecOutputPhase(candidate)

OutputRoundMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputRound(candidate) = SpecOutputRound(candidate)

OutputSubjectMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputSubject(candidate) = SpecOutputSubject(candidate)

OutputHighestMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputHighest(candidate) = SpecOutputHighest(candidate)

SafePrepareCachesRoundSubject ==
  "safe_prepare" \in tried =>
    /\ ImplementationCacheKey("safe_prepare") = "round_current"
    /\ ImplementationCacheSubject("safe_prepare") = "subject_a"

SafePrepareEmitsExactCommitVote ==
  "safe_prepare" \in tried =>
    /\ ImplementationOutputPhase("safe_prepare") = "phase_commit"
    /\ ImplementationOutputRound("safe_prepare") = "round_current"
    /\ ImplementationOutputSubject("safe_prepare") = "subject_a"
    /\ ImplementationOutputHighest("safe_prepare") = "highest_none"

RejectedPrepareDoesNotCacheOrVote ==
  \A candidate \in tried:
    candidate \in RejectedCases =>
      /\ ImplementationCacheKey(candidate) = "none"
      /\ ImplementationCacheSubject(candidate) = "none"
      /\ ImplementationOutputPhase(candidate) = "none"
      /\ ImplementationOutputRound(candidate) = "none"
      /\ ImplementationOutputSubject(candidate) = "none"
      /\ ImplementationOutputHighest(candidate) = "none"

ReplayConflictPreservesCache ==
  \A candidate \in tried:
    candidate \in ReplayConflictCases =>
      /\ ImplementationCacheKey(candidate) = "round_current"
      /\ ImplementationCacheSubject(candidate) = "subject_a"

ReplayConflictDoesNotVote ==
  \A candidate \in tried:
    candidate \in ReplayConflictCases =>
      /\ ImplementationOutputPhase(candidate) = "none"
      /\ ImplementationOutputRound(candidate) = "none"
      /\ ImplementationOutputSubject(candidate) = "none"
      /\ ImplementationOutputHighest(candidate) = "none"

OutputOnlyForSafePrepare ==
  \A candidate \in tried:
    candidate # "safe_prepare" =>
      /\ ImplementationOutputPhase(candidate) = "none"
      /\ ImplementationOutputRound(candidate) = "none"
      /\ ImplementationOutputSubject(candidate) = "none"
      /\ ImplementationOutputHighest(candidate) = "none"

CacheNeverUsesWrongRoundForSafePrepare ==
  "safe_prepare" \in tried =>
    ImplementationCacheKey("safe_prepare") = CertificateRound("safe_prepare")

CacheNeverUsesWrongSubjectForSafePrepare ==
  "safe_prepare" \in tried =>
    ImplementationCacheSubject("safe_prepare") = CertificateSubject("safe_prepare")

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ ImplementationCacheKey(candidate) \in Values
    /\ ImplementationCacheSubject(candidate) \in Values
    /\ ImplementationOutputPhase(candidate) \in Values
    /\ ImplementationOutputRound(candidate) \in Values
    /\ ImplementationOutputSubject(candidate) \in Values
    /\ ImplementationOutputHighest(candidate) \in Values

EnginePrepareVoteCacheExactness ==
  /\ CacheKeyMatchesSpec
  /\ CacheSubjectMatchesSpec
  /\ OutputPhaseMatchesSpec
  /\ OutputRoundMatchesSpec
  /\ OutputSubjectMatchesSpec
  /\ OutputHighestMatchesSpec
  /\ SafePrepareCachesRoundSubject
  /\ SafePrepareEmitsExactCommitVote
  /\ RejectedPrepareDoesNotCacheOrVote
  /\ ReplayConflictPreservesCache
  /\ ReplayConflictDoesNotVote
  /\ OutputOnlyForSafePrepare
  /\ CacheNeverUsesWrongRoundForSafePrepare
  /\ CacheNeverUsesWrongSubjectForSafePrepare
  /\ ValuesStayInDomain

Safety ==
  EnginePrepareVoteCacheExactness

EnginePrepareVoteCacheCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EnginePrepareVoteCacheExactness

SafetyFast == EnginePrepareVoteCacheExactness

====
