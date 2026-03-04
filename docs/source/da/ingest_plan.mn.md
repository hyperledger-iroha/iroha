---
lang: mn
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Өгөгдлийн хүртээмжийг хүлээн авах төлөвлөгөө

_Зохиогдсон: 2026-02-20 - Эзэмшигч: Үндсэн протоколын АХ / Хадгалах баг / ДА АХ_

DA-2 ажлын урсгал нь Norito ялгаруулдаг blob ingest API-ээр Torii-г өргөтгөдөг.
мета өгөгдөл ба үрийн SoraFS хуулбар. Энэхүү баримт бичиг нь санал болгож буй зүйлийг тусгасан болно
схем, API гадаргуу болон баталгаажуулалтын урсгалтай тул хэрэгжилтийг үргэлжлүүлэх боломжтой
гайхалтай симуляци дээр хаах (DA-1 дагалт). Бүх ачааллын формат ЗААВАЛ
Norito кодлогч ашиглах; ямар ч serde/JSON нөөцийг зөвшөөрөхгүй.

## Зорилго

- Том бөмбөлгийг хүлээн авах (Тайкай сегментүүд, эгнээний хажуугийн тэрэг, засаглалын олдворууд)
  тодорхой хэмжээгээр Torii.
- Blob, кодек параметрүүдийг тодорхойлсон каноник Norito манифестуудыг гаргах,
  устгах профайл, хадгалах бодлого.
- SoraFS халуун хадгалах болон хувилах ажлыг дараалалд оруулах мета өгөгдлийг хадгалах.
- SoraFS бүртгэл болон засаглалд пин зорилго + бодлогын шошго нийтлэх
  ажиглагчид.
- Үйлчлүүлэгчид нийтэлсэн тодорхой нотолгоог олж авахын тулд элсэлтийн баримтыг ил болго.

## API гадаргуу (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Ачаалал нь Norito кодлогдсон `DaIngestRequest` юм. Хариултуудыг ашиглах
`application/norito+v1` ба `DaIngestReceipt` буцаана.

| Хариулт | Утга |
| --- | --- |
| 202 Хүлээн зөвшөөрсөн | Бөмбөг хэсэглэх/хуулбарлахаар дараалалд орсон; баримтыг буцааж өгсөн. |
| 400 муу хүсэлт | Схем/хэмжээний зөрчил (баталгаажуулалтын шалгалтыг үзнэ үү). |
| 401 Зөвшөөрөгдөөгүй | API токен дутуу/хүчингүй. |
| 409 Зөрчил | Тохиромжгүй мета өгөгдөлтэй `client_blob_id` хуулбар. |
| 413 Ачаалал хэт том | Блобын тохируулсан уртын хязгаараас хэтэрсэн. |
| 429 Хэт олон хүсэлт | Үнийн хязгаарт хүрсэн. |
| 500 дотоод алдаа | Гэнэтийн алдаа (бүртгэгдсэн + анхааруулга). |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

Одоогийн эгнээний каталогоос авсан `DaProofPolicyBundle` хувилбарыг буцаана.
Багц нь `version` (одоогоор `1`), `policy_hash` (хэш
захиалгат бодлогын жагсаалт) болон `policies` оруулгуудыг агуулсан `lane_id`, `dataspace_id`,
`alias`, мөн `proof_scheme` (өнөөдөр `merkle_sha256`; KZG эгнээ
KZG амлалтууд бэлэн болтол хүлээн авахаас татгалзсан). Одоо блокийн толгой
`da_proof_policies_hash`-ээр дамжуулан багцыг хүлээн авдаг тул үйлчлүүлэгчид
DA-ийн амлалт эсвэл нотолгоог баталгаажуулах үед тогтоосон идэвхтэй бодлого. Энэ төгсгөлийн цэгийг татна уу
эгнээний бодлого болон урсгалтай нийцэж байгаа эсэхийг баталгаажуулахын тулд нотлох баримтуудыг барихаас өмнө
багц хэш. Амлалтын жагсаалт/баталгаажуулах эцсийн цэгүүд нь ижил багцыг агуулж байдаг тул SDK-ууд
Идэвхтэй бодлогын багцад нотлох баримтыг холбохын тулд нэмэлт хоёр талын аялал хийх шаардлагагүй.

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Захиалсан бодлогын жагсаалтыг агуулсан `DaProofPolicyBundle`-г буцаана
`policy_hash` тул SDK нь блок үйлдвэрлэхэд ашигласан хувилбарыг тогтоох боломжтой. The
хэшийг Norito кодлогдсон бодлогын массиваар тооцдог бөгөөд хэзээ ч өөрчлөгддөг.
lane-ийн `proof_scheme` шинэчлэгдсэн нь үйлчлүүлэгчид хоорондын шилжилтийг илрүүлэх боломжийг олгодог.
кэш баталгаа болон хэлхээний тохиргоо.

## Санал болгож буй Norito схем

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
```> Хэрэгжүүлэлтийн тэмдэглэл: эдгээр ачааллын стандарт зэвсгийн дүрслэл одоо доор ажиллаж байна
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest` дахь хүсэлт/баримтын боодолтой
> болон `iroha_data_model::da::manifest` дахь манифест бүтэц.

`compression` талбар нь залгагчид ачааг хэрхэн бэлдсэнийг сурталчилдаг. Torii зөвшөөрнө
`identity`, `gzip`, `deflate`, болон `zstd`, өмнөх байтуудыг ил тод задалдаг.
Нэмэлт манифестуудыг хэшлэх, бүлэглэх, шалгах.

### Баталгаажуулах хяналтын хуудас

