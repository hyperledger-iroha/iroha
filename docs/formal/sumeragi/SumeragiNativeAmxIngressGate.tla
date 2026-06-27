---- MODULE SumeragiNativeAmxIngressGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for native AMX control-plane ingress.

This slice models the message boundary formed by `on_native_amx_message(...)`,
`handle_native_amx_attestation_request(...)`, `record_native_amx_vote(...)`,
and `NativeAmxSessionCache::insert_vote(...)`. A node replies to prepare/commit
attestation requests only when the request body phase matches the request kind,
the local consensus key is BLS-normal, and the local key has a live
proof-of-possession at the planned coordinator height. Inbound votes are cached
only when the signer is BLS-normal, has a live and valid proof-of-possession,
and the BLS signature verifies over the canonical body preimage. Duplicate
votes for the same body/signature slot are ignored, while the same signer may
vote on retried bodies and distinct participant legs.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

ValidPrepareRequest == 1
ValidCommitRequest == 2
WrongPreparePhaseRequest == 3
WrongCommitPhaseRequest == 4
LocalNonBlsRequest == 5
LocalMissingPopRequest == 6
ValidPrepareVote == 7
ValidCommitVote == 8
VoteSignerNonBls == 9
VoteSignerMissingPop == 10
VoteSignerInvalidPop == 11
VoteInvalidSignature == 12
DuplicateSignerSameBody == 13
RetriedBodySameSigner == 14
DifferentParticipantSameSigner == 15

Candidates == 1..15

NoBug == 0
ReplyWrongPreparePhaseBug == 1
ReplyWrongCommitPhaseBug == 2
ReplyLocalNonBlsBug == 3
ReplyLocalMissingPopBug == 4
DropValidPrepareRequestBug == 5
DropValidCommitRequestBug == 6
WrongReplyPeerBug == 7
WrongReplyPhaseBug == 8
WrongReplySignerBug == 9
WrongReplyBodyBug == 10
CacheNonBlsVoteBug == 11
CacheMissingPopVoteBug == 12
CacheInvalidPopVoteBug == 13
CacheInvalidSignatureVoteBug == 14
DropValidPrepareVoteBug == 15
DropValidCommitVoteBug == 16
CacheDuplicateSignerTwiceBug == 17
DropRetriedBodyBug == 18
DropDifferentParticipantBug == 19

Bugs == 0..19

BugReplyWrongPreparePhase == Bug = ReplyWrongPreparePhaseBug
BugReplyWrongCommitPhase == Bug = ReplyWrongCommitPhaseBug
BugReplyLocalNonBls == Bug = ReplyLocalNonBlsBug
BugReplyLocalMissingPop == Bug = ReplyLocalMissingPopBug
BugDropValidPrepareRequest == Bug = DropValidPrepareRequestBug
BugDropValidCommitRequest == Bug = DropValidCommitRequestBug
BugWrongReplyPeer == Bug = WrongReplyPeerBug
BugWrongReplyPhase == Bug = WrongReplyPhaseBug
BugWrongReplySigner == Bug = WrongReplySignerBug
BugWrongReplyBody == Bug = WrongReplyBodyBug
BugCacheNonBlsVote == Bug = CacheNonBlsVoteBug
BugCacheMissingPopVote == Bug = CacheMissingPopVoteBug
BugCacheInvalidPopVote == Bug = CacheInvalidPopVoteBug
BugCacheInvalidSignatureVote == Bug = CacheInvalidSignatureVoteBug
BugDropValidPrepareVote == Bug = DropValidPrepareVoteBug
BugDropValidCommitVote == Bug = DropValidCommitVoteBug
BugCacheDuplicateSignerTwice == Bug = CacheDuplicateSignerTwiceBug
BugDropRetriedBody == Bug = DropRetriedBodyBug
BugDropDifferentParticipant == Bug = DropDifferentParticipantBug

SpecReply(candidate) ==
  candidate \in {ValidPrepareRequest, ValidCommitRequest}

SpecCachedVotes(candidate) ==
  CASE candidate \in {ValidPrepareVote, ValidCommitVote} -> 1
    [] candidate = DuplicateSignerSameBody -> 1
    [] candidate \in {RetriedBodySameSigner, DifferentParticipantSameSigner} -> 2
    [] OTHER -> 0

ImplementationReply(candidate) ==
  \/ /\ candidate = ValidPrepareRequest
     /\ ~BugDropValidPrepareRequest
  \/ /\ candidate = ValidCommitRequest
     /\ ~BugDropValidCommitRequest
  \/ /\ candidate = WrongPreparePhaseRequest
     /\ BugReplyWrongPreparePhase
  \/ /\ candidate = WrongCommitPhaseRequest
     /\ BugReplyWrongCommitPhase
  \/ /\ candidate = LocalNonBlsRequest
     /\ BugReplyLocalNonBls
  \/ /\ candidate = LocalMissingPopRequest
     /\ BugReplyLocalMissingPop

ImplementationReplyPeerMatches(candidate) ==
  /\ ImplementationReply(candidate)
  /\ ~BugWrongReplyPeer

ImplementationReplyPhaseMatches(candidate) ==
  /\ ImplementationReply(candidate)
  /\ ~BugWrongReplyPhase

ImplementationReplySignerLocal(candidate) ==
  /\ ImplementationReply(candidate)
  /\ ~BugWrongReplySigner

