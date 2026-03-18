---
slug: /sdks/recipes/rust-ledger-flow
lang: dz
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

འདི་གི་ཐབས་ཤེས་འདི་གིས་ [CLI ledger walththrough ](I18NU000009X)
དེ་འབདཝ་ད་ Rust གཉིས་ལྡན་ལས་ ག་ར་གཡོག་བཀོལཝ་ཨིན། དེ་གིས་ སྔོན་སྒྲིག་ཌི་ཝི་ཡོངས་འབྲེལ་འདི་ ལོག་ལག་ལེན་འཐབ་ཨིན།
(`docker compose -f defaults/docker-compose.single.yml up --build`) དང་ བརྡ་སྟོན་ནི།
I18NI000000012X ནང་ཡིད་ཆེས་ཡོད་ན་ ཁྱོད་ཀྱིས་ SDK དང་ CLI hashes གཅིག་ག་བསྡུར་རྐྱབ་ཚུགས།
གཅིག་གི་དོན་ལུ་ཨིན།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལེན་/རཱསི་/ཨེསི་ཨར་སི/མེན་.རེསི།"
  fiter na="src/main.srs."
  design="ཁྱོད་ཀྱི་བསྒྱུར་བཅོས་ལུ་རྒྱབ་འགལ་འབད་ནི་དང་ ཡང་ན་ ཌིཕ་འབད་ནི་ལུ་ རཱསིཊི་འབྱུང་ཁུངས་ཡིག་སྣོད་འདི་གཞི་རྟེན་སྦེ་ལག་ལེན་འཐབ།"
།/>།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

1. dev མཉམ་རོགས་ I18NT0000002X བརྩམས་སྒྲིག ( [Norito མགྱོགས་མྱུར་](I18NU000000010X))
2. སྔོན་སྒྲིག་བདག་སྐྱོང་/ལེན་མི་རྩིས་ཐོ་ཚུ་དང་ བདག་སྐྱོང་སྒེར་གྱི་ལྡེ་མིག་འདི་ ཕྱིར་འདྲེན་འབད།
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="i105..."
   export RECEIVER_ACCOUNT="i105..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   སྒེར་གྱི་ལྡེ་མིག་ཡིག་རྒྱུན་འདི་ `[account].private_key` གི་འོག་ལུ་གསོག་འཇོག་འབད་ཡོད་པའི་ སྣ་མང་ཨིན་ཀོཌི་གནས་གོང་ཨིན།
༣ ལཱ་གི་ས་སྒོ་གསརཔ་ཅིག་གསར་བསྐྲུན་འབད་ (ཡང་ན་ ད་ལྟོ་ཡོད་མི་ཅིག་ལོག་སྟེ་ལག་ལེན་འཐབ།)

   I18NF0000004X

4. བརྟེན་པ་ (ཁྱོད་རང་ཕྱི་རོལ་དུ་ཡོད་ན་ crates.io ཐོན་རིམ་ལག་ལེན་འཐབ།
   ལས་ཀའི་ས་སྟོང་):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## དཔེར་བརྗོད།

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

    // 1) Register coffee#wonderland if it does not exist yet.
    let asset_definition_id = AssetDefinitionId::from_str("coffee#wonderland")?;
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

## བཟོ་ཐངས་བརྒྱུད།

```bash
cargo run
```

ཁྱོད་ཀྱིས་ དྲན་དེབ་ཨའུཊི་པུཊི་འདི་ ཅོག་འཐདཔ་:

I18NF0000008X

རྒྱུ་དངོས་ངེས་ཚིག་འདི་ཧེ་མ་ལས་ཡོད་པ་ཅིན་ ཐོ་བཀོད་ཀྱི་འབོད་བརྡ་འདི་གིས་ ༡ སླར་ལོག་འབདཝ་ཨིན།
`ValidationError::Duplicate`. ཡང་ན་ སྣང་མེད་བཞག་སྟེ་ (mint འདི་ད་དུང་ཡང་ མཐར་འཁྱོལ་བྱུང་ཡོདཔ་) ཡང་ན་ འདམ་ཁ་རྐྱབ།
མིང་གསརཔ།

## ཧེ་ཤེ་དང་ ཆ་སྙོམས་བདེན་དཔྱད་འབད།

- `iroha --config defaults/client.toml transaction get --hash <hash>` ལུ་ལག་ལེན་འཐབ།
  ཨེསི་ཌི་ཀེ་གིས་ ཕུལ་མི་ ཚོང་འབྲེལ་ཚུ་ བརྟག་དཔྱད་འབད།
- `iroha --config defaults/client.toml asset list all --table` དང་མཉམ་པའི་ བཤེར་ཡིག་འདྲ་མཉམ་ཚུ།
  ཡང་ན་ `asset list filter '{"id":"norito:4e52543000000002"}'`.
- ཁ་ཐོག་གཉིས་ཆ་རའི་ཐོན་སྐྱེད་ངེས་གཏན་བཟོ་ནིའི་དོན་ལུ་ སི་ཨེལ་ཨའི་ འགྲུལ་བསྐྱོད་ལས་ འདྲ་མཚུངས་ཀྱི་ རྒྱུན་འགྲུལ་འདི་ བསྐྱར་ལོག་འབད།
  དེ་དང་འདྲ་བའི་I1NT0000001X དངུལ་སྤྲོད་འབབ་དང་ཚོང་འབྲེལ་གྱི་གནས་ཚད།