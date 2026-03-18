---
lang: az
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

başlıq: Məlumatların Əlçatımlılığının Qəbul Planı
sidebar_label: Qəbul Planı
təsvir: Torii blob qəbulu üçün sxem, API səthi və doğrulama planı.
---

:::Qeyd Kanonik Mənbə
:::

# Sora Nexus Məlumat Əlçatımlılığı Qəbul Planı

_Tərtib tarixi: 2026-02-20 - Sahib: Əsas Protokol WG / Saxlama Qrupu / DA WG_

DA-2 iş axını Norito yayan blob qəbulu API ilə Torii-i genişləndirir
metadata və toxumların SoraFS təkrarlanması. Bu sənəd təklif olunanı əhatə edir
sxem, API səthi və təsdiqləmə axını, beləliklə həyata keçirmə olmadan davam edə bilər
görkəmli simulyasiyaların bloklanması (DA-1 təqibləri). Bütün faydalı yük formatları MÜTLƏQDİR
Norito kodeklərindən istifadə edin; heç bir serde/JSON geri qaytarılmasına icazə verilmir.

## Məqsədlər

- Böyük ləkələri qəbul edin (Taikai seqmentləri, zolaqlı yan arabalar, idarəetmə artefaktları)
  deterministik olaraq Torii üzərində.
- Blob, kodek parametrlərini təsvir edən kanonik Norito manifestləri istehsal edin,
  profili silmək və saxlama siyasəti.
- SoraFS isti saxlama və növbəli təkrarlama işlərində parça metadatasını davam etdirin.
- SoraFS reyestrinə və idarəçiliyinə pin niyyətləri + siyasət teqlərini dərc edin
  müşahidəçilər.
- Qəbul qəbzlərini ifşa edin ki, müştərilər nəşrin deterministik sübutunu bərpa etsinlər.

## API Səthi (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Yük yükü Norito kodlu `DaIngestRequest`-dir. Cavablardan istifadə
`application/norito+v1` və `DaIngestReceipt` qaytarın.

| Cavab | Məna |
| --- | --- |
| 202 Qəbul | Blob parçalanma/replikasiya üçün növbəyə qoyuldu; qəbz qaytarıldı. |
| 400 Səhv İstək | Sxem/ölçü pozuntusu (təsdiqləmə yoxlamalarına baxın). |
| 401 İcazəsiz | Çatışmayan/etibarsız API nişanı. |
| 409 Münaqişə | Uyğun olmayan metadata ilə dublikat `client_blob_id`. |
| 413 Yük çox böyük | Konfiqurasiya edilmiş blob uzunluğu limitini aşır. |
| 429 Çox Çox Sorğu | Məzənnə limitinə çatdı. |
| 500 Daxili xəta | Gözlənilməz uğursuzluq (daxil edilmiş + xəbərdarlıq). |

## Təklif olunan Norito Sxemi

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

