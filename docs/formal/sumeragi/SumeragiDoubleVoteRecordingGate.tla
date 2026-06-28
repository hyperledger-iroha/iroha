---- MODULE SumeragiDoubleVoteRecordingGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for double-vote detection and recording.

This slice pins:
- `check_double_vote(...)` and `check_double_vote_with_context(...)` detection
  gates over height, epoch, topology-resolved signer identity, block/root
  conflicts, phase-pair support, and canonical evidence kind/key selection,
- cross-phase PREPARE/COMMIT equivocation as `DoubleCommit` evidence,
- root-only COMMIT equivocation as a real double-vote even when both votes name
  the same block hash,
- `record_double_vote(...)` control flow: no evidence has no side effects, store
  rejection stops persistence, successful store insertion calls persistence,
  and persistence rejection returns false while leaving the in-memory canonical
  key recorded.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoEvidence == "none"
DoublePrepare == "DoublePrepare"
DoubleCommit == "DoubleCommit"

DetectionCases == {
  "bare_prepare_conflict",
  "bare_commit_block_conflict",
  "bare_commit_root_conflict",
  "bare_cross_phase_prepare_commit",
  "bare_cross_phase_commit_prepare",
  "bare_swapped_prepare_conflict",
  "bare_same_hash_prepare",
  "bare_same_hash_commit_same_roots",
  "bare_height_mismatch",
  "bare_epoch_mismatch",
  "bare_signer_mismatch",
  "bare_bad_phase_pair",
  "ctx_same_peer_same_index",
  "ctx_same_peer_rotated_index",
  "ctx_cross_view_same_peer",
  "ctx_same_raw_different_peer",
  "ctx_out_of_range_first",
  "ctx_out_of_range_second",
  "ctx_nonconflict"
}

RecordCases == {
  "record_no_evidence",
  "record_new_valid",
  "record_store_duplicate",
  "record_store_validation_reject",
  "record_persist_duplicate_fresh_store",
  "record_persist_horizon_reject",
  "record_swapped_duplicate",
  "record_cross_phase_new",
  "record_commit_root_new"
}

SpecEmits(c) ==
  c \in {
    "bare_prepare_conflict",
    "bare_commit_block_conflict",
    "bare_commit_root_conflict",
    "bare_cross_phase_prepare_commit",
    "bare_cross_phase_commit_prepare",
    "bare_swapped_prepare_conflict",
    "ctx_same_peer_same_index",
    "ctx_same_peer_rotated_index",
    "ctx_cross_view_same_peer"
  }

ActualEmits(c) ==
  CASE Bug = "detect_height_mismatch_emits"
       /\ c = "bare_height_mismatch" -> TRUE
    [] Bug = "detect_epoch_mismatch_emits"
       /\ c = "bare_epoch_mismatch" -> TRUE
    [] Bug = "detect_signer_mismatch_emits"
       /\ c = "bare_signer_mismatch" -> TRUE
    [] Bug = "detect_same_hash_prepare_emits"
       /\ c = "bare_same_hash_prepare" -> TRUE
    [] Bug = "detect_same_hash_commit_roots_emit"
       /\ c = "bare_same_hash_commit_same_roots" -> TRUE
    [] Bug = "detect_bad_phase_emits"
       /\ c = "bare_bad_phase_pair" -> TRUE
    [] Bug = "detect_commit_root_ignored"
       /\ c = "bare_commit_root_conflict" -> FALSE
    [] Bug = "detect_cross_phase_rejected"
       /\ c = "bare_cross_phase_prepare_commit" -> FALSE
    [] Bug = "detect_rejects_rotated_same_peer"
       /\ c = "ctx_same_peer_rotated_index" -> FALSE
    [] Bug = "detect_rejects_cross_view_same_peer"
       /\ c = "ctx_cross_view_same_peer" -> FALSE
    [] Bug = "detect_accepts_same_raw_different_peer"
       /\ c = "ctx_same_raw_different_peer" -> TRUE
    [] Bug = "detect_out_of_range_first_emits"
       /\ c = "ctx_out_of_range_first" -> TRUE
    [] Bug = "detect_out_of_range_second_emits"
       /\ c = "ctx_out_of_range_second" -> TRUE
    [] OTHER -> SpecEmits(c)

SpecKind(c) ==
  CASE c \in {
         "bare_prepare_conflict",
         "bare_swapped_prepare_conflict",
         "ctx_same_peer_same_index",
         "ctx_same_peer_rotated_index",
         "ctx_cross_view_same_peer"
       } -> DoublePrepare
    [] c \in {
         "bare_commit_block_conflict",
         "bare_commit_root_conflict",
         "bare_cross_phase_prepare_commit",
         "bare_cross_phase_commit_prepare"
       } -> DoubleCommit
    [] OTHER -> NoEvidence

