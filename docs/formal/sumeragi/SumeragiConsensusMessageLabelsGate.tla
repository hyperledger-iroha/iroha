---- MODULE SumeragiConsensusMessageLabelsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for consensus-message status labels.

This slice pins `ConsensusMessageKind::as_str(...)`,
`ConsensusMessageOutcome::as_str(...)`, and
`ConsensusMessageReason::as_str(...)` from `status.rs`. The ingress status
counter gate proves counters keep the kind/outcome/reason dimensions separate;
this companion gate fixes the exported label strings so status snapshots,
metrics, and logs cannot silently alias distinct handling paths.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

KindCases == 1..21
OutcomeCases == 1..2
ReasonCases == 1..39

SpecKindLabel(c) ==
  CASE c = 1 -> "block_created"
    [] c = 2 -> "block_sync_update"
    [] c = 3 -> "fetch_block_body"
    [] c = 4 -> "block_body_response"
    [] c = 5 -> "certified_block_fetch"
    [] c = 6 -> "consensus_params"
    [] c = 7 -> "proposal_hint"
    [] c = 8 -> "proposal"
    [] c = 9 -> "qc_vote"
    [] c = 10 -> "qc"
    [] c = 11 -> "vrf_commit"
    [] c = 12 -> "vrf_reveal"
    [] c = 13 -> "exec_witness"
    [] c = 14 -> "rbc_init_request"
    [] c = 15 -> "rbc_chunk_request"
    [] c = 16 -> "rbc_init"
    [] c = 17 -> "rbc_chunk"
    [] c = 18 -> "rbc_ready"
    [] c = 19 -> "rbc_deliver"
    [] c = 20 -> "fetch_pending_block"
    [] c = 21 -> "evidence"

ActualKindLabel(c) ==
  CASE Bug = "kind_block_created_uses_proposal"
       /\ c = 1 -> "proposal"
    [] Bug = "kind_certified_fetch_uses_fetch_body"
       /\ c = 5 -> "fetch_block_body"
    [] Bug = "kind_proposal_hint_uses_proposal"
       /\ c = 7 -> "proposal"
    [] Bug = "kind_qc_vote_uses_qc"
       /\ c = 9 -> "qc"
    [] Bug = "kind_qc_uses_vote"
       /\ c = 10 -> "qc_vote"
    [] Bug = "kind_rbc_chunk_uses_ready"
       /\ c = 17 -> "rbc_ready"
    [] Bug = "kind_fetch_pending_uses_block_body"
       /\ c = 20 -> "block_body_response"
    [] Bug = "kind_evidence_uses_consensus_params"
       /\ c = 21 -> "consensus_params"
    [] OTHER -> SpecKindLabel(c)

SpecOutcomeLabel(c) ==
  CASE c = 1 -> "dropped"
    [] c = 2 -> "deferred"

ActualOutcomeLabel(c) ==
  CASE Bug = "outcome_dropped_uses_deferred"
       /\ c = 1 -> "deferred"
    [] Bug = "outcome_deferred_uses_dropped"
       /\ c = 2 -> "dropped"
    [] OTHER -> SpecOutcomeLabel(c)

SpecReasonLabel(c) ==
  CASE c = 1 -> "future_window"
    [] c = 2 -> "stale_height"
    [] c = 3 -> "stale_view"
    [] c = 4 -> "duplicate"
    [] c = 5 -> "conflicting_vote"
    [] c = 6 -> "locked_qc"
    [] c = 7 -> "missing_highest_qc"
    [] c = 8 -> "highest_qc_mismatch"
    [] c = 9 -> "hint_mismatch"
    [] c = 10 -> "payload_mismatch"
    [] c = 11 -> "invalid_payload"
    [] c = 12 -> "payload_too_large"
    [] c = 13 -> "commit_conflict"
    [] c = 14 -> "roster_missing"
    [] c = 15 -> "invalid_signature"
    [] c = 16 -> "quorum_missing"
    [] c = 17 -> "payload_unapplied"
    [] c = 18 -> "signature_mismatch_deferred"
    [] c = 19 -> "commit_pipeline_active"
    [] c = 20 -> "aggregate_verify_deferred"
    [] c = 21 -> "enqueue_failed"
    [] c = 22 -> "backpressure"
    [] c = 23 -> "penalized_sender"
    [] c = 24 -> "epoch_mismatch"
    [] c = 25 -> "committed"
    [] c = 26 -> "roster_hash_mismatch"
    [] c = 27 -> "chunk_digest_mismatch"
    [] c = 28 -> "chunk_root_mismatch"
    [] c = 29 -> "stash_session_limit"
    [] c = 30 -> "stash_cap"
    [] c = 31 -> "ready_quorum_missing"
    [] c = 32 -> "chunks_missing"
    [] c = 33 -> "init_missing"
    [] c = 34 -> "roster_missing_deferred"
    [] c = 35 -> "roster_hash_mismatch_deferred"
    [] c = 36 -> "roster_unverified_deferred"
    [] c = 37 -> "mode_mismatch"
    [] c = 38 -> "membership_mismatch"
    [] c = 39 -> "not_found"

