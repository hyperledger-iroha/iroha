---
lang: az
direction: ltr
source: docs/source/crypto/sm_risk_register.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba5f4fdc9221210a793fd0c2120d8cfb68487d7ddcbe67c208976798446ca5db
source_last_modified: "2025-12-29T18:16:35.945760+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 aktivləşdirilməsi üçün SM proqramı risk reyestri.

# SM Proqramı Risk Qeydiyyatı

Son yeniləmə: 2025-03-12.

Bu registr `sm_program.md`-dəki xülasəni genişləndirir və hər bir riski birləşdirir.
sahiblik, monitorinq tetikleyicileri və cari yumşaldılma vəziyyəti. The Crypto WG
və Əsas Platforma rəhbərləri bu reyestri həftəlik SM ritmində nəzərdən keçirir; dəyişikliklər
həm burada, həm də ictimai yol xəritəsində öz əksini tapmışdır.

## Risk Xülasəsi

| ID | Risk | Kateqoriya | Ehtimal | Təsir | Ciddilik | Sahibi | Azaldılması | Status | Tətiklər |
|----|------|----------|-------------|--------|----------|-------|------------|--------|----------|
| R1 | RustCrypto SM qutuları üçün xarici audit validator GA | imzalamadan əvvəl icra olunmayıb Təchizat zənciri | Orta | Yüksək | Yüksək | Kripto WG | Bits/NCC Qrupunun Müqavilə İzi, hesabat qəbul edilənə qədər yalnız doğrulama mövqeyini saxlayın | Təsirlərin azaldılması davam edir | SOW auditi 2025-04-15 tarixində imzasızdır və ya audit hesabatı 2025-06-01 tarixinə qədər gecikir |
| R2 | SDK-lar arasında deterministik olmayan reqressiyalar | İcra | Orta | Yüksək | Yüksək | SDK Proqram Rəhbərləri | Qurğuları SDK CI-də paylaşın, kanonik r∥s kodlamasını tətbiq edin, SDK arası saxtakarlıq testləri əlavə edin | Monitorinq | SM qurğuları olmadan CI və ya SDK buraxılışında fikstür sürüşməsi aşkar edildi |
| R3 | İntrinsiklərdə ISA-ya xas səhvlər (NEON/SIMD) | Performans | Aşağı | Orta | Orta | Performans WG | Xüsusiyyət bayraqlarının arxasındakı Gate intrinsics, ARM-də CI əhatəsini tələb edir, skalyar geri dönüşü qoruyur | Təsirlərin azaldılması davam edir | NEON skamyaları uğursuz olur və ya SM perf matrisində aşkar edilmiş aparat reqresiyası |
| R4 | Uyğunluq qeyri-müəyyənliyi SM qəbulunu gecikdirir | İdarəetmə | Orta | Orta | Orta | Sənədlər və Hüquqi Əlaqə | GA-dan əvvəl uyğunluq haqqında qısa məlumatı, operator yoxlama siyahısını, hüquq məsləhətçisi ilə əlaqəni dərc edin | Təsirlərin azaldılması davam edir | 01.05.2025 tarixindən sonra qanuni baxış gözlənilmir və ya yoxlanış siyahısı yeniləmələri yoxdur |
| R5 | Provayder yeniləmələri ilə FFI backend drift | İnteqrasiya | Orta | Orta | Orta | Platforma Əməliyyatları | Provayder versiyalarını bağlayın, paritet testləri əlavə edin, OpenSSL/Tongsuo önizləməsini daxil edin | Monitorinq | Paket güncəlləməsi pilot əhatə dairəsindən kənarda paritet işləmədən və ya önizləmədən aktivləşdirildi |

## Təkmilləşdirməni nəzərdən keçirin

- Həftəlik Crypto WG sinxronizasiyası (daimi gündəm məsələsi).
- Uyğunluğu təsdiqləmək üçün Platforma Əməliyyatları və Sənədlər ilə aylıq birgə baxış.
- Buraxılışdan əvvəl yoxlama məntəqəsi: risk reyestrinin dondurulması və GA ilə birlikdə attestasiya
  artefaktlar.

## Çıxış

| Rol | Nümayəndə | Tarix | Qeydlər |
|------|----------------|------|-------|
| Kripto WG Aparıcı | (faylda imza) | 2025-03-12 | Nəşr üçün təsdiqləndi və WG-nin geridə qalması ilə paylaşıldı. |
| Əsas Platforma Rəhbəri | (faylda imza) | 2025-03-12 | Qəbul edilmiş yumşaldılmalar və monitorinq kadansı. |

Tarixi təsdiqlər və iclas protokolları üçün baxın `docs/source/crypto/sm_program.md`
(`Communication Plan`) və Crypto WG-dən əlaqəli SM gündəliyi arxivi
iş sahəsi.