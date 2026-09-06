# Sumeragi Evidence (Audit API)

Sumeragi evidence audit endpoints.

All finite reads on this page require a fresh allow-listed operator signature
bound to the node's exact runtime `NetworkId`, method, target, and empty body.
The maintained CLI accepts that key only through the explicit absolute
`--operator-private-key-file` runtime option; it never falls back to an account
key, token, environment variable, or client TOML credential.
Omitting `Accept` selects canonical Norito, while an explicit JSON-compatible
range selects JSON; unacceptable or malformed negotiation returns a JSON `406`
error. Every negotiated response declares `Vary: Accept`.

- GET `/v1/sumeragi/evidence/count`
  - Returns the number of unique evidence entries admitted by committed blocks.
  - Response (Norito payload): `count: u64`.
  - Set `Accept: application/json` to receive `{ "count": <u64> }`.
  - Both encodings have a 1 KiB response-body ceiling enforced before allocation.
  - Notes:
    - Backed by the per-node WSV store (`world.consensus_evidence`) persisted with Norito codecs.
    - Survives restarts and feeds `/v1/sumeragi/evidence`; entries are deduplicated by evidence hash.
    - Node-local pending proofs may differ, but only canonically ordered proofs
      admitted by a committed block are replicated and penalty-eligible.

- GET `/v1/sumeragi/evidence`
  - Lists recent evidence entries admitted by committed blocks and retained in
    the WSV audit snapshot; node-local pending observations are excluded.
  - Query params: `limit` (default 50, range 1..=1000), `offset` (default 0, range 0..=10000), `kind` (optional; the sole accepted value is `SumeragiV2Equivocation`).
  - Response (Norito payload): the shared `SumeragiEvidenceListWireResponse { total: u64, items: Vec<EvidenceRecord> }` DTO.
  - Set `Accept: application/json` to receive a JSON object `{ "total": <u64>, "items": [ ... ] }`.
  - The projected JSON body is limited to 1 MiB. The full-proof Norito body is
    limited to 17 MiB: committed proof payloads consume at most 16 MiB and the
    remaining budget covers the bounded record and frame envelope. Torii
    measures first and allocates only an accepted exact-size body.
  - Every JSON audit item includes the non-null `consensus_admitted_height` and one closed `penalty_status` object. Its exact shape is `{ "status": "pending", "details": null }`, `{ "status": "applied", "details": { "height": <u64> } }`, or `{ "status": "cancelled", "details": { "height": <u64> } }`; the terminal height is the canonical block that applied or cancelled the penalty.
  - The persisted first-release Norito `EvidenceRecord` stores `recorded_at_height`, `recorded_at_view`, `recorded_at_ms`, and the same closed `EvidencePenaltyStatus` sum type. Shortened pre-release records and retired boolean/nullable penalty layouts are rejected rather than default-filled.
  - `EvidenceRecord` is not itself the JSON response DTO. Torii exposes a fixed, closed audit projection; full typed `SumeragiV2EquivocationEvidence` JSON, where embedded in signed data, is also a closed object.
  - Node-local pending observations have no data-model record and never appear in either endpoint. Governance cancellation is accepted only after the proof has been admitted by a committed block, so a transaction never depends on an observation other validators may not have received.
- Evidence with a subject height older than governed
  `SumeragiNposParameters.reconfig.evidence_horizon_blocks` is dropped on
  ingress; the actor logs the rejection to help operators investigate stale
  submissions. This signed value and `slashing_delay_blocks` are immutable
  after initial installation; their sum cannot exceed three epochs. They are
  on-chain state, not local `[sumeragi]` config or executor-owned defaults.

Evidence mutation is not an HTTP or CLI operation. Evidence enters through the
authenticated consensus peer path and, for exact v2 equivocation proofs,
through canonically ordered proof batches bound to signed blocks. Validators
anchor the frozen height context only to cryptographically verified committed
v2 finality history (never the structural recovery context store), then reverify
roster-ordered proofs of possession, both artifact signatures, referenced
current-context certificates, the evidence horizon, canonical ordering, batch
bounds, and the durable deduplication key before admission. Torii and the SDKs
  expose only the two read-only audit endpoints above.

Committed evidence is part of canonical WSV snapshot state. An at-tip restart
must restore each pending or terminal record exactly; peer-local gossip is not
a reconstruction authority for penalty liens or replay fences. The table holds
at most four complete validator rosters (124 records) and at most 16 MiB of
canonical proof payloads after stale terminal records are reclaimed. Candidate
validation, post-execution insertion, snapshot recovery, and proposer selection
all enforce the same checked byte accounting.

The binary `Evidence` shape is also v2-only. Retired global-v1 kind/payload
records fail decode and are never reconstructed from mutable topology state.

Additional consensus status

- GET `/v1/sumeragi/status` — returns the typed Norito `SumeragiV2StatusResponse`
  envelope by default. With `Accept: application/json`, Torii flattens the authoritative reducer
  status and adds all five canonical lane arrays, `local_peer_removed`, and bounded local
  `operator` diagnostics. Tagged unit enums retain their Norito object form, fixed byte arrays are
  uppercase exact-width hex, and lane settlement `u128` totals and receipt amounts are canonical
  unsigned decimal strings. Every current reducer-status field is required; nullable QC/TC,
  persistence, commit-frontier, outbound-intent, queue-age, and liveness artifacts use explicit
  `null` when absent. A sparse projection that omits one of those slots is rejected. See
  `specs/sumeragi_v2.md` and the `SumeragiStatusResponse` OpenAPI schema for the exact fields.
- GET `/v1/sumeragi/qc` — returns the canonical `SumeragiV2QcResponse` by default. Its required `highest_prepare_qc` and `locked_prepare_qc` slots are nullable; each non-null value carries the full context-bound `QuorumCertificateRef`. Set `Accept: application/json` for the identical schema, including explicit `null` for an unavailable reference.
- GET `/v1/sumeragi/status/sse` — operator-authenticated SSE stream of the same payload (≈1s cadence).

The current authenticated ledger state-root and proof contract is specified in
[`ledger_state_finality.md`](ledger_state_finality.md). Retired mutable-QC and
validator-set projections are not exposed by Torii.
