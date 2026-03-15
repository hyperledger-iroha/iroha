---
lang: ka
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

title: მონაცემთა ხელმისაწვდომობის შეყვანის გეგმა
sidebar_label: ჩასმის გეგმა
აღწერა: სქემა, API ზედაპირი და ვალიდაციის გეგმა Torii blob-ისთვის.
---

:::შენიშვნა კანონიკური წყარო
:::

# Sora Nexus მონაცემთა ხელმისაწვდომობის ჩარიცხვის გეგმა

_შემუშავებული: 2026-02-20 - მფლობელი: Core Protocol WG / Storage Team / DA WG_

DA-2 სამუშაო ნაკადი აფართოებს Torii blob ingest API-ით, რომელიც ასხივებს Norito
მეტამონაცემები და თესლი SoraFS რეპლიკაცია. ეს დოკუმენტი ასახავს შემოთავაზებულს
სქემა, API ზედაპირი და ვალიდაციის ნაკადი, ასე რომ განხორციელება შეიძლება გაგრძელდეს გარეშე
გამორჩეული სიმულაციების ბლოკირება (DA-1 შემდგომი დაკვირვებები). ყველა დატვირთვის ფორმატი უნდა იყოს
გამოიყენეთ Norito კოდეკები; ნებადართული არ არის serde/JSON ჩანაცვლება.

## გოლები

- მიიღე დიდი ბურთები (ტაიკაის სეგმენტები, ზოლის გვერდითი კარები, მმართველობის არტეფაქტები)
  განმსაზღვრელი Torii-ზე.
- შექმენით კანონიკური Norito მანიფესტები, რომლებიც აღწერს blob-ს, კოდეკის პარამეტრებს,
  წაშლის პროფილი და შენახვის პოლიტიკა.
- შენარჩუნებული ნაწილის მეტამონაცემები SoraFS ცხელ საცავში და რეპლიკაციის სამუშაოების რიგში.
- გამოაქვეყნეთ პინის მიზნები + პოლიტიკის ტეგები SoraFS რეესტრში და მმართველობაში
  დამკვირვებლები.
- გამოავლინეთ დაშვების ქვითრები, რათა კლიენტებმა დაიბრუნონ გამოქვეყნების დეტერმინისტული მტკიცებულება.

## API ზედაპირი (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Payload არის Norito-ში კოდირებული `DaIngestRequest`. პასუხების გამოყენება
`application/norito+v1` და დააბრუნეთ `DaIngestReceipt`.

| პასუხი | მნიშვნელობა |
| --- | --- |
| 202 მიღებული | Blob რიგში დაქუცმაცების/განმეორებისთვის; ქვითარი დაბრუნდა. |
| 400 ცუდი მოთხოვნა | სქემის/ზომის დარღვევა (იხილეთ ვალიდაციის შემოწმებები). |
| 401 არასანქცირებული | აკლია/არასწორი API ჟეტონი. |
| 409 კონფლიქტი | დუბლიკატი `client_blob_id` შეუსაბამო მეტამონაცემებით. |
| 413 დატვირთვა ძალიან დიდი | აჭარბებს კონფიგურირებულ ბლოკის სიგრძის ლიმიტს. |
| 429 ძალიან ბევრი მოთხოვნა | შეფასების ლიმიტი მიღწეულია. |
| 500 შიდა შეცდომა | მოულოდნელი მარცხი (რეგისტრირებული + გაფრთხილება). |

## შემოთავაზებული Norito სქემა

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

> განხორციელების შენიშვნა: კანონიკური Rust-ის წარმოდგენები ამ ტვირთამწეობისთვის ახლა მოთავსებულია ქვეშ
> `iroha_data_model::da::types`, მოთხოვნის/ქვითრის შეფუთვით `iroha_data_model::da::ingest`-ში
> და მანიფესტის სტრუქტურა `iroha_data_model::da::manifest`-ში.

`compression` ველი აქვეყნებს რეკლამას, თუ როგორ მოამზადეს აბონენტებმა ტვირთი. Torii იღებს
`identity`, `gzip`, `deflate` და `zstd`, ბაიტების გამჭვირვალედ დეკომპრესირებამდე
არჩევითი მანიფესტების ჰეშირება, დაქუცმაცება და გადამოწმება.

