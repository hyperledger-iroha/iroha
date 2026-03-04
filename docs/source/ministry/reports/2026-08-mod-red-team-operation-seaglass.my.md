---
lang: my
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill - စစ်ဆင်ရေး SeaGlass

- **Drill ID:** `20260818-operation-seaglass`
- **ရက်စွဲနှင့် ဝင်းဒိုး-** `2026-08-18 09:00Z – 11:00Z`
- ** ဇာတ်လမ်းအမျိုးအစား-** `smuggling`
- **အော်ပရေတာများ-** `Miyu Sato, Liam O'Connor`
- ** Dashboards များကို commit မှ ရပ်တန့်ထားသည်-** `364f9573b`
- **အထောက်အထားအတွဲ-** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (ချန်လှပ်):** `not pinned (local bundle only)`
- **ဆက်စပ်လမ်းပြမြေပုံ-** `MINFO-9`၊ နှင့် ချိတ်ဆက်ထားသော နောက်ဆက်တွဲများ `MINFO-RT-17` / `MINFO-RT-18`။

## 1. Objectives & Entry Conditions

**မူလရည်မှန်းချက်များ**
  - ဝန်ချမှုသတိပေးချက်များနေစဉ် လူမှောင်ခိုကူးရန် ကြိုးပမ်းမှုတစ်ခုအတွင်း TTL ပြဋ္ဌာန်းချက်နှင့် ဂိတ်ဝေး သီးခြားခွဲထွက်ခြင်းစာရင်းကို ငြင်းဆိုခြင်းကို အတည်ပြုပါ။
  - ထိန်းညှိပေးသည့်စာအုပ်တွင် အုပ်ချုပ်မှုပြန်လည်ဖွင့်ခြင်းကို အတည်ပြုပြီး အညိုရောင်ထွက်ခြင်းကို ကိုင်တွယ်ရန် သတိပေးချက်ကို အတည်ပြုပါ။
- **ကြိုတင်လိုအပ်ချက်များကို အတည်ပြုပါ**
  - `emergency_canon_policy.md` ဗားရှင်း `v2026-08-seaglass`။
  - `dashboards/grafana/ministry_moderation_overview.json` digest `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`။
  - ခေါ်ဆိုမှုတွင် အခွင့်အာဏာ- `Kenji Ito (GovOps pager)`။

## 2. Execution Timeline

| Timestamp (UTC) | မင်းသား | လုပ်ဆောင်ချက်/ Command | ရလဒ် / မှတ်စုများ |
|-----------------|-------|------------------|----------------|
| 09:00:12 | Miyu Sato | `scripts/ministry/export_red_team_evidence.py --freeze-only` မှတစ်ဆင့် `364f9573b` တွင် ဒက်ရှ်ဘုတ်များ/သတိပေးချက်များကို ရပ်ထား `dashboards/` | အောက်တွင် ဖမ်းယူပြီး သိမ်းဆည်းထားသည်။
| 09:07:44 | Liam O'Connor | ထုတ်ပြန်ထားသော ငြင်းပယ်စာရင်းလျှပ်တစ်ပြက် + GAR ကို `sorafs_cli ... gateway update-denylist --policy-tier emergency` | လျှပ်တစ်ပြက်ဓာတ်ပုံကို လက်ခံခဲ့သည်။ Alertmanager | တွင် မှတ်တမ်းတင်ထားသော ဝင်းဒိုးကို အစားထိုးပါ။
| 09:17:03 | Miyu Sato | `moderation_payload_tool.py --scenario seaglass` ကို အသုံးပြု၍ မှောင်ခိုလုပ်ငန်းကို ထိုးသွင်းလိုက်သည် + 3m12s အကြာတွင် သတိပေးချက် ထွက်ပေါ်လာသည်။ အုပ်ချုပ်မှု ပြန်လည်ဖွင့်ခြင်းကို အလံတင် |
| 09:31:47 | Liam O'Connor | အထောက်အထားများ ထုတ်ယူပြီး တံဆိပ်ခတ်ထားသော သရုပ်ပြ `seaglass_evidence_manifest.json` | `manifests/` | အောက်တွင် သိမ်းဆည်းထားသည့် အထောက်အထားအစုအဝေးနှင့် ဟက်ရှ်များ

## 3. Observations & Metrics

