---
lang: my
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus ဒေတာရရှိနိုင်မှု Ingest အစီအစဉ်

_Drafted: 2026-02-20 - ပိုင်ရှင်- Core Protocol WG / Storage Team / DA WG_

DA-2 အလုပ်ရေစီးကြောင်းသည် Torii ကို Norito ထုတ်လွှတ်သည့် blob ingest API ဖြင့် တိုးချဲ့သည်
မက်တာဒေတာနှင့် အစေ့များ SoraFS ပွားခြင်း။ ဤစာတမ်းသည် အဆိုပြုချက်ကို ဖမ်းယူထားသည်။
schema၊ API မျက်နှာပြင်နှင့် validation flow တို့ကို အကောင်အထည်မဖော်ဘဲ ဆက်လက်လုပ်ဆောင်နိုင်သည်။
ထင်ရှားသော simulations များကိုပိတ်ဆို့ခြင်း (DA-1 နောက်ဆက်တွဲ)။ payload ဖော်မတ်များအားလုံး လိုအပ်သည်။
Norito ကုဒ်ဒစ်များကို အသုံးပြုပါ။ serde/JSON တုံ့ပြန်မှုများကို ခွင့်မပြုပါ။

## ပန်းတိုင်

- ကြီးမားသော blob များ (Taikai အပိုင်းများ၊ လမ်းသွားဆိုက်ကားများ၊ အုပ်ချုပ်မှုဆိုင်ရာပစ္စည်းများ) ကိုလက်ခံပါ။
  Torii ကို အဆုံးအဖြတ်ပေးသည်။
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

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

လက်ရှိလမ်းကြောင်းကက်တလောက်မှဆင်းသက်လာသော `DaProofPolicyBundle` ဗားရှင်းကို ပြန်ပေးသည်။
အစုအစည်းသည် `version` (လောလောဆယ် `1`)၊ `policy_hash` (ဟက်ရှ်)
မှာယူထားသည့် မူဝါဒစာရင်း) နှင့် `policies` တွင် `lane_id`၊ `dataspace_id` တို့ကို သယ်ဆောင်လာခြင်း၊
`alias` နှင့် ပြဌာန်းထားသော `proof_scheme` (ယနေ့ `merkle_sha256`၊ KZG လမ်းကြောများဖြစ်သည်
KZG ကတိကဝတ်များ မရရှိနိုင်မချင်း ထည့်သွင်းခြင်းဖြင့် ပယ်ချခဲ့သည်။) အခု block header ပါ။
`da_proof_policies_hash` မှတစ်ဆင့် အစုအဝေးသို့ ကတိပြုသည်၊ ထို့ကြောင့် သုံးစွဲသူများသည် ပင်နံပါတ်ကို ချိတ်နိုင်သည်။
DA ကတိကဝတ်များ သို့မဟုတ် အထောက်အထားများကို အတည်ပြုသည့်အခါတွင် တက်ကြွသောမူဝါဒကို သတ်မှတ်ထားသည်။ ဤအဆုံးမှတ်ကို ရယူပါ။
လမ်းကြော၏မူဝါဒနှင့် လက်ရှိနှင့်ကိုက်ညီကြောင်း သေချာစေရန် အထောက်အထားများ မတည်ဆောက်မီ
အစုအဝေး hash ။ ကတိကဝတ်စာရင်း/အဆုံးမှတ်များသည် တူညီသောအစုအစည်းများပါရှိသောကြောင့် SDK များ
တက်ကြွသောမူဝါဒသတ်မှတ်ထားသည့် သက်သေအထောက်အထားကို ချည်နှောင်ရန် အပိုအသွားအပြန် မလိုအပ်ပါ။

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

မှာယူထားသော မူဝါဒစာရင်း နှင့် a ပါရှိသော `DaProofPolicyBundle` ကို ပြန်ပေးသည်။
`policy_hash` ထို့ကြောင့် SDK များသည် ဘလောက်တစ်ခုထုတ်လုပ်သောအခါ အသုံးပြုသည့်ဗားရှင်းကို ပင်ထိုးနိုင်သည်။ ဟိ
hash ကို Norito-ကုဒ်လုပ်ထားသော မူဝါဒ ခင်းကျင်းမှုအပေါ် တွက်ချက်ပြီး ပြောင်းလဲသည့်အခါတိုင်း၊
Lane ၏ `proof_scheme` ကို အပ်ဒိတ်လုပ်ထားပြီး သုံးစွဲသူများအကြား ပျံ့လွင့်မှုကို သိရှိနိုင်စေသည်
သိမ်းဆည်းထားသော အထောက်အထားများနှင့် ကွင်းဆက်ဖွဲ့စည်းမှု။

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
```> အကောင်အထည်ဖော်မှုမှတ်စု- ဤ payloads အတွက် canonical Rust ကိုယ်စားပြုမှုများသည် ယခုအောက်တွင် ရှိနေပါသည်။
> `iroha_data_model::da::types`၊ `iroha_data_model::da::ingest` ရှိ တောင်းဆိုချက်/ပြေစာထုပ်ပိုးမှုများပါရှိသော
> နှင့် `iroha_data_model::da::manifest` ရှိ ထင်ရှားသော ဖွဲ့စည်းပုံ။

`compression` အကွက်သည် ခေါ်ဆိုသူများသည် ဝန်ဆောင်ခကို မည်သို့ပြင်ဆင်ကြောင်း ကြော်ငြာသည်။ Torii လက်ခံသည်။
`identity`၊ `gzip`၊ `deflate` နှင့် `zstd` မတိုင်မီ bytes အား ပွင့်လင်းမြင်သာစွာ ချုံ့ခြင်း
ဟက်ခြင်း၊ ချခြင်း နှင့် ရွေးချယ်နိုင်သော မန်နီးဖက်စ်များကို အတည်ပြုခြင်း။

### မှန်ကန်ကြောင်း စစ်ဆေးချက်စာရင်း

1. တောင်းဆိုချက် Norito ခေါင်းစီး `DaIngestRequest` ကိုက်ညီကြောင်း အတည်ပြုပါ။
2. `total_size` သည် canonical (decompressed) payload length နှင့် ကွာခြားသည် သို့မဟုတ် configured max ထက်ကျော်လွန်ပါက မအောင်မြင်ပါ။
3. `chunk_size` ချိန်ညှိမှု (ပါဝါ-နှစ်ခု၊ = 2 ကို သေချာပါစေ။
5. `retention_policy.required_replica_count` သည် အုပ်ချုပ်မှုအခြေခံလိုင်းအား လေးစားရမည်။
6. Canonical hash (လက်မှတ်အကွက်မှအပ မပါ) ကို လက်မှတ်စစ်ခြင်း
7. payload hash + metadata တူညီခြင်းမရှိပါက `client_blob_id` ပွားခြင်းကို ငြင်းပယ်ပါ။
8. `norito_manifest` ကို ပံ့ပိုးပေးသောအခါ၊ schema + hash ကိုက်ညီမှုများကို ပြန်လည်တွက်ချက်အတည်ပြုပါ
   အတုံးလိုက်ပြီးနောက် ထင်ရှားသည်။ မဟုတ်ပါက node သည် manifest ကိုထုတ်ပေးပြီး ၎င်းကို သိမ်းဆည်းသည်။
9. ပြင်ဆင်ထားသော ပုံတူပွားခြင်းမူဝါဒကို ကျင့်သုံးပါ- Torii သည် တင်သွင်းထားသောစာကို ပြန်လည်ရေးသားသည်
   `RetentionPolicy` နှင့် `torii.da_ingest.replication_policy` (ကြည့်ပါ
   `replication_policy.md`) နှင့် ထိန်းသိမ်းထားသည့် ကြိုတင်တည်ဆောက်ထားသော သရုပ်များကို ငြင်းပယ်သည်
   မက်တာဒေတာသည် ပြဌာန်းထားသော ပရိုဖိုင်နှင့် မကိုက်ညီပါ။

### Chunking & Replication Flow1. payload ကို `chunk_size` တွင် အပိုင်းလိုက်၊ အတုံးတစ်ခုလျှင် BLAKE3 + Merkle root ကို တွက်ချက်ပါ။
2. တည်ဆောက်ခြင်း Norito `DaManifestV1` (ဖွဲ့စည်းပုံအသစ်) အပိုင်းအစများကို ကတိကဝတ်များရယူခြင်း (အခန်းကဏ္ဍ/group_id)၊
   ဖျက်ပစ်သည့် အပြင်အဆင် (အတန်းနှင့် ကော်လံ တူညီမှု အရေအတွက်များ အပေါင်း `ipa_commitment`)၊ ထိန်းသိမ်းမှု မူဝါဒ၊
   နှင့် metadata။
3. `config.da_ingest.manifest_store_dir` အောက်တွင် canonical manifest bytes ကို တန်းစီပါ
   (Torii သည် `manifest.encoded` ဖိုင်များကို lane/epoch/sequence/ticket/ fingerprint ဖြင့် သော့ခတ်ထားသော `manifest.encoded` ဖြင့်ရေးသည်) ထို့ကြောင့် SoraFS
   Orchestration သည် ၎င်းတို့ကို ထည့်သွင်းနိုင်ပြီး သိုလှောင်မှုလက်မှတ်ကို ဆက်တိုက်ဒေတာနှင့် ချိတ်ဆက်နိုင်သည်။
4. အုပ်ချုပ်မှုတက်ဂ် + မူဝါဒဖြင့် `sorafs_car::PinIntent` မှတစ်ဆင့် ပင်ထိုးရည်ရွယ်ချက်များကို ထုတ်ဝေပါ။
5. လေ့လာသူများကို အသိပေးရန် Norito ဖြစ်ရပ် `DaIngestPublished` ကို ထုတ်လွှတ်သည် (အလင်းဖောက်သည်များ၊
   အုပ်ချုပ်မှု၊ ခွဲခြမ်းစိတ်ဖြာမှု)။
6. Return `DaIngestReceipt` (Torii DA ဝန်ဆောင်မှုကီးဖြင့် လက်မှတ်ထိုး) နှင့် ထည့်ပါ
   Base64 Norito ကုဒ်နံပါတ်ပါဝင်သော `Sora-PDP-Commitment` တုံ့ပြန်မှုခေါင်းစီး
   SDK များသည် နမူနာမျိုးစေ့ကို ချက်ချင်း သိမ်းဆည်းထားနိုင်သောကြောင့် ရရှိလာသော ကတိကဝတ်များ။
   ယခုပြေစာတွင် `rent_quote` (`DaRentQuote`) နှင့် `stripe_layout` တို့ကို မြှုပ်နှံထားသည်
   ထို့ကြောင့် တင်ပြသူများသည် XOR တာဝန်များ၊ အရန်ရှယ်ယာများ၊ PDP/PoTR ဘောနပ်စ်မျှော်လင့်ချက်များကို ဖော်ပြနိုင်သည်၊
   နှင့် ရန်ပုံငွေမတည်မီ သိုလှောင်မှု-လက်မှတ် မက်တာဒေတာနှင့်အတူ 2D မက်ထရစ်ကို ဖျက်ပစ်သည်။
7. ရွေးချယ်နိုင်သော မှတ်ပုံတင်ခြင်း မက်တာဒေတာ-
   - `da.registry.alias` — pin registry entry ကို ရှာဖွေရန် အများသူငှာ၊ ကုဒ်မထားသော UTF-8 alias string။
   - `da.registry.owner` — မှတ်ပုံတင်ခြင်းပိုင်ဆိုင်မှုကို မှတ်တမ်းတင်ရန်အတွက် အများသူငှာ၊ ကုဒ်မထားသော `AccountId` စာကြောင်း။
   Torii သည် ၎င်းတို့ကို ထုတ်လုပ်ထားသော `DaPinIntent` သို့ မိတ္တူကူးထားသောကြောင့် downstream pin processing သည် aliases ကို ချည်နှောင်နိုင်သည်
   ကုန်ကြမ်း မက်တာဒေတာမြေပုံကို ပြန်လည်ခွဲခြမ်းစိတ်ဖြာခြင်းမရှိဘဲ ပိုင်ရှင်များ၊ ပုံမမှန်သော သို့မဟုတ် ဗလာတန်ဖိုးများကို ပယ်ချပါသည်။
   စားသုံးမှုအတည်ပြုချက်။

## သိုလှောင်မှု / Registry အပ်ဒိတ်များ

- `sorafs_manifest` ကို `DaManifestV1` ဖြင့် ချဲ့ထွင်ပြီး အဆုံးအဖြတ်ပိုင်းခြားမှုကို ဖွင့်ပေးသည်။
- ဗားရှင်းလုပ်ထားသော payload ရည်ညွှန်းချက်ဖြင့် registry stream အသစ် `da.pin_intent` ကိုထည့်ပါ။
  hash + လက်မှတ် ID ကိုဖော်ပြပါ။
- စားသုံးနိုင်မှု latency ကိုခြေရာခံရန်၊ အတုံးလိုက်အတုံးလိုက်အတုံးလိုက်အခဲလိုက်ပြုလုပ်ရန်၊
  ပုံတူကူးယူခြင်း နှင့် ပျက်ကွက်မှု အရေအတွက်။
- Torii `/status` တုံ့ပြန်မှုများသည် နောက်ဆုံးပေါ်ထွက်ပေါ်နေသည့် `taikai_ingest` ခင်းကျင်းတစ်ခု ပါ၀င်သည်
  ကုဒ်ဒါမှထည့်သွင်းခြင်း latency၊ တိုက်ရိုက်-အစွန်း ပျံ့လွင့်ခြင်းနှင့် (အစုအဝေး၊ ထုတ်လွှင့်မှု) တစ်ခုချင်းအလိုက် အမှားကောင်တာ DA-9
  Prometheus ကို မဖြုန်းတီးဘဲ node များမှ ကျန်းမာရေး လျှပ်တစ်ပြက်ပုံများကို တိုက်ရိုက်ထည့်သွင်းရန် ဒက်ရှ်ဘုတ်များ။

## စမ်းသပ်ခြင်းဗျူဟာ- schema validation၊ လက်မှတ်စစ်ဆေးမှုများ၊ ထပ်နေသောရှာဖွေတွေ့ရှိမှုအတွက်ယူနစ်စမ်းသပ်မှုများ။
- `DaIngestRequest` ၏ Norito ကုဒ်နံပါတ်ကို အတည်ပြုသည့် ရွှေရောင်စစ်ဆေးမှုများ၊
- ပေါင်းစည်းခြင်းကြိုးသည် SoraFS + registry ကို အတုယူ၍ အတုံးအခဲ + ပင်နံပါတ် စီးဆင်းမှုများကို အခိုင်အမာ ပေါင်းစပ်ထားသည်။
- ကျပန်းဖျက်ပစ်သည့်ပရိုဖိုင်များနှင့် ထိန်းသိမ်းမှုပေါင်းစပ်မှုများ ပါဝင်သည့် ပိုင်ဆိုင်မှုစစ်ဆေးမှုများ။
- ပုံသဏ္ဍာန်မမှန်သော မက်တာဒေတာကို ကာကွယ်ရန် Norito ပေးချေမှုများအား ရှုပ်ယှက်ခတ်ခြင်း။
- blob အတန်းတိုင်းအတွက် ရွှေရောင်ပွဲများ
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` အဖော်အတုံးတစ်ခု
  `fixtures/da/ingest/sample_chunk_records.txt` တွင် စာရင်းပေးထားသည်။ လျစ်လျူရှုစမ်းပါ။
  `regenerate_da_ingest_fixtures` သည် ပွဲစဉ်များကို လန်းဆန်းစေသည်။
  `manifest_fixtures_cover_all_blob_classes` ဗားရှင်းအသစ် `BlobClass` ကို ထည့်လိုက်သည်နှင့် ပျက်သွားသည်
  Norito/JSON အတွဲကို အပ်ဒိတ်မလုပ်ဘဲ။ ၎င်းသည် Torii၊ SDKs နှင့် DA-2 အား အချိန်တိုင်းတွင် ရိုးသားနေပါသည်။
  blob မျက်နှာပြင်အသစ်ကို လက်ခံသည်။【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI & SDK Tooling (DA-8)- `iroha app da submit` (CLI ဝင်ခွင့်အမှတ်အသစ်) သည် ယခုအခါ မျှဝေသုံးစွဲနေသော တည်ဆောက်သူ/ထုတ်ဝေသူအား ခြုံငုံမိသောကြောင့် အော်ပရေတာများ
  Taikai အစုအဝေးစီးဆင်းမှု၏ အပြင်ဘက်တွင် မထင်သလို blobs များကို ထည့်သွင်းနိုင်သည်။ အမိန့်ပေးသည်။
  `crates/iroha_cli/src/commands/da.rs:1` နှင့် payload ကိုအသုံးပြုသည်၊ ဖျက်/သိမ်းထားသော ပရိုဖိုင်နှင့်
  CLI ဖြင့် canonical `DaIngestRequest` ကို မလက်မှတ်မထိုးမီ ရွေးချယ်နိုင်သော မက်တာဒေတာ/မန်နီးဖက်စ်ဖိုင်များ
  config key အောင်မြင်သော လုပ်ဆောင်မှုများသည် `da_request.{norito,json}` နှင့် `da_receipt.{norito,json}` အောက်တွင် ဆက်ရှိနေသည်
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` မှတဆင့် override) ထို့ကြောင့် ပစ္စည်းများကို ထုတ်ဝေနိုင်သည်
  စားသုံးနေစဉ်အသုံးပြုသည့် Norito bytes အတိအကျကို မှတ်တမ်းတင်ပါ။
- command သည် `client_blob_id = blake3(payload)` သို့ ပုံသေသတ်မှတ်ထားသော်လည်း မှတဆင့် overrides ကိုလက်ခံပါသည်။
  `--client-blob-id`၊ မက်တာဒေတာ JSON မြေပုံများ (`--metadata-json`) နှင့် ကြိုတင်ထုတ်လုပ်ထားသော မန်နီးဖက်စ်များကို ဂုဏ်ပြုပါသည်။
  (`--manifest`) နှင့် စိတ်ကြိုက်အတွက် အော့ဖ်လိုင်းပြင်ဆင်မှုအပြင် `--endpoint` အတွက် `--endpoint` ကို ပံ့ပိုးပေးသည်
  Torii တန်ဆာပလာများ။ ပြေစာကို JSON ကို ပိတ်ပြီး disk သို့ စာရေးထားသည့်အပြင် stdout သို့ ရိုက်နှိပ်ထားသည်။
  DA-8 "submit_blob" ကိရိယာတန်ဆာပလာလိုအပ်ချက်နှင့် SDK တူညီမှုလုပ်ငန်းကို ပိတ်ဆို့ခြင်းအား ပြန်ဖွင့်ခြင်း။
- `iroha app da get` သည် စွမ်းအားရှိပြီးသား ရင်းမြစ်အစုံလိုက်သံစုံတီးဝိုင်းအတွက် DA-အာရုံစူးစိုက်မှုကို ပေါင်းထည့်သည်
  `iroha app sorafs fetch`။ အော်ပရေတာများသည် ၎င်းကို manifest + chunk-plan artefacts (`--manifest`၊
  `--plan`၊ `--manifest-id`) ** သို့မဟုတ် ** Torii သိုလှောင်မှုလက်မှတ်ကို `--storage-ticket` မှတစ်ဆင့် ရိုးရိုးရှင်းရှင်း ဖြတ်သန်းပါ။ ဟို
  လက်မှတ်လမ်းကြောင်းကို အသုံးပြုပြီး CLI သည် မန်နီးဖက်စ်ကို `/v2/da/manifests/<ticket>` မှ ဆွဲထုတ်ကာ အတွဲလိုက်ကို ဆက်ရှိနေသည်
  `artifacts/da/fetch_<timestamp>/` အောက်တွင် (`--manifest-cache-dir`) ဖြင့် ပြန်ရေးသည် **manifest မှ ဆင်းသက်လာသည်
  `--manifest-id` အတွက် hash**၊ ထို့နောက် ပံ့ပိုးပေးထားသော `--gateway-provider` ဖြင့် သံစုံတီးဝိုင်းကို လုပ်ဆောင်သည်
  စာရင်း။ ဂိတ်ဝေး ID သည် ရှိနေစဉ် Payload အတည်ပြုခြင်းတွင် ထည့်သွင်းထားသော CAR/`blob_hash` အချေအတင်ပေါ်တွင် မှီခိုနေရဆဲဖြစ်သည်။
  ယခုဖော်ပြချက် hash သည် client များနှင့် validators သည် blob identifier တစ်ခုတည်းကိုမျှဝေပါသည်။ အဆင့်မြင့် အဖုများ အားလုံး
  SoraFS ထုတ်ယူသည့် မျက်နှာပြင်သည် နဂိုအတိုင်း (ထင်ရှားသော စာအိတ်များ၊ သုံးစွဲသူ တံဆိပ်များ၊ အစောင့်ကက်ရှ်များ၊ အမည်ဝှက် သယ်ယူပို့ဆောင်ရေး
  အစားထိုးမှုများ၊ ရမှတ်ဘုတ်တင်ပို့ခြင်းနှင့် `--output` လမ်းကြောင်းများ) နှင့် manifest endpoint မှတဆင့် လွှမ်းမိုးနိုင်သည်
  စိတ်ကြိုက် Torii hosts အတွက် `--manifest-endpoint`၊ ထို့ကြောင့် အဆုံးမှ အဆုံးအထိရရှိနိုင်မှုစစ်ဆေးမှုများသည် လုံး၀အောက်တွင်ရှိသည်။
  `da` တီးမှုတ်တီးခတ်သူ လော့ဂျစ်ကို ထပ်ခြင်းမပြုဘဲ namespace။
- `iroha app da get-blob` သည် `GET /v2/da/manifests/{storage_ticket}` မှတစ်ဆင့် Torii မှ တိုက်ရိုက် Canonical manifest များကို ဆွဲထုတ်သည်။
  ယခု command သည် artefacts များကို manifest hash (blob id) ဖြင့် အညွှန်းရေးပေးပါသည်။
  `manifest_{manifest_hash}.norito`၊ `manifest_{manifest_hash}.json` နှင့် `chunk_plan_{manifest_hash}.json`
  အတိအကျကို သံယောင်လိုက်နေစဉ် `artifacts/da/fetch_<timestamp>/` (သို့မဟုတ် အသုံးပြုသူမှပေးသော `--output-dir`) အောက်တွင်
  `iroha app da get` တောင်းခံလွှာ (`--manifest-id` အပါအဝင်) နောက်ဆက်တွဲ တီးမှုတ်သူ ထုတ်ယူမှုအတွက် လိုအပ်သည်။
  ၎င်းသည် အော်ပရေတာများအား manifest spool လမ်းညွှန်များမှနေ၍ ဖမ်းယူသူသည် ၎င်းကို အမြဲတမ်းအသုံးပြုကြောင်း အာမခံပါသည်။
  Torii မှ ထုတ်လွှတ်သော လက်မှတ်ရေးထိုးထားသော ရှေးဟောင်းပစ္စည်းများ။ JavaScript Torii client သည် ဤစီးဆင်းမှုကို ထင်ဟပ်စေသည်။
  `ToriiClient.getDaManifest(storageTicketHex)` သည် Swift SDK ကို ယခု ထုတ်ဖော်နေစဉ်
  `ToriiClient.getDaManifestBundle(...)`။ နှစ်ခုလုံးကုဒ်သုံးထားသော Norito bytes၊ manifest JSON၊ manifest hash၊SDK ခေါ်ဆိုသူများသည် CLI သို့ ပစ်မချဘဲ တီးမှုတ်ခြင်းဆက်ရှင်များကို ရေဓာတ်ဖြည့်ပေးနိုင်သောကြောင့် အတုံးလိုက်အစီအမံများ နှင့် Swift
  ဖောက်သည်များသည် ၎င်းအစုအဝေးများကို ဇာတိမှတဆင့် ပိုက်ရန် `fetchDaPayloadViaGateway(...)` ကို ထပ်မံခေါ်ဆိုနိုင်သည်
  SoraFS သံစုံတီးဝိုင်း ထုပ်ပိုးခြင်း။【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v2/da/manifests` တုံ့ပြန်မှုများ ယခု `manifest_hash` နှင့် CLI + SDK အကူအညီပေးသူများ (`iroha app da get`၊
  `ToriiClient.fetchDaPayloadViaGateway` နှင့် Swift/JS gateway wrappers များ) သည် ဤ digest အဖြစ် မှတ်ယူသည် ။
  ထည့်သွင်းထားသော CAR/blob hash ကို ဆက်လက်စစ်ဆေးနေစဉ် canonical manifest identifier
- `iroha app da rent-quote` သည် ပံ့ပိုးပေးထားသော သိုလှောင်မှုအရွယ်အစားအတွက် အဆုံးအဖြတ်ပေးသော ငှားရမ်းခနှင့် မက်လုံးပေးမှု အပိုင်းများကို တွက်ချက်သည်
  နှင့် retention window ။ အကူအညီပေးသူက လက်ရှိအသုံးပြုနေသော `DaRentPolicyV1` (JSON သို့မဟုတ် Norito bytes) သို့မဟုတ်
  built-in မူရင်း၊ မူဝါဒကို အတည်ပြုပြီး JSON အကျဉ်းချုပ် (`gib`၊ `months`၊ မူဝါဒ မက်တာဒေတာ၊
  နှင့် `DaRentQuote` အကွက်များ) ထို့ကြောင့် စာရင်းစစ်များသည် အုပ်ချုပ်မှုမိနစ်အတွင်း မလိုအပ်ဘဲ XOR ကောက်ခံမှုများကို အတိအကျကိုးကားနိုင်သည်။
  ad hoc script တွေရေးတယ်။ ယခု command သည် JSON မတိုင်မီ တစ်ကြောင်း `rent_quote ...` အကျဉ်းချုပ်ကိုလည်း ထုတ်လွှတ်သည်
  ဖြစ်ရပ်များအတွင်း ကိုးကားချက်များထုတ်ပေးသည့်အခါ ကွန်ဆိုးလ်မှတ်တမ်းများနှင့် runbooks များကို ပိုမိုလွယ်ကူစွာ စကင်န်ဖတ်နိုင်စေရန် payload။
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` ကိုဖြတ်ပါ (သို့မဟုတ် အခြားလမ်းကြောင်း)
  လှပသောပုံနှိပ်ထားသောအနှစ်ချုပ်ကိုဆက်လက်ထိန်းသိမ်းထားရန်နှင့် `--policy-label "governance ticket #..."` ကိုအသုံးပြုသည့်အခါ၊
  artefact သည် တိကျသောမဲ/ config အတွဲတစ်ခုကို ကိုးကားရန် လိုအပ်သည်။ CLI သည် စိတ်ကြိုက်တံဆိပ်များကို ချုံ့ပြီး ဗလာကို ပယ်ချသည်။
  အထောက်အထားအစုအဝေးများတွင် `policy_source` တန်ဖိုးများကို အဓိပ္ပါယ်ရှိစေရန် ကြိုးများ။ ကြည့်ပါ။
  subcommand အတွက် `crates/iroha_cli/src/commands/da.rs` နှင့် `docs/source/da/rent_policy.md`
  မူဝါဒအစီအစဉ်အတွက်။ 【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- Pin registry parity ကို ယခု SDKs တွင် တိုးချဲ့ထားသည်- `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK သည် `iroha app sorafs pin register` မှအသုံးပြုသော တိကျသော payload ကိုတည်ဆောက်သည်၊ canonical ကိုလိုက်နာသည်
  chunker metadata၊ pin policy၊ alias proofs နှင့် ပို့စ်မတင်မီ ဆက်ခံမည့် digest များ
  `/v2/sorafs/pin/register`။ ၎င်းသည် CI ဘော့တ်များနှင့် အလိုအလျောက်လုပ်ဆောင်မှုကို CLI ထံသို့ မည်သည့်အချိန်တွင် ဖယ်ရှားခြင်းမှ ကာကွယ်ပေးသည်။
  ထင်ရှားသော မှတ်ပုံတင်မှုများကို မှတ်တမ်းတင်ခြင်း နှင့် ကူညီသူသည် TypeScript/README လွှမ်းခြုံမှုဖြင့် ပို့ဆောင်ပေးသောကြောင့် DA-8 ၏
  "submit/get/prove" tooling parity သည် Rust/Swift နှင့်အတူ JS တွင် အပြည့်အဝ စိတ်ကျေနပ်မှုရှိပါသည်။ 【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` သည် အထက်ဖော်ပြပါ အားလုံးကို ချိတ်ဆက်ထားသည်- ၎င်းသည် သိုလှောင်မှု လက်မှတ်တစ်ခုယူသည်၊ ဒေါင်းလုဒ်လုပ်သည်
  canonical manifest အစုအစည်းသည် အရင်းအမြစ်ပေါင်းစုံ သံစုံတီးဝိုင်း (`iroha app sorafs fetch`) ကို လုပ်ဆောင်သည်
  ပေးထားသော `--gateway-provider` စာရင်း၊ ဒေါင်းလုဒ်လုပ်ထားသော payload + scoreboard အောက်တွင် ဆက်ရှိနေသည်
  `artifacts/da/prove_availability_<timestamp>/` နှင့် လက်ရှိ PoR အထောက်အကူကို ချက်ချင်းခေါ်သည်။
  ရယူထားသော ဘိုက်များကို အသုံးပြု၍ (`iroha app da prove`) အော်ပရေတာများသည် သံစုံတီးဝိုင်းခလုတ်များကို ညှိနိုင်သည်။
  (`--max-peers`၊ `--scoreboard-out`၊ manifest endpoint overrides) နှင့် သက်သေနမူနာ
  (`--sample-count`၊ `--leaf-index`၊ `--sample-seed`) တစ်ခုတည်းသော command သည် artefacts ကိုထုတ်လုပ်နေစဉ်
  DA-5/DA-9 စာရင်းစစ်များမှ မျှော်လင့်ထားသည်- ပေးဆောင်မှုမိတ္တူ၊ အမှတ်စာရင်းအထောက်အထားများနှင့် JSON အထောက်အထား အနှစ်ချုပ်များ။- `da_reconstruct` (DA-6 တွင်အသစ်) သည် အတုံးလိုက်မှထုတ်လွှတ်သော အတုံးအခဲလမ်းညွှန်ကို ခွဲခြမ်းစိတ်ဖြာခြင်းအပြင် Canonical manifest ကိုဖတ်သည်
  စတိုးဆိုင် (`chunk_{index:05}.bin` အပြင်အဆင်) နှင့် စစ်ဆေးအတည်ပြုနေစဉ် ပေးဆောင်မှုအား ပြန်လည်စုစည်းပေးသည် ။
  Blake3 ကတိကဝတ်တိုင်း။ CLI သည် `crates/sorafs_car/src/bin/da_reconstruct.rs` အောက်တွင်နေထိုင်ပြီး သင်္ဘောအဖြစ် ပို့ဆောင်သည်။
  SoraFS ကိရိယာအစုံအလင်၏ အစိတ်အပိုင်း။ ပုံမှန်စီးဆင်းမှု-
  1. `iroha app da get-blob --storage-ticket <ticket>` `manifest_<manifest_hash>.norito` နှင့် အတုံးအခဲအစီအစဉ်ကို ဒေါင်းလုဒ်လုပ်ရန်။
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (သို့မဟုတ် `iroha app da prove-availability`၊ အောက်တွင် ထုတ်ယူထားသော ပစ္စည်းများကို ရေးသည်။
     `artifacts/da/prove_availability_<ts>/` နှင့် `chunks/` လမ်းညွှန်အတွင်းတွင် တစ်ခုချင်းအတုံးဖိုင်များကို ဆက်လက်တည်ရှိနေပါသည်။
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`။

  Regression fixture သည် `fixtures/da/reconstruct/rs_parity_v1/` အောက်တွင် နေထိုင်ပြီး အပြည့်အစုံကို ဖမ်းယူသည်
  နှင့် `tests::reconstructs_fixture_with_parity_chunks` မှအသုံးပြုသော အတုံးလိုက်မက်ထရစ် (ဒေတာ + တူညီမှု)။ ၎င်းကို ပြန်ထုတ်ပါ။

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  ခံစစ်မှ ထုတ်လွှတ်သည်-

  - `manifest.{norito.hex,json}` — canonical `DaManifestV1` ကုဒ်နံပါတ်များ။
  - `chunk_matrix.json` — doc/testing အကိုးအကားများအတွက် အညွှန်း/အော့ဖ်ဆက်/အလျား/ဒိုင်ဂျစ်/ကွာဟမှုအတန်းများ။
  - `chunks/` — `chunk_{index:05}.bin` ဒေတာနှင့် parity shard နှစ်ခုလုံးအတွက် payload အချပ်များ။
  - `payload.bin` — parity-aware harness test မှအသုံးပြုသော အဆုံးအဖြတ်ပေးဆောင်မှု။
  - `commitment_bundle.{json,norito.hex}` — နမူနာ `DaCommitmentBundle` စာရွက်စာတမ်း/စစ်ဆေးမှုများအတွက် အဆုံးအဖြတ်ပေးသော KZG ကတိကဝတ်။

  ကြိုးသည် ပျောက်ဆုံးနေသော သို့မဟုတ် ဖြတ်တောက်ထားသော အတုံးများကို ငြင်းဆန်သည်၊ `blob_hash` နှင့် ပတ်သက်သည့် နောက်ဆုံး payload Blake3 hash ကို စစ်ဆေးသည်၊
  နှင့် အကျဉ်းချုပ် JSON blob (payload bytes၊ chunk count, storage ticket) ကို ထုတ်လွှတ်သောကြောင့် CI သည် ပြန်လည်တည်ဆောက်မှုကို အခိုင်အမာရနိုင်သည်
  အထောက်အထား။ ၎င်းသည် အော်ပရေတာများနှင့် QA တို့၏ အဆုံးအဖြတ်ပေးသော ပြန်လည်တည်ဆောက်ရေးကိရိယာအတွက် DA-6 လိုအပ်ချက်ကို ပိတ်လိုက်သည်။
  အလုပ်များသည် စိတ်ကြိုက် script များကို ကြိုးတပ်ခြင်းမပြုဘဲ ခေါ်ဆိုနိုင်ပါသည်။

## TODO Resolution အကျဉ်းချုပ်

ယခင်က ပိတ်ဆို့ထားသော စားသုံးမှု TODO အားလုံးကို အကောင်အထည် ဖော်ပြီး စစ်ဆေးပြီးပါပြီ-- **Compression အရိပ်အမြွက်** — Torii သည် ခေါ်ဆိုသူမှပေးသော အညွှန်းများ (`identity`၊ `gzip`၊ `deflate`၊
  `zstd`) နှင့် validation မလုပ်မီ payload များကို ပုံမှန်ဖြစ်အောင် ပြုလုပ်ပေးသောကြောင့် canonical manifest hash သည် ၎င်းနှင့် ကိုက်ညီပါသည်။
  ချုံ့လိုက်သော bytes။【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **အုပ်ချုပ်မှု-သီးသန့် မက်တာဒေတာ ကုဒ်ဝှက်ခြင်း** — Torii သည် ယခုအခါ အုပ်ချုပ်မှု မက်တာဒေတာကို ကုဒ်ဝှက်ထားသည်
  ChaCha20-Poly1305 သော့ကို configure လုပ်ထားပြီး၊ မကိုက်ညီသော အညွှန်းများကို ပယ်ချပြီး ရှင်းလင်းပြတ်သားသော နှစ်ခုကို ပြသည်
  ဖွဲ့စည်းမှုခလုတ်များ (`torii.da_ingest.governance_metadata_key_hex`၊
  `torii.da_ingest.governance_metadata_key_label`) လှည့်ပတ်မှုကို အဆုံးအဖြတ်ပေးသည်။【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **ကြီးမားသော payload streaming** — အပိုင်းပေါင်းများစွာ ထည့်သွင်းအသုံးပြုမှုကို တိုက်ရိုက်ထုတ်လွှင့်နေသည်။ ဖောက်သည်များသည် အဆုံးအဖြတ်ပေးသော စီးဆင်းမှုဖြစ်သည်။
  `DaIngestChunk` သော့ခတ်ထားသော စာအိတ်များကို `client_blob_id`၊ Torii သည် အချပ်တစ်ခုစီကို အတည်ပြုပြီး ၎င်းတို့ကို အဆင့်လိုက်ပြုလုပ်သည်
  `manifest_store_dir` အောက်တွင် နှင့် `is_last` အလံ ပြုတ်ကျသည်နှင့် တပြိုင်နက် ထင်ရှားသော အနုမြူကို ပြန်လည်တည်ဆောက်သည်၊
  ခေါ်ဆိုမှုတစ်ခုတည်းဖြင့် အပ်လုဒ်တင်မှုများဖြင့် မြင်တွေ့ရသည့် RAM spikes များကို ဖယ်ရှားခြင်း။【crates/iroha_torii/src/da/ingest.rs:392】
- **Manifest ဗားရှင်း** — `DaManifestV1` သည် တိကျသော `version` အကွက်ကို သယ်ဆောင်ထားပြီး Torii သည် ငြင်းဆိုသည်
  manifest အပြင်အဆင်အသစ်များ ပို့ဆောင်သည့်အခါတွင် အဆုံးအဖြတ်ပေးသော အဆင့်မြှင့်တင်မှုများကို အာမခံသော အမည်မသိဗားရှင်းများ။ 【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR ချိတ်များ** — PDP ကတိကဝတ်များသည် အတုံးလိုက်စတိုးမှ တိုက်ရိုက်ဆင်းသက်လာပြီး ဆက်ရှိနေသည်
  DA-5 အချိန်ဇယားဆွဲသူများသည် canonical data မှနမူနာစိန်ခေါ်မှုများကိုစတင်နိုင်သည်၊ အဆိုပါ
  `Sora-PDP-Commitment` ခေါင်းစီးကို ယခု `/v2/da/ingest` နှင့် `/v2/da/manifests/{ticket}` နှစ်မျိုးလုံးဖြင့် ပို့ဆောင်ပေးနေပါပြီ
  တုံ့ပြန်မှုများကြောင့် SDKs များသည် အနာဂတ်စုံစမ်းစစ်ဆေးမှုများကို ကိုးကားမည့် လက်မှတ်ရေးထိုးထားသည့် ကတိကဝတ်များကို ချက်ခြင်းသိရှိနိုင်သည်။【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】
- **Shard cursor journal** — လမ်းသွား မက်တာဒေတာသည် `da_shard_id` (`lane_id` သို့ ပုံသေသတ်မှတ်ထားသည်) နှင့်
  ယခု Sumeragi သည် `(shard_id, lane_id)` တွင် အမြင့်ဆုံး `(epoch, sequence)` ကို ဆက်ရှိနေသည်
  `da-shard-cursors.norito` သည် DA spool နှင့် တွဲလျက် ဖြစ်သောကြောင့် ပြန်လည်စတင်ပြီး ပြန်လည်ပြင်ဆင်ထားသော/အမည်မသိလမ်းကြောင်းများကို ချလိုက်ပြီး ဆက်ထားပါ
  အဆုံးအဖြတ်ကို ပြန်ဖွင့်သည်။ in-memory shard cursor အညွှန်းသည် ယခု ကတိကဝတ်များအတွက် အမြန်ပျက်ကွက်ပါသည်။
  လမ်းကြောင်း id ကို ပုံသေလုပ်မည့်အစား မြေပုံမပြထားသော လမ်းကြောများ၊ cursor တိုးတက်မှုနှင့် ပြန်ဖွင့်သည့် အမှားများ ပြုလုပ်သည်
  ပြတ်သားစွာနှင့် ပိတ်ဆို့အတည်ပြုချက်သည် သီးခြားတစ်ခုဖြင့် shard-cursor ဆုတ်ယုတ်မှုများကို ငြင်းပယ်သည်။
  `DaShardCursorViolation` အကြောင်းပြချက် + အော်ပရေတာများအတွက် တယ်လီမီတာ အညွှန်းများ။ စတင်ခြင်း/ဖမ်းယူခြင်းများသည် DA ကို ရပ်တန့်လိုက်ပါပြီ။
  Kura သည် အမည်မသိလမ်းကြောတစ်ခု သို့မဟုတ် နောက်ပြန်လှည့်နေသော cursor ပါ၀င်ပြီး ပြစ်မှားမှုကို မှတ်တမ်းတင်ထားလျှင် အညွှန်းကိန်း ရေဓါတ်ဖြည့်ပေးသည်။
  DA ကို မထမ်းဆောင်မီ အော်ပရေတာများက ပြန်လည်ပြင်ဆင်နိုင်စေရန် အမြင့်ပိတ်ဆို့ခြင်း ပြည်နယ်။ 【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs 】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Shard cursor lag တယ်လီမီတာ** — `da_shard_cursor_lag_blocks{lane,shard}` gauge က မည်သို့တင်ပြသည်အတည်ပြုထားတဲ့ အမြင့်ကို ခက်ခက်ခဲခဲနဲ့ လိုက်နေပါတယ်။ ပျောက်ဆုံးနေသော/ဟောင်းနွမ်း/အမည်မသိလမ်းကြောင်းများသည် နောက်ကျကျန်နေမှုကို သတ်မှတ်ပေးသည်။
  လိုအပ်သော အမြင့် (သို့မဟုတ် မြစ်ဝကျွန်းပေါ်ဒေသ) နှင့် အောင်မြင်သော တိုးတက်မှုများက ၎င်းကို သုညသို့ ပြန်လည်သတ်မှတ်ထားသောကြောင့် တည်ငြိမ်သောအခြေအနေသည် ပြားပြားနေမည်ဖြစ်သည်။
  အော်ပရေတာများသည် သုညမဟုတ်သော ပြတ်တောက်မှုများတွင် အချက်ပေးသင့်သည်၊ ပြစ်မှားသောလမ်းအတွက် DA spool/ဂျာနယ်ကို စစ်ဆေးပါ၊
  ပိတ်ဆို့ခြင်းကို ရှင်းလင်းရန် ပြန်လည်မကစားမီ မတော်တဆ ပြန်လည်မျှဝေခြင်းအတွက် လမ်းသွားကတ်တလောက်ကို စစ်ဆေးအတည်ပြုပါ။
  ကွာဟချက်။
