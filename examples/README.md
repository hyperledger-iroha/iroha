# Kotodama + IVM Examples

This folder contains small, auditable examples of Kotodama source `.ko` and how to compile them to IVM bytecode `.to`, then run with an IVM runner.

Note: This repository may not include the `koto_compile` or `ivm_run` binaries. The commands below are a doc-test style walkthrough you can run locally if you have those binaries on your PATH. If not present, see the ignored integration test at `integration_tests/tests/kotodama_examples.rs` which auto-skips.

Quick start (doc-test style, safe to copy/paste)

```sh
# Compile Kotodama to IVM bytecode (.to)
koto_compile examples/hello/hello.ko -o target/examples/hello.to --abi 1 --max-cycles 0
# Lint runs automatically; add --no-lint to skip or --deny-lint-warnings to fail on warnings.

# Inspect header (optional)
ivm_tool inspect target/examples/hello.to

# Run on the IVM (starts from the artifact's default entrypoint)
ivm_run target/examples/hello.to --args '{}'

# Transfer example (if your runtime host supports it)
koto_compile examples/transfer/transfer.ko -o target/examples/transfer.to --abi 1
ivm_run target/examples/transfer.to --args '{}'
```

Expected output
- An info log like: `Hello from Kotodama`.
- A successful `SET_ACCOUNT_DETAIL` syscall with the current authority as the account, writing a small JSON blob.
- For transfer: a successful `TRANSFER_ASSET` syscall (depends on host permissions/state).

Raw `ivm_run` and `iroha transaction ivm` execution start from a single compiled
default entrypoint. In `hello/hello.ko`, that entrypoint is `main`, which logs
the greeting and then calls `write_detail()`.

Files
- `hello/hello.ko`: Minimal contract whose raw-IVM `main()` entrypoint logs a greeting and calls `write_detail()`.
- `transfer/transfer.ko`: Example that calls `transfer_asset(...)` using typed pointer constructors.
- `nft/nft.ko`: Examples that call `nft_mint_asset(...)` and `nft_transfer_asset(...)`.
- `crates/kotodama_lang/src/samples/native_escrow.ko`: Native escrow wrapper that calls the ledger-managed escrow ISIs instead of using a contract escrow account.
- `map/map.ko`: Design example showing deterministic map iteration using `.take(n)`; compile/run may depend on compiler/runtime support.

Docs
- See `docs/source/kotodama_examples.md` for the syscall mappings and a design example of deterministic map iteration with bounds.
- Rust snippets in the docs (wide opcode helpers, lowering examples) describe compiler internals and are not valid Kotodama `.ko` source.

Sample `ivm_tool inspect` output (illustrative)

```
Artifact: target/examples/hello.to
Header:
  magic:      IVM\0
  version:    1.0
  abi_version: 1
  feature_bits: ZK=off, VECTOR=off
  vector_len:  0
  max_cycles:  0
Sections:
  code_size:   312 bytes
  data_size:   128 bytes (Norito TLVs)
Entry points:
  main
  hajimari
  write_detail
```