1. Norito хүсэлт `DaIngestRequest`-тай таарч байгааг шалгана уу.
2. Хэрэв `total_size` нь каноник (дарагдсан) ачааллын уртаас ялгаатай эсвэл тохируулсан дээд хэмжээнээс хэтэрсэн тохиолдолд амжилтгүй болно.
3. `chunk_size` зэрэгцүүлэлтийг хэрэгжүүлэх (хоёрын хүчин чадал, = 2 гэдгийг баталгаажуулна уу.
5. `retention_policy.required_replica_count` нь засаглалын суурь үзүүлэлтийг хүндэтгэх ёстой.
6. Каноник хэшийн эсрэг гарын үсгийн баталгаажуулалт (гарын үсгийн талбараас бусад).
7. Ачааллын хэш + мета өгөгдөл ижил биш л бол `client_blob_id` давхардсанаас татгалз.
8. `norito_manifest` өгсөн үед схем + хэш таарч дахин тооцоолсон эсэхийг шалгана уу
   хэсэгчилсэн дараа илэрдэг; Үгүй бол зангилаа манифест үүсгэж хадгалдаг.
9. Тохируулсан хуулбарлах бодлогыг хэрэгжүүлэх: Torii илгээсэн зүйлийг дахин бичнэ.
   `RetentionPolicy`, `torii.da_ingest.replication_policy` (харна уу).
   `replication_policy.md`) ба хадгалалт нь урьдчилан бүтээгдсэн манифестаас татгалздаг.
   мета өгөгдөл нь хүчинтэй профайлтай таарахгүй байна.

### Учирчлах ба хуулбарлах урсгал1. `chunk_size`-д бөөн ачааллыг оруулаад, нэг хэсэг болгон BLAKE3 + Merkle root-ийг тооцоол.
2. Norito `DaManifestV1` (шинэ бүтэц)-ийг бүтээх (шинэ бүтэц) хэсэгчилсэн үүрэг хариуцлага (үүрэг/бүлгийн_id),
   бүдүүвчийг арилгах (мөр ба баганын паритын тоо нэмэх `ipa_commitment`), хадгалах бодлого,
   болон мета өгөгдөл.
3. `config.da_ingest.manifest_store_dir` доор каноник манифест байтыг дараалалд оруулна уу
   (Torii нь эгнээ/эрин үе/ дэс дараалал/тасалбар/хурууны хээгээр тэмдэглэгдсэн `manifest.encoded` файлуудыг бичдэг) тиймээс SoraFS
   Оркестр нь тэдгээрийг залгиж, хадгалах тасалбарыг тогтвортой өгөгдөлтэй холбож болно.
4. `sorafs_car::PinIntent`-ээр дамжуулан засаглалын шошго + бодлого бүхий пин санааг нийтлэх.
5. Ажиглагчдад (хөнгөн үйлчлүүлэгчид,
   засаглал, аналитик).
6. `DaIngestReceipt` (Torii DA үйлчилгээний түлхүүрээр гарын үсэг зурсан) буцааж,
   base64 Norito кодчилол агуулсан `Sora-PDP-Commitment` хариу толгой
   SDK-ууд түүврийн үрийг нэн даруй хадгалах боломжтой.
   Баримт нь одоо `rent_quote` (a `DaRentQuote`) болон `stripe_layout`-г оруулсан байна.
   Тиймээс илгээгчид XOR үүрэг, нөөц хувьцаа, PDP/PoTR урамшууллын хүлээлт,
   Мөнгө өгөхөөс өмнө хадгалах тасалбарын мета өгөгдлийн хажууд 2D матрицын хэмжээсийг арилгана.
7. Нэмэлт бүртгэлийн мета өгөгдөл:
   - `da.registry.alias` — нийтийн, шифрлэгдээгүй UTF-8 нэрийн стринг нь бүртгэлийн бичилтийг суулгахад зориулагдсан.
   - `da.registry.owner` — бүртгэлийн эзэмшлийг бүртгэх нийтийн, шифрлэгдээгүй `AccountId` мөр.
   Torii нь эдгээрийг үүсгэсэн `DaPinIntent` руу хуулдаг тул доод талын пин боловсруулалт нь бусад нэрийг холбох боломжтой
   түүхий мета өгөгдлийн газрын зургийг дахин задлан шинжлэхгүйгээр эзэмшигчид; алдаатай эсвэл хоосон утгуудын үед татгалзсан
   залгих баталгаажуулалт.

## Хадгалах / Бүртгэлийн шинэчлэлтүүд

- `sorafs_manifest`-г `DaManifestV1`-ээр өргөтгөж, детерминист задлан шинжлэхийг идэвхжүүлнэ.
- Хувилбарын ачааллын лавлагаа бүхий `da.pin_intent` бүртгэлийн шинэ урсгалыг нэмнэ үү
  манифест хэш + тасалбарын ID.
- Залгих хоцрогдол, хэсэгчилсэн дамжуулалтыг хянахын тулд ажиглалтын шугамыг шинэчлэх,
  хуулбарлалтын хоцрогдол, бүтэлгүйтлийн тоо.
- Torii `/status` хариултууд нь одоо хамгийн сүүлийн үеийн `taikai_ingest` массивыг агуулдаг.
  кодлогчоос залгих хоцрогдол, шууд ирмэгийн шилжилт, алдааны тоолуур (кластер, урсгал), DA-9-г идэвхжүүлдэг
  Prometheus-г хусахгүйгээр эрүүл мэндийн агшин зуурын агшингуудыг зангилаанаас шууд оруулах хяналтын самбар.

