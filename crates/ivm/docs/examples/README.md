# Kotodama Examples

This directory contains small Kotodama (`.ko`) snippets that demonstrate language features and how they map to IVM bytecode and host syscalls.

How to compile and inspect
- From Rust: use the Kotodama compiler API

```rust
use ivm::{KotodamaCompiler, ProgramMetadata};

fn main() {
    let code = KotodamaCompiler::new()
        .compile_file("crates/ivm/docs/examples/10_meta_header.ko")
        .expect("compile");
    let parsed = ProgramMetadata::parse(&code).unwrap();
    let meta = parsed.metadata;
    println!(
        "abi_version={} mode=0x{:02x} vl={} max_cycles={}",
        meta.abi_version,
        meta.mode,
        meta.vector_length,
        meta.max_cycles
    );
}
```

- In tests: `cargo test -p ivm --test kotodama` exercises many of these patterns.

- Using the CLI bin (writes `.to`):

```
cargo run -p ivm --bin koto_compile -- crates/ivm/docs/examples/10_meta_header.ko --out /tmp/meta.to --manifest-out /tmp/meta.manifest.json
```

Flags: `--abi <u8>`, `--vl <u8>`, `--max-cycles <u64>`, `--iter-cap <u8>`, `--force-zk`, `--force-vector`, `--manifest-out <path.json|->`.
Use `--manifest-out -` to print the manifest JSON to stdout instead of writing a file.
Seiyaku `meta {}` in the source takes precedence where provided.

Contents
- `01_hajimari.ko`: Minimal initializer function inside a contract.
- `02_kotoage_public_fn.ko`: Public function form (enforced by the runtime dispatcher).
- `03_kaizen_permission.ko`: Upgrade hook with a permission tag (policy enforced by governance).
- `04_foreach_map.ko`: For-each syntax (unbounded form is rejected by semantics).
- `05_range_for.ko`: Range sugar lowered to C-style loop.
- `06_map_ops.ko`: Map indexing get/set on minimal 2-word layout.
- `07_set_detail_authority.ko`: Write account detail for `authority()` using the prelude macros.
- `08_call_transfer_asset.ko`: Direct builtin `transfer_asset(...)` usage from a contract entrypoint.
- `09_struct_and_state.ko`: Parsed-only examples for struct/state declarations.
- `10_meta_header.ko`: seiyaku-level `meta {}` setting IVM header fields and emitting `SETVL`/`ASSERT` to exercise vector/zk flags.
- `11_detail_and_transfer.ko`: Pointer-ABI typed calls for metadata write and asset transfer.
- `12_nft_flow.ko`: Create an NFT and transfer it to another account.
- `13_register_and_mint.ko`: Register a new asset and mint to an account.
- `14_map_sum_take2.ko`: Deterministic two-iteration map sum via `.take(2)` on a state map.
- `15_modulo.ko`: `%` modulo operator, returns `a % b`.
 - `16_dynamic_take.ko`: Dynamic `.take(n)` guarded iteration (feature-gated).
 - `17_dynamic_range.ko`: Dynamic `.range(start,end)` guarded iteration (feature-gated).
- `18_ternary.ko`: Ternary conditional `cond ? then : else` expression.

Notes
- Kotodama targets the Iroha Virtual Machine (IVM) and produces `.to` bytecode. RISC‑V–like encodings in the implementation are IVM’s mixed-format details and not a hardware target.
- Pointer-ABI typed constructors (and their `account!(...)`, `name!(...)`, `json!(...)`, `nft_id!(...)`, `asset_definition!(...)` macro aliases) are compiled into Norito-encoded TLV blobs and passed to the host as pointers.
- Seiyaku `meta {}` influences header fields (`abi_version`, `vector_length`, `max_cycles`) and mode bits (`zk`, `vector`). The compiler validates that `zk`/`vector` flags match the opcodes actually emitted. When omitted, the compiler defaults `abi_version` to 1 and leaves other fields at their option defaults.
Dynamic bounds (feature-gated)
- Enable the `kotodama_dynamic_bounds` feature on the `ivm` crate to accept non-literal bounds for `.take(n)` and `.range(start, end)`.

Compile examples with dynamic bounds enabled:

```
cargo run -p ivm --features kotodama_dynamic_bounds --bin koto_compile -- \
  crates/ivm/docs/examples/16_dynamic_take.ko --out /tmp/dyn_take.to

cargo run -p ivm --features kotodama_dynamic_bounds --bin koto_compile -- \
  crates/ivm/docs/examples/17_dynamic_range.ko --out /tmp/dyn_range.to
```

Notes:
- The compiler inserts runtime assertions to ensure bounds are non-negative and consistent (e.g., `end >= start`).
- Lowering emits up to K guarded iterations with `if (i < n)` checks (default K = 2) to avoid extra body executions.
- You can tune K via CLI `--iter-cap <u8>` or programmatically with `CompilerOptions { dynamic_iter_cap, .. }`.
- Dynamic bounds are supported on state maps; in-memory maps accept only literal bounds (max 1 element).
- Run `koto_lint` on source files to review Kotodama lint warnings before invoking the compiler.

Implementation notes:
- Dynamic `.range(start, end)` computes the per-iteration address using `start + i` and loads key/value from `addr + {0,8}`; the iteration count is `n = end - start`. The compiler guards each unrolled body with `if (i < n)`.
