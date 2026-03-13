---
lang: az
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Məlumat Əlçatımlılığı Qəbul Planı

_Tərtib tarixi: 2026-02-20 - Sahib: Əsas Protokol WG / Saxlama Qrupu / DA WG_

DA-2 iş axını Norito yayan blob ingest API ilə Torii-i genişləndirir
metadata və toxumların SoraFS replikasiyası. Bu sənəd təklif olunanı əhatə edir
sxem, API səthi və təsdiqləmə axını, beləliklə həyata keçirmə olmadan davam edə bilər
görkəmli simulyasiyaların bloklanması (DA-1 təqibləri). Bütün faydalı yük formatları MÜTLƏQDİR
Norito kodeklərindən istifadə edin; heç bir serde/JSON geri qaytarılmasına icazə verilmir.

## Məqsədlər

- Böyük ləkələri qəbul edin (Taikai seqmentləri, zolaqlı yan arabalar, idarəetmə artefaktları)
  deterministik olaraq Torii üzərində.
- Blob, kodek parametrlərini təsvir edən kanonik Norito manifestləri istehsal edin,
  profili silmək və saxlama siyasəti.
- SoraFS isti saxlama və növbəli təkrarlama işlərində parça metadatasını davam etdirin.
- SoraFS reyestrində və idarəetmədə pin niyyətləri + siyasət teqlərini dərc edin
  müşahidəçilər.
- Qəbul qəbzlərini ifşa edin ki, müştərilər nəşrin deterministik sübutunu bərpa etsinlər.

## API Səthi (Torii)

