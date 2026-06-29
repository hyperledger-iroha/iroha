---- MODULE SumeragiMissingBlockFetchGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for missing-block fetch planning.

This slice models the recovery boundary formed by
`touch_missing_block_request(...)`, `plan_missing_block_fetch_with_mode(...)`,
`defer_qc_for_missing_block_with_mode(...)`,
`missing_block_request_targets_without_local(...)`, and the fetch-request
builders. Missing-block request state is keyed by block hash; conflicting
heights for the same hash must not rewrite the identity, views and phases only
advance, priority and retry windows follow the consensus/backoff rules, fetch
attempts are accounted only when a request is emitted, target selection respects
default/signer/topology modes, and the final network send path removes the
local peer.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

FirstSeenFetch == 1
BackoffSuppressesFetch == 2
RetryAfterWindowFetch == 3
ForceRetryFetch == 4
PriorityUpgradeFetch == 5
ConflictingHeightStable == 6
AdvancedViewAccepted == 7
StaleViewIgnored == 8
PhaseRankAdvances == 9
ConsensusPriorityWins == 10
RetryMinBeforeAttempt == 11
RetryMaxAfterAttempt == 12
ViewChangeNoneClears == 13
AttemptsIncrementOnRequest == 14
AttemptsStableOnBackoff == 15
EmptyTopologyNoTargets == 16
DefaultPrefersSigners == 17
DefaultFallbackNoSigners == 18
DefaultFallbackAfterAttempts == 19
StrictSignersNoFallback == 20
AggressiveUsesTopology == 21
OutOfRangeSignerIgnored == 22
SendFiltersLocal == 23
KnownBlockNoDeferral == 24
FetchRequestFieldsPreserved == 25
CommitQcOnlyFlagPreserved == 26

Candidates == 1..26

NoBug == 0
DropFirstFetchBug == 1
FetchDuringBackoffBug == 2
DropRetryAfterWindowBug == 3
IgnoreForceRetryBug == 4
IgnorePriorityUpgradeBug == 5
OverwriteHeightOnConflictBug == 6
IgnoreAdvancedViewBug == 7
AcceptStaleViewBug == 8
RegressPhaseBug == 9
KeepBackgroundPriorityBug == 10
UseMaxRetryBeforeAttemptBug == 11
UseMinRetryAfterAttemptBug == 12
KeepViewChangeOnNoneBug == 13
SkipAttemptIncrementBug == 14
IncrementAttemptOnBackoffBug == 15
RequestWithoutTargetsBug == 16
SkipSignerPreferenceBug == 17
NoTopologyFallbackBug == 18
KeepSignerAfterFallbackAttemptsBug == 19
StrictSignersFallbackBug == 20
AggressiveUsesSignersBug == 21
UseOutOfRangeSignerBug == 22
SendToLocalBug == 23
DeferKnownBlockBug == 24
WrongFetchRequestFieldsBug == 25
WrongCommitQcOnlyFlagBug == 26

Bugs == 0..26

BugDropFirstFetch == Bug = DropFirstFetchBug
BugFetchDuringBackoff == Bug = FetchDuringBackoffBug
BugDropRetryAfterWindow == Bug = DropRetryAfterWindowBug
BugIgnoreForceRetry == Bug = IgnoreForceRetryBug
BugIgnorePriorityUpgrade == Bug = IgnorePriorityUpgradeBug
BugOverwriteHeightOnConflict == Bug = OverwriteHeightOnConflictBug
BugIgnoreAdvancedView == Bug = IgnoreAdvancedViewBug
BugAcceptStaleView == Bug = AcceptStaleViewBug
BugRegressPhase == Bug = RegressPhaseBug
BugKeepBackgroundPriority == Bug = KeepBackgroundPriorityBug
BugUseMaxRetryBeforeAttempt == Bug = UseMaxRetryBeforeAttemptBug
BugUseMinRetryAfterAttempt == Bug = UseMinRetryAfterAttemptBug
BugKeepViewChangeOnNone == Bug = KeepViewChangeOnNoneBug
BugSkipAttemptIncrement == Bug = SkipAttemptIncrementBug
BugIncrementAttemptOnBackoff == Bug = IncrementAttemptOnBackoffBug
BugRequestWithoutTargets == Bug = RequestWithoutTargetsBug
BugSkipSignerPreference == Bug = SkipSignerPreferenceBug
BugNoTopologyFallback == Bug = NoTopologyFallbackBug
BugKeepSignerAfterFallbackAttempts == Bug = KeepSignerAfterFallbackAttemptsBug
BugStrictSignersFallback == Bug = StrictSignersFallbackBug
BugAggressiveUsesSigners == Bug = AggressiveUsesSignersBug
BugUseOutOfRangeSigner == Bug = UseOutOfRangeSignerBug
BugSendToLocal == Bug = SendToLocalBug
BugDeferKnownBlock == Bug = DeferKnownBlockBug
BugWrongFetchRequestFields == Bug = WrongFetchRequestFieldsBug
BugWrongCommitQcOnlyFlag == Bug = WrongCommitQcOnlyFlagBug

