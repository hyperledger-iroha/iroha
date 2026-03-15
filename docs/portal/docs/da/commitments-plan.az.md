---
lang: az
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fd1985901145d0dbcc587d953b0b1a3b5210132c3f915ffd36ec81fbe0692b7
source_last_modified: "2026-01-22T14:45:01.276618+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

başlıq: Məlumatların Əlçatımlılığı Öhdəlikləri Planı
sidebar_label: Öhdəliklər Planı
təsvir: Nexus-də DA öhdəliklərinin daxil edilməsi üçün blok, RPC və sübut santexnika.
---

:::Qeyd Kanonik Mənbə
:::

# Sora Nexus Məlumat Əlçatımlılığı Öhdəlikləri Planı (DA-3)

_Tərtib tarixi: 25-03-2026 — Sahiblər: Əsas Protokol WG / Ağıllı Müqavilə Qrupu / Saxlama Komandası_

DA-3 Nexus blok formatını genişləndirir ki, hər bir zolaq deterministik qeydləri yerləşdirir.
DA-2 tərəfindən qəbul edilən blobları təsvir edir. Bu qeyd kanonik məlumatları əhatə edir
konstruksiyalar, blok boru kəməri qarmaqları, yüngül müştəri sübutları və Torii/RPC səthləri
validatorlar qəbul zamanı DA öhdəliklərinə etibar etməzdən əvvəl yerə enməlidir və ya
idarəetmə yoxlamaları. Bütün faydalı yüklər Norito kodludur; SCALE və ya ad-hoc JSON yoxdur.

## Məqsədlər

- Blob üzrə öhdəlikləri yerinə yetirin (kök kök + manifest hash + isteğe bağlı KZG
  öhdəlik) hər Nexus blokunun daxilində həmyaşıdların mövcudluğu yenidən qura bilməsi üçün
  off-ledger storage məsləhətləşmədən dövlət.
- Yüngül müştərilərin a
  manifest hash verilmiş blokda yekunlaşdırıldı.
- Torii sorğularını (`/v1/da/commitments/*`) və relelərə imkan verən sübutları ifşa edin,
  SDK-lar və hər birini təkrarlamadan idarəetmənin avtomatlaşdırılması auditinin mövcudluğu
  blok.
- Mövcud `SignedBlockWire` zərfini yenisini keçirərək kanonik saxlayın
  strukturları Norito metadata başlığı və blok hash törəməsi vasitəsilə.

## Əhatə dairəsinə baxış

1. `iroha_data_model::da::commitment` plus blokunda **Data modeli əlavələri**
   `iroha_data_model::block`-də başlıq dəyişiklikləri.
2. **İcraçı qarmaqları** beləliklə, `iroha_core` Torii tərəfindən buraxılan DA qəbzlərini qəbul edir
   (`crates/iroha_core/src/queue.rs` və `crates/iroha_core/src/block.rs`).
3. **Dözümlülük/indekslər** beləliklə, WSV öhdəlik sorğularına tez cavab verə bilər
   (`iroha_core/src/wsv/mod.rs`).
4. Aşağıdakı siyahı/sorğu/sübut nöqtələri üçün **Torii RPC əlavələri**
   `/v1/da/commitments`.
5. **İnteqrasiya testləri + qurğular** tel düzümünü və sızma axınını təsdiqləyir
   `integration_tests/tests/da/commitments.rs`.