## Туршилтын стратеги- Схемийн баталгаажуулалт, гарын үсгийн шалгалт, давхардлыг илрүүлэх нэгжийн туршилтууд.
- `DaIngestRequest`-ийн Norito кодчилол, манифест, хүлээн авалтыг баталгаажуулах алтан тестүүд.
- Интеграцийн бэхэлгээ нь хуурамч SoraFS + регистрийг эргүүлж, бөөгнөрөл + зүү урсгалыг баталгаажуулдаг.
- Санамсаргүй устгах профайл болон хадгалалтын хослолыг хамарсан үл хөдлөх хөрөнгийн тест.
- Буруу хэлбэрийн мета өгөгдлөөс хамгаалахын тулд Norito ачааллыг бүдгэрүүлэх.
- Blob ангилал бүрийн алтан бэхэлгээний доор амьдардаг
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` дагалдах хэсэгтэй
  `fixtures/da/ingest/sample_chunk_records.txt` дээр жагсаасан. Үл тоомсорлосон тест
  `regenerate_da_ingest_fixtures` бэхэлгээг шинэчилж байхад
  `manifest_fixtures_cover_all_blob_classes` шинэ `BlobClass` хувилбар нэмэнгүүт бүтэлгүйтдэг.
  Norito/JSON багцыг шинэчлэхгүйгээр. Энэ нь DA-2 болгонд Torii, SDK болон баримт бичгүүдийг шударга байлгадаг.
  шинэ blob гадаргууг хүлээн авдаг.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI & SDK хэрэгсэл (DA-8)- `iroha app da submit` (шинэ CLI нэвтрэх цэг) нь одоо хуваалцсан ingest builder/publisher-ийг нэгтгэж, операторууд
  Taikai багцын урсгалаас гадна дур зоргоороо бөмбөрцөгийг залгих боломжтой. Тушаал нь амьдардаг
  `crates/iroha_cli/src/commands/da.rs:1` ба ачаалал, устгах/хадгалах профайл болон
  CLI-тай каноник `DaIngestRequest` гарын үсэг зурахаас өмнө нэмэлт мета өгөгдөл/манифест файлууд
  тохиргооны түлхүүр. Амжилттай гүйлтүүд `da_request.{norito,json}` болон `da_receipt.{norito,json}`-ийн дагуу үргэлжилсээр байна.
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir`-ээр дарах) тул олдворуудыг гаргах боломжтой
  залгих явцад ашигласан яг Norito байтыг тэмдэглэ.
- Энэ тушаал нь анхдагчаар `client_blob_id = blake3(payload)` боловч дамжуулан дарж бичихийг зөвшөөрдөг
  `--client-blob-id`, мета өгөгдлийн JSON газрын зураг (`--metadata-json`) болон урьдчилан үүсгэсэн манифестуудыг хүндэтгэдэг
  (`--manifest`) ба офлайнаар бэлтгэхэд `--no-submit` болон захиалгат `--endpoint`-г дэмждэг.
  Torii хостууд. Баримт JSON нь дискэнд бичигдэхээс гадна stdout дээр хэвлэгддэг
  DA-8 "submit_blob" хэрэгслийн шаардлага болон SDK паритетийн ажлыг блокоос гаргах.
- `iroha app da get` нь аль хэдийн ажиллаж байгаа олон эх сурвалжийн найруулагчийн хувьд DA-д чиглэсэн өөр нэрийг нэмдэг.
  `iroha app sorafs fetch`. Операторууд үүнийг манифест + хэсэгчилсэн төлөвлөгөөний олдворууд дээр зааж болно (`--manifest`,
  `--plan`, `--manifest-id`) **эсвэл** `--storage-ticket`-ээр дамжуулан Torii хадгалах тасалбарыг дамжуулаарай. Хэзээ
  тасалбарын замыг ашиглаж байна CLI нь `/v1/da/manifests/<ticket>`-аас манифест татаж, багцыг үргэлжлүүлнэ
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir`-р дарж бичих) доор **манифестийг гаргана
  `--manifest-id`-д зориулсан хэш**, дараа нь нийлүүлсэн `--gateway-provider`-ээр найруулагчийг ажиллуулна.
  жагсаалт. Ачааллын баталгаажуулалт нь гарцын ID нь байгаа үед суулгагдсан CAR/`blob_hash` дижест дээр тулгуурласан хэвээр байна.
  одоо манифест хэш тул үйлчлүүлэгчид болон баталгаажуулагч нар нэг лбб танигчийг хуваалцдаг. Бүх дэвшилтэт товчлуурууд
  SoraFS зөөвөрлөгчийн гадаргуу бүрэн бүтэн (манифест дугтуй, үйлчлүүлэгчийн шошго, хамгаалалтын кэш, нэрээ нууцлах тээвэрлэлт)
  хүчингүй болгох, онооны самбарын экспорт болон `--output` замууд) ба илэрхий төгсгөлийн цэгийг дараах байдлаар дарж болно.
  Захиалгат Torii хостуудад зориулсан `--manifest-endpoint`, тиймээс эцсийн хүртээмжтэй эсэхийг шалгах нь бүхэлдээ
  Оркестрийн логикийг хуулбарлахгүйгээр `da` нэрийн орон зай.
- `iroha app da get-blob` нь `GET /v1/da/manifests/{storage_ticket}`-ээр дамжуулан Torii-ээс каноник манифестуудыг шууд татдаг.
  Одоо команд нь олдворуудыг манифест хэш (блоб id) бичих шошготой болгож байна
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json`, `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/` (эсвэл хэрэглэгчийн нийлүүлсэн `--output-dir`) дор яг цуурайтаж байхад
  `iroha app da get` дуудлагыг (`--manifest-id`-г оруулаад) дараагийн найрал хөгжимчийг татахад шаардлагатай.
  Энэ нь операторуудыг манифест дамар лавлахаас хол байлгаж, зөөгчийг үргэлж ашиглахыг баталгаажуулдаг
  Torii-ийн ялгаруулсан гарын үсэгтэй олдворууд. JavaScript Torii клиент нь энэ урсгалыг дамжуулдаг
  Swift SDK одоо ил гарч байхад `ToriiClient.getDaManifest(storageTicketHex)`
  `ToriiClient.getDaManifestBundle(...)`. Хоёулаа код тайлагдсан Norito байт, манифест JSON, манифест хэш,SDK дуудагч нар CLI болон Swift-д орохгүйгээр найруулагчийн сессийг чийгшүүлэх боломжтой болохын тулд хэсэгчилсэн төлөвлөгөө
  Үйлчлүүлэгчид нэмэлт `fetchDaPayloadViaGateway(...)` руу залгаж эдгээр багцуудыг эх утсаар дамжуулах боломжтой.
  SoraFS найруулагчийн боодол.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v1/da/manifests` хариултууд одоо `manifest_hash` болон CLI + SDK туслах (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway`, болон Swift/JS гарцын боодол) нь энэхүү тоймыг
  суулгагдсан CAR/blob хэштэй харьцуулах ачааллыг үргэлжлүүлэн шалгахын зэрэгцээ каноник манифест танигч.
