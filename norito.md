# Norito Format (v1)

This document is the source of truth for Norito's on-wire encoding in the
Iroha workspace. It defines the header, flags, and the canonical length and
string layouts used across components.

## Header

The Norito header is always present on wire and on disk. It frames the payload
and supplies the schema hash and checksum needed for deterministic decoding.

| Field | Size (bytes) | Notes |
| --- | --- | --- |
| Magic | 4 | ASCII `NRT0` |
| Major | 1 | `VERSION_MAJOR = 0` |
| Minor | 1 | `VERSION_MINOR = 0x00` |
| Schema hash | 16 | FNV-1a hash of fully qualified type name (v1) |
| Compression | 1 | `0 = None`, `1 = Zstd` |
| Payload length | 8 | Uncompressed payload length (u64, little-endian) |
| CRC64 | 8 | CRC64-ECMA over the payload |
| Flags | 1 | Layout flags (see below) |

Total header size: 40 bytes.

## Header Flags

These flags are ORed into the final header byte. Unknown bits are rejected.

| Flag | Hex | Meaning |
| --- | --- | --- |
| `PACKED_SEQ` | `0x01` | Packed sequence layout for variable-sized collections. |
| `COMPACT_LEN` | `0x02` | Per-value length prefixes are compact varints. |
| `PACKED_STRUCT` | `0x04` | Packed struct layout for derive-generated types. |
| `VARINT_OFFSETS` | `0x08` | When `PACKED_SEQ` is set, element lengths are varint-coded; otherwise packed sequences use `(len + 1)` u64 offsets. |
| `COMPACT_SEQ_LEN` | `0x10` | The outer sequence length header is a compact varint. |
| `FIELD_BITSET` | `0x20` | Packed-struct hybrid uses a bitset indicating which fields carry explicit sizes (requires `PACKED_STRUCT` + `COMPACT_LEN`). |

Flag scoping rules:
- `COMPACT_LEN` affects per-value length prefixes only.
- `COMPACT_SEQ_LEN` affects only the outer sequence length header.
- `VARINT_OFFSETS` affects only packed-sequence offsets.

These flags are independent; no heuristic cross-effects are permitted.

## Length Prefixes

Norito uses length prefixes in multiple places, with explicit flags deciding the
encoding:

- Per-value prefixes (fields, elements, strings, blobs) use `COMPACT_LEN`.
  - If set: unsigned varint (7-bit continuation).
  - If not set: fixed 8-byte little-endian u64.
- Sequence length headers use `COMPACT_SEQ_LEN`.
  - If set: unsigned varint.
  - If not set: fixed 8-byte little-endian u64.
- Packed-sequence offsets use `VARINT_OFFSETS`.
  - If set: `len` varints representing element sizes, followed by concatenated
    data.
  - If not set: `(len + 1)` u64 offsets, monotonic with the first offset 0.

## String Encoding

`String` and `&str` values are encoded as:

```
[len][utf8-bytes]
```

`len` uses the per-value prefix rules above (`COMPACT_LEN`). Decoders must not
apply nested-length heuristics or reinterpret string payloads based on their
contents.

## AoS Ad-hoc (Adaptive Columnar)

The `norito::aos` helpers used by adaptive columnar encoders follow the same
length prefix rules, but the choice is compile-time `compact-len` only and does
not depend on Norito header flags.

TODO: expand with packed-struct layout, map/set encoding, compression
negotiation, and schema hash details.
