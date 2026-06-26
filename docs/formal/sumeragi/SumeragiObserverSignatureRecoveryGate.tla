---- MODULE SumeragiObserverSignatureRecoveryGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for
`should_accept_observer_signature_mismatch_with_commit_qc(...)`.

The helper lets a peer outside the commit topology continue commit-only
progress after selected signature-verification mismatches, but only when the
pending block already has observed commit-QC evidence or the actor can find a
cached QC for the exact pending-block context. Local validators, unrelated
validation errors, unsupported signature-verification errors, and missing or
mismatched QC evidence must fail closed.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "observed_unknown_signature",
  "cached_unknown_signatory",
  "both_missing_pop",
  "cached_leader_missing",
  "local_validator_observed",
  "local_validator_cached",
  "prev_height_error",
  "other_signature_error",
  "non_signature_error_with_qc",
  "no_commit_qc",
  "cached_context_mismatch"
}

LocalInCommitTopology(c) ==
  c \in {"local_validator_observed", "local_validator_cached"}

ObservedCommitQc(c) ==
  c \in {
    "observed_unknown_signature",
    "both_missing_pop",
    "local_validator_observed",
    "prev_height_error",
    "non_signature_error_with_qc"
  }

CachedCommitQcMatches(c) ==
  c \in {
    "cached_unknown_signatory",
    "both_missing_pop",
    "cached_leader_missing",
    "local_validator_cached",
    "other_signature_error"
  }

HasCommitQc(c) ==
  ObservedCommitQc(c) \/ CachedCommitQcMatches(c)

AllowedSignatureMismatch(c) ==
  c \in {
    "observed_unknown_signature",
    "cached_unknown_signatory",
    "both_missing_pop",
    "cached_leader_missing",
    "local_validator_observed",
    "local_validator_cached",
    "no_commit_qc",
    "cached_context_mismatch"
  }

SpecAccept(c) ==
  /\ ~LocalInCommitTopology(c)
  /\ AllowedSignatureMismatch(c)
  /\ HasCommitQc(c)

