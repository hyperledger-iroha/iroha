---
lang: mn
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 710286691d09a5707829a36ca98ed24a6af5c5629e708dd7b1bd0f01db4e31c1
source_last_modified: "2026-01-22T14:35:36.737834+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

гарчиг: Өгөгдлийн хүртээмжийг хүлээн авах төлөвлөгөө
sidebar_label: Inngest Plan
тайлбар: Torii blob залгихад зориулсан схем, API гадаргуу болон баталгаажуулалтын төлөвлөгөө.
---

::: Каноник эх сурвалжийг анхаарна уу
:::

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
POST /v2/da/ingest
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
```

> Хэрэгжүүлэлтийн тэмдэглэл: эдгээр ачааллын стандарт зэвсгийн дүрслэл одоо доор ажиллаж байна
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest` дахь хүсэлт/баримтын боодолтой
> болон `iroha_data_model::da::manifest` дахь манифест бүтэц.

`compression` талбар нь залгагчид ачааг хэрхэн бэлдсэнийг сурталчилдаг. Torii зөвшөөрнө
`identity`, `gzip`, `deflate`, болон `zstd`, өмнөх байтуудыг ил тод задалдаг.
Нэмэлт манифестуудыг хэшлэх, бүлэглэх, шалгах.

### Баталгаажуулах хяналтын хуудас

1. Norito хүсэлт `DaIngestRequest`-тай таарч байгааг шалгана уу.
2. Хэрэв `total_size` нь каноник (шахагдсан) ачааллын уртаас ялгаатай эсвэл тохируулсан дээд хэмжээнээс хэтэрсэн тохиолдолд амжилтгүй болно.
3. `chunk_size` зэрэгцүүлэлтийг хэрэгжүүлэх (хоёрын хүчин чадал, <= 2 МБ).
4. `data_shards + parity_shards` <= дэлхийн максимум ба паритет >= 2-ыг баталгаажуулна уу.
5. `retention_policy.required_replica_count` нь засаглалын суурь үзүүлэлтийг хүндэтгэх ёстой.
6. Каноник хэшийн эсрэг гарын үсгийн баталгаажуулалт (гарын үсгийн талбараас бусад).
7. Ачааллын хэш + мета өгөгдөл ижил биш л бол `client_blob_id` давхардсанаас татгалз.
8. `norito_manifest` өгсөн үед схем + хэш таарч дахин тооцоолсон эсэхийг шалгана уу
   хэсэгчилсэн дараа илэрдэг; Үгүй бол зангилаа манифест үүсгэж хадгалдаг.
9. Тохируулсан хуулбарлах бодлогыг хэрэгжүүлэх: Torii илгээсэн зүйлийг дахин бичнэ.
   `RetentionPolicy`, `torii.da_ingest.replication_policy` (харна уу).
   `replication-policy.md`) ба хадгалагдах нь урьдчилан бүтээгдсэн манифестээс татгалздаг.
   мета өгөгдөл нь хүчинтэй профайлтай таарахгүй байна.

### Учирчлах ба хуулбарлах урсгал

1. `chunk_size` руу бөөн ачааллыг оруулаад, нэг хэсэг болгон BLAKE3 + Merkle root-ийг тооцоол.
2. Norito `DaManifestV1` (шинэ бүтэц)-ийг бүтээх (шинэ бүтэц) хэсэгчилсэн үүрэг хариуцлагыг (үүрэг/бүлгийн_id),
   бүдүүвчийг арилгах (мөр ба баганын паритын тоо нэмэх `ipa_commitment`), хадгалах бодлого,
   болон мета өгөгдөл.
3. `config.da_ingest.manifest_store_dir` доор каноник манифест байтыг дараалалд оруулна уу
   (Torii нь эгнээ/эрин үе/ дэс дараалал/тасалбар/хурууны хээгээр тэмдэглэгдсэн `manifest.encoded` файлуудыг бичдэг) тиймээс SoraFS
   Оркестр нь тэдгээрийг залгиж, хадгалах тасалбарыг тогтвортой өгөгдөлтэй холбож болно.
