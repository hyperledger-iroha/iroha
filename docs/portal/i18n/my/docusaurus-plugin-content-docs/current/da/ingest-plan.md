---
lang: my
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ခေါင်းစဉ်- ဒေတာရရှိနိုင်မှု Ingest အစီအစဉ်
sidebar_label- ထည့်သွင်းရန် အစီအစဉ်
ဖော်ပြချက်- Schema၊ API မျက်နှာပြင်နှင့် Torii blob ထည့်သွင်းခြင်းအတွက် တရားဝင်အတည်ပြုခြင်းအစီအစဉ်။
---

::: Canonical Source ကို သတိပြုပါ။
:::

# Sora Nexus ဒေတာရရှိနိုင်မှု Ingest အစီအစဉ်

_Drafted: 2026-02-20 - ပိုင်ရှင်- Core Protocol WG / Storage Team / DA WG_

DA-2 အလုပ်ရေစီးကြောင်းသည် Torii ကို Norito ထုတ်ပေးသည့် blob ingest API ဖြင့် တိုးချဲ့သည်
မက်တာဒေတာနှင့် အစေ့များ SoraFS ပုံတူပွား။ ဤစာတမ်းသည် အဆိုပြုချက်ကို ဖမ်းယူထားသည်။
schema၊ API မျက်နှာပြင်နှင့် validation flow တို့ကို အကောင်အထည်မဖော်ဘဲ ဆက်လက်လုပ်ဆောင်နိုင်သည်။
ထင်ရှားသော simulations များကိုပိတ်ဆို့ခြင်း (DA-1 နောက်ဆက်တွဲ)။ payload ဖော်မတ်များအားလုံး လိုအပ်သည်။
Norito ကုဒ်ဒစ်များကို အသုံးပြုပါ။ serde/JSON တုံ့ပြန်မှုများကို ခွင့်မပြုပါ။

## ပန်းတိုင်

- ကြီးမားသော blob များ (Taikai အပိုင်းများ၊ လမ်းသွားဆိုက်ကားများ၊ အုပ်ချုပ်မှုဆိုင်ရာပစ္စည်းများ) ကိုလက်ခံပါ။
  အဆုံးအဖြတ်အရ Torii ကျော်သည်။
- blob၊ codec ဘောင်များကို ဖော်ပြသည့် canonical Norito ကို ထုတ်လုပ်ပါ
  ပရိုဖိုင်ကို ဖျက်ရန်နှင့် ထိန်းသိမ်းမှုမူဝါဒ။
- SoraFS hot storage နှင့် enqueue replication အလုပ်များတွင် chunk metadata ကို ဆက်ရှိနေပါစေ။
- SoraFS မှတ်ပုံတင်ခြင်းနှင့် အုပ်ချုပ်မှုသို့ ပင်ထိုးရည်ရွယ်ချက်များ + မူဝါဒတဂ်များကို ထုတ်ဝေပါ
  လေ့လာသူများ။
- ဝင်ခွင့်လက်ခံဖြတ်ပိုင်းများကို ဖောက်သည်များသည် တိကျသေချာသော ထုတ်ဝေမှုဆိုင်ရာ အထောက်အထားများ ပြန်လည်ရရှိစေရန် ထုတ်ဖော်ပြသပါ။

## API Surface (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Payload သည် Norito-encoded `DaIngestRequest` ဖြစ်သည်။ တုံ့ပြန်မှုများကို အသုံးပြုသည်။
`application/norito+v1` နှင့် `DaIngestReceipt` ကို ပြန်ပေးပါ။

| တုံ့ပြန်မှု | အဓိပ္ပါယ် |
| ---| ---|
| 202 လက်ခံ | အတုံးအခဲ/ပုံတူခြင်းအတွက် Blob တန်းစီထားသည်။ ပြေစာ ပြန်ပေးတယ်။ |
| 400 Bad Request | အစီအစဉ်/အရွယ်အစား ချိုးဖောက်မှု (တရားဝင်စစ်ဆေးမှုများကို ကြည့်ပါ)။ |
| 401 ခွင့်ပြုချက်မရှိဘဲ | API တိုကင် ပျောက်နေသည်/မမှန်ပါ။ |
| 409 ပဋိပက္ခ | မကိုက်ညီသော မက်တာဒေတာဖြင့် `client_blob_id` ကို မိတ္တူပွားပါ။ |
| 413 Payload Too Large | ပြင်ဆင်သတ်မှတ်ထားသော blob အရှည်ကန့်သတ်ချက်ထက် ကျော်လွန်နေပါသည်။ |
| 429 တောင်းဆိုမှုများ အလွန်များ | နှုန်းကန့်သတ်ချက်ထိသွားတယ်။ |
| 500 အတွင်းပိုင်းအမှား | မမျှော်လင့်ထားသော ပျက်ကွက်မှု (မှတ်တမ်းဝင် + သတိပေးချက်)။ |

## အဆိုပြုထားသော Norito Schema

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> အကောင်အထည်ဖော်မှုမှတ်စု- ဤ payloads အတွက် canonical Rust ကိုယ်စားပြုမှုများသည် ယခုအောက်တွင် ရှိနေပါသည်။
> `iroha_data_model::da::types`၊ `iroha_data_model::da::ingest` ရှိ တောင်းဆိုချက်/ပြေစာထုပ်ပိုးမှုများပါရှိသော
> နှင့် `iroha_data_model::da::manifest` ရှိ ထင်ရှားသော ဖွဲ့စည်းပုံ။

