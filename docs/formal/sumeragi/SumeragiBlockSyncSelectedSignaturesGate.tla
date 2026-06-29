---- MODULE SumeragiBlockSyncSelectedSignaturesGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for the selected-roster signature gate in
`handle_block_sync_update(...)`.

After a BlockSyncUpdate selects a verified roster and the block is not already
known locally, the live path resolves block signers before QC candidate
selection:

* cached signer sets are reused without revalidation;
* successful validation is cached only when a signer-cache key exists;
* unresolved signature context while the node is behind defers the update,
  requests the missing parent with the effective commit topology and selected
  roster as context, optionally requests the wider gap, records a deferred
  signature-mismatch status, forwards the block through payload-only recovery,
  and returns `Ok(())`;
* signature errors with incoming QC, selected commit QC, or selected checkpoint
  evidence continue with an empty signer set;
* signature errors without roster/QC evidence record an invalid-signature drop,
  increment the drop metric, and return `Ok(())`.

The model intentionally uses boolean obligations instead of string-valued
labels, keeping the CI check small while still pinning each branch decision.
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
  "cache_hit",
  "validated_with_key",
  "validated_without_key",
  "defer_parent_only",
  "defer_gap",
  "parent_known_invalid",
  "not_ahead_invalid",
  "nondefer_error_invalid",
  "incoming_qc_evidence",
  "selection_qc_evidence",
  "checkpoint_evidence",
  "no_evidence_drop"
}

CacheHit(c) ==
  c = "cache_hit"

ValidationOk(c) ==
  c \in {"validated_with_key", "validated_without_key"}

CacheKeyAvailable(c) ==
  c \in {"cache_hit", "validated_with_key"}

Deferred(c) ==
  c \in {"defer_parent_only", "defer_gap"}

GapDeferred(c) ==
  c = "defer_gap"

RosterEvidence(c) ==
  c \in {
    "incoming_qc_evidence",
    "selection_qc_evidence",
    "checkpoint_evidence"
  }

InvalidDrop(c) ==
  c \in {
    "parent_known_invalid",
    "not_ahead_invalid",
    "nondefer_error_invalid",
    "no_evidence_drop"
  }

SpecUsesCache(c) == CacheHit(c)
SpecValidatesSigners(c) == ~CacheHit(c)
SpecCachesValidatedSigners(c) == ValidationOk(c) /\ CacheKeyAvailable(c)
SpecSignerSetCached(c) == CacheHit(c)
SpecSignerSetValidated(c) == ValidationOk(c)
SpecSignerSetEmpty(c) == RosterEvidence(c)
SpecSignerSetInvalid(c) == FALSE
SpecDeferred(c) == Deferred(c)
SpecRequestsMissingParent(c) == Deferred(c)
SpecRequestsGap(c) == GapDeferred(c)
SpecRequestUsesEffectiveTopology(c) == Deferred(c)
SpecRequestCarriesSelectedRoster(c) == Deferred(c)
SpecRecordsDeferredStatus(c) == Deferred(c)
SpecRecordsDroppedStatus(c) == InvalidDrop(c)
SpecReasonSignatureDeferred(c) == Deferred(c)
SpecReasonInvalidSignature(c) == InvalidDrop(c)
SpecForwardsBlockCreated(c) == Deferred(c)
SpecRecoveryPayloadOnly(c) == Deferred(c)
SpecDropInvalidSignature(c) == InvalidDrop(c)
SpecDropInvalidSignatureMetric(c) == InvalidDrop(c)
SpecContinues(c) == CacheHit(c) \/ ValidationOk(c) \/ RosterEvidence(c)
SpecProceedsToQcCandidate(c) == SpecContinues(c)
SpecReturnsOk(c) == Deferred(c) \/ InvalidDrop(c)
SpecClearsMissing(c) == FALSE

ActualUsesCache(c) ==
  SpecUsesCache(c)

ActualValidatesSigners(c) ==
  IF Bug = "cache_hit_revalidates"
     /\ c = "cache_hit"
  THEN TRUE
  ELSE SpecValidatesSigners(c)

ActualCachesValidatedSigners(c) ==
  IF Bug = "validated_signers_not_cached"
     /\ c = "validated_with_key"
  THEN FALSE
  ELSE IF Bug = "validated_signers_cached_without_key"
          /\ c = "validated_without_key" THEN TRUE
  ELSE SpecCachesValidatedSigners(c)

ActualSignerSetCached(c) ==
  SpecSignerSetCached(c)

ActualSignerSetValidated(c) ==
  SpecSignerSetValidated(c)

ActualSignerSetEmpty(c) ==
  IF Bug = "cache_hit_uses_empty_signers"
     /\ c = "cache_hit"
  THEN TRUE
  ELSE IF Bug = "validation_success_uses_empty_signers"
          /\ c = "validated_with_key" THEN TRUE
  ELSE SpecSignerSetEmpty(c)

