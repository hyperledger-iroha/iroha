# Norito Getting Started

This quick guide shows the minimal workflow for compiling a Kotodama contract,
inspecting the generated Norito bytecode, running it locally, and deploying it
to an Iroha node.

## Prerequisites

1. Install the Rust toolchain (1.76 or newer) and check out this repository.
2. Build or download the supporting binaries:
   - `koto_compile` – Kotodama compiler that emits IVM/Norito bytecode
   - `ivm_run` and `ivm_tool` – local execution and inspection utilities
   - `iroha` – used for contract deployment and contract calls via Torii

   The repository Makefile expects these binaries on `PATH`. You can either
   download prebuilt artifacts or build them from source. If you compile the
   toolchain locally, point the Makefile helpers at the binaries:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Ensure an Iroha node is running when you reach the deployment step. The
   examples below assume Torii is reachable at the URL configured in your
   `iroha` CLI profile (`~/.config/iroha/cli.toml`).

## 1. Compile a Kotodama contract

The repository ships a minimal “hello world” contract in
`examples/hello/hello.ko`. Compile it to Norito/IVM bytecode (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Key flags:

- `--abi 1` locks the contract to ABI version 1 (the only supported version at
  the time of writing).
- `--max-cycles 0` requests unbounded execution; set a positive number to bound
  cycle padding for zero-knowledge proofs.

## 2. Inspect the Norito artifact (optional)

Use `ivm_tool` to verify the header and embedded metadata:

```sh
ivm_tool inspect target/examples/hello.to
```

You should see the ABI version, enabled feature flags, and the exported entry
points. This is a quick sanity check before deployment.

## 3. Run the contract locally

Execute the bytecode with `ivm_run` to confirm behaviour without touching a
node:

```sh
ivm_run target/examples/hello.to --args '{}'
```

The `hello` example logs a greeting and issues a `SET_ACCOUNT_DETAIL` syscall.
Running locally is useful while iterating on contract logic before publishing
it on-chain.

Raw `ivm_run` and `iroha transaction ivm` execution enter only the compiled
default entrypoint. The checked-in `examples/hello/hello.ko` declares `main()`
so the smoke test reaches `write_detail()` without needing an explicit
selector.

## 4. Deploy via `iroha`

When you are satisfied with the contract, deploy it to a node using the CLI.
Provide an authority account, its signing key, and either a `.to` file or
Base64 payload:

```sh
iroha app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

The command submits a Norito manifest + bytecode bundle over Torii and prints
the resulting transaction status. Once the transaction is committed, the code
hash shown in the response can be used to retrieve the on-chain manifest:

```sh
iroha app contracts manifest get --code-hash 0x<hash>
```

## 5. Run against Torii

With the contract deployed, you can invoke it through
`iroha app contracts call --contract-address <contract-address> --entrypoint main --wait`
or your application client. Ensure the account permissions allow the desired
syscalls (`set_account_detail`, `transfer_asset`, etc.).

## Tips & troubleshooting

- Use `make examples-run` to compile and execute the provided examples in one
  shot. Override `KOTO`/`IVM` environment variables if the binaries are not on
  `PATH`.
- If `koto_compile` rejects the ABI version, verify that the compiler and node
  both target ABI v1 (run `koto_compile --abi` without arguments to list
  support).
- The CLI accepts either hex or Base64 signing keys. For testing, you can reuse
  the dev key from `defaults/client.toml` or generate fresh keys with
  `kagami keys --json`.
- When debugging Norito payloads, the `ivm_tool disassemble` subcommand helps
  correlate instructions with Kotodama source.

This flow mirrors the steps used in CI and the integration tests. For a deeper
dive into Kotodama grammar, syscall mappings, and Norito internals, see:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
