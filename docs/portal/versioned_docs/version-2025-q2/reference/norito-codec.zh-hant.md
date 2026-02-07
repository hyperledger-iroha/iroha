---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
---

# Norito Codec Reference

Norito is Iroha’s canonical serialization layer. Every on-wire message, on-disk
payload, and cross-component API uses Norito so nodes agree on identical bytes
even when they run on different hardware. This page summarises the moving parts
and points to the full specification in `norito.md`.

## Core layout

| Component | Purpose | Source |
| --- | --- | --- |
| **Header** | Negotiates features (packed structs/sequences, compact lengths, compression flags) and embeds a CRC64 checksum so payload integrity is checked before decode. | `norito::header` — see `norito.md` (“Header & Flags”, repository root) |
| **Bare payload** | Deterministic value encoding used for hashing/comparison. The same layout is wrapped by the header for transport. | `norito::codec::{Encode, Decode}` |
| **Compression** | Optional Zstd (and experimental GPU acceleration) activated via the `compression` flag byte. | `norito.md`, “Compression negotiation” |

The full flag registry (packed-struct, packed-seq, compact lengths, compression)
lives in `norito::header::flags`. `norito::header::Flags` exposes convenience
checks for runtime inspection; reserved layout bits are rejected by decoders.

## Derive support

`norito_derive` ships `Encode`, `Decode`, `IntoSchema`, and JSON helper derives.
Key conventions:

- Structs/enums derive packed layouts when the `packed-struct` feature is
  enabled (default). Implementation lives in `crates/norito_derive/src/derive_struct.rs`
  and the behaviour is documented in `norito.md` (“Packed layouts”).
- Packed collections use fixed-width sequence headers and offsets in v1; only
  per-value length prefixes are affected by `COMPACT_LEN`.
- JSON helpers (`norito::json`) provide deterministic Norito-backed JSON for
  open APIs. Use `norito::json::{to_json_pretty, from_json}` — never `serde_json`.

## Multicodec & identifier tables

Norito keeps its multicodec assignments in `norito::multicodec`. The reference
table (hashes, key types, payload descriptors) is maintained in `multicodec.md`
at the repository root. When a new identifier is added:

1. Update `norito::multicodec::registry`.
2. Extend the table in `multicodec.md`.
3. Regenerate downstream bindings (Python/Java) if they consume the map.

## Regenerating docs & fixtures

With the portal currently hosting a prose summary, use the upstream Markdown
sources as the source of truth:

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

When the Docusaurus automation goes live, the portal will be updated via a
sync script (tracked in `docs/portal/scripts/`) that pulls the data from these
files. Until then, keep this page aligned manually whenever the spec changes.
