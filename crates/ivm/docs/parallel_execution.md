# Deterministic Parallel Block Execution

IVM executes each contract's instruction stream sequentially. The block scheduler may execute
independent transactions concurrently, but consensus-visible results always follow block order.

## State access sets

Each transaction carries the state keys and register tags it may read or write. The dependency
graph places an edge between transactions whose declared accesses conflict. Missing or
conservative access information reduces concurrency; it never changes transaction semantics.

## Scheduling

The scheduler releases transactions only after their graph dependencies complete. Independent
transactions run in isolated ExecutionContext values on a lazily created Rayon pool. Ordinary
single-contract execution does not create scheduler threads.

The pool size may adapt within configured bounds. This affects throughput only. Transaction
results, gas, and state updates do not depend on thread count, completion timing, or host CPU
features.

## Commit

Workers return buffered StateUpdate values instead of mutating shared state. The coordinator holds
completed outputs until every lower transaction index has completed, then publishes each successful
batch in original block order. State::apply sorts each batch by key and holds one RwLock write guard
for the complete batch, so readers cannot observe a partial commit.

Failed transactions publish no writes. Dependency edges serialize transactions whose declared
access sets conflict; independent transactions may finish in any order, but publication remains in
canonical block order.

## Determinism and gas

Every transaction owns its gas meter and runs the same sequential interpreter used outside the
block scheduler. Scheduling work is not consensus-metered, and hardware capabilities cannot alter
the gas exhaustion point. Tests compare parallel block execution with sequential execution and
exercise conflict ordering, atomic publication, and dynamic pool limits.
