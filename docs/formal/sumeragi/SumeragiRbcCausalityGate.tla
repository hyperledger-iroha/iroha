---- MODULE SumeragiRbcCausalityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi RBC message causality.

This slice models the implementation contracts around `handle_rbc_init(...)`,
`handle_rbc_chunk(...)`, `maybe_emit_rbc_ready(...)`,
`handle_rbc_ready(...)`, and `handle_rbc_deliver(...)`. The concrete code has
many recovery paths, but the consensus-critical safety shape is finite:
accepted INIT binds header, leader-signature, roster, payload-hash,
chunk-digest, and chunk-root evidence; chunks are only recorded after a session
exists and their digest matches INIT evidence; local READY emission waits for
payload/chunk-root evidence; remote READY recording waits for roster, signature,
and chunk-root validation; DELIVER recording waits for roster, signature, and
chunk-root validation, and bundled READY signatures only seed state after
independent READY-signature validation.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Set(Str);
  actions

\* @type: <<Str, Set(Str)>>;
vars == <<candidate, actions>>

Candidates == {
  "valid_init",
  "invalid_init_rejected",
  "chunk_before_init",
  "valid_chunk_recorded",
  "chunk_bad_digest",
  "complete_chunks_emit_ready",
  "chunk_root_mismatch_blocks_ready",
  "ready_before_init",
  "valid_ready_recorded",
  "ready_bad_signature",
  "ready_roster_mismatch",
  "ready_root_mismatch",
  "ready_conflict",
  "deliver_before_init",
  "valid_deliver",
  "deliver_bad_signature",
  "deliver_root_mismatch",
  "deliver_ready_bundle",
  "deliver_invalid_ready_bundle",
  "deliver_duplicate"
}

ValidationActions == {
  "validate_epoch",
  "validate_nonzero_chunks",
  "validate_digest_count",
  "validate_roster_nonempty",
  "validate_roster_unique",
  "validate_roster_hash",
  "validate_derived_roster",
  "validate_existing_session",
  "validate_header_hash",
  "validate_header_height_view",
  "verify_leader_signature",
  "validate_chunk_root",
  "validate_layout",
  "validate_chunk_epoch",
  "validate_chunk_size",
  "validate_chunk_digest",
  "validate_ready_epoch",
  "validate_ready_roster",
  "validate_ready_signature",
  "validate_ready_chunk_root",
  "validate_deliver_epoch",
  "validate_deliver_roster",
  "validate_deliver_signature",
  "validate_deliver_chunk_root",
  "validate_ready_bundle_signatures"
}

MutationActions == {
  "drop",
  "stash",
  "create_session",
  "record_roster",
  "bind_header",
  "bind_leader_signature",
  "bind_payload_hash",
  "bind_chunk_digests",
  "bind_chunk_root",
  "record_chunk",
  "complete_payload",
  "emit_local_ready",
  "sign_ready",
  "record_local_ready",
  "record_ready",
  "mark_invalid",
  "clear_pending",
  "record_ready_bundle",
  "record_deliver",
  "wake_commit_pipeline",
  "ignore_duplicate",
  "drop_invalid_ready_bundle"
}

AllActions == ValidationActions \union MutationActions

