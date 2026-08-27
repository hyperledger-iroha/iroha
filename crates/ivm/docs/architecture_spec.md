# Iroha Virtual Machine Architecture

IVM is the deterministic 64-bit virtual machine used by Iroha 3 smart contracts. Kotodama compiles
to IVM bytecode (.to); RISC-V-like field layouts are implementation details and do not define a
separate target architecture.

This document describes the first-release implementation. ABI version 1 is the only accepted ABI.

## Program format

- Every executable instruction is one naturally aligned 32-bit little-endian word.
- Program loading validates metadata, instruction boundaries, opcodes, indexed literals, syscall
  policy, and the terminating HALT before execution.
- Compact 16-bit and mixed-width instruction streams are rejected.
- The only mode bits are ZK and VECTOR. Unknown bits are invalid metadata.
- Prepared programs cache validated decode results. The execution loop consumes the canonical
  prepared representation directly; there is no compatibility decoder or alternate interpreter.

## Registers and memory

IVM has 256 64-bit registers. r0 is hardwired to zero. In ZK mode each register also carries a
privacy tag, and instructions either propagate compatible tags or trap before a private value can
affect a public control-flow or host boundary.

Memory is byte-addressable and divided into code, heap, input, output, and stack regions with
explicit permissions. Multi-byte guest accesses must be naturally aligned and fully in bounds.
Private guest spills are stack-only. Their byte ranges are tracked so partial public overwrites
cannot silently declassify a value; reset, program replacement, and leaving ZK mode scrub private
bytes.

The register Merkle tree is rebuilt lazily after writes. A burst of register or tag updates marks
the tree dirty without hashing on every write; the first root or path request rebuilds one
canonical tree, and subsequent reads reuse it until the next mutation.

## Execution and gas

The contract interpreter is a single sequential fetch-decode-execute loop. This keeps trap
precedence, gas exhaustion, privacy checks, and proof traces in exact program order.

Each opcode has one ABI-v1 gas rule. Vector costs scale with the declared logical vector length,
not the host SIMD width. Cryptographic and staged syscall work is charged before bounded parsing or
expensive host work. Hardware selection and block scheduling are never consensus-metered.

max_cycles bounds execution and determines ZK trace padding when enabled. Branches always charge
their fixed instruction and cycle costs; execution history and predictor state cannot affect the
result.

## Zero-knowledge mode

ZK mode enables privacy tags, private-memory range tracking, constraints, and deterministic trace
padding. Secret-dependent public branches, addresses, traps, and host arguments are rejected.
Execution proofs commit to canonical register and memory Merkle roots. Proof generation does not
introduce a second instruction semantics.

## Vector and cryptographic acceleration

The VECTOR mode enables logical vector opcodes. The logical lane count is part of program
metadata and is capped by ABI policy. Scalar, SIMD, Metal, and CUDA helpers must return byte-for-byte
identical outputs. Runtime feature detection selects only a throughput implementation; a
deterministic scalar fallback remains authoritative.

Optional acceleration can fail closed or fall back without changing opcodes, gas, traps, register
values, memory, or proofs. Qualification tests compare available accelerated paths with the scalar
implementation.

## Parallel block execution

One contract remains sequential. At block scope, transactions with nonconflicting declared access
sets may run in isolated contexts on a lazily created worker pool. Workers return buffered writes
without publishing them. The coordinator applies successful batches in original block order
through one safe-Rust RwLock state path. There is no hardware transactional-memory path.

Thread count, completion order, and CPU capabilities affect throughput only. Declared access-set
conflicts add dependency edges that reduce concurrency without changing observable state. See
[parallel_execution.md](parallel_execution.md).

## Host boundary and ABI

SCALL delegates to IVMHost under the ABI-v1 syscall policy. Unknown or disallowed syscall
numbers trap as VMError::UnknownSyscall. Pointer-ABI objects are validated for type, version,
length, checksum, ownership, and privacy before use.

Contract admission checks the code and ABI hashes embedded in manifests. Since this is the first
release, obsolete encodings and speculative compatibility surfaces are removed instead of retained
as alternate execution paths.

## Safety invariants

- No consensus-visible behavior depends on wall-clock time, thread scheduling, prior VM history,
  or optional hardware.
- Failed transactions expose no partial state batch.
- Invalid addresses, alignments, metadata, opcodes, syscalls, and pointer objects trap explicitly.
- Private data cannot cross a public boundary without an allowed commitment or reveal operation.
- Runtime state mutation uses safe Rust; acceleration-specific unsafe code is isolated behind
  parity-tested helpers and deterministic fallbacks.
