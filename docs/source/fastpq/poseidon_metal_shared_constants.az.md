---
lang: az
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2025-12-29T18:16:35.955568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Poseidon Metal Paylaşılan Sabitlər

Metal ləpələr, CUDA ləpələri, Rust proveri və hər bir SDK qurğusu paylaşmalıdır
hardware sürətləndirilmiş saxlamaq üçün eyni Poseidon2 parametrləri
hashing deterministik. Bu sənəd kanonik snapshot, necə ediləcəyini qeyd edir
onu bərpa edin və GPU boru kəmərlərinin məlumatları necə qəbul edəcəyi gözlənilir.

## Snapshot Manifest

Parametrlər `PoseidonSnapshot` RON sənədi kimi dərc olunur. Nüsxələrdir
versiya nəzarəti altında saxlanılır, ona görə də GPU alət zəncirləri və SDK-lar qurulma müddətinə etibar etmir
kod generasiyası.

| Yol | Məqsəd | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}`-dən yaradılan kanonik şəkil; GPU qurmaları üçün həqiqət mənbəyi. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Swift vahid testləri və XCFramework tüstü kəməri Metal ləpələrinin gözlədiyi eyni sabitləri yükləyərək kanonik görüntünü əks etdirir. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin qurğuları paritet və seriallaşdırma testləri üçün eyni manifest paylaşır. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

Hər bir istehlakçı sabitləri GPU-ya köçürməzdən əvvəl hashı yoxlamalıdır
boru kəməri. Manifest dəyişdikdə (yeni parametrlər dəsti və ya profil), SHA və
aşağı axın güzgüləri kilidləmə addımında yenilənməlidir.

## Regenerasiya

Manifest `xtask` işlətməklə Rust mənbələrindən yaradılıb.
köməkçi. Komanda həm kanonik faylı, həm də SDK güzgülərini yazır:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

Təyinatları ləğv etmək üçün `--constants <path>`/`--vectors <path>` istifadə edin və ya
`--no-sdk-mirror` yalnız kanonik görüntünü bərpa edərkən. Köməkçi olacaq
bayraq buraxıldıqda artefaktları Swift və Android ağaclarına əks etdirin,
bu, hashləri CI üçün uyğunlaşdırılmış saxlayır.

## Metal/CUDA konstruksiyalarının qidalanması

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` və
  `crates/fastpq_prover/cuda/fastpq_cuda.cu`-dən bərpa edilməlidir
  cədvəl dəyişdikdə görünür.
- Dairəvi və MDS sabitləri bitişik `MTLBuffer`/`__constant` halına salınır
  manifest tərtibinə uyğun olan seqmentlər: `round_constants[round][state_width]`
  ardınca 3x3 MDS matrisi.
- `fastpq_prover::poseidon_manifest()` anlık şəkli yükləyir və təsdiqləyir
  iş vaxtı (Metalın istiləşməsi zamanı) belə ki, diaqnostik alətlər bunu təsdiq edə bilər
  şeyder sabitləri vasitəsilə dərc edilmiş hashla uyğun gəlir
  `fastpq_prover::poseidon_manifest_sha256()`.
- SDK qurğu oxuyucuları (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) və
  Norito oflayn alətlər yalnız GPU-nun qarşısını alan eyni manifestə əsaslanır.
  parametr çəngəlləri.

## Doğrulama

1. Manifesti bərpa etdikdən sonra `cargo test -p xtask`-i işə salın
   Poseidon armatur istehsal vahidi testləri.
2. Yeni SHA-256-nı bu sənəddə və monitorinq edən istənilən idarə panelində qeyd edin
   GPU artefaktları.
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` təhlil edir
   `poseidon2.metal` və `fastpq_cuda.cu` tikinti zamanı və iddia edir ki, onların
   Seriallaşdırılmış sabitlər CUDA/Metal cədvəllərini saxlayaraq manifestə uyğun gəlir
   kilid addımında kanonik snapshot.Manifesti GPU qurma təlimatları ilə yanaşı saxlamaq Metal/CUDA verir
deterministik əl sıxma iş axınları: nüvələr yaddaşlarını optimallaşdırmaqda sərbəstdir
paylaşılan sabitləri qəbul etdikcə və hashı ifşa etdikcə layout
paritet yoxlamaları üçün telemetriya.