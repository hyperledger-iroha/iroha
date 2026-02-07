---
lang: my
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#SoraFS CLI & SDK — ထုတ်ဝေမှုမှတ်စုများ (v0.1.0)

## ပေါ်လွင်ချက်များ
- ယခု `sorafs_cli` သည် ထုပ်ပိုးမှုပိုက်လိုင်းတစ်ခုလုံးကို ထုပ်ပိုးထားပြီး (`car pack`၊ `manifest build`၊
  `proof verify`၊ `manifest sign`၊ `manifest verify-signature`) ထို့ကြောင့် CI အပြေးသမားများက တစ်ခုအား ဖိတ်ခေါ်ပါသည်။
  တစ်ခုတည်းသော binary သည် စိတ်ကြိုက်အကူအညီပေးသူများအစား သော့မဲ့လက်မှတ်ထိုးခြင်းအသစ်သည် မူရင်းအတိုင်းဖြစ်သည်။
  `SIGSTORE_ID_TOKEN`၊ GitHub လုပ်ဆောင်ချက်များ OIDC ဝန်ဆောင်မှုပေးသူများကို နားလည်ပြီး အဆုံးအဖြတ်ပေးသော ထုတ်လွှတ်မှု
  လက်မှတ်အတွဲနှင့်အတူ JSON အနှစ်ချုပ်။
- ရင်းမြစ်ပေါင်းများစွာရယူခြင်း * အမှတ်စာရင်း* သည် `sorafs_car` ၏တစ်စိတ်တစ်ပိုင်းအဖြစ် ပို့ဆောင်သည်- ၎င်းသည် ပုံမှန်ဖြစ်စေသည်
  ဝန်ဆောင်မှုပေးသော တယ်လီမီတာ၊ စွမ်းဆောင်ရည် ပြစ်ဒဏ်များကို ပြဋ္ဌာန်းပေးသည်၊ JSON/Norito အစီရင်ခံစာများကို ဆက်လက်ထားရှိရန်နှင့်
  မျှဝေထားသော registry လက်ကိုင်မှတဆင့် သံတွဲတီးမှုတ်ခြင်း simulator (`sorafs_fetch`) ကို ကျွေးမွေးပါသည်။
  `fixtures/sorafs_manifest/ci_sample/` အောက်ရှိ တန်ဆာပလာများသည် အဆုံးအဖြတ်ကို ပြသသည်။
  CI/CD နှင့် ကွာခြားနိုင်ဖွယ်ရှိသော သွင်းအားစုများနှင့် အထွက်များ။
- ဖြန့်ချိသည့် အလိုအလျောက်စနစ်အား `ci/check_sorafs_cli_release.sh` နှင့် ကုဒ်လုပ်ထားသည်။
  `scripts/release_sorafs_cli.sh`။ ယခုထွက်ရှိမှုတိုင်းသည် ထင်ရှားသောအစုအဝေးကို သိမ်းဆည်းထားပြီး၊
  လက်မှတ်၊ `manifest.sign/verify` အနှစ်ချုပ်များနှင့် အမှတ်စာရင်း လျှပ်တစ်ပြက် အုပ်ချုပ်ရေး
  ပြန်လည်သုံးသပ်သူများသည် ပိုက်လိုင်းကို ပြန်လည်မလုပ်ဆောင်ဘဲ ရှေးဟောင်းပစ္စည်းများကို ခြေရာခံနိုင်သည်။

## အဆင့်မြှင့်တင်ခြင်း။
1. သင့်အလုပ်ခွင်ရှိ ညှိထားသောသေတ္တာများကို အပ်ဒိတ်လုပ်ပါ-
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. fmt/clippy/test အကျုံးဝင်မှုကို အတည်ပြုရန် စက်တွင်း (သို့မဟုတ် CI တွင်) ပြန်ဖွင့်ပါ-
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. ရွေးချယ်ထားသော config ဖြင့် လက်မှတ်ထိုးထားသော ရှေးဟောင်းပစ္စည်းများနှင့် အနှစ်ချုပ်များကို ပြန်ထုတ်ပါ-
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   ပြန်လည်ဆန်းသစ်ထားသော အစုအစည်း/အထောက်အထားများကို `fixtures/sorafs_manifest/ci_sample/` သို့ ကူးယူပါ။
   Canonical fixtures များ အပ်ဒိတ်များကို ထုတ်ပြန်ခြင်း။

