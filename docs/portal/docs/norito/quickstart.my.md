---
lang: my
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e39dc94f52395bd9323177df1a7feeb7bbd4f9a3cdea07b02f9d60e7826e199e
source_last_modified: "2026-01-22T16:26:46.506936+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
slug: /norito/quickstart
translator: machine-google-reviewed
---

ဤလမ်းညွှန်ချက်သည် လေ့လာသင်ယူသောအခါတွင် developer များလိုက်နာရန် မျှော်လင့်ထားသော အလုပ်အသွားအလာကို ထင်ဟပ်စေသည်။
ပထမအကြိမ် Norito နှင့် Kotodama- အဆုံးအဖြတ်ပေးသော တစ်ခုတည်းသော-ရွယ်တူကွန်ရက်ကို စတင်ပါ၊
စာချုပ်ကို ပြုစုပြီး ပြည်တွင်းမှာ ခြောက်အောင် လည်ပတ်ပြီး Torii ဖြင့် ပေးပို့ပါ။
ကိုးကား CLI

ဥပမာ စာချုပ်တွင် သင်လုပ်နိုင်စေရန်အတွက် ခေါ်ဆိုသူ၏အကောင့်တွင် သော့/တန်ဖိုးအတွဲတစ်ခုကို ရေးပေးသည်။
`iroha_cli` ဖြင့် ဘေးထွက်ဆိုးကျိုးကို ချက်ချင်းစစ်ဆေးပါ။

## လိုအပ်ချက်များ

