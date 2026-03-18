# ivm
Iroha Virtual Machine (IVM)

ivm/                         # → Cargo workspace root (a single Rust library crate)
├── Cargo.toml               # crate manifest (name = "ivm", edition = 2024, deps: sha2, halo2curves, etc.)
├── README.md                # architecture overview, build & usage instructions
├── benches/                 # Criterion performance benchmarks
│   └── bench_vm.rs
├── examples/                # small runnable demos
│   ├── add.rs               # “hello‑world” ADD / HALT program
│   └── sha256.rs            # demo using the SHA256BLOCK instruction
│
│   Tuple return demo
│   - Example: `koto_tuple_return_demo.rs` compiles an inline Kotodama function
│     that returns a tuple and prints r10/r11.
│   - Run: `cargo run -p ivm --example koto_tuple_return_demo`
│   - Related source sample: `crates/kotodama_lang/src/samples/tuple_return_demo.ko` shows
│     tuple creation and destructuring with `.0`/`.1`.
├── src/
│   ├── lib.rs               # public re‑exports and crate‑level docs
│   ├── memory.rs            # region‑based memory manager (perm checks, loads/stores)
│   ├── registers.rs         # GPR & vector‑register structs + helpers
│   ├── gas.rs               # gas table and accounting utilities
│   ├── error.rs             # `VMError`, `Perm` and other enums
│   ├── instruction.rs       # opcode constants, field extractors, immed‑helpers
│   ├── decoder.rs           # 16‑/32‑bit instruction decoding logic
│   ├── host.rs              # `IVMHost` trait + default/dummy host impls
│   ├── syscalls.rs          # Iroha‑specific syscall number map & helpers
│   ├── vector.rs            # SIMD helpers, SHA‑256 compression adapter
│   ├── zk.rs                # zero‑knowledge‑mode helpers (ASSERT tracking, padding)
│   ├── kotodama/            # KOTODAMA language compiler placeholder
│   └── vm.rs                # `IVM` struct, fetch‑decode‑execute loop, public API
├── tests/                   # `cargo test` integration + property tests
│   ├── arithmetic.rs        # ADD/SUB/MUL/… correctness
│   ├── memory.rs            # loads/stores, OOB & alignment traps
│   ├── control_flow.rs      # branches, jumps, JAL/JR, HALT behaviour
│   ├── syscalls.rs          # host interaction and error propagation
│   └── zk_mode.rs           # ASSERT, padding to MAXCYCLES, constraint‑failure paths
└── .github/
    └── workflows/ci.yml     # continuous‑integration script (fmt, clippy, tests)

# IVM – Iroha Virtual Machine

**IVM** is a Rust library implementing the Iroha VM for executing Iroha smart contract bytecode. It implements the IVM instruction set (not RISC‑V) with a canonical 32‑bit wide instruction format, gas metering, and features for cryptography and zero‑knowledge proofs. Earlier drafts shared bit layouts with RISC‑V, but only the wide IVM encoding is supported in the first release.

Note on Kotodama bytecode target: Kotodama smart contracts compile to IVM bytecode (`.to`) for execution by this virtual machine. They do not target “risc5”/RISC‑V as a standalone ISA. Earlier RISC‑V‑like encodings (e.g., 0x33/0x13 formats) are rejected by the VM loader and interpreter; they now exist only in regression tests that prove the trap path. Kotodama and the reference tooling emit IVM’s native wide helpers exclusively. Observable behavior and outputs are defined by IVM, not raw RISC‑V.
For the latest architecture ideas including deterministic parallel execution and zero-knowledge capabilities, see [docs/architecture_spec.md](docs/architecture_spec.md).

## Status and TODOs

- Implemented:
  - 256 general-purpose registers with privacy tags and Merkle commitments (current implementation; the encoding leaves headroom for future expansion).
  - Segmented memory with permissions and alignment checks; bulk copy helpers.
  - Gas metering; `GETGAS` and per-op cost accounting.
  - Syscall/host trait with a default host implementation.
  - Vector helpers with runtime SIMD detection (SSE/AVX/NEON) and scalar fallback; SHA-256 compression accelerated via Apple Metal when available.
  - AES, SHA-3, Poseidon helpers and BN254 utilities on CPU; Ed25519 and ECDSA verification; optional ML-DSA (Dilithium) behind the `ml-dsa` feature.
  - Deterministic parallel scheduler with conflict-aware grouping; optional HTM fast path on x86_64 (feature `htm`).
  - Mock Halo2 circuits and ZK trace logging for prototyping (see `halo2.rs`, `zk.rs`).

