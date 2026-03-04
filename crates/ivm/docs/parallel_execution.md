# Implementation Specification: Deterministic Parallel Execution for Iroha VM (IVM)

This document describes how IVM can run instructions and transactions in parallel while preserving deterministic results across all nodes. The reference implementation currently provides transaction-level parallelism and an experimental instruction-level scheduler.

## Goals

- **Instruction-Level Parallelism (ILP)**: run independent instructions from a single contract concurrently.
- **Transaction-Level Parallelism (TLP)**: execute multiple transactions in parallel when their state access sets do not overlap.
- **Deterministic results**: the observable state and output must match a single canonical sequential order.

The design targets a Rust implementation using thread pools and careful ordering of commits.

## Instruction-Level Parallelism

1. **Dependency Graph**
   - For each instruction we record the registers and memory locations it reads and writes.
   - Edges are added between instructions whenever one depends on results of another or writes the same location.

2. **Scheduling**
   - Instructions whose dependencies are satisfied are placed in a ready set.
   - Each cycle we take ready instructions in program order up to a window size and ensure their combined gas cost fits within the remaining gas budget.
   - These instructions are dispatched to worker threads for execution. They operate on immutable snapshots and write results to a thread-safe buffer.

3. **Commit**
   - Once all threads for the cycle finish, their results are committed to the VM state **in program order**. Gas is deducted during this step.
   - Any instruction that fails (e.g., out-of-gas) aborts the transaction; later instructions from the same cycle are discarded.

4. **Memory Safety**
   - Worker threads never modify shared state directly. They return their intended writes which are applied by the coordinator thread during commit.
   - This avoids race conditions and ensures Rust’s ownership rules are upheld.

## Transaction-Level Parallelism

1. **State Access Sets**
   - Every transaction is associated with the set of state keys it reads and writes. Sets can be declared or derived by pre-execution analysis.

2. **Grouping Algorithm**
   - Transactions are processed in the order they appear in the block.
   - We create groups such that transactions in the same group have disjoint write sets and do not read keys written by another transaction in that group.
   - Each group is executed in parallel while groups themselves are processed sequentially.
   - A conflict prediction stage may further split groups when heuristics indicate two transactions will likely touch the same keys. This reduces costly rollbacks when hardware transactions abort.

3. **Execution and Commit**
   - Each transaction runs in its own IVM instance. The underlying world state is read-only during execution and each transaction accumulates a write set.
   - After all transactions in a group finish, their results are committed sequentially in block order. Because their state access sets do not overlap, the commit order does not affect the final state.

4. **Error Handling**
   - A failing transaction does not roll back other transactions in its group; its write set is simply discarded.
   - If dependency analysis was incorrect and a conflict is detected at commit time, the executor falls back to sequential execution for that block.

## Gas Accounting

- Gas is deducted based on the sum of the instruction costs in each parallel cycle before executing them.
- This guarantees that the gas exhaustion point is identical to a sequential run.
- Each transaction has its own gas meter; parallelism never allows exceeding individual limits.

## ZK Mode

- In zero-knowledge proving mode a single cycle is represented as one transition in the proof trace containing all its instructions.
- The circuit enforces that instructions in the same cycle are conflict-free and that their combined gas cost matches the gas update.

## Threading Model

- A fixed-size thread pool (e.g., from the `rayon` crate) executes instruction tasks. By
  default this pool uses all **physical** CPU cores so parallelism is transparent and
  automatic.
- The scheduler and commit logic run on a coordinator thread which holds mutable access to the VM state.
- Global state writes from transactions are applied under a mutex or using per-key locks.
- When the `htm` feature is enabled and the CPU supports Intel RTM, the scheduler
  first attempts to commit transaction results inside a hardware transaction.
  If the transaction aborts because of contention or the CPU lacks HTM support,
  the scheduler immediately retries under a mutex to guarantee progress.
- Register-tag conflicts are checked both when building dependency graphs and
  at runtime so that conflicting transactions serialize deterministically
  regardless of the execution path.
- Each transaction may carry **register tags** which are included in its access
  set. These tags are compared when building dependency graphs so that conflicts
  are resolved deterministically based on tag ordering.

## Conclusion

By combining ILP and TLP with careful scheduling and ordered commits, IVM can utilise multiple CPU cores without breaking determinism. The architecture keeps concurrency concerns within the runtime, so smart contracts remain single-threaded from the developer’s perspective.
