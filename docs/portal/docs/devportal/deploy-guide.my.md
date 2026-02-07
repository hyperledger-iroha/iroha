---
lang: my
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ee0a1d26e0c9f3c1ab8908f7eb0dc73049d452e451aff9d2d892c19d733557e
source_last_modified: "2026-01-22T16:26:46.492940+00:00"
translation_last_reviewed: 2026-02-07
id: deploy-guide
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
---

## ခြုံငုံသုံးသပ်ချက်

ဤပြခန်းစာအုပ်သည် လမ်းပြမြေပုံပါအရာများကို **DOCS-7** (SoraFS ထုတ်ဝေခြင်း) နှင့် **DOCS-8** အဖြစ် ပြောင်းလဲပေးသည်
(CI/CD pin automation) developer portal အတွက် လုပ်ဆောင်နိုင်သော လုပ်ထုံးလုပ်နည်းတစ်ခု။
၎င်းသည် တည်ဆောက်မှု/စိမ်ခံအဆင့်၊ SoraFS ထုပ်ပိုးမှု၊ Sigstore-ကျောထောက်နောက်ခံပြုထားသော မန်နီးဖက်စ်ကို အကျုံးဝင်သည်
လက်မှတ်ထိုးခြင်း၊ နာမည်တူမြှင့်တင်ခြင်း၊ အတည်ပြုခြင်းနှင့် နောက်ပြန်လှည့်ခြင်းဆိုင်ရာ လေ့ကျင့်မှုများ၊ အစမ်းကြည့်ရှုမှုတိုင်းနှင့်
ထုတ်ဝေမှုအနုပညာသည် ပြန်လည်ထုတ်လုပ်၍ စစ်ဆေးနိုင်သည်။

flow သည် သင့်တွင် `sorafs_cli` binary (ဖြင့်တည်ဆောက်ထားသည်
`--features cli`)၊ pin-registry ခွင့်ပြုချက်ဖြင့် Torii အဆုံးမှတ်သို့ ဝင်ရောက်ကြည့်ရှုခြင်း၊
OIDC အထောက်အထားများ Sigstore။ သက်တမ်းရှည်လျှို့ဝှက်ချက်များကို သိမ်းဆည်းပါ (`IROHA_PRIVATE_KEY`၊
သင်၏ CI ခန်းရှိ `SIGSTORE_ID_TOKEN`၊ Torii တိုကင်များ) ဒေသတွင်း လည်ပတ်မှုများသည် ၎င်းတို့ကို ရင်းမြစ်ပေးနိုင်သည်။
ခွံတင်ပို့မှုမှ

## လိုအပ်ချက်များ

- `npm` သို့မဟုတ် `pnpm` ပါသော Node 18.18+။
- `sorafs_cli` မှ `cargo run -p sorafs_car --features cli --bin sorafs_cli`။
- Torii URL နှင့် `/v1/sorafs/*` နှင့် အခွင့်အာဏာအကောင့်/ပုဂ္ဂလိကသော့ကို ဖော်ထုတ်ပေးသည်
  ၎င်းသည် manifests နှင့် aliases များကိုတင်ပြနိုင်သည်။
- OIDC ထုတ်ပေးသူ (GitHub လုပ်ဆောင်ချက်များ၊ GitLab၊ အလုပ်ဝန်အထောက်အထား စသည်ဖြင့်)
  `SIGSTORE_ID_TOKEN`။
- ရွေးချယ်နိုင်သည်- `examples/sorafs_cli_quickstart.sh` နှင့် ခြောက်သွေ့သောပြေးခြင်းများအတွက်
  GitHub/GitLab အလုပ်အသွားအလာငြမ်းအတွက် `docs/source/sorafs_ci_templates.md`။
- Try it OAuth variables (`DOCS_OAUTH_*`) ကို configure လုပ်ပြီး run ပါ။
  [လုံခြုံရေး-တင်းမာမှုစစ်ဆေးစာရင်း](./security-hardening.md) တည်ဆောက်မှုကို မမြှင့်တင်မီ
  ဓာတ်ခွဲခန်းအပြင်ဘက်။ ဤကိန်းရှင်များ ပျောက်ဆုံးနေချိန်တွင် ပေါ်တယ်တည်ဆောက်မှု ပျက်ကွက်သွားပါသည်။
  သို့မဟုတ် TTL/မဲရုံခလုတ်များသည် ပြဌာန်းထားသော ပြတင်းပေါက်များအပြင်ဘက်သို့ ကျရောက်သည့်အခါ၊ တင်ပို့ခြင်း။
  `DOCS_OAUTH_ALLOW_INSECURE=1` သည် တစ်ခါသုံး ဒေသတွင်း အစမ်းကြည့်ရှုခြင်းအတွက်သာ။ ပူးတွဲပါ
  pen-test သက်သေခံချက် လက်မှတ်ကို ထုတ်ပေးသည်။

## အဆင့် 0 — စမ်းသုံးကြည့်ပါ ပရောက်စီအတွဲကို ဖမ်းပါ။

Netlify သို့မဟုတ် gateway သို့ အစမ်းကြည့်ရှုခြင်းအား မကြော်ငြာမီ၊ Try it proxy ကို တံဆိပ်နှိပ်ပါ။
ရင်းမြစ်များနှင့် OpenAPI တွင် ရေးထိုးထားသော manifest ကို အဆုံးအဖြတ် အစုအဝေးတစ်ခုအဖြစ် ချေဖျက်ပါ-

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` သည် proxy/probe/rollback helpers၊
OpenAPI လက်မှတ်ကို စစ်ဆေးပြီး `release.json` plus ရေးသည်
`checksums.sha256`။ ဤအစုအဝေးကို Netlify/SoraFS ဂိတ်ဝေးပရိုမိုးရှင်းသို့ ပူးတွဲပါ
သုံးသပ်သူများသည် တိကျသော ပရောက်စီရင်းမြစ်များနှင့် Torii ပစ်မှတ်အရိပ်အမြွက်များကို ပြန်လည်ပြသနိုင်စေရန် လက်မှတ်
ပြန်လည်တည်ဆောက်ခြင်းမရှိဘဲ။ အစုအဝေးသည် ဖောက်သည်ပေးဆောင်သော သယ်ဆောင်သူ ဟုတ်မဟုတ်ကိုလည်း မှတ်တမ်းတင်ပါသည်။
ထုတ်ဝေမှုအစီအစဉ်နှင့် CSP စည်းမျဉ်းများကို တစ်ပြိုင်တည်းထားရှိရန် (`allow_client_auth`) ကို ဖွင့်ထားသည်။

## အဆင့် 1 — ပေါ်တယ်ကို တည်ဆောက်ပြီး အလင်းတန်းလုပ်ပါ။

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` သည် `scripts/write-checksums.mjs` ကို အလိုအလျောက် လုပ်ဆောင်ပေးသည်၊ ထုတ်လုပ်သည်-

- `build/checksums.sha256` — SHA256 မန်နီးဖက်စ် `sha256sum -c`။
- `build/release.json` — မက်တာဒေတာ (`tag`၊ `generated_at`၊ `source`) တွင် ပင်ထိုးထားသည်
  CAR/manifest တိုင်း။

သုံးသပ်သူများသည် ကွဲပြားစွာ အစမ်းကြည့်ရှုနိုင်စေရန် CAR အနှစ်ချုပ်နှင့်အတူ ဖိုင်နှစ်ခုလုံးကို သိမ်းဆည်းပါ။
ပြန်လည်တည်ဆောက်ခြင်းမပြုဘဲ ရှေးဟောင်းပစ္စည်းများ။

## အဆင့် 2 — တည်ငြိမ်သောပိုင်ဆိုင်မှုများကို ထုပ်ပိုးပါ။

CAR packer ကို Docusaurus အထွက်လမ်းကြောင်းနှင့် ဆန့်ကျင်ပြီး လုပ်ဆောင်ပါ။ အောက်ပါဥပမာ
`artifacts/devportal/` အောက်တွင် ရှေးဟောင်းပစ္စည်းအားလုံးကို ရေးသည်။

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

အကျဉ်းချုပ် JSON သည် အစုအပုံလိုက် အရေအတွက်များ၊ အချေအတင်များနှင့် သက်သေပြခြင်းဆိုင်ရာ အရိပ်အမြွက်များကို ဖမ်းယူသည်
`manifest build` နှင့် CI ဒက်ရှ်ဘုတ်များကို နောက်ပိုင်းတွင် ပြန်သုံးသည်။

## အဆင့် 2b — Package OpenAPI နှင့် SBOM အဖော်များ

DOCS-7 သည် ပေါ်တယ်ဆိုက်၊ OpenAPI လျှပ်တစ်ပြက်ရိုက်ချက်နှင့် SBOM ပေးချေမှုများအား ထုတ်ဝေရန် လိုအပ်သည်
ထင်ရှားသော ထင်ရှားသည့်အတိုင်း ဂိတ်ဝေးများသည် `Sora-Proof`/`Sora-Content-CID` ကို အဓိကထားနိုင်သည်
ပစ္စည်းတစ်ခုစီအတွက် ခေါင်းစီးများ။ လွတ်မြောက်ရေးအထောက်
(`scripts/sorafs-pin-release.sh`) OpenAPI လမ်းညွှန်ကို ထုပ်ပိုးပြီးသား
(`static/openapi/`) နှင့် `syft` မှတဆင့် ထုတ်လွှတ်သော SBOM များကို သီးခြား
`openapi.*`/`*-sbom.*` မော်တော်ကားများနှင့် မက်တာဒေတာကို မှတ်တမ်းတင်သည်
`artifacts/sorafs/portal.additional_assets.json`။ manual flow ကို လည်ပတ်တဲ့အခါ၊
၎င်း၏ကိုယ်ပိုင်ရှေ့ဆက်များနှင့် မက်တာဒေတာအညွှန်းများဖြင့် payload တစ်ခုစီအတွက် အဆင့် 2-4 ကို ပြန်လုပ်ပါ။
(ဥပမာ `--car-out "$OUT"/openapi.car` အပေါင်း
`--metadata alias_label=docs.sora.link/openapi`)။ manifest/alias တိုင်းကို မှတ်ပုံတင်ပါ။
DNS ကိုမပြောင်းမီ Torii (ဆိုက်၊ OpenAPI၊ portal SBOM၊ OpenAPI SBOM) တွင် တွဲထားပါ
တံခါးပေါက်သည် ထုတ်ဝေထားသော ပစ္စည်းအားလုံးအတွက် ချည်မျှင်အထောက်အထားများကို ဆောင်ရွက်ပေးနိုင်သည်။

## အဆင့် 3 — ဖော်ပြချက်ကို တည်ဆောက်ပါ။

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

သင်၏ ထုတ်ဝေမှုဝင်းဒိုးတွင် ပင်နံပါတ်မူဝါဒအလံများကို ချိန်ညှိပါ (ဥပမာ၊ `--pin-storage-class
ကိန္နရီများအတွက် hot`)။ JSON မူကွဲသည် ရွေးချယ်နိုင်သော်လည်း ကုဒ်ပြန်လည်သုံးသပ်ရန်အတွက် အဆင်ပြေသည်။

## အဆင့် 4 — Sigstore ဖြင့် လက်မှတ်ထိုးပါ။

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

