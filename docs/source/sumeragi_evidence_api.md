# Sumeragi Evidence (Audit API)

Temporary audit endpoints for Sumeragi evidence.

- GET `/v1/sumeragi/evidence/count`
  - Returns the number of unique Evidence entries observed by this node.
  - Response (Norito payload): `count: u64`.
  - Set `Accept: application/json` to receive `{ "count": <u64> }`.
  - Notes:
    - Backed by the per-node WSV store (`world.consensus_evidence`) persisted with Norito codecs.
    - Survives restarts and feeds `/v1/sumeragi/evidence`; entries are deduplicated by evidence hash.
    - Still local to each validator (not consensus-replicated); governance ingestion will follow.

- GET `/v1/sumeragi/evidence`
  - Lists recent evidence entries persisted in the WSV audit snapshot.
  - Query params: `limit` (default 50, max 1000), `offset` (default 0), `kind` (optional; one of `DoublePrepare|DoubleCommit|InvalidCommitCertificate|InvalidProposal|Censorship`).
  - Response (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - Set `Accept: application/json` to receive a JSON object `{ "total": <u64>, "items": [ ... ] }`.
- Evidence with a subject height older than `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7 200) is dropped on ingress; the actor logs the rejection to help operators
  investigate stale submissions.
- POST `/v1/sumeragi/evidence`
  - Submit hex-encoded Norito evidence to the Sumeragi actor (`ControlFlow::Evidence`).
  - Request body (JSON): `{ "evidence_hex": "<hex string>" }`; the hex string encodes Norito-framed `ConsensusEvidence` bytes and ignores whitespace.
  - Response (JSON): `{ "status": "accepted", "kind": "<variant>" }` on success.
  - Validation covers signer/height/view/epoch equality for double-vote payloads, requires non-empty single-signer payloads, enforces receipt quorums for `Censorship` evidence (signed `TransactionSubmissionReceipt` payloads), and rejects `InvalidProposal` records that fail to advance height/view or whose parent hash disagrees with the embedded commit certificate.
  - CLI helper: `iroha sumeragi evidence submit --evidence-hex <hex>` or `--evidence-hex-file <path>`.

Additional consensus status and execution proofs

- GET `/v1/sumeragi/status` — returns a Norito-encoded `SumeragiStatusWire` payload by default. Set `Accept: application/json` to receive `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Note: `highest_qc`/`locked_qc` report commit certificate snapshots and `view_change_causes.missing_qc_total` counts missing commit certificates. `view_change_causes.da_gate_total` is reserved for compatibility and should remain zero; DA availability warnings are surfaced via `status.da_gate` and `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` is legacy.
- GET `/v1/sumeragi/qc` — returns a Norito-encoded commit certificate snapshot (`SumeragiQcSnapshot`) by default. Set `Accept: application/json` to receive `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` — SSE stream of the same payload (≈1s cadence).
- GET `/v1/sumeragi/exec_root/:hash` — returns a Norito payload `(block_hash, Option<ExecRoot>)` by default. Pass `Accept: application/json` to receive `{ block_hash, exec_root }` (hex) or `exec_root = null` if missing.
- GET `/v1/sumeragi/exec_qc/:hash` — returns a Norito-encoded `ExecutionQcRecord` for `:hash` (parent block hash) by default. With `Accept: application/json` the response expands to:
  - `post_state_root` (hex), `height`, `view`, `epoch`, `signers_bitmap` (hex), `bls_aggregate_signature` (hex).
  - If missing, returns `{ subject_block_hash, exec_qc: null }`.

Example (curl)

```bash
# Replace HASH with a real parent block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_qc/$HASH | jq .

# Example response (when present):
# {
#   "subject_block_hash": "BA6733…F5B2",
#   "post_state_root": "1f9a7d…2c0e",
#   "height": 42,
#   "view": 3,
#   "epoch": 0,
#   "signers_bitmap": "0700",
#   "bls_aggregate_signature": ""
# }
```

Exec root example (curl)

```bash
# Replace HASH with a real parent block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_root/$HASH | jq .

# Example response (when present):
# {
#   "block_hash": "BA6733…F5B2",
#   "exec_root": "1f9a7d…2c0e"
# }

# When missing:
# {
#   "block_hash": "BA6733…F5B2",
#   "exec_root": null
# }
```

Quick jq one‑liners (consistency + wrappers)

- Parent consistency check (ExecQC vs exec_root):

```bash
P=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
QC=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_qc/$P | jq -r '.post_state_root // empty')
RT=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_root/$P | jq -r '.exec_root // empty')
if [ -z "$QC" ]; then echo "no ExecQC record in WSV for $P"; exit 1; fi
if [ -z "$RT" ]; then echo "no exec_root in WSV for $P"; exit 1; fi
if [ "$QC" = "$RT" ]; then echo "OK: parent ExecQC equals exec_root ($QC)"; else echo "MISMATCH: QC=$QC root=$RT"; fi
```

- Shell helpers you can paste into your terminal:

```bash
get_exec_qc_root() { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_qc/$1" | jq -r '.post_state_root // empty'; }
get_exec_root()    { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_root/$1" | jq -r '.exec_root // empty'; }
check_parent_consistency() {
  local h="$1"; local qc=$(get_exec_qc_root "$h"); local rt=$(get_exec_root "$h");
  if [ -z "$qc" ]; then echo "no ExecQC record for $h"; return 2; fi
  if [ -z "$rt" ]; then echo "no exec_root for $h"; return 3; fi
  if [ "$qc" = "$rt" ]; then echo "OK: $h QC==root ($qc)"; else echo "MISMATCH: QC=$qc root=$rt"; return 1; fi
}

# usage:
#   H=...
#   check_parent_consistency "$H"
```

Note
- Cross‑height equality (child vs parent) is enforced internally by Sumeragi in strict mode
  (`require_wsv_exec_qc=true`) and not directly exposed via a query yet. The operator‑visible
  consistency checks above verify that the parent’s ExecutionQC record matches its persisted
  execution root. A future endpoint will expose proposal/header roots to allow an external
  parent‑vs‑child comparison.
