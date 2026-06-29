---- MODULE SumeragiBlockSyncSelectedApplyGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for the selected-roster BlockSyncUpdate apply and
recovery-mode gate in `handle_block_sync_update(...)`.

After quorum admission, the live path computes:

  * whether a same-height/non-extending commit QC is allowed;
  * whether payload mismatch should preserve local frontier ownership;
  * whether the block-sync payload may supersede a conflicting frontier owner;
  * which `BlockSyncRecoveryMode` is passed into
    `handle_block_created_from_block_sync(...)`;
  * whether signed-quorum commit-QC repair is activated after block creation;
  * whether sparse next-height payload recovery requests a known-block commit
    QC; and
  * whether the block is ready for QC application or is recorded as an
    unapplied payload drop.

The model keeps the obligations boolean-only and expands the finite case set
explicitly so Apalache can constant-fold the CI check.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "selection_commit_qc_allows_nonextending",
  "incoming_qc_valid_allows_nonextending",
  "incoming_qc_usable_allows_nonextending",
  "no_qc_disallows_nonextending",
  "same_height_signature_conflict",
  "signature_quorum_supersedes_without_conflict",
  "incoming_qc_supersedes",
  "commit_cert_supersedes",
  "checkpoint_supersedes",
  "payload_only_no_authority",
  "commit_votes_recovery",
  "incoming_qc_recovery",
  "commit_cert_recovery",
  "checkpoint_recovery",
  "signed_quorum_frontier_repair",
  "preserve_payload_mismatch",
  "signed_quorum_commit_repair_active",
  "signed_quorum_repair_creation_error",
  "signed_quorum_repair_unknown_after",
  "signed_quorum_repair_no_signature_quorum",
  "signed_quorum_repair_not_frontier",
  "signed_quorum_repair_has_qc",
  "sparse_next_height_recovery",
  "sparse_known_before",
  "sparse_unknown_after",
  "sparse_has_commit_quorum",
  "ready_for_qc",
  "not_ready_creation_error",
  "not_ready_unknown_after"
}

SelectionCommitQc(c) ==
  c = "selection_commit_qc_allows_nonextending"

IncomingQcValidated(c) ==
  c = "incoming_qc_valid_allows_nonextending"

IncomingQcUsable(c) ==
  c \in {
    "incoming_qc_usable_allows_nonextending",
    "incoming_qc_supersedes",
    "incoming_qc_recovery"
  }

HasCommitVotes(c) ==
  c = "commit_votes_recovery"

CommitCertPresent(c) ==
  c \in {"commit_cert_supersedes", "commit_cert_recovery"}

CheckpointPresent(c) ==
  c \in {"checkpoint_supersedes", "checkpoint_recovery"}

IncomingQcObject(c) ==
  IncomingQcValidated(c) \/ IncomingQcUsable(c) \/ CommitCertPresent(c)

QcEvidencePresent(c) ==
  IncomingQcObject(c) \/ c = "ready_for_qc" \/ c = "signed_quorum_repair_has_qc"

BlockQuorumMet(c) ==
  c \in {
    "same_height_signature_conflict",
    "signature_quorum_supersedes_without_conflict",
    "signed_quorum_frontier_repair"
  }

LocalConflictingFrontierVote(c) ==
  c = "same_height_signature_conflict"

SignatureQuorumMet(c) ==
  c \in {
    "signed_quorum_frontier_repair",
    "signed_quorum_commit_repair_active",
    "signed_quorum_repair_creation_error",
    "signed_quorum_repair_unknown_after",
    "signed_quorum_repair_not_frontier",
    "signed_quorum_repair_has_qc"
  }

ExactContiguousFrontier(c) ==
  c \in {
    "signed_quorum_frontier_repair",
    "signed_quorum_commit_repair_active",
    "signed_quorum_repair_creation_error",
    "signed_quorum_repair_unknown_after",
    "signed_quorum_repair_no_signature_quorum",
    "signed_quorum_repair_has_qc",
    "sparse_next_height_recovery",
    "sparse_known_before",
    "sparse_unknown_after",
    "sparse_has_commit_quorum"
  }