SpecActions(c) ==
  CASE c = "valid_init" ->
      {
        "validate_epoch",
        "validate_nonzero_chunks",
        "validate_digest_count",
        "validate_roster_nonempty",
        "validate_roster_unique",
        "validate_roster_hash",
        "validate_derived_roster",
        "validate_existing_session",
        "validate_header_hash",
        "validate_header_height_view",
        "verify_leader_signature",
        "validate_chunk_root",
        "validate_layout",
        "create_session",
        "record_roster",
        "bind_header",
        "bind_leader_signature",
        "bind_payload_hash",
        "bind_chunk_digests",
        "bind_chunk_root"
      }
    [] c = "invalid_init_rejected" ->
      {
        "validate_epoch",
        "validate_nonzero_chunks",
        "validate_digest_count",
        "validate_roster_hash",
        "validate_header_hash",
        "verify_leader_signature",
        "validate_chunk_root",
        "drop"
      }
    [] c = "chunk_before_init" ->
      {"validate_chunk_epoch", "validate_chunk_size", "stash"}
    [] c = "valid_chunk_recorded" ->
      {
        "validate_chunk_epoch",
        "validate_chunk_size",
        "validate_chunk_digest",
        "record_chunk"
      }
    [] c = "chunk_bad_digest" ->
      {"validate_chunk_epoch", "validate_chunk_size", "validate_chunk_digest", "drop"}
    [] c = "complete_chunks_emit_ready" ->
      {
        "validate_chunk_digest",
        "record_chunk",
        "complete_payload",
        "validate_chunk_root",
        "bind_chunk_root",
        "sign_ready",
        "emit_local_ready",
        "record_local_ready"
      }
    [] c = "chunk_root_mismatch_blocks_ready" ->
      {"validate_chunk_root", "drop"}
    [] c = "ready_before_init" ->
      {"validate_ready_epoch", "stash"}
    [] c = "valid_ready_recorded" ->
      {
        "validate_ready_epoch",
        "validate_ready_roster",
        "validate_ready_signature",
        "validate_ready_chunk_root",
        "record_ready"
      }
    [] c = "ready_bad_signature" ->
      {"validate_ready_signature", "drop"}
    [] c = "ready_roster_mismatch" ->
      {"validate_ready_roster", "drop"}
    [] c = "ready_root_mismatch" ->
      {"validate_ready_chunk_root", "drop"}
    [] c = "ready_conflict" ->
      {
        "validate_ready_signature",
        "validate_ready_chunk_root",
        "mark_invalid",
        "clear_pending"
      }
    [] c = "deliver_before_init" ->
      {"validate_deliver_epoch", "stash"}
    [] c = "valid_deliver" ->
      {
        "validate_deliver_epoch",
        "validate_deliver_roster",
        "validate_deliver_signature",
        "validate_deliver_chunk_root",
        "record_deliver",
        "wake_commit_pipeline"
      }
    [] c = "deliver_bad_signature" ->
      {"validate_deliver_signature", "drop"}
    [] c = "deliver_root_mismatch" ->
      {"validate_deliver_chunk_root", "drop"}
    [] c = "deliver_ready_bundle" ->
      {
        "validate_deliver_signature",
        "validate_deliver_chunk_root",
        "validate_ready_bundle_signatures",
        "record_ready_bundle",
        "record_deliver",
        "wake_commit_pipeline"
      }
    [] c = "deliver_invalid_ready_bundle" ->
      {
        "validate_deliver_signature",
        "validate_deliver_chunk_root",
        "validate_ready_bundle_signatures",
        "drop_invalid_ready_bundle",
        "record_deliver",
        "wake_commit_pipeline"
      }
    [] c = "deliver_duplicate" ->
      {"ignore_duplicate"}
    [] OTHER -> {}