```
POST /v2/da/ingest
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

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

Cari zolaq kataloqundan alınmış versiyalı `DaProofPolicyBundle` qaytarır.
Paket `version` (hazırda `1`), `policy_hash` (hesh-i) reklam edir.
sifariş edilmiş siyasət siyahısı) və `policies` qeydləri, `lane_id`, `dataspace_id`,
`alias` və məcburi `proof_scheme` (bu gün `merkle_sha256`; KZG zolaqları
KZG öhdəlikləri mövcud olana qədər qəbul edilərək rədd edilir). İndi blok başlığı
`da_proof_policies_hash` vasitəsilə paketi qəbul edir, beləliklə müştərilər
DA öhdəlikləri və ya sübutları yoxlanarkən təyin edilmiş aktiv siyasət. Bu son nöqtəni gətirin
zolağın siyasətinə və cərəyanına uyğun olduğundan əmin olmaq üçün sübutlar yaratmadan əvvəl
paket hash. Öhdəlik siyahısı/sübut son nöqtələri SDK-lar üçün eyni paketi daşıyır
sübutu aktiv siyasət dəstinə bağlamaq üçün əlavə gediş-gəlişə ehtiyac yoxdur.

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Sifariş edilmiş siyasət siyahısını üstəgəl a daşıyan `DaProofPolicyBundle` qaytarır
`policy_hash` beləliklə, SDK-lar blok istehsal edilərkən istifadə olunan versiyanı bağlaya bilsin. The
hash Norito kodlu siyasət massivi üzərindən hesablanır və hər dəfə dəyişdikdə
zolağın `proof_scheme` yenilənir və müştərilərə yollar arasında sürüşməni aşkar etməyə imkan verir.
önbelleğe alınmış sübutlar və zəncir konfiqurasiyası.

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
```> İcra qeydi: bu faydalı yüklər üçün kanonik Rust təqdimatları indi altındadır
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest`-də sorğu/qəbz sarğıları ilə
> və `iroha_data_model::da::manifest`-dəki manifest strukturu.

`compression` sahəsi zəng edənlərin faydalı yükü necə hazırladığını elan edir. Torii qəbul edir
`identity`, `gzip`, `deflate` və `zstd` əvvəl baytları şəffaf şəkildə açır
isteğe bağlı manifestlərin hashing, parçalanması və yoxlanması.

### Doğrulama Yoxlama Siyahısı

1. Sorğunun Norito başlığının `DaIngestRequest` ilə uyğun gəldiyini yoxlayın.
2. `total_size` kanonik (sıxışdırılmış) faydalı yük uzunluğundan fərqlənirsə və ya konfiqurasiya edilmiş maks.
3. `chunk_size` uyğunlaşdırılmasını tətbiq edin (ikinin gücü, = 2 olduğundan əmin olun.
5. `retention_policy.required_replica_count` idarəetmənin ilkin səviyyəsinə hörmət etməlidir.
6. Kanonik hash-ə qarşı imzanın yoxlanılması (imza sahəsi istisna olmaqla).
7. Faydalı yük hash + metadata eyni deyilsə, `client_blob_id` dublikatını rədd edin.
8. `norito_manifest` təmin edildikdə, yenidən hesablanmış heş uyğunluqları + sxemi yoxlayın
   parçalanmadan sonra aşkar; əks halda node manifest yaradır və onu saxlayır.
9. Konfiqurasiya edilmiş təkrarlama siyasətini tətbiq edin: Torii təqdim edilənləri yenidən yazır
   `RetentionPolicy`, `torii.da_ingest.replication_policy` ilə (bax.
   `replication_policy.md`) və saxlanması əvvəlcədən qurulmuş manifestləri rədd edir
   metadata məcburi profilə uyğun gəlmir.

### Parçalanma və Replikasiya axını1. `chunk_size`-ə yığın yükü, hər parça üçün BLAKE3 + Merkle kökünü hesablayın.
2. Parça öhdəlikləri (rol/qrup_id) ələ keçirən Norito `DaManifestV1` (yeni struktur) qurun,
   düzeni silmək (sətir və sütun paritetləri üstəgəl `ipa_commitment`), saxlama siyasəti,
   və metadata.
3. `config.da_ingest.manifest_store_dir` altında kanonik manifest baytlarını növbəyə qoyun
   (Torii, zolaq/epox/ardıcıllıq/bilet/barmaq izi ilə düymələnmiş `manifest.encoded` fayllarını yazır) beləliklə, SoraFS
   orkestr onları qəbul edə və saxlama biletini davamlı data ilə əlaqələndirə bilər.
4. İdarəetmə etiketi + siyasəti ilə `sorafs_car::PinIntent` vasitəsilə pin niyyətlərini dərc edin.
5. Müşahidəçiləri xəbərdar etmək üçün Norito hadisəsini buraxın (yüngül müştərilər,
   idarəetmə, analitika).
6. `DaIngestReceipt` (Torii DA xidmət açarı ilə imzalanmış) qaytarın və əlavə edin
   base64 Norito kodlamasını ehtiva edən `Sora-PDP-Commitment` cavab başlığı
   SDK-lar dərhal seçmə toxumunu saxlaya bilsinlər.
   Qəbz indi `rent_quote` (a `DaRentQuote`) və `stripe_layout`-i daxil edir
   beləliklə, təqdim edənlər XOR öhdəliklərini, ehtiyat payı, PDP/PoTR bonus gözləntilərini,
   və pul köçürməzdən əvvəl saxlama bileti metadatası ilə yanaşı 2D silmə matrisinin ölçüləri.
7. Əlavə reyestr metadatası:
   - `da.registry.alias` — pin reyestrinə daxil olmaq üçün ictimai, şifrələnməmiş UTF-8 ləqəb sətri.
   - `da.registry.owner` — reyestr sahibliyini qeyd etmək üçün ictimai, şifrələnməmiş `AccountId` sətri.
   Torii bunları yaradılan `DaPinIntent`-ə kopyalayır, beləliklə aşağı axın pin emalı ləqəbləri birləşdirə bilər
   və xam metadata xəritəsini yenidən təhlil etmədən sahiblər; zamanı pozulmuş və ya boş dəyərlər rədd edilir
   doğrulamanı qəbul edin.

## Saxlama / Reyestr yeniləmələri

- `sorafs_manifest`-i `DaManifestV1` ilə genişləndirin, deterministik təhlilə imkan verin.
- Versiyalaşdırılmış faydalı yük istinadı ilə yeni reyestr axını `da.pin_intent` əlavə edin
  manifest hash + bilet id.
- Müşahidə oluna bilən boru kəmərlərinin qəbulu gecikməsini, ötürmə qabiliyyətini izləmək üçün yeniləyin,
  replikasiya geriliyi və uğursuzluqların sayı.
- Torii `/status` cavablarına indi ən son versiyaları əks etdirən `taikai_ingest` massivi daxildir
  DA-9-u işə salan kodlayıcıdan qəbul üçün gecikmə, canlı kənar sürüşmə və səhv sayğacları (klaster, axın)
  Prometheus-ni qırmadan birbaşa qovşaqlardan sağlamlıq anlıq görüntülərini qəbul etmək üçün idarə panelləri.

## Test Strategiyası- Sxemlərin yoxlanılması, imzaların yoxlanılması, dublikatın aşkarlanması üçün vahid testləri.
- `DaIngestRequest`, manifest və qəbzin Norito kodlamasını təsdiqləyən qızıl testlər.
- İstehzalı SoraFS + reyestrini fırlanan inteqrasiya qoşqu, yığın + pin axınlarını təsdiqləyir.
- Təsadüfi silmə profillərini və saxlama birləşmələrini əhatə edən əmlak testləri.
- Səhv formalaşdırılmış metadatadan qorunmaq üçün Norito faydalı yüklərinin puslanması.
- Hər blob sinfi üçün qızıl qurğular altında yaşayır
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` köməkçi parça ilə
  `fixtures/da/ingest/sample_chunk_records.txt`-də siyahı. Göz ardı edilən test
  `regenerate_da_ingest_fixtures` isə armaturları təzələyir
  `manifest_fixtures_cover_all_blob_classes` yeni `BlobClass` variantı əlavə edilən kimi uğursuz olur
  Norito/JSON paketini yeniləmədən. Bu, DA-2 olduqda Torii, SDK və sənədləri dürüst saxlayır
  yeni blob səthini qəbul edir.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI və SDK Alətləri (DA-8)- `iroha app da submit` (yeni CLI giriş nöqtəsi) indi operatorlar üçün paylaşılan qəbul qurucusunu/naşirini əhatə edir
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
  `--plan`, `--manifest-id`) **və ya** sadəcə `--storage-ticket` vasitəsilə Torii yaddaş biletini keçin. Zaman
  bilet yolundan istifadə olunur CLI manifestini `/v2/da/manifests/<ticket>`-dən çıxarır, paketi davam etdirir
  `artifacts/da/fetch_<timestamp>/` altında (`--manifest-cache-dir` ilə əvəzlənir), **manifest əldə edir
  `--manifest-id` üçün hash** və sonra təchiz edilmiş `--gateway-provider` ilə orkestratoru idarə edir
  siyahı. Şlüz identifikatoru olduğu halda faydalı yükün yoxlanılması hələ də daxil edilmiş CAR/`blob_hash` həzminə əsaslanır.
  indi manifest hashdır ki, müştərilər və validatorlar tək blob identifikatorunu paylaşsınlar. Bütün qabaqcıl düymələr
  SoraFS alıcı səthi (manifest zərfləri, müştəri etiketləri, qoruyucu keşlər, anonimlik daşımaları)
  ləğvetmələr, tablonun ixracı və `--output` yolları) və manifest son nöqtəsi vasitəsilə ləğv edilə bilər
  Fərdi Torii hostları üçün `--manifest-endpoint`, beləliklə, başdan sona mövcudluq yoxlamaları tamamilə
  Orkestr məntiqini təkrarlamadan `da` ad sahəsi.
