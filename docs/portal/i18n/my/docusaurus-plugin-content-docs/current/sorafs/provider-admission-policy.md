---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

#SoraFS ဝန်ဆောင်မှုပေးသူ ဝင်ခွင့်နှင့် အထောက်အထားမူဝါဒ (SF-2b မူကြမ်း)

**SF-2b** အတွက် အရေးယူနိုင်သော ပေးပို့မှုများကို ဖမ်းယူထားပါသည်- သတ်မှတ်ခြင်းနှင့်
ဝင်ခွင့်လုပ်ငန်းအသွားအလာ၊ အထောက်အထားလိုအပ်ချက်များနှင့် သက်သေအထောက်အထားတို့ကို ပြဋ္ဌာန်းခြင်း။
SoraFS သိုလှောင်မှုပံ့ပိုးပေးသူများအတွက် payloads။ ၎င်းသည် အဆင့်မြင့်လုပ်ငန်းစဉ်ကို ချဲ့ထွင်သည်။
SoraFS Architecture RFC တွင် အကွပ်ပြပြီး ကျန်ရှိသော အလုပ်များကို ခွဲထုတ်သည်။
ခြေရာခံနိုင်သော အင်ဂျင်နီယာအလုပ်များ။

## မူဝါဒပန်းတိုင်

- စစ်ဆေးထားသော အော်ပရေတာများသာ `ProviderAdvertV1` မှတ်တမ်းများကို ထုတ်ဝေနိုင်သည်ကို သေချာပါစေ။
  ကွန်ရက်က လက်ခံပါလိမ့်မယ်။
- ကြော်ငြာသော့တိုင်းကို အုပ်ချုပ်မှုမှ အတည်ပြုထားသော အထောက်အထားစာရွက်စာတမ်းတစ်ခုနှင့် ချိတ်ပါ၊
  သက်သေအထောက်အထားများနှင့် အနည်းဆုံး လောင်းကြေးပံ့ပိုးမှု။
- အဆုံးအဖြတ်ပေးသော အတည်ပြုကိရိယာကို ပေးဆောင်ပါ ထို့ကြောင့် Torii၊ ဂိတ်ဝေးများနှင့်
  `sorafs-node` တူညီသောစစ်ဆေးမှုများကို ပြဋ္ဌာန်းပါ။
- သတ်မှတ်ပြဋ္ဌာန်းချက်ကို မချိုးဖောက်ဘဲ သက်တမ်းတိုးခြင်းနှင့် အရေးပေါ် ရုပ်သိမ်းခြင်းကို ပံ့ပိုးပါ။
  ကိရိယာတန်ဆာပလာ ergonomics ။

## Identity & Stake လိုအပ်ချက်များ

| လိုအပ်ချက် | ဖော်ပြချက် | ပေးပို့နိုင်သော |
|-------------|----------------|-------------|
| ကြော်ငြာသော့သက်သေ | ဝန်ဆောင်မှုပေးသူများသည် ကြော်ငြာတိုင်းကို လက်မှတ်ရေးထိုးသည့် Ed25519 သော့တွဲတစ်ခုကို မှတ်ပုံတင်ရပါမည်။ ဝင်ခွင့်အတွဲတွင် အုပ်ချုပ်မှုလက်မှတ်နှင့်အတူ အများသူငှာသော့ကို သိမ်းဆည်းထားသည်။ | `ProviderAdmissionProposalV1` schema ကို `advert_key` (32 bytes) ဖြင့် တိုးချဲ့ပြီး registry (`sorafs_manifest::provider_admission`) မှ ကိုးကားပါ။ |
| လောင်းကြေးညွှန်း | ဝင်ခွင့်သည် သုညမဟုတ်သော `StakePointer` တက်ကြွသောလောင်းကြေးထည့်သည့်ရေကန်ကို ညွှန်ပြရန် လိုအပ်သည်။ | `sorafs_manifest::provider_advert::StakePointer::validate()` တွင် တရားဝင်အတည်ပြုခြင်းနှင့် CLI/tests များတွင် မျက်နှာပြင်အမှားအယွင်းများကို ထည့်ပါ။ |
| တရားစီရင်ပိုင်ခွင့် tags | ဝန်ဆောင်မှုပေးသူများသည် စီရင်ပိုင်ခွင့်အာဏာ + တရားဝင်ဆက်သွယ်မှုကို ကြေညာသည်။ | `jurisdiction_code` (ISO 3166-1 alpha-2) နှင့် ရွေးချယ်နိုင်သော `contact_uri` ဖြင့် အဆိုပြုချက်ပုံစံကို တိုးချဲ့ပါ။ |
| အဆုံးမှတ် ထောက်ခံချက် | ကြော်ငြာထားသော အဆုံးမှတ်တစ်ခုစီကို mTLS သို့မဟုတ် QUIC လက်မှတ်အစီရင်ခံစာက ကျောထောက်နောက်ခံပေးရပါမည်။ | `EndpointAttestationV1` Norito အား ဝန်ခံချက်အစုအဝေးအတွင်း အဆုံးမှတ်တစ်ခုအတွက် သိမ်းဆည်းသတ်မှတ်ပါ။ |

