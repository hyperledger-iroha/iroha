<!-- Japanese translation of docs/source/samples/signed_query_iterable_find_peers.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/signed_query_iterable_find_peers.md
status: complete
translator: manual
---

# イテラブルクエリ — `FindPeers`（Start / Continue）

この例は、イテラブルクエリである `FindPeers` を `Start` と `Continue` リクエストからなる Norito `SignedQuery` フレームとして構築する方法を示します。

Start リクエスト（最初のバッチ）
```rust
use iroha_data_model::prelude::*;
use iroha_data_model::query::{
    builder::QueryExecutor,
    parameters::{FetchSize, Pagination},
    QueryRequest, QueryRequestWithAuthority, SignedQuery,
};
use iroha_crypto::KeyPair;

fn build_start_find_peers(authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    // PASS プレディケートと任意のページネーション／取得サイズを設定してクエリを構築
    let mut b = iroha::Client::new(Default::default())
        .query(iroha_data_model::query::peer::prelude::FindPeers);
    b = b.with_pagination(Pagination::new(Some(core::num::NonZeroU64::new(100).unwrap()), 0));
    b = b.with_fetch_size(FetchSize::new(Some(core::num::NonZeroU64::new(100).unwrap())));

    // ビルダーを型消去された QueryWithParams に変換
    let qwp = b.into_query_with_params();

    // Start としてラップし署名
    let req = QueryRequest::Start(qwp);
    let with_auth = QueryRequestWithAuthority { authority, request: req };
    let signed = with_auth.sign(kp);
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

Continue リクエスト
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

備考
- `Start` に対するレスポンスは `QueryResponse::Iterable(QueryOutput)` であり、`batch`、`remaining_items`、`continue_cursor`（任意）を含みます。
- 次のバッチを取得するには、返ってきた `continue_cursor` を `Continue` リクエストに渡します。
- CLI には `query stdin-raw` という利便モードがあり、base64 または 16 進表現の `SignedQuery` フレームを直接送信できます。