## အတည်ပြုခြင်း။
- ဖြန့်ချိရေးဂိတ်- `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` ဂိတ်အောင်မြင်ပြီးပြီးချင်း)။
- `ci/check_sorafs_cli_release.sh` အထွက်- သိမ်းဆည်းထားသည်။
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (ထုတ်လွှတ်မှုအစုအဝေးတွင် ပူးတွဲပါရှိသည်)။
- Manifest အစုအစည်းအချေအတင်- `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`)။
- အထောက်အထား အနှစ်ချုပ်- `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`)။
- Manifest digest (မြစ်အောက်ပိုင်းသက်သေခံချက် အပြန်အလှန်စစ်ဆေးမှုများအတွက်)
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json` မှ)။

## အော်ပရေတာများအတွက်မှတ်စုများ
- ယခု Torii ဂိတ်ဝေးသည် `X-Sora-Chunk-Range` စွမ်းရည်ဆိုင်ရာ ခေါင်းစီးကို ပြဌာန်းထားသည်။ မွမ်းမံ
  ထုတ်လွှင့်မှု တိုကင်နယ်ပယ်အသစ်ကို တင်ပြသည့် ဖောက်သည်များအား ခွင့်ပြုစာရင်းများကို လက်ခံပါသည်။ တိုကင်အဟောင်းများ
  အကွာအဝေးတောင်းဆိုမှုမရှိဘဲ အခိုးခံရမည်ဖြစ်သည်။
- `scripts/sorafs_gateway_self_cert.sh` သည် manifest အတည်ပြုခြင်းကို ပေါင်းစပ်ထားသည်။ ပြေးတဲ့အခါ
  ထုပ်ပိုးနိုင်စေရန်အတွက် ကိုယ်ပိုင်အသိအမှတ်ပြုကြိုးကြိုး၊
  လက်မှတ်ပျံ့ပေါ်တွင် လျှင်မြန်စွာ ကျရှုံးသည်။
- Telemetry ဒက်ရှ်ဘုတ်များသည် ရမှတ်ဘုတ်တင်ပို့မှုအသစ် (`scoreboard.json`) သို့ ထည့်သွင်းသင့်သည်
  ဝန်ဆောင်မှုပေးသူ၏ အရည်အချင်းပြည့်မီမှု၊ အလေးချိန်တာဝန်များနှင့် ငြင်းဆိုထားသည့် အကြောင်းရင်းများကို ပြန်လည်ညှိနှိုင်းပါ။
- ဖြန့်ချိမှုတိုင်းနှင့်အတူ Canonical အနှစ်ချုပ်လေးခုကို သိမ်းဆည်းပါ။
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`၊
  `manifest.verify.summary.json`။ အုပ်ချုပ်မှုလက်မှတ်များသည် ဤကာလအတွင်း အတိအကျဖိုင်များကို ကိုးကားပါသည်။
  အတည်ပြုချက်။

## ကျေးဇူးတင်လွှာ
- သိုလှောင်မှုအဖွဲ့ — အဆုံးမှအဆုံးသို့ CLI ပေါင်းစပ်မှု၊ အတုံးအခဲ-အစီအစဉ်တင်ဆက်သူနှင့် အမှတ်စာရင်းဇယား
  telemetry ပိုက်။
- Tooling WG — ပိုက်လိုင်း (`ci/check_sorafs_cli_release.sh`၊
  `scripts/release_sorafs_cli.sh`) နှင့် အဆုံးအဖြတ်ပေးသော fixture အတွဲ။
- Gateway လည်ပတ်မှုများ — စွမ်းဆောင်ရည်ဂိတ်ပေါက်၊ တိုက်ရိုက်-တိုကင် မူဝါဒ ပြန်လည်သုံးသပ်ခြင်းနှင့် အပ်ဒိတ်လုပ်ခြင်း။
  ကိုယ်ပိုင် အသိအမှတ်ပြု ဂိမ်းစာအုပ်များ။