### დადასტურების ჩამონათვალი

1. დაადასტურეთ მოთხოვნა Norito სათაური ემთხვევა `DaIngestRequest`.
2. წარუმატებლობა, თუ `total_size` განსხვავდება კანონიკური (დეკომპრესიული) დატვირთვის სიგრძისგან ან აღემატება კონფიგურირებულ მაქსიმუმს.
3. განახორციელეთ `chunk_size` გასწორება (ძალა ორიდან, <= 2 MiB).
4. დარწმუნდით, რომ `data_shards + parity_shards` <= გლობალური მაქსიმუმი და პარიტეტი >= 2.
5. `retention_policy.required_replica_count` უნდა იცავდეს მმართველობის საბაზისო ხაზს.
6. ხელმოწერის შემოწმება კანონიკური ჰეშის წინააღმდეგ (ხელმოწერის ველის გამოკლებით).
7. უარი თქვით `client_blob_id` დუბლიკატზე, გარდა იმ შემთხვევისა, როდესაც payload ჰეში + მეტამონაცემები იდენტურია.
8. როდესაც მოწოდებულია `norito_manifest`, გადაამოწმეთ სქემა + ჰეშის დამთხვევები ხელახლა გამოთვლილი
   გამოიხატება დაქუცმაცების შემდეგ; წინააღმდეგ შემთხვევაში, კვანძი ქმნის მანიფესტს და ინახავს მას.
9. განახორციელეთ კონფიგურირებული რეპლიკაციის პოლიტიკა: Torii ხელახლა წერს გაგზავნილს
   `RetentionPolicy` `torii.da_ingest.replication_policy`-თან ერთად (იხ.
   `replication-policy.md`) და უარყოფს წინასწარ ჩაშენებულ მანიფესტებს, რომელთა შეკავება
   მეტამონაცემები არ ემთხვევა იძულებით პროფილს.

### დაქუცმაცება და რეპლიკაციის ნაკადი

1. ჩაყარეთ დატვირთვა `chunk_size`-ში, გამოთვალეთ BLAKE3 თითო ნაწილზე + Merkle root.
2. Build Norito `DaManifestV1` (ახალი სტრუქტურა) ბლოკის ვალდებულებების აღრიცხვა (role/group_id),
   წაშლის განლაგება (სტრიქონებისა და სვეტების პარიტეტის რაოდენობა პლუს `ipa_commitment`), შენახვის პოლიტიკა,
   და მეტამონაცემები.
3. რიგით მანიფესტის კანონიკური ბაიტები `config.da_ingest.manifest_store_dir`-ში
   (Torii წერს `manifest.encoded` ფაილებს, რომლებიც ჩასმულია ზოლის/ეპოქის/მიმდევრობის/ბილეთის/თითის ანაბეჭდის მიხედვით) ასე რომ, SoraFS
   ორკესტრაციას შეუძლია მათი გადაყლაპვა და შენახვის ბილეთის დაკავშირება მუდმივ მონაცემებთან.
4. გამოაქვეყნეთ პინის მიზნები `sorafs_car::PinIntent`-ის მეშვეობით მმართველობის ტეგით + პოლიტიკით.
5. გამოუშვით Norito მოვლენა `DaIngestPublished` დამკვირვებლების შესატყობინებლად (მსუბუქი კლიენტები,
   მმართველობა, ანალიტიკა).
6. დაუბრუნეთ `DaIngestReceipt` აბონენტს (ხელმოწერილია Torii DA სერვისის გასაღებით) და გამოუშვით
   `Sora-PDP-Commitment` სათაური, რათა SDK-ებმა შეძლონ დაშიფრული ვალდებულების დაფიქსირება დაუყოვნებლივ. ქვითარი
   ახლა მოიცავს `rent_quote` (a Norito `DaRentQuote`) და `stripe_layout`, რაც გამომგზავნის ჩვენების საშუალებას აძლევს
   საბაზისო ქირა, სარეზერვო წილი, PDP/PoTR ბონუსის მოლოდინი და 2D წაშლის განლაგება
   შენახვის ბილეთი თანხის ჩადებამდე.