ActualAccept(c) ==
  CASE Bug = "reject_unknown_signature"
       /\ c = "observed_unknown_signature" -> FALSE
    [] Bug = "reject_unknown_signatory"
       /\ c = "cached_unknown_signatory" -> FALSE
    [] Bug = "reject_missing_pop"
       /\ c = "both_missing_pop" -> FALSE
    [] Bug = "reject_leader_missing"
       /\ c = "cached_leader_missing" -> FALSE
    [] Bug = "accept_local_validator_observed"
       /\ c = "local_validator_observed" -> TRUE
    [] Bug = "accept_local_validator_cached"
       /\ c = "local_validator_cached" -> TRUE
    [] Bug = "accept_prev_height_error"
       /\ c = "prev_height_error" -> TRUE
    [] Bug = "accept_other_signature_error"
       /\ c = "other_signature_error" -> TRUE
    [] Bug = "accept_non_signature_error_with_qc"
       /\ c = "non_signature_error_with_qc" -> TRUE
    [] Bug = "accept_without_qc"
       /\ c = "no_commit_qc" -> TRUE
    [] Bug = "accept_cached_context_mismatch"
       /\ c = "cached_context_mismatch" -> TRUE
    [] Bug = "require_both_qc_sources"
       /\ c \in {"observed_unknown_signature", "cached_unknown_signatory"} ->
         FALSE
    [] OTHER -> SpecAccept(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

SafetyFast ==
  \A c \in Cases: ActualAccept(c) = SpecAccept(c)

BugRejectUnknownSignature ==
  ActualAccept("observed_unknown_signature") =
    SpecAccept("observed_unknown_signature")

BugRejectUnknownSignatory ==
  ActualAccept("cached_unknown_signatory") =
    SpecAccept("cached_unknown_signatory")

BugRejectMissingPop ==
  ActualAccept("both_missing_pop") = SpecAccept("both_missing_pop")

BugRejectLeaderMissing ==
  ActualAccept("cached_leader_missing") = SpecAccept("cached_leader_missing")

BugAcceptLocalValidatorObserved ==
  ActualAccept("local_validator_observed") =
    SpecAccept("local_validator_observed")

BugAcceptLocalValidatorCached ==
  ActualAccept("local_validator_cached") = SpecAccept("local_validator_cached")

BugAcceptPrevHeightError ==
  ActualAccept("prev_height_error") = SpecAccept("prev_height_error")

BugAcceptOtherSignatureError ==
  ActualAccept("other_signature_error") = SpecAccept("other_signature_error")

BugAcceptNonSignatureErrorWithQc ==
  ActualAccept("non_signature_error_with_qc") =
    SpecAccept("non_signature_error_with_qc")

BugAcceptWithoutQc ==
  ActualAccept("no_commit_qc") = SpecAccept("no_commit_qc")

BugAcceptCachedContextMismatch ==
  ActualAccept("cached_context_mismatch") =
    SpecAccept("cached_context_mismatch")

BugRequireBothQcSources ==
  /\ ActualAccept("observed_unknown_signature") =
       SpecAccept("observed_unknown_signature")
  /\ ActualAccept("cached_unknown_signatory") =
       SpecAccept("cached_unknown_signatory")

AllObserverRecoveryCasesMatchSpec ==
  \A c \in Cases:
    ActualAccept(c) = SpecAccept(c)

RecoverableSignatureMismatchAnchors ==
  /\ ActualAccept("observed_unknown_signature")
  /\ ActualAccept("cached_unknown_signatory")
  /\ ActualAccept("both_missing_pop")
  /\ ActualAccept("cached_leader_missing")

CommitQcSourceIndependenceAnchors ==
  /\ ObservedCommitQc("observed_unknown_signature")
  /\ ~CachedCommitQcMatches("observed_unknown_signature")
  /\ ActualAccept("observed_unknown_signature")
  /\ ~ObservedCommitQc("cached_unknown_signatory")
  /\ CachedCommitQcMatches("cached_unknown_signatory")
  /\ ActualAccept("cached_unknown_signatory")
  /\ ObservedCommitQc("both_missing_pop")
  /\ CachedCommitQcMatches("both_missing_pop")
  /\ ActualAccept("both_missing_pop")

LocalValidatorRejectionAnchors ==
  /\ LocalInCommitTopology("local_validator_observed")
  /\ LocalInCommitTopology("local_validator_cached")
  /\ HasCommitQc("local_validator_observed")
  /\ HasCommitQc("local_validator_cached")
  /\ ~ActualAccept("local_validator_observed")
  /\ ~ActualAccept("local_validator_cached")

UnsupportedErrorRejectionAnchors ==
  /\ HasCommitQc("prev_height_error")
  /\ HasCommitQc("other_signature_error")
  /\ HasCommitQc("non_signature_error_with_qc")
  /\ ~AllowedSignatureMismatch("prev_height_error")
  /\ ~AllowedSignatureMismatch("other_signature_error")
  /\ ~AllowedSignatureMismatch("non_signature_error_with_qc")
  /\ ~ActualAccept("prev_height_error")
  /\ ~ActualAccept("other_signature_error")
  /\ ~ActualAccept("non_signature_error_with_qc")

MissingOrMismatchedQcRejectionAnchors ==
  /\ AllowedSignatureMismatch("no_commit_qc")
  /\ AllowedSignatureMismatch("cached_context_mismatch")
  /\ ~HasCommitQc("no_commit_qc")
  /\ ~HasCommitQc("cached_context_mismatch")
  /\ ~ActualAccept("no_commit_qc")
  /\ ~ActualAccept("cached_context_mismatch")

AcceptedCasesRequireObserverMismatchAndQc ==
  \A c \in Cases:
    ActualAccept(c) =>
      /\ ~LocalInCommitTopology(c)
      /\ AllowedSignatureMismatch(c)
      /\ HasCommitQc(c)

SafetyAnchors ==
  /\ AllObserverRecoveryCasesMatchSpec
  /\ RecoverableSignatureMismatchAnchors
  /\ CommitQcSourceIndependenceAnchors
  /\ LocalValidatorRejectionAnchors
  /\ UnsupportedErrorRejectionAnchors
  /\ MissingOrMismatchedQcRejectionAnchors
  /\ AcceptedCasesRequireObserverMismatchAndQc

ObserverSignatureRecoveryCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ SafetyAnchors

====
