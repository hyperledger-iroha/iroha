---
lang: my
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rust SDK အမြန်စတင်ပါ။

Rust client API သည် `iroha` သေတ္တာထဲတွင် နေထိုင်သည်၊၊ `client::Client` ကို ဖော်ထုတ်ပေးသည်
Torii ကို စကားပြောရန်အတွက် ရိုက်ထည့်ပါ။ အရောင်းအ၀ယ်ပြုလုပ်ရန် လိုအပ်သောအခါတွင် ၎င်းကိုအသုံးပြုပါ၊
အစီအစဉ်များကို စာရင်းသွင်းပါ သို့မဟုတ် Rust အပလီကေးရှင်းမှ အခြေအနေကို မေးမြန်းပါ။

## 1. ဗူးကိုထည့်ပါ။

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

အလုပ်ခွင်နမူနာသည် `client` အင်္ဂါရပ်မှတစ်ဆင့် သုံးစွဲသူ module ကို လော့ခ်ဖွင့်ပေးသည်။ သင်
ထုတ်ဝေထားသော သေတ္တာကို စားသုံးပါ၊ `path` ရည်ညွှန်းချက်ကို လက်ရှိဖြင့် အစားထိုးပါ။
ဗားရှင်းစာကြောင်း။

## 2. client ကို စီစဉ်သတ်မှတ်ပါ။

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

`ClientConfiguration` သည် CLI ဖွဲ့စည်းမှုပုံစံဖိုင်ကို ထင်ဟပ်စေသည်- ၎င်းတွင် Torii နှင့်
တယ်လီမီတာ URL များ၊ စစ်မှန်ကြောင်းအထောက်အထားပြပစ္စည်း၊ အချိန်ကုန်သွားခြင်း၊ နှင့် အတွဲလိုက်ရွေးချယ်မှုများ။

## 3. ငွေပေးငွေယူတစ်ခု တင်သွင်းပါ။

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

အဖုံးအောက်တွင် ဖောက်သည်သည် ငွေလွှဲခပေးချေမှုအား ကုဒ်သွင်းရန် Norito ကို အသုံးပြုသည်။
Torii မှာ တင်လိုက်တယ်။ တင်ပြမှုအောင်မြင်ပါက၊ ပြန်ပေးထားသော hash ကို အသုံးပြုနိုင်သည်။
`client.poll_transaction_status(hash)` မှတစ်ဆင့် အခြေအနေကို ခြေရာခံပါ။

## 4. DA blobs တင်သွင်းပါ။

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

၎င်းကိုမပို့ဘဲ Norito payload ကို စစ်ဆေးရန် သို့မဟုတ် ဆက်လက်လုပ်ဆောင်ရန် လိုအပ်သည့်အခါ၊
Torii၊ `client.build_da_ingest_request(...)` ကိုခေါ်ဆို၍ လက်မှတ်ရေးထိုးထားသော တောင်းဆိုချက်ကို ရယူပါ။
`iroha app da submit --no-submit` ကို ရောင်ပြန်ဟပ်ကာ JSON/bytes အဖြစ် ပြန်ဆိုပါ။

## 5. Query data

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

မေးမြန်းချက်များသည် တောင်းဆိုမှု/တုံ့ပြန်မှုပုံစံကို လိုက်နာပါ- မှ မေးမြန်းမှုအမျိုးအစားတစ်ခုကို တည်ဆောက်ပါ။
`iroha_data_model::query`၊ ၎င်းကို `client.request` မှတစ်ဆင့် ပေးပို့ပြီး ထပ်လောင်းဖော်ပြပါ
ရလဒ်များ။ တုံ့ပြန်မှုများသည် Norito-ကျောထောက်နောက်ခံ JSON ကိုအသုံးပြုသည်၊ ထို့ကြောင့် ဝါယာဖော်မတ်သည် အဆုံးအဖြတ်ဖြစ်သည်။

## 6. Explorer QR ဓာတ်ပုံများ

```rust
use iroha::client::{
    AddressFormat, Client, ClientConfiguration, ExplorerAccountQrOptions,
};

fn download_qr() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let snapshot = client.get_explorer_account_qr(
        "ih58...",
        Some(ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        }),
    )?;
    println!("Canonical literal: {}", snapshot.literal);
    println!("SVG payload: {}", snapshot.svg);
    Ok(())
}
```