- `iroha app da rent-quote` нь нийлүүлсэн хадгалах багтаамжийн тодорхойлогдох түрээс болон урамшууллын задаргааг тооцдог.
  болон хадгалах цонх. Туслагч нь идэвхтэй `DaRentPolicyV1` (JSON эсвэл Norito байт) эсвэл ашигладаг.
  суулгасан өгөгдмөл нь бодлогыг баталгаажуулж, JSON хураангуйг хэвлэдэг (`gib`, `months`, бодлогын мета өгөгдөл,
  болон `DaRentQuote` талбарууд) ингэснээр аудиторууд засаглалын протокол дотор XOR-ийн төлбөрийг тодорхой дурдах боломжтой.
  тусгай скрипт бичих. Одоо тушаал нь JSON-ийн өмнө нэг мөрт `rent_quote ...` хураангуйг гаргадаг.
  ослын үед ишлэл үүсгэх үед консолын лог болон runbook-ийг скан хийхэд хялбар болгохын тулд ачаалал.
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (эсвэл өөр ямар ч замаар) нэвтрүүлэх
  дажгүй хэвлэсэн хураангуйг үргэлжлүүлэхийн тулд `--policy-label "governance ticket #..."`-г ашиглана уу.
  олдвор нь тодорхой санал / тохиргооны багцыг иш татах шаардлагатай; CLI нь захиалгат шошгыг тайрч, хоосон зайнаас татгалздаг
  нотлох баримтын багцад `policy_source` утгыг утга учиртай байлгах мөрүүд. Харна уу
  Дэд командын `crates/iroha_cli/src/commands/da.rs` ба `docs/source/da/rent_policy.md`
  бодлогын схемийн хувьд.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- Пин бүртгэлийн паритет одоо SDK-д хүрч байна: `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK нь `iroha app sorafs pin register`-ийн ашигладаг яг ачааллыг бүтээж, каноник стандартыг хэрэгжүүлдэг.
  chunker мета өгөгдөл, пин бодлого, нэрийн баталгаа, залгамжлагчийн мэдээлэл
  `/v1/sorafs/pin/register`. Энэ нь CI роботууд болон автоматжуулалтыг CLI руу нэвтрэхээс хамгаалдаг
  манифест бүртгэлийг бүртгэх ба туслагч нь TypeScript/README хамрах хүрээтэй тул DA-8
  Rust/Swift-тэй зэрэгцэн JS дээр "илгээх/авах/баталгаажуулах" хэрэгслийн паритет бүрэн хангагдсан.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:78】
- `iroha app da prove-availability` дээрх бүх зүйлийг холбодог: хадгалах тасалбар авч, татаж авдаг.
  каноник манифест багц нь олон эх сурвалжийн найруулагчийг (`iroha app sorafs fetch`) эсрэг ажиллуулдаг.
  нийлүүлсэн `--gateway-provider` жагсаалт, татаж авсан ачаалал + онооны самбарын доор хадгалагдана
  `artifacts/da/prove_availability_<timestamp>/` ба одоо байгаа PoR туслагчийг шууд дуудна
  (`iroha app da prove`) татаж авсан байтыг ашиглан. Операторууд оркестрын бариулыг өөрчлөх боломжтой
  (`--max-peers`, `--scoreboard-out`, манифестийн төгсгөлийн цэгийг дарах) болон нотлох дээж авагч
  (`--sample-count`, `--leaf-index`, `--sample-seed`) нэг команд нь олдворуудыг үүсгэдэг.
  DA-5/DA-9 аудитаар хүлээгдэж буй: ачааллын хуулбар, онооны самбарын нотлох баримт, JSON нотлох хураангуй.- `da_reconstruct` (DA-6-д шинэ) нь каноник манифестийг уншдаг, дээр нь уг багцаас ялгарах бөөгнөрөл лавлах
  (`chunk_{index:05}.bin` layout) хадгалах ба шалгах явцад ачааллыг тодорхой хэмжээгээр дахин угсардаг.
  Блэйк3 амлалт бүр. CLI нь `crates/sorafs_car/src/bin/da_reconstruct.rs`-ийн дагуу амьдардаг бөгөөд дараах байдлаар тээвэрлэгддэг
  SoraFS багаж хэрэгслийн багцын нэг хэсэг. Ердийн урсгал:
  1. `iroha app da get-blob --storage-ticket <ticket>` `manifest_<manifest_hash>.norito` болон хэсэгчилсэн төлөвлөгөөг татаж авах.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (эсвэл `iroha app da prove-availability`, доор авчрах олдворуудыг бичдэг
     `artifacts/da/prove_availability_<ts>/` ба `chunks/` директор доторх хэсэг болгон файлуудыг хадгалдаг).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Регрессийн бэхэлгээ нь `fixtures/da/reconstruct/rs_parity_v1/`-ийн дагуу ажиллаж, манифестийг бүрэн хэмжээгээр авдаг.
  мөн `tests::reconstructs_fixture_with_parity_chunks`-ийн ашигладаг бөөн матриц (өгөгдөл + паритет). Үүнийг ашиглан сэргээнэ үү

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  Угсралт нь дараахь зүйлийг ялгаруулдаг.

  - `manifest.{norito.hex,json}` — каноник `DaManifestV1` кодчилол.
  - `chunk_matrix.json` — док/туршилтын лавлагаанд зориулж эрэмбэлсэн индекс/офсет/урт/дижест/паритын мөрүүд.
  - `chunks/` — `chunk_{index:05}.bin` өгөгдөл болон паритын хэсгүүдийн аль алинд нь ашигтай ачааны зүсмэлүүд.
  - `payload.bin` — Паритетийг мэддэг морины туршилтад ашигладаг тодорхойлогч ачаалал.
  - `commitment_bundle.{json,norito.hex}` — `DaCommitmentBundle` дээж, баримт бичиг/тестийн тодорхойлогч KZG амлалттай.

  Морь нь алга болсон эсвэл таслагдсан хэсгүүдээс татгалзаж, Blake3 хэшийг `blob_hash`-тэй харьцуулж, эцсийн даацыг шалгана.
  мөн хураангуй JSON blob (ачааллын байт, бөөмийн тоо, хадгалах тасалбар) гаргадаг тул CI нь сэргээн босголтыг баталгаажуулах боломжтой
  нотлох баримт. Энэ нь операторууд болон QA-ийн тодорхойлогч сэргээн босгох хэрэгсэлд тавигдах DA-6 шаардлагыг хаадаг
  Захиалгат скриптийг холбохгүйгээр ажлуудыг дуудаж болно.

## TODO Шийдвэрийн хураангуй

Өмнө нь блоклосон бүх залгих TODO-г хэрэгжүүлж, баталгаажуулсан:- **Шахалтын зөвлөмж** — Torii нь дуудлага хийгчийн өгсөн шошгыг хүлээн авдаг (`identity`, `gzip`, `deflate`,
  `zstd`) ба баталгаажуулалтын өмнө ачааллыг хэвийн болгодог тул каноник манифест хэш нь дараахтай таарч байна.
  задалсан байт.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Зөвхөн засаглалын мета өгөгдлийн шифрлэлт** — Torii одоо засаглалын мета өгөгдлийг шифрлэдэг.
  тохируулсан ChaCha20-Poly1305 түлхүүр, таарахгүй шошгыг няцааж, хоёрыг тодорхой харуулж байна
  тохиргооны товчлуурууд (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) эргэлтийг тодорхой байлгахын тулд.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Их хэмжээний ачаалалтай дамжуулалт** — олон хэсгээс бүрдсэн дамжуулалтыг шууд дамжуулж байна. Үйлчлүүлэгчид детерминистик урсгал
  `client_blob_id`, Torii-р түлхүүрлэгдсэн `DaIngestChunk` дугтуйнууд нь зүсмэл бүрийг баталгаажуулж, үе шаттай болгодог
  `manifest_store_dir`-ийн дагуу, `is_last` туг буусны дараа манифестийг атомын аргаар дахин бүтээдэг.
  Нэг удаагийн дуудлагын байршуулалтад ажиглагддаг RAM-ийн огцом өсөлтийг арилгах.【crates/iroha_torii/src/da/ingest.rs:392】
- **Манифест хувилбар** — `DaManifestV1` нь тодорхой `version` талбартай бөгөөд Torii татгалздаг
  үл мэдэгдэх хувилбарууд нь шинэ манифестын бүдүүвчийг илгээх үед тодорхойлогч шинэчлэлтүүдийг баталгаажуулдаг.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR дэгээ** — PDP амлалтууд нь бөөгнөрсөн дэлгүүрээс шууд гардаг бөгөөд хадгалагдах болно.
  манифестийн хажуугаар DA-5 хуваарь гаргагчид каноник өгөгдлөөс түүвэрлэлтийн сорилтуудыг эхлүүлэх боломжтой; нь
  `Sora-PDP-Commitment` толгой нь одоо `/v1/da/ingest` болон `/v1/da/manifests/{ticket}` хоёрын аль алинд нь нийлүүлэгддэг.
  Хариултууд нь SDK-ууд ирээдүйн судалгаанд лавлах гарын үсэг зурсан амлалтыг даруй мэдэж авдаг.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da:476】rs.
- **Шард курсорын журнал** — эгнээний мета өгөгдөл нь `da_shard_id` (өгөгдмөл нь `lane_id`)-г зааж өгч болно.
  Sumeragi одоо `(shard_id, lane_id)` тутамд хамгийн өндөр `(epoch, sequence)` хэвээр байна
  `da-shard-cursors.norito` DA дамартай зэрэгцэн дахин эхлүүлсэн тул дахин хэсэгчилсэн/тодорхойгүй эгнээ буулгаж, хадгална.
  детерминистик давталт. Санах ойн хэлтэрхий курсорын индекс одоо амлалтдаа хурдан бүтэлгүйтдэг
  эгнээний id-д өгөгдмөл болгохын оронд зураглаагүй эгнээнүүд, курсорыг ахиулах, дахин тоглуулах алдаа гаргах
  тодорхой, блок баталгаажуулалт нь зориулалтын тусламжтайгаар хэлтэрхий курсорын регрессээс татгалздаг
  `DaShardCursorViolation` шалтгаан + операторуудад зориулсан телеметрийн шошго. Одоо эхлүүлэх/гүйцүүлэх нь DA-г зогсоож байна
  Кура нь үл мэдэгдэх эгнээ эсвэл регрессийн курсорыг агуулж, зөрчлийг тэмдэглэсэн бол чийгшүүлэх индекс
  блокийн өндөр нь операторууд DA-д үйлчлэхээс өмнө засч залруулах боломжтой төлөв.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Хэсэг курсорын хоцрогдлын телеметр** — `da_shard_cursor_lag_blocks{lane,shard}` хэмжигч хэрхэн болохыг мэдээлдэгхол хэлтэрхий баталгаажуулж буй өндрийг дагана. Алга болсон/хуучирсан/тодорхойгүй эгнээ нь хоцролтыг тохируулсан
  шаардлагатай өндөр (эсвэл гурвалжин) ба амжилттай ахиц дэвшил нь үүнийг тэг болгож дахин тохируулснаар тогтворжсон төлөв жигд хэвээр байна.
  Операторууд тэггүй хоцрогдолд дохио өгөх, DA дамар/журнал дээр зөрчил гаргасан эгнээ байгаа эсэхийг шалгах,
  мөн блокийг арилгахын тулд дахин тоглуулахын өмнө эгнээний каталогийг санамсаргүйгээр дахин хуваах эсэхийг шалгана уу
  цоорхой.
- **Нууц тооцооны эгнээ** — тэмдэглэгдсэн эгнээ
  `metadata.confidential_compute=true` болон `confidential_key_version`-г дараах байдлаар авч үзнэ.
  SMPC/шифрлэгдсэн DA замууд: Sumeragi нь тэгээс өөр ачаалал/манифест дижест болон хадгалах тасалбаруудыг хэрэгжүүлдэг.
  бүрэн хуулбар хадгалах профайлаас татгалзаж, SoraFS тасалбар + бодлогын хувилбарыг индексжүүлдэггүй.
  ачааны байтыг ил гаргаж байна. Дахин тоглуулах явцад Курагийн хүлээн авалт нь чийгшүүлдэг тул баталгаажуулагчид мөн адил сэргээдэг
  нууцлалын мета өгөгдлийн дараа дахин эхлүүлнэ.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_rs】/stat.

## Хэрэгжүүлэх тэмдэглэл- Torii-ийн `/v1/da/ingest` төгсгөлийн цэг нь ачааллын шахалтыг хэвийн болгож, дахин тоглуулах кэшийг ажиллуулж,
  Каноник байтуудыг тодорхой хэмжээгээр хувааж, `DaManifestV1`-г дахин бүтээж, кодлогдсон ачааллыг бууруулдаг
  Баримт бичгийг гаргахаас өмнө SoraFS найруулгын `config.da_ingest.manifest_store_dir` руу; нь
  зохицуулагч нь мөн `Sora-PDP-Commitment` толгойг хавсаргасан тул үйлчлүүлэгчид кодлогдсон амлалтыг авах боломжтой.
  нэн даруй.【crates/iroha_torii/src/da/ingest.rs:220】
- Каноник `DaCommitmentRecord` хэвээр байсны дараа Torii одоо ялгардаг.
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` файл манифест дамарын хажууд байна.
  Оруулга бүр нь бичлэгийг түүхий Norito `PdpCommitment` байтаар багцалдаг тул DA-3 багц бүтээгчид болон
  DA-5 хуваарь гаргагчид манифестуудыг дахин унших эсвэл бөөгнөрөл хадгалахгүйгээр ижил оролтуудыг залгидаг.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK туслахууд нь PDP толгойн байтыг үйлчлүүлэгч бүрийг Norito задлан шинжилгээнд дахин оруулахыг албадахгүйгээр ил гаргадаг:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` хамгаалах Rust, Python `ToriiClient`
  одоо `decode_pdp_commitment_header` экспортлож, `IrohaSwift` нь маш хөдөлгөөнт төхөөрөмжид тохирох туслахуудыг илгээдэг.
  үйлчлүүлэгчид кодлогдсон түүвэрлэлтийн хуваарийг нэн даруй хадгалах боломжтой.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】:
- Torii нь мөн `GET /v1/da/manifests/{storage_ticket}`-г ил гаргадаг тул SDK болон операторууд манифестуудыг татаж авах боломжтой.
  зангилааны дамар лавлахад хүрэлгүйгээр төлөвлөгөөг хуваах. Хариулт нь Norito байтыг буцаана
  (base64), үзүүлсэн манифест JSON, `chunk_plan` JSON blob `sorafs fetch`-д бэлэн, дээр нь холбогдох
  hex digests (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) тул доод урсгалын хэрэгслүүд
  Дахин тооцоолохгүйгээр найруулагчийг тэжээж, ижил `Sora-PDP-Commitment` толгойг гаргана.
  залгих хариуг толин тусгал. `block_hash=<hex>`-г асуулгын параметр болгон дамжуулах нь детерминистикийг буцаана
  `sampling_plan` `block_hash || client_blob_id` (баталгаажуулагчид хуваалцсан) дээр үндэслэсэн
  `assignment_hash`, хүссэн `sample_window`, түүвэрлэсэн `(index, role, group)` хүрээ
  PoR дээж авагчид болон баталгаажуулагчид ижил индексүүдийг дахин тоглуулах боломжтой тул 2D зураасын бүтцийг бүхэлд нь харуулна. Дээж авагч
  `client_blob_id`, `chunk_root`, `ipa_commitment`-г даалгаврын хэш болгон холино; `iroha програмыг авах
  --block-hash ` now writes `sampling_plan_.json` manifest + chunk plan-ын хажууд
  хэш хадгалагдан үлдсэн бөгөөд JS/Swift Torii үйлчлүүлэгчид ижил `assignment_hash_hex`-ийг илрүүлдэг тул баталгаажуулагч
  болон судлаачид нэг детерминист шалгалтын багцыг хуваалцдаг. Torii түүврийн төлөвлөгөөг буцаах үед `iroha app da
  нотлох боломжтой` now reuses that deterministic probe set (seed derived from `sample_seed`) оронд нь
  түр зуурын дээж авахын тулд PoR гэрчүүд оператор нь орхигдуулсан байсан ч баталгаажуулагчийн даалгаврыг дагаж мөрддөг.
  `--block-hash` хүчингүй болгох.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Ачаа ихтэй урсгалын урсгалНэг хүсэлтийн тохируулсан хязгаараас их хэмжээний хөрөнгийг залгих шаардлагатай үйлчлүүлэгчид
`POST /v1/da/ingest/chunk/start` руу залгаж цацах сесс. Torii a гэж хариулна
`ChunkSessionId` (BLAKE3-хүссэн blob мета өгөгдлөөс үүсэлтэй) болон тохиролцсон бөөмийн хэмжээ.
Дараагийн `DaIngestChunk` хүсэлт бүрд:

- `client_blob_id` — эцсийн `DaIngestRequest`-тэй ижил.
- `chunk_session_id` — зүсмэлүүдийг ажиллаж байгаа сесстэй холбоно.
- `chunk_index` ба `offset` — детерминист дарааллыг хэрэгжүүлдэг.
- `payload` — тохиролцсон хэмжээ хүртэл.
- `payload_hash` — Зүсмэлийн BLAKE3 хэш нь Torii нь бүхэл бүтэн блобыг буферлэхгүйгээр баталгаажуулах боломжтой.
- `is_last` — терминалын зүсмэлийг заана.

Torii нь `config.da_ingest.manifest_store_dir/chunks/<session>/`-ийн дагуу баталгаажуулсан зүсмэлүүдийг хадгалах ба
чадваргүй байдлыг хүндэтгэхийн тулд дахин тоглуулах кэш доторх явцыг бүртгэдэг. Эцсийн зүсмэл газардах үед Torii
дискэн дээрх ачааллыг дахин угсардаг (санах ойн өсөлтөөс зайлсхийхийн тулд бөөгнөрсөн лавлахаар дамжуулдаг),
Каноник манифест/баримтыг нэг удаагийн байршуулалттай яг адилхан тооцоолж, эцэст нь хариу өгнө
Үе шаттай олдворыг хэрэглэснээр `POST /v1/da/ingest`. Амжилтгүй болсон сессийг тодорхой цуцалж болно
`config.da_ingest.replay_cache_ttl` дараа хог цуглуулдаг. Энэ загвар нь сүлжээний форматыг хадгалдаг
Norito-д ээлтэй, үйлчлүүлэгчид зориулсан дахин сэргээх протоколоос зайлсхийж, одоо байгаа манифест дамжуулах хоолойг дахин ашигладаг
өөрчлөгдөөгүй.

**Хэрэгжүүлэлтийн байдал.** Каноник Norito төрлүүд одоо амьдарч байна.
`crates/iroha_data_model/src/da/`:

- `ingest.rs` нь `DaIngestRequest`/`DaIngestReceipt`-г тодорхойлдог.
  Torii ашигладаг `ExtraMetadata` сав.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` нь дараа нь Torii ялгаруулдаг `DaManifestV1` ба `ChunkCommitment` хостуудыг
  хэсэглэх ажил дууслаа.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` хуваалцсан нэрээр хангадаг (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` гэх мэт) ба доор бичигдсэн бодлогын өгөгдмөл утгуудыг кодлодог.【crates/iroha_data_model/src/da/types.rs:240】
