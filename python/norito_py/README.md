# iroha-norito

`iroha-norito` is a pure-Python reference implementation of the
[Norito](../../norito.md) binary serialization codec used by Hyperledger Iroha.
The library mirrors the key features of the production Rust implementation so
Python tooling and tests can produce deterministic, interoperable payloads.

## Features
- Header parsing/serialization (`NRT0`, major 0 with default layout-bit minor) with CRC64-XZ validation
- Encoding/decoding primitives (bool, signed/unsigned 8–64 bit integers, IEEE-754 floats)
- Strings, bytes, options (`None`), results (`Ok`/`Err` wrappers)
- Packed and compat sequences with compact lengths and fixed u64 offsets
- Struct adapters honouring the PACKED_STRUCT/ FIELD_BITSET hybrid layout
- Optional Zstandard compression (enable the `compression` extra, requires the `zstandard` package)
- Maps and tuples built from composable adapters
- Schema hash helpers (type-name and structural) matching Rust's canonical JSON hashing
- Columnar (NCB) helpers and adaptive AoS/NCB selection for string, optional
  string/u32, raw byte, and enum row layouts

## Rust trait parity
The Python adapters track the Rust core traits `NoritoSerialize` and
`NoritoDeserialize` that back all data model types. When the Rust crate adds
ergonomic helpers via `norito::codec::{Encode, Decode}`, the Python façade
mirrors the behaviour through `encode(...)`/`decode(...)` so application code
sees the same semantics that the refreshed `norito.md` documents.

### Current scope
- Columnar helpers cover `(u64, value, bool)` rows whose value is a string,
  optional string, optional u32, raw bytes, or an enum name/code.
- Zstandard compression is optional and raises a clear error when the backend
  is missing.

## Installation
Install directly from the workspace root:

```bash
python3 -m pip install ./python/norito_py
# optional Zstandard support
python3 -m pip install ./python/norito_py[compression]
```

## Quick start
```python
from norito import SchemaDescriptor, encode, decode, seq, u32

schema = SchemaDescriptor("iroha.example.Numbers")
adapter = seq(u32())
payload = encode([1, 1, 2, 3, 5], schema, adapter)
print(payload.hex())

values = decode(payload, adapter, schema=schema)
assert values == [1, 1, 2, 3, 5]
```

The library exposes builder helpers for common Norito types (`u64()`,
`string()`, `bytes_()`, `option(...)`, etc.) that can be nested to describe
complex payloads.

## CLI
The package installs a small inspector:

```bash
$ norito-dump my_payload.to
Norito payload header:
  version: 1.0
  schema hash: 0123456789abcdef0123456789abcdef
  compression: 0
  payload length: 128
  checksum: 0xdeadbeefcafebabe
  flags: 0x13
```

## Development
Run the unit test suite:

```bash
PYTHONPATH=python/norito_py/src python3 -m unittest discover -s python/norito_py/tests
```

Before committing, format Rust code with `cargo fmt --all` (edition 2024) and ensure the Python
package passes the test suite.

### Keeping in sync with Rust Norito

CI runs `scripts/check_norito_bindings_sync.sh` to ensure changes under
`crates/norito` (or related docs) have matching updates in the Python and Java
bindings. The script now executes lightweight parity checks (schema hash and
columnar roundtrips) so regressions are caught early. Run it locally after
touching the Rust Norito sources to avoid surprises (any shell works because the helper is Python-based):

```bash
scripts/check_norito_bindings_sync.sh
# or
python3 scripts/check_norito_bindings_sync.py
```

The Rust crate `norito` also invokes this script via its build.rs so the
parity checks run automatically whenever the crate is built. Packagers that
ship a trimmed source tree can rely on the build script's detection of missing
VCS metadata to skip the check; manual overrides are no longer supported.

> Streaming manifests, control frames, and telemetry helpers are all available
> under `norito.streaming`, providing parity with the Rust Norito structures for
> manifest distribution, capability negotiation, and runtime counters.

## License

`iroha-norito` is licensed under the Apache License, Version 2.0. See
[`LICENSE`](LICENSE) for the full text.
