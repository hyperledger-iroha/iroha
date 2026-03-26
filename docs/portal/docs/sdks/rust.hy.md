---
lang: hy
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:11:30.697433+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK Quickstart

Rust client API-ն ապրում է `iroha` տուփում, որը բացահայտում է `client::Client`
մուտքագրեք Torii-ի հետ խոսելու համար: Օգտագործեք այն, երբ անհրաժեշտ է գործարքներ ներկայացնել,
բաժանորդագրվել իրադարձություններին կամ հարցումներ կատարել Rust հավելվածից:

## 1. Ավելացնել արկղը

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Աշխատանքային տարածքի օրինակը բացում է հաճախորդի մոդուլը `client` ֆունկցիայի միջոցով: Եթե դուք
սպառեք հրապարակված արկղը, փոխարինեք `path` հատկանիշը ընթացիկով
տարբերակի տող.

## 2. Կարգավորեք հաճախորդը

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

`ClientConfiguration`-ը արտացոլում է CLI կազմաձևման ֆայլը. այն ներառում է Torii և
հեռաչափության URL-ներ, նույնականացման նյութեր, ժամանակի ընդհատումներ և փաթեթավորման նախապատվություններ:

## 3. Ներկայացրե՛ք գործարք

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

Կափարիչի տակ հաճախորդը օգտագործում է Norito գործարքի բեռնվածությունը նախկինում կոդավորելու համար
տեղադրելով այն Torii-ում: Եթե ներկայացումը հաջողվի, վերադարձված հեշը կարող է օգտագործվել
Հետևել կարգավիճակը `client.poll_transaction_status(hash)`-ի միջոցով:

## 4. Ներկայացրե՛ք DA բլբեր

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

Երբ դուք պետք է ստուգեք կամ պահպանեք Norito օգտակար բեռը՝ առանց այն ուղարկելու
Torii, զանգահարեք `client.build_da_ingest_request(...)`՝ ստորագրված հարցումը ստանալու համար
և արտապատկերեք այն JSON/bytes ձևաչափով՝ արտացոլելով `iroha app da submit --no-submit`:

## 5. Հարցման տվյալներ

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

Հարցումները հետևում են հարցում/պատասխանի օրինակին. կառուցեք հարցման տեսակը
`iroha_data_model::query`, ուղարկեք այն `client.request`-ով և կրկնեք
արդյունքները։ Պատասխաններն օգտագործում են Norito-ով ապահովված JSON, ուստի հաղորդալարի ձևաչափը որոշիչ է:

## 6. Explorer QR նկարներ

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "soraカタカナ...",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot`-ը արտացոլում է `/v1/explorer/accounts/{id}/qr` JSON-ը
մակերես. այն ներառում է կանոնական հաշվի id-ն, բառացի թարգմանվածը
կանոնական i105 literal, ցանցի նախածանցը/սխալների ուղղման մետատվյալները, QR չափերը և
ներկառուցված SVG ծանրաբեռնվածությունը, որը դրամապանակները/հետախույզները կարող են ուղղակիորեն տեղադրել: Բաց թողնել

## 7. Բաժանորդագրվեք միջոցառումներին

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

Հաճախորդը ցուցադրում է համաժամանակյա հոսքեր Torii-ի SSE վերջնակետերի համար, ներառյալ խողովակաշարը
իրադարձություններ, տվյալների իրադարձություններ և հեռաչափության հոսքեր:

## Լրացուցիչ օրինակներ

- Ծայրից ծայր հոսքերը ապրում են `tests/`-ի ներքո՝ `crates/iroha`-ում: Ինտեգրման որոնում
 թեստեր, ինչպիսիք են `transaction_submission.rs` ավելի հարուստ սցենարների համար:
- CLI (`iroha_cli`) օգտագործում է նույն հաճախորդի մոդուլը; թերթել
 `crates/iroha_cli/src/`՝ տեսնելու, թե ինչպես են նույնականացումը, փաթեթավորումը և կրկնվող փորձերը
 մշակվում է արտադրական գործիքավորման մեջ:
- Նկատի ունեցեք Norito-ը. հաճախորդը երբեք չի վերադառնում `serde_json`-ին: Երբ դու
 ընդլայնել SDK-ն, ապավինել `norito::json` օգնականներին JSON վերջնակետերի և
 `norito::codec` երկուական բեռների համար:

## Առնչվող Norito օրինակներ

- [Hajimari մուտքի կետի կմախք] (../norito/examples/hajimari-entrypoint) - կազմել, գործարկել և տեղակայել
 նվազագույն Kotodama փայտամած, որը արտացոլում է տեղադրման փուլն այս արագ մեկնարկի ժամանակ:
- [Գրանցեք տիրույթի և դրամահատարանի ակտիվները] (../norito/examples/register-and-mint) — համընկնում է
 Վերևում ներկայացված `Register` + `Mint` հոսքը, որպեսզի կարողանաք վերարտադրել նույն գործողությունները պայմանագրով:
- [Ակտիվների փոխանցում հաշիվների միջև] (../norito/examples/transfer-asset) — ցույց է տալիս
 `transfer_asset` syscall նույն հաշվի ID-ներով, որոնք օգտագործում են SDK-ի արագ մեկնարկները:

Այս շինանյութերի միջոցով դուք կարող եք ինտեգրել Torii-ը Rust ծառայությունների կամ CLI-ների մեջ:
Ամբողջական փաթեթի համար տե՛ս ստեղծված փաստաթղթերը և տվյալների մոդելային տուփերը
հրահանգներ, հարցումներ և իրադարձություններ: