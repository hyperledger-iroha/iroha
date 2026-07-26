---
lang: my
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
translator: machine-google-reviewed
---

# ဆန္ဒခံယူပွဲ Packet လုပ်ငန်းအသွားအလာ (MINFO-4)

လမ်းပြမြေပုံ အကြောင်းအရာ **MINFO-4 — ပြန်လည်သုံးသပ်မှု အကန့်နှင့် လူထုဆန္ဒခံယူပွဲ ပေါင်းစပ်ဖန်တီးမှု** သည် ယခုဖြစ်သည်။
အသစ် `ReferendumPacketV1` Norito schema နှင့် CLI အကူအညီပေးသူများ
အောက်တွင်ဖော်ပြထားသည်။ အလုပ်အသွားအလာသည် မူဝါဒ-ခုံသမာဓိအတွက် လိုအပ်သော အရာတိုင်းကို စုစည်းထားသည်။
JSON စာရွက်စာတမ်းတစ်ခုတည်းတွင် မဲပေးသည် ထို့ကြောင့် အုပ်ချုပ်ရေး၊ စာရင်းစစ်များနှင့် ပွင့်လင်းမြင်သာမှု
ပေါ်တယ်များသည် အထောက်အထားများကို အဆုံးအဖြတ်ဖြင့် ပြန်လည်ပြသနိုင်သည်။

## သွင်းအားစုများ

1. **Agenda proposal** — `cargo xtask ministry-agenda validate` အတွက် အသုံးပြုသည့် JSON တစ်ခုတည်း။
2. **စေတနာ့ဝန်ထမ်းအကျဉ်းများ** — ဖြတ်ပြီး အလင်းပြန်ပြီးနောက် ထုတ်လုပ်သည့် စုစည်းထားသော ဒေတာအတွဲ
   `cargo xtask ministry-transparency volunteer-validate`။
3. **AI ထိန်းကျောင်းမှု သရုပ်ပြ** — အုပ်ချုပ်မှု-လက်မှတ်ထိုးထားသော `ModerationReproManifestV1`။
4. **အမျိုးအစားခွဲခြင်းအနှစ်ချုပ်** — မှထုတ်လွှတ်သော အဆုံးအဖြတ်ပေးသည့်အရာ
   `cargo xtask ministry-agenda sortition`။ JSON သည် အောက်ပါအတိုင်းဖြစ်သည်။
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) ထို့ကြောင့် အုပ်ချုပ်ရေး
   POP snapshot digest နှင့် waitlist/failover wiring ကို ပြန်ထုတ်ပါ။
5. **အကျိုးသက်ရောက်မှုအစီရင်ခံစာ** — hash-family/report မှတဆင့်ထုတ်ပေးပါသည်။
   `cargo xtask ministry-agenda impact`။

## CLI အသုံးပြုမှု

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

`packet` subcommand သည် ကြားနေ-အကျဉ်းချုပ် ပေါင်းစပ်ဖွဲ့စည်းမှု (MINFO-4a) ကို လုပ်ဆောင်သည်)၊
ရှိပြီးသား စေတနာ့ဝန်ထမ်းပွဲများ နှင့် output ကို ကြွယ်ဝစေသည်-

- `ReferendumSortitionEvidence` — အယ်လဂိုရီသမ်၊ မျိုးစေ့နှင့် စာရင်းဇယားများမှ ချေဖျက်သည်။
  အမျိုးအစားခွဲခြင်းအနုပညာ။
- `ReferendumPanelist[]` — ရွေးချယ်ထားသော ကောင်စီအဖွဲ့ဝင်တစ်ဦးစီနှင့် Merkle အထောက်အထား
  သူတို့ရဲ့ ဆွဲငင်မှုကို စစ်ဆေးဖို့ လိုအပ်တယ်။
- `ReferendumImpactSummary` — တစ်-hash-family စုစုပေါင်းများနှင့် ပဋိပက္ခစာရင်းများမှ
  သက်ရောက်မှုအစီရင်ခံစာ။

သီးခြား `ReviewPanelSummaryV1` လိုအပ်နေချိန်တွင် `--summary-out` ကိုသုံးပါ
ဖိုင်; မဟုတ်ပါက ပက်ကတ်သည် `review_summary` အောက်တွင် အကျဉ်းချုပ်ကို မြှုပ်ထားသည်။

## Output ဖွဲ့စည်းပုံ

`ReferendumPacketV1` နေထိုင်သည်။
`crates/iroha_data_model/src/ministry/mod.rs` နှင့် SDK များတွင် ရနိုင်ပါသည်။
အဓိကကဏ္ဍများ ပါဝင်သည်-

- `proposal` — မူရင်း `AgendaProposalV1` အရာဝတ္ထု။
- `review_summary` — MINFO-4a မှ ထုတ်လွှတ်သော မျှတသော အနှစ်ချုပ်။
- `sortition` / `panelists` — ထိုင်ကောင်စီအတွက် ပြန်လည်ထုတ်ပေးနိုင်သော အထောက်အထားများ။
- `impact_summary` — hash မိသားစုအတွက် မိတ္တူပွား/မူဝါဒပဋိပက္ခ အထောက်အထား။

နမူနာအပြည့်အစုံအတွက် `docs/examples/ministry/referendum_packet_example.json` ကိုကြည့်ပါ။
လက်မှတ်ရေးထိုးထားသော AI နှင့်အတူ လူထုဆန္ဒခံယူပွဲတိုင်းတွင် ထုတ်ပေးထားသော ပက်ကတ်ကို ပူးတွဲပါ။
မီးမောင်းထိုးပြသည့်အပိုင်းမှ ကိုးကားထားသော ထင်ရှားမြင်သာသည့်အရာများ။