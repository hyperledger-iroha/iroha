---- MODULE SumeragiClassicSignatureGate ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for classic Sumeragi Vote/QC signature verification.

The Rust path `validate_qc_against_votes(...)` validates the QC mode tag,
validator-set binding, signer bitmap, count or stake quorum, aggregate BLS
signature inputs, vote availability, vote subject roots, per-vote signatures,
view-specific signer mapping, and NewView highest-QC agreement. NPoS QCs may
accept missing local vote bodies after the aggregate signature and stake quorum
validate; permissioned QCs must have the bitmap-selected votes locally.

This model captures those fail-closed gates and the successful return contract:
accepted QCs return exactly the bitmap-selected voting signers, while rejected
QCs return no signers.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  accepted,
  \* @type: Set(Int);
  returned

\* @type: <<Str, Bool, Set(Int)>>;
vars == <<candidate, accepted, returned>>

Cases == {
  "valid_commit_count",
  "valid_new_view_count",
  "valid_npos_stake_missing_vote",
  "mode_tag_mismatch",
  "validator_set_mismatch",
  "empty_roster",
  "wrong_bitmap_length",
  "bitmap_oob",
  "empty_signer_set",
  "under_count_quorum",
  "stake_boundary",
  "missing_stake_snapshot",
  "aggregate_missing_signature",
  "aggregate_missing_pop",
  "aggregate_bad_signature",
  "missing_vote_permissioned",
  "subject_mismatch",
  "roots_mismatch",
  "vote_invalid_signature",
  "view_mapping_missing",
  "non_new_view_highest_present",
  "new_view_missing_highest",
  "new_view_highest_subject_mismatch",
  "new_view_highest_height_mismatch",
  "new_view_highest_epoch_future",
  "new_view_highest_phase_invalid",
  "new_view_vote_highest_mismatch"
}

ValidCases == {
  "valid_commit_count",
  "valid_new_view_count",
  "valid_npos_stake_missing_vote"
}

BitmapFailureCases == {
  "empty_roster",
  "wrong_bitmap_length",
  "bitmap_oob",
  "empty_signer_set"
}

QuorumFailureCases == {
  "under_count_quorum",
  "stake_boundary",
  "missing_stake_snapshot"
}

AggregateFailureCases == {
  "aggregate_missing_signature",
  "aggregate_missing_pop",
  "aggregate_bad_signature"
}

VoteFailureCases == {
  "missing_vote_permissioned",
  "subject_mismatch",
  "roots_mismatch",
  "vote_invalid_signature",
  "view_mapping_missing"
}

HighestFailureCases == {
  "non_new_view_highest_present",
  "new_view_missing_highest",
  "new_view_highest_subject_mismatch",
  "new_view_highest_height_mismatch",
  "new_view_highest_epoch_future",
  "new_view_highest_phase_invalid",
  "new_view_vote_highest_mismatch"
}

StakeModeCases == {
  "valid_npos_stake_missing_vote",
  "stake_boundary",
  "missing_stake_snapshot"
}

NewViewCases == {
  "valid_new_view_count",
  "new_view_missing_highest",
  "new_view_highest_subject_mismatch",
  "new_view_highest_height_mismatch",
  "new_view_highest_epoch_future",
  "new_view_highest_phase_invalid",
  "new_view_vote_highest_mismatch"
}

Validators == 1..4

ConsensusMode(c) ==
  IF c \in StakeModeCases THEN "npos" ELSE "permissioned"

Roster(c) ==
  CASE c = "empty_roster" -> {}
    [] c = "bitmap_oob" -> {1, 2, 3}
    [] OTHER -> Validators

SelectedByBitmap(c) ==
  CASE c = "empty_signer_set" -> {}
    [] c = "bitmap_oob" -> {1, 2, 3, 4}
    [] c \in {"under_count_quorum", "stake_boundary"} -> {1, 2}
    [] OTHER -> {1, 2, 3}

VotesPresent(c) ==
  CASE c \in {"missing_vote_permissioned", "valid_npos_stake_missing_vote"} ->
      SelectedByBitmap(c) \ {3}
    [] OTHER -> SelectedByBitmap(c)

ModeTagOk(c) == c # "mode_tag_mismatch"

ValidatorSetOk(c) == c # "validator_set_mismatch"

BitmapLengthOk(c) == c # "wrong_bitmap_length"

BitmapWithinRoster(c) == SelectedByBitmap(c) \subseteq Roster(c)

SignerSetNonEmpty(c) == SelectedByBitmap(c) # {}

StakeSnapshotOk(c) == c # "missing_stake_snapshot"

StrictStakeQuorumOk(c) == c # "stake_boundary"

CountQuorumOk(c) == Cardinality(SelectedByBitmap(c)) >= 3

SpecQuorumOk(c) ==
  IF ConsensusMode(c) = "npos"
  THEN /\ StakeSnapshotOk(c)
       /\ StrictStakeQuorumOk(c)
  ELSE CountQuorumOk(c)

