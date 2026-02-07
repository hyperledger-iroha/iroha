---
lang: my
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EU တရားဝင် အကောင့်ပိတ်မှတ်စု – 2026.1 GA (Android SDK)

## အကျဉ်းချုပ်

- **ဖြန့်ချိ/ရထား-** 2026.1 GA (Android SDK)
- **ပြန်လည်သုံးသပ်သည့်ရက်စွဲ-** 2026-04-15
- **အကြံပေး/သုံးသပ်သူ-** Sofia Martins — လိုက်နာမှုနှင့် ဥပဒေ
- ** နယ်ပယ်-** ETSI EN 319 401 လုံခြုံရေးပစ်မှတ်၊ GDPR DPIA အနှစ်ချုပ်၊ SBOM သက်သေအထောက်အထား၊ AND6 စက်-ဓာတ်ခွဲခန်းဆိုင်ရာ အထောက်အထားများ
- **ဆက်စပ်လက်မှတ်များ-** `_android-device-lab` / AND6-DR-202602၊ AND6 အုပ်ချုပ်မှုခြေရာခံကိရိယာ (`GOV-AND6-2026Q1`)

## Artefact Checklist

| Artefact | SHA-256 | တည်နေရာ/လင့်ခ် | မှတ်စုများ |
|----------|---------|-----------------|--------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA ထုတ်ဝေမှု ခွဲခြားသတ်မှတ်မှုများနှင့် ခြိမ်းခြောက်မှု မော်ဒယ် ဒယ်လ်တာများ (Torii NRPC ထပ်လောင်းမှုများ) နှင့် ကိုက်ညီပါသည်။ |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | အကိုးအကား AND7 တယ်လီမီတာမူဝါဒ (`docs/source/sdk/android/telemetry_redaction.md`)။ |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore အတွဲ (`android-sdk-release#4821`)။ | CycloneDX + သက်သေကို ပြန်လည်သုံးသပ်ပြီး၊ Buildkite အလုပ် `android-sdk-release#4821` နှင့် တိုက်ဆိုင်သည်။ |
| အထောက်အထားမှတ်တမ်း | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (အတန်း `android-device-lab-failover-20260220`) | မှတ်တမ်းဖမ်းယူထားသော အတွဲလိုက် ဟက်ရှ်များ + စွမ်းရည်လျှပ်တစ်ပြက် + မှတ်စုတို ထည့်သွင်းမှုကို အတည်ပြုသည်။ |
| စက်ပစ္စည်း-ဓာတ်ခွဲခန်း အရေးပေါ်အစုအဝေး | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json` မှ ထုတ်ယူထားသော Hash လက်မှတ် AND6-DR-202602 သည် တရားဝင်/လိုက်နာမှုသို့ လက်လွှဲခြင်းကို မှတ်တမ်းတင်ထားသည်။ |

## တွေ့ရှိမှုနှင့် ခြွင်းချက်

- ပိတ်ဆို့ခြင်းပြဿနာများကို မဖော်ထုတ်ပါ။ Artefacts များသည် ETSI/GDPR လိုအပ်ချက်များနှင့် ကိုက်ညီသည်၊ DPIA အကျဉ်းချုပ်တွင် မှတ်သားထားသော AND7 တယ်လီမီတာ ညီမျှခြင်း နှင့် နောက်ထပ် လျော့ပါးသက်သာစေရန် မလိုအပ်ပါ။
- အကြံပြုချက်- စီစဉ်ထားသည့် DR-2026-05-Q2 လေ့ကျင့်မှု (လက်မှတ် AND6-DR-202605) ကို စောင့်ကြည့်ပြီး နောက်အုပ်ချုပ်မှုစစ်ဆေးရေးဂိတ်မတိုင်မီ အထောက်အထားမှတ်တမ်းတွင် ရလဒ်အတွဲလိုက်ကို ပေါင်းထည့်ပါ။

## ခွင့်ပြုချက်

- **ဆုံးဖြတ်ချက်-** အတည်ပြုခဲ့သည်။
- **လက်မှတ်/အချိန်တံဆိပ်တုံး-** _Sofia Martins (အုပ်ချုပ်မှုပေါ်တယ်မှ ဒစ်ဂျစ်တယ်စနစ်ဖြင့် လက်မှတ်ရေးထိုးပြီး၊ 2026-04-15 14:32 UTC)_
- **နောက်ဆက်တွဲပိုင်ရှင်များ-** စက်ပစ္စည်းဓာတ်ခွဲခန်း Ops (2026-05-31 ခုနှစ်မတိုင်မီ DR-2026-05-Q2 အထောက်အထားအစုအဝေးကို ပေးပို့ပါ)