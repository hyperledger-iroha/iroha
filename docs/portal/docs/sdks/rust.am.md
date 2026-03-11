---
lang: am
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:11:30.697433+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ዝገት ኤስዲኬ ፈጣን ጅምር

የ Rust ደንበኛ ኤፒአይ በ`iroha` ሳጥን ውስጥ ይኖራል፣ ይህም `client::Client`ን ያጋልጣል።
ከ Torii ጋር ለመነጋገር ይተይቡ። ግብይቶችን ለማስገባት ሲፈልጉ ይጠቀሙበት፣
ለክስተቶች ደንበኝነት ይመዝገቡ, ወይም የጥያቄ ሁኔታ ከ Rust መተግበሪያ.

## 1. ሣጥኑን ይጨምሩ

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

የስራ ቦታ ምሳሌ የደንበኛ ሞጁሉን በI18NI0000024X ባህሪ በኩል ይከፍታል። አንተ ከሆነ
የታተመውን ሣጥን ይበላል፣ የ`path` አይነታውን አሁን ባለው ይተኩ
የስሪት ሕብረቁምፊ.

## 2. ደንበኛውን ያዋቅሩ

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

`ClientConfiguration` የCLI ውቅር ፋይልን ያንጸባርቃል፡ Torii እና
የቴሌሜትሪ ዩአርኤሎች፣ የማረጋገጫ ቁሳቁስ፣ የጊዜ ማብቂያዎች እና የባንግ ምርጫዎች።

## 3. ግብይት አስገባ

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

በኮድ ስር ደንበኛው የግብይቱን ክፍያ ከዚህ በፊት ለማስቀመጥ Norito ይጠቀማል።
ወደ Torii በመለጠፍ ላይ። ማስረከብ ከተሳካ፣ የተመለሰው ሃሽ ጥቅም ላይ ሊውል ይችላል።
የትራክ ሁኔታ በ `client.poll_transaction_status(hash)`.

## 4. DA blobs አስረክብ

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

የ Norito ክፍያን ወደ እሱ ሳይልኩ መመርመር ወይም መቀጠል ሲፈልጉ
Torii፣ የተፈረመውን ጥያቄ ለማግኘት ወደ `client.build_da_ingest_request(...)` ይደውሉ
እና `iroha app da submit --no-submit` በማንጸባረቅ እንደ JSON/ባይት አድርገው።

## 5. የመጠይቅ ውሂብ

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

መጠይቆች የጥያቄውን/የምላሽ ስርአቱን ይከተሉ፡ የመጠይቁን አይነት ከ
`iroha_data_model::query`፣ በ`client.request` ይላኩት እና ይድገሙት
ውጤቶች. ምላሾች I18NT0000003X የሚደገፍ JSON ይጠቀማሉ፣ ስለዚህ የሽቦ ቅርጸቱ የሚወስነው ነው።

## 6. Explorer QR ቅጽበተ-ፎቶዎች

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

`ExplorerAccountQrSnapshot` `/v1/explorer/accounts/{id}/qr` JSON ያንጸባርቃል
ላዩን፡ እሱ የቀኖናዊ መለያ መታወቂያን፣ ቀጥተኛውን ከ. ጋር ያካትታል
የተጠየቀው ቅርጸት፣ የአውታረ መረብ ቅድመ ቅጥያ/ስህተት ማስተካከያ ዲበ ውሂብ፣ የQR ልኬቶች፣ እና
የኪስ ቦርሳ/አሳሾች በቀጥታ ሊከተቱት የሚችሉት የውስጠ-መስመር SVG ጭነት። ተወው
`ExplorerAccountQrOptions` ወደ ተመራጭ I105 ውፅዓት ወይም ስብስብ ነባሪ
ሁለተኛውን ምርጡን ለማውጣት canonical I105 output
በADDR-6b ጥቅም ላይ የዋለው `i105` ልዩነት።

## 7. ለክስተቶች ይመዝገቡ

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

ደንበኛው የቧንቧ መስመርን ጨምሮ ለTorii's SSE የመጨረሻ ነጥቦችን ያልተመሳሰሉ ዥረቶችን ያጋልጣል
ክስተቶች፣ የውሂብ ክስተቶች እና የቴሌሜትሪ ምግቦች።

## ተጨማሪ ምሳሌዎች

- ከጫፍ እስከ ጫፍ ፍሰቶች በ`tests/` በ `crates/iroha` ስር ይኖራሉ። ውህደትን ይፈልጉ
  ለበለጸጉ ሁኔታዎች እንደ `transaction_submission.rs` ያሉ ሙከራዎች።
- CLI (I18NI0000040X) ተመሳሳይ የደንበኛ ሞጁል ይጠቀማል; ማሰስ
  `crates/iroha_cli/src/` የማረጋገጫ፣ የመጥመቂያ እና የድጋሚ ሙከራዎች እንዴት እንደሆኑ ለማየት
  በምርት መሳሪያዎች ውስጥ ተይዟል.
- Noritoን ያስታውሱ፡ ደንበኛው ወደ `serde_json` አይመለስም። እርስዎ ሲሆኑ
  ኤስዲኬን ያራዝሙ፣ በ`norito::json` ረዳቶች ለJSON የመጨረሻ ነጥቦች እና
  `norito::codec` ለሁለትዮሽ ጭነት።

## ተዛማጅ Norito ምሳሌዎች

- [የሀጂማሪ መግቢያ ነጥብ አጽም](../norito/examples/hajimari-entrypoint) - ማጠናቀር፣ ማስኬድ እና ማሰማራት
  በዚህ ፈጣን ጅምር ውስጥ የማዋቀር ደረጃውን የሚያንፀባርቀው ዝቅተኛው I18NT0000000X ስካፎል።
- [ጎራ እና ሚንት ንብረቶችን ይመዝገቡ](../norito/examples/register-and-mint) - ከ
  ከላይ የሚታየው `Register` + I18NI0000046X ፍሰት ተመሳሳይ ክንውኖችን ከውል ማጫወት ይችላሉ።
- (ንብረቱን በመለያዎች መካከል ያስተላልፉ)(../norito/examples/transfer-asset) - ያሳያል
  `transfer_asset` syscall ኤስዲኬ ፈጣን ጅማሬዎች ከሚጠቀሙት ተመሳሳይ መለያ መታወቂያዎች ጋር።

በእነዚህ የግንባታ ብሎኮች Torii ወደ Rust አገልግሎቶች ወይም CLIዎች ማዋሃድ ይችላሉ።
ለሙሉ ስብስብ የመነጩ ሰነዶችን እና የውሂብ-ሞዴል ሳጥኖችን ይመልከቱ
መመሪያዎች, ጥያቄዎች እና ክስተቶች.