---
lang: ru
direction: ltr
source: norito.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f91db1ad9e91a6fca60c5ecbd70e7700bc1a4cbebdcbe61233dd83a03bc89f
source_last_modified: "2026-01-30T12:29:10.234437+00:00"
translation_last_reviewed: 2026-02-07
---

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
| CRC64 | 8 | CRC64-XZ (ECMA polynomial, reflected, init/xor all ones) over the payload |
| Flags | 1 | Layout flags (see below) |

Total header size: 40 bytes.

Alignment padding:
- For uncompressed payloads, encoders must insert zero padding between the
  header and payload when the archived type's alignment would otherwise be
  violated.
- Padding length must be the exact alignment padding required for the type and
  padding bytes must be zero. Decoders without a concrete type alignment must
  accept any zero padding up to 64 bytes and treat the remaining bytes as the
  payload. Extra non-zero bytes are rejected.

Schema enforcement:
- Typed decoders must reject payloads whose header schema hash does not match
  the expected type. `ArchiveView::decode` performs this check; use
  `ArchiveView::decode_unchecked` only for raw inspection tools.

## Header Flags

These flags are ORed into the final header byte. Unknown bits are rejected.

| Flag | Hex | Meaning |
| --- | --- | --- |
| `PACKED_SEQ` | `0x01` | Packed sequence layout for variable-sized collections. |
| `COMPACT_LEN` | `0x02` | Per-value length prefixes are compact varints. |
| `PACKED_STRUCT` | `0x04` | Packed struct layout for derive-generated types. |
| `VARINT_OFFSETS` | `0x08` | Reserved in v1; packed sequences always use `(len + 1)` u64 offsets. |
| `COMPACT_SEQ_LEN` | `0x10` | Reserved in v1; sequence length headers are fixed u64. |
| `FIELD_BITSET` | `0x20` | Packed-struct hybrid uses a bitset indicating which fields carry explicit sizes (requires `PACKED_STRUCT` + `COMPACT_LEN`). |

Flag scoping rules:
- `COMPACT_LEN` affects per-value length prefixes only.
- Reserved layout bits (`VARINT_OFFSETS`, `COMPACT_SEQ_LEN`) are rejected when decoding headers.

These flags are independent; no heuristic cross-effects are permitted.

## Length Prefixes

Norito uses length prefixes in multiple places, with explicit flags deciding the
encoding:

- Per-value prefixes (fields, elements, strings, blobs) use `COMPACT_LEN`.
  - If set: unsigned varint (7-bit continuation).
  - If not set: fixed 8-byte little-endian u64.
- Sequence length headers are fixed 8-byte little-endian u64 in v1.
- Packed-sequence offsets are always `(len + 1)` u64 offsets, monotonic with the
  first offset 0.

Varint encodings must fit in `u64` and use the shortest (canonical) encoding;
overflow or overlong encodings are rejected.

## String Encoding

`String` and `&str` values are encoded as:

```
[len][utf8-bytes]
```

`len` uses the per-value prefix rules above (`COMPACT_LEN`). Decoders must not
apply nested-length heuristics or reinterpret string payloads based on their
contents.

## Numeric and BigInt

`BigInt` encodes as:

```
[len_u32][twos_complement_le_bytes]
```

`len_u32` is a 4-byte little-endian length of the following payload. The bytes
are little-endian two's complement and must fit within the 512-bit cap.

`Numeric` encodes as a struct `(mantissa, scale)`:
- `mantissa` is a `BigInt` containing the raw integer value (no decimal scale
  is embedded in the integer).
- `scale` is a `u32` count of fractional digits (e.g., `1.88` is mantissa `188`,
  scale `2`).

## Map Encoding

Maps encode deterministically with the same active layout flags:

- Entry count uses a fixed 8-byte little-endian u64 header.
- Compat layout (`PACKED_SEQ` unset): for each entry,
  `[key_len][key_payload][value_len][value_payload]` with key/value lengths
  encoded via `COMPACT_LEN`.
- Packed layout (`PACKED_SEQ` set): key sizes and value sizes precede the data,
  followed by concatenated key payloads and concatenated value payloads. Uses
  `(len + 1)` u64 offsets for keys, then `(len + 1)` u64 offsets for values;
  offsets are monotonic with the first offset 0.
- `HashMap` encodes entries in sorted key order for deterministic output;
  `BTreeMap` uses its natural ordering.

## NCB Columnar (internal)

NCB payloads are exact and canonical:
- Alignment padding between NCB columns must be zero-filled.
- Bitset padding bits (flags and presence) must be zero.
- Trailing bytes after the NCB payload are rejected.

## AoS Ad-hoc (Adaptive Columnar)

The `norito::aos` helpers used by adaptive columnar encoders follow the same
length prefix rules and honor the active `COMPACT_LEN` flag, so embedded AoS
payloads stay consistent with their parent Norito headers.

## Packed-Struct Layout

When the `PACKED_STRUCT` flag is set, derive-generated structs/tuples are
encoded as a single packed payload with one of two layouts:

- Compat packed-struct (no `FIELD_BITSET`): `(field_count + 1)` little-endian
  `u64` offsets followed by concatenated field payloads. Offsets start at 0,
  are cumulative byte lengths of each field payload in declaration order, and
  the final offset equals the total data length. Offsets are fixed-width even
  when `COMPACT_LEN` is enabled.
- Hybrid packed-struct (`FIELD_BITSET` + `COMPACT_LEN`): a bitset of length
  `ceil(field_count / 8)` bytes, followed by size prefixes for fields whose
  bit is set (varint-encoded per `COMPACT_LEN`), followed by concatenated field
  payloads in declaration order. Bit 0 of byte 0 refers to field 0, bit 1 to
  field 1, and so on. Fields that are fixed-size or self-delimiting omit the
  explicit size header and are decoded sequentially.

Field payloads themselves use the active layout flags (e.g., `PACKED_SEQ`,
`COMPACT_LEN`) when encoding nested collections or string/blob values.

## Compression Selection and Validation

The header `Compression` byte identifies the payload encoding:

- `0 = None`: payload bytes follow the header (with optional alignment padding).
- `1 = Zstd`: payload bytes are compressed with Zstandard.

`Payload length` and `CRC64` always describe the uncompressed payload. For
compressed payloads, the encoded byte stream begins immediately after the
header with no alignment padding. Decoders must reject unknown compression
values or unsupported algorithms; builds without the `compression` feature
accept only `None`.

Encoders choose compression explicitly (`to_compressed_bytes`) or via the
adaptive helper (`to_bytes_auto`) that applies deterministic heuristics. The
chosen algorithm is recorded in the header; there is no on-wire negotiation.

## Schema Hash Details

The 16-byte schema hash is computed as:

- Default: FNV-1a 64-bit hash of the fully qualified type name (Rust
  `core::any::type_name::<T>()`), duplicated to fill 16 bytes.
- With `schema-structural`: canonical JSON schema produced by
  `iroha_schema::IntoSchema`, serialized with Norito’s JSON writer and hashed
  with the same FNV-1a routine.

Typed decoders must reject payloads whose header schema hash does not match the
expected type. `ArchiveView::decode` enforces this check; `decode_unchecked`
is reserved for tooling that explicitly opts out of schema validation.
