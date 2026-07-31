# Sumeragi Evidence (Audit API)

Sumeragi evidence audit endpoints.

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
  - Query params: `limit` (default 50, max 1000), `offset` (default 0), `kind` (optional; one of `DoublePrepare|DoubleCommit|InvalidQc|InvalidProposal|Censorship|SumeragiV2Equivocation`).
  - Response (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - Set `Accept: application/json` to receive a JSON object `{ "total": <u64>, "items": [ ... ] }`.
  - `EvidenceRecord` entries include `penalty_applied`, `penalty_cancelled`, `penalty_cancelled_at_height`, `penalty_applied_at_height`, and `consensus_admitted_at_height`. For exact Sumeragi v2 equivocation evidence, a missing admission height means the proof is only a node-local pending observation and cannot drive penalties; a committed admission height is identical on every peer.
  - Governance cancellation of exact v2 evidence is accepted only after that admission height is committed. This prevents a transaction from depending on a node-local pending observation that other validators may not have received.
- Evidence with a subject height older than `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7 200) is dropped on ingress; the actor logs the rejection to help operators
  investigate stale submissions.
- POST `/v1/sumeragi/evidence`
  - Submit hex-encoded Norito evidence to the Sumeragi actor (`ControlFlow::Evidence`).
  - Request body (JSON): `{ "evidence_hex": "<hex string>" }`; the hex string encodes Norito-framed `ConsensusEvidence` bytes and ignores whitespace.
  - Response (JSON): `{ "status": "accepted", "kind": "<variant>" }` on success.
  - Validation covers signer/height/view/epoch equality for double-vote payloads, requires non-empty single-signer payloads, enforces receipt quorums for `Censorship` evidence (signed `TransactionSubmissionReceipt` payloads), and rejects `InvalidProposal` records that fail to advance height or whose parent hash disagrees with the embedded commit certificate. `SumeragiV2Equivocation` retains the frozen height context, roster-ordered BLS proofs of possession, and the exact two signed proposal, phase-vote, or timeout-vote artifacts; validators canonicalize pair order, anchor the context to immutable committed v2 finality/context history, reverify every PoP, both signatures, and referenced current-context certificates, and deduplicate the durable WSV key across restart replay. Proposers may attach up to eight canonically ordered exact proofs (at most 4 MiB encoded) to a signed block. Followers validate those proofs without relying on local gossip, reject stale, future, forged, unanchored, duplicate, reordered, oversized, or previously admitted batches, and only derive penalties from admissions committed by an earlier block.
  - CLI helper: `iroha ops sumeragi evidence submit --evidence-hex <hex>` or `--evidence-hex-file <path>`.

Additional consensus status and commit QC proofs

- GET `/v1/sumeragi/status` — returns the typed Norito `SumeragiV2StatusResponse`
  envelope by default. With `Accept: application/json`, Torii flattens the authoritative reducer
  status and adds all five canonical lane arrays, `local_peer_removed`, and bounded local
  `operator` diagnostics. Tagged unit enums retain their Norito object form, fixed byte arrays are
  uppercase exact-width hex, and lane settlement `u128` totals and receipt amounts are canonical
  unsigned decimal strings. Optional reducer artifacts are omitted when absent. See
  `specs/sumeragi_v2.md` and the `SumeragiStatusResponse` OpenAPI schema for the exact fields.
- GET `/v1/sumeragi/qc` — returns a Norito-encoded highest/locked QC snapshot (`SumeragiQcSnapshot`) by default. Set `Accept: application/json` to receive `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` — SSE stream of the same payload (≈1s cadence).
- GET `/v1/sumeragi/commit-qcs/{block_hash}` — returns a Norito-encoded `Option<Qc>` for `:hash` (block hash) by default. With `Accept: application/json` the response expands to:
  - If present, `{ subject_block_hash, commit_qc: { phase, parent_state_root, post_state_root, height, view, epoch, mode_tag, validator_set_hash, validator_set_hash_version, validator_set, signers_bitmap, bls_aggregate_signature } }`.
  - If missing, returns `{ subject_block_hash, commit_qc: null }`.

Example (curl)

```bash
# Replace HASH with a real block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/commit-qcs/$HASH | jq .

# Example response (when present):
# {
#   "subject_block_hash": "BA6733…F5B2",
#   "commit_qc": {
#     "phase": "Commit",
#     "parent_state_root": "1f9a7d…2c0e",
#     "post_state_root": "9b2f11…a12c",
#     "height": 42,
#     "view": 3,
#     "epoch": 0,
#     "mode_tag": "iroha2-consensus::permissioned-sumeragi@v2",
#     "validator_set_hash": "…",
#     "validator_set_hash_version": 1,
#     "validator_set": ["…"],
#     "signers_bitmap": "0700",
#     "bls_aggregate_signature": ""
#   }
# }
```

Note
- Commit QCs always bind the parent/post state roots; there is no separate execution-root endpoint.