4. `sorafs_car::PinIntent`-ээр дамжуулан засаглалын шошго + бодлого бүхий пин санааг нийтлээрэй.
5. Ажиглагчдад (хөнгөн үйлчлүүлэгчид,
   засаглал, аналитик).
6. `DaIngestReceipt` утсыг залгагч руу буцаана (Torii DA үйлчилгээний түлхүүрээр гарын үсэг зурсан) болон
   `Sora-PDP-Commitment` толгой нь SDK-ууд кодлогдсон амлалтыг нэн даруй авах боломжтой. Баримт
   одоо `rent_quote` (a Norito `DaRentQuote`) болон `stripe_layout` багтаж, илгээгчид харуулахыг зөвшөөрнө.
   үндсэн түрээс, нөөцийн хувь, PDP/PoTR урамшууллын хүлээлт, 2D устгалын зохион байгуулалт зэрэг
   мөнгө хийхээс өмнө хадгалах тасалбар.

## Хадгалах / Бүртгэлийн шинэчлэлтүүд

- `sorafs_manifest`-г `DaManifestV1`-ээр өргөтгөж, детерминист задлан шинжлэхийг идэвхжүүлнэ.
- Хувилбарын ачааллын лавлагаа бүхий `da.pin_intent` бүртгэлийн шинэ урсгалыг нэмнэ үү
  манифест хэш + тасалбарын ID.
- Залгих хоцрогдол, хэсэгчилсэн дамжуулалтыг хянахын тулд ажиглалтын шугамыг шинэчлэх,
  хуулбарлалтын хоцрогдол, бүтэлгүйтлийн тоо.

## Туршилтын стратеги

- Схемийн баталгаажуулалт, гарын үсгийн шалгалт, давхардлыг илрүүлэх нэгжийн туршилтууд.
- Norito кодчилол `DaIngestRequest`, манифест, хүлээн авалтыг баталгаажуулах алтан тестүүд.
- Интеграцийн бэхэлгээ нь хуурамч SoraFS + регистрийг эргүүлж, хэсэг + зүү урсгалыг баталгаажуулдаг.
- Санамсаргүй устгах профайл болон хадгалалтын хослолыг хамарсан үл хөдлөх хөрөнгийн тест.
- Буруу хэлбэрийн мета өгөгдлөөс хамгаалахын тулд Norito ачааллыг бүдгэрүүлэх.

