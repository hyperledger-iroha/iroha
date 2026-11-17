# iroha-norito Documentation

## Overview
`iroha-norito` provides the core building blocks required to parse and emit
Norito payloads from Python. The API mirrors the structure of the Rust codec by
exposing composable adapters that describe how values should be encoded.

## Core Concepts
- **SchemaDescriptor** – wraps the canonical type path used to compute the 16
  byte schema hash embedded in Norito headers.
- **Type adapters** – lightweight objects that teach the encoder and decoder how
  to serialize a particular shape. The module ships factory helpers such as
  `u64()`, `i64()`, `string()`, `option(inner)`, `seq(inner)`,
  `map_adapter(key, value)`, `StructAdapter([...])`, and `tuple_adapter(...)`.
- **Flags** – minor 0 accepts `COMPACT_LEN` (`0x02`) for per-value varint
  lengths, or `0` for fixed-width lengths. Python's existing `DEFAULT_FLAGS = 0`
  remains unchanged. Dense (minor 1) requires flags `0` and always uses shortest
  varints for lengths and counts.
- **Header version** – `WireLayout.CURRENT` selects minor `0` and
  `WireLayout.DENSE` selects minor `1`. Framed decoding reads the layout only
  from that byte; an optional expected `layout=` rejects a different minor.
  Dense has no alignment padding. Unknown minors and trailing bytes are rejected.

## High-Level API
- `norito.encode(value, schema, adapter, flags=0, layout=WireLayout.CURRENT)` returns a Norito payload
  (`header + payload`) as bytes.
- `norito.decode(data, adapter, schema=None, layout=None)` validates the header, CRC, and
  flags before decoding the payload into Python objects.

Bare `encode_adaptive`/`decode_adaptive` calls must receive `layout=WireLayout.DENSE`
for dense bytes; no payload heuristic selects a layout. Composite adapters use
canonical field prefixes in dense. Their `framed=True` schema option also selects
Rust-derived field prefixes in minor 0 while preserving existing unframed defaults.
See the [README](../README.md#dense-layout) for fixed-array descriptors and examples.

## Example: Map of Account Balances
```python
from norito import SchemaDescriptor, encode, decode, map_adapter, string, u64

schema = SchemaDescriptor("iroha.demo.AccountBalances")
adapter = map_adapter(string(), u64())
payload = encode({"alice": 10, "bob": 20}, schema, adapter)
restored = decode(payload, adapter, schema=schema)
assert restored == {"alice": 10, "bob": 20}
```

## CLI Usage
Inspect a payload header without decoding the body:

```bash
norito-dump path/to/payload.to
```

## Current scope

The package includes structural schema descriptors, columnar helpers, streaming
decode APIs, and optional compression support. Layout flags and compression
policies are always explicit; decoders do not infer wire layouts.

## License

The Python implementation is distributed under the Apache License, Version 2.0
(`LICENSE`).