SpecRequested(candidate) ==
  candidate \in {
    FirstSeenFetch,
    RetryAfterWindowFetch,
    ForceRetryFetch,
    PriorityUpgradeFetch,
    AttemptsIncrementOnRequest,
    DefaultPrefersSigners,
    DefaultFallbackNoSigners,
    DefaultFallbackAfterAttempts,
    AggressiveUsesTopology,
    FetchRequestFieldsPreserved,
    CommitQcOnlyFlagPreserved
  }

SpecNoTargets(candidate) ==
  candidate \in {
    EmptyTopologyNoTargets,
    StrictSignersNoFallback,
    OutOfRangeSignerIgnored
  }

SpecBackoff(candidate) ==
  candidate \in {BackoffSuppressesFetch, AttemptsStableOnBackoff}

ImplementationRequested(candidate) ==
  CASE candidate = FirstSeenFetch -> ~BugDropFirstFetch
    [] candidate = BackoffSuppressesFetch -> BugFetchDuringBackoff
    [] candidate = RetryAfterWindowFetch -> ~BugDropRetryAfterWindow
    [] candidate = ForceRetryFetch -> ~BugIgnoreForceRetry
    [] candidate = PriorityUpgradeFetch -> ~BugIgnorePriorityUpgrade
    [] candidate = AttemptsIncrementOnRequest -> TRUE
    [] candidate = AttemptsStableOnBackoff -> BugFetchDuringBackoff
    [] candidate = EmptyTopologyNoTargets -> BugRequestWithoutTargets
    [] candidate = DefaultPrefersSigners -> TRUE
    [] candidate = DefaultFallbackNoSigners -> ~BugNoTopologyFallback
    [] candidate = DefaultFallbackAfterAttempts -> TRUE
    [] candidate = StrictSignersNoFallback -> BugStrictSignersFallback
    [] candidate = AggressiveUsesTopology -> TRUE
    [] candidate = OutOfRangeSignerIgnored -> BugUseOutOfRangeSigner
    [] candidate = KnownBlockNoDeferral -> BugDeferKnownBlock
    [] candidate = FetchRequestFieldsPreserved -> TRUE
    [] candidate = CommitQcOnlyFlagPreserved -> TRUE
    [] OTHER -> FALSE

ImplementationNoTargets(candidate) ==
  CASE candidate = EmptyTopologyNoTargets -> ~BugRequestWithoutTargets
    [] candidate = StrictSignersNoFallback -> ~BugStrictSignersFallback
    [] candidate = OutOfRangeSignerIgnored -> ~BugUseOutOfRangeSigner
    [] OTHER -> FALSE

ImplementationBackoff(candidate) ==
  CASE candidate = BackoffSuppressesFetch -> ~BugFetchDuringBackoff
    [] candidate = AttemptsStableOnBackoff -> ~BugFetchDuringBackoff
    [] OTHER -> FALSE

ImplementationHeightStable(candidate) ==
  /\ candidate = ConflictingHeightStable
  /\ ~BugOverwriteHeightOnConflict

ImplementationAdvancedViewAccepted(candidate) ==
  /\ candidate = AdvancedViewAccepted
  /\ ~BugIgnoreAdvancedView

ImplementationStaleViewIgnored(candidate) ==
  /\ candidate = StaleViewIgnored
  /\ ~BugAcceptStaleView

ImplementationPhaseRankMonotonic(candidate) ==
  /\ candidate = PhaseRankAdvances
  /\ ~BugRegressPhase

ImplementationConsensusPriorityWins(candidate) ==
  /\ candidate = ConsensusPriorityWins
  /\ ~BugKeepBackgroundPriority

ImplementationRetryMinBeforeAttempt(candidate) ==
  /\ candidate = RetryMinBeforeAttempt
  /\ ~BugUseMaxRetryBeforeAttempt

