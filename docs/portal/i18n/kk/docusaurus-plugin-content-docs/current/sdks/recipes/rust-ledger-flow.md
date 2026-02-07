---
slug: /sdks/recipes/rust-ledger-flow
lang: kk
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
---

import SampleDownload from '@site/src/components/SampleDownload';

This recipe mirrors the [CLI ledger walkthrough](../../norito/ledger-walkthrough.md)
but runs everything from a Rust binary. It reuses the default dev network
(`docker compose -f defaults/docker-compose.single.yml up --build`) and the demo
credentials in `defaults/client.toml`, so you can compare SDK and CLI hashes one
for one.

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="Use this Rust source file as a baseline to follow along or to diff against your changes."
/>

## Prerequisites

1. Run the dev peer with Docker Compose (see the [Norito quickstart](../../norito/quickstart.md)).
2. Export the default admin/receiver accounts and the admin private key from
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="ih58..."
   export RECEIVER_ACCOUNT="ih58..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   The private key string is the multihash-encoded value stored under `[account].private_key`.
3. Create a new workspace binary (or reuse an existing one):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Add the dependencies (use a crates.io version if you are outside the
   workspace):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Example program

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

## Run the recipe

```bash
cargo run
```

You should see log output similar to:

```
ih58... now holds:
  50 units of coffee#wonderland
```

If the asset definition already exists, the register call returns a
`ValidationError::Duplicate`. Either ignore it (the mint still succeeds) or pick
a new name.

## Verify hashes and parity

- Use `iroha --config defaults/client.toml transaction get --hash <hash>` to
  inspect the transactions that the SDK submitted.
- Cross-check balances with `iroha --config defaults/client.toml asset list all --table`
  or `asset list filter '{"id":"coffee#wonderland##<account>"}'`.
- Repeat the same flow from the CLI walkthrough to confirm both surfaces produce
  the same Norito payloads and transaction statuses.
