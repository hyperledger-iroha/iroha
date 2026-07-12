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

iroha --config defaults/client.toml \
  contract debug-call \
  --code-file target/quickstart/hello.to \
  --source-file target/quickstart/hello.ko \
  --entrypoint write_detail \
  --payload-json '{}'
```

The debugger reports gas use, cycles, syscalls, queued instructions, durable
state, and source-aware traps. Entrypoints are always selected by name; V1 has
no implicit `main` or source-order dispatch.

## 4. Start a development network

The repository includes a Docker Compose development bundle:

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Keep it running while using the commands below. Release and integration
validation uses representative four-validator networks; this single-node
bundle is only a local authoring convenience.

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
