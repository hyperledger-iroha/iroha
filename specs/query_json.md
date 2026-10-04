# Query JSON Envelope

Iroha exposes a Norito-based `/v1/query` endpoint that accepts signed frames. For
interactive tooling (CLI, scripting) it is convenient to author the request as
JSON and let the tooling convert it into a signed `SignedQuery`. The
`iroha_data_model::query::json` module defines the canonical envelope used by
`iroha_cli ledger query stdin` and other utilities.

Signed `/v1/query` is not the listing API. It admits singular queries and four
unfiltered iterable shapes (see [Admission](#admission-on-signed-v1query)).
Read domains, accounts, asset definitions, NFTs, RWA lots, account assets,
asset holders, transactions and repo agreements through the
[Torii collection endpoints](torii/collection_queries.md).

This authoring envelope is not the signed wire payload. Before submission, the
client binds the exact genesis-derived `network_id`, authority, Unix creation
time, non-zero TTL, a fresh 32-byte nonce, and the complete query request into
`QueryRequestWithAuthority`, then signs all six fields. The client configuration
must therefore contain the exact `network_id` for the target deployment.

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
    "type": "FindContractManifestByArtifactId",
    "payload": {
      "artifact_id": {
        "dataspace_id": 18446744073709551615,
        "code_hash": "hash:BAF171AF0123F8A6C0BFAD9A4CA03A80C678DA21355E484320E2E5C667408D2F#0BB7"
      }
    }
  }
}
```

The artifact identity binds the complete `.to` image to one exact dataspace;
zero explicitly selects the universal dataspace. Use the artifact's actual
domain-separated hash in place of the illustrative value above. The signed
query context independently binds the network.

The following singular queries are supported:

- `FindAbiVersion`
- `FindExecutorDataModel`
- `FindParameters`
- `FindAssetDefinitionById` with `{ "asset": "<base58-asset-definition-id>" }`
- `FindAssetById` with `{ "asset": "<base58-asset-definition-id>", "account_id": "<canonical-i105>", "scope": { "kind": "Global" } }`
- `FindContractManifestByArtifactId` (requires `artifact_id` with an explicit unsigned 64-bit `dataspace_id` and a canonical checksummed Norito `code_hash` literal)

Example singular asset-definition lookup:

```json
{
  "singular": {
    "type": "FindAssetDefinitionById",
    "payload": {
      "asset": "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
    }
  }
}
```

Example singular owned-asset lookup:

```json
{
  "singular": {
    "type": "FindAssetById",
    "payload": {
      "asset": "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
      "account_id": "<i105-account-id>",
      "scope": {
        "kind": "Global"
      }
    }
  }
}
```

## Iterable queries

Iterable requests identify the query and may carry optional execution modifiers
and a predicate payload. The envelope supports selected typed iterable queries,
including all four sources admitted by signed `/v1/query`. Execution admits
only the shapes listed under
[Admission](#admission-on-signed-v1query):

```json
{
  "iterable": {
    "type": "FindAccountIds",
    "params": {
      "limit": 25,
      "fetch_size": 50
    }
  }
}
```

### Admission on signed `/v1/query`

Torii admits these iterable starts, each with no predicate (the pass
predicate), bounded counting (the default `count_mode`), a zero `offset` and no
`sort_by_metadata_key`:

| Query | Cursor modes |
| --- | --- |
| `FindPeers` | ephemeral only |
| `FindAccountIds` | ephemeral and stored |
| `FindTriggers` | ephemeral and stored |
| `FindActiveTriggerIds` | ephemeral and stored |

Every other iterable query, and any of these with a predicate, an offset,
metadata sorting, exact counting or (for `FindPeers`) stored cursor mode, is
refused before execution with HTTP 400 `query_validation_failed`. The message
starts with the stable reason `signed_query_shape_not_admitted`, names the
query and the refused modifier, and lists the collection endpoints:

```text
signed_query_shape_not_admitted: FindDomains has no bounded signed-query source. Signed POST /v1/query admits singular queries and FindPeers, FindAccountIds, FindTriggers and FindActiveTriggerIds starts with a pass predicate, bounded counting, zero offset and no sorting (FindPeers in ephemeral cursor mode only). Read listings through the Torii collection endpoints: /v1/domains, /v1/accounts, /v1/assets/definitions, /v1/nfts, /v1/rwas, /v1/accounts/{id}/assets, /v1/assets/{definition}/holders, /v1/transactions/query, /v1/repo/agreements.
```

The collection endpoints provide filters, sorting, projections and keyset
cursors for those listings; see
[Torii collection queries](torii/collection_queries.md).

### Parameters

The optional `params` object configures pagination and sorting:

- `limit` (`u64`, optional) — maximum total items to fetch.
- `offset` (`u64`, default `0`) — number of items to skip.
- `fetch_size` (`u64`, optional) — batch size for cursor streaming.
- `sort_by_metadata_key` (`string`, optional) — metadata key used for
  stable sorting.
- `order` (`"Asc" | "Desc"`, optional) — sort order; only accepted when a
  metadata key is provided.

All numeric limits must be non-zero when provided. The sort key is validated
using the canonical [`Name`](../crates/iroha_data_model/src/name.rs) rules.
Any other member, including `lane_id`, `dsid` and `ids_projection`, is rejected
as an unknown field. Iterable queries always
return whole items: the selector has a single data-free layout and never projects.

Signed `/v1/query` refuses a non-zero `offset` and any `sort_by_metadata_key`,
as described under [Admission](#admission-on-signed-v1query).

### Predicate mini DSL

Signed `/v1/query` refuses iterable starts that carry a predicate; filter
listings through the collection endpoints instead. The predicate payload is
represented as an object with three optional arrays:

- `equals`: list of `{ "field": <path>, "value": <json value> }` entries.
- `in`: list of `{ "field": <path>, "values": [<json value>, …] }` entries
  with non-empty value lists.
- `exists`: list of field paths that must be present (non-null).

Field paths use dotted notation (`metadata.display_name`, `authority`, etc.).
The encoder canonicalises the predicate by sorting sections by field name to
ensure deterministic signatures.

## CLI usage

The CLI reads the envelope from stdin, signs the request with the configured
account, and submits it to `/v1/query`:

```shell
$ cargo run -p iroha_cli -- ledger query stdin <<'JSON'
{
  "iterable": {
    "type": "FindAccountIds",
    "params": {"limit": 5}
  }
}
JSON
```

Filtered or sorted listings use the CLI collection commands, for example
`iroha ledger domain list --filter 'exists(metadata.display_name)' --sort -id`
(see [Torii collection queries](torii/collection_queries.md)).

The response is printed using the configured output format. Programmatic
callers convert the envelope into a raw `QueryRequest` with
`QueryEnvelopeJson::into_request`, then pass that request to
`Client::execute_query_request`. The client supplies the configured network
identity, current creation time, bounded lifetime, and a fresh operating-system
random nonce before signing. See
[`iroha_data_model::query::json`](../crates/iroha_data_model/src/query/json)
for the JSON conversion API.

Signed query frames are one-shot. Neither the CLI nor SDK clients resend an
identical nonce-bearing body after a timeout, connection loss, or malformed
response because the node may already have admitted it. A caller that chooses
to issue the read again must construct and sign a new timestamp and nonce.
