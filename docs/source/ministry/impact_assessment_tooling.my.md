---
lang: my
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ထိခိုက်မှုအကဲဖြတ်ခြင်းကိရိယာ (MINFO‑4b)

လမ်းပြမြေပုံအကိုးအကား- **MINFO‑4b — သက်ရောက်မှု အကဲဖြတ်ခြင်းကိရိယာ။**  
ပိုင်ရှင်- အုပ်ချုပ်ရေးကောင်စီ/ ပိုင်းခြားစိတ်ဖြာချက်

ဤမှတ်စုသည် ယခု `cargo xtask ministry-agenda impact` အမိန့်ကို မှတ်တမ်းတင်ထားသည်။
ဆန္ဒခံယူပွဲ packets အတွက် လိုအပ်သော အလိုအလျောက် hash-family diff ကို ထုတ်လုပ်သည်။ ဟိ
ကိရိယာသည် တရားဝင်သော လုပ်ငန်းစဉ်ကောင်စီ အဆိုပြုချက်များ၊ ပွားနေသော မှတ်ပုံတင်ခြင်းနှင့် တို့ကို စားသုံးသည်။
ရွေးချယ်နိုင်သော ငြင်းပယ်စာရင်း/မူဝါဒ လျှပ်တစ်ပြက် ရိုက်ချက်တစ်ခုအား သုံးသပ်သူများသည် မည်သည့်အရာကို အတိအကျမြင်နိုင်မည်နည်း။
လက်ဗွေရာများသည် လက်ရှိပေါ်လစီနှင့် မည်မျှဝင်ရောက်မှုများနှင့် ကိုက်ညီသည်ဖြစ်စေ အသစ်ဖြစ်သည်။
hash မိသားစုတစ်ခုစီက ပံ့ပိုးပေးတယ်။

## သွင်းအားစုများ

1. **Agenda proposals.** တစ်ခု သို့မဟုတ် တစ်ခုထက်ပိုသော ဖိုင်များ
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md)။
   သူတို့ကို `--proposal <path>` ဖြင့် ပြတ်သားစွာ ဖြတ်ပါ သို့မဟုတ် အမိန့်ကို ညွှန်ပြပါ။
   ထိုလမ်းကြောင်းအောက်ရှိ `--proposal-dir <dir>` နှင့် `*.json` ဖိုင်တိုင်း၊
   ပါဝင်ပါသည်။
2. **Registry ကိုပွားခြင်း (ချန်လှပ်ထားနိုင်သည်)။** JSON ဖိုင်နှင့် ကိုက်ညီသော
   `docs/examples/ministry/agenda_duplicate_registry.json`။ ပဋိပက္ခတွေဖြစ်ကြတယ်။
   `source = "duplicate_registry"` အောက်တွင် အစီရင်ခံထားသည်။
3. **မူဝါဒ လျှပ်တစ်ပြက်ပုံ (ချန်လှပ်ထားနိုင်သည်)။** တစ်ခုချင်းစီကို စာရင်းပြုစုထားသော ပေါ့ပါးသော မန်နီးဖက်စ်တစ်ခု
   GAR/ဝန်ကြီးဌာန မူဝါဒအရ လက်ဗွေရာကို ပြဋ္ဌာန်းထားပြီးဖြစ်သည်။ loader က မျှော်လင့်ပါတယ်။
   အောက်တွင်ဖော်ပြထားသော schema (ကြည့်ပါ။
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   နမူနာအပြည့်အစုံအတွက်):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

`hash_family:hash_hex` လက်ဗွေရာ အဆိုပြုချက် ပစ်မှတ်နှင့် ကိုက်ညီသည့် မည်သည့် ဝင်ခွင့်မဆို
ရည်ညွှန်းထားသော `policy_id` ဖြင့် `source = "policy_snapshot"` အောက်တွင် အစီရင်ခံထားသည်။

##အသုံးပြုမှု

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

နောက်ထပ် အဆိုပြုချက်များကို ထပ်ခါတလဲလဲ `--proposal` အလံများမှတစ်ဆင့် သို့မဟုတ် ဖြည့်စွက်နိုင်ပါသည်။
ပြည်လုံးကျွတ်ဆန္ဒခံယူပွဲအသုတ်ပါရှိသော လမ်းညွှန်ကို ပံ့ပိုးပေးသည်-

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

`--out` ကို ချန်လှပ်ထားသောအခါတွင် ထုတ်လုပ်လိုက်သော JSON အား ပြင်းထန်စေရန် အမိန့်ပေးသည်။

## အထွက်

အစီရင်ခံစာသည် လက်မှတ်ရေးထိုးထားသော လက်ရာတစ်ခုဖြစ်သည် (၎င်းကို ဆန္ဒခံယူပွဲ ထုပ်ပိုးမှုအောက်တွင် မှတ်တမ်းတင်ထားသည်။
`artifacts/ministry/impact/` လမ်းညွှန်) အောက်ပါဖွဲ့စည်းပုံနှင့်။

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

ဤ JSON ကို ကြားနေအကျဉ်းချုပ်နှင့်အတူ ဆန္ဒခံယူပွဲတိုင်းတွင် ပူးတွဲပါ
ပါဝင်ဆောင်ရွက်သူများ၊ တရားသူကြီးများနှင့် အုပ်ချုပ်ရေးလေ့လာသူများသည် ပေါက်ကွဲမှုအချင်းဝက်၏ အတိအကျကို တွေ့မြင်နိုင်သည်။
အဆိုပြုချက်တစ်ခုစီ။ အထွက်အား အဆုံးအဖြတ်ပေးသည် ( hash မိသားစုဖြင့် စီထားသည်) နှင့် ဘေးကင်းသည်။
CI/runbooks များတွင် ပါ၀င်သည် ။ ပွားနေသော မှတ်ပုံတင်ခြင်း သို့မဟုတ် မူဝါဒလျှပ်တစ်ပြက်တွင် ပြောင်းလဲပါက၊
အမိန့်ကို ပြန်ဖွင့်ပြီး မဲမဖွင့်မီ ပြန်လည်ဆန်းသစ်ထားသော အနုပညာပစ္စည်းများကို ပူးတွဲပါ။

> **နောက်တစ်ဆင့်-** ထုတ်ပေးထားသော အကျိုးသက်ရောက်မှုအစီရင်ခံစာကို ထည့်သွင်းပါ။
> [`cargo xtask ministry-panel packet`](referendum_packet.md) ဒါကြောင့်
> `ReferendumPacketV1` dossier တွင် hash-family breakdown နှင့် the နှစ်ခုလုံးပါရှိသည်။
> ပြန်လည်သုံးသပ်နေသည့် အဆိုပြုချက်အတွက် အသေးစိတ်ပဋိပက္ခစာရင်း။