MissingCommitQcRepairActive(c) ==
  c \in {
    "signed_quorum_commit_repair_active",
    "signed_quorum_repair_creation_error",
    "signed_quorum_repair_unknown_after",
    "signed_quorum_repair_no_signature_quorum",
    "signed_quorum_repair_not_frontier",
    "signed_quorum_repair_has_qc"
  }

CreationOk(c) ==
  ~(c \in {"signed_quorum_repair_creation_error", "not_ready_creation_error"})

BlockKnownBefore(c) ==
  c = "sparse_known_before"

BlockKnownAfter(c) ==
  ~(c \in {
      "signed_quorum_repair_unknown_after",
      "sparse_unknown_after",
      "not_ready_unknown_after"
    })

NextHeight(c) ==
  c \in {
    "sparse_next_height_recovery",
    "sparse_known_before",
    "sparse_unknown_after",
    "sparse_has_commit_quorum"
  }

BlockSignerBelowCommitQuorum(c) ==
  c # "sparse_has_commit_quorum"

PendingBlockMatchesNonInvalid(c) ==
  c = "signed_quorum_commit_repair_active"

SpecAllowNonextendingQc(c) ==
  SelectionCommitQc(c) \/ IncomingQcValidated(c) \/ IncomingQcUsable(c)

SpecSameHeightFrontierConflict(c) ==
  /\ BlockQuorumMet(c)
  /\ ~IncomingQcUsable(c)
  /\ ~CommitCertPresent(c)
  /\ ~CheckpointPresent(c)
  /\ LocalConflictingFrontierVote(c)

SpecPreserveOnPayloadMismatch(c) ==
  ~IncomingQcUsable(c) /\ ~CommitCertPresent(c) /\ ~CheckpointPresent(c)

SpecAuthoritativeSupersede(c) ==
  \/ IncomingQcUsable(c)
  \/ CommitCertPresent(c)
  \/ CheckpointPresent(c)
  \/ /\ BlockQuorumMet(c)
     /\ ~SpecSameHeightFrontierConflict(c)

SpecRecoveryCommitEvidence(c) ==
  HasCommitVotes(c) \/ IncomingQcUsable(c) \/ CommitCertPresent(c) \/ CheckpointPresent(c)

SpecRecoverySignedQuorum(c) ==
  ~SpecRecoveryCommitEvidence(c) /\ SpecAuthoritativeSupersede(c)

SpecRecoveryPayloadOnly(c) ==
  ~SpecRecoveryCommitEvidence(c) /\ ~SpecRecoverySignedQuorum(c)

SpecObservedEpochIncoming(c) ==
  SpecRecoveryCommitEvidence(c) /\ IncomingQcObject(c)

SpecObservedEpochCheckpoint(c) ==
  SpecRecoveryCommitEvidence(c) /\ ~IncomingQcObject(c) /\ CheckpointPresent(c)

SpecObservedEpochNone(c) ==
  SpecRecoveryCommitEvidence(c) /\ ~IncomingQcObject(c) /\ ~CheckpointPresent(c)

SpecAllowAbortedRevival(c) ==
  SpecRecoveryCommitEvidence(c)
    /\ (HasCommitVotes(c) \/ CommitCertPresent(c) \/ CheckpointPresent(c))

SpecHandleCreatedCalled(c) ==
  TRUE

SpecPassPreserveFlag(c) ==
  SpecPreserveOnPayloadMismatch(c)

SpecPassCommitEvidenceMode(c) ==
  SpecRecoveryCommitEvidence(c)

SpecPassSignedQuorumMode(c) ==
  SpecRecoverySignedQuorum(c)

SpecPassPayloadOnlyMode(c) ==
  SpecRecoveryPayloadOnly(c)

SpecSignedQuorumCommitRepairActive(c) ==
  /\ CreationOk(c)
  /\ BlockKnownAfter(c)
  /\ SignatureQuorumMet(c)
  /\ ExactContiguousFrontier(c)
  /\ ~QcEvidencePresent(c)
  /\ ~CommitCertPresent(c)
  /\ ~CheckpointPresent(c)
  /\ MissingCommitQcRepairActive(c)

SpecPendingCommitQcObserved(c) ==
  SpecSignedQuorumCommitRepairActive(c) /\ PendingBlockMatchesNonInvalid(c)

SpecFrontierCommitQcObserved(c) ==
  SpecSignedQuorumCommitRepairActive(c)

