# Sumeragi Evidence (Audit API)

Sumeragi evidence audit endpoints.

All finite reads on this page require a fresh allow-listed operator signature
bound to the node's exact runtime `NetworkId`, method, target, and empty body.
The maintained CLI accepts that key only through the explicit absolute
`--operator-private-key-file` runtime option; it never falls back to an account
key, token, environment variable, or client TOML credential.

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
- Evidence with a subject height older than governed
  `SumeragiNposParameters.reconfig.evidence_horizon_blocks` is dropped on
  ingress; the actor logs the rejection to help operators investigate stale
  submissions. This value is on-chain state, not local `[sumeragi]` config.

Evidence mutation is not an HTTP or CLI operation. Evidence enters through the
authenticated consensus peer path and, for exact v2 equivocation proofs,
through canonically ordered proof batches bound to signed blocks. Validators
anchor the frozen height context only to cryptographically verified committed
v2 finality history (never the structural recovery context store), then reverify
roster-ordered proofs of possession, both artifact signatures, referenced
current-context certificates, the evidence horizon, canonical ordering, batch
bounds, and the durable deduplication key before admission. Torii and the SDKs
expose only the two read-only audit endpoints above.

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

Example (CLI)

```bash
# Replace HASH with a real block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

iroha --operator-private-key-file /run/secrets/iroha/operator.key \
  ops sumeragi commit-qc-get --hash "$HASH"

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