ActualKind(c) ==
  CASE Bug = "detect_cross_phase_prepare_kind"
       /\ c = "bare_cross_phase_prepare_commit" -> DoublePrepare
    [] Bug = "detect_commit_block_prepare_kind"
       /\ c = "bare_commit_block_conflict" -> DoublePrepare
    [] OTHER -> SpecKind(c)

SpecKey(c) ==
  CASE c \in {
         "bare_prepare_conflict",
         "bare_swapped_prepare_conflict",
         "ctx_same_peer_same_index",
         "ctx_same_peer_rotated_index",
         "ctx_cross_view_same_peer"
       } -> "kind_double_prepare|vote_a|vote_b"
    [] c = "bare_commit_block_conflict" -> "kind_double_commit|block_1|block_2"
    [] c = "bare_commit_root_conflict" -> "kind_double_commit|root_1|root_2"
    [] c \in {
         "bare_cross_phase_prepare_commit",
         "bare_cross_phase_commit_prepare"
       } -> "kind_double_commit|prepare_vote|commit_vote"
    [] OTHER -> "none"

ActualKey(c) ==
  CASE Bug = "detect_swapped_not_canonical"
       /\ c = "bare_swapped_prepare_conflict" -> "kind_double_prepare|vote_b|vote_a"
    [] Bug = "detect_cross_phase_wrong_key"
       /\ c = "bare_cross_phase_commit_prepare" -> "kind_double_commit|commit_vote|prepare_vote"
    [] OTHER -> SpecKey(c)

SpecRecordReturn(c) ==
  c = "record_new_valid" \/ c = "record_cross_phase_new" \/ c = "record_commit_root_new"

ActualRecordReturn(c) ==
  CASE Bug = "record_no_evidence_returns_true"
       /\ c = "record_no_evidence" -> TRUE
    [] Bug = "record_store_duplicate_returns_true"
       /\ c = "record_store_duplicate" -> TRUE
    [] Bug = "record_new_valid_returns_false"
       /\ c = "record_new_valid" -> FALSE
    [] Bug = "record_persist_duplicate_returns_true"
       /\ c = "record_persist_duplicate_fresh_store" -> TRUE
    [] Bug = "record_horizon_reject_returns_true"
       /\ c = "record_persist_horizon_reject" -> TRUE
    [] OTHER -> SpecRecordReturn(c)

SpecStoreInserted(c) ==
  c \in {
    "record_new_valid",
    "record_persist_duplicate_fresh_store",
    "record_persist_horizon_reject",
    "record_cross_phase_new",
    "record_commit_root_new"
  }

ActualStoreInserted(c) ==
  CASE Bug = "record_no_evidence_stores"
       /\ c = "record_no_evidence" -> TRUE
    [] Bug = "record_new_valid_skips_store"
       /\ c = "record_new_valid" -> FALSE
    [] Bug = "record_persist_duplicate_skips_store"
       /\ c = "record_persist_duplicate_fresh_store" -> FALSE
    [] Bug = "record_horizon_reject_skips_store"
       /\ c = "record_persist_horizon_reject" -> FALSE
    [] OTHER -> SpecStoreInserted(c)

SpecPersistCalled(c) ==
  SpecStoreInserted(c)

ActualPersistCalled(c) ==
  CASE Bug = "record_no_evidence_persists"
       /\ c = "record_no_evidence" -> TRUE
    [] Bug = "record_store_duplicate_persists"
       /\ c = "record_store_duplicate" -> TRUE
    [] Bug = "record_store_validation_persists"
       /\ c = "record_store_validation_reject" -> TRUE
    [] Bug = "record_new_valid_skips_persist"
       /\ c = "record_new_valid" -> FALSE
    [] Bug = "record_persist_duplicate_not_called"
       /\ c = "record_persist_duplicate_fresh_store" -> FALSE
    [] Bug = "record_horizon_reject_not_called"
       /\ c = "record_persist_horizon_reject" -> FALSE
    [] Bug = "record_swapped_duplicate_persists"
       /\ c = "record_swapped_duplicate" -> TRUE
    [] OTHER -> SpecPersistCalled(c)

SpecPersisted(c) ==
  c \in {"record_new_valid", "record_cross_phase_new", "record_commit_root_new"}

