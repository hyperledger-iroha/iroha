---
slug: /norito/ledger-walkthrough
lang: my
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ဤဖော်ပြချက်သည် [Norito အမြန်စတင်ခြင်း](./quickstart.md) ကိုပြသခြင်းဖြင့် အားဖြည့်ပေးသည်
`iroha` CLI ဖြင့် လယ်ဂျာပြည်နယ်ကို ဘယ်လိုပြောင်းပြီး စစ်ဆေးမလဲ။ စာရင်းသွင်းရလိမ့်မယ်။
ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်အသစ်၊ အချို့ယူနစ်များကို မူရင်းအော်ပရေတာအကောင့်သို့ လွှဲပြောင်းပါ၊ လွှဲပြောင်းပါ။
လက်ကျန်ငွေ၏တစ်စိတ်တစ်ပိုင်းကို အခြားအကောင့်သို့ လွှဲပြောင်းပေးပြီး ရလဒ်များကို စိစစ်ပါ။
နှင့် ပိုင်ဆိုင်မှု။ အဆင့်တစ်ဆင့်ချင်းစီသည် Rust/Python/JavaScript တွင် ပါရှိသော စီးဆင်းမှုများကို ထင်ဟပ်စေသည်။
SDK သည် CLI နှင့် SDK အပြုအမူကြား တူညီမှုကို အတည်ပြုနိုင်သောကြောင့် SDK အမြန်စတင်ပါသည်။

## လိုအပ်ချက်များ

- တစ်ခုတည်းသောရွယ်တူကွန်ရက်ကိုဖွင့်ရန် [အမြန်စတင်ခြင်း](./quickstart.md) ကို လိုက်နာပါ။
  `docker compose -f defaults/docker-compose.single.yml up --build`။
- `iroha` (CLI) ကို တည်ဆောက်ထားသည် သို့မဟုတ် ဒေါင်းလုဒ်လုပ်ထားပြီး သင်ရောက်ရှိနိုင်သည်ကို သေချာပါစေ။
  `defaults/client.toml` ကို အသုံးပြုထားသည့် အမျိုးအစား။
- ရွေးချယ်နိုင်သော အကူအညီများ- `jq` (JSON တုံ့ပြန်မှုများကို ဖော်မတ်ပေးခြင်း) နှင့် POSIX ခွံတစ်ခု၊
  အောက်တွင်အသုံးပြုထားသော ပတ်ဝန်းကျင်-ပြောင်းလဲနိုင်သော အတိုအထွာများ။

လမ်းညွှန်တစ်ခုလုံးတွင် `$ADMIN_ACCOUNT` နှင့် `$RECEIVER_ACCOUNT` ကို အစားထိုးပါ။
သင်အသုံးပြုရန် စီစဉ်ထားသော အကောင့် ID များ။ မူရင်းအစုအဝေးတွင် အကောင့်နှစ်ခု ပါဝင်ပြီးဖြစ်သည်။
သရုပ်ပြသော့များမှ ဆင်းသက်လာသည်-

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

ပထမအကောင့်အနည်းငယ်ကို စာရင်းပြုစုခြင်းဖြင့် တန်ဖိုးများကို အတည်ပြုပါ-

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## ၁။ ဥပါဒ်အခြေအနေကို စစ်ဆေးပါ။

CLI က ပစ်မှတ်ထားသော စာရင်းဇယားကို ရှာဖွေခြင်းဖြင့် စတင်ပါ-

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

ဤ command များသည် Norito ကျောထောက်နောက်ခံပြုထားသော တုံ့ပြန်မှုများအပေါ် အားကိုးသောကြောင့် filtering နှင့် pagination သည်
အဆုံးအဖြတ်ပေးပြီး SDK များရရှိသည့်အရာနှင့် ကိုက်ညီသည်။

## 2. ပိုင်ဆိုင်မှုအဓိပ္ပါယ်ဖွင့်ဆိုချက်ကို မှတ်ပုံတင်ပါ။

`wonderland` အတွင်းရှိ `coffee` ဟုခေါ်သော အကန့်အသတ်မရှိ သေးငယ်သော ပိုင်ဆိုင်မှုအသစ်ကို ဖန်တီးပါ
ဒိုမိန်း-

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI သည် တင်ပြထားသော ငွေပေးငွေယူ hash ကို print ထုတ်သည် (ဥပမာ၊
`0x5f…`)။ အခြေအနေကို နောက်မှ မေးမြန်းနိုင်စေရန် ၎င်းကို သိမ်းဆည်းပါ။

