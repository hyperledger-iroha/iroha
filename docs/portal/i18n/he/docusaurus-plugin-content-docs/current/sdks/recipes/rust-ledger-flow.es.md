---
lang: he
direction: rtl
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: es
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3e1cd7a1ec89819f8f3c7916774e07b2c467fcd53381c8629c92ebc86abc6d73
source_last_modified: "2025-11-11T10:23:19.175496+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Receta de flujo del libro mayor en Rust
description: Usa el SDK de Rust para registrar un activo, acuñar suministro, transferirlo y consultar saldos contra la red de un solo peer por defecto.
slug: /sdks/recipes/rust-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

Esta receta refleja el [recorrido del libro mayor de la CLI](../../norito/ledger-walkthrough.md), pero ejecuta todo desde un binario de Rust. Reutiliza la red de desarrollo por defecto (`docker compose -f defaults/docker-compose.single.yml up --build`) y las credenciales de demo en `defaults/client.toml`, para que puedas comparar hashes del SDK y la CLI uno a uno.

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="Usa este archivo fuente de Rust como base para seguir o comparar con tus cambios."
/>

## Prerequisitos

1. Ejecuta el peer de desarrollo con Docker Compose (consulta el [quickstart de Norito](../../norito/quickstart.md)).
2. Exporta las cuentas admin/receptor por defecto y la clave privada del admin desde
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="ih58..."
   export RECEIVER_ACCOUNT="ih58..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   La cadena de clave privada es el valor codificado en multihash almacenado bajo `[account].private_key`.
3. Crea un nuevo binario de workspace (o reutiliza uno existente):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Agrega las dependencias (usa la versión de crates.io si estás fuera del workspace):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Programa de ejemplo

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

## Ejecuta la receta

```bash
cargo run
```

Deberías ver una salida similar a:

```
ih58... now holds:
  50 units of coffee#wonderland
```

Si la definición del activo ya existe, la llamada de registro devuelve `ValidationError::Duplicate`. Ignórala (la acuñación sigue funcionando) o elige otro nombre.

## Verifica hashes y paridad

- Usa `iroha --config defaults/client.toml transaction get --hash <hash>` para inspeccionar las transacciones que envió el SDK.
- Contrasta los saldos con `iroha --config defaults/client.toml asset list all --table` o `asset list filter '{"id":"coffee#wonderland##<account>"}'`.
- Repite el mismo flujo desde el recorrido de la CLI para confirmar que ambas superficies producen los mismos payloads Norito y estados de transacción.
