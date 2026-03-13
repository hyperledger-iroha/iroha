---
lang: hy
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

վերնագիր՝ Տվյալների հասանելիության ներածման պլան
sidebar_label. Ներառում պլան
նկարագրություն. Torii բլբի ներթափանցման սխեմա, API մակերես և վավերացման պլան:
---

:::note Կանոնական աղբյուր
:::

# Sora Nexus Տվյալների առկայություն Ներմուծման պլան

_Նախագծված՝ 2026-02-20 - Սեփականատեր՝ Core Protocol WG / Storage Team / DA WG_

DA-2 աշխատանքային հոսքը ընդլայնում է Torii-ը blob ingest API-ով, որն արտանետում է Norito
մետատվյալներ և սերմեր SoraFS կրկնօրինակում: Այս փաստաթուղթը ներկայացնում է առաջարկվածը
սխեմա, API մակերես և վավերացման հոսք, այնպես որ իրականացումը կարող է շարունակվել առանց
արգելափակում ակնառու սիմուլյացիաների վրա (DA-1-ի հետևում): Բոլոր ծանրաբեռնված ձևաչափերը ՊԵՏՔ Է
օգտագործել Norito կոդեկներ; ոչ մի serde/JSON հետադարձ կապ չի թույլատրվում:

## Գոլեր

- Ընդունեք մեծ բլիթներ (Taikai հատվածներ, երթևեկելի կողային սայլեր, կառավարման արտեֆակտներ)
  վճռականորեն ավելի քան Torii:
- Ստեղծեք Norito կանոնական մանիֆեստներ, որոնք նկարագրում են բլբը, կոդեկի պարամետրերը,
  ջնջման պրոֆիլը և պահպանման քաղաքականությունը:
- Պահպանեք կտոր մետատվյալները SoraFS տաք պահեստում և հերթագրեք կրկնօրինակման աշխատանքներում:
- Հրապարակեք փին մտադրությունները + քաղաքականության պիտակները SoraFS գրանցամատյանում և կառավարման մեջ
  դիտորդներ.
- Բացահայտեք ընդունելության անդորրագրերը, որպեսզի հաճախորդները վերականգնեն հրապարակման որոշիչ ապացույցը:

## API մակերես (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Payload-ը Norito կոդավորված `DaIngestRequest` է: Պատասխանների օգտագործումը
`application/norito+v1` և վերադարձ `DaIngestReceipt`:

| Արձագանք | Իմաստը |
| --- | --- |
| 202 Ընդունված | Blob հերթագրված է chunking / replication; անդորրագիրը վերադարձվել է. |
| 400 Վատ խնդրանք | Սխեմայի/չափի խախտում (տես վավերացման ստուգումներ): |
| 401 Չլիազորված | Բացակայում է/անվավեր API նշան: |
| 409 Հակամարտություն | Կրկնօրինակեք `client_blob_id`՝ անհամապատասխան մետատվյալներով: |
| 413 Բեռը չափազանց մեծ է | Գերազանցում է կոնֆիգուրացված բլբի երկարության սահմանաչափը: |
| 429 Չափազանց շատ հարցումներ | Գնահատման սահմանաչափը հարվածել է: |
| 500 Ներքին սխալ | Անսպասելի ձախողում (գրանցված + ահազանգ): |

## Առաջարկվող Norito սխեման

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

