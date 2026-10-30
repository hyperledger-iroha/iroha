% Proof retention and pruning

Iroha keeps a registry of proof verification results (backend + hash) for audit
and replay. Retention is enforced deterministically inside the consensus path.

## Policy

The committed `zk_validity_policy_v1` parameter controls retention:

- `proofs.proof_history_cap` — max records per backend (0 = unlimited)
- `proofs.proof_retention_grace_blocks` — grace window for age-based pruning
- `proofs.proof_prune_batch` — max removals per enforcement pass (0 = unlimited)

An authorized `SetParameter` changes the policy for subsequent blocks. Node-local
`[zk]` settings cannot override these validity values.

## Enforcement

- **On insert:** `VerifyProof` enforces the cap/grace/batch for the proof’s
  backend after recording the new entry.
- **Manual:** the Initial executor admits `PruneProofs` from any signed account.
  It prunes all backends (or the selected backend) under the same committed policy;
  the caller cannot supply different cap, grace or batch values. Use the CLI helper:

  ```bash
  iroha app zk proofs prune --backend pipa-r/pasta
  ```

Both paths emit `ProofEvent::Pruned` with the backend, removed ids (bounded by
`prune_batch`), remaining count, cap/grace/batch, height, authority, and origin
(`Insert` or `Manual`) for audit trails.

## Surfacing and tooling

- Status endpoint: `GET /v1/proofs/retention` returns caps, grace, prune_batch,
  total records, total prunable, and per-backend counts. Because these are
  operator status details, the request requires a fresh, exact-network
  `OperatorSignature` over `GET`, the exact path and query, and an empty body;
  redirects, retries, and token fallback are rejected. Status aggregation keeps
  only one scalar summary per backend; it does not clone, sort, or retain the
  full proof-id population in response memory. The first-release protocol caps
  one status response at 256 distinct backends and fails closed with HTTP 422
  before inserting a 257th backend summary.
- CLI: `iroha app zk proofs retention` (status) and `iroha app zk proofs prune` (manual
  enforcement).
- Events: subscribe to `DataEvent::Proof(ProofEvent::Pruned)` via SSE/WS filters
  to watch pruning activity.

Pruning is deterministic and runs inside block execution, so all peers converge
on the same retained set. Removed proof ids are also stripped from the tag
indexes.
