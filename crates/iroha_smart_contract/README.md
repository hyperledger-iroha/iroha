# Iroha Smart Contract

Library crate used for writing Iroha-compliant smart contracts targeting the Iroha Virtual Machine (IVM).

## Usage

Kotodama sources (`.ko`) compile to IVM bytecode (`.to`) which can be submitted to
the network or executed locally. The toolchain lives in this repository under the
`ivm` crate and exposes the compiler binary:

- `koto_compile` – Kotodama compiler that produces `.to` bytecode (and optional
  manifests)

### 1. Install the toolchain

Build the compiler directly from this workspace:

```bash
cargo install --path crates/ivm --bin koto_compile
```

Alternatively, invoke it in-place via `cargo run -p ivm --bin koto_compile -- …`.
For local execution and inspection flows, use the Rust examples under
`crates/ivm/examples` or embed `ivm::IVM` in a small harness/test.

### 2. Compile Kotodama source to IVM bytecode

```bash
# Compile examples/hello/hello.ko into target/examples/hello.to
koto_compile examples/hello/hello.ko \
  --out target/examples/hello.to \
  --abi 1 \
  --max-cycles 0
# Lint runs automatically; use --no-lint to skip or --deny-lint-warnings to fail on lint output.

# Optional: emit a manifest alongside the bytecode
koto_compile path/to/contract.ko \
  --out target/contract.to \
  --manifest-out target/contract.manifest.json \
  --abi 1
```

The compiler enforces the IVM ABI header (default `abi_version = 1`). You can
override the defaults with CLI flags or by embedding metadata blocks in Kotodama
source (`meta { abi_version = 1; max_cycles = … }`).

### 3. Exercise contracts locally (optional)

This workspace does not currently ship a standalone `ivm_run` binary. For local
execution, use one of the Rust examples or a small harness built against the
`ivm` crate. For example:

```bash
cargo run -p ivm --example koto_tuple_return_demo
```

That path is useful for verifying contract logic before shipping bytecode to a
node.

### 4. Submit bytecode to an Iroha network

Use the CLI to upload the compiled `.to` artifact:

```bash
# Submit to a running node
iroha transaction ivm --file target/contract.to

# Or pipe the bytecode in
cat target/contract.to | iroha transaction ivm
```

The `.to` file can also be embedded into genesis or manifests depending on your
deployment pipeline. See `examples/README.md` and `docs/source/kotodama_examples.md`
for additional workflows, including end-to-end examples that exercise the host
ABI and integration tests that compile and execute the samples automatically.
