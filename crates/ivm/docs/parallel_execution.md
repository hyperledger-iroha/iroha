# Transaction Concurrency and Declared Access Sets

IVM executes each contract's instruction stream sequentially and has no in-VM block scheduler.
Transaction-level concurrency belongs to the host: the node pipeline in `iroha_core` derives
deterministic access sets, orders conflicting transactions, and commits results in canonical block
order.

## State access sets

`ivm::parallel::StateAccessSet` carries the state keys and register tags a transaction may read or
write. Hosts receive the declared set through `IVMHost::begin_tx` and return the observed accesses
from `IVMHost::finish_tx` when `IVMHost::access_logging_supported` is true. Callers that execute a
transaction atomically take `IVMHost::checkpoint` first and restore it when execution fails or the
observed accesses exceed the declaration, so a failed transaction leaves no host side effects.

Missing or conservative access information reduces host concurrency; it never changes transaction
semantics.

## Thread limits

`ivm::set_scheduler_thread_limits` records the operator's `concurrency.scheduler_min_threads` and
`concurrency.scheduler_max_threads` bounds (0 means the physical core count), which VM construction
reports in the startup banner. `ivm::init_global_rayon` and `ivm::apply_stack_sizes` size the host
Rayon pool and its worker stacks. These settings affect throughput only.

## Determinism and gas

Every transaction owns its gas meter and runs the same sequential interpreter. Host scheduling work
is not consensus-metered, and thread count, completion timing, or CPU features cannot alter
transaction results, gas, or state updates.