### Gated test suites

- VRF tests (BLS/VRF helpers): gated by feature `ivm_vrf_tests`.
  - Run: `cargo test -p ivm --features ivm_vrf_tests vrf`
- ZK/Circuit‑heavy tests (Halo2 circuits, BN254, Poseidon gadgets, Merkle proofs): gated by feature `ivm_zk_tests`.
  - Run: `cargo test -p ivm --features ivm_zk_tests`
  - This keeps quick local runs fast while allowing full coverage on CI or dev machines.

### Zero-knowledge backend

`ivm` links the Halo2-backed verifiers from `iroha_zkp_halo2` in every build (feature `halo2`, enabled by default). The mock routines remain only for targeted unit tests behind flags such as `ivm_zk_tests`; end users no longer need to toggle anything to exercise the true prover circuits.

Notes
- Real proving is not performed on consensus paths; the host verifies proofs only. Any future proving flows should run off‑chain or outside consensus‑critical logic.
- All paths remain deterministic across hardware (no nondeterministic parallel reductions). When acceleration is enabled, results are required to match scalar fallbacks bit‑for‑bit.
- Acceleration milestones (see `roadmap.md`, sections **WP1–WP4**) include:
  1. **CUDA parity hardening** — implement the Poseidon2/6 kernels, AES/SHA-3 helpers, and extend the PTX build script so CI can execute the parity suite on the dedicated GPU lane (`roadmap.md`, WP1-B/C and WP1-G/H).
  2. **Metal vector hot path (delivered)** — interpreter vector ops (`VADD32/64`, `VAND`, `VXOR`, `VOR`, `VROT32`) now route through the shared vector helpers so Metal/CUDA/CPU back-ends are selected at runtime with deterministic fallbacks and chunked logical vector lengths (`roadmap.md`, WP2-A/B/D).
  3. **Ed25519 batch opcode (delivered)** — `ED25519BATCHVERIFY` now consumes a Norito-encoded request, charges a base + per-entry gas cost, writes the failure index to `rs2`, and reuses the CUDA fast path when available with deterministic CPU fallback (closing `roadmap.md`, WP3-A/B/C).
  4. **CRC64 GPU back-ends (delivered)** — Chunked Metal/CUDA helpers now feed `hardware_crc64` with a 192 KiB default cutoff (`NORITO_GPU_CRC64_MIN_BYTES` override) and support explicit helper overrides via `NORITO_CRC64_GPU_LIB` (stubbed in tests). The CUDA path composes per-chunk CRC outputs on-host, Metal mirrors the same chunking, and Stage‑1 cutovers were re-benchmarked (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), keeping the scalar cutover at 4 KiB while aligning the Stage‑1 GPU minimum to 192 KiB, closing WP4-A/B/C.
  5. **Kotodama codegen completion** — finish the pointer-ABI-aware bytecode emission and Norito argument helpers so the compiler can target every syscall surface without falling back to manual stubs (tracked in the Kotodama compiler backlog in `roadmap.md`).

## Kotodama

High-level smart contract language targeting IVM bytecode:

- Grammar and syntax (current + planned): `../../docs/source/kotodama_grammar.md`
- Gaps vs. implementation and roadmap: see `../../roadmap.md`


## Features

- **Register-based ISA:** 256 general-purpose **64-bit** registers (`r0`–`r255`) with `r0` fixed at zero. In zero-knowledge mode each register carries a 1-bit privacy tag. The ISA supports arithmetic (ADD, SUB, MUL, DIV, etc.), logic (AND, OR, XOR), memory loads/stores, control flow (jumps and branches), and system calls.
- **Compressed Instructions:** 16-bit compressed forms for common operations to reduce program size (e.g., short jumps, immediate moves, etc.).
- **Mixed Encoding:** 32-bit instructions expose full-width register fields, while
  the 16-bit variants operate on a smaller register subset. The encoding is sized to address the current 256-register file and leaves headroom for future extensions.