> Իրականացման նշում. այս օգտակար բեռների կանոնական Rust-ի ներկայացումները այժմ գործում են
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest`-ում հարցումների/անդորրագրերի փաթեթավորմամբ
> և մանիֆեստի կառուցվածքը `iroha_data_model::da::manifest`-ում:

`compression` դաշտը գովազդում է, թե ինչպես են զանգահարողները պատրաստել օգտակար բեռը: Torii ընդունում է
`identity`, `gzip`, `deflate` և `zstd`՝ թափանցիկորեն ապասեղմելով բայթերը նախկինում
ընտրովի մանիֆեստների հեշում, մասնատում և ստուգում:

### Վավերացման ստուգաթերթ

1. Ստուգեք հարցումը Norito վերնագրի համընկնում `DaIngestRequest`.
2. Չհաջողվեց, եթե `total_size`-ը տարբերվում է կանոնական (դեսեղմված) օգտակար բեռնվածքի երկարությունից կամ գերազանցում է կազմաձևված առավելագույնը:
3. Կիրառել `chunk_size` հավասարեցում (հզորությունը երկուից, <= 2 ՄԲ):
4. Ապահովել `data_shards + parity_shards` <= գլոբալ առավելագույն և հավասարություն >= 2:
5. `retention_policy.required_replica_count`-ը պետք է հարգի կառավարման ելակետը:
6. Ստորագրության ստուգում կանոնական հեշի դեմ (բացառությամբ ստորագրության դաշտի):
7. Մերժեք կրկնօրինակ `client_blob_id`-ը, եթե ծանրաբեռնվածության հեշը + մետատվյալները նույնական չեն:
8. Երբ `norito_manifest` տրամադրվի, ստուգեք սխեման + հեշ համընկնումները վերահաշվարկված
   դրսևորվում է կտորից հետո; Հակառակ դեպքում հանգույցը ստեղծում է մանիֆեստ և պահպանում այն:
9. Կիրառեք կազմաձևված կրկնօրինակման քաղաքականությունը. Torii-ը վերագրում է ներկայացվածը
   `RetentionPolicy` `torii.da_ingest.replication_policy`-ի հետ (տես
   `replication-policy.md`) և մերժում է նախապես կառուցված մանիֆեստները, որոնց պահպանումը
   մետատվյալները չեն համապատասխանում պարտադրված պրոֆիլին:

### Հատման և կրկնօրինակման հոսք

1. Հատված ծանրաբեռնվածությունը `chunk_size`-ում, հաշվարկեք BLAKE3 մեկ կտորի համար + Մերկլի արմատը:
2. Կառուցեք Norito `DaManifestV1` (նոր կառուցվածք)՝ գրավելով հատվածի պարտավորությունները (role/group_id),
   ջնջման դասավորությունը (տողերի և սյունակների հավասարության քանակը գումարած `ipa_commitment`), պահպանման քաղաքականություն,
   և մետատվյալներ։
3. Կանոնական մանիֆեստի բայթերը հերթագրեք `config.da_ingest.manifest_store_dir`-ի տակ
   (Torii-ը գրում է `manifest.encoded` ֆայլեր՝ ստեղնավորված ըստ գծի/դարաշրջանի/հաջորդականության/տոմսի/մատնահետքի), այնպես որ SoraFS
   նվագախումբը կարող է կուլ տալ դրանք և կապել պահպանման տոմսը մշտական տվյալների հետ:
4. Հրապարակեք փին մտադրությունները `sorafs_car::PinIntent`-ի միջոցով կառավարման պիտակով + քաղաքականությամբ:
5. Թողարկեք Norito իրադարձություն `DaIngestPublished` դիտորդներին ծանուցելու համար (թեթև հաճախորդներ,
   կառավարում, վերլուծություն):
6. Վերադարձեք `DaIngestReceipt` զանգահարողին (ստորագրված է Torii DA սպասարկման բանալիով) և թողարկեք
   `Sora-PDP-Commitment` վերնագիր, որպեսզի SDK-ները կարողանան անմիջապես գրավել կոդավորված պարտավորությունը: Անդորրագիրը
   այժմ ներառում է `rent_quote` (a Norito `DaRentQuote`) և `stripe_layout`՝ թույլ տալով, որ ներկայացնեն ցուցադրվեն
   հիմնական վարձավճարը, պահուստային մասնաբաժինը, PDP/PoTR բոնուսային ակնկալիքները և 2D ջնջման դասավորությունը
   պահեստավորման տոմսը նախքան միջոցները ներգրավելը:

## Պահպանման / Ռեեստրի թարմացումներ

- Ընդլայնել `sorafs_manifest`-ը `DaManifestV1`-ով` հնարավորություն տալով դետերմինիստական վերլուծություն:
- Ավելացնել ռեեստրի նոր հոսք `da.pin_intent`՝ տարբերակված օգտակար բեռնվածքի հղումով
  մանիֆեստ հեշ + տոմսի ID:
