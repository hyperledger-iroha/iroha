---
lang: my
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08d231445c0eb56985d360594393a1fd0fec06b53fdcf8defbe0b2439191ee2f
source_last_modified: "2026-01-22T14:45:01.264878+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-quickstarts
title: Nexus SDK quickstarts
description: Minimal steps for Rust/JS/Swift/Android/CLI SDKs to connect to Sora Nexus.
translator: machine-google-reviewed
---

အမြန်စတင်မှု အပြည့်အစုံသည် `docs/source/nexus_sdk_quickstarts.md` တွင် တည်ရှိသည်။ အဲဒါပေါ်တယ်။
အနှစ်ချုပ်သည် ဆော့ဖ်ဝဲရေးသားသူများအား မျှဝေထားသော ကြိုတင်လိုအပ်ချက်များနှင့် တစ်-SDK အမိန့်များကို မီးမောင်းထိုးပြထားသည်။
၎င်းတို့၏ setup ကို လျှင်မြန်စွာ အတည်ပြုနိုင်သည်။

## မျှဝေထားသော စနစ်ထည့်သွင်းမှု

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus config အတွဲကို ဒေါင်းလုဒ်လုပ်ပါ၊ SDK တစ်ခုစီ၏ မှီခိုမှုကို ထည့်သွင်းပြီး သေချာပါစေ။
TLS လက်မှတ်များသည် ထုတ်ဝေမှုပရိုဖိုင်နှင့် ကိုက်ညီသည် (ကြည့်ရှုပါ။
`docs/source/sora_nexus_operator_onboarding.md`)။

## သံချေး

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Refs: `docs/source/sdk/rust.md`

## JavaScript/TypeScript

```bash
npm run demo:nexus
```

script သည် `ToriiClient` ကို အပေါ်က env vars ဖြင့် instantiates လုပ်ပြီး print ထုတ်သည် ။
နောက်ဆုံးပိတ်။

## လျင်မြန်ခြင်း။

```bash
make swift-nexus-demo
```

`FindNetworkStatus` ကို ရယူရန် `IrohaSwift` မှ `Torii.Client` ကို အသုံးပြုသည်။

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus အဆင့်သတ်မှတ်မှု အဆုံးမှတ်ကို နှိပ်၍ စီမံခန့်ခွဲထားသော စက်ပစ္စည်းစမ်းသပ်မှုကို လုပ်ဆောင်သည်။

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## ပြဿနာဖြေရှင်းခြင်း။

- TLS ကျရှုံးမှုများ → Nexus ထွက်ရှိသည့် tarball မှ CA အစုအဝေးကို အတည်ပြုပါ။
- `ERR_UNKNOWN_LANE` → pass `--lane-id`/`--dataspace-id` တစ်ကြိမ် လမ်းသွားလမ်းကြောင်းပေါင်းစုံ
  ပြဌာန်းထားသည်။
- `ERR_SETTLEMENT_PAUSED` → [Nexus operations](../nexus/nexus-operations) အတွက် စစ်ဆေးပါ
  အဖြစ်အပျက်ဖြစ်စဉ်; အုပ်ချုပ်ရေးက လမ်းကြောကို ခေတ္တရပ်ထားနိုင်တယ်။

ပိုမိုနက်နဲသော အကြောင်းအရာနှင့် SDK ဆိုင်ရာ ရှင်းလင်းချက်များအတွက် ကြည့်ပါ။
`docs/source/nexus_sdk_quickstarts.md`။