## CLI & SDK хэрэгсэл (DA-8)- `iroha app da submit` (шинэ CLI нэвтрэх цэг) нь одоо хуваалцсан ingest builder/publisher-ийг багцалж, операторууд
  Taikai багцын урсгалаас гадна дур зоргоороо бөмбөрцөгийг залгих боломжтой. Тушаал нь амьдардаг
  `crates/iroha_cli/src/commands/da.rs:1` ба ачаалал, устгах/хадгалах профайл болон
  CLI-тай каноник `DaIngestRequest` гарын үсэг зурахаас өмнө нэмэлт мета өгөгдөл/манифест файлууд
  тохиргооны түлхүүр. Амжилттай гүйлтүүд `da_request.{norito,json}` болон `da_receipt.{norito,json}`-ийн дагуу үргэлжилсээр байна.
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir`-ээр дамжуулан дарах) тул олдворуудыг гаргах боломжтой
  залгихад ашигласан яг Norito байтыг тэмдэглэ.
- Команд нь анхдагчаар `client_blob_id = blake3(payload)` боловч дамжуулан дарж бичихийг зөвшөөрдөг
  `--client-blob-id`, мета өгөгдлийн JSON газрын зураг (`--metadata-json`) болон урьдчилан үүсгэсэн манифестуудыг хүндэтгэдэг
  (`--manifest`) ба офлайнаар бэлтгэхэд `--no-submit` болон захиалгат `--endpoint`-г дэмждэг.
  Torii хостууд. Баримт JSON нь дискэнд бичигдэхээс гадна stdout дээр хэвлэгддэг
  DA-8 "submit_blob" хэрэгслийн шаардлага болон SDK паритетийн ажлыг блокоос гаргах.
- `iroha app da get` нь аль хэдийн ажиллаж байгаа олон эх сурвалжийн найруулагчийн хувьд DA-д чиглэсэн өөр нэрийг нэмдэг.
  `iroha app sorafs fetch`. Операторууд үүнийг манифест + хэсэгчилсэн төлөвлөгөөний олдворууд дээр зааж болно (`--manifest`,
  `--plan`, `--manifest-id`) **эсвэл** `--storage-ticket`-ээр дамжуулан Torii хадгалах тасалбарыг дамжуулна уу. Тасалбар хэзээ
  замыг ашиглаж байгаа бол CLI нь `/v2/da/manifests/<ticket>`-аас манифестийг татаж, доорх багцыг хадгалдаг.
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir`-р дарж бичих) нь blob хэшийг гаргаж авдаг
  `--manifest-id`, дараа нь нийлүүлсэн `--gateway-provider` жагсаалтаар найруулагчийг ажиллуулна. Бүгд
  SoraFS зөөгч гадаргуугийн дэвшилтэт товчлуурууд (манифест дугтуй, үйлчлүүлэгчийн шошго, хамгаалалтын кэш,
  нэрээ нууцлах тээвэрлэлтийг хүчингүй болгох, онооны самбарын экспорт, `--output` замууд) ба манифестийн төгсгөлийн цэг нь боломжтой.
  Захиалгат Torii хостуудад `--manifest-endpoint`-ээр дамжуулан дарж бичих тул бэлэн байдлыг шууд шалгаж болно.
  Оркестрийн логикийг хуулбарлахгүйгээр бүхэлд нь `da` нэрийн талбарт.