- `iroha app da get-blob` `GET /v2/da/manifests/{storage_ticket}` vasitəsilə kanonik manifestləri birbaşa Torii-dən çıxarır.
  Komanda indi artefaktları manifest hash (blob id), yazı ilə etiketləyir
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` və `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/` altında (və ya istifadəçi tərəfindən təchiz edilmiş `--output-dir`) dəqiq əks-səda verir
  `iroha app da get` çağırışı (`--manifest-id` daxil olmaqla) təqib orkestrinin gətirilməsi üçün tələb olunur.
  Bu, operatorları manifest spool qovluqlarından kənarda saxlayır və alıcının həmişə istifadə etdiyinə zəmanət verir
  Torii tərəfindən yayılan imzalı artefaktlar. JavaScript Torii müştərisi bu axını vasitəsilə əks etdirir
  Swift SDK indi ifşa edərkən `ToriiClient.getDaManifest(storageTicketHex)`
  `ToriiClient.getDaManifestBundle(...)`. Hər ikisi deşifrə edilmiş Norito baytını, JSON manifestini, manifest hashını,və SDK-ya zəng edənlərin CLI və Swift-ə müdaxilə etmədən orkestr seanslarını nəmləndirə bilməsi üçün yığın planı
  müştərilər əlavə olaraq `fetchDaPayloadViaGateway(...)`-ə zəng edərək həmin paketləri yerli şəbəkədən keçirə bilərlər.
  SoraFS orkestr sarğı.【IrohaSwift/Mənbələr/IrohaSwift/ToriiClient.swift:240】
- `/v2/da/manifests` cavabları indi `manifest_hash` və hər iki CLI + SDK köməkçiləri (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` və Swift/JS şlüz sarğıları) bu həzmi
  yerləşdirilmiş CAR/blob heşinə qarşı faydalı yükləri yoxlamağa davam edərkən kanonik manifest identifikatoru.