- Թարմացրեք դիտարկելիության խողովակաշարերը՝ հետևելու ներթափանցման հետաձգմանը, մասնատված թողունակությանը,
  կրկնօրինակման հետաձգված և ձախողումների քանակը:

## Փորձարկման ռազմավարություն

- Միավորի թեստեր սխեմայի վավերացման, ստորագրության ստուգման, կրկնակի հայտնաբերման համար:
- Ոսկե թեստեր, որոնք հաստատում են `DaIngestRequest`-ի Norito կոդավորումը, մանիֆեստը և ստացականը:
- Ինտեգրման զրահը պտտվում է կեղծ SoraFS + գրանցամատյան՝ հաստատելով կտոր + փին հոսքեր:
- Սեփականության թեստեր, որոնք ընդգրկում են պատահական ջնջման պրոֆիլները և պահպանման համակցությունները:
- Norito բեռնատար բեռների խառնում` սխալ ձևավորված մետատվյալներից պաշտպանվելու համար:

## CLI և SDK գործիքավորում (DA-8)- `iroha app da submit` (նոր CLI մուտքի կետ) այժմ փաթաթում է ընդհանուր ներածման ստեղծող/հրատարակիչը, որպեսզի օպերատորները
  կարող է կամայական բլբեր ընդունել Taikai փաթեթի հոսքից դուրս: Հրամանատարությունը ապրում է
  `crates/iroha_cli/src/commands/da.rs:1` և սպառում է օգտակար բեռ, ջնջում/պահման պրոֆիլ և
  կամընտիր մետատվյալներ/մանիֆեստ ֆայլեր՝ նախքան կանոնական `DaIngestRequest`-ը CLI-ով ստորագրելը
  կազմաձևման բանալին: Հաջող վազքները պահպանվում են `da_request.{norito,json}` և `da_receipt.{norito,json}` տակ
  `artifacts/da/submission_<timestamp>/` (չեղյալ համարել `--artifact-dir`-ի միջոցով), այնպես որ արտեֆակտները կարող են ազատվել
  գրանցել ճշգրիտ Norito բայթ, որն օգտագործվում է կուլ տալու ժամանակ:
- Հրամանը լռելյայն է `client_blob_id = blake3(payload)`, բայց ընդունում է անտեսումները միջոցով
  `--client-blob-id`, հարգում է մետատվյալների JSON քարտեզները (`--metadata-json`) և նախապես ստեղծված մանիֆեստները
  (`--manifest`) և աջակցում է `--no-submit`՝ անցանց պատրաստման համար, գումարած `--endpoint` մաքսայինի համար
  Torii հոսթեր: JSON անդորրագիրը տպվում է stdout-ում, բացի սկավառակի վրա գրվելուց, փակելով այն
  DA-8 «submit_blob» գործիքավորման պահանջը և SDK-ի հավասարաչափ աշխատանքի ապաշրջափակումը:
- `iroha app da get`-ն ավելացնում է DA-ի վրա հիմնված այլանուն բազմաղբյուր նվագախմբի համար, որն արդեն իսկ ուժ ունի
  `iroha app sorafs fetch`. Օպերատորները կարող են այն ուղղել մանիֆեստի + բլոկ-պլանի արտեֆակտների վրա (`--manifest`,
  `--plan`, `--manifest-id`) **կամ** փոխանցեք Torii պահեստավորման տոմս `--storage-ticket`-ի միջոցով: Երբ տոմսը
  ուղին օգտագործվում է, CLI-ն հանում է մանիֆեստը `/v2/da/manifests/<ticket>`-ից, պահպանում է փաթեթը տակ
  `artifacts/da/fetch_<timestamp>/` (փոխանցել `--manifest-cache-dir`-ով), բխում է բլոբ հեշը
  `--manifest-id`, և այնուհետև ղեկավարում է նվագախմբին տրամադրված `--gateway-provider` ցուցակով: Բոլորը
  առաջադեմ բռնակներ SoraFS բերման մակերևույթից՝ անձեռնմխելի (մանիֆեստի ծրարներ, հաճախորդի պիտակներ, պահակային պահոցներ,
  անանունության փոխադրման վերացումները, ցուցատախտակի արտահանումը և `--output` ուղիները), իսկ մանիֆեստի վերջնակետը կարող է
  կարող է չեղարկվել `--manifest-endpoint`-ի միջոցով Torii մաքսային հոստերերի համար, այնպես որ հասանելիության վերջնական ստուգումները ուղիղ եթերում
  ամբողջությամբ `da` անվանատարածքի տակ՝ առանց նվագախմբի տրամաբանության կրկնօրինակման:
- `iroha app da get-blob`-ը քաշում է կանոնական մանիֆեստները ուղիղ Torii-ից `GET /v2/da/manifests/{storage_ticket}`-ի միջոցով:
  Հրամանը գրում է `manifest_{ticket}.norito`, `manifest_{ticket}.json` և `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/`-ի տակ (կամ օգտագործողի կողմից տրամադրված `--output-dir`)՝ միաժամանակ կրկնելով ճշգրիտ
  `iroha app da get` կոչում (ներառյալ `--manifest-id`) պահանջվում է հաջորդ նվագախմբի հետ բերելու համար:
  Սա օպերատորներին հեռու է պահում մանիֆեստի պարույրի գրացուցակներից և երաշխավորում է, որ բեռնիչը միշտ օգտագործում է այն
  ստորագրված արտեֆակտներ, որոնք թողարկվել են Torii-ի կողմից: JavaScript Torii հաճախորդը արտացոլում է այս հոսքը միջոցով
  `ToriiClient.getDaManifest(storageTicketHex)`, վերադարձնելով վերծանված Norito բայթերը, մանիֆեստ JSON,
  և բլոկ պլան, որպեսզի SDK զանգահարողները կարողանան խոնավացնել նվագախմբի նիստերը՝ առանց CLI-ին վճարելու:
  Swift SDK-ն այժմ բացահայտում է նույն մակերեսները (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), խողովակաշարերը միացվում են տեղական SoraFS նվագախմբի փաթաթմանը, որպեսզի
  iOS-ի հաճախորդները կարող են ներբեռնել մանիֆեստներ, կատարել բազմաղբյուրի ներբեռնումներ և առանց ապացույցներ գրավել
  կանչելով CLI:【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote`-ը հաշվարկում է դետերմինիստական վարձավճարը և խրախուսական խզումները մատակարարված պահեստի չափի համար
  և պահպանման պատուհան: Օգնականը սպառում է կամ ակտիվ `DaRentPolicyV1` (JSON կամ Norito բայթ) կամ
  ներկառուցված լռելյայն, վավերացնում է քաղաքականությունը և տպում JSON ամփոփագիր (`gib`, `months`, քաղաքականության մետատվյալներ,
  և `DaRentQuote` դաշտերը), որպեսզի աուդիտորները կարողանան նշել XOR ճշգրիտ վճարները կառավարման րոպեների ընթացքում՝ առանց
  ժամանակավոր սցենարներ գրելը. Հրամանը նաև թողարկում է մեկ տողով `rent_quote ...` ամփոփում JSON-ից առաջ
  ծանրաբեռնվածություն՝ կոնսոլի տեղեկամատյանները ընթեռնելի պահելու միջադեպերի զորավարժությունների ժամանակ: Զույգ `--quote-out artifacts/da/rent_quotes/<stamp>.json` հետ
  `--policy-label "governance ticket #..."` պահպանելու գեղարվեստական արտեֆակտները, որոնք մեջբերում են ճշգրիտ քաղաքականության քվեարկությունը
  կամ կազմաձևման փաթեթ; CLI-ն կտրում է հատուկ պիտակը և մերժում դատարկ տողերը, որպեսզի `policy_source` արժեքները
  մնալ գործող գանձապետական վահանակների վրա: Տե՛ս `crates/iroha_cli/src/commands/da.rs` ենթահրամանի համար
  և `docs/source/da/rent_policy.md` քաղաքականության սխեմայի համար:【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability`-ը կապում է վերը նշված բոլորը. վերցնում է պահեստավորման տոմս, ներբեռնում է
  կանոնական մանիֆեստի փաթեթ, գործարկում է բազմաղբյուր նվագախմբի (`iroha app sorafs fetch`) դեմ
  մատակարարված `--gateway-provider` ցուցակը, պահպանում է ներբեռնված ծանրաբեռնվածությունը + ցուցատախտակը տակ
  `artifacts/da/prove_availability_<timestamp>/` և անմիջապես կանչում է առկա PoR օգնականին
  (`iroha app da prove`)՝ օգտագործելով առբերված բայթերը: Օպերատորները կարող են կսմթել նվագախմբի բռնակները
  (`--max-peers`, `--scoreboard-out`, մանիֆեստի վերջնակետի վերացում) և ապացուցման նմուշառիչը
  (`--sample-count`, `--leaf-index`, `--sample-seed`), մինչդեռ մեկ հրամանը արտադրում է արտեֆակտները
  սպասվում է DA-5/DA-9 աուդիտների կողմից. օգտակար բեռնվածության պատճեն, ցուցատախտակի ապացույցներ և JSON ապացույցների ամփոփագրեր:

## TODO բանաձեւի ամփոփում

Նախկինում արգելափակված մուտքի բոլոր TODO-ները ներդրվել և հաստատվել են.

- **Կոմպրեսիոն ակնարկներ** — Torii ընդունում է զանգահարողի կողմից տրամադրված պիտակները (`identity`, `gzip`, `deflate`,
  `zstd`) և նորմալացնում է օգտակար բեռները մինչև վավերացումը, որպեսզի կանոնական մանիֆեստի հեշը համապատասխանի
  ապասեղմված բայթեր։【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Միայն կառավարման մետատվյալների գաղտնագրում** — Torii-ն այժմ գաղտնագրում է կառավարման մետատվյալները
  կազմաձևված ChaCha20-Poly1305 ստեղնը, մերժում է անհամապատասխան պիտակները և բացահայտում է երկու բացահայտ
  կոնֆիգուրացիայի բռնակներ (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) ռոտացիան դետերմինիստական պահելու համար:【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Խոշոր ծանրաբեռնված հոսք** — բազմամասի ընդունումը ուղիղ եթերում է: Հաճախորդների հոսքը դետերմինիստական է
  `DaIngestChunk` ծրարները, որոնք ստեղնված են `client_blob_id`-ով, Torii-ը վավերացնում է յուրաքանչյուր հատվածը, փուլավորում է դրանք
  `manifest_store_dir`-ի տակ և ատոմային կերպով վերակառուցում է մանիֆեստը, երբ `is_last` դրոշը վայրէջք կատարի,
  վերացնելով RAM-ի բարձրացումները, որոնք դիտվում են մեկ զանգով վերբեռնումների ժամանակ:【crates/iroha_torii/src/da/ingest.rs:392】
- **Ակնհայտ տարբերակում** — `DaManifestV1`-ը կրում է բացահայտ `version` դաշտ, իսկ Torii-ը մերժում է
  անհայտ տարբերակներ, որոնք երաշխավորում են դետերմինիստական արդիականացումներ, երբ նոր մանիֆեստների դասավորությունները ուղարկվում են:【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR կեռիկներ** — PDP պարտավորությունները բխում են անմիջապես կտորների պահեստից և պահպանվում են
  մանիֆեստների կողքին, որպեսզի DA-5 ժամանակացույցերը կարողանան օրինական տվյալներից նմուշառման մարտահրավերներ գործարկել, և
  `/v2/da/ingest` գումարած `/v2/da/manifests/{ticket}` այժմ ներառում է `Sora-PDP-Commitment` վերնագիր
  կրում է base64 Norito ծանրաբեռնվածությունը, որպեսզի SDK-ները պահեն ճշգրիտ պարտավորվածությունը DA-5 զոնդերը թիրախ.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## Իրականացման նշումներ

- Torii-ի `/v2/da/ingest` վերջնակետն այժմ նորմալացնում է ծանրաբեռնվածության սեղմումը, ամրացնում է վերարտադրման քեշը,
  վճռականորեն բաժանում է կանոնական բայթերը, վերակառուցում `DaManifestV1`, գցում կոդավորված օգտակար բեռը
  մեջ `config.da_ingest.manifest_store_dir` SoraFS նվագախմբի համար և ավելացնում է `Sora-PDP-Commitment`
  վերնագիր, որպեսզի օպերատորները վերցնեն այն պարտավորությունը, որին կհղվեն PDP ժամանակացույցները:【crates/iroha_torii/src/da/ingest.rs:220】
- Յուրաքանչյուր ընդունված բշտիկ այժմ արտադրում է `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito`
  մուտքը `manifest_store_dir`-ի ներքո՝ միավորելով կանոնական `DaCommitmentRecord`-ը հումքի հետ միասին
  `PdpCommitmentV1` բայթ, այնպես որ DA-3 փաթեթի ստեղծողները և DA-5 ժամանակացույցները խոնավացնում են նույնական մուտքերը՝ առանց
  մանիֆեստների կամ կտորների վերընթերցում:【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK օգնական API-ները բացահայտում են PDP վերնագրի ծանրաբեռնվածությունը՝ չստիպելով զանգահարողներին վերագործարկել Norito ապակոդավորումը.
  The Rust crate-ն արտահանում է `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python-ը
  `ToriiClient`-ն այժմ ներառում է `decode_pdp_commitment_header` և `IrohaSwift` նավերը
  `decodePdpCommitmentHeader` ծանրաբեռնված վերնագրերի չմշակված քարտեզների կամ `HTTPURLResponse` օրինակներ։【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii-ը նաև բացահայտում է `GET /v2/da/manifests/{storage_ticket}`-ը, որպեսզի SDK-ները և օպերատորները կարողանան առբերել մանիֆեստները
  և բլոկների պլանները՝ առանց հանգույցի կծիկի գրացուցակին դիպչելու: Պատասխանը վերադարձնում է Norito բայթ
  (base64), ներկայացվել է JSON մանիֆեստը, `chunk_plan` JSON բլբակ, որը պատրաստ է `sorafs fetch`-ի համար, համապատասխանը
  վեցանկյուն մարսվում է (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) և արտացոլում է
  `Sora-PDP-Commitment` վերնագիր՝ ներծծման պատասխաններից՝ հավասարության համար: `block_hash=<hex>` մատակարարում է
  հարցման տողը վերադարձնում է որոշիչ `sampling_plan` (հանձնարարության հեշ, `sample_window` և նմուշառված
  `(index, role, group)` բազմեր, որոնք ընդգրկում են ամբողջական 2D դասավորությունը), այնպես որ վավերացնողներն ու PoR գործիքները նույնն են նկարում
  ցուցանիշները։

