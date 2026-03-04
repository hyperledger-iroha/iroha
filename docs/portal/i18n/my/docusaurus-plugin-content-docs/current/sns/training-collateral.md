---
id: training-collateral
lang: my
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> မှန်ချပ် `docs/source/sns/training_collateral.md`။ ရှင်းလင်းတင်ပြသည့်အခါ ဤစာမျက်နှာကို အသုံးပြုပါ။
> မှတ်ပုံတင်အရာရှိ၊ DNS၊ အုပ်ထိန်းသူနှင့် ဘဏ္ဍာရေးအဖွဲ့များသည် နောက်ဆက်တွဲတစ်ခုစီကို စတင်ခြင်းမပြုမီ။

## 1. သင်ရိုးညွှန်းတမ်း လျှပ်တစ်ပြက်

| တစ်ပုဒ် | ရည်မှန်းချက်များ | ကြိုဖတ် |
|---------|------------|-----------|
| မှတ်ပုံတင်အရာရှိ ops | မန်နီးဖက်စ်များကို တင်သွင်းပါ၊ KPI ဒက်ရှ်ဘုတ်များကို စောင့်ကြည့်ပါ၊ အမှားအယွင်းများကို တိုးမြင့်ပါ။ | `sns/onboarding-kit`၊ `sns/kpi-dashboard`။ |
| DNS & ဂိတ်ဝေး | ဖြေရှင်းသူအရိုးစုများကို အသုံးပြုပါ၊ အေးခဲခြင်း/ပြန်လှည့်ခြင်းကို အစမ်းလေ့ကျင့်ပါ။ | `sorafs/gateway-dns-runbook`၊ တိုက်ရိုက်မုဒ်မူဝါဒနမူနာများ။ |
| အုပ်ထိန်းသူများ & ကောင်စီ | အငြင်းပွားမှုများကို လုပ်ဆောင်ပါ၊ အုပ်ချုပ်မှုဆိုင်ရာ အပိုဆောင်းကို အပ်ဒိတ်လုပ်ပါ၊ မှတ်တမ်း နောက်ဆက်တွဲများကို လုပ်ဆောင်ပါ။ | `sns/governance-playbook`၊ ဘဏ္ဍာစိုး အမှတ်စာရင်းများ။ |
| ဘဏ္ဍာရေး & ခွဲခြမ်းစိတ်ဖြာမှု | ARPU/အစုလိုက် မက်ထရစ်များကို ဖမ်းယူပါ၊ နောက်ဆက်တွဲအတွဲများကို ထုတ်ဝေပါ။ | `finance/settlement-iso-mapping`၊ KPI ဒက်ရှ်ဘုတ် JSON။ |

### Module စီးဆင်းမှု

1. **M1 — KPI orientation (30min):** လမ်းလျှောက်ရန် နောက်ဆက်တွဲ စစ်ထုတ်မှုများ၊ ပို့ကုန်များနှင့် ထွက်ပြေးသူ
   အေးခဲသောကောင်တာများ။ ပေးပို့နိုင်သည်- SHA-256 အညွှန်းဖြင့် PDF/CSV လျှပ်တစ်ပြက်ရိုက်ချက်များ။
2. **M2 — Manifest lifecycle (45min):** မှတ်ပုံတင်အရာရှိ manifests များကို တည်ဆောက်ပြီး အတည်ပြုခြင်း၊
   `scripts/sns_zonefile_skeleton.py` မှတစ်ဆင့် ဖြေရှင်းသူအရိုးစုများကို ထုတ်လုပ်ပါ။ ပေးပို့နိုင်သည်-
   git diff အရိုးစု + GAR အထောက်အထားပြသခြင်း။
3. **M3 — အငြင်းပွားမှုလေ့ကျင့်ခန်းများ (40 မိနစ်):** အုပ်ထိန်းသူအား အေးခဲစေခြင်း + အယူခံဝင်ခြင်း၊ ဖမ်းယူခြင်း
   `artifacts/sns/training/<suffix>/<cycle>/logs/` အောက်တွင် အုပ်ထိန်းသူ CLI မှတ်တမ်းများ။
4. **M4 — နောက်ဆက်တွဲ ဖမ်းယူမှု (25 မိနစ်):** ဒက်ရှ်ဘုတ် JSON ကို ထုတ်ယူပြီး run-

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   ပေးအပ်နိုင်သည်- အပ်ဒိတ်လုပ်ထားသော နောက်ဆက်တွဲ Markdown + စည်းမျဉ်းစည်းကမ်း + ပေါ်တယ်မှတ်စုတိုတုံးများ။