- Compose V2 ကိုဖွင့်ထားပြီး [Docker](https://docs.docker.com/engine/install/) (အသုံးပြုသည်
  `defaults/docker-compose.single.yml` တွင် သတ်မှတ်ထားသော နမူနာမျိုးတူကို စတင်ရန်။
- သင်ဒေါင်းလုဒ်မလုပ်ပါက helper binaries ကိုတည်ဆောက်ရန်အတွက် Rust toolchain (1.76+)
  ထုတ်ဝေသူများ။
- `koto_compile`၊ `ivm_run` နှင့် `iroha_cli` binaries ၎င်းတို့ကို သင်တည်ဆောက်နိုင်သည်။
  အောက်တွင်ပြထားသည့်အတိုင်း workspace checkout သို့မဟုတ် ကိုက်ညီသောထွက်ရှိထားသော artifact ကို ဒေါင်းလုဒ်လုပ်ပါ-

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> အထက်ဖော်ပြပါ binaries များသည် အခြား workspace များနှင့်အတူ ထည့်သွင်းရန် ဘေးကင်းပါသည်။
> ၎င်းတို့သည် `serde`/`serde_json` သို့ ဘယ်သောအခါမှ ချိတ်ဆက်ခြင်းမရှိပါ။ Norito ကုဒ်ဒစ်များကို အဆုံးမှအဆုံးထိ ပြဌာန်းထားသည်။

## 1. တစ်ခုတည်းသော peer dev network ကို စတင်ပါ။

သိုလှောင်ခန်းတွင် `kagami swarm` မှထုတ်ပေးသော Docker Compose အတွဲတစ်ခု ပါ၀င်သည်
(`defaults/docker-compose.single.yml`)။ ၎င်းသည် မူလဥပါဒ်၊ ဖောက်သည်အား ကြိုးပေးသည်။
ဖွဲ့စည်းမှုပုံစံ၊ နှင့် ကျန်းမာရေးစုံစမ်းစစ်ဆေးချက်များကြောင့် Torii ကို `http://127.0.0.1:8080` တွင် ရရှိနိုင်မည်ဖြစ်သည်။

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

ကွန်တိန်နာကို လည်ပတ်နေအောင် ထားလိုက်ပါ။ အားလုံး
နောက်ဆက်တွဲ CLI ခေါ်ဆိုမှုများသည် `defaults/client.toml` မှတစ်ဆင့် ဤမျိုးတူရွယ်တူကို ပစ်မှတ်ထားသည်။

## 2. စာရေးသူ စာချုပ်

အလုပ်လမ်းညွှန်တစ်ခုဖန်တီးပြီး အနည်းဆုံး Kotodama ဥပမာကို သိမ်းဆည်းပါ။

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> ဗားရှင်းထိန်းချုပ်မှုတွင် Kotodama ရင်းမြစ်များကို ထားရှိခြင်းကို နှစ်သက်သည်။ Portal-hosted ဥပမာများ
> အကယ်၍ သင်သည် [Norito နမူနာပြခန်း](./examples/) အောက်တွင် ရနိုင်သည်
> ပိုချမ်းသာတဲ့ အစမှတ်ကို လိုချင်တယ်။

## 3. IVM ဖြင့် စုစည်းပြီး ခြောက်အောင်လုပ်ဆောင်ပါ။

စာချုပ်ကို IVM/Norito bytecode (`.to`) တွင် စုစည်းပြီး ၎င်းကို စက်တွင်းတွင် လုပ်ဆောင်ရန်
ကွန်ရက်ကိုမထိမီ လက်ခံဆောင်ရွက်ပေးသည့် syscalls အောင်မြင်ကြောင်း အတည်ပြုပါ-

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

အပြေးသမားသည် `info("Hello from Kotodama")` မှတ်တမ်းကို ရိုက်နှိပ်ပြီး လုပ်ဆောင်သည်။
`SET_ACCOUNT_DETAIL` သည် လှောင်ပြောင်ထားသော host ကိုဆန့်ကျင်သည့် syscall ဖြစ်သည်။ ရွေးချယ်နိုင်လျှင် `ivm_tool`
binary ကို ရနိုင်သည်၊ `ivm_tool inspect target/quickstart/hello.to` မှ ဖော်ပြသည်။
ABI ခေါင်းစီး၊ အင်္ဂါရပ်ဘစ်များနှင့် ထုတ်ယူထားသော ဝင်ခွင့်အမှတ်များ။

## 4. Torii မှတဆင့် bytecode ပေးပို့ပါ။

node လည်ပတ်နေသေးသဖြင့် CLI ကို အသုံးပြု၍ စုစည်းထားသော bytecode ကို Torii သို့ ပေးပို့ပါ။
ပုံသေဖွံ့ဖြိုးတိုးတက်မှုအထောက်အထားသည် အများသူငှာသော့မှဆင်းသက်လာသည်။
`defaults/client.toml`၊ ဒါကြောင့် အကောင့် ID ပါ။
```
i105...
```

Torii URL၊ ကွင်းဆက် ID နှင့် လက်မှတ်ထိုးသော့တို့ကို ပံ့ပိုးရန်အတွက် config ဖိုင်ကို အသုံးပြုပါ-

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI သည် ငွေပေးငွေယူကို Norito ဖြင့် ကုဒ်လုပ်ပြီး၊ ၎င်းကို dev သော့ဖြင့် လက်မှတ်ထိုးကာ၊
၎င်းကို အပြေးလုပ်သူထံ ပေးပို့ပါ။ `set_account_detail` အတွက် Docker မှတ်တမ်းများကို ကြည့်ပါ
ကတိပြုထားသော ငွေပေးငွေယူ hash အတွက် syscall သို့မဟုတ် CLI အထွက်ကို စောင့်ကြည့်ပါ။

## 5. ပြည်နယ်ပြောင်းလဲမှုကို အတည်ပြုပါ။

စာချုပ်တွင်ရေးထားသည့် အကောင့်အသေးစိတ်အချက်အလက်များကို ရယူရန် တူညီသော CLI ပရိုဖိုင်ကို အသုံးပြုပါ-

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Norito ကျောထောက်နောက်ခံပြုထားသော JSON ပေးဆောင်မှုအား သင်တွေ့ရပါမည်။

```json
{
  "hello": "world"
}
```

တန်ဖိုးပျောက်ဆုံးနေပါက Docker ရေးစပ်ခြင်းဝန်ဆောင်မှု ရှိနေသေးကြောင်း အတည်ပြုပါ။
လည်ပတ်နေပြီး `iroha` မှ အစီရင်ခံထားသော ငွေလွှဲ hash သည် `Committed` သို့ ရောက်ရှိသွားသည်
ပြည်နယ်။

## နောက်တစ်ဆင့်

- အလိုအလျောက်ထုတ်လုပ်ထားသော [ဥပမာပြခန်း](./examples/) ကိုကြည့်ရှုလေ့လာပါ
  Kotodama အတိုအထွာများ Norito syscalls များသို့ မည်မျှအဆင့်မြင့်သည် ။
- ပိုမိုလေးနက်စေရန် [Norito စတင်လမ်းညွှန်](./getting-started) ကိုဖတ်ပါ။
  compiler/runner tooling၊ manifest deployment နှင့် IVM ၏ ရှင်းလင်းချက်
  မက်တာဒေတာ။
- သင့်ကိုယ်ပိုင်စာချုပ်များကို ထပ်ခါတလဲလဲလုပ်သည့်အခါ၊ `npm run sync-norito-snippets` ကို အသုံးပြုပါ။
  ဒေါင်းလုဒ်လုပ်နိုင်သော အတိုအထွာများကို ပြန်လည်ထုတ်ပေးရန် အလုပ်နေရာသည် portal docs နှင့် artefacts များရှိနေစေရန်
  `crates/ivm/docs/examples/` အောက်ရှိ အရင်းအမြစ်များနှင့် ထပ်တူပြုထားသည်။