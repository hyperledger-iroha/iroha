---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e43956ccc1ca325fa3423492fc2b75ca97f7eb54360cd192796b9577159e9c8
source_last_modified: "2026-01-30T15:15:25+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/rust-ledger-flow
title: Recette de flux du registre Rust
description: Utilisez le SDK Rust pour enregistrer un actif, frapper l'offre, le transférer et interroger les soldes sur le réseau par défaut à un seul nœud.
---

import SampleDownload from '@site/src/components/SampleDownload';

Cette recette reflète le [parcours du registre via la CLI](../../norito/ledger-walkthrough.md), mais exécute tout depuis un binaire Rust. Elle réutilise le réseau de développement par défaut (`docker compose -f defaults/docker-compose.single.yml up --build`) et les identifiants de démo dans `defaults/client.toml`, afin que vous puissiez comparer les hashes SDK et CLI à l'identique.

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="Utilisez ce fichier source Rust comme base pour suivre ou comparer avec vos changements."
/>

## Prérequis

1. Lancez le nœud de développement avec Docker Compose (voir le [quickstart Norito](../../norito/quickstart.md)).
2. Exportez les comptes admin/receiver par défaut et la clé privée admin depuis
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="<katakana-i105-account-id>"
   export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   La chaîne de clé privée est la valeur encodée en multihash stockée sous `[account].private_key`.
3. Créez un nouveau binaire de workspace (ou réutilisez-en un existant) :

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Ajoutez les dépendances (utilisez la version crates.io si vous êtes hors du workspace) :

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Programme d’exemple

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

## Exécuter la recette

```bash
cargo run
```

Vous devriez voir une sortie similaire à :

```
<katakana-i105-account-id> now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Si la définition d’actif existe déjà, l’appel d’enregistrement renvoie `ValidationError::Duplicate`. Ignorez-le (la frappe fonctionne toujours) ou choisissez un autre nom.

## Vérifier les hashes et la parité

- Utilisez `iroha --config defaults/client.toml transaction get --hash <hash>` pour inspecter les transactions soumises par le SDK.
- Vérifiez les soldes avec `iroha --config defaults/client.toml asset list all --table` ou `asset list filter '{"id":"norito:4e52543000000002"}'`.
- Répétez le même flux via le parcours CLI pour confirmer que les deux surfaces produisent les mêmes payloads Norito et statuts de transaction.
