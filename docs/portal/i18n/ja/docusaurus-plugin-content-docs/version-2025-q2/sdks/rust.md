---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK クイックスタート

Rust クライアント API は `iroha` クレートに存在し、`client::Client` を公開します。
Torii と通信するためのタイプ。トランザクションを送信する必要がある場合に使用します。
イベントをサブスクライブしたり、Rust アプリケーションから状態をクエリしたりできます。

## 1. クレートを追加します

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

ワークスペースの例では、`client` 機能を介してクライアント モジュールのロックを解除します。もしあなたが
公開されたクレートを消費し、`path` 属性を現在の属性に置き換えます。
バージョン文字列。

## 2. クライアントを構成する

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

`ClientConfiguration` は CLI 設定ファイルをミラーリングします。これには Torii と
テレメトリ URL、認証マテリアル、タイムアウト、バッチ設定。

## 3. トランザクションを送信する

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

内部では、クライアントは Norito を使用してトランザクション ペイロードをエンコードします。
Torii に投稿します。送信が成功した場合、返されたハッシュは次の目的で使用できます。
`client.poll_transaction_status(hash)` 経由でステータスを追跡します。

## 4. DA BLOB を送信する

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

Norito ペイロードを送信せずに検査または永続化する必要がある場合
Torii、`client.build_da_ingest_request(...)` を呼び出して署名付きリクエストを取得します
それを JSON/バイトとしてレンダリングし、`iroha app da submit --no-submit` をミラーリングします。

## 5. データのクエリ

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

クエリはリクエスト/レスポンス パターンに従います。クエリ タイプを次から構築します。
`iroha_data_model::query`、`client.request` 経由で送信し、
結果。応答には Norito ベースの JSON が使用されるため、ワイヤ形式は確定的です。

## 6. エクスプローラーの QR スナップショット

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "<katakana-i105-account-id>",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` は `/v1/explorer/accounts/{id}/qr` JSON をミラーリングします。
表面: 正規のアカウント ID、つまり、
要求された形式、ネットワーク プレフィックス/エラー修正メタデータ、QR 寸法、および
ウォレット/エクスプローラーが直接埋め込むことができるインライン SVG ペイロード。

## 7. イベントを購読する

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

クライアントは、パイプラインを含む Torii の SSE エンドポイントの非同期ストリームを公開します
イベント、データ イベント、テレメトリ フィード。

## その他の例

- エンドツーエンド フローは、`crates/iroha` の `tests/` の下に存在します。統合の検索
 より豊富なシナリオのための `transaction_submission.rs` などのテスト。
- CLI (`iroha_cli`) は同じクライアント モジュールを使用します。閲覧する
 `crates/iroha_cli/src/`: 認証、バッチ処理、および再試行がどのように行われるかを確認します。
 生産ツールで処理されます。
- Norito に留意してください。クライアントが `serde_json` にフォールバックすることはありません。あなたが
 SDK を拡張し、JSON エンドポイントの `norito::json` ヘルパーに依存し、
 バイナリペイロードの場合は `norito::codec`。

## 関連する Norito の例

- [始まりのエントリポイント スケルトン](../norito/examples/hajimari-entrypoint) — コンパイル、実行、デプロイ
 このクイックスタートのセットアップ フェーズを反映する最小限の Kotodama スキャフォールド。
- [ドメインとミント資産の登録](../norito/examples/register-and-mint) —
 上記の `Register` + `Mint` フローにより、コントラクトから同じ操作を再現できます。
- [アカウント間での資産の転送](../norito/examples/transfer-asset) — を示します。
 SDK クイックスタートが使用するのと同じアカウント ID を持つ `transfer_asset` syscall。

これらのビルディング ブロックを使用すると、Torii を Rust サービスまたは CLI に統合できます。
完全なセットについては、生成されたドキュメントとデータ モデル クレートを参照してください。
指示、クエリ、イベント。