SpecClearMissingCommitQcRequest(c) ==
  SpecSignedQuorumCommitRepairActive(c)

SpecRequestCommitPipeline(c) ==
  SpecSignedQuorumCommitRepairActive(c)

SpecSparseNextHeightPayloadRecovered(c) ==
  /\ ~BlockKnownBefore(c)
  /\ BlockKnownAfter(c)
  /\ NextHeight(c)
  /\ BlockSignerBelowCommitQuorum(c)
  /\ ~IncomingQcUsable(c)
  /\ ~CommitCertPresent(c)
  /\ ~CheckpointPresent(c)

SpecRequestKnownBlockCommitQcRecovery(c) ==
  SpecSparseNextHeightPayloadRecovered(c)

SpecReadyForQc(c) ==
  CreationOk(c) /\ BlockKnownAfter(c)

SpecRecordPayloadUnappliedDrop(c) ==
  ~SpecReadyForQc(c)

SpecProcessCommitVotes(c) ==
  TRUE

SpecQcToApply(c) ==
  SpecReadyForQc(c) /\ QcEvidencePresent(c)

ActualAllowNonextendingQc(c) ==
  IF Bug = "selection_qc_disallows_nonextending"
     /\ c = "selection_commit_qc_allows_nonextending"
  THEN FALSE
  ELSE IF Bug = "incoming_valid_disallows_nonextending"
          /\ c = "incoming_qc_valid_allows_nonextending" THEN FALSE
  ELSE IF Bug = "incoming_usable_disallows_nonextending"
          /\ c = "incoming_qc_usable_allows_nonextending" THEN FALSE
  ELSE IF Bug = "no_qc_allows_nonextending"
          /\ c = "no_qc_disallows_nonextending" THEN TRUE
  ELSE SpecAllowNonextendingQc(c)

ActualSameHeightFrontierConflict(c) ==
  SpecSameHeightFrontierConflict(c)

ActualPreserveOnPayloadMismatch(c) ==
  IF Bug = "preserve_flag_with_qc"
     /\ c = "incoming_qc_supersedes"
  THEN TRUE
  ELSE IF Bug = "preserve_flag_missing_without_cert"
          /\ c = "preserve_payload_mismatch" THEN FALSE
  ELSE SpecPreserveOnPayloadMismatch(c)

ActualAuthoritativeSupersede(c) ==
  IF Bug = "signature_conflict_supersedes"
     /\ c = "same_height_signature_conflict"
  THEN TRUE
  ELSE IF Bug = "signature_quorum_no_conflict_not_supersede"
          /\ c = "signature_quorum_supersedes_without_conflict" THEN FALSE
  ELSE IF Bug = "incoming_qc_not_supersede"
          /\ c = "incoming_qc_supersedes" THEN FALSE
  ELSE IF Bug = "commit_cert_not_supersede"
          /\ c = "commit_cert_supersedes" THEN FALSE
  ELSE IF Bug = "checkpoint_not_supersede"
          /\ c = "checkpoint_supersedes" THEN FALSE
  ELSE SpecAuthoritativeSupersede(c)

ActualRecoveryCommitEvidence(c) ==
  IF Bug = "commit_votes_not_commit_evidence"
     /\ c = "commit_votes_recovery"
  THEN FALSE
  ELSE IF Bug = "incoming_qc_not_commit_evidence"
          /\ c = "incoming_qc_recovery" THEN FALSE
  ELSE IF Bug = "commit_cert_not_commit_evidence"
          /\ c = "commit_cert_recovery" THEN FALSE
  ELSE IF Bug = "checkpoint_not_commit_evidence"
          /\ c = "checkpoint_recovery" THEN FALSE
  ELSE SpecRecoveryCommitEvidence(c)

ActualRecoverySignedQuorum(c) ==
  IF Bug = "signed_quorum_uses_payload_only"
     /\ c = "signed_quorum_frontier_repair"
  THEN FALSE
  ELSE IF Bug = "payload_only_uses_signed_quorum"
          /\ c = "payload_only_no_authority" THEN TRUE
  ELSE SpecRecoverySignedQuorum(c)