- `iroha app da rent-quote` təchiz edilmiş saxlama ölçüsü üçün deterministik icarə və həvəsləndirici qəzaları hesablayır
  və saxlama pəncərəsi. Köməkçi ya aktiv `DaRentPolicyV1` (JSON və ya Norito bayt) istehlak edir, ya da
  daxili defolt, siyasəti doğrulayır və JSON xülasəsini çap edir (`gib`, `months`, siyasət metadatası,
  və `DaRentQuote` sahələri) beləliklə, auditorlar idarəetmə protokolları daxilində dəqiq XOR ittihamlarına istinad edə bilərlər.
  ad hoc skriptlərin yazılması. Komanda indi də JSON-dan əvvəl bir sətirli `rent_quote ...` xülasəsini yayır
  hadisələr zamanı sitatlar yarandıqda konsol qeydlərini və runbook-ları skan etməyi asanlaşdırmaq üçün faydalı yük.
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (və ya hər hansı digər yol) keçin
  gözəl çap edilmiş xülasəni davam etdirmək üçün `--policy-label "governance ticket #..."` istifadə edin
  artefakt xüsusi səs/konfiqurasiya paketinə istinad etməlidir; CLI xüsusi etiketləri kəsir və boşları rədd edir
  `policy_source` dəyərlərini sübut paketlərində mənalı saxlamaq üçün sətirlər. Bax
  Alt komanda üçün `crates/iroha_cli/src/commands/da.rs` və `docs/source/da/rent_policy.md`
  siyasət sxemi üçün.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- Pin reyestrinin pariteti indi SDK-lara yayılır: `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK `iroha app sorafs pin register` tərəfindən istifadə olunan dəqiq yükü qurur və kanonik standartları tətbiq edir.
  POST-a göndərməzdən əvvəl chunker metadata, pin siyasətləri, ləqəb sübutları və davamçı həzmlər
  `/v2/sorafs/pin/register`. Bu, CI botlarını və avtomatlaşdırmanı zaman CLI-yə atmaqdan qoruyur
  manifest qeydiyyatlarını qeyd edir və köməkçi TypeScript/README əhatə dairəsi ilə göndərilir ki, DA-8
  Rust/Swift ilə yanaşı JS-də “submit/get/prove” alət pariteti tam təmin edilir.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:78】
- `iroha app da prove-availability` yuxarıda göstərilənlərin hamısını zəncirləyir: saxlama bileti alır,
  kanonik manifest paketi, çoxmənbəli orkestratoru (`iroha app sorafs fetch`)
  təchiz edilmiş `--gateway-provider` siyahısı, yüklənmiş faydalı yük + skorbord altında saxlanılır
  `artifacts/da/prove_availability_<timestamp>/` və dərhal mövcud PoR köməkçisini işə salır
  (`iroha app da prove`) gətirilən baytlardan istifadə edir. Operatorlar orkestr düymələrini düzəldə bilərlər
  (`--max-peers`, `--scoreboard-out`, manifest son nöqtəni ləğv edir) və sübut nümunəsi
  (`--sample-count`, `--leaf-index`, `--sample-seed`) tək bir əmr artefaktlar yaradır
  DA-5/DA-9 auditləri tərəfindən gözlənilən: faydalı yük nüsxəsi, skorbord sübutu və JSON sübut xülasələri.- `da_reconstruct` (DA-6-da yeni) kanonik manifest və yığın tərəfindən buraxılan yığın kataloqunu oxuyur
  saxlayır (`chunk_{index:05}.bin` layout) və yoxlayarkən faydalı yükü müəyyən bir şəkildə yenidən yığır
  hər Blake3 öhdəliyi. CLI `crates/sorafs_car/src/bin/da_reconstruct.rs` altında yaşayır və kimi göndərilir
  SoraFS alətlər paketinin bir hissəsi. Tipik axın:
  1. `manifest_<manifest_hash>.norito` və yığın planını yükləmək üçün `iroha app da get-blob --storage-ticket <ticket>`.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (və ya `iroha app da prove-availability`, altında gətirmə artefaktlarını yazır
     `artifacts/da/prove_availability_<ts>/`).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Reqressiya qurğusu `fixtures/da/reconstruct/rs_parity_v1/` altında yaşayır və tam manifestini çəkir
  və `tests::reconstructs_fixture_with_parity_chunks` tərəfindən istifadə edilən yığın matrisi (məlumat + paritet). İlə bərpa edin

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  Qurğu yayır:

  - `manifest.{norito.hex,json}` — kanonik `DaManifestV1` kodlaşdırmaları.
  - `chunk_matrix.json` — sənəd/sınaq istinadları üçün sifarişli indeks/ofset/uzunluq/həzm/paritet sıraları.
  - `chunks/` — `chunk_{index:05}.bin` həm məlumat, həm də paritet parçaları üçün faydalı yük dilimləri.
  - `payload.bin` — paritetdən xəbərdar olan qoşqu testi tərəfindən istifadə edilən deterministik faydalı yük.
  - `commitment_bundle.{json,norito.hex}` — sənədlər/testlər üçün deterministik KZG öhdəliyi ilə `DaCommitmentBundle` nümunəsi.

  Qoşqu çatışmayan və ya kəsilmiş parçaları rədd edir, son faydalı yük Blake3 hashını `blob_hash` ilə yoxlayır,
  və xülasə JSON blob (faydalı yük baytları, yığın sayı, saxlama bileti) yayır, beləliklə CI yenidənqurma işini təsdiq edə bilər
  sübut. Bu, operatorların və QA-nın deterministik yenidənqurma aləti üçün DA-6 tələbini bağlayır
  işlər sifarişli skriptlərə qoşulmadan işə salına bilər.

## TODO Qətnamə Xülasəsi

Əvvəllər bloklanmış bütün qəbul TODO-ları həyata keçirilib və təsdiqlənib:- **Sıxılma göstərişləri** — Torii zəng edənin təqdim etdiyi etiketləri qəbul edir (`identity`, `gzip`, `deflate`,
  `zstd`) və doğrulamadan əvvəl faydalı yükləri normallaşdırır, beləliklə kanonik manifest hash ilə uyğun gəlir.
  sıxılmış baytlar.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Yalnız idarəetmə üçün metadata şifrələməsi** — Torii indi idarəetmə metadatasını şifrələyir
  konfiqurasiya edilmiş ChaCha20-Poly1305 açarı, uyğun olmayan etiketləri rədd edir və iki açıq-aydın görünür
  konfiqurasiya düymələri (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) fırlanmanı deterministik saxlamaq üçün.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Böyük yük axını** — çox hissəli qəbul canlıdır. Müştərilər deterministik axın
  `client_blob_id`, Torii tərəfindən əsaslanan `DaIngestChunk` zərfləri hər bir dilimi doğrulayır, onları mərhələləşdirir
  `manifest_store_dir` altında və `is_last` bayrağı endikdən sonra manifesti atomik şəkildə yenidən qurur,
  tək zəngli yükləmələrdə görülən RAM artımlarının aradan qaldırılması.【crates/iroha_torii/src/da/ingest.rs:392】
- **Manifest versiyası** — `DaManifestV1` açıq `version` sahəsini daşıyır və Torii imtina edir
  yeni manifest tərtibatları göndərildikdə deterministik təkmilləşdirmələrə zəmanət verən naməlum versiyalar.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR qarmaqları** — PDP öhdəlikləri birbaşa yığın mağazasından alınır və davamlıdır
  manifestlərdən əlavə, beləliklə DA-5 planlaşdırıcıları kanonik məlumatlardan seçmə problemləri başlaya bilər; the
  `Sora-PDP-Commitment` başlığı indi həm `/v2/da/ingest`, həm də `/v2/da/manifests/{ticket}` ilə göndərilir
  cavablar belə ki, SDK-lar gələcək tədqiqatların istinad edəcəyi imzalanmış öhdəliyi dərhal öyrənsinlər.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da:4/76rs.
- **Kəskin kursor jurnalı** — zolaqlı metadata `da_shard_id` (defolt olaraq `lane_id`) təyin edə bilər və
  Sumeragi indi `(shard_id, lane_id)` üçün ən yüksək `(epoch, sequence)` olaraq qalır.
  DA makarasının yanında `da-shard-cursors.norito`, beləliklə, yenidən işə salınmış/naməlum zolaqları buraxır və saxlayır
  deterministik təkrar. Yaddaşdaxili kursor indeksi indi öhdəliklər üzrə tez uğursuz olur
  zolaq identifikatoruna defolt etmək əvəzinə xəritəsiz zolaqlar, kursorun irəliləməsi və təkrar oynatma xətaları
  açıq, və blok doğrulama xüsusi ilə shard-kursor reqressiyalarını rədd edir
  `DaShardCursorViolation` səbəb + operatorlar üçün telemetriya etiketləri. Başlanğıc/tutma indi DA-nı dayandırır
  əgər Kürdə naməlum zolaq və ya reqressiya kursoru varsa və pozuntunu qeyd edirsə, nəmləndirmə indeksi
  operatorların DA-ya xidmət etməzdən əvvəl düzəliş edə bilməsi üçün blok hündürlüyü vəziyyət.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Kəskin kursor lag telemetriyası** — `da_shard_cursor_lag_blocks{lane,shard}` ölçmə cihazı necə olduğunu bildiriruzaq bir qırıq doğrulanan hündürlüyü izləyir. Çatışmayan/köhnəlmiş/naməlum zolaqlar gecikməni təyin edir
  tələb olunan hündürlük (və ya delta) və uğurlu irəliləyişlər onu sıfıra qaytarır ki, sabit vəziyyət düz qalır.
  Operatorlar sıfır olmayan gecikmələr barədə xəbərdarlıq etməli, DA makarasını/jurnalını pozan zolaq üçün yoxlamalıdır,
  və bloku silmək üçün təkrar oynatmazdan əvvəl təsadüfən yenidən parçalanma üçün zolaq kataloqunu yoxlayın
  boşluq.
- **Məxfi hesablama zolaqları** — ilə işarələnmiş zolaqlar
  `metadata.confidential_compute=true` və `confidential_key_version` kimi qəbul edilir
  SMPC/şifrələnmiş DA yolları: Sumeragi sıfır olmayan faydalı yük/manifest həzmləri və saxlama biletlərini tətbiq edir,
  tam replika yaddaş profillərini rədd edir və SoraFS bilet + siyasət versiyasını olmadan indeksləşdirir
  faydalı yük baytlarını ifşa edir. Qəbzlər təkrar oynatma zamanı Kürdən hidratlanır, beləliklə, validatorlar eyni şeyi bərpa edirlər
  sonra məxfilik metadata yenidən başladır.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_config/src/states.

## İcra Qeydləri- Torii-in `/v2/da/ingest` son nöqtəsi indi faydalı yükün sıxılmasını normallaşdırır, təkrar oynatma keşini tətbiq edir,
  kanonik baytları deterministik olaraq parçalayır, `DaManifestV1`-i yenidən qurur və kodlaşdırılmış faydalı yükü azaldır
  qəbzi verməzdən əvvəl SoraFS orkestrasiyası üçün `config.da_ingest.manifest_store_dir`-ə; the
  işləyici həmçinin `Sora-PDP-Commitment` başlığını əlavə edir ki, müştərilər kodlaşdırılmış öhdəliyi əldə edə bilsinlər
  dərhal.【crates/iroha_torii/src/da/ingest.rs:220】
- Kanonik `DaCommitmentRecord` davam etdikdən sonra Torii indi
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` faylı manifest spoolunun yanındadır.
  Hər bir giriş rekordu xam Norito `PdpCommitment` baytları ilə birləşdirir, beləliklə DA-3 paket qurucuları və
  DA-5 planlaşdırıcıları manifestləri təkrar oxumadan və ya yığın saxlamadan eyni girişləri qəbul edir.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK köməkçiləri hər bir müştərini Norito təhlilini təkrar etməyə məcbur etmədən PDP başlıq baytlarını ifşa edir:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` Rust örtüyü, Python `ToriiClient`
  indi `decode_pdp_commitment_header` ixrac edir və `IrohaSwift` uyğun gələn köməkçiləri o qədər mobil göndərir
  müştərilər dərhal kodlaşdırılmış seçmə cədvəlini saxlaya bilərlər.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii həmçinin `GET /v2/da/manifests/{storage_ticket}`-i ifşa edir ki, SDK və operatorlar manifestləri əldə edə bilsinlər
  və qovşağın makara kataloquna toxunmadan yığın planları. Cavab Norito baytını qaytarır
  (base64), göstərilən manifest JSON, `chunk_plan` üçün hazır olan `chunk_plan` JSON blobu, üstəgəl müvafiq
  hex həzmlər (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) belə aşağı axın alətləri
  həzmləri yenidən hesablamadan orkestratoru qidalandırın və eyni `Sora-PDP-Commitment` başlığını yayır
  güzgü qəbul cavabları. `block_hash=<hex>`-i sorğu parametri kimi ötürmək deterministi qaytarır
  `sampling_plan` köklü `block_hash || client_blob_id` (validatorlar arasında paylaşılır) ehtiva edən
  `assignment_hash`, tələb olunan `sample_window` və seçmə `(index, role, group)` dəstləri
  bütün 2D zolaq düzeni beləliklə, PoR nümunələri və validatorlar eyni indeksləri təkrarlaya bilsinlər. Nümunəçi
  `client_blob_id`, `chunk_root` və `ipa_commitment`-i təyinat hash-ə qarışdırır; `iroha proqramı əldə edin
  --block-hash ` now writes `sampling_plan_.json` manifest + yığın planının yanında
  hash qorunub saxlanılır və JS/Swift Torii müştəriləri eyni `assignment_hash_hex`-i ifşa edir, beləliklə validatorlar
  və provers vahid deterministik sonda dəstini bölüşürlər. Torii seçmə planını qaytardıqda, `iroha app da
  sübut-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) əvəzinə
  xüsusi nümunə götürmənin beləliklə, PoR şahidləri operatorun bir şərti buraxdığı halda belə, validator tapşırıqları ilə sıralanır.
  `--block-hash` ləğv edin.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Böyük faydalı yük axını axınıKonfiqurasiya edilmiş tək sorğu limitindən daha böyük aktivləri qəbul etməli olan müştərilər
