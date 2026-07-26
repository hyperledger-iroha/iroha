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
- **Flags** – the codec honours `PACKED_SEQ`, `COMPACT_LEN`, `PACKED_STRUCT`,
  and `FIELD_BITSET`. By default all optional optimisations are disabled (so
  `norito.DEFAULT_FLAGS` is `0`), matching the canonical Rust layout policy; reserved
  layout bits are rejected during decode.
- **Header version** – v1 fixes the minor byte to `0x00`; decoders reject minor
  mismatches and unknown flag bits. Non-zero header flags are allowed only when
  explicitly set and within the supported mask.

## High-Level API
- `norito.encode(value, schema, adapter, flags=None)` returns a Norito payload
  (`header + payload`) as bytes.
- `norito.decode(data, adapter, schema=None)` validates the header, CRC, and
  flags before decoding the payload into Python objects.

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

## Roadmap
- Structural schema hashing (iroha_schema-compatible)
- Columnar (NCB) helpers and streaming decode APIs
- Compression benchmarks and user-configurable policy knobs

## License

The Python implementation is distributed under the Apache License, Version 2.0
(`LICENSE`).
