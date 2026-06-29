---- MODULE SumeragiVoteAdmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for classic Sumeragi inbound vote admission.

The model covers the combined contract of `handle_vote(...)`,
`validate_and_record_vote_with_signature_result(...)`, and
`apply_validated_vote(...)`: early height/view, lock, and roster gates must
fail closed; duplicate or malformed votes must not overwrite vote evidence;
invalid NEW_VIEW highest-QC references must be rejected; same-signer conflicts
are either rejected with double-vote evidence, deferred until supersession
context exists, or accepted only when a newer QC/local quorum proves
supersession; accepted votes record exactly once and drive the expected
QC/progress/roster-cache/new-view/pipeline side effects.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  accepted,
  \* @type: Bool;
  recorded,
  \* @type: Bool;
  deferred,
  \* @type: Bool;
  dropped,
  \* @type: Bool;
  evidence,
  \* @type: Bool;
  qc_attempted,
  \* @type: Bool;
  roster_cached,
  \* @type: Bool;
  new_view_tracked,
  \* @type: Bool;
  pipeline_requested,
  \* @type: Bool;
  progress_touched

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, accepted, recorded, deferred, dropped, evidence,
  qc_attempted, roster_cached, new_view_tracked, pipeline_requested,
  progress_touched>>

Cases == {
  "valid_prepare",
  "valid_commit",
  "valid_new_view",
  "stale_new_view",
  "duplicate_vote",
  "roster_missing_prepare",
  "height_or_view_drop",
  "locked_conflict",
  "non_new_view_highest",
  "chain_order_mismatch",
  "bad_signature",
  "new_view_missing_highest",
  "new_view_bad_highest_epoch",
  "new_view_bad_highest_phase",
  "new_view_hash_mismatch",
  "new_view_height_mismatch",
  "new_view_local_metadata_mismatch",
  "same_slot_conflict",
  "same_slot_conflict_superseded",
  "same_slot_conflict_deferred",
  "same_key_conflict",
  "cross_phase_conflict"
}

NewViewCases == {
  "valid_new_view",
  "stale_new_view",
  "new_view_missing_highest",
  "new_view_bad_highest_epoch",
  "new_view_bad_highest_phase",
  "new_view_hash_mismatch",
  "new_view_height_mismatch",
  "new_view_local_metadata_mismatch"
}

PrepareCommitCases == Cases \ NewViewCases

AcceptedCases == {
  "valid_prepare",
  "valid_commit",
  "valid_new_view",
  "stale_new_view",
  "same_slot_conflict_superseded",
  "cross_phase_conflict"
}

DeferredCases == {
  "roster_missing_prepare",
  "same_slot_conflict_deferred"
}

EvidenceCases == {
  "same_slot_conflict",
  "same_key_conflict",
  "cross_phase_conflict"
}

InvalidCases == (Cases \ AcceptedCases) \ DeferredCases

HeightViewOk(c) == c # "height_or_view_drop"

LockOk(c) == c # "locked_conflict"

RosterAvailable(c) == c # "roster_missing_prepare"

NotDuplicate(c) == c # "duplicate_vote"

NonNewViewHighestOk(c) == c # "non_new_view_highest"

ChainOrderOk(c) == c # "chain_order_mismatch"

SignatureOk(c) == c # "bad_signature"

NewViewHighestPresent(c) == c # "new_view_missing_highest"

NewViewHighestEpochOk(c) == c # "new_view_bad_highest_epoch"

NewViewHighestPhaseOk(c) == c # "new_view_bad_highest_phase"

NewViewHighestHashOk(c) == c # "new_view_hash_mismatch"

NewViewHeightOk(c) == c # "new_view_height_mismatch"

NewViewLocalMetadataOk(c) == c # "new_view_local_metadata_mismatch"

SpecNewViewHighestOk(c) ==
  IF c \in NewViewCases
  THEN
    /\ NewViewHighestPresent(c)
    /\ NewViewHighestEpochOk(c)
    /\ NewViewHighestPhaseOk(c)
    /\ NewViewHighestHashOk(c)
    /\ NewViewHeightOk(c)
    /\ NewViewLocalMetadataOk(c)
  ELSE TRUE

