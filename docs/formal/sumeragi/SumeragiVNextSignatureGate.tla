---- MODULE SumeragiVNextSignatureGate ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for vNext aggregate certificate verification.

The Rust verifier first checks certificate-body consistency for re-chain
certificates, then validates the signer bitmap and aggregate BLS signature for
both re-chain and view-change certificates. The gate must reject empty
signatures, empty rosters, non-canonical bitmap length, out-of-range bitmap
bits, empty signer sets, PoP/roster length mismatch, under-quorum signers,
non-BLS signer keys, bad aggregate signatures, and inconsistent re-chain body
fields. Accepted certificates return exactly the bitmap-selected signers.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptMissingSignature,
  \* @type: Bool;
  BugAllowEmptyRoster,
  \* @type: Bool;
  BugIgnoreBitmapLength,
  \* @type: Bool;
  BugIgnoreBitmapOutOfRange,
  \* @type: Bool;
  BugAllowEmptySignerSet,
  \* @type: Bool;
  BugIgnorePopLength,
  \* @type: Bool;
  BugIgnoreCountQuorum,
  \* @type: Bool;
  BugUseNonStrictStake,
  \* @type: Bool;
  BugAllowNonBlsSigner,
  \* @type: Bool;
  BugAcceptBadAggregateSignature,
  \* @type: Bool;
  BugIgnoreRechainSlotMismatch,
  \* @type: Bool;
  BugIgnoreRechainHashMismatch,
  \* @type: Bool;
  BugIgnoreRechainSequenceMismatch,
  \* @type: Bool;
  BugReturnFullRoster,
  \* @type: Bool;
  BugDropReturnedSigner,
  \* @type: Bool;
  BugReturnSignersOnReject

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  accepted,
  \* @type: Set(Int);
  returned

vars == <<candidate, accepted, returned>>

Cases == {
  "valid_rechain_count",
  "valid_view_count",
  "valid_view_stake",
  "missing_signature",
  "empty_roster",
  "wrong_bitmap_length",
  "bitmap_oob",
  "empty_signer_set",
  "pop_len_mismatch",
  "under_count_quorum",
  "stake_boundary",
  "non_bls_signer",
  "bad_aggregate_signature",
  "rechain_slot_mismatch",
  "rechain_hash_mismatch",
  "rechain_seq_mismatch"
}

ValidCases == {"valid_rechain_count", "valid_view_count", "valid_view_stake"}

MalformedBitmapCases == {"wrong_bitmap_length", "bitmap_oob", "empty_signer_set"}

QuorumFailureCases == {"under_count_quorum", "stake_boundary"}

RechainBodyMismatchCases ==
  {"rechain_slot_mismatch", "rechain_hash_mismatch", "rechain_seq_mismatch"}

SignatureFailureCases == {"missing_signature", "non_bls_signer", "bad_aggregate_signature"}

Validators == 1..4

CertKind(c) ==
  CASE c \in {
      "valid_rechain_count",
      "rechain_slot_mismatch",
      "rechain_hash_mismatch",
      "rechain_seq_mismatch"
    } -> "rechain"
    [] OTHER -> "view"

Roster(c) ==
  CASE c = "empty_roster" -> {}
    [] c = "bitmap_oob" -> {1, 2, 3}
    [] OTHER -> Validators

SelectedByBitmap(c) ==
  CASE c = "empty_signer_set" -> {}
    [] c = "bitmap_oob" -> {1, 2, 3, 4}
    [] c \in {"under_count_quorum", "stake_boundary"} -> {1, 2}
    [] c = "valid_view_stake" -> {1, 2}
    [] OTHER -> {1, 2, 3}

BitmapLengthOk(c) == c # "wrong_bitmap_length"

BitmapWithinRoster(c) ==
  SelectedByBitmap(c) \subseteq Roster(c)

AggregateSignaturePresent(c) ==
  c # "missing_signature"

PopLengthOk(c) ==
  c # "pop_len_mismatch"

AllSelectedBls(c) ==
  c # "non_bls_signer"

AggregateSignatureOk(c) ==
  c # "bad_aggregate_signature"

RechainSlotMatches(c) ==
  c # "rechain_slot_mismatch"

RechainHashMatches(c) ==
  c # "rechain_hash_mismatch"

RechainSequenceMatches(c) ==
  c # "rechain_seq_mismatch"

SpecRechainBodyOk(c) ==
  CertKind(c) # "rechain"
    \/ /\ RechainSlotMatches(c)
       /\ RechainHashMatches(c)
       /\ RechainSequenceMatches(c)

ActualRechainBodyOk(c) ==
  CertKind(c) # "rechain"
    \/ /\ (RechainSlotMatches(c) \/ BugIgnoreRechainSlotMismatch)
       /\ (RechainHashMatches(c) \/ BugIgnoreRechainHashMismatch)
       /\ (RechainSequenceMatches(c) \/ BugIgnoreRechainSequenceMismatch)

CountQuorumOk(c) ==
  Cardinality(SelectedByBitmap(c)) >= 3

StrictStakeQuorumOk(c) ==
  c # "stake_boundary"