အစုအဝေးသည် ထင်ရှားသော ချေဖျက်မှု၊ အတုံးအခဲများ နှင့် BLAKE3 hash တို့ကို မှတ်တမ်းတင်သည်။
JWT ကို ဆက်မလုပ်ဘဲ OIDC တိုကင်။ အစုအဝေးနှစ်ခုလုံးကို သီးသန့်ထားပြီး သိမ်းထားပါ။
လက်မှတ်; ထုတ်လုပ်မှုမြှင့်တင်ရေးများသည် ရာထူးမှနှုတ်ထွက်မည့်အစား တူညီသောပစ္စည်းများကို ပြန်လည်အသုံးပြုနိုင်ပါသည်။
Local runs များသည် ဝန်ဆောင်မှုပေးသောအလံများကို `--identity-token-env` (သို့မဟုတ်သတ်မှတ်ထားသည်။
ပတ်ဝန်းကျင်ရှိ `SIGSTORE_ID_TOKEN`) ပြင်ပ OIDC အကူအညီပေးသူက ထုတ်ပေးသည့်အခါ၊
တိုကင်။

## အဆင့် 5 — pin registry သို့ တင်သွင်းပါ။

Torii သို့ လက်မှတ်ရေးထိုးထားသော မန်နီးဖက်စ် (နှင့် အပိုင်းအစီအစဉ်) ကို တင်ပြပါ။ အနှစ်ချုပ်ကို အမြဲတောင်းဆိုပါ။
ထို့ကြောင့် ရရှိလာသော မှတ်ပုံတင်ခြင်း/အမည်လွဲ အထောက်အထားကို စစ်ဆေးနိုင်သည်။

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

အစမ်းကြည့်ရှုခြင်း သို့မဟုတ် Canary alias (`docs-preview.sora`) ကို စတင်သောအခါ၊
QA သည် ထုတ်လုပ်ခြင်းမပြုမီ အကြောင်းအရာကို အတည်ပြုနိုင်စေရန် သီးသန့်အမည်နာမတစ်ခုဖြင့် တင်သွင်းခြင်း။
ပရိုမိုးရှင်း။

Alias binding သည် အကွက်သုံးခု လိုအပ်သည်- `--alias-namespace`၊ `--alias-name` နှင့်
`--alias-proof`။ အုပ်ချုပ်ရေးသည် သက်သေအတွဲ (base64 သို့မဟုတ် Norito bytes) ကို ထုတ်လုပ်သည်
အမည်နာမတောင်းဆိုမှုကို အတည်ပြုသောအခါ၊ ၎င်းကို CI လျှို့ဝှက်ချက်များတွင် သိမ်းဆည်းပြီး ၎င်းကို ပုံသဏ္ဍာန်အဖြစ် ဖော်ပြပါ။
`manifest submit` ကို မခေါ်မီ ဖိုင်။ သင်သောအခါတွင် နာမည်ဝှက်အလံများကို မသတ်မှတ်ဘဲထားလိုက်ပါ။
DNS ကို မထိဘဲ မန်နီးဖက်စ်ကို ပင်ထိုးရန်သာ ရည်ရွယ်သည်။

## အဆင့် 5b — အုပ်ချုပ်မှုအဆိုပြုချက်ကို ဖန်တီးပါ။

