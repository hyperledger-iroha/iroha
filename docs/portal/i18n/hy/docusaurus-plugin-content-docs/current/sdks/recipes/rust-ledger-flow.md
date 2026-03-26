---
slug: /sdks/recipes/rust-ledger-flow
lang: hy
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ներմուծել SampleDownload-ից '@site/src/components/SampleDownload';

Այս բաղադրատոմսը արտացոլում է [CLI մատյանների ուղեցույցը] (../../norito/ledger-walkthrough.md)
բայց ամեն ինչ աշխատում է Rust երկուականից: Այն նորից օգտագործում է լռելյայն մշակող ցանցը
(`docker compose -f defaults/docker-compose.single.yml up --build`) և ցուցադրումը
հավատարմագրերը `defaults/client.toml`-ում, այնպես որ կարող եք համեմատել SDK և CLI հեշերը մեկը
մեկի համար.

<Նմուշի ներբեռնում
  href="/sdk-recipes/rust/src/main.rs"
  filename = "src/main.rs"
  description="Օգտագործիր այս Rust սկզբնաղբյուր ֆայլը որպես հիմք՝ հետևելու կամ տարբերվելու քո փոփոխություններին։"
/>

## Նախադրյալներ

1. Գործարկեք մշակող սարքը Docker Compose-ով (տես [Norito արագ մեկնարկը](../../norito/quickstart.md)):
2. Արտահանեք լռելյայն ադմինիստրատորի/ստացողի հաշիվները և ադմինիստրատորի անձնական բանալին
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="soraカタカナ..."
   export RECEIVER_ACCOUNT="soraカタカナ..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   Անձնական բանալի տողը բազմահեշ կոդավորված արժեքն է, որը պահվում է `[account].private_key`-ում:
3. Ստեղծեք նոր աշխատանքային տարածք երկուական (կամ վերօգտագործեք գոյություն ունեցողը).

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Ավելացրեք կախվածությունները (օգտագործեք crates.io տարբերակը, եթե դուք դուրս եք
   աշխատանքային տարածք):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Օրինակ ծրագիր

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

## Գործարկեք բաղադրատոմսը

```bash
cargo run
```

Դուք պետք է տեսնեք գրանցամատյանի ելքը նման.

```
i105... now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Եթե ակտիվի սահմանումն արդեն գոյություն ունի, ռեգիստրի կանչը վերադարձնում է a
`ValidationError::Duplicate`. Կամ անտեսեք այն (անանուխը դեռ հաջողվում է) կամ ընտրեք
նոր անուն.

## Ստուգեք հեշերը և հավասարությունը

- Օգտագործեք `iroha --config defaults/client.toml transaction get --hash <hash>` դեպի
  ստուգել SDK-ի կողմից ներկայացված գործարքները:
- Խաչաձև ստուգեք մնացորդները `iroha --config defaults/client.toml asset list all --table`-ով
  կամ `asset list filter '{"id":"norito:4e52543000000002"}'`.
- Կրկնեք նույն հոսքը CLI-ից՝ երկու մակերևույթների արտադրությունը հաստատելու համար
  նույն Norito ծանրաբեռնվածությունը և գործարքի կարգավիճակները: