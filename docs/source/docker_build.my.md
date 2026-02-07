---
lang: my
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker Builder ပုံ

ဤကွန်တိန်နာကို `Dockerfile.build` တွင် သတ်မှတ်ထားပြီး toolchain အားလုံးကို စုစည်းထားသည်
CI နှင့် local release builds အတွက် လိုအပ်သော မှီခိုမှုများ။ ပုံသည် ယခု ကဲ့သို့ လည်ပတ်နေသည်။
မူရင်းအတိုင်း root မဟုတ်သောအသုံးပြုသူဖြစ်သောကြောင့် Git လုပ်ဆောင်ချက်များသည် Arch Linux ၏ဆက်လက်အလုပ်လုပ်ပါသည်။
ကမ္ဘာလုံးဆိုင်ရာ `safe.directory` ကိုဖြေရှင်းရန်မလိုအပ်ဘဲ `libgit2` ပက်ကေ့ဂျ်။

## အငြင်းအခုံများတည်ဆောက်ပါ။

- `BUILDER_USER` – ကွန်တိန်နာအတွင်း အကောင့်ဝင်အမည်ကို ဖန်တီးထားသည် (မူရင်း- `iroha`)။
- `BUILDER_UID` – ဂဏန်းအသုံးပြုသူ ID (မူရင်း- `1000`)။
- `BUILDER_GID` – အဓိကအုပ်စု ID (မူလ- `1000`)။

သင့်အိမ်ရှင်မှ အလုပ်ခွင်ကို တပ်ဆင်သည့်အခါ၊ ကိုက်ညီသော UID/GID တန်ဖိုးများကို ဖြတ်ပါ။
ထုတ်လုပ်ထားသော ရှေးဟောင်းပစ္စည်းများကို ဆက်လက်ရေးသားနိုင်သည်-

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

toolchain လမ်းညွှန်များ (`/usr/local/rustup`၊ `/usr/local/cargo`၊ `/opt/poetry`)
ပြင်ဆင်သတ်မှတ်ထားသော အသုံးပြုသူမှ ပိုင်ဆိုင်ထားသောကြောင့် Cargo၊ rustup, နှင့် Poetry commands များသည် အပြည့်အဝရှိနေပါသည်။
ကွန်တိန်နာသည် root အခွင့်ထူးများကို လွှတ်လိုက်သည်နှင့် အလုပ်လုပ်နိုင်သည် ။

## အပြေးတည်ဆောက်ခြင်း။

ခေါ်ဆိုသောအခါတွင် သင်၏အလုပ်ခွင်နေရာကို `/workspace` (ကွန်တိန်နာ `WORKDIR`) တွင် ပူးတွဲပါ
ပုံ။ ဥပမာ-

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

ပုံသည် `docker` အဖွဲ့အဖွဲ့ဝင်ဖြစ်မှုကို ထိန်းသိမ်းထားသောကြောင့် Docker ညွှန်ကြားချက်များ (ဥပမာ။
`docker buildx bake`) host PID ကို တပ်ဆင်သည့် CI အလုပ်အသွားအလာအတွက် ရနိုင်သည်
နှင့် socket ။ သင့်ပတ်ဝန်းကျင်အတွက် လိုအပ်သလို အုပ်စုမြေပုံဆွဲခြင်းကို ချိန်ညှိပါ။

## Iroha 2 နှင့် Iroha 3 လက်ရာများ

တိုက်မိခြင်းများကို ရှောင်ရှားရန် အလုပ်နေရာသည် ယခုအခါ သီးခြား binaries တစ်ခုစီကို ထုတ်လွှတ်သည်-
`iroha3`/`iroha3d` (မူရင်း) နှင့် `iroha2`/`iroha2d` (Iroha 2)။ အကူအညီပေးသူများကို အသုံးပြုပါ။
လိုချင်သောအတွဲကိုထုတ်ပါ

- Iroha 3 အတွက် `make build` (သို့မဟုတ် `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`)
- Iroha 2 အတွက် `make build-i2` (သို့မဟုတ် `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`)

ရွေးချယ်သူသည် အင်္ဂါရပ်အစုံများကို ပင်ထိုးပေးသည် (`telemetry` + `schema-endpoint` နှင့်
line-specific `build-i{2,3}` အလံ) ထို့ကြောင့် Iroha 2 builds သည် မတော်တဆ ကောက်ယူ၍မရပါ။
Iroha 3-သာ ပုံသေများ။

`scripts/build_release_bundle.sh` မှတစ်ဆင့် မှန်ကန်သော ဒွိစုံကို ရွေးထုတ်ပါ
`--profile` ကို `iroha2` သို့မဟုတ် `iroha3` သို့ သတ်မှတ်သောအခါ အလိုအလျောက် အမည်ပေးသည်။