| မက်ထရစ် | ပစ်မှတ် | စောင့်ကြည့်လေ့လာ | Pass/Fail | မှတ်စုများ |
|--------|--------|----------------|-----------|--------|
| သတိပေးချက်တုံ့ပြန်မှု latency | = 0.98 | 0.992 | ✅ | မှောင်ခိုနှင့် ပြန်ဖွင့်သည့် ဝန်ထုပ်ဝန်ပိုး နှစ်မျိုးလုံးကို ရှာဖွေတွေ့ရှိ |
| Gateway ကွဲလွဲမှုကို ထောက်လှမ်းခြင်း | သတိပေးချက် | အလုပ်ဖြုတ် သတိပေးချက် + အလိုအလျောက် သီးသန့်ခွဲထွက်ခြင်း | ✅ | ထပ်စမ်းကြည့်ပါ ဘတ်ဂျက်ကုန်သွားပြီ |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. ရှာဖွေတွေ့ရှိမှုများ & ပြန်လည်ပြင်ဆင်ခြင်း။

| ပြင်းထန်မှု | ရှာဖွေခြင်း | ပိုင်ရှင် | ပစ်မှတ်နေ့စွဲ | အဆင့်အတန်း / လင့်ခ် |
|----------|---------|------|----------------|----------------|
| မြင့် | အုပ်ချုပ်မှုပြန်လည်ဖွင့်ခြင်းသတိပေးချက်ကို အလုပ်ဖြုတ်လိုက်သော်လည်း SoraFS တံဆိပ်သည် စောင့်ဆိုင်းစာရင်းပျက်ကွက်မှုစတင်သောအခါ 2m နှောင့်နှေးခဲ့သည် | Governance Ops (Liam O'Connor) | 2026-09-05 | `MINFO-RT-17` ပွင့်သည် — ပျက်ကွက်သည့်လမ်းကြောင်းသို့ ပြန်ဖွင့်သည့် တံဆိပ်ကို အလိုအလျောက်ထည့်သွင်းခြင်း |
| လတ် | ဒက်ရှ်ဘုတ်ကို SoraFS တွင် တွဲမထားပါ ။ အော်ပရေတာများသည် ဒေသခံအစုအဝေး | မြင်နိုင်စွမ်း (Miyu Sato) | 2026-08-25 | `MINFO-RT-18` ကိုဖွင့်သည် — ပင်နံပါတ် `dashboards/*` မှ SoraFS သို့ လက်မှတ်မထိုးမီ CID ဖြင့် နောက်တစ်ကြိမ်လေ့ကျင့်မှု |
| နိမ့် | CLI မှတ်တမ်းစာအုပ်ကို ချန်လှပ်ထားခဲ့ပြီး Norito ၏ ပထမဆုံး pass တွင် hash ကိုဖော်ပြခြင်း | Ministry Ops (Kenji Ito) | 2026-08-22 | လေ့ကျင့်နေစဉ်အတွင်းပုံသေ; logbook | တွင် မွမ်းမံထားသော နမူနာပုံစံချိန်ညှိခြင်းဖော်ပြပုံ၊ ငြင်းပယ်စာရင်းမူဝါဒများ သို့မဟုတ် SDK/ကိရိယာကို ပြောင်းလဲရမည်ဟု မှတ်တမ်းမှတ်ထားပါ။ GitHub/Jira ပြဿနာများနှင့် ချိတ်ဆက်ပြီး ပိတ်ဆို့ထားသော/ပိတ်ဆို့ခံထားရသော ပြည်နယ်များကို မှတ်သားပါ။

## 5. အုပ်ချုပ်မှုနှင့် အတည်ပြုချက်များ

- **Incident commander sign-off:** `Miyu Sato @ 2026-08-18T11:22Z`
- **အုပ်ချုပ်ရေးကောင်စီ ပြန်လည်သုံးသပ်သည့်ရက်စွဲ-** `GovOps-2026-08-22`
- **နောက်ဆက်တွဲ စစ်ဆေးရန်စာရင်း-** `[x] status.md updated`၊ `[x] roadmap row updated`၊ `[x] transparency packet annotated`

## ၆။တွယ်တာမှု

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

ပူးတွဲပါဖိုင်တစ်ခုစီကို `[x]` ဖြင့် အထောက်အထားအစုအဝေးသို့ အပ်လုဒ်လုပ်ပြီးသည်နှင့် SoraFS ကို အမှတ်အသားပြုပါ။

---

_နောက်ဆုံးအပ်ဒိတ်လုပ်ခဲ့သည်- 2026-08-18_