ActualRecoveryPayloadOnly(c) ==
  IF Bug = "signed_quorum_uses_payload_only"
     /\ c = "signed_quorum_frontier_repair"
  THEN TRUE
  ELSE IF Bug = "payload_only_uses_signed_quorum"
          /\ c = "payload_only_no_authority" THEN FALSE
  ELSE SpecRecoveryPayloadOnly(c)

ActualObservedEpochIncoming(c) ==
  IF Bug = "incoming_epoch_dropped"
     /\ c = "incoming_qc_recovery"
  THEN FALSE
  ELSE SpecObservedEpochIncoming(c)

ActualObservedEpochCheckpoint(c) ==
  IF Bug = "checkpoint_epoch_dropped"
     /\ c = "checkpoint_recovery"
  THEN FALSE
  ELSE SpecObservedEpochCheckpoint(c)

ActualObservedEpochNone(c) ==
  SpecObservedEpochNone(c)

ActualAllowAbortedRevival(c) ==
  IF Bug = "incoming_qc_allows_aborted_revival"
     /\ c = "incoming_qc_recovery"
  THEN TRUE
  ELSE IF Bug = "commit_votes_disallow_aborted_revival"
          /\ c = "commit_votes_recovery" THEN FALSE
  ELSE SpecAllowAbortedRevival(c)

ActualHandleCreatedCalled(c) ==
  SpecHandleCreatedCalled(c)

ActualPassPreserveFlag(c) ==
  ActualPreserveOnPayloadMismatch(c)

ActualPassCommitEvidenceMode(c) ==
  ActualRecoveryCommitEvidence(c)

ActualPassSignedQuorumMode(c) ==
  ActualRecoverySignedQuorum(c)

ActualPassPayloadOnlyMode(c) ==
  ActualRecoveryPayloadOnly(c)

ActualSignedQuorumCommitRepairActive(c) ==
  IF Bug = "signed_quorum_repair_not_active"
     /\ c = "signed_quorum_commit_repair_active"
  THEN FALSE
  ELSE IF Bug = "signed_quorum_repair_with_qc"
          /\ c = "signed_quorum_repair_has_qc" THEN TRUE
  ELSE IF Bug = "signed_quorum_repair_without_creation_ok"
          /\ c = "signed_quorum_repair_creation_error" THEN TRUE
  ELSE SpecSignedQuorumCommitRepairActive(c)

ActualPendingCommitQcObserved(c) ==
  SpecPendingCommitQcObserved(c)

ActualFrontierCommitQcObserved(c) ==
  IF Bug = "signed_quorum_no_frontier_note"
     /\ c = "signed_quorum_commit_repair_active"
  THEN FALSE
  ELSE SpecFrontierCommitQcObserved(c)

ActualClearMissingCommitQcRequest(c) ==
  IF Bug = "signed_quorum_no_clear"
     /\ c = "signed_quorum_commit_repair_active"
  THEN FALSE
  ELSE SpecClearMissingCommitQcRequest(c)

ActualRequestCommitPipeline(c) ==
  IF Bug = "signed_quorum_no_pipeline"
     /\ c = "signed_quorum_commit_repair_active"
  THEN FALSE
  ELSE SpecRequestCommitPipeline(c)

ActualSparseNextHeightPayloadRecovered(c) ==
  IF Bug = "sparse_recovery_known_before"
     /\ c = "sparse_known_before"
  THEN TRUE
  ELSE IF Bug = "sparse_recovery_with_commit_quorum"
          /\ c = "sparse_has_commit_quorum" THEN TRUE
  ELSE SpecSparseNextHeightPayloadRecovered(c)

ActualRequestKnownBlockCommitQcRecovery(c) ==
  IF Bug = "sparse_recovery_not_requested"
     /\ c = "sparse_next_height_recovery"
  THEN FALSE
  ELSE ActualSparseNextHeightPayloadRecovered(c)

ActualReadyForQc(c) ==
  IF Bug = "ready_with_creation_error"
     /\ c = "not_ready_creation_error"
  THEN TRUE
  ELSE IF Bug = "ready_without_known"
          /\ c = "not_ready_unknown_after" THEN TRUE
  ELSE SpecReadyForQc(c)

ActualRecordPayloadUnappliedDrop(c) ==
  IF Bug = "payload_unapplied_no_status"
     /\ c = "not_ready_unknown_after"
  THEN FALSE
  ELSE SpecRecordPayloadUnappliedDrop(c)

