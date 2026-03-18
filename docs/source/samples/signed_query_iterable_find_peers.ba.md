---
lang: ba
direction: ltr
source: docs/source/samples/signed_query_iterable_find_peers.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2add6de2f4578702d668dd414b092f65854f5673169870b2056d595855b4101f
source_last_modified: "2025-12-29T18:16:36.033892+00:00"
translation_last_reviewed: 2026-02-07
---

# Iterable Queries — `FindPeers` (Start / Continue)

This example shows how to construct Norito `SignedQuery` frames for an iterable query using `Start` and `Continue` requests.

Start request (First batch)
```rust
use iroha_data_model::prelude::*;
use iroha_data_model::query::{
    builder::QueryExecutor,
    parameters::{FetchSize, Pagination},
    QueryRequest, QueryRequestWithAuthority, SignedQuery,
};
use iroha_crypto::KeyPair;

fn build_start_find_peers(authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    // Build the query with an optional predicate plus pagination/fetch size
    let mut b = iroha::Client::new(Default::default())
        .query(iroha_data_model::query::peer::prelude::FindPeers);
    b = b.with_pagination(Pagination::new(Some(core::num::NonZeroU64::new(100).unwrap()), 0));
    b = b.with_fetch_size(FetchSize::new(Some(core::num::NonZeroU64::new(100).unwrap())));

    // Convert the builder into a type-erased QueryWithParams
    let qwp = b.into_query_with_params();

    // Wrap and sign as Start
    let req = QueryRequest::Start(qwp);
    let with_auth = QueryRequestWithAuthority { authority, request: req };
    let signed = with_auth.sign(kp);
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

Continue request
```rust
use iroha_data_model::query::{QueryRequest, QueryRequestWithAuthority, SignedQuery};
use iroha_crypto::KeyPair;

fn build_continue(cursor: iroha_data_model::query::parameters::ForwardCursor, authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    let req = QueryRequest::Continue(cursor);
    let with_auth = QueryRequestWithAuthority { authority, request: req };
    let signed = with_auth.sign(kp);
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

Notes
- The response to `Start` is a `QueryResponse::Iterable(QueryOutput)` which contains `batch`, `remaining_items`, and an optional `continue_cursor`.
- Feed the `continue_cursor` into the `Continue` request to fetch the next batch.
- The CLI provides a convenience mode (`query stdin-raw`) to submit base64/hex-encoded `SignedQuery` frames directly.
