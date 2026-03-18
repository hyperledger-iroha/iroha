<!--
Copyright 2024 Hyperledger Iroha Contributors
SPDX-License-Identifier: Apache-2.0
-->

# Iroha Python SDK Architecture

## Purpose

`iroha_python` provides a cohesive Python surface for Hyperledger Iroha v2
applications. The package bundles three pillars:

- **Norito codec access** via the maintained `norito` pure-Python module.
- **Torii client workflows** that layer higher-level submission/query utilities.
- **Cryptographic and transaction helpers** bridged from the Rust workspace
  through a `maturin`-built PyO3 extension (`iroha_python._crypto`).
- **Data model wrappers** exposing Rust-validated identifiers (domain/account/
  asset IDs) and high-level instruction builders so Python code can assemble
  canonical Norito payloads without hand-crafted JSON, with JSON round-tripping
  helpers for the remaining instruction families.
- **Torii transport layer** that layers configurable retries/backoff, bearer
  auth, and JSON helpers on top of the HTTP client, providing parity with the
  Rust SDK while keeping gRPC integration pluggable for environments that ship
  the proto bindings.
- **Governance and Connect helpers** that expose high-level wrappers for the
  Torii governance endpoints (`/v1/gov/*`) and pave the way for Connect flows
  by sharing the transport and optional gRPC scaffolding.

The SDK aims to expose deterministic, Norito-first APIs that mirror the Rust
bindings.

## Package Layout

```
python/iroha_python/
├── pyproject.toml            # Unified package metadata (PEP 621)
├── src/iroha_python/
│   ├── __init__.py           # Public API surface + norito re-export
│   ├── crypto.py             # High-level Ed25519 + transaction helpers
│   ├── client.py             # Torii HTTP wrapper and polling helpers
│   ├── query.py              # Structured account/query envelope builders
│   ├── query_filter.py       # Deterministic filter DSL for metadata/fields
├── bin/submit_envelope_json.py  # CLI helper for JSON envelope replay
├── iroha_python_rs/          # PyO3 cdylib built by maturin
└── tests/                    # pytest parity tests
```

The Rust crate (`iroha_python_rs`) links against `iroha_crypto`,
`iroha_data_model`, and `norito` to guarantee parity with the Rust toolchain.
`pyproject.toml` configures `maturin` so `cargo build --workspace` builds the
extension alongside the rest of the workspace.

## Packaging and Tooling

- `maturin develop` builds the PyO3 extension against the local Rust workspace.
- `pytest` runs the Python parity suite; tests expect the extension to be
  available (either via `maturin develop` or an installed wheel).
- `cargo test --workspace` continues to validate the Rust crates, including
  `iroha_python_rs`.

Release automation will publish a single `iroha-python` wheel/sdist to PyPI.