`POST /v2/da/ingest/chunk/start`-ə zəng edərək axın sessiyası. Torii a ilə cavab verir
`ChunkSessionId` (BLAKE3-tələb olunan blob metadatasından əldə edilib) və razılaşdırılmış yığın ölçüsü.
Hər bir sonrakı `DaIngestChunk` sorğusu aşağıdakıları ehtiva edir:

- `client_blob_id` — yekun `DaIngestRequest` ilə eynidir.
- `chunk_session_id` — dilimləri işləyən sessiyaya bağlayır.
- `chunk_index` və `offset` — deterministik sifarişi tətbiq edir.
- `payload` — razılaşdırılmış parça ölçüsünə qədər.
- `payload_hash` — dilimin BLAKE3 hashı beləliklə, Torii bütün blobu bufer etmədən doğrulaya bilsin.
- `is_last` — terminal dilimini göstərir.

Torii təsdiqlənmiş dilimləri `config.da_ingest.manifest_store_dir/chunks/<session>/` altında saxlayır və
qeyri-mümkünlüyünə hörmət etmək üçün təkrar oynatma keşində irəliləyişi qeyd edir. Son dilim yerə düşəndə Torii
diskdəki faydalı yükü yenidən yığır (yaddaş sıçrayışlarının qarşısını almaq üçün yığın kataloqu vasitəsilə axın),
kanonik manifest/qəbzi birdəfəlik yükləmələrdə olduğu kimi hesablayır və nəhayət cavab verir
`POST /v2/da/ingest` mərhələli artefaktı istehlak edərək. Uğursuz seanslar açıq şəkildə dayandırıla bilər və ya
`config.da_ingest.replay_cache_ttl`-dən sonra zibil yığılır. Bu dizayn şəbəkə formatını saxlayır
Norito dostudur, müştəriyə xas bərpa edilə bilən protokollardan qaçır və mövcud manifest boru kəmərindən yenidən istifadə edir
dəyişməz.

