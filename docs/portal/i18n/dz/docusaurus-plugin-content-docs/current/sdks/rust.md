---
lang: dz
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# རསཊ་ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་པ།

རསཊ་མཁོ་མངགས་ཨེ་པི་ཨའི་འདི་ `iroha` cret ནང་ལུ་སྡོད་དོ་ཡོདཔ་ད་ དེ་གིས་ `client::Client` གསལ་སྟོན་འབདཝ་ཨིན།
Torii ལུ་སླབ་ནིའི་དོན་ལུ་ཡིག་དཔར་རྐྱབས། ཁྱོད་ཀྱིས་ ཚོང་འབྲེལ་ཚུ་ བཙུགས་དགོཔ་ད་ ལག་ལེན་འཐབ།
བྱུང་ལས་ཚུ་ལུ་ མཉམ་བསྡོམས་འབད་ ཡང་ན་ རཱསིཊི་གློག་རིམ་ལས་ འདྲི་དཔྱད་གནས་སྟངས་ཨིན།

## 1. ཀྲེག་བསྣན།

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

ལཱ་གི་ས་སྒོ་འདི་གིས་ `client` ཁྱད་རྣམ་བརྒྱུད་དེ་ མཁོ་སྤྲོད་འབད་མི་ཚད་གཞི་འདི་ ལྡེ་མིག་ཕྱེཝ་ཨིན། ཁྱོད།
དཔར་བསྐྲུན་འབད་ཡོད་པའི་ཀེརེཊི་འདི་བཟའ་སྤྱོད་འབད་ ད་ལྟོའི་འདི་གིས་ `path` ཁྱད་ཆོས་འདི་ཚབ་བཙུགསཔ་ཨིན།
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

I18NI000000026X གིས་ སི་ཨེལ་ཨའི་རིམ་སྒྲིག་ཡིག་སྣོད་འདི་ གཟུགས་བརྙན་བཟོཝ་ཨིན།
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

ཁ་དོག་འོག་ལུ་ མཁོ་མངགས་འབད་མི་གིས་ I18NT0000001X འདི་ ཧེ་མ་ལས་ བརྗེ་སོར་གྱི་ པེ་ལོཌི་འདི་ སྔོན་མ་ལས་ ཨིན་ཀོ་ཌིང་འབད་ནི་གི་དོན་ལུ་ ལག་ལེན་འཐབ་ཨིན།
I18NT0000008X ལུ་བཙུགས་ནི། ཞུ་ཡིག་འདི་མཐར་འཁྱོལ་ཅན་ཨིན་པ་ཅིན་ སླར་ལོག་འབད་མི་ ཧ་ཤི་འདི་ ལག་ལེན་འཐབ་བཏུབ།
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
Torii, མཚན་རྟགས་བཀོད་པའི་ཞུ་བ་འདི་ཐོབ་ནིའི་དོན་ལུ་ `client.build_da_ingest_request(...)` ལུ་ ཁ་པར་གཏང་།
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
`iroha_data_model::query`, I18NI000000031X བརྒྱུད་དེ་གཏང་ཞིནམ་ལས་ བསྐྱར་དུ་བསྐྱར་ཟློས་འབད་དགོ།
གྲུབ་འབྲས། ལན་ཚུ་གིས་ Norito-backed JSON ལག་ལེན་འཐབ་དོ་ཡོདཔ་ལས་ གློག་ཐག་རྩ་སྒྲིག་འདི་ གཏན་འབེབས་བཟོཝ་ཨིན།

## 6. འཚོལ་ཞིབ་པ་ QR པར་ལེན་པ།

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

`ExplorerAccountQrSnapshot` གིས་ `/v1/explorer/accounts/{id}/qr` JSON
ཁ་ཐོག་: དེ་ནང་ལུ་ ཀེ་ནོ་ནིག་རྩིས་ཐོ་ id དང་ ཡིག་ཆའི་བརྡ་སྟོན་འབད་མི་ཚུ་ཨིན།
ཞུ་བ་འབད་ཡོད་པའི་རྩ་སྒྲིག་, ཡོངས་འབྲེལ་སྔོན་སྒྲིག་/ནོར་འཁྲུལ་-ནོར་བཅོས་མེ་ཊ་ཌེ་ཊ་, ཀིའུ་ཨར་རྒྱ་ཚད་, དང་།
དངུལ་ཁུག་/འཚོལ་ཞིབ་འབད་མི་ ནང་ཐིག་ཨེསི་ཝི་ཇི་ པེ་ལོཌ་འདི་ ཐད་ཀར་དུ་ བཙུགས་ཚུགས། བདོད༌ནི༌; བདོབར༌ནི
I105 ཐོན་འབྲས་ཡང་ན་ གཞི་སྒྲིག་ལུ་ སྔོན་སྒྲིག་འབད་ནི་ལུ་ I18NI000000034X ཨིན།
I18NI000000035X ལ་དྲག་ཤོས་གཉིས་པ་བསྐྱར་ལེན་བྱེད་པར་།
I18NI000000036X གིས་ ADDR-6b གིས་ལག་ལེན་འཐབ་མི་ འགྱུར་ལྡོག་ཅན་།

