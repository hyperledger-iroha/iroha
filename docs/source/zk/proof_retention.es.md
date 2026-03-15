---
lang: es
direction: ltr
source: docs/source/zk/proof_retention.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 16b5b6bd9cf1bfdc901d0aa3c863addd77e033a0e91a8187fa354b4d22058bf9
source_last_modified: "2026-01-22T15:38:30.730892+00:00"
translation_last_reviewed: 2026-01-30
---

% Proof retention and pruning

Iroha keeps a registry of proof verification results (backend + hash) for audit
and replay. Retention is enforced deterministically inside the consensus path.

## Configuration

The `zk` config controls retention:

- `zk.proof_history_cap` — max records per backend (0 = unlimited)
- `zk.proof_retention_grace_blocks` — minimum blocks to keep before age-based pruning
- `zk.proof_prune_batch` — max removals per enforcement pass (0 = unlimited)

These values are read from `iroha_config` (user → actual → defaults).

## Enforcement

- **On insert:** `VerifyProof` enforces the cap/grace/batch for the proof’s
  backend after recording the new entry.
- **Manual:** the new `PruneProofs` instruction prunes all backends (or a single
  backend when provided) using the same policy. Use the CLI helper:

  ```bash
  iroha app zk proofs prune --backend halo2/ipa
  ```

Both paths emit `ProofEvent::Pruned` with the backend, removed ids (bounded by
`prune_batch`), remaining count, cap/grace/batch, height, authority, and origin
(`Insert` or `Manual`) for audit trails.

## Surfacing and tooling

- Status endpoint: `GET /v2/proofs/retention` returns caps, grace, prune_batch,
  total records, total prunable, and per-backend counts.
- CLI: `iroha app zk proofs retention` (status) and `iroha app zk proofs prune` (manual
  enforcement).
- Events: subscribe to `DataEvent::Proof(ProofEvent::Pruned)` via SSE/WS filters
  to watch pruning activity.

Pruning is deterministic and runs inside block execution, so all peers converge
on the same retained set. Removed proof ids are also stripped from the tag
indexes.