- **Memory Management:** Region-based memory with permission checks. Code, heap, input, output and stack regions are predefined. Misaligned or out-of-bounds accesses cause traps.
- **Gas Accounting:** Each instruction consumes a specified amount of gas from a gas budget. Execution halts with an error if gas is exhausted before completion.
- **GETGAS Instruction:** Programs may query the current remaining gas via opcode `0x61` which writes the value to a destination register.
- **Extended Arithmetic:** Support for `DIVU`, `REMU`, `MULH` and `MULHU` instructions providing unsigned division, unsigned remainder and high-word multiplication.
- **Host Interoperability (Syscalls):** A trait-based host interface (`IVMHost`) allows the VM to invoke host environment services via the `SCALL` instruction (opcode `0x60`) with an 8-bit call number. A default host implementation is provided (which treats all calls as unimplemented).
 - **Vector Extensions (feature: `simd`)**: Compile-time gated intrinsics (x86 SSE/AVX, aarch64 NEON) with runtime selection among the compiled-in options. If disabled, the scalar path is used. `SHA256BLOCK` uses a Metal kernel on macOS when enabled (`metal` feature). Lane‑wise ops (vadd32/vadd64/and/xor/or/rot32) leverage SIMD when compiled in; deterministic CPU fallbacks remain.
 - **Apple Metal (feature: `metal`, macOS-only):** When enabled and a compatible device is present, Metal kernels accelerate vector ops (`vadd*`, `vand`, `vxor`, `vor`, `vrot32`) and SHA‑256 compression. The code is not compiled on non-macOS targets and falls back to CPU/SIMD when Metal is unavailable or disabled.
 - **CUDA (feature: `cuda`):** Optional PTX kernels with a `build.rs` that compiles or falls back to bundled PTX. If not enabled or unavailable, CPU fallbacks are used.
- **Zero-Knowledge Support:** When a program specifies a non-zero `max_cycles` limit, execution traces are padded to that length and assertion failures do not immediately abort. Per-cycle Merkle roots of registers and memory are logged so proofs can verify each step without reconstructing the entire state. The default padding limit has been increased to **131,072 cycles** so more complex programs can be proven.
- **Execution Proof Circuits:** Recorded traces are checked by mock circuits implemented in Rust as a stand‑in for future Halo2 circuits. Future milestone: replace the mocks with real Halo2 circuits once the prover pipeline is available.
- **Program Hashing:** When a program is loaded the VM computes a SHA-256 hash of the code. It can be obtained via `IVM::code_hash()` and supplied as a public input so verifiers agree on the exact contract that was executed.
- **Turing Complete & Gas-Limited:** Branching, jumping and memory operations allow any algorithm to be expressed. A contract must also supply a gas budget, ensuring execution halts deterministically.
- **Optimised for Financial Operations:** Fast 64-bit arithmetic and register access keep asset calculations efficient, suitable for high-throughput ledgers.
- **Bulk Memory Helpers:** `load_bytes` and `store_bytes` efficiently copy contiguous regions, speeding up cryptographic hashing and serialization.
- **Quantum-Resistant Signatures:** Optional verification of ML‑DSA (Crystals Dilithium) signatures via the `ml-dsa` feature.
- **SIMD Poseidon Hashing:** Automatically uses vector instructions for the `POSEIDON2` and `POSEIDON6` opcodes when supported by the host CPU.
- **SIMD Field Arithmetic:** BN254 helpers are implemented on CPU with runtime SIMD detection plumbed through vector utilities. For benchmarking or deterministic testing, thread `AccelerationPolicy::with_forced_simd(Some(SimdChoice::{Scalar|Sse2|Avx2|Avx512|Neon}))` through `IvmConfig`, or call `ivm::set_forced_simd` in tests; unsupported requests automatically fall back to the scalar implementation to preserve safety. Future work: add architecture-specific intrinsics where beneficial.
- **Apple Metal Acceleration:** On macOS the VM accelerates vector lanes (`vadd32`/`vadd64`/`vand`/`vxor`/`vor`), SHA‑256 compression and tree reductions, Keccak‑f1600, AES rounds/batches and ed25519 batch verification (when the `ed25519` feature is enabled) via Metal when a compatible device is present. Acceleration is gated by `AccelerationPolicy::with_metal(true)`, honours developer toggles like `IVM_DISABLE_METAL`/`IVM_FORCE_METAL_ENUM`, and falls back to CPU/SIMD with identical semantics when Metal is unavailable or disabled.
- **Optional backends remain deterministic:** Metal/CUDA are best-effort accelerators; when features are disabled or hardware is unavailable, helpers fall back to scalar/SIMD paths so results stay identical across hosts.
- **CUDA Acceleration:** The `cuda` feature enables CUDA bindings. Kernel coverage is partial; build scripts invoke `nvcc` (overridable via `IVM_CUDA_NVCC`/`NVCC`) to compile the kernels in `cuda/` to PTX and fall back to lightweight stubs when CUDA isn’t available so CPU paths stay usable. Use `IVM_DISABLE_CUDA=1` to force CPU-only execution, `IVM_MAX_GPUS` to cap device count, and `IVM_CUDA_GENCODE` / `IVM_CUDA_NVCC_EXTRA` to tune compilation flags.
- **Hybrid STM/HTM Transactions:** When built with the optional `htm` feature on x86‑64, the scheduler attempts to commit transactions using hardware transactional memory (RTM) and falls back to a mutex-based path if unavailable.
- **Startup Jingle:** When built with the optional `beep` feature,
  `irohad` calls `IVM::beep_music()` and plays a short tune when the
  configuration enables it. Disable via `ivm.banner.beep = false` in your node
  config.
