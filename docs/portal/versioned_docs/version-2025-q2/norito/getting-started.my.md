---
lang: my
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T16:26:46.562262+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Norito စတင်နေပါပြီ။

ဤအမြန်လမ်းညွှန်သည် Kotodama စာချုပ်ကို ပြုစုရန်အတွက် အနည်းဆုံး အလုပ်အသွားအလာကို ပြသသည်၊
ထုတ်လုပ်လိုက်သော Norito bytecode ကို စစ်ဆေးခြင်း၊ ၎င်းကို စက်တွင်းသုံး၍ အသုံးပြုခြင်း၊
Iroha node တစ်ခုသို့။

## လိုအပ်ချက်များ

1. Rust toolchain (1.76 သို့မဟုတ် ထို့ထက် ပို) ကို ထည့်သွင်းပြီး ဤသိမ်းဆည်းမှုကို စစ်ဆေးပါ။
2. ပံ့ပိုးပေးထားသော binaries ကို တည်ဆောက်ပါ သို့မဟုတ် ဒေါင်းလုဒ်လုပ်ပါ-
   - `koto_compile` – Kotodama IVM/Norito bytecode ကိုထုတ်လွှတ်သော compiler
   - `ivm_run` နှင့် `ivm_tool` - ဒေသဆိုင်ရာ ကွပ်မျက်ရေးနှင့် စစ်ဆေးရေး အသုံးဝင်မှုများ
   - `iroha_cli` - Torii မှတစ်ဆင့် စာချုပ်ဖြန့်ကျက်ရာတွင် အသုံးပြုသည်

   Makefile repository သည် `PATH` တွင် ဤ binaries ကိုမျှော်လင့်ထားသည်။ သင်လည်းကောင်း
   ကြိုတင်တည်ဆောက်ထားသော ရှေးဟောင်းပစ္စည်းများကို ဒေါင်းလုဒ်လုပ်ပါ သို့မဟုတ် ၎င်းတို့ကို အရင်းအမြစ်မှ တည်ဆောက်ပါ။ compile လုပ်ရင်
   စက်တွင်းရှိ toolchain တွင် Makefile အကူအညီပေးသူများကို binaries တွင်ညွှန်ပြပါ-

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. သင်အသုံးပြုမှုအဆင့်သို့ရောက်ရှိသောအခါ Iroha node တစ်ခုလည်ပတ်ကြောင်းသေချာပါစေ။ ဟိ
   အောက်ဖော်ပြပါဥပမာများသည် Torii ကို သင့်တွင်ပြင်ဆင်ထားသော URL တွင်ရောက်ရှိနိုင်သည်ဟုယူဆသည်
   `iroha_cli` ပရိုဖိုင် (`~/.config/iroha/cli.toml`)။

## 1. Kotodama စာချုပ်ကို စုစည်းပါ။

သိုလှောင်မှုတွင် အနည်းငယ်မျှသာသော “ဟယ်လိုကမ္ဘာ” စာချုပ်ကို ပေးပို့သည်။
`examples/hello/hello.ko`။ ၎င်းကို Norito/IVM bytecode (`.to`) သို့ စုစည်းပါ။

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

အဓိက အလံများ-

- `--abi 1` သည် ABI ဗားရှင်း 1 သို့ စာချုပ်ကို လော့ခ်ချသည် (တစ်ခုတည်းသော ပံ့ပိုးထားသော ဗားရှင်းမှာ
  စာရေးချိန်)။
- `--max-cycles 0` သည် အကန့်အသတ်မဲ့ ကွပ်မျက်မှုကို တောင်းဆိုသည်။ အပြုသဘောဆောင်သော နံပါတ်ကို ချည်နှောင်ရန် သတ်မှတ်ပါ။
  သုည-အသိပညာအထောက်အထားများအတွက် သံသရာ padding။

## 2. Norito ကို စစ်ဆေးပါ (ချန်လှပ်ထားနိုင်သည်)

ခေါင်းစီးနှင့် ထည့်သွင်းထားသော မက်တာဒေတာကို အတည်ပြုရန် `ivm_tool` ကို အသုံးပြုပါ-

```sh
ivm_tool inspect target/examples/hello.to
```

ABI ဗားရှင်း၊ ဖွင့်ထားသည့် အင်္ဂါရပ်အလံများနှင့် ထုတ်ယူထားသော ဝင်ခွင့်တို့ကို သင်တွေ့ရပါမည်။
အမှတ်များ။ ဤအရာသည် ဖြန့်ကျက်ခြင်းမပြုမီ လျင်မြန်သော စိတ်ပိုင်းဆိုင်ရာ စစ်ဆေးမှုတစ်ခုဖြစ်သည်။

## 3. စာချုပ်ကို ပြည်တွင်းတွင် လုပ်ဆောင်ပါ။