- Манифестын дамар файлууд `config.da_ingest.manifest_store_dir` дээр бууж, SoraFS зохион байгуулалтад бэлэн байна
  ажиглагчийг хадгалах сан руу татах.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi нь DA багцуудыг битүүмжлэх эсвэл баталгаажуулах үед манифест бэлэн байдлыг баталгаажуулдаг:
  Хэрэв дамард манифест байхгүй эсвэл хэш ялгаатай байвал блокууд баталгаажуулалтад амжилтгүй болно
  амлалтаас.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

Хүсэлт, манифест, төлбөрийн баримтын ачааллын хоёр талын хамрах хүрээг хянадаг
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito кодлогчийг баталгаажуулдаг
шинэчлэлтүүдийн туршид тогтвортой хэвээр байна.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Хэдийгээр хадгалалтын үлдэгдлүүд.** Засаглал энэ хугацаанд анхны хадгалах бодлогыг соёрхон баталсан
SF-6; `RetentionPolicy::default()`-ийн хэрэгжүүлсэн өгөгдмөл нь:- халуун шат: 7 хоног (`604_800` секунд)
- хүйтэн түвшин: 90 хоног (`7_776_000` секунд)
- шаардлагатай хуулбар: `3`
- хадгалах ангилал: `StorageClass::Hot`
- засаглалын шошго: `"da.default"`

