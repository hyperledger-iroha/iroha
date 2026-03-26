---
slug: /sdks/recipes/rust-ledger-flow
lang: am
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ናሙና አውርድን ከ'@site/src/components/SampleDownload' አስመጣ;

ይህ የምግብ አሰራር [CLI ledger walkthrough](../../norito/ledger-walkthrough.md) ያንጸባርቃል
ነገር ግን ሁሉንም ነገር ከ Rust binary ይሰራል። ነባሪውን የዴቪ አውታረ መረብ እንደገና ይጠቀማል
(`docker compose -f defaults/docker-compose.single.yml up --build`) እና ማሳያ
ምስክርነቶች በI18NI0000012X፣ ስለዚህ ኤስዲኬን እና CLI hashes አንድን ማወዳደር ይችላሉ።
ለአንድ.

<ናሙና አውርድ
  href="/sdk-recipes/rust/src/main.rs"
  ፋይል ስም = "src/main.rs"
  description="ይህንን የዝገት ምንጭ ፋይል ለመከተል ወይም ከለውጦችዎ ጋር ለመለያየት እንደ መነሻ ይጠቀሙ።"
/>

## ቅድመ ሁኔታዎች

1. የዴቭ አቻውን በDocker Compose ያሂዱ ([Norito quickstart](../../norito/quickstart.md ይመልከቱ))።
2. ነባሪውን የአስተዳዳሪ/ተቀባዩ መለያዎችን እና የአስተዳዳሪውን የግል ቁልፍ ከ
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="<katakana-i105-account-id>"
   export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   የግላዊ ቁልፍ ሕብረቁምፊ በ`[account].private_key` ስር የተከማቸ ባለ ብዙሃሽ ኮድ ነው።
3. አዲስ የስራ ቦታ ሁለትዮሽ ይፍጠሩ (ወይም ያለውን እንደገና ይጠቀሙ):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. ጥገኞቹን ያክሉ (ከ ውጭ ከሆኑ የ crates.io ስሪት ይጠቀሙ
   የስራ ቦታ፡-

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

#ፕሮግራም ምሳሌ

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

## የምግብ አዘገጃጀቱን ያሂዱ

```bash
cargo run
```

ተመሳሳይ የምዝግብ ማስታወሻ ውፅዓት ማየት አለብህ፡-

```
<katakana-i105-account-id> now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

የንብረት ፍቺው አስቀድሞ ካለ፣ የመመዝገቢያ ጥሪው ይመለሳል ሀ
`ValidationError::Duplicate`. ወይም ችላ ይበሉት (አዝሙድ አሁንም ተሳክቷል) ወይም ይምረጡ
አዲስ ስም.

## ሃሽ እና እኩልነት ያረጋግጡ

- `iroha --config defaults/client.toml transaction get --hash <hash>` ይጠቀሙ
  ኤስዲኬ ያቀረበውን ግብይቶች ይፈትሹ።
- ሂሳቦችን ከ `iroha --config defaults/client.toml asset list all --table` ጋር ያረጋግጡ
  ወይም `asset list filter '{"id":"norito:4e52543000000002"}'`.
- የሁለቱም ንጣፎች ምርትን ለማረጋገጥ ከCLI መራመጃ ተመሳሳይ ፍሰት ይድገሙ
  ተመሳሳይ የ Norito ጭነት እና የግብይት ሁኔታዎች።