ActualSignerSetInvalid(c) ==
  IF Bug = "evidence_uses_invalid_signers"
     /\ c = "incoming_qc_evidence"
  THEN TRUE
  ELSE SpecSignerSetInvalid(c)

ActualDeferred(c) ==
  IF Bug = "parent_known_deferred"
     /\ c = "parent_known_invalid"
  THEN TRUE
  ELSE IF Bug = "not_ahead_deferred"
          /\ c = "not_ahead_invalid" THEN TRUE
  ELSE IF Bug = "nondefer_error_deferred"
          /\ c = "nondefer_error_invalid" THEN TRUE
  ELSE SpecDeferred(c)

ActualRequestsMissingParent(c) ==
  IF Bug = "defer_parent_not_requested"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE SpecRequestsMissingParent(c)

ActualRequestsGap(c) ==
  IF Bug = "defer_gap_not_requested"
     /\ c = "defer_gap"
  THEN FALSE
  ELSE IF Bug = "defer_gap_requested_for_parent_only"
          /\ c = "defer_parent_only" THEN TRUE
  ELSE SpecRequestsGap(c)

ActualRequestUsesEffectiveTopology(c) ==
  IF Bug = "defer_uses_trusted_topology"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE SpecRequestUsesEffectiveTopology(c)

ActualRequestCarriesSelectedRoster(c) ==
  IF Bug = "defer_drops_selected_roster"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE SpecRequestCarriesSelectedRoster(c)

ActualRecordsDeferredStatus(c) ==
  IF Bug = "defer_no_status"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE SpecRecordsDeferredStatus(c)

ActualRecordsDroppedStatus(c) ==
  IF Bug = "no_evidence_no_status"
     /\ c = "no_evidence_drop"
  THEN FALSE
  ELSE SpecRecordsDroppedStatus(c)

ActualReasonSignatureDeferred(c) ==
  IF Bug = "defer_wrong_reason"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE IF Bug = "no_evidence_wrong_reason"
          /\ c = "no_evidence_drop" THEN TRUE
  ELSE SpecReasonSignatureDeferred(c)

ActualReasonInvalidSignature(c) ==
  IF Bug = "defer_wrong_reason"
     /\ c = "defer_parent_only"
  THEN TRUE
  ELSE IF Bug = "no_evidence_wrong_reason"
          /\ c = "no_evidence_drop" THEN FALSE
  ELSE SpecReasonInvalidSignature(c)

ActualForwardsBlockCreated(c) ==
  IF Bug = "defer_not_forwarded"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE SpecForwardsBlockCreated(c)

ActualRecoveryPayloadOnly(c) ==
  IF Bug = "defer_wrong_recovery_mode"
     /\ c = "defer_parent_only"
  THEN FALSE
  ELSE SpecRecoveryPayloadOnly(c)

ActualDropInvalidSignature(c) ==
  IF Bug = "incoming_evidence_dropped"
     /\ c = "incoming_qc_evidence"
  THEN TRUE
  ELSE IF Bug = "commit_qc_evidence_dropped"
          /\ c = "selection_qc_evidence" THEN TRUE
  ELSE IF Bug = "checkpoint_evidence_dropped"
          /\ c = "checkpoint_evidence" THEN TRUE
  ELSE IF Bug = "no_evidence_continues"
          /\ c = "no_evidence_drop" THEN FALSE
  ELSE SpecDropInvalidSignature(c)

ActualDropInvalidSignatureMetric(c) ==
  IF Bug = "no_evidence_no_metric"
     /\ c = "no_evidence_drop"
  THEN FALSE
  ELSE SpecDropInvalidSignatureMetric(c)

ActualContinues(c) ==
  IF Bug = "defer_continues"
     /\ c = "defer_parent_only"
  THEN TRUE
  ELSE IF Bug = "no_evidence_continues"
          /\ c = "no_evidence_drop" THEN TRUE
  ELSE SpecContinues(c)

ActualProceedsToQcCandidate(c) ==
  IF Bug = "no_evidence_proceeds_qc"
     /\ c = "no_evidence_drop"
  THEN TRUE
  ELSE SpecProceedsToQcCandidate(c)

ActualReturnsOk(c) ==
  IF Bug = "no_evidence_returns_error"
     /\ c = "no_evidence_drop"
  THEN FALSE
  ELSE SpecReturnsOk(c)

ActualClearsMissing(c) ==
  IF Bug = "drop_clears_missing"
     /\ c = "no_evidence_drop"
  THEN TRUE
  ELSE SpecClearsMissing(c)