`compression` အကွက်သည် ခေါ်ဆိုသူများသည် ဝန်ဆောင်ခကို မည်သို့ပြင်ဆင်ကြောင်း ကြော်ငြာသည်။ Torii လက်ခံသည်။
`identity`၊ `gzip`၊ `deflate` နှင့် `zstd` မတိုင်မီ bytes အား ပွင့်လင်းမြင်သာစွာ ချုံ့ခြင်း
ဟက်ခြင်း၊ ချခြင်း နှင့် ရွေးချယ်နိုင်သော မန်နီးဖက်စ်များကို အတည်ပြုခြင်း။

### မှန်ကန်ကြောင်း စစ်ဆေးချက်စာရင်း

1. တောင်းဆိုချက် Norito ခေါင်းစီး `DaIngestRequest` ကိုက်ညီကြောင်း အတည်ပြုပါ။
2. `total_size` သည် canonical (decompressed) payload length နှင့် ကွာခြားသည် သို့မဟုတ် configured max ထက်ကျော်လွန်ပါက မအောင်မြင်ပါ။
3. `chunk_size` ချိန်ညှိမှု (ပါဝါ-နှစ်ခု၊ <= 2 MiB) ကို တွန်းအားပေးပါ။
4. `data_shards + parity_shards` <= ကမ္ဘာလုံးဆိုင်ရာ အမြင့်ဆုံးနှင့် တန်းတူညီမျှမှု >= 2 ကို သေချာပါစေ။
5. `retention_policy.required_replica_count` သည် အုပ်ချုပ်မှုအခြေခံအချက်အား လေးစားရမည်။
6. Canonical hash (လက်မှတ်အကွက်မှအပ မပါ) ကို လက်မှတ်စစ်ခြင်း
7. payload hash + metadata တူညီခြင်းမရှိပါက `client_blob_id` ပွားခြင်းကို ငြင်းပယ်ပါ။
8. `norito_manifest` ကို ပံ့ပိုးပေးသောအခါ၊ ဇယားကွက် + hash ကိုက်ညီမှုများကို ပြန်လည်တွက်ချက်စစ်ဆေးပါ
   အတုံးလိုက်ပြီးနောက် ထင်ရှားသည်။ မဟုတ်ပါက node သည် manifest ကိုထုတ်ပေးပြီး ၎င်းကို သိမ်းဆည်းသည်။
9. စီစဉ်သတ်မှတ်ထားသော ပုံတူပွားခြင်းမူဝါဒကို ကျင့်သုံးပါ- Torii သည် တင်ပြထားသောစာကို ပြန်လည်ရေးသားသည်
   `RetentionPolicy` နှင့် `torii.da_ingest.replication_policy` (ကြည့်ပါ
   `replication-policy.md`) နှင့် ထိန်းသိမ်းထားသည့် ကြိုတင်တည်ဆောက်ထားသော သရုပ်များကို ငြင်းပယ်သည်
   မက်တာဒေတာသည် ပြဌာန်းထားသော ပရိုဖိုင်နှင့် မကိုက်ညီပါ။

### Chunking & Replication Flow

1. payload ကို `chunk_size` တွင် အပိုင်းလိုက်၊ အတုံးတစ်ခုလျှင် BLAKE3 + Merkle root ကို တွက်ချက်ပါ။
2. တည်ဆောက်ခြင်း Norito `DaManifestV1` (ဖွဲ့စည်းပုံအသစ်) (အခန်းကဏ္ဍ/group_id) ကတိကဝတ်များကို ရိုက်ကူးခြင်း၊
   ဖျက်ပစ်သည့် အပြင်အဆင် (အတန်းနှင့် ကော်လံ တူညီမှု အရေအတွက် အပေါင်း `ipa_commitment`)၊ ထိန်းသိမ်းမှု မူဝါဒ၊
   နှင့် metadata။
3. `config.da_ingest.manifest_store_dir` အောက်တွင် canonical manifest bytes ကို တန်းစီပါ
   (Torii သည် Lane/epoch/sequence/ticket/ fingerprint ဖြင့်သော့ခတ်ထားသော `manifest.encoded` ဖိုင်များကိုရေးသည်) ထို့ကြောင့် SoraFS
   Orchestration သည် ၎င်းတို့ကို ထည့်သွင်းနိုင်ပြီး သိုလှောင်မှုလက်မှတ်ကို ဆက်တိုက်ဒေတာနှင့် ချိတ်ဆက်နိုင်သည်။
4. အုပ်ချုပ်မှုတက်ဂ် + မူဝါဒဖြင့် `sorafs_car::PinIntent` မှတစ်ဆင့် ပင်ထိုးရည်ရွယ်ချက်များကို ထုတ်ဝေပါ။
5. လေ့လာသူများကို အသိပေးရန် Norito ဖြစ်ရပ် `DaIngestPublished` ကို ထုတ်လွှတ်သည် (အလင်းဖောက်သည်များ၊
   အုပ်ချုပ်မှု၊ ခွဲခြမ်းစိတ်ဖြာမှု)။