## შენახვის / რეესტრის განახლებები

- გააფართოვეთ `sorafs_manifest` `DaManifestV1`-ით, რაც საშუალებას იძლევა განმსაზღვრელი ანალიზი.
- დაამატეთ ახალი რეესტრის ნაკადი `da.pin_intent`, ვერსიის მომგებიანი დატვირთვის მითითებით
  მანიფესტის ჰეში + ბილეთის ID.
- განაახლეთ დაკვირვებადობის მილსადენები, რათა თვალყური ადევნოთ შეყოვნებას, დაშლის გამტარუნარიანობას,
  რეპლიკაციის ჩამორჩენა და წარუმატებლობის რაოდენობა.

## ტესტირების სტრატეგია

- ერთეულის ტესტები სქემის ვალიდაციისთვის, ხელმოწერის შემოწმებისთვის, დუბლიკატების გამოვლენისთვის.
- ოქროს ტესტები, რომლებიც ადასტურებენ `DaIngestRequest`-ის Norito კოდირებას, მანიფესტს და ქვითარს.
- ინტეგრაციის აღკაზმულობა ტრიალებს იმიტირებულ SoraFS + რეესტრს, ამტკიცებს ნაწილს + პინის ნაკადებს.
- საკუთრების ტესტები, რომლებიც მოიცავს შემთხვევითი წაშლის პროფილებს და შეკავების კომბინაციებს.
- Norito დატვირთვის დაბინძურება არასწორი მეტამონაცემებისგან თავის დასაცავად.

## CLI & SDK Tooling (DA-8)- `iroha app da submit` (ახალი CLI შესვლის წერტილი) ახლა ახვევს გაზიარებულ ჩაწერის შემქმნელს/გამომცემელს, რათა ოპერატორები
  შეუძლია თვითნებური ბლომების გადაყლაპვა ტაიკაის შეკვრის ნაკადის გარეთ. ბრძანება ცხოვრობს
  `crates/iroha_cli/src/commands/da.rs:1` და მოიხმარს დატვირთვას, წაშლის/შეკავების პროფილს და
  არჩევითი მეტამონაცემების/მანიფესტის ფაილები კანონიკურ `DaIngestRequest`-ზე ხელმოწერამდე CLI-ით
  კონფიგურაციის გასაღები. წარმატებული გაშვებები გრძელდება `da_request.{norito,json}` და `da_receipt.{norito,json}` ქვეშ
  `artifacts/da/submission_<timestamp>/` (გადალახვა `--artifact-dir`-ის მეშვეობით), ასე რომ, არტეფაქტების გამოშვება შესაძლებელია
  ჩაწერეთ ზუსტი Norito ბაიტი, რომელიც გამოიყენება მიღების დროს.
- ბრძანება ნაგულისხმევად არის `client_blob_id = blake3(payload)`, მაგრამ იღებს უგულებელყოფას
  `--client-blob-id`, აფასებს მეტამონაცემებს JSON რუკებს (`--metadata-json`) და წინასწარ გენერირებულ მანიფესტებს
  (`--manifest`) და მხარს უჭერს `--no-submit` ხაზგარეშე მომზადებისთვის პლუს `--endpoint` მორგებისთვის
  Torii ჰოსტები. ქვითარი JSON იბეჭდება stdout-ზე, გარდა იმისა, რომ იწერება დისკზე, ხურავს მას
  DA-8 "submit_blob" ხელსაწყოების მოთხოვნა და SDK პარიტეტული მუშაობის განბლოკვა.
