# Norito Getting Started

This guide uses the strict Kotodama V1 language and the unified Rust toolchain
to check, build, inspect, exercise, and deploy one canonical IVM (`.to`)
artifact.

## Prerequisites

Install a Rust toolchain and check out this repository. Build the two commands
used below:

```sh
cargo build -p ivm --bin koto
cargo build -p iroha_cli --bin iroha
```

Add `target/debug` to `PATH`, or prefix the commands with
`./target/debug/`. A running Iroha network is needed only for deployment and
remote calls; the local debug command uses an in-process host.

## 1. Check and build a Kotodama contract

The repository ships `examples/hello/hello.ko`. Validate it without writing an
artifact, then build it with the release default cycle ceiling:

```sh
koto check examples/hello/hello.ko

koto build examples/hello/hello.ko \
  --max-cycles 1000000 \
  --out target/examples/hello.to \
  --manifest-out target/examples/hello.manifest.json
```

`koto check` runs parsing, resolution, type checking, effect analysis, and
linting. `koto build` writes the canonical artifact plus hash-keyed source-map
and budget sidecars. A repeated build with unchanged inputs reports `fresh` and
does not rewrite outputs.

ABI v1 is unconditional. Contract source cannot select an ABI, vector width,
or execution feature. The positive cycle ceiling is embedded in the execution
header, covered by `code_hash`, and must not exceed node admission policy.

## 2. Inspect the interface

Generate Markdown documentation from the same compiler driver:

```sh
koto doc examples/hello/hello.ko
```

The generated interface lists the named `hajimari`/`始まり`, authorized
`kotoage fn`/`言挙げ fn`, and
read-only `view fn` declarations, their typed parameters and returns, stable
contract error codes, and compiler-derived effects. CNTR carries this interface
inside the artifact, but nodes independently validate bytecode control flow and
derive security-relevant effects/access at admission.

## 3. Exercise an entrypoint locally

Use the Iroha CLI's local contract debugger. Always name the entrypoint; V1 has
no implicit source entrypoint or source-order dispatch.

```sh
build_record=target/examples/.fingerprints/hello.to.record
artifact_hash=$(sed -n 's/^artifact_hash=//p' "$build_record")
input_fingerprint=$(sed -n 's/^input=//p' "$build_record")
source_map_file="target/examples/.sidecars/$artifact_hash/$input_fingerprint/source-map.json"
test -f "$source_map_file"

iroha --config defaults/client.toml \
  contract debug-call \
  --code-file target/examples/hello.to \
  --source-map-file "$source_map_file" \
  --source-file examples/hello/hello.ko \
  --entrypoint main \
  --payload-json '{}'
```

The response includes the typed result, gas/cycle budget, syscall trace, queued
instructions, and durable-state overlay. To add source locations and snippets
for traps, pass the hash-bound compiler sidecar with `--source-map-file`; an
optional `--source-file` overrides the sidecar's source path and is accepted
only when the sidecar is also supplied. The example reconstructs the exact
sidecar path from the build record; the debugger rejects a sidecar whose
`artifact_hash` does not match `--code-file`. Relative source paths in the
sidecar are resolved from the current working directory. For a read-only
declaration use `contract debug-view` instead.

Contract test files use the same compiler and runtime path:

```sh
koto test path/to/contract.test.ko
```

## 4. Deploy via Iroha

Deploy the exact `.to` artifact. The alias identifies the deployed contract;
the authority and private key sign the deployment request.

```sh
iroha --config defaults/client.toml \
  contract deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --contract-alias hello::universal \
  --code-file target/examples/hello.to \
  --wait
```

The response includes the canonical contract address and code hash. Fetch the
admitted manifest by that hash:

```sh
iroha --config defaults/client.toml \
  contract manifest get \
  --code-hash 0x<64-hex-digits>
```

## 5. Call the deployed contract

Public call payloads are JSON at the CLI boundary. The runtime converts the
object to one canonical Norito argument record and the wrapper decodes it once.
Zero-parameter entries omit the payload entirely; an empty object is still a
payload and is rejected.

```sh
iroha --config defaults/client.toml \
  contract call \
  --contract-alias hello::universal \
  --entrypoint main \
  --wait
```

The caller must hold the permission named by `authorize`, and the host still
applies operation-specific authorization. Views use `contract view` and
cannot mutate durable or ledger state.

## Next steps

- Read `docs/source/kotodama_grammar.md`, the normative Kotodama V1 grammar.
- Explore the [compile-checked example gallery](./examples/).
- Use `koto explain <diagnostic-code>` for a stable compiler diagnostic.
- Use `iroha contract dev` for manifest-based multi-module projects; it
  calls the same compiler driver in process.
