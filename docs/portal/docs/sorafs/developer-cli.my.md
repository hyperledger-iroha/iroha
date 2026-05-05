---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c9acf8a8d9c298ad2fe95e5480942441aa790346f90c5b5e1a8c1ff638c5e73
source_last_modified: "2026-01-22T16:26:46.522695+00:00"
translation_last_reviewed: 2026-02-07
id: developer-cli
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

ပေါင်းစပ်ထားသော `sorafs_cli` မျက်နှာပြင် (`sorafs_car` သေတ္တာဖြင့် ပံ့ပိုးပေးသည်
`cli` လုပ်ဆောင်ချက်ကို ဖွင့်ထားသည်) SoraFS ပြင်ဆင်ရန် လိုအပ်သော အဆင့်တိုင်းကို ဖော်ထုတ်ပေးသည်
ရှေးဟောင်းပစ္စည်း။ အများသုံး အလုပ်အသွားအလာများဆီသို့ တိုက်ရိုက်တက်ရန် ဤဟင်းချက်စာအုပ်ကို အသုံးပြုပါ။ ၎င်းကိုတွဲပါ။
လုပ်ငန်းလည်ပတ်မှုဆိုင်ရာ အကြောင်းအရာအတွက် ထင်ရှားသော ပိုက်လိုင်းနှင့် တီးမှုတ်ခြင်းဆိုင်ရာ စာအုပ်များ။

## အထုပ်ဝန်ပိုးများ

အဆုံးအဖြတ်ပေးသော CAR မော်ကွန်းတိုက်များနှင့် အတုံးအခဲအစီအစဥ်များကို ထုတ်လုပ်ရန် `car pack` ကို အသုံးပြုပါ။ ဟိ
လက်ကိုင်ကို ပေးမထားပါက command သည် SF-1 chunker ကို အလိုအလျောက် ရွေးချယ်သည်။

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- ပုံသေချန်းကာလက်ကိုင်- `sorafs.sf1@1.0.0`။
- လမ်းညွှန်ထည့်သွင်းမှုများကို အဘိဓာန်အစီအစဥ်အတိုင်း လုပ်ဆောင်ထားသောကြောင့် checksum များသည် တည်ငြိမ်နေမည်ဖြစ်သည်။
  ပလက်ဖောင်းများတစ်လျှောက်။
- JSON အနှစ်ချုပ်တွင် payload digests၊ per-chunk metadata နှင့် root တို့ ပါဝင်ပါသည်။
  CID သည် မှတ်ပုံတင်ခြင်းနှင့် တီးမှုတ်သူမှ အသိအမှတ်ပြုသည်။