Урсгалын урсгалын операторууд эгнээ нэвтрүүлэх үед эдгээр утгыг тодорхой зааж өгөх ёстой
илүү хатуу шаардлага.

## Зэвэрсэн үйлчлүүлэгчийн олдворууд

Rust клиентийг суулгасан SDK-ууд нь CLI руу шилжих шаардлагагүй болсон
каноник PoR JSON багцыг үйлдвэрлэх. `Client` нь хоёр туслахыг харуулж байна:

- `build_da_proof_artifact` үүсгэсэн яг бүтцийг буцаана
  `iroha app da prove --json-out`, өгөгдсөн манифест/ачааллын тэмдэглэгээг оруулаад
  [`DaProofArtifactMetadata`]-ээр дамжуулан.【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` бүтээгчийг боож, олдворыг дискэнд хадгална
  (хөөрхөн JSON + анхдагчаар арын шинэ мөр) тул автоматжуулалт нь файлыг хавсаргах боломжтой
  гаргах эсвэл засаглалын нотлох баримтыг багцлах.【crates/iroha/src/client.rs:3653】

### Жишээ

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

Туслагчийг орхиж буй JSON ачаалал нь CLI-д талбарын нэр хүртэл таарч байна
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest` гэх мэт), одоо байгаа
автоматжуулалт нь форматын салбаргүйгээр файлыг ялгах/паркет/байршуулах боломжтой.