ActualNewViewHighestOk(c) ==
  IF c \in NewViewCases
  THEN
    /\ (NewViewHighestPresent(c) \/ Bug = "accept_new_view_missing_highest")
    /\ (NewViewHighestEpochOk(c) \/ Bug = "accept_new_view_bad_highest_epoch")
    /\ (NewViewHighestPhaseOk(c) \/ Bug = "accept_new_view_bad_highest_phase")
    /\ (NewViewHighestHashOk(c) \/ Bug = "accept_new_view_hash_mismatch")
    /\ (NewViewHeightOk(c) \/ Bug = "accept_new_view_height_mismatch")
    /\ (NewViewLocalMetadataOk(c) \/ Bug = "accept_new_view_local_metadata_mismatch")
  ELSE TRUE

SpecConflictOk(c) ==
  CASE c = "same_slot_conflict" -> FALSE
    [] c = "same_slot_conflict_deferred" -> FALSE
    [] c = "same_key_conflict" -> FALSE
    [] OTHER -> TRUE

ActualConflictOk(c) ==
  CASE c = "same_slot_conflict" -> Bug = "record_same_slot_conflict"
    [] c = "same_slot_conflict_superseded" -> Bug # "drop_superseded_conflict"
    [] c = "same_slot_conflict_deferred" -> Bug = "record_deferred_conflict"
    [] c = "same_key_conflict" -> Bug = "accept_same_key_conflict"
    [] OTHER -> TRUE

SpecAccept(c) ==
  /\ HeightViewOk(c)
  /\ LockOk(c)
  /\ RosterAvailable(c)
  /\ NotDuplicate(c)
  /\ NonNewViewHighestOk(c)
  /\ ChainOrderOk(c)
  /\ SignatureOk(c)
  /\ SpecNewViewHighestOk(c)
  /\ SpecConflictOk(c)

ActualAccept(c) ==
  /\ (HeightViewOk(c) \/ Bug = "accept_height_or_view_drop")
  /\ (LockOk(c) \/ Bug = "accept_locked_conflict")
  /\ (RosterAvailable(c) \/ Bug = "accept_roster_missing")
  /\ (NotDuplicate(c) \/ Bug = "record_duplicate")
  /\ (NonNewViewHighestOk(c) \/ Bug = "accept_non_new_view_highest")
  /\ (ChainOrderOk(c) \/ Bug = "accept_chain_order_mismatch")
  /\ (SignatureOk(c) \/ Bug = "accept_bad_signature")
  /\ ActualNewViewHighestOk(c)
  /\ ActualConflictOk(c)

SpecDeferred(c) == c \in DeferredCases

ActualDeferred(c) ==
  CASE c = "roster_missing_prepare" ->
       /\ Bug # "accept_roster_missing"
       /\ Bug # "skip_roster_defer"
    [] c = "same_slot_conflict_deferred" ->
       /\ Bug # "record_deferred_conflict"
       /\ Bug # "skip_defer_missing_context"
    [] OTHER -> FALSE

SpecDropped(c) == c \in InvalidCases

ActualDropped(c) ==
  IF ActualAccept(c) \/ ActualDeferred(c) THEN FALSE ELSE TRUE

SpecRecorded(c) == SpecAccept(c)

ActualRecorded(c) == ActualAccept(c)

SpecEvidence(c) == c \in EvidenceCases

ActualEvidence(c) ==
  \/ /\ c \in {"same_slot_conflict", "same_key_conflict"}
     /\ ~ActualAccept(c)
     /\ Bug # "skip_double_vote_evidence"
  \/ /\ c = "cross_phase_conflict"
     /\ ActualAccept(c)
     /\ Bug # "skip_double_vote_evidence"
     /\ Bug # "skip_cross_phase_evidence"
  \/ /\ c = "same_slot_conflict_superseded"
     /\ Bug = "evidence_on_superseded"

SpecQcAttempted(c) == SpecAccept(c)

ActualQcAttempted(c) ==
  IF ActualAccept(c) THEN Bug # "skip_qc_attempt_on_accept" ELSE Bug = "qc_attempt_on_reject"

SpecRosterCached(c) == SpecAccept(c) /\ c \in PrepareCommitCases

ActualRosterCached(c) ==
  IF ActualAccept(c)
  THEN
    IF c \in NewViewCases
    THEN Bug = "cache_new_view_roster"
    ELSE Bug # "skip_roster_cache"
  ELSE FALSE

