# Sumeragi relay ingress fail-close — 2026-09-23

Scope: the existing `/Users/takemiyamakoto/devstuff/iroha` `optimizations`
checkout. This slice was edited during an unresolved external merge; no branch,
worktree, stage, or commit was created by this work.

The production relay handle could report `Accepted` for `LaneRelayMessage`
variants other than `QueuePlanAdmissionCertificate`. The Native runner's relay
drain in `crates/iroha_core/src/sumeragi/v2_runner.rs` transfers only QueuePlan
certificates to `QueuePlanAdmissionOwner` and retires every other variant.
`crates/irohad/src/nexus_fee_relay_worker.rs` uses the Boolean envelope helper
and inserts a finalized relay key into `announced_relays` after `true`, so a
Nexus envelope could be acknowledged and then silently discarded. The Native
lane process consumes a separate authenticated `BlockMessage` ingress and has
no owner for this older envelope.

`SumeragiHandle::try_incoming_lane_relay_owned` now returns
`Rejected(original)` for every non-QueuePlan variant before channel enqueue.
QueuePlan retains its existing nonempty/maximum-byte prefilter and exact
bounded-channel transfer. Redundant prechecks for retired sidecar and drain
variants were removed from this handle. The change does not convert a retired
message into a Native message or claim its protocol statement is equivalent.

Focused regressions in
`crates/iroha_core/src/sumeragi/tests/mod_authoritative_runtime_gate_03_admission_and_fairness.rs`
are:

- `retired_nexus_relay_envelope_returns_original_before_queue_admission`:
  exact constructed envelope returns to the caller, the Boolean helper is
  false, and the channel stays empty.
- `authenticated_lane_drain_vote_retains_original_owner_without_live_consumer`:
  a valid signed vote returns intact and does not enter the channel.
- `retired_lane_ingress_rejects_exact_messages_without_using_queue_plan_capacity`:
  merge signatures return intact without consuming the one channel slot; the
  subsequent QueuePlan occurrence is accepted and transfers the same `Arc`.
- `retired_sidecar_request_returns_exact_owner_before_queue_admission`:
  a valid request and authenticated return route come back intact, with no
  channel entry. The exact release-gate test selector was updated to this name
  in `scripts/run_sumeragi_v2_release_gates.sh`; no selector alias remains.

The runner's
`open_height_lane_relay_drain_services_exactly_one_occurrence_per_turn` case
now exercises handle rejection before proving that two QueuePlan occurrences
still reach the one-per-turn runner drain. The shared Core build ran the drain
vote, retired merge signature, retired sidecar and runner QueuePlan cases:
**4/4 passed**. A broader `retired_` selector passed **138/138**. The new Nexus
envelope case selected **0 tests** because its edit landed after that `rustc`
invocation began. A fresh `iroha_core` test build subsequently completed, and
the Nexus envelope, drain vote, merge signature, sidecar request, and runner
QueuePlan cases each passed (**5/5 focused cases**). These are scoped
test results, not a complete Core or F03 qualification. `rustfmt --check
--edition 2024 --config skip_children=true` on the
three changed Rust files and `git diff --check` on this slice passed. The host
`/bin/bash` 3.2 parser rejects an existing FD-closing syntax at line 892 of
the release script; the same error occurs on the unmodified `HEAD` script, so
it does not validate the new selector or indicate a new syntax regression.

The focused Core selectors used for this rebuilt check were:

```sh
cargo test --offline --locked -p iroha_core --lib retired_nexus_relay_envelope_returns_original_before_queue_admission -- --nocapture
cargo test --offline --locked -p iroha_core --lib authenticated_lane_drain_vote_retains_original_owner_without_live_consumer -- --nocapture
cargo test --offline --locked -p iroha_core --lib retired_lane_ingress_rejects_exact_messages_without_using_queue_plan_capacity -- --nocapture
cargo test --offline --locked -p iroha_core --lib retired_sidecar_request_returns_exact_owner_before_queue_admission -- --nocapture
cargo test --offline --locked -p iroha_core --lib open_height_lane_relay_drain_services_exactly_one_occurrence_per_turn -- --nocapture
```

This is an admission fail-close only. In `crates/irohad/src/main.rs`, network
`Rejected` becomes terminal `Failed` with an active reply route or `Retired`
without one; only `Retry(original)` schedules another forwarding attempt.
The Nexus fee worker calls the Boolean wrapper with a clone of the finalized
World record. On `false` it does not mark that record announced, so a later
reconciliation may try the retained finalized record again. The Boolean call
does not itself retain the rejected clone, and neither path guarantees that
all rejected producer work retries. The runner still has no Native/Nexus relay
replacement, and direct non-QueuePlan channel injection would still reach its
retired discard branch. The patch prevents false `Accepted` and silent runner
loss through the public handle; it does not complete the Native cutover,
durable ownership, lifecycle recovery, or release qualification. Multilane F03
remains **OPEN**.
