---
lang: my
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM နှင့် သက်သေအထောက်အထား - Android SDK

| လယ် | တန်ဖိုး |
|---------|-------|
| နယ်ပယ် | Android SDK (`java/iroha_android`) + နမူနာအက်ပ်များ (`examples/android/*`) |
| အလုပ်အသွားအလာ ပိုင်ရှင် | ဖြန့်ချိရေးအင်ဂျင်နီယာ (Alexei Morozov) |
| နောက်ဆုံးအတည်ပြုပြီး | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. မျိုးဆက်လုပ်ငန်းအသွားအလာ

helper script ကို run (AND6 automation အတွက် ထည့်ထားသည်)

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

ဇာတ်ညွှန်းသည် အောက်ပါတို့ကို လုပ်ဆောင်သည်-

1. `ci/run_android_tests.sh` နှင့် `scripts/check_android_samples.sh` ကို လုပ်ဆောင်သည်။
2. CycloneDX SBOM များတည်ဆောက်ရန်အတွက် `examples/android/` အောက်တွင် Gradle wrapper ကို ဖိတ်ခေါ်သည်
   ထောက်ပံ့ပေးထားသော `:android-sdk`၊ `:operator-console` နှင့် `:retail-wallet`
   `-PversionName`။
3. SBOM တစ်ခုစီကို `artifacts/android/sbom/<sdk-version>/` သို့ ကျမ်းဂန်အမည်များဖြင့် မိတ္တူကူးပါ။
   (`iroha-android.cyclonedx.json` စသည်ဖြင့်)။

## 2. Provenance & Signing

တူညီသော script သည် SBOM တစ်ခုစီတိုင်းတွင် `cosign sign-blob --bundle <file>.sigstore --yes` ဖြင့် အမှတ်အသားပြုပါသည်။
ဦးတည်ရာလမ်းကြောင်းတွင် `checksums.txt` (SHA-256) ကို ထုတ်လွှတ်သည်။ `COSIGN` ကို သတ်မှတ်ပါ။
binary သည် `$PATH` အပြင်ဘက်တွင် နေထိုင်ပါက ပတ်ဝန်းကျင် ပြောင်းလဲနိုင်သည်။ ဇာတ်ညွှန်းပြီးရင်၊
bundle/checksum လမ်းကြောင်းများအပြင် Buildkite run id ကို မှတ်တမ်းတင်ပါ။
`docs/source/compliance/android/evidence_log.csv`။

## 3. အတည်ပြုခြင်း။

ထုတ်ပြန်ထားသော SBOM ကို အတည်ပြုရန်-

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

အထွက် SHA ကို `checksums.txt` တွင်ဖော်ပြထားသောတန်ဖိုးနှင့် နှိုင်းယှဉ်ပါ။ ပြန်လည်သုံးသပ်သူများသည် မှီခိုမှုမြစ်ဝကျွန်းပေါ်ဒေသများသည် ရည်ရွယ်ချက်ရှိရှိဖြင့် သေချာစေရန် ယခင်ထုတ်ဝေမှုနှင့် SBOM တို့ကို ကွဲပြားစေပါသည်။

## 4. အထောက်အထား လျှပ်တစ်ပြက် (2026-02-11)

| အစိတ်အပိုင်း | SBOM | SHA-256 | Sigstore Bundle |
|-----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` SBOM | ဘေးတွင်သိမ်းဆည်းထားသောအတွဲ
| အော်ပရေတာ ကွန်ဆိုးလ်နမူနာ | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| လက်လီပိုက်ဆံအိတ်နမူနာ | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite run ထားသော `android-sdk-release#4821` မှဖမ်းယူထားသော Hashes များ ၊ အပေါ်က verification command မှတဆင့် ပြန်ထုတ်ပေးပါသည်။)*

## 5. ထူးချွန်သောအလုပ်

- GA မတိုင်မီ လွှတ်တင်သည့်ပိုက်လိုင်းအတွင်းရှိ SBOM + cosign အဆင့်များကို အလိုအလျောက်လုပ်ပါ။
- AND6 သည် စစ်ဆေးရန်စာရင်း ပြီးသည်နှင့်တစ်ပြိုင်နက် အများသူငှာ ပစ္စည်းများပုံးသို့ Mirror SBOM များ။
- မိတ်ဖက်ရင်ဆိုင်နေရသော ထုတ်ဝေမှုမှတ်စုများမှ SBOM ဒေါင်းလုဒ်တည်နေရာများကို ချိတ်ဆက်ရန် Docs နှင့် ညှိနှိုင်းပါ။