ActualPersisted(c) ==
  CASE Bug = "record_persist_duplicate_persisted"
       /\ c = "record_persist_duplicate_fresh_store" -> TRUE
    [] Bug = "record_horizon_reject_persisted"
       /\ c = "record_persist_horizon_reject" -> TRUE
    [] OTHER -> SpecPersisted(c)

SpecRecordKey(c) ==
  CASE c \in {
         "record_new_valid",
         "record_persist_duplicate_fresh_store",
         "record_persist_horizon_reject",
         "record_swapped_duplicate"
       } -> "kind_double_prepare|vote_a|vote_b"
    [] c = "record_cross_phase_new" -> "kind_double_commit|prepare_vote|commit_vote"
    [] c = "record_commit_root_new" -> "kind_double_commit|root_1|root_2"
    [] OTHER -> "none"

ActualRecordKey(c) ==
  CASE Bug = "record_cross_phase_wrong_key"
       /\ c = "record_cross_phase_new" -> "kind_double_commit|commit_vote|prepare_vote"
    [] Bug = "record_commit_root_wrong_kind"
       /\ c = "record_commit_root_new" -> "kind_double_prepare|root_1|root_2"
    [] OTHER -> SpecRecordKey(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "detect_height_mismatch_emits",
       "detect_epoch_mismatch_emits",
       "detect_signer_mismatch_emits",
       "detect_same_hash_prepare_emits",
       "detect_same_hash_commit_roots_emit",
       "detect_bad_phase_emits",
       "detect_commit_root_ignored",
       "detect_cross_phase_rejected",
       "detect_rejects_rotated_same_peer",
       "detect_rejects_cross_view_same_peer",
       "detect_accepts_same_raw_different_peer",
       "detect_out_of_range_first_emits",
       "detect_out_of_range_second_emits",
       "detect_cross_phase_prepare_kind",
       "detect_commit_block_prepare_kind",
       "detect_swapped_not_canonical",
       "detect_cross_phase_wrong_key",
       "record_no_evidence_returns_true",
       "record_no_evidence_stores",
       "record_no_evidence_persists",
       "record_store_duplicate_returns_true",
       "record_store_duplicate_persists",
       "record_store_validation_persists",
       "record_new_valid_returns_false",
       "record_new_valid_skips_store",
       "record_new_valid_skips_persist",
       "record_persist_duplicate_returns_true",
       "record_persist_duplicate_skips_store",
       "record_persist_duplicate_not_called",
       "record_persist_duplicate_persisted",
       "record_horizon_reject_returns_true",
       "record_horizon_reject_skips_store",
       "record_horizon_reject_not_called",
       "record_horizon_reject_persisted",
       "record_swapped_duplicate_persists",
       "record_cross_phase_wrong_key",
       "record_commit_root_wrong_kind"
     }
  /\ checked = 0

SpecDetect(c) ==
  <<SpecEmits(c), SpecKind(c), SpecKey(c)>>

ActualDetect(c) ==
  <<ActualEmits(c), ActualKind(c), ActualKey(c)>>

DetectMatches(c) ==
  ActualDetect(c) = SpecDetect(c)

SpecRecord(c) ==
  <<SpecRecordReturn(c), SpecStoreInserted(c), SpecPersistCalled(c),
    SpecPersisted(c), SpecRecordKey(c)>>

ActualRecord(c) ==
  <<ActualRecordReturn(c), ActualStoreInserted(c), ActualPersistCalled(c),
    ActualPersisted(c), ActualRecordKey(c)>>

RecordMatches(c) ==
  ActualRecord(c) = SpecRecord(c)

DoubleVoteRecordingMatchesSpec ==
  /\ DetectMatches("bare_prepare_conflict")
  /\ DetectMatches("bare_commit_block_conflict")
  /\ DetectMatches("bare_commit_root_conflict")
  /\ DetectMatches("bare_cross_phase_prepare_commit")
  /\ DetectMatches("bare_cross_phase_commit_prepare")
  /\ DetectMatches("bare_swapped_prepare_conflict")
  /\ DetectMatches("bare_same_hash_prepare")
  /\ DetectMatches("bare_same_hash_commit_same_roots")
  /\ DetectMatches("bare_height_mismatch")
  /\ DetectMatches("bare_epoch_mismatch")
  /\ DetectMatches("bare_signer_mismatch")
  /\ DetectMatches("bare_bad_phase_pair")
  /\ DetectMatches("ctx_same_peer_same_index")
  /\ DetectMatches("ctx_same_peer_rotated_index")
  /\ DetectMatches("ctx_cross_view_same_peer")
  /\ DetectMatches("ctx_same_raw_different_peer")
  /\ DetectMatches("ctx_out_of_range_first")
  /\ DetectMatches("ctx_out_of_range_second")
  /\ DetectMatches("ctx_nonconflict")
  /\ RecordMatches("record_no_evidence")
  /\ RecordMatches("record_new_valid")
  /\ RecordMatches("record_store_duplicate")
  /\ RecordMatches("record_store_validation_reject")
  /\ RecordMatches("record_persist_duplicate_fresh_store")
  /\ RecordMatches("record_persist_horizon_reject")
  /\ RecordMatches("record_swapped_duplicate")
  /\ RecordMatches("record_cross_phase_new")
  /\ RecordMatches("record_commit_root_new")

