<!--
Copyright 2024 Hyperledger Iroha Contributors
SPDX-License-Identifier: Apache-2.0
-->

# Iroha Python SDK Architecture

## Purpose

`iroha_python` is the first-release Python surface for Hyperledger Iroha 3
applications. It has four responsibilities:

- **Norito codec access** via the maintained `norito` pure-Python module.
- **One public Torii client** for typed queries, signed submissions, bounded
  streaming, and higher-level workflows.
- **Cryptographic and transaction helpers** bridged from the Rust workspace
  through a `maturin`-built PyO3 extension (`iroha_python._crypto`).
- **Data-model and workflow helpers** exposing Rust-validated identifiers and
  canonical instruction, governance, privacy, SoraFS, and Connect operations.

The SDK is intentionally canonical rather than compatibility-oriented. Public
inputs use one spelling and one wire representation, secrets never appear in
normal representations, Torii configuration does not read ambient process
state, and network responses are size-bounded where they are streamed or
buffered. The native bridge remains the authority for cryptographic and
wire-sensitive operations.

## Package Layout

```
python/iroha_python/
├── pyproject.toml            # Unified package metadata (PEP 621)
├── src/iroha_python/
│   ├── __init__.py           # Public API surface + norito re-export
│   ├── client.py             # Canonical Torii client and typed response models
│   ├── crypto.py             # Validated key/signature and transaction helpers
│   ├── tx.py                 # Immutable transaction-draft workflow
│   ├── norito_rpc.py         # Bounded Norito-RPC transport facade
│   ├── privacy.py            # Bounded privacy event/query client
│   ├── connect.py            # Exact Connect V1 URI and session primitives
│   ├── query.py              # Structured account/query envelope builders
│   ├── query_filter.py       # Deterministic filter DSL for metadata/fields
├── bin/submit_envelope_json.py  # CLI helper for JSON envelope replay
├── iroha_python_rs/          # PyO3 cdylib built by maturin
└── tests/                    # pytest parity tests
```

The Rust crate (`iroha_python_rs`) links against `iroha_crypto`,
`iroha_data_model`, and `norito` to guarantee parity with the Rust toolchain.
Its crypto bridge enables the same signature families used by the workspace
(`ed25519`, `secp256k1`, `ml-dsa`, TC26 GOST parameter sets, `bls_normal`,
`bls_small`, and `sm2`) and exposes them through generic key/sign/verify and
multihash helpers. `pyproject.toml` configures `maturin` so
`cargo build --workspace` builds the extension alongside the rest of the
workspace.

## Client and Configuration Boundaries

`ToriiClient` is the public transport owner. It accepts only an HTTP(S) origin,
constructs origin-relative routes, disables redirects, bounds retries, closes
discarded retry responses, and leaves caller-supplied sessions open. SDK-owned
sessions ignore proxy and credential environment variables.

`resolve_torii_client_config()` is a pure merge over explicit `config`, `env`,
and `overrides` mappings. It never reads `os.environ`. Callers that deliberately
want environment configuration pass `env=os.environ`; explicit factory
keywords then take precedence. The resolved model is immutable and redacts
tokens from its representation.

Public query methods return typed result models directly. Raw dictionaries are
retained only where the current protocol family does not yet have a complete
typed model; adding a typed model should replace that public raw result rather
than introduce a parallel `*_typed` compatibility method.

## Packaging and Tooling

- `maturin develop` builds the PyO3 extension against the local Rust workspace.
- `pytest` runs the Python parity suite; tests expect the native extension to be
  available (either via `maturin develop` or an installed wheel).
- `cargo test --workspace` continues to validate the Rust crates, including
  `iroha_python_rs`.

The package metadata builds a single `iroha-python` wheel and source
distribution through Maturin.
