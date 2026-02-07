---
lang: my
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82939c8aa73add3f3817490ab1a24bef5388c2fc5a00d19d76e4be6a3fa9c559
source_last_modified: "2025-12-29T18:16:35.111267+00:00"
translation_last_reviewed: 2026-02-07
id: preview-host-exposure
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
---

DOCS-SORA လမ်းပြမြေပုံသည် အများသူငှာ အကြိုကြည့်ရှုမှုတိုင်းကို တူညီစွာစီးနင်းရန် လိုအပ်သည်
ပြန်လည်သုံးသပ်သူများသည် စက်တွင်းတွင် ကျင့်သုံးသည့် checksum-verified အတွဲ။ ဒီ runbook ကိုသုံးပါ။
ပြန်လည်သုံးသပ်သူ စတင်တက်ရောက်ပြီးနောက် (နှင့် ဖိတ်ကြားချက်အတည်ပြုချက်လက်မှတ်) ပြီးမြောက်သွားပါပြီ။
ဘီတာအကြိုကြည့်ရှုမှုလက်ခံသည့် အွန်လိုင်း။

## လိုအပ်ချက်များ

- သုံးသပ်သူသည် စတင်အသုံးပြုသည့်လှိုင်းကို အတည်ပြုပြီး အစမ်းကြည့်ရှုသည့် ခြေရာခံစနစ်တွင် အကောင့်ဝင်ထားသည်။
- `docs/portal/build/` နှင့် checksum အောက်တွင် နောက်ဆုံးပေါ်ပေါ်တယ်တည်ဆောက်မှု
  စစ်ဆေးပြီး (`build/checksums.sha256`)။
- SoraFS အစမ်းကြည့်ရှုခြင်းအထောက်အထားများ (Torii URL၊ အခွင့်အာဏာ၊ သီးသန့်သော့၊ တင်ပြထားသည်
  epoch) ပတ်ဝန်းကျင် variables သို့မဟုတ် JSON config တွင် သိမ်းဆည်းထားသည်။
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)။
- အလိုရှိသော hostname (`docs-preview.sora.link`၊ DNS ပြောင်းလဲခြင်းလက်မှတ်၊
  `docs.iroha.tech` စသည်ဖြင့်) နှင့် ဖုန်းခေါ်ဆိုမှုဆိုင်ရာ အဆက်အသွယ်များ။

## အဆင့် 1 – အစုအဝေးကို တည်ဆောက်ပြီး အတည်ပြုပါ။

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

checksum manifest ပျောက်နေသည့်အခါ သို့မဟုတ် verify script သည် ဆက်လက်လုပ်ဆောင်ရန် ငြင်းဆိုထားသည်။
အစမ်းကြည့်ရှုသည့်အရာတိုင်းကို စာရင်းစစ်ထားရှိခြင်းဖြင့် အနှောင့်အယှက်ပေးသည်။

## အဆင့် 2 – SoraFS ပစ္စည်းများကို ထုပ်ပိုးပါ။

အငြိမ်ဆိုက်ကို အဆုံးအဖြတ်ပေးသော CAR/မန်နီးဖက်စ်အတွဲအဖြစ် ပြောင်းပါ။ `ARTIFACT_DIR`
`docs/portal/artifacts/` သို့ ပုံသေဖြစ်သည်။

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

ထုတ်လုပ်ထားသော `portal.car`၊ `portal.manifest.*`၊ ဖော်ပြချက်နှင့် checksum ကို ပူးတွဲပါ
အကြိုကြည့်ရှုခြင်း လှိုင်းလက်မှတ်ကို ဖော်ပြပါ။

## အဆင့် 3 – အစမ်းကြည့်ခြင်းအမည်ကို ထုတ်ဝေပါ။

သင်ဖော်ထုတ်ရန်အဆင်သင့်ဖြစ်သောအခါတွင် pin helper ** `--skip-submit` မပါဘဲ ** ပြန်ဖွင့်ပါ။
အိမ်ရှင်။ JSON config သို့မဟုတ် တိကျသော CLI အလံများကို ပေးဆောင်ပါ-

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

