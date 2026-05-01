# Sumeragi Formal Model (TLA+ / Apalache)

This directory contains bounded formal models for Sumeragi safety and liveness.

## Scope

`Sumeragi.tla` captures the commit path:
- phase progression (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- vote and quorum thresholds (`CommitQuorum`, `ViewQuorum`),
- weighted stake quorum (`StakeQuorum`) for NPoS-style commit guards,
- RBC causality (`Init -> Chunk -> Ready -> Deliver`) with header/digest evidence,
- GST and weak fairness assumptions over honest progress actions.

`SumeragiFrontierRecovery.tla` captures the focused Taira hang class around one
active pending contiguous frontier block plus one concrete future frontier slot:
- commit-vote evidence below or at quorum,
- vote queue backlog and local drain,
- missing vs. local payload state,
- fresh vs. stale frontier recovery ownership,
- quorum-reschedule marker/window pacing,
- future slot presence, contiguity, vote evidence, payload state, and recovery
  ownership,
- future frontier/new-view evidence derived from that future slot and consumed
  through a two-step reanchor/promotion path,
- deterministic post-GST commit, retransmit, bounded view-rotation, and
  zero-evidence drop outcomes.

Both models intentionally abstract away wire formats, ECDSA/signature
verification, and full networking details.

## Files

- `Sumeragi.tla`: protocol model and properties.
- `Sumeragi_fast.cfg`: smaller CI-friendly parameter set.
- `Sumeragi_deep.cfg`: larger stress parameter set.
- `SumeragiFrontierRecovery.tla`: focused frontier recovery model.
- `SumeragiFrontierRecovery_fast.cfg`: smaller CI-friendly frontier parameter set.
- `SumeragiFrontierRecovery_deep.cfg`: larger frontier backlog/window/view bound set.
- `SumeragiFrontierRecovery_wide.cfg`: wider frontier bound set used by formal CI.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: expected-failure stale-owner mutation.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: expected-failure vote-queue mutation.
- `SumeragiFrontierRecovery_bug_payload_recovery.cfg`: expected-failure payload-recovery mutation.
- `SumeragiFrontierRecovery_bug_retransmit_followthrough.cfg`: expected-failure retransmit-follow-through mutation.
- `SumeragiFrontierRecovery_bug_future_promotion.cfg`: expected-failure future-promotion mutation.

## Properties

Invariants:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Temporal property:
- `EventuallyCommit` (`[] (gst => <> committed)`), with post-GST fairness encoded
  operationally in `Next` (timeout/fault preemption guards on enabled
  progress actions). This keeps the model checkable with Apalache 0.52.x, which
  does not support `WF_` fairness operators inside checked temporal properties.

Frontier recovery invariants:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, which rules out a terminal
  post-GST state where `pending /\ voteBacked /\ ~committed` has no recovery,
  commit, retransmit, rotation, or bounded-drop transition.
- `FuturePromotionReadyHasProgress`, which rules out a terminal post-GST
  state where the current pending wrapper has cleared for future evidence but
  the future slot cannot be promoted.

Frontier recovery temporal property:
- `PostGstVoteBackedFrontierEventuallyResolves`: after GST, every unresolved
  active vote-backed pending frontier state eventually clears its pending
  wrapper.
- `RecoveredPayloadEventuallyAdvances`: a vote-backed frontier state that has
  recovered the payload cannot remain pending forever without commit,
  retransmit, reanchor, or rotation.
- `QuorumRetransmitEventuallyLeavesPending`: once quorum retransmit has fired
  for a vote-backed frontier state, the pending wrapper must eventually clear.
- `FutureFrontierEvidenceEventuallyReanchors`: later frontier/new-view evidence
  must be consumed through reanchor and future-slot promotion.
- `FuturePromotionReadyEventuallyPromotes`: a cleared current wrapper with
  promotion-ready future evidence must eventually promote that future slot.
- `PromotedSecondSlotEventuallyClears`: after promotion, the second slot must
  satisfy the same vote-backed pending-clear property as the original active
  slot.

## Assumption map

The frontier model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `frontierSlot`, `pending`, `contiguous`, `payloadState` | `PendingBlock` handling and local payload checks in `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, plus BlockCreated/frontier ownership materialization in `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Commit-vote counting and vote ingress gating exercised by `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` and `reschedule_ignores_quorum_timeout_vote_queue_backlog` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | Active/stale frontier owner state in `frontier_slot_has_active_owner_state_for_view(...)`, stale-owner yield in `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`, and supersede cleanup in `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Vote-backed quorum reschedule pacing in `reschedule_stale_pending_blocks_with_now(...)`; regression coverage includes `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Exact frontier body repair and stale RBC repair admission in `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)`, and `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Quorum retransmit target selection, `rebroadcast_pending_block_updates(...)`, and deterministic view-change calls in `reschedule_stale_pending_blocks_with_now(...)`. |
| `futurePresent`, `futureContiguous`, `futureCommitVotes`, `futureQueuedVotes`, `futurePayloadState`, `futureRecoveryOwner` | One concrete future frontier slot. `FutureFrontierEvidence` is derived from the slot instead of stored as an independent Boolean. |
| `futurePromotionReady`, `futurePromoted` | The two-step future reanchor path: clear the stale/current pending wrapper, then promote the future slot into the active slot with active progress flags reset. This maps to future new-view / higher-frontier quorum handling in `on_pacemaker_propose_ready(...)`, covered by `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## Running

From repository root:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

The runner sets an explicit Apalache `--length` for each mode:

| Mode | Length | Intended use |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path check |
| `deep` | 10 | Larger commit-path check |
| `frontier-fast` | 10 | CI frontier check |
| `frontier-deep` | 12 | Larger frontier check |
| `frontier-wide` | 12 | Wider PR formal CI frontier check |

`APALACHE_LENGTH=<n>` overrides the per-mode default when locally exploring a
counterexample or widening a bounded proof.

### Reproducible local setup (no Docker required)

Install the pinned local Apalache toolchain used by this repository:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

The runner auto-detects this install at:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
After installation, `ci/check_sumeragi_formal.sh` should work without extra env vars:

```bash
bash ci/check_sumeragi_formal.sh
```

The expected-failure mutations are part of normal formal CI through
`ci/check_sumeragi_formal.sh`. They should fail under Apalache and are useful
when changing the model:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Individual mutation modes are also accepted by the runner:

```bash
bash scripts/formal/sumeragi_apalache.sh frontier-bug-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-vote-queue
bash scripts/formal/sumeragi_apalache.sh frontier-bug-payload-recovery
bash scripts/formal/sumeragi_apalache.sh frontier-bug-retransmit-followthrough
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-promotion
```

If Apalache is not in `PATH`, you can:

- set `APALACHE_BIN` to the executable path, or
- use the Docker fallback (enabled by default when `docker` is available):
  - image: `APALACHE_DOCKER_IMAGE` (default `ghcr.io/apalache-mc/apalache:0.52.2`)
  - requires a running Docker daemon
  - disable fallback with `APALACHE_ALLOW_DOCKER=0`.

Examples:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Notes

- This model complements (does not replace) executable Rust model tests in
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  and
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- The checks are bounded by constant values in the `.cfg` files.
- PR CI runs these checks in `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.
- English documentation is authoritative for the current frontier formal slice.
  Translated `docs/formal/sumeragi/README.*.md` files are intentionally not
  refreshed here and may remain source-current stale until a separate
  translation refresh.
