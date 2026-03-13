---
lang: ka
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus მონაცემთა ხელმისაწვდომობა Ingest გეგმა

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
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Payload არის Norito-ში კოდირებული `DaIngestRequest`. პასუხების გამოყენება
`application/norito+v1` და დაბრუნება `DaIngestReceipt`.

| პასუხი | მნიშვნელობა |
| --- | --- |
| 202 მიღებული | Blob რიგში დაქუცმაცების/განმეორებისთვის; ქვითარი დაბრუნდა. |
| 400 ცუდი მოთხოვნა | სქემის/ზომის დარღვევა (იხილეთ ვალიდაციის შემოწმებები). |
| 401 არასანქცირებული | აკლია/არასწორი API ჟეტონი. |
| 409 კონფლიქტი | დუბლიკატი `client_blob_id` შეუსაბამო მეტამონაცემებით. |
| 413 დატვირთვა ძალიან დიდი | აჭარბებს კონფიგურირებულ ბლოკის სიგრძის ლიმიტს. |
| 429 ძალიან ბევრი მოთხოვნა | შეფასების ლიმიტი მიღწეულია. |
| 500 შიდა შეცდომა | მოულოდნელი მარცხი (რეგისტრირებული + გაფრთხილება). |

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

აბრუნებს `DaProofPolicyBundle` ვერსიას, რომელიც მიღებულია მიმდინარე ზოლის კატალოგიდან.
პაკეტში რეკლამირებულია `version` (ამჟამად `1`), `policy_hash` (ჰეში)
შეკვეთილი პოლიტიკის სია), და `policies` ჩანაწერები, რომლებიც შეიცავს `lane_id`, `dataspace_id`,
`alias` და იძულებითი `proof_scheme` (`merkle_sha256` დღეს; KZG ზოლები არის
უარყოფილია შეყვანის გზით, სანამ KZG ვალდებულებები არ იქნება ხელმისაწვდომი). ბლოკის სათაური ახლა
ავალდებულებს პაკეტს `da_proof_policies_hash`-ის მეშვეობით, რათა კლიენტებს შეეძლოთ ჩამაგრება
აქტიური პოლიტიკა დაწესებულია DA ვალდებულებების ან მტკიცებულებების შემოწმებისას. მიიღეთ ეს საბოლოო წერტილი
მტკიცებულებების შექმნამდე, რათა დარწმუნდეთ, რომ ისინი შეესაბამება ზოლის პოლიტიკასა და მიმდინარეობას
შეკვრა ჰეში. ვალდებულებების სია/დასამტკიცებელი საბოლოო წერტილები ატარებს იგივე პაკეტს, ასე რომ SDK-ები
არ დაგჭირდებათ დამატებითი ორმხრივი მგზავრობა, რომ დამადასტურებელი საბუთი დააკავშიროთ აქტიურ პოლიტიკას.

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

