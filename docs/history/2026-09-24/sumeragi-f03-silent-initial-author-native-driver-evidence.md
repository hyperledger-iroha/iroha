# F03 silent initial author Native driver evidence — 2026-09-24

Scope: the `optimizations` checkout at `/Users/takemiyamakoto/devstuff/iroha`.
The existing `native_driver_silent_initial_author_reaches_real_decision_without_global_view_input`
test already uses three production `NativeLaneDriver` instances for the surviving
members of one frozen four-validator committee. The initial author has no
driver, message, or injected payload. The fixture uses actual State/Kura, native
WAL and worker custody, the shared reducer, and authenticated Native message
admission. It does not use the retired autonomous lane adapter.

The test now checks that each survivor has a timeout deadline before any
payload, WAL record, or outbound proposal; that each sends a signed view-zero
timeout with no highest Prepare; and that the replacement author's view-one
proposal carries a three-survivor timeout certificate and the frozen signed
RS16 layout. It also checks the final Decision retains that exact manifest and
the Commit quorum excludes the offline author. The test continues to hold the
original unapplied economic effect through Native retirement turns.

The first focused Core run compiled but failed at a newly added assertion that
`NativeLaneDriver::next_deadline()` equaled the one-second timeout. That API
also exposes the earlier 100-millisecond retransmission wake. The assertion
was corrected to inspect each `LaneInstance::timeout_deadline()` directly. A
second run showed that the source fixture also has two other Native lane
contexts, whose packets were already routed by the test. The new assertions
were narrowed to the target instance while preserving every packet's routing.
The settled-source selector then passed 1/1:
`scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p
iroha_core --lib native_driver_silent_initial_author_reaches_real_decision_without_global_view_input
-- --nocapture`.

This is a driver-level four-validator test, not a four-process P2P outage or
restart/lifecycle qualification. F03 remains open for the first-release
production hard cut, stale-artifact and missing-execution-evidence recovery,
Apply/restart proof, larger deterministic fault matrix, and drain/archive/
recreation evidence.