## ဝင်ခွင့်အလုပ်အသွားအလာ

1. **အဆိုပြုချက် ဖန်တီးခြင်း**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …` ထည့်ပါ။
     `ProviderAdmissionProposalV1` + သက်သေအတွဲကို ထုတ်လုပ်သည်။
   - အတည်ပြုခြင်း- `profile_id` တွင် လိုအပ်သော အကွက်များ၊ လောင်းကြေး > 0၊ canonical chunker လက်ကိုင်ကို သေချာပါစေ။
2. **အုပ်ချုပ်မှု ထောက်ခံချက်**
   - လက်ရှိအသုံးပြုနေသော ကောင်စီ ဆိုင်းဘုတ်များ `blake3("sorafs-provider-admission-v1" || canonical_bytes)`
     စာအိတ်ကိရိယာတန်ဆာပလာ (`sorafs_manifest::governance` မော်ဂျူး)။
   - စာအိတ်ကို `governance/providers/<provider_id>/admission.json` တွင် ဆက်လက်တည်ရှိနေပါသည်။
3. **Registry ထည့်သွင်းခြင်း**
   - မျှဝေထားသော အတည်ပြုစနစ် (`sorafs_manifest::provider_admission::validate_envelope`) ကို အကောင်အထည်ဖော်ပါ။
     အဲဒါက Torii/gateways/CLI ကို ပြန်သုံးတယ်။
   - စာအိတ်နှင့် သက်တမ်းကုန်ဆုံးမှု ကွဲပြားသည့် ကြော်ငြာများကို ငြင်းပယ်ရန် Torii ဝင်ခွင့်လမ်းကြောင်းကို အပ်ဒိတ်လုပ်ပါ။
4. **သက်တမ်းတိုးခြင်းနှင့် ရုပ်သိမ်းခြင်း**
   - ရွေးချယ်နိုင်သော အဆုံးမှတ်/လောင်းကြေးမွမ်းမံမှုများဖြင့် `ProviderAdmissionRenewalV1` ကို ထည့်ပါ။
   - ရုပ်သိမ်းခြင်းအကြောင်းရင်းကို မှတ်တမ်းတင်ပြီး အုပ်ချုပ်မှုဖြစ်ရပ်ကို တွန်းအားပေးသည့် `--revoke` CLI လမ်းကြောင်းကို ဖော်ထုတ်ပါ။

## အကောင်အထည်ဖော်ရေး လုပ်ငန်းတာဝန်များ

| ဧရိယာ | တာဝန် | ပိုင်ရှင်(များ) | အဆင့်အတန်း |
|------|------|----------------|--------|
| Schema | `ProviderAdmissionProposalV1`၊ `ProviderAdmissionEnvelopeV1`၊ `EndpointAttestationV1` (I18NT0000001X) ကို `crates/sorafs_manifest/src/provider_admission.rs` အောက်တွင် သတ်မှတ်ပါ။ `sorafs_manifest::provider_admission` တွင် အတည်ပြုပေးသော အကူအညီများဖြင့် အကောင်အထည်ဖော်ထားသည်။【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | သိမ်းဆည်း/အုပ်ချုပ်မှု | ✅ ပြီးစီး |
| CLI tooling | `sorafs_manifest_stub` ကို ကွန်မန်းခွဲများဖြင့် တိုးချဲ့ပါ- `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`။ | Tooling WG | ✅ |

ယခု CLI စီးဆင်းမှုသည် အလယ်အလတ်လက်မှတ်အစုအဝေးများ (`--endpoint-attestation-intermediate`) ကို လက်ခံသည်
Canonical proposal/ envelope bytes နှင့် `sign`/`verify` ကာလအတွင်း ကောင်စီ၏ လက်မှတ်များကို သက်သေပြပါသည်။ အော်ပရေတာတွေ လုပ်နိုင်ပါတယ်။
ကြော်ငြာအဖွဲ့များကို တိုက်ရိုက်ပေးဆောင်ပါ သို့မဟုတ် လက်မှတ်ထိုးထားသော ကြော်ငြာများကို ပြန်လည်အသုံးပြုကာ တွဲချိတ်ခြင်းဖြင့် လက်မှတ်ဖိုင်များကို ပံ့ပိုးပေးနိုင်ပါသည်။
အော်တိုမက်စဖော်ရွေမှုအတွက် `--council-signature-public-key` နှင့် `--council-signature-file`။

### CLI အကိုးအကား

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …` မှတဆင့် command တစ်ခုစီကို run ပါ။

