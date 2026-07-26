---
lang: my
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2025-12-29T18:16:35.986892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI Bundle Tooling

MOCHI သည် ပေါ့ပါးသော ထုပ်ပိုးမှုလုပ်ငန်းအသွားအလာဖြင့် ပို့ဆောင်ပေးသောကြောင့် developer များက ထုတ်လုပ်နိုင်သည်။
CI scripts များကို ကြိုးသွယ်ခြင်းမရှိဘဲ သယ်ဆောင်ရလွယ်ကူသော desktop အတွဲ။ `xtask`
subcommand သည် compilation၊ layout၊ hashing နှင့် (optionally) archive ကို ကိုင်တွယ်သည်။
တစ်ချက်တည်းနဲ့ ဖန်တီးမှု။

## အစုအဝေးတစ်ခု ဖန်တီးခြင်း။

```bash
cargo xtask mochi-bundle
```

default အနေဖြင့် command သည် release binaries ကိုတည်ဆောက်သည်၊ အောက်တွင် bundle ကိုစုဝေးစေသည်။
`target/mochi-bundle/` နှင့် `mochi-<os>-<arch>-release.tar.gz` မှတ်တမ်းကို ထုတ်လွှတ်သည်
အဆုံးအဖြတ်ပေးသော `manifest.json` နှင့်အတူ။ မန်နီးဖက်စ်တွင် ဖိုင်တိုင်းကို စာရင်းပြုစုထားသည်။
၎င်း၏အရွယ်အစားနှင့် SHA-256 hash ဖြစ်သောကြောင့် CI ပိုက်လိုင်းများ ပြန်လည်စစ်ဆေးခြင်း သို့မဟုတ် ထုတ်ဝေနိုင်သည်။
သက်သေခံချက်များ။ အကူအညီပေးသူက `mochi` ဒက်စ်တော့ရှဲလ်နှင့် နှစ်ခုလုံးကို သေချာစေသည်။
workspace `kagami` binary သည် ရှိနေသောကြောင့် ဥပါဒ် မျိုးဆက်သည် ပြင်ပမှ အလုပ်လုပ်ပါသည်။
သေတ္တာ။

### အလံများ

| အလံ | ဖော်ပြချက် |
|--------------------------------------------------------------------------------------------------------------------|
| `--out <dir>` | အထွက်လမ်းကြောင်းကို အစားထိုးပါ (ပုံသေ `target/mochi-bundle`)။         |
| `--profile <name>` | သီးခြား Cargo ပရိုဖိုင် (ဥပမာ၊ စမ်းသပ်မှုများအတွက် `debug`) ဖြင့် တည်ဆောက်ပါ။              |
| `--no-archive` | `.tar.gz` မော်ကွန်းတိုက်ကို ကျော်ပြီး ပြင်ဆင်ထားသည့်ဖိုင်တွဲကိုသာ ချန်ထားပါ။               |
| `--kagami <path>` | `iroha_kagami` ကို တည်ဆောက်မည့်အစား တိကျပြတ်သားသော `kagami` ကို အသုံးပြုပါ။         |
| `--matrix <path>` | CI provenance ခြေရာခံခြင်းအတွက် အတွဲလိုက် မက်တာဒေတာကို JSON matrix တွင် ပေါင်းထည့်ပါ။         |
| `--smoke` | အခြေခံလုပ်ဆောင်မှုတံခါးအဖြစ် ထုပ်ပိုးထားသောအတွဲမှ `mochi --help` ကိုဖွင့်ပါ။      |
| `--stage <dir>` | ပြီးသွားသောအစုအဝေး (နှင့် မော်ကွန်းတင်သည့်အခါ) ကို အဆင့်မြှင့်တင်သည့်ဖိုင်တွဲတစ်ခုသို့ ကူးယူပါ။ |

`--stage` သည် တည်ဆောက်သူ အေးဂျင့်တစ်ဦးစီက ၎င်းကို အပ်လုဒ်လုပ်သည့် CI ပိုက်လိုင်းများအတွက် ရည်ရွယ်သည်
မျှဝေထားသော တည်နေရာအတွက် အနုပညာပစ္စည်းများ။ အကူအညီပေးသူက အတွဲလိုက်လမ်းညွှန်ကို ပြန်လည်ဖန်တီးပေးပြီး
ထုတ်ပေးထားသော archive ကို staging directory ထဲသို့ ကူးယူကာ အလုပ်များကို ထုတ်ဝေနိုင်ပါသည်။
shell scripting မပါဘဲ platform-specific output များကိုစုဆောင်းပါ။

အစုအဝေးအတွင်း အပြင်အဆင်သည် ရည်ရွယ်ချက်ရှိရှိ ရိုးရှင်းသည်-

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### Runtime သည် overrides ဖြစ်သည်။

ထုပ်ပိုးထားသော `mochi` သည် အများစုအတွက် command-line overrides ကိုလက်ခံသည်
သာမန်ကြီးကြပ်ရေးမှူး ဆက်တင်များ။ တည်းဖြတ်ခြင်းအစား ဤအလံများကို အသုံးပြုပါ။
စမ်းသပ်သောအခါ `config/local.toml`

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

မည်သည့် CLI တန်ဖိုးသည် `config/local.toml` ထည့်သွင်းမှုများနှင့် ပတ်ဝန်းကျင်ထက် ဦးစားပေးပါသည်။
ကိန်းရှင်များ။

## လျှပ်တစ်ပြက်အလိုအလျောက်စနစ်

`manifest.json` သည် မျိုးဆက်အချိန်တံဆိပ်၊ ပစ်မှတ်သုံးဆ၊ ကုန်စည်ပရိုဖိုင်၊
ပြီးပြည့်စုံသော ဖိုင်စာရင်း။ ပိုက်လိုင်းများသည် မည်သည့်အချိန်တွင် သိရှိနိုင်သည်ကို သိရှိရန် ကွဲပြားနိုင်သည်။
အသစ်ထွက်ရှိထားသော ပစ္စည်းများနှင့် အတူ JSON ကို အပ်လုဒ်လုပ်ပါ၊ သို့မဟုတ် စစ်ဆေးပါ။
အော်ပရေတာများသို့ အစုအစည်းတစ်ခုအား မကြော်ငြာမီ hashes။

အကူအညီပေးသူက အရည်အချင်းမရှိပါ- အမိန့်ကို ပြန်လည်လုပ်ဆောင်ခြင်းသည် မန်နီးဖက်စ်ကို အပ်ဒိတ်လုပ်ကာ၊
`target/mochi-bundle/` ကို တစ်ခုတည်းအဖြစ်ထားကာ ယခင် archive ကို ထပ်ရေးသည်
လက်ရှိစက်ရှိ နောက်ဆုံးထွက်အတွဲအတွက် အမှန်တရားအရင်းအမြစ်။