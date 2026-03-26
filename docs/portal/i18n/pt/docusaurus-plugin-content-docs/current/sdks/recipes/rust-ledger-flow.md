---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/rust-ledger-flow
title: Receita de fluxo do ledger em Rust
description: Use o SDK de Rust para registrar um ativo, cunhar o suprimento, transferi-lo e consultar saldos na rede padrão de um único peer.
---

import SampleDownload from '@site/src/components/SampleDownload';

Esta receita espelha o [passo a passo do ledger na CLI](../../norito/ledger-walkthrough.md), mas executa tudo a partir de um binário Rust. Ela reutiliza a rede de desenvolvimento padrão (`docker compose -f defaults/docker-compose.single.yml up --build`) e as credenciais de demo em `defaults/client.toml`, para que você possa comparar hashes do SDK e da CLI diretamente.

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="Use este arquivo fonte em Rust como base para acompanhar ou comparar com suas alterações."
/>

## Pré-requisitos

1. Inicie o peer de desenvolvimento com Docker Compose (veja o [quickstart Norito](../../norito/quickstart.md)).
2. Exporte as contas admin/receiver padrão e a chave privada do admin a partir de
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="soraカタカナ..."
   export RECEIVER_ACCOUNT="soraカタカナ..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   A string da chave privada é o valor codificado em multihash armazenado em `[account].private_key`.
3. Crie um novo binário de workspace (ou reutilize um existente):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Adicione as dependências (use a versão do crates.io se estiver fora do workspace):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Programa de exemplo

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

## Execute a receita

```bash
cargo run
```

Você deve ver uma saída semelhante a:

```
soraカタカナ... now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Se a definição do ativo já existir, a chamada de registro retorna `ValidationError::Duplicate`. Ignore-a (a cunhagem ainda funciona) ou escolha outro nome.

## Verifique hashes e paridade

- Use `iroha --config defaults/client.toml transaction get --hash <hash>` para inspecionar as transações enviadas pelo SDK.
- Confira os saldos com `iroha --config defaults/client.toml asset list all --table` ou `asset list filter '{"id":"norito:4e52543000000002"}'`.
- Repita o mesmo fluxo no walkthrough da CLI para confirmar que ambas as superfícies produzem os mesmos payloads Norito e estados de transação.

