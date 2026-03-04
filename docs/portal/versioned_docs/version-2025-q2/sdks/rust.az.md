---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T14:35:36.896251+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK Quickstart

Rust müştəri API-si `iroha` qutusunda yaşayır və `client::Client`-i ifşa edir.
Torii ilə danışmaq üçün yazın. Əməliyyatları təqdim etmək lazım olduqda istifadə edin,
hadisələrə abunə olun və ya Rust tətbiqindən vəziyyəti sorğulayın.

## 1. Sandığı əlavə edin

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

İş sahəsi nümunəsi `client` funksiyası vasitəsilə müştəri modulunun kilidini açır. Əgər sən
dərc edilmiş qutunu istehlak edin, `path` atributunu cari ilə əvəz edin
versiya sətri.

## 2. Müştərini konfiqurasiya edin

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

`ClientConfiguration` CLI konfiqurasiya faylını əks etdirir: ona Torii və daxildir
telemetriya URL-ləri, autentifikasiya materialı, fasilələr və toplu seçimlər.

## 3. Əməliyyat təqdim edin

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

Başlıq altında müştəri əməliyyat yükünü kodlaşdırmaq üçün Norito istifadə edir.
onu Torii ünvanına göndərin. Təqdimat uğurlu olarsa, qaytarılan hash üçün istifadə edilə bilər
statusu `client.poll_transaction_status(hash)` vasitəsilə izləyin.

## 4. DA bloblarını təqdim edin

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

Norito faydalı yükünü göndərmədən yoxlamaq və ya saxlamaq lazım olduqda
Torii, imzalanmış sorğunu əldə etmək üçün `client.build_da_ingest_request(...)` nömrəsinə zəng edin
və onu `iroha app da submit --no-submit` əks etdirərək JSON/bayt kimi göstərin.

## 5. Sorğu məlumatı

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

Sorğular sorğu/cavab nümunəsinə uyğundur: sorğu növü qurun
`iroha_data_model::query`, onu `client.request` vasitəsilə göndərin və təkrarlayın
nəticələr. Cavablar Norito dəstəkli JSON-dan istifadə edir, ona görə də tel formatı deterministikdir.

## 6. Tədbirlərə abunə olun

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

Müştəri, boru kəməri də daxil olmaqla, Torii-in SSE son nöqtələri üçün asinxron axınları ifşa edir.
hadisələr, məlumat hadisələri və telemetriya lentləri.

## Daha çox nümunə

- Uçdan uca axınlar `crates/iroha`-də `tests/` altında yaşayır. İnteqrasiya axtarın
  daha zəngin ssenarilər üçün `transaction_submission.rs` kimi testlər.
- CLI (`iroha_cli`) eyni müştəri modulundan istifadə edir; gözdən keçirin
  `crates/iroha_cli/src/` autentifikasiya, toplulaşdırma və təkrar cəhdlərin necə olduğunu görmək
  istehsal alətlərində idarə olunur.
- Norito-i yadda saxlayın: müştəri heç vaxt `serde_json`-ə qayıtmır. Sən zaman
  SDK-nı genişləndirin, JSON son nöqtələri üçün `norito::json` köməkçilərinə etibar edin və
  İkili yüklər üçün `norito::codec`.

Bu tikinti blokları ilə siz Torii-i Rust xidmətlərinə və ya CLI-lərə inteqrasiya edə bilərsiniz.
Tam dəst üçün yaradılan sənədlərə və məlumat modeli qutularına baxın
təlimatlar, sorğular və hadisələr.