## 1. Məlumat Modeli Əlavələri

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` aşağıda istifadə olunan mövcud 48 baytlıq nöqtəni yenidən istifadə edir
  `iroha_crypto::kzg`. Yox olduqda biz yalnız Merkle sübutlarına qayıdırıq.
- `proof_scheme` zolaq kataloqundan götürülüb; Merkle zolaqları KZG-ni rədd edir
  faydalı yüklər, `kzg_bls12_381` zolaqları isə sıfırdan fərqli KZG öhdəlikləri tələb edir. Torii
  hazırda yalnız Merkle öhdəlikləri istehsal edir və KZG tərəfindən konfiqurasiya edilmiş zolaqları rədd edir.
- `KzgCommitment` aşağıda istifadə olunan mövcud 48 baytlıq nöqtəni yenidən istifadə edir
  `iroha_crypto::kzg`. Merkle zolaqlarında olmadıqda biz Merkle sübutlarına qayıdırıq
  yalnız.
- `proof_digest` DA-5 PDP/PoTR inteqrasiyasını gözləyir, buna görə də eyni rekord
  blobları canlı saxlamaq üçün istifadə edilən seçmə cədvəlini sadalayır.

### 1.2 Blok başlığının genişləndirilməsi

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

Paket hash həm blok hashına, həm də `SignedBlockWire` metadatasına qidalanır.
yuxarı.

İcra qeydi: `BlockPayload` və şəffaf `BlockBuilder` indi ifşa olunur
`da_commitments` təyinedicilər/qəbuledicilər (bax: `BlockBuilder::set_da_commitments` və
`SignedBlock::set_da_commitments`), beləliklə hostlar əvvəlcədən qurulmuş paketi əlavə edə bilər
bloku bağlamadan əvvəl. Bütün köməkçi konstruktorlar bu sahəni `None` olaraq təyin edirlər
Torii vasitəsilə real paketləri keçirənə qədər.

### 1.3 Tel kodlaşdırması

- `SignedBlockWire::canonical_wire()` üçün Norito başlığını əlavə edir
  Mövcud əməliyyat siyahısından dərhal sonra `DaCommitmentBundle`. The
  versiya baytı `0x01`-dir.
- `SignedBlockWire::decode_wire()` `version` məlum olmayan paketləri rədd edir,
  `norito.md`-də təsvir olunan Norito siyasətinə uyğun gəlir.
- Hash törəmə yeniləmələri yalnız `block::Hasher`-də mövcuddur; yüngül müştərilərin dekodlanması
  mövcud tel formatı avtomatik olaraq yeni sahəni qazanır, çünki Norito
  başlıq onun mövcudluğunu elan edir.

## 2. İstehsal axını bloklayın

1. Torii DA qəbulu `DaIngestReceipt`-i yekunlaşdırır və onu internet səhifəsində dərc edir
   daxili növbə (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` `lane_id` altındakı bloka uyğun gələn bütün qəbzləri toplayır
   tikinti, `(lane_id, client_blob_id, manifest_hash)` tərəfindən təkrarlanma.
3. Bağlamadan dərhal əvvəl blok qurucusu öhdəlikləri `(lane_id,
   epoch, sequence)` hash deterministik saxlamaq üçün paketi ilə kodlaşdırır
   Norito kodek və `da_commitments_hash` yeniləmələri.
4. Tam paket WSV-də saxlanılır və içəridəki blokla yanaşı buraxılır
   `SignedBlockWire`.

Blok yaradılması uğursuz olarsa, qəbzlər növbədə qalır, beləliklə növbəti blok
cəhd onları götürə bilər; inşaatçı son daxil `sequence` başına qeyd edir
təkrar hücumların qarşısını almaq üçün zolaq.

## 3. RPC və Sorğu Səthi

Torii üç son nöqtəni ifşa edir:

| Marşrut | Metod | Yük | Qeydlər |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (zolaq/epox/ardıcıllıqla diapazon filtri, səhifələmə) | Ümumi say, öhdəliklər və blok hash ilə `DaCommitmentPage` qaytarır. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (zolaqlı + manifest hash və ya `(epoch, sequence)` dəsti). | `DaCommitmentProof` (rekord + Merkle yolu + blok hash) ilə cavab verir. |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Blok hash hesablamasını təkrarlayan və daxil edilməsini təsdiqləyən vətəndaşlığı olmayan köməkçi; birbaşa `iroha_crypto` ilə əlaqələndirə bilməyən SDK-lar tərəfindən istifadə olunur. |

Bütün faydalı yüklər `iroha_data_model::da::commitment` altında yaşayır. Torii marşrutlaşdırıcıları quraşdırılır
mövcud DA-nın yanında olan işləyicilər token/mTLS-ni təkrar istifadə etmək üçün son nöqtələri qəbul edir
siyasətlər.

## 4. Daxiletmə Sübutları və Yüngül Müştərilər

- Blok istehsalçısı seriallaşdırılmış üzərində ikili Merkle ağacı qurur
  `DaCommitmentRecord` siyahısı. Kök `da_commitments_hash` qidalandırır.
- `DaCommitmentProof` hədəf qeydini üstəgəl `(sibling_hash,
  mövqe) ` girişləri, beləliklə yoxlayıcılar kökü yenidən qura bilsinlər. Sübutlar da daxildir
  blok hash və imzalanmış başlıq, belə ki, yüngül müştərilər yekunluğu yoxlaya bilsin.
