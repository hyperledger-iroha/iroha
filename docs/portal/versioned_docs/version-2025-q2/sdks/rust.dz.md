---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T14:35:36.896251+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# རསཊ་ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་པ།

རསཊ་མཁོ་མངགས་ཨེ་པི་ཨའི་ `iroha` cret ནང་ལུ་སྡོད་དོ་ཡོདཔ་ད་ དེ་གིས་ `client::Client` གསལ་སྟོན་འབདཝ་ཨིན།
Torii ལུ་སླབ་ནིའི་དོན་ལུ་ ཡིག་དཔར་རྐྱབས། ཁྱོད་ཀྱིས་ ཚོང་འབྲེལ་ཚུ་ བཙུགས་དགོཔ་ད་ ལག་ལེན་འཐབ།
བྱུང་ལས་ཚུ་ལུ་ མཉམ་བསྡོམས་འབད་ ཡང་ན་ རཱསིཊི་གློག་རིམ་ལས་ འདྲི་དཔྱད་གནས་སྟངས་ཨིན།

## 1. ཀྲེག་བསྣན།

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

ལཱ་གི་ས་སྒོ་འདི་གིས་ `client` ཁྱད་རྣམ་བརྒྱུད་དེ་ མཁོ་སྤྲོད་འབད་མི་ཚད་གཞི་འདི་ ལྡེ་མིག་ཕྱེཝ་ཨིན། ཁྱོད།
དཔར་བསྐྲུན་འབད་ཡོད་པའི་ཀེརེཊི་འདི་ཟ་སྤྱོད་འབད་ཞིནམ་ལས་ ད་ལྟོའི་ད་ལྟོའི་འདི་གིས་ `path` ཁྱད་ཆོས་འདི་ཚབ་བཙུགསཔ་ཨིན།
ཐོན་རིམ་ཡིག་རྒྱུན་།

## 2. མཁོ་སྤྲོད་པ་རིམ་སྒྲིག་འབད།

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

Norito གིས་ CLI རིམ་སྒྲིག་ཡིག་སྣོད་འདི་ གཟུགས་བརྙན་བཟོཝ་ཨིན།
telemetry URLs བདེན་བཤད་རྒྱུ་ཆ་ དུས་ཚོད་རྫོགས་ནི་ དེ་ལས་ བེཆ་དགའ་གདམ་ཚུ།

## 3. ཚོང་འབྲེལ་ཅིག་ཕུལ་ནི།

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

ཁ་དོག་འོག་ལུ་ མཁོ་མངགས་འབད་མི་གིས་ Norito འདི་ ཧེ་མ་ལས་ ཚོང་འབྲེལ་གྱི་ པེ་ལོཌི་འདི་ ཧེ་མ་ལས་ ཨིན་ཀོ་ཌིང་འབད་ནི་གི་དོན་ལུ་ ལག་ལེན་འཐབ་ཨིན།
Torii ལུ་བཙུགས་ནི། ཞུ་ཡིག་འདི་མཐར་འཁྱོལ་ཅན་ཨིན་པ་ཅིན་ སླར་ལོག་འབད་མི་ ཧ་ཤི་འདི་ ལག་ལེན་འཐབ་བཏུབ།
བརྟག་ཞིབ་གནས་རིམ་ `client.poll_transaction_status(hash)` བརྒྱུད་དེ་ཨིན།

## 4. DA blobs ཕུལ་བ།

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

ཁྱོད་ཀྱིས་ Norito གི་སྤྲོད་ལེན་འདི་ བརྟག་དཔྱད་འབད་དགོཔ་ད་ དེ་ ལུ་མ་གཏང་པར་
Torii, མཚན་རྟགས་བཀོད་ཡོད་པའི་ཞུ་བ་འདི་ཐོབ་ནིའི་དོན་ལུ་ `client.build_da_ingest_request(...)` ལུ་ ཁ་པར་གཏང་།
དང་ JSON/bytes སྦེ་སྟོན་ཞིནམ་ལས་ `iroha app da submit --no-submit` མེ་ལོང་བཟོཝ་ཨིན།