DoubleVoteRecordingExactness ==
  /\ DoubleVoteRecordingMatchesSpec
DoubleVoteRecordingCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DoubleVoteRecordingExactness

SafetyFast ==
  DoubleVoteRecordingExactness

BugDetectHeightMismatchEmits ==
  DetectMatches("bare_height_mismatch")

BugDetectEpochMismatchEmits ==
  DetectMatches("bare_epoch_mismatch")

BugDetectSignerMismatchEmits ==
  DetectMatches("bare_signer_mismatch")

BugDetectSameHashPrepareEmits ==
  DetectMatches("bare_same_hash_prepare")

BugDetectSameHashCommitRootsEmit ==
  DetectMatches("bare_same_hash_commit_same_roots")

BugDetectBadPhaseEmits ==
  DetectMatches("bare_bad_phase_pair")

BugDetectCommitRootIgnored ==
  DetectMatches("bare_commit_root_conflict")

BugDetectCrossPhaseRejected ==
  DetectMatches("bare_cross_phase_prepare_commit")

BugDetectRejectsRotatedSamePeer ==
  DetectMatches("ctx_same_peer_rotated_index")

BugDetectRejectsCrossViewSamePeer ==
  DetectMatches("ctx_cross_view_same_peer")

BugDetectAcceptsSameRawDifferentPeer ==
  DetectMatches("ctx_same_raw_different_peer")

BugDetectOutOfRangeFirstEmits ==
  DetectMatches("ctx_out_of_range_first")

BugDetectOutOfRangeSecondEmits ==
  DetectMatches("ctx_out_of_range_second")

BugDetectCrossPhasePrepareKind ==
  DetectMatches("bare_cross_phase_prepare_commit")

BugDetectCommitBlockPrepareKind ==
  DetectMatches("bare_commit_block_conflict")

BugDetectSwappedNotCanonical ==
  DetectMatches("bare_swapped_prepare_conflict")

BugDetectCrossPhaseWrongKey ==
  DetectMatches("bare_cross_phase_commit_prepare")

BugRecordNoEvidenceReturnsTrue ==
  RecordMatches("record_no_evidence")

BugRecordNoEvidenceStores ==
  RecordMatches("record_no_evidence")

BugRecordNoEvidencePersists ==
  RecordMatches("record_no_evidence")

BugRecordStoreDuplicateReturnsTrue ==
  RecordMatches("record_store_duplicate")

BugRecordStoreDuplicatePersists ==
  RecordMatches("record_store_duplicate")

BugRecordStoreValidationPersists ==
  RecordMatches("record_store_validation_reject")

BugRecordNewValidReturnsFalse ==
  RecordMatches("record_new_valid")

BugRecordNewValidSkipsStore ==
  RecordMatches("record_new_valid")

BugRecordNewValidSkipsPersist ==
  RecordMatches("record_new_valid")

BugRecordPersistDuplicateReturnsTrue ==
  RecordMatches("record_persist_duplicate_fresh_store")

BugRecordPersistDuplicateSkipsStore ==
  RecordMatches("record_persist_duplicate_fresh_store")

BugRecordPersistDuplicateNotCalled ==
  RecordMatches("record_persist_duplicate_fresh_store")

BugRecordPersistDuplicatePersisted ==
  RecordMatches("record_persist_duplicate_fresh_store")

BugRecordHorizonRejectReturnsTrue ==
  RecordMatches("record_persist_horizon_reject")

BugRecordHorizonRejectSkipsStore ==
  RecordMatches("record_persist_horizon_reject")

BugRecordHorizonRejectNotCalled ==
  RecordMatches("record_persist_horizon_reject")

BugRecordHorizonRejectPersisted ==
  RecordMatches("record_persist_horizon_reject")

BugRecordSwappedDuplicatePersists ==
  RecordMatches("record_swapped_duplicate")

BugRecordCrossPhaseWrongKey ==
  RecordMatches("record_cross_phase_new")

BugRecordCommitRootWrongKind ==
  RecordMatches("record_commit_root_new")

====
