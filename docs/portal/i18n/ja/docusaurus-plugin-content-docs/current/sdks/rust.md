---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Rust SDK クイックスタート

Rust クライアント API は `iroha` クレートにあり、Torii と通信する `client::Client` 型を提供します。トランザクション送信、イベント購読、状態問い合わせを Rust アプリから行うときに使用します。

## 1. クレートを追加

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

workspace の例では `client` フィーチャでクライアントモジュールを有効化します。公開クレートを使う場合は `path` 属性を現在のバージョン文字列に置き換えてください。

## 2. クライアントを設定

```rust title="src/main.rs"
use iroha::client::{Client, ClientConfiguration};

fn main() -> eyre::Result<()> {
 let cfg = ClientConfiguration {
 torii_url: "http://127.0.0.1:8080".parse()?,
 telemetry_url: Some("http://127.0.0.1:8080".parse()?),
 // account_id, key_pair and other options can be populated here or via helper builders
 ..ClientConfiguration::default()
 };

 let client = Client::new(cfg)?;
 println!("Node status: {:?}", client.get_status()?);
 Ok(())
}
```

`ClientConfiguration` は CLI の設定ファイルを反映します。Torii とテレメトリの URL、認証情報、タイムアウト、バッチ設定を含みます。

## 3. トランザクションを送信

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::{
 isi::prelude::*,
 prelude::{AccountId, ChainId, DomainId, Name},
};
use iroha_crypto::{KeyPair, PublicKey};

fn submit_example() -> eyre::Result<()> {
 let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
 let account_id = AccountId::new(
 Name::from_str("alice")?,
 DomainId::from_str("wonderland")?,
 );

 let key_pair = KeyPair::generate_ed25519(); // replace with a persistent key in real apps

 let cfg = ClientConfiguration {
 chain: chain_id.clone(),
 account: account_id.clone(),
 key_pair: key_pair.clone(),
 ..ClientConfiguration::test()
 };

 let client = Client::new(cfg)?;

 let instruction = Register {
 object: Domain::new(Name::from_str("research")?, None),
 };

 let tx = client.build_transaction([instruction]);
 let signed = tx.sign(&key_pair)?;
 let hash = client.submit_transaction(&signed)?;
 println!("Submitted transaction: {hash}");
 Ok(())
}
```

内部ではクライアントが Norito を使ってペイロードをエンコードし Torii に送信します。送信が成功すると返されるハッシュを `client.poll_transaction_status(hash)` で追跡できます。

## 4. DA blob を送信

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha::da::DaIngestParams;
use iroha_data_model::{da::types::ExtraMetadata, nexus::LaneId};

fn submit_da_blob() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let mut params = DaIngestParams::default();
 params.lane_id = LaneId::new(7);
 params.epoch = 42;
 let payload = std::fs::read("payload.car")?;
 let metadata = ExtraMetadata::default();
 let result = client.submit_da_blob(payload, &params, metadata, None)?;
 println!(
 "status={} duplicate={} bytes={}",
 result.status, result.duplicate, result.payload_len
 );
 Ok(())
}
```

Torii に送信せずに Norito ペイロードを検査・保存したい場合は `client.build_da_ingest_request(...)` を呼び出し、署名済みリクエストを JSON/bytes にレンダリングします。`iroha app da submit --no-submit` と同等です。

## 5. データをクエリ

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::query::prelude::*;

fn list_domains() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let response = client.request(&FindAllDomains::new())?;
 for domain in response {
 println!("{}", domain.name());
 }
 Ok(())
}
```

クエリは request/response パターンです。`iroha_data_model::query` からクエリ型を作成し `client.request` で送信、結果を反復します。応答は Norito ベースの JSON で、ワイヤーフォーマットは決定的です。

## 6. Explorer QR スナップショット

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "i105...",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` は `/v1/explorer/accounts/{id}/qr` の JSON を反映します。標準アカウント ID、標準 I105 リテラル、ネットワーク接頭辞/誤り訂正メタデータ、QR 寸法、ウォレットが直接埋め込める inline SVG を含みます。

## 7. イベントに購読

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::events::pipeline::PipelineEventFilterBox;
use futures_lite::stream::StreamExt;

async fn listen_for_blocks() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let mut stream = client
 .listen_for_events([PipelineEventFilterBox::any()])
 .await?;

 while let Some(event) = stream.next().await {
 println!("Received event: {:?}", event?);
 }
 Ok(())
}
```

クライアントは Torii の SSE エンドポイント向けに async ストリームを提供します。pipeline イベント、データイベント、テレメトリフィードを含みます。

## さらに例を見る

- エンドツーエンドのフローは `crates/iroha` の `tests/` にあります。`transaction_submission.rs` などの統合テストを探してください。
- CLI（`iroha_cli`）は同じクライアントモジュールを使います。`crates/iroha_cli/src/` を見て、認証・バッチ処理・リトライの実装を確認してください。
- Norito を念頭に置いてください。クライアントは `serde_json` にフォールバックしません。SDK を拡張する際は JSON エンドポイントで `norito::json`、バイナリペイロードで `norito::codec` を使用します。

## 関連 Norito 例

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — このクイックスタートのセットアップ段階を反映する最小 Kotodama スキャフォールドをコンパイル・実行・デプロイします。
- [Register domain and mint assets](../norito/examples/register-and-mint) — 上記の `Register` + `Mint` フローに合わせ、同じ操作をコントラクトから再現できます。
- [Transfer asset between accounts](../norito/examples/transfer-asset) — SDK クイックスタートで使う同じアカウント ID で `transfer_asset` システムコールを示します。

これらの構成要素で Torii を Rust サービスや CLI に統合できます。完全な命令・クエリ・イベントは生成ドキュメントとデータモデルクレートを参照してください。