**İcra vəziyyəti.** Kanonik Norito növləri indi yaşayır
`crates/iroha_data_model/src/da/`:

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` ilə birlikdə müəyyən edir
  Torii tərəfindən istifadə edilən `ExtraMetadata` konteyner.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs`, sonra Torii buraxan `DaManifestV1` və `ChunkCommitment` hostlarını
  parçalanma tamamlanır.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` paylaşılan ləqəbləri təmin edir (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` və s.) və aşağıda sənədləşdirilmiş standart siyasət dəyərlərini kodlayır.【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool faylları `config.da_ingest.manifest_store_dir`-ə enir, SoraFS orkestrasiyası üçün hazırdır
  müşahidəçi saxlama girişinə çəkmək üçün.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi DA paketlərini möhürləyərkən və ya təsdiqləyərkən açıq-aşkar əlçatanlığı tətbiq edir:
  Əgər spoolda manifest yoxdursa və ya hash fərqlidirsə, bloklar doğrulamadan uğursuz olur
  öhdəlikdən.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

Sorğu, manifest və qəbz yükləri üçün gediş-gəliş əhatə dairəsi izlənilir
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito kodekini təmin edir
yeniləmələr arasında sabit qalır.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Saxlama defoltları.** İdarəetmə bu müddət ərzində ilkin saxlama siyasətini ratifikasiya etdi
SF-6; `RetentionPolicy::default()` tərəfindən tətbiq edilən defoltlar bunlardır:- isti səviyyə: 7 gün (`604_800` saniyə)
- soyuq səviyyə: 90 gün (`7_776_000` saniyə)
- tələb olunan replikalar: `3`
- saxlama sinfi: `StorageClass::Hot`
- idarəetmə etiketi: `"da.default"`