- **Adaptive Branch Prediction:** A built‑in two‑bit predictor tracks recent branch outcomes so tight loops execute faster. Accuracy can be queried via `IVM::branch_prediction_accuracy()` for diagnostics.
- **Merkle‑Backed Memory:** Memory writes are batched until `commit()` recomputes a Merkle root over the entire image. The root calculation now hashes chunks in parallel with Rayon for faster commits. Authentication paths can be requested for proofs.
- **Dynamic Heap Growth:** The heap may be extended at runtime using the `SYSCALL_GROW_HEAP` syscall when allocations exceed the initial 64 KB.
- **Conflict Prediction Scheduler:** Parallel execution groups transactions by predicted access conflicts to maximize throughput while preserving determinism.
- **Expanded Crypto Opcodes:** Poseidon permutations, BLS12‑381 key operations and pairings are provided for advanced ZK applications.

## Memory Model

The VM divides memory into a set of regions. Code is loaded starting at
`0x0000_0000` and is marked read/execute only.  The heap begins at
`0x0010_0000` and can dynamically grow (via `SYSCALL_GROW_HEAP`) beyond its
initial 64&nbsp;KB as allocations are requested through `SYSCALL_ALLOC`.
The read-only input buffer begins at `0x0020_0000` (64&nbsp;KB) and the
read/write output buffer begins at `0x0021_0000` (32&nbsp;KB).  Finally a
4&nbsp;MB stack begins at `0x0030_0000`.  All loads and stores are checked
for permissions and proper
alignment by [`Memory`](src/memory.rs).  The entire memory image is committed
via a Merkle tree; pending writes are batched until `commit()` re-hashes only
the dirty subtrees, enabling succinct ZK proofs of state without touching
unchanged memory.

## Opcode Reference

IVM now standardises on the wide encoding (8-bit primary opcode + three 8-bit operand slots). The primary opcode ranges are:

- **0x01-0x26:** Integer arithmetic and logical operations (`ADD`, `SUB`, `MULH`, `DIVU`, `ADDI`, …).
- **0x30-0x33:** Memory access (`LOAD64`, `STORE64`, `LOAD128`, `STORE128`).
- **0x40-0x4B:** Control flow (`BEQ`, `BNE`, `JAL`, `JR`, `HALT`, `JMP`, `JALS`).
- **0x60-0x62:** System helpers (`SCALL`, `GETGAS`, `SYSTEM`).
- **0x70-0x78 / 0x80-0x8E:** Vector and cryptographic primitives (`VADD32`, `SETVL`, `PARBEGIN`, `SHA256BLOCK`, `POSEIDON6`, `AESENC`, `ED25519VERIFY`, `PAIRING`, …).
- **0x90-0x9F:** ISO 20022 messaging helpers.
- **0xA0-0xA6:** Zero-knowledge field operations (`ASSERT`, `FADD`, `ASSERT_RANGE`).

