---
lang: es
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3e1cd7a1ec89819f8f3c7916774e07b2c467fcd53381c8629c92ebc86abc6d73
source_last_modified: "2025-11-11T10:23:19.175496+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Рецепт потока реестра на Rust
description: Используйте Rust SDK, чтобы зарегистрировать актив, выпустить предложение, перевести его и запросить балансы в стандартной сети из одного узла.
slug: /sdks/recipes/rust-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

Этот рецепт повторяет [CLI walkthrough реестра](../../norito/ledger-walkthrough.md), но выполняет все из бинарника Rust. Он использует стандартную dev-сеть (`docker compose -f defaults/docker-compose.single.yml up --build`) и демонстрационные учетные данные в `defaults/client.toml`, чтобы вы могли сравнить хеши SDK и CLI один к одному.

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="Используйте этот исходник Rust как базу для прохождения или сравнения своих изменений."
/>

## Предварительные требования

1. Запустите dev-узел через Docker Compose (см. [quickstart Norito](../../norito/quickstart.md)).
2. Экспортируйте стандартные учетные записи admin/receiver и приватный ключ admin из
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="i105..."
   export RECEIVER_ACCOUNT="i105..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   Строка приватного ключа — это значение, закодированное в multihash, сохраненное в `[account].private_key`.
3. Создайте новый бинарник в workspace (или используйте существующий):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Добавьте зависимости (используйте версию crates.io, если вы вне workspace):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Пример программы

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

## Запуск рецепта

```bash
cargo run
```

Вы должны увидеть вывод примерно такого вида:

```
i105... now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Если определение актива уже существует, вызов регистрации возвращает `ValidationError::Duplicate`. Игнорируйте его (выпуск все равно успешен) или выберите другое имя.

## Проверьте хеши и паритет

- Используйте `iroha --config defaults/client.toml transaction get --hash <hash>` для просмотра транзакций, отправленных SDK.
- Сверьте балансы с `iroha --config defaults/client.toml asset list all --table` или `asset list filter '{"id":"norito:4e52543000000002"}'`.
- Повторите тот же поток через CLI walkthrough, чтобы подтвердить, что обе поверхности создают одинаковые payloads Norito и статусы транзакций.