> İcra qeydi: bu faydalı yüklər üçün kanonik Rust təqdimatları indi altındadır
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest`-də sorğu/qəbz sarğıları ilə
> və `iroha_data_model::da::manifest`-də manifest strukturu.

`compression` sahəsi zəng edənlərin faydalı yükü necə hazırladığını elan edir. Torii qəbul edir
`identity`, `gzip`, `deflate` və `zstd`, əvvəl baytları şəffaf şəkildə açır
isteğe bağlı manifestlərin hashing, parçalanması və yoxlanması.

### Doğrulama Yoxlama Siyahısı

1. Sorğunun Norito başlığının `DaIngestRequest` uyğunluğunu yoxlayın.
2. `total_size` kanonik (sıxışdırılmış) faydalı yük uzunluğundan fərqlənirsə və ya konfiqurasiya edilmiş maks.
3. `chunk_size` uyğunlaşdırılmasını tətbiq edin (ikinin gücü, <= 2 MiB).
4. `data_shards + parity_shards` <= qlobal maksimum və paritet >= 2 olduğundan əmin olun.
5. `retention_policy.required_replica_count` idarəetmənin ilkin göstəricilərinə hörmət etməlidir.
6. Kanonik hash-ə qarşı imzanın yoxlanılması (imza sahəsi istisna olmaqla).
7. Faydalı yük hash + metadata eyni deyilsə, `client_blob_id` dublikatını rədd edin.
8. `norito_manifest` təmin edildikdə, yenidən hesablanmış şema + hash uyğunluqlarını yoxlayın
   parçalanmadan sonra aşkar; əks halda node manifest yaradır və onu saxlayır.
9. Konfiqurasiya edilmiş təkrarlama siyasətini tətbiq edin: Torii təqdim edilənləri yenidən yazır
   `RetentionPolicy`, `torii.da_ingest.replication_policy` ilə (bax.
   `replication-policy.md`) və saxlanması əvvəlcədən qurulmuş manifestləri rədd edir
   metadata məcburi profilə uyğun gəlmir.

### Parçalanma və Replikasiya axını

1. `chunk_size`-ə pay yükünü yığın, hər parça üçün BLAKE3 + Merkle kökünü hesablayın.
2. Yığma öhdəlikləri (rol/qrup_id) tutaraq Norito `DaManifestV1` (yeni struktur) qurun,
   düzeni silmək (sətir və sütun paritetləri üstəgəl `ipa_commitment`), saxlama siyasəti,
   və metadata.
3. `config.da_ingest.manifest_store_dir` altında kanonik manifest baytlarını növbəyə qoyun
   (Torii zolaq/epox/ardıcıllıq/bilet/barmaq izi ilə əsaslanan `manifest.encoded` fayllarını yazır) buna görə də SoraFS
   orkestr onları qəbul edə və saxlama biletini davamlı data ilə əlaqələndirə bilər.
4. İdarəetmə etiketi + siyasəti ilə `sorafs_car::PinIntent` vasitəsilə pin niyyətlərini dərc edin.
5. Müşahidəçiləri xəbərdar etmək üçün Norito hadisəsini buraxın (yüngül müştərilər,
   idarəetmə, analitika).
6. Zəng edənə `DaIngestReceipt` qaytarın (Torii DA xidmət açarı ilə imzalanmış) və
   `Sora-PDP-Commitment` başlığı beləliklə SDK-lar kodlaşdırılmış öhdəliyi dərhal ələ keçirə bilsin. Qəbz
   indi `rent_quote` (a Norito `DaRentQuote`) və `stripe_layout` daxildir, təqdim edənlərə göstərməyə imkan verir
   əsas icarə, ehtiyat pay, PDP/PoTR bonus gözləntiləri və 2D silmə planı
   pul köçürməzdən əvvəl saxlama bileti.

## Saxlama / Reyestr yeniləmələri

- `sorafs_manifest`-i `DaManifestV1` ilə genişləndirin, deterministik təhlilə imkan verin.
- Versiyalaşdırılmış faydalı yük istinadı ilə yeni reyestr axını `da.pin_intent` əlavə edin
  manifest hash + bilet id.
- Müşahidə oluna bilən boru kəmərlərinin qəbulu gecikməsini, ötürmə qabiliyyətini izləmək üçün yeniləyin,
  replikasiya geriliyi və uğursuzluqların sayı.

## Test Strategiyası

- Sxemlərin yoxlanılması, imzaların yoxlanılması, dublikatın aşkarlanması üçün vahid testləri.
- `DaIngestRequest`, manifest və qəbzin Norito kodlamasını təsdiqləyən qızıl testlər.
- SoraFS + reyestrini fırladan inteqrasiya qoşqu, yığın + pin axınlarını təsdiqləyir.
- Təsadüfi silmə profillərini və saxlama birləşmələrini əhatə edən əmlak testləri.
- Səhv formalaşdırılmış metadatadan qorunmaq üçün Norito faydalı yüklərinin puslanması.

## CLI və SDK Alətləri (DA-8)- `iroha app da submit` (yeni CLI giriş nöqtəsi) indi operatorlar üçün paylaşılan ingest builder/naşiri əhatə edir
  Taikai paket axınından kənar ixtiyari blobları qəbul edə bilər. Komandanlıq yaşayır
  `crates/iroha_cli/src/commands/da.rs:1` və faydalı yük, silmə/saxlama profilini istehlak edir və
  CLI ilə kanonik `DaIngestRequest` imzalamadan əvvəl isteğe bağlı metadata/manifest faylları
  konfiqurasiya açarı. Uğurlu qaçışlar `da_request.{norito,json}` və `da_receipt.{norito,json}` altında davam edir
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` vasitəsilə ləğv edin) artefaktları buraxa bilər
  qəbul zamanı istifadə olunan dəqiq Norito baytlarını qeyd edin.