See [`docs/opcodes.md`](docs/opcodes.md) for the full opcode tables, operand conventions, and encoding details. The canonical helpers in `encoding::wide` and `instruction::wide` cover the entire surface and are used throughout Kotodama and the runtime.

## Syscall Interface (ISI)

Smart contracts interact with the host ledger through the `SCALL` instruction
(opcode `0x60`).  The immediate operand selects an "Iroha Special Instruction"
(ISI) whose numeric assignments are listed in
[`syscalls.rs`](src/syscalls.rs).  The host implementation determines the exact
semantics and gas costs of these calls.

## Build and Usage

Ensure you have Rust (edition 2024 or later). To include IVM in your project, add to `Cargo.toml`:
```toml
ivm = { path = "path/to/ivm" }
```

Build the library and run the test-suite with Cargo:

```bash
cargo build --release
cargo test
```

To enable the optional startup jingle, build `irohad` with the `beep` feature:

```bash
cargo build -p irohad --features beep
```

Beep runs by default; set `ivm.banner.beep = false` in your configuration to disable
it for local runs.

To compile with CUDA support enabled, build with the `cuda` feature. The CUDA
toolkit and drivers must be installed. At runtime GPUs are detected automatically.
Set `IVM_DISABLE_CUDA=1` to force CPU mode or `IVM_MAX_GPUS` to limit how many
devices are initialised. The build script is a no-op without this feature, so builds
that omit `--features cuda` skip CUDA kernel compilation.

You can also override CPU SIMD detection via configuration: set
`AccelerationPolicy::with_forced_simd(Some(SimdChoice::Scalar|Sse2|Avx2|Avx512|Neon))`
when building an `IvmConfig`, or call `ivm::set_forced_simd` in tests/benches.
The runtime validates that the requested backend is actually available; if not,
the scalar path is used to avoid undefined behaviour.

```bash
cargo build --features cuda
```

See [docs/gpu_deployment.md](docs/gpu_deployment.md) for operational
guidelines, including a rollout plan for clusters with eight A100 GPUs.

Several small examples are provided. They can be executed with `cargo run`:

```bash
cargo run --example add       # simple ADD/HALT demo
cargo run --example sha256    # SHA256BLOCK vector instruction
```

### Smart-contract analysis CLI

`koto_lint` now bundles static analysis and fuzzing for Kotodama contracts and can report results in JSON for editor/CI integration:

```bash
# Lint + deterministic static analysis on source
cargo run -p ivm --bin koto_lint -- --static path/to/contract.ko

# Full pipeline (lint + static + fuzz) on bytecode
cargo run -p ivm --bin koto_lint -- --all path/to/contract.to

# Emit machine-readable JSON report
cargo run -p ivm --bin koto_lint -- --all --json path/to/contract.ko
```

## Benchmarking

Criterion-based benchmarks live under `benches/` and are run with `cargo bench`.
If the `gnuplot` command is missing you will see:

```
Gnuplot not found, using plotters backend
```

The benchmarks still execute, but installing `gnuplot` provides nicer graphs.
On Debian/Ubuntu:

```bash
sudo apt-get install gnuplot
```

On macOS (Homebrew):

```bash
brew install gnuplot
```
The `bench_field` suite exercises the BN254 backends and shows the speedup from SIMD dispatch.


The `bench_poseidon` benchmark measures performance of the Poseidon hashing
implementation and automatically selects the SIMD or scalar path.

Additional suggestions for optimising performance can be found in
[docs/performance_tasks.md](docs/performance_tasks.md).

## Merkle Commitments: VM vs Ledger

IVM maintains Merkle commitments during execution to complement ledger‑level commitments. These layers serve different purposes:

- VM root: A Merkle tree over VM execution state (registers and/or memory regions). The VM recomputes this deterministically (and, when enabled, in a parallel but deterministic manner) to produce execution receipts and ZK‑friendly traces. VM roots are ephemeral and scoped to a program’s execution.
- Ledger roots: The node/WSV maintains Merkle commitments for blocks (e.g., transactions in a block) and may expose a world‑state snapshot root for light‑client proofs. These are durable consensus objects and live outside the VM.
- Compatibility: Both use the canonical `iroha_crypto::MerkleTree` type and the same hashing semantics (inner nodes are SHA‑256 of left||right; a missing right child promotes the left). Integration tests ensure roots and proofs match across crates so a proof built in one context verifies against a root computed in the other.
- Determinism: The VM root computation is hardware‑independent. Optional parallel implementations avoid non‑deterministic reductions to preserve identical outputs on all nodes.