- **လျှို့ဝှက်တွက်ချက်ထားသော လမ်းကြောများ** — ဖြင့် အမှတ်အသားပြုထားသော လမ်းကြောင်းများ
  `metadata.confidential_compute=true` နှင့် `confidential_key_version` အဖြစ် ဆက်ဆံသည်
  SMPC/ကုဒ်ဝှက်ထားသော DA လမ်းကြောင်းများ- Sumeragi သည် သုညမဟုတ်သော payload/manifest အချေအတင်များနှင့် သိုလှောင်မှုလက်မှတ်များကို တွန်းအားပေးသည်၊
  ပုံတူသိုလှောင်မှုပရိုဖိုင်များ အပြည့်အစုံကို ပယ်ချပြီး SoraFS လက်မှတ် + မူဝါဒဗားရှင်းကို အညွှန်းမတင်ဘဲ၊
  payload bytes ဖော်ထုတ်ခြင်း။ ပြန်လည်ပြသစဉ်တွင် Kura ထံမှ ပြေစာများသည် ရေဓာတ်ဖြည့်ပေးသောကြောင့် တရားဝင်စစ်ဆေးသူများသည် အလားတူပြန်လည်ရယူသည်။
  ပြန်လည်စတင်ပြီးနောက် လျှို့ဝှက်အချက်အလက်များကို ပြန်လည်စတင်သည်။ 【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## အကောင်အထည်ဖော်မှုမှတ်စုများ- ယခုအခါ Torii ၏ `/v2/da/ingest` အဆုံးမှတ်သည် payload ချုံ့မှုကို ပုံမှန်ဖြစ်စေပြီး ပြန်ဖွင့်သည့် ကက်ရှ်ကို တွန်းအားပေးသည်၊
  Canonical bytes များကို ပိုင်းဖြတ်ပိုင်းဖြတ်ပြီး `DaManifestV1` ကို ပြန်လည်တည်ဆောက်ကာ ကုဒ်လုပ်ထားသော payload ကို ချပစ်လိုက်သည်
  ပြေစာမထုတ်ပေးမီ SoraFS အတွက် `config.da_ingest.manifest_store_dir` သို့ အဆိုပါ
  ကိုင်တွယ်သူသည် `Sora-PDP-Commitment` ခေါင်းစီးကိုလည်း ပူးတွဲပါရှိသည်
  ချက်ချင်း။【crates/iroha_torii/src/da/ingest.rs:220】
- Canonical `DaCommitmentRecord` ကို ဆက်လက်တည်မြဲပြီးနောက်၊ Torii သည် ယခုထွက်လာသည်
  manifest spool ဘေးရှိ `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ဖိုင်။
  ထည့်သွင်းမှုတစ်ခုစီသည် အကြမ်းထည် Norito `PdpCommitment` bytes ဖြင့် မှတ်တမ်းကို စုစည်းထားသောကြောင့် DA-3 အစုအဝေးတည်ဆောက်သူများနှင့်
  DA-5 အချိန်ဇယားဆွဲသူများသည် မန်နီးဖက်စ်များ သို့မဟုတ် အပိုင်းအစများကို ပြန်လည်ဖတ်ရှုခြင်းမပြုဘဲ ထပ်တူထပ်မျှသော ထည့်သွင်းမှုများကို ထည့်သွင်းပါသည်။ 【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK အကူအညီပေးသူများသည် Norito ခွဲခြမ်းစိတ်ဖြာမှုကို ပြန်လည်အသုံးမပြုဘဲ PDP ခေါင်းစီးဘိုက်များကို ထုတ်ဖော်ပြသသည်-
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` ကာဗာ Rust၊ Python `ToriiClient`
  ယခု `decode_pdp_commitment_header` နှင့် `IrohaSwift` တို့သည် မိုဘိုင်းလ်အကူအညီပေးသူများနှင့် ကိုက်ညီသောသင်္ဘောများ
  ဖောက်သည်များသည် ကုဒ်လုပ်ထားသောနမူနာအချိန်ဇယားကို ချက်ချင်းသိမ်းဆည်းနိုင်သည်။【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii သည် `GET /v2/da/manifests/{storage_ticket}` ကိုလည်း ဖော်ထုတ်ပေးသောကြောင့် SDK နှင့် အော်ပရေတာများသည် မန်နီးဖက်စ်များကို ရယူနိုင်သည်။
  နှင့် node ၏ spool directory ကိုမထိဘဲ အစီအစဥ်များကို အပိုင်းပိုင်းဖြတ်ပါ။ တုံ့ပြန်မှုသည် Norito bytes ကို ပြန်ပေးသည်။
  (base64)၊ manifest ပြန်ဆိုထားသော JSON၊ `chunk_plan` JSON blob သည် `sorafs fetch` အတွက် အဆင်သင့်ဖြစ်သည့်အပြင် သက်ဆိုင်ရာ၊
  hex digests (`storage_ticket`၊ `client_blob_id`၊ `blob_hash`၊ `chunk_root`) ထို့ကြောင့် downstream tooling လုပ်နိုင်သည်
  အချေအတင်များကို ပြန်လည်တွက်ချက်ခြင်းမပြုဘဲ တီးမှုတ်ဆရာအား ကျွေးမွေးပြီး တူညီသော `Sora-PDP-Commitment` ခေါင်းစီးအား ထုတ်လွှတ်သည်
  mirror ingest တုံ့ပြန်မှုများ။ မေးမြန်းမှု ကန့်သတ်ချက်တစ်ခုအနေဖြင့် `block_hash=<hex>` ကို ကျော်ဖြတ်ခြင်းသည် အဆုံးအဖြတ်တစ်ခုကို ပြန်ပေးသည်
  `sampling_plan` ပါဝင်သော `block_hash || client_blob_id` (တရားဝင်စစ်ဆေးသူများတွင် မျှဝေထားသည်)
  တောင်းဆိုထားသော `assignment_hash`၊ တောင်းဆိုထားသော `sample_window`၊ နှင့် `(index, role, group)` ပတ်ထားသော tuples နမူနာများ
  2D အစင်းအကွက် တစ်ခုလုံးသည် PoR နမူနာများနှင့် စစ်ဆေးသူများ တူညီသော အညွှန်းကိန်းများကို ပြန်ဖွင့်နိုင်သည်။ နမူနာယူပါ။
  assignment hash တွင် `client_blob_id`၊ `chunk_root` နှင့် `ipa_commitment` တို့ကို ရောနှောထားသည်။ `iroha app da get
  --block-hash ` now writes `sampling_plan_.json` နှင့်အတူ manifest + အတုံးအခဲ အစီအစဉ်ဘေးရှိ
  hash ကို ထိန်းသိမ်းထားပြီး JS/Swift Torii ဖောက်သည်များသည် တူညီသော `assignment_hash_hex` ကို ဖော်ထုတ်ပေးသောကြောင့် validators
  နှင့် သုသေသနများသည် တစ်ခုတည်းသော အဆုံးအဖြတ်ပေးသည့် စုံစမ်းစစ်ဆေးမှုတစ်ခုကို မျှဝေကြသည်။ Torii သည် နမူနာအစီအစဉ်ကို ပြန်ပေးသောအခါ `iroha app da
  prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) အစား
  အော်ပရေတာမှ ချန်လှပ်ထားသော်လည်း PoR သက်သေများသည် တရားဝင်သတ်မှတ်ပေးထားသော တာဝန်များနှင့် ကိုက်ညီအောင် နမူနာယူခြင်း၏
  `--block-hash` override

### ကြီးမားသော Payload Streaming Flowစီစဉ်သတ်မှတ်ထားသော တောင်းဆိုချက် ကန့်သတ်ချက်ထက် ကြီးသော ပိုင်ဆိုင်မှုများကို ထည့်သွင်းရန် လိုအပ်သော ဖောက်သည်များသည် a စတင်သည်။
`POST /v2/da/ingest/chunk/start` ကိုခေါ်ဆိုခြင်းဖြင့် တိုက်ရိုက်ထုတ်လွှင့်ခြင်း Torii သည် a ဖြင့် တုံ့ပြန်သည်။
`ChunkSessionId` (တောင်းဆိုထားသော blob မက်တာဒေတာမှဆင်းသက်လာသော BLAKE3) နှင့် ညှိနှိုင်းထားသော အတုံးအရွယ်အစား။
နောက်ဆက်တွဲ `DaIngestChunk` တောင်းဆိုမှုတစ်ခုစီသည်-

- `client_blob_id` — နောက်ဆုံး `DaIngestRequest` နှင့် ဆင်တူသည်။
- `chunk_session_id` — အချပ်များကို လည်ပတ်နေသည့် စက်ရှင်နှင့် ချိတ်ဆက်ထားသည်။
- `chunk_index` နှင့် `offset` — အဆုံးအဖြတ်ပေးသော အမိန့်ကို ပြဋ္ဌာန်းပါ။
- `payload` — ညှိနှိုင်းထားသော အတုံးအရွယ်အစားအထိ။
- `payload_hash` — အချပ်၏ BLAKE3 hash ဖြစ်သောကြောင့် Torii သည် blob တစ်ခုလုံးကို buffering မလုပ်ဘဲ validate လုပ်နိုင်သည်။
- `is_last` — terminal အချပ်ကို ညွှန်ပြသည်။

Torii သည် `config.da_ingest.manifest_store_dir/chunks/<session>/` အောက်တွင် တရားဝင်အတည်ပြုထားသော အချပ်များကို ဆက်ရှိနေသည်နှင့်
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
- `manifest.rs` သည် `DaManifestV1` နှင့် `ChunkCommitment` ကို လက်ခံဆောင်ရွက်ပေးထားပြီး၊ Torii ပြီးနောက် Torii ထွက်လာသည်
  အပိုင်းအစများ ပြီးပါပြီ။ 【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` သည် မျှဝေထားသော aliases (`BlobDigest`၊ `RetentionPolicy`၊
  `ErasureProfile` စသည်ဖြင့်) နှင့် အောက်တွင် မှတ်တမ်းတင်ထားသော မူရင်းမူဝါဒတန်ဖိုးများကို ကုဒ်လုပ်ပါသည်။【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool ဖိုင်များသည် `config.da_ingest.manifest_store_dir` တွင် ဆင်းသက်ပြီး၊ SoraFS တီးမှုတ်ခြင်းအတွက် အဆင်သင့်ဖြစ်သည်
  သိုလှောင်ခန်းထဲသို့ ဆွဲသွင်းရန် စောင့်ကြည့်သူ။【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi သည် DA အစုအဝေးများကို တံဆိပ်ခတ်ခြင်း သို့မဟုတ် အတည်ပြုသည့်အခါတွင် ထင်ရှားစွာရရှိနိုင်မှုကို တွန်းအားပေးသည်-
  spool သည် manifest ပျောက်နေသည် သို့မဟုတ် hash ကွဲပြားပါက blocks များသည် validation ပျက်ကွက်ပါသည်။
  ကတိကဝတ်များမှ။ 【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

