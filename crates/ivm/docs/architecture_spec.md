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
- Two engines share this format, each selected by an authenticated profile
  ([profiles record](../../../specs/runtime/ivm_baseline_and_c10_profiles.md)). ABI V1 as every root
  selects it runs the frozen baseline engine: a 49-byte header whose mode bits are ZK and VECTOR
  (unknown bits are invalid metadata) and whose positive `max_cycles` is the invocation's cycle
  budget; 256 registers; 8-bit `SCALL` plus the 24-bit `SYSTEM`/`SCALLX` form; 90 opcodes including
  vector, cryptographic and zero-knowledge ones ([opcodes](opcodes.md)).
- The C10 profile (the runtime lifecycle profile now, its own ABI V1 addition in phase 5) has a
  48-byte header whose only mode bit is ZK and whose `max_cycles` field is the ZK trace bound,
  nonzero exactly when the ZK bit is set; gas, supplied by the caller, is its only execution budget.
  It has 32 registers; five-bit register fields; signed 14-bit immediates, displacements and branch
  offsets; signed 19-bit `JAL` and 24-bit `JMP`/`JALS` word offsets; one 24-bit `SCALL`; reserved
  operand bits must be zero. It admits 58 opcodes (scalar arithmetic, 64-bit memory, typed
  literals, control flow and host calls); hashing and signature verification are syscalls.
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

The register Merkle tree is rebuilt lazily after writes, resets and runtime-template restores. A
burst of register or tag updates marks the tree dirty without hashing on every write; the first
root or path request rebuilds one canonical tree, and subsequent reads reuse it until the next
mutation.

## Preparation and runtime retention

`ivm_cache_max_bytes` supplies one aggregate retention budget (64 MiB by default);
zero disables retention. Immutable artifact images, decoded/prepared operations,
metadata, literal indexes and analyses carry a reservation in their allocation
owner. Dropping a cache entry does not refund another VM's live borrower. Native
collection capacities and conservative B-tree node bounds account for cache
indexes; index storage is physically freed when retention is denied.

Contract and user-provided executor runtime leases restore their baseline on
return, error and unwind. Pooled memory
images, register/memory Merkle arrays and call-frame storage carry independent
owners. Checkout moves uniquely owned runtime storage into active accounting;
shared immutable owners keep their retention charge. Admission never waits for
an executing borrower, and a denied cache admission executes cold with identical
validation, gas and results. Reducing the budget clears idle Core/global cache
references while outstanding borrowers remain charged until destruction.

Each memory image owns two fixed bitmaps with one bit per 32-byte Merkle leaf:
leaves whose digests are pending and changes since the runtime baseline. Until
the first root or path request allocates the Merkle storage, the pending bitmap
holds every leaf written since the zero image. Constructor and
snapshot plans reserve the complete bitmap geometry from the original execution
lease; local images use the aggregate cache accounting owner. Stores and warm
reset change initialized words without growing tracking sets. Bitmap backing
keeps its charge through retention and final borrower release. Memory commits
stream the original set bits into fixed leaves and update cached paths in
ascending order without copied index or digest vectors. Existing full-rebuild
and hardware selection still apply for large updates.

`CacheLimitsGuard`, the test-support owner of the cache configuration, also gives
its holder a private budget. While it lives, allocations made on the holding
thread are charged to a budget of their own with the installed limit, and
`memory_stats()` on that thread reports that budget. Every allocation keeps the
budget it was charged to, so what other threads retain cannot refuse the
holder retention. A holder that runs cache work on threads it spawns enters
`CacheLimitsGuard::budget_share()` on each of them. Production threads hold no
guard and share the aggregate budget.

`cache_memory::memory_stats()` exposes retained/active bytes, their measured
resident total, the peak measured reservation, and the number of active owners
with an unmeasured dynamic footprint. Metadata which cannot be
normalized into a measured Norito owner remains valid and uncacheable. Embedded
trigger descriptors currently remain uncacheable because independently cloned
JSON backing can outlive their aggregate metadata owner.
Cache-control blocks, shard arrays and eviction-registry capacity remain active
infrastructure when retention is disabled. TODO: Complete active scratch
ownership/admission and borrowed/evicted observability before treating these
counters as a complete process-memory bound. Independently shared trigger metadata
still needs its own allocation reservation.

Fallible physical call-frame allocation returns a typed local execution deferral,
separate from deterministic guest bounds faults. Core preserves the reason across
nested calls, executor validation and query validation; the outer output owner
abandons the attempt before retaining gas, fees, rejection rows or trigger
completion. Original capacity-refusal observations propagate through Core to the
Sumeragi retry owner without creating a durable ledger rejection. A reason-only
refusal requires local recovery. TODO: complete production root/nested allocation
funding, configuration-reload pool custody and notification-safe cache release.

## Execution and gas

The contract interpreter is a single sequential fetch-decode-execute loop. This keeps trap
precedence, gas exhaustion, privacy checks, and proof traces in exact program order.