- `proposal`
  - လိုအပ်သောအလံများ- `--provider-id=<hex32>`၊ `--chunker-profile=<namespace.name@semver>`၊
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`၊
    `--jurisdiction-code=<ISO3166-1>` နှင့် အနည်းဆုံး `--endpoint=<kind:host>`။
  - အဆုံးမှတ်သက်သေတစ်ခုသည် `--endpoint-attestation-attested-at=<secs>` မျှော်မှန်းထားသည်
    `--endpoint-attestation-expires-at=<secs>` မှတဆင့် လက်မှတ်တစ်ခု
    `--endpoint-attestation-leaf=<path>` (အပြင် စိတ်ကြိုက်ရွေးချယ်နိုင်သော `--endpoint-attestation-intermediate=<path>`
    ကွင်းဆက်ဒြပ်စင်တစ်ခုစီအတွက်) နှင့် ညှိနှိုင်းထားသော ALPN ID များ
    (`--endpoint-attestation-alpn=<token>`)။ QUIC အဆုံးမှတ်များသည် သယ်ယူပို့ဆောင်ရေးအစီရင်ခံစာများနှင့်အတူ ပံ့ပိုးပေးနိုင်ပါသည်။
    `--endpoint-attestation-report[-hex]=…`။
  - ထုတ်ယူမှု- Canonical Norito အဆိုပြုချက် ဘိုက်များ (`--proposal-out`) နှင့် JSON အနှစ်ချုပ်
    (မူရင်း stdout သို့မဟုတ် `--json-out`)။
- `sign`
  - ထည့်သွင်းမှုများ- အဆိုပြုချက် (`--proposal`)၊ လက်မှတ်ရေးထိုးထားသော ကြော်ငြာတစ်ခု (`--advert`)၊ ရွေးချယ်နိုင်သော ကြော်ငြာကိုယ်ထည်
    (`--advert-body`)၊ ထိန်းသိမ်းမှုကာလ၊ နှင့် အနည်းဆုံး ကောင်စီလက်မှတ်တစ်ခု။ လက်မှတ်များ ပေးနိုင်သည်။
    inline (`--council-signature=<signer_hex:signature_hex>`) သို့မဟုတ် ပေါင်းစပ်ခြင်းဖြင့် ဖိုင်များမှတဆင့်
    `--council-signature-public-key` နှင့် `--council-signature-file=<path>`။
  - အတည်ပြုထားသော စာအိတ် (`--envelope-out`) နှင့် စုစည်းမှုများကို ညွှန်ပြသော JSON အစီရင်ခံစာကို ထုတ်လုပ်သည်၊
    လက်မှတ်ထိုးသူအရေအတွက်နှင့် ဖြည့်သွင်းလမ်းကြောင်းများ။
- `verify`
  - ရှိပြီးသားစာအိတ် (`--envelope`) နှင့် ကိုက်ညီသော အဆိုပြုချက်အား စစ်ဆေးခြင်းကို ရွေးချယ်နိုင်သည်၊
    ကြော်ငြာ၊ သို့မဟုတ် ကြော်ငြာကိုယ်ထည်။ JSON အစီရင်ခံစာသည် အနှစ်ချုပ်တန်ဖိုးများ၊ လက်မှတ်အတည်ပြုခြင်းအခြေအနေ၊
    နှင့် မည်သည့်ရွေးချယ်ခွင့်ရှိသော ပစ္စည်းများနှင့် ကိုက်ညီသနည်း။
- `renewal`
  - ယခင်အတည်ပြုထားသောစာအိတ်နှင့် အသစ်အတည်ပြုထားသောစာအိတ်ကို လင့်ခ်ချိတ်ပါ။ လိုအပ်ပါသည်။
    `--previous-envelope=<path>` နှင့် ဆက်ခံသူ `--envelope=<path>` (Norito နှစ်ခုစလုံး)။
    CLI သည် ပရိုဖိုင်အမည်တူများ၊ လုပ်ဆောင်နိုင်စွမ်းများနှင့် ကြော်ငြာသော့များ မပြောင်းလဲကြောင်း အတည်ပြုပါသည်။
    လောင်းကြေး၊ အဆုံးမှတ်များနှင့် မက်တာဒေတာ အပ်ဒိတ်များကို ခွင့်ပြုခြင်း။ Canonical ကို ထုတ်ပေးသည်။
    `ProviderAdmissionRenewalV1` bytes (`--renewal-out`) နှင့် JSON အနှစ်ချုပ်။
- `revoke`
  - စာအိတ်လိုအပ်သောဝန်ဆောင်မှုပေးသူအတွက် အရေးပေါ် `ProviderAdmissionRevocationV1` အတွဲကို ထုတ်ပေးသည်
    ရုပ်သိမ်းခံရ။ `--envelope=<path>`၊ `--reason=<text>`၊ အနည်းဆုံး တစ်ခု လိုအပ်သည်
    `--council-signature` နှင့် ရွေးချယ်နိုင်သော `--revoked-at`/`--notes`။ CLI သည် နိမိတ်လက္ခဏာများနှင့် အတည်ပြုသည်။
    ပြန်လည်ရုတ်သိမ်းခြင်းဆိုင်ရာ အနှစ်ချုပ်၊ Norito payload ကို `--revocation-out` မှတစ်ဆင့် ရေးသားပြီး JSON အစီရင်ခံစာကို ပရင့်ထုတ်သည်
    အချေအတင်နှင့် လက်မှတ်အရေအတွက်ကို ဖမ်းယူခြင်း။
| အတည်ပြုခြင်း | Torii၊ ဂိတ်ဝေးများနှင့် `sorafs-node` အသုံးပြုထားသော မျှဝေထားသော အတည်ပြုစနစ်ကို အကောင်အထည်ဖော်ပါ။ ယူနစ် + CLI ပေါင်းစပ်စမ်းသပ်မှုများကို ပံ့ပိုးပေးပါသည်။【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | ကွန်ရက်ချိတ်ဆက်ခြင်း TL / Storage | ✅ ပြီးစီး |
| Torii ပေါင်းစပ်မှု | Torii တွင် ကြော်ငြာထည့်သွင်းခြင်း၊ မူဝါဒပြင်ပကြော်ငြာများကို ငြင်းပယ်ခြင်း၊ တယ်လီမီတာထုတ်ခြင်း | ကွန်ရက်ချိတ်ဆက်ခြင်း TL | ✅ ပြီးစီး | ယခု Torii သည် အုပ်ချုပ်မှုစာအိတ်များ (`torii.sorafs.admission_envelopes_dir`) ကို စားသုံးနေစဉ်အတွင်း အချေအတင်/လက်မှတ် ကိုက်ညီမှုများကို စစ်ဆေးပြီး မျက်နှာပြင်များ ဝင်ခွင့်ပေးသည် telemetry
| သက်တမ်းတိုး | သက်တမ်းတိုးခြင်း/ပြန်လည်ရုတ်သိမ်းခြင်းအစီအစဉ် + CLI အထောက်အကူများထည့်ပါ၊ မှတ်တမ်းစာအုပ်တွင် ဘဝသံသရာလမ်းညွှန်ကို ထုတ်ဝေပါ (အောက်ပါစာအုပ်နှင့် CLI ညွှန်ကြားချက်များတွင် ကြည့်ပါ `provider-admission renewal`/`revoke`)။【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | သိမ်းဆည်း/အုပ်ချုပ်မှု | ✅ ပြီးစီး |
| Telemetry | `provider_admission` ဒိုင်ခွက်များနှင့် သတိပေးချက်များ (သက်တမ်းတိုးခြင်း၊ စာအိတ်သက်တမ်းကုန်ဆုံးခြင်း) ကို သတ်မှတ်ပါ။ | မြင်နိုင်စွမ်း | 🟠 လုပ်ဆောင်နေဆဲ | ကောင်တာ `torii_sorafs_admission_total{result,reason}` ရှိပြီး၊ dashboards/alerts ဆိုင်းငံ့ထားသည်။【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### သက်တမ်းတိုးခြင်းနှင့် ရုတ်သိမ်းခြင်းဆိုင်ရာ စာအုပ်

#### စီစဉ်ထားသော သက်တမ်းတိုးခြင်း (လောင်းကြေး/ထိပ်ပိုင်းဗေဒ အပ်ဒိတ်များ)
1. `provider-admission proposal` နှင့် `provider-admission sign` ဖြင့် ဆက်ခံမည့် အဆိုပြုချက်/ကြော်ငြာအတွဲကို တည်ဆောက်ပါ၊ `--retention-epoch` ကို တိုးမြှင့်ပြီး လိုအပ်သလို လောင်းကြေး/အဆုံးမှတ်များကို အပ်ဒိတ်လုပ်ပါ။
2. အကောင်အထည်ဖော်ပါ။  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   အမိန့်သည် မပြောင်းလဲနိုင်သော စွမ်းရည်/ပရိုဖိုင် အကွက်များမှတစ်ဆင့် မှန်ကန်ကြောင်း အတည်ပြုသည်။
   `AdmissionRecord::apply_renewal`၊ `ProviderAdmissionRenewalV1` ကို ထုတ်လွှတ်ပြီး အချေအတင်များကို ပရင့်ထုတ်သည်
   အုပ်ချုပ်မှုမှတ်တမ်း။【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` ရှိ ယခင်စာအိတ်ကို အစားထိုးပါ၊ သက်တမ်းတိုးခြင်း Norito/JSON ကို အုပ်ချုပ်မှုသိုလှောင်ရာသို့ အပ်နှံပြီး သက်တမ်းတိုး hash + retention အပိုင်းကို `docs/source/sorafs/migration_ledger.md` သို့ ပေါင်းထည့်ပါ။