- CLI köməkçiləri (`iroha_cli app da prove-commitment`) sübut sorğusunu əhatə edir/doğrulayın
  operatorlar üçün dövr və səth Norito/hex çıxışları.

## 5. Saxlama və İndeksləmə

WSV öhdəlikləri `manifest_hash` tərəfindən əsaslanan xüsusi sütun ailəsində saxlayır.
İkinci dərəcəli indekslər `(lane_id, epoch)` və `(lane_id, sequence)` sorğularını əhatə edir
tam paketləri skan etməkdən çəkinin. Hər bir qeyd onu möhürləyən blokun hündürlüyünü izləyir,
tutma qovşaqlarına indeksi blok jurnalından tez bir zamanda bərpa etməyə imkan verir.

## 6. Telemetriya və Müşahidə Edilə bilənlik

- Blok ən azı birini möhürlədikdə `torii_da_commitments_total` artır
  rekord.
- `torii_da_commitment_queue_depth` paketlənməyi gözləyən qəbzləri izləyir (başına
  zolaq).
- Grafana idarə paneli `dashboards/grafana/da_commitments.json` bloku vizuallaşdırır
  daxiletmə, növbə dərinliyi və sübut ötürmə qabiliyyəti beləliklə DA-3 buraxılış qapıları yoxlaya bilsin
  davranış.

## 7. Test Strategiyası

1. `DaCommitmentBundle` kodlaşdırma/deşifrə və blok hash üçün **vahid testləri**
   törəmə yeniləmələri.
2. `fixtures/da/commitments/` altında **qızıl qurğular** kanonik təsvirlər
   paket baytları və Merkle sübutları.
3. **İnteqrasiya testləri** iki validatorun yüklənməsi, nümunə bloblarının qəbulu və
   hər iki qovşağın paketin məzmunu və sorğu/sübutla razılaşdığını iddia edir
   cavablar.
4. `integration_tests/tests/da/commitments.rs`-də **Light-client testləri**
   (Rust) ki, `/prove`-ə zəng edir və Torii ilə danışmadan sübutu yoxlayır.
5. Operatoru saxlamaq üçün **CLI smoke** skripti `scripts/da/check_commitments.sh`
   alətlər təkrar istehsal olunur.

## 8. Yayım Planı

| Faza | Təsvir | Çıxış meyarları |
|-------|-------------|---------------|
| P0 — Məlumat modelinin birləşməsi | Land `DaCommitmentRecord`, blok başlıq yeniləmələri və Norito kodekləri. | `cargo test -p iroha_data_model` yaşıl, yeni qurğularla. |
| P1 — Əsas/WSV naqilləri | Mövzu növbəsi + blok qurucu məntiqi, davamlı indekslər və RPC işləyicilərini ifşa edin. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` paket sübut təsdiqləri ilə keçir. |
| P2 — Operator alətləri | CLI köməkçilərini, Grafana tablosunu və sübut doğrulama sənədi yeniləmələrini göndərin. | `iroha_cli app da prove-commitment` devnet əleyhinə işləyir; tablosuna canlı məlumatları göstərir. |
| P3 — İdarəetmə qapısı | `iroha_config::nexus`-də işarələnmiş zolaqlarda DA öhdəlikləri tələb edən blok təsdiqləyicisini aktivləşdirin. | Status girişi + yol xəritəsi yeniləməsi DA-3-ü 🈴 olaraq qeyd edin. |

## Açıq Suallar

1. **KZG vs Merkle defoltları** — Kiçik bloblar həmişə KZG öhdəliklərini atlamalıdırlar
   blok ölçüsünü azaltmaq? Təklif: `kzg_commitment`-i isteğe bağlı saxlayın və keçid edin
   `iroha_config::da.enable_kzg`.
2. **Ardıcıl boşluqlar** — Biz sıradan çıxmış zolaqlara icazə veririkmi? Cari plan boşluqları rədd edir
   idarəetmə `allow_sequence_skips`-i fövqəladə təkrar oynatma üçün dəyişdirmədikdə.
3. **Light-client cache** — SDK komandası üçün yüngül SQLite keşi tələb olundu
   sübutlar; DA-8 altında təqibi gözləyir.

Tətbiq PR-lərində bunlara cavab vermək DA-3-ü 🈸 (bu sənəd)-dən 🈺-ə köçürür.
kod işi başladıqdan sonra.