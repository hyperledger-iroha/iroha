---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

##ရည်ရွယ်ချက်

ဤအပြေးစာအုပ်သည် SoraFS စွမ်းရည်ဆိုင်ရာ အငြင်းပွားမှုများကို တင်သွင်းခြင်း၊ ပြန်လည်ရုတ်သိမ်းခြင်းများကို ညှိနှိုင်းဆောင်ရွက်ခြင်း၊ နှင့် ဒေတာ ရွှေ့ပြောင်းခြင်းတို့ကို တိကျပြတ်သားစွာ ပြီးမြောက်စေကြောင်း သေချာစေခြင်းဖြင့် အုပ်ချုပ်မှုအော်ပရေတာများကို လမ်းညွှန်ပေးပါသည်။

## 1. အဖြစ်အပျက်ကို အကဲဖြတ်ပါ။

- **အစပျိုးအခြေအနေများ-** SLA ချိုးဖောက်မှု (ဖွင့်ချိန်/PoR ချို့ယွင်းမှု)၊ ပုံတူကူးချမှု ပြတ်တောက်မှု သို့မဟုတ် ငွေပေးချေမှု ကွဲလွဲမှုကို ထောက်လှမ်းခြင်း။
- ** တယ်လီမီတာကို အတည်ပြုပါ-** ပံ့ပိုးပေးသူအတွက် `/v1/sorafs/capacity/state` နှင့် `/v1/sorafs/capacity/telemetry` လျှပ်တစ်ပြက်ရိုက်ချက်များ။
- **သက်ဆိုင်သူများကို အကြောင်းကြားရန်-** သိုလှောင်မှုအဖွဲ့ (ပံ့ပိုးပေးသူ၏ လုပ်ငန်းဆောင်ရွက်မှုများ)၊ အုပ်ချုပ်ရေးကောင်စီ (ဆုံးဖြတ်ချက်အဖွဲ့)၊ စောင့်ကြည့်နိုင်မှု (ဒက်ရှ်ဘုတ် အပ်ဒိတ်များ)။

## 2. Evidence Bundle ကို ပြင်ဆင်ပါ။

1. အကြမ်းထည်ပစ္စည်းများ (တယ်လီမီတာ JSON၊ CLI မှတ်တမ်းများ၊ စာရင်းစစ်မှတ်စုများ) စုဆောင်းပါ။
2. အဆုံးအဖြတ်ပေးသော မှတ်တမ်းအဖြစ် ပုံမှန်ပြုလုပ်ပါ (ဥပမာ၊ တာဘောတစ်လုံး); မှတ်တမ်း-
   - BLAKE3-256 digest (`evidence_digest`)
   - မီဒီယာအမျိုးအစား (`application/zip`၊ `application/jsonl` စသည်ဖြင့်)
   - URI ကို hosting လုပ်ခြင်း (အရာဝတ္ထုသိုလှောင်မှု၊ SoraFS ပင်နံပါတ် သို့မဟုတ် Torii-အသုံးပြုခွင့်ရှိသော အဆုံးမှတ်)
3. အစုအဝေးကို အုပ်ချုပ်ရေးဆိုင်ရာ အထောက်အထားများ စုဆောင်းရေးပုံးတွင် တစ်ကြိမ်စာရေးပြီး သိမ်းဆည်းပါ။

## 3. အငြင်းပွားမှုကို တင်သွင်းပါ။

1. `sorafs_manifest_stub capacity dispute` အတွက် spec JSON ဖန်တီးပါ-

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI ကိုဖွင့်ပါ-

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. ပြန်လည်သုံးသပ်ခြင်း `dispute_summary.json` (အမျိုးအစားကိုအတည်ပြုပါ၊ အထောက်အထားအနှစ်ချုပ်၊ အချိန်တံဆိပ်တုံးများ)။
4. တောင်းဆိုချက် JSON ကို Torii `/v1/sorafs/capacity/dispute` သို့ အုပ်ချုပ်မှုငွေပေးငွေယူတန်းစီမှတစ်ဆင့် တင်သွင်းပါ။ `dispute_id_hex` တုံ့ပြန်မှုတန်ဖိုးကို ဖမ်းယူပါ။ ၎င်းသည် နောက်ဆက်တွဲ ရုပ်သိမ်းခြင်းဆိုင်ရာ လုပ်ဆောင်ချက်များနှင့် စာရင်းစစ်အစီရင်ခံစာများကို ကျောက်ချထားသည်။

