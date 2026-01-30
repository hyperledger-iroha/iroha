---
lang: ru
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-18T05:31:56.955438+00:00"
translation_last_reviewed: 2026-01-30
---

# Merge Ledger Design — Lane Finality and Global Reduction

This note finalises the merge-ledger design for Milestone 5. It explains the
non-empty block policy, cross-lane QC merge semantics, and the finality workflow
that binds lane-level execution to the global world state commitment.

The design extends the Nexus architecture described in `nexus.md`. Terms such as
"lane block", "lane QC", "merge hint", and "merge ledger" inherit their
definition from that document; this note focuses on behavioural rules and
implementation guidance that must be enforced by the runtime, storage, and WSV
layers.

## 1. Non-Empty Block Policy

**Rule (MUST):** A lane proposer issues a block only when the block contains at
least one executed transaction fragment, time-based trigger, or deterministic
artifact update (e.g., DA artifact roll-up). Empty blocks are forbidden.

**Implications:**

- Slot keep-alive: when no transaction meets its deterministic commit window,
the lane emits no block and simply advances to the next slot. The merge ledger
remains on the previous tip for that lane.
- Trigger batching: background triggers that produce no state transition (e.g.,
cron that reaffirms invariants) are considered empty and MUST be skipped or
bundled with other work before producing a block.
- Telemetry: `pipeline_detached_merged` and follow-up metrics treat skipped
slots explicitly—operators can distinguish "no work" from "pipeline stalled".
- Replay: block storage does not insert synthetic empty placeholders. The Kura
replay loop simply observes the same parent hash for consecutive slots if no
block was emitted.

**Canonical Check:** During block proposal and validation, `ValidBlock::commit`
asserts that the associated `StateBlock` carries at least one committed overlay
(delta, artifact, trigger). This aligns with the `StateBlock::is_empty` guard
that already ensures no-op writes are elided. Enforcement happens before
signatures are requested so committees never vote on empty payloads.

## 2. Cross-Lane QC Merge Semantics

Each lane block `B_i` finalised by its committee produces:

- `lane_state_root_i`: Poseidon2-SMT commitment over per-DS state roots touched
in the block.
- `merge_hint_root_i`: rolling candidate for the merge ledger (`tag =
"iroha:merge:candidate:v1\0"`).
- `lane_qc_i`: aggregated signatures from the lane committee over the
  execution-vote preimage (block hash, `parent_state_root`,
  `post_state_root`, height/view/epoch, chain_id, and mode tag).

Merge nodes collect the latest tips `{(B_i, lane_qc_i, merge_hint_root_i)}` for
all lanes `i ∈ [0, K)`.

**Merge Entry (MUST):**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```

- `lane_tips[i]` is the hash of the lane block the merge entry seals for lane
  `i`. If a lane emitted no block since the previous merge entry, this value is
  repeated.
- `merge_hint_root[i]` is the `merge_hint_root` from the corresponding lane
  block. It is repeated when `lane_tips[i]` repeats.
- `global_state_root` equals `ReduceMergeHints(merge_hint_root[0..K-1])`, a
  Poseidon2 fold with domain separation tag
  `"iroha:merge:reduce:v1\0"`. The reduction is deterministic and MUST
  reconstruct the same value across peers.
- `merge_qc` is a BFT quorum certificate from the merge committee over the
  serialized entry.

**Merge QC Payload (MUST):**

Merge committee members sign a deterministic digest:

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` is the merge-committee view derived from the lane tips (max
  `view_change_index` across the lane headers sealed by the entry).
- `chain_id` is the configured chain identifier string (UTF-8 bytes).
- The payload uses Norito encoding with the field order shown above.

The resulting digest is stored in `merge_qc.message_digest` and is the message
verified by BLS signatures.

**Merge QC Construction (MUST):**

- The merge committee roster is the current commit-topology validator set.
- Required quorum = `commit_quorum_from_len(roster_len)`.
- `merge_qc.signers_bitmap` encodes participating validator indices (LSB-first)
  in commit-topology order.
- `merge_qc.aggregate_signature` is the BLS-normal aggregate for the digest
  above.

**Validation (MUST):**

1. Verify each `lane_qc_i` against `lane_tips[i]` and confirm the block headers
   include the matching `merge_hint_root_i`.
2. Ensure no `lane_qc_i` points to an `Invalid` or unexecuted block. The
   non-empty policy above ensures the header includes state overlays.