သရုပ်ပြမှုတိုင်းသည် Sora အဆိုပြုချက်တစ်ခုနှင့် ပါလီမန်-အဆင်သင့်ဖြစ်သင့်သည်။
နိုင်ငံသားသည် အခွင့်ထူးခံအထောက်အထားများကို မချေးဘဲ အပြောင်းအလဲကို မိတ်ဆက်နိုင်သည်။
တင်သွင်းခြင်း/လက်မှတ်ထိုးခြင်း အဆင့်များပြီးနောက်၊ လုပ်ဆောင်ရန်-

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` သည် Canonical `RegisterPinManifest` ကို ရိုက်ကူးသည်
ညွန်ကြားချက်၊ အတုံးအခဲအကြေ၊ မူဝါဒ၊ နှင့် နာမည်အရင်း အရိပ်အမြွက်။ အဲဒါကို အုပ်ချုပ်ရေးနဲ့ တွဲပါ။
လက်မှတ် သို့မဟုတ် ပါလီမန်ပေါ်တယ်မှ ကိုယ်စားလှယ်များသည် ပြန်လည်တည်ဆောက်ခြင်းမပြုဘဲ payload ကွဲပြားနိုင်သည်။
ပစ္စည်းများ။ command သည် Torii အခွင့်အာဏာကီးကို ဘယ်သောအခါမှ မထိသောကြောင့်၊
နိုင်ငံသားများသည် ပြည်တွင်းတွင် အဆိုပြုချက် ရေးဆွဲနိုင်သည်။

## အဆင့် 6 — အထောက်အထားများနှင့် တယ်လီမီတာကို စစ်ဆေးပါ။

ပင်ထိုးပြီးနောက်၊ ဆုံးဖြတ်အတည်ပြုခြင်းအဆင့်များကို လုပ်ဆောင်ပါ-

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- `torii_sorafs_gateway_refusals_total` နှင့် စစ်ဆေးပါ။
  ကွဲလွဲချက်များများအတွက် `torii_sorafs_replication_sla_total{outcome="missed"}`။
- Try-It ပရောက်စီနှင့် မှတ်တမ်းတင်ထားသော လင့်ခ်များကို ကျင့်သုံးရန် `npm run probe:portal` ကိုဖွင့်ပါ။
  အသစ်ထိုးထားသော အကြောင်းအရာနှင့် ဆန့်ကျင်ဘက်။
- တွင်ဖော်ပြထားသော စောင့်ကြည့်ရေးအထောက်အထားများကို ဖမ်းယူပါ။
  [ထုတ်ဝေခြင်းနှင့် စောင့်ကြည့်လေ့လာခြင်း](./publishing-monitoring.md) ထို့ကြောင့် DOCS-3c's
  ထုတ်ဝေမှုအဆင့်များနှင့်အတူ စောင့်ကြည့်လေ့လာနိုင်မှုတံခါးသည် ကျေနပ်မှုရှိသည်။ အထောက်အမ
  ယခု `bindings` အများအပြားကို လက်ခံပါသည် (ဆိုက်၊ OpenAPI၊ ပေါ်တယ် SBOM၊ OpenAPI
  SBOM) နှင့် `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` ကို ပစ်မှတ်ပေါ်တွင် ပြဋ္ဌာန်းသည်
  စိတ်ကြိုက်ရွေးချယ်နိုင်သော `hostname` အစောင့်တပ်မှတဆင့် လက်ခံဆောင်ရွက်ပေးသည်။ အောက်ဖော်ပြပါ ဖိတ်စာတွင် a နှစ်ခုစလုံးကို ရေးထားသည်။
  JSON တစ်ခုတည်း အနှစ်ချုပ်နှင့် အထောက်အထားအစုအဝေး (`portal.json`၊ `tryit.json`၊
  ထုတ်ဝေမှုလမ်းညွှန်အောက်တွင် `binding.json` နှင့် `checksums.sha256`)

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## အဆင့် 6a — ဂိတ်ဝေးလက်မှတ်များကို စီစဉ်ပါ။

GAR ပက်ကတ်များကို မဖန်တီးမီ TLS SAN/စိန်ခေါ်မှု အစီအစဉ်ကို ရယူပါ။
အဖွဲ့နှင့် DNS အတည်ပြုသူများသည် တူညီသောအထောက်အထားများကို ပြန်လည်သုံးသပ်သည်။ အထောက်အပံအသစ်က ကြေးမုံပြင်
Canonical wildcard host ကိုရေတွက်ခြင်းဖြင့် DG-3 အလိုအလျောက်စနစ်ထည့်သွင်းမှုများ၊
pretty-host SANs၊ DNS-01 အညွှန်းများနှင့် ACME စိန်ခေါ်မှုများကို အကြံပြုထားသည်-

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

ထုတ်ဝေမှုအစုအဝေးနှင့်အတူ JSON ကို ထည့်သွင်းပါ (သို့မဟုတ် အပြောင်းအလဲနှင့်အတူ ၎င်းကို အပ်လုဒ်လုပ်ပါ။
လက်မှတ်) ထို့ကြောင့် အော်ပရေတာများသည် SAN တန်ဖိုးများကို Torii ထဲသို့ ကူးထည့်နိုင်သည်။
`torii.sorafs_gateway.acme` configuration နှင့် GAR reviewers မှ အတည်ပြုနိုင်ပါသည်။
လက်ခံသူ၏ ဆင်းသက်လာခြင်းမရှိဘဲ ပြန်လည်လုပ်ဆောင်ခြင်းမရှိဘဲ canonical/ pretty mappings များ။ ထပ်လောင်းထည့်ပါ။
`--name` တူညီသောထုတ်ဝေမှုတွင်မြှင့်တင်ထားသော နောက်ဆက်တွဲတစ်ခုစီအတွက် အကြောင်းပြချက်များ။

## အဆင့် 6b — canonical host mappings များကို ရယူပါ။

GAR payload များကို နမူနာပုံစံမဖော်မီ၊ တစ်ခုချင်းစီအတွက် သတ်မှတ်ထားသော host mapping ကို မှတ်တမ်းတင်ပါ။
နာမည်များ `cargo xtask soradns-hosts` သည် `--name` တစ်ခုစီကို ၎င်း၏ canonical အဖြစ်သို့ hashe လုပ်သည်
အညွှန်း (`<base32>.gw.sora.id`)၊ လိုအပ်သော သင်္ကေတကို ထုတ်လွှတ်သည်။
(`*.gw.sora.id`) နှင့် လှပသော host (`<alias>.gw.sora.name`) ကို ဆင်းသက်လာသည်။ ဆက်နေပါ။
DG-3 ပြန်လည်သုံးသပ်သူများသည် မြေပုံထုတ်ခြင်းကို ကွဲပြားစေနိုင်သောကြောင့် ထုတ်ဝေမှုတွင် ထွက်ပေါ်လာသည့်အရာများ
GAR တင်ပြချက်နှင့်အတူ-

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

GAR သို့မဟုတ် gateway တစ်ခုရှိတိုင်း အမြန်ပျက်ကွက်ရန် `--verify-host-patterns <file>` ကိုသုံးပါ။
ချိတ်ဆက်ထားသော JSON သည် လိုအပ်သော host များထဲမှ တစ်ခုကို ချန်လှပ်ထားသည်။ အကူအညီပေးသူက မျိုးစုံကို လက်ခံသည်။
GAR နမူနာပုံစံနှင့် ဖိုင်နှစ်ခုစလုံးကို လွယ်ကူစွာ လင်းစေခြင်းဖြင့် အတည်ပြုခြင်းဖိုင်များ
တူညီသောတောင်းဆိုချက်တွင် `portal.gateway.binding.json` ကို ချည်ထိုးထားသည်။

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

အကျဉ်းချုပ် JSON နှင့် အတည်ပြုခြင်းမှတ်တမ်းကို DNS/gateway ပြောင်းလဲခြင်းလက်မှတ်သို့ တွဲပေးပါ။
စာရင်းစစ်များသည် ပြန်လည်လည်ပတ်ခြင်းမပြုဘဲ canonical၊ wildcard နှင့် pretty host များကို အတည်ပြုနိုင်သည်။
ဆင်းသက်လာသော script များ။ နံပတ်အသစ်များကို ထည့်သွင်းသည့်အခါတိုင်း အမိန့်ကို ပြန်လည်လုပ်ဆောင်ပါ။
ထို့ကြောင့် နောက်ဆက်တွဲ GAR အပ်ဒိတ်များသည် တူညီသောအထောက်အထားလမ်းကြောင်းကို အမွေဆက်ခံပါသည်။

## အဆင့် 7 — DNS ဖြတ်တောက်ခြင်းဖော်ပြချက်ကို ဖန်တီးပါ။

ထုတ်လုပ်မှုဖြတ်တောက်မှုများသည် စာရင်းစစ်နိုင်သော ပြောင်းလဲမှု ပက်ကေ့ခ်ျတစ်ခု လိုအပ်သည်။ အောင်မြင်ပြီးနောက်
submission (alias binding) အထောက်အပံ ထွက်လာတယ်။
`artifacts/sorafs/portal.dns-cutover.json`၊ ရိုက်ကူးခြင်း-- အမည်များ ပေါင်းစပ်ထားသော မက်တာဒေတာ (namespace/name/proof၊ manifest digest၊ Torii URL၊
  တင်ပြသည့်ခေတ်၊ အခွင့်အာဏာ);
- ထုတ်လွှတ်သည့်အကြောင်းအရာ (တက်ဂ်၊ နာမည်တူ အညွှန်း၊ မန်နီးဖက်စ်/ကားလမ်းကြောင်းများ၊ အပိုင်းအစီအစဉ်၊ Sigstore
  အတွဲ);
- အတည်ပြုညွှန်ပြချက်များ (စုံစမ်းစစ်ဆေးရန် ညွှန်ကြားချက်၊ နံပတ်များ + Torii အဆုံးမှတ်); နှင့်
- ရွေးချယ်နိုင်သောပြောင်းလဲမှုထိန်းချုပ်မှုအကွက်များ (လက်မှတ် ID၊ ဖြတ်တောက်ထားသောဝင်းဒိုး၊ ops အဆက်အသွယ်၊
  ထုတ်လုပ်ရေးဌာနအမည်/ဇုန်);
- stapled `Sora-Route-Binding` မှရရှိသော လမ်းကြောင်းမြှင့်တင်ရေး မက်တာဒေတာ
  header (canonical host/CID၊ header + binding paths၊ verification commands)၊
  GAR ပရိုမိုးရှင်းနှင့် နောက်ပြန်ဆုတ်လေ့ကျင့်ခန်းများ တူညီသောအထောက်အထားများကို ကိုးကားရန် သေချာစေခြင်း၊
- ထုတ်လုပ်ထားသော လမ်းကြောင်း-အစီအစဥ် လက်ရာများ (`gateway.route_plan.json`၊
  ခေါင်းစီးတင်းပလိတ်များ၊ နှင့် ရွေးချယ်နိုင်သော နောက်ပြန်လှည့်မှု ခေါင်းစီးများ) ထို့ကြောင့် လက်မှတ်များနှင့် CI ကိုပြောင်းပါ။
  ပိုးမွှားချိတ်များသည် DG-3 packetတိုင်းသည် canonical ကို ကိုးကားကြောင်း အတည်ပြုနိုင်သည်။
  ခွင့်ပြုချက်မရသေးမီ ပရိုမိုးရှင်း/ပြန်လှည့်မည့် အစီအစဉ်များ။
- ရွေးချယ်နိုင်သော ကက်ရှ်အကျုံးမဝင်သော မက်တာဒေတာ (အဆုံးသတ်မှတ်၊ စစ်မှန်သော ပြောင်းလဲနိုင်သော၊ JSON
  payload နှင့် ဥပမာ `curl` command); နှင့်
- ယခင်ဖော်ပြချက်အား ညွှန်ပြသည့် အရိပ်အမြွက်များ (ထုတ်လွှတ်ခြင်း tag နှင့် မန်နီးဖက်စ်
  digest) ထို့ကြောင့် လက်မှတ်များကို အဆုံးအဖြတ်ပေးသော ဆုတ်ယုတ်မှုလမ်းကြောင်းကို ဖမ်းယူရန် လက်မှတ်များကို ပြောင်းလဲပါ။

ထုတ်ဝေမှုတွင် ကက်ရှ်ရှင်းလင်းမှုများ လိုအပ်သောအခါ၊ ၎င်းနှင့် တွဲလျက် canonical အစီအစဉ်ကို ဖန်တီးပါ။
ဖြတ်တောက်ဖော်ပြချက်

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

ရလဒ် `portal.cache_plan.json` ကို DG-3 ပက်ကတ်သို့ ချိတ်ပါ သို့မှသာ အော်ပရေတာများ၊
ထုတ်ပေးသည့်အခါတွင် သတ်မှတ်ထားသော host/paths (နှင့် ကိုက်ညီသော auth အရိပ်အမြွက်များ) ရှိသည်။
`PURGE` တောင်းဆိုချက်များ။ ဖော်ပြသူ၏ ရွေးချယ်နိုင်သော ကက်ရှ် မက်တာဒေတာအပိုင်းကို ကိုးကားနိုင်သည်။
ဤဖိုင်သည် အပြောင်းအလဲ-ထိန်းချုပ်မှု ပြန်လည်သုံးသပ်သူများကို အတိအကျ လိုက်လျောညီထွေဖြစ်စေမည့် ဤဖိုင်ကို တိုက်ရိုက်ပေးသည်။
ဖြတ်တောက်မှုအတွင်း အဆုံးမှတ်များကို ဖယ်ထုတ်သည်။

DG-3 ပက်ကတ်တိုင်းသည် ပရိုမိုးရှင်း + လှည့်ပြန်စစ်ဆေးမှုစာရင်းလည်း လိုအပ်ပါသည်။ မှတဆင့်ထုတ်လုပ်ပါ။
`cargo xtask soradns-route-plan` ထို့ကြောင့် အပြောင်းအလဲထိန်းချုပ်ရေးသုံးသပ်သူများသည် အတိအကျခြေရာခံနိုင်သည်။
နာမည်တူတစ်ခုအတွက် ကြိုတင်ပျံသန်းမှု၊ ဖြတ်တောက်မှုနှင့် ပြန်လှည့်မှုအဆင့်များ-

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

ထုတ်လွှတ်သော `gateway.route_plan.json` သည် သရုပ်ဖော်ပုံများ/လှပသော host များကို ဖမ်းယူ၊
ကျန်းမာရေးစစ်ဆေးမှုသတိပေးချက်များ၊ GAR ချိတ်ဆက်မှုမွမ်းမံမှုများ၊ ကက်ရှ်ရှင်းလင်းမှုများနှင့် ပြန်လှည့်လုပ်ဆောင်မှုများ။
အပြောင်းအလဲကို မတင်ပြမီ GAR/binding/cutover artefacts များဖြင့် စုစည်းပါ။
လက်မှတ်ကို Ops က အစမ်းလေ့ကျင့်ပြီး တူညီသော ဇာတ်ညွှန်းရေးထားသော အဆင့်များတွင် လက်မှတ်ထိုးနိုင်သည်။

`scripts/generate-dns-cutover-plan.mjs` သည် ဤဖော်ပြချက်အား အားကောင်းစေပြီး အလုပ်လုပ်သည်။
`sorafs-pin-release.sh` မှ အလိုအလျောက်။ ပြန်ထုတ်ရန် သို့မဟုတ် စိတ်ကြိုက်ပြင်ဆင်ပါ။
ကိုယ်တိုင်-

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

ပင်ကိုမဖွင့်မီ ပတ်ဝန်းကျင် ကိန်းရှင်များမှတစ်ဆင့် ရွေးချယ်နိုင်သော မက်တာဒေတာကို ထည့်သွင်းပါ။
ကူညီသူ-

| ပြောင်းလဲနိုင်သော | ရည်ရွယ်ချက် |
|----------|---------|
| `DNS_CHANGE_TICKET` | လက်မှတ် ID ကို ဖော်ပြချက်တွင် သိမ်းဆည်းထားသည်။ |
| `DNS_CUTOVER_WINDOW` | ISO8601 ဖြတ်တောက်ထားသော ဝင်းဒိုး (ဥပမာ၊ `2026-03-21T15:00Z/2026-03-21T15:30Z`)။ |
| `DNS_HOSTNAME`, `DNS_ZONE` | ထုတ်လုပ်သူအမည် + တရားဝင်ဇုန်။ |
| `DNS_OPS_CONTACT` | ဖုန်းခေါ်ဆိုမှုတွင် အမည်တူ သို့မဟုတ် တိုးမြင့်ဆက်သွယ်မှု။ |
| `DNS_CACHE_PURGE_ENDPOINT` | ဖော်ပြချက်တွင် မှတ်တမ်းတင်ထားသော ကက်ရှ် အဆုံးမှတ်ကို ဖယ်ရှားပါ။ |
| `DNS_CACHE_PURGE_AUTH_ENV` | သန့်ရှင်းမှုတိုကင်ပါရှိသော Env var (ပုံသေ `CACHE_PURGE_TOKEN`)။ |
| `DNS_PREVIOUS_PLAN` | rollback မက်တာဒေတာအတွက် ယခင်ဖြတ်တောက်ထားသော ဖော်ပြချက်သို့ လမ်းကြောင်း။ |

အတည်ပြုသူများသည် ထင်ရှားစွာအတည်ပြုနိုင်စေရန်အတွက် JSON ကို DNS ပြောင်းလဲမှုပြန်လည်သုံးသပ်မှုတွင် ပူးတွဲပါရှိသည်။
CI မှတ်တမ်းများကို မခြစ်ဘဲ ချေဖျက်မှုများ၊ alias bindings နှင့် probe commands များ။
CLI အလံများ `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`၊
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`၊
`--cache-purge-auth-env` နှင့် `--previous-dns-plan` သည် တူညီသော overrides များကို ပေးသည်
CI အပြင်ဘက်တွင် helper ကို run သောအခါ။

## အဆင့် 8 — ဖြေရှင်းရေးဇုန်ဖိုင်အရိုးစုကို ထုတ်လွှတ်ပါ (ချန်လှပ်ထားနိုင်သည်)

ထုတ်လုပ်မှုဖြတ်တောက်ခြင်းဝင်းဒိုးကို သိသောအခါ၊ ထုတ်ဝေမှု ဇာတ်ညွှန်းသည် ၎င်းကို ထုတ်လွှတ်နိုင်သည်။
SNS ဇုန်ဖိုင်အရိုးစုနှင့် ဖြေရှင်းသူအတိုအထွာ အလိုအလျောက်။ လိုချင်သော DNS ကိုဖြတ်သန်းပါ။
ပတ်ဝန်းကျင် ပြောင်းလဲနိုင်သော သို့မဟုတ် CLI ရွေးချယ်မှုများမှတစ်ဆင့် မှတ်တမ်းများနှင့် မက်တာဒေတာ၊ ကူညီသူ
ဖြတ်တောက်ပြီးနောက် ချက်ချင်း `scripts/sns_zonefile_skeleton.py` သို့ ဖုန်းခေါ်ဆိုပါမည်။
ဖော်ပြချက်ထုတ်ပေးသည်။ အနည်းဆုံး A/AAAA/CNAME တန်ဖိုးတစ်ခုနှင့် GAR ကို ပေးပါ။
အကြမ်းဖျင်း (BLAKE3-256 လက်မှတ်ထိုးထားသော GAR ပေးဆောင်မှု)။ ဇုန်/အိမ်ရှင်အမည်ကို သိပါက
`--dns-zonefile-out` ကို ချန်လှပ်ထားပြီး အကူအညီပေးသူက စာရေးသည်။
`artifacts/sns/zonefiles/<zone>/<hostname>.json` နဲ့ တွေ့ရပါမယ်။
ဖြေရှင်းသူအတိုအထွာအဖြစ် `ops/soradns/static_zones.<hostname>.json`။