SpecNewViewTracked(c) == c = "valid_new_view"

ActualNewViewTracked(c) ==
  IF ActualAccept(c) /\ c \in NewViewCases
  THEN
    CASE c = "valid_new_view" -> Bug # "skip_new_view_track"
      [] c = "stale_new_view" -> Bug = "track_stale_new_view"
      [] OTHER -> FALSE
  ELSE FALSE

SpecPipelineRequested(c) == SpecAccept(c) /\ c # "stale_new_view"

ActualPipelineRequested(c) ==
  IF ActualAccept(c)
  THEN
    IF c = "stale_new_view"
    THEN Bug = "request_pipeline_for_stale_new_view"
    ELSE Bug # "skip_commit_pipeline_request"
  ELSE FALSE

SpecProgressTouched(c) == SpecAccept(c)

ActualProgressTouched(c) ==
  IF ActualAccept(c) THEN Bug # "skip_progress_touch" ELSE FALSE

BugModes == {
  "none",
  "accept_height_or_view_drop",
  "accept_locked_conflict",
  "accept_roster_missing",
  "record_duplicate",
  "accept_non_new_view_highest",
  "accept_chain_order_mismatch",
  "accept_bad_signature",
  "accept_new_view_missing_highest",
  "accept_new_view_bad_highest_epoch",
  "accept_new_view_bad_highest_phase",
  "accept_new_view_hash_mismatch",
  "accept_new_view_height_mismatch",
  "accept_new_view_local_metadata_mismatch",
  "record_same_slot_conflict",
  "drop_superseded_conflict",
  "record_deferred_conflict",
  "accept_same_key_conflict",
  "skip_defer_missing_context",
  "skip_roster_defer",
  "skip_double_vote_evidence",
  "evidence_on_superseded",
  "skip_cross_phase_evidence",
  "skip_qc_attempt_on_accept",
  "qc_attempt_on_reject",
  "cache_new_view_roster",
  "skip_roster_cache",
  "track_stale_new_view",
  "skip_new_view_track",
  "request_pipeline_for_stale_new_view",
  "skip_commit_pipeline_request",
  "skip_progress_touch"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ accepted \in BOOLEAN
  /\ recorded \in BOOLEAN
  /\ deferred \in BOOLEAN
  /\ dropped \in BOOLEAN
  /\ evidence \in BOOLEAN
  /\ qc_attempted \in BOOLEAN
  /\ roster_cached \in BOOLEAN
  /\ new_view_tracked \in BOOLEAN
  /\ pipeline_requested \in BOOLEAN
  /\ progress_touched \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ accepted = FALSE
  /\ recorded = FALSE
  /\ deferred = FALSE
  /\ dropped = FALSE
  /\ evidence = FALSE
  /\ qc_attempted = FALSE
  /\ roster_cached = FALSE
  /\ new_view_tracked = FALSE
  /\ pipeline_requested = FALSE
  /\ progress_touched = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ accepted' = ActualAccept(c)
  /\ recorded' = ActualRecorded(c)
  /\ deferred' = ActualDeferred(c)
  /\ dropped' = ActualDropped(c)
  /\ evidence' = ActualEvidence(c)
  /\ qc_attempted' = ActualQcAttempted(c)
  /\ roster_cached' = ActualRosterCached(c)
  /\ new_view_tracked' = ActualNewViewTracked(c)
  /\ pipeline_requested' = ActualPipelineRequested(c)
  /\ progress_touched' = ActualProgressTouched(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

AcceptMatchesSpec ==
  candidate = "none" \/ accepted = SpecAccept(candidate)

RecordMatchesSpec ==
  candidate = "none" \/ recorded = SpecRecorded(candidate)

DeferredMatchesSpec ==
  candidate = "none" \/ deferred = SpecDeferred(candidate)

DroppedMatchesSpec ==
  candidate = "none" \/ dropped = SpecDropped(candidate)

EvidenceMatchesSpec ==
  candidate = "none" \/ evidence = SpecEvidence(candidate)

QcAttemptMatchesSpec ==
  candidate = "none" \/ qc_attempted = SpecQcAttempted(candidate)

RosterCacheMatchesSpec ==
  candidate = "none" \/ roster_cached = SpecRosterCached(candidate)

NewViewTrackingMatchesSpec ==
  candidate = "none" \/ new_view_tracked = SpecNewViewTracked(candidate)

PipelineRequestMatchesSpec ==
  candidate = "none" \/ pipeline_requested = SpecPipelineRequested(candidate)

ProgressTouchMatchesSpec ==
  candidate = "none" \/ progress_touched = SpecProgressTouched(candidate)

AcceptedCasesAccepted ==
  candidate \in AcceptedCases => accepted

InvalidCasesRejected ==
  candidate \in InvalidCases => ~accepted

DeferredCasesDeferredOnly ==
  candidate \in DeferredCases =>
    /\ deferred
    /\ ~accepted
    /\ ~recorded
    /\ ~evidence
    /\ ~qc_attempted
    /\ ~roster_cached
    /\ ~new_view_tracked
    /\ ~pipeline_requested
    /\ ~progress_touched

DroppedVotesHaveNoSideEffects ==
  candidate \in InvalidCases =>
    /\ dropped
    /\ ~recorded
    /\ ~qc_attempted
    /\ ~roster_cached
    /\ ~new_view_tracked
    /\ ~pipeline_requested
    /\ ~progress_touched

RejectedConflictsPersistEvidence ==
  candidate \in {"same_slot_conflict", "same_key_conflict"} => evidence

SupersededConflictRecordsWithoutEvidence ==
  candidate = "same_slot_conflict_superseded" =>
    /\ accepted
    /\ recorded
    /\ ~deferred
    /\ ~evidence

DeferredConflictDoesNotRecordEvidence ==
  candidate = "same_slot_conflict_deferred" =>
    /\ deferred
    /\ ~recorded
    /\ ~evidence

CrossPhaseConflictRecordsAndPersistsEvidence ==
  candidate = "cross_phase_conflict" =>
    /\ accepted
    /\ recorded
    /\ evidence

NewViewVotesNeverCacheRoster ==
  candidate \in NewViewCases => ~roster_cached

AcceptedPrepareCommitCachesRoster ==
  candidate \in PrepareCommitCases /\ SpecAccept(candidate) => roster_cached

StaleNewViewAggregatesOnly ==
  candidate = "stale_new_view" =>
    /\ accepted
    /\ recorded
    /\ qc_attempted
    /\ ~new_view_tracked
    /\ ~pipeline_requested

ValidNewViewTracked ==
  candidate = "valid_new_view" => new_view_tracked

AcceptedVotesAttemptQc ==
  candidate \in AcceptedCases => qc_attempted

AcceptedVotesTouchProgress ==
  candidate \in AcceptedCases => progress_touched

AcceptedVotesRequestPipelineExceptStaleNewView ==
  candidate \in (AcceptedCases \ {"stale_new_view"}) => pipeline_requested

VoteAdmissionExactness ==
  /\ AcceptMatchesSpec
  /\ RecordMatchesSpec
  /\ DeferredMatchesSpec
  /\ DroppedMatchesSpec
  /\ EvidenceMatchesSpec
  /\ QcAttemptMatchesSpec
  /\ RosterCacheMatchesSpec
  /\ NewViewTrackingMatchesSpec
  /\ PipelineRequestMatchesSpec
  /\ ProgressTouchMatchesSpec
  /\ AcceptedCasesAccepted
  /\ InvalidCasesRejected
  /\ DeferredCasesDeferredOnly
  /\ DroppedVotesHaveNoSideEffects
  /\ RejectedConflictsPersistEvidence
  /\ SupersededConflictRecordsWithoutEvidence
  /\ DeferredConflictDoesNotRecordEvidence
  /\ CrossPhaseConflictRecordsAndPersistsEvidence
  /\ NewViewVotesNeverCacheRoster
  /\ AcceptedPrepareCommitCachesRoster
  /\ StaleNewViewAggregatesOnly
  /\ ValidNewViewTracked
  /\ AcceptedVotesAttemptQc
  /\ AcceptedVotesTouchProgress
  /\ AcceptedVotesRequestPipelineExceptStaleNewView

Safety == VoteAdmissionExactness

VoteAdmissionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VoteAdmissionExactness

NoBugInvariant == VoteAdmissionExactness

====