6. `DaIngestReceipt` ကို ခေါ်ဆိုသူထံ ပြန်ပို့ပါ (Torii DA ဝန်ဆောင်မှုကီးဖြင့် လက်မှတ်ရေးထိုးထားသည်) နှင့် ထုတ်လွှတ်ခြင်း
   `Sora-PDP-Commitment` ခေါင်းစီးကြောင့် SDK များသည် ကုဒ်သွင်းထားသော ကတိကဝတ်ကို ချက်ချင်းဖမ်းယူနိုင်သည် ။ ပြေစာ
   ယခု `rent_quote` (a Norito `DaRentQuote`) နှင့် `stripe_layout` ပါ၀င်သည် ၊ တင်ပြသူများကို ပြသခွင့်ပြုသည် ။
   အခြေခံငှားရမ်းခ၊ အရန်အစုရှယ်ယာ၊ PDP/PoTR အပိုဆုမျှော်လင့်ချက်များ၊ နှင့် 2D ဖျက်ခြင်းအပြင်အဆင်
   ရန်ပုံငွေမတည်မီ သိုလှောင်မှုလက်မှတ်။

## သိုလှောင်မှု / Registry အပ်ဒိတ်များ

- `sorafs_manifest` ကို `DaManifestV1` ဖြင့် ချဲ့ထွင်ပြီး အဆုံးအဖြတ်ပိုင်းခြားမှုကို ဖွင့်ပေးသည်။
- ဗားရှင်းလုပ်ထားသော payload ရည်ညွှန်းချက်ဖြင့် registry stream အသစ် `da.pin_intent` ကိုထည့်ပါ။
  hash + လက်မှတ် ID ကိုဖော်ပြပါ။
- စားသုံးနိုင်မှု latency ကိုခြေရာခံရန်၊ အတုံးလိုက်အတုံးလိုက်အတုံးလိုက်အခဲလိုက်ပြုလုပ်ရန်၊
  ပုံတူကူးယူခြင်း နှင့် ပျက်ကွက်မှု အရေအတွက်။

## စမ်းသပ်ခြင်းဗျူဟာ

- schema validation၊ လက်မှတ်စစ်ဆေးမှုများ၊ ထပ်နေသောရှာဖွေတွေ့ရှိမှုအတွက်ယူနစ်စမ်းသပ်မှုများ။
- `DaIngestRequest` ၏ Norito ကုဒ်နံပါတ်၊ မန်နီးဖက်စ်နှင့် ပြေစာတို့ကို အတည်ပြုသည့် ရွှေရောင်စစ်ဆေးမှုများ။
- ပေါင်းစည်းခြင်းကြိုးသည် SoraFS + registry ကို အတုယူ၍ အတုံးအခဲ + ပင်နံပါတ် စီးဆင်းမှုများကို အခိုင်အမာ ပေါင်းစပ်ထားသည်။
- ကျပန်းဖျက်ပစ်သည့်ပရိုဖိုင်များနှင့် ထိန်းသိမ်းမှုပေါင်းစပ်မှုများ ပါဝင်သည့် ပိုင်ဆိုင်မှုစစ်ဆေးမှုများ။
- ပုံသဏ္ဍာန်မမှန်သော မက်တာဒေတာကို ကာကွယ်ရန် Norito ပေးချေမှုများအား ရှုပ်ယှက်ခတ်ခြင်း။