| ပြောင်းလဲနိုင်သော / အလံ | ရည်ရွယ်ချက် |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | ထုတ်ပေးထားသော ဇုန်ဖိုင်အရိုးစုအတွက် လမ်းကြောင်း။ |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | ဖြေရှင်းချက်အတိုအထွာလမ်းကြောင်း (ချန်လှပ်ထားသည့်အခါ `ops/soradns/static_zones.<hostname>.json` သို့ ပုံသေများ)။ |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | ထုတ်ပေးထားသော မှတ်တမ်းများတွင် TTL ကို အသုံးပြုသည် (မူလ- 600 စက္ကန့်)။ |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 လိပ်စာများ (ကော်မာ-ခြားထားသော env သို့မဟုတ် ထပ်လုပ်နိုင်သော CLI အလံ)။ |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 လိပ်စာများ။ |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | ရွေးချယ်နိုင်သော CNAME ပစ်မှတ်။ |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI တံများ (base64)။ |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | အပို TXT ထည့်သွင်းမှုများ (`key=value`)။ |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | တွက်ချက်ထားသော ဇုန်ဖိုင်ဗားရှင်းအညွှန်းကို အစားထိုးပါ။ |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | ဖြတ်တောက်ထားသော ဝင်းဒိုးစတင်ခြင်းအစား `effective_at` အချိန်တံဆိပ် (RFC3339) ကို အတင်းအကျပ် ဖိအားပေးပါ။ |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | မက်တာဒေတာတွင် မှတ်တမ်းတင်ထားသော ပကတိအထောက်အထားကို အစားထိုးပါ။ |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | မက်တာဒေတာတွင် မှတ်တမ်းတင်ထားသော CID ကို အစားထိုးပါ။ |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | အုပ်ထိန်းသူသည် အေးခဲနေသောအခြေအနေ (ပျော့ပျောင်းသော၊ မာကြောသော၊ ရောနှောခြင်း၊ စောင့်ကြည့်ခြင်း၊ အရေးပေါ်)။ |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | အုပ်ထိန်းသူ/ကောင်စီ လက်မှတ် ကိုးကားချက်။ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | ရောနှောခြင်းအတွက် RFC3339 အချိန်တံဆိပ် |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | နောက်ထပ် အေးခဲသောမှတ်စုများ (ကော်မာ-ခြားထားသော env သို့မဟုတ် ထပ်ခါတလဲလဲနိုင်သော အလံ)။ |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 ရေးထိုးထားသော GAR ပေးဆောင်မှု၏ ချေဖျက်မှု (hex)။ Gateway binding ရှိနေသည့်အခါတိုင်း လိုအပ်ပါသည်။ |

