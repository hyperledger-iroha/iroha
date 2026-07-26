---
lang: my
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **အသုံးပြုနည်း-** ဤတမ်းပလိတ်ကို `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` တွင် တူးပြီးတိုင်း ချက်ချင်းပွားပါ။ ဖိုင်အမည်များကို စာလုံးသေး၊ တုံးတုံးထားသော၊ Alertmanager တွင် အကောင့်ဝင်ထားသော drill ID နှင့် ချိန်ညှိထားပါ။

# Red-Team Drill Report — `<SCENARIO NAME>`

- **Drill ID:** `<YYYYMMDD>-<scenario>`
- **ရက်စွဲနှင့် ဝင်းဒိုး-** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- ** ဇာတ်လမ်းအမျိုးအစား-** `smuggling | bribery | gateway | ...`
- **အော်ပရေတာများ-** `<names / handles>`
- ** Dashboards များကို commit မှ ရပ်တန့်ထားသည်-** `<git SHA>`
- **အထောက်အထားအတွဲ-** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (ချန်လှပ်):** `<cid>`  
- **ဆက်စပ်လမ်းပြမြေပုံ-** `MINFO-9` နှင့် ချိတ်ဆက်ထားသော လက်မှတ်များ။

## 1. Objectives & Entry Conditions

**မူလရည်မှန်းချက်များ**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **ကြိုတင်လိုအပ်ချက်များကို အတည်ပြုပါ**
  - `emergency_canon_policy.md` ဗားရှင်း `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` digest `<sha256>`
  - ခေါ်ဆိုမှုတွင် အခွင့်အာဏာ- `<name>` ကို အစားထိုးပါ။

## 2. Execution Timeline

| Timestamp (UTC) | မင်းသား | လုပ်ဆောင်ချက်/ Command | ရလဒ် / မှတ်စုများ |
|-----------------|-------|------------------|----------------|
|  |  |  |  |

> Torii တောင်းဆိုချက် ID များ၊ အတုံးအတုံးများ ဟက်ခ်များ၊ အတည်ပြုချက်များကို အစားထိုးခြင်းနှင့် သတိပေးချက်မန်နေဂျာ လင့်ခ်များ ပါဝင်ပါ။

## 3. Observations & Metrics

| မက်ထရစ် | ပစ်မှတ် | စောင့်ကြည့်လေ့လာ | Pass/Fail | မှတ်စုများ |
|--------|--------|----------------|-----------|--------|
| သတိပေးချက်တုံ့ပြန်မှု latency | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| ထိန်းချုပ်မှု ထောက်လှမ်းမှုနှုန်း | `>= <value>` |  |  |  |
| Gateway ကွဲလွဲမှုကို ထောက်လှမ်းခြင်း | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. ရှာဖွေတွေ့ရှိမှုများ & ပြန်လည်ပြင်ဆင်ခြင်း။

| ပြင်းထန်မှု | ရှာဖွေခြင်း | ပိုင်ရှင် | ပစ်မှတ်နေ့စွဲ | အဆင့်အတန်း / လင့်ခ် |
|----------|---------|------|----------------|----------------|
| မြင့် |  |  |  |  |

ချိန်ညှိခြင်းဖော်ပြပုံ၊ ငြင်းပယ်စာရင်းမူဝါဒများ သို့မဟုတ် SDK/ကိရိယာကို ပြောင်းလဲရမည်ဟု မှတ်တမ်းမှတ်ထားပါ။ GitHub/Jira ပြဿနာများနှင့် ချိတ်ဆက်ပြီး ပိတ်ဆို့ထားသော/ပိတ်ဆို့ခံထားရသော ပြည်နယ်များကို မှတ်သားပါ။

## 5. အုပ်ချုပ်မှုနှင့် အတည်ပြုချက်များ

- **Incident commander sign-off:** `<name / timestamp>`
- **အုပ်ချုပ်ရေးကောင်စီ ပြန်လည်သုံးသပ်သည့်ရက်စွဲ-** `<meeting id>`
- **နောက်ဆက်တွဲ စစ်ဆေးရန်စာရင်း-** `[ ] status.md updated`၊ `[ ] roadmap row updated`၊ `[ ] transparency packet annotated`

## ၆။တွယ်တာမှု

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

ပူးတွဲပါဖိုင်တစ်ခုစီကို `[x]` ဖြင့် အထောက်အထားအစုအဝေးသို့ အပ်လုဒ်လုပ်ပြီးသည်နှင့် SoraFS လျှပ်တစ်ပြက်ရိုက်ချက်သို့ အမှတ်အသားပြုပါ။

---

_နောက်ဆုံးမွမ်းမံထားသည်- {{ ရက်စွဲ | မူရင်း("2026-02-20") }}_