ActualQuorumOk(c) ==
  IF ConsensusMode(c) = "npos"
  THEN /\ (StakeSnapshotOk(c) \/ Bug = "allow_missing_stake_snapshot")
       /\ (StrictStakeQuorumOk(c) \/ Bug = "use_non_strict_stake")
  ELSE
    \/ CountQuorumOk(c)
    \/ Bug = "ignore_count_quorum"
    \/ (Bug = "allow_empty_signer_set" /\ SelectedByBitmap(c) = {})

SpecSignerBitmapOk(c) ==
  /\ Roster(c) # {}
  /\ BitmapLengthOk(c)
  /\ BitmapWithinRoster(c)
  /\ SignerSetNonEmpty(c)

ActualSignerBitmapOk(c) ==
  IF Roster(c) = {} /\ Bug = "allow_empty_roster"
  THEN TRUE
  ELSE
    /\ Roster(c) # {}
    /\ (BitmapLengthOk(c) \/ Bug = "ignore_bitmap_length")
    /\ (BitmapWithinRoster(c) \/ Bug = "ignore_bitmap_out_of_range")
    /\ (SignerSetNonEmpty(c) \/ Bug = "allow_empty_signer_set")

AggregateSignaturePresent(c) == c # "aggregate_missing_signature"

AggregatePopPresent(c) == c # "aggregate_missing_pop"

AggregateSignatureOk(c) == c # "aggregate_bad_signature"

SpecAggregateOk(c) ==
  /\ AggregateSignaturePresent(c)
  /\ AggregatePopPresent(c)
  /\ AggregateSignatureOk(c)

ActualAggregateOk(c) ==
  /\ (AggregateSignaturePresent(c) \/ Bug = "accept_missing_aggregate_signature")
  /\ (AggregatePopPresent(c) \/ Bug = "ignore_missing_pop")
  /\ (AggregateSignatureOk(c) \/ Bug = "accept_bad_aggregate_signature")

MissingVotesAllowed(c) ==
  Cardinality(SelectedByBitmap(c) \ VotesPresent(c)) = 0
    \/ ConsensusMode(c) = "npos"

ActualMissingVotesAllowed(c) ==
  MissingVotesAllowed(c) \/ Bug = "ignore_missing_votes"

VoteSubjectOk(c) == c # "subject_mismatch"

VoteRootsOk(c) == c # "roots_mismatch"

VoteSignatureOk(c) == c # "vote_invalid_signature"

ViewMappingOk(c) == c # "view_mapping_missing"

SpecVoteChecksOk(c) ==
  /\ MissingVotesAllowed(c)
  /\ VoteSubjectOk(c)
  /\ VoteRootsOk(c)
  /\ VoteSignatureOk(c)
  /\ ViewMappingOk(c)

ActualVoteChecksOk(c) ==
  /\ ActualMissingVotesAllowed(c)
  /\ (VoteSubjectOk(c) \/ Bug = "ignore_subject_mismatch")
  /\ (VoteRootsOk(c) \/ Bug = "ignore_roots_mismatch")
  /\ (VoteSignatureOk(c) \/ Bug = "ignore_vote_invalid_signature")
  /\ (ViewMappingOk(c) \/ Bug = "ignore_view_mapping_failure")

IsNewView(c) == c \in NewViewCases

HighestAbsentForNonNewView(c) == c # "non_new_view_highest_present"

NewViewHighestPresent(c) == c # "new_view_missing_highest"

NewViewHighestSubjectOk(c) == c # "new_view_highest_subject_mismatch"

NewViewHighestHeightOk(c) == c # "new_view_highest_height_mismatch"

NewViewHighestEpochOk(c) == c # "new_view_highest_epoch_future"

NewViewHighestPhaseOk(c) == c # "new_view_highest_phase_invalid"

VoteHighestAgrees(c) == c # "new_view_vote_highest_mismatch"

SpecHighestOk(c) ==
  IF IsNewView(c)
  THEN /\ NewViewHighestPresent(c)
       /\ NewViewHighestSubjectOk(c)
       /\ NewViewHighestHeightOk(c)
       /\ NewViewHighestEpochOk(c)
       /\ NewViewHighestPhaseOk(c)
       /\ VoteHighestAgrees(c)
  ELSE HighestAbsentForNonNewView(c)

ActualHighestOk(c) ==
  IF IsNewView(c)
  THEN /\ (NewViewHighestPresent(c) \/ Bug = "allow_new_view_missing_highest")
       /\ (NewViewHighestSubjectOk(c) \/ Bug = "ignore_new_view_highest_subject")
       /\ (NewViewHighestHeightOk(c) \/ Bug = "ignore_new_view_highest_height")
       /\ (NewViewHighestEpochOk(c) \/ Bug = "ignore_new_view_highest_epoch")
       /\ (NewViewHighestPhaseOk(c) \/ Bug = "ignore_new_view_highest_phase")
       /\ (VoteHighestAgrees(c) \/ Bug = "ignore_vote_highest_mismatch")
  ELSE HighestAbsentForNonNewView(c) \/ Bug = "allow_non_new_view_highest"