တောင်းဆိုမှု၊ မန်နီးဖက်စ်နှင့် ပြေစာပေးချေမှုများအတွက် အသွားအပြန် အကျုံးဝင်မှုကို ခြေရာခံထားသည်။
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`၊ Norito codec ကို သေချာစေသည်
အပ်ဒိတ်များတွင် တည်ငြိမ်နေပါသည်။【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**ထိန်းသိမ်းထားမှု ပုံသေများ။** အုပ်ချုပ်မှုစနစ်သည် ကာလအတွင်း ကနဦးထိန်းသိမ်းထားမှုမူဝါဒကို အတည်ပြုခဲ့သည်။
SF-6; `RetentionPolicy::default()` မှ ပြဌာန်းထားသော ပုံသေများမှာ-- ပူပြင်းသောအဆင့်- 7 ရက် (`604_800` စက္ကန့်)
- အအေးအဆင့်- ရက် 90 (`7_776_000` စက္ကန့်)
- လိုအပ်သော ပုံတူများ- `3`
- သိုလှောင်မှုအဆင့်- `StorageClass::Hot`
- အုပ်ချုပ်မှု tag: `"da.default"`

လမ်းကြောင်းတစ်ခုကို လက်ခံလိုက်သောအခါ အောက်ပိုင်းအော်ပရေတာများသည် ဤတန်ဖိုးများကို ပြတ်သားစွာ အစားထိုးရပါမည်။
တင်းကျပ်သောလိုအပ်ချက်များ။

## သံချေးဖောက်သည် အထောက်အထားများ

Rust ကလိုင်းယင့်ကို မြှုပ်နှံထားသည့် SDK များသည် CLI သို့ ဖယ်ထုတ်ရန် မလိုအပ်တော့ပါ။
Canonical PoR JSON အတွဲကို ထုတ်လုပ်သည်။ `Client` သည် အကူအညီပေးသူနှစ်ဦးကို ဖော်ထုတ်ပေးသည်-

- `build_da_proof_artifact` သည် ထုတ်ပေးသော အတိအကျ ဖွဲ့စည်းပုံကို ပြန်ပေးသည်။
  ပေးထားသော manifest/payload မှတ်ချက်များအပါအဝင် `iroha app da prove --json-out`
  [`DaProofArtifactMetadata`] မှတဆင့်။【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` သည် builder ကို ခြုံပြီး artefact ကို disk တွင် ဆက်လက်ထားရှိသည်
  (ပုံမှန်အားဖြင့် JSON + လိုင်းအသစ်နောက်သို့ လိုက်နေသည်) ထို့ကြောင့် အလိုအလျောက်စနစ်ဖြင့် ဖိုင်ကို ပူးတွဲနိုင်သည်။
  ထုတ်ပြန်ရန် သို့မဟုတ် အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထား အစုအဝေးများ။【crates/iroha/src/client.rs:3653】

### ဥပမာ

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