In short, VM‑level commitments provide verifiable execution evidence, while ledger‑level commitments provide durable inclusion/state proofs. They are complementary, not competing.

Canonical helpers and re-exports:

- Use `iroha_crypto::MerkleTree<[u8;32]>::from_byte_chunks(bytes, 32)?` (or `from_chunked_bytes_parallel` with the `rayon` feature) to build byte‑chunk trees. These helpers return `Result` and validate chunk sizes (`1..=32`).
- Use `iroha_crypto::MerkleTree::get_proof(idx)` to obtain Merkle proofs; verify with `MerkleProof::<[u8;32]>::verify_sha256`.
- The `ivm` crate re‑exports the canonical type as `ivm::MerkleTree` for convenience; VM adapters like `ivm::ByteMerkleTree` are thin wrappers that build on it.

## Documentation

Generate HTML API docs using Cargo:

```bash
cargo doc --open
```

This builds the crate documentation and opens it in your browser so you can
explore the VM implementation in detail.

You can skip building dependencies with `--no-deps` or enable optional crate
features when generating the docs:

```bash
cargo doc --no-deps --features "parallel"
```

The rendered API documentation for the latest release is also available online
at <https://docs.rs/ivm>. To include private items in the generated docs set
`RUSTDOCFLAGS`:

```bash
RUSTDOCFLAGS="--document-private-items" cargo doc
```

The `instruction` module in the generated docs enumerates every opcode
constant used by the VM. A convenient summary is also provided in
[docs/opcodes.md](docs/opcodes.md).

## KOTODAMA Source Language

「言霊の幸わうブロックチェーン」

`kotodama` is a higher level language that will compile down to IVM bytecode.
The compiler lives under `src/kotodama/` and currently contains placeholders
for the parser, AST and code generator. Once completed it will allow writing
smart contracts in a more ergonomic syntax while producing the same bytecode
understood by the VM.

## Example Programs

IVM programs consist of simple assembly instructions operating on 512
general‑purpose registers. Below are a few toy programs illustrating the style
of assembly understood by the VM. Pseudocode labels are used for clarity.

### Add two numbers

```asm
ADDI x1, x0, 2   ; load immediate 2
ADDI x2, x0, 3   ; load immediate 3
ADD  x3, x1, x2  ; x3 = x1 + x2
HALT             ; stop execution
```

### Conditional asset transfer

```asm
BLTU x13, x12, abort    ; if balance < amount -> abort
SCALL 0x24              ; TRANSFER_ASSET
HALT
abort:
SCALL 0x02             ; ABORT
HALT
```

## Specification Compliance

This crate implements the complete IVM v1.1 specification. Field arithmetic,
vector operations, zero‑knowledge assertions and the extensible syscall
interface are fully supported.


## Parallel Execution (Experimental)

IVM can optionally execute independent instructions and even whole transactions in parallel.
The goal is higher throughput on multi‑core machines without sacrificing deterministic
results. The VM groups non‑conflicting operations in *cycles* and dispatches them to a
thread pool. Operations that read or write the same registers or memory locations are not
allowed in the same cycle. After all instructions in a cycle finish, their effects are
committed in program order so the final state is identical to sequential execution.

At the transaction level the runtime may schedule distinct transactions concurrently when
their declared state access sets do not overlap. Access lists are derived from ISI
parameters so every node computes the same schedule.

Gas is deducted before executing each cycle based on the sum of its instruction costs;
this guarantees out‑of‑gas behaviour matches sequential semantics. In zero‑knowledge mode
the proof circuit models a cycle as a single state transition encompassing its parallel
instructions while enforcing the same dependency rules.

Parallel execution is enabled by default and automatically utilises all physical CPU
cores. This scales IVM across multi‑core machines while preserving strict determinism
for consensus and ZK verification.

See `docs/parallel_execution.md` for the full implementation specification.

## Validation Rules

Transactions may be rejected due to invalid signatures, missing authority, and incorrect currency denominations.