SpecAccept(c) ==
  /\ ModeTagOk(c)
  /\ ValidatorSetOk(c)
  /\ SpecSignerBitmapOk(c)
  /\ SpecQuorumOk(c)
  /\ SpecAggregateOk(c)
  /\ SpecVoteChecksOk(c)
  /\ SpecHighestOk(c)

ActualAccept(c) ==
  /\ (ModeTagOk(c) \/ Bug = "accept_mode_tag_mismatch")
  /\ (ValidatorSetOk(c) \/ Bug = "accept_validator_set_mismatch")
  /\ ActualSignerBitmapOk(c)
  /\ ActualQuorumOk(c)
  /\ ActualAggregateOk(c)
  /\ ActualVoteChecksOk(c)
  /\ ActualHighestOk(c)

ActualReturned(c) ==
  IF ActualAccept(c)
  THEN
    IF Bug = "return_full_roster"
    THEN Roster(c)
    ELSE IF Bug = "drop_returned_signer"
    THEN {}
    ELSE SelectedByBitmap(c)
  ELSE
    IF Bug = "return_signers_on_reject"
    THEN SelectedByBitmap(c) \cap Roster(c)
    ELSE {}

BugModes == {
  "none",
  "accept_mode_tag_mismatch",
  "accept_validator_set_mismatch",
  "allow_empty_roster",
  "ignore_bitmap_length",
  "ignore_bitmap_out_of_range",
  "allow_empty_signer_set",
  "ignore_count_quorum",
  "use_non_strict_stake",
  "allow_missing_stake_snapshot",
  "accept_missing_aggregate_signature",
  "ignore_missing_pop",
  "accept_bad_aggregate_signature",
  "ignore_missing_votes",
  "ignore_subject_mismatch",
  "ignore_roots_mismatch",
  "ignore_vote_invalid_signature",
  "ignore_view_mapping_failure",
  "allow_non_new_view_highest",
  "allow_new_view_missing_highest",
  "ignore_new_view_highest_subject",
  "ignore_new_view_highest_height",
  "ignore_new_view_highest_epoch",
  "ignore_new_view_highest_phase",
  "ignore_vote_highest_mismatch",
  "return_full_roster",
  "drop_returned_signer",
  "return_signers_on_reject"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ accepted \in BOOLEAN
  /\ returned \subseteq Validators

Init ==
  /\ candidate = "none"
  /\ accepted = FALSE
  /\ returned = {}

Apply(c) ==
  /\ candidate' = c
  /\ accepted' = ActualAccept(c)
  /\ returned' = ActualReturned(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

AcceptMatchesSpec ==
  candidate = "none" \/ accepted = SpecAccept(candidate)

AcceptedReturnsBitmapSigners ==
  candidate = "none" \/ (accepted => returned = SelectedByBitmap(candidate))

ReturnedSignersWithinRoster ==
  candidate = "none" \/ returned \subseteq Roster(candidate)

RejectedReturnsNoSigners ==
  candidate = "none" \/ (~accepted => returned = {})

ValidCasesAccepted ==
  candidate \in ValidCases => accepted

ModeAndRosterFailuresFailClosed ==
  candidate \in {"mode_tag_mismatch", "validator_set_mismatch"} => ~accepted

BitmapFailuresFailClosed ==
  candidate \in BitmapFailureCases => ~accepted

QuorumFailuresFailClosed ==
  candidate \in QuorumFailureCases => ~accepted

AggregateFailuresFailClosed ==
  candidate \in AggregateFailureCases => ~accepted

VoteFailuresFailClosed ==
  candidate \in VoteFailureCases => ~accepted

HighestFailuresFailClosed ==
  candidate \in HighestFailureCases => ~accepted

NposAggregateMayTolerateMissingVotes ==
  candidate = "valid_npos_stake_missing_vote" =>
    /\ accepted
    /\ Cardinality(SelectedByBitmap(candidate) \ VotesPresent(candidate)) = 1

Safety ==
  /\ AcceptMatchesSpec
  /\ AcceptedReturnsBitmapSigners
  /\ ReturnedSignersWithinRoster
  /\ RejectedReturnsNoSigners
  /\ ValidCasesAccepted
  /\ ModeAndRosterFailuresFailClosed
  /\ BitmapFailuresFailClosed
  /\ QuorumFailuresFailClosed
  /\ AggregateFailuresFailClosed
  /\ VoteFailuresFailClosed
  /\ HighestFailuresFailClosed
  /\ NposAggregateMayTolerateMissingVotes

====
