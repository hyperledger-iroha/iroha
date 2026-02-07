---
lang: my
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docs Portal အစမ်းကြည့်ခြင်း တုံ့ပြန်ချက် Digest (Template)

အုပ်ချုပ်ရေးအတွက်၊ ထုတ်လွှတ်ခြင်းအတွက် အစမ်းကြည့်လှိုင်းကို အကျဉ်းချုပ်သည့်အခါ ဤပုံစံကို အသုံးပြုပါ။
သုံးသပ်ချက်များ သို့မဟုတ် `status.md`။ Markdown ကို ခြေရာခံလက်မှတ်တွင် ကူးယူပြီး အစားထိုးပါ။
အစစ်အမှန်ဒေတာနှင့်အတူ နေရာယူထားသူများနှင့် ပေးပို့ထားသော JSON အကျဉ်းချုပ်ကို ပူးတွဲတင်ပြပါ။
`npm run --prefix docs/portal preview:log -- --summary --summary-json`။ ဟိ
`preview:digest` အကူအညီပေးသူ (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
အောက်တွင်ဖော်ပြထားသော မက်ထရစ်အပိုင်းကို ထုတ်ပေးသောကြောင့် သင်ဖြည့်စွက်ရန်သာ လိုအပ်ပါသည်။
မီးမောင်းထိုးပြမှုများ/လုပ်ဆောင်ချက်များ/အနုပညာပစ္စည်းများ အတန်းများ။

```markdown
## Wave preview-<tag> feedback digest (YYYY-MM-DD)
- Invite window: <start → end>
- Reviewers invited: <count> (open: <count>)
- Feedback submissions: <count>
- Issues opened: <count>
- Latest event timestamp: <ISO8601 from summary.json>

| Category | Details | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <e.g., “ISO builder walkthrough landed well”> | <owner + due date> |
| Blocking findings | <list issue IDs or tracker links> | <owner> |
| Minor polish items | <group cosmetic or copy edits> | <owner> |
| Telemetry anomalies | <link to dashboard snapshot / probe log> | <owner> |

## Actions
1. <Action item + link + ETA>
2. <Optional second action>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

သုံးသပ်သူများနှင့် အုပ်ချုပ်မှု တတ်နိုင်စေရန် ဖိတ်ခေါ်ခြေရာခံ လက်မှတ်ဖြင့် အချေအတင်တစ်ခုစီကို သိမ်းဆည်းပါ။
CI မှတ်တမ်းများကို မတူးဘဲ အထောက်အထားလမ်းကြောင်းကို ပြန်ဖွင့်ပါ။