Each opcode has one gas rule in its engine's schedule ([gas model](../../../specs/gas_model.md)).
Cryptographic and staged syscall work is charged before bounded parsing or expensive host work.
Hardware selection and block scheduling are never consensus-metered.

Under the baseline engine (ABI V1) the header `max_cycles` is required, bounded by the available
fuel and the committed ceiling, and converted into the invocation's gas limit; vector costs scale
with the declared logical vector length, and nested calls share the parent's cycle allowance.
Under the C10 profile gas is the only execution budget: the caller supplies it (transaction limit
within the remaining block budget) and a nested invocation draws from its parent's remaining gas;
no header field converts into gas, only the terminating HALT is free, and the header `max_cycles`
field is the ZK trace bound only: a ZK run traps `ExceededMaxCycles` when its cycles reach it and
pads a successful trace to exactly that length (one gas per padded cycle). Ordinary C10 execution
has no cycle bound. Branches always charge their fixed instruction cost;
execution history and predictor state cannot affect the result.

## Zero-knowledge mode

ZK mode enables privacy tags, private-memory range tracking and deterministic
trace padding. Secret-dependent public branches, addresses, traps and host
arguments are rejected. Register/memory Merkle logs and diagnostic traces do not
constitute a complete execution proof. `IvmProved` admission is closed; private
invocation requires the complete native STARK relation and State-owned finalized
full-state commitments. Those
release gates remain open in the [completion record](../../../specs/kotodama_ivm_completion.md).

## Hardware acceleration

The baseline engine's VECTOR mode enables logical vector opcodes, whose lane count is part of
program metadata and capped by ABI policy; the C10 instruction set has no vector or cryptographic
opcodes. Scalar, SIMD, Metal and CUDA implementations, whether behind opcodes or host-side helpers
(Merkle hashing, syscall primitives, proof tooling), must return byte-for-byte identical outputs. Runtime feature detection selects only a
throughput implementation; a deterministic scalar fallback remains authoritative.

Optional acceleration can fail closed or fall back without changing opcodes, gas, traps, register
values, memory, or proofs. Qualification tests compare available accelerated paths with the scalar
implementation.

## Transaction concurrency

Each contract executes sequentially and IVM has no block scheduler. Hosts own transaction-level
concurrency: they pass declared access sets through `IVMHost::begin_tx`, receive observed accesses
from `IVMHost::finish_tx`, and commit results in canonical block order. There is no hardware
transactional-memory path.

Thread count, completion order, and CPU capabilities affect throughput only. See
[parallel_execution.md](parallel_execution.md).

## Host boundary and ABI

All compiled functions use caller-owned argument/result tables: `r10/r11` carry
argument address/count, `r12/r13` result address/capacity, and returns carry result
address/exact initialized count in `r10/r11`. Each table is aligned and bounded
to 8,192 words (64 KiB). Mandatory callable descriptors bind table roles and
frame bounds; the runtime enforces initialization, ownership and storage lifetime.
See [calling convention](calling_convention.md) for the canonical table layout.

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

### Write-log allocation custody

Memory records writes through private row and payload owners. Row growth and payload
copying are reserved before guest bytes or output cursors change. Each payload is
scrubbed on final drop, and clearing the log retains the charged row capacity.
`Memory::try_write_log_snapshot` returns an independently owned immutable snapshot;
it holds no lock across later stores. The raw `Vec<WriteLogEntry>` snapshot and
mutable public byte-vector escape are removed. Entries expose `address()` and
`bytes()`; snapshots are explicitly fallible to clone.

State-owned logs reserve each append's complete replacement-row and payload
layouts from their original finite execution pool before physical allocation or
guest mutation. A replacement keeps the old row backing charged until after all
entries move into the completed new backing; unchanged payload owners move with
their entries. The old allocation is destroyed before its original charge is
refunded. Capacity and physical allocation failures leave guest bytes and the
existing write history unchanged and use the existing local execution-deferral
path. Cache admission cannot supply execution credit.

Detached snapshots are explicit independent copies: they reserve the complete
exact row and payload demand from that same original pool. Their fallible clones
retain this rule after the source VM disappears. Runtime-template copies instead
partition the existing parent reservation after checking exact pool identity;
their actual row/payload owners hold those split charges without retaining a
duplicate aggregate charge for the same bytes. Each allocation also retains the
existing aggregate retention-accounting owner. A shared outer borrower keeps all
charges until final backing destruction; shrinking either budget does not refund
live storage. Standalone local Memory uses its existing retention owners and
never becomes a fallback for a refused State-owned allocation.

Full worker snapshot admission, hardware scratch and complete nested-execution
admission remain open execution-memory gates. The read/write-log subsets of
worker copies are funded from the original pool even when the worker's other
private copies still use local accounting. No gas formula changes;
Core preserves resource deferrals as operational outcomes before rejected-work
fee accounting. The direct VM can contain attempted work when an opcode defers,
so callers must follow the existing attempt rollback/retry boundary rather than
resume it as though the failed attempt were a successful instruction.