- Komanda defolt olaraq `client_blob_id = blake3(payload)`-ə uyğundur, lakin vasitəsilə ləğvetmələri qəbul edir
  `--client-blob-id`, metadata JSON xəritələrini (`--metadata-json`) və əvvəlcədən yaradılmış manifestləri qiymətləndirir
  (`--manifest`) və oflayn hazırlıq üçün `--no-submit` və xüsusi üçün `--endpoint` dəstəkləyir
  Torii hostları. Qəbz JSON, diskə yazılmaqla yanaşı, stdout-da çap olunur
  DA-8 “submit_blob” alət tələbi və SDK paritet işinin blokdan çıxarılması.
- `iroha app da get` artıq səlahiyyət verən çoxmənbəli orkestrator üçün DA-yönümlü ləqəb əlavə edir
  `iroha app sorafs fetch`. Operatorlar onu manifest + yığın plan artefaktlarında göstərə bilər (`--manifest`,
  `--plan`, `--manifest-id`) **və ya** `--storage-ticket` vasitəsilə Torii saxlama biletini keçin. Bilet olanda
  yol istifadə olunur, CLI manifestini `/v1/da/manifests/<ticket>`-dən çıxarır, altındakı paketi saxlayır
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir` ilə əvəz edir), üçün blob hash əldə edir
  `--manifest-id` və sonra orkestratoru təchiz edilmiş `--gateway-provider` siyahısı ilə işə salır. Hamısı
  SoraFS alıcı səthindən qabaqcıl düymələr (manifest zərfləri, müştəri etiketləri, qoruyucu keşlər,
  anonimlik nəqliyyatı ləğv edir, skorbord ixracı və `--output` yolları) və manifest son nöqtəsi ola bilər
  xüsusi Torii hostları üçün `--manifest-endpoint` vasitəsilə ləğv oluna bilər, beləliklə canlı olaraq başdan sona mövcudluğu yoxlayır
  orkestr məntiqini təkrarlamadan tamamilə `da` ad sahəsi altında.
- `iroha app da get-blob` `GET /v1/da/manifests/{storage_ticket}` vasitəsilə birbaşa Torii-dən kanonik manifestləri çıxarır.
  Komanda `manifest_{ticket}.norito`, `manifest_{ticket}.json` və `chunk_plan_{ticket}.json` yazır.
  `artifacts/da/fetch_<timestamp>/` altında (və ya istifadəçi tərəfindən təchiz edilmiş `--output-dir`) dəqiq əks-səda verir
  `iroha app da get` çağırışı (`--manifest-id` daxil olmaqla) təqib orkestrinin gətirilməsi üçün tələb olunur.
  Bu, operatorları manifest spool qovluqlarından kənarda saxlayır və alıcının həmişə istifadə etdiyinə zəmanət verir
  Torii tərəfindən yayılan imzalı artefaktlar. JavaScript Torii müştərisi bu axını vasitəsilə əks etdirir
  `ToriiClient.getDaManifest(storageTicketHex)`, deşifrə edilmiş Norito baytını qaytarır, JSON manifest,
  və SDK-ya zəng edənlərin CLI-yə müdaxilə etmədən orkestr seanslarını nəmləndirə bilməsi üçün yığın planı.
  Swift SDK indi eyni səthləri ifşa edir (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), boru kəmərləri yerli SoraFS orkestr sarğısına daxil olur.
  iOS müştəriləri manifestləri yükləyə, çoxmənbəli əldəetmələri yerinə yetirə və sübutları əldə edə bilər
  CLI-ni işə salır.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` təchiz edilmiş saxlama ölçüsü üçün deterministik icarə və həvəsləndirici qəzaları hesablayır
  və saxlama pəncərəsi. Köməkçi ya aktiv `DaRentPolicyV1` (JSON və ya Norito bayt) istehlak edir, ya da
  daxili defolt, siyasəti doğrulayır və JSON xülasəsini çap edir (`gib`, `months`, siyasət metadatası,
  və `DaRentQuote` sahələri) beləliklə, auditorlar idarəetmə protokolları daxilində dəqiq XOR ittihamlarına istinad edə bilərlər.
  ad hoc skriptlərin yazılması. Komanda JSON-dan əvvəl bir sətirli `rent_quote ...` xülasəsini də yayır.
  hadisə təlimləri zamanı konsol qeydlərini oxunaqlı saxlamaq üçün faydalı yük. `--quote-out artifacts/da/rent_quotes/<stamp>.json` ilə cütləşdirin
  `--policy-label "governance ticket #..."`, dəqiq siyasət səsverməsinə istinad edən gözəlləşdirilmiş artefaktları saxlamaq üçün
  və ya konfiqurasiya paketi; CLI xüsusi etiketi kəsir və boş sətirlərdən imtina edir, beləliklə `policy_source` dəyərləri
  xəzinədarlıq tablosunda fəaliyyət göstərə bilər. Alt komanda üçün `crates/iroha_cli/src/commands/da.rs`-ə baxın
  və siyasət sxemi üçün `docs/source/da/rent_policy.md`.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` yuxarıda göstərilənlərin hamısını zəncirləyir: saxlama bileti alır,
  kanonik manifest paketi, çoxmənbəli orkestratoru (`iroha app sorafs fetch`)
  təchiz edilmiş `--gateway-provider` siyahısı, yüklənmiş faydalı yükü + tablonun altında saxlayır
  `artifacts/da/prove_availability_<timestamp>/` və dərhal mövcud PoR köməkçisini işə salır
  (`iroha app da prove`) gətirilən baytlardan istifadə edir. Operatorlar orkestr düymələrini düzəldə bilərlər
  (`--max-peers`, `--scoreboard-out`, manifest son nöqtəni ləğv edir) və sübut nümunəsi
  (`--sample-count`, `--leaf-index`, `--sample-seed`) tək bir əmr artefaktlar yaradır
  DA-5/DA-9 auditləri tərəfindən gözlənilən: faydalı yük nüsxəsi, skorbord sübutu və JSON sübut xülasələri.

## TODO Qətnamə Xülasəsi

Əvvəllər bloklanmış bütün qəbul TODO-ları həyata keçirilib və təsdiqlənib:

- **Sıxılma göstərişləri** — Torii zəng edənin təqdim etdiyi etiketləri qəbul edir (`identity`, `gzip`, `deflate`,
  `zstd`) və doğrulamadan əvvəl faydalı yükləri normallaşdırır ki, kanonik manifest hash ilə uyğun olsun.
  sıxılmış baytlar.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Yalnız idarəetmə üçün metadata şifrələməsi** — Torii indi idarəetmə metadatasını şifrələyir
  konfiqurasiya edilmiş ChaCha20-Poly1305 açarı, uyğun olmayan etiketləri rədd edir və iki açıq-aydın görünür
  konfiqurasiya düymələri (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) fırlanma deterministik saxlamaq üçün.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Böyük yük axını** — çox hissəli qəbul canlıdır. Müştərilər deterministik axın
  `client_blob_id`, Torii ilə əsaslanan `DaIngestChunk` zərfləri hər bir dilimi doğrulayır, onları mərhələləşdirir
  `manifest_store_dir` altında və `is_last` bayrağı endikdən sonra manifesti atomik şəkildə yenidən qurur,
  tək zəngli yükləmələrdə görülən RAM artımlarının aradan qaldırılması.【crates/iroha_torii/src/da/ingest.rs:392】
- **Manifest versiyası** — `DaManifestV1` açıq `version` sahəsini daşıyır və Torii imtina edir
  yeni manifest tərtibatları göndərildikdə deterministik təkmilləşdirmələrə zəmanət verən naməlum versiyalar.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR qarmaqları** — PDP öhdəlikləri birbaşa yığın mağazasından alınır və davamlıdır
  manifestlərdən əlavə, beləliklə DA-5 planlaşdırıcıları kanonik məlumatlardan seçmə problemləri başlaya bilər və
  `/v1/da/ingest` plus `/v1/da/manifests/{ticket}` indi `Sora-PDP-Commitment` başlığını ehtiva edir
  base64 Norito faydalı yükü daşıyan SDK-lar dəqiq öhdəlik DA-5 zondlarını önbelleğe alır hədəf.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## İcra Qeydləri

- Torii-in `/v1/da/ingest` son nöqtəsi indi faydalı yükün sıxılmasını normallaşdırır, təkrar oynatma keşini tətbiq edir,
  kanonik baytları deterministik olaraq parçalayır, `DaManifestV1`-i yenidən qurur, kodlanmış yükü azaldır
  SoraFS orkestrasiyası üçün `config.da_ingest.manifest_store_dir`-ə daxil olur və `Sora-PDP-Commitment` əlavə edir
  başlıq beləliklə operatorlar PDP planlaşdırıcılarının istinad edəcəyi öhdəliyi götürsünlər.【crates/iroha_torii/src/da/ingest.rs:220】
- Hər qəbul edilmiş blob indi `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` istehsal edir
  kanonik `DaCommitmentRecord`-i xammal ilə birlikdə paketləyən `manifest_store_dir` altında giriş
  `PdpCommitmentV1` baytdır, beləliklə DA-3 paket qurucuları və DA-5 planlaşdırıcıları eyni girişləri nəmləndirir
  yenidən oxunan manifestlər və ya yığınlar.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK köməkçi API-ləri zəng edənləri Norito deşifrəsini yenidən həyata keçirməyə məcbur etmədən PDP başlığının faydalı yükünü ifşa edir:
  Rust qutusu `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python ixrac edir
  `ToriiClient` indi `decode_pdp_commitment_header` və `IrohaSwift` gəmilərini əhatə edir
  `decodePdpCommitmentHeader` xam başlıq xəritələri və ya `HTTPURLResponse` üçün həddindən artıq yükləmələr misallar.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii həmçinin `GET /v1/da/manifests/{storage_ticket}`-i ifşa edir ki, SDK və operatorlar manifestləri əldə edə bilsinlər
  və qovşağın makara kataloquna toxunmadan yığın planları. Cavab Norito baytını qaytarır
  (base64), göstərilən manifest JSON, `sorafs fetch` üçün hazır olan `chunk_plan` JSON blobu, müvafiq
  hex həzm edir (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) və əks etdirir
  Paritet üçün qəbul edilən cavablardan `Sora-PDP-Commitment` başlığı. `block_hash=<hex>` təchizatı
  sorğu sətri deterministik `sampling_plan` qaytarır (təyinat hash, `sample_window` və nümunə götürülmüşdür
  Tam 2D tərtibini əhatə edən `(index, role, group)` dəstləri) beləliklə, validatorlar və PoR alətləri eyni şeyi çəkir
  indekslər.