## Баталгаажуулалтын жишиг

Өмнө нь төлөөлөх ачаалал дээр баталгаажуулагчийн төсвийг баталгаажуулахын тулд DA нотлох жишиг утсыг ашиглана уу
блок түвшний тагийг чангалах:

- `cargo xtask da-proof-bench` нь манифест/ачааллын хосын багцын дэлгүүрийг дахин бүтээж, PoR дээж авдаг
  орхиж, тохируулсан төсөвтэй харьцуулсан хугацааны баталгаажуулалт. Taikai мета өгөгдлийг автоматаар дүүргэх ба
  Хэрэв бэхэлгээний хос таарахгүй байвал оосор нь синтетик манифест руу буцдаг. Хэзээ `--payload-bytes`
  тодорхой `--payload`гүйгээр тохируулагдсан бөгөөд үүсгэсэн blob нь дараах руу бичигдсэн байна.
  `artifacts/da/proof_bench/payload.bin` тул эд хогшил хөндөгдөөгүй.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Тайлангууд нь анхдагч `artifacts/da/proof_bench/benchmark.{json,md}` бөгөөд нотлох баримт/гүйлт, нийт болон
  Баталгаажуулах хугацаа, төсвийн нэвтрүүлэх хувь, санал болгож буй төсөв (хамгийн удаан давталтын 110%)
  `zk.halo2.verifier_budget_ms`.【artifacts/da/proof_bench/benchmark.md:1】