აბრუნებს `DaProofPolicyBundle`-ს, რომელსაც აქვს შეკვეთილი პოლიტიკის სია პლუს a
`policy_hash`, რათა SDK-ებმა შეძლონ ჩამაგრება ვერსია, რომელიც გამოიყენება ბლოკის წარმოებისას. The
ჰეში გამოითვლება Norito-ში დაშიფრული პოლიტიკის მასივზე და იცვლება, როდესაც
lane's `proof_scheme` განახლებულია, რაც კლიენტებს საშუალებას აძლევს ამოიცნონ დრიფტი შორის
ქეშირებული მტკიცებულებები და ჯაჭვის კონფიგურაცია.

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
```> განხორციელების შენიშვნა: კანონიკური Rust-ის წარმოდგენები ამ ტვირთამწეობისთვის ახლა მოთავსებულია ქვეშ
> `iroha_data_model::da::types`, მოთხოვნის/ქვითრის შეფუთვით `iroha_data_model::da::ingest`-ში
> და მანიფესტის სტრუქტურა `iroha_data_model::da::manifest`-ში.

`compression` ველი აქვეყნებს რეკლამას, თუ როგორ მოამზადეს აბონენტებმა ტვირთი. Torii იღებს
`identity`, `gzip`, `deflate` და `zstd`, ბაიტების გამჭვირვალედ დეკომპრესია მანამდე
არჩევითი მანიფესტების ჰეშირება, დაქუცმაცება და გადამოწმება.

### დადასტურების ჩამონათვალი

1. დაადასტურეთ მოთხოვნა Norito სათაური ემთხვევა `DaIngestRequest`.
2. წარუმატებლობა, თუ `total_size` განსხვავდება კანონიკური (დეკომპრესირებული) დატვირთვის სიგრძისგან ან აღემატება კონფიგურირებულ მაქსიმუმს.
3. განახორციელეთ `chunk_size` გასწორება (ძალა ორიდან, = 2.
5. `retention_policy.required_replica_count` უნდა იცავდეს მმართველობის საწყისს.
6. ხელმოწერის შემოწმება კანონიკური ჰეშის წინააღმდეგ (ხელმოწერის ველის გამოკლებით).
7. უარი თქვით `client_blob_id`-ის დუბლიკატზე, გარდა იმ შემთხვევისა, როდესაც payload ჰეში + მეტამონაცემები იდენტურია.
8. როდესაც მოწოდებულია `norito_manifest`, გადაამოწმეთ სქემა + ჰეშის შესატყვისები ხელახლა გამოთვლილი
   გამოიხატება დაქუცმაცების შემდეგ; წინააღმდეგ შემთხვევაში, კვანძი ქმნის მანიფესტს და ინახავს მას.
9. განახორციელეთ კონფიგურირებული რეპლიკაციის პოლიტიკა: Torii ხელახლა წერს წარდგენილ
   `RetentionPolicy` `torii.da_ingest.replication_policy`-თან ერთად (იხ.
   `replication_policy.md`) და უარყოფს წინასწარ ჩაშენებულ მანიფესტებს, რომელთა შეკავება
   მეტამონაცემები არ ემთხვევა იძულებით პროფილს.

### დაქუცმაცება და რეპლიკაციის ნაკადი1. ჩაყარეთ დატვირთვა `chunk_size`-ში, გამოთვალეთ BLAKE3 თითო ნაწილზე + Merkle root.
2. Build Norito `DaManifestV1` (ახალი სტრუქტურა) ბლოკის ვალდებულებების აღბეჭდვა (role/group_id),
   წაშლის განლაგება (სტრიქონებისა და სვეტების პარიტეტის რაოდენობა პლუს `ipa_commitment`), შენახვის პოლიტიკა,
   და მეტამონაცემები.
3. რიგით მანიფესტის კანონიკური ბაიტები `config.da_ingest.manifest_store_dir` ქვეშ
   (Torii წერს `manifest.encoded` ფაილებს, რომლებიც ჩასმულია ზოლის/ეპოქის/მიმდევრობის/ბილეთის/თითის ანაბეჭდის მიხედვით) ასე რომ, SoraFS
   ორკესტრაციას შეუძლია მათი გადაყლაპვა და შენახვის ბილეთის დაკავშირება მუდმივ მონაცემებთან.
4. გამოაქვეყნეთ პინის მიზნები `sorafs_car::PinIntent`-ის მეშვეობით მმართველობის ტეგით + პოლიტიკით.
5. გამოუშვით Norito მოვლენა `DaIngestPublished` დამკვირვებლების შესატყობინებლად (მსუბუქი კლიენტები,
   მმართველობა, ანალიტიკა).
6. დააბრუნეთ `DaIngestReceipt` (ხელმოწერილი Torii DA სერვისის გასაღებით) და დაამატეთ
   `Sora-PDP-Commitment` პასუხის სათაური, რომელიც შეიცავს base64 Norito დაშიფვრას
   მიღებული ვალდებულება, ასე რომ SDK-ებს შეუძლიათ დაუყოვნებლივ შეინახონ სინჯის თესლი.
   ქვითარში ახლა ჩაშენებულია `rent_quote` (a `DaRentQuote`) და `stripe_layout`
   ასე რომ, წარმდგენებს შეუძლიათ გამოავლინონ XOR ვალდებულებები, სარეზერვო წილი, PDP/PoTR ბონუსების მოლოდინები,
   და 2D წაშლის მატრიცის ზომები შენახვის ბილეთების მეტამონაცემებთან ერთად თანხების ჩადებამდე.
7. არჩევითი რეესტრის მეტამონაცემები:
   - `da.registry.alias` — საჯარო, დაუშიფრავი UTF-8 მეტსახელის სტრიქონი პინის რეესტრის ჩანაწერის დასათესად.
   - `da.registry.owner` — საჯარო, დაუშიფრავი `AccountId` სტრიქონი რეესტრის საკუთრების ჩასაწერად.
   Torii აკოპირებს მათ გენერირებულ `DaPinIntent`-ში, ასე რომ, ქვედა დინების პინის დამუშავებამ შეიძლება დააკავშიროს მეტსახელები
   და მფლობელები ნედლი მეტამონაცემების რუკის ხელახალი ანალიზის გარეშე; არასწორი ან ცარიელი მნიშვნელობები უარყოფილია დროს
   გადაყლაპვის ვალიდაცია.

## შენახვის / რეესტრის განახლებები

- გააფართოვეთ `sorafs_manifest` `DaManifestV1`-ით, რაც საშუალებას იძლევა განმსაზღვრელი ანალიზი.
- დაამატეთ ახალი რეესტრის ნაკადი `da.pin_intent`, ვერსიული დატვირთვის მითითებით
  მანიფესტის ჰეში + ბილეთის ID.
- განაახლეთ დაკვირვებადობის მილსადენები, რათა თვალყური ადევნოთ შეყოვნებას, დაშლის გამტარუნარიანობას,
  რეპლიკაციის ჩამორჩენა და წარუმატებლობის რაოდენობა.
- Torii `/status` პასუხები ახლა მოიცავს `taikai_ingest` მასივს, რომელიც ასახავს უახლეს
  დაშიფვრის შეყოვნების შეყოვნება, პირდაპირი გვერდის დრიფტი და შეცდომების მრიცხველები თითო (კლასტერი, ნაკადი), DA-9-ის ჩართვა
  საინფორმაციო დაფები ჯანმრთელობის სნეპშოტების პირდაპირ კვანძებიდან გადასაღებად Prometheus გაფხეკის გარეშე.

## ტესტირების სტრატეგია- ერთეულის ტესტები სქემის ვალიდაციისთვის, ხელმოწერის შემოწმებისთვის, დუბლიკატების გამოვლენისთვის.
- ოქროს ტესტები, რომლებიც ადასტურებენ `DaIngestRequest`-ის Norito კოდირებას, მანიფესტს და ქვითარს.
- ინტეგრაციის აღკაზმულობა ტრიალებს იმიტირებულ SoraFS + რეესტრს, ამტკიცებს ნაწილს + პინის ნაკადებს.
- საკუთრების ტესტები, რომლებიც მოიცავს შემთხვევითი წაშლის პროფილებს და შეკავების კომბინაციებს.
- Norito დატვირთვის დაბინძურება არასწორი მეტამონაცემებისგან თავის დასაცავად.
- ოქროს მოწყობილობები ყველა ბლომად კლასისთვის ცხოვრობს ქვეშ
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` კომპანიონ ნაწილთან ერთად
  ჩამონათვალი `fixtures/da/ingest/sample_chunk_records.txt`-ში. იგნორირებული ტესტი
  `regenerate_da_ingest_fixtures` განაახლებს მოწყობილობებს, ხოლო
  `manifest_fixtures_cover_all_blob_classes` მარცხდება, როგორც კი დაემატება ახალი `BlobClass` ვარიანტი
  Norito/JSON პაკეტის განახლების გარეშე. ეს ინარჩუნებს Torii-ს, SDK-ებსა და დოკუმენტებს პატიოსანი, როდესაც DA-2
  იღებს ახალ blob ზედაპირს.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI & SDK Tooling (DA-8)- `iroha app da submit` (ახალი CLI შესვლის წერტილი) ახლა ახვევს გაზიარებულ ჩაწერის შემქმნელს/გამომცემელს, რათა ოპერატორები
  შეუძლია თვითნებური ბლომების გადაყლაპვა ტაიკაის შეკვრის ნაკადის გარეთ. ბრძანება ცხოვრობს
  `crates/iroha_cli/src/commands/da.rs:1` და მოიხმარს დატვირთვას, წაშლის/შეკავების პროფილს და
  არჩევითი მეტამონაცემების/მანიფესტის ფაილები კანონიკურ `DaIngestRequest`-ზე ხელმოწერამდე CLI-ით
  კონფიგურაციის გასაღები. წარმატებული გაშვებები გრძელდება `da_request.{norito,json}` და `da_receipt.{norito,json}` ქვეშ
  `artifacts/da/submission_<timestamp>/` (გადალახვა `--artifact-dir`-ის მეშვეობით), ასე რომ, არტეფაქტების გამოშვება შესაძლებელია
  ჩაწერეთ ზუსტი Norito ბაიტი, რომელიც გამოიყენება მიღების დროს.
