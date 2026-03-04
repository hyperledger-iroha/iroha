---
lang: my
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

#SoraFS CI နမူနာ တန်ဆာပလာများ

ဤလမ်းညွှန်သည် နမူနာမှထုတ်ပေးသော အဆုံးအဖြတ်ပေးသည့်အရာများကို ထုပ်ပိုးထားသည်။
`fixtures/sorafs_manifest/ci_sample/` အောက်တွင် payload အစုအဝေးက သရုပ်ပြသည်။
CI အလုပ်အသွားအလာကို လေ့ကျင့်ခန်းလုပ်သော ထုပ်ပိုးခြင်းနှင့် လက်မှတ်ထိုးခြင်း ပိုက်လိုင်းမှ အဆုံးအထိ SoraFS။

## Artefact Inventory

| ဖိုင် | ဖော်ပြချက် |
|--------|-------------|
| `payload.txt` | fixture scripts (plain-text sample) မှအသုံးပြုသော source payload။ |
| `payload.car` | `sorafs_cli car pack` မှထုတ်လွှတ်သော CAR မှတ်တမ်း။ |
| `car_summary.json` | အတိုအထွာများ နှင့် မက်တာဒေတာကိုဖမ်းယူခြင်း `car pack` မှ ထုတ်ပေးသော အကျဉ်းချုပ်။ |
| `chunk_plan.json` | အတုံးအပိုင်းအခြားများနှင့် ဝန်ဆောင်မှုပေးသူမျှော်လင့်ချက်များကို ဖော်ပြသည့် ထုတ်ယူမှုအစီအစဉ် JSON။ |
| `manifest.to` | Norito သည် `sorafs_cli manifest build` မှ ထုတ်လုပ်သော မန်နီးဖက်စ်။ |
| `manifest.json` | အမှားရှာပြင်ခြင်းအတွက် လူသားဖတ်နိုင်သော သရုပ်ဖော်ခြင်း |
| `proof.json` | `sorafs_cli proof verify` မှ ထုတ်လွှတ်သော PoR အနှစ်ချုပ်။ |
| `manifest.bundle.json` | `sorafs_cli manifest sign` မှထုတ်ပေးသော သော့မဲ့လက်မှတ်အတွဲ။ |
| `manifest.sig` | မန်နီးဖက်စ်နှင့်သက်ဆိုင်သော Ed25519 လက်မှတ်ကို ဖြုတ်ထားသည်။ |
| `manifest.sign.summary.json` | လက်မှတ်ထိုးစဉ်အတွင်း CLI အနှစ်ချုပ် ( hash များ၊ မက်တာဒေတာအတွဲများ)။ |
| `manifest.verify.summary.json` | `manifest verify-signature` မှ CLI အနှစ်ချုပ်။ |

ထုတ်ဝေမှုမှတ်စုများနှင့် စာရွက်စာတမ်းများတွင် ကိုးကားထားသော အချေအတင်အားလုံးကို အရင်းအမြစ်မှ အရင်းခံထားပါသည်။
ဤဖိုင်များ။ `ci/check_sorafs_cli_release.sh` အလုပ်အသွားအလာသည် တူညီပါသည်။
အနုပညာလက်ရာများနှင့် ကျူးလွန်ထားသောဗားရှင်းများနှင့် ၎င်းတို့ကို ကွဲပြားစေသည်။

## Fixture Regeneration

fixture set ကိုပြန်ထုတ်ရန် repository root မှအောက်ပါ command များကို run ပါ။
၎င်းတို့သည် `sorafs-cli-fixture` အလုပ်အသွားအလာတွင် အသုံးပြုသည့် အဆင့်များကို ထင်ဟပ်နေပါသည်။

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

အဆင့်တစ်ဆင့်တွင် မတူညီသော hash များထုတ်ပေးပါက၊ တပ်ဆင်မှုများကို မွမ်းမံပြင်ဆင်ခြင်းမပြုမီ စုံစမ်းစစ်ဆေးပါ။
CI လုပ်ငန်းအသွားအလာများသည် ဆုတ်ယုတ်မှုများကို သိရှိရန် အဆုံးအဖြတ်ရလဒ်အပေါ် အားကိုးသည်။

## အနာဂတ်လွှမ်းခြုံမှု

လမ်းပြမြေပုံမှ ထပ်လောင်းချန်းကာပရိုဖိုင်များနှင့် အထောက်အထားဖော်မတ်များ ပြည့်စုံလာသည်နှင့်အမျှ၊
၎င်းတို့၏ canonical fixtures များကို ဤလမ်းညွှန်အောက်တွင် ထည့်သွင်းပါမည် (ဥပမာ၊
`sorafs.sf2@1.0.0` (`fixtures/sorafs_manifest/ci_sample_sf2/` ကိုကြည့်ပါ) သို့မဟုတ် PDP
ထုတ်လွှင့်မှုအထောက်အထားများ)။ ပရိုဖိုင်အသစ်တစ်ခုစီသည် တူညီသောဖွဲ့စည်းပုံအတိုင်းဖြစ်သည်— payload၊ CAR၊
အစီအစဥ်၊ ထင်ရှားသော၊ သက်သေများနှင့် လက်မှတ်လက်ရာများ—ဒါကြောင့် အောက်ပိုင်းအလိုအလျောက်စနစ်ဖြင့် လုပ်ဆောင်နိုင်သည်။
စိတ်ကြိုက် scripting မပါဘဲ diff ထုတ်ဝေမှုများ။