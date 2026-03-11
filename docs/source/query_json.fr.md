---
lang: fr
direction: ltr
source: docs/source/query_json.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5d0e6ac8f8bcb6d9b4f3bca2496c1dd9121114f1ca3099d22baecf88696439
source_last_modified: "2026-01-22T15:38:30.682097+00:00"
translation_last_reviewed: 2026-01-30
---

# Query JSON Envelope

Iroha exposes a Norito-based `/query` endpoint that accepts signed frames. For
interactive tooling (CLI, scripting) it is convenient to author the request as
JSON and let the tooling convert it into a signed `SignedQuery`. The
`iroha_data_model::query::json` module defines the canonical envelope used by
`iroha_cli ledger query stdin` and other utilities.

## Envelope shape

The top-level document is an object containing either a `singular` or
`iterable` section:

```json
{"singular": { /* singular query */ }}
{"iterable": { /* iterable query */ }}
```

Submissions containing both sections, or neither, are rejected.

## Singular queries

Singular requests identify the query by name and optionally include a payload:

```json
{
  "singular": {
    "type": "FindContractManifestByCodeHash",
    "payload": {
      "code_hash": "0x00112233…"
    }
  }
}
```

The following singular queries are supported:

- `FindActiveAbiVersions`
- `FindExecutorDataModel`
- `FindParameters`
- `FindContractManifestByCodeHash` (requires a 32-byte `code_hash` hex string)

## Iterable queries

Iterable requests identify the query and may carry optional execution modifiers
and a predicate payload:

```json
{
  "iterable": {
    "type": "FindDomains",
    "params": {
      "limit": 25,
      "offset": 10,
      "fetch_size": 50,
      "sort_by_metadata_key": "ui.order",
      "order": "Desc",
      "ids_projection": false,
      "lane_id": null,   // reserved for future cursor lanes (TBD)
      "dsid": null       // reserved for future data shard identifiers (TBD)
    },
    "predicate": {
      "equals": [
        {"field": "authority", "value": "i105..."}
      ],
      "in": [
        {"field": "metadata.tier", "values": [1, 2, 3]}
      ],
      "exists": ["metadata.display_name"]
    }
  }
}
```

### Parameters

The optional `params` object configures pagination and sorting:

- `limit` (`u64`, optional) — maximum total items to fetch.
- `offset` (`u64`, default `0`) — number of items to skip.
- `fetch_size` (`u64`, optional) — batch size for cursor streaming.
- `sort_by_metadata_key` (`string`, optional) — metadata key used for
  stable sorting.
- `order` (`"Asc" | "Desc"`, optional) — sort order; only accepted when a
  metadata key is provided.
- `ids_projection` (`bool`, optional) — request id-only responses when the
  node is built with the experimental `ids_projection` feature.
- `lane_id`, `dsid` (optional) — reserved fields for upcoming cursor lane and
  data-shard routing support. They are accepted by the parser but currently
  ignored (TBD).

All numeric limits must be non-zero when provided. The sort key is validated
using the canonical [`Name`](../../crates/iroha_data_model/src/name.rs) rules.

### Predicate mini DSL

The predicate payload is represented as an object with three optional arrays:

- `equals`: list of `{ "field": <path>, "value": <json value> }` entries.
- `in`: list of `{ "field": <path>, "values": [<json value>, …] }` entries
  with non-empty value lists.
- `exists`: list of field paths that must be present (non-null).

Field paths use dotted notation (`metadata.display_name`, `authority`, etc.).
The encoder canonicalises the predicate by sorting sections by field name to
ensure deterministic signatures.

## CLI usage

The CLI reads the envelope from stdin, signs the request with the configured
account, and submits it to `/query`:

```shell
$ cargo run -p iroha_cli -- query stdin <<'JSON'
{
  "iterable": {
    "type": "FindDomains",
    "params": {"limit": 5, "sort_by_metadata_key": "ui.order", "order": "Asc"},
    "predicate": {"exists": ["metadata.display_name"]}
  }
}
JSON
```

The response is printed using the configured output format. The same envelope
can be converted into a signed frame programmatically via
`QueryEnvelopeJson::into_signed_request`. See
[`iroha_data_model::query::json`](../../crates/iroha_data_model/src/query/json)
for the full Rust API.
