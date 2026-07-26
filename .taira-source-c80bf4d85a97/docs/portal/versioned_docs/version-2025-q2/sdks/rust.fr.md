---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Démarrage rapide du SDK Rust

L'API client Rust réside dans la caisse `iroha`, qui expose un `client::Client`
tapez pour parler à Torii. Utilisez-le lorsque vous devez soumettre des transactions,
abonnez-vous à des événements ou interrogez l'état à partir d'une application Rust.

## 1. Ajoutez la caisse

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

L'exemple d'espace de travail déverrouille le module client via la fonctionnalité `client`. Si vous
consommer la caisse publiée, remplacer l'attribut `path` par l'attribut actuel
chaîne de version.

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

`ClientConfiguration` reflète le fichier de configuration CLI : il inclut Torii et
URL de télémétrie, matériel d'authentification, délais d'attente et préférences de traitement par lots.

## 3. Soumettre une transaction

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::{
    isi::prelude::*,
    prelude::{AccountId, ChainId, Domain, DomainId},
};
use iroha_crypto::KeyPair;

fn submit_example() -> eyre::Result<()> {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let key_pair = KeyPair::generate_ed25519(); // replace with a persistent key in real apps
    let account_id = AccountId::new(key_pair.public_key().clone());

    let cfg = ClientConfiguration {
        chain: chain_id.clone(),
        account: account_id.clone(),
        key_pair: key_pair.clone(),
        ..ClientConfiguration::test()
    };

    let client = Client::new(cfg)?;

    let instruction = Register::domain(Domain::new(DomainId::try_new("research", "universal")?));

    let tx = client.build_transaction([instruction]);
    let signed = tx.sign(&key_pair)?;
    let hash = client.submit_transaction(&signed)?;
    println!("Submitted transaction: {hash}");
    Ok(())
}
```

Sous le capot, le client utilise Norito pour coder la charge utile de la transaction avant
en le publiant sur Torii. Si la soumission réussit, le hachage renvoyé peut être utilisé pour
suivre l'état via `client.poll_transaction_status(hash)`.

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

Lorsque vous devez inspecter ou conserver la charge utile Norito sans l'envoyer à
Torii, appelez `client.build_da_ingest_request(...)` pour obtenir la demande signée
et restituez-le au format JSON/octets, en miroir de `iroha app da submit --no-submit`.

## 5. Interroger les données

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

Les requêtes suivent le modèle requête/réponse : construisez un type de requête à partir de
`iroha_data_model::query`, envoyez-le via `client.request` et parcourez le
résultats. Les réponses utilisent du JSON soutenu par Norito, le format filaire est donc déterministe.

## 6. Abonnez-vous aux événements

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

Le client expose les flux asynchrones pour les points de terminaison SSE de Torii, y compris le pipeline
événements, événements de données et flux de télémétrie.

## Plus d'exemples

- Les flux de bout en bout vivent sous `tests/` dans `crates/iroha`. Recherche d'intégration
  des tests tels que `transaction_submission.rs` pour des scénarios plus riches.
- La CLI (`iroha_cli`) utilise le même module client ; parcourir
  `crates/iroha_cli/src/` pour voir comment l'authentification, le traitement par lots et les tentatives sont effectués
  manipulés dans les outils de production.
- Gardez Norito à l'esprit : le client ne revient jamais à `serde_json`. Quand tu
  étendre le SDK, s'appuyer sur les assistants `norito::json` pour les points de terminaison JSON et
  `norito::codec` pour les charges utiles binaires.

Avec ces éléments de base, vous pouvez intégrer Torii dans les services Rust ou les CLI.
Reportez-vous à la documentation générée et aux caisses de modèles de données pour l'ensemble complet des
instructions, requêtes et événements.