---
id: pin-registry-plan
lang: my
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

#SoraFS Pin Registry Implementation Plan (SF-4)

SF-4 သည် Pin Registry စာချုပ်နှင့် သိမ်းဆည်းသော ဝန်ဆောင်မှုများကို ပံ့ပိုးပေးသည်။
ကတိကဝတ်များကို ထင်ရှားစွာပြပါ၊ ပင်နံပါတ်မူဝါဒများကို ကျင့်သုံးရန်နှင့် APIs များကို Torii၊ ဂိတ်ဝေးများ၊
တီးမှုတ်သူများ၊ ဤစာတမ်းသည် အတည်ပြုခြင်းအစီအစဥ်အား တိကျခိုင်မာစွာဖြင့် ချဲ့ထွင်ထားသည်။
အကောင်အထည်ဖော်ခြင်းလုပ်ငန်းများ၊ ကွင်းဆက်ယုတ္တိ၊ အိမ်ရှင်ဘက်ဆိုင်ရာ ဝန်ဆောင်မှုများ၊ ပြင်ဆင်မှုများ၊
နှင့် လုပ်ငန်းဆောင်ရွက်မှု လိုအပ်ချက်များ။

## နယ်ပယ်

1. **Registry state machine**- Norito သည် manifests၊ aliases အတွက် သတ်မှတ်ထားသော မှတ်တမ်းများ၊
   ဆက်ခံသည့် ကွင်းဆက်များ၊ ထိန်းသိမ်းထားသည့် ခေတ်များနှင့် အုပ်ချုပ်မှု မက်တာဒေတာ။
2. **စာချုပ်အကောင်အထည်ဖော်ခြင်း**- pin lifecycle အတွက် အဆုံးအဖြတ်ပေးသော CRUD လုပ်ဆောင်ချက်များ
   (`ReplicationOrder`၊ `Precommit`၊ `Completion`၊ နှင်ထုတ်ခြင်း)။
3. **ဝန်ဆောင်မှုမျက်နှာစာ**- Torii ၏ မှတ်ပုံတင်မှုမှ ကျောထောက်နောက်ခံပြုထားသော gRPC/REST အဆုံးမှတ်များ
   pagination နှင့် အထောက်အထား အပါအဝင် SDK များကို စားသုံးပါသည်။
4. **Tooling & fixtures**- CLI helpers, test vectors, and documentation to keep
   ထင်ရှားသော၊ နာမည်တူနှင့် အုပ်ချုပ်မှုစာအိတ်များကို ထပ်တူပြုထားသည်။
5. **Telemetry & ops**- စာရင်းသွင်းကျန်းမာရေးအတွက် မက်ထရစ်များ၊ သတိပေးချက်များ၊ နှင့် runbooks။

## ဒေတာမော်ဒယ်

### Core Records (Norito)

| ဖွဲ့စည်းပုံ | ဖော်ပြချက် | လယ်ကွင်းများ |
|--------|----------------|--------|
| `PinRecordV1` | Canonical manifest ဝင်ရောက်မှု။ | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, I18NI00000010N0302,10NIX `governance_envelope_hash`။ |
| `AliasBindingV1` | Maps alias -> ထင်ရှားသော CID။ | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`။ |
| `ReplicationOrderV1` | မန်နီးဖက်စ်ကို ထိုးရန် ပံ့ပိုးပေးသူများအတွက် ညွှန်ကြားချက်။ | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`။ |
| `ReplicationReceiptV1` | ဝန်ဆောင်မှုပေးသူ အသိအမှတ်ပြုမှု။ | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`။ |
| `ManifestPolicyV1` | အုပ်ချုပ်မှုမူဝါဒ လျှပ်တစ်ပြက်။ | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`။ |

အကောင်အထည်ဖော်မှု ရည်ညွှန်းချက်- `crates/sorafs_manifest/src/pin_registry.rs` ကို ကြည့်ပါ။
Rust Norito schemas နှင့် validation helpers သည် ဤမှတ်တမ်းများကို ကျောထောက်နောက်ခံပြုသည်။ အတည်ပြုချက်
ထင်ရှားသော ကိရိယာတန်ဆာပလာ (chunker registry lookup၊ pin policy gating) ကို ထင်ဟပ်စေသည်။
စာချုပ်၊ Torii မျက်နှာစာများနှင့် CLI တို့သည် တူညီသောပုံစံကွဲများကို မျှဝေပါသည်။

