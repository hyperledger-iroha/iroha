---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/version-2025-q2/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4737eb18f51eb5ad638b7e405ead3bfb259ec8aa26a221422de05d8a2e69dc2
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T15:55:55+00:00"
translation_last_reviewed: 2026-01-30
---

# Norito Getting Started

This quick guide shows the minimal workflow for compiling a Kotodama contract,
inspecting the generated Norito bytecode, running it locally, and deploying it
to an Iroha node.

## Prerequisites

1. Install the Rust toolchain (1.76 or newer) and check out this repository.
2. Build or download the supporting binaries:
   - `koto_compile` – Kotodama compiler that emits IVM/Norito bytecode
   - `ivm_run` and `ivm_tool` – local execution and inspection utilities
   - `iroha_cli` – used for contract deployment via Torii

   The repository Makefile expects these binaries on `PATH`. You can either
   download prebuilt artifacts or build them from source. If you compile the
   toolchain locally, point the Makefile helpers at the binaries:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Ensure an Iroha node is running when you reach the deployment step. The
   examples below assume Torii is reachable at the URL configured in your
   `iroha_cli` profile (`~/.config/iroha/cli.toml`).

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

## 4. Deploy via `iroha_cli`

When you are satisfied with the contract, deploy it to a node using the CLI.
Provide an authority account, its signing key, and either a `.to` file or
Base64 payload:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

The command submits a Norito manifest + bytecode bundle over Torii and prints
the resulting transaction status. Once the transaction is committed, the code
hash shown in the response can be used to retrieve manifests or list instances:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Run against Torii

With the bytecode registered, you can invoke it by submitting an instruction
that references the stored code (e.g., through `iroha_cli ledger transaction submit`
or your application client). Ensure the account permissions allow the desired
syscalls (`set_account_detail`, `transfer_asset`, etc.).

## Tips & troubleshooting

- Use `make examples-run` to compile and execute the provided examples in one
  shot. Override `KOTO`/`IVM` environment variables if the binaries are not on
  `PATH`.
- If `koto_compile` rejects the ABI version, verify that the compiler and node
  both target ABI v1 (run `koto_compile --abi` without arguments to list
  support).
- The CLI accepts either hex or Base64 signing keys. For testing, you can use
  keys emitted by `iroha_cli tools crypto keypair`.
- When debugging Norito payloads, the `ivm_tool disassemble` subcommand helps
  correlate instructions with Kotodama source.

This flow mirrors the steps used in CI and the integration tests. For a deeper
dive into Kotodama grammar, syscall mappings, and Norito internals, see:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
