# Refund notification custody

Work remains in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The previous checkpoint attached original cursor/reader charges to actual
allocations. This checkpoint handles reclamation during a larger mutation and
preserves pending notifications across callback unwind. It does not establish
complete State funding or activate the live retained validator.

## Reproduced lost wakeup

The release notifier advanced its sequence and removed the original waiter
cohort before invoking callbacks. If the first `Waker::wake` panicked, Rust
dropped the remaining iterator without notifying its registrations. Their
futures were ready if polled, but their executors received no wake. A caught
callback failure could therefore strand a surviving waiter until some unrelated
event caused another poll, even though the original capacity had been freed.

The regression first failed against the original implementation: survivor wake
count was zero instead of one. The correction retains that same original
iterator in a lexical owner. Normal execution drains it; unwind drains the
unvisited remainder. The failed callback is not replayed, no registration is
reconstructed, and all callbacks run after both notifier and registration locks
are released. The original panic propagates. A second independent callback
panic during unwind has ordinary Rust double-panic semantics.

## Reclamation while a writer is held

Retired readers and future charged node/vector buffers can be destroyed while
the same thread holds a newer writer. Returning an allocation's credits must
not synchronously reenter that writer before it unlocks. Delaying physical
reclamation or retaining all freed charges until publication is unnecessary.

`AllocationBudget::with_deferred_refund_notifications` instead returns freed
credits immediately and defers only the original pool's same-thread wake.
Borrowed stack records form a private thread-local chain, retaining the exact
pool identity and one pending bit. Scope exit removes its record before invoking
any callback. Nested scopes coalesce into the nearest matching outer scope;
different pools and other threads continue notifying independently. Entering,
recording and leaving scopes needs no allocation or global suppression.

Every physical guard must be acquired and released inside the synchronous
closure. The primitive exposes no movable scope guard, but cannot itself forbid
an arbitrary return value from retaining a physical lock. The closed funded
map-operation API must enforce that restriction and declare all nested pools.
Returning an unpolled future does not run its later work inside the scope.

## Evidence and remaining work

The failed original loop is preserved in
`dist/sumeragi-main-work/generation114/lost-cohort-before.log`, with the exact
old notifier source alongside it. New controls exercise callback-body and
consumed-waker destructor panic, surviving original registrations, unlocked
reentry, no stale scope after panic, nested/different pools, immediate credit
reuse, cross-thread progress, and actual retained-reader reclamation under a
newer writer. The external allocator regression observes `System.dealloc`,
original charge destruction and immediate credit return before the delayed wake.
The final source/executable join is `dist/sumeragi-main-work/validation114.json`;
separate source epochs and failed attempts are not combined into a passing run.

The map audit confirms that generic `Clone` and unrestricted `get_mut`, `Extend`,
lazy mutable iteration and removal buffers cannot silently become completely
prepaid operations. The next concrete boundary is a closed insert/update against
the original locked root, with exact cache-padded node layouts, bounded original
path/split allocations, fixed-capacity tracking buffers, and an explicit nested
payload policy. Original retirement buffers and charges must move to the reader
generation until actual free. Initial root/control funding and MV undo storage
also remain required. Full production admission, retained Validate/Apply and
retirement integration, participant durability, recovery, and unchanged real
four/seven-validator qualification remain open. No liveness goal is closed.