- ბრძანება ნაგულისხმევად არის `client_blob_id = blake3(payload)`, მაგრამ იღებს უგულებელყოფას მეშვეობით
  `--client-blob-id`, აფასებს მეტამონაცემებს JSON რუკებს (`--metadata-json`) და წინასწარ გენერირებულ მანიფესტებს
  (`--manifest`) და მხარს უჭერს `--no-submit` ხაზგარეშე მომზადებისთვის პლუს `--endpoint` მორგებისთვის
  Torii მასპინძლები. ქვითარი JSON იბეჭდება stdout-ზე, გარდა იმისა, რომ იწერება დისკზე, ხურავს მას
  DA-8 "submit_blob" ხელსაწყოების მოთხოვნა და SDK პარიტეტული მუშაობის განბლოკვა.
- `iroha app da get` ამატებს DA-ზე ფოკუსირებულ მეტსახელს მრავალ წყაროს ორკესტრისთვის, რომელიც უკვე მუშაობს
  `iroha app sorafs fetch`. ოპერატორებს შეუძლიათ მიუთითონ მანიფესტზე + ბლოკ-გეგმის არტეფაქტებზე (`--manifest`,
  `--plan`, `--manifest-id`) **ან** უბრალოდ გაიარეთ Torii შენახვის ბილეთი `--storage-ticket`-ის მეშვეობით. როცა
  ბილეთის ბილიკი გამოიყენება, CLI ამოიღებს მანიფესტს `/v2/da/manifests/<ticket>`-დან, აგრძელებს პაკეტს
  `artifacts/da/fetch_<timestamp>/` ქვეშ (გადალახვა `--manifest-cache-dir`-ით), გამოდის **მანიფესტი
  ჰაში** `--manifest-id`-ისთვის და შემდეგ მართავს ორკესტრატორს მოწოდებული `--gateway-provider`-ით
  სია. დატვირთვის დადასტურება კვლავ ეყრდნობა ჩაშენებულ CAR/`blob_hash` დაიჯესტს, სანამ კარიბჭის ID არის
  ახლა მანიფესტის ჰეშია, ასე რომ კლიენტები და ვალიდატორები იზიარებენ ერთი blob იდენტიფიკატორს. ყველა მოწინავე ღილაკიდან
  SoraFS ჩამოტანის ზედაპირი ხელუხლებელია (მანიფესტის კონვერტები, კლიენტის ეტიკეტები, დაცვის ქეში, ანონიმურობის ტრანსპორტირება
  უგულებელყოფს, ქულების ექსპორტს და `--output` ბილიკებს) და მანიფესტის საბოლოო წერტილის გადაფარვა შესაძლებელია მეშვეობით
  `--manifest-endpoint` მორგებული Torii ჰოსტებისთვის, ამიტომ ხელმისაწვდომობის ხელმისაწვდომობის შემოწმებები მთლიანად მოქმედებს
  `da` სახელთა სივრცე ორკესტრის ლოგიკის დუბლირების გარეშე.