### Մեծ բեռի հոսքային հոսք

Հաճախորդները, որոնք պետք է ներծծեն ավելի մեծ ակտիվներ, քան կազմաձևված մեկ հարցման սահմանաչափը, նախաձեռնում են a
հոսքային նիստ՝ զանգահարելով `POST /v2/da/ingest/chunk/start`: Torii-ը պատասխանում է a
`ChunkSessionId` (BLAKE3-ը ստացվել է պահանջվող բլբի մետատվյալներից) և սակարկվող կտորի չափը:
Յուրաքանչյուր հաջորդ `DaIngestChunk` հարցումը կրում է.- `client_blob_id` — նույնական է վերջնական `DaIngestRequest`-ին:
- `chunk_session_id` — հատվածները կապում է ընթացիկ նստաշրջանին:
- `chunk_index` և `offset` — կիրառում են դետերմինիստական ​​կարգադրություն:
- `payload` — մինչև սակարկվող կտորի չափը:
- `payload_hash` — BLAKE3 հատվածի հեշը, որպեսզի Torii-ը կարողանա վավերացնել առանց ամբողջ բլբի բուֆերացման:
- `is_last` — ցույց է տալիս տերմինալի հատվածը:

Torii-ը պահպանում է վավերացված հատվածները `config.da_ingest.manifest_store_dir/chunks/<session>/`-ի ներքո և
արձանագրում է առաջընթացը վերարտադրման քեշի ներսում՝ հարգելու անզորությունը: Երբ վերջնական հատվածը վայրէջք է կատարում, Torii
վերահավաքում է բեռնվածությունը սկավառակի վրա (հոսում է կտորների գրացուցակում՝ հիշողության աճից խուսափելու համար),
հաշվարկում է կանոնական մանիֆեստը/ստացումը ճիշտ այնպես, ինչպես մեկ կրակոց վերբեռնումների դեպքում, և վերջապես արձագանքում է
`POST /v2/da/ingest`՝ սպառելով բեմականացված արտեֆակտը: Չհաջողված նիստերը կարող են ուղղակիորեն ընդհատվել կամ
աղբ են հավաքվում `config.da_ingest.replay_cache_ttl`-ից հետո։ Այս դիզայնը պահպանում է ցանցի ձևաչափը
Norito-ը հարմար է, խուսափում է հաճախորդի համար հատուկ վերսկսվող արձանագրություններից և վերօգտագործում առկա մանիֆեստի խողովակաշարը
անփոփոխ.