### Böyük faydalı yük axını axını

Konfiqurasiya edilmiş tək sorğu limitindən daha böyük aktivləri qəbul etməli olan müştərilər
`POST /v1/da/ingest/chunk/start`-ə zəng edərək axın sessiyası. Torii a ilə cavab verir
`ChunkSessionId` (BLAKE3-tələb olunan blob metadatasından əldə edilib) və razılaşdırılmış yığın ölçüsü.
Hər bir sonrakı `DaIngestChunk` sorğusu aşağıdakıları ehtiva edir:- `client_blob_id` — yekun `DaIngestRequest` ilə eynidir.
- `chunk_session_id` — dilimləri işləyən sessiyaya bağlayır.
- `chunk_index` və `offset` — deterministik sifarişi tətbiq edir.
- `payload` — razılaşdırılmış parça ölçüsünə qədər.
- `payload_hash` — dilimin BLAKE3 hashı beləliklə, Torii bütün blobu bufer etmədən doğrulaya bilsin.
- `is_last` — terminal dilimini göstərir.

Torii `config.da_ingest.manifest_store_dir/chunks/<session>/` altında təsdiqlənmiş dilimləri saxlayır və
qeyri-mümkünlüyünə hörmət etmək üçün təkrar oynatma keşində irəliləyişi qeyd edir. Son dilim yerə düşəndə Torii
diskdəki faydalı yükü yenidən yığır (yaddaş sıçrayışlarının qarşısını almaq üçün yığın kataloqu vasitəsilə axın),
kanonik manifest/qəbzi birdəfəlik yükləmələrdə olduğu kimi hesablayır və nəhayət cavab verir
`POST /v1/da/ingest` mərhələli artefaktı istehlak edərək. Uğursuz seanslar açıq şəkildə dayandırıla bilər və ya
`config.da_ingest.replay_cache_ttl`-dən sonra zibil yığılır. Bu dizayn şəbəkə formatını saxlayır
Norito dostudur, müştəriyə xas bərpa edilə bilən protokollardan qaçır və mövcud manifest boru kəmərindən yenidən istifadə edir
dəyişməz.

