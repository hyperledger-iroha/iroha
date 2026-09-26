# F03 finalized closed Native control — 2026-09-24

Scope: `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`.

The existing four-validator `native_driver_transfers_exact_diagnostic_and_closed_custody`
fixture now retains a signed timeout control in original fair-ingress custody
before the State/Kura-finalized admission obligation is resolved. After the
original Native driver drains physical handles into closed custody, the old
observation returns the exact control as a local Retry. The current empty
opening set rejects that pre-closure fair-ingress occurrence through
`NativeLaneDriver::admit_owned`, returning its same signed payload and
validated ownership evidence. A subsequent driver poll cannot reopen the
closed instance or alter its WAL records and held effects. The original closed
owner still transfers once; dropping it without
acknowledgement closes consensus output, as before.

This is a closed-instance stale-artifact boundary, not a recreation test. It
does not prove a second finalized admission, new incarnation, archive, Queue
drain frontier, restart, or economic Apply. Those F03 lifecycle gates remain
open. The settled-source Core lib test binary
`target/debug/deps/iroha_core-1b859b12063a70cf` passed the focused
`native_driver_transfers_exact_diagnostic_and_closed_custody` selector 1/1.
