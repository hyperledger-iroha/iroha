---
lang: ka
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rust SDK Quickstart

Rust კლიენტის API ცხოვრობს `iroha` ყუთში, რომელიც ავლენს `client::Client`-ს
აკრიფეთ Torii-თან სასაუბროდ. გამოიყენეთ ის, როდესაც გჭირდებათ ტრანზაქციის წარდგენა,
გამოიწერეთ ღონისძიებები, ან მოითხოვეთ მდგომარეობა Rust აპლიკაციიდან.

## 1. დაამატეთ კრატი

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

სამუშაო სივრცის მაგალითი განბლოკავს კლიენტის მოდულს `client` ფუნქციის მეშვეობით. თუ თქვენ
მოიხმარეთ გამოქვეყნებული ყუთი, შეცვალეთ `path` ატრიბუტი მიმდინარე
ვერსიის სტრიქონი.

## 2. კლიენტის კონფიგურაცია

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

`ClientConfiguration` ასახავს CLI კონფიგურაციის ფაილს: მოიცავს Torii და
ტელემეტრიის URL-ები, ავთენტიფიკაციის მასალა, ვადები და სერიული პარამეტრები.

## 3. წარადგინეთ ტრანზაქცია

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

ქუდის ქვეშ კლიენტი იყენებს Norito-ს ტრანზაქციის დატვირთვის დაშიფვრად მანამდე
განთავსდება Torii-ზე. თუ წარდგენა წარმატებით დასრულდა, დაბრუნებული ჰეში შეიძლება გამოყენებულ იქნას
ტრეკის სტატუსი `client.poll_transaction_status(hash)`-ის საშუალებით.

## 4. წარადგინეთ DA blobs

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

როდესაც გჭირდებათ Norito ტვირთის შემოწმება ან შენარჩუნება მის გაგზავნის გარეშე
Torii, დარეკეთ `client.build_da_ingest_request(...)` ხელმოწერილი მოთხოვნის მისაღებად
და გადაიყვანეთ როგორც JSON/ბაიტი, `iroha app da submit --no-submit`-ის ასახვით.

## 5. შეკითხვის მონაცემები

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

მოთხოვნები მიჰყვება მოთხოვნის/პასუხის შაბლონს: შექმენით შეკითხვის ტიპი
`iroha_data_model::query`, გაგზავნეთ `client.request`-ით და გაიმეორეთ
შედეგები. პასუხებში გამოიყენება Norito მხარდაჭერილი JSON, ამიტომ მავთულის ფორმატი განმსაზღვრელია.

## 6. Explorer QR სნეპშოტები

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "<i105-account-id>",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` ასახავს `/v1/explorer/accounts/{id}/qr` JSON-ს
ზედაპირი: მასში შედის ანგარიშის კანონიკური id,
კანონიკური i105 literal, ქსელის პრეფიქსი/შეცდომის გამოსწორების მეტამონაცემები, QR ზომები და
inline SVG დატვირთვა, რომელიც საფულეებს/გამომძიებლებს შეუძლიათ პირდაპირ ჩასვან.

## 7. გამოიწერეთ ღონისძიებები

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

კლიენტი ავლენს ასინქრონულ ნაკადებს Torii-ის SSE ბოლო წერტილებისთვის, მილსადენის ჩათვლით
მოვლენები, მონაცემთა მოვლენები და ტელემეტრიის არხები.

## მეტი მაგალითი

- ბოლოდან ბოლომდე ნაკადები ცხოვრობს `tests/` ქვეშ `crates/iroha`-ში. მოძებნეთ ინტეგრაცია
 ტესტები, როგორიცაა `transaction_submission.rs` უფრო მდიდარი სცენარისთვის.
- CLI (`iroha_cli`) იყენებს იმავე კლიენტის მოდულს; დაათვალიერეთ
 `crates/iroha_cli/src/`, რათა ვნახო, როგორია ავთენტიფიკაცია, ჯგუფური და ხელახალი ცდები
 დამუშავებული წარმოების ხელსაწყოებში.
- გაითვალისწინეთ Norito: კლიენტი არასოდეს იბრუნებს `serde_json`-ს. როცა შენ
 გააფართოვეთ SDK, დაეყრდნოთ `norito::json` დამხმარეებს JSON ბოლო წერტილებისთვის და
 `norito::codec` ბინარული დატვირთვისთვის.

## დაკავშირებული Norito მაგალითები

- [Hajimari შესვლის წერტილის ჩონჩხი] (../norito/examples/hajimari-entrypoint) - შედგენა, გაშვება და დანერგვა
 მინიმალური Kotodama ხარაჩო, რომელიც ასახავს დაყენების ფაზას ამ სწრაფი დაწყებისას.
- [დომენის და ზარაფხანის აქტივების რეგისტრაცია] (../norito/examples/register-and-mint) — შეესაბამება
 `Register` + `Mint` ნაკადი ნაჩვენებია ზემოთ, ასე რომ თქვენ შეგიძლიათ გაიმეოროთ იგივე ოპერაციები კონტრაქტიდან.
- [აქტივის გადარიცხვა ანგარიშებს შორის] (../norito/examples/transfer-asset) — აჩვენებს
 `transfer_asset` syscall იგივე ანგარიშის ID-ებით, რომლებსაც SDK სწრაფი სტარტები იყენებს.

ამ სამშენებლო ბლოკებით შეგიძლიათ Torii ინტეგრირდეთ Rust სერვისებში ან CLI-ებში.
იხილეთ გენერირებული დოკუმენტაცია და მონაცემთა მოდელის ყუთები სრული ნაკრებისთვის
ინსტრუქციები, შეკითხვები და მოვლენები.