4. စာအိတ်အသစ်သည် တိုက်ရိုက်လွှင့်နေပြီး `torii_sorafs_admission_total{result="accepted",reason="stored"}` ကို ထည့်သွင်းအတည်ပြုရန် အော်ပရေတာများအား အကြောင်းကြားပါ။
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` မှတစ်ဆင့် canonical fixtures များကို ပြန်လည်ထုတ်ပေးပြီး ကတိပြုပါ။ CI (`ci/check_sorafs_fixtures.sh`) သည် Norito အထွက်များ တည်ငြိမ်နေစေရန် အတည်ပြုသည်။

#### အရေးပေါ်ရုတ်သိမ်းခြင်း။
1. အပေးအယူခံရသော စာအိတ်ကို ဖော်ထုတ်ပြီး ပြန်လည်ရုတ်သိမ်းကြောင်း ထုတ်ပြန်ပါ-
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI သည် `ProviderAdmissionRevocationV1` ကို ဆိုင်းဘုတ်တပ်ပြီး သတ်မှတ်လက်မှတ်ကို အတည်ပြုသည် ။
   `verify_revocation_signatures`၊ နှင့် ပြန်လည်ရုပ်သိမ်းခြင်းဆိုင်ရာ အစီရင်ခံတင်ပြပါသည်။ 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` မှ စာအိတ်ကို ဖယ်ရှားပါ၊ ရုပ်သိမ်းခြင်း Norito/JSON ကို ဝင်ခွင့် ကက်ရှ်များသို့ ဖြန့်ဝေပြီး အကြောင်းပြချက် hash ကို အုပ်ချုပ်မှု မိနစ်များတွင် မှတ်တမ်းတင်ပါ။