- `iroha app da get` ამატებს DA-ზე ორიენტირებულ მეტსახელს მრავალ წყაროს ორკესტრისთვის, რომელიც უკვე მუშაობს
  `iroha app sorafs fetch`. ოპერატორებს შეუძლიათ მიუთითონ მანიფესტზე + ბლოკ-გეგმის არტეფაქტებზე (`--manifest`,
  `--plan`, `--manifest-id`) **ან** გაიარეთ Torii შენახვის ბილეთი `--storage-ticket`-ით. როცა ბილეთი
  ბილიკი გამოიყენება, CLI ამოიღებს მანიფესტს `/v1/da/manifests/<ticket>`-დან, აგრძელებს პაკეტს ქვეშ
  `artifacts/da/fetch_<timestamp>/` (გადააჭარბებს `--manifest-cache-dir`-ს), იღებს blob ჰეშის
  `--manifest-id` და შემდეგ მართავს ორკესტრატორს მიწოდებული `--gateway-provider` სიით. ყველა
  მოწინავე სახელურები SoraFS ჩამოტანის ზედაპირიდან ხელუხლებელი (მანიფესტის კონვერტები, კლიენტის ეტიკეტები, დამცავი ქეში,
  ანონიმურობის ტრანსპორტის უგულებელყოფა, შედეგების დაფის ექსპორტი და `--output` ბილიკები) და მანიფესტის საბოლოო წერტილი შეიძლება
  გაუქმდეს `--manifest-endpoint`-ის მეშვეობით მორგებული Torii ჰოსტებისთვის, ასე რომ, ხელმისაწვდომობის შემოწმება პირდაპირ ეთერში
  მთლიანად `da` სახელთა სივრცის ქვეშ, ორკესტრის ლოგიკის დუბლირების გარეშე.
- `iroha app da get-blob` გამოაქვს კანონიკური მანიფესტები პირდაპირ Torii-დან `GET /v1/da/manifests/{storage_ticket}`-ის მეშვეობით.
  ბრძანება წერს `manifest_{ticket}.norito`, `manifest_{ticket}.json` და `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/`-ის ქვეშ (ან მომხმარებლის მიერ მოწოდებული `--output-dir`) ზუსტი ექოს დროს
  `iroha app da get` გამოძახება (მათ შორის `--manifest-id`) საჭიროა შემდგომი ორკესტრატორის მისაღებად.
  ეს აშორებს ოპერატორებს მანიფესტის კოჭის დირექტორიებიდან და გარანტიას იძლევა, რომ მიმღები ყოველთვის იყენებს მას
  Torii-ის მიერ გამოშვებული ხელმოწერილი არტეფაქტები. JavaScript Torii კლიენტი ასახავს ამ ნაკადს
  `ToriiClient.getDaManifest(storageTicketHex)`, აბრუნებს დეკოდირებულ Norito ბაიტს, მანიფესტის JSON,
  და ბლოკის გეგმა ისე, რომ SDK აბონენტებს შეეძლოთ ორკესტრის სესიების დატენიანება CLI-ზე გადარიცხვის გარეშე.
  Swift SDK ახლა აჩვენებს იმავე ზედაპირებს (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), მილსადენები ერწყმის მშობლიურ SoraFS ორკესტრატორის შეფუთვას.
  iOS-ის კლიენტებს შეუძლიათ ჩამოტვირთოთ მანიფესტები, შეასრულონ მრავალ წყაროს ამოღება და მტკიცებულებების გარეშე
  CLI-ის გამოძახება.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` ითვლის დეტერმინისტულ ქირას და სტიმულირების ავარიებს მიწოდებული შენახვის ზომისთვის
  და შეკავების ფანჯარა. დამხმარე მოიხმარს ან აქტიურ `DaRentPolicyV1` (JSON ან Norito ბაიტს) ან
  ჩაშენებული ნაგულისხმევი, ამოწმებს პოლიტიკას და ბეჭდავს JSON რეზიუმეს (`gib`, `months`, პოლიტიკის მეტამონაცემებს,
  და `DaRentQuote` ველები), ასე რომ აუდიტორებს შეუძლიათ მოიყვანონ ზუსტი XOR გადასახადები მმართველობის წუთებში, გარეშე
  ad hoc სკრიპტების წერა. ბრძანება ასევე გამოსცემს ერთხაზოვან `rent_quote ...` შეჯამებას JSON-ის წინ
  დატვირთვა, რათა შეინარჩუნოს კონსოლის ჟურნალები იკითხება ინციდენტის წვრთნების დროს. დააწყვილეთ `--quote-out artifacts/da/rent_quotes/<stamp>.json` ერთად
  `--policy-label "governance ticket #..."` შენარჩუნებული გაფორმებული არტეფაქტები, რომლებიც მოჰყავს ზუსტი პოლიტიკის კენჭისყრას
  ან კონფიგურაციის პაკეტი; CLI ჭრის მორგებულ ლეიბლს და უარს ამბობს ცარიელ სტრიქონებზე, ასე რომ `policy_source` ფასდება
  დარჩება ქმედითუნარიანი სახაზინო დაფებზე. იხილეთ `crates/iroha_cli/src/commands/da.rs` ქვებრძანებისთვის
  და `docs/source/da/rent_policy.md` პოლიტიკის სქემისთვის.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` აკავშირებს ყველა ჩამოთვლილს: იღებს შენახვის ბილეთს, ჩამოტვირთავს
  კანონიკური მანიფესტის ნაკრები, აწარმოებს მრავალ წყაროს ორკესტრატორს (`iroha app sorafs fetch`) წინააღმდეგ
  მოწოდებული `--gateway-provider` სია, შენარჩუნებულია ჩამოტვირთული დატვირთვა + ანგარიშის დაფა ქვემოთ
  `artifacts/da/prove_availability_<timestamp>/` და დაუყოვნებლივ გამოიძახებს არსებულ PoR დამხმარეს
  (`iroha app da prove`) მოტანილი ბაიტების გამოყენებით. ოპერატორებს შეუძლიათ ორკესტრის სახელურების შესწორება
  (`--max-peers`, `--scoreboard-out`, მანიფესტის საბოლოო წერტილის უგულებელყოფა) და მტკიცებულების ნიმუში
  (`--sample-count`, `--leaf-index`, `--sample-seed`) ხოლო ერთი ბრძანება აწარმოებს არტეფაქტებს
  მოსალოდნელია DA-5/DA-9 აუდიტის მიერ: დატვირთვის ასლი, ანგარიშის დაფის მტკიცებულება და JSON მტკიცებულების შეჯამებები.