- `iroha app da get-blob` გამოაქვს კანონიკური მანიფესტები პირდაპირ Torii-დან `GET /v2/da/manifests/{storage_ticket}`-ის მეშვეობით.
  ბრძანება ახლა ასახელებს არტეფაქტებს მანიფესტის ჰეშით (blob id), წერით
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` და `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/`-ის ქვეშ (ან მომხმარებლის მიერ მოწოდებული `--output-dir`) ზუსტი ექოს დროს
  `iroha app da get` გამოძახება (მათ შორის `--manifest-id`) საჭიროა შემდგომი ორკესტრატორის მისაღებად.
  ეს აშორებს ოპერატორებს მანიფესტის კოჭის დირექტორიებიდან და გარანტიას იძლევა, რომ მიმღები ყოველთვის იყენებს მას
  Torii-ის მიერ გამოშვებული ხელმოწერილი არტეფაქტები. JavaScript Torii კლიენტი ასახავს ამ ნაკადს
  `ToriiClient.getDaManifest(storageTicketHex)`, ხოლო Swift SDK ახლა გამოაშკარავდება
  `ToriiClient.getDaManifestBundle(...)`. ორივე აბრუნებს გაშიფრულ Norito ბაიტს, მანიფესტის JSON, მანიფესტის ჰეშს,და ბლოკის გეგმა ისე, რომ SDK აბონენტებმა შეძლონ ორკესტრის სესიების დატენიანება CLI-სა და Swift-ზე გადარიცხვის გარეშე
  კლიენტებს შეუძლიათ დამატებით დარეკონ `fetchDaPayloadViaGateway(...)` ამ პაკეტების გადასატანად მშობლიურში
  SoraFS ორკესტრის შეფუთვა.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v2/da/manifests` პასუხები ახლა გამოჩნდება `manifest_hash` და ორივე CLI + SDK დამხმარე (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` და Swift/JS კარიბჭის შეფუთვა) ამ დაიჯესტს განიხილავს, როგორც
  კანონიკური მანიფესტის იდენტიფიკატორი, როდესაც აგრძელებთ დატვირთვის გადამოწმებას ჩაშენებული CAR/blob ჰეშის მიხედვით.
- `iroha app da rent-quote` ითვლის დეტერმინისტულ ქირაობას და სტიმულირებას მოწოდებული შენახვის ზომისთვის
  და შეკავების ფანჯარა. დამხმარე მოიხმარს ან აქტიურ `DaRentPolicyV1`-ს (JSON ან Norito ბაიტს) ან
  ჩაშენებული ნაგულისხმევი, ამოწმებს პოლიტიკას და ბეჭდავს JSON რეზიუმეს (`gib`, `months`, წესების მეტამონაცემებს,
  და `DaRentQuote` ველები), ასე რომ აუდიტორებს შეუძლიათ მოიყვანონ ზუსტი XOR გადასახადები მმართველობის წუთებში, გარეშე
  ad hoc სკრიპტების წერა. ბრძანება ახლა ასევე გამოსცემს ერთხაზოვან `rent_quote ...` შეჯამებას JSON-მდე
  payload რათა კონსოლის ჟურნალები და წიგნები უფრო ადვილად სკანირდეს, როდესაც ციტატები გენერირდება ინციდენტების დროს.
  საშვი `--quote-out artifacts/da/rent_quotes/<stamp>.json` (ან ნებისმიერი სხვა გზა)
  რომ შენარჩუნდეს ლამაზად დაბეჭდილი რეზიუმე და გამოიყენოთ `--policy-label "governance ticket #..."`, როდესაც
  არტეფაქტს სჭირდება კონკრეტული ხმის/კონფიგურაციის ნაკრების ციტირება; CLI ჭრის მორგებულ ეტიკეტებს და უარყოფს ცარიელებს
  სტრიქონები, რათა შეინარჩუნოს `policy_source` მნიშვნელობები მტკიცებულების პაკეტებში. იხ
  `crates/iroha_cli/src/commands/da.rs` ქვებრძანებისთვის და `docs/source/da/rent_policy.md`
  პოლიტიკის სქემისთვის.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- პინის რეესტრის პარიტეტი ახლა ვრცელდება SDK-ებზე: `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK აშენებს `iroha app sorafs pin register`-ის მიერ გამოყენებულ ზუსტ დატვირთვას, ახორციელებს კანონიკურ
  chunker-ის მეტამონაცემები, პინის წესები, ალიასის მტკიცებულებები და შემდგომი დაიჯესტები POST-ში
  `/v2/sorafs/pin/register`. ეს აფერხებს CI ბოტებს და ავტომატიზაციას CLI-ზე დაბომბვისგან
  ჩაწერს მანიფესტის რეგისტრაციებს და დამხმარე იგზავნება TypeScript/README დაფარვით, ასე რომ DA-8
  "submit/get/prove" ხელსაწყოების პარიტეტი სრულად დაკმაყოფილებულია JS-ზე Rust/Swift-თან ერთად.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788
- `iroha app da prove-availability` აკავშირებს ყველა ჩამოთვლილს: იღებს შენახვის ბილეთს, ჩამოტვირთავს
  კანონიკური მანიფესტის ნაკრები, აწარმოებს მრავალ წყაროს ორკესტრატორს (`iroha app sorafs fetch`) წინააღმდეგ
  მოწოდებული `--gateway-provider` სია, ნარჩუნდება გადმოწერილი დატვირთვა + ქულების დაფა ქვემოთ
  `artifacts/da/prove_availability_<timestamp>/` და დაუყოვნებლივ გამოიძახებს არსებულ PoR დამხმარეს
  (`iroha app da prove`) მოტანილი ბაიტების გამოყენებით. ოპერატორებს შეუძლიათ ორკესტრის სახელურების შესწორება
  (`--max-peers`, `--scoreboard-out`, მანიფესტის საბოლოო წერტილის უგულებელყოფა) და მტკიცებულების ნიმუში
  (`--sample-count`, `--leaf-index`, `--sample-seed`) ხოლო ერთი ბრძანება აწარმოებს არტეფაქტებს
  მოსალოდნელია DA-5/DA-9 აუდიტის მიერ: დატვირთვის ასლი, ანგარიშის დაფის მტკიცებულება და JSON მტკიცებულების შეჯამებები.- `da_reconstruct` (ახალი DA-6-ში) კითხულობს კანონიკურ მანიფესტს პლუს ნაწილის მიერ გამოშვებული განყოფილების დირექტორია
  შეინახოს (`chunk_{index:05}.bin` განლაგება) და გადამოწმების დროს გადამწყვეტად ააწყობს დატვირთვას
  Blake3-ის ყოველი ვალდებულება. CLI ცხოვრობს `crates/sorafs_car/src/bin/da_reconstruct.rs` ქვეშ და იგზავნება როგორც
  SoraFS ხელსაწყოების ნაკრების ნაწილი. ტიპიური ნაკადი:
  1. `iroha app da get-blob --storage-ticket <ticket>` ჩამოტვირთეთ `manifest_<manifest_hash>.norito` და ცალი გეგმა.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (ან `iroha app da prove-availability`, რომელიც წერს მოტანილი არტეფაქტების ქვეშ
     `artifacts/da/prove_availability_<ts>/` და რჩება თითო ცალი ფაილი `chunks/` დირექტორიაში).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  რეგრესიული მოწყობილობა ცხოვრობს `fixtures/da/reconstruct/rs_parity_v1/`-ის ქვეშ და იღებს სრულ მანიფესტს
  და ნაწილის მატრიცა (მონაცემები + პარიტეტი) გამოყენებული `tests::reconstructs_fixture_with_parity_chunks`-ის მიერ. განაახლეთ იგი

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  მოწყობილობა გამოსცემს:

  - `manifest.{norito.hex,json}` — კანონიკური `DaManifestV1` კოდირებები.
  - `chunk_matrix.json` — შეკვეთილი ინდექსი/ოფსეტური/სიგრძე/დაჯესტი/პარიტეტი რიგები დოკუმენტის/ტესტირების მითითებისთვის.
  - `chunks/` — `chunk_{index:05}.bin` დატვირთვის ფრაგმენტები როგორც მონაცემებისთვის, ასევე პარიტეტული ფრაგმენტებისთვის.
  - `payload.bin` — დეტერმინისტული დატვირთვა, რომელიც გამოიყენება პარიტეტული აღკაზმულობის ტესტით.
  - `commitment_bundle.{json,norito.hex}` — ნიმუში `DaCommitmentBundle` დეტერმინისტული KZG ვალდებულებით დოკუმენტებისთვის/ტესტებისთვის.

  აღკაზმულობა უარს ამბობს გამოტოვებულ ან შეკვეცილ ნაწილებზე, ამოწმებს Blake3-ის საბოლოო დატვირთვას `blob_hash`-თან მიმართებაში,
  და ასხივებს შემაჯამებელ JSON ბლოკს (გადატვირთვის ბაიტები, ნაჭრების რაოდენობა, შენახვის ბილეთი), რათა CI-მ შეძლოს რეკონსტრუქციის დამტკიცება
  მტკიცებულება. ეს ხურავს DA-6 მოთხოვნას განმსაზღვრელი რეკონსტრუქციის ხელსაწყოსათვის, რომელიც ოპერატორებს და QA-ს ემსახურება
  სამუშაოებს შეუძლიათ გამოიძახონ შეკვეთილი სკრიპტების გაყვანილობის გარეშე.

## TODO რეზოლუციის შეჯამება

ყველა ადრე დაბლოკილი შეყვანის TODO დანერგილი და დამოწმებულია:- **შეკუმშვის მინიშნებები** — Torii იღებს აბონენტის მიერ მოწოდებულ ეტიკეტებს (`identity`, `gzip`, `deflate`,
  `zstd`) და ახდენს დატვირთვის ნორმალიზებას ვალიდაციამდე ისე, რომ კანონიკური მანიფესტის ჰეში ემთხვევა
  დეკომპრესირებული ბაიტები.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **მეტამონაცემების მხოლოდ მმართველობითი დაშიფვრა** — Torii ახლა შიფრავს მმართველობის მეტამონაცემებს
  კონფიგურირებული კლავიში ChaCha20-Poly1305, უარყოფს შეუსაბამო ლეიბლებს და ასახავს ორ აშკარად
  კონფიგურაციის სახელურები (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) ბრუნვის დეტერმინისტულად შესანარჩუნებლად.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **დიდი დატვირთვის ნაკადი** — მრავალ ნაწილის მიღება პირდაპირ ეთერშია. კლიენტების ნაკადი განმსაზღვრელია
  `DaIngestChunk` კონვერტები ჩასმულია `client_blob_id`-ით, Torii ამოწმებს თითოეულ ნაჭერს და ეტაპებს
  `manifest_store_dir` ქვეშ და ატომურად აღადგენს მანიფესტს, როგორც კი `is_last` დროშა დადგება,
  RAM-ის მწვერვალების აღმოფხვრა ერთი ზარის ატვირთვით.【crates/iroha_torii/src/da/ingest.rs:392】
- **მანიფესტური ვერსიირება** — `DaManifestV1` შეიცავს აშკარა `version` ველს და Torii უარს ამბობს
  უცნობი ვერსიები, გარანტირებულია დეტერმინისტული განახლებები, როდესაც ახალი მანიფესტის განლაგება გაიგზავნება.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR კაკვები** — PDP-ის ვალდებულებები უშუალოდ მომდინარეობს ნაჭრების შენახვისგან და შენარჩუნებულია
  მანიფესტების გარდა, რათა DA-5 განრიგებმა შეძლონ შერჩევის გამოწვევების გაშვება კანონიკური მონაცემებიდან; The
  `Sora-PDP-Commitment` სათაური ახლა მიეწოდება `/v2/da/ingest` და `/v2/da/manifests/{ticket}`
  პასუხები, ასე რომ SDK-ებმა დაუყოვნებლივ გაიგეს ხელმოწერილი ვალდებულება, რომელსაც მომავალი ზონდები მიუთითებენ.
- **Shard კურსორის ჟურნალი** — ხაზის მეტამონაცემებმა შეიძლება მიუთითოს `da_shard_id` (ნაგულისხმევად `lane_id`) და
  Sumeragi ახლა ინარჩუნებს უმაღლეს `(epoch, sequence)` `(shard_id, lane_id)`-ზე
  `da-shard-cursors.norito` DA კოჭის გვერდით, ასე რომ გადატვირთეთ გადახაზული/უცნობი ზოლები და შეინახეთ
  განმეორებითი დეტერმინისტული. მეხსიერების ფრაგმენტის კურსორის ინდექსი ახლა სწრაფად ვერ ხერხდება ვალდებულებებისთვის
  გაუსწორებელი ზოლები, ნაცვლად ზოლის ID-ის ნაგულისხმევი, კურსორის წინსვლისა და გამეორების შეცდომის გამო
  მკაფიო და ბლოკის ვალიდაცია უარყოფს კურსორის ნატეხების რეგრესიას გამოყოფილი
  `DaShardCursorViolation` მიზეზი + ტელემეტრიული ეტიკეტები ოპერატორებისთვის. Startup/catch-up ახლა წყვეტს DA
  დატენიანების ინდექსი, თუ კურა შეიცავს უცნობ ხაზს ან რეგრესირებულ კურსორს და ჩაწერს შეურაცხყოფას
  ბლოკის სიმაღლე, რათა ოპერატორებმა შეძლონ რემედიაცია DA-ს მომსახურებამდე state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- ** კურსორის ჩამორჩენის ტელემეტრია** — `da_shard_cursor_lag_blocks{lane,shard}` ლიანდაგი იუწყება, თუ როგორშორს ნატეხი გადის დამოწმებულ სიმაღლეს. დაკარგული/მოძველებული/უცნობი ბილიკები დააწესეს ჩამორჩენას
  საჭირო სიმაღლე (ან დელტა) და წარმატებული მიღწევები მას ნულამდე აყენებს, რათა სტაბილური მდგომარეობა დარჩეს.
  ოპერატორებმა უნდა გააფრთხილონ ნულოვანი ჩამორჩენების შესახებ, შეამოწმონ DA კოჭა/ჟურნალი დამრღვევი ზოლისთვის,
  და გადაამოწმეთ ზოლის კატალოგში შემთხვევითი გადართვა ბლოკის გასასუფთავებლად
  უფსკრული.
- **კონფიდენციალური გამოთვლითი ზოლები** — ზოლები, რომლებიც მონიშნულია
  `metadata.confidential_compute=true` და `confidential_key_version` განიხილება როგორც
  SMPC/დაშიფრული DA ბილიკები: Sumeragi ახორციელებს ნულოვანი დატვირთვის/მანიფესტის შეჯამებებს და შენახვის ბილეთებს,
  უარყოფს სრული რეპლიკა შენახვის პროფილებს და ინდექსებს SoraFS ბილეთს + პოლიტიკის ვერსიას გარეშე
  დატვირთვის ბაიტების გამოვლენა. ქვითრები ატენიანდება Kura-დან გამეორების დროს, ასე რომ, ვალიდატორები იმავეს აღადგენენ
  კონფიდენციალურობის მეტამონაცემების შემდეგ გადაიტვირთება.

## განხორციელების შენიშვნები- Torii-ის `/v2/da/ingest` ბოლო წერტილი ახლა ახდენს დატვირთვის შეკუმშვის ნორმალიზებას, ახორციელებს განმეორებითი ქეშის,
  დეტერმინისტულად ანაწილებს კანონიკურ ბაიტებს, აღადგენს `DaManifestV1`-ს და ჩამოაგდებს კოდირებულ დატვირთვას
  `config.da_ingest.manifest_store_dir`-ში SoraFS ორკესტრირებისთვის ქვითრის გაცემამდე; The
  დამმუშავებელი ასევე ანიჭებს `Sora-PDP-Commitment` სათაურს, რათა კლიენტებმა შეძლონ დაშიფრული ვალდებულების აღება
  დაუყოვნებლივ.【crates/iroha_torii/src/da/ingest.rs:220】
- კანონიკური `DaCommitmentRecord`-ის შენარჩუნების შემდეგ, Torii ახლა ასხივებს
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ფაილი manifest spool-ის გვერდით.
  თითოეული ჩანაწერი აერთიანებს ჩანაწერს ნედლეული Norito `PdpCommitment` ბაიტით, ამიტომ DA-3 პაკეტის შემქმნელები და
  DA-5 განრიგები იღებენ იდენტურ შენატანებს მანიფესტების ხელახლა წაკითხვის ან ბლოკების შენახვის გარეშე.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK დამხმარეები ამჟღავნებენ PDP სათაურის ბაიტებს ყოველ კლიენტს არ აიძულებენ განაახლონ Norito პარსინგი:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` საფარი Rust, Python `ToriiClient`
  ახლა ექსპორტს ახორციელებს `decode_pdp_commitment_header` და `IrohaSwift` აგზავნის შესაბამის დამხმარეებს ასე მობილურზე
  კლიენტებს შეუძლიათ დაუყონებლივ შეინახონ შერჩევის კოდირებული განრიგი.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1
- Torii ასევე ამჟღავნებს `GET /v2/da/manifests/{storage_ticket}`-ს, რათა SDK-ებმა და ოპერატორებმა შეძლონ მანიფესტების მიღება
  და ბლოკის გეგმები კვანძის კოჭის დირექტორიაში შეხების გარეშე. პასუხი აბრუნებს Norito ბაიტს
  (base64), გამოსახულია მანიფესტი JSON, `chunk_plan` JSON blob მზად `sorafs fetch`-ისთვის, პლუს შესაბამისი
  ექვსკუთხედი დაიჯესტები (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) ასე რომ, ქვემო დინების ინსტრუმენტები შეიძლება
  კვებავს ორკესტრატორს დისჯესტების ხელახალი გამოთვლის გარეშე და გამოსცემს იგივე `Sora-PDP-Commitment` სათაურს
  სარკისებური გადაყლაპვის პასუხები. `block_hash=<hex>` მოთხოვნის პარამეტრად გადაცემა აბრუნებს დეტერმინისტიკას
  `sampling_plan` დაფუძნებულია `block_hash || client_blob_id`-ში (გაზიარებული ვალიდატორებში), რომელიც შეიცავს
  `assignment_hash`, მოთხოვნილი `sample_window` და ნიმუში `(index, role, group)` ტოპები, რომლებიც მოიცავს
  მთელი 2D ზოლის განლაგება, რათა PoR სემპლერებმა და ვალიდატორებმა შეძლონ იგივე ინდექსების გამეორება. სემპლერი
  აერთიანებს `client_blob_id`, `chunk_root` და `ipa_commitment` დავალების ჰეშში; `იროჰა აპლიკაცია მიიღეთ
  --block-hash ` now writes `sampling_plan_.json` manifest-ის გვერდით + ნაჭერი გეგმა
  ჰეში შენახულია და JS/Swift Torii კლიენტები ავლენენ იმავე `assignment_hash_hex`-ის ვალიდატორებს
  და პროვერები იზიარებენ ერთი დეტერმინისტული გამოძიების კომპლექტს. როდესაც Torii დააბრუნებს შერჩევის გეგმას, `iroha app da
  prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) ნაცვლად
  ad-hoc შერჩევისას, ასე რომ PoR მოწმეები ასრულებენ ვალიდატორის დავალებებს, მაშინაც კი, თუ ოპერატორი გამოტოვებს
  `--block-hash` override.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### დიდი დატვირთვის ნაკადის ნაკადიკლიენტები, რომლებსაც სჭირდებათ კონფიგურირებული მოთხოვნის ლიმიტზე მეტი აქტივების მიღება, იწყებენ ა
