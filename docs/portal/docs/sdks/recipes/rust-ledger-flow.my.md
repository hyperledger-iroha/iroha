---
lang: my
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0865789ddc73146a5a6065c3326d7c6bde40d9136d51cfe32d244022df6b5cf
source_last_modified: "2026-01-22T16:26:46.513514+00:00"
translation_last_reviewed: 2026-02-07
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
slug: /sdks/recipes/rust-ledger-flow
translator: machine-google-reviewed
---

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

ဤစာရွက်သည် [CLI လယ်ဂျာ လမ်းညွှန်ချက်](../../norito/ledger-walkthrough.md) ကို ထင်ဟပ်စေသည်
သို့သော် Rust binary မှ အရာအားလုံးကို လုပ်ဆောင်သည်။ ၎င်းသည် မူရင်း dev ကွန်ရက်ကို ပြန်သုံးသည်။
(`docker compose -f defaults/docker-compose.single.yml up --build`) နှင့် ဒီမို
`defaults/client.toml` ရှိ အထောက်အထားများ ဖြစ်သောကြောင့် SDK နှင့် CLI hash တစ်ခုကို နှိုင်းယှဉ်နိုင်သည်
တစ်ခုအတွက်။

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/rust/src/main.rs"
  ဖိုင်အမည် = "src/main.rs"
  description="ဤ Rust အရင်းအမြစ်ဖိုင်ကို လိုက်နာရန် သို့မဟုတ် သင်၏ပြောင်းလဲမှုများနှင့် ကွဲပြားစေရန်အတွက် အခြေခံလိုင်းအဖြစ် အသုံးပြုပါ။"
/>

## လိုအပ်ချက်များ

1. Docker Compose ဖြင့် dev peer ကို run ( [Norito အမြန်စတင်ခြင်း](../../norito/quickstart.md))။
2. မူရင်း စီမံခန့်ခွဲသူ/လက်ခံသူ အကောင့်များနှင့် စီမံခန့်ခွဲသူ သီးသန့်သော့ကို ထုတ်ယူပါ။
   `defaults/client.toml`-

   ```bash
   export ADMIN_ACCOUNT="i105..."
   export RECEIVER_ACCOUNT="i105..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   သီးသန့်သော့စာကြောင်းသည် `[account].private_key` အောက်တွင် သိမ်းဆည်းထားသည့် multihash-encoded တန်ဖိုးဖြစ်သည်။
3. workspace binary အသစ်တစ်ခု ဖန်တီးပါ (သို့မဟုတ် ရှိပြီးသားတစ်ခုကို ပြန်သုံးပါ)။

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. မှီခိုမှုများကို ပေါင်းထည့်ပါ (သင်ပြင်ပတွင်ရှိနေပါက crates.io ဗားရှင်းကို အသုံးပြုပါ။
   အလုပ်ခွင်):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## နမူနာအစီအစဉ်

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

## စာရွက်ကို Run လိုက်ပါ။

```bash
cargo run
```

မှတ်တမ်းအထွက်ကို သင်မြင်ရပါမည်-

```
i105... now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် ရှိနှင့်ပြီးပါက၊ မှတ်ပုံတင်ရန်ခေါ်ဆိုမှုသည် a ကို ပြန်ပေးသည်။
`ValidationError::Duplicate`။ လျစ်လျူရှုသည်ဖြစ်စေ (မိုင်းသည် အောင်မြင်ဆဲဖြစ်သည်) သို့မဟုတ် ရွေးပါ။
နာမည်အသစ်။

## hash နှင့် တူညီမှုကို စစ်ဆေးပါ။

- `iroha --config defaults/client.toml transaction get --hash <hash>` ကိုသုံးပါ။
  SDK တင်သွင်းသည့် အရောင်းအ၀ယ်များကို စစ်ဆေးပါ။
- `iroha --config defaults/client.toml asset list all --table` ဖြင့် အပြန်အလှန်စစ်ဆေးသော လက်ကျန်ငွေ
  သို့မဟုတ် `asset list filter '{"id":"norito:4e52543000000002"}'`။
- မျက်နှာပြင်နှစ်ခုလုံးမှထုတ်လုပ်ကြောင်းအတည်ပြုရန် CLI လမ်းညွှန်ချက်မှတူညီသောစီးဆင်းမှုကိုပြန်လုပ်ပါ။
  တူညီသော Norito ပေးချေမှုများနှင့် ငွေပေးငွေယူ အခြေအနေများ။