**İcra vəziyyəti.** Kanonik Norito növləri indi yaşayır
`crates/iroha_data_model/src/da/`:

- `ingest.rs` ilə birlikdə `DaIngestRequest`/`DaIngestReceipt` müəyyən edir
  Torii tərəfindən istifadə edilən `ExtraMetadata` konteyner.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` sonra Torii buraxan `DaManifestV1` və `ChunkCommitment` hostları
  parçalanma tamamlanır.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` paylaşılan ləqəbləri təmin edir (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` və s.) və aşağıda sənədləşdirilmiş standart siyasət dəyərlərini kodlayır.【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool faylları `config.da_ingest.manifest_store_dir`-ə enir, SoraFS orkestrasiyası üçün hazırdır
  müşahidəçi saxlama girişinə çəkmək üçün.【crates/iroha_torii/src/da/ingest.rs:220】

Sorğu, manifest və qəbz yükləri üçün gediş-gəliş əhatə dairəsi izlənilir
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito kodekini təmin edir
yeniləmələr arasında sabit qalır.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Saxlama defoltları.** İdarəetmə bu müddət ərzində ilkin saxlama siyasətini ratifikasiya etdi
SF-6; `RetentionPolicy::default()` tərəfindən tətbiq edilən defoltlar bunlardır:

- isti səviyyə: 7 gün (`604_800` saniyə)
- soyuq səviyyə: 90 gün (`7_776_000` saniyə)
- tələb olunan replikalar: `3`
- saxlama sinfi: `StorageClass::Hot`
- idarəetmə etiketi: `"da.default"`

Aşağı axın operatorları zolaq qəbul edildikdə bu dəyərləri açıq şəkildə ləğv etməlidir
daha sərt tələblər.