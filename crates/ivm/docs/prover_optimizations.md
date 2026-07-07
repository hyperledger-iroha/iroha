# Proving System Optimizations

This document outlines internal optimizations used by IVM's Halo2 circuit
gadgets and trace-verification helpers. Runtime consensus paths verify
OpenVerify IPA/Pasta envelopes; these optimizations apply to local circuit tests
and off-chain proving flows.

## Custom Gates

Certain arithmetic operations are grouped into dedicated circuits to minimise
constraint usage.  A notable example is `AddCarryCircuit` which verifies a
32-bit addition with an incoming carry and outputs the carry bit in a single
check. The Halo2 gadget expresses this as a compact custom-gate shape so the
operation avoids separate range checks for each bit.

## Parallel Verification

`zk::verify_trace` builds a global rayon thread pool to evaluate constraints in
parallel.  The thread count can be adjusted with the `IVM_PROVER_THREADS`
environment variable which helps improve proving throughput on multi‑core
machines.

## Compact Trace Representation

For long execution traces the memory overhead of recording the full register set
at every cycle can be significant. `DeltaTraceLog` stores only the registers
that changed between cycles and reconstructs the full trace on demand. The VM's
`run` method automatically switches to this compact format whenever
zero-knowledge padding is enabled. This avoids redundant data while keeping the
current APIs stable.

## Incremental Merkle Proofs

Zero‑knowledge mode now records Merkle authentication paths for every register
and memory access.  `MemEvent` and the new `RegEvent` include the path and the
resulting root so the prover can verify inclusion without recomputing the entire
tree after each operation.  During execution the VM updates the internal Merkle
trees incrementally and stores proofs as operations occur.  In addition a
`StepLog` records the register and memory Merkle roots after every cycle so an
external circuit can follow the execution without materialising the full state.
`verify_trace` checks these proofs in parallel alongside regular constraints.