## CLI & SDK Tooling (DA-8)- `iroha app da submit` (CLI ဝင်ခွင့်အမှတ်အသစ်) သည် ယခုအခါ မျှဝေသုံးစွဲနေသော တည်ဆောက်သူ/ထုတ်ဝေသူအား ခြုံငုံမိသောကြောင့် အော်ပရေတာများ
  Taikai အစုအဝေးစီးဆင်းမှု၏ အပြင်ဘက်တွင် မထင်သလို blobs များကို ထည့်သွင်းနိုင်သည်။ အမိန့်ပေးသည်။
  `crates/iroha_cli/src/commands/da.rs:1` နှင့် payload ကိုအသုံးပြုသည်၊ ဖျက်ပစ်ခြင်း/ ထိန်းသိမ်းခြင်းပရိုဖိုင်ကို စားသုံးသည် နှင့်
  CLI ဖြင့် canonical `DaIngestRequest` ကို မလက်မှတ်မထိုးမီ ရွေးချယ်နိုင်သော မက်တာဒေတာ/မန်နီးဖက်စ်ဖိုင်များ
  config key အောင်မြင်သော လုပ်ဆောင်မှုများသည် `da_request.{norito,json}` နှင့် `da_receipt.{norito,json}` အောက်တွင် ဆက်ရှိနေသည်
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` မှတဆင့် override) ထို့ကြောင့် ပစ္စည်းများကို ထုတ်ဝေနိုင်သည်
  စားသုံးနေစဉ်အသုံးပြုသည့် Norito ဘိုက်အတိအကျကို မှတ်တမ်းတင်ပါ။
- command သည် `client_blob_id = blake3(payload)` သို့ ပုံသေသတ်မှတ်ထားသော်လည်း မှတဆင့် overrides ကိုလက်ခံပါသည်။
  `--client-blob-id`၊ မက်တာဒေတာ JSON မြေပုံများ (`--metadata-json`) နှင့် ကြိုတင်ထုတ်လုပ်ထားသော မန်နီးဖက်စ်များကို ဂုဏ်ပြုပါသည်။
  (`--manifest`) နှင့် စိတ်ကြိုက်အတွက် အော့ဖ်လိုင်းပြင်ဆင်မှုအပြင် `--endpoint` အတွက် `--endpoint` ကို ပံ့ပိုးပေးသည်
  Torii တန်ဆာပလာများ။ ပြေစာကို JSON ကို ပိတ်ပြီး disk သို့ စာရေးထားသည့်အပြင် stdout သို့ ရိုက်နှိပ်ထားသည်။
  DA-8 "submit_blob" ကိရိယာတန်ဆာပလာလိုအပ်ချက်နှင့် SDK တူညီမှုလုပ်ငန်းကို ပိတ်ဆို့ခြင်းအား ပြန်ဖွင့်ခြင်း။
- `iroha app da get` သည် စွမ်းအားရှိပြီးသား ရင်းမြစ်အစုံလိုက်သံစုံတီးဝိုင်းအတွက် DA-အာရုံစူးစိုက်မှုကို ပေါင်းထည့်သည်
  `iroha app sorafs fetch`။ အော်ပရေတာများသည် ၎င်းကို manifest + chunk-plan artefacts (`--manifest`၊
  `--plan`၊ `--manifest-id`) **သို့မဟုတ်** Torii သိုလှောင်မှုလက်မှတ်ကို `--storage-ticket` မှတစ်ဆင့် ဖြတ်သန်းပါ။ လက်မှတ်ရလိုက်တာ
  လမ်းကြောင်းကိုအသုံးပြုသည် CLI သည် `/v2/da/manifests/<ticket>` မှ manifest ကိုဆွဲထုတ်ပြီး၊ အစုအဝေးအောက်တွင်ဆက်လက်တည်ရှိသည်
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir`) သည် blob hash အတွက် ဆင်းသက်လာသည်
  `--manifest-id`၊ ထို့နောက် ပံ့ပိုးပေးထားသော `--gateway-provider` စာရင်းဖြင့် သံစုံတီးဝိုင်းကို လုပ်ဆောင်သည်။ အားလုံး
  SoraFS fetcher မျက်နှာပြင်မှ အဆင့်မြင့် ခလုတ်များ မပျက်မစီး (မန်နီးဖက်စ် စာအိတ်များ၊ ကလိုင်းယင့်တံဆိပ်များ၊ အစောင့်ကက်ရှ်များ၊
  အမည်ဝှက် သယ်ယူပို့ဆောင်ရေး အစားထိုးမှုများ၊ ရမှတ်ဘုတ်တင်ပို့မှု၊ နှင့် `--output` လမ်းကြောင်းများ) နှင့် ထင်ရှားသော နိဂုံးချုပ်နိုင်သည်
  စိတ်ကြိုက် Torii လက်ခံဆောင်ရွက်ပေးရန်အတွက် `--manifest-endpoint` မှတစ်ဆင့် လွှမ်းမိုးထားသောကြောင့် အဆုံးမှအစအဆုံးရရှိနိုင်မှုစစ်ဆေးမှုများကို တိုက်ရိုက်ထုတ်လွှင့်သည်
  သံစုံတီးဝိုင်းလော့ဂျစ်ကို ထပ်ခြင်းမပြုဘဲ `da` ၏အမည်နေရာအောက်တွင် လုံးလုံးလျားလျား။