## 4. Evacuation & Revocation

1. **Grace Window:** ပြန်လည်ရုပ်သိမ်းခါနီးတွင် ဝန်ဆောင်မှုပေးသူကို အကြောင်းကြားပါ။ မူဝါဒခွင့်ပြုသည့်အခါတွင် ပင်ထိုးထားသောဒေတာများကို ဘေးလွတ်ရာသို့ ရွှေ့ပြောင်းခွင့်ပြုပါ။
2. **`ProviderAdmissionRevocationV1` ကို ထုတ်လုပ်ပါ-**
   - အတည်ပြုထားသောအကြောင်းပြချက်ဖြင့် `sorafs_manifest_stub provider-admission revoke` ကိုသုံးပါ။
   - လက်မှတ်များနှင့် ရုတ်သိမ်းခြင်းဆိုင်ရာ အချက်များကို အတည်ပြုပါ။
3. **ထုတ်ဝေမှု ရုပ်သိမ်းခြင်း-**
   - ရုပ်သိမ်းခြင်းတောင်းဆိုချက်ကို Torii သို့ ပေးပို့ပါ။
   - ဝန်ဆောင်မှုပေးသောကြော်ငြာများကိုပိတ်ဆို့ထားကြောင်းသေချာစေပါ (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ကိုတက်ရန်မျှော်လင့်သည်)။
4. **ဒက်ရှ်ဘုတ်များကို အပ်ဒိတ်လုပ်ပါ-** ဝန်ဆောင်မှုပေးသူကို ရုပ်သိမ်းလိုက်ကြောင်း အလံပြပြီး အငြင်းပွားမှု ID ကို ကိုးကားပြီး အထောက်အထားအစုအဝေးကို ချိတ်ဆက်ပါ။

## 5. Post-mortem & Follow-Up

- အုပ်ချုပ်မှုဖြစ်ရပ်ခြေရာခံစနစ်တွင် အချိန်ဇယား၊ အရင်းခံအကြောင်းတရားနှင့် ပြန်လည်ပြင်ဆင်ရေး လုပ်ဆောင်ချက်များကို မှတ်တမ်းတင်ပါ။
- ပြန်လည်ပေးအပ်ခြင်း (လောင်းကြေးဖြတ်တောက်ခြင်း၊ အခကြေးငွေပေးချေခြင်း၊ ဖောက်သည်ပြန်အမ်းငွေများ) ကို ဆုံးဖြတ်ပါ။
- စာရွက်စာတမ်းသင်ယူမှု; လိုအပ်ပါက SLA သတ်မှတ်ချက်များ သို့မဟုတ် စောင့်ကြည့်သတိပေးချက်များကို အပ်ဒိတ်လုပ်ပါ။

## 6. ရည်ညွှန်းပစ္စည်းများ

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (အငြင်းပွားမှုအပိုင်း)
- `docs/source/sorafs/provider_admission_policy.md` (ရုတ်သိမ်းခြင်းလုပ်ငန်းအသွားအလာ)
- မြင်နိုင်မှု ဒက်ရှ်ဘုတ်- `SoraFS / Capacity Providers`

## စစ်ဆေးရန်စာရင်း

- [ ] အထောက်အထားအစုအဝေးကို ဖမ်းမိပြီး ဖျက်လိုက်သည် ။
- [ ] စက်တွင်း၌ အတည်ပြုထားသော ပေးဆောင်မှု အငြင်းပွားမှု။
- [ ] Torii အငြင်းပွားမှု ငွေလွှဲခြင်းကို လက်ခံသည်။
- [ ] ရုပ်သိမ်းခြင်း (အတည်ပြုပါက) ကွပ်မျက်ခဲ့သည်။
- [ ] ဒက်ရှ်ဘုတ်များ/ပြေးစာအုပ်များကို အပ်ဒိတ်လုပ်ထားသည်။
- [ ] အလောင်းကို အုပ်ချုပ်ရေးကောင်စီသို့ တင်သွင်းသည်။