SpecTrace(c) ==
  [
    uses_cache |-> SpecUsesCache(c),
    validates_signers |-> SpecValidatesSigners(c),
    caches_validated_signers |-> SpecCachesValidatedSigners(c),
    signer_set_cached |-> SpecSignerSetCached(c),
    signer_set_validated |-> SpecSignerSetValidated(c),
    signer_set_empty |-> SpecSignerSetEmpty(c),
    signer_set_invalid |-> SpecSignerSetInvalid(c),
    deferred |-> SpecDeferred(c),
    requests_missing_parent |-> SpecRequestsMissingParent(c),
    requests_gap |-> SpecRequestsGap(c),
    request_uses_effective_topology |-> SpecRequestUsesEffectiveTopology(c),
    request_carries_selected_roster |-> SpecRequestCarriesSelectedRoster(c),
    records_deferred_status |-> SpecRecordsDeferredStatus(c),
    records_dropped_status |-> SpecRecordsDroppedStatus(c),
    reason_signature_deferred |-> SpecReasonSignatureDeferred(c),
    reason_invalid_signature |-> SpecReasonInvalidSignature(c),
    forwards_block_created |-> SpecForwardsBlockCreated(c),
    recovery_payload_only |-> SpecRecoveryPayloadOnly(c),
    drop_invalid_signature |-> SpecDropInvalidSignature(c),
    drop_invalid_signature_metric |-> SpecDropInvalidSignatureMetric(c),
    continues |-> SpecContinues(c),
    proceeds_to_qc_candidate |-> SpecProceedsToQcCandidate(c),
    returns_ok |-> SpecReturnsOk(c),
    clears_missing |-> SpecClearsMissing(c)
  ]

ActualTrace(c) ==
  [
    uses_cache |-> ActualUsesCache(c),
    validates_signers |-> ActualValidatesSigners(c),
    caches_validated_signers |-> ActualCachesValidatedSigners(c),
    signer_set_cached |-> ActualSignerSetCached(c),
    signer_set_validated |-> ActualSignerSetValidated(c),
    signer_set_empty |-> ActualSignerSetEmpty(c),
    signer_set_invalid |-> ActualSignerSetInvalid(c),
    deferred |-> ActualDeferred(c),
    requests_missing_parent |-> ActualRequestsMissingParent(c),
    requests_gap |-> ActualRequestsGap(c),
    request_uses_effective_topology |-> ActualRequestUsesEffectiveTopology(c),
    request_carries_selected_roster |-> ActualRequestCarriesSelectedRoster(c),
    records_deferred_status |-> ActualRecordsDeferredStatus(c),
    records_dropped_status |-> ActualRecordsDroppedStatus(c),
    reason_signature_deferred |-> ActualReasonSignatureDeferred(c),
    reason_invalid_signature |-> ActualReasonInvalidSignature(c),
    forwards_block_created |-> ActualForwardsBlockCreated(c),
    recovery_payload_only |-> ActualRecoveryPayloadOnly(c),
    drop_invalid_signature |-> ActualDropInvalidSignature(c),
    drop_invalid_signature_metric |-> ActualDropInvalidSignatureMetric(c),
    continues |-> ActualContinues(c),
    proceeds_to_qc_candidate |-> ActualProceedsToQcCandidate(c),
    returns_ok |-> ActualReturnsOk(c),
    clears_missing |-> ActualClearsMissing(c)
  ]

Matches(c) ==
  ActualTrace(c) = SpecTrace(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "cache_hit_revalidates",
       "cache_hit_uses_empty_signers",
       "validated_signers_not_cached",
       "validated_signers_cached_without_key",
       "validation_success_uses_empty_signers",
       "defer_parent_not_requested",
       "defer_gap_not_requested",
       "defer_gap_requested_for_parent_only",
       "defer_uses_trusted_topology",
       "defer_drops_selected_roster",
       "defer_no_status",
       "defer_wrong_reason",
       "defer_not_forwarded",
       "defer_wrong_recovery_mode",
       "defer_continues",
       "parent_known_deferred",
       "not_ahead_deferred",
       "nondefer_error_deferred",
       "incoming_evidence_dropped",
       "commit_qc_evidence_dropped",
       "checkpoint_evidence_dropped",
       "evidence_uses_invalid_signers",
       "no_evidence_continues",
       "no_evidence_no_metric",
       "no_evidence_no_status",
       "no_evidence_wrong_reason",
       "no_evidence_returns_error",
       "no_evidence_proceeds_qc",
       "drop_clears_missing"
     }
  /\ checked = 0

SelectedSignatureMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncSelectedSignaturesExactness ==
  /\ SelectedSignatureMatchesSpec

BlockSyncSelectedSignaturesCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncSelectedSignaturesExactness

SafetyFast ==
  BlockSyncSelectedSignaturesExactness

CacheAndValidation ==
  Matches("cache_hit")
    /\ Matches("validated_with_key")
    /\ Matches("validated_without_key")

SignatureDeferral ==
  Matches("defer_parent_only")
    /\ Matches("defer_gap")
    /\ Matches("parent_known_invalid")
    /\ Matches("not_ahead_invalid")
    /\ Matches("nondefer_error_invalid")

RosterEvidenceContinuation ==
  Matches("incoming_qc_evidence")
    /\ Matches("selection_qc_evidence")
    /\ Matches("checkpoint_evidence")

InvalidSignatureDrop ==
  Matches("no_evidence_drop")

=============================================================================
====
