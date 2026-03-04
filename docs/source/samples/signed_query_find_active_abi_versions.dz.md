---
lang: dz
direction: ltr
source: docs/source/samples/signed_query_find_active_abi_versions.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ad22eb3cbdc3a118add29721651aa7819bcd08621df3783ac0752a8591029d0
source_last_modified: "2026-01-22T14:35:37.771710+00:00"
translation_last_reviewed: 2026-02-07
---

# Constructing a SignedQuery for `FindActiveAbiVersions`

This snippet shows how to build, sign, and encode a Norito `SignedQuery` that calls the core singular query `FindActiveAbiVersions`. The resulting bytes can be POSTed to `/query` or piped to the CLI `query stdin-raw`.

Steps
- Build a `SingularQueryBox::FindActiveAbiVersions`.
- Wrap into `QueryRequest::Singular` and `QueryRequestWithAuthority { authority, request }`.
- Sign with the authority’s `KeyPair` to obtain `SignedQuery`.
- Encode with `norito::codec::Encode::encode`.

Rust example
```rust
use iroha_data_model::prelude::*;
use iroha_data_model::query::{QueryRequest, QueryRequestWithAuthority, SignedQuery};
use iroha_crypto::KeyPair;

fn build_signed_query_find_active_abi(authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    // 1) Construct the singular query box
    let q = iroha_data_model::query::runtime::prelude::FindActiveAbiVersions;
    let box_ = iroha_data_model::query::SingularQueryBox::FindActiveAbiVersions(q);

    // 2) Wrap as a query request and attach authority
    let req = QueryRequest::Singular(box_);
    let with_auth = QueryRequestWithAuthority { authority, request: req };

    // 3) Sign — produces SignedQuery
    let signed = with_auth.sign(kp);

    // 4) Encode Norito TLV
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

Submitting
- Raw HTTP: `POST /query` with the encoded bytes as the request body.
- CLI: base64‑encode the bytes and pipe to `iroha ledger query stdin-raw`.

Output
- On success, the node returns a Norito `QueryResponse::Singular(ActiveAbiVersions)`.
- The CLI prints the decoded JSON via Norito JSON wrappers.

```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```