**Իրականացման կարգավիճակը։** Կանոնական Norito տեսակներն այժմ ապրում են
`crates/iroha_data_model/src/da/`:

- `ingest.rs`-ը սահմանում է `DaIngestRequest`/`DaIngestReceipt`-ի հետ միասին
  `ExtraMetadata` կոնտեյներ, որն օգտագործվում է Torii-ի կողմից:【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` հոսթեր `DaManifestV1` և `ChunkCommitment`, որոնք Torii-ը թողարկում է հետո
  chunking-ն ավարտվում է:【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs`-ը տրամադրում է ընդհանուր անուններ (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` և այլն) և կոդավորում է ստորև փաստաթղթավորված կանխադրված քաղաքականության արժեքները:【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool ֆայլերը վայրէջք են կատարում `config.da_ingest.manifest_store_dir`-ում՝ պատրաստ SoraFS նվագախմբի համար
  դիտորդը պետք է մտնի պահեստային մուտք։【crates/iroha_torii/src/da/ingest.rs:220】

Հետագծվում է հարցումների, մանիֆեստների և անդորրագրերի օգտակար բեռների ծածկույթը
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, ապահովելով Norito կոդեկ
մնում է կայուն թարմացումների ընթացքում:【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Պահպանման դեֆոլտներ** Կառավարությունը վավերացրել է պահպանման սկզբնական քաղաքականությունը ընթացքում
SF-6; `RetentionPolicy::default()`-ի կողմից կիրառվող կանխադրվածներն են.

- տաք մակարդակ՝ 7 օր (`604_800` վայրկյան)
- սառը մակարդակ՝ 90 օր (`7_776_000` վայրկյան)
- պահանջվող կրկնօրինակները՝ `3`
- պահեստավորման դաս՝ `StorageClass::Hot`
- կառավարման պիտակ՝ `"da.default"`

Հոսանքից ներքև գտնվող օպերատորները պետք է բացահայտորեն անտեսեն այս արժեքները, երբ երթուղին ընդունվի
ավելի խիստ պահանջներ.