GitHub လုပ်ဆောင်ချက်များ အလုပ်အသွားအလာသည် ဤတန်ဖိုးများကို သိုလှောင်မှုလျှို့ဝှက်ချက်များမှ ဖတ်ပြသောကြောင့် ထုတ်လုပ်မှုပင်နံပါတ်တိုင်းသည် zonefile artefacts ကို အလိုအလျောက် ထုတ်လွှတ်ပါသည်။ အောက်ပါလျှို့ဝှက်ချက်များကို စီစဉ်သတ်မှတ်ပါ (စာတန်းများတွင် တန်ဖိုးများစွာ အကွက်များအတွက် ကော်မာ-ခြားထားသော စာရင်းများ ပါဝင်နိုင်သည်-

| လျှို့ဝှက်ချက် | ရည်ရွယ်ချက် |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | ထုတ်လုပ်သူအမည်/ဇုန်ကို ကူညီသူထံ လွှဲပြောင်းပေးခဲ့သည်။ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | ဖော်ပြချက်တွင် သိမ်းဆည်းထားသော ဖုန်းခေါ်ဆိုမှုဆိုင်ရာ အမည်များ။ |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | ထုတ်ဝေရန် IPv4/IPv6 မှတ်တမ်းများ။ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | ရွေးချယ်နိုင်သော CNAME ပစ်မှတ်။ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI တံများ။ |
| `DOCS_SORAFS_ZONEFILE_TXT` | အပို TXT ထည့်သွင်းမှုများ။ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | အရိုးစုတွင် မှတ်တမ်းတင်ထားသော မက်တာဒေတာကို အေးခဲထားပါ။ |
| `DOCS_SORAFS_GAR_DIGEST` | လက်မှတ်ရေးထိုးထားသော GAR ပေးဆောင်မှု၏ Hex-encoded BLAKE3 ၏ အနှစ်ချုပ်။ |

`.github/workflows/docs-portal-sorafs-pin.yml` ကို စတင်သောအခါ၊ `dns_change_ticket` နှင့် `dns_cutover_window` ထည့်သွင်းမှုများကို ပံ့ပိုးပေးခြင်းဖြင့် ဖော်ပြချက်/ဇုန်ဖိုင်သည် မှန်ကန်သောပြောင်းလဲမှုဝင်းဒိုးမက်တာဒေတာကို အမွေဆက်ခံပါသည်။ ခြောက်သွေ့သော အပြေးအလွှားများ လုပ်ဆောင်နေမှသာ ၎င်းတို့အား ကွက်လပ်ထားပါ။

ပုံမှန်တောင်းဆိုချက် (SN-7 ပိုင်ရှင် runbook နှင့် ကိုက်ညီသည်)

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

အကူအညီပေးသူက TXT ဝင်ခွင့်နှင့် အပြောင်းအလဲလက်မှတ်ကို အလိုအလျောက် သယ်ဆောင်သွားပါသည်။
`effective_at` timestamp မဟုတ်လျှင် cutover window start ကို အမွေဆက်ခံသည်။
လွမ်းမိုး။ လုပ်ငန်းလည်ပတ်မှု အပြည့်အ၀အတွက်၊ ကြည့်ပါ။
`docs/source/sorafs_gateway_dns_owner_runbook.md`။

### အများသူငှာ DNS ကိုယ်စားလှယ်အဖွဲ့ မှတ်ချက်

zonefile အရိုးစုသည် ဇုန်အတွက် တရားဝင်မှတ်တမ်းများကိုသာ သတ်မှတ်သည်။ မင်း
သင်၏မှတ်ပုံတင်အရာရှိ သို့မဟုတ် DNS တွင် မိဘရပ်ဝန်း NS/DS ကိုယ်စားလှယ်အဖွဲ့ကို စီစဉ်သတ်မှတ်ရန် လိုအပ်နေသေးသည်။
ပံ့ပိုးပေးသူဖြစ်သောကြောင့် ပုံမှန်အင်တာနက်သည် nameservers များကိုရှာဖွေနိုင်သည်။

- အထွတ်/TLD ဖြတ်တောက်မှုများအတွက်၊ ALIAS/ANAME (ဝန်ဆောင်မှုပေးသူ-သီးသန့်) ကိုသုံးပါ သို့မဟုတ် A/AAAA ထုတ်ဝေပါ။
  gateway anycast IP များကိုညွှန်ပြသည့်မှတ်တမ်းများ။
- ဒိုမိန်းခွဲများအတွက်၊ ဆင်းသက်လာသော လှပသော host သို့ CNAME ကို ထုတ်ဝေပါ။
  (`<fqdn>.gw.sora.name`)။
- canonical host (`<hash>.gw.sora.id`) သည် gateway domain အောက်တွင်ရှိနေသည်
  သင်၏ အများသူငှာ ဇုန်အတွင်း မထုတ်ဝေပါ။

### Gateway ခေါင်းစီးပုံစံ

ဖြန့်ကျက်ကူညီသူသည် `portal.gateway.headers.txt` နှင့်လည်း ထုတ်လွှတ်သည်။
`portal.gateway.binding.json`၊ DG-3 ၏ စိတ်ကျေနပ်စေမည့် ပစ္စည်းနှစ်ခု
gateway-content-binding လိုအပ်ချက်-

- `portal.gateway.headers.txt` တွင် HTTP header block အပြည့်အစုံပါရှိသည် (အပါအဝင်
  `Sora-Name`၊ `Sora-Content-CID`၊ `Sora-Proof`၊ CSP၊ HSTS နှင့်
  `Sora-Route-Binding` ဖော်ပြချက်) အဆိုပါ အစွန်းထွက်ပေါက်များသည် နေရာတိုင်းတွင် ချည်နှောင်ရမည်၊
  တုံ့ပြန်မှု။
- `portal.gateway.binding.json` သည် စက်ဖြင့်ဖတ်နိုင်သော တူညီသောအချက်အလက်များကို မှတ်တမ်းတင်သည်။
  ဖောင်ပုံစံကြောင့် လက်မှတ်များကို ပြောင်းလဲခြင်းနှင့် အလိုအလျောက်စနစ်သည် host/cid binding မပါဘဲ ကွဲပြားနိုင်သည်။
  ခြစ်ခွံအထွက်။

သူတို့ကတဆင့်အလိုအလျောက်ထုတ်ပေးပါတယ်။
`cargo xtask soradns-binding-template`
နှင့် ပေးဆောင်ထားသည့် alias ၊ manifest digest နှင့် gateway hostname ကိုဖမ်းယူပါ။
`sorafs-pin-release.sh` သို့။ ခေါင်းစီးဘလောက်ကို ပြန်ထုတ်ရန် သို့မဟုတ် စိတ်ကြိုက်ပြင်ဆင်ရန်၊ လုပ်ဆောင်ရန်-

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

အစားထိုးရန် `--csp-template`၊ `--permissions-template` သို့မဟုတ် `--hsts-template` ကို ကျော်ဖြတ်ပါ
သီးခြားအသုံးချမှုတစ်ခု ထပ်လောင်းလိုအပ်သည့်အခါ မူရင်းခေါင်းစီး နမူနာပုံစံသည်
ညွှန်ကြားချက်များ; ခေါင်းစီးတစ်ခုချရန် ၎င်းတို့ကို လက်ရှိ `--no-*` ခလုတ်များနှင့် ပေါင်းစပ်ပါ။
လုံးဝ

ခေါင်းစီးအတိုအထွာကို CDN ပြောင်းလဲမှုတောင်းဆိုချက်တွင် ပူးတွဲပြီး JSON စာရွက်စာတမ်းကို ဖြည့်စွက်ပါ။
gateway automation pipeline ထဲသို့ အမှန်တကယ် host promotion နှင့် ကိုက်ညီပါသည်။
အထောက်အထားတွေ ထုတ်ပေးတယ်။

ထုတ်ဝေမှု script သည် အတည်ပြုခြင်းအထောက်အကူကို အလိုအလျောက်လုပ်ဆောင်ပေးသောကြောင့် DG-3 လက်မှတ်များ
မကြာသေးမီက အထောက်အထားများကို အမြဲထည့်သွင်းပါ။ ပြင်ဆင်ချိန်ညှိသည့်အခါတိုင်း ၎င်းကို ကိုယ်တိုင်ပြန်လုပ်ပါ။
JSON ကို လက်ဖြင့် ချိတ်ထားသည်-

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

binding bundle တွင်ဖမ်းယူထားသော `Sora-Proof` payload အား တရားဝင်စေသည်၊
`Sora-Route-Binding` မက်တာဒေတာသည် ထင်ရှားသော CID + လက်ခံသူအမည်နှင့် ကိုက်ညီကြောင်း သေချာစေသည်၊
ခေါင်းစီးတစ်ခုခု လွင့်သွားပါက လျှင်မြန်စွာ မအောင်မြင်ပါ။ ဘေးရှိ console output ကို သိမ်းဆည်းပါ။
သင် CI အပြင်ဘက် command ကို run သည့်အခါတိုင်း DG-3 သည် အခြားသော deployment artefact များဖြစ်သည်။
ဖြတ်တောက်ခြင်းမပြုမီ စည်းနှောင်မှုအား အတည်ပြုကြောင်း ပြန်လည်သုံးသပ်သူများတွင် အထောက်အထားရှိသည်။> **DNS သရုပ်ဖော်ပေါင်းစပ်မှု-** `portal.dns-cutover.json` သည် ယခု မြှုပ်နှံထားသည်
> `gateway_binding` အပိုင်းသည် ဤအရာများကို ညွှန်ပြသည် (လမ်းကြောင်းများ၊ အကြောင်းအရာ CID၊
> အထောက်အထားအခြေအနေနှင့် ပကတိခေါင်းစီးပုံစံ) **နှင့်** `route_plan` တစ်ပိုဒ်
> ရည်ညွှန်းခြင်း `gateway.route_plan.json` နှင့် ပင်မ + နောက်ပြန်ဆုတ်ခြင်း ခေါင်းစီး
> ပုံစံများ။ ဝေဖန်သုံးသပ်သူများ တတ်နိုင်စေရန် DG-3 ပြောင်းလဲမှု လက်မှတ်တိုင်းတွင် ထိုပိတ်ဆို့မှုများကို ထည့်သွင်းပါ။
> အတိအကျ `Sora-Name/Sora-Proof/CSP` တန်ဖိုးများကို ကွဲပြားစေပြီး လမ်းကြောင်းကို အတည်ပြုပါ။
> ပရိုမိုးရှင်း/ပြန်လှည့်ခြင်းအစီအစဉ်များသည် တည်ဆောက်မှုကို မဖွင့်ဘဲ အထောက်အထားအစုအဝေးနှင့် ကိုက်ညီသည်။
> တင်ထားပါတယ်။

## အဆင့် 9 — ထုတ်ဝေခြင်းမော်နီတာများကိုဖွင့်ပါ။

လမ်းပြမြေပုံလုပ်ဆောင်စရာ **DOCS-3c** ပေါ်တယ်ကို စဉ်ဆက်မပြတ် အထောက်အထား လိုအပ်သည်၊ စမ်းကြည့်ပါ။
proxy၊ နှင့် gateway binding များသည် ထုတ်လွှင့်ပြီးနောက် ကျန်းမာနေပါသည်။ ပေါင်းစုကို ပြေးပါ။
အဆင့် 7–8 ပြီးနောက် ချက်ချင်းစောင့်ကြည့်ပြီး သင်၏စီစဉ်ထားသော ပရောဖက်များထဲသို့ ကြိုးဖြင့်သွယ်တန်းလိုက်ပါ-

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` သည် config ဖိုင်ကိုတင်သည် (ကြည့်ပါ။
  schema အတွက် `docs/portal/docs/devportal/publishing-monitoring.md`) နှင့်
  စစ်ဆေးချက်သုံးခုကို လုပ်ဆောင်သည်- portal path probes + CSP/Permissions-Policy validation၊
  ၎င်းကို proxy probes စမ်းကြည့်ပါ (၎င်း၏ `/metrics` endpoint ကို စိတ်ကြိုက်ရွေးချယ်နိုင်သည်) နှင့်
  စစ်ဆေးပေးသော gateway binding verifier (`cargo xtask soradns-verify-binding`)
  မျှော်လင့်ထားသည့် နံပတ်၊ အိမ်ရှင်၊ အထောက်အထားအခြေအနေ၊
  နှင့် JSON ကိုဖော်ပြပါ။
- မည်သည့် probe တစ်ခုမှပျက်ကွက်သည့်အခါတိုင်း command သည် CI, cron jobs, သို့မဟုတ်
  runbook အော်ပရေတာများသည် aliases ကိုမမြှင့်တင်မီထုတ်ဝေမှုကိုရပ်တန့်နိုင်သည်။
- `--json-out` ကို ကျော်ဖြတ်ခြင်းသည် တစ်ခုတည်းသော အကျဉ်းချုပ် JSON ပေးချေမှုအား ပစ်မှတ်တစ်ခုဖြင့် ရေးသားသည်
  အဆင့်အတန်း; `--evidence-dir` သည် `summary.json`၊ `portal.json`၊ `tryit.json`၊
  `binding.json` နှင့် `checksums.sha256` ထို့ကြောင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများသည် ကွဲပြားနိုင်သည်။
  မော်နီတာများကို ပြန်မလည်ပတ်ဘဲ ရလဒ်များ။ ဤလမ်းညွှန်ကို အောက်တွင် သိမ်းဆည်းပါ။
  `artifacts/sorafs/<tag>/monitoring/` သည် Sigstore အတွဲလိုက် နှင့် DNS
  cutover ဖော်ပြချက်။
- မော်နီတာအထွက်ကိုထည့်သွင်းပါ၊ Grafana တင်ပို့မှု (`dashboards/grafana/docs_portal.json`)၊
  နှင့် DOCS-3c SLO သည် ထုတ်ဝေခွင့်လက်မှတ်ရှိ Alertmanager drill ID ဖြစ်သည်
  နောက်မှ စာရင်းစစ်တယ်။ သီးသန့်ထုတ်ဝေခြင်းဆိုင်ရာ မော်နီတာဖွင့်စာအုပ်မှာ နေထိုင်သည်။
  `docs/portal/docs/devportal/publishing-monitoring.md`။

Portal probe များသည် HTTPS လိုအပ်ပြီး `http://` အခြေစိုက် URL များကို ငြင်းပယ်ခြင်းမရှိပါက၊
`allowInsecureHttp` ကို monitor config တွင်သတ်မှတ်ထားသည်။ ထုတ်လုပ်မှု / ဇာတ်ညွှန်းထိန်းသိမ်းပါ။
TLS ပေါ်ရှိ ပစ်မှတ်များ နှင့် ဒေသန္တရ အကြိုကြည့်ရှုခြင်းများအတွက် အစားထိုးမှုကိုသာ ဖွင့်ပါ။

Buildkite/cron တွင် `npm run monitor:publishing` မှတစ်ဆင့် မော်နီတာအား အလိုအလျောက်လုပ်ပါ။
portal သည် တိုက်ရိုက်လွှင့်နေသည်။ ထုတ်လုပ်မှု URLs များတွင် ညွှန်ပြထားသည့် တူညီသောအမိန့်သည် လက်ရှိလုပ်ဆောင်နေသည့်အရာကို ကျွေးမွေးသည်။
ထုတ်ဝေမှုများကြားတွင် SRE/Docs အားကိုးသည့် ကျန်းမာရေးစစ်ဆေးမှုများ။

## `sorafs-pin-release.sh` ဖြင့် အလိုအလျောက်လုပ်ဆောင်ခြင်း။

`docs/portal/scripts/sorafs-pin-release.sh` သည် အဆင့် 2-6 ကို ဖုံးအုပ်ထားသည်။ ၎င်း-

1. မော်ကွန်းတိုက် `build/` အား အဆုံးအဖြတ်ပေးသော တာဘောတစ်ခုအဖြစ်၊
2. လုပ်ဆောင်သည် `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`၊
   နှင့် `proof verify`၊
3. Torii သောအခါတွင် `manifest submit` (alias binding အပါအဝင်) ကို ရွေးချယ်နိုင်သည်
   အထောက်အထားများ ရှိနေသည်၊ နှင့်
4. `artifacts/sorafs/portal.pin.report.json` တွင် ရွေးချယ်နိုင်သည်ဟု ရေးသည်။
  `portal.pin.proposal.json`၊ DNS ဖြတ်တောက်မှုဖော်ပြချက် (တင်ပြမှုများပြီးနောက်)၊
  နှင့် gateway binding bundle (`portal.gateway.binding.json` နှင့်
  စာသားခေါင်းစီးပိတ်ဆို့ခြင်း) ထို့ကြောင့် အုပ်ချုပ်မှု၊ ကွန်ရက်ချိတ်ဆက်မှုနှင့် ops အဖွဲ့များသည် ကွဲပြားနိုင်သည်။
  CI မှတ်တမ်းများကို မခြစ်ဘဲ အထောက်အထားအစုအဝေး။

`PIN_ALIAS`၊ `PIN_ALIAS_NAMESPACE`၊ `PIN_ALIAS_NAME` နှင့် (ရွေးချယ်နိုင်သည်)
ဇာတ်ညွှန်းကို မခေါ်မီ `PIN_ALIAS_PROOF_PATH`။ အခြောက်အတွက် `--skip-submit` ကိုသုံးပါ။
ပြေး; အောက်တွင်ဖော်ပြထားသော GitHub အလုပ်အသွားအလာသည် `perform_submit` မှတဆင့် ၎င်းကို ပြောင်းပေးသည်
ထည့်သွင်းမှု။

## အဆင့် 8 — OpenAPI သတ်မှတ်ချက်များနှင့် SBOM အတွဲများကို ထုတ်ဝေပါ

DOCS-7 သည် ခရီးသွားရန်အတွက် portal build၊ OpenAPI spec နှင့် SBOM artefacts လိုအပ်သည်
တူညီသော အဆုံးအဖြတ်ပေးသော ပိုက်လိုင်းမှတဆင့်။ ရှိပြီးသား အကူအညီပေးသူများသည် သုံးခုလုံးကို အကျုံးဝင်သည်-

1. ** သတ်မှတ်ချက်ကို ပြန်ထုတ်ပြီး လက်မှတ်ထိုးပါ။**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   သင်ထိန်းသိမ်းလိုသည့်အခါတိုင်း `--version=<label>` မှတစ်ဆင့် ထုတ်လွှတ်သည့်တံဆိပ်ကို ဖြတ်ပါ။
   သမိုင်းဝင် လျှပ်တစ်ပြက် (ဥပမာ `2025-q3`)။ အကူအညီပေးသူက လျှပ်တစ်ပြက်ကို ရေးသည်။
   `static/openapi/versions/<label>/torii.json` သို့ ကြေးမုံပြင်ပါ။
   `versions/current` နှင့် metadata ကို မှတ်တမ်းတင်သည် (SHA-256၊ ထင်ရှားသော အခြေအနေနှင့် နှင့်
   `static/openapi/versions.json` တွင် အပ်ဒိတ်လုပ်ထားသော အချိန်တံဆိပ်တုံး။ developer ပေါ်တယ်။
   Swagger/RapiDoc အကန့်များသည် ဗားရှင်းရွေးချယ်မှုတစ်ခုကို တင်ပြနိုင်စေရန် ထိုအညွှန်းကိုဖတ်သည်။
   နှင့် ဆက်စပ်သော အနှစ်ချုပ်/ လက်မှတ် အချက်အလက်ကို လိုင်းတွင် ပြသပါ။ ချန်လှပ်ခြင်း။
   `--version` သည် ယခင်ထွက်ရှိထားသော အညွှန်းများကို နဂိုအတိုင်းထားရှိပြီး ပြန်လည်ဆန်းသစ်စေသည်
   `current` + `latest` လို့ မြင်ပါတယ်။

   မန်နီးဖက်စ်သည် SHA-256/BLAKE3 ၏ အချေအတင်များကို ဖမ်းယူထားသောကြောင့် တံခါးပေါက်သည် ရိုးစင်းနိုင်သည်
   `/reference/torii-swagger` အတွက် `Sora-Proof` ခေါင်းစီးများ။

2. **CycloneDX SBOMs များကို ထုတ်လွှတ်သည်။** ပိုက်လိုင်းသည် syft-based ကို မျှော်လင့်ထားပြီးသား၊
   `docs/source/sorafs_release_pipeline_plan.md` အတွက် SBOMs အထွက်ကို ထိန်းထားပါ။
   တည်ဆောက်ထားသော ပစ္စည်းများ၏ ဘေးတွင်၊

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **ဝန်ထုပ်ဝန်ပိုးတစ်ခုစီကို ကားတစ်စီးတွင် ထုပ်ပိုးပါ။**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   ပင်မဆိုက်အဖြစ် တူညီသော `manifest build` / `manifest sign` အဆင့်များကို လိုက်နာပါ၊
   ပိုင်ဆိုင်မှုတစ်ခုစီ၏ နာမည်တူများကို ချိန်ညှိခြင်း (ဥပမာ၊ spec အတွက် `docs-openapi.sora` နှင့်
   လက်မှတ်ထိုးထားသော SBOM အတွဲအတွက် `docs-sbom.sora`)။ ကွဲပြားသောအမည်များကို ထိန်းသိမ်းခြင်း။
   SoraDNS အထောက်အထားများ၊ GAR များနှင့် အပြန်ပြန်အမ်းငွေလက်မှတ်များကို အတိအကျ payload တွင် ထားရှိထားပါသည်။

4. **တင်ပြပြီး စည်းပါ။** ရှိပြီးသား အခွင့်အာဏာ + Sigstore အတွဲကို ပြန်သုံးပါ၊
   စာရင်းစစ်များသည် မည်သည့်အရာကို ခြေရာခံနိုင်စေရန် ထုတ်ဝေမှုစစ်ဆေးစာရင်းတွင် alias tuple ကို မှတ်တမ်းတင်ပါ။
   Sora အမည် မြေပုံများကို ချေဖျက်ပေးသည်။

ပေါ်တယ်တည်ဆောက်မှုနှင့်အတူ spec/SBOM ဖော်ပြချက်များကို သိမ်းဆည်းခြင်းသည် တစ်ခုချင်းစီကို သေချာစေသည်။
ထုတ်ပေးသည့် လက်မှတ်တွင် ထုပ်ပိုးခြင်းကို ပြန်မလုပ်ဆောင်ဘဲ အစုံအလင် ပါဝင်ပါသည်။

### အော်တိုမက်တစ် အကူအညီပေးသူ (CI/package script)

`./ci/package_docs_portal_sorafs.sh` သည် အဆင့် 1-8 ဖြစ်သောကြောင့် လမ်းပြမြေပုံပါ အကြောင်းအရာ
**DOCS-7** ကို အမိန့်တစ်ခုတည်းဖြင့် ကျင့်သုံးနိုင်သည်။ ကူညီသူ-

- လိုအပ်သော portal ကြိုတင်ပြင်ဆင်မှု (`npm ci`၊ OpenAPI/norito ထပ်တူပြုခြင်း၊ ဝစ်ဂျက်စမ်းသပ်မှုများ)
- ပေါ်တယ်၊ OpenAPI နှင့် SBOM CARs + `sorafs_cli` မှတဆင့် ထင်ရှားသောအတွဲများကို ထုတ်လွှတ်သည်။
- ရွေးချယ်နိုင်သည် `sorafs_cli proof verify` (`--proof`) နှင့် Sigstore လက်မှတ်ရေးထိုးခြင်း
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- ပစ္စည်းတိုင်းကို `artifacts/devportal/sorafs/<timestamp>/` နှင့် အောက်တွင် ချပေးပါ။
  `package_summary.json` ရေးထားသောကြောင့် CI/release tooling သည် အစုအဝေးကို ထည့်သွင်းနိုင်သည်။ နှင့်
- နောက်ဆုံးထွက်ပြေးခြင်းကိုညွှန်ပြရန် `artifacts/devportal/sorafs/latest` ကို ပြန်လည်စတင်ပါ။

ဥပမာ (Sigstore + PoR ဖြင့် ပိုက်လိုင်း အပြည့်အစုံ)။

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

သိသင့်သိထိုက်သော အလံများ

- `--out <dir>` - artefact root ကို အစားထိုးပါ (မူရင်းက အချိန်တံဆိပ်နှိပ်ထားသော ဖိုင်တွဲများကို သိမ်းဆည်းသည်)။
- `--skip-build` - ရှိပြီးသား `docs/portal/build` ကို ပြန်သုံးပါ (CI မရနိုင်သောအခါ အဆင်ပြေသည်
  အော့ဖ်လိုင်းကြည့်မှန်များကြောင့် ပြန်လည်တည်ဆောက်ပါ။)
- `--skip-sync-openapi` – `cargo xtask openapi` သောအခါ `npm run sync-openapi` ကျော်သွားသည်
  crates.io မရောက်နိုင်ပါ။
- `--skip-sbom` - binary ကိုထည့်သွင်းမထားသောအခါ `syft` ကိုခေါ်ဆိုခြင်းမှရှောင်ကြဉ်ပါ (the
  script အစား သတိပေးချက်ကို print ထုတ်ပါသည်။)
- `--proof` – CAR/မန်နီးဖက်စ်အတွဲတစ်ခုစီအတွက် `sorafs_cli proof verify` ကိုဖွင့်ပါ။ Multi-
  ဖိုင်ပေးချေမှုများသည် CLI တွင် အတုံးလိုက်အစီအစဥ်ပံ့ပိုးမှု လိုအပ်နေသေးသောကြောင့် ဤအလံကို ထားခဲ့ပါ။
  `plan chunk count` အမှားအယွင်းများကို နှိပ်ပြီး ကိုယ်တိုင်စစ်ဆေးပါက သတ်မှတ်မထားပါ။
  ရေဆန်တံခါးမြေများ။
- `--sign` – `sorafs_cli manifest sign` ကိုခေါ်ပါ။ တိုကင်တစ်ခုပေးပါ။
  `SIGSTORE_ID_TOKEN` (သို့မဟုတ် `--sigstore-token-env`) သို့မဟုတ် CLI ကို အသုံးပြု၍ ၎င်းကို ရယူခွင့်ပြုပါ
  `--sigstore-provider/--sigstore-audience`။

တင်ပို့သည့်အခါတွင် ထုတ်လုပ်သည့် ပစ္စည်းများကို `docs/portal/scripts/sorafs-pin-release.sh` ကို အသုံးပြုပါ။
ယခု ၎င်းသည် portal၊ OpenAPI နှင့် SBOM payloads များကို ထုပ်ပိုးထားပြီး၊ manifest တစ်ခုစီကို သင်္ကေတပြုကာ၊
`portal.additional_assets.json` တွင် အပိုပိုင်ဆိုင်မှု မက်တာဒေတာကို မှတ်တမ်းတင်သည်။ အထောက်အမ
CI ထုပ်ပိုးမှုအပြင် အသစ်အသုံးပြုသည့် တူညီသောရွေးချယ်နိုင်သောခလုတ်များကို နားလည်သည်။
`--openapi-*`၊ `--portal-sbom-*` နှင့် `--openapi-sbom-*` ခလုတ်များကို သင်လုပ်နိုင်သည်
artefact တစ်ခုလျှင် alias tuples များသတ်မှတ်ပေးသည်၊ SBOM အရင်းအမြစ်ကို ကျော်ပါ။
`--openapi-sbom-source`၊ အချို့သော payloads ကိုကျော်ပါ (`--skip-openapi`/`--skip-sbom`)၊
ပြီးလျှင် `--syft-bin` ဖြင့် ပုံသေမဟုတ်သော `syft` binary ကို ညွှန်ပါ။

script သည် ၎င်းလုပ်ဆောင်သည့် command တိုင်းကို ဖော်ပြသည်။ ထွက်ခွာခွင့်လက်မှတ်ထဲသို့ မှတ်တမ်းကို ကူးယူပါ။
`package_summary.json` နှင့်အတူ သုံးသပ်သူများသည် CAR ၏ အနှစ်သာရများကို ကွဲပြားစေပြီး အစီအစဉ်ဆွဲနိုင်သည်
မက်တာဒေတာ၊ နှင့် Sigstore အစုအဝေးသည် ad-hoc shell အထွက်ကို စာလုံးပေါင်းမဖော်ဘဲ hash များ။

## အဆင့် 9 — Gateway + SoraDNS အတည်ပြုခြင်း။

ဖြတ်တောက်ခြင်းအား မကြေငြာမီ၊ SoraDNS မှတစ်ဆင့် ခွဲခြမ်းစိတ်ဖြာမှုအသစ်ကို သက်သေပြပါ။
တံခါးပေါက်များ အဓိက လတ်ဆတ်သော အထောက်အထားများ

1. ** probe gate ကို run ပါ။** `ci/check_sorafs_gateway_probe.sh` လေ့ကျင့်ခန်းများ
   `cargo xtask sorafs-gateway-probe` တွင် ဒီမိုပွဲများနှင့် ဆန့်ကျင်ဘက်
   `fixtures/sorafs_gateway/probe_demo/`။ စစ်မှန်သော ဖြန့်ကျက်မှုအတွက်၊ စုံစမ်းစစ်ဆေးမှုကို ထောက်ပြပါ။
   ပစ်မှတ် hostname မှာ-

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   စုံစမ်းစစ်ဆေးမှုတွင် `Sora-Name`၊ `Sora-Proof` နှင့် `Sora-Proof-Status` နှုန်းဖြင့် ကုဒ်လုပ်သည်
   `docs/source/sorafs_alias_policy.md` နှင့် manifest ကို ချေဖျက်သောအခါ ကျရှုံးသည်၊
   TTLs သို့မဟုတ် GAR bindings များ ပျံ့လွင့်နေသည်။

   ပေါ့ပါးသောအစက်အပြောက်စစ်ဆေးမှုများအတွက် (ဥပမာ၊ စည်းနှောင်ထားသောအစုအဝေးသာရှိသည့်အခါ
   ပြောင်းလိုက်သည်) `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` ကို run ပါ။
   အကူအညီပေးသူက ဖမ်းယူထားသော ချည်နှောင်ထားသောအတွဲကို သက်သေပြပြီး လွှတ်ပေးရန် အဆင်ပြေသည်။
   စစ်ဆေးမှုအပြည့်အစုံဖြင့် စစ်ဆေးခြင်းအစား ချိတ်ဆက်အတည်ပြုချက်သာလိုအပ်သည့် လက်မှတ်များ။

2. **drill အထောက်အထားများကို ဖမ်းယူပါ။** အော်ပရေတာ လေ့ကျင့်ခန်းများ သို့မဟုတ် PagerDuty ခြောက်သွေ့သော ပြေးခြင်းအတွက်၊
   `scripts/telemetry/run_sorafs_gateway_probe.sh ပါသော စုံစမ်းစစ်ဆေးမှု --scenario
   devportal-rollout -- ...`။ wrapper သည် ခေါင်းစီး/မှတ်တမ်းများကို အောက်တွင် သိမ်းဆည်းထားသည်။
   `artifacts/sorafs_gateway_probe/<stamp>/`၊ အပ်ဒိတ် `ops/drill-log.md` နှင့်
   (ရွေးချယ်နိုင်သည်) rollback ချိတ်များ သို့မဟုတ် PagerDuty payload များကို အစပျိုးပေးသည်။ သတ်မှတ်
   `--host docs.sora` ကို hard-coding IP အစား SoraDNS လမ်းကြောင်းကို အတည်ပြုရန်။3. ** DNS bindings များကို အတည်ပြုပါ။** အုပ်ချုပ်ရေးမှ alias အထောက်အထားကို ထုတ်ဝေသည့်အခါ မှတ်တမ်းတင်ပါ၊
   စုံစမ်းစစ်ဆေးမှုတွင် ကိုးကားထားသော GAR ဖိုင် (`--gar`) နှင့် ၎င်းကို ထုတ်ဝေမှုတွင် ပူးတွဲပါ
   အထောက်အထား။ Resolver ပိုင်ရှင်များသည် တူညီသောထည့်သွင်းမှုကို ကူးယူနိုင်သည်။
   ကက်ရှ်လုပ်ထားသည့်အရာများသည် မန်နီးဖက်စ်အသစ်ကို ဂုဏ်ပြုကြောင်းသေချာစေရန် `tools/soradns-resolver`။
   JSON ကို မတွဲမီ၊ လုပ်ဆောင်ပါ။
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   ထို့ကြောင့် အဆုံးအဖြတ်ပေးသော host မြေပုံ၊ manifest metadata နှင့် telemetry အညွှန်းများဖြစ်သည်။
   အော့ဖ်လိုင်းဖြင့် အတည်ပြုထားသည်။ အကူအညီပေးသူက `--json-out` အကျဉ်းချုပ်ကို ထုတ်လွှတ်နိုင်သည်။
   GAR ကို လက်မှတ်ရေးထိုးထားသောကြောင့် ပြန်လည်သုံးသပ်သူများသည် binary ကိုမဖွင့်ဘဲ အတည်ပြုနိုင်သော အထောက်အထားများရှိသည်။
  GAR အသစ်တစ်ခုရေးဆွဲတဲ့အခါ ပိုကြိုက်တယ်။
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (မန်နီးဖက်စ်ဖိုင်တစ်ခုမဟုတ်သောအခါမှသာ `--manifest-cid <cid>` သို့ ပြန်သွားပါ။
  ရနိုင်သည်)။ ကူညီသူသည် ယခု CID **and** BLAKE3 မှ တိုက်ရိုက် ထုတ်ယူရရှိသည်
  မန်နီးဖက်စ် JSON၊ နေရာလွတ်ကို ချုံ့လိုက်သည်၊ ထပ်ကာထပ်ကာ ထပ်ကာထပ်ကာ `--telemetry-label`
  အလံများ၊ အညွှန်းများကို စီရန်နှင့် မူရင်း CSP/HSTS/Permissions-Policy ကို ထုတ်လွှတ်သည်
  JSON ကိုမရေးမီ တင်းပလိတ်များသည် payload သည် မည်သည့်အချိန်တွင်ပင် အဆုံးအဖြတ်ရှိစေပါသည်။
  အော်ပရေတာများသည် မတူညီသော အခွံများမှ အညွှန်းများကို ဖမ်းယူသည်။

4. **အတိုကောက် မက်ထရစ်များကို ကြည့်ရှုပါ။** `torii_sorafs_alias_cache_refresh_duration_ms` ကို သိမ်းဆည်းပါ
   နှင့် `torii_sorafs_gateway_refusals_total{profile="docs"}` သည် စခရင်ပေါ်တွင် ရှိနေစဉ်
   စုံစမ်းစစ်ဆေးမှု လုပ်ဆောင်နေပါသည်။ စီးရီးနှစ်ခုစလုံးကိုဇယားကွက်တွင်ဖော်ပြထားသည်။
   `dashboards/grafana/docs_portal.json`။

## အဆင့် 10 — စောင့်ကြည့်ခြင်းနှင့် အထောက်အထားများ စုစည်းခြင်း။

- ** ဒက်ရှ်ဘုတ်များ။** `dashboards/grafana/docs_portal.json` (ပေါ်တယ် SLO များ) ကို ထုတ်ယူရန်၊
  `dashboards/grafana/sorafs_gateway_observability.json` (gateway latency +
  အထောက်အထားကျန်းမာရေး) နှင့် `dashboards/grafana/sorafs_fetch_observability.json`
  ထုတ်ဝေမှုတိုင်းအတွက် (သံစုံတီးဝိုင်းကျန်းမာရေး)။ JSON ထုတ်ယူမှုများကို the နှင့် ပူးတွဲပါ။
  သုံးသပ်သူများသည် Prometheus မေးခွန်းများကို ပြန်ဖွင့်နိုင်စေရန် လက်မှတ်ထုတ်ပေးသည်။
- **Probe archives** `artifacts/sorafs_gateway_probe/<stamp>/` ကို git-annex တွင် သိမ်းထားပါ
  သို့မဟုတ် သင့်အထောက်အထားပုံး။ စုံစမ်းစစ်ဆေးမှုအနှစ်ချုပ်၊ ခေါင်းစီးများနှင့် PagerDuty တို့ကို ထည့်သွင်းပါ။
  telemetry script မှဖမ်းယူထားသော payload။
- **အစုအဝေးကို ထုတ်ဝေပါ။** ပေါ်တယ်/SBOM/OpenAPI CAR အနှစ်ချုပ်များ၊ မန်နီးဖက်စ်ကို သိမ်းဆည်းပါ
  အစုအဝေးများ၊ Sigstore လက်မှတ်များ၊ `portal.pin.report.json`၊ Try-It probe မှတ်တမ်းများနှင့်
  တစ်ခုတည်းသောအချိန်တံဆိပ်ရိုက်ထားသောဖိုင်တွဲအောက်တွင် link-check အစီရင်ခံစာများ (ဥပမာ၊
  `artifacts/sorafs/devportal/20260212T1103Z/`)။
- **Drill log.** probes များသည် drill တစ်ခု၏ အစိတ်အပိုင်းဖြစ်သောအခါ၊
  `scripts/telemetry/run_sorafs_gateway_probe.sh` ကို `ops/drill-log.md` သို့ ပေါင်းထည့်သည်
  ထို့ကြောင့် တူညီသောအထောက်အထားသည် SNNet-5 ပရမ်းပတာလိုအပ်ချက်ကို ကျေနပ်စေသည်။
- **လက်မှတ်လင့်ခ်များ။** Grafana panel IDs သို့မဟုတ် ပူးတွဲပါ PNG ထုတ်ယူမှုများကို ကိုးကားပါ။
  ပြောင်းလဲမှု လက်မှတ်၊ စုံစမ်းစစ်ဆေးရေး အစီရင်ခံစာ လမ်းကြောင်း နှင့် တွဲနေသောကြောင့် ပြောင်းလဲ သုံးသပ်သူများ
  shell access မပါဘဲ SLO များကို အပြန်အလှန်စစ်ဆေးနိုင်သည်။

## အဆင့် 11 — Multi-source အကျုံးဝင်သော လေ့ကျင့်မှုနှင့် အမှတ်စာရင်း အထောက်အထားများ

SoraFS သို့ ထုတ်ဝေခြင်းသည် ယခုအခါ ရင်းမြစ်များစွာ ထုတ်ယူမှု အထောက်အထား လိုအပ်သည် (DOCS-7/SF-6)
အထက်ပါ DNS/gateway အထောက်အထားများနှင့်အတူ။ မန်နီးဖက်စ်ကို ပင်ထိုးပြီးနောက်-

1. **`sorafs_fetch` ကို တိုက်ရိုက်ဖော်ပြခြင်းအား လုပ်ဆောင်ပါ။** တူညီသော အစီအစဉ်/မန်နီးဖက်စ်ကို အသုံးပြုပါ။
   အဆင့် 2–3 တွင် ထုတ်လုပ်ထားသော ပစ္စည်းများနှင့် တစ်ခုစီအတွက် ထုတ်ပေးသည့် ဂိတ်ဝေးအထောက်အထားများ
   ပံ့ပိုးပေးသူ။ စာရင်းစစ်များသည် သံစုံတီးဝိုင်းကို ပြန်လည်တီးခတ်နိုင်စေရန် အထွက်တိုင်းကို ဆက်လက်လုပ်ဆောင်ပါ။
   ဆုံးဖြတ်ချက်လမ်းကြောင်း

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - မန်နီးဖက်စ်မှ ကိုးကားထားသော ဝန်ဆောင်မှုပေးသူ ကြော်ငြာများကို ဦးစွာ ရယူပါ (ဥပမာ
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     နှင့် `--provider-advert name=path` မှတဆင့် ၎င်းတို့ကို ကျော်ဖြတ်နိုင်စေရန် အမှတ်စာရင်း တင်သွင်းနိုင်သည်။
     စွမ်းရည်ပြတင်းပေါက်များကို အဆုံးအဖြတ်ဖြင့် အကဲဖြတ်ပါ။ သုံးပါ။
     ပွဲစဉ်များကို ပြန်လည်ကစားသည့်အခါ `--allow-implicit-provider-metadata` **သာ**
     CI; ထုတ်လုပ်ရေးလေ့ကျင့်မှုများတွင် လက်မှတ်ရေးထိုးထားသည့် ကြော်ငြာများကို ကိုးကား၍ ဖော်ပြရပါမည်။
     pin
   - မန်နီးဖက်စ်သည် အပိုဒေသများကို ကိုးကားသောအခါ၊ အမိန့်ဖြင့် ပြန်လုပ်ပါ။
     သက်ဆိုင်ရာ ပံ့ပိုးပေးသူသည် tuples ဖြစ်သောကြောင့် cache/alias တိုင်းတွင် ကိုက်ညီမှုရှိသည်
     ပစ္စည်းကိုရယူပါ။

2. **အထွက်များကို သိမ်းဆည်းပါ။** Store `scoreboard.json`၊
   `providers.ndjson`၊ `fetch.json` နှင့် `chunk_receipts.ndjson` အောက်တွင်
   အထောက်အထား ဖိုင်တွဲကို လွှတ်ပေးပါ။ ဤဖိုင်များသည် သက်တူရွယ်တူ တွက်ဆမှုကို ဖမ်းယူ၍ ထပ်စမ်းကြည့်ပါ။
   ဘတ်ဂျက်၊ ကြာမြင့်ချိန် EWMA နှင့် အုပ်ချုပ်မှုပက်ကေ့ချ်တွင် လိုအပ်သော ဖြတ်ပိုင်းတစ်ခုစီ၊
   SF-7 ကို ထိန်းသိမ်းပါ။

3. **တယ်လီမီတာကို အပ်ဒိတ်လုပ်ပါ။** ထုတ်ယူထားသော ရလဒ်များကို **SoraFS Fetch သို့ တင်သွင်းပါ။
   မြင်နိုင်စွမ်း** ဒက်ရှ်ဘုတ် (`dashboards/grafana/sorafs_fetch_observability.json`)၊
   `torii_sorafs_fetch_duration_ms`/`_failures_total` ကိုကြည့်ရှုခြင်းနှင့်
   ကွဲလွဲချက်များများအတွက် ဝန်ဆောင်မှုပေးသူအပိုင်းအခြားအကန့်များ။ Grafana အကန့်ကို လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် ချိတ်ဆက်ပါ။
   အမှတ်စာရင်း လမ်းကြောင်း တစ်လျှောက် လက်မှတ်ကို ထုတ်ပါ။

4. **သတိပေးချက်စည်းမျဉ်းများကို ဆေးလိပ်သောက်ပါ။** `scripts/telemetry/test_sorafs_fetch_alerts.sh` ကိုဖွင့်ပါ။
   ထုတ်ဝေမှုကို မပိတ်မီ Prometheus သတိပေးချက်အတွဲကို အတည်ပြုရန်။ တွဲပါ။
   DOCS-7 ပြန်လည်သုံးသပ်သူများသည် အရောင်းဆိုင်ကို အတည်ပြုနိုင်စေရန် လက်မှတ်သို့ promtool ထုတ်ပေးပါသည်။
   နှင့် နှေးကွေးသော ဝန်ဆောင်မှုပေးသည့် သတိပေးချက်များကို လက်နက်ကိုင်ဆောင်ထားဆဲဖြစ်သည်။

5. ** CI သို့ Wire ။** portal pin အလုပ်အသွားအလာသည် `sorafs_fetch` ကို နောက်သို့ ဆုတ်သွားစေသည်။
   `perform_fetch_probe` ထည့်သွင်းမှု; ဇာတ်ညွှန်း/ထုတ်လုပ်ရေး လုပ်ဆောင်ရန်အတွက် ၎င်းကို ဖွင့်ပါ။
   သက်သေခံအထောက်အထားများကို လက်စွဲမပါဘဲ ထင်ရှားသောအစုအဝေးနှင့်အတူ ထုတ်လုပ်သည်။
   ဝင်ရောက်စွက်ဖက်ခြင်း။ Local drills များသည် တူညီသော script ကို ထုတ်ယူခြင်းဖြင့် ပြန်လည်အသုံးပြုနိုင်ပါသည်။
   gateway တိုကင်များနှင့် `PIN_FETCH_PROVIDERS` ကို ကော်မာ-ခြားထားသောအဖြစ် သတ်မှတ်ခြင်း
   ပံ့ပိုးသူစာရင်း။

## ပရိုမိုးရှင်း၊ ကြည့်ရှုနိုင်မှု၊ နှင့် နောက်ပြန်ဆွဲခြင်း။

1. **ပရိုမိုးရှင်း-** သီးခြားဇာတ်ခုံနှင့် ထုတ်လုပ်ရေးအမည်တူများကို ထားရှိပါ။ မြှင့်တင်ပါ။
   တူညီသော manifest/အစုအဝေးဖြင့် `manifest submit` ကို ပြန်လည်လည်ပတ်ခြင်း၊ လဲလှယ်ခြင်း
   ထုတ်လုပ်မှုအမည်ကို ညွှန်ပြရန် `--alias-namespace/--alias-name`။ ဒီ
   QA သည် ဇာတ်ညွှန်းပင်ကို အတည်ပြုပြီးသည်နှင့် ပြန်လည်တည်ဆောက်ခြင်း သို့မဟုတ် နုတ်ထွက်ခြင်းကို ရှောင်ကြဉ်သည်။
2. **စောင့်ကြည့်ခြင်း-** pin-registry dashboard ကို တင်သွင်းပါ။
   (`docs/source/grafana_sorafs_pin_registry.json`) နှင့် ပေါ်တယ်-အတိအကျ
   probes (`docs/portal/docs/devportal/observability.md` ကိုကြည့်ပါ)။ checksum တွင်သတိပေးချက်
   ပျံ့လွင့်ခြင်း၊ မအောင်မြင်သော စုံစမ်းစစ်ဆေးမှုများ သို့မဟုတ် ထပ်စမ်းကြည့်ခြင်း အထောက်အထားများ။
3. **ပြန်လှည့်ရန်-** ပြန်ပြောင်းရန်၊ ယခင်မန်နီးဖက်စ်ကို ပြန်လည်တင်ပြရန် (သို့မဟုတ် အနားယူပါ။
   `sorafs_cli manifest submit --alias ... --retire` ကိုအသုံးပြုနေသည့် လက်ရှိအမည်များ)။
   နောက်ဆုံးထွက်ရှိထားသော အတွဲလိုက်နှင့် CAR အနှစ်ချုပ်ကို အမြဲသိမ်းဆည်းထားပါ၊ သို့မှသာ နောက်ကြောင်းပြန်အထောက်အထားများ ရရှိနိုင်မည်ဖြစ်သည်။
   CI မှတ်တမ်းများ လှည့်ပါက ပြန်လည်ဖန်တီးပါ။

## CI အလုပ်အသွားအလာ နမူနာပုံစံ

အနည်းဆုံး၊ သင့်ပိုက်လိုင်းသည်-

1. Build + lint (`npm ci`၊ `npm run build`၊ checksum မျိုးဆက်)။
2. Package (`car pack`) နှင့် compute manifests ။
3. အလုပ်ကန့်သတ်ထားသော OIDC တိုကင် (`manifest sign`) ကို အသုံးပြု၍ လက်မှတ်ထိုးပါ။
4. စစ်ဆေးခြင်းအတွက် အနုပညာပစ္စည်းများ (CAR၊ သရုပ်ဖော်မှု၊ အစုအဝေး၊ အစီအစဉ်၊ အနှစ်ချုပ်) ကို အပ်လုဒ်လုပ်ပါ။
5. Pin registry သို့ တင်သွင်းပါ-
   - တောင်းဆိုချက်များ → `docs-preview.sora` ကို ဆွဲထုတ်ပါ။
   - တံဆိပ်များ / ကာကွယ်ထားသော အကိုင်းအခက်များ → ထုတ်လုပ်မှုအမည်များ မြှင့်တင်ရေး။
6. မထွက်ခင် probes + proof verification gates ကို run ပါ။

`.github/workflows/docs-portal-sorafs-pin.yml` သည် ဤအဆင့်များအားလုံးကို အတူတကွ ကြိုးပေးသည်။
လက်စွဲစာအုပ်များအတွက်။ အလုပ်အသွားအလာ-

- ပေါ်တယ်ကိုတည်ဆောက်ခြင်း / စမ်းသပ်ခြင်း၊
- `scripts/sorafs-pin-release.sh` မှတစ်ဆင့် တည်ဆောက်မှုကို ပက်ကေ့ဂျ်များ၊
- GitHub OIDC ကို အသုံးပြု၍ ထင်ရှားသောအစုအဝေးကို ဆိုင်းဘုတ်များ/အတည်ပြုခြင်း
- CAR/manifest/bundle/plan/proof summaries ကို artifacts အဖြစ် အပ်လုဒ်လုပ်သည်၊
- လျှို့ဝှက်ချက်များရှိနေသောအခါ (ရွေးချယ်နိုင်သည်) သည် ထင်ရှားသော + အမည်နာမကို စည်းနှောင်ခြင်းကို တင်သွင်းသည်။

အလုပ်မစတင်မီ အောက်ပါသိုလှောင်မှုလျှို့ဝှက်ချက်များ/ကိန်းရှင်များကို ပြင်ဆင်သတ်မှတ်ပါ-

| အမည် | ရည်ရွယ်ချက် |
|--------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii သည် `/v1/sorafs/pin/register` ကို ဖော်ထုတ်သည့် လက်ခံဆောင်ရွက်ပေးသည်။ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | တင်ပြချက်များနှင့်အတူ မှတ်တမ်းတင်ထားသော ခေတ်အမှတ်အသား။ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | ထင်ရှားစွာတင်ပြချက်အတွက် အခွင့်အာဏာကို လက်မှတ်ရေးထိုးခြင်း။ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` သည် `true` ဖြစ်သောအခါတွင် ထင်ရှားသောအမည်များ tuple ကို ချည်နှောင်ထားသည်။ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-encoded alias proof bundle (ချန်လှပ်ထားနိုင်သည်၊ alias binding ကိုကျော်ရန် ချန်လှပ်ထား)။ |
| `DOCS_ANALYTICS_*` | အခြားအလုပ်အသွားအလာများဖြင့် ပြန်လည်အသုံးပြုထားသော လက်ရှိခွဲခြမ်းစိတ်ဖြာမှု/စုံစမ်းစစ်ဆေးရေး အဆုံးမှတ်များ။ |

လုပ်ဆောင်ချက်များ UI မှတစ်ဆင့် အလုပ်အသွားအလာကို အစပျိုးပါ-

1. `alias_label` (e.g., `docs.sora.link`), ချန်လှပ်ထားသော `proposal_alias` ကို ပေးပါ။
   နှင့် ရွေးချယ်နိုင်သော `release_tag` ကို ထပ်ရေးပါ။
2. `perform_submit` ကို Torii ကိုမထိဘဲ ရှေးဟောင်းပစ္စည်းများ ထုတ်လုပ်ရန် အမှတ်ခြစ်မထားပါနှင့်။
   (ခြောက်သွေ့သောအပြေးများအတွက်အသုံးဝင်သည်) သို့မဟုတ် configured ထံသို့တိုက်ရိုက်ထုတ်ဝေရန်၎င်းကိုဖွင့်ပါ။
   နာမည်များ

`docs/source/sorafs_ci_templates.md` သည် ယေဘူယျ CI အထောက်အကူများကို မှတ်တမ်းတင်ထားဆဲဖြစ်သည်။
ဤသိုလှောင်မှုအပြင်ဘက်တွင် ပရောဂျက်များဖြစ်သော်လည်း ပေါ်တယ်လုပ်ငန်းအသွားအလာကို ဦးစားပေးသင့်သည်။
နေ့စဉ်ထုတ်များအတွက်။

## စစ်ဆေးရန်စာရင်း

- [ ] `npm run build`၊ `npm run test:*` နှင့် `npm run check:links` သည် အစိမ်းရောင်ဖြစ်သည်။
- [ ] `build/checksums.sha256` နှင့် `build/release.json` တို့ကို သိမ်းဆည်းရမိခဲ့သည်။
- [ ] `artifacts/` အောက်တွင် ထုတ်ပေးထားသော CAR၊ အစီအစဉ်၊ သရုပ်ဖော်ခြင်းနှင့် အကျဉ်းချုပ်။
- [ ] Sigstore အစုအဝေး + မှတ်တမ်းများနှင့်အတူ သိမ်းဆည်းထားသော သီးခြားလက်မှတ်။
- [ ] `portal.manifest.submit.summary.json` နှင့် `portal.manifest.submit.response.json`
      တင်ပြမှု ဖြစ်ပေါ်လာသောအခါ ဖမ်းသည်။
- [ ] `portal.pin.report.json` (နှင့် ရွေးချယ်နိုင်သော `portal.pin.proposal.json`)
      CAR/manifest artefacts နှင့်အတူ မော်ကွန်းတင်ထားသည်။
- [ ] `proof verify` နှင့် `manifest verify-signature` မှတ်တမ်းများကို သိမ်းဆည်းပြီးပါပြီ။
- [ ] Grafana ဒက်ရှ်ဘုတ်များကို အပ်ဒိတ်လုပ်ထားသည် + စမ်းသုံးကြည့်ပါက အောင်မြင်သည်။
- [ ] Rollback မှတ်စုများ (ယခင် manifest ID + alias digest) ကို ပူးတွဲပါရှိသည်။
      လက်မှတ်ထုတ်ပေးသည်။