ActualActions(c) ==
  CASE c = "valid_init" /\ Bug = "init_skip_header_hash" ->
      SpecActions(c) \ {"validate_header_hash"}
    [] c = "valid_init" /\ Bug = "init_skip_leader_signature" ->
      SpecActions(c) \ {"verify_leader_signature"}
    [] c = "valid_init" /\ Bug = "init_skip_chunk_root" ->
      SpecActions(c) \ {"validate_chunk_root"}
    [] c = "valid_init" /\ Bug = "init_skip_roster_hash" ->
      SpecActions(c) \ {"validate_roster_hash"}
    [] c = "invalid_init_rejected" /\ Bug = "invalid_init_creates_session" ->
      (SpecActions(c) \ {"drop"}) \union {"create_session", "record_roster"}
    [] c = "invalid_init_rejected" /\ Bug = "drop_mutates_session" ->
      SpecActions(c) \union {"record_chunk"}
    [] c = "chunk_before_init" /\ Bug = "chunk_before_init_records" ->
      (SpecActions(c) \ {"stash"}) \union {"record_chunk"}
    [] c = "chunk_bad_digest" /\ Bug = "chunk_bad_digest_records" ->
      (SpecActions(c) \ {"drop"}) \union {"record_chunk"}
    [] c = "complete_chunks_emit_ready" /\ Bug = "local_ready_before_complete_payload" ->
      SpecActions(c) \ {"complete_payload"}
    [] c = "complete_chunks_emit_ready" /\ Bug = "local_ready_without_root_check" ->
      SpecActions(c) \ {"validate_chunk_root"}
    [] c = "ready_before_init" /\ Bug = "ready_before_init_records" ->
      (SpecActions(c) \ {"stash"}) \union {"record_ready"}
    [] c = "ready_bad_signature" /\ Bug = "ready_bad_signature_records" ->
      (SpecActions(c) \ {"drop"}) \union {"record_ready"}
    [] c = "ready_roster_mismatch" /\ Bug = "ready_roster_mismatch_records" ->
      (SpecActions(c) \ {"drop"}) \union {"record_ready"}
    [] c = "ready_root_mismatch" /\ Bug = "ready_root_mismatch_records" ->
      (SpecActions(c) \ {"drop"}) \union {"record_ready"}
    [] c = "ready_conflict" /\ Bug = "ready_conflict_not_invalidated" ->
      SpecActions(c) \ {"mark_invalid"}
    [] c = "ready_conflict" /\ Bug = "ready_conflict_keeps_pending" ->
      SpecActions(c) \ {"clear_pending"}
    [] c = "deliver_before_init" /\ Bug = "deliver_before_init_records" ->
      (SpecActions(c) \ {"stash"}) \union {"record_deliver"}
    [] c = "deliver_bad_signature" /\ Bug = "deliver_bad_signature_records" ->
      (SpecActions(c) \ {"drop"}) \union {"record_deliver"}
    [] c = "deliver_root_mismatch" /\ Bug = "deliver_root_mismatch_records" ->
      (SpecActions(c) \ {"drop"}) \union {"record_deliver"}
    [] c = "deliver_ready_bundle" /\ Bug = "deliver_unvalidated_ready_bundle_records" ->
      SpecActions(c) \ {"validate_ready_bundle_signatures"}
    [] c = "deliver_invalid_ready_bundle" /\ Bug = "deliver_invalid_ready_bundle_records" ->
      SpecActions(c) \union {"record_ready_bundle"}
    [] c = "deliver_duplicate" /\ Bug = "deliver_duplicate_records" ->
      (SpecActions(c) \ {"ignore_duplicate"}) \union {"record_deliver"}
    [] c = "valid_deliver" /\ Bug = "deliver_records_without_wake" ->
      SpecActions(c) \ {"wake_commit_pipeline"}
    [] c = "deliver_duplicate" /\ Bug = "deliver_wakes_without_record" ->
      SpecActions(c) \union {"wake_commit_pipeline"}
    [] c = "ready_before_init" /\ Bug = "stash_wakes_commit" ->
      SpecActions(c) \union {"wake_commit_pipeline"}
    [] OTHER -> SpecActions(c)