ImplementationRetryMaxAfterAttempt(candidate) ==
  /\ candidate = RetryMaxAfterAttempt
  /\ ~BugUseMinRetryAfterAttempt

ImplementationViewChangeNoneClears(candidate) ==
  /\ candidate = ViewChangeNoneClears
  /\ ~BugKeepViewChangeOnNone

ImplementationAttemptsIncrement(candidate) ==
  /\ candidate = AttemptsIncrementOnRequest
  /\ ImplementationRequested(candidate)
  /\ ~BugSkipAttemptIncrement

ImplementationAttemptsStableOnBackoff(candidate) ==
  /\ candidate = AttemptsStableOnBackoff
  /\ ImplementationBackoff(candidate)
  /\ ~BugIncrementAttemptOnBackoff

ImplementationTargetKindSigners(candidate) ==
  CASE candidate = DefaultPrefersSigners -> ~BugSkipSignerPreference
    [] candidate = DefaultFallbackAfterAttempts -> BugKeepSignerAfterFallbackAttempts
    [] candidate = AggressiveUsesTopology -> BugAggressiveUsesSigners
    [] OTHER -> FALSE

ImplementationTargetKindTopology(candidate) ==
  CASE candidate = DefaultFallbackNoSigners -> ~BugNoTopologyFallback
    [] candidate = DefaultFallbackAfterAttempts -> ~BugKeepSignerAfterFallbackAttempts
    [] candidate = AggressiveUsesTopology -> ~BugAggressiveUsesSigners
    [] candidate = DefaultPrefersSigners -> BugSkipSignerPreference
    [] OTHER -> FALSE

ImplementationSendFiltersLocal(candidate) ==
  /\ candidate = SendFiltersLocal
  /\ ~BugSendToLocal

ImplementationKnownBlockNoDeferral(candidate) ==
  /\ candidate = KnownBlockNoDeferral
  /\ ~ImplementationRequested(candidate)
  /\ ~BugDeferKnownBlock

ImplementationFetchRequestFieldsPreserved(candidate) ==
  /\ candidate = FetchRequestFieldsPreserved
  /\ ImplementationRequested(candidate)
  /\ ~BugWrongFetchRequestFields

ImplementationCommitQcOnlyFlagPreserved(candidate) ==
  /\ candidate = CommitQcOnlyFlagPreserved
  /\ ImplementationRequested(candidate)
  /\ ~BugWrongCommitQcOnlyFlag

TypeInvariant ==
  /\ Bug \in Bugs
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

FetchDecisionMatchesSpec ==
  \A candidate \in tried:
    /\ ImplementationRequested(candidate) <=> SpecRequested(candidate)
    /\ ImplementationNoTargets(candidate) <=> SpecNoTargets(candidate)
    /\ ImplementationBackoff(candidate) <=> SpecBackoff(candidate)

RequestIdentityStable ==
  ConflictingHeightStable \in tried =>
    ImplementationHeightStable(ConflictingHeightStable)

RequestProgressMonotonic ==
  /\ AdvancedViewAccepted \in tried =>
       ImplementationAdvancedViewAccepted(AdvancedViewAccepted)
  /\ StaleViewIgnored \in tried =>
       ImplementationStaleViewIgnored(StaleViewIgnored)
  /\ PhaseRankAdvances \in tried =>
       ImplementationPhaseRankMonotonic(PhaseRankAdvances)

PriorityAndRetryWindowsSafe ==
  /\ ConsensusPriorityWins \in tried =>
       ImplementationConsensusPriorityWins(ConsensusPriorityWins)
  /\ RetryMinBeforeAttempt \in tried =>
       ImplementationRetryMinBeforeAttempt(RetryMinBeforeAttempt)
  /\ RetryMaxAfterAttempt \in tried =>
       ImplementationRetryMaxAfterAttempt(RetryMaxAfterAttempt)
  /\ ViewChangeNoneClears \in tried =>
       ImplementationViewChangeNoneClears(ViewChangeNoneClears)

AttemptsAccountedOnlyForEmittedFetches ==
  /\ AttemptsIncrementOnRequest \in tried =>
       ImplementationAttemptsIncrement(AttemptsIncrementOnRequest)
  /\ AttemptsStableOnBackoff \in tried =>
       ImplementationAttemptsStableOnBackoff(AttemptsStableOnBackoff)