- `iroha app da get-blob` သည် `GET /v2/da/manifests/{storage_ticket}` မှတစ်ဆင့် Torii မှ တိုက်ရိုက် Canonical manifest များကို ဆွဲထုတ်သည်။
  command သည် `manifest_{ticket}.norito`၊ `manifest_{ticket}.json` နှင့် `chunk_plan_{ticket}.json` တို့ကို ရေးသားသည်
  `artifacts/da/fetch_<timestamp>/` အောက်တွင် (သို့မဟုတ် သုံးစွဲသူမှပေးသော `--output-dir`) အောက်တွင် အတိအကျ ပဲ့တင်ထပ်သည်
  `iroha app da get` တောင်းခံလွှာ (`--manifest-id` အပါအဝင်) နောက်ဆက်တွဲ တီးမှုတ်သူ ထုတ်ယူမှုအတွက် လိုအပ်သည်။
  ၎င်းသည် အော်ပရေတာများအား manifest spool လမ်းညွှန်များမှနေ၍ ဖမ်းယူသူသည် ၎င်းကို အမြဲတမ်းအသုံးပြုကြောင်း အာမခံပါသည်။
  Torii မှ ထုတ်လွှတ်သော လက်မှတ်ရေးထိုးထားသော ရှေးဟောင်းပစ္စည်းများ။ JavaScript Torii ဖောက်သည်သည် ဤစီးဆင်းမှုကို ထင်ဟပ်စေသည်။
  `ToriiClient.getDaManifest(storageTicketHex)`၊ ကုဒ်လုပ်ထားသော Norito bytes ကို ပြန်ပေးသည်၊ ထင်ရှားသော JSON၊
  နှင့် SDK ခေါ်ဆိုသူများသည် CLI သို့ ပစ်မ၀င်ဘဲ သံစုံတီးဝိုင်းအစည်းအဝေးများကို ရေဓာတ်ဖြည့်ပေးနိုင်သော အစီအစဉ်ကို အပိုင်းလိုက်လုပ်ပါ။
  Swift SDK သည် ယခု တူညီသော မျက်နှာပြင်များကို ထုတ်ဖော်ပြသသည် (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`)၊ မူလ SoraFS သံစုံတီးဝိုင်း ထုပ်ပိုးခြင်းသို့ ပိုက်ထုပ်များ
  iOS ဖောက်သည်များသည် မန်နီးဖက်စ်များကို ဒေါင်းလုဒ်လုပ်ခြင်း၊ ရင်းမြစ်များစွာကို ရယူခြင်းနှင့် အထောက်အထားများကို ဖမ်းယူခြင်းမပြုဘဲ လုပ်ဆောင်နိုင်သည်။
  CLI ကို ခေါ်ဆိုခြင်း။【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` သည် ပံ့ပိုးပေးထားသော သိုလှောင်မှုအရွယ်အစားအတွက် အဆုံးအဖြတ်ပေးသော ငှားရမ်းခနှင့် မက်လုံးပေးမှု အပိုင်းများကို တွက်ချက်သည်
  နှင့် retention window ။ အကူအညီပေးသူက လက်ရှိအသုံးပြုနေသော `DaRentPolicyV1` (JSON သို့မဟုတ် Norito bytes) သို့မဟုတ်
  built-in မူရင်း၊ မူဝါဒကို အတည်ပြုပြီး JSON အကျဉ်းချုပ် (`gib`၊ `months`၊ မူဝါဒ မက်တာဒေတာ၊
  နှင့် `DaRentQuote` အကွက်များ) ထို့ကြောင့် စာရင်းစစ်များသည် အုပ်ချုပ်မှုမိနစ်အတွင်း မလိုအပ်ဘဲ XOR ကောက်ခံမှုများကို အတိအကျကိုးကားနိုင်သည်။
  ad hoc script တွေရေးတယ်။ အဆိုပါ command သည် JSON ရှေ့တွင် တစ်ကြောင်းတစ်ကြောင်း `rent_quote ...` အကျဉ်းကိုလည်း ထုတ်လွှတ်သည်
  အဖြစ်အပျက်လေ့ကျင့်ခန်းများအတွင်း ကွန်ဆိုးလ်မှတ်တမ်းများကို ဖတ်ရှုနိုင်စေရန် payload။ `--quote-out artifacts/da/rent_quotes/<stamp>.json` နှင့်တွဲပါ။
  `--policy-label "governance ticket #..."` အတိအကျ မူဝါဒမဲများကို ကိုးကားသည့် သပ်ရပ်သော လက်ရာများ ဆက်လက်တည်ရှိရန်
  သို့မဟုတ် config အစုအဝေး; CLI သည် စိတ်ကြိုက်အညွှန်းကို ဖြတ်တောက်ပြီး ကြိုးလွတ်များကို ငြင်းပယ်သောကြောင့် `policy_source` တန်ဖိုးများ
  ဘဏ္ဍာတိုက် ဒက်ရှ်ဘုတ်များတွင် ဆက်လက်လုပ်ဆောင်နိုင်သည်။ subcommand အတွက် `crates/iroha_cli/src/commands/da.rs` ကိုကြည့်ပါ။
  မူဝါဒအစီအစဉ်အတွက် `docs/source/da/rent_policy.md` 【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` သည် အထက်ဖော်ပြပါ အားလုံးကို ချိတ်ဆက်ထားသည်- ၎င်းသည် သိုလှောင်မှု လက်မှတ်တစ်ခုယူသည်၊ ဒေါင်းလုဒ်လုပ်သည်
  canonical manifest အစုအစည်းသည် အရင်းအမြစ်ပေါင်းစုံ သံစုံတီးဝိုင်း (`iroha app sorafs fetch`) ကို လုပ်ဆောင်သည်
  ပံ့ပိုးထားသော `--gateway-provider` စာရင်း၊ ဒေါင်းလုဒ်လုပ်ထားသော payload + ရမှတ်ဘုတ်အောက်တွင် ဆက်ရှိနေသည်
  `artifacts/da/prove_availability_<timestamp>/` နှင့် လက်ရှိ PoR အထောက်အကူကို ချက်ချင်းခေါ်သည်။
  ရယူထားသော ဘိုက်များကို အသုံးပြု၍ (`iroha app da prove`) အော်ပရေတာများသည် သံစုံတီးဝိုင်းခလုတ်များကို ညှိနိုင်သည်။
  (`--max-peers`၊ `--scoreboard-out`၊ manifest endpoint overrides) နှင့် သက်သေနမူနာ
  (`--sample-count`၊ `--leaf-index`၊ `--sample-seed`) တစ်ခုတည်းသော command သည် artefacts ကိုထုတ်လုပ်နေစဉ်
  DA-5/DA-9 စာရင်းစစ်များမှ မျှော်လင့်ထားသည်- ပေးဆောင်မှုမိတ္တူ၊ အမှတ်စာရင်းအထောက်အထားများနှင့် JSON အထောက်အထား အနှစ်ချုပ်များ။

## TODO Resolution အကျဉ်းချုပ်

ယခင်က ပိတ်ဆို့ထားသော စားသုံးမှု TODO အားလုံးကို အကောင်အထည် ဖော်ပြီး စစ်ဆေးပြီးပါပြီ-

- **Compression အရိပ်အမြွက်** — Torii သည် ခေါ်ဆိုသူမှပေးသော အညွှန်းများ (`identity`၊ `gzip`၊ `deflate`၊
  `zstd`) နှင့် အတည်ပြုခြင်းမပြုမီ payload များကို ပုံမှန်ဖြစ်အောင် ပြုလုပ်ပေးသောကြောင့် canonical manifest hash သည် ကိုက်ညီသည်
  ချုံ့လိုက်သော bytes။【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **အုပ်ချုပ်မှု-သီးသန့် မက်တာဒေတာ ကုဒ်ဝှက်ခြင်း** — Torii သည် ယခုအခါ အုပ်ချုပ်ရေးဆိုင်ရာ မက်တာဒေတာကို ကုဒ်ဝှက်ထားသည်။
  ChaCha20-Poly1305 သော့ကို configure လုပ်ထားပြီး၊ မကိုက်ညီသော အညွှန်းများကို ပယ်ချပြီး ရှင်းလင်းပြတ်သားသော နှစ်ခုကို ပြသည်
  ဖွဲ့စည်းမှုခလုတ်များ (`torii.da_ingest.governance_metadata_key_hex`၊
  `torii.da_ingest.governance_metadata_key_label`) လည်ပတ်မှုအား အဆုံးအဖြတ်ပေးနိုင်ရန်။ 【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **ကြီးမားသော payload streaming** — အပိုင်းပေါင်းများစွာ ထည့်သွင်းအသုံးပြုမှုကို တိုက်ရိုက်ထုတ်လွှင့်နေသည်။ ဖောက်သည်များသည် အဆုံးအဖြတ်ပေးသော စီးဆင်းမှုဖြစ်သည်။
  `DaIngestChunk` သော့ခတ်ထားသော စာအိတ်များကို `client_blob_id`၊ Torii သည် အချပ်တစ်ခုစီကို အတည်ပြုပြီး အဆင့်လိုက် ပြုလုပ်သည်
  `manifest_store_dir` အောက်တွင် နှင့် `is_last` အလံ ပြုတ်သွားသည်နှင့် တပြိုင်နက် အက်တမ်အား ပြန်လည်တည်ဆောက်သည်၊
  ခေါ်ဆိုမှုတစ်ခုတည်းဖြင့် အပ်လုဒ်တင်မှုများဖြင့် မြင်တွေ့ရသည့် RAM spikes များကို ဖယ်ရှားခြင်း။【crates/iroha_torii/src/da/ingest.rs:392】
- **Manifest ဗားရှင်း** — `DaManifestV1` သည် တိကျသော `version` အကွက်ကို သယ်ဆောင်ထားပြီး Torii သည် ငြင်းဆိုသည်
  manifest အပြင်အဆင်အသစ်များ ပို့ဆောင်သည့်အခါတွင် အဆုံးအဖြတ်ပေးသော အဆင့်မြှင့်တင်မှုများကို အာမခံသော အမည်မသိဗားရှင်းများ။ 【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR ချိတ်များ** — PDP ကတိကဝတ်များသည် အတုံးလိုက်စတိုးမှ တိုက်ရိုက်ဆင်းသက်လာပြီး ဆက်ရှိနေသည်
  DA-5 အချိန်ဇယားဆွဲသူများသည် canonical data မှနမူနာစိန်ခေါ်မှုများကိုစတင်နိုင်စေရန်နှင့်၊
  ယခု `/v2/da/ingest` နှင့် `/v2/da/manifests/{ticket}` တွင် ယခု `Sora-PDP-Commitment` ခေါင်းစီးပါဝင်သည်
  base64 Norito payload ကို တင်ဆောင်သောကြောင့် SDKs သည် အတိအကျ ကတိကဝတ် DA-5 probes ပစ်မှတ်ကို သိမ်းဆည်းထားသည်။【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_datorii/7】

## အကောင်အထည်ဖော်မှုမှတ်စုများ

- ယခုအခါ Torii ၏ `/v2/da/ingest` အဆုံးမှတ်သည် payload ချုံ့မှုကို ပုံမှန်ဖြစ်စေပြီး ပြန်ဖွင့်သည့် ကက်ရှ်ကို တွန်းအားပေးသည်၊
  Canonical bytes များကို အဆုံးအဖြတ်ဖြတ်ကာ `DaManifestV1` ကို ပြန်လည်တည်ဆောက်ကာ ကုဒ်လုပ်ထားသော payload ကို ချပစ်လိုက်သည်
  SoraFS အတွက် `config.da_ingest.manifest_store_dir` ထဲသို့၊ `Sora-PDP-Commitment` ကို ထည့်ပါ
  ခေါင်းစီးကြောင့် အော်ပရေတာများသည် PDP အစီအစဉ်ဆွဲသူများ ရည်ညွှန်းမည့် ကတိကဝတ်ကို ဖမ်းယူထားသည်။【crates/iroha_torii/src/da/ingest.rs:220】
- လက်ခံထားသော blob တစ်ခုစီသည် ယခု `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ကို ထုတ်လုပ်သည်။
  `manifest_store_dir` အောက်တွင် ထည့်သွင်းထားသော canonical `DaCommitmentRecord` ကို ကုန်ကြမ်းနှင့်အတူ ပေါင်းစည်းခြင်း
  `PdpCommitmentV1` bytes ဖြစ်သောကြောင့် DA-3 အစုအဝေးတည်ဆောက်သူများနှင့် DA-5 အချိန်ဇယားဆွဲသူများသည် တူညီသောထည့်သွင်းမှုများမပါဘဲ ရေဓါတ်ဖြည့်ပေးသည်
  သရုပ်ပြများ သို့မဟုတ် အပိုင်းအစများကို ပြန်လည်ဖတ်ရှုခြင်း။【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK helper APIs များသည် Norito ကုဒ်ကို ပြန်လည်အကောင်အထည်ဖော်ရန် ခေါ်ဆိုသူများကို အတင်းအကြပ်မလုပ်ဘဲ PDP header payload ကို ထုတ်ဖော်ပြသသည်-
  သံချေးသေတ္တာသည် `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`၊ Python ကို တင်ပို့သည်။
  ယခု `ToriiClient` တွင် `decode_pdp_commitment_header` နှင့် `IrohaSwift` သင်္ဘောများ ပါ၀င်သည် ။
  ခေါင်းစီးမြေပုံကြမ်းများ သို့မဟုတ် `HTTPURLResponse` သာဓကများအတွက် `decodePdpCommitmentHeader` လွန်ဆွဲနေပါသည်။ 【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources】/IrohaSwift။
- Torii သည် `GET /v2/da/manifests/{storage_ticket}` ကိုလည်း ဖော်ထုတ်ပေးသောကြောင့် SDK နှင့် အော်ပရေတာများသည် မန်နီးဖက်စ်များကို ရယူနိုင်ပါသည်။
  နှင့် node ၏ spool directory ကိုမထိဘဲ အစီအစဥ်များကို အပိုင်းပိုင်းဖြတ်ပါ။ တုံ့ပြန်မှုသည် Norito bytes ကို ပြန်ပေးသည်။
  (base64)၊ ထင်ရှားစွာပြန်ဆိုထားသော JSON၊ `chunk_plan` JSON blob `sorafs fetch` အတွက် အဆင်သင့်ဖြစ်ပြီ သက်ဆိုင်ရာ၊
  hex digests (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) နှင့် mirrors
  တန်းတူညီမျှမှုအတွက် ထည့်သွင်းထားသော တုံ့ပြန်မှုများမှ `Sora-PDP-Commitment` ခေါင်းစီး။ `block_hash=<hex>` ကို ထောက်ပံ့ပေးနေပါသည်။
  query string သည် အဆုံးအဖြတ်ပေးသော `sampling_plan` (assignment hash၊ `sample_window`၊ နှင့် နမူနာကို ပြန်ပေးသည်
  `(index, role, group)` tuples များသည် 2D အပြင်အဆင်ကို လွှမ်းခြုံထားသည်) ထို့ကြောင့် validator နှင့် PoR ကိရိယာများသည် အတူတူပင်ဖြစ်သည်
  အညွှန်းကိန်းများ

### ကြီးမားသော Payload Streaming Flow

စီစဉ်သတ်မှတ်ထားသော တောင်းဆိုချက် ကန့်သတ်ချက်ထက် ကြီးသော ပိုင်ဆိုင်မှုများကို ထည့်သွင်းရန် လိုအပ်သော ဖောက်သည်များသည် a စတင်သည်။
`POST /v2/da/ingest/chunk/start` ကိုခေါ်ဆိုခြင်းဖြင့် တိုက်ရိုက်ထုတ်လွှင့်ခြင်း Torii သည် a ဖြင့် တုံ့ပြန်သည်။
`ChunkSessionId` (တောင်းဆိုထားသော blob မက်တာဒေတာမှဆင်းသက်လာသော BLAKE3) နှင့် ညှိနှိုင်းထားသော အတုံးအရွယ်အစား။
နောက်ဆက်တွဲ `DaIngestChunk` တောင်းဆိုမှုတစ်ခုစီသည်-- `client_blob_id` — နောက်ဆုံး `DaIngestRequest` နှင့် ဆင်တူသည်။
- `chunk_session_id` — လည်ပတ်နေသည့် စက်ရှင်နှင့် အချပ်များကို ချိတ်ဆက်ထားသည်။
- `chunk_index` နှင့် `offset` — အဆုံးအဖြတ်ပေးသော အမိန့်ကို ပြဋ္ဌာန်းပါ။
- `payload` — ညှိနှိုင်းထားသော အတုံးအရွယ်အစားအထိ။
- `payload_hash` — အချပ်၏ BLAKE3 hash ဖြစ်သောကြောင့် Torii သည် blob တစ်ခုလုံးကို buffering မလုပ်ဘဲ validate လုပ်နိုင်သည်။
- `is_last` — terminal အချပ်ကို ညွှန်ပြသည်။

Torii သည် `config.da_ingest.manifest_store_dir/chunks/<session>/` အောက်တွင် တရားဝင်အတည်ပြုထားသော အချပ်များ ဆက်လက်တည်ရှိနေပြီး
စွမ်းရည်ချို့တဲ့မှုကို ဂုဏ်ပြုရန်အတွက် ပြန်လည်ကစားသည့် ကက်ရှ်အတွင်း တိုးတက်မှုကို မှတ်တမ်းတင်သည်။ နောက်ဆုံးအချပ်က Torii
disk ပေါ်ရှိ payload ကို ပြန်လည်စုစည်းသည် ( memory spikes ကိုရှောင်ရှားရန် chunk directory မှတဆင့် streaming )
တစ်ချက်တည်းရိုက်ချက်ဖြင့် အပ်လုဒ်များကဲ့သို့ Canonical manifest/လက်ခံဖြတ်ပိုင်းကို အတိအကျတွက်ချက်ပြီး နောက်ဆုံးတွင် တုံ့ပြန်သည်
အဆင့်ဆင့်ပြုလုပ်ထားသော ပစ္စည်းကိုစားသုံးခြင်းဖြင့် `POST /v2/da/ingest`။ မအောင်မြင်သော ဆက်ရှင်များကို ပြတ်သားစွာ ဖျက်သိမ်းနိုင်သည် သို့မဟုတ်
`config.da_ingest.replay_cache_ttl` ပြီးနောက် အမှိုက်များကို စုဆောင်းကြသည်။ ဤဒီဇိုင်းသည် ကွန်ရက်ဖော်မတ်ကို ထိန်းသိမ်းထားသည်။
Norito နှင့် အဆင်ပြေသည်၊ ကလိုင်းယင့်-သတ်သတ်မှတ်မှတ် ပြန်လည်စတင်နိုင်သော ပရိုတိုကောများကို ရှောင်ကြဉ်ပြီး ရှိပြီးသား manifest ပိုက်လိုင်းကို ပြန်သုံးသည်
မပြောင်းလဲ။

** အကောင်အထည်ဖော်မှုအခြေအနေ။** Canonical Norito အမျိုးအစားများသည် ယခုနေထိုင်လျက်ရှိသည်
`crates/iroha_data_model/src/da/`-

- `ingest.rs` သည် `DaIngestRequest`/`DaIngestReceipt` နှင့်အတူ၊
  Torii အသုံးပြုသော `ExtraMetadata` ကွန်တိန်နာ။ 【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` သည် `DaManifestV1` နှင့် `ChunkCommitment` တို့ကို လက်ခံဆောင်ရွက်ပေးသည်၊၊
  အပိုင်းအစများ ပြီးပါပြီ။ 【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` သည် မျှဝေထားသော aliases (`BlobDigest`၊ `RetentionPolicy`၊
  `ErasureProfile` စသည်ဖြင့်) နှင့် အောက်တွင် မှတ်တမ်းတင်ထားသော မူရင်းမူဝါဒတန်ဖိုးများကို ကုဒ်လုပ်ပါသည်။ 【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool ဖိုင်များသည် `config.da_ingest.manifest_store_dir` တွင် ဆင်းသက်ပြီး၊ SoraFS တီးမှုတ်ခြင်းအတွက် အဆင်သင့်ဖြစ်သည်
  သိုလှောင်ခန်းထဲသို့ ဆွဲသွင်းရန် စောင့်ကြည့်သူ။【crates/iroha_torii/src/da/ingest.rs:220】

တောင်းဆိုမှု၊ မန်နီးဖက်စ်နှင့် ပြေစာပေးချေမှုများအတွက် အသွားအပြန် အကျုံးဝင်မှုကို ခြေရာခံထားသည်။
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`၊ Norito codec ကို သေချာစေသည်
အပ်ဒိတ်များတွင် တည်ငြိမ်နေပါသည်။【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**ထိန်းသိမ်းထားမှု ပုံသေများ။** အုပ်ချုပ်မှုစနစ်သည် ကာလအတွင်း ကနဦးထိန်းသိမ်းထားမှုမူဝါဒကို အတည်ပြုခဲ့သည်။
SF-6; `RetentionPolicy::default()` မှ ပြဌာန်းထားသော ပုံသေများမှာ-

- ပူပြင်းသောအဆင့်- 7 ရက် (`604_800` စက္ကန့်)
- အအေးအဆင့်- ရက် 90 (`7_776_000` စက္ကန့်)
- လိုအပ်သော ပုံတူများ- `3`
- သိုလှောင်မှုအဆင့်- `StorageClass::Hot`
- အုပ်ချုပ်မှု tag: `"da.default"`

လမ်းကြောင်းတစ်ခုကို လက်ခံလိုက်သောအခါ အောက်ပိုင်းအော်ပရေတာများသည် ဤတန်ဖိုးများကို ပြတ်သားစွာ အစားထိုးရပါမည်။
တင်းကျပ်သောလိုအပ်ချက်များ။