- `iroha app da get-blob` нь `GET /v2/da/manifests/{storage_ticket}`-ээр дамжуулан Torii-ээс каноник манифестуудыг шууд татдаг.
  Энэ тушаал нь `manifest_{ticket}.norito`, `manifest_{ticket}.json`, `chunk_plan_{ticket}.json` гэж бичдэг.
  `artifacts/da/fetch_<timestamp>/` (эсвэл хэрэглэгчийн нийлүүлсэн `--output-dir`) дор яг цуурайтаж байхад
  `iroha app da get` дуудлагыг (`--manifest-id`-г оруулаад) дараагийн найрал хөгжимчийг татахад шаардлагатай.
  Энэ нь операторуудыг манифест дамар лавлахаас хол байлгаж, зөөгчийг үргэлж ашиглахыг баталгаажуулдаг
  Torii-аас ялгарсан гарын үсэгтэй олдворууд. JavaScript Torii клиент нь энэ урсгалыг дамжуулдаг
  `ToriiClient.getDaManifest(storageTicketHex)`, код тайлагдсан Norito байтыг буцааж, JSON манифест,
  SDK дуудагчид CLI-д оруулахгүйгээр найруулагчийн сессийг чийгшүүлэх боломжтой.
  Swift SDK одоо ижил гадаргууг ил гаргаж байна (`ToriiClient.getDaManifestBundle(...)` plus)
  `fetchDaPayloadViaGateway(...)`), хоолойн багц нь SoraFS оркестрын ороосон цаас руу ордог.
  iOS-ийн үйлчлүүлэгчид манифестуудыг татаж авах, олон эх сурвалжаас татах, нотлох баримтуудыг авах боломжтой
  CLI-г дуудаж байна.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` нь нийлүүлсэн хадгалах багтаамжийн тодорхойлогдох түрээс болон урамшууллын задаргааг тооцдог.
  болон хадгалах цонх. Туслагч идэвхтэй `DaRentPolicyV1` (JSON эсвэл Norito байт) эсвэл
  суулгасан өгөгдмөл нь бодлогыг баталгаажуулж, JSON хураангуйг хэвлэдэг (`gib`, `months`, бодлогын мета өгөгдөл,
  болон `DaRentQuote` талбарууд) ингэснээр аудиторууд засаглалын протокол дотор XOR-ийн хураамжийг тодорхой дурдах боломжтой.
  тусгай скрипт бичих. Энэ тушаал нь мөн JSON-ийн өмнө нэг мөрт `rent_quote ...` хураангуйг гаргадаг.
  ослын дасгал сургуулилтын үед консолын бүртгэлийг унших боломжтой байлгахын тулд ачаалал. `--quote-out artifacts/da/rent_quotes/<stamp>.json`-тай хослуул
  `--policy-label "governance ticket #..."` яг бодлогын санал хураалтаас иш татсан гоёмсог олдворуудыг хадгалахын тулд
  эсвэл тохиргооны багц; CLI нь захиалгат шошгыг тайрч, хоосон мөрүүдээс татгалздаг тул `policy_source` утгууд
  төрийн сангийн хяналтын самбар дээр ажиллах боломжтой хэвээр байна. Дэд командыг `crates/iroha_cli/src/commands/da.rs`-с харна уу
  болон бодлогын схемд зориулсан `docs/source/da/rent_policy.md`.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` дээрх бүх гинж: хадгалах тасалбар авч, татаж авдаг
  каноник манифест багц нь олон эх сурвалжийн найруулагчийг (`iroha app sorafs fetch`) эсрэг ажиллуулдаг.
  нийлүүлсэн `--gateway-provider` жагсаалт, татаж авсан ачаалал + онооны самбарын доор хадгалагдана
  `artifacts/da/prove_availability_<timestamp>/` ба одоо байгаа PoR туслагчийг шууд дуудна
  (`iroha app da prove`) татаж авсан байтыг ашиглан. Операторууд оркестрын бариулыг өөрчлөх боломжтой
  (`--max-peers`, `--scoreboard-out`, манифестийн төгсгөлийн цэгийг дарах) болон нотлох дээж авагч
  (`--sample-count`, `--leaf-index`, `--sample-seed`) нэг команд нь олдворуудыг үүсгэдэг.
  DA-5/DA-9 аудитаар хүлээгдэж буй: ачааллын хуулбар, онооны самбарын нотлох баримт, JSON нотлох хураангуй.

## TODO Шийдвэрийн хураангуй

Өмнө нь блоклосон бүх залгих TODO-г хэрэгжүүлж, баталгаажуулсан:

- **Шахалтын зөвлөмж** — Torii нь дуудлага хийгчийн өгсөн шошгыг хүлээн авдаг (`identity`, `gzip`, `deflate`,
  `zstd`) ба баталгаажуулалтын өмнө ачааллыг хэвийн болгодог тул каноник манифест хэш нь дараахтай таарч байна.
  задалсан байт.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Зөвхөн засаглалын мета өгөгдлийн шифрлэлт** — Torii одоо засаглалын мета өгөгдлийг шифрлэдэг.
  тохируулсан ChaCha20-Poly1305 түлхүүр, таарахгүй шошгыг няцааж, хоёрыг тодорхой харуулж байна
  тохиргооны товчлуурууд (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) эргэлтийг тодорхойлогч байлгах.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Их хэмжээний ачаалалтай дамжуулалт** — олон хэсгээс бүрдсэн дамжуулалтыг шууд дамжуулж байна. Үйлчлүүлэгчид детерминистик урсгал
  `client_blob_id`, Torii-р түлхүүрлэгдсэн `DaIngestChunk` дугтуйнууд нь зүсмэл бүрийг баталгаажуулж, үе шаттай болгодог
  `manifest_store_dir`-ийн дагуу, `is_last` туг буусны дараа манифестийг атомын аргаар дахин бүтээдэг.
  Нэг удаагийн дуудлагын байршуулалтад ажиглагддаг RAM-ийн огцом өсөлтийг арилгах.【crates/iroha_torii/src/da/ingest.rs:392】