Aşağı axın operatorları zolaq qəbul edildikdə bu dəyərləri açıq şəkildə ləğv etməlidir
daha sərt tələblər.

## Pas müştəri sübut artefaktlar

Rust müştərisini yerləşdirən SDK-ların artıq CLI-yə daxil olmasına ehtiyac yoxdur
kanonik PoR JSON paketini istehsal edin. `Client` iki köməkçini ifşa edir:

- `build_da_proof_artifact` tərəfindən yaradılan dəqiq strukturu qaytarır
  `iroha app da prove --json-out`, verilən manifest/faydalı yük annotasiyaları daxil olmaqla
  [`DaProofArtifactMetadata`] vasitəsilə.【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` qurucusunu sarar və artefaktı diskdə saxlayır
  (defolt olaraq gözəl JSON + arxada gələn yeni sətir) beləliklə avtomatlaşdırma faylı əlavə edə bilər
  nəşrlər və ya idarəetmə sübut paketləri üçün.【crates/iroha/src/client.rs:3653】

### Nümunə

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

Köməkçini tərk edən JSON yükü sahə adlarına qədər CLI ilə uyğun gəlir
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest` və s.), belə ki, mövcud
avtomatlaşdırma faylı formata uyğun filiallar olmadan fərqləndirə / parketə / yükləyə bilər.

## Sübut yoxlama meyarları

Daha əvvəl təmsil olunan faydalı yüklər üzrə yoxlayıcı büdcələri təsdiqləmək üçün DA sübut etalon kəmərindən istifadə edin
blok səviyyəli qapaqların bərkidilməsi:

- `cargo xtask da-proof-bench` manifest/faydalı yük cütündən yığın mağazasını yenidən qurur, PoR nümunələri
  vərəqlər və konfiqurasiya edilmiş büdcəyə uyğun vaxtların yoxlanılması. Taikai metadata avtomatik doldurulur və
  Armatur cütü uyğun gəlmirsə, qoşqu sintetik manifestə qayıdır. Zaman `--payload-bytes`
  aydın `--payload` olmadan təyin edilir, yaradılan blob yazılır
  `artifacts/da/proof_bench/payload.bin` beləliklə qurğular toxunulmaz qalır.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Hesabatlar standart olaraq `artifacts/da/proof_bench/benchmark.{json,md}`-ə uyğundur və sübutlar/çalışma, cəmi və
  hər sübut üçün vaxt, büdcə keçid dərəcəsi və tövsiyə olunan büdcə (ən yavaş iterasiyanın 110%-i)
  `zk.halo2.verifier_budget_ms` ilə sıralayın.【artifacts/da/proof_bench/benchmark.md:1】
- Ən son buraxılış (sintetik 1 MiB faydalı yük, 64 KiB parça, 32 sübut/çalışma, 10 iterasiya, 250 ms büdcə)
  qapaq daxilində təkrarların 100%-i olan 3 ms doğrulayıcı büdcəni tövsiyə etdi.【artifacts/da/proof_bench/benchmark.md:1】
- Nümunə (deterministik faydalı yük yaradır və hər iki hesabatı yazır):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

Blok montajı eyni büdcələri tətbiq edir: `sumeragi.da_max_commitments_per_block` və
`sumeragi.da_max_proof_openings_per_block` bloka daxil edilməzdən əvvəl DA paketini bağlayır və
hər bir öhdəlik sıfırdan fərqli `proof_digest` daşımalıdır. Mühafizəçi paketin uzunluğuna kimi yanaşır
açıq-aydın sübut xülasələri konsensus vasitəsilə yivli qədər sübut-açma sayı, saxlanılması
≤128-blok sərhəddində tətbiq edilə bilən açılış hədəfi.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR nasazlığının idarə edilməsi və kəsilməsiSaxlama işçiləri indi hər biri ilə yanaşı PoR uğursuzluq zolaqlarını və yapışdırılmış kəsik tövsiyələrini ortaya qoyurlar
hökm. Konfiqurasiya edilmiş tətil həddinin üstündəki ardıcıl uğursuzluqlar tövsiyə verir
provayder/manifest cütlüyü, kəsik xəttinə səbəb olan zolağın uzunluğu və təklif olunan
provayder istiqrazından hesablanmış cərimə və `penalty_bond_bps`; soyutma pəncərələri (saniyələri) saxlayır
eyni hadisəyə atəşdən dublikat kəsiklər.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:34

- Saxlama işçisi qurucusu vasitəsilə hədləri/soyutma müddətini konfiqurasiya edin (defoltlar idarəetməni əks etdirir
  cəza siyasəti).
- Slash tövsiyələri hökmün xülasəsi JSON-da qeyd olunur ki, idarəetmə/auditorlar əlavə edə bilsinlər
  dəlil paketlərinə.
- Zolaq düzümü + hər bir hissə rolları indi Torii saxlama pininin son nöqtəsindən keçir
  (`stripe_layout` + `chunk_roles` sahələri) və saxlama işçisinə belə davam etdi
  auditorlar/təmir alətləri yuxarıdan planı yenidən əldə etmədən sıra/sütun təmirini planlaşdıra bilər

### Yerləşdirmə + təmir qoşqu

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` indi
`(index, role, stripe/column, offsets)` üzərində yerləşdirmə hashını hesablayır və cərgəni əvvəl yerinə yetirir
Faydalı yükün yenidən qurulmasından əvvəl RS(16) sütununun təmiri:

- Mövcud olduqda yerləşdirmə defolt olaraq `total_stripes`/`shards_per_stripe` olaraq qalır və yenidən hissəyə düşür
- Çatışmayan/korlanmış parçalar əvvəlcə sıra pariteti ilə yenidən qurulur; Qalan boşluqlar ilə təmir edilir
  zolaq (sütun) pariteti. Təmir edilmiş parçalar yenidən yığın kataloquna və JSON-a yazılır
  xülasə yerləşdirmə hashını və sətir/sütun təmir sayğaclarını çəkir.
- Əgər sətir+sütun pariteti çatışmayan dəsti təmin edə bilmirsə, qoşqu bərpa olunmayan ilə tez sıradan çıxır.
  auditorların düzəlməz manifestləri qeyd edə bilməsi üçün indekslər.