სტრიმინგის სესიის დარეკვით `POST /v2/da/ingest/chunk/start`. Torii პასუხობს a
`ChunkSessionId` (BLAKE3-მომდინარეობს მოთხოვნილი blob მეტამონაცემებიდან) და შეთანხმებული ნაწილის ზომა.
ყოველი მომდევნო `DaIngestChunk` მოთხოვნა შეიცავს:

- `client_blob_id` — იდენტურია საბოლოო `DaIngestRequest`-ისა.
- `chunk_session_id` — აკავშირებს ნაჭრებს გაშვებულ სესიასთან.
- `chunk_index` და `offset` — აღასრულონ დეტერმინისტული შეკვეთა.
- `payload` - შეთანხმებული ნაწილის ზომამდე.
- `payload_hash` — BLAKE3 ჰეშის ნაჭერი, რათა Torii-მა შეძლოს ვალიდაცია მთელი ბლოკის ბუფერის გარეშე.
- `is_last` - მიუთითებს ტერმინალის ნაწილზე.

Torii აგრძელებს დადასტურებულ ნაწილებს `config.da_ingest.manifest_store_dir/chunks/<session>/`-ში და
ჩანაწერს პროგრესს განმეორებითი ქეშის შიგნით, რათა პატივი სცეს იმპოტენციას. როდესაც საბოლოო ნაჭერი ჩამოდის, Torii
ხელახლა აწყობს დატვირთვას დისკზე (გადის განყოფილების დირექტორიაში, რათა თავიდან აიცილოს მეხსიერების მწვერვალები),
ითვლის კანონიკურ მანიფესტს/მიღებს ზუსტად ისე, როგორც ერთჯერადი ატვირთვისას და ბოლოს პასუხობს
`POST /v2/da/ingest` დადგმული არტეფაქტის მოხმარებით. წარუმატებელი სესიები შეიძლება შეწყდეს აშკარად ან
ნაგავი გროვდება `config.da_ingest.replay_cache_ttl`-ის შემდეგ. ეს დიზაინი ინარჩუნებს ქსელის ფორმატს
Norito-მეგობრული, თავს არიდებს კლიენტის სპეციფიკურ განახლების პროტოკოლებს და ხელახლა იყენებს არსებულ manifest მილსადენს
უცვლელი.