3. Recompute `ReduceMergeHints` and compare with `global_state_root`.
4. Recompute the merge QC digest and verify the signer bitmap, quorum threshold,
   and aggregate signature against the commit-topology roster.

**Observability:** Merge nodes emit Prometheus counters for
`merge_entry_lane_repeats_total{i}` to highlight lanes that skipped slots for
operational visibility.

## 3. Finality Workflow

### 3.1 Lane-Level Finality

1. Transactions are scheduled per lane in deterministic slots.
2. The executor applies overlays into `StateBlock`, producing deltas and
artifacts.
3. Upon validation, the lane committee signs the execution-vote preimage that
   binds the block hash, state roots, and height/view/epoch. The tuple
   `(block_hash, lane_qc_i, merge_hint_root_i)` is considered lane-final.
4. Light clients MAY treat the lane tip as final for DS-limited proofs, but
must record the associated `merge_hint_root` to reconcile with the merge ledger
later.

Lane committees are per-dataspace and do not replace the global commit
topology. Committee size is fixed at `3f+1`, where `f` comes from the
dataspace catalog (`fault_tolerance`). The validator pool is the dataspace's
validators (lane governance manifests for admin-managed lanes, or public-lane
staking records for stake-elected lanes). Committee membership is
deterministically sampled once per epoch using the VRF epoch seed bound with
`dataspace_id` and `lane_id`. If the pool is smaller than `3f+1`, lane finality
pauses until quorum is restored (emergency recovery is handled separately).

### 3.2 Merge-Ledger Finality

1. Merge committee collects the latest lane tips, verifies each `lane_qc_i`, and
constructs the `MergeLedgerEntry` as defined above.
2. After verifying the deterministic reduction, the merge committee signs the
entry (`merge_qc`).
3. Nodes append the entry to the merge ledger log and persist it alongside the
lane block references.
4. `global_state_root` becomes the authoritative world state commitment for the
epoch/slot. Full nodes update their WSV checkpoint metadata to mirror this
value; deterministic replay must reproduce the same reduction.

### 3.3 WSV and Storage Integration

- `State::commit_merge_entry` records the per-lane state roots and the
  final `global_state_root`, bridging lane execution with the global checksum.
- Kura persists `MergeLedgerEntry` adjacent to the lane block artifacts so a
  replay can reconstruct both lane-level and global finality sequences.
- When a lane skips a slot, storage simply retains the previous tip; no
  placeholder merge entries are created until at least one lane produces a new
  block.
- API surfaces (Torii, telemetry) expose both lane tips and the latest merge
  entry so operators and clients can reconcile per-lane and global views.

## 4. Implementation Notes

- `crates/iroha_core/src/state.rs`: `State::commit_merge_entry` validates the
  reduction and wires the lane/global metadata into the world state so queries
  and observers can access the merge hints and the authoritative global hash.
- `crates/iroha_core/src/kura.rs`: `Kura::store_block_with_merge_entry` enqueues
  the block and persists the associated merge entry in one step, rolling back
  the in-memory block when the append fails so storage never records a block
  without its sealing metadata. The merge-ledger log is pruned in lock-step
  with the validated block height during startup recovery, and cached in memory
  with a bounded window (`kura.merge_ledger_cache_capacity`, default 256) to
  avoid unbounded growth on long-running nodes. Recovery truncates partial or
  oversized merge-ledger tail entries, and append rejects entries above the
  maximum payload size guard to cap allocations.
- `crates/iroha_core/src/block.rs`: block validation rejects blocks without
  entrypoints (external transactions or time triggers) and without deterministic
  artifacts such as DA bundles (`BlockValidationError::EmptyBlock`), ensuring
  the non-empty policy is enforced before signatures are requested and carried
  into the merge ledger.
- Deterministic reduction helper lives in the merge service: `reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) implements the Poseidon2 fold described above.
  Hardware acceleration hooks remain future work, but the scalar path now enforces
  the canonical reduction deterministically.
- Telemetry integration: exposing per-lane merge repeats and the
  `global_state_root` gauge remains tracked in the observability backlog so the
  dashboard work can ship alongside the merge service rollout.
- Cross-component tests: golden replay coverage for the merge reduction is
  tracked with the integration-test backlog to ensure future changes to
  `reduce_merge_hint_roots` keep the recorded roots stable.