## 3. အော်ပရေတာအကောင့်ထဲသို့ Mint ယူနစ်များ

ပိုင်ဆိုင်မှုပမာဏများသည် `(asset definition, account)` အတွဲအောက်တွင် နေထိုင်ပါသည်။ Mint 250
`coffee#wonderland` ၏ ယူနစ် `$ADMIN_ACCOUNT` သို့

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

တဖန်၊ CLI အထွက်မှ ငွေပေးငွေယူ hash (`$MINT_HASH`) ကို ဖမ်းယူပါ။ ရန်
လက်ကျန်ကို နှစ်ခါစစ်ဆေးပါ၊ ပြေးပါ-

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

သို့မဟုတ် ပိုင်ဆိုင်မှုအသစ်ကိုသာ ပစ်မှတ်ထားရန်-

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. လက်ကျန်ငွေ၏ အစိတ်အပိုင်းကို အခြားအကောင့်သို့ လွှဲပြောင်းပါ။

အော်ပရေတာအကောင့်မှ `$RECEIVER_ACCOUNT` သို့ ယူနစ် 50 ရွှေ့ပါ။

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

ငွေပေးငွေယူ hash ကို `$TRANSFER_HASH` အဖြစ် သိမ်းဆည်းပါ။ နှစ်ခုစလုံးတွင် ပိုင်ဆိုင်မှုကို မေးမြန်းပါ။
လက်ကျန်အသစ်များကို စစ်ဆေးရန် အကောင့်များ-

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. လယ်ဂျာ အထောက်အထားကို စစ်ဆေးပါ။

ငွေပေးငွေယူ နှစ်ခုလုံး ကျူးလွန်ကြောင်း အတည်ပြုရန် သိမ်းဆည်းထားသော ဟက်ကာများကို သုံးပါ-

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

လွှဲပြောင်းမှုတွင် ပါဝင်သည့် ဘလောက်များကို ကြည့်ရှုရန် မကြာသေးမီက ဘလောက်များကို ထုတ်လွှင့်နိုင်သည်။

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

အထက်ဖော်ပြပါ command တိုင်းသည် SDKs ကဲ့သို့တူညီသော Norito payloads ကိုအသုံးပြုသည်။ ပုံတူကူးရင်
ကုဒ်မှတစ်ဆင့် ဤစီးဆင်းမှု (အောက်ပါ SDK အမြန်စတင်မှုများကိုကြည့်ပါ)၊ ဟက်ကာများနှင့် ချိန်ခွင်လျှာများ ရှိလိမ့်မည်။
တူညီသောကွန်ရက်နှင့် ပုံသေများကို ပစ်မှတ်ထားသရွေ့ တန်းစီပါ။

## SDK တူညီသောလင့်ခ်များ

- [Rrust SDK အမြန်စတင်ခြင်း](../sdks/rust) — မှတ်ပုံတင်ခြင်းဆိုင်ရာ ညွှန်ကြားချက်များကို သရုပ်ပြသည်၊
  အရောင်းအ၀ယ်များနှင့် မဲရုံအခြေအနေများကို Rust မှတင်ပြခြင်း။
- [Python SDK quickstart](../sdks/python) — တူညီသော မှတ်ပုံတင်ခြင်း/ mint ကို ပြသည်
  Norito ကျောထောက်နောက်ခံပြုထားသော JSON အကူအညီများဖြင့် လုပ်ဆောင်မှုများ။
- [JavaScript SDK အမြန်စတင်ခြင်း](../sdks/javascript) — Torii တောင်းဆိုချက်များကို အကျုံးဝင်သည် ။
  အုပ်ချုပ်မှုအထောက် အကူပြုသူများ၊ နှင့် စာရိုက်ထားသော မေးခွန်းထုပ်များ။

CLI လမ်းညွှန်ချက်ကို ဦးစွာလုပ်ဆောင်ပါ၊ ထို့နောက် သင်နှစ်သက်သော SDK ဖြင့် ဇာတ်လမ်းကို ပြန်လုပ်ပါ။
မျက်နှာပြင်နှစ်ခုလုံးသည် ငွေပေးငွေယူ hashes၊ လက်ကျန်များနှင့် query အပေါ် သဘောတူကြောင်း သေချာစေရန်
အထွက်များ