ActualProcessCommitVotes(c) ==
  SpecProcessCommitVotes(c)

ActualQcToApply(c) ==
  IF Bug = "qc_not_applied_when_ready"
     /\ c = "ready_for_qc"
  THEN FALSE
  ELSE IF Bug = "qc_applied_when_not_ready"
          /\ c = "not_ready_unknown_after" THEN TRUE
  ELSE SpecQcToApply(c)

Matches(c) ==
  /\ ActualAllowNonextendingQc(c) = SpecAllowNonextendingQc(c)
  /\ ActualSameHeightFrontierConflict(c) = SpecSameHeightFrontierConflict(c)
  /\ ActualPreserveOnPayloadMismatch(c) = SpecPreserveOnPayloadMismatch(c)
  /\ ActualAuthoritativeSupersede(c) = SpecAuthoritativeSupersede(c)
  /\ ActualRecoveryCommitEvidence(c) = SpecRecoveryCommitEvidence(c)
  /\ ActualRecoverySignedQuorum(c) = SpecRecoverySignedQuorum(c)
  /\ ActualRecoveryPayloadOnly(c) = SpecRecoveryPayloadOnly(c)
  /\ ActualObservedEpochIncoming(c) = SpecObservedEpochIncoming(c)
  /\ ActualObservedEpochCheckpoint(c) = SpecObservedEpochCheckpoint(c)
  /\ ActualObservedEpochNone(c) = SpecObservedEpochNone(c)
  /\ ActualAllowAbortedRevival(c) = SpecAllowAbortedRevival(c)
  /\ ActualHandleCreatedCalled(c) = SpecHandleCreatedCalled(c)
  /\ ActualPassPreserveFlag(c) = SpecPassPreserveFlag(c)
  /\ ActualPassCommitEvidenceMode(c) = SpecPassCommitEvidenceMode(c)
  /\ ActualPassSignedQuorumMode(c) = SpecPassSignedQuorumMode(c)
  /\ ActualPassPayloadOnlyMode(c) = SpecPassPayloadOnlyMode(c)
  /\ ActualSignedQuorumCommitRepairActive(c) = SpecSignedQuorumCommitRepairActive(c)
  /\ ActualPendingCommitQcObserved(c) = SpecPendingCommitQcObserved(c)
  /\ ActualFrontierCommitQcObserved(c) = SpecFrontierCommitQcObserved(c)
  /\ ActualClearMissingCommitQcRequest(c) = SpecClearMissingCommitQcRequest(c)
  /\ ActualRequestCommitPipeline(c) = SpecRequestCommitPipeline(c)
  /\ ActualSparseNextHeightPayloadRecovered(c) = SpecSparseNextHeightPayloadRecovered(c)
  /\ ActualRequestKnownBlockCommitQcRecovery(c) = SpecRequestKnownBlockCommitQcRecovery(c)
  /\ ActualReadyForQc(c) = SpecReadyForQc(c)
  /\ ActualRecordPayloadUnappliedDrop(c) = SpecRecordPayloadUnappliedDrop(c)
  /\ ActualProcessCommitVotes(c) = SpecProcessCommitVotes(c)
  /\ ActualQcToApply(c) = SpecQcToApply(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "selection_qc_disallows_nonextending",
       "incoming_valid_disallows_nonextending",
       "incoming_usable_disallows_nonextending",
       "no_qc_allows_nonextending",
       "signature_conflict_supersedes",
       "signature_quorum_no_conflict_not_supersede",
       "incoming_qc_not_supersede",
       "commit_cert_not_supersede",
       "checkpoint_not_supersede",
       "commit_votes_not_commit_evidence",
       "incoming_qc_not_commit_evidence",
       "commit_cert_not_commit_evidence",
       "checkpoint_not_commit_evidence",
       "incoming_qc_allows_aborted_revival",
       "commit_votes_disallow_aborted_revival",
       "incoming_epoch_dropped",
       "checkpoint_epoch_dropped",
       "signed_quorum_uses_payload_only",
       "payload_only_uses_signed_quorum",
       "preserve_flag_with_qc",
       "preserve_flag_missing_without_cert",
       "signed_quorum_repair_not_active",
       "signed_quorum_repair_with_qc",
       "signed_quorum_repair_without_creation_ok",
       "signed_quorum_no_frontier_note",
       "signed_quorum_no_clear",
       "signed_quorum_no_pipeline",
       "sparse_recovery_not_requested",
       "sparse_recovery_known_before",
       "sparse_recovery_with_commit_quorum",
       "ready_with_creation_error",
       "ready_without_known",
       "payload_unapplied_no_status",
       "qc_not_applied_when_ready",
       "qc_applied_when_not_ready"
     }
  /\ checked = 0