## 5. འདྲི་དཔྱད་གནས་སྡུད།

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

འདྲི་དཔྱད་ཚུ་གིས་ ཞུ་བ་/ལན་འདེབས་དཔེ་གཞི་ལུ་རྗེས་སུ་འཇུག་སྟེ་ འདྲི་དཔྱད་ཀྱི་དབྱེ་བ་འདི་ ལས་བཟོ་བསྐྲུན་འབད།
`iroha_data_model::query`, `client.request` བརྒྱུད་དེ་གཏང་ཞིནམ་ལས་ བསྐྱར་དུ་བསྐྱར་ཟློས་འབད་དགོ།
གྲུབ་འབྲས། ལན་ཚུ་གིས་ Norito-backed JSON ལག་ལེན་འཐབ་དོ་ཡོདཔ་ལས་ གློག་ཐག་རྩ་སྒྲིག་འདི་ གཏན་འབེབས་བཟོཝ་ཨིན།

## 6. བྱུང་རིམ་ལ་མྱུན།

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

མཁོ་མངགས་འབད་མི་གིས་ Torii གི་ SSE མཇུག་བསྡུའི་དོན་ལུ་ async tways, དེ་ཡང་ པའིཔ་ལཱའིན་རྩིས་ཏེ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
བྱུང་ལས་དང་ གནད་སྡུད་བྱུང་ལས་ དེ་ལས་ ཊེ་ལི་མི་ཊི་ཕིཌི་ཚུ།

## དཔེར་བརྗོད།

- མཐའ་མའི་རྒྱུན་འབབ་འདི་ `tests/` འོག་ལུ་ `crates/iroha` གི་འོག་ལུ་ཡོདཔ་ཨིན། མཉམ་བསྡོམས་འཚོལ།
  བརྟག་དཔྱད་ཚུ་ དཔེར་ན་ `transaction_submission.rs` ཚུ་ ཕྱུགཔོ་བཟོ་བའི་དོན་ལུ་ཨིན།
- སི་ཨེལ་ཨའི་ (`iroha_cli`) གིས་ མཁོ་སྤྲོད་འབད་མི་ཚད་གཞི་གཅིག་པ་ལག་ལེན་འཐབ་ཨིན། བལྟ་; བལྟ་ནི
  `crates/iroha_cli/src/` བདེན་བཤད་དང་ བེཆ་ དེ་ལས་ བསྐྱར་བཟློས་ཚུ་ ག་དེ་སྦེ་ཨིན་ན་ བལྟ་ནི་ལུ་ བལྟ་ཚུགས།
  བཀོལ་སྤྱོད་འབད་ནི། བཟོ་བསྐྲུན་ལག་ཆས།
- སེམས་ཁར་བཞག་དགོཔ་འདི་ སེམས་ཁར་བཞག་དགོ། ཁྱེད་རང་གི་ཚེ།
  SDK རྒྱ་སྐྱེད་འབད་ནི། `norito::json` གྲོགས་རམ་པ་ཚུ་ལུ་ JSON མཐའ་མཚམས་དང་ དང་ བརྟེན་དགོ།
  `norito::codec` གཉིས་ལྡན་གྱི་པེ་ལོཊི་ཚུ་གི་དོན་ལུ་.

འ་ནི་སྒྲིང་ཁྱིམ་གྱི་སྡེབ་ཚན་ཚུ་གིས་ ཁྱོད་ཀྱིས་ Torii འདི་ Rust ཞབས་ཏོག་ཡང་ན་ CLIs ནང་ལུ་ མཉམ་བསྡོམས་འབད་ཚུགས།
བཟོ་བཏོན་འབད་ཡོད་པའི་ཡིག་ཆ་དང་ གནད་སྡུད་-དཔེ་ཚད་ཀེརེསི་ལུ་ ཆ་ཚང་ཆ་ཚང་གི་དོན་ལུ་ གཞི་བསྟུན་འབད།
བཀོད་རྒྱ་དང་འདྲི་དཔྱད་ དེ་ལས་བྱུང་ལས་ཚུ།