3. ရုတ်သိမ်းထားသော ကြော်ငြာကို ကက်ရှ်များ ချပေးကြောင်း အတည်ပြုရန် `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ကို ကြည့်ရှုပါ။ ရုပ်သိမ်းခြင်းဆိုင်ရာ ပစ္စည်းများကို အဖြစ်အပျက် နောက်ကြောင်းပြန်တွင် သိမ်းဆည်းပါ။

## စမ်းသပ်ခြင်းနှင့် Telemetry- ဝင်ခွင့်အဆိုပြုချက်များနှင့် စာအိတ်များအောက်တွင် ရွှေရောင်အစုံအလင်ထည့်ပါ။
  `fixtures/sorafs_manifest/provider_admission/`။
- အဆိုပြုချက်များကို ပြန်ထုတ်ပြီး စာအိတ်များကို စစ်ဆေးရန် CI (`ci/check_sorafs_fixtures.sh`) ကို တိုးချဲ့ပါ။
- ထုတ်လုပ်ထားသော ဆက်စပ်ပစ္စည်းများတွင် Canonical digests များဖြင့် `metadata.json` ပါဝင်သည်။ ရေအောက်ပိုင်း စမ်းသပ်မှုများ အခိုင်အမာ
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`။
- ပေါင်းစပ်စမ်းသပ်မှုများကို ပံ့ပိုးပေးသည်-
  - Torii သည် ပျောက်ဆုံးနေသော သို့မဟုတ် သက်တမ်းကုန်ဆုံးသွားသော ဝင်ခွင့်စာအိတ်များဖြင့် ကြော်ငြာများကို ငြင်းပယ်သည်။
  - CLI အသွားအပြန် အဆိုပြုချက် → စာအိတ် → အတည်ပြုခြင်း။
  - စီမံအုပ်ချုပ်မှုသက်တမ်းတိုးခြင်းသည် ဝန်ဆောင်မှုပေးသူ ID ကိုပြောင်းလဲခြင်းမရှိဘဲ အဆုံးမှတ်အတည်ပြုချက်ကို လှည့်ပတ်သည်။
