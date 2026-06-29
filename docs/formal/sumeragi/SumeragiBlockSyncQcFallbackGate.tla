---- MODULE SumeragiBlockSyncQcFallbackGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `block_sync_qc_is_missing_context_error(...)` and
`block_sync_qc_aggregate_fallback_ok(...)`.

Block-sync QC validation should retry only errors that can be explained by
missing local context: missing votes, unavailable NPoS stake snapshots, and
aggregate mismatches that may resolve after sidecar/roster convergence. The
aggregate fallback path must accept only COMMIT QCs without a nested highest QC,
with a valid aggregate, parseable signer bitmap, and a satisfied quorum under
the active consensus mode.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MissingVotesErr == 1
StakeSnapshotErr == 2
AggregateMismatchErr == 3
InvalidSignatureErr == 4
SignerOutErr == 5
InsufficientSignersErr == 6

ClassifierCases == {
  MissingVotesErr,
  StakeSnapshotErr,
  AggregateMismatchErr,
  InvalidSignatureErr,
  SignerOutErr,
  InsufficientSignersErr
}

SpecRetryable(e) ==
  e \in {MissingVotesErr, StakeSnapshotErr, AggregateMismatchErr}

ActualRetryable(e) ==
  CASE Bug = "missing_votes_not_retryable"
       /\ e = MissingVotesErr -> FALSE
    [] Bug = "stake_snapshot_not_retryable"
       /\ e = StakeSnapshotErr -> FALSE
    [] Bug = "aggregate_not_retryable"
       /\ e = AggregateMismatchErr -> FALSE
    [] Bug = "invalid_signature_retryable"
       /\ e = InvalidSignatureErr -> TRUE
    [] Bug = "signer_out_retryable"
       /\ e = SignerOutErr -> TRUE
    [] Bug = "insufficient_signers_retryable"
       /\ e = InsufficientSignersErr -> TRUE
    [] OTHER -> SpecRetryable(e)

RetryableMatches(e) ==
  ActualRetryable(e) = SpecRetryable(e)

PermHappy == 101
PermUnderQuorum == 102
PermZeroMinNoSigner == 103
PermZeroMinOneSigner == 104
PreparePhase == 105
HighestPresent == 106
BadAggregate == 107
BadBitmap == 108
NposHappy == 109
NposMissingSnapshot == 110
NposBadSignerPeers == 111
NposNoStakeQuorum == 112
NposStakeError == 113

FallbackCases == {
  PermHappy,
  PermUnderQuorum,
  PermZeroMinNoSigner,
  PermZeroMinOneSigner,
  PreparePhase,
  HighestPresent,
  BadAggregate,
  BadBitmap,
  NposHappy,
  NposMissingSnapshot,
  NposBadSignerPeers,
  NposNoStakeQuorum,
  NposStakeError
}

SpecFallbackOk(c) ==
  c \in {PermHappy, PermZeroMinOneSigner, NposHappy}

ActualFallbackOk(c) ==
  CASE Bug = "accept_prepare_phase"
       /\ c = PreparePhase -> TRUE
    [] Bug = "accept_highest_present"
       /\ c = HighestPresent -> TRUE
    [] Bug = "accept_bad_aggregate"
       /\ c = BadAggregate -> TRUE
    [] Bug = "accept_bad_bitmap"
       /\ c = BadBitmap -> TRUE
    [] Bug = "accept_permissioned_under_quorum"
       /\ c = PermUnderQuorum -> TRUE
    [] Bug = "reject_permissioned_exact_quorum"
       /\ c = PermHappy -> FALSE
    [] Bug = "zero_min_not_floored"
       /\ c = PermZeroMinNoSigner -> TRUE
    [] Bug = "reject_permissioned_zero_min_one_signer"
       /\ c = PermZeroMinOneSigner -> FALSE
    [] Bug = "accept_npos_missing_snapshot"
       /\ c = NposMissingSnapshot -> TRUE
    [] Bug = "accept_npos_bad_signer_peers"
       /\ c = NposBadSignerPeers -> TRUE
    [] Bug = "accept_npos_no_stake_quorum"
       /\ c = NposNoStakeQuorum -> TRUE
    [] Bug = "accept_npos_stake_error"
       /\ c = NposStakeError -> TRUE
    [] Bug = "reject_npos_with_stake_quorum"
       /\ c = NposHappy -> FALSE
    [] OTHER -> SpecFallbackOk(c)

FallbackMatches(c) ==
  ActualFallbackOk(c) = SpecFallbackOk(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "missing_votes_not_retryable",
       "stake_snapshot_not_retryable",
       "aggregate_not_retryable",
       "invalid_signature_retryable",
       "signer_out_retryable",
       "insufficient_signers_retryable",
       "accept_prepare_phase",
       "accept_highest_present",
       "accept_bad_aggregate",
       "accept_bad_bitmap",
       "accept_permissioned_under_quorum",
       "reject_permissioned_exact_quorum",
       "zero_min_not_floored",
       "reject_permissioned_zero_min_one_signer",
       "accept_npos_missing_snapshot",
       "accept_npos_bad_signer_peers",
       "accept_npos_no_stake_quorum",
       "accept_npos_stake_error",
       "reject_npos_with_stake_quorum"
     }
  /\ checked = 0

QcFallbackMatchesSpec ==
  /\ \A e \in ClassifierCases: RetryableMatches(e)
  /\ \A c \in FallbackCases: FallbackMatches(c)

BlockSyncQcFallbackExactness ==
  /\ QcFallbackMatchesSpec
BlockSyncQcFallbackCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncQcFallbackExactness

SafetyFast == BlockSyncQcFallbackExactness

BugMissingVotesNotRetryable ==
  RetryableMatches(MissingVotesErr)

BugStakeSnapshotNotRetryable ==
  RetryableMatches(StakeSnapshotErr)

BugAggregateNotRetryable ==
  RetryableMatches(AggregateMismatchErr)

BugInvalidSignatureRetryable ==
  RetryableMatches(InvalidSignatureErr)

BugSignerOutRetryable ==
  RetryableMatches(SignerOutErr)

BugInsufficientSignersRetryable ==
  RetryableMatches(InsufficientSignersErr)

BugAcceptPreparePhase ==
  FallbackMatches(PreparePhase)

BugAcceptHighestPresent ==
  FallbackMatches(HighestPresent)

BugAcceptBadAggregate ==
  FallbackMatches(BadAggregate)

BugAcceptBadBitmap ==
  FallbackMatches(BadBitmap)

BugAcceptPermissionedUnderQuorum ==
  FallbackMatches(PermUnderQuorum)

BugRejectPermissionedExactQuorum ==
  FallbackMatches(PermHappy)

BugZeroMinNotFloored ==
  FallbackMatches(PermZeroMinNoSigner)

BugRejectPermissionedZeroMinOneSigner ==
  FallbackMatches(PermZeroMinOneSigner)

BugAcceptNposMissingSnapshot ==
  FallbackMatches(NposMissingSnapshot)

BugAcceptNposBadSignerPeers ==
  FallbackMatches(NposBadSignerPeers)

BugAcceptNposNoStakeQuorum ==
  FallbackMatches(NposNoStakeQuorum)

BugAcceptNposStakeError ==
  FallbackMatches(NposStakeError)

BugRejectNposWithStakeQuorum ==
  FallbackMatches(NposHappy)

====