## TODO რეზოლუციის შეჯამება

ყველა ადრე დაბლოკილი შეყვანის TODO დანერგილი და დამოწმებულია:

- ** შეკუმშვის მინიშნებები ** — Torii იღებს აბონენტის მიერ მოწოდებულ ეტიკეტებს (`identity`, `gzip`, `deflate`,
  `zstd`) და ახდენს დატვირთვის ნორმალიზებას ვალიდაციამდე, რათა კანონიკური მანიფესტის ჰეში ემთხვეოდეს
  დეკომპრესირებული ბაიტები.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **მხოლოდ მმართველობის მეტამონაცემების დაშიფვრა** — Torii ახლა შიფრავს მმართველობის მეტამონაცემებს
  კონფიგურირებული კლავიში ChaCha20-Poly1305, უარყოფს შეუსაბამო ლეიბლებს და ასახავს ორ აშკარად
  კონფიგურაციის სახელურები (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) ბრუნვის დეტერმინისტულად შესანარჩუნებლად.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **დიდი დატვირთვის ნაკადი** — მრავალ ნაწილის მიღება პირდაპირ ეთერშია. კლიენტების ნაკადი განმსაზღვრელია
  `DaIngestChunk` კონვერტები ჩასმულია `client_blob_id`-ით, Torii ამოწმებს თითოეულ ნაჭერს, ეტაპებს
  `manifest_store_dir` ქვეშ და ატომურად აღადგენს მანიფესტს, როგორც კი `is_last` დროშა დაეშვება,
  RAM-ის მწვერვალების აღმოფხვრა ერთი ზარის ატვირთვით.【crates/iroha_torii/src/da/ingest.rs:392】