- Telemetry လိုအပ်ချက်များ
  - Torii တွင် `provider_admission_envelope_{accepted,rejected}` ကောင်တာများကို ထုတ်လွှတ်ပါ။ ✅ `torii_sorafs_admission_total{result,reason}` ယခု လက်ခံ/ပယ်ချသော ရလဒ်များကို ပေါ်လွင်စေပါသည်။
  - ကြည့်ရှုနိုင်မှု ဒက်ရှ်ဘုတ်များတွင် သက်တမ်းကုန်ဆုံးမှုသတိပေးချက်များကို ပေါင်းထည့်ပါ (7 ရက်အတွင်း သက်တမ်းတိုးရန်)။

## နောက်အဆင့်များ

1. ✅ Norito schema အပြောင်းအလဲများကို အပြီးသတ်ပြီး validation helpers တွင် ရောက်ရှိခဲ့သည်
   `sorafs_manifest::provider_admission`။ အင်္ဂါရပ်အလံများမလိုအပ်ပါ။
2. ✅ CLI လုပ်ငန်းအသွားအလာများ (`proposal`၊ `sign`၊ `verify`၊ `renewal`၊ `revoke`) တို့ကို ပေါင်းစပ်စစ်ဆေးမှုများမှတစ်ဆင့် မှတ်တမ်းတင်ပြီး ကျင့်သုံးပါသည်။ အုပ်ချုပ်မှု script များကို runbook နှင့်ထပ်တူပြုထားပါ။
3. ✅ Torii ဝင်ခွင့်/ရှာဖွေတွေ့ရှိမှုသည် စာအိတ်များကို ထည့်သွင်းပြီး လက်ခံ/ငြင်းပယ်မှုအတွက် တယ်လီမီတာကောင်တာများကို ဖော်ထုတ်ပါ။
4. ကြည့်ရှုနိုင်မှုကို အာရုံစိုက်ပါ- ဝင်ခွင့်ဒိုင်ခွက်များ/သတိပေးချက်များကို အပြီးသတ်ပါ ထို့ကြောင့် ခုနစ်ရက်အတွင်း သက်တမ်းတိုးမှုများ သတိပေးချက်များ တိုးလာသည် (`torii_sorafs_admission_total`၊ သက်တမ်းကုန်တိုင်းထွာမှုများ)။