ActualReasonLabel(c) ==
  CASE Bug = "reason_future_window_uses_stale_height"
       /\ c = 1 -> "stale_height"
    [] Bug = "reason_locked_qc_uses_missing_highest"
       /\ c = 6 -> "missing_highest_qc"
    [] Bug = "reason_missing_highest_uses_highest_mismatch"
       /\ c = 7 -> "highest_qc_mismatch"
    [] Bug = "reason_backpressure_uses_enqueue_failed"
       /\ c = 22 -> "enqueue_failed"
    [] Bug = "reason_commit_pipeline_uses_aggregate_deferred"
       /\ c = 19 -> "aggregate_verify_deferred"
    [] Bug = "reason_stash_session_uses_stash_cap"
       /\ c = 29 -> "stash_cap"
    [] Bug = "reason_stash_cap_uses_session"
       /\ c = 30 -> "stash_session_limit"
    [] Bug = "reason_ready_quorum_uses_chunks"
       /\ c = 31 -> "chunks_missing"
    [] Bug = "reason_init_missing_uses_roster_deferred"
       /\ c = 33 -> "roster_missing_deferred"
    [] Bug = "reason_roster_deferred_uses_roster_missing"
       /\ c = 34 -> "roster_missing"
    [] Bug = "reason_roster_hash_deferred_uses_roster_hash"
       /\ c = 35 -> "roster_hash_mismatch"
    [] Bug = "reason_roster_unverified_uses_roster_deferred"
       /\ c = 36 -> "roster_missing_deferred"
    [] Bug = "reason_mode_mismatch_uses_membership"
       /\ c = 37 -> "membership_mismatch"
    [] Bug = "reason_membership_mismatch_uses_mode"
       /\ c = 38 -> "mode_mismatch"
    [] Bug = "reason_not_found_uses_committed"
       /\ c = 39 -> "committed"
    [] OTHER -> SpecReasonLabel(c)

KindLabels == {SpecKindLabel(c): c \in KindCases}
OutcomeLabels == {SpecOutcomeLabel(c): c \in OutcomeCases}
ReasonLabels == {SpecReasonLabel(c): c \in ReasonCases}

BugSet == {
  "none",
  "kind_block_created_uses_proposal",
  "kind_certified_fetch_uses_fetch_body",
  "kind_proposal_hint_uses_proposal",
  "kind_qc_vote_uses_qc",
  "kind_qc_uses_vote",
  "kind_rbc_chunk_uses_ready",
  "kind_fetch_pending_uses_block_body",
  "kind_evidence_uses_consensus_params",
  "outcome_dropped_uses_deferred",
  "outcome_deferred_uses_dropped",
  "reason_future_window_uses_stale_height",
  "reason_locked_qc_uses_missing_highest",
  "reason_missing_highest_uses_highest_mismatch",
  "reason_backpressure_uses_enqueue_failed",
  "reason_commit_pipeline_uses_aggregate_deferred",
  "reason_stash_session_uses_stash_cap",
  "reason_stash_cap_uses_session",
  "reason_ready_quorum_uses_chunks",
  "reason_init_missing_uses_roster_deferred",
  "reason_roster_deferred_uses_roster_missing",
  "reason_roster_hash_deferred_uses_roster_hash",
  "reason_roster_unverified_uses_roster_deferred",
  "reason_mode_mismatch_uses_membership",
  "reason_membership_mismatch_uses_mode",
  "reason_not_found_uses_committed"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in KindCases: ActualKindLabel(c) \in KindLabels
  /\ \A c \in OutcomeCases: ActualOutcomeLabel(c) \in OutcomeLabels
  /\ \A c \in ReasonCases: ActualReasonLabel(c) \in ReasonLabels

KindLabelsExact ==
  \A c \in KindCases:
    ActualKindLabel(c) = SpecKindLabel(c)

OutcomeLabelsExact ==
  \A c \in OutcomeCases:
    ActualOutcomeLabel(c) = SpecOutcomeLabel(c)

ReasonLabelsExact ==
  \A c \in ReasonCases:
    ActualReasonLabel(c) = SpecReasonLabel(c)

KindLabelsDistinct ==
  \A a, b \in KindCases:
    a # b => ActualKindLabel(a) # ActualKindLabel(b)

OutcomeLabelsDistinct ==
  ActualOutcomeLabel(1) # ActualOutcomeLabel(2)

ReasonLabelsDistinct ==
  \A a, b \in ReasonCases:
    a # b => ActualReasonLabel(a) # ActualReasonLabel(b)

SampleHandlingTripleStable ==
  /\ ActualKindLabel(2) = "block_sync_update"
  /\ ActualOutcomeLabel(2) = "deferred"
  /\ ActualReasonLabel(35) = "roster_hash_mismatch_deferred"

SafetyFast ==
  /\ KindLabelsExact
  /\ OutcomeLabelsExact
  /\ ReasonLabelsExact
  /\ KindLabelsDistinct
  /\ OutcomeLabelsDistinct
  /\ ReasonLabelsDistinct
  /\ SampleHandlingTripleStable

====