## 2. Localization အလုပ်အသွားအလာ

- ဘာသာစကားများ- `ar`, `es`, `fr`, `ja`, `pt`, `ru`, I18NI00000
- ဘာသာပြန်တစ်ခုစီသည် အရင်းအမြစ်ဖိုင်၏ဘေးတွင် ရှိနေသည်။
  (`docs/source/sns/training_collateral.<lang>.md`)။ `status`+ ကို အပ်ဒိတ်လုပ်ပါ။
  ပြီးနောက် `translation_last_reviewed` မပါလို့။
- ဘာသာစကားအလိုက် ပိုင်ဆိုင်မှုများ
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (ဆလိုက်များ/၊ အလုပ်စာအုပ်များ/၊
  မှတ်တမ်းများ/၊ မှတ်တမ်းများ/)။
- အင်္ဂလိပ်ကိုတည်းဖြတ်ပြီးနောက် `python3 scripts/sync_docs_i18n.py --lang <code>` ကိုဖွင့်ပါ။
  အရင်းအမြစ်ဖြစ်တာကြောင့် ဘာသာပြန်သူတွေက hash အသစ်ကို မြင်နိုင်ပါတယ်။

### ပေးပို့မှုစာရင်း

1. ဘာသာပြန်ဆိုချက် ဆောင်းပါးတို (`status: complete`) ကို တစ်ခါတည်း ပြောင်းပြီး အပ်ဒိတ်လုပ်ပါ။
2. ဆလိုက်များကို PDF သို့ ထုတ်ယူပြီး ဘာသာစကားတစ်ခုစီ `slides/` လမ်းညွှန်သို့ အပ်လုဒ်လုပ်ပါ။
3. ≤10min KPI လမ်းညွှန်ချက် မှတ်တမ်း ဘာသာစကား ဆောင်းပါးတိုမှ လင့်ခ်။
4. Slide/workbook ပါရှိသော `sns-training` ကို တဂ်လုပ်ထားသော ဖိုင်အုပ်ချုပ်မှုလက်မှတ်
   အချေအတင်များ၊ မှတ်တမ်းတင်လင့်ခ်များနှင့် နောက်ဆက်တွဲ အထောက်အထားများ။

## 3. သင်တန်းအပ်ပါတယ်။

- ဆလိုက်ကောက်ကြောင်း- `docs/examples/sns_training_template.md`။
- Workbook ပုံစံ- `docs/examples/sns_training_workbook.md` (တက်ရောက်သူ တစ်ဦးလျှင် တစ်ခု)။
- ဖိတ်ကြားချက် + သတိပေးချက်များ- `docs/examples/sns_training_invite_email.md`။
- အကဲဖြတ်ပုံစံ- `docs/examples/sns_training_eval_template.md` (တုံ့ပြန်မှုများ
  `artifacts/sns/training/<suffix>/<cycle>/feedback/`) အောက်တွင် သိမ်းဆည်းထားသည်။

## 4. အချိန်ဇယားနှင့် တိုင်းတာမှုများ

| သံသရာ | ပြတင်းပေါက် | မက်ထရစ်များ | မှတ်စုများ |
|---------|--------|---------|---------|
| 2026-03 | KPI သုံးသပ်ချက် | ပို့စ်တင်ပါ။ တက်ရောက်သူ % ၊ နောက်ဆက်တွဲ မှတ်တမ်း မှတ်တမ်း | `.sora` + `.nexus` အတွဲများ |
| 2026-06 | ကြိုတင် `.dao` GA | ဘဏ္ဍာရေး စေတနာ ≥90% | မူဝါဒ ပြန်လည်စတင်ခြင်း | ပါဝင်ပါ။
| 2026-09 | ချဲ့ထွင် | အငြင်းပွားမှု drill <20min၊ နောက်ဆက်တွဲ SLA ≤2days | SN-7 မက်လုံးများ |

`docs/source/sns/reports/sns_training_feedback.md` တွင် အမည်မသိ တုံ့ပြန်ချက်ကို ရိုက်ကူးပါ။
ထို့ကြောင့် နောက်ဆက်တွဲအုပ်စုများသည် ဒေသန္တရပြုခြင်းနှင့် ဓာတ်ခွဲခန်းများကို တိုးတက်စေနိုင်သည်။