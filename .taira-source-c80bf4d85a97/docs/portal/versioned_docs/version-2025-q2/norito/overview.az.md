---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2025-12-29T18:16:35.906407+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito Baxış

Norito Iroha-də istifadə olunan ikili serializasiya qatıdır: o, verilənlərin necə olduğunu müəyyən edir.
strukturlar naqildə kodlanır, diskdə saxlanılır və onlar arasında mübadilə edilir
müqavilələr və ev sahibləri. İş yerindəki hər qutu əvəzinə Norito-ə əsaslanır
`serde` beləliklə, müxtəlif aparatdakı həmyaşıdlar eyni bayt istehsal edir.

Bu icmal əsas hissələri ümumiləşdirir və kanonik istinadlara keçid verir.

## Bir baxışda memarlıq

- **Başlıq + faydalı yük** – Hər bir Norito mesajı xüsusiyyət danışıqları ilə başlayır
  başlıq (bayraqlar, yoxlama məbləği) və ardınca çılpaq faydalı yük. Paketlənmiş planlar və
  sıxılma başlıq bitləri vasitəsilə müzakirə edilir.
- **Deterministik kodlaşdırma** – `norito::codec::{Encode, Decode}` həyata keçirir
  çılpaq kodlaşdırma. Yükləri başlıqlara yığarkən eyni tərtibat təkrar istifadə olunur
  hashing və imzalama deterministik olaraq qalır.
- **Sxem + törədir** – `norito_derive` `Encode`, `Decode` və
  `IntoSchema` tətbiqləri. Paketli strukturlar/ardıcıllıqlar defolt olaraq aktivdir
  və `norito.md`-də sənədləşdirilmişdir.
- **Multicodec reyestri** – Haşlar, əsas növlər və faydalı yük üçün identifikatorlar
  deskriptorlar `norito::multicodec`-də yaşayır. Səlahiyyətli cədvəldir
  `multicodec.md`-də saxlanılır.

## Alətlər

| Tapşırıq | Komanda / API | Qeydlər |
| --- | --- | --- |
| Başlıq/bölmələri yoxlayın | `ivm_tool inspect <file>.to` | ABI versiyasını, bayraqları və giriş nöqtələrini göstərir. |
| Rust-da kodla/şifrəni aç | `norito::codec::{Encode, Decode}` | Bütün əsas məlumat modeli növləri üçün həyata keçirilir. |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | Norito dəyərləri ilə dəstəklənən deterministik JSON. |
| Sənədlər/spesifikasiyalar yaradın | `norito.md`, `multicodec.md` | Repo kökündəki həqiqət mənbəyi sənədləri. |

## İnkişaf iş axını

1. **Törəmə əlavə et** – Yeni məlumatlar üçün `#[derive(Encode, Decode, IntoSchema)]`-ə üstünlük verin
   strukturlar. Zəruri olmadıqda, əl ilə yazılmış serializatorlardan çəkinin.
2. **Yüklənmiş planları təsdiq edin** – `cargo test -p norito` (və qablaşdırılan) istifadə edin
   yenisini təmin etmək üçün `scripts/run_norito_feature_matrix.sh`-də xüsusiyyət matrisi
   planlar sabit qalır.
3. **Sənədləri bərpa edin** – Kodlaşdırma dəyişdikdə, `norito.md` və
   multicodec cədvəli, sonra portal səhifələrini yeniləyin (`/reference/norito-codec`
   və bu ümumi baxış).
4. **Norito-birinci testləri saxlayın** – İnteqrasiya testləri Norito JSON-dan istifadə etməlidir.
   `serde_json` əvəzinə köməkçilər istehsalla eyni yollardan istifadə edirlər.

## Sürətli bağlantılar

- Spesifikasiya: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multikodek təyinatları: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Xüsusiyyət matrisi skripti: `scripts/run_norito_feature_matrix.sh`
- Paketli tərtibat nümunələri: `crates/norito/tests/`

Bu icmalı sürətli başlanğıc bələdçisi (`/norito/getting-started`) ilə birləşdirin.
Norito istifadə edən bayt kodunu tərtib etmək və işlətmək üçün praktiki təlimat
faydalı yüklər.