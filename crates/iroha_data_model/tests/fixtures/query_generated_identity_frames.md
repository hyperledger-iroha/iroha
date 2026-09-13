# Generated query identity capture

`query_generated_identity_frames.json` was captured on 2026-09-07 before
`queries!` acquired an identity derive and its 127 declarations acquired
explicit names. The current catalog adds `FindSettlementReceiptById`, captured
with the native codec on 2026-09-12. It records 172 actual payloads and their JSON plus complete
root, `Vec`, `Option` and `BTreeMap<u8, T>` frames: 688 frames in total.
The catalog covers unit queries, domainless account identifiers, both alias
scopes, present/absent pagination anchors and every query-filter enum variant.
Every JSON value and every frame is roundtripped during capture and checking.

Fixture SHA-256:
`6143df7829bba7d739e5856249cb5b91016791901a16a068aa500b5e018938a8`.
The unchanged production sources during capture were:

| Source | SHA-256 |
| --- | --- |
| `src/query/mod.rs` | `e31c36f6bc454db0c99f098e6dd0b52c1aa9b52bde1d3c4d7df0ff829778ff1d` |
| `src/query/domain_queries.rs` | `d7c61174e88b0095592e48203279f4e87d7bc4035b10472d69bb50f8bf13406a` |

All 127 actual compiler names and both directional hashes also match the
earlier controlled model capture's generated-family review, SHA-256
`2e00a0c82cc36b5b14d97586bd392f4563fde0de303b7414724a2822a29f2778`.
The literals retain the captured private `model` module components; they were
not reconstructed from public imports or physical source paths.

The one-time writer was removed before post-declaration execution. The
permanent `generated_queries_preserve_captured_frames` test compares the
explicit identity, both codec hashes, every JSON value and every frame with
this immutable fixture. The focused query suite passes 28 tests locally.
This prepares declarations for the model split; it does not change active
codec dispatch or qualify other features, FFI or a release candidate.

The current inventory is 128 query types. The receipt-query capture roundtrips
canonical JSON and all four frame contexts and rejects a substituted schema
header. It does not establish live query or settlement execution.