- Хамгийн сүүлийн үеийн гүйлт (синтетик 1 МБ ачаалал, 64 КиБ хэсэг, 32 баталгаа/гүйлт, 10 давталт, 250 мс төсөв)
  таг дотор 100% давталттай 3 ms баталгаажуулагч төсвийг санал болгосон.【artifacts/da/proof_bench/benchmark.md:1】
- Жишээ (детерминист ачааллыг үүсгэж, хоёр тайланг бичнэ):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

Блок угсралт нь ижил төсвийг хэрэгжүүлдэг: `sumeragi.da_max_commitments_per_block` болон
`sumeragi.da_max_proof_openings_per_block` DA багцыг блок дотор оруулахаас өмнө хаах ба
амлалт бүр нь тэгээс өөр `proof_digest` байх ёстой. Хамгаалагч багцын уртыг
тодорхой нотлох хураангуйг зөвшилцөлд хүрэх хүртэл нотолгоо нээх тоолох, хадгалах
≤128-блокны хил дээр хэрэгжих боломжтой нээх зорилт.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR алдаатай харьцах, таслахХадгалах ажилчид одоо PoR-ийн дутагдлын зураас, налуу зураастай зөвлөмжийг тус бүрээр нь гаргаж өгдөг
шийдвэр. Тохируулсан цохилтын босгыг давсан дараалсан алдаа нь дараах зөвлөмжийг гаргадаг
үйлчилгээ үзүүлэгч/манифест хос, ташуу зураасыг үүсгэсэн зураасын урт болон санал болгож буй
үйлчилгээ үзүүлэгчийн бонд болон `penalty_bond_bps`-ээс тооцсон торгууль; хөргөх цонх (секунд) хадгална
ижил үйл явдал дээр буудсан давхар налуу зураас.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs】34:

- Хадгалах ажилтан бүтээгчээр дамжуулан босго/хөргөх хугацааг тохируулах (өгөгдмөл нь засаглалыг тусгадаг)
  торгуулийн бодлого).
- Шийдэх зөвлөмжийг JSON-ийн шийдвэрийн хураангуйд тэмдэглэсэн тул засаглал/аудиторууд хавсаргах боломжтой
  тэдгээрийг нотлох баримтын багц болгон.
- Зураасан байрлал + хэсэг тус бүрийн үүргүүдийг одоо Torii-ийн хадгалах пин төгсгөлийн цэгээр дамжуулсан
  (`stripe_layout` + `chunk_roles` талбарууд) мөн хадгалалтын ажилтан руу үргэлжлүүлэв.
  аудиторууд/засварын хэрэгсэл нь мөр/баганын засварыг дээд урсгалаас дахин гаргахгүйгээр төлөвлөх боломжтой.

### Байрлуулах + засварын оосор

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` одоо
`(index, role, stripe/column, offsets)` дээр байршуулах хэшийг тооцоолж, дараа нь эхлээд мөрийг гүйцэтгэнэ
RS(16) баганын даацыг сэргээн засварлахаас өмнө:

- Байршуулах өгөгдмөл нь байгаа үед `total_stripes`/`shards_per_stripe` болж, хэсэг рүү буцдаг.
- Алга болсон/эвдэрсэн хэсгүүдийг эхлээд мөрийн тэгшитгэлээр сэргээнэ; үлдсэн цоорхойг ашиглан засна
  судал (багана) паритет. Зассан хэсгүүдийг chunk лавлах болон JSON руу буцааж бичдэг
  хураангуй байршуулалтын хэш нэмэх мөр/баганын засварын тоолуурыг агуулна.
- Хэрэв мөр+баганын паритет дутуу олонлогийг хангаж чадахгүй бол оосор нь нөхөгдөөгүй бол хурдан эвдрэх болно.
  аудиторууд нөхөж баршгүй манифестуудыг тэмдэглэж болохын тулд индексүүд.