- **მანიფესტური ვერსიირება** — `DaManifestV1` შეიცავს აშკარა `version` ველს და Torii უარს ამბობს
  უცნობი ვერსიები, გარანტირებულია დეტერმინისტული განახლებები, როდესაც ახალი მანიფესტის განლაგება გაიგზავნება.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR კაკვები** — PDP-ის ვალდებულებები უშუალოდ მომდინარეობს ნაჭრების შენახვისგან და შენარჩუნებულია
  გარდა ამისა, მანიფესტებს, რათა DA-5 განრიგებმა შეძლონ შერჩევის გამოწვევების გაშვება კანონიკური მონაცემებიდან და
  `/v1/da/ingest` პლუს `/v1/da/manifests/{ticket}` ახლა მოიცავს `Sora-PDP-Commitment` სათაურს
  ატარებს base64 Norito დატვირთვის დატვირთვას, რათა SDK-ები ქეშინდეს ზუსტი ვალდებულების DA-5 ზონდებს target.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## განხორციელების შენიშვნები

- Torii-ის `/v1/da/ingest` ბოლო წერტილი ახლა ახდენს დატვირთვის შეკუმშვის ნორმალიზებას, აიძულებს განმეორებითი ქეშის,
  განმსაზღვრელად ანაწილებს კანონიკურ ბაიტებს, აღადგენს `DaManifestV1`-ს, ჩამოაგდებს დაშიფრულ დატვირთვას
  შევიდა `config.da_ingest.manifest_store_dir` SoraFS ორკესტრირებისთვის და ამატებს `Sora-PDP-Commitment`
  სათაური, რათა ოპერატორებმა აითვისონ ვალდებულება, რომელსაც PDP განრიგები მიუთითებენ.【crates/iroha_torii/src/da/ingest.rs:220】
- ყოველი მიღებული ბლოკი ახლა აწარმოებს `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito`-ს
  შესვლის ქვეშ `manifest_store_dir` შეფუთვა კანონიკური `DaCommitmentRecord` ნედლეულთან ერთად
  `PdpCommitmentV1` ბაიტი, ასე რომ, DA-3 პაკეტების შემქმნელები და DA-5 განრიგები ატენიანებენ იდენტურ შეყვანებს გარეშე
  მანიფესტების ხელახლა წაკითხვა ან ბლოკების შენახვა.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK დამხმარე API-ები ავლენს PDP სათაურის დატვირთვას აბონენტების იძულების გარეშე, განაახლონ Norito დეკოდირება:
  Rust crate ექსპორტს ახორციელებს `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python
  `ToriiClient` ახლა მოიცავს `decode_pdp_commitment_header` და `IrohaSwift` გემებს
  `decodePdpCommitmentHeader` გადატვირთვები ნედლი სათაურის რუკებისთვის ან `HTTPURLResponse` შემთხვევები.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii ასევე ამჟღავნებს `GET /v1/da/manifests/{storage_ticket}`-ს, რათა SDK-ებმა და ოპერატორებმა შეძლონ მანიფესტების მიღება
  და ბლოკის გეგმები კვანძის კოჭის დირექტორიაში შეხების გარეშე. პასუხი აბრუნებს Norito ბაიტს
  (base64), გამოსახულია მანიფესტი JSON, `chunk_plan` JSON blob მზად `sorafs fetch`-ისთვის, შესაბამისი
  თექვსმეტობითი დამუშავება (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) და ასახავს
  `Sora-PDP-Commitment` სათაური პარიტეტისთვის მიღებული პასუხებიდან. მიწოდება `block_hash=<hex>`-ში
  შეკითხვის სტრიქონი აბრუნებს დეტერმინისტულ `sampling_plan`-ს (დავალების ჰეში, `sample_window` და ნიმუშის აღება
  `(index, role, group)` ტოპები, რომლებიც მოიცავს სრულ 2D განლაგებას) ასე რომ, ვალიდატორები და PoR ხელსაწყოები ერთნაირად ხატავენ
  ინდექსები.

### დიდი დატვირთვის ნაკადის ნაკადი

კლიენტები, რომლებსაც სჭირდებათ კონფიგურირებული მოთხოვნის ლიმიტზე მეტი აქტივების მიღება, იწყებენ ა
სტრიმინგის სესიის დარეკვით `POST /v1/da/ingest/chunk/start`. Torii პასუხობს a
`ChunkSessionId` (BLAKE3-მომდინარეობს მოთხოვნილი blob მეტამონაცემებიდან) და შეთანხმებული ნაწილის ზომა.
ყოველი მომდევნო `DaIngestChunk` მოთხოვნა შეიცავს:- `client_blob_id` — იდენტურია საბოლოო `DaIngestRequest`-ისა.
- `chunk_session_id` — აკავშირებს ნაჭრებს გაშვებულ სესიასთან.
- `chunk_index` და `offset` — აღასრულონ დეტერმინისტული შეკვეთა.
- `payload` - შეთანხმებული ნაწილის ზომამდე.
- `payload_hash` — BLAKE3 ჰეშის ნაჭერი, რათა Torii-მა შეძლოს ვალიდაცია მთელი ბლოკის ბუფერის გარეშე.
- `is_last` - მიუთითებს ტერმინალის ნაწილზე.

Torii აგრძელებს დადასტურებულ ნაჭრებს `config.da_ingest.manifest_store_dir/chunks/<session>/`-ში და
ჩანაწერს პროგრესს განმეორებითი ქეშის შიგნით, რათა პატივი სცეს იმპოტენციას. როდესაც საბოლოო ნაჭერი ჩამოდის, Torii
ხელახლა აწყობს დატვირთვას დისკზე (გადის განყოფილების დირექტორიაში, რათა თავიდან აიცილოს მეხსიერების მწვერვალები),
ითვლის კანონიკურ მანიფესტს/მიღებს ზუსტად ისე, როგორც ერთჯერადი ატვირთვისას და ბოლოს პასუხობს
`POST /v1/da/ingest` დადგმული არტეფაქტის მოხმარებით. წარუმატებელი სესიები შეიძლება შეწყდეს აშკარად ან
ნაგავი გროვდება `config.da_ingest.replay_cache_ttl`-ის შემდეგ. ეს დიზაინი ინარჩუნებს ქსელის ფორმატს
Norito მეგობრული, თავს არიდებს კლიენტის სპეციფიკურ განახლების პროტოკოლებს და ხელახლა იყენებს არსებულ manifest მილსადენს
უცვლელი.

**განხორციელების სტატუსი.** კანონიკური Norito ტიპები ახლა ცხოვრობენ
`crates/iroha_data_model/src/da/`:

- `ingest.rs` განსაზღვრავს `DaIngestRequest`/`DaIngestReceipt`, ერთად
  `ExtraMetadata` კონტეინერი გამოიყენება Torii-ის მიერ.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` მასპინძლებს `DaManifestV1` და `ChunkCommitment`, რომელსაც Torii გამოსცემს შემდეგ
  დაქუცმაცება დასრულდა.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` უზრუნველყოფს საზიარო მეტსახელებს (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` და ა.შ.) და დაშიფვრავს ქვემოთ მოყვანილ პოლიტიკის ნაგულისხმევ მნიშვნელობებს.【crates/iroha_data_model/src/da/types.rs:240】
