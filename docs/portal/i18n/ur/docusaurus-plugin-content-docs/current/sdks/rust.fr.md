---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2151bc561df9599865e2a5aa5d159171c1aa6f4830bfa51bd32d726f0c70a6f
source_last_modified: "2025-11-11T10:23:01.761192+00:00"
translation_last_reviewed: 2026-01-30
---

# Quickstart du SDK Rust

L’API client Rust vit dans le crate `iroha`, qui expose un type `client::Client` pour parler à Torii. Utilisez‑le pour soumettre des transactions, vous abonner aux événements ou interroger l’état depuis une application Rust.

## 1. Ajouter le crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

L’exemple workspace active le module client via la feature `client`. Si vous utilisez le crate publié, remplacez l’attribut `path` par la version courante.

## 2. Configurer le client

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

`ClientConfiguration` reflète le fichier de configuration CLI : il inclut les URL Torii et télémétrie, les éléments d’authentification, les timeouts et les préférences de batching.

## 3. Soumettre une transaction

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

En coulisse, le client utilise Norito pour encoder le payload avant l’envoi à Torii. En cas de succès, le hash retourné permet de suivre le statut via `client.poll_transaction_status(hash)`.

## 4. Soumettre des blobs DA

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

Lorsque vous devez inspecter ou persister le payload Norito sans l’envoyer à Torii, appelez `client.build_da_ingest_request(...)` pour obtenir la requête signée et la rendre en JSON/bytes, comme `iroha app da submit --no-submit`.

## 5. Interroger des données

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

Les requêtes suivent le schéma request/response : construisez un type de requête depuis `iroha_data_model::query`, envoyez‑le via `client.request` et itérez sur les résultats. Les réponses utilisent un JSON adossé à Norito, donc le format wire est déterministe.

## 6. Snapshots QR Explorer

```rust
use iroha::client::{
    Client, ClientConfiguration, ExplorerAccountQrOptions,
};

fn download_qr() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let snapshot = client.get_explorer_account_qr(
        "i105...",
        Some(ExplorerAccountQrOptions {
        }),
    )?;
    println!("Canonical literal: {}", snapshot.literal);
    println!("SVG payload: {}", snapshot.svg);
    Ok(())
}
```

`ExplorerAccountQrSnapshot` reflète le JSON `/v1/explorer/accounts/{id}/qr` : identifiant de compte canonique, littéral dans le format demandé, métadonnées de préfixe/correction d’erreur, dimensions du QR et payload SVG inline. Omettez `ExplorerAccountQrOptions` pour l’I105 par défaut ou définissez canonical I105 output pour la salida canonica I105 utilisée par ADDR-6b.

## 7. S’abonner aux événements

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

Le client expose des streams async pour les endpoints SSE de Torii, y compris événements pipeline, événements de données et flux de télémétrie.

## Plus d’exemples

- Les flux end-to-end se trouvent sous `tests/` dans `crates/iroha`. Cherchez des tests d’intégration comme `transaction_submission.rs` pour des scénarios plus riches.
- La CLI (`iroha_cli`) utilise le même module client ; consultez `crates/iroha_cli/src/` pour voir l’authentification, le batching et les retries en production.
- Gardez Norito en tête : le client ne retombe jamais sur `serde_json`. En étendant le SDK, utilisez `norito::json` pour les endpoints JSON et `norito::codec` pour les payloads binaires.

## Exemples Norito associés

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — compile, exécute et déploie le scaffold Kotodama minimal qui reflète la phase de setup de ce quickstart.
- [Register domain and mint assets](../norito/examples/register-and-mint) — s’aligne avec le flux `Register` + `Mint` ci‑dessus pour rejouer les mêmes opérations depuis un contrat.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — démontre le syscall `transfer_asset` avec les mêmes IDs de compte que ceux utilisés par les quickstarts du SDK.

Avec ces briques, vous pouvez intégrer Torii dans des services ou CLI Rust. Référez‑vous à la documentation générée et aux crates du modèle de données pour l’ensemble complet d’instructions, requêtes et événements.
