---
id: runbooks-index
lang: my
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> `docs/source/sorafs/runbooks/` အောက်တွင်နေထိုင်သော ပိုင်ရှင်စာရင်းကို မှန်ကြည့်သည်။
> SoraFS လည်ပတ်မှုလမ်းညွှန်အသစ်တိုင်းကို ၎င်းကိုထုတ်ဝေပြီးသည်နှင့် ဤနေရာတွင် လင့်ခ်ချိတ်ရပါမည်။
> ပေါ်တယ်တည်ဆောက်ခြင်း။

မည်သည့် runbooks များမှ ပြောင်းရွှေ့ခြင်းကို အပြီးသတ်ကြောင်း အတည်ပြုရန် ဤစာမျက်နှာကို အသုံးပြုပါ။
အရင်းအမြစ်လမ်းကြောင်း နှင့် ပေါ်တယ်ကော်ပီကို ဝေဖန်သုံးသပ်သူများသည် အလိုရှိရာသို့ တည့်တည့်ခုန်တက်နိုင်သည်။
beta အစမ်းကြည့်ရှုမှုအတွင်း လမ်းညွှန်။

## ဘီတာ အစမ်းကြည့်ရှုမှု အစီအစဉ်

ယခု DocOps လှိုင်းသည် ဝေဖန်သုံးသပ်သူ-အတည်ပြုထားသော ဘီတာအစမ်းကြည့်ရှုမှုဌာနကို မြှင့်တင်လိုက်ပါပြီ။
`https://docs.iroha.tech/`။ အော်ပရေတာများ သို့မဟုတ် ဝေဖန်သုံးသပ်သူများကို ပြောင်းရွှေ့ထားသည့်နေရာသို့ ညွှန်ပြသည့်အခါ
runbook၊ ထို hostname ကို ကိုးကား၍ checksum-gated portal ကို ကျင့်သုံးပါ။
လျှပ်တစ်ပြက်။ ထုတ်ဝေခြင်း/ပြန်လှည့်ခြင်းဆိုင်ရာ လုပ်ထုံးလုပ်နည်းများ တိုက်ရိုက်ပါဝင်ပါသည်။
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)။

| Runbook | ပိုင်ရှင်(များ) | Portal မိတ္တူ | အရင်းအမြစ် |
|---------|----------|----------------|--------|
| Gateway & DNS kickoff | ကွန်ရက်ချိတ်ဆက်ခြင်း TL၊ Ops အလိုအလျောက်စနစ်၊ Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS operations playbook | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| စွမ်းရည်ပြန်လည်သင့်မြတ်ရေး | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| မှတ်ပုံတင်ခြင်း ops | ပင်ထိုးပါ။ Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Node လည်ပတ်မှုများ စစ်ဆေးရန်စာရင်း | သိုလှောင်ရေးအဖွဲ့၊ SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| အငြင်းပွားမှုနှင့် ပြန်လည်ရုပ်သိမ်းခြင်းဆိုင်ရာ စာအုပ်ချုပ် | အုပ်ချုပ်ရေးကောင်စီ | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Staging manifest playbook | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai မျိုးရိုးမြင်နိုင်စွမ်း | Media Platform WG/DA Program/ Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## စိစစ်ရေးစာရင်း

- [x] ဤအညွှန်း (sidebar entry) သို့ Portal တည်ဆောက်လင့်ခ်များ။
- [x] ရွှေ့ပြောင်းထားသော runbook တိုင်းသည် ဝေဖန်သုံးသပ်သူများထားရှိရန် canonical source path ကို စာရင်းပြုစုထားသည်။
  doc သုံးသပ်ချက်အတွင်း ချိန်ညှိထားသည်။
- [x] စာရင်းပြုစုထားသော runbook ပျောက်ဆုံးသွားသောအခါ DocOps အစမ်းကြည့်ရှုသည့်ပိုက်လိုင်းသည် ပေါင်းစည်းခြင်းကို ပိတ်ဆို့ထားသည်။
  portal output မှ။

အနာဂတ် ရွှေ့ပြောင်းနေထိုင်မှုများ (ဥပမာ၊ ပရမ်းပတာလေ့ကျင့်မှုအသစ်များ သို့မဟုတ် အုပ်ချုပ်မှုနောက်ဆက်တွဲများ) ထည့်သွင်းသင့်သည်
အပေါ်က ဇယားကို အတန်းလိုက်လုပ်ပြီး မြှုပ်ထားတဲ့ DocOps checklist ကို အပ်ဒိတ်လုပ်ပါ။
`docs/examples/docs_preview_request_template.md`။