ကူညီသူမှထားခဲ့သော JSON payload သည် CLI ကို အကွက်အမည်များနှင့် ကိုက်ညီသည်။
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest`, etc.) ဒါကြောင့် ရှိပြီးသား၊
အလိုအလျောက်စနစ်သည် ဖောမတ်သီးသန့်အကိုင်းအခက်များမပါဘဲ ဖိုင်ကို ကွဲပြား/ပါကေး/တင်နိုင်သည်။

## အထောက်အထားစိစစ်ရေးစံနှုန်း

ကိုယ်စားလှယ်ပေးဆောင်မှုများမတိုင်မီတွင် ကိုယ်စားလှယ်ပေးဆောင်မှုများအတွက် စိစစ်ရေးဘတ်ဂျက်များကို တရားဝင်အတည်ပြုရန် DA အထောက်အထားစံကြိုးကြိုးကို အသုံးပြုပါ။
တင်းကျပ်သောပိတ်ဆို့အဆင့်ထုပ်များ

- `cargo xtask da-proof-bench` သည် manifest/payload pair မှ အတုံးအခဲစတိုးဆိုင်ကို ပြန်လည်တည်ဆောက်သည်၊ နမူနာ PoR
  ပြင်ဆင်ထားသော ဘတ်ဂျက်နှင့် ကိုက်ညီသော အချိန်များကို စိစစ်ခြင်း။ Taikai မက်တာဒေတာကို အလိုအလျောက် ဖြည့်ပေးပါသည်။
  ကြိုးတစ်စုံသည် တစ်သမတ်တည်း မကိုက်ညီပါက ကြိုးသည် ပေါင်းစပ်ဖော်ပြချက်သို့ ပြန်ကျသည်။ `--payload-bytes` တုန်းက
  အတိအလင်း `--payload` မပါဘဲ သတ်မှတ်ထားသည်၊ ထုတ်လုပ်ထားသော blob ကို စာရေးသည်။
  `artifacts/da/proof_bench/payload.bin` ထို့ကြောင့် ပွဲများကို မထိမခိုက်မိပါစေနှင့်။【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- `artifacts/da/proof_bench/benchmark.{json,md}` သို့ မူရင်းအစီရင်ခံချက်များနှင့် အထောက်အထားများ/လည်ပတ်မှု၊ စုစုပေါင်းနှင့်
  သက်သေပြမည့်အချိန်၊ ဘတ်ဂျက်ဖြတ်နှုန်းနှင့် အကြံပြုထားသောဘတ်ဂျက် (အနှေးဆုံးထပ်ခြင်း၏ 110%) သို့
  `zk.halo2.verifier_budget_ms` ဖြင့် တန်းစီပါ။【artifacts/da/proof_bench/benchmark.md:1】
- နောက်ဆုံးထွက်ပြေးခြင်း (ဓာတု 1 MiB ပေးဆောင်မှု၊ 64 KiB အပိုင်းများ၊ 32 အထောက်အထားများ/ပြေးခြင်း၊ 10 ထပ်လုပ်ခြင်း၊ 250 ms ဘတ်ဂျက်)
  ဦးထုပ်အတွင်းတွင် 100% ထပ်တလဲလဲပြုလုပ်ထားသည့် 3 ms အတည်ပြုသည့်ဘတ်ဂျက်ကို အကြံပြုထားသည်။ 【artifacts/da/proof_bench/benchmark.md:1】
- ဥပမာ (အဆုံးအဖြတ်ပေးသော ဝန်ထုပ်ဝန်ပိုးကို ဖန်တီးပြီး အစီရင်ခံစာ နှစ်ခုလုံးကို ရေးသားသည်)။

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

Block assembly သည် တူညီသောဘတ်ဂျက်များကို တွန်းအားပေးသည်- `sumeragi.da_max_commitments_per_block` နှင့်
`sumeragi.da_max_proof_openings_per_block` သည် ဘလောက်တစ်ခုတွင် မမြှုပ်မီ DA အစုအဝေးကို ဂိတ်ပေါက်၊
ကတိကဝတ်တစ်ခုစီတွင် သုညမဟုတ်သော `proof_digest` ကို ဆောင်ထားရမည်။ အစောင့်သည် အစုအဝေးကို အရှည်အဖြစ် သတ်မှတ်သည်။
တိကျသေချာသော သက်သေအနှစ်ချုပ်များကို အများသဘောဆန္ဒဖြင့် ပေါင်းစပ်ပြီးသည်အထိ သက်သေဖွင့်ဆိုမှု အရေအတွက်ကို ထိန်းသိမ်းခြင်း၊
≤128-အဖွင့်ပစ်မှတ်သည် ဘလောက်နယ်နိမိတ်တွင် ပြဋ္ဌာန်းနိုင်သည်။ 【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR ကိုင်တွယ်ခြင်းနှင့် ဖြတ်တောက်ခြင်း မအောင်မြင်ခြင်း။ယခုအခါ သိုလှောင်မှုလုပ်သားများသည် PoR ချို့ယွင်းချက်များနှင့် အချိတ်အဆက်ရှိသော မျဉ်းစောင်း အကြံပြုချက်များကို တစ်ခုစီနှင့်တွဲလျက် ဖော်ပြသည်။
စီရင်ချက်။ ပြင်ဆင်သတ်မှတ်ထားသော ဒဏ်ခတ်မှုအဆင့်ထက် ဆက်တိုက်မအောင်မြင်ပါက အကြံပြုချက်တစ်ခု ထုတ်လွှတ်ပါသည်။
ပံ့ပိုးပေးသူ/မန်နီးဖက်စ်အတွဲ၊ မျဥ်းစောင်းကို အစပျိုးပေးသည့် မျဉ်းစောင်းအရှည်နှင့် အဆိုပြုထားသည့် အပိုင်းများ ပါဝင်သည်
ပေးဆောင်သူစာချုပ်မှတွက်ချက်ထားသောပြစ်ဒဏ်နှင့် `penalty_bond_bps`၊ cooldown windows (စက္ကန့်) ထားပါ။
တူညီသော ဖြစ်ရပ်တွင် ပစ်ခတ်ခြင်းမှ မျဉ်းစောင်းများ ထပ်နေခြင်း။ 【crates/sorafs_node/src/lib.rs:486】 【crates/sorafs_node/src/config.rs:89】 【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- သိုလှောင်မှုလုပ်သားတည်ဆောက်သူမှတစ်ဆင့် သတ်မှတ်ချက်များ/အအေးခံစနစ်ကို စီစဉ်သတ်မှတ်ပါ (မူရင်းများသည် အုပ်ချုပ်ရေးကို ထင်ဟပ်စေသည်
  ပြစ်ဒဏ်မူဝါဒ)။
- Slash အကြံပြုချက်များကို စီရင်ချက်အကျဉ်းချုပ် JSON တွင် မှတ်တမ်းတင်ထားသောကြောင့် အုပ်ချုပ်ရေး/စာရင်းစစ်များ ပူးတွဲဆောင်ရွက်နိုင်သည်
  အထောက်အထား အစုအဝေးများဆီသို့
- Stripe layout + per-chunk roles များကို ယခု Torii ၏ storage pin endpoint မှတဆင့် ချည်နှောင်ထားပါသည်။
  (`stripe_layout` + `chunk_roles` အကွက်များ) နှင့် သိုလှောင်မှုဝန်ထမ်းသို့ ဆက်လက်ရှိနေသည် ။
  စာရင်းစစ်/ပြုပြင်ရေးကိရိယာများသည် အထက်ပိုင်းမှ အဆင်အပြင်ကို ပြန်လည်ရယူခြင်းမရှိဘဲ အတန်း/ကော်လံ ပြုပြင်မှုများကို စီစဉ်နိုင်သည်

### နေရာချထားခြင်း + ကြိုးဝိုင်းပြုပြင်ခြင်း။

ယခု `cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>`
`(index, role, stripe/column, offsets)` ထက် နေရာချထားမှု hash ကိုတွက်ချက်ပြီး row-first လုပ်သည်
payload ကို ပြန်လည်မတည်ဆောက်မီ RS(16) ကော်လံ ပြုပြင်ခြင်း-

- နေရာချထားမှု ပုံသေသည် `total_stripes`/`shards_per_stripe` သို့ ပစ္စုပ္ပန်နှင့် အတုံးလိုက်ပြန်ကျသွားသောအခါ၊
- ပျောက်ဆုံးနေသော/ပျက်စီးနေသောအပိုင်းများကို အတန်းချိန်ညှိမှုဖြင့် ပထမဦးစွာ ပြန်လည်တည်ဆောက်ပါသည်။ ကျန်ကွက်လပ်များကို ပြုပြင်ပေးပါသည်။
  stripe (ကော်လံ) parity။ ပြုပြင်ထားသောအပိုင်းများကို အတုံးလိုက်လမ်းညွှန်နှင့် JSON သို့ ပြန်ရေးထားသည်။
  အနှစ်ချုပ်သည် နေရာချထားမှု hash နှင့် အတန်း/ကော်လံ ပြုပြင်ရေးကောင်တာများကို ဖမ်းယူသည်။
- row+column parity သည် ပျောက်ဆုံးနေသော set ကို ကျေနပ်အောင် မလုပ်နိုင်ပါက၊ ကြိုးသည် အမြန်ပြန်မရနိုင်တော့ပါ။
  စာရင်းစစ်များသည် ပြန်လည်ပြုပြင်၍မရသောဖော်ပြချက်များကို အလံပြနိုင်စေရန် အညွှန်းကိန်းများ။