TargetSelectionMatchesMode ==
  /\ DefaultPrefersSigners \in tried =>
       /\ ImplementationRequested(DefaultPrefersSigners)
       /\ ImplementationTargetKindSigners(DefaultPrefersSigners)
  /\ DefaultFallbackNoSigners \in tried =>
       /\ ImplementationRequested(DefaultFallbackNoSigners)
       /\ ImplementationTargetKindTopology(DefaultFallbackNoSigners)
  /\ DefaultFallbackAfterAttempts \in tried =>
       /\ ImplementationRequested(DefaultFallbackAfterAttempts)
       /\ ImplementationTargetKindTopology(DefaultFallbackAfterAttempts)
  /\ StrictSignersNoFallback \in tried =>
       /\ ImplementationNoTargets(StrictSignersNoFallback)
       /\ ~ImplementationTargetKindTopology(StrictSignersNoFallback)
  /\ AggressiveUsesTopology \in tried =>
       /\ ImplementationRequested(AggressiveUsesTopology)
       /\ ImplementationTargetKindTopology(AggressiveUsesTopology)
  /\ OutOfRangeSignerIgnored \in tried =>
       ImplementationNoTargets(OutOfRangeSignerIgnored)

FinalSendPathFiltersLocal ==
  SendFiltersLocal \in tried =>
    ImplementationSendFiltersLocal(SendFiltersLocal)

KnownBlocksDoNotDefer ==
  KnownBlockNoDeferral \in tried =>
    ImplementationKnownBlockNoDeferral(KnownBlockNoDeferral)

FetchRequestsCarryCorrectFields ==
  /\ FetchRequestFieldsPreserved \in tried =>
       ImplementationFetchRequestFieldsPreserved(FetchRequestFieldsPreserved)
  /\ CommitQcOnlyFlagPreserved \in tried =>
       ImplementationCommitQcOnlyFlagPreserved(CommitQcOnlyFlagPreserved)

MissingBlockFetchDecisionCases == {
  FirstSeenFetch, BackoffSuppressesFetch, RetryAfterWindowFetch,
  ForceRetryFetch, PriorityUpgradeFetch, EmptyTopologyNoTargets,
  StrictSignersNoFallback, OutOfRangeSignerIgnored
}

MissingBlockRequestStateCases == {
  ConflictingHeightStable, AdvancedViewAccepted, StaleViewIgnored,
  PhaseRankAdvances
}

MissingBlockPriorityRetryCases == {
  ConsensusPriorityWins, RetryMinBeforeAttempt, RetryMaxAfterAttempt,
  ViewChangeNoneClears
}

MissingBlockAttemptCases == {
  AttemptsIncrementOnRequest, AttemptsStableOnBackoff
}

MissingBlockTargetCases == {
  DefaultPrefersSigners, DefaultFallbackNoSigners,
  DefaultFallbackAfterAttempts, AggressiveUsesTopology
}

MissingBlockSendAndDeferralCases == {
  SendFiltersLocal, KnownBlockNoDeferral
}

MissingBlockRequestFieldCases == {
  FetchRequestFieldsPreserved, CommitQcOnlyFlagPreserved
}

MissingBlockFetchGroupedCases ==
  MissingBlockFetchDecisionCases \cup MissingBlockRequestStateCases \cup
  MissingBlockPriorityRetryCases \cup MissingBlockAttemptCases \cup
  MissingBlockTargetCases \cup MissingBlockSendAndDeferralCases \cup
  MissingBlockRequestFieldCases

MissingBlockFetchCaseGroupsComplete ==
  MissingBlockFetchGroupedCases = Candidates

MissingBlockRequestStateExact ==
  /\ RequestIdentityStable
  /\ RequestProgressMonotonic

MissingBlockSendDeferralExact ==
  /\ FinalSendPathFiltersLocal
  /\ KnownBlocksDoNotDefer

MissingBlockFetchExactness ==
  /\ MissingBlockFetchCaseGroupsComplete
  /\ FetchDecisionMatchesSpec
  /\ MissingBlockRequestStateExact
  /\ PriorityAndRetryWindowsSafe
  /\ AttemptsAccountedOnlyForEmittedFetches
  /\ TargetSelectionMatchesMode
  /\ MissingBlockSendDeferralExact
  /\ FetchRequestsCarryCorrectFields

MissingBlockFetchCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MissingBlockFetchExactness

Safety ==
  MissingBlockFetchExactness

====