### Read-log allocation custody

Reads use a specific row owner. State-owned Memory reserves the complete next
row array from its original finite execution pool before allocation, diagnostic
publication or caller output. The old array stays charged while its initialized
ranges are copied to the replacement. Diagnostic refusal discards that replacement
and leaves the old rows, capacity and output unchanged. Successful publication
installs the complete next history before destroying the old backing. Appends
within existing capacity and clear/reset retain the same prepaid array.

`Memory::try_read_log_snapshot` replaces the raw-vector read snapshot directly.
Its immutable result owns an independent exact copy and exposes only a borrowed
range slice. Snapshot creation and explicit fallible cloning reserve from the
same original pool; template copying partitions the original prepaid parent
lease after an identity check. Each backing keeps both execution and retention
charges until actual deallocation. Shared outer borrowers add no duplicate row
charge, and budget shrink or eviction cannot refund live backing.

Memory defers allocation-release callbacks until the read-log mutex has released,
including diagnostic refusal and unwind. Invalid address/alignment errors still
precede row admission. Ordinary guest loads, unprepared instruction fetches and
trusted call-result reads retain their existing tracking rules; quote-only
inspection and prepared diagnostic fetches remain separate. Allocation refusal
uses the existing operational deferral and attempt rollback boundary, without
changing gas schedules or transaction validity. TLV/numeric decoders and signature
preflight/verification retain the complete operational error instead of reporting
malformed input or a false verification decision. INPUT cursor reconstruction
performs its tracked scan fallibly and publishes the valid-prefix cursor only
after the scan completes; admission failure cannot silently truncate that prefix.
Kotodama test invocation propagates operational failures before rejection matching,
and the outer test runner reports an execution failure rather than a contract
rejection. Deterministic semantic decoder errors retain their existing mappings.

Private-memory write cleanup still requires a separate admission/error-propagation
cut: byte-wise scrub can exhaust write-row credit, and its existing boolean result
is erased by reset or mapped to PrivacyViolation by mode/load transitions. This
read-owner change does not close that gate or the remaining active-memory owners.

## Fixed canonical memory Merkle nodes

Memory construction and runtime-template copying prepay the complete canonical
SHA-256 node geometry alongside the memory image, leaf digests and dirty bitmaps.
Only the image and the bitmaps are allocated then. Leaf digests and canonical
nodes cost about three times the image, and ordinary execution never reads a
memory root, so they are allocated by the first root or path request
(`Memory::root`, `merkle_path`, `merkle_root_and_path`, `merkle_compact`,
`merkle_commitment`). ZK trace collection, which records a root every cycle,
makes that request when the invocation begins, before any guest effect.

Construction charges the image and its dirty bitmaps. The first root or path request
admits the complete leaf and node storage from the image's original execution pool.
A deferred image and a copy of one hold no cache retention or pool credit for that tree.
A copy of an allocated tree includes its backing in the copy plan.

A pool or allocator refusal leaves the tree absent and the pending bitmap intact.
Dropping a partially built tree refunds its backing to the original pool. A later
request can retry after capacity returns. If a syscall already consumed gas before
the refusal, the caller abandons that execution attempt and replays it from its
original state.

The checked Crypto geometry includes its exact retained node capacity; the actual
allocation must match that request before the owner is published. Funded nodes
carry a split charge from the original execution lease. Both charges remain until
the canonical tree's node array has been destroyed. The allocation is the tree of
a zeroed image: its leaves are identical, so it computes one pair hash per tree
level and fills each level with the bit-identical canonical nodes. The pending
bitmap is then hashed through the ordinary commit paths, so a root is the same
function of the image bytes whenever it is first requested.

Every byte tree retains this node storage. Sparse updates use canonical leaf-to-root
updates; dense CPU or successful hardware leaf replacement marks the existing
nodes stale. Root and proof queries rewrite their retained breadth-first slots
using Crypto's existing SHA-256 relation. Warm resets keep node storage even when
the known template root lets execution defer rewriting it, and never allocate or
release it: an allocated worker restored from an allocated template copies the
template's digests for its restored and still-pending leaves; two deferred images
copy the template's pending bitmap; a worker whose allocation state differs from
its template keeps its own pending leaves and adds the restored ones. No query
allocates a replacement canonical node array or queue. Proof output vectors, hardware block
and digest scratch, telemetry/Rayon bookkeeping and worker snapshot resources are
separate from this canonical-node backing guarantee.

`ivm_merkle_rebuild_total` counts completed in-place canonical node refreshes once
at the refresh owner. It excludes initial construction and independent template
copies. Dense leaf recomputation is classified by the memory-commit path metrics
and does not separately increment this counter. Incremental path updates retain
their separate leaf-update counter. IVM owns these Merkle and memory-commit
metrics in `ivm::metrics`, independent of the node telemetry crate; Core
registers `ivm::metrics::collector()` with the node registry when telemetry
starts. Byte trees have no infallible `Clone` wrapper; internal copies
explicitly choose the fallible local or original-lease path.