## သရုပ်ဖော်သည်။

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` ရွေးချယ်စရာများကို `PinPolicy` အကွက်များသို့ တိုက်ရိုက်မြေပုံဆွဲပါ။
  `sorafs_manifest::ManifestBuilder`။
- သင် CLI ကို SHA3 အပိုင်းကို ပြန်လည်တွက်ချက်လိုပါက `--chunk-plan` ပေးပါ။
  တင်ပြခြင်းမပြုမီ ချေဖျက်ပါ။ သို့မဟုတ်ပါက ၎င်းတွင် ထည့်သွင်းထားသော ချေဖျက်မှုကို ပြန်လည်အသုံးပြုသည်။
  အနှစ်ချုပ်။
- JSON output သည် Norito payload ကို အချိန်အတွင်း ရိုးရှင်းသောကွဲပြားမှုများအတွက် ထင်ဟပ်စေသည်။
  သုံးသပ်ချက်

## ဆိုင်းဘုတ်သည် ကြာရှည်သောသော့များမပါဘဲ ထင်ရှားသည်။

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- အတွင်းပိုင်းတိုကင်များ၊ ပတ်ဝန်းကျင်ပြောင်းလွဲမှုများ သို့မဟုတ် ဖိုင်အခြေခံအရင်းအမြစ်များကို လက်ခံသည်။
- သက်သေပြချက် မက်တာဒေတာ (`token_source`၊ `token_hash_hex`၊ အတုံးအခဲအကြေ)
  `--include-token=true` မဟုတ်လျှင် JWT အကြမ်းကို မတည်မြဲပါ။
- CI တွင် ကောင်းမွန်စွာလုပ်ဆောင်နိုင်သည်- setting ဖြင့် GitHub Actions OIDC နှင့် ပေါင်းစပ်ပါ။
  `--identity-token-provider=github-actions`။

## မန်နီးဖက်စ်များကို Torii သို့ တင်ပြပါ။

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- alias အထောက်အထားများအတွက် Norito ကုဒ်ကုဒ်ကို လုပ်ဆောင်ပြီး ၎င်းတို့နှင့် ကိုက်ညီကြောင်း အတည်ပြုသည်။
  Torii သို့ မတင်မီ ထုတ်ဖော်ပြသပါ။
- မတိုက်ဆိုင်သောတိုက်ခိုက်မှုများကိုကာကွယ်ရန် အစီအစဉ်မှ SHA3 အတုံးအခဲများကို ပြန်လည်တွက်ချက်သည်။
- တုံ့ပြန်မှုအနှစ်ချုပ်များသည် HTTP အခြေအနေ၊ ခေါင်းစီးများနှင့် မှတ်ပုံတင်ကြေးများကို ဖမ်းယူသည်။
  နောက်ပိုင်း စာရင်းစစ်။

## CAR အကြောင်းအရာနှင့် အထောက်အထားများကို စစ်ဆေးပါ။

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR သစ်ပင်ကို ပြန်လည်တည်ဆောက်ပြီး payload digest များကို manifest အကျဉ်းချုပ်နှင့် နှိုင်းယှဉ်ပါ။
- ပုံတူပွားခြင်းဆိုင်ရာ အထောက်အထားများ တင်ပြသည့်အခါ လိုအပ်သော အရေအတွက်နှင့် အထောက်အထားများကို ဖမ်းယူပါ။
  အုပ်ချုပ်မှုဆီသို့။

## တိုက်ရိုက်ဓာတ်ခံတယ်လီမီတာ

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- တိုက်ရိုက်ထုတ်လွှင့်မှုအထောက်အထားတစ်ခုစီအတွက် NDJSON ပစ္စည်းများကို ထုတ်လွှတ်သည် (ဖြင့် ပြန်လည်ကစားခြင်းကို ပိတ်ပါ။
  `--emit-events=false`)။
- အောင်မြင်မှု/ကျရှုံးမှု အရေအတွက်၊ latency histograms နှင့် နမူနာပြထားသော ကျရှုံးမှုများကို စုစည်းပေးသည်
  အကျဉ်းချုပ် JSON သည် မှတ်တမ်းများကို မခြစ်ဘဲ ဒိုင်ခွက်များမှ ရလဒ်များကို ကြံစည်နိုင်သည်။
- gateway သည် ပျက်ကွက်မှုများ သို့မဟုတ် ဒေသဆိုင်ရာ PoR အတည်ပြုခြင်းအား အစီရင်ခံသည့်အခါ သုညမဟုတ်သော ထွက်သွားပါသည်။
  (`--por-root-hex`) မှ အထောက်အထားများကို ငြင်းပယ်သည်။ တံခါးခုံများဖြင့် ချိန်ညှိပါ။
  အစမ်းလေ့ကျင့်မှုများအတွက် `--max-failures` နှင့် `--max-verification-failures`။
- ယနေ့ PoR ကိုပံ့ပိုးသည်; PDP နှင့် PoTR သည် SF-13/SF-14 တစ်ကြိမ် တူညီသောစာအိတ်ကို ပြန်သုံးသည်။
  မြေ။
- `--governance-evidence-dir` သည် ပြန်ဆိုထားသော အကျဉ်းချုပ်၊ မက်တာဒေတာ (အချိန်တံဆိပ်၊
  CLI ဗားရှင်း၊ ဂိတ်ဝ URL၊ မန်နီးဖက်စ် ချေဖျက်မှု) နှင့် မန်နီးဖက်စ်၏ မိတ္တူ
  စီမံအုပ်ချုပ်မှုပက်ကတ်များသည် အထောက်အထား-စီးကြောင်းကို မော်ကွန်းတင်နိုင်စေရန် ပံ့ပိုးပေးထားသောလမ်းညွှန်
  ပြေးခြင်းမပြုဘဲ အထောက်အထားများ။

## ထပ်လောင်းကိုးကား

- `docs/source/sorafs_cli.md` — အပြည့်အစုံ အလံစာရွက်စာတမ်း။
- `docs/source/sorafs_proof_streaming.md` — အထောက်အထား တယ်လီမီတာစနစ် နှင့် Grafana
  ဒက်ရှ်ဘုတ် နမူနာပုံစံ။
- `docs/source/sorafs/manifest_pipeline.md` — အတုံးလိုက်အခဲလိုက်၊ ထင်ရှားစွာ နက်ရှိုင်းစွာ ငုပ်လျှိုးနေသည်။
  ဖွဲ့စည်းမှုနှင့် CAR ကိုင်တွယ်မှု။