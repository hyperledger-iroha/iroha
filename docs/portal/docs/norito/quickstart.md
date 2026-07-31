---
title: Norito Quickstart
description: Check, build, debug, and deploy one strict Kotodama V1 contract.
slug: /norito/quickstart
---

This walkthrough exercises the release path end to end: the canonical Rust
compiler produces one hashed IVM artifact, the local contract debugger invokes
a named entrypoint, and the Iroha CLI submits that same artifact through Torii.

## 1. Build the tools

From the repository root:

```sh
cargo build -p ivm --bin koto
cargo build -p iroha_cli --bin iroha
cargo build -p iroha_kagami --bin kagami
export PATH="$PWD/target/debug:$PATH"
```

## 2. Author one deployable source

Create `target/quickstart/hello.ko` with the following content. A deployable
file contains exactly one named `seiyaku`/`誓約` unit.

```kotodama
seiyaku Hello {
    hajimari() {
        debug::info("Hello from hajimari");
    }

    kotoage fn write_detail() authorize("Admin") {
        ledger::account::set_detail(
            account: context::authority(),
            key: Name::parse("example"),
            value: Json::parse("{\"hello\":\"world\"}"),
        );
    }

    view fn healthy() -> bool {
        return true;
    }
}
```

`kotoage fn`/`言挙げ fn` is a mutating public call and therefore declares caller
authorization. `view fn` is read-only and public by default. Lifecycle
authorization for `hajimari`/`始まり` is runtime-defined.

## 3. Check, build, and debug

```sh
mkdir -p target/quickstart
koto check target/quickstart/hello.ko
koto build target/quickstart/hello.ko \
  --out target/quickstart/hello.to \
  --manifest-out target/quickstart/hello.manifest.json \
  --max-cycles 1000000

build_record=target/quickstart/.fingerprints/hello.to.record
artifact_hash=$(sed -n 's/^artifact_hash=//p' "$build_record")
input_fingerprint=$(sed -n 's/^input=//p' "$build_record")
source_map_file="target/quickstart/.sidecars/$artifact_hash/$input_fingerprint/source-map.json"
test -f "$source_map_file"

iroha --config defaults/client.toml \
  contract debug-call \
  --code-file target/quickstart/hello.to \
  --source-map-file "$source_map_file" \
  --source-file target/quickstart/hello.ko \
  --entrypoint write_detail \
  --payload-json '{}'
```

The debugger reports gas use, cycles, syscalls, queued instructions, durable
state, and trap diagnostics. Pass the build's hash-bound source map with
`--source-map-file` to resolve a trap to source; `--source-file` may accompany
it to override the sidecar's source path, but cannot be used alone. The
debugger verifies that the sidecar's `artifact_hash` matches the contract
artifact before using any locations. Entrypoints are always selected by name;
V1 has no implicit `main` or source-order dispatch.

## 4. Start a development network

The repository includes a Docker Compose development bundle:

```sh
kagami keys --out-dir target/quickstart/genesis-keys

export IROHA_GENESIS_PUBLIC_KEY_FILE="$PWD/target/quickstart/genesis-keys/public.key"
export IROHA_GENESIS_PRIVATE_KEY_FILE="$PWD/target/quickstart/genesis-keys/private.key"
docker compose -f defaults/docker-compose.single.yml up --build
```

Keep it running while using the commands below. Release and integration
validation uses representative four-validator networks; this single-node
bundle is only a local authoring convenience. The Compose file contains no
genesis signing secret and refuses to evaluate unless both runtime key-file
paths are set; never commit the generated private file.

## 5. Deploy and call

```sh
iroha --config defaults/client.toml \
  contract deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --contract-alias hello::universal \
  --code-file target/quickstart/hello.to \
  --wait

iroha --config defaults/client.toml \
  contract call \
  --contract-alias hello::universal \
  --entrypoint write_detail \
  --payload-json '{}' \
  --wait
```

The deployment verifies the complete artifact hash, ABI-v1 surface, embedded
interface, and control flow. The call checks `authorize("Admin")` separately
from the ledger permission required by `ledger::account::set_detail`.

Use `contract view --entrypoint healthy --payload-json '{}'` to exercise
the read-only path.

## Next steps

- Browse the [compile-checked example gallery](./examples/).
- Read `docs/source/kotodama_grammar.md`, the single normative V1 grammar.
- Use `koto test` for `.test.ko` suites and `koto explain` for stable
  diagnostic codes.
- Use `iroha contract dev` for a multi-module manifest project; it shares
  the same compiler-driver library as `koto`.
