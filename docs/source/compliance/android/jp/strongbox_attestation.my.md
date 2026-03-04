---
lang: my
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox သက်သေအထောက်အထား - ဂျပန်ဖြန့်ကျက်မှုများ

| လယ် | တန်ဖိုး |
|---------|-------|
| အကဲဖြတ် Window | 2026-02-10 – 2026-02-12 |
| Artefact တည်နေရာ | `artifacts/android/attestation/<device-tag>/<date>/` (အစုအဝေးဖော်မတ်အတွက် `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| Capture Tooling | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| သုံးသပ်သူများ | Hardware Lab Lead, Compliance & Legal (JP) |

## 1. Capture လုပ်ထုံးလုပ်နည်း

1. StrongBox matrix တွင်ဖော်ပြထားသော စက်ပစ္စည်းတစ်ခုစီတွင်၊ စိန်ခေါ်မှုတစ်ခုဖန်တီးပြီး သက်သေအတွဲကို ဖမ်းယူပါ-
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. အစုအဝေး မက်တာဒေတာ (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) ကို အထောက်အထားသစ်သို့ ပေးပို့ပါ။
3. အော့ဖ်လိုင်း အစုအဝေးအားလုံးကို ပြန်လည်စစ်ဆေးရန် CI အထောက်အကူကို ဖွင့်ပါ-
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. စက်အကျဉ်းချုပ် (2026-02-12)

| စက်ပစ္စည်း Tag | မော်ဒယ် / StrongBox | Bundle Path | ရလဒ် | မှတ်စုများ |
|--------------------|--------------------|----------------|--------|--------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ အောင်မြင်ပြီး (ဟာ့ဒ်ဝဲ-ကျောထောက်နောက်ခံပြု) | စိန်ခေါ်မှု၊ OS patch 2025-03-05။ |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ အောင်မြင်ပြီး | မူလတန်း CI လမ်းသွား ကိုယ်စားလှယ်လောင်း၊ spec အတွင်းမှ အပူချိန်။ |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ အောင်မြင်ပြီး (ပြန်လည်စစ်ဆေးမှု) | USB-C hub ကို အစားထိုးပြီး၊ Buildkite `android-strongbox-attestation#221` သည် ဖြတ်သန်းသွားသောအတွဲကို ဖမ်းယူထားသည်။ |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ အောင်မြင်ပြီး | Knox သက်သေပရိုဖိုင်ကို 2026-02-09 တွင် တင်သွင်းခဲ့သည်။ |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ အောင်မြင်ပြီး | Knox သက်သေပရိုဖိုင်ကို တင်သွင်းပြီးပါပြီ။ CI လမ်းကြောက စိမ်းနေပြီ။ |

စက်တဂ်များသည် `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` သို့ မြေပုံညွှန်းပါသည်။

## 3. ပြန်လည်သုံးသပ်သူ စစ်ဆေးရန်စာရင်း

- [x] Verify `result.json` သည် `strongbox_attestation: true` နှင့် အသိအမှတ်ပြုထားသော root သို့ လက်မှတ်များကွင်းဆက်ကို ပြသသည်။
- [x] စိန်ခေါ်မှုဘိုက်များ Buildkite သည် `android-strongbox-attestation#219` (initial sweep) နှင့် `#221` (Pixel 8 Pro ပြန်လည်စမ်းသပ်မှု + S24 ဖမ်းယူမှု) နှင့် ကိုက်ညီကြောင်း အတည်ပြုပါ။
- [x] ဟာ့ဒ်ဝဲပြင်ဆင်ပြီးနောက် Pixel 8 Pro ဖမ်းယူမှုကို ပြန်လည်လုပ်ဆောင်ခြင်း (ပိုင်ရှင်- ဟာ့ဒ်ဝဲဓာတ်ခွဲခန်းခေါင်းဆောင်၊ ပြီးခဲ့သော 2026-02-13)။
- [x] Knox ပရိုဖိုင်အတည်ပြုချက်ရောက်ရှိသည်နှင့်တစ်ပြိုင်နက် Galaxy S24 ဖမ်းယူမှု (ပိုင်ရှင်- Device Lab Ops၊ 2026-02-13 ပြီးစီး)။

## 4. ဖြန့်ဝေခြင်း။

- ဤအကျဉ်းချုပ်နှင့် နောက်ဆုံးအစီရင်ခံစာစာသားဖိုင်ကို ပါတနာလိုက်နာမှုဆိုင်ရာ ပက်ကေ့ခ်ျများ (FISC စစ်ဆေးရေးစာရင်း §Data နေထိုင်ခွင့်) တွင် ပူးတွဲပါ ။
- စည်းကမ်းထိန်းသိမ်းရေးစာရင်းစစ်များကို တုံ့ပြန်သည့်အခါ အတွဲလိုက်လမ်းကြောင်းများကို ကိုးကားပါ။ ကုဒ်ဝှက်ထားသော ချန်နယ်များအပြင် သက်သေခံလက်မှတ်အကြမ်းများကို မပို့ပါနှင့်။

## 5. မှတ်တမ်းပြောင်းပါ။

| ရက်စွဲ | ပြောင်းလဲရန် | စာရေးသူ |
|--------|--------|--------|
| 2026-02-12 | ကနဦး JP အတွဲလိုက်ဖမ်းယူမှု + အစီရင်ခံစာ။ | စက်ပစ္စည်း ဓာတ်ခွဲခန်း ဝန်ဆောင်မှု |