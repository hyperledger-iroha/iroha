---
lang: ru
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/version-2025-q2/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 77b83b94386b1b8f0be8f386e10b87e36e5a8f3aa93c508b23d5603d14852a68
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-01-30
---

# Norito Overview

Norito is the binary serialization layer used across Iroha: it defines how data
structures are encoded on the wire, persisted on disk, and exchanged between
contracts and hosts. Every crate in the workspace relies on Norito instead of
`serde` so peers on different hardware produce identical bytes.

This overview summarises the core pieces and links to the canonical references.

## Architecture at a glance

- **Header + payload** – Each Norito message begins with a feature-negotiation
  header (flags, checksum) followed by the bare payload. Packed layouts and
  compression are negotiated via header bits.
- **Deterministic encoding** – `norito::codec::{Encode, Decode}` implement the
  bare encoding. The same layout is reused when wrapping payloads in headers so
  hashing and signing remain deterministic.
- **Schema + derives** – `norito_derive` generates `Encode`, `Decode`, and
  `IntoSchema` implementations. Packed structs/sequences are enabled by default
  and documented in `norito.md`.
- **Multicodec registry** – Identifiers for hashes, key types, and payload
  descriptors live in `norito::multicodec`. The authoritative table is
  maintained in `multicodec.md`.

## Tooling

| Task | Command / API | Notes |
| --- | --- | --- |
| Inspect header/sections | `ivm_tool inspect <file>.to` | Shows ABI version, flags, and entrypoints. |
| Encode/decode in Rust | `norito::codec::{Encode, Decode}` | Implemented for all core data-model types. |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | Deterministic JSON backed by Norito values. |
| Generate docs/specs | `norito.md`, `multicodec.md` | Source-of-truth documentation in the repo root. |

## Development workflow

1. **Add derives** – Prefer `#[derive(Encode, Decode, IntoSchema)]` for new data
   structures. Avoid hand-written serializers unless absolutely necessary.
2. **Validate packed layouts** – Use `cargo test -p norito` (and the packed
   feature matrix in `scripts/run_norito_feature_matrix.sh`) to ensure new
   layouts remain stable.
3. **Regenerate docs** – When the encoding changes, update `norito.md` and the
   multicodec table, then refresh the portal pages (`/reference/norito-codec`
   and this overview).
4. **Keep tests Norito-first** – Integration tests should use the Norito JSON
   helpers instead of `serde_json` so they exercise the same paths as production.

## Quick links

- Specification: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec assignments: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Feature matrix script: `scripts/run_norito_feature_matrix.sh`
- Packed-layout examples: `crates/norito/tests/`

Pair this overview with the quickstart guide (`/norito/getting-started`) for a
hands-on walkthrough of compiling and running bytecode that uses Norito
payloads.