**განხორციელების სტატუსი.** კანონიკური Norito ტიპები ახლა ცხოვრობენ
`crates/iroha_data_model/src/da/`:

- `ingest.rs` განსაზღვრავს `DaIngestRequest`/`DaIngestReceipt`, ერთად
  `ExtraMetadata` კონტეინერი გამოყენებული Torii-ის მიერ.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` მასპინძლებს `DaManifestV1` და `ChunkCommitment`, რომელსაც Torii გამოსცემს შემდეგ
  დაქუცმაცება დასრულდა.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` უზრუნველყოფს საზიარო მეტსახელებს (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` და ა.შ.) და შიფრავს ქვემოთ მოყვანილ პოლიტიკის ნაგულისხმევ მნიშვნელობებს.【crates/iroha_data_model/src/da/types.rs:240】
- მანიფესტის კოჭის ფაილები ჩამოდის `config.da_ingest.manifest_store_dir`-ში, მზად არის SoraFS ორკესტრირებისთვის
  მეთვალყურე უნდა გაიყვანოს საცავში შესასვლელად.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi აიძულებს მანიფესტ ხელმისაწვდომობას DA პაკეტების დალუქვის ან დამოწმებისას:
  ბლოკების ვალიდაცია ვერ ხერხდება, თუ კოჭს აკლია მანიფესტი ან ჰეში განსხვავდება
  ვალდებულებიდან.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

მოთხოვნის, მანიფესტისა და ქვითრის ტვირთამწეობის ორმხრივი დაფარვის თვალყურის დევნება ხდება
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, უზრუნველყოფს Norito კოდეკს
რჩება სტაბილური განახლებების დროს.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**შეკავების ნაგულისხმევი.** მმართველობამ რატიფიცირება მოახდინა თავდაპირველი შენახვის პოლიტიკის დროს
SF-6; ნაგულისხმევი ნაგულისხმევი `RetentionPolicy::default()` არის:- ცხელი იარუსი: 7 დღე (`604_800` წამი)
- ცივი დონე: 90 დღე (`7_776_000` წამი)
- საჭირო ასლები: `3`
- შენახვის კლასი: `StorageClass::Hot`
- მმართველობის ნიშანი: `"da.default"`

ქვედა დინების ოპერატორებმა აშკარად უნდა გადალახონ ეს მნიშვნელობები, როდესაც ზოლი მიიღება
უფრო მკაცრი მოთხოვნები.

## ჟანგის კლიენტის არტეფაქტები

SDK-ებს, რომლებიც ათავსებენ Rust კლიენტს, აღარ სჭირდებათ CLI-ში გადარიცხვა
შექმენით კანონიკური PoR JSON პაკეტი. `Client` ავლენს ორ დამხმარეს:

- `build_da_proof_artifact` აბრუნებს გენერირებულ ზუსტ სტრუქტურას
  `iroha app da prove --json-out`, მოწოდებული მანიფესტის/სასარგებლო დატვირთვის ანოტაციების ჩათვლით
  მეშვეობით [`DaProofArtifactMetadata`].【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` ახვევს მშენებელს და აგრძელებს არტეფაქტს დისკზე
  (საკმაოდ JSON + ახალი ხაზი ნაგულისხმევად) ასე რომ ავტომატიზაციას შეუძლია ფაილის მიმაგრება
  გამოშვებების ან მმართველობის მტკიცებულებების პაკეტებისთვის.【crates/iroha/src/client.rs:3653】

### მაგალითი

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

JSON დატვირთვა, რომელიც ტოვებს დამხმარეს, ემთხვევა CLI-ს ველების სახელებს
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest` და სხვ.), ისე არსებული
ავტომატიზაციას შეუძლია განასხვავოს/პარკეტი/ატვირთოს ფაილი ფორმატის სპეციფიკური განშტოებების გარეშე.

## დადასტურების გადამოწმების საორიენტაციო ნიშანი

გამოიყენეთ DA მტკიცებულების საორიენტაციო აღკაზმულობა, რათა გადაამოწმოთ დამადასტურებელი ბიუჯეტები წარმომადგენლობით დატვირთვაზე ადრე
ბლოკის დონის ქუდების გამკაცრება:

- `cargo xtask da-proof-bench` აღადგენს ბლოკის მაღაზიას manifest/payload წყვილიდან, ნიმუშები PoR
  ფურცლები და ჯერ გადამოწმება კონფიგურირებული ბიუჯეტის მიხედვით. Taikai მეტამონაცემები ავტომატურად ივსება და
  აღკაზმულობა უბრუნდება სინთეზურ მანიფესტს, თუ არმატურის წყვილი არათანმიმდევრულია. როცა `--payload-bytes`
  დაყენებულია აშკარა `--payload`-ის გარეშე, გენერირებული blob იწერება
  `artifacts/da/proof_bench/payload.bin`, რათა მოწყობილობები ხელუხლებელი დარჩეს.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- იტყობინება ნაგულისხმევად `artifacts/da/proof_bench/benchmark.{json,md}` და მოიცავს მტკიცებულებებს/გაშვებას, საერთო და
  თითო მტკიცებულების დროები, ბიუჯეტის გავლის მაჩვენებელი და რეკომენდებული ბიუჯეტი (ყველაზე ნელი გამეორების 110%)
  შედით `zk.halo2.verifier_budget_ms`-თან.【artifacts/da/proof_bench/benchmark.md:1】
- უახლესი გაშვება (სინთეზური 1 MiB დატვირთვა, 64 KiB ნაჭერი, 32 მტკიცებულება/გაშვება, 10 გამეორება, 250 ms ბიუჯეტი)
  რეკომენდირებულია 3 მწმ-იანი გადამოწმების ბიუჯეტი გამეორებების 100%-ით ქუდის შიგნით.【artifacts/da/proof_bench/benchmark.md:1】
- მაგალითი (წარმოქმნის დეტერმინისტულ დატვირთვას და წერს ორივე მოხსენებას):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

ბლოკის შეკრება ახორციელებს იგივე ბიუჯეტებს: `sumeragi.da_max_commitments_per_block` და
`sumeragi.da_max_proof_openings_per_block` აკარებს DA პაკეტს ბლოკში ჩასვლამდე და
თითოეულ ვალდებულებას უნდა ჰქონდეს არანულოვანი `proof_digest`. მცველი შეკვრის სიგრძეს განიხილავს, როგორც
მტკიცებულების გახსნის რაოდენობა მანამ, სანამ აშკარა მტკიცებულებათა შეჯამება არ გადაიჭრება კონსენსუსის გზით, დაიცავით
≤128-გახსნის სამიზნე შესასრულებელი ბლოკის საზღვარზე.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR წარუმატებლობის მართვა და შემცირებაშემნახველი მუშაკები ახლა აშუქებენ PoR უკმარისობის ზოლებს და შეკრული ხაზების რეკომენდაციებს თითოეულთან ერთად
განაჩენი. კონფიგურირებული დარტყმის ზღურბლზე ზევით თანმიმდევრული წარუმატებლობა იძლევა რეკომენდაციას, რომ
მოიცავს პროვაიდერს/მანიფესტის წყვილს, ზოლის სიგრძეს, რომელმაც გამოიწვია ხაზი და შემოთავაზებული
ჯარიმა გამოითვლება პროვაიდერის ობლიგაციიდან და `penalty_bond_bps`; გაგრილების ფანჯრები (წამები) შენარჩუნება
დუბლიკატი შტრიხების გასროლისგან იმავე ინციდენტზე.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343

- ზღვრების/გაგრილების კონფიგურაცია შენახვის მუშა შემქმნელის მეშვეობით (ნაგულისხმევი ასახავს მმართველობას
  საჯარიმო პოლიტიკა).
- Slash რეკომენდაციები ჩაწერილია ვერდიქტის შეჯამებაში JSON, რათა მმართველობას/აუდიტორებს შეეძლოთ დაურთოთ
  ისინი მტკიცებულებების პაკეტებისთვის.
- ზოლის განლაგება + თითო ნაწილზე როლები ახლა გადადის Torii-ის შენახვის პინის ბოლო წერტილში
  (`stripe_layout` + `chunk_roles` ველები) და გრძელდებოდა შენახვის მუშაკში ასე
  აუდიტორებს/შეკეთების ხელსაწყოებს შეუძლიათ დაგეგმონ მწკრივის/სვეტის შეკეთება განლაგების ხელახლა გამოტანის გარეშე

### განთავსება + შეკეთება აღკაზმულობა

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` ახლა
ითვლის განლაგების ჰეშს `(index, role, stripe/column, offsets)`-ზე და ასრულებს მწკრივს ჯერ შემდეგ
სვეტი RS(16) შეკეთება დატვირთვის რეკონსტრუქციამდე:

- განთავსება ნაგულისხმევად არის `total_stripes`/`shards_per_stripe`, როდესაც არსებობს და იშლება ნაწილებად
- დაკარგული/დაზიანებული ფრაგმენტები თავიდან აშენებულია მწკრივის პარიტეტით; დარჩენილი ხარვეზები გარემონტებულია
  ზოლის (სვეტის) პარიტეტი. გარემონტებული ნაწილაკები იწერება ბლოკის დირექტორიაში და JSON-ში
  რეზიუმე აღწერს განლაგების ჰეშს პლუს მწკრივების/სვეტების შეკეთების მრიცხველებს.
- თუ მწკრივი+სვეტის პარიტეტი ვერ დააკმაყოფილებს გამოტოვებულ კომპლექტს, აღკაზმულობა სწრაფად იშლება გამოუსწორებელთან ერთად
  ინდექსები, რათა აუდიტორებმა შეძლონ გამოუსწორებელი მანიფესტების მონიშვნა.