NonStrictStakeBoundary(c) ==
  c = "stake_boundary"

SpecQuorumOk(c) ==
  IF c \in {"valid_view_stake", "stake_boundary"}
  THEN StrictStakeQuorumOk(c)
  ELSE CountQuorumOk(c)

ActualQuorumOk(c) ==
  IF c \in {"valid_view_stake", "stake_boundary"}
  THEN StrictStakeQuorumOk(c) \/ (BugUseNonStrictStake /\ NonStrictStakeBoundary(c))
  ELSE
    \/ CountQuorumOk(c)
    \/ BugIgnoreCountQuorum
    \/ (BugAllowEmptySignerSet /\ SelectedByBitmap(c) = {})

SpecSignerBitmapOk(c) ==
  /\ Roster(c) # {}
  /\ BitmapLengthOk(c)
  /\ BitmapWithinRoster(c)
  /\ SelectedByBitmap(c) # {}

ActualSignerBitmapOk(c) ==
  IF Roster(c) = {} /\ BugAllowEmptyRoster
  THEN TRUE
  ELSE
    /\ Roster(c) # {}
    /\ (BitmapLengthOk(c) \/ BugIgnoreBitmapLength)
    /\ (BitmapWithinRoster(c) \/ BugIgnoreBitmapOutOfRange)
    /\ (SelectedByBitmap(c) # {} \/ BugAllowEmptySignerSet)

SpecAccept(c) ==
  /\ SpecRechainBodyOk(c)
  /\ AggregateSignaturePresent(c)
  /\ PopLengthOk(c)
  /\ SpecSignerBitmapOk(c)
  /\ SpecQuorumOk(c)
  /\ AllSelectedBls(c)
  /\ AggregateSignatureOk(c)

ActualAccept(c) ==
  /\ ActualRechainBodyOk(c)
  /\ (AggregateSignaturePresent(c) \/ BugAcceptMissingSignature)
  /\ (PopLengthOk(c) \/ BugIgnorePopLength)
  /\ ActualSignerBitmapOk(c)
  /\ ActualQuorumOk(c)
  /\ (AllSelectedBls(c) \/ BugAllowNonBlsSigner)
  /\ (AggregateSignatureOk(c) \/ BugAcceptBadAggregateSignature)

ActualReturned(c) ==
  IF ActualAccept(c)
  THEN
    IF BugReturnFullRoster
    THEN Roster(c)
    ELSE IF BugDropReturnedSigner
    THEN {}
    ELSE SelectedByBitmap(c)
  ELSE
    IF BugReturnSignersOnReject
    THEN SelectedByBitmap(c) \cap Roster(c)
    ELSE {}

TypeInvariant ==
  /\ BugAcceptMissingSignature \in BOOLEAN
  /\ BugAllowEmptyRoster \in BOOLEAN
  /\ BugIgnoreBitmapLength \in BOOLEAN
  /\ BugIgnoreBitmapOutOfRange \in BOOLEAN
  /\ BugAllowEmptySignerSet \in BOOLEAN
  /\ BugIgnorePopLength \in BOOLEAN
  /\ BugIgnoreCountQuorum \in BOOLEAN
  /\ BugUseNonStrictStake \in BOOLEAN
  /\ BugAllowNonBlsSigner \in BOOLEAN
  /\ BugAcceptBadAggregateSignature \in BOOLEAN
  /\ BugIgnoreRechainSlotMismatch \in BOOLEAN
  /\ BugIgnoreRechainHashMismatch \in BOOLEAN
  /\ BugIgnoreRechainSequenceMismatch \in BOOLEAN
  /\ BugReturnFullRoster \in BOOLEAN
  /\ BugDropReturnedSigner \in BOOLEAN
  /\ BugReturnSignersOnReject \in BOOLEAN
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

ValidCertificatesAccepted ==
  candidate \in ValidCases => accepted

MalformedBitmapFailsClosed ==
  candidate \in MalformedBitmapCases => ~accepted

QuorumFailuresFailClosed ==
  candidate \in QuorumFailureCases => ~accepted

SignatureFailuresFailClosed ==
  candidate \in SignatureFailureCases => ~accepted

RechainBodyMismatchesFailClosed ==
  candidate \in RechainBodyMismatchCases => ~accepted

EmptyRosterFailsClosed ==
  candidate = "empty_roster" => ~accepted

PopLengthMismatchFailsClosed ==
  candidate = "pop_len_mismatch" => ~accepted

VNextSignatureExactness ==
  /\ AcceptMatchesSpec
  /\ AcceptedReturnsBitmapSigners
  /\ ReturnedSignersWithinRoster
  /\ RejectedReturnsNoSigners
  /\ ValidCertificatesAccepted
  /\ MalformedBitmapFailsClosed
  /\ QuorumFailuresFailClosed
  /\ SignatureFailuresFailClosed
  /\ RechainBodyMismatchesFailClosed
  /\ EmptyRosterFailsClosed
  /\ PopLengthMismatchFailsClosed

Safety ==
  VNextSignatureExactness

VNextSignatureCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextSignatureExactness

SafetyFast ==
  VNextSignatureExactness

====