NonextendingAndOwner ==
  /\ ActualAllowNonextendingQc("selection_commit_qc_allows_nonextending")
       = SpecAllowNonextendingQc("selection_commit_qc_allows_nonextending")
  /\ ActualAllowNonextendingQc("incoming_qc_valid_allows_nonextending")
       = SpecAllowNonextendingQc("incoming_qc_valid_allows_nonextending")
  /\ ActualAllowNonextendingQc("incoming_qc_usable_allows_nonextending")
       = SpecAllowNonextendingQc("incoming_qc_usable_allows_nonextending")
  /\ ActualAllowNonextendingQc("no_qc_disallows_nonextending")
       = SpecAllowNonextendingQc("no_qc_disallows_nonextending")
  /\ ActualSameHeightFrontierConflict("same_height_signature_conflict")
       = SpecSameHeightFrontierConflict("same_height_signature_conflict")
  /\ ActualAuthoritativeSupersede("same_height_signature_conflict")
       = SpecAuthoritativeSupersede("same_height_signature_conflict")
  /\ ActualAuthoritativeSupersede("signature_quorum_supersedes_without_conflict")
       = SpecAuthoritativeSupersede("signature_quorum_supersedes_without_conflict")
  /\ ActualAuthoritativeSupersede("incoming_qc_supersedes")
       = SpecAuthoritativeSupersede("incoming_qc_supersedes")
  /\ ActualAuthoritativeSupersede("commit_cert_supersedes")
       = SpecAuthoritativeSupersede("commit_cert_supersedes")
  /\ ActualAuthoritativeSupersede("checkpoint_supersedes")
       = SpecAuthoritativeSupersede("checkpoint_supersedes")
  /\ ActualPassPreserveFlag("incoming_qc_supersedes") = SpecPassPreserveFlag("incoming_qc_supersedes")
  /\ ActualPassPreserveFlag("preserve_payload_mismatch") = SpecPassPreserveFlag("preserve_payload_mismatch")

RecoveryMode ==
  /\ ActualRecoveryCommitEvidence("commit_votes_recovery") = SpecRecoveryCommitEvidence("commit_votes_recovery")
  /\ ActualRecoveryCommitEvidence("incoming_qc_recovery") = SpecRecoveryCommitEvidence("incoming_qc_recovery")
  /\ ActualRecoveryCommitEvidence("commit_cert_recovery") = SpecRecoveryCommitEvidence("commit_cert_recovery")
  /\ ActualRecoveryCommitEvidence("checkpoint_recovery") = SpecRecoveryCommitEvidence("checkpoint_recovery")
  /\ ActualObservedEpochIncoming("incoming_qc_recovery") = SpecObservedEpochIncoming("incoming_qc_recovery")
  /\ ActualObservedEpochCheckpoint("checkpoint_recovery") = SpecObservedEpochCheckpoint("checkpoint_recovery")
  /\ ActualAllowAbortedRevival("incoming_qc_recovery") = SpecAllowAbortedRevival("incoming_qc_recovery")
  /\ ActualAllowAbortedRevival("commit_votes_recovery") = SpecAllowAbortedRevival("commit_votes_recovery")
  /\ ActualPassCommitEvidenceMode("incoming_qc_recovery") = SpecPassCommitEvidenceMode("incoming_qc_recovery")
  /\ ActualPassCommitEvidenceMode("checkpoint_recovery") = SpecPassCommitEvidenceMode("checkpoint_recovery")
  /\ ActualRecoverySignedQuorum("signed_quorum_frontier_repair") = SpecRecoverySignedQuorum("signed_quorum_frontier_repair")
  /\ ActualRecoveryPayloadOnly("signed_quorum_frontier_repair") = SpecRecoveryPayloadOnly("signed_quorum_frontier_repair")
  /\ ActualPassSignedQuorumMode("signed_quorum_frontier_repair") = SpecPassSignedQuorumMode("signed_quorum_frontier_repair")
  /\ ActualRecoverySignedQuorum("payload_only_no_authority") = SpecRecoverySignedQuorum("payload_only_no_authority")
  /\ ActualRecoveryPayloadOnly("payload_only_no_authority") = SpecRecoveryPayloadOnly("payload_only_no_authority")
  /\ ActualPassPayloadOnlyMode("payload_only_no_authority") = SpecPassPayloadOnlyMode("payload_only_no_authority")

