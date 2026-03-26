---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3e1cd7a1ec89819f8f3c7916774e07b2c467fcd53381c8629c92ebc86abc6d73
source_last_modified: "2025-11-11T10:23:19.175496+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Rust 台帳フローレシピ
description: Rust SDK を使ってアセットを登録し、供給をミントし、転送し、既定の単一ピアネットワークで残高を照会します。
slug: /sdks/recipes/rust-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

このレシピは [CLI の台帳ウォークスルー](../../norito/ledger-walkthrough.md) を反映していますが、すべてを Rust バイナリから実行します。既定の開発ネットワーク (`docker compose -f defaults/docker-compose.single.yml up --build`) と `defaults/client.toml` のデモ資格情報を再利用するため、SDK と CLI のハッシュを1対1で比較できます。

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="この Rust ソースファイルをベースラインとして使い、手順を進めたり差分を比較したりできます。"
/>

## 前提条件

1. Docker Compose で開発ピアを起動します（[Norito クイックスタート](../../norito/quickstart.md) を参照）。
2. 既定の admin/receiver アカウントと admin の秘密鍵を次からエクスポートします
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="<katakana-i105-account-id>"
   export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   秘密鍵の文字列は `[account].private_key` に保存されている multihash エンコード値です。
3. 新しい workspace バイナリを作成します（または既存のものを再利用します）:

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. 依存関係を追加します（workspace 外の場合は crates.io のバージョンを使用してください）:

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## サンプルプログラム

```rust title="src/main.rs"
use std::str::FromStr;

use eyre::Result;
use iroha::client::{Client, ClientConfiguration};
use iroha_crypto::{KeyPair, PrivateKey};
use iroha_data_model::{
    isi::prelude::*,
    prelude::*,
    query::prelude::FindAccountAssets,
};

fn main() -> Result<()> {

    let admin_account = std::env::var("ADMIN_ACCOUNT").expect("export ADMIN_ACCOUNT");
    let receiver_account = std::env::var("RECEIVER_ACCOUNT").expect("export RECEIVER_ACCOUNT");
    let admin_private_key = std::env::var("ADMIN_PRIVATE_KEY").expect("export ADMIN_PRIVATE_KEY");

    let mut cfg = ClientConfiguration::test();
    cfg.torii_url = "http://127.0.0.1:8080".parse()?;
    cfg.chain = ChainId::from("00000000-0000-0000-0000-000000000000");
    cfg.account = AccountId::from_str(&admin_account)?;
    cfg.key_pair = KeyPair::from_private_key(PrivateKey::from_str(&admin_private_key)?)?;

    let client = Client::new(cfg)?;

    // 1) Register 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ if it does not exist yet.
    let asset_definition_id = AssetDefinitionId::from_str("7Sp2j6zDvJFnMoscAiMaWbWHRDBZ")?;
    client.submit_blocking(Register::asset_definition(
        AssetDefinition::numeric(asset_definition_id.clone()),
    ))?;

    // 2) Mint 250 units into the admin account.
    let admin_asset = AssetId::new(asset_definition_id.clone(), AccountId::from_str(&admin_account)?);
    client.submit_blocking(Mint::asset_numeric(250_u32, admin_asset.clone()))?;

    // 3) Transfer 50 units to the receiver.
    let receiver_id = AccountId::from_str(&receiver_account)?;
    client.submit_blocking(Transfer::asset_numeric(admin_asset.clone(), 50_u32, receiver_id.clone()))?;

    // 4) Query the receiver balance to confirm the transfer.
    let assets = client.request(&FindAccountAssets::new(receiver_id.clone()))?;
    println!("{} now holds:", receiver_id);
    for asset in assets {
        if asset.id().definition() == &asset_definition_id {
            println!("  {} units of {}", asset.value(), asset.id().definition());
        }
    }

    Ok(())
}
```

## レシピを実行

```bash
cargo run
```

次のようなログ出力が表示されるはずです:

```
<katakana-i105-account-id> now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

アセット定義が既に存在する場合、登録呼び出しは `ValidationError::Duplicate` を返します。無視するか（ミントは成功します）、別の名前を選んでください。

## ハッシュとパリティの確認

- `iroha --config defaults/client.toml transaction get --hash <hash>` を使って SDK が送信したトランザクションを確認します。
- `iroha --config defaults/client.toml asset list all --table` または `asset list filter '{"id":"norito:4e52543000000002"}'` で残高を突き合わせます。
- CLI のウォークスルーと同じフローを繰り返し、両方の経路が同じ Norito ペイロードとトランザクションステータスを生成することを確認します。

