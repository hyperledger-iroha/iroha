# Sumeragi revision-4 deadlines and retransmission

This note records the implemented first-release revision-4 timing contract.
The old EMA/RTT/jitter pacemaker and its `sumeragi.advanced.*` configuration
were V1 surfaces and are not part of the live protocol.

## Deterministic timing

The signed genesis/current-height context supplies the block cadence. Every
validator derives the same two values:

- view-zero round deadline: `signed_block_cadence * 10`;
- critical-message retransmission interval: `view_zero_deadline / 5`.

For certified view `v`, the runtime deadline is
`min(view_zero_deadline * (v + 1), view_zero_deadline * 10)`. The same bound
applies to autonomous lane NewView clocks, while retransmission remains fixed.
A zero cadence or an overflowing view-zero derivation is rejected before the
runner starts.

There is no local timeout override, RTT floor, latency EMA input, random or
peer-specific jitter, adaptive pacing governor, or DA-specific reschedule
switch. Those inputs would let validators derive different schedules from the
same signed height context.

## Clock ownership

Startup and recovery work do not consume the first live view's deadline. The
runtime arms both clocks once height construction and startup effects finish.
After that boundary, only a certified `EnterView` transition restarts the
round and retransmission clocks. A local elapsed timer may emit a signed
timeout vote or retransmit authenticated evidence; it cannot install a new
view by itself. A quorum TimeoutCertificate authorizes the view transition.

## Operator diagnosis

When progress slows, preserve the signed height context and shared Sumeragi
configuration fingerprint together with authenticated status/telemetry. Check
that at least `2f + 1` committee members are responsive, body reconstruction
and durable storage terminate, and finite ingress/effect queues are not
saturated. Pacemaker-named backpressure metrics describe producer deferral;
they do not expose a timer-tuning API.

Change cadence through a signed genesis/current-height rollout. Change only
finite node-local bounds through current `sumeragi.block`, `sumeragi.queues`,
or `sumeragi.limits` fields, and keep their fingerprinted projection aligned
across validators. Do not restore retired `sumeragi.advanced.pacemaker`,
`sumeragi.advanced.npos.timeouts`, or debug fault tables.

## Test control

Revision-4 fault coverage injects authenticated consensus messages at the
runner boundary or exercises real process/network outages. Local configuration
cannot forge a timeout certificate, lower a quorum, disable DA, or synthesize
Byzantine messages.

The canonical derivation is implemented by `sumeragi_v2_timing_ms` in
`crates/iroha_config/src/parameters/actual.rs`; the linear view deadline and
clock-ownership rules are implemented in
`crates/iroha_core/src/sumeragi/v2_runtime.rs`; autonomous lanes call that same
helper rather than maintaining a second timer formula.
