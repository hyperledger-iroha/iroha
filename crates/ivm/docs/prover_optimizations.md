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

## Parallel Diagnostic Checks

`zk::check_diagnostic_trace` uses the prover rayon pool to evaluate recorded
constraints and register authentication paths in parallel. The helper is not a
proof verifier: it does not check VM transition semantics, trace completeness,
or memory-event membership. The thread count can be adjusted with the
`IVM_PROVER_THREADS` environment variable for local proving diagnostics.

## Compact Trace Representation

For long execution traces the memory overhead of recording the full register set
at every cycle can be significant. `DeltaTraceLog` stores only the registers
that changed between cycles and reconstructs the full trace on demand. The VM's
`run` method automatically switches to this compact format whenever
zero-knowledge padding is enabled. This avoids redundant data while keeping the
current APIs stable.

## Incremental Merkle Diagnostics

Zero‑knowledge trace collection records Merkle authentication paths for register
accesses. `RegEvent` includes the complete register leaf, path, and fixed-size
register-tree root, so `check_diagnostic_trace` can authenticate it. `MemEvent`
records only the accessed value plus a path/root snapshot; because it lacks the
complete 32-byte leaf and an authenticated leaf count, it is diagnostic material
and not an independently verifiable membership proof. `StepLog` records register
and memory roots for downstream off-chain tooling without claiming to prove the
execution.
