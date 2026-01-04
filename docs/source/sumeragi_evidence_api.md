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
  - Query params: `limit` (default 50, max 1000), `offset` (default 0), `kind` (optional; one of `DoublePrevote|DoublePrecommit|InvalidQC|InvalidProposal|Censorship`).
  - Response (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - Set `Accept: application/json` to receive a JSON object `{ "total": <u64>, "items": [ ... ] }`.
- Evidence with a subject height older than `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7 200) is dropped on ingress; the actor logs the rejection to help operators
  investigate stale submissions.
- POST `/v1/sumeragi/evidence`
  - Submit hex-encoded Norito evidence to the Sumeragi actor (`ControlFlow::Evidence`).
  - Request body (JSON): `{ "evidence_hex": "<hex string>" }`; the hex string encodes Norito-framed `ConsensusEvidence` bytes and ignores whitespace.
  - Response (JSON): `{ "status": "accepted", "kind": "<variant>" }` on success.
  - Validation covers signer/height/view/epoch equality for double-vote payloads, requires non-empty single-signer payloads, enforces receipt quorums for `Censorship` evidence (signed `TransactionSubmissionReceipt` payloads), and rejects `InvalidProposal` records that fail to advance height/view or whose parent hash disagrees with the embedded QC.
  - CLI helper: `iroha sumeragi evidence submit --evidence-hex <hex>` or `--evidence-hex-file <path>`.

Additional consensus status and execution proofs

- GET `/v1/sumeragi/qc` — returns a Norito-encoded `SumeragiQcSnapshot` by default. Set `Accept: application/json` to receive `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
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
