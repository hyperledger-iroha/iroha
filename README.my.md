---
lang: my
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Hyperledger Iroha

[![လိုင်စင်](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha သည် ခွင့်ပြုချက်နှင့် လုပ်ငန်းစုခွဲဝေမှုများအတွက် သတ်မှတ်ထားသော blockchain ပလပ်ဖောင်းတစ်ခုဖြစ်သည်။ ၎င်းသည် Iroha Virtual Machine (IVM) မှတဆင့် အကောင့်/ပိုင်ဆိုင်မှုစီမံခန့်ခွဲမှု၊ ကွင်းဆက်ခွင့်ပြုချက်များနှင့် စမတ်စာချုပ်များကို ပံ့ပိုးပေးပါသည်။

> လုပ်ငန်းခွင်အခြေအနေနှင့် လတ်တလောပြောင်းလဲမှုများကို [`status.md`](./status.md) တွင် ခြေရာခံပါသည်။

## တေးသွားများ

ဤသိမ်းဆည်းမှုသည် တူညီသော codebase မှ ဖြန့်ကျက်မှုလမ်းကြောင်းနှစ်ခုကို ပို့ဆောင်ပေးသည်-

- **Iroha 2**- ကိုယ်တိုင်လက်ခံကျင်းပခွင့်ပြုထားသော/လုပ်ငန်းစုကွန်ရက်များ။
- **Iroha 3 (SORA Nexus)**- တူညီသော core သေတ္တာများကို အသုံးပြု၍ Nexus-အသားပေး ဖြန့်ကျက်လမ်းကြောင်း။

သီချင်းနှစ်ပုဒ်စလုံးသည် Norito အမှတ်စဉ်သတ်မှတ်ခြင်း၊ Sumeragi သဘောတူညီမှု နှင့် Kotodama -> IVM တူးလ်ကွင်းဆက်အပါအဝင် တူညီသော core အစိတ်အပိုင်းများကို မျှဝေပါသည်။

## Repository Layout

- [`crates/`](./crates): အူတိုင် သံချေးသေတ္တာများ (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `iroha_core`,018NI0 `norito` စသည်ဖြင့်)။
- [`integration_tests/`](./integration_tests)- အစိတ်အပိုင်းနှစ်ခု ကွန်ရက်/ပေါင်းစည်းမှု စမ်းသပ်မှုများ။
- [`IrohaSwift/`](./IrohaSwift): Swift SDK ပက်ကေ့ဂျ်။
- [`java/iroha_android/`](./java/iroha_android): Android SDK ပက်ကေ့ဂျ်။
- [`docs/`](./docs): အသုံးပြုသူ/အော်ပရေတာ/ဆော့ဖ်ဝဲရေးသားသူ စာရွက်စာတမ်း။

## အမြန်စတင်ပါ။

### ကြိုတင်လိုအပ်ချက်များ

- [သံမဏိ တည်ငြိမ်](https://www.rust-lang.org/tools/install)
- ရွေးချယ်နိုင်သည်- Docker + Docker ပြည်တွင်း-မျိုးတူ အများအပြား လုပ်ဆောင်မှုများအတွက် ရေးဖွဲ့ပါ

### တည်ဆောက်ပြီး စမ်းသပ်ခြင်း (အလုပ်နေရာ)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

မှတ်စုများ-

- လုပ်ငန်းခွင်အပြည့်တည်ဆောက်ရန် မိနစ် 20 ခန့် ကြာနိုင်သည်။
- အပြည့်အဝအလုပ်ခွင်စာမေးပွဲများသည်နာရီပေါင်းများစွာကြာနိုင်သည်။
- အလုပ်ခွင်သည် `std` ကို ပစ်မှတ်ထားပါသည် (WASM/no-std တည်ဆောက်မှုများကို မပံ့ပိုးပါ)။

### ပစ်မှတ်ထား စမ်းသပ်သည့် အမိန့်များ

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK Test Commands

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Local Network ကိုဖွင့်ပါ။

ပေးထားသော Docker Compose ကွန်ရက်ကို စတင်ပါ။

```bash
docker compose -f defaults/docker-compose.yml up
```

default client config ကိုဆန့်ကျင်သည့် CLI ကိုသုံးပါ

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

daemon သီးသန့်အသုံးပြုမှုအဆင့်များအတွက်၊ [`crates/irohad/README.md`](./crates/irohad/README.md) ကို ကြည့်ပါ။

## API နှင့် Observability

Torii သည် Norito နှင့် JSON API နှစ်ခုလုံးကို ဖော်ထုတ်သည်။ ဘုံအော်ပရေတာအဆုံးမှတ်များ-

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

အဆုံးမှတ်ကိုးကားချက်ကို အပြည့်အစုံကြည့်ပါ-

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## အမာခံသေတ္တာများ

- [`crates/iroha`](./crates/iroha): သုံးစွဲသူ ဒစ်ဂျစ်တိုက်။
- [`crates/irohad`](./crates/irohad): peer daemon binaries
- [`crates/iroha_cli`](./crates/iroha_cli): ကိုးကား CLI။
- [`crates/iroha_core`](./crates/iroha_core): လယ်ဂျာ/အမာခံ လုပ်ဆောင်ချက်အင်ဂျင်။
- [`crates/iroha_config`](./crates/iroha_config): ရိုက်ထည့်ထားသော ဖွဲ့စည်းမှုပုံစံ။
- [`crates/iroha_data_model`](./crates/iroha_data_model) : canonical data model
- [`crates/iroha_crypto`](./crates/iroha_crypto)- ကုဒ်ဝှက်စာမူများ။
- [`crates/norito`](./crates/norito): အဆုံးအဖြတ်ပေးသော အမှတ်စဉ် ကုဒ်ဒက်။
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine။
- [`crates/iroha_kagami`](./crates/iroha_kagami): key/genesis/config tooling။

## စာရွက်စာတမ်းမြေပုံ

- Main docs အညွှန်း- [`docs/README.md`](./docs/README.md)
- ကမ္ဘာဦးကျမ်း- [`docs/genesis.md`](./docs/genesis.md)
- သဘောတူညီမှု (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- ငွေပေးငွေယူ ပိုက်လိုင်း- [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P အတွင်းပိုင်း- [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM syscalls- [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama သဒ္ဒါ- [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito ဝိုင်ယာဖော်မတ်- [`norito.md`](./norito.md)
- လက်ရှိအလုပ်ခြေရာခံခြင်း- [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## ဘာသာပြန်များ

ဂျပန်အနှစ်ချုပ်- [`README.ja.md`](./README.ja.md)

အခြားသုံးသပ်ချက်များ-
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

ဘာသာပြန်လုပ်ငန်းအသွားအလာ- [`docs/i18n/README.md`](./docs/i18n/README.md)

## ပါဝင်ကူညီခြင်း။

- ပံ့ပိုးကူညီမှုလမ်းညွှန်- [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- အသိုင်းအဝိုင်း/ပံ့ပိုးမှုချန်နယ်များ- [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

##လိုင်စင်

Iroha ကို Apache-2.0 အောက်တွင် လိုင်စင်ရထားသည်။ [`LICENSE`](./LICENSE) ကိုကြည့်ပါ။

စာရွက်စာတမ်းကို CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/ အောက်တွင် လိုင်စင်ရထားသည်။