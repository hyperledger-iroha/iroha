# Iterable Queries — `FindPeers` (Start / Continue)

This example shows how to construct Norito `SignedQuery` frames for an iterable
query using `Start` and `Continue` requests. Every frame needs its own creation
time and nonce; an exact signed body is never retried.

Start request (First batch)
```rust
use core::num::NonZeroU64;
use iroha_data_model::{NetworkId, prelude::*, query::{QueryRequest, QueryWithParams}};
use iroha_crypto::KeyPair;
use iroha_version::codec::EncodeVersioned;

fn build_start_find_peers(
    qwp: QueryWithParams,
    network_id: NetworkId,
    authority: AccountId,
    creation_time_ms: u64,
    nonce: [u8; 32],
    kp: &KeyPair,
) -> Result<Vec<u8>, iroha_crypto::Error> {
    let req = QueryRequest::Start(qwp);
    let signed = req
        .with_authority(
            network_id,
            authority,
            creation_time_ms,
            NonZeroU64::new(100_000).expect("non-zero query TTL"),
            nonce,
        )
        .try_sign(kp)?;
    Ok(signed.encode_versioned())
}
```

Construct `qwp` with the typed query builder (`FindPeers`, pagination, fetch
size, predicate, and selector) before calling this wire helper. Production
callers should normally let `iroha::Client` perform both construction and
fresh-context signing.

Continue request
```rust
use core::num::NonZeroU64;
use iroha_data_model::{NetworkId, prelude::*, query::QueryRequest};
use iroha_crypto::KeyPair;
use iroha_version::codec::EncodeVersioned;

fn build_continue(
    cursor: iroha_data_model::query::parameters::ForwardCursor,
    network_id: NetworkId,
    authority: AccountId,
    creation_time_ms: u64,
    nonce: [u8; 32],
    kp: &KeyPair,
) -> Result<Vec<u8>, iroha_crypto::Error> {
    let req = QueryRequest::Continue(cursor);
    let signed = req
        .with_authority(
            network_id,
            authority,
            creation_time_ms,
            NonZeroU64::new(100_000).expect("non-zero query TTL"),
            nonce,
        )
        .try_sign(kp)?;
    Ok(signed.encode_versioned())
}
```

Notes
- The response to `Start` is a `QueryResponse::Iterable(QueryOutput)` which contains `batch`, `remaining_items`, and an optional `continue_cursor`.
- Feed the `continue_cursor` into the `Continue` request to fetch the next batch.
- Generate a new operating-system nonce and creation time for every `Continue` request.
- The CLI provides a convenience mode (`query stdin-raw`) to submit base64/hex-encoded `SignedQuery` frames directly.