command သည် `portal.pin.report.json` ရေးသည်၊
`portal.manifest.submit.summary.json` နှင့် `portal.submit.response.json` ၊
ဖိတ်ကြားချက် အထောက်အထား အတွဲနှင့်အတူ ပို့ဆောင်ပေးရမည်။

## အဆင့် 4 – DNS ဖြတ်တောက်ခြင်းအစီအစဉ်ကို ဖန်တီးပါ။

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

ရလဒ် JSON ကို Ops နှင့် မျှဝေရန် DNS ခလုတ်သည် အတိအကျ ရည်ညွှန်းပါသည်။
ထင်ရှားစွာ ချေဖျက်ပါ။ အစောပိုင်းဖော်ပြချက်အား rollback အရင်းအမြစ်အဖြစ် ပြန်လည်အသုံးပြုသောအခါ၊
`--previous-dns-plan path/to/previous.json` ၏ နောက်ဆက်တွဲ။

## အဆင့် 5 – ဖြန့်ကျက်ထားသော host ကို စစ်ဆေးပါ။

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

စုံစမ်းစစ်ဆေးမှုသည် ဝန်ဆောင်မှုပေးသည့် ထုတ်ဝေမှုတဂ်၊ CSP ခေါင်းစီးများနှင့် လက်မှတ် မက်တာဒေတာတို့ကို အတည်ပြုသည်။
စာရင်းစစ်များမြင်နိုင်စေရန် ဒေသနှစ်ခုမှ (သို့မဟုတ် curl output ကို ပူးတွဲပါ) မှ အမိန့်ကို ပြန်လုပ်ပါ။
edge cache က နွေးထွေးတယ်။

## အထောက်အထား အတွဲ

အစမ်းကြည့်လှိုင်းလက်မှတ်တွင် အောက်ပါပစ္စည်းများကို ထည့်သွင်းပြီး ၎င်းတို့ကို ရည်ညွှန်းပါ။
ဖိတ်ကြားချက်အီးမေးလ်-

| Artefact | ရည်ရွယ်ချက် |
|----------|---------|
| `build/checksums.sha256` | အတွဲသည် CI တည်ဆောက်မှုနှင့် ကိုက်ညီကြောင်း သက်သေပြသည်။ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Canonical SoraFS payload + manifest။ |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | ထင်ရှားသော တင်ပြချက် + အမည်များ ချိတ်ဆက်မှု အောင်မြင်ကြောင်း ပြသသည်။ |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS မက်တာဒေတာ (လက်မှတ်၊ ပြတင်းပေါက်၊ အဆက်အသွယ်များ)၊ လမ်းကြောင်းမြှင့်တင်ရေး (`Sora-Route-Binding`) အနှစ်ချုပ်၊ `route_plan` ညွှန်ပြချက် (အစီအစဉ် JSON + ခေါင်းစီးနမူနာများ)၊ ကက်ရှ်ရှင်းလင်းခြင်း အချက်အလက်နှင့် Ops အတွက် ပြန်လှည့်ရန် ညွှန်ကြားချက်များ။ |
| `artifacts/sorafs/preview-descriptor.json` | မှတ်တမ်းဟောင်း + checksum ကို တွဲချိတ်ထားသည့် ဖော်ပြချက်တွင် လက်မှတ်ထိုးထားသည်။ |
| `probe` အထွက် | တိုက်ရိုက်ထုတ်လွှလက်ခံသူသည် မျှော်လင့်ထားသည့် ဖြန့်ချိမှုတက်ဂ်ကို ကြော်ငြာကြောင်း အတည်ပြုသည်။ |

အစီအစဉ်တင်ဆက်သူသည် တိုက်ရိုက်လွှင့်သည်နှင့်၊ [အစမ်းကြည့်ရှုရန် ဖိတ်ကြားချက်ဖွင့်စာအုပ်](./public-preview-invite.md) ကို လိုက်နာပါ
လင့်ခ်ကိုဖြန့်ဝေရန်၊ မှတ်တမ်းဖိတ်ကြားမှုများနှင့် တယ်လီမီတာကို စောင့်ကြည့်ရန်။