- მანიფესტის კოჭის ფაილები ჩამოდის `config.da_ingest.manifest_store_dir`-ში, მზად არის SoraFS ორკესტრირებისთვის
  მეთვალყურე უნდა გაიყვანოს საცავში შესასვლელად.【crates/iroha_torii/src/da/ingest.rs:220】

მოთხოვნის, მანიფესტისა და ქვითრის ტვირთამწეობის ორმხრივი დაფარვის თვალყურის დევნება ხდება
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, უზრუნველყოფს Norito კოდეკს
რჩება სტაბილური განახლებების დროს.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**შეკავების ნაგულისხმევი.** მმართველობამ რატიფიცირება მოახდინა თავდაპირველი შენახვის პოლიტიკის დროს
SF-6; ნაგულისხმევი ნაგულისხმევი `RetentionPolicy::default()` არის:

- ცხელი დონე: 7 დღე (`604_800` წამი)
- ცივი იარუსი: 90 დღე (`7_776_000` წამი)
- საჭირო ასლები: `3`
- შენახვის კლასი: `StorageClass::Hot`
- მმართველობის ნიშანი: `"da.default"`

ქვედა დინების ოპერატორებმა აშკარად უნდა გადალახონ ეს მნიშვნელობები, როდესაც ზოლი მიიღება
უფრო მკაცრი მოთხოვნები.