<!--
Copyright 2024 Hyperledger Iroha Contributors
SPDX-License-Identifier: Apache-2.0
-->

# Iroha Python SDK Architecture

## Purpose

`iroha_python` provides a cohesive Python surface for Hyperledger Iroha v2
applications. The package bundles three pillars that historically lived in
separate helper projects:

- **Norito codec access** via the maintained `norito` pure-Python module.
- **Torii client workflows** that reuse the battle-tested HTTP helpers from
  `iroha_torii_client` while layering higher-level submission/query utilities.
- **Cryptographic and transaction helpers** bridged from the Rust workspace
  through a `maturin`-built PyO3 extension (`iroha_python._crypto`).
- **Data model wrappers** exposing Rust-validated identifiers (domain/account/
  asset IDs) and high-level instruction builders so Python code can assemble
  canonical Norito payloads without hand-crafted JSON, with JSON round-tripping
  helpers for the remaining instruction families.
- **Torii transport layer** that layers configurable retries/backoff, bearer
  auth, and JSON helpers on top of the deprecated HTTP client, providing parity with
  the Rust SDK while keeping gRPC integration pluggable for environments that
  ship the proto bindings.
- **Governance and Connect helpers** that expose high-level wrappers for the
  Torii governance endpoints (`/v1/gov/*`) and pave the way for Connect flows
  by sharing the transport and optional gRPC scaffolding.

The SDK aims to expose deterministic, Norito-first APIs that mirror the Rust
bindings. It keeps the deprecated modules importable so downstream code can migrate
incrementally.

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
│   └── _compat.py            # Deprecated import shims (see below)
├── bin/submit_envelope_json.py  # CLI helper for JSON envelope replay
├── iroha_python_rs/          # PyO3 cdylib built by maturin
└── tests/                    # pytest parity tests and migration coverage
```

The Rust crate (`iroha_python_rs`) links against `iroha_crypto`,
`iroha_data_model`, and `norito` to guarantee parity with the Rust toolchain.
`pyproject.toml` configures `maturin` so `cargo build --workspace` builds the
extension alongside the rest of the workspace.

## Deprecated Package Integration

Many downstream projects still import helpers from `norito_py` and
`iroha_torii_client`. To unblock upgrades, the SDK keeps those modules
available and routes their public entry points through the new package:

| Deprecated Import | New Home | Notes |
|---------------|----------|-------|
| `norito_py.norito` | `iroha_python.norito` | Identical module re-export. |
| `iroha_torii_client.client.ToriiClient` | `iroha_python.client.ToriiClient` | Wrapper subclasses the deprecated client while adding transaction helpers. |
| `iroha_torii_client.mock` | `iroha_python.client` | Tests import the shim to ensure fixtures continue to work. |

The `_compat.py` module issues `DeprecationWarning`s and provides helper
functions so consumers receive actionable migration guidance at import time.
Back-compat pytest coverage exercises both deprecated entry points and their new
equivalents.

## Packaging and Tooling

- `maturin develop` builds the PyO3 extension against the local Rust workspace.
- `pytest` runs the Python parity suite; tests expect the extension to be
  available (either via `maturin develop` or an installed wheel).
- `cargo test --workspace` continues to validate the Rust crates, including
  `iroha_python_rs`.

Release automation will publish a single `iroha-python` wheel/sdist to PyPI
while the deprecated packages issue a final compatibility release that depends on
the new SDK.

## Migration Expectations

- New development should target `import iroha_python` exclusively.
- Deprecated helpers remain importable but emit `DeprecationWarning`s with pointers
  to the migration guide (`python/iroha_python/MIGRATION.md`).
- Fixtures that relied on `norito_py` or `iroha_torii_client` stay deterministic
  because the implementation now funnels through the canonical SDK modules.