BugModes == {
  "none",
  "init_skip_header_hash",
  "init_skip_leader_signature",
  "init_skip_chunk_root",
  "init_skip_roster_hash",
  "invalid_init_creates_session",
  "drop_mutates_session",
  "chunk_before_init_records",
  "chunk_bad_digest_records",
  "local_ready_before_complete_payload",
  "local_ready_without_root_check",
  "ready_before_init_records",
  "ready_bad_signature_records",
  "ready_roster_mismatch_records",
  "ready_root_mismatch_records",
  "ready_conflict_not_invalidated",
  "ready_conflict_keeps_pending",
  "deliver_before_init_records",
  "deliver_bad_signature_records",
  "deliver_root_mismatch_records",
  "deliver_unvalidated_ready_bundle_records",
  "deliver_invalid_ready_bundle_records",
  "deliver_duplicate_records",
  "deliver_records_without_wake",
  "deliver_wakes_without_record",
  "stash_wakes_commit"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Candidates \union {"none"}
  /\ actions \subseteq AllActions

Init ==
  /\ candidate = "none"
  /\ actions = {}

Apply(c) ==
  /\ candidate' = c
  /\ actions' = ActualActions(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Candidates: Apply(c)
  \/ Stable

ActionsMatchSpec ==
  candidate = "none" \/ actions = SpecActions(candidate)

AcceptedInitBindsEvidence ==
  candidate = "valid_init" =>
    {
      "validate_header_hash",
      "validate_header_height_view",
      "verify_leader_signature",
      "validate_roster_hash",
      "validate_chunk_root",
      "create_session",
      "record_roster",
      "bind_header",
      "bind_leader_signature",
      "bind_payload_hash",
      "bind_chunk_digests",
      "bind_chunk_root"
    } \subseteq actions

DroppedMessagesDoNotMutateSession ==
  "drop" \in actions =>
    actions \cap {
      "create_session",
      "record_roster",
      "record_chunk",
      "emit_local_ready",
      "record_local_ready",
      "record_ready",
      "record_ready_bundle",
      "record_deliver",
      "wake_commit_pipeline"
    } = {}

StashedMessagesDoNotMutateConsensus ==
  "stash" \in actions =>
    actions \cap {
      "record_chunk",
      "emit_local_ready",
      "record_local_ready",
      "record_ready",
      "record_deliver",
      "wake_commit_pipeline"
    } = {}

LocalReadyRequiresPayloadEvidence ==
  "emit_local_ready" \in actions =>
    /\ "complete_payload" \in actions
    /\ "validate_chunk_root" \in actions
    /\ "bind_chunk_root" \in actions
    /\ "sign_ready" \in actions
    /\ "record_local_ready" \in actions

RemoteReadyRequiresRosterSigRoot ==
  "record_ready" \in actions =>
    /\ "validate_ready_roster" \in actions
    /\ "validate_ready_signature" \in actions
    /\ "validate_ready_chunk_root" \in actions

ReadyConflictInvalidatesAndClearsPending ==
  candidate = "ready_conflict" =>
    /\ "mark_invalid" \in actions
    /\ "clear_pending" \in actions
    /\ "record_ready" \notin actions

DeliverRequiresSignatureAndRoot ==
  "record_deliver" \in actions =>
    /\ "validate_deliver_signature" \in actions
    /\ "validate_deliver_chunk_root" \in actions

DeliverReadyBundleSeedsOnlyAfterValidation ==
  "record_ready_bundle" \in actions =>
    "validate_ready_bundle_signatures" \in actions

DeliverWakeRequiresFirstDeliver ==
  "wake_commit_pipeline" \in actions =>
    "record_deliver" \in actions

DuplicateDeliverDoesNotMutate ==
  candidate = "deliver_duplicate" =>
    /\ "ignore_duplicate" \in actions
    /\ "record_deliver" \notin actions
    /\ "wake_commit_pipeline" \notin actions

Safety ==
  /\ ActionsMatchSpec
  /\ AcceptedInitBindsEvidence
  /\ DroppedMessagesDoNotMutateSession
  /\ StashedMessagesDoNotMutateConsensus
  /\ LocalReadyRequiresPayloadEvidence
  /\ RemoteReadyRequiresRosterSigRoot
  /\ ReadyConflictInvalidatesAndClearsPending
  /\ DeliverRequiresSignatureAndRoot
  /\ DeliverReadyBundleSeedsOnlyAfterValidation
  /\ DeliverWakeRequiresFirstDeliver
  /\ DuplicateDeliverDoesNotMutate

====
