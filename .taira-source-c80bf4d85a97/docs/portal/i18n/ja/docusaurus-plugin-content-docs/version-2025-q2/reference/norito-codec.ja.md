---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73fa18173e84c2151eb0f2b3667f82d630646237625790b60aa317341227e6fa
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8de31f9e066b729fda8324b8847badba23de926888574d02a44fb0e6d4472f77
source_last_modified: "2026-01-18T05:31:56+00:00"
translation_last_reviewed: 2026-01-30
---

# Norito Codec Reference

Norito is Iroha’s canonical serialization layer. Every on-wire message, on-disk
payload, and cross-component API uses Norito so nodes agree on identical bytes
even when they run on different hardware. This page summarises the moving parts
and points to the full specification in `norito.md`.

## Core layout

| Component | Purpose | Source |
| --- | --- | --- |
| **Header** | Frames payloads with magic/version/schema hash, CRC64, length, and compression tag; v1 requires `VERSION_MINOR = 0x00` and validates header flags against the supported mask (default `0x00`). | `norito::header` — see `norito.md` (“Header & Flags”, repository root) |
| **Bare payload** | Deterministic value encoding used for hashing/comparison. On-wire transport always uses a header; bare bytes are internal-only. | `norito::codec::{Encode, Decode}` |
| **Compression** | Optional Zstd (and experimental GPU acceleration) selected via the header compression byte. | `norito.md`, “Compression negotiation” |

The layout flag registry (packed-struct, packed-seq, field bitset, compact
lengths) lives in `norito::header::flags`. V1 defaults to flags `0x00` but
accepts explicit header flags within the supported mask; unknown bits are
rejected. `norito::header::Flags` is retained for internal inspection and
future versions.

## Derive support

`norito_derive` ships `Encode`, `Decode`, `IntoSchema`, and JSON helper derives.
Key conventions:

- Derives generate both AoS and packed code paths; v1 defaults to the AoS
  layout (flags `0x00`) unless header flags opt into packed variants.
  Implementation lives in `crates/norito_derive/src/derive_struct.rs`.
- Layout-affecting features (`packed-struct`, `packed-seq`, `compact-len`) are
  opt-in via header flags and must be encoded/decoded consistently across peers.
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