အပြုအမူကို မထိဘဲ အတည်ပြုရန် `ivm_run` ဖြင့် bytecode ကို လုပ်ဆောင်ပါ
node-

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` သည် နှုတ်ခွန်းဆက်စကား မှတ်တမ်းတင်ပြီး `SET_ACCOUNT_DETAIL` syscall ကိုထုတ်ပေးသည်။
ထုတ်ဝေခြင်းမပြုမီ စာချုပ်ယုတ္တိကို ထပ်ခါတလဲလဲ လုပ်ဆောင်နေချိန်တွင် စက်တွင်းတွင် လုပ်ဆောင်ခြင်းသည် အသုံးဝင်ပါသည်။
ကွင်းဆက်တစ်ခု။

## 4. `iroha_cli` မှတစ်ဆင့် အသုံးပြုပါ။

စာချုပ်ကို ကျေနပ်သောအခါ၊ CLI ကို အသုံးပြု၍ node သို့ ဖြန့်ကျက်ပါ။
အခွင့်အာဏာအကောင့်တစ်ခု၊ ၎င်း၏လက်မှတ်ထိုးသော့နှင့် `.to` ဖိုင်တစ်ခု သို့မဟုတ်
Base64 ပေးဆောင်မှု-

```sh
iroha_cli app contracts deploy \
  --authority i105... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

ညွှန်ကြားချက်သည် Norito မန်နီးဖက်စ် + ဘိုက်ကုဒ်အတွဲအစည်းကို Torii နှင့် ပရင့်ထုတ်သည်
ရလဒ်ထွက်ငွေအခြေအနေ။ ငွေလွှဲပြီးသည်နှင့်၊ ကုဒ်
တုံ့ပြန်မှုတွင်ပြသထားသော hash ကို မန်နီးဖက်စ်များ သို့မဟုတ် ဖြစ်ရပ်များကို စာရင်းပြုစုရန် အသုံးပြုနိုင်သည်။

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii ကို run ပါ။

bytecode မှတ်ပုံတင်ထားပြီး၊ ညွှန်ကြားချက်တစ်ခုတင်ပြခြင်းဖြင့် ၎င်းကိုသင်တောင်းဆိုနိုင်ပါသည်။
သိမ်းဆည်းထားသောကုဒ်ကို ရည်ညွှန်းသည် (ဥပမာ၊ `iroha_cli ledger transaction submit` မှတဆင့်
သို့မဟုတ် သင့်လျှောက်လွှာဖောက်သည်)။ အကောင့်ခွင့်ပြုချက်များ လိုချင်သောကိုခွင့်ပြုကြောင်းသေချာပါစေ။
syscalls (`set_account_detail`၊ `transfer_asset` စသည်ဖြင့်)။

## အကြံပြုချက်များနှင့် ပြဿနာဖြေရှင်းခြင်း။

- ပေးထားသော ဥပမာများကို တစ်ခုတည်းတွင် စုစည်းပြီး လုပ်ဆောင်ရန် `make examples-run` ကို အသုံးပြုပါ။
  ရိုက်ချက်။ binaries ကိုဖွင့်မထားပါက `KOTO`/`IVM` ကို အစားထိုးပါ
  `PATH`။
- `koto_compile` သည် ABI ဗားရှင်းကို ငြင်းပယ်ပါက compiler နှင့် node ကို စစ်ဆေးပါ။
  ပစ်မှတ် နှစ်ခုစလုံးသည် ABI v1 (`koto_compile --abi` ကို စာရင်းသွင်းရန် အကြောင်းပြချက်များမပါဘဲ လုပ်ဆောင်သည်
  ပံ့ပိုးမှု)။
- CLI သည် hex သို့မဟုတ် Base64 လက်မှတ်ထိုးသော့များကို လက်ခံသည်။ စမ်းသပ်ရန်အတွက်၊ သင်သုံးနိုင်သည်။
  `iroha_cli tools crypto keypair` မှ ထုတ်လွှတ်သော သော့များ။
- Norito payloads ကို အမှားရှာသောအခါ၊ `ivm_tool disassemble` subcommand သည် ကူညီသည်
  Kotodama အရင်းအမြစ်နှင့် ညွှန်ကြားချက်များ ဆက်စပ်နေသည်။

ဤစီးဆင်းမှုသည် CI တွင်အသုံးပြုသည့်အဆင့်များနှင့် ပေါင်းစပ်စစ်ဆေးမှုများကို ထင်ဟပ်စေသည်။ နက်နဲသည်
Kotodama သဒ္ဒါ၊ syscall mappings နှင့် Norito အတွင်းပိုင်းသို့ ဝင်ရောက်ကြည့်ရှုပါ-

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`