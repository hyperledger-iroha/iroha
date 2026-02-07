---
lang: my
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# ဓမ္မဒုံ-အသင်း အဆင့်အတန်း

ဤစာမျက်နှာသည် [Moderation Red-Team Plan](moderation_red_team_plan.md) ကို ဖြည့်စွက်ထားပါသည်။
အနီးနားရှိ လေ့ကျင့်ရေးပြက္ခဒိန်၊ အထောက်အထားအစုအဝေးများနှင့် ပြန်လည်ပြင်ဆင်မှုများကို ခြေရာခံခြင်းဖြင့်
အခြေအနေ အောက်ရှိ ဖမ်းယူထားသော ပစ္စည်းများနှင့် တွဲပြီး ပြေးတိုင်း အပ်ဒိတ်လုပ်ပါ။
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`။

## လာမည့် လေ့ကျင့်မှုများ

| ရက်စွဲ (UTC) | ဇာတ်လမ်း | ပိုင်ရှင်(များ) | အထောက်အထားပြင်ဆင်ခြင်း | မှတ်စုများ |
|--------------------|---------|----------------|----------------|--------|
| 2026-11-12 | **Operation Blindfold** — တံခါးပေါက် အဆင့်နှိမ့်ချရန် ကြိုးပမ်းမှုဖြင့် Taikai ရောနှော မှောင်ခိုမုဒ် အစမ်းလေ့ကျင့်မှု | လုံခြုံရေးအင်ဂျင်နီယာ (Miyu Sato)၊ ဝန်ကြီးဌာန Ops (Liam O'Connor) | `scripts/ministry/scaffold_red_team_drill.py` အတွဲလိုက် `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + ဇာတ်ညွှန်းလမ်းညွှန် `artifacts/ministry/red-team/2026-11/operation-blindfold/` | GAR/Taikai ထပ်နေသော လေ့ကျင့်ခန်းများနှင့် DNS ပျက်ကွက်ခြင်း မစတင်မီ Merkle လျှပ်တစ်ပြက်ရိုက်ချက်အား ငြင်းဆိုရန်စာရင်း လိုအပ်ပြီး ဒက်ရှ်ဘုတ်များကို ဖမ်းယူပြီးနောက် `export_red_team_evidence.py` ကို လုပ်ဆောင်သည်။ |

## နောက်ဆုံး Drill Snapshot

| ရက်စွဲ (UTC) | ဇာတ်လမ်း | အထောက်အထားအတွဲ | ပြန်လည်ပြင်ဆင်ရေးနှင့် နောက်ဆက်တွဲများ |
|--------------------|---------|-----------------|--------------------------------|
| 2026-08-18 | **Operation SeaGlass** — ဂိတ်ဝတွင် လူမှောင်ခိုကူးခြင်း၊ အုပ်ချုပ်မှု ပြန်လည်ဖွင့်ခြင်းနှင့် သတိပေးချက် ဖောက်ပြန်ခြင်း အစမ်းလေ့ကျင့်မှု | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana တင်ပို့မှုများ၊ Alertmanager မှတ်တမ်းများ၊ `seaglass_evidence_manifest.json`) | **ဖွင့်ခြင်း-** တံဆိပ်ကို အလိုအလျောက်စနစ်ဖြင့် ပြန်ဖွင့်ခြင်း (`MINFO-RT-17`၊ ပိုင်ရှင်- Governance Ops၊ သတ်မှတ်ရက် 2026-09-05); ပင်နံပါတ် ဒက်ရှ်ဘုတ်ကို SoraFS (`MINFO-RT-18`၊ Observability၊ 2026-08-25 အပြီးသတ်) တွင် ရပ်တန့်ထားသည်။ **ပိတ်ထားသည်-** Norito manifest hash များကိုသယ်ဆောင်ရန် logbook နမူနာပုံစံကို အပ်ဒိတ်လုပ်ထားသည်။ |

## ခြေရာခံခြင်းနှင့် ကိရိယာတန်ဆာပလာ

- ထိုးဆေးထုပ်ပိုးရန်အတွက် `scripts/ministry/moderation_payload_tool.py` ကိုသုံးပါ။
  အခြေအနေတစ်ခုအတွက် payloads နှင့် denylist patches များ။
- `scripts/ministry/export_red_team_evidence.py` မှတစ်ဆင့် ဒက်ရှ်ဘုတ်/မှတ်တမ်းဖမ်းယူမှုများကို မှတ်တမ်းတင်ပါ။
  အစီအစဥ်တစ်ခုစီပြီးပြီးချင်း သက်သေပြချက်တွင် ဆိုင်းဘုတ်များပါရှိသော မျဉ်းများပါရှိသည်။
- CI အစောင့်သည် `ci/check_ministry_red_team.sh` သည် လေ့ကျင့်မှုအစီရင်ခံစာများကို ကျူးလွန်ကြောင်း ပြဋ္ဌာန်းသည်
  နေရာယူထားသော စာသားမပါဝင်ပါ၊ ရည်ညွှန်းထားသော ရှေးဟောင်းပစ္စည်းများသည် ယခင်ကရှိခဲ့သည်။
  ပေါင်းစည်းခြင်း။

တိုက်ရိုက်ကိုးကားထားသောတိုက်ရိုက်အကျဉ်းချုပ်အတွက် `status.md` (§ *ဝန်ကြီးဌာနအနီ-အဖွဲ့အခြေအနေ*) ကိုကြည့်ပါ
အပတ်စဉ်ညှိနှိုင်းခေါ်ဆိုမှုများ။