- **Манифест хувилбар** — `DaManifestV1` нь тодорхой `version` талбартай бөгөөд Torii татгалздаг
  үл мэдэгдэх хувилбарууд нь шинэ манифестын бүдүүвчийг илгээх үед тодорхойлогч шинэчлэлтүүдийг баталгаажуулдаг.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR дэгээ** — PDP амлалтууд нь бөөгнөрсөн дэлгүүрээс шууд гардаг бөгөөд хадгалагдах болно.
  манифестуудаас гадна DA-5 хуваарь гаргагчид каноник өгөгдлөөс түүвэрлэлтийн сорилтуудыг эхлүүлж, мөн
  `/v2/da/ingest` дээр `/v2/da/manifests/{ticket}` одоо `Sora-PDP-Commitment` гарчигтай
  үндсэн64 Norito ачааг зөөвөрлөх тул SDK нь DA-5 датчикуудыг яг таг хадгалдаг. зорилтот.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## Хэрэгжүүлэх тэмдэглэл

- Torii-ийн `/v2/da/ingest` төгсгөлийн цэг нь ачааллын шахалтыг хэвийн болгож, дахин тоглуулах кэшийг ажиллуулж,
  Каноник байтуудыг тодорхой хэмжээгээр хувааж, `DaManifestV1`-г дахин бүтээж, кодлогдсон ачааллыг бууруулдаг
  `config.da_ingest.manifest_store_dir` руу SoraFS найруулгад зориулж `Sora-PDP-Commitment` нэмдэг
  толгой нь операторууд PDP хуваарьлагчдын лавлагаа өгөх амлалтыг авах болно.【crates/iroha_torii/src/da/ingest.rs:220】