ImplementationReplyBodyMatches(candidate) ==
  /\ ImplementationReply(candidate)
  /\ ~BugWrongReplyBody

ImplementationCachedVotes(candidate) ==
  CASE candidate = ValidPrepareVote ->
        IF BugDropValidPrepareVote THEN 0 ELSE 1
    [] candidate = ValidCommitVote ->
        IF BugDropValidCommitVote THEN 0 ELSE 1
    [] candidate = VoteSignerNonBls ->
        IF BugCacheNonBlsVote THEN 1 ELSE 0
    [] candidate = VoteSignerMissingPop ->
        IF BugCacheMissingPopVote THEN 1 ELSE 0
    [] candidate = VoteSignerInvalidPop ->
        IF BugCacheInvalidPopVote THEN 1 ELSE 0
    [] candidate = VoteInvalidSignature ->
        IF BugCacheInvalidSignatureVote THEN 1 ELSE 0
    [] candidate = DuplicateSignerSameBody ->
        IF BugCacheDuplicateSignerTwice THEN 2 ELSE 1
    [] candidate = RetriedBodySameSigner ->
        IF BugDropRetriedBody THEN 1 ELSE 2
    [] candidate = DifferentParticipantSameSigner ->
        IF BugDropDifferentParticipant THEN 1 ELSE 2
    [] OTHER -> 0

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

RepliesMatchSpec ==
  \A candidate \in tried:
    ImplementationReply(candidate) <=> SpecReply(candidate)

CachedVotesMatchSpec ==
  \A candidate \in tried:
    ImplementationCachedVotes(candidate) = SpecCachedVotes(candidate)

InvalidRequestsFailClosed ==
  \A candidate \in tried:
    candidate \in {
      WrongPreparePhaseRequest,
      WrongCommitPhaseRequest,
      LocalNonBlsRequest,
      LocalMissingPopRequest
    } => ~ImplementationReply(candidate)

ValidRequestsReply ==
  /\ ValidPrepareRequest \in tried => ImplementationReply(ValidPrepareRequest)
  /\ ValidCommitRequest \in tried => ImplementationReply(ValidCommitRequest)

RepliesAreWellFormed ==
  \A candidate \in tried:
    ImplementationReply(candidate) =>
      /\ ImplementationReplyPeerMatches(candidate)
      /\ ImplementationReplyPhaseMatches(candidate)
      /\ ImplementationReplySignerLocal(candidate)
      /\ ImplementationReplyBodyMatches(candidate)

InvalidVotesFailClosed ==
  \A candidate \in tried:
    candidate \in {
      VoteSignerNonBls,
      VoteSignerMissingPop,
      VoteSignerInvalidPop,
      VoteInvalidSignature
    } => ImplementationCachedVotes(candidate) = 0

ValidVotesAreCached ==
  /\ ValidPrepareVote \in tried => ImplementationCachedVotes(ValidPrepareVote) = 1
  /\ ValidCommitVote \in tried => ImplementationCachedVotes(ValidCommitVote) = 1

DuplicateSignerDoesNotDuplicateBody ==
  DuplicateSignerSameBody \in tried =>
    ImplementationCachedVotes(DuplicateSignerSameBody) = 1

DistinctVoteBodiesRemainSeparate ==
  \A candidate \in tried:
    candidate \in {RetriedBodySameSigner, DifferentParticipantSameSigner} =>
      ImplementationCachedVotes(candidate) = 2

NativeAmxIngressRequestCases == {
  ValidPrepareRequest,
  ValidCommitRequest,
  WrongPreparePhaseRequest,
  WrongCommitPhaseRequest,
  LocalNonBlsRequest,
  LocalMissingPopRequest
}

NativeAmxIngressVoteAdmissionCases == {
  ValidPrepareVote,
  ValidCommitVote,
  VoteSignerNonBls,
  VoteSignerMissingPop,
  VoteSignerInvalidPop,
  VoteInvalidSignature
}

NativeAmxIngressVoteCacheCases == {
  DuplicateSignerSameBody,
  RetriedBodySameSigner,
  DifferentParticipantSameSigner
}

NativeAmxIngressGroupedCases ==
  NativeAmxIngressRequestCases \cup
  NativeAmxIngressVoteAdmissionCases \cup
  NativeAmxIngressVoteCacheCases

NativeAmxIngressCaseGroupsComplete ==
  NativeAmxIngressGroupedCases = Candidates

NativeAmxIngressRequestExactness ==
  /\ RepliesMatchSpec
  /\ InvalidRequestsFailClosed
  /\ ValidRequestsReply
  /\ RepliesAreWellFormed

NativeAmxIngressVoteAdmissionExactness ==
  /\ CachedVotesMatchSpec
  /\ InvalidVotesFailClosed
  /\ ValidVotesAreCached

NativeAmxIngressVoteCacheExactness ==
  /\ DuplicateSignerDoesNotDuplicateBody
  /\ DistinctVoteBodiesRemainSeparate

NativeAmxIngressExactness ==
  /\ NativeAmxIngressCaseGroupsComplete
  /\ NativeAmxIngressRequestExactness
  /\ NativeAmxIngressVoteAdmissionExactness
  /\ NativeAmxIngressVoteCacheExactness

NativeAmxIngressCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NativeAmxIngressExactness

Safety ==
  NativeAmxIngressExactness

====
