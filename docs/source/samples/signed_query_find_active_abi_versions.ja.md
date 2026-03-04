<!-- Japanese translation of docs/source/samples/signed_query_find_active_abi_versions.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/signed_query_find_active_abi_versions.md
status: complete
translator: manual
---

# `FindActiveAbiVersions` 用の SignedQuery を構築する

このスニペットは、コアの単一クエリ `FindActiveAbiVersions` を呼び出す Norito `SignedQuery` を生成・署名・エンコードする手順を示します。生成されたバイト列は `/query` への POST もしくは CLI の `query stdin-raw` にパイプできます。

手順
- `SingularQueryBox::FindActiveAbiVersions` を構築する。
- `QueryRequest::Singular` と `QueryRequestWithAuthority { authority, request }` にラップする。
- 権限アカウントの `KeyPair` で署名し、`SignedQuery` を得る。
- `norito::codec::Encode::encode` でエンコードする。

Rust の例
```rust
use iroha_data_model::prelude::*;
use iroha_data_model::query::{QueryRequest, QueryRequestWithAuthority, SignedQuery};
use iroha_crypto::KeyPair;

fn build_signed_query_find_active_abi(authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    // 1) 単一クエリボックスを構築
    let q = iroha_data_model::query::runtime::prelude::FindActiveAbiVersions;
    let box_ = iroha_data_model::query::SingularQueryBox::FindActiveAbiVersions(q);

    // 2) クエリリクエストへラップし、権限情報を付与
    let req = QueryRequest::Singular(box_);
    let with_auth = QueryRequestWithAuthority { authority, request: req };

    // 3) 署名して SignedQuery を得る
    let signed = with_auth.sign(kp);

    // 4) Norito TLV でエンコード
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

送信方法
- HTTP 生リクエスト: エンコード済みバイト列をボディに入れて `POST /query`。
- CLI: バイト列を base64 に変換して `iroha ledger query stdin-raw` に渡す。

出力
- 成功するとノードは Norito の `QueryResponse::Singular(ActiveAbiVersions)` を返します。
- CLI は Norito の JSON ラッパー経由でデコード済み JSON を表示します。

```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```