- Хүлээн зөвшөөрөгдсөн blob бүр одоо `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito`-г үүсгэдэг
  Каноник `DaCommitmentRecord`-ийг түүхий эдтэй хамт багцалсан `manifest_store_dir`-ийн дагуу оруулга
  `PdpCommitmentV1` байт тул DA-3 багц бүтээгчид болон DA-5 хуваарьчид ижил оролтыг чийгшүүлэхгүйгээр
  дахин унших манифест эсвэл бөөгнөрөл.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK туслах API-ууд нь дуудагчдыг Norito код тайлахыг албадахгүйгээр PDP толгойн ачааллыг илрүүлдэг:
  Rust хайрцаг нь `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python-г экспортолдог
  `ToriiClient`-д одоо `decode_pdp_commitment_header`, `IrohaSwift` хөлөг багтана
  `decodePdpCommitmentHeader` түүхий толгойн газрын зураг эсвэл `HTTPURLResponse` хэт ачаалал тохиолдлууд.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii нь мөн `GET /v2/da/manifests/{storage_ticket}`-г ил гаргадаг тул SDK болон операторууд манифест татаж авах боломжтой.
  зангилааны дамар лавлахад хүрэлгүйгээр төлөвлөгөөг хуваах. Хариулт нь Norito байтыг буцаана
  (base64), manifest JSON, `chunk_plan` JSON blob `sorafs fetch`-д бэлэн, холбогдох
  hex задлах (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) ба
  `Sora-PDP-Commitment` гарчиг нь паритын хувьд хүлээн авах хариултаас. -д `block_hash=<hex>` нийлүүлж байна
  асуулгын мөр нь тодорхойлогч `sampling_plan` (даалгаврын хэш, `sample_window`, түүвэрлэсэн) буцаана
  `(index, role, group)` багцууд нь бүтэн 2D зохион байгуулалтыг хамардаг) тул баталгаажуулагч болон PoR хэрэгслүүд ижил зүйлийг зурдаг.
  индексүүд.

### Ачаа ихтэй урсгалын урсгал

Нэг хүсэлтийн тохируулсан хязгаараас их хэмжээний хөрөнгийг залгих шаардлагатай үйлчлүүлэгчид
`POST /v2/da/ingest/chunk/start` руу залгаж цацах сесс. Torii a гэж хариулна
`ChunkSessionId` (BLAKE3-хүссэн blob мета өгөгдлөөс үүсэлтэй) болон тохиролцсон бөөмийн хэмжээ.
Дараагийн `DaIngestChunk` хүсэлт бүрд:- `client_blob_id` — эцсийн `DaIngestRequest`-тэй ижил.
- `chunk_session_id` — зүсмэлүүдийг ажиллаж байгаа сесстэй холбоно.
- `chunk_index` ба `offset` — детерминист дарааллыг хэрэгжүүлдэг.
- `payload` — тохиролцсон хэмжээ хүртэл.
- `payload_hash` — Зүсмэлийн BLAKE3 хэш нь Torii нь бүхэл бүтэн блобыг буферлэхгүйгээр баталгаажуулах боломжтой.
- `is_last` — терминалын зүсмэлийг заана.

Torii нь `config.da_ingest.manifest_store_dir/chunks/<session>/` доор баталгаажуулсан зүсмэлүүдийг хадгалах ба
чадваргүй байдлыг хүндэтгэхийн тулд дахин тоглуулах кэш доторх явцыг бүртгэдэг. Эцсийн зүсмэл газардах үед Torii
дискэн дээрх ачааллыг дахин угсардаг (санах ойн өсөлтөөс зайлсхийхийн тулд бөөгнөрсөн лавлахаар дамжуулдаг),
Каноник манифест/баримтыг нэг удаагийн байршуулалттай яг адилхан тооцоолж, эцэст нь хариу өгнө
`POST /v2/da/ingest` үе шаттай олдворыг хэрэглэснээр. Амжилтгүй болсон сессийг тодорхой цуцалж болно
`config.da_ingest.replay_cache_ttl` дараа хог цуглуулдаг. Энэ загвар нь сүлжээний форматыг хадгалдаг
Norito-д ээлтэй, үйлчлүүлэгчид зориулсан дахин сэргээх протоколоос зайлсхийж, одоо байгаа манифест дамжуулах хоолойг дахин ашигладаг
өөрчлөгдөөгүй.

**Хэрэгжүүлэлтийн байдал.** Каноник Norito төрлүүд одоо амьдарч байна.
`crates/iroha_data_model/src/da/`:

- `ingest.rs` нь `DaIngestRequest`/`DaIngestReceipt`-г тодорхойлдог.
  Torii ашигладаг `ExtraMetadata` сав.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` нь дараа нь Torii ялгаруулдаг `DaManifestV1` болон `ChunkCommitment` хостуудыг
  хэсэглэх ажил дууслаа.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` хуваалцсан нэрээр хангадаг (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` гэх мэт) ба доор бичигдсэн бодлогын өгөгдмөл утгуудыг кодлодог.【crates/iroha_data_model/src/da/types.rs:240】
- Манифестын дамар файлууд `config.da_ingest.manifest_store_dir` дээр бууж, SoraFS зохион байгуулалтад бэлэн байна
  ажиглагчийг хадгалах сан руу татах.【crates/iroha_torii/src/da/ingest.rs:220】

Хүсэлт, манифест, төлбөрийн баримтын ачааллын хоёр талын хамрах хүрээг хянадаг
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito кодлогчийг баталгаажуулдаг
шинэчлэлтүүдийн туршид тогтвортой хэвээр байна.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Хэдийгээр хадгалалтын үлдэгдлүүд.** Засаглал энэ хугацаанд анхны хадгалах бодлогыг соёрхон баталсан
SF-6; `RetentionPolicy::default()`-ийн хэрэгжүүлсэн өгөгдмөл нь:

- халуун шат: 7 хоног (`604_800` секунд)
- хүйтэн түвшин: 90 хоног (`7_776_000` секунд)
- шаардлагатай хуулбар: `3`
- хадгалах ангилал: `StorageClass::Hot`
- засаглалын шошго: `"da.default"`

Урсгалын урсгалын операторууд эгнээ нэвтрүүлэх үед эдгээр утгыг тодорхой зааж өгөх ёстой
илүү хатуу шаардлага.