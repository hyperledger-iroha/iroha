# Sumeragi Evidence (Audit API)

Sumeragi evidence audit endpoints.

All finite reads on this page require a fresh allow-listed operator signature
bound to the node's exact runtime `NetworkId`, method, target, and empty body.
The maintained CLI accepts that key through the explicit absolute
`--operator-private-key-file` runtime option or the mutually exclusive inherited,
read-only `--operator-private-key-fd` descriptor (3–65535); it never falls back to an account
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
  - Query params: `limit` (default 50, range 1..=1000), `offset` (default 0, range 0..=10000), `kind` (optional; the sole accepted value is `NativeSumeragiEvidence`).
  - Response (Norito payload): the shared `SumeragiEvidenceListWireResponse { total: u64, items: Vec<EvidenceRecord> }` DTO.
  - Set `Accept: application/json` to receive a JSON object `{ "total": <u64>, "items": [ ... ] }`.
  - The projected JSON body is limited to 1 MiB. The full-proof Norito body is
    limited to 17 MiB: committed proof payloads consume at most 16 MiB and the
    remaining budget covers the bounded record and frame envelope. Torii
    measures first and allocates only an accepted exact-size body.
  - Every JSON audit item includes the non-null `consensus_admitted_height` and one closed `penalty_status` object. Its exact shape is `{ "status": "pending", "details": null }` or `{ "status": "applied", "details": { "height": <u64> } }`; the terminal height is the canonical block that applied the mandatory penalty. Unsupported lifecycle tags fail binary and JSON decoding.
  - The persisted first-release Norito `EvidenceRecord` stores `recorded_at_height`, `recorded_at_view`, `recorded_at_ms`, and the same closed `EvidencePenaltyStatus` sum type. Shortened pre-release records and retired boolean/nullable penalty layouts are rejected rather than default-filled.
  - `offenders` may be empty only for a `conflicting_certificates` item with
    `safety_violation: true`: CommitQCs from different views establish a safety
    violation without attributing individual signers. SDKs preserve that report
    and its exact proof hash; they never invent an offender to make it parse.
  - `EvidenceRecord` is not itself the JSON response DTO. Torii exposes a fixed, closed audit projection; the embedded `Evidence` holds one canonical native Sumeragi evidence frame (`iroha_sumeragi::message::Evidence`).
  - Node-local pending observations have no data-model record and never appear in either endpoint. No instruction cancels a mandatory penalty. The canonical lifecycle has only `pending` and `applied` states.
- Root evidence uses its authenticated global subject height for the governed
  `SumeragiNposParameters.reconfig.evidence_horizon_blocks` check. Lane evidence
  uses its authenticated incarnation's lifetime: live custody remains liable,
  and retirement admits through `retired_at + evidence_horizon_blocks`, inclusive.
  A native lane subject height is never interpreted as a global horizon or tenure
  clock. This signed horizon and `slashing_delay_blocks` are immutable after
  initial installation; their sum cannot exceed three epochs. They are on-chain
  state, not local `[sumeragi]` config or executor-owned defaults.

Evidence mutation is not an HTTP or CLI operation. Evidence enters through the
authenticated consensus peer path and, for exact native equivocation proofs,
through canonically ordered proof batches bound to signed blocks. Validators
anchor the frozen height context only to cryptographically verified committed
finality history (never the structural recovery context store), then reverify
roster-ordered proofs of possession, both artifact signatures, referenced
current-context certificates, the evidence lifetime, canonical ordering, batch
bounds, and the durable deduplication key before admission. Lane proofs additionally
verify the original creation and admission-parent carriers, pinned committee and
proofs of possession, every required native ancestry/demotion header, and the exact
globally merged branch frontier. Their persisted attribution carries an explicit
lane scope and original signer custody; missing positive creation-time custody is
forensic-only. Torii and the SDKs expose only the two read-only audit endpoints above.

Committed evidence is part of canonical WSV snapshot state. An at-tip restart
must restore each pending or terminal record exactly; peer-local gossip is not
a reconstruction authority for penalty liens or replay fences. The table holds
at most four complete validator rosters (124 records) and at most 16 MiB of
canonical proof payloads after stale terminal records are reclaimed. Candidate
validation, post-execution insertion, snapshot recovery, and proposer selection
all enforce the same checked byte accounting. Saturation defers additional admissions
without evicting retained proofs. The immutable penalty delay starts at the global
admission carrier. A live lane's terminal record remains a replay fence; a retired
lane's terminal record is prunable only strictly after its admission deadline and
only with its authenticated original custody row and immutable policy. Unknown,
reclaimed or substituted incarnations cannot authorize admission or reopen replay.

Existing-State record restoration authenticates original carrier inclusion, attribution,
clock and terminal effects while retaining the original native reader on local refusal.
Typed original-history and custody decoder budget refusals defer under the same owner,
including consuming handoffs. Missing or corrupt lane sources fail that acquisition and
release its pending admission slot; local proposer selection can continue to another proof
without deleting the failed observation or treating a storage failure as signer guilt.
This does not qualify accelerated whole-State startup: nonempty snapshot caches remain
rejected until complete State and certified-history restoration is authenticated.

The binary `Evidence` shape carries only the canonical native frame. Retired kind/payload
records fail decode and are never reconstructed from mutable topology state.

Additional consensus status

- GET `/v1/sumeragi/status` — operator-authenticated; returns the node's
  `SumeragiStatus` (Norito by default, JSON with `Accept: application/json`):
  protocol version, configuration fingerprint, instance id, round
  height/view/stage, leader and proxy tail, lock view, pacemaker levels,
  committed and applied heights, signer, halt reason and footprint counters.
  It answers `503` before consensus starts. See [`sumeragi.md`](sumeragi.md).
- GET `/v1/sumeragi/lanes` — every lane of the committed state with the status
  of the node's instance of it ([`sumeragi_lanes.md`](sumeragi_lanes.md) §8).
- GET `/v1/sumeragi/status/sse` — operator-authenticated SSE stream of the same payload (≈1s cadence).

The current authenticated ledger state-root and proof contract is specified in
[`ledger_state_finality.md`](ledger_state_finality.md). Retired mutable-QC and
validator-set projections are not exposed by Torii.
