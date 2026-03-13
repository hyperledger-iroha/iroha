---
id: staging-manifest-playbook
lang: my
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

## ခြုံငုံသုံးသပ်ချက်

ဤပြခန်းစာအုပ်သည် ထုတ်လုပ်မှုသို့ ပြောင်းလဲခြင်းအား မြှင့်တင်ခြင်းမပြုမီ အဆင့် Torii ဖြန့်ကျက်မှုတွင် ပါလီမန်မှ အတည်ပြုထားသော အစုအဝေးပရိုဖိုင်ကို ဖွင့်ပေးခြင်းဖြင့် လမ်းညွှန်ပေးပါသည်။ ၎င်းသည် SoraFS အုပ်ချုပ်မှုပဋိဉာဉ်ကို အတည်ပြုပြီးဖြစ်ပြီး ကာနိုနီဆိုင်ရာပစ္စည်းများကို သိုလှောင်ခန်းတွင် ရနိုင်သည်ဟု ၎င်းကဆိုသည်။

## 1. ကြိုတင်လိုအပ်ချက်များ

1. Canonical fixtures များနှင့် လက်မှတ်များကို ထပ်တူပြုပါ-

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. စတင်ချိန်တွင် Torii ဖတ်မည့် ဝင်ခွင့်စာအိတ်လမ်းညွှန်ကို ပြင်ဆင်ပါ (ဥပမာ လမ်းကြောင်း)- `/var/lib/iroha/admission/sorafs`။
3. Torii config သည် ရှာဖွေတွေ့ရှိမှု ကက်ရှ်နှင့် ဝင်ခွင့်ဆိုင်ရာ ပြဋ္ဌာန်းချက်များကို ဖွင့်ထားကြောင်း သေချာစေသည်-

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. ဝင်ခွင့်စာအိတ်များကို ထုတ်ဝေပါ။

1. အတည်ပြုထားသော ဝန်ဆောင်မှုပေးသူ ဝင်ခွင့်စာအိတ်များကို `torii.sorafs.discovery.admission.envelopes_dir` ရည်ညွှန်းထားသော လမ်းညွှန်ထဲသို့ ကူးယူပါ-

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii ကို ပြန်လည်စတင်ပါ (သို့မဟုတ် အကယ်၍ သင်သည် loader ကို on-the-fly reload ဖြင့်ပတ်ထားလျှင် SIGHUP ပေးပို့ပါ)။
3. ဝင်ခွင့်မက်ဆေ့ဂျ်များအတွက် မှတ်တမ်းများကို မှတ်သားပါ-

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Discovery Propagation ကို အတည်ပြုပါ။

1. သင့်မှထုတ်လုပ်သော ဝန်ဆောင်မှုပေးသူ၏ ကြော်ငြာပေးချေမှု (Norito bytes) ကို ပို့စ်တင်ပါ
   ပံ့ပိုးပေးသူ ပိုက်လိုင်း-

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. ရှာဖွေတွေ့ရှိမှု အဆုံးမှတ်ကို မေးမြန်းပြီး ကြော်ငြာကို canonical aliases ဖြင့် အတည်ပြုပါ-

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   `profile_aliases` တွင် `"sorafs.sf1@1.0.0"` ပါဝင်ကြောင်း သေချာပါစေ။

## 4. လေ့ကျင့်ခန်းဖော်ပြချက်နှင့် အဆုံးအဖြတ်များကို စီစဉ်ပါ။

1. မန်နီးဖက်စ် မက်တာဒေတာကို ရယူပါ (ဝင်ခွင့်ကို ပြဌာန်းပါက ထုတ်လွှင့်မှု တိုကင်တစ်ခု လိုအပ်သည်-

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON အထွက်ကို စစ်ဆေးပြီး အတည်ပြုပါ-
   - `chunk_profile_handle` သည် `sorafs.sf1@1.0.0` ဖြစ်သည်။
   - `manifest_digest_hex` သည် သတ်မှတ်ပြဋ္ဌာန်းချက်အစီရင်ခံစာနှင့် ကိုက်ညီသည်။
   - `chunk_digests_blake3` သည် ပြန်လည်ထုတ်လုပ်ထားသော ပစ္စည်းများနှင့် ချိန်ညှိပါ။

## 5. Telemetry Checks

- Prometheus သည် ပရိုဖိုင်မက်ထရစ်အသစ်များကို ဖော်ထုတ်အတည်ပြုသည်-

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ဒက်ရှ်ဘုတ်များသည် ပရိုဖိုင်ကိုဖွင့်နေချိန်တွင် မျှော်လင့်ထားသည့်အမည်များအောက်တွင် အဆင့်သတ်မှတ်ပေးသူကိုပြသသင့်ပြီး အညိုရောင်ကောင်တာများကို သုညတွင်ထားပါ။

## 6. စတင်ပြင်ဆင်မှု

1. URLs များ၊ manifest ID နှင့် telemetry snapshot များဖြင့် အစီရင်ခံစာတိုကို ရိုက်ကူးပါ။
2. အစီရင်ခံစာကို စီစဉ်ထုတ်လုပ်သည့် အသက်သွင်းမှုဝင်းဒိုးနှင့်အတူ Nexus ထုတ်လွှင့်သည့်ချန်နယ်တွင် မျှဝေပါ။
3. သက်ဆိုင်သူများ လက်မှတ်ထိုးပြီးသည်နှင့် ထုတ်လုပ်မှုစစ်ဆေးမှုစာရင်း (`chunker_registry_rollout_checklist.md`) တွင် အပိုင်း 4 သို့ ဆက်လက်ဆောင်ရွက်ပါ။

ဤပြခန်းစာအုပ်ကို အပ်ဒိတ်လုပ်ထားပါက အစုအဝေး/ဝင်ခွင့် ဖြန့်ချိမှုတိုင်းသည် အဆင့်သတ်မှတ်ခြင်းနှင့် ထုတ်လုပ်ခြင်းတစ်လျှောက် တူညီသော အဆုံးအဖြတ်ပေးသည့် အဆင့်များကို လိုက်နာကြောင်း သေချာစေပါသည်။