## 7. བྱུང་རིམ་ཚུ་ མཁོ་སྒྲུབ་འབད་ནི།

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

- མཐའ་མའི་རྒྱུན་འབབ་འདི་ I18NI000000037X འོག་ལུ་ I18NI000000038X ནང་ལུ་ཡོདཔ་ཨིན། མཉམ་བསྡོམས་འཚོལ།
  བརྟག་དཔྱད་ཚུ་ དཔེར་ན་ I18NI000000039X ཚུ་ ཕྱུགཔོ་སྦེ་ཡོད་མི་ཚུ་གི་དོན་ལུ་ཨིན།
- སི་ཨེལ་ཨའི་ (`iroha_cli`) གིས་ མཁོ་སྤྲོད་འབད་མི་ཚད་གཞི་གཅིག་པ་ལག་ལེན་འཐབ་ཨིན། བལྟ་; བལྟ་ནི
  `crates/iroha_cli/src/` བདེན་བཤད་དང་ བེཆ་ དེ་ལས་ བསྐྱར་ལོག་ཚུ་ ག་དེ་སྦེ་ཨིན་ན་ བལྟ་ནི་ལུ་ བལྟ་ནི་ལུ་ བལྟ་ནི་ལུ་ཨིན།
  བཀོལ་སྤྱོད་འབད་ནི། བཟོ་བསྐྲུན་ལག་ཆས།
- སེམས་ཁར་བཞག་དགོཔ་འདི་ སེམས་ཁར་བཞག་དགོ། ཁྱེད་རང་གི་ཚེ།
  SDK རྒྱ་སྐྱེད་འབད་ནི། `norito::json` གྲོགས་རམ་པ་ཚུ་ལུ་ JSON མཇུག་བསྡུའི་ས་ཚིགས་ཚུ་དང་ བརྟེན་དགོ།
  `norito::codec` གཉིས་ལྡན་གྱི་པེ་ལོཊི་ཚུ་གི་དོན་ལུ་.

## འབྲེལ་ཡོད་ Norito དཔེར་ན།

- [ཧ་ཇི་མ་རི་འཛུལ་སྒོ་ ཀེང་རུས་](I18NU0000019X) — བསྡུ་སྒྲིག་འབད་ནི་དང་ གཡོག་བཀོལ་ནི།
  ཉུང་མཐའ་ I1NT00000000X གིས་ མགྱོགས་འགོ་བཙུགས་འདི་ནང་ གཞི་སྒྲིག་འབད་བའི་ གནས་རིམ་འདི་ མེ་ལོང་ནང་ བཏོནམ་ཨིན།
- [མངའ་ཁོངས་དང་ མིན་ཊི་རྒྱུ་དངོས་ཐོ་འགོད་](../norito/examples/register-and-mint) — འདི་དང་གཅིག་ཁར་ཕྲང་སྒྲིག་འབདཝ་ཨིན།
  I18NI000000045X + `Mint` གིས་ གོང་ལུ་སྟོན་ཡོད་མི་དེ་ལུ་ ཁྱོད་ཀྱིས་ གན་རྒྱ་ཅིག་ལས་ བཀོལ་སྤྱོད་གཅིག་མཚུངས་ཚུ་ ལོག་གཏང་ཚུགས།
- [རྩིས་ཐོ་ཚུ་གི་བར་ན་ བརྒྱུད་འཕྲིན་གྱི་རྒྱུ་དངོས་ཚུ་ ] (../norito/examples/transfer-asset) — འདི་སྟོནམ་ཨིན།
  I18NI000000047X རྩིས་ཐོ་གཅིག་པའི་ཨའི་ཌི་ཚུ་ ཨེསི་ཌི་ཀེ་ མགྱོགས་དྲགས་འགོ་བཙུགས་ལག་ལེན་འཐབ་ཨིན།

འ་ནི་སྒྲིང་ཁྱིམ་གྱི་སྡེབ་ཚན་ཚུ་གིས་ ཁྱོད་ཀྱིས་ Torii འདི་ Rust ཞབས་ཏོག་ཡང་ན་ CLIs ནང་ལུ་ མཉམ་བསྡོམས་འབད་ཚུགས།
བཟོ་བཏོན་འབད་ཡོད་པའི་ཡིག་ཆ་དང་ གནད་སྡུད་-དཔེ་ཚད་ཀེརེསི་ལུ་ ཆ་ཚང་ཆ་ཚང་གི་དོན་ལུ་ གཞི་བསྟུན་འབད།
བཀོད་རྒྱ་དང་འདྲི་དཔྱད་ དེ་ལས་བྱུང་ལས་ཚུ།