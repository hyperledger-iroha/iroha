# Constructing a SignedQuery for `FindAbiVersion`

This snippet shows how to build, sign, and encode a Norito `SignedQuery` that calls the core singular query `FindAbiVersion`. The resulting bytes can be POSTed to `/v1/query` or piped to the CLI `query stdin-raw`.

Steps
- Build a `SingularQueryBox::FindAbiVersion`.
- Bind the request to the exact genesis-derived `NetworkId`, creation time,
  non-zero TTL, authority, and a fresh 32-byte nonce.
- Sign with the authority’s `KeyPair` to obtain `SignedQuery`.
- Encode with `EncodeVersioned` and send the resulting bytes exactly once.

Rust example
```rust
use core::num::NonZeroU64;
use std::time::{SystemTime, UNIX_EPOCH};

use iroha_crypto::KeyPair;
use iroha_data_model::{NetworkId, prelude::*, query::QueryRequest};
use iroha_version::codec::EncodeVersioned;
use rand::{rand_core::TryRngCore as _, rngs::OsRng};

fn build_signed_query_find_abi_version(
    network_id: NetworkId,
    authority: AccountId,
    kp: &KeyPair,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // 1) Construct the singular query box
    let q = iroha_data_model::query::runtime::prelude::FindAbiVersion;
    let box_ = iroha_data_model::query::SingularQueryBox::FindAbiVersion(q);

    // 2) Bind the complete one-shot request context.
    let req = QueryRequest::Singular(box_);
    let creation_time_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let mut nonce = [0_u8; 32];
    for _ in 0..16 {
        OsRng.try_fill_bytes(&mut nonce)?;
        if nonce != [0_u8; 32] {
            break;
        }
    }
    if nonce == [0_u8; 32] {
        return Err("OS RNG repeatedly returned an all-zero query nonce".into());
    }
    let with_auth = req.with_authority(
        network_id,
        authority,
        creation_time_ms,
        NonZeroU64::new(100_000).expect("non-zero query TTL"),
        nonce,
    );

    // 3) Sign — produces SignedQuery
    let signed = with_auth.try_sign(kp)?;

    // 4) Encode the canonical versioned wire frame.
    Ok(signed.encode_versioned())
}
```

Submitting
- Raw HTTP: `POST /v1/query` with the encoded bytes as the request body.
- CLI: base64‑encode the bytes and pipe to `iroha ledger query stdin-raw`.
- Do not retry the same signed bytes after an ambiguous transport failure. Build
  a newly timestamped and nonced request only when the operation is safe to issue again.

Output
- On success, the node returns a Norito `QueryResponse::Singular(AbiVersion)`.
- The CLI prints the decoded JSON via Norito JSON wrappers.

```json
{
  "abi_version": 1
}
```