SignedQuorumRepair ==
  /\ ActualSignedQuorumCommitRepairActive("signed_quorum_commit_repair_active")
       = SpecSignedQuorumCommitRepairActive("signed_quorum_commit_repair_active")
  /\ ActualSignedQuorumCommitRepairActive("signed_quorum_repair_has_qc")
       = SpecSignedQuorumCommitRepairActive("signed_quorum_repair_has_qc")
  /\ ActualSignedQuorumCommitRepairActive("signed_quorum_repair_creation_error")
       = SpecSignedQuorumCommitRepairActive("signed_quorum_repair_creation_error")
  /\ ActualSignedQuorumCommitRepairActive("signed_quorum_repair_unknown_after")
       = SpecSignedQuorumCommitRepairActive("signed_quorum_repair_unknown_after")
  /\ ActualSignedQuorumCommitRepairActive("signed_quorum_repair_no_signature_quorum")
       = SpecSignedQuorumCommitRepairActive("signed_quorum_repair_no_signature_quorum")
  /\ ActualSignedQuorumCommitRepairActive("signed_quorum_repair_not_frontier")
       = SpecSignedQuorumCommitRepairActive("signed_quorum_repair_not_frontier")
  /\ ActualFrontierCommitQcObserved("signed_quorum_commit_repair_active")
       = SpecFrontierCommitQcObserved("signed_quorum_commit_repair_active")
  /\ ActualClearMissingCommitQcRequest("signed_quorum_commit_repair_active")
       = SpecClearMissingCommitQcRequest("signed_quorum_commit_repair_active")
  /\ ActualRequestCommitPipeline("signed_quorum_commit_repair_active")
       = SpecRequestCommitPipeline("signed_quorum_commit_repair_active")

SparseAndReady ==
  /\ ActualRequestKnownBlockCommitQcRecovery("sparse_next_height_recovery")
       = SpecRequestKnownBlockCommitQcRecovery("sparse_next_height_recovery")
  /\ ActualSparseNextHeightPayloadRecovered("sparse_known_before")
       = SpecSparseNextHeightPayloadRecovered("sparse_known_before")
  /\ ActualSparseNextHeightPayloadRecovered("sparse_unknown_after")
       = SpecSparseNextHeightPayloadRecovered("sparse_unknown_after")
  /\ ActualSparseNextHeightPayloadRecovered("sparse_has_commit_quorum")
       = SpecSparseNextHeightPayloadRecovered("sparse_has_commit_quorum")
  /\ ActualReadyForQc("ready_for_qc") = SpecReadyForQc("ready_for_qc")
  /\ ActualReadyForQc("not_ready_creation_error") = SpecReadyForQc("not_ready_creation_error")
  /\ ActualReadyForQc("not_ready_unknown_after") = SpecReadyForQc("not_ready_unknown_after")
  /\ ActualRecordPayloadUnappliedDrop("not_ready_unknown_after")
       = SpecRecordPayloadUnappliedDrop("not_ready_unknown_after")
  /\ ActualProcessCommitVotes("ready_for_qc") = SpecProcessCommitVotes("ready_for_qc")
  /\ ActualQcToApply("ready_for_qc") = SpecQcToApply("ready_for_qc")
  /\ ActualQcToApply("not_ready_unknown_after") = SpecQcToApply("not_ready_unknown_after")

BlockSyncSelectedApplyExactness ==
  /\ NonextendingAndOwner
  /\ RecoveryMode
  /\ SignedQuorumRepair
  /\ SparseAndReady

BlockSyncSelectedApplyCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncSelectedApplyExactness

SafetyFast ==
  BlockSyncSelectedApplyExactness

=============================================================================
====