`ExplorerAccountQrSnapshot` သည် `/v1/explorer/accounts/{id}/qr` JSON ကို မှန်သည်
မျက်နှာပြင်- ၎င်းတွင် canonical account id ပါ၀င်သည်၊ ပကတိနှင့်ပြန်ဆိုထားသော
တောင်းဆိုထားသော ဖော်မတ်၊ ကွန်ရက်ရှေ့ဆက်/အမှား-ပြင်ပေးသည့် မက်တာဒေတာ၊ QR အတိုင်းအတာနှင့်
ပိုက်ဆံအိတ်/ရှာဖွေသူများ တိုက်ရိုက်ထည့်သွင်းနိုင်သော အတွင်းလိုင်း SVG ပေးဆောင်မှု။ ချန်လှပ်ပါ။
`ExplorerAccountQrOptions` ကို နှစ်သက်သော IH58 အထွက် သို့မဟုတ် သတ်မှတ်ရန် ပုံသေ
`address_format: Some(AddressFormat::Compressed)` သည် ဒုတိယအကောင်းဆုံးကို ရယူရန်
ADDR-6b အသုံးပြုသော `sora…` ဗားရှင်း။

## 7. ပွဲများကို စာရင်းသွင်းပါ။

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

ဖောက်သည်သည် ပိုက်လိုင်းအပါအဝင် Torii ၏ SSE အဆုံးမှတ်များအတွက် sync stream များကို ဖော်ထုတ်ပေးသည်
ဖြစ်ရပ်များ၊ ဒေတာဖြစ်ရပ်များနှင့် တယ်လီမီတာ ဖိဒ်များ။

## နောက်ထပ်ဥပမာ

- အဆုံးမှအဆုံးသို့စီးဆင်းမှုများသည် `crates/iroha` တွင် `tests/` အောက်တွင် တိုက်ရိုက်နေပါသည်။ ပေါင်းစည်းမှုကို ရှာဖွေပါ။
  ပိုမိုကြွယ်ဝသော အခြေအနေများအတွက် `transaction_submission.rs` ကဲ့သို့သော စမ်းသပ်မှုများ။
- CLI (`iroha_cli`) သည် တူညီသော client module ကိုအသုံးပြုသည်။ ရှာဖွေကြည့်ရှုပါ။
  စစ်မှန်ကြောင်းအထောက်အထားပြခြင်း၊ အတွဲလိုက်ခြင်းနှင့် ထပ်စမ်းခြင်းများ ပြုလုပ်ပုံကိုကြည့်ရန် `crates/iroha_cli/src/`
  ထုတ်လုပ်မှု tooling တွင်ကိုင်တွယ်။
- Norito ကိုစိတ်ထဲထားပါ- client သည် `serde_json` သို့ ဘယ်တော့မှ ပြန်မလာခဲ့ပါ။ သင်လိုက်တာ
  SDK ကို တိုးချဲ့ပါ၊ JSON အဆုံးမှတ်များအတွက် `norito::json` ကူညီပေးသူများကို အားကိုးပြီး
  binary payloads အတွက် `norito::codec`။

## ဆက်စပ် Norito ဥပမာများ

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — compile, run, and deploy
  ဤအမြန်စတင်ခြင်းတွင် တပ်ဆင်မှုအဆင့်ကို ထင်ဟပ်စေသည့် အနိမ့်ဆုံး Kotodama Scaffold။
- [ဒိုမိန်းနှင့် mint ပိုင်ဆိုင်မှုများကို မှတ်ပုံတင်ခြင်း](../norito/examples/register-and-mint) — နှင့် ကိုက်ညီသည်
  အထက်တွင်ပြထားသည့် `Register` + `Mint` စီးဆင်းမှုကြောင့် သင်သည် စာချုပ်တစ်ခုမှ တူညီသောလုပ်ဆောင်မှုများကို ပြန်ဖွင့်နိုင်သည်။
- [အကောင့်များအကြားပိုင်ဆိုင်မှုလွှဲပြောင်းခြင်း](../norito/examples/transfer-asset) — သရုပ်ပြသည်
  SDK အမြန်စတင်အသုံးပြုသည့် တူညီသောအကောင့် ID များဖြင့် `transfer_asset` syscall

ဤအဆောက်အဦလုပ်ကွက်များဖြင့် Torii ကို Rust ဝန်ဆောင်မှုများ သို့မဟုတ် CLI များတွင် ပေါင်းစည်းနိုင်သည်။
အပြည့်အစုံအတွက် ထုတ်လုပ်ထားသော စာရွက်စာတမ်းများနှင့် ဒေတာပုံစံသေတ္တာများကို ကိုးကားပါ။
ညွှန်ကြားချက်များ၊ မေးမြန်းချက်များနှင့် ဖြစ်ရပ်များ။