လုပ်ဆောင်စရာများ-
- Norito အစီအစဉ်များကို `crates/sorafs_manifest/src/pin_registry.rs` တွင် အပြီးသတ်ပါ။
- Norito macro ကို အသုံးပြု၍ ကုဒ် (Rust + အခြား SDKs) ကို ဖန်တီးပါ။
- schemas land တစ်ကြိမ် (`sorafs_architecture_rfc.md`) ကို အပ်ဒိတ်လုပ်ပါ။

## စာချုပ်အကောင်အထည်ဖော်ခြင်း။

| တာဝန် | ပိုင်ရှင်(များ) | မှတ်စုများ |
|------|----------|-------|
| မှတ်ပုံတင်ခြင်းသိုလှောင်မှု (sled/sqlite/off-chain) သို့မဟုတ် စမတ်စာချုပ် module ကို အကောင်အထည်ဖော်ပါ။ | Core Infra / Smart Contract Team | အဆုံးအဖြတ်ပေးသည့် ဟတ်ချခြင်းကို ပံ့ပိုးပါ၊ ရေပေါ်အမှတ်ကို ရှောင်ကြဉ်ပါ။ |
| ဝင်ခွင့်အမှတ်များ- `submit_manifest`၊ `approve_manifest`၊ `bind_alias`၊ `issue_replication_order`၊ `complete_replication`၊ `evict_manifest`။ | Core Infra | အတည်ပြုခြင်းအစီအစဉ်မှ `ManifestValidator` ကို အသုံးချပါ။ Alias ​​binding သည် ယခုအခါ `RegisterPinManifest` (Torii DTO surfacing) မှတဆင့် စီးဆင်းနေပြီး `bind_alias` သည် ဆက်တိုက်မွမ်းမံမှုများအတွက် စီစဉ်ထားဆဲဖြစ်သည်။ |
| ပြည်နယ်အကူးအပြောင်းများ- ဆက်ခံမှုကို တွန်းအားပေးပါ (ဖော်ပြချက် A -> B)၊ ထိန်းသိမ်းမှုခေတ်၊ နာမည်တူ ထူးခြားမှု။ | အုပ်ချုပ်ရေးကောင်စီ / Core Infra | နာမည်တူ ထူးခြားမှု၊ ထိန်းသိမ်းမှု ကန့်သတ်ချက်များနှင့် ယခင်ခွင့်ပြုချက်/အငြိမ်းစားစစ်ဆေးမှုများသည် ယခု `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` တွင် နေထိုင်လျက်ရှိပါသည်။ multi-hop ဆက်ခံမှုကို ထောက်လှမ်းခြင်းနှင့် ထပ်တူပြုခြင်း စာရင်းရေးသွင်းခြင်းတို့ကို ဆက်လက်ဖွင့်ထားသည်။ |
| စီမံထားသော ကန့်သတ်ချက်များ- `ManifestPolicyV1` ကို config/governance state မှ ရယူပါ။ အုပ်ချုပ်မှုဖြစ်ရပ်များမှတစ်ဆင့် အပ်ဒိတ်များကို ခွင့်ပြုပါ။ | အုပ်ချုပ်ရေးကောင်စီ | မူဝါဒ အပ်ဒိတ်များအတွက် CLI ကို ပေးပါ။ |
| အစီအစဉ်ထုတ်လွှတ်မှု- တယ်လီမီတာအတွက် Norito ဖြစ်ရပ်များ (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) ကို ထုတ်လွှတ်သည်။ | မြင်နိုင်စွမ်း | အစီအစဉ်အစီအစဉ် + မှတ်တမ်းကို သတ်မှတ်ပါ။ |

စမ်းသပ်ခြင်း-
- ဝင်ခွင့်အမှတ်တစ်ခုစီအတွက် ယူနစ်စစ်ဆေးမှုများ (အပြုသဘော + ငြင်းပယ်ခြင်း)။
- ဆက်ခံခြင်းကွင်းဆက်များအတွက် ပိုင်ဆိုင်မှုစမ်းသပ်မှုများ (သံသရာမရှိ၊ monotonic ခေတ်များ)။
- ကျပန်းမန်နီးဖက်စ်များဖန်တီးခြင်းဖြင့် Fuzz အတည်ပြုခြင်း

## ဝန်ဆောင်မှုမျက်နှာစာ (Torii/SDK ပေါင်းစပ်မှု)

| အစိတ်အပိုင်း | တာဝန် | ပိုင်ရှင်(များ) |
|----------|------|----------|
| Torii ဝန်ဆောင်မှု | `/v2/sorafs/pin` (တင်သွင်းရန်)၊ `/v2/sorafs/pin/{cid}` (ရှာဖွေမှု)၊ `/v2/sorafs/aliases` (စာရင်း/စည်း)၊ `/v2/sorafs/replication` (အော်ဒါ/ပြေစာများ) ကို ဖော်ထုတ်ပါ။ pagination + filtering ပေးပါ။ | ကွန်ရက်ချိတ်ဆက်ခြင်း TL / Core Infra |
| သက်သေခံချက် | တုံ့ပြန်မှုများတွင် registry အမြင့်/hash ကို ထည့်သွင်းပါ။ SDKs မှအသုံးပြုသော Norito သက်သေပြတည်ဆောက်ပုံကို ထည့်ပါ။ | Core Infra |
| CLI | `sorafs_manifest_stub` သို့မဟုတ် `sorafs_pin` CLI အသစ်ကို `pin submit`၊ `alias bind`၊ `order issue`၊ `registry export` ဖြင့် တိုးချဲ့ပါ။ | Tooling WG |
| SDK | Norito schema မှ client bindings (Rust/Go/TS) ကို ဖန်တီးပါ။ ပေါင်းစပ်စစ်ဆေးမှုများထည့်ပါ။ | SDK အဖွဲ့များ |

လည်ပတ်မှုများ-
- အဆုံးမှတ်များရယူရန်အတွက် ကက်ရှ်အလွှာ/ETag ကိုထည့်ပါ။
- Torii မူဝါဒများနှင့် ကိုက်ညီသော နှုန်းထားကန့်သတ်ချက် / အထောက်အထားကို ပေးပါ။

## တန်ဆာပလာများနှင့် CI

- Fixtures directory- `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` စတိုးဆိုင်များမှ လက်မှတ်ရေးထိုးထားသော manifest/alias/order snapshots များကို `cargo run -p iroha_core --example gen_pin_snapshot` မှ ပြန်လည်ထုတ်ပေးပါသည်။
- CI အဆင့်- `ci/check_sorafs_fixtures.sh` သည် လျှပ်တစ်ပြက်ရိုက်ချက်အား ပြန်လည်ထုတ်ပေးပြီး CI ပစ္စည်းများကို ချိန်ညှိထားခြင်းဖြင့် ကွဲပြားမှုများပေါ်လာပါက မအောင်မြင်ပါ။
- ပေါင်းစည်းခြင်းစမ်းသပ်မှုများ (`crates/iroha_core/tests/pin_registry.rs`) သည် ပျော်ရွှင်သောလမ်းကြောင်းနှင့် ထပ်တူထပ်တူသော-အမည်တူများကို ပယ်ချခြင်း၊ အမည်တူခွင့်ပြုချက်/ထိန်းသိမ်းခြင်းအစောင့်များ၊ ကိုက်ညီမှုမရှိသော chunker လက်ကိုင်များ၊ ပုံစံတူ-ရေတွက်မှုအတည်ပြုချက်နှင့် ဆက်ခံ-စောင့်ကြပ်မှုကျရှုံးမှုများ (အမည်မသိ/ကြိုတင်အတည်ပြုထားသော/အငြိမ်းစား/အငြိမ်းစား/ကိုယ်ကိုတိုင်ညွှန်ပြချက်များ)၊ လွှမ်းခြုံမှုအသေးစိတ်အတွက် `register_manifest_rejects_*` အမှုများကို ကြည့်ပါ။
- ယခု ယူနစ်စမ်းသပ်မှုများသည် `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` တွင် နာမည်အရင်းအတည်ပြုခြင်း၊ ထိန်းသိမ်းစောင့်ကြပ်ခြင်းနှင့် ဆက်ခံခြင်းစစ်ဆေးမှုများကို အကျုံးဝင်ပါသည်။ နိုင်ငံပိုင်စက်များ ဆင်းသက်သည်နှင့် တစ်ပြိုင်နက် multi-hop succession detection။
- မြင်နိုင်စွမ်းရှိသော ပိုက်လိုင်းများအသုံးပြုသည့် ဖြစ်ရပ်များအတွက် Golden JSON။

## Telemetry & Observability

မက်ထရစ်များ (Prometheus)
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- လက်ရှိပံ့ပိုးပေးသူ တယ်လီမီတာ (`torii_sorafs_capacity_*`၊ `torii_sorafs_fee_projection_nanos`) သည် အဆုံးမှအဆုံးအထိ ဒက်ရှ်ဘုတ်များအတွက် နယ်ပယ်တွင် ကျန်ရှိနေပါသည်။

မှတ်တမ်းများ-
- အုပ်ချုပ်မှုစာရင်းစစ်များအတွက် ဖွဲ့စည်းတည်ဆောက်ထားသော Norito ဖြစ်ရပ်စီးကြောင်း (လက်မှတ်ထိုး?)။

သတိပေးချက်များ-
- SLA ထက်ကျော်လွန်သော ပုံတူကူးယူမှုများကို ဆိုင်းငံ့ထားသည်။
- Alias ​​သက်တမ်းကုန် < သတ်မှတ်ချက်။
- ထိန်းသိမ်းထားသော ချိုးဖောက်မှုများ (သက်တမ်းမကုန်မီ သက်တမ်းမတိုးမီ ဖော်ပြချက်)။

ဒက်ရှ်ဘုတ်များ-
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` သည် ဘဝလည်ပတ်မှုစုစုပေါင်း၊ အမည်များ အကျုံးဝင်မှု၊ မှတ်တမ်းများ ပြည့်ဝမှု၊ SLA အချိုး၊ latency နှင့် slack ထပ်ဆင့်မှုများနှင့် လွတ်သွားသော အမှာစာနှုန်းများကို မှတ်သားထားသည်။

## Runbooks & Documentation

- မှတ်ပုံတင်ခြင်းအခြေအနေမွမ်းမံမှုများပါဝင်ရန် `docs/source/sorafs/migration_ledger.md` ကို အပ်ဒိတ်လုပ်ပါ။
- အော်ပရေတာလမ်းညွှန်- `docs/source/sorafs/runbooks/pin_registry_ops.md` (ယခုထုတ်ဝေသည်) မက်ထရစ်များ၊ သတိပေးချက်၊ အသုံးချမှု၊ အရန်သိမ်းခြင်းနှင့် ပြန်လည်ရယူခြင်းစီးဆင်းမှုများကို အကျုံးဝင်သည်။
- အုပ်ချုပ်မှုလမ်းညွှန်- မူဝါဒသတ်မှတ်ချက်များ၊ ခွင့်ပြုချက်လုပ်ငန်းအသွားအလာ၊ အငြင်းပွားမှုကိုင်တွယ်ပုံကို ဖော်ပြပါ။
- အဆုံးမှတ်တစ်ခုစီအတွက် API ရည်ညွှန်းစာမျက်နှာများ (Docusaurus docs)။

## မှီခိုမှုနှင့် စီစစ်ခြင်း။

1. တရားဝင်အတည်ပြုခြင်းအစီအစဥ်များကို အပြီးသတ်ပါ (ManifestValidator ပေါင်းစပ်မှု)။
2. Norito schema + မူဝါဒ ပုံသေများကို အပြီးသတ်ပါ။
3. စာချုပ် + ဝန်ဆောင်မှု၊ ဝါယာကြိုး တယ်လီမီတာကို အကောင်အထည်ဖော်ပါ။
4. ပြင်ဆင်မှုများ ပြန်ထုတ်ပါ၊ ပေါင်းစည်းမှုအစုံများကို ဖွင့်ပါ။
5. docs/runbooks များကို အပ်ဒိတ်လုပ်ပြီး လမ်းပြမြေပုံပါ အရာများ ပြီးမြောက်ကြောင်း အမှတ်အသားပြုပါ။

SF-4 အောက်တွင် လမ်းပြမြေပုံ စစ်ဆေးရေးစာရင်း အကြောင်းအရာတစ်ခုစီသည် တိုးတက်မှုလုပ်ဆောင်သည့်အခါ ဤအစီအစဉ်ကို ကိုးကားသင့်သည်။
REST façade သည် ယခုအခါ အတည်ပြုစာရင်းဝင်သည့် အဆုံးမှတ်များနှင့်အတူ ပို့ဆောင်ပေးသည်-

- `GET /v2/sorafs/pin` နှင့် `GET /v2/sorafs/pin/{digest}` return manifests နှင့်အတူ
  alias bindings, replication orders, and a attestation object တို့မှ ဆင်းသက်လာသည်။
  နောက်ဆုံးပိတ် hash။
- `GET /v2/sorafs/aliases` နှင့် `GET /v2/sorafs/replication` တက်ကြွမှုကို ဖော်ထုတ်ပါ
  တသမတ်တည်း pagination နှင့် alias catalog နှင့် replication order backlog
  အခြေအနေ စစ်ထုတ်မှုများ။

CLI သည် ဤခေါ်ဆိုမှုများကို အဆုံးသတ်သည် (`iroha app sorafs pin list`၊ `pin show`၊ `alias list`၊
`replication list`) ထို့ကြောင့် အော်ပရေတာများသည် script registry စစ်ဆေးမှုများကို မထိဘဲ လုပ်နိုင်သည်
အောက်အဆင့် API များ။