import { Buffer } from "node:buffer";

export function hashLiteral(hex) {
  const body = hex.toUpperCase();
  let crc = 0xffff;
  for (const byte of Buffer.from(`hash:${body}`, "utf8")) {
    crc ^= (byte & 0xff) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  }
  return `hash:${body}#${crc.toString(16).toUpperCase().padStart(4, "0")}`;
}
function browserSumeragiHash(byte) {
  const bytes = Buffer.alloc(32, byte & 0xff);
  bytes[31] |= 1;
  return hashLiteral(bytes.toString("hex"));
}

export function browserSumeragiStatusFixture() {
  const subject = {
    parent_block_hash: browserSumeragiHash(0x31),
    block_hash: browserSumeragiHash(0x32),
    payload_hash: browserSumeragiHash(0x33),
  };
  const executionCommitment = {
    parent_state_root: browserSumeragiHash(0x34),
    post_state_root: browserSumeragiHash(0x35),
    ordinary_writes_root: browserSumeragiHash(0x36),
    offline_cash_top_up_root: null,
    offline_cash_top_up_count: 0,
    native_amx_application_manifest_version: 1,
    native_amx_application_manifest_root:
      "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F",
    native_amx_application_manifest_count: 0,
    lane_finality_manifest: null,
    merge_carrier: null,
    executed_block_wire_len: 123,
    executed_block_wire_hash: browserSumeragiHash(0x37),
  };
  const heightContextId = [browserSumeragiHash(0x14)];
  const commitContextId = [browserSumeragiHash(0x41)];
  return {
    protocol_version: 4,
    node_fingerprint: browserSumeragiHash(0x11),
    build_fingerprint: browserSumeragiHash(0x12),
    config_fingerprint: browserSumeragiHash(0x13),
    restart_required: false,
    height_context_id: heightContextId,
    height: 10,
    view: 2,
    phase: { phase: "prepare", details: null },
    leader: 1,
    locked_prepare_qc: null,
    highest_prepare_qc: null,
    last_timeout_certificate: null,
    body_state: { state: "validated", details: null },
    pending_persistence_id: null,
    last_committed_height: 9,
    last_committed_subject: subject,
    height_context: {
      epoch: 1,
      epoch_end_height: 20,
      mode: { mode: "permissioned", details: null },
      epoch_seed: Buffer.from(
        Array.from({ length: 32 }, (_, index) => index),
      ).toString("hex").toUpperCase(),
      validator_count: 4,
      quorum: { min_signers: 3, total_power: 4 },
    },
    last_commit_qc: {
      certificate: {
        round: { context_id: commitContextId, height: 9, view: 1 },
        proposal_round: { context_id: commitContextId, height: 9, view: 1 },
        phase: { phase: "commit", details: null },
        subject,
        execution_commitment: executionCommitment,
      },
      validator_count: 4,
      signer_count: 3,
      min_signers: 3,
      signed_power: 3,
      total_power: 4,
    },
    liveness: {
      generation: 2,
      prepare_quorums: [
        {
          round: { context_id: heightContextId, height: 10, view: 1 },
          proposal_round: {
            context_id: heightContextId,
            height: 10,
            view: 1,
          },
          subject,
          execution_commitment: executionCommitment,
          signer_count: 2,
          signed_power: 2,
          min_signers: 3,
          total_power: 4,
        },
      ],
      commit_quorums: [],
      timeout_quorums: [],
      outbound_intents: [
        {
          kind: { kind: "proposal", details: null },
          round: { context_id: heightContextId, height: 10, view: 1 },
          proposal_round: {
            context_id: heightContextId,
            height: 10,
            view: 1,
          },
          subject,
          execution_commitment: null,
          stage: { stage: "sent", details: null },
        },
      ],
      work: {
        candidate: { stage: "idle", details: null },
        body_recovery: { stage: "idle", details: null },
        body_store: { stage: "idle", details: null },
        validation: { stage: "complete", details: null },
        application: { stage: "idle", details: null },
        successor_height: { stage: "idle", details: null },
      },
      queues: [
        {
          queue: { queue: "network_ingress", details: null },
          depth: 1,
          capacity: 4,
          oldest_age_ms: 17,
          service_debt: 2,
        },
      ],
      last_progress: {
        generation: 2,
        round: { context_id: heightContextId, height: 10, view: 1 },
        transition: { transition: "prepare_vote_admitted", details: null },
        age_ms: 19,
      },
      no_progress_age_ms: 19,
      blocker: { blocker: "prepare_quorum_missing", details: null },
      ignore_counts: [
        {
          reason: { reason: "duplicate", details: null },
          count: 2,
        },
      ],
    },
  };
}

export function browserSumeragiDiagnosticsFixture() {
  return {
    pipeline_execution: {
      tx_vertices_total: 1,
      tx_edges_total: 0,
      overlay_count_total: 1,
      overlay_instr_total: 2,
      overlay_bytes_total: 128,
      rbc_chunks_total: 1,
      rbc_bytes_total: 256,
      detached_prepared_total: 1,
      detached_merged_total: 1,
      detached_fallback_total: 0,
      detached_fallback_fee_postprocessing_total: 0,
      detached_fallback_user_executor_total: 0,
      detached_fallback_durable_state_total: 0,
      detached_fallback_unsupported_instruction_total: 0,
      detached_fallback_rejected_eval_total: 0,
      detached_fallback_overlay_error_total: 0,
      quarantine_executed_total: 0,
    },
    tx_queue_depth: 3,
    tx_queue_capacity: 32,
    tx_queue_retained_bytes: 4096,
    tx_queue_max_retained_bytes: 65536,
    tx_queue_saturated: false,
    tx_queue_saturated_by_count: false,
    tx_queue_saturated_by_bytes: false,
    tx_queue_saturated_by_age: false,
    tx_queue_oldest_queued_age_ms: 25,
    npos: null,
    lane_commitments: [],
    dataspace_commitments: [],
    lane_settlement_commitments: [],
    lane_relay_envelopes: [],
    lane_payload_ownerships: [],
    committed_lane_blocks: [],
    lane_block_sessions: [],
    lane_governance_sealed_total: 0,
    lane_governance_sealed_aliases: [],
    lane_governance: [],
    native_amx_